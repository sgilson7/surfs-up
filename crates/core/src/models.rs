//! The people and things drawn each frame: other surfers (kids in gym
//! clothes), ghosts of finished runs, and your own hands, pressed together
//! until you earn something to hold. Built from boxes and balls in the
//! low-poly, flat-shaded style of Ape Race.

use crate::mesh::{push_polygon, M_GHOST, M_HAND, M_KID};
use crate::vec3::{angle_vectors, sin_cos_deg, v3, V3};

/// A frame: an origin and three axes (not necessarily unit: a scale).
#[derive(Clone, Copy, Debug)]
pub struct Xf {
    pub o: V3,
    pub x: V3,
    pub y: V3,
    pub z: V3,
}

impl Xf {
    pub fn new(o: V3, x: V3, y: V3, z: V3) -> Xf {
        Xf { o, x, y, z }
    }
    pub fn p(&self, l: V3) -> V3 {
        self.o + self.x * l.x + self.y * l.y + self.z * l.z
    }
    pub fn dir(&self, l: V3) -> V3 {
        (self.x * l.x + self.y * l.y + self.z * l.z).normalized()
    }
    /// The same frame moved to local point `at`.
    pub fn at(&self, at: V3) -> Xf {
        Xf {
            o: self.p(at),
            ..*self
        }
    }
    /// Rotated about its own x axis (roll), in degrees.
    pub fn roll(&self, deg: f32) -> Xf {
        let (s, c) = sin_cos_deg(deg as f64);
        let (s, c) = (s as f32, c as f32);
        Xf {
            o: self.o,
            x: self.x,
            y: self.y * c + self.z * s,
            z: self.z * c - self.y * s,
        }
    }
    /// Rotated about its own y axis (pitch, nose down positive), in degrees.
    pub fn pitch(&self, deg: f32) -> Xf {
        let (s, c) = sin_cos_deg(deg as f64);
        let (s, c) = (s as f32, c as f32);
        Xf {
            o: self.o,
            x: self.x * c - self.z * s,
            y: self.y,
            z: self.z * c + self.x * s,
        }
    }
    /// Rotated about its own z axis (yaw), in degrees.
    pub fn yaw(&self, deg: f32) -> Xf {
        let (s, c) = sin_cos_deg(deg as f64);
        let (s, c) = (s as f32, c as f32);
        Xf {
            o: self.o,
            x: self.x * c + self.y * s,
            y: self.y * c - self.x * s,
            z: self.z,
        }
    }
    pub fn scaled(&self, k: f32) -> Xf {
        Xf {
            o: self.o,
            x: self.x * k,
            y: self.y * k,
            z: self.z * k,
        }
    }
}

pub type Rgb = [f32; 3];

/// A box face: four corners as signs of the half sizes, and its normal.
type Quad = ([(f32, f32, f32); 4], V3);

/// A box with centre `c` and half sizes `h`, in the frame.
pub fn cuboid(out: &mut Vec<f32>, f: &Xf, c: V3, h: V3, col: Rgb, m: f32) {
    let corner = |sx: f32, sy: f32, sz: f32| f.p(c + v3(h.x * sx, h.y * sy, h.z * sz));
    let faces: [Quad; 6] = [
        (
            [(1., -1., -1.), (1., 1., -1.), (1., 1., 1.), (1., -1., 1.)],
            v3(1., 0., 0.),
        ),
        (
            [(-1., -1., -1.), (-1., -1., 1.), (-1., 1., 1.), (-1., 1., -1.)],
            v3(-1., 0., 0.),
        ),
        (
            [(-1., 1., -1.), (-1., 1., 1.), (1., 1., 1.), (1., 1., -1.)],
            v3(0., 1., 0.),
        ),
        (
            [(-1., -1., -1.), (1., -1., -1.), (1., -1., 1.), (-1., -1., 1.)],
            v3(0., -1., 0.),
        ),
        (
            [(-1., -1., 1.), (1., -1., 1.), (1., 1., 1.), (-1., 1., 1.)],
            v3(0., 0., 1.),
        ),
        (
            [(-1., -1., -1.), (-1., 1., -1.), (1., 1., -1.), (1., -1., -1.)],
            v3(0., 0., -1.),
        ),
    ];
    for (q, n) in faces {
        let poly: Vec<V3> = q.iter().map(|&(a, b, d)| corner(a, b, d)).collect();
        push_polygon(out, &poly, f.dir(n), col, m);
    }
}

