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
/// Source's PLAYER_FATAL_FALL_SPEED (shareddefs.h, "approx 60 feet"): a
/// landing this fast takes all of a full 100 health.
pub const FATAL_FALL_SPEED: f32 = 1024.0;
/// A player's full health in Counter-Strike.
pub const FULL_HEALTH: f32 = 100.0;
/// Source's trigger_hurt deals its damage every half second while touched
/// (CTriggerHurt::HurtThink, `HurtAllTouchers(0.5)`, triggers.cpp): 32 ticks.
pub const HURT_EVERY: u16 = 32;

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
    /// Health, which only a map's hurt triggers take: at none, back to the start.
    pub health: f32,
    /// Ticks until a hurt trigger touched now hurts again (0: at once).
    pub hurt_wait: u16,
    /// A delayed rename on its way: the name, and the ticks until it lands.
    pub renaming: Option<(u16, u16)>,
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
            health: FULL_HEALTH,
            hurt_wait: 0,
            renaming: None,
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
            health: FULL_HEALTH,
            hurt_wait: 0,
            renaming: None,
        }
    }

    fn back_to_start(&mut self, map: &Map) {
        self.body = Body::at(map.spawn);
        self.phase = Phase::Start;
        self.stats = Stats::default();
        self.snap_yaw = Some(map.spawn_yaw);
        self.name = 0;
        self.stage = 0;
        self.health = FULL_HEALTH;
        self.hurt_wait = 0;
        self.renaming = None;
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
        out.moved = mv.player_move(&mut self.body, cmd, self.last_cmd);
        if self.phase == Phase::Running {
            self.tally(mv, cmd, airborne, vel_before, out.moved.jumped);
        }

        // Triggers fire along the whole swept box (not only where the move
        // ended), and reach 2 units under the feet, so a fast surfer cannot
        // pass through a thin trigger (RNGFix's telehop and trigger-jump fixes).
        // A filtered trigger fires only for a surfer with the name its filter
        // wants; a rename takes effect after this tick's teleports, as an
        // output fires after the touch that caused it. A delayed rename
        // lands that many ticks later, touched or not, as Source's delayed
        // output does (surf_summit's falls take a quarter second to name you).
        let maxs = self.body.maxs();
        let a = self.body.origin;
        let lo = v3(before.x.min(a.x), before.y.min(a.y), before.z.min(a.z)) + HULL_MINS - v3(0.0, 0.0, 2.0);
        let hi = v3(before.x.max(a.x), before.y.max(a.y), before.z.max(a.z)) + maxs;
        let mut push = ZERO;
        let mut pushed = false;
        let mut jump: Option<(Role, u16)> = None;
        let mut rename: Option<u16> = None;
        let mut hurt = 0.0f32;
        let mut hurting = false;
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
                TriggerKind::Rename(n, 0) => rename = Some(n),
                TriggerKind::Rename(n, wait) => {
                    if self.renaming.is_none() {
                        self.renaming = Some((n, wait));
                    }
                }
                TriggerKind::Hurt(per_second) => {
                    hurting = true;
                    hurt += per_second * HURT_EVERY as f32 / 64.0;
                }
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
        // a finish zone stops the clock like a finish door, swept as the
        // triggers are
        if !matches!(jump, Some((Role::Finish(_), _))) && self.phase == Phase::Running {
            if let Some(z) = &course.finish_zone {
                if z.contains_box(lo, hi) {
                    jump = Some((Role::Finish(1), 0));
                }
            }
        }
        // Source's trigger_hurt: the first touch hurts at once, then every
        // half second while touched; leaving it lets the next touch hurt at
        // once again. With no health left, back to the start.
        if hurting {
            if self.hurt_wait == 0 {
                self.health -= hurt;
                self.hurt_wait = HURT_EVERY;
            }
            self.hurt_wait -= 1;
        } else {
            self.hurt_wait = 0;
        }
        if jump.is_none() && self.health <= 0.0 {
            jump = Some((Role::Restart, 0));
        }
        // Source's CheckFalling with a full 100 health: a landing at
        // PLAYER_FATAL_FALL_SPEED (1024, shareddefs.h) or faster kills,
        // unless the feet are in water; here it is a fall back to the start
        if jump.is_none()
            && course.fall_damage
            && out.moved.landed
            && out.moved.fall_speed >= FATAL_FALL_SPEED
            && !map.in_water(self.body.origin + v3(0.0, 0.0, 1.0))
        {
            jump = Some((Role::Restart, 0));
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
                    // Source's trigger_teleport (triggers.cpp, Source SDK 2013):
                    // the new place and facing, and the velocity KEPT. Its
                    // comment says so: "BUGBUG - This does not set the player's
                    // velocity to zero!!!" Surf maps rely on it: a stage door
                    // carries your speed into the next stage.
                    self.body.origin = p;
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
        if let Some((n, wait)) = self.renaming {
            if wait <= 1 {
                self.name = n;
                self.renaming = None;
            } else {
                self.renaming = Some((n, wait - 1));
            }
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
                // a run starts whole (a recorded run replays from here)
                self.health = FULL_HEALTH;
                self.hurt_wait = 0;
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
    use crate::map::{Trigger, HINT_TERRAIN};
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
    fn sandtrap_stands_on_its_sand_and_finishes_at_its_end_door() {
        let (m, c) = load("surf_sandtrap");
        let mut mv = Mover::new(&m);
        // you stand in the start room
        let mut s = Surfer::at_start(&m);
        s.body.origin = v3(1072.0, -900.0, 14400.0);
        for _ in 0..200 {
            s.step(&mut mv, &c, Cmd::new(0, 0.0, 0.0));
            if s.body.on_ground {
                break;
            }
        }
        assert!(s.body.on_ground, "you stand in the start room");
        // a flat sand triangle with no brush within 300 units above or
        // under it:
        // the player's box, dropped on it (movement alone, no triggers),
        // comes to rest on the sand
        let tri = m
            .brushes
            .iter()
            .filter(|b| {
                b.sides.iter().any(|s| s.drawn && s.hint == HINT_TERRAIN) && b.sides[0].plane.n.z > 0.9
            })
            .find(|t| {
                !m.brushes.iter().any(|o| {
                    !o.sides.iter().any(|s| s.hint == HINT_TERRAIN)
                        && o.maxs.z > t.mins.z - 300.0
                        && o.mins.z < t.maxs.z + 300.0
                        && o.mins.x < t.maxs.x
                        && o.maxs.x > t.mins.x
                        && o.mins.y < t.maxs.y
                        && o.maxs.y > t.mins.y
                })
            })
            .expect("a sand triangle with nothing under it");
        let centre = (tri.mins + tri.maxs) * 0.5;
        let mut b = crate::movement::Body::at(v3(centre.x, centre.y, tri.maxs.z + 200.0));
        for _ in 0..300 {
            mv.player_move_same(&mut b, Cmd::new(0, 0.0, 0.0));
            if b.on_ground {
                break;
            }
        }
        assert!(b.on_ground, "the sand holds you ({:?})", b.origin);
        assert!(
            b.origin.z > tri.mins.z - 1.0 && b.origin.z < tri.maxs.z + 20.0,
            "on the sand at {:.0}..{:.0}, not through it ({:.1})",
            tri.mins.z,
            tri.maxs.z,
            b.origin.z
        );
        // the end door finishes a run; the other doors back to the spawn restart
        let start = m.name_index("start").unwrap();
        let end = m
            .triggers
            .iter()
            .find(|t| t.kind == TriggerKind::Teleport(start) && t.maxs.z < 0.0 && t.mins.y < -14000.0)
            .expect("the end door");
        let mut s = Surfer::at_start(&m);
        s.phase = Phase::Running;
        s.body.origin = (end.mins + end.maxs) * 0.5;
        let t = s.step(&mut mv, &c, Cmd::new(0, 0.0, 0.0));
        assert!(t.finished.is_some(), "the end door finishes the run");
    }

    #[test]
    fn anubis_runs_stage_one_to_three_and_finishes_in_its_end_zone() {
        let (m, c) = load("surf_anubis");
        let mut mv = Mover::new(&m);
        let mut s = Surfer::at_start(&m);
        assert_eq!(c.stages.len(), 3);
        let (s1, _) = m.destination_named("stage1").unwrap();
        assert_eq!(s.body.origin, s1, "you start on stage 1's pad");
        assert!(c.zone.contains(s1));
        // walk off stage 1's pad: the clock starts
        let mut started = false;
        for _ in 0..200 {
            if s.step(&mut mv, &c, Cmd::new(B_FORWARD, 0.0, 0.0)).started {
                started = true;
                break;
            }
        }
        assert!(started, "leaving stage 1's pad starts the clock");
        // stage 1's end door (a teleport to stage 2) moves you on, clock running
        let door = |target: &str| {
            let d = m.name_index(target).unwrap();
            m.triggers
                .iter()
                .find(|t| {
                    t.kind == TriggerKind::Teleport(d)
                        && (t.maxs.y - t.mins.y < 64.0 || t.maxs.x - t.mins.x < 64.0)
                        && t.maxs.z - t.mins.z > 100.0
                })
                .unwrap_or_else(|| panic!("a door to {target}"))
                .clone()
        };
        for (target, stage) in [("stage2", 1u8), ("stage3", 2u8)] {
            let d = door(target);
            s.body.origin = (d.mins + d.maxs) * 0.5 - v3(0.0, 0.0, 30.0);
            s.body.velocity = ZERO;
            let t = s.step(&mut mv, &c, Cmd::new(0, 0.0, 0.0));
            assert_eq!(t.stage, Some(stage), "through the door to {target}");
            assert_eq!(s.phase, Phase::Running);
        }
        // into the end zone: the clock stops
        let z = c.finish_zone.unwrap();
        s.body.origin = (z.mins + z.maxs) * 0.5;
        s.body.velocity = ZERO;
        let t = s.step(&mut mv, &c, Cmd::new(0, 0.0, 0.0));
        assert!(t.finished.is_some(), "the end zone finishes the run");
        assert_eq!(s.phase, Phase::Start);
    }

    #[test]
    fn on_egypt_a_fatal_fall_to_the_ground_restarts_and_water_is_safe() {
        let (m, c) = load("surf_egypt");
        assert!(c.fall_damage);
        let mut mv = Mover::new(&m);
        // dropped from 1000 units onto the start room's roof (its top at
        // z 5312, open to the sky): a fatal fall
        let mut s = Surfer::at_start(&m);
        s.phase = Phase::Running;
        s.body.origin = v3(510.0, 1705.0, 6312.0);
        let mut fell = false;
        for _ in 0..200 {
            let t = s.step(&mut mv, &c, Cmd::new(0, 0.0, 0.0));
            if t.fell {
                fell = true;
                break;
            }
            if s.body.on_ground {
                break;
            }
        }
        assert!(fell, "a 1000-unit drop lands at over 1024 u/s and restarts");
        assert_eq!(s.phase, Phase::Start);
        // a short hop down onto it does not hurt
        let mut s = Surfer::at_start(&m);
        s.phase = Phase::Running;
        s.body.origin = v3(510.0, 1705.0, 5512.0);
        for _ in 0..200 {
            let t = s.step(&mut mv, &c, Cmd::new(0, 0.0, 0.0));
            assert!(!t.fell, "a 200-unit drop is safe");
            if s.body.on_ground {
                break;
            }
        }
        assert!(s.body.on_ground);
        // the plain ground beside the pyramid's pool is under the map's
        // Killzone (a trigger_hurt of 6666 a second): even a short hop
        // down onto it ends the run
        let mut s = Surfer::at_start(&m);
        s.phase = Phase::Running;
        s.body.origin = v3(2000.0, 4000.0, 218.0);
        let mut fell = false;
        for _ in 0..200 {
            if s.step(&mut mv, &c, Cmd::new(0, 0.0, 0.0)).fell {
                fell = true;
                break;
            }
        }
        assert!(fell, "the Killzone over egypt's ground");
        // the same big drop into the pyramid's pool, beside the pyramid, is
        // safe (the pool's floor is under water)
        let mut s = Surfer::at_start(&m);
        s.phase = Phase::Running;
        s.body.origin = v3(3280.0, 5600.0, 1100.0);
        for _ in 0..200 {
            let t = s.step(&mut mv, &c, Cmd::new(0, 0.0, 0.0));
            assert!(!t.fell, "landing in water is safe");
            if s.body.on_ground {
                break;
            }
        }
        assert!(s.body.on_ground && s.phase == Phase::Running);
        // and utopia has no fall damage at all
        let (_, uc) = load("surf_utopia");
        assert!(!uc.fall_damage);
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
    fn a_stage_door_keeps_your_speed_as_source_does() {
        let (m, c) = load("surf_kitsune");
        let mut mv = Mover::new(&m);
        let orange = m.name_index("orange").unwrap();
        let door = m
            .triggers
            .iter()
            .find(|t| t.kind == TriggerKind::Teleport(orange) && t.maxs.z - t.mins.z > 100.0)
            .expect("stage 1's door to stage 2");
        let mut s = Surfer::at_start(&m);
        s.phase = Phase::Running;
        s.body.origin = v3(
            (door.mins.x + door.maxs.x) * 0.5,
            door.mins.y - 20.0,
            door.mins.z + 10.0,
        );
        s.body.velocity = v3(0.0, 1000.0, -100.0);
        let mut through = false;
        for _ in 0..8 {
            let t = s.step(&mut mv, &c, Cmd::new(0, 90.0, 0.0));
            if t.stage == Some(1) {
                through = true;
                break;
            }
        }
        assert!(through, "through the door into stage 2");
        assert!(
            s.body.velocity.y > 950.0,
            "the speed came along: {:?}",
            s.body.velocity
        );
        assert_eq!(s.phase, Phase::Running);
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
            .find(|t| t.kind == TriggerKind::Rename(x, 0))
            .expect("the naming trigger");
        let mut n = Surfer::at_start(&m);
        n.body.origin = (namer.mins + namer.maxs) * 0.5 - v3(0.0, 0.0, 36.0);
        n.step(&mut mv, &c, Cmd::new(0, 90.0, 0.0));
        assert_eq!(n.name, x);
    }

    #[test]
    fn aircontrols_hurt_triggers_hurt_every_half_second_and_its_kill_floors_at_once() {
        let (m, c) = load("surf_aircontrol");
        let mut mv = Mover::new(&m);
        let hurts = |dps: f32| {
            m.triggers
                .iter()
                .filter(move |t| t.kind == TriggerKind::Hurt(dps))
                .collect::<Vec<_>>()
        };
        // the plate of 100 a second: held in it, half a second's worth (50)
        // at once, the rest half a second later, and back to the start
        let plate = hurts(100.0)[0];
        let inside = (plate.mins + plate.maxs) * 0.5 - v3(0.0, 0.0, 30.0);
        let mut s = Surfer::at_start(&m);
        s.phase = Phase::Running;
        let mut died = None;
        for i in 0..64 {
            s.body.origin = inside;
            s.body.velocity = ZERO;
            if s.step(&mut mv, &c, Cmd::new(0, 0.0, 0.0)).fell {
                died = Some(i);
                break;
            }
            if i == 0 {
                assert_eq!(s.health, 50.0, "half a second's damage at the first touch");
            }
        }
        assert_eq!(
            died,
            Some(HURT_EVERY as i32),
            "the second hurt, half a second on, kills"
        );
        // stepping out and back in hurts again at once
        let mut s = Surfer::at_start(&m);
        s.phase = Phase::Running;
        for (i, at) in [inside, inside + v3(0.0, 0.0, 400.0), inside]
            .into_iter()
            .enumerate()
        {
            s.body.origin = at;
            s.body.velocity = ZERO;
            let t = s.step(&mut mv, &c, Cmd::new(0, 0.0, 0.0));
            assert_eq!(t.fell, i == 2, "tick {i}: health {}", s.health);
        }
        // a kill floor under the first ramps (1000 a second) ends a run at once
        let floor = hurts(1000.0)
            .into_iter()
            .find(|t| t.maxs.z - t.mins.z < 32.0 && t.mins.z > 12000.0)
            .expect("the kill floor under the first ramps");
        let mut s = Surfer::at_start(&m);
        s.phase = Phase::Running;
        s.body.origin = (floor.mins + floor.maxs) * 0.5 + v3(0.0, 0.0, 40.0);
        let mut fell = false;
        for _ in 0..64 {
            if s.step(&mut mv, &c, Cmd::new(0, 0.0, 0.0)).fell {
                fell = true;
                break;
            }
        }
        assert!(fell, "the kill floor");
        assert_eq!(s.body.origin, m.spawn);
    }

    #[test]
    fn aircontrol_starts_in_its_spawn_room_and_finishes_in_its_arena() {
        let (m, c) = load("surf_aircontrol");
        let mut mv = Mover::new(&m);
        // past the foot of the first ramp, in the gap beside it
        let mut s = Surfer::at_start(&m);
        s.body.origin = v3(-1680.0, -9600.0, 14340.0);
        let mut started = false;
        for _ in 0..640 {
            if s.step(&mut mv, &c, Cmd::new(B_FORWARD, 90.0, 0.0)).started {
                started = true;
                break;
            }
        }
        assert!(
            started,
            "walking out of the spawn room starts the clock at {:?}",
            s.body.origin
        );
        // dropped into the arena where the weapons lie, the clock stops
        s.body.origin = v3(12160.0, 3072.0, -12800.0);
        let mut finished = None;
        for _ in 0..200 {
            if let Some(f) = s.step(&mut mv, &c, Cmd::new(0, 0.0, 0.0)).finished {
                finished = Some(f);
                break;
            }
        }
        assert!(matches!(finished, Some((1, _))), "the arena is the end");
    }

    #[test]
    fn mesa_stands_on_its_canyon_and_its_end_tunnel_finishes() {
        let (m, c) = load("surf_mesa");
        let mut mv = Mover::new(&m);
        let mut s = Surfer::at_start(&m);
        for _ in 0..200 {
            s.step(&mut mv, &c, Cmd::new(0, 0.0, 0.0));
        }
        assert!(
            s.body.on_ground && s.phase == Phase::Start,
            "you stand on the spawn platform"
        );
        // the terrain, built from the displacement grids, holds you up
        let tri = m
            .brushes
            .iter()
            .find(|b| b.sides[0].hint == HINT_TERRAIN && b.sides[0].plane.n.z > 0.95)
            .expect("a flat bit of canyon floor");
        let mut d = Surfer::at_start(&m);
        d.phase = Phase::Running;
        d.body.origin = v3(
            (tri.mins.x + tri.maxs.x) * 0.5,
            (tri.mins.y + tri.maxs.y) * 0.5,
            tri.maxs.z + 8.0,
        );
        let mut stood = false;
        for _ in 0..64 {
            let mut b = d.body;
            mv.player_move(&mut b, Cmd::new(0, 0.0, 0.0), Cmd::new(0, 0.0, 0.0));
            d.body = b;
            if d.body.on_ground {
                stood = true;
                break;
            }
        }
        assert!(
            stood && d.body.origin.z > tri.mins.z - 1.0,
            "on the canyon at {:?}",
            d.body.origin
        );
        // the door at the end of the last tunnel finishes; every other
        // teleport to the spawn is a fall
        let spawn = m.name_index("spawn").unwrap();
        let mut ends = 0;
        let mut falls = 0;
        for t in m
            .triggers
            .iter()
            .filter(|t| t.kind == TriggerKind::Teleport(spawn))
        {
            match c.role(t, spawn) {
                Role::Finish(_) => ends += 1,
                Role::Restart => falls += 1,
                r => panic!("{r:?}"),
            }
        }
        assert_eq!(ends, 1, "one end door");
        assert!(falls > 8, "the falls (one record per brush): {falls}");
    }

    #[test]
    fn ambience_runs_from_the_green_pad_to_the_red_pad_and_its_first_pit_restarts() {
        let (m, c) = load("bhop_ambience");
        let mut mv = Mover::new(&m);
        let mut s = Surfer::at_start(&m);
        for _ in 0..64 {
            s.step(&mut mv, &c, Cmd::new(0, 270.0, 0.0));
        }
        assert!(s.body.on_ground && s.phase == Phase::Start, "on the start pad");
        // hop forward (auto-hop, holding jump) off the pad: the clock starts,
        // and the first pit sends you back to the pad
        let mut started = false;
        let mut fell = false;
        for _ in 0..640 {
            let t = s.step(&mut mv, &c, Cmd::new(B_FORWARD | B_JUMP, 270.0, 0.0));
            started |= t.started;
            if t.fell {
                fell = true;
                break;
            }
        }
        assert!(started && fell, "off the pad and into the first pit");
        assert_eq!((s.body.origin, s.phase), (m.spawn, Phase::Start));
        // landing on the red pad stops the clock
        s.phase = Phase::Running;
        s.body.origin = v3(2176.0, 600.0, 200.0);
        let mut finished = false;
        for _ in 0..200 {
            if s.step(&mut mv, &c, Cmd::new(0, 90.0, 0.0)).finished.is_some() {
                finished = true;
                break;
            }
        }
        assert!(finished, "the red pad is the end");
    }

    /// A teleport to `target` that is a thin, tall door (a stage's end), or
    /// failing that the largest one (a stage's fall).
    fn teleport_to(m: &Map, target: &str, door: bool) -> Trigger {
        let d = m.name_index(target).unwrap();
        let mut to = m.triggers.iter().filter(|t| t.kind == TriggerKind::Teleport(d));
        let size = |t: &&Trigger| {
            let e = t.maxs - t.mins;
            e.x * e.y * e.z
        };
        if door {
            to.find(|t| {
                (t.maxs.y - t.mins.y < 100.0 || t.maxs.x - t.mins.x < 100.0) && t.maxs.z - t.mins.z > 300.0
            })
            .unwrap_or_else(|| panic!("a door to {target}"))
            .clone()
        } else {
            to.max_by(|a, b| size(a).total_cmp(&size(b)))
                .unwrap_or_else(|| panic!("a fall to {target}"))
                .clone()
        }
    }

    #[test]
    fn summer_runs_its_eleven_stages_and_its_last_door_finishes() {
        let (m, c) = load("surf_summer");
        let mut mv = Mover::new(&m);
        let mut s = Surfer::at_start(&m);
        assert_eq!(c.stages.len(), 11);
        // its only pushes are jetski boosts for a player its stage 4 falls
        // have given another classname, which no plain player passes
        assert!(
            !m.triggers.iter().any(|t| matches!(t.kind, TriggerKind::Push(..))),
            "no boost for a plain player"
        );
        let (s1, yaw) = m.destination_named("s1").unwrap();
        assert_eq!(s.body.origin, s1, "you start on stage 1's pad");
        assert!(c.zone.contains(s1), "in the map's own timer start zone");
        let mut started = false;
        for _ in 0..300 {
            if s.step(&mut mv, &c, Cmd::new(B_FORWARD, yaw, 0.0)).started {
                started = true;
                break;
            }
        }
        assert!(started, "walking off stage 1's pad starts the clock");
        // each stage's end door moves you on with the clock running
        for (target, stage) in [("s2", 1u8), ("s3", 2u8)] {
            let d = teleport_to(&m, target, true);
            s.body.origin = (d.mins + d.maxs) * 0.5;
            s.body.velocity = ZERO;
            let t = s.step(&mut mv, &c, Cmd::new(0, 0.0, 0.0));
            assert_eq!(t.stage, Some(stage), "through the door to {target}");
            assert_eq!(s.phase, Phase::Running);
        }
        // a fall in stage 3 sends you back to stage 3's start, clock running
        let pit = teleport_to(&m, "s3", false);
        s.body.origin = (pit.mins + pit.maxs) * 0.5;
        s.body.velocity = ZERO;
        let t = s.step(&mut mv, &c, Cmd::new(0, 0.0, 0.0));
        assert!(t.fell, "a fall in stage 3");
        assert_eq!(
            (s.body.origin, s.stage, s.phase),
            (m.destination_named("s3").unwrap().0, 2, Phase::Running)
        );
        // the end of stage 11 takes you into the end room: the clock stops
        let end = teleport_to(&m, "timer_end", false);
        s.body.origin = (end.mins + end.maxs) * 0.5;
        s.body.velocity = ZERO;
        let t = s.step(&mut mv, &c, Cmd::new(0, 0.0, 0.0));
        assert!(t.finished.is_some(), "the end of stage 11 finishes the run");
    }

    #[test]
    fn deadline_falls_land_in_their_stage_and_its_end_zone_finishes() {
        let (m, c) = load("surf_deadline");
        let mut mv = Mover::new(&m);
        let mut s = Surfer::at_start(&m);
        assert_eq!(c.stages.len(), 4);
        let (s1, yaw) = m.destination_named("stage1").unwrap();
        assert_eq!(s.body.origin, s1, "you start on stage 1's pad");
        assert!(c.zone.contains(s1), "in the map's own start zone");
        let mut started = false;
        for _ in 0..300 {
            if s.step(&mut mv, &c, Cmd::new(B_FORWARD, yaw, 0.0)).started {
                started = true;
                break;
            }
        }
        assert!(started, "walking off stage 1's pad starts the clock");
        // stages run on into each other; a fall goes by a drop room whose
        // pad sends you to that stage's start: the first marks stage 2
        // reached, the next is a fall, and the clock runs throughout
        let (s2, _) = m.destination_named("stage2").unwrap();
        let drop = teleport_to(&m, "s2_fixdrop", false);
        let mut seen = Vec::new();
        for _ in 0..2 {
            s.body.origin = (drop.mins + drop.maxs) * 0.5;
            s.body.velocity = ZERO;
            let (mut stage, mut fell) = (None, false);
            for _ in 0..4 {
                let t = s.step(&mut mv, &c, Cmd::new(0, 0.0, 0.0));
                stage = stage.or(t.stage);
                fell |= t.fell;
                if s.body.origin == s2 {
                    break;
                }
            }
            assert_eq!(s.body.origin, s2, "the drop room's pad sends you to stage 2");
            seen.push((stage, fell));
        }
        assert_eq!(seen, [(Some(1), false), (None, true)]);
        assert_eq!((s.stage, s.phase), (1, Phase::Running));
        // into the end zone: the clock stops
        let z = c.finish_zone.unwrap();
        s.body.origin = (z.mins + z.maxs) * 0.5;
        s.body.velocity = ZERO;
        let t = s.step(&mut mv, &c, Cmd::new(0, 0.0, 0.0));
        assert!(t.finished.is_some(), "the end zone finishes the run");
    }

    #[test]
    fn beginner_runs_seven_stages_from_its_first_platform_to_its_end_room() {
        let (m, c) = load("surf_beginner");
        let mut mv = Mover::new(&m);
        let mut s = Surfer::at_start(&m);
        for _ in 0..200 {
            s.step(&mut mv, &c, Cmd::new(0, 90.0, 0.0));
        }
        assert!(
            s.body.on_ground && s.phase == Phase::Start,
            "on stage 1's platform at {:?}",
            s.body.origin
        );
        // walk off the platform: the clock starts, and the floor under the
        // first ramp sends you back to the platform, where it waits again
        let mut started = false;
        let mut fell = false;
        for _ in 0..1200 {
            let t = s.step(&mut mv, &c, Cmd::new(B_FORWARD, 90.0, 0.0));
            started |= t.started;
            if t.fell {
                fell = true;
                break;
            }
        }
        assert!(started && fell, "off the platform and onto the floor");
        assert_eq!((s.phase, s.stage), (Phase::Start, 0), "back in the start zone");
        // the door at the end of the first ramp is stage 2
        s.phase = Phase::Running;
        s.body.origin = v3(-128.0, 2004.0, -300.0);
        s.body.velocity = v3(0.0, 300.0, 0.0);
        let t = s.step(&mut mv, &c, Cmd::new(0, 90.0, 0.0));
        assert_eq!((t.stage, s.stage), (Some(1), 1), "stage 2 of 7");
        assert_eq!(c.stages.len(), 7);
        // and the last door, down the end room's toilet, finishes
        s.body.origin = v3(-5312.0, 5236.0, -160.0);
        let mut finished = false;
        for _ in 0..64 {
            if s.step(&mut mv, &c, Cmd::new(0, 90.0, 0.0)).finished.is_some() {
                finished = true;
                break;
            }
        }
        assert!(finished, "the end room");
        // the spawn room is not part of a run: its teleports restart
        let spawn_room = m.name_index("spawnman").unwrap();
        let t = m
            .triggers
            .iter()
            .find(|t| t.kind == TriggerKind::Teleport(spawn_room))
            .unwrap();
        assert_eq!(c.role(t, spawn_room), Role::Restart);
    }

    #[test]
    fn summits_falls_name_you_a_quarter_second_late_and_its_end_zone_finishes() {
        let (m, c) = load("surf_summit");
        let mut mv = Mover::new(&m);
        let mut s = Surfer::at_start(&m);
        for _ in 0..120 {
            s.step(&mut mv, &c, Cmd::new(0, 0.0, 0.0));
        }
        assert!(s.body.on_ground && s.phase == Phase::Start, "on the castle floor");
        // walk east off the castle and down its causeway: the clock starts,
        // and the drop at its end sends you back to the castle
        let mut started = false;
        let mut fell = false;
        for _ in 0..1200 {
            let t = s.step(&mut mv, &c, Cmd::new(B_FORWARD, 0.0, 0.0));
            started |= t.started;
            if t.fell {
                fell = true;
                break;
            }
        }
        assert!(started && fell, "off the castle and down the causeway's end");
        assert_eq!((s.body.origin, s.phase), (m.spawn, Phase::Start));
        // under the last section, a fall names you "filter_fail" after a
        // quarter second (the output's 0.24 s delay), and only then does its
        // teleport, which wants that name, send you back
        let fail = m.name_index("filter_fail").unwrap();
        let mut d = Surfer::at_start(&m);
        d.phase = Phase::Running;
        d.body.origin = v3(-1824.0, 864.0, -7128.0);
        let mut back_at = None;
        for i in 0..40 {
            if d.step(&mut mv, &c, Cmd::new(0, 0.0, 0.0)).fell {
                back_at = Some(i);
                break;
            }
            assert!(i < 14 || d.name == fail || d.renaming.is_some());
        }
        let back_at = back_at.expect("the fall sends you back");
        assert!(
            (14..=17).contains(&back_at),
            "not before a quarter second inside the fall: back at tick {back_at}"
        );
        // the timer's end zone (tm_mapend) stops the clock
        d.phase = Phase::Running;
        d.body.origin = v3(-14344.0, 0.0, -5120.0);
        assert!(
            d.step(&mut mv, &c, Cmd::new(0, 0.0, 0.0)).finished.is_some(),
            "the end zone"
        );
    }

    #[test]
    fn eazy_holds_its_blocks_still_and_runs_from_its_pad_to_its_last_wall() {
        let (m, c) = load("bhop_eazy");
        let mut mv = Mover::new(&m);
        // a block (a func_door that sinks in Source) holds you where it rests
        let mut b = Surfer::at_start(&m);
        b.phase = Phase::Running;
        b.body.origin = v3(-160.0, -320.0, 90.0);
        for _ in 0..64 {
            b.step(&mut mv, &c, Cmd::new(0, 0.0, 0.0));
        }
        assert!(
            b.body.on_ground && (63.0..=66.0).contains(&b.body.origin.z),
            "standing on the first block at {:?}",
            b.body.origin
        );
        // hop off the pad: the clock starts, and the first lane's floor
        // sends you back to the pad
        let mut s = Surfer::at_start(&m);
        let mut started = false;
        let mut fell = false;
        for _ in 0..640 {
            let t = s.step(&mut mv, &c, Cmd::new(B_FORWARD | B_JUMP, 30.0, 0.0));
            started |= t.started;
            if t.fell {
                fell = true;
                break;
            }
        }
        assert!(started && fell, "off the pad and onto the first lane's floor");
        assert_eq!((s.body.origin, s.phase), (m.spawn, Phase::Start));
        // a later lane's floor sends you back to that lane, the clock running
        let t22 = m.name_index("t22").unwrap();
        let mut l = Surfer::at_start(&m);
        l.phase = Phase::Running;
        l.body.origin = v3(320.0, -352.0, 17.0);
        l.step(&mut mv, &c, Cmd::new(0, 0.0, 0.0));
        assert_eq!(
            (l.body.origin, l.phase),
            (m.destination(t22).unwrap().0, Phase::Running),
            "the second lane's floor"
        );
        // the wall at the end of the last lane stops the clock
        let mut f = Surfer::at_start(&m);
        f.phase = Phase::Running;
        f.body.origin = v3(1300.0, -864.0, 100.0);
        let mut finished = false;
        for _ in 0..64 {
            if f.step(&mut mv, &c, Cmd::new(B_FORWARD, 180.0, 0.0))
                .finished
                .is_some()
            {
                finished = true;
                break;
            }
        }
        assert!(finished, "the last wall is the end");
    }

    #[test]
    fn lego_starts_off_its_first_platform_boosts_from_its_pads_and_ends_in_its_end_room() {
        let (m, c) = load("bhop_lego");
        let mut mv = Mover::new(&m);
        // from the spawn, down the hall and off the first platform: the clock
        // starts, and the floor under the first blocks sends you back
        let mut s = Surfer::at_start(&m);
        let mut started = false;
        let mut fell = false;
        for _ in 0..1280 {
            let t = s.step(&mut mv, &c, Cmd::new(B_FORWARD, 270.0, 0.0));
            started |= t.started;
            if t.fell {
                fell = true;
                break;
            }
        }
        assert!(started && fell, "off the first platform at {:?}", s.body.origin);
        assert_eq!((s.body.origin, s.phase), (m.spawn, Phase::Start));
        // a block holds you: its anti-stand teleport (a filter_multi) is left out
        let mut b = Surfer::at_start(&m);
        b.phase = Phase::Running;
        b.body.origin = v3(-104.0, -120.0, 150.0);
        for _ in 0..128 {
            assert!(
                !b.step(&mut mv, &c, Cmd::new(0, 0.0, 0.0)).teleported,
                "kicked off a block"
            );
        }
        assert!(b.body.on_ground, "standing on a block at {:?}", b.body.origin);
        // dropping into stage 3's lift over its pad (shot in Source; reached
        // here) names you "boost", and the lift's push takes you up to 3c;
        // without the name you fall to the floor and back to 3b
        let b3 = m.destination(m.name_index("3b").unwrap()).unwrap().0;
        let mut lift = |name: bool| {
            let mut p = Surfer::at_start(&m);
            p.phase = Phase::Running;
            p.body.origin = v3(-848.0, -3760.0, 700.0);
            let mut top: f32 = 0.0;
            for _ in 0..192 {
                if !name {
                    p.name = 0;
                }
                p.step(&mut mv, &c, Cmd::new(0, 0.0, 0.0));
                if p.body.origin == b3 {
                    return (top, true);
                }
                top = top.max(p.body.origin.z);
            }
            (top, false)
        };
        let (top, fell) = lift(true);
        assert!(top > 800.0 && !fell, "the pad boosts you up to 3c: top {top}");
        let (top, fell) = lift(false);
        assert!(fell, "no name, no boost: back to 3b (top {top})");
        // the room after the last strip, the map's "end", stops the clock
        let mut f = Surfer::at_start(&m);
        f.phase = Phase::Running;
        f.body.origin = v3(1700.0, -10300.0, -2000.0);
        let mut finished = false;
        for _ in 0..64 {
            if f.step(&mut mv, &c, Cmd::new(0, 0.0, 0.0)).finished.is_some() {
                finished = true;
                break;
            }
        }
        assert!(finished, "the end room is the end");
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
