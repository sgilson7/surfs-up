//! A surfer on the course: one tick of movement, then the map's triggers, the
//! start zone and the timer, and the numbers a surf timer keeps (sync,
//! strafes, gain). The same `step` runs the local player, every replayed
//! ghost and every live surfer resimulated from another browser's inputs, so
//! all three move by exactly the same rule.

use crate::map::{box_touches, Map, TriggerKind};
use crate::movement::{Body, Cmd, MoveEvents, Mover, B_LEFT, B_RESET, B_RIGHT, HULL_MINS};
use crate::vec3::{angle_vectors, v3, wrap180, V3, ZERO};

/// What a teleport target means on this course.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Fell into a foam pit: back to the start, the run is over.
    Fall,
    /// A door back to the start.
    Restart,
    /// One of the course's six endings.
    Ending(u8),
}

/// The six endings of surf_utopia, by the name of the jail room the original
/// sent you to. Two mirrored doors share a room, so they share an ending.
pub const ENDINGS: [&str; 6] = [
    "jail_mid1|jail_mid2",
    "jail_rear",
    "jail_top",
    "jail_control_room1",
    "jail_bottom1",
    "jail_center",
];

/// The endings on the last wall of the course, where the lowest ramp runs
/// out: reaching one of these finishes a timed run. The other three are
/// early exits partway down, and a run that takes one is not timed.
pub const FINISH_ENDINGS: [u8; 3] = [3, 4, 5];

pub fn role_of(target: &str) -> Role {
    match target {
        "jail1" => Role::Fall,
        "start" => Role::Restart,
        t => {
            let i = ENDINGS
                .iter()
                .position(|e| e.split('|').any(|n| n == t))
                .unwrap_or(0);
            Role::Ending(i as u8)
        }
    }
}

/// Horizontal speed allowed inside the start zone (shavit's default prespeed
/// limit, 260 + 30).
pub const START_SPEED_CAP: f32 = 290.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zone {
    pub mins: V3,
    pub maxs: V3,
}

impl Zone {
    pub fn contains_box(&self, lo: V3, hi: V3) -> bool {
        hi.x >= self.mins.x
            && lo.x <= self.maxs.x
            && hi.y >= self.mins.y
            && lo.y <= self.maxs.y
            && hi.z >= self.mins.z
            && lo.z <= self.maxs.z
    }
}

/// The start platform of surf_utopia: the ledge you drop off into the first
/// trough (the TF2 map's no-build volume over it, from the BSP).
pub fn start_zone(map: &Map) -> Zone {
    let s = map.spawn;
    Zone {
        mins: v3(-14336.0, -1088.0, s.z - 80.0),
        maxs: v3(-13760.0, 1088.0, s.z + 240.0),
    }
}

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
}

impl Surfer {
    pub fn at_start(map: &Map) -> Surfer {
        Surfer {
            body: Body::at(map.spawn),
            phase: Phase::Start,
            stats: Stats::default(),
            last_cmd: Cmd::new(0, map.spawn_yaw, 0.0),
            snap_yaw: Some(map.spawn_yaw),
        }
    }

    pub fn from_body(body: Body, cmd: Cmd) -> Surfer {
        Surfer {
            body,
            phase: Phase::Running,
            stats: Stats::default(),
            last_cmd: cmd,
            snap_yaw: None,
        }
    }

    fn back_to_start(&mut self, map: &Map) {
        self.body = Body::at(map.spawn);
        self.phase = Phase::Start;
        self.stats = Stats::default();
        self.snap_yaw = Some(map.spawn_yaw);
    }

    /// One tick. `mv` carries the map; `zone` is the start zone.
    pub fn step(&mut self, mv: &mut Mover, zone: &Zone, cmd: Cmd) -> Tick {
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
        let maxs = self.body.maxs();
        let a = self.body.origin;
        let lo = v3(before.x.min(a.x), before.y.min(a.y), before.z.min(a.z)) + HULL_MINS - v3(0.0, 0.0, 2.0);
        let hi = v3(before.x.max(a.x), before.y.max(a.y), before.z.max(a.z)) + maxs;
        let mut push = ZERO;
        let mut pushed = false;
        let mut role: Option<Role> = None;
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
            if !box_touches(&t.planes, lo, hi) {
                continue;
            }
            match t.kind {
                TriggerKind::Push(dir, speed) => {
                    push += dir * speed;
                    pushed = true;
                }
                TriggerKind::Teleport(i) => {
                    let r = role_of(map.target_name(i));
                    // an ending outranks a fall in the same tick
                    role = match (role, r) {
                        (Some(Role::Ending(e)), _) => Some(Role::Ending(e)),
                        (_, r) => Some(r),
                    };
                }
            }
        }
        if pushed {
            self.body.base_velocity = push;
            out.boosted = !self.body.pushed;
        }
        self.body.pushed = pushed;