/// A square bar of half thickness `t` from `a` to `b`, in the frame.
pub fn bar(out: &mut Vec<f32>, f: &Xf, a: V3, b: V3, t: f32, col: Rgb, m: f32) {
    let wa = f.p(a);
    let wb = f.p(b);
    let axis = wb - wa;
    let len = axis.length();
    if len < 1e-4 {
        return;
    }
    let d = axis * (1.0 / len);
    let helper = if d.z.abs() < 0.9 {
        v3(0.0, 0.0, 1.0)
    } else {
        v3(1.0, 0.0, 0.0)
    };
    let s = d.cross(helper).normalized();
    let u = s.cross(d).normalized();
    let k = (f.x.length() + f.y.length() + f.z.length()) / 3.0;
    let local = Xf::new((wa + wb) * 0.5, d, s, u);
    cuboid(
        out,
        &local,
        v3(0.0, 0.0, 0.0),
        v3(len * 0.5, t * k, t * k),
        col,
        m,
    );
}

/// A low-poly ball (an octahedron split once: 32 faces, flat shaded).
pub fn ball(out: &mut Vec<f32>, f: &Xf, c: V3, r: V3, col: Rgb, m: f32) {
    let axes = [v3(1., 0., 0.), v3(0., 1., 0.), v3(0., 0., 1.)];
    let p = |v: V3| f.p(c + v3(v.x * r.x, v.y * r.y, v.z * r.z));
    for sx in [-1.0f32, 1.0] {
        for sy in [-1.0f32, 1.0] {
            for sz in [-1.0f32, 1.0] {
                let a = axes[0] * sx;
                let b = axes[1] * sy;
                let d = axes[2] * sz;
                let ab = (a + b).normalized();
                let bd = (b + d).normalized();
                let da = (d + a).normalized();
                for tri in [[a, ab, da], [ab, b, bd], [da, bd, d], [ab, bd, da]] {
                    let poly = [p(tri[0]), p(tri[1]), p(tri[2])];
                    let n = (poly[1] - poly[0]).cross(poly[2] - poly[0]).normalized();
                    let centre = f.p(c);
                    let outward = if n.dot(poly[0] - centre) < 0.0 { -n } else { n };
                    push_polygon(out, &poly, outward, col, m);
                }
            }
        }
    }
}

/// How another surfer looks: their colours and what they hold.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub shirt: Rgb,
    pub shorts: Rgb,
    pub skin: Rgb,
    pub band: Rgb,
}

pub const SHIRTS: [Rgb; 8] = [
    [0.90, 0.25, 0.25],
    [0.20, 0.45, 0.85],
    [0.98, 0.78, 0.20],
    [0.25, 0.70, 0.40],
    [0.95, 0.55, 0.20],
    [0.60, 0.35, 0.80],
    [0.15, 0.70, 0.75],
    [0.95, 0.45, 0.65],
];
const SKINS: [Rgb; 5] = [
    [0.96, 0.80, 0.66],
    [0.87, 0.66, 0.50],
    [0.72, 0.52, 0.36],
    [0.55, 0.38, 0.26],
    [0.40, 0.27, 0.19],
];
const SHORTS: [Rgb; 3] = [[0.12, 0.16, 0.32], [0.20, 0.20, 0.22], [0.85, 0.85, 0.88]];

impl Look {
    pub fn from_seed(seed: u32) -> Look {
        let s = seed as usize;
        Look {
            shirt: SHIRTS[s % SHIRTS.len()],
            shorts: SHORTS[(s / 8) % SHORTS.len()],
            skin: SKINS[(s / 3) % SKINS.len()],
            band: SHIRTS[(s + 3) % SHIRTS.len()],
        }
    }
}

