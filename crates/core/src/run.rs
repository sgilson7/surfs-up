//! A surfer on the course: one tick of movement, then the map's triggers, the
//! start zone and the timer, and the numbers a surf timer keeps (sync,
//! strafes, gain). The same `step` runs the local player, every replayed
//! ghost and every live surfer resimulated from another browser's inputs, so
//! all three move by exactly the same rule.

use crate::course::{Course, Role};
use crate::map::{box_touches, Map, TriggerKind, NO_FILTER};
use crate::movement::{Body, Cmd, MoveEvents, Mover, B_LEFT, B_RESET, B_RIGHT, HULL_MINS};
use crate::vec3::{angle_vectors, v3, wrap180, V3, ZERO};

/// Horizontal speed allowed inside the start zone (shavit's default prespeed
/// limit, 260 + 30).
pub const START_SPEED_CAP: f32 = 290.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// On the start platform; the clock has not started.
    Start,
    /// Surfing; the clock runs.
    Running,
}

/// Running tallies a surf timer shows.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    pub ticks: u32,
    /// shavit's sync: airborne ticks with mouse movement, and those where the
    /// held strafe key matched the turn.
    pub sync_total: u32,
    pub sync_good: u32,
    pub strafes: u32,
    pub jumps: u32,
    /// SSJ's gain: summed per-tick coefficient, and the airborne ticks with a strafe key.
    pub gain_sum: f32,
    pub gain_ticks: u32,
    pub top_speed: f32,
}

impl Stats {
    pub fn sync_pct(&self) -> f32 {
        if self.sync_total == 0 {
            0.0
        } else {
            self.sync_good as f32 * 100.0 / self.sync_total as f32
        }
    }
    pub fn gain_pct(&self) -> f32 {
        if self.gain_ticks == 0 {
            0.0
        } else {
            self.gain_sum * 100.0 / self.gain_ticks as f32
        }
    }
}

/// What a tick did, for the run, the sound and the screen.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tick {
    pub moved: MoveEvents,
    pub teleported: bool,
    pub fell: bool,
    pub started: bool,
    pub finished: Option<(u8, u32)>,
    pub exited: Option<u8>,
    pub boosted: bool,
    /// A new stage reached (0 is the first).
    pub stage: Option<u8>,
}

/// A surfer: the body, where it is in a run, and the run's tallies.
#[derive(Clone, Debug, PartialEq)]
pub struct Surfer {
    pub body: Body,
    pub phase: Phase,
    pub stats: Stats,
    pub last_cmd: Cmd,
    /// The yaw a teleport turned the view to; the page's view follows it.
    pub snap_yaw: Option<f32>,
    /// The name the map has given this surfer (a name index, 0 for none):
    /// how a map's secrets remember where you have been.
    pub name: u16,
    /// The furthest stage reached this run (0 is the first).
    pub stage: u8,
}

impl Surfer {
    pub fn at_start(map: &Map) -> Surfer {
        Surfer {
            body: Body::at(map.spawn),
            phase: Phase::Start,
            stats: Stats::default(),
            last_cmd: Cmd::new(0, map.spawn_yaw, 0.0),
            snap_yaw: Some(map.spawn_yaw),
            name: 0,
            stage: 0,
        }
    }

    pub fn from_body(body: Body, cmd: Cmd) -> Surfer {
        Surfer {
            body,
            phase: Phase::Running,
            stats: Stats::default(),
            last_cmd: cmd,
            snap_yaw: None,
            name: 0,
            stage: 0,
        }
    }

    fn back_to_start(&mut self, map: &Map) {
        self.body = Body::at(map.spawn);
        self.phase = Phase::Start;
        self.stats = Stats::default();
        self.snap_yaw = Some(map.spawn_yaw);
        self.name = 0;
        self.stage = 0;
    }

