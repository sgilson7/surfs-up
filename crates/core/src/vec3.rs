//! Three-vectors in Source's units (inches; z up) and the few angle functions
//! the movement needs. Floats are `f32`, as Source's are.
//!
//! Determinism: every peer runs the same WebAssembly module, and WebAssembly
//! float arithmetic is IEEE 754 with no fused operations, so the same inputs
//! give the same bits in every browser. Sine and cosine are computed here from
//! `+`, `*` and `round` (all exactly specified) rather than from a platform
//! library, so the native test build agrees with the browser too.

use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub const fn v3(x: f32, y: f32, z: f32) -> V3 {
    V3 { x, y, z }
}

pub const ZERO: V3 = v3(0.0, 0.0, 0.0);

impl V3 {
    pub fn dot(self, o: V3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: V3) -> V3 {
        v3(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }
    pub fn length2d(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
    /// Normalises in place and returns the old length (Source's VectorNormalize).
    pub fn normalize(&mut self) -> f32 {
        let l = self.length();
        if l != 0.0 {
            let inv = 1.0 / l;
            self.x *= inv;
            self.y *= inv;
            self.z *= inv;
        }
        l
    }
    pub fn normalized(self) -> V3 {
        let mut v = self;
        v.normalize();
        v
    }
    pub fn get(self, axis: usize) -> f32 {
        match axis {
            0 => self.x,
            1 => self.y,
            _ => self.z,
        }
    }
    pub fn is_zero(self) -> bool {
        self.x == 0.0 && self.y == 0.0 && self.z == 0.0
    }
    pub fn lerp(self, o: V3, t: f32) -> V3 {
        self + (o - self) * t
    }
}

impl Add for V3 {
    type Output = V3;
    fn add(self, o: V3) -> V3 {
        v3(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl Sub for V3 {
    type Output = V3;
    fn sub(self, o: V3) -> V3 {
        v3(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl Mul<f32> for V3 {
    type Output = V3;
    fn mul(self, s: f32) -> V3 {
        v3(self.x * s, self.y * s, self.z * s)
    }
}
impl Neg for V3 {
    type Output = V3;
    fn neg(self) -> V3 {
        v3(-self.x, -self.y, -self.z)
    }
}
impl AddAssign for V3 {
    fn add_assign(&mut self, o: V3) {
        *self = *self + o;
    }
}
impl SubAssign for V3 {
    fn sub_assign(&mut self, o: V3) {
        *self = *self - o;
    }
}

const PI: f64 = std::f64::consts::PI;

/// Sine and cosine of an angle in degrees, from a quadrant reduction and a
/// Taylor series on [-45, 45] degrees (error below 1e-15).
pub fn sin_cos_deg(deg: f64) -> (f64, f64) {
    let q = (deg / 90.0).round();
    let r = (deg - q * 90.0) * (PI / 180.0);
    let r2 = r * r;
    let mut s = r;
    let mut c = 1.0;
    let mut ts = r;
    let mut tc = 1.0;
    let mut k = 1.0;
    while k < 16.0 {
        ts *= -r2 / ((k + 1.0) * (k + 2.0));
        tc *= -r2 / (k * (k + 1.0));
        s += ts;
        c += tc;
        k += 2.0;
    }
    let quadrant = (q as i64).rem_euclid(4);
    match quadrant {
        0 => (s, c),
        1 => (c, -s),
        2 => (-s, -c),
        _ => (-c, s),
    }
}

/// Source's AngleVectors: forward, right and up for a pitch (down positive)
/// and yaw in degrees. Roll is always zero here.
pub fn angle_vectors(pitch: f32, yaw: f32) -> (V3, V3, V3) {
    let (sy, cy) = sin_cos_deg(yaw as f64);
    let (sp, cp) = sin_cos_deg(pitch as f64);
    let (sy, cy, sp, cp) = (sy as f32, cy as f32, sp as f32, cp as f32);
    let forward = v3(cp * cy, cp * sy, -sp);
    let right = v3(sy, -cy, 0.0);
    let up = v3(sp * cy, sp * sy, cp);
    (forward, right, up)
}

/// An angle in degrees brought into (-180, 180].
pub fn wrap180(a: f32) -> f32 {
    let mut a = a % 360.0;
    if a > 180.0 {
        a -= 360.0;
    }
    if a <= -180.0 {
        a += 360.0;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sine_and_cosine_match_the_textbook_values() {
        let half = 0.5f64;
        let root_half = half.sqrt();
        let known = [
            (0.0, 0.0, 1.0),
            (30.0, half, 3f64.sqrt() / 2.0),
            (45.0, root_half, root_half),
            (90.0, 1.0, 0.0),
            (-135.0, -root_half, -root_half),
            (390.0, half, 3f64.sqrt() / 2.0),
            (-720.0, 0.0, 1.0),
        ];
        for (deg, sv, cv) in known {
            let (s, c) = sin_cos_deg(deg);
            assert!((s - sv).abs() < 1e-14 && (c - cv).abs() < 1e-14, "{deg}: {s} {c}");
        }
        for k in 0..720 {
            let (s, c) = sin_cos_deg(k as f64 * 1.37 - 400.0);
            assert!((s * s + c * c - 1.0).abs() < 1e-14);
        }
    }

    #[test]
    fn angle_vectors_face_along_yaw() {
        let (f, r, _) = angle_vectors(0.0, 90.0);
        assert!((f.y - 1.0).abs() < 1e-6 && f.x.abs() < 1e-6);
        assert!((r.x - 1.0).abs() < 1e-6, "right of +y is +x");
        let (f, _, _) = angle_vectors(89.0, 0.0);
        assert!(f.z < -0.99, "pitch down looks down");
    }

    #[test]
    fn wrap_keeps_angles_in_one_turn() {
        assert_eq!(wrap180(190.0), -170.0);
        assert_eq!(wrap180(-180.0), 180.0);
        assert_eq!(wrap180(540.0), 180.0);
    }
}
