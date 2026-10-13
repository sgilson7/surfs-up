//! Player movement: a port of Source's CGameMovement (game/shared/
//! gamemovement.cpp, Source SDK 2013) as CS:GO surf servers run it. The order
//! of operations, the constants and the clipping are Source's, because the
//! feel of surfing lives in exactly these details:
//!
//! - a slope is ground only when its normal's z is at least 0.7 (about 45.6
//!   degrees); a surf ramp is steeper, so on a ramp you are always in the air;
//! - in the air the wish speed is capped at 30 units per second before it is
//!   compared with the current speed along the wish direction, but the
//!   acceleration itself uses the uncapped wish speed, so turning the mouse
//!   while holding the strafe key into the turn adds speed;
//! - a ramp removes only the part of velocity going into it (ClipVelocity with
//!   overbounce 1), and gravity, applied half before and half after the move,
//!   pulls you along the ramp;
//! - while moving up and off the ground, air acceleration is a quarter
//!   (CategorizePosition sets surface friction to 0.25).
//!
//! RESEARCH-surf-movement.md has the sources for each of these.

use crate::map::Map;
use crate::trace::{trace_box, Scratch, Trace, DIST_EPSILON};
use crate::vec3::{angle_vectors, v3, V3, ZERO};

/// Every number the movement uses, with Source's (or CS:GO surf servers')
/// value. One place, so tuning is one edit.
#[derive(Clone, Copy, Debug)]
pub struct Tuning {
    pub tick_rate: u32,
    pub gravity: f32,
    pub accelerate: f32,
    pub air_accelerate: f32,
    pub air_wish_cap: f32,
    pub friction: f32,
    pub stop_speed: f32,
    pub max_speed: f32,
    pub max_velocity: f32,
    pub forward_speed: f32,
    pub side_speed: f32,
    pub jump_impulse: f32,
    pub step_size: f32,
    pub duck_speed_scale: f32,
    pub non_jump_velocity: f32,
    pub ground_normal_z: f32,
    pub up_surface_friction: f32,
    /// Air acceleration steps a tick, the view turning between them. 1 is
    /// Source 1; CS2 splits a tick at sub-tick input times (cs2kz's
    /// competitive mode rounds those to half ticks, which is 2 here).
    pub air_substeps: u32,
    /// How far (units) a move that has hit a plane is aimed off it before the
    /// next bump: CS2's fix for snagging on map geometry while surfing.
    pub snag_offset: f32,
}

pub const TUNING: Tuning = Tuning {
    tick_rate: 64,
    gravity: 800.0,
    accelerate: 10.0,
    air_accelerate: 150.0,
    air_wish_cap: 30.0,
    friction: 5.2,
    stop_speed: 80.0,
    max_speed: 250.0,
    max_velocity: 3500.0,
    forward_speed: 450.0,
    side_speed: 450.0,
    // sqrt(2 * 800 * 57), Source's GAMEMOVEMENT_JUMP_HEIGHT of 57 units
    jump_impulse: 301.993_38,
    step_size: 18.0,
    duck_speed_scale: 0.34,
    non_jump_velocity: 140.0,
    ground_normal_z: 0.7,
    // Source 1 cuts air acceleration to a quarter while rising slowly off a
    // surface; CS2's November 2025 update fixed "slowly surfing up a ramp ...
    // misidentified as a jump apex". This follows CS2.
    up_surface_friction: 1.0,
    air_substeps: 2,
    snag_offset: 0.031_25,
};