    /// One tick. `mv` carries the map; `course` says what its teleports mean.
    pub fn step(&mut self, mv: &mut Mover, course: &Course, cmd: Cmd) -> Tick {
        let map = mv.map;
        let mut out = Tick::default();
        if cmd.has(B_RESET) && !self.last_cmd.has(B_RESET) {
            self.back_to_start(map);
            out.teleported = true;
            self.last_cmd = cmd;
            return out;
        }
        let before = self.body.origin;
        let airborne = !self.body.on_ground;
        let vel_before = self.body.velocity;
        out.moved = mv.player_move(&mut self.body, cmd);
        if self.phase == Phase::Running {
            self.tally(mv, cmd, airborne, vel_before, out.moved.jumped);
        }

        // Triggers fire along the whole swept box (not only where the move
        // ended), and reach 2 units under the feet, so a fast surfer cannot
        // pass through a thin trigger (RNGFix's telehop and trigger-jump fixes).
        // A filtered trigger fires only for a surfer with the name its filter
        // wants; a rename takes effect after this tick's teleports, as an
        // output fires after the touch that caused it.
        let maxs = self.body.maxs();
        let a = self.body.origin;
        let lo = v3(before.x.min(a.x), before.y.min(a.y), before.z.min(a.z)) + HULL_MINS - v3(0.0, 0.0, 2.0);
        let hi = v3(before.x.max(a.x), before.y.max(a.y), before.z.max(a.z)) + maxs;
        let mut push = ZERO;
        let mut pushed = false;
        let mut jump: Option<(Role, u16)> = None;
        let mut rename: Option<u16> = None;
        for t in &map.triggers {
            if t.maxs.x < lo.x
                || t.mins.x > hi.x
                || t.maxs.y < lo.y
                || t.mins.y > hi.y
                || t.maxs.z < lo.z
                || t.mins.z > hi.z
            {
                continue;
            }
            if t.filter != NO_FILTER && t.filter != self.name {
                continue;
            }
            if !box_touches(&t.planes, lo, hi) {
                continue;
            }
            match t.kind {
                TriggerKind::Push(dir, speed) => {
                    push += dir * speed;
                    pushed = true;
                }
                TriggerKind::Rename(n) => rename = Some(n),
                TriggerKind::Teleport(d) => {
                    let r = course.role(t, d);
                    // a finish outranks anything else touched in the same tick
                    let keep = matches!(jump, Some((Role::Finish(_), _)));
                    if !keep {
                        jump = Some((r, d));
                    }
                }
            }
        }
        if pushed {
            self.body.base_velocity = push;
            out.boosted = !self.body.pushed;
        }
        self.body.pushed = pushed;

        match jump {
            Some((Role::Finish(e), _)) => {
                if self.phase == Phase::Running {
                    out.finished = Some((e, self.stats.ticks));
                } else {
                    out.exited = Some(e);
                }
                self.back_to_start(map);
                out.teleported = true;
            }
            Some((Role::Exit(e), _)) => {
                out.exited = Some(e);
                self.back_to_start(map);
                out.teleported = true;
            }
            Some((Role::Restart, _)) => {
                out.fell = true;
                self.back_to_start(map);
                out.teleported = true;
            }
            Some((Role::Move, d)) => {
                if let Some((p, yaw)) = map.destination(d) {
                    // Source's trigger_teleport: the new place and facing, no velocity
                    self.body.origin = p;
                    self.body.velocity = ZERO;
                    self.body.base_velocity = ZERO;
                    self.body.pushed = false;
                    self.snap_yaw = Some(yaw);
                    match course.stage_of(d) {
                        Some(st) if st > self.stage || (st == 0 && self.phase == Phase::Start) => {
                            self.stage = st;
                            out.stage = Some(st);
                        }
                        Some(st) if st == self.stage => out.fell = true,
                        _ => {}
                    }
                }
            }
            None => {}
        }
        if let Some(n) = rename {
            self.name = n;
        }

        let restarted = out.teleported;
        out.teleported |= matches!(jump, Some((Role::Move, _)));
        if !restarted {
            let maxs = self.body.maxs();
            let in_zone = course
                .zone
                .contains_box(self.body.origin + HULL_MINS, self.body.origin + maxs);
            if in_zone {
                if self.phase == Phase::Running {
                    self.stats = Stats::default();
                }
                self.phase = Phase::Start;
                let v = self.body.velocity;
                let h = v.length2d();
                if h > START_SPEED_CAP {
                    let k = START_SPEED_CAP / h;
                    self.body.velocity = v3(v.x * k, v.y * k, v.z);
                }
            } else if self.phase == Phase::Start {
                self.phase = Phase::Running;
                self.stats = Stats::default();
                out.started = true;
            }
        }
        self.last_cmd = cmd;
        out
    }