/// A kid surfing, origin at the feet, facing `yaw`, leaning `lean` degrees
/// (positive leans left). `ghost` draws them see-through.
pub fn kid(out: &mut Vec<f32>, origin: V3, yaw: f32, lean: f32, ducked: bool, look: &Look, ghost: bool) {
    let (f, r, _) = angle_vectors(0.0, yaw);
    let base = Xf::new(origin, f, -r, v3(0.0, 0.0, 1.0));
    // lean about the hips, the way a surfer leans into a ramp
    let hip = 30.0;
    let body = base.at(v3(0.0, 0.0, hip)).roll(lean).at(v3(0.0, 0.0, -hip));
    let m = if ghost { M_GHOST } else { M_KID };
    let tint = |c: Rgb| {
        if ghost {
            [c[0] * 0.6 + 0.4, c[1] * 0.6 + 0.4, c[2] * 0.6 + 0.4]
        } else {
            c
        }
    };
    let crouch = if ducked { 14.0 } else { 0.0 };
    let white = tint([0.96, 0.96, 0.96]);
    let skin = tint(look.skin);
    // sneakers and legs, feet apart in a surf stance
    for side in [-1.0f32, 1.0] {
        cuboid(
            out,
            &base,
            v3(1.0 + side * 2.0, side * 7.0, 2.5),
            v3(6.0, 3.0, 2.5),
            white,
            m,
        );
        bar(
            out,
            &body,
            v3(side * 2.0, side * 7.0, 4.0),
            v3(-1.0, side * 6.0, 22.0 - crouch * 0.5),
            2.6,
            skin,
            m,
        );
    }
    let up = -crouch;
    cuboid(
        out,
        &body,
        v3(0.0, 0.0, 26.0 + up * 0.5),
        v3(6.0, 9.5, 6.0),
        tint(look.shorts),
        m,
    );
    cuboid(
        out,
        &body,
        v3(0.0, 0.0, 42.0 + up),
        v3(6.5, 10.0, 10.5),
        tint(look.shirt),
        m,
    );
    // arms out for balance
    for side in [-1.0f32, 1.0] {
        let shoulder = v3(0.0, side * 10.5, 50.0 + up);
        let elbow = v3(2.0, side * 20.0, 46.0 + up);
        let hand = v3(4.0, side * 29.0, 44.0 + up);
        bar(out, &body, shoulder, elbow, 2.6, tint(look.shirt), m);
        bar(out, &body, elbow, hand, 2.2, skin, m);
    }
    // head, hair and a sweatband
    cuboid(out, &body, v3(0.0, 0.0, 61.0 + up), v3(5.5, 5.5, 6.5), skin, m);
    cuboid(
        out,
        &body,
        v3(-0.5, 0.0, 67.0 + up),
        v3(6.0, 6.0, 1.8),
        tint([0.25, 0.17, 0.10]),
        m,
    );
    cuboid(
        out,
        &body,
        v3(0.0, 0.0, 64.0 + up),
        v3(5.9, 5.9, 1.2),
        tint(look.band),
        m,
    );
    // a face: two eyes, looking where they go
    for side in [-1.0f32, 1.0] {
        cuboid(
            out,
            &body,
            v3(5.6, side * 2.2, 62.0 + up),
            v3(0.3, 0.9, 1.0),
            tint([0.1, 0.1, 0.12]),
            m,
        );
    }
}

/// What can be in your hands. Index 0 is the clasped hands you start with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
    Prayer,
    Noodle,
    Kickball,
    Duck,
    FoamFinger,
    Chicken,
}

impl Held {
    pub const ALL: [Held; 6] = [
        Held::Prayer,
        Held::Noodle,
        Held::Kickball,
        Held::Duck,
        Held::FoamFinger,
        Held::Chicken,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Held::Prayer => "prayer",
            Held::Noodle => "noodle",
            Held::Kickball => "kickball",
            Held::Duck => "duck",
            Held::FoamFinger => "foam_finger",
            Held::Chicken => "chicken",
        }
    }
    pub fn from_id(id: &str) -> Option<Held> {
        Held::ALL.iter().copied().find(|h| h.id() == id)
    }
}