pub const HULL_MINS: V3 = v3(-16.0, -16.0, 0.0);
pub const HULL_MAXS: V3 = v3(16.0, 16.0, 72.0);
pub const DUCK_MAXS: V3 = v3(16.0, 16.0, 54.0);
pub const EYE_STAND: f32 = 64.09;
pub const EYE_DUCK: f32 = 46.08;
const MAX_CLIP_PLANES: usize = 5;
/// Source makes four bumps a tick; the rampbug fixes (MomSurfFix) use eight,
/// so a seam between two ramp brushes cannot use them all up.
const BUMPS: usize = 8;
/// Two clip planes this close to parallel are one plane. Without this, a seam
/// between adjoining ramp brushes gives a crease of zero length and stops the
/// surfer dead: the classic rampbug.
const SAME_PLANE: f32 = 0.999;
/// How hard (units a second) a surfer is pushed off a ramp they hit twice in
/// one tick: far too little to feel, enough to clear float rounding.
const PUSH_OFF: f32 = 0.25;

/// One tick's input: keys and the view angles, quantised so a recorded run
/// replays to the same bits (65536 steps a turn, about 0.0055 degrees).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cmd {
    pub buttons: u8,
    pub yaw: u16,
    pub pitch: i16,
}

pub const B_FORWARD: u8 = 1;
pub const B_BACK: u8 = 2;
pub const B_LEFT: u8 = 4;
pub const B_RIGHT: u8 = 8;
pub const B_JUMP: u8 = 16;
pub const B_DUCK: u8 = 32;
/// Not movement: squeezes whatever is in your hands. Carried in the command so
/// other browsers hear it too.
pub const B_USE: u8 = 64;
/// Not movement: sends you back to the start.
pub const B_RESET: u8 = 128;

const TURN: f32 = 65536.0;

impl Cmd {
    pub fn new(buttons: u8, yaw_deg: f32, pitch_deg: f32) -> Cmd {
        let y = (yaw_deg.rem_euclid(360.0) / 360.0 * TURN).round();
        let yaw = if y >= TURN { 0 } else { y as u16 };
        let pitch = (pitch_deg.clamp(-89.0, 89.0) / 360.0 * TURN).round() as i16;
        Cmd { buttons, yaw, pitch }
    }
    pub fn yaw_deg(self) -> f32 {
        self.yaw as f32 * (360.0 / TURN)
    }
    pub fn pitch_deg(self) -> f32 {
        self.pitch as f32 * (360.0 / TURN)
    }
    pub fn has(self, b: u8) -> bool {
        self.buttons & b != 0
    }
    /// Forward and side move in Source's units (cl_forwardspeed, cl_sidespeed).
    pub fn moves(self, t: &Tuning) -> (f32, f32) {
        let mut f = 0.0;
        let mut s = 0.0;
        if self.has(B_FORWARD) {
            f += t.forward_speed;
        }
        if self.has(B_BACK) {
            f -= t.forward_speed;
        }
        if self.has(B_RIGHT) {
            s += t.side_speed;
        }
        if self.has(B_LEFT) {
            s -= t.side_speed;
        }
        (f, s)
    }
}

/// Everything that moves with a player from tick to tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub origin: V3,
    pub velocity: V3,
    pub base_velocity: V3,
    pub on_ground: bool,
    pub ducked: bool,
    /// FL_BASEVELOCITY: a push trigger touched the player this tick.
    pub pushed: bool,
    /// Jump was held on the previous tick (for the no-autohop case).
    pub old_jump: bool,
    pub surface_friction: f32,
}

impl Body {
    pub fn at(origin: V3) -> Body {
        Body {
            origin,
            velocity: ZERO,
            base_velocity: ZERO,
            on_ground: false,
            ducked: false,
            pushed: false,
            old_jump: false,
            surface_friction: 1.0,
        }
    }
    pub fn maxs(&self) -> V3 {
        if self.ducked {
            DUCK_MAXS
        } else {
            HULL_MAXS
        }
    }
    pub fn eye(&self) -> f32 {
        if self.ducked {
            EYE_DUCK
        } else {
            EYE_STAND
        }
    }
}

/// What happened during a move that the run and the sound care about.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MoveEvents {
    pub jumped: bool,
    pub landed: bool,
    /// Speed lost against walls this tick (for the thud).
    pub impact: f32,
}

pub struct Mover<'a> {
    pub map: &'a Map,
    pub t: Tuning,
    pub scratch: Scratch,
    pub autohop: bool,
}