    fn tally(&mut self, mv: &Mover, cmd: Cmd, airborne: bool, vel: V3, jumped: bool) {
        let s = &mut self.stats;
        s.ticks += 1;
        if jumped {
            s.jumps += 1;
        }
        let speed = self.body.velocity.length2d();
        if speed > s.top_speed {
            s.top_speed = speed;
        }
        if !airborne {
            return;
        }
        let (_, side) = cmd.moves(&mv.t);
        let yaw_delta = wrap180(cmd.yaw_deg() - self.last_cmd.yaw_deg());
        if yaw_delta != 0.0 && vel.length2d() > 0.0 {
            s.sync_total += 1;
            if (yaw_delta > 0.0 && side < 0.0) || (yaw_delta < 0.0 && side > 0.0) {
                s.sync_good += 1;
            }
        }
        let pressed = |b: u8| cmd.has(b) && !self.last_cmd.has(b);
        if pressed(B_LEFT) || pressed(B_RIGHT) {
            s.strafes += 1;
        }
        // SSJ gain: how close the wish direction stayed to perpendicular
        if side != 0.0 {
            let (f, r, _) = angle_vectors(0.0, cmd.yaw_deg());
            let (fm, _) = cmd.moves(&mv.t);
            let mut wish = f * fm + r * side;
            wish.z = 0.0;
            let ws = wish.normalize().min(mv.t.max_speed);
            if ws > 0.0 {
                let cap = ws.min(mv.t.air_wish_cap);
                let u = v3(vel.x, vel.y, 0.0).dot(wish);
                s.gain_ticks += 1;
                if u < cap {
                    s.gain_sum += ((cap - u.abs()) / cap).max(0.0);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::course::tests::load;
    use crate::movement::{B_FORWARD, B_JUMP};

    #[test]
    fn the_clock_starts_when_you_leave_the_platform() {
        let (m, c) = load("surf_utopia");
        let mut mv = Mover::new(&m);
        let mut s = Surfer::at_start(&m);
        let mut started = None;
        for i in 0..640 {
            let t = s.step(&mut mv, &c, Cmd::new(B_FORWARD, 0.0, 0.0));
            if t.started {
                started = Some(i);
                break;
            }
            assert_eq!(s.phase, Phase::Start);
            assert!(s.body.velocity.length2d() <= START_SPEED_CAP + 0.01);
        }
        assert!(
            started.is_some(),
            "walking forward leaves the start at {:?}",
            s.body.origin
        );
    }

    #[test]
    fn falling_into_a_utopia_pit_sends_you_back_to_the_start() {
        let (m, c) = load("surf_utopia");
        let mut mv = Mover::new(&m);
        let mut s = Surfer::at_start(&m);
        let jail = m.name_index("jail1").unwrap();
        let pit = m
            .triggers
            .iter()
            .find(|t| t.kind == TriggerKind::Teleport(jail) && t.mins.z == 8192.0 && t.maxs.x > 5000.0)
            .expect("the pit under the first section");
        s.phase = Phase::Running;
        s.body.origin = v3(-4000.0, 0.0, pit.maxs.z + 30.0);
        let mut fell = false;
        for _ in 0..64 {
            let t = s.step(&mut mv, &c, Cmd::new(0, 0.0, 0.0));
            if t.fell {
                fell = true;
                break;
            }
        }
        assert!(fell);
        assert_eq!(s.phase, Phase::Start);
        assert_eq!(s.body.origin, m.spawn);
    }

    #[test]
    fn kitsune_the_red_door_starts_stage_one_and_a_fall_keeps_the_clock() {
        let (m, c) = load("surf_kitsune");
        let mut mv = Mover::new(&m);
        let mut s = Surfer::at_start(&m);
        assert_eq!(m.spawn_yaw, 90.0, "the spawn faces the red door (+y)");
        let mut started = false;
        for _ in 0..640 {
            let t = s.step(&mut mv, &c, Cmd::new(B_FORWARD, 90.0, 0.0));
            if t.started {
                started = true;
                assert_eq!(t.stage, Some(0));
                break;
            }
        }
        assert!(
            started,
            "through the red door into stage 1 at {:?}",
            s.body.origin
        );
        let (red, _) = m.destination_named("red").unwrap();
        assert_eq!(s.body.origin, red);
        // drop onto stage 1's pit: back to the red start, clock still running
        let pit = m
            .triggers
            .iter()
            .find(|t| {
                t.kind == TriggerKind::Teleport(m.name_index("red").unwrap())
                    && t.maxs.z - t.mins.z < 100.0
                    && t.maxs.x - t.mins.x > 500.0
            })
            .expect("stage 1's pit");
        s.body.origin = v3(
            (pit.mins.x + pit.maxs.x) * 0.5,
            (pit.mins.y + pit.maxs.y) * 0.5,
            pit.maxs.z + 20.0,
        );
        let ticks = s.stats.ticks;
        let mut fell = false;
        for _ in 0..64 {
            let t = s.step(&mut mv, &c, Cmd::new(0, 90.0, 0.0));
            if t.fell {
                fell = true;
                break;
            }
        }
        assert!(fell, "stage 1's pit is a fall");
        assert_eq!(s.phase, Phase::Running, "the clock keeps running after a fall");
        assert!(s.stats.ticks > ticks);
        assert_eq!(s.body.origin, red);
    }

    #[test]
    fn kitsune_secrets_remember_your_name() {
        let (m, c) = load("surf_kitsune");
        let mut mv = Mover::new(&m);
        // the shortcut out of stage 1 (to orange1) fires only for someone the
        // secret room behind the spawn has named "x"
        let orange1 = m.name_index("orange1").unwrap();
        let door = m
            .triggers
            .iter()
            .find(|t| t.kind == TriggerKind::Teleport(orange1))
            .expect("the shortcut");
        let mid = (door.mins + door.maxs) * 0.5 - v3(0.0, 0.0, 36.0);
        let mut s = Surfer::at_start(&m);
        s.phase = Phase::Running;
        s.body.origin = mid;
        s.step(&mut mv, &c, Cmd::new(0, 90.0, 0.0));
        assert_ne!(
            s.body.origin,
            m.destination(orange1).unwrap().0,
            "no name, no shortcut"
        );
        let mut named = Surfer::at_start(&m);
        named.phase = Phase::Running;
        named.name = m.name_index("x").unwrap();
        named.body.origin = mid;
        named.step(&mut mv, &c, Cmd::new(0, 90.0, 0.0));
        assert_eq!(
            named.body.origin,
            m.destination(orange1).unwrap().0,
            "named, the shortcut takes you"
        );
        // and the room that names you
        let x = m.name_index("x").unwrap();
        let namer = m
            .triggers
            .iter()
            .find(|t| t.kind == TriggerKind::Rename(x))
            .expect("the naming trigger");
        let mut n = Surfer::at_start(&m);
        n.body.origin = (namer.mins + namer.maxs) * 0.5 - v3(0.0, 0.0, 36.0);
        n.step(&mut mv, &c, Cmd::new(0, 90.0, 0.0));
        assert_eq!(n.name, x);
    }

    #[test]
    fn sync_counts_turning_into_the_held_key() {
        let (m, c) = load("surf_utopia");
        let mut mv = Mover::new(&m);
        let mut s = Surfer::from_body(Body::at(v3(0.0, 0.0, 14000.0)), Cmd::new(0, 0.0, 0.0));
        s.body.velocity = v3(500.0, 0.0, 0.0);
        let mut yaw = 0.0;
        for i in 0..40 {
            // turn left holding A for 20 ticks, then left holding D (bad)
            yaw += 2.0;
            let key = if i < 20 { B_LEFT } else { B_RIGHT };
            s.step(&mut mv, &c, Cmd::new(key | B_JUMP, yaw, 0.0));
        }
        assert_eq!(s.stats.sync_total, 40);
        assert_eq!(s.stats.sync_good, 20);
        assert_eq!(s.stats.strafes, 2);
        assert!((s.stats.sync_pct() - 50.0).abs() < 0.01);
    }
}