#[allow(clippy::too_many_arguments)]
/// Your own hands, drawn in front of the eye. `cam` is the eye frame
/// (x forward, y left, z up); `squeeze` runs 0 to 1 while you squeeze;
/// `sway` is how far the hands trail the view (degrees, yaw and pitch).
pub fn hands(
    out: &mut Vec<f32>,
    cam: &Xf,
    held: Held,
    squeeze: f32,
    sway: (f32, f32),
    bob: f32,
    skin: Rgb,
    sleeve: Rgb,
) {
    let m = M_HAND;
    let f = cam.yaw(sway.0).pitch(sway.1).at(v3(0.0, 0.0, bob));
    match held {
        Held::Prayer => {
            // gassho: palms pressed together in front of the chest, fingers up
            let centre = f.at(v3(20.0, -1.5, -11.0)).yaw(28.0).pitch(22.0).scaled(0.72);
            for side in [-1.0f32, 1.0] {
                let palm = centre.at(v3(0.0, side * 0.7, 0.0)).roll(side * 6.0);
                cuboid(out, &palm, v3(0.0, 0.0, 0.0), v3(2.2, 0.65, 3.6), skin, m);
                // four fingers, slightly fanned, pressed to their twins
                for k in 0..4 {
                    let x = -1.65 + k as f32 * 1.1;
                    let tall = [3.0, 3.6, 3.4, 2.6][k];
                    cuboid(
                        out,
                        &palm,
                        v3(x, 0.0, 3.6 + tall * 0.5),
                        v3(0.48, 0.6, tall * 0.5),
                        skin,
                        m,
                    );
                }
                // thumb along the front edge
                bar(
                    out,
                    &palm,
                    v3(-2.0, side * 0.6, -2.4),
                    v3(-2.6, side * 0.4, 1.0),
                    0.6,
                    skin,
                    m,
                );
                // wrist and sleeve
                bar(
                    out,
                    &centre,
                    v3(0.3, side * 0.9, -3.4),
                    v3(-4.0, side * 5.5, -13.0),
                    1.5,
                    skin,
                    m,
                );
                bar(
                    out,
                    &centre,
                    v3(-3.2, side * 4.6, -11.0),
                    v3(-9.0, side * 9.0, -20.0),
                    2.6,
                    sleeve,
                    m,
                );
            }
        }
        _ => {
            // both hands cupped under the thing, a little to the right
            let hold = f.at(v3(19.0, -5.0, -11.0)).pitch(-8.0).scaled(0.7);
            for side in [-1.0f32, 1.0] {
                let hand = hold.at(v3(0.0, side * 2.2, -1.0)).roll(-side * 25.0);
                cuboid(out, &hand, v3(0.0, 0.0, 0.0), v3(2.4, 1.8, 0.8), skin, m);
                bar(
                    out,
                    &hold,
                    v3(-1.0, side * 2.6, -1.6),
                    v3(-6.0, side * 6.0, -10.0),
                    1.5,
                    skin,
                    m,
                );
                bar(
                    out,
                    &hold,
                    v3(-5.0, side * 5.0, -8.0),
                    v3(-10.0, side * 8.0, -17.0),
                    2.6,
                    sleeve,
                    m,
                );
            }
            let k = 1.0 - squeeze * 0.25;
            let item = hold.at(v3(0.0, 0.0, 1.0));
            thing(out, &item, held, k, m);
        }
    }
}