impl<'a> Mover<'a> {
    pub fn new(map: &'a Map) -> Mover<'a> {
        Mover {
            map,
            t: TUNING,
            scratch: Scratch::default(),
            autohop: true,
        }
    }

    fn dt(&self) -> f32 {
        1.0 / self.t.tick_rate as f32
    }

    pub fn trace(&mut self, b: &Body, start: V3, end: V3) -> Trace {
        trace_box(self.map, &mut self.scratch, start, end, HULL_MINS, b.maxs())
    }

    /// One tick of Source's PlayerMove for a walking player (FullWalkMove).
    /// `prev` is the previous tick's command: its view is where this tick's
    /// sub-steps turn from.
    pub fn player_move(&mut self, b: &mut Body, cmd: Cmd, prev: Cmd) -> MoveEvents {
        let mut ev = MoveEvents::default();
        let dt = self.dt();
        // CheckStuck: a box that starts inside a solid is nudged out first;
        // one that cannot be freed does not move this tick.
        if self.unstick(b) {
            let t = self.trace(b, b.origin, b.origin);
            if t.start_solid {
                b.velocity = ZERO;
                return ev;
            }
        }
        let was_ground = b.on_ground;
        let speed_before = b.velocity.length();

        // A push trigger left last tick: its push becomes the player's own
        // momentum, plus half a frame (CBasePlayer, "apply momentum").
        if !b.pushed && !b.base_velocity.is_zero() {
            b.velocity += b.base_velocity * (1.0 + dt * 0.5);
            b.base_velocity = ZERO;
        }

        self.duck(b, cmd);
        let (mut fmove, mut smove) = cmd.moves(&self.t);
        // CheckParameters: the move is no longer than max speed
        let spd2 = fmove * fmove + smove * smove;
        let max = self.t.max_speed;
        if spd2 != 0.0 && spd2 > max * max {
            let ratio = max / spd2.sqrt();
            fmove *= ratio;
            smove *= ratio;
        }
        if b.ducked && b.on_ground {
            fmove *= self.t.duck_speed_scale;
            smove *= self.t.duck_speed_scale;
        }

        self.start_gravity(b);
        if cmd.has(B_JUMP) {
            if self.check_jump(b) {
                ev.jumped = true;
            }
        } else {
            b.old_jump = false;
        }
        if b.on_ground {
            b.velocity.z = 0.0;
            self.friction(b);
        }
        self.check_velocity(b);

        let (fwd, right, _) = angle_vectors(cmd.pitch_deg(), cmd.yaw_deg());
        if b.on_ground {
            self.walk_move(b, fwd, right, fmove, smove);
        } else {
            self.air_move(b, cmd, prev, fmove, smove);
        }
        self.categorize_position(b);
        self.check_velocity(b);
        if !b.on_ground {
            self.finish_gravity(b);
        }
        if b.on_ground {
            b.velocity.z = 0.0;
        }
        if b.on_ground && !was_ground {
            ev.landed = true;
        }
        let lost = speed_before - b.velocity.length();
        if lost > 0.0 && !ev.jumped {
            ev.impact = lost;
        }
        ev
    }

    /// One tick with no turn since the last (tests and simple callers).
    pub fn player_move_same(&mut self, b: &mut Body, cmd: Cmd) -> MoveEvents {
        self.player_move(b, cmd, cmd)
    }

    fn duck(&mut self, b: &mut Body, cmd: Cmd) {
        let want = cmd.has(B_DUCK);
        if want && !b.ducked {
            // FinishDuck: in the air the feet tuck up, so the origin rises by
            // the difference in hull height; on the ground the head lowers.
            if !b.on_ground {
                b.origin.z += HULL_MAXS.z - DUCK_MAXS.z;
            }
            b.ducked = true;
            self.categorize_position(b);
        } else if !want && b.ducked {
            // CanUnduck: only if the standing hull fits.
            let mut stand = *b;
            stand.ducked = false;
            if !b.on_ground {
                stand.origin.z -= HULL_MAXS.z - DUCK_MAXS.z;
            }
            let t = self.trace(&stand, stand.origin, stand.origin);
            if !t.start_solid {
                *b = stand;
                self.categorize_position(b);
            }
        }
    }

    fn start_gravity(&mut self, b: &mut Body) {
        let dt = self.dt();
        b.velocity.z -= self.t.gravity * 0.5 * dt;
        b.velocity.z += b.base_velocity.z * dt;
        b.base_velocity.z = 0.0;
        self.check_velocity(b);
    }

    fn finish_gravity(&mut self, b: &mut Body) {
        b.velocity.z -= self.t.gravity * self.dt() * 0.5;
        self.check_velocity(b);
    }

    fn check_velocity(&self, b: &mut Body) {
        let m = self.t.max_velocity;
        b.velocity.x = b.velocity.x.clamp(-m, m);
        b.velocity.y = b.velocity.y.clamp(-m, m);
        b.velocity.z = b.velocity.z.clamp(-m, m);
    }

    fn check_jump(&mut self, b: &mut Body) -> bool {
        if !b.on_ground {
            b.old_jump = true;
            return false;
        }
        if b.old_jump && !self.autohop {
            return false;
        }
        b.on_ground = false;
        if b.ducked {
            b.velocity.z = self.t.jump_impulse;
        } else {
            b.velocity.z += self.t.jump_impulse;
        }
        self.finish_gravity(b);
        b.old_jump = true;
        true
    }

    fn friction(&self, b: &mut Body) {
        let speed = b.velocity.length();
        if speed < 0.1 {
            return;
        }
        let friction = self.t.friction * b.surface_friction;
        let control = if speed < self.t.stop_speed {
            self.t.stop_speed
        } else {
            speed
        };
        let drop = control * friction * self.dt();
        let mut newspeed = speed - drop;
        if newspeed < 0.0 {
            newspeed = 0.0;
        }
        if newspeed != speed {
            b.velocity = b.velocity * (newspeed / speed);
        }
    }

    fn wish(&self, fwd: V3, right: V3, fmove: f32, smove: f32) -> (V3, f32) {
        let mut f = fwd;
        let mut r = right;
        f.z = 0.0;
        r.z = 0.0;
        f.normalize();
        r.normalize();
        let mut wishvel = f * fmove + r * smove;
        wishvel.z = 0.0;
        let mut wishdir = wishvel;
        let mut wishspeed = wishdir.normalize();
        if wishspeed != 0.0 && wishspeed > self.t.max_speed {
            wishspeed = self.t.max_speed;
        }
        if wishspeed == 0.0 {
            wishdir = ZERO;
        }
        (wishdir, wishspeed)
    }

    fn accelerate(&self, b: &mut Body, wishdir: V3, wishspeed: f32, accel: f32) {
        let current = b.velocity.dot(wishdir);
        let add = wishspeed - current;
        if add <= 0.0 {
            return;
        }
        let mut a = accel * self.dt() * wishspeed * b.surface_friction;
        if a > add {
            a = add;
        }
        b.velocity += wishdir * a;
    }

    /// The heart of surfing: the 30 unit cap applies to the comparison, not to
    /// the acceleration.
    pub fn air_accelerate(&self, b: &mut Body, wishdir: V3, wishspeed: f32) {
        self.air_accelerate_for(b, wishdir, wishspeed, self.dt());
    }

    fn air_accelerate_for(&self, b: &mut Body, wishdir: V3, wishspeed: f32, dt: f32) {
        let wishspd = wishspeed.min(self.t.air_wish_cap);
        let current = b.velocity.dot(wishdir);
        let add = wishspd - current;
        if add <= 0.0 {
            return;
        }
        let mut a = self.t.air_accelerate * wishspeed * dt * b.surface_friction;
        if a > add {
            a = add;
        }
        b.velocity += wishdir * a;
    }

    /// Air acceleration in sub-steps, the view turning from last tick's yaw
    /// to this tick's, then one move. Only yaw matters: the wish direction is
    /// flattened.
    fn air_move(&mut self, b: &mut Body, cmd: Cmd, prev: Cmd, fmove: f32, smove: f32) {
        let n = self.t.air_substeps.max(1);
        let from = prev.yaw_deg();
        let turn = crate::vec3::wrap180(cmd.yaw_deg() - from);
        let dt = self.dt() / n as f32;
        for k in 1..=n {
            let yaw = from + turn * (k as f32 / n as f32);
            let (fwd, right, _) = angle_vectors(0.0, yaw);
            let (wishdir, wishspeed) = self.wish(fwd, right, fmove, smove);
            self.air_accelerate_for(b, wishdir, wishspeed, dt);
        }
        b.velocity += b.base_velocity;
        self.try_player_move(b);
        b.velocity -= b.base_velocity;
    }

    fn walk_move(&mut self, b: &mut Body, fwd: V3, right: V3, fmove: f32, smove: f32) {
        let (wishdir, wishspeed) = self.wish(fwd, right, fmove, smove);
        b.velocity.z = 0.0;
        self.accelerate(b, wishdir, wishspeed, self.t.accelerate);
        b.velocity.z = 0.0;
        b.velocity += b.base_velocity;
        let spd = b.velocity.length();
        if spd < 1.0 {
            b.velocity = ZERO;
            b.velocity -= b.base_velocity;
            return;
        }
        let dt = self.dt();
        let mut dest = b.origin + b.velocity * dt;
        dest.z = b.origin.z;
        let t = self.trace(b, b.origin, dest);
        if t.fraction == 1.0 {
            b.origin = t.end;
            b.velocity -= b.base_velocity;
            self.stay_on_ground(b);
            return;
        }
        self.step_move(b);
        b.velocity -= b.base_velocity;
        self.stay_on_ground(b);
    }

    fn step_move(&mut self, b: &mut Body) {
        let pos = b.origin;
        let vel = b.velocity;
        self.try_player_move(b);
        let down_pos = b.origin;
        let down_vel = b.velocity;

        b.origin = pos;
        b.velocity = vel;
        let up = pos + v3(0.0, 0.0, self.t.step_size + DIST_EPSILON);
        let t = self.trace(b, b.origin, up);
        if !t.start_solid && !t.all_solid {
            b.origin = t.end;
        }
        self.try_player_move(b);
        let down = b.origin - v3(0.0, 0.0, self.t.step_size + DIST_EPSILON);
        let t = self.trace(b, b.origin, down);
        if t.normal.z < self.t.ground_normal_z {
            b.origin = down_pos;
            b.velocity = down_vel;
            return;
        }
        if !t.start_solid && !t.all_solid {
            b.origin = t.end;
        }
        let up_pos = b.origin;
        let dd = (down_pos.x - pos.x) * (down_pos.x - pos.x) + (down_pos.y - pos.y) * (down_pos.y - pos.y);
        let ud = (up_pos.x - pos.x) * (up_pos.x - pos.x) + (up_pos.y - pos.y) * (up_pos.y - pos.y);
        if dd > ud {
            b.origin = down_pos;
            b.velocity = down_vel;
        } else {
            b.velocity.z = down_vel.z;
        }
    }

    fn stay_on_ground(&mut self, b: &mut Body) {
        let start = b.origin + v3(0.0, 0.0, 2.0);
        let end = b.origin - v3(0.0, 0.0, self.t.step_size);
        let t = self.trace(b, b.origin, start);
        let start = t.end;
        let t = self.trace(b, start, end);
        if t.fraction > 0.0 && t.fraction < 1.0 && !t.start_solid && t.normal.z >= self.t.ground_normal_z {
            let delta = (b.origin.z - t.end.z).abs();
            if delta > 0.5 / 32.0 {
                b.origin = t.end;
            }
        }
    }

    /// Source's TryPlayerMove: move, and on each hit clip the velocity along
    /// the plane, at most four times a tick.
    pub fn try_player_move(&mut self, b: &mut Body) -> u8 {
        let mut blocked = 0u8;
        let primal = b.velocity;
        let mut original = b.velocity;
        let mut planes = [ZERO; MAX_CLIP_PLANES];
        let mut numplanes = 0usize;
        let mut all_fraction = 0.0f32;
        let mut time_left = self.dt();

        for _ in 0..BUMPS {
            if b.velocity.is_zero() {
                break;
            }
            let mut end = b.origin + b.velocity * time_left;
            if numplanes > 0 {
                // aim a little off the plane already hit (CS2's snag fix)
                end += planes[numplanes - 1] * self.t.snag_offset;
            }
            let t = self.trace(b, b.origin, end);
            all_fraction += t.fraction;
            if t.all_solid {
                // Stuck inside a solid. Source zeroes the velocity here, and that
                // is one way a ramp "bug" stops a surfer dead. Keep the velocity
                // and let unstick() move the box out next tick.
                return 4;
            }
            if t.fraction > 0.0 {
                if t.fraction == 1.0 {
                    // Source re-tests the end with an unswept box (a precision
                    // guard). If that end is in a solid, do not move there.
                    let stuck = self.trace(b, t.end, t.end);
                    if stuck.start_solid || stuck.fraction != 1.0 {
                        break;
                    }
                }
                b.origin = t.end;
                original = b.velocity;
                numplanes = 0;
            }
            if t.fraction == 1.0 {
                break;
            }
            if t.normal.z > self.t.ground_normal_z {
                blocked |= 1;
            }
            if t.normal.z == 0.0 {
                blocked |= 2;
            }
            time_left -= time_left * t.fraction;
            if numplanes >= MAX_CLIP_PLANES {
                b.velocity = ZERO;
                break;
            }
            if planes[..numplanes].iter().any(|p| p.dot(t.normal) > SAME_PLANE) {
                // The same ramp again. Clipping leaves a sliver of velocity
                // into it (float rounding), so the next trace hits it again at
                // once, and Source then finds a crease of zero length and
                // stops the surfer: the rampbug. Clip once more and push off
                // by a hair (Momentum's fix pushes off too), and carry on.
                b.velocity = clip_velocity(b.velocity, t.normal, 1.0) + t.normal * PUSH_OFF;
                original = b.velocity;
                continue;
            }
            planes[numplanes] = t.normal;
            numplanes += 1;

            if numplanes == 1 && !b.on_ground {
                // In the air, one plane: clip against it with overbounce 1
                // (sv_bounce is 0, so floors and walls clip alike).
                let nv = clip_velocity(original, planes[0], 1.0);
                b.velocity = nv;
                original = nv;
            } else {
                let mut i = 0;
                while i < numplanes {
                    b.velocity = clip_velocity(original, planes[i], 1.0);
                    let mut j = 0;
                    while j < numplanes {
                        if j != i && b.velocity.dot(planes[j]) < 0.0 {
                            break;
                        }
                        j += 1;
                    }
                    if j == numplanes {
                        break;
                    }
                    i += 1;
                }
                if i == numplanes {
                    // go along the crease
                    if numplanes != 2 {
                        b.velocity = ZERO;
                        break;
                    }
                    let dir = planes[0].cross(planes[1]).normalized();
                    let d = dir.dot(b.velocity);
                    b.velocity = dir * d;
                }
                // moving back against the original direction: stop dead, so
                // a sloping corner does not make you oscillate
                if b.velocity.dot(primal) <= 0.0 {
                    b.velocity = ZERO;
                    break;
                }
            }
        }
        // Source stops a player who could not move at all this tick; in the
        // air that is a rampbug, so only on the ground.
        if all_fraction == 0.0 && b.on_ground {
            b.velocity = ZERO;
        }
        blocked
    }

    /// CategorizePosition: are we on ground? A plane is ground only when its
    /// normal's z is at least 0.7, and never while moving up faster than 140.
    pub fn categorize_position(&mut self, b: &mut Body) {
        b.surface_friction = 1.0;
        let point = b.origin - v3(0.0, 0.0, 2.0);
        if b.velocity.z > self.t.non_jump_velocity {
            b.on_ground = false;
            return;
        }
        let t = self.trace(b, b.origin, point);
        let mut ground = t.hit() && t.normal.z >= self.t.ground_normal_z;
        if !ground {
            ground = self.touch_ground_in_quadrants(b, point);
        }
        if ground {
            if !b.on_ground {
                // Landing on a walkable slope: Source drops the vertical speed
                // (snap), unless the move happened to collide first (clip), and
                // which one happens depends on sub-tick position. Take the
                // better of the two every time, as slopefix and RNGFix do.
                let snapped = v3(b.velocity.x, b.velocity.y, 0.0);
                let n = if t.hit() && t.normal.z >= self.t.ground_normal_z {
                    t.normal
                } else {
                    v3(0.0, 0.0, 1.0)
                };
                let clipped = clip_velocity(b.velocity, n, 1.0);
                b.velocity = if clipped.length2d() > snapped.length2d() {
                    v3(clipped.x, clipped.y, 0.0)
                } else {
                    snapped
                };
            }
            b.on_ground = true;
        } else {
            b.on_ground = false;
            if b.velocity.z > 0.0 {
                b.surface_friction = self.t.up_surface_friction;
            }
        }
    }

    /// TryTouchGroundInQuadrants: four half-boxes, for standing on an edge
    /// that the whole box finds only as a steep bevel.
    fn touch_ground_in_quadrants(&mut self, b: &Body, end: V3) -> bool {
        let maxs = b.maxs();
        let mins = HULL_MINS;
        let mid_x = (mins.x + maxs.x) * 0.5;
        let mid_y = (mins.y + maxs.y) * 0.5;
        let quads = [
            (v3(mid_x, mid_y, mins.z), v3(maxs.x, maxs.y, maxs.z)),
            (v3(mins.x, mins.y, mins.z), v3(mid_x, mid_y, maxs.z)),
            (v3(mins.x, mid_y, mins.z), v3(mid_x, maxs.y, maxs.z)),
            (v3(mid_x, mins.y, mins.z), v3(maxs.x, mid_y, maxs.z)),
        ];
        for (lo, hi) in quads {
            let t = trace_box(self.map, &mut self.scratch, b.origin, end, lo, hi);
            if t.hit() && t.normal.z >= self.t.ground_normal_z {
                return true;
            }
        }
        false
    }

    /// If the box starts inside a solid (a precision slip against a ramp),
    /// move it out along the smallest of a few nudges, as Source's CheckStuck
    /// does. Returns whether it was stuck.
    pub fn unstick(&mut self, b: &mut Body) -> bool {
        let t = self.trace(b, b.origin, b.origin);
        if !t.start_solid {
            return false;
        }
        let steps = [0.125f32, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0];
        let dirs = [
            v3(0.0, 0.0, 1.0),
            v3(1.0, 0.0, 0.0),
            v3(-1.0, 0.0, 0.0),
            v3(0.0, 1.0, 0.0),
            v3(0.0, -1.0, 0.0),
            v3(0.0, 0.0, -1.0),
        ];
        for s in steps {
            // prefer the direction of travel, then the axes
            let mut tries = vec![b.velocity.normalized() * -s];
            tries.extend(dirs.iter().map(|d| *d * s));
            for d in tries {
                let p = b.origin + d;
                let t = self.trace(b, p, p);
                if !t.start_solid {
                    b.origin = p;
                    return true;
                }
            }
        }
        true
    }
}

/// Source's ClipVelocity: remove the part of `v` going into the plane, times
/// the overbounce, then make sure nothing still goes into it.
pub fn clip_velocity(v: V3, n: V3, overbounce: f32) -> V3 {
    let backoff = v.dot(n) * overbounce;
    let mut out = v - n * backoff;
    let adjust = out.dot(n);
    if adjust < 0.0 {
        out -= n * adjust;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::tests::utopia;

    #[test]
    fn standing_at_the_start_comes_to_rest_on_the_floor() {
        let m = utopia();
        let mut mv = Mover::new(m);
        let mut b = Body::at(m.spawn);
        for _ in 0..128 {
            mv.player_move_same(&mut b, Cmd::new(0, 0.0, 0.0));
        }
        assert!(b.on_ground, "{b:?}");
        assert!(b.velocity.length() < 0.01);
    }

    #[test]
    fn walking_forward_reaches_max_speed_and_no_more() {
        let m = utopia();
        let mut mv = Mover::new(m);
        let mut b = Body::at(m.spawn);
        for _ in 0..64 {
            mv.player_move_same(&mut b, Cmd::new(0, 90.0, 0.0));
        }
        let start = b.origin;
        // walk along +y on the start platform (it is wide in y)
        for _ in 0..64 {
            mv.player_move_same(&mut b, Cmd::new(B_FORWARD, 90.0, 0.0));
        }
        let s = b.velocity.length2d();
        assert!((s - 250.0).abs() < 1.0, "speed {s}");
        assert!(b.origin.y > start.y + 100.0);
    }

    #[test]
    fn turning_in_the_air_gains_more_with_two_sub_steps() {
        let m = utopia();
        let mut gains = Vec::new();
        for steps in [1, 2] {
            let mut mv = Mover::new(m);
            mv.t.air_substeps = steps;
            let mut b = Body::at(v3(0.0, 0.0, 14500.0));
            b.velocity = v3(600.0, 0.0, 0.0);
            let mut prev = Cmd::new(B_LEFT, 0.0, 0.0);
            let mut yaw = 0.0;
            for _ in 0..32 {
                // turn left at about the rate that keeps the wish square to the velocity
                yaw += 2.8;
                let cmd = Cmd::new(B_LEFT, yaw, 0.0);
                mv.player_move(&mut b, cmd, prev);
                prev = cmd;
            }
            gains.push(b.velocity.length2d() - 600.0);
        }
        assert!(gains[1] > gains[0] * 1.5, "half-tick steps gain more: {gains:?}");
    }

    #[test]
    fn clip_velocity_removes_only_the_part_into_the_plane() {
        let n = v3(0.0, -0.78, 0.62).normalized();
        let v = v3(1000.0, 300.0, -200.0);
        let out = clip_velocity(v, n, 1.0);
        assert!(out.dot(n).abs() < 1e-3);
        assert!((out.x - 1000.0).abs() < 1e-3, "along the ramp is untouched");
    }

    #[test]
    fn air_strafing_gains_speed_and_holding_forward_does_not() {
        let m = utopia();
        let mv = Mover::new(m);
        // the ideal strafe: wish direction kept perpendicular to velocity adds
        // the 30-unit cap at right angles every tick, 900 to speed squared
        let mut b = Body::at(ZERO);
        b.velocity = v3(400.0, 0.0, 0.0);
        for _ in 0..64 {
            let v = b.velocity;
            let wd = v3(-v.y, v.x, 0.0).normalized();
            mv.air_accelerate(&mut b, wd, 250.0);
        }
        let expect = (400.0f32 * 400.0 + 64.0 * 900.0).sqrt();
        assert!(
            (b.velocity.length2d() - expect).abs() < 0.5,
            "strafe gain {}",
            b.velocity.length2d()
        );
        // pressing forward along the velocity adds nothing above 30
        let mut c = Body::at(ZERO);
        c.velocity = v3(400.0, 0.0, 0.0);
        for _ in 0..64 {
            let (f, r, _) = angle_vectors(0.0, 0.0);
            let (wd, ws) = mv.wish(f, r, 450.0, 0.0);
            mv.air_accelerate(&mut c, wd, ws);
        }
        assert_eq!(c.velocity.length2d(), 400.0);
    }
}
