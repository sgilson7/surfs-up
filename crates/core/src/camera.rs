//! The eye: view and projection matrices (column-major, for WebGL), and
//! putting a point in the world onto the screen for name tags.

use crate::vec3::{angle_vectors, sin_cos_deg, V3};

pub const NEAR: f32 = 3.0;
pub const FAR: f32 = 80000.0;

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub eye: V3,
    pub forward: V3,
    pub right: V3,
    pub up: V3,
    /// tan of half the vertical field of view
    pub tan_half_v: f32,
    pub aspect: f32,
}

impl Camera {
    /// Source's fov is horizontal at 4:3 (so 90 shows 90 degrees on a 4:3
    /// screen, and more to the sides on a wider one, as CS:GO does).
    pub fn new(eye: V3, pitch: f32, yaw: f32, fov43: f32, aspect: f32) -> Camera {
        let (forward, right, up) = angle_vectors(pitch, yaw);
        let (s, c) = sin_cos_deg(fov43 as f64 * 0.5);
        let tan_half_h43 = (s / c) as f32;
        Camera {
            eye,
            forward,
            right,
            up,
            tan_half_v: tan_half_h43 * 0.75,
            aspect: if aspect > 0.0 { aspect } else { 1.0 },
        }
    }

    /// projection × view, column-major.
    pub fn matrix(&self) -> [f32; 16] {
        let (r, u, f, e) = (self.right, self.up, self.forward, self.eye);
        // view: rows are right, up, -forward (camera looks down -z)
        let view = [
            [r.x, r.y, r.z, -r.dot(e)],
            [u.x, u.y, u.z, -u.dot(e)],
            [-f.x, -f.y, -f.z, f.dot(e)],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let sy = 1.0 / self.tan_half_v;
        let sx = sy / self.aspect;
        let a = (FAR + NEAR) / (NEAR - FAR);
        let b = 2.0 * FAR * NEAR / (NEAR - FAR);
        let proj = [
            [sx, 0.0, 0.0, 0.0],
            [0.0, sy, 0.0, 0.0],
            [0.0, 0.0, a, b],
            [0.0, 0.0, -1.0, 0.0],
        ];
        let mut m = [0.0f32; 16];
        for col in 0..4 {
            for row in 0..4 {
                let mut s = 0.0;
                for k in 0..4 {
                    s += proj[row][k] * view[k][col];
                }
                m[col * 4 + row] = s;
            }
        }
        m
    }

    /// Where a world point lands on a screen of `w` × `h` CSS pixels, and how
    /// far away it is; None behind the eye.
    pub fn project(&self, p: V3, w: f32, h: f32) -> Option<(f32, f32, f32)> {
        let d = p - self.eye;
        let z = d.dot(self.forward);
        if z < NEAR {
            return None;
        }
        let x = d.dot(self.right) / (z * self.tan_half_v * self.aspect);
        let y = d.dot(self.up) / (z * self.tan_half_v);
        Some(((x + 1.0) * 0.5 * w, (1.0 - y) * 0.5 * h, z))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec3::v3;

    #[test]
    fn a_point_ahead_lands_in_the_middle_and_clip_space_agrees() {
        let c = Camera::new(v3(10.0, 20.0, 30.0), 0.0, 90.0, 90.0, 16.0 / 9.0);
        let ahead = v3(10.0, 520.0, 30.0);
        let (x, y, z) = c.project(ahead, 1600.0, 900.0).unwrap();
        assert!((x - 800.0).abs() < 0.01 && (y - 450.0).abs() < 0.01 && (z - 500.0).abs() < 0.01);
        let m = c.matrix();
        let clip = |p: V3| {
            let v = [p.x, p.y, p.z, 1.0];
            let mut o = [0.0f32; 4];
            for row in 0..4 {
                for k in 0..4 {
                    o[row] += m[k * 4 + row] * v[k];
                }
            }
            o
        };
        let o = clip(ahead);
        assert!((o[0] / o[3]).abs() < 1e-4 && (o[1] / o[3]).abs() < 1e-4);
        assert!(o[2] / o[3] > -1.0 && o[2] / o[3] < 1.0);
        // up on screen is up in the world; right is right
        let o = clip(v3(10.0, 520.0, 130.0));
        assert!(o[1] / o[3] > 0.0);
        let o = clip(v3(110.0, 520.0, 30.0));
        assert!(o[0] / o[3] > 0.0, "looking along +y, +x is to the right");
        assert!(c.project(v3(10.0, -500.0, 30.0), 1600.0, 900.0).is_none());
    }
}