/// An item, standing on the frame's origin. `k` squashes it (a squeeze).
pub fn thing(out: &mut Vec<f32>, f: &Xf, held: Held, k: f32, m: f32) {
    let sq = Xf {
        z: f.z * k,
        y: f.y * (2.0 - k),
        x: f.x * (2.0 - k),
        ..*f
    };
    match held {
        Held::Prayer => {}
        Held::Noodle => {
            // a pool noodle, bending over the shoulder
            let green = [0.35, 0.85, 0.35];
            let mut prev = v3(0.0, 0.0, 0.0);
            for i in 1..9 {
                let t = i as f32;
                let p = v3(t * 0.6, -t * 0.25, t * 3.2 - t * t * 0.12);
                bar(out, &sq, prev, p, 1.6, green, m);
                prev = p;
            }
        }
        Held::Kickball => {
            ball(
                out,
                &sq,
                v3(0.0, 0.0, 4.5),
                v3(4.5, 4.5, 4.5),
                [0.86, 0.18, 0.16],
                m,
            );
            bar(
                out,
                &sq,
                v3(-4.6, 0.0, 4.5),
                v3(4.6, 0.0, 4.5),
                0.25,
                [0.6, 0.1, 0.1],
                m,
            );
        }
        Held::Duck => {
            let yellow = [1.0, 0.86, 0.18];
            ball(out, &sq, v3(0.0, 0.0, 2.6), v3(3.6, 3.0, 2.6), yellow, m);
            ball(out, &sq, v3(2.2, 0.0, 6.2), v3(2.0, 2.0, 2.0), yellow, m);
            cuboid(
                out,
                &sq,
                v3(4.4, 0.0, 6.0),
                v3(1.0, 0.8, 0.45),
                [0.98, 0.50, 0.12],
                m,
            );
            for side in [-1.0f32, 1.0] {
                cuboid(
                    out,
                    &sq,
                    v3(3.6, side * 1.1, 6.9),
                    v3(0.3, 0.35, 0.4),
                    [0.08, 0.08, 0.1],
                    m,
                );
            }
        }
        Held::FoamFinger => {
            let blue = [0.18, 0.42, 0.90];
            cuboid(out, &sq, v3(0.0, 0.0, 4.0), v3(1.6, 4.0, 4.0), blue, m);
            cuboid(out, &sq, v3(0.0, 1.6, 12.0), v3(1.4, 1.5, 4.5), blue, m);
            cuboid(out, &sq, v3(0.0, 4.4, 6.0), v3(1.3, 1.3, 2.0), blue, m);
            cuboid(out, &sq, v3(1.7, 0.0, 4.0), v3(0.2, 2.6, 1.6), [1.0, 1.0, 1.0], m);
        }
        Held::Chicken => {
            // the rubber chicken: long, pale, bald, with a red comb, mouth agape
            let body = [0.98, 0.90, 0.55];
            let orange = [0.98, 0.52, 0.12];
            ball(out, &sq, v3(0.0, 0.0, 3.0), v3(3.2, 2.4, 3.0), body, m);
            bar(out, &sq, v3(1.0, 0.0, 5.0), v3(3.0, 0.0, 13.0), 0.9, body, m);
            ball(out, &sq, v3(3.4, 0.0, 14.2), v3(1.8, 1.4, 1.6), body, m);
            cuboid(
                out,
                &sq,
                v3(3.4, 0.0, 16.2),
                v3(1.2, 0.3, 0.8),
                [0.90, 0.12, 0.14],
                m,
            );
            let gape = (1.0 - k) * 4.0;
            cuboid(
                out,
                &sq,
                v3(5.4, 0.0, 14.6 + gape * 0.1),
                v3(1.2, 0.5, 0.3),
                orange,
                m,
            );
            cuboid(
                out,
                &sq,
                v3(5.2, 0.0, 13.7 - gape * 0.2),
                v3(1.0, 0.45, 0.25),
                orange,
                m,
            );
            for side in [-1.0f32, 1.0] {
                cuboid(
                    out,
                    &sq,
                    v3(4.0, side * 1.15, 14.8),
                    v3(0.3, 0.2, 0.4),
                    [0.05, 0.05, 0.06],
                    m,
                );
                bar(
                    out,
                    &sq,
                    v3(-1.0, side * 1.2, 0.5),
                    v3(-2.5, side * 1.6, -4.0),
                    0.35,
                    orange,
                    m,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::FLOATS_PER_VERTEX;

    fn cam() -> Xf {
        Xf::new(
            v3(0.0, 0.0, 64.0),
            v3(1.0, 0.0, 0.0),
            v3(0.0, 1.0, 0.0),
            v3(0.0, 0.0, 1.0),
        )
    }

    #[test]
    fn every_held_thing_draws_and_stays_in_front_of_the_eye() {
        for held in Held::ALL {
            let mut out = Vec::new();
            hands(
                &mut out,
                &cam(),
                held,
                0.0,
                (0.0, 0.0),
                0.0,
                [0.9, 0.7, 0.6],
                [0.2, 0.4, 0.8],
            );
            assert!(!out.is_empty(), "{held:?}");
            assert_eq!(out.len() % (3 * FLOATS_PER_VERTEX), 0);
            for v in out.chunks(FLOATS_PER_VERTEX) {
                assert!(
                    v[0] > 4.0,
                    "{held:?}: a vertex behind the near plane at x {}",
                    v[0]
                );
            }
            assert_eq!(Held::from_id(held.id()), Some(held));
        }
    }

    #[test]
    fn the_prayer_hands_sit_low_in_the_middle_of_the_view() {
        let mut out = Vec::new();
        hands(
            &mut out,
            &cam(),
            Held::Prayer,
            0.0,
            (0.0, 0.0),
            0.0,
            [0.9, 0.7, 0.6],
            [0.2, 0.4, 0.8],
        );
        let n = (out.len() / FLOATS_PER_VERTEX) as f32;
        let mean_y: f32 = out.chunks(FLOATS_PER_VERTEX).map(|v| v[1]).sum::<f32>() / n;
        let top = out
            .chunks(FLOATS_PER_VERTEX)
            .map(|v| v[2])
            .fold(f32::MIN, f32::max);
        assert!(mean_y.abs() < 6.0, "near the middle: {mean_y}");
        assert!(top < 64.0, "below the eye, so the crosshair is clear: {top}");
    }

    #[test]
    fn a_kid_stands_about_as_tall_as_the_hull() {
        let mut out = Vec::new();
        kid(
            &mut out,
            v3(0.0, 0.0, 0.0),
            30.0,
            0.0,
            false,
            &Look::from_seed(3),
            false,
        );
        let zs: Vec<f32> = out.chunks(FLOATS_PER_VERTEX).map(|v| v[2]).collect();
        let top = zs.iter().cloned().fold(f32::MIN, f32::max);
        assert!(top > 60.0 && top < 74.0, "{top}");
    }
}