        match role {
            Some(Role::Ending(e)) => {
                if self.phase == Phase::Running && FINISH_ENDINGS.contains(&e) {
                    out.finished = Some((e, self.stats.ticks));
                } else {
                    out.exited = Some(e);
                }
                self.back_to_start(map);
                out.teleported = true;
            }
            Some(Role::Fall) | Some(Role::Restart) => {
                out.fell = role == Some(Role::Fall);
                self.back_to_start(map);
                out.teleported = true;
            }
            None => {}
        }

        if !out.teleported {
            let in_zone = zone.contains_box(self.body.origin + HULL_MINS, self.body.origin + maxs);
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
    use crate::map::tests::utopia;
    use crate::movement::{B_FORWARD, B_JUMP};

    #[test]
    fn roles_cover_the_six_endings() {
        assert_eq!(role_of("jail1"), Role::Fall);
        assert_eq!(role_of("start"), Role::Restart);
        assert_eq!(role_of("jail_mid2"), Role::Ending(0));
        assert_eq!(role_of("jail_center"), Role::Ending(5));
        let m = utopia();
        let mut seen = [false; 6];
        for t in &m.triggers {
            if let TriggerKind::Teleport(i) = t.kind {
                if let Role::Ending(e) = role_of(m.target_name(i)) {
                    seen[e as usize] = true;
                }
            }
        }
        assert_eq!(seen, [true; 6], "every ending has a door on the map");
    }

    #[test]
    fn the_clock_starts_when_you_leave_the_platform() {
        let m = utopia();
        let mut mv = Mover::new(m);
        let zone = start_zone(m);
        let mut s = Surfer::at_start(m);
        let mut started = None;
        for i in 0..640 {
            let t = s.step(&mut mv, &zone, Cmd::new(B_FORWARD, 0.0, 0.0));
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
    fn falling_into_a_pit_sends_you_back_to_the_start() {
        let m = utopia();
        let mut mv = Mover::new(m);
        let zone = start_zone(m);
        let mut s = Surfer::at_start(m);
        // just above a fall trigger's top face
        let pit = m
            .triggers
            .iter()
            .find(|t| {
                matches!(t.kind, TriggerKind::Teleport(i) if m.target_name(i) == "jail1")
                    && t.mins.z == 8192.0
                    && t.maxs.x > 5000.0
            })
            .expect("the pit under the first section");
        s.phase = Phase::Running;
        s.body.origin = v3(-4000.0, 0.0, pit.maxs.z + 30.0);
        let mut fell = false;
        for _ in 0..64 {
            let t = s.step(&mut mv, &zone, Cmd::new(0, 0.0, 0.0));
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
    fn sync_counts_turning_into_the_held_key() {
        let m = utopia();
        let mut mv = Mover::new(m);
        let zone = start_zone(m);
        let mut s = Surfer::from_body(Body::at(v3(0.0, 0.0, 14000.0)), Cmd::new(0, 0.0, 0.0));
        s.body.velocity = v3(500.0, 0.0, 0.0);
        let mut yaw = 0.0;
        for i in 0..40 {
            // turn left holding A for 20 ticks, then left holding D (bad)
            yaw += 2.0;
            let key = if i < 20 { B_LEFT } else { B_RIGHT };
            s.step(&mut mv, &zone, Cmd::new(key | B_JUMP, yaw, 0.0));
        }
        assert_eq!(s.stats.sync_total, 40);
        assert_eq!(s.stats.sync_good, 20);
        assert_eq!(s.stats.strafes, 2);
        assert!((s.stats.sync_pct() - 50.0).abs() < 0.01);
    }
}
