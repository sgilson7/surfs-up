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
    Pinwheel,
    Ribbon,
    Lantern,
    Bell,
    Fox,
    Bucket,
    Camel,
    Pyramid,
    Scarab,
    Mummy,
    Jumprope,
    Cat,
    Glowstick,
    Sceptre,
    Jackal,
    Sunglasses,
    Lizard,
    Sandcastle,
    Hourglass,
    Cactus,
    PaperPlane,
    Balloon,
    Kite,
    Beanie,
    Eagle,
    PetRock,
    HardHat,
    Crystal,
    Crowbar,
    Scorpion,
    Carrot,
    Leaf,
    Pogo,
    Acorn,
    Bunny,
    Floaties,
    Whistle,
    GoldStar,
    SwimRing,
    GradCap,
    IceAxe,
    Snowflake,
    Compass,
    SummitFlag,
    Yeti,
    IceCream,
    BeachBall,
    FlipFlop,
    Watermelon,
    Crab,
    AlarmClock,
    CoffeeMug,
    Paperclip,
    Pencil,
    Owl,
    Sneaker,
    Feather,
    Spring,
    Stopwatch,
    Frog,
    ToyBrick,
    BlockFigure,
    ToyCar,
    ToyDino,
    ToyCrown,
    FoldingFan,
    Koi,
    Daruma,
    Bonsai,
    PaperCrane,
    BananaPeel,
    Lollipop,
    Whoopee,
    TopHat,
    Anvil,
}

impl Held {
    pub const ALL: [Held; 81] = [
        Held::Prayer,
        Held::Noodle,
        Held::Kickball,
        Held::Duck,
        Held::FoamFinger,
        Held::Chicken,
        Held::Pinwheel,
        Held::Ribbon,
        Held::Lantern,
        Held::Bell,
        Held::Fox,
        Held::Bucket,
        Held::Camel,
        Held::Pyramid,
        Held::Scarab,
        Held::Mummy,
        Held::Jumprope,
        Held::Cat,
        Held::Glowstick,
        Held::Sceptre,
        Held::Jackal,
        Held::Sunglasses,
        Held::Lizard,
        Held::Sandcastle,
        Held::Hourglass,
        Held::Cactus,
        Held::PaperPlane,
        Held::Balloon,
        Held::Kite,
        Held::Beanie,
        Held::Eagle,
        Held::PetRock,
        Held::HardHat,
        Held::Crystal,
        Held::Crowbar,
        Held::Scorpion,
        Held::Carrot,
        Held::Leaf,
        Held::Pogo,
        Held::Acorn,
        Held::Bunny,
        Held::Floaties,
        Held::Whistle,
        Held::GoldStar,
        Held::SwimRing,
        Held::GradCap,
        Held::IceAxe,
        Held::Snowflake,
        Held::Compass,
        Held::SummitFlag,
        Held::Yeti,
        Held::IceCream,
        Held::BeachBall,
        Held::FlipFlop,
        Held::Watermelon,
        Held::Crab,
        Held::AlarmClock,
        Held::CoffeeMug,
        Held::Paperclip,
        Held::Pencil,
        Held::Owl,
        Held::Sneaker,
        Held::Feather,
        Held::Spring,
        Held::Stopwatch,
        Held::Frog,
        Held::ToyBrick,
        Held::BlockFigure,
        Held::ToyCar,
        Held::ToyDino,
        Held::ToyCrown,
        Held::FoldingFan,
        Held::Koi,
        Held::Daruma,
        Held::Bonsai,
        Held::PaperCrane,
        Held::BananaPeel,
        Held::Lollipop,
        Held::Whoopee,
        Held::TopHat,
        Held::Anvil,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Held::Prayer => "prayer",
            Held::Noodle => "noodle",
            Held::Kickball => "kickball",
            Held::Duck => "duck",
            Held::FoamFinger => "foam_finger",
            Held::Chicken => "chicken",
            Held::Pinwheel => "pinwheel",
            Held::Ribbon => "ribbon",
            Held::Lantern => "lantern",
            Held::Bell => "bell",
            Held::Fox => "fox",
            Held::Bucket => "bucket",
            Held::Camel => "camel",
            Held::Pyramid => "pyramid",
            Held::Scarab => "scarab",
            Held::Mummy => "mummy",
            Held::Jumprope => "jumprope",
            Held::Cat => "cat",
            Held::Glowstick => "glowstick",
            Held::Sceptre => "sceptre",
            Held::Jackal => "jackal",
            Held::Sunglasses => "sunglasses",
            Held::Lizard => "lizard",
            Held::Sandcastle => "sandcastle",
            Held::Hourglass => "hourglass",
            Held::Cactus => "cactus",
            Held::PaperPlane => "paper_plane",
            Held::Balloon => "balloon",
            Held::Kite => "kite",
            Held::Beanie => "beanie",
            Held::Eagle => "eagle",
            Held::PetRock => "pet_rock",
            Held::HardHat => "hard_hat",
            Held::Crystal => "crystal",
            Held::Crowbar => "crowbar",
            Held::Scorpion => "scorpion",
            Held::Carrot => "carrot",
            Held::Leaf => "leaf",
            Held::Pogo => "pogo",
            Held::Acorn => "acorn",
            Held::Bunny => "bunny",
            Held::Floaties => "floaties",
            Held::Whistle => "whistle",
            Held::GoldStar => "gold_star",
            Held::SwimRing => "swim_ring",
            Held::GradCap => "grad_cap",
            Held::IceAxe => "ice_axe",
            Held::Snowflake => "snowflake",
            Held::Compass => "compass",
            Held::SummitFlag => "summit_flag",
            Held::Yeti => "yeti",
            Held::IceCream => "ice_cream",
            Held::BeachBall => "beach_ball",
            Held::FlipFlop => "flip_flop",
            Held::Watermelon => "watermelon",
            Held::Crab => "crab",
            Held::AlarmClock => "alarm_clock",
            Held::CoffeeMug => "coffee_mug",
            Held::Paperclip => "paperclip",
            Held::Pencil => "pencil",
            Held::Owl => "owl",
            Held::Sneaker => "sneaker",
            Held::Feather => "feather",
            Held::Spring => "spring",
            Held::Stopwatch => "stopwatch",
            Held::Frog => "frog",
            Held::ToyBrick => "toy_brick",
            Held::BlockFigure => "block_figure",
            Held::ToyCar => "toy_car",
            Held::ToyDino => "toy_dino",
            Held::ToyCrown => "toy_crown",
            Held::FoldingFan => "folding_fan",
            Held::Koi => "koi",
            Held::Daruma => "daruma",
            Held::Bonsai => "bonsai",
            Held::PaperCrane => "paper_crane",
            Held::BananaPeel => "banana_peel",
            Held::Lollipop => "lollipop",
            Held::Whoopee => "whoopee",
            Held::TopHat => "top_hat",
            Held::Anvil => "anvil",
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
        Held::Pinwheel => {
            // a paper pinwheel on a stick; a squeeze spins it
            bar(
                out,
                f,
                v3(0.0, 0.0, -2.0),
                v3(0.0, 0.0, 12.0),
                0.3,
                [0.85, 0.85, 0.88],
                m,
            );
            let hub = f.at(v3(0.6, 0.0, 12.0)).roll((1.0 - k) * 360.0);
            let colours = [
                [0.86, 0.24, 0.22],
                [0.98, 0.75, 0.18],
                [0.18, 0.42, 0.90],
                [0.25, 0.66, 0.40],
            ];
            for (i, col) in colours.iter().enumerate() {
                let blade = hub.roll(i as f32 * 90.0);
                cuboid(out, &blade, v3(0.0, 1.2, 2.4), v3(0.12, 1.2, 2.4), *col, m);
                cuboid(out, &blade, v3(0.0, 0.5, 4.2), v3(0.12, 0.5, 0.6), *col, m);
            }
            ball(
                out,
                &hub,
                v3(0.4, 0.0, 0.0),
                v3(0.6, 0.6, 0.6),
                [0.98, 0.98, 1.0],
                m,
            );
        }
        Held::Ribbon => {
            // a rhythmic-gymnastics ribbon: a short wand and a pink ribbon
            // trailing in loops; a squeeze flicks it higher
            bar(
                out,
                f,
                v3(0.0, 0.0, -1.0),
                v3(1.0, 0.0, 8.0),
                0.35,
                [0.95, 0.95, 0.97],
                m,
            );
            let pink = [0.95, 0.48, 0.58];
            let lift = (1.0 - k) * 8.0;
            let mut prev = v3(1.0, 0.0, 8.0);
            for i in 1..12 {
                let t = i as f32;
                let side = if i % 2 == 0 { 3.5 } else { -3.5 };
                let p = v3(
                    1.0 + t * 0.4,
                    side - t * 0.9,
                    8.0 + t * (1.4 + lift * 0.08) - t * t * 0.09,
                );
                bar(out, f, prev, p, 0.4, pink, m);
                prev = p;
            }
        }
        Held::Lantern => {
            // a round red paper lantern with black caps, a handle and a tassel
            let red = [0.86, 0.22, 0.18];
            let black = [0.1, 0.1, 0.12];
            ball(out, &sq, v3(0.0, 0.0, 5.0), v3(3.6, 3.6, 4.2), red, m);
            for z in [3.6f32, 6.4] {
                bar(
                    out,
                    &sq,
                    v3(-3.62, 0.0, z),
                    v3(3.62, 0.0, z),
                    0.18,
                    [0.62, 0.12, 0.1],
                    m,
                );
            }
            cuboid(out, &sq, v3(0.0, 0.0, 9.2), v3(1.8, 1.8, 0.4), black, m);
            cuboid(out, &sq, v3(0.0, 0.0, 0.8), v3(1.8, 1.8, 0.4), black, m);
            bar(out, f, v3(0.0, 0.0, 9.6), v3(0.0, 0.0, 13.0), 0.2, black, m);
            bar(
                out,
                f,
                v3(0.0, 0.0, 0.4),
                v3(0.0, 0.0, -2.5),
                0.35,
                [0.98, 0.75, 0.18],
                m,
            );
        }
        Held::Bell => {
            // a brass hand bell on a wooden handle; a squeeze swings the clapper
            let brass = [0.95, 0.74, 0.22];
            let wood = [0.62, 0.40, 0.22];
            bar(out, f, v3(0.0, 0.0, 5.0), v3(0.0, 0.0, 12.0), 0.7, wood, m);
            cuboid(out, f, v3(0.0, 0.0, 4.4), v3(1.4, 1.4, 0.8), brass, m);
            cuboid(out, f, v3(0.0, 0.0, 2.8), v3(2.3, 2.3, 1.0), brass, m);
            cuboid(out, f, v3(0.0, 0.0, 1.1), v3(3.2, 3.2, 0.8), brass, m);
            let swing = (1.0 - k) * 6.0;
            ball(
                out,
                f,
                v3(swing, 0.0, -0.4),
                v3(0.9, 0.9, 0.9),
                [0.7, 0.52, 0.16],
                m,
            );
        }
        Held::Fox => {
            // a fox plush: orange, white muzzle and chest, black-tipped ears
            // and nose, a big white-tipped tail
            let orange = [0.95, 0.50, 0.16];
            let white = [0.98, 0.96, 0.92];
            let black = [0.08, 0.08, 0.1];
            ball(out, &sq, v3(0.0, 0.0, 3.0), v3(3.0, 2.6, 3.0), orange, m);
            ball(out, &sq, v3(1.6, 0.0, 2.4), v3(1.6, 1.8, 2.0), white, m);
            ball(out, &sq, v3(0.6, 0.0, 7.6), v3(2.6, 2.6, 2.3), orange, m);
            ball(out, &sq, v3(2.8, 0.0, 7.0), v3(1.4, 1.2, 1.0), white, m);
            cuboid(out, &sq, v3(4.1, 0.0, 7.2), v3(0.35, 0.4, 0.35), black, m);
            for side in [-1.0f32, 1.0] {
                cuboid(out, &sq, v3(2.2, side * 1.1, 8.4), v3(0.3, 0.3, 0.35), black, m);
                let ear = sq.at(v3(0.4, side * 1.5, 9.8)).roll(side * 18.0);
                cuboid(out, &ear, v3(0.0, 0.0, 0.0), v3(0.5, 0.9, 1.3), orange, m);
                cuboid(out, &ear, v3(0.0, 0.0, 1.3), v3(0.45, 0.6, 0.4), black, m);
                cuboid(out, &sq, v3(1.5, side * 1.4, 0.4), v3(0.9, 0.7, 0.5), orange, m);
            }
            ball(out, &sq, v3(-3.2, 1.8, 3.8), v3(1.8, 1.6, 3.2), orange, m);
            ball(out, &sq, v3(-3.6, 2.2, 7.0), v3(1.2, 1.1, 1.2), white, m);
        }
        Held::Bucket => {
            // a sandpit bucket, full of sand, with its handle and a spade
            let blue = [0.18, 0.55, 0.92];
            let sand = [0.93, 0.80, 0.52];
            cuboid(out, &sq, v3(0.0, 0.0, 1.2), v3(2.6, 2.6, 1.2), blue, m);
            cuboid(out, &sq, v3(0.0, 0.0, 3.6), v3(3.1, 3.1, 1.3), blue, m);
            cuboid(out, &sq, v3(0.0, 0.0, 4.9), v3(2.8, 2.8, 0.2), sand, m);
            ball(out, &sq, v3(0.0, 0.0, 5.1), v3(2.2, 2.2, 0.9), sand, m);
            for side in [-1.0f32, 1.0] {
                bar(
                    out,
                    &sq,
                    v3(0.0, side * 3.1, 4.6),
                    v3(0.0, side * 2.0, 8.0),
                    0.2,
                    [0.9, 0.9, 0.92],
                    m,
                );
            }
            bar(
                out,
                &sq,
                v3(0.0, -2.0, 8.0),
                v3(0.0, 2.0, 8.0),
                0.2,
                [0.9, 0.9, 0.92],
                m,
            );
            bar(
                out,
                &sq,
                v3(1.0, 0.6, 4.8),
                v3(2.6, 1.4, 10.0),
                0.3,
                [0.95, 0.30, 0.25],
                m,
            );
            cuboid(
                out,
                &sq.at(v3(2.8, 1.5, 10.8)),
                v3(0.0, 0.0, 0.0),
                v3(0.25, 1.0, 1.3),
                [0.95, 0.30, 0.25],
                m,
            );
        }
        Held::Camel => {
            // a toy camel: fawn, two humps, long neck, a small head
            let fawn = [0.84, 0.64, 0.38];
            let dark = [0.55, 0.38, 0.20];
            cuboid(out, &sq, v3(0.0, 0.0, 4.5), v3(3.6, 1.6, 1.4), fawn, m);
            ball(out, &sq, v3(-1.6, 0.0, 6.2), v3(1.3, 1.2, 1.3), fawn, m);
            ball(out, &sq, v3(1.4, 0.0, 6.2), v3(1.3, 1.2, 1.3), fawn, m);
            bar(out, &sq, v3(3.2, 0.0, 5.2), v3(5.0, 0.0, 9.0), 0.7, fawn, m);
            cuboid(out, &sq, v3(5.8, 0.0, 9.3), v3(1.4, 0.8, 0.8), fawn, m);
            for side in [-1.0f32, 1.0] {
                cuboid(
                    out,
                    &sq,
                    v3(6.3, side * 0.85, 9.6),
                    v3(0.2, 0.1, 0.2),
                    [0.08, 0.08, 0.1],
                    m,
                );
                for x in [-2.6f32, 2.6] {
                    bar(
                        out,
                        &sq,
                        v3(x, side * 1.0, 3.2),
                        v3(x, side * 1.0, 0.0),
                        0.4,
                        fawn,
                        m,
                    );
                }
            }
            bar(out, &sq, v3(-3.6, 0.0, 5.0), v3(-4.6, 0.0, 3.0), 0.2, dark, m);
        }
        Held::Pyramid => {
            // a toy pyramid of sand-coloured blocks, four steps, a gold cap
            let stone = [0.90, 0.76, 0.48];
            for (i, h) in [4.0f32, 3.1, 2.2, 1.3].iter().enumerate() {
                let shade = 1.0 - i as f32 * 0.05;
                cuboid(
                    out,
                    &sq,
                    v3(0.0, 0.0, 0.9 + i as f32 * 1.8),
                    v3(*h, *h, 0.9),
                    [stone[0] * shade, stone[1] * shade, stone[2] * shade],
                    m,
                );
            }
            cuboid(
                out,
                &sq,
                v3(0.0, 0.0, 8.0),
                v3(0.5, 0.5, 0.5),
                [1.0, 0.82, 0.20],
                m,
            );
        }
        Held::Scarab => {
            // a golden scarab beetle: a shiny shell, a head, six legs
            let gold = [1.0, 0.78, 0.18];
            let deep = [0.62, 0.42, 0.08];
            ball(out, &sq, v3(0.0, 0.0, 2.2), v3(3.0, 2.4, 1.8), gold, m);
            bar(out, &sq, v3(-2.8, 0.0, 3.9), v3(2.0, 0.0, 3.9), 0.12, deep, m);
            ball(out, &sq, v3(3.4, 0.0, 2.2), v3(1.2, 1.4, 1.0), gold, m);
            for side in [-1.0f32, 1.0] {
                for x in [-1.6f32, 0.0, 1.6] {
                    bar(
                        out,
                        &sq,
                        v3(x, side * 2.0, 1.4),
                        v3(x + 0.6, side * 3.6, 0.2),
                        0.2,
                        deep,
                        m,
                    );
                }
                bar(
                    out,
                    &sq,
                    v3(4.2, side * 0.6, 2.6),
                    v3(5.4, side * 1.4, 3.6),
                    0.15,
                    deep,
                    m,
                );
            }
        }
        Held::Jumprope => {
            // a skipping rope: two red handles, the rope looping down between
            let red = [0.90, 0.22, 0.22];
            let rope = [0.98, 0.86, 0.30];
            bar(out, &sq, v3(0.0, -1.2, 0.0), v3(0.0, -1.2, 5.0), 0.6, red, m);
            bar(out, &sq, v3(1.5, 1.6, 0.5), v3(2.2, 2.4, 5.0), 0.6, red, m);
            let lift = (1.0 - k) * 6.0;
            let mut prev = v3(0.0, -1.2, 5.0);
            for i in 1..=10 {
                let t = i as f32 / 10.0;
                let sag = 4.0 * t * (1.0 - t);
                let p = v3(2.2 * t + sag * 2.5, -1.2 + 3.6 * t, 5.0 + sag * (3.0 + lift));
                bar(out, &sq, prev, p, 0.18, rope, m);
                prev = p;
            }
        }
        Held::Cat => {
            // a toy Egyptian cat: black, sitting tall, gold collar and ear
            let black = [0.14, 0.13, 0.16];
            let gold = [1.0, 0.78, 0.18];
            cuboid(out, &sq, v3(0.0, 0.0, 2.0), v3(1.8, 1.4, 2.0), black, m);
            cuboid(out, &sq, v3(0.6, 0.0, 5.0), v3(1.2, 1.1, 1.4), black, m);
            cuboid(out, &sq, v3(1.1, 0.0, 7.2), v3(1.3, 1.2, 1.1), black, m);
            cuboid(out, &sq, v3(0.8, 0.0, 6.2), v3(1.3, 1.25, 0.25), gold, m);
            for side in [-1.0f32, 1.0] {
                cuboid(out, &sq, v3(1.0, side * 0.8, 8.8), v3(0.35, 0.3, 0.7), black, m);
                cuboid(
                    out,
                    &sq,
                    v3(2.4, side * 0.5, 7.5),
                    v3(0.1, 0.25, 0.15),
                    [0.30, 0.85, 0.40],
                    m,
                );
            }
            cuboid(out, &sq, v3(1.0, 0.8, 8.4), v3(0.2, 0.15, 0.15), gold, m);
            bar(out, &sq, v3(-1.8, 0.0, 0.6), v3(-2.6, 1.6, 0.4), 0.35, black, m);
        }
        Held::Glowstick => {
            // a glow stick, bright green; a squeeze cracks it brighter
            let glow = 0.75 + (1.0 - k) * 1.0;
            bar(
                out,
                f,
                v3(0.0, 0.0, -1.0),
                v3(1.5, 0.0, 12.0),
                0.55,
                [0.35 * glow, 1.0, 0.30 * glow],
                crate::mesh::M_LIGHT,
            );
            bar(
                out,
                f,
                v3(1.5, 0.0, 12.0),
                v3(1.65, 0.0, 13.2),
                0.35,
                [0.85, 0.85, 0.9],
                m,
            );
        }
        Held::Sceptre => {
            // a toy sceptre: a gold and blue striped staff with a hooked top
            let gold = [1.0, 0.80, 0.22];
            let blue = [0.15, 0.32, 0.80];
            for i in 0..6 {
                let z0 = -1.0 + i as f32 * 2.0;
                let col = if i % 2 == 0 { gold } else { blue };
                bar(out, f, v3(0.0, 0.0, z0), v3(0.0, 0.0, z0 + 2.0), 0.45, col, m);
            }
            bar(out, f, v3(0.0, 0.0, 11.0), v3(1.6, 0.0, 12.6), 0.45, gold, m);
            bar(out, f, v3(1.6, 0.0, 12.6), v3(2.8, 0.0, 12.0), 0.45, gold, m);
            bar(out, f, v3(2.8, 0.0, 12.0), v3(2.9, 0.0, 10.8), 0.45, blue, m);
        }
        Held::Jackal => {
            // a jackal plush: black, tall pointed ears, a long snout, gold
            // collar and gold-trimmed ears
            let black = [0.12, 0.12, 0.15];
            let gold = [1.0, 0.78, 0.18];
            ball(out, &sq, v3(0.0, 0.0, 3.0), v3(2.8, 2.4, 3.0), black, m);
            cuboid(out, &sq, v3(0.4, 0.0, 6.2), v3(1.9, 2.0, 0.35), gold, m);
            ball(out, &sq, v3(0.6, 0.0, 8.0), v3(2.2, 2.0, 1.9), black, m);
            cuboid(out, &sq, v3(3.2, 0.0, 7.4), v3(1.4, 0.7, 0.6), black, m);
            cuboid(
                out,
                &sq,
                v3(4.7, 0.0, 7.6),
                v3(0.2, 0.35, 0.3),
                [0.05, 0.05, 0.06],
                m,
            );
            for side in [-1.0f32, 1.0] {
                let ear = sq.at(v3(0.2, side * 1.2, 10.8)).roll(side * 10.0);
                cuboid(out, &ear, v3(0.0, 0.0, 0.0), v3(0.45, 0.7, 1.8), black, m);
                cuboid(out, &ear, v3(0.46, 0.0, 0.0), v3(0.05, 0.4, 1.4), gold, m);
                cuboid(out, &sq, v3(2.2, side * 0.9, 8.6), v3(0.2, 0.3, 0.25), gold, m);
                cuboid(out, &sq, v3(1.4, side * 1.5, 0.4), v3(1.0, 0.7, 0.5), black, m);
            }
        }
        Held::Sunglasses => {
            // big star-shaped party sunglasses, held up by one arm
            let pink = [0.98, 0.38, 0.62];
            let lens = [0.10, 0.12, 0.20];
            for side in [-1.0f32, 1.0] {
                cuboid(out, &sq, v3(0.0, side * 2.0, 6.0), v3(0.4, 1.8, 1.4), pink, m);
                cuboid(out, &sq, v3(0.0, side * 2.0, 6.0), v3(0.25, 1.9, 0.5), pink, m);
                cuboid(out, &sq, v3(0.0, side * 2.0, 6.0), v3(0.25, 0.5, 1.9), pink, m);
                cuboid(out, &sq, v3(0.3, side * 2.0, 6.0), v3(0.2, 1.3, 1.0), lens, m);
                bar(
                    out,
                    &sq,
                    v3(-0.2, side * 3.7, 6.4),
                    v3(-4.0, side * 3.7, 5.6),
                    0.25,
                    pink,
                    m,
                );
            }
            cuboid(out, &sq, v3(0.0, 0.0, 6.3), v3(0.3, 0.5, 0.25), pink, m);
            bar(out, &sq, v3(0.0, 2.0, 4.6), v3(0.0, 2.0, 0.0), 0.3, pink, m);
        }
        Held::Lizard => {
            // a toy lizard: long green body, four splayed legs, a curling tail
            let green = [0.40, 0.78, 0.32];
            let dark = [0.22, 0.52, 0.20];
            cuboid(out, &sq, v3(0.0, 0.0, 1.0), v3(2.4, 1.0, 0.6), green, m);
            cuboid(out, &sq, v3(3.2, 0.0, 1.2), v3(1.1, 0.8, 0.6), green, m);
            for side in [-1.0f32, 1.0] {
                cuboid(
                    out,
                    &sq,
                    v3(3.8, side * 0.6, 1.6),
                    v3(0.2, 0.15, 0.2),
                    [0.05, 0.05, 0.06],
                    m,
                );
                for x in [-1.6f32, 1.6] {
                    bar(
                        out,
                        &sq,
                        v3(x, side * 0.9, 1.0),
                        v3(x + 0.6, side * 2.4, 0.2),
                        0.3,
                        dark,
                        m,
                    );
                }
            }
            let mut prev = v3(-2.4, 0.0, 1.0);
            for i in 1..7 {
                let t = i as f32;
                let p = v3(-2.4 - t * 0.8, (t * 0.35) * (t * 0.35) * 0.4, 1.0 + t * 0.15);
                bar(out, &sq, prev, p, 0.45 - t * 0.04, green, m);
                prev = p;
            }
        }
        Held::Sandcastle => {
            // a sandcastle: a keep, four corner towers, a little red flag
            let sand = [0.93, 0.80, 0.52];
            let shade = [0.82, 0.68, 0.42];
            cuboid(out, &sq, v3(0.0, 0.0, 1.6), v3(3.0, 3.0, 1.6), sand, m);
            cuboid(out, &sq, v3(0.0, 0.0, 4.2), v3(1.5, 1.5, 1.2), shade, m);
            for (x, y) in [(-2.6f32, -2.6f32), (2.6, -2.6), (-2.6, 2.6), (2.6, 2.6)] {
                cuboid(out, &sq, v3(x, y, 2.4), v3(0.8, 0.8, 2.4), sand, m);
                cuboid(out, &sq, v3(x, y, 5.1), v3(0.9, 0.9, 0.3), shade, m);
            }
            bar(
                out,
                &sq,
                v3(0.0, 0.0, 5.4),
                v3(0.0, 0.0, 9.0),
                0.12,
                [0.9, 0.9, 0.9],
                m,
            );
            cuboid(
                out,
                &sq,
                v3(0.0, 0.9, 8.4),
                v3(0.05, 0.8, 0.5),
                [0.92, 0.22, 0.22],
                m,
            );
        }
        Held::Hourglass => {
            // an hourglass: gold ends, glass bulbs, sand running down
            let gold = [1.0, 0.78, 0.20];
            let glass = [0.80, 0.90, 0.95];
            let sand = [0.95, 0.78, 0.40];
            let run = 1.0 - k;
            cuboid(out, &sq, v3(0.0, 0.0, 0.3), v3(2.4, 2.4, 0.3), gold, m);
            cuboid(out, &sq, v3(0.0, 0.0, 9.7), v3(2.4, 2.4, 0.3), gold, m);
            ball(out, &sq, v3(0.0, 0.0, 2.8), v3(1.9, 1.9, 2.3), glass, m);
            ball(out, &sq, v3(0.0, 0.0, 7.2), v3(1.9, 1.9, 2.3), glass, m);
            ball(
                out,
                &sq,
                v3(0.0, 0.0, 1.8 + run),
                v3(1.6, 1.6, 1.2 + run),
                sand,
                m,
            );
            ball(
                out,
                &sq,
                v3(0.0, 0.0, 7.6),
                v3(1.3, 1.3, 1.0 - run * 0.5),
                sand,
                m,
            );
            for (x, y) in [(-2.1f32, -2.1f32), (2.1, -2.1), (-2.1, 2.1), (2.1, 2.1)] {
                bar(out, &sq, v3(x, y, 0.6), v3(x, y, 9.4), 0.2, gold, m);
            }
        }
        Held::Cactus => {
            // a cactus plush in a little pot: two arms, a pink flower
            let green = [0.30, 0.66, 0.36];
            let pot = [0.82, 0.42, 0.24];
            cuboid(out, &sq, v3(0.0, 0.0, 1.2), v3(2.0, 2.0, 1.2), pot, m);
            cuboid(out, &sq, v3(0.0, 0.0, 6.4), v3(1.3, 1.3, 4.0), green, m);
            for side in [-1.0f32, 1.0] {
                cuboid(out, &sq, v3(0.0, side * 2.0, 6.0), v3(0.7, 0.8, 0.6), green, m);
                cuboid(out, &sq, v3(0.0, side * 2.6, 7.6), v3(0.7, 0.6, 1.6), green, m);
                cuboid(
                    out,
                    &sq,
                    v3(1.35, side * 0.5, 7.4),
                    v3(0.1, 0.15, 0.2),
                    [0.05, 0.05, 0.06],
                    m,
                );
            }
            ball(
                out,
                &sq,
                v3(0.0, 0.0, 10.8),
                v3(0.9, 0.9, 0.6),
                [0.98, 0.45, 0.68],
                m,
            );
        }
        Held::PaperPlane => {
            // a paper plane: a long folded keel and two swept wings
            let paper = [0.96, 0.96, 0.94];
            let crease = [0.80, 0.82, 0.86];
            for side in [-1.0f32, 1.0] {
                for i in 0..5 {
                    let t = i as f32;
                    cuboid(
                        out,
                        &sq,
                        v3(4.0 - t * 1.6, side * (0.4 + t * 0.55), 5.0 + t * 0.05),
                        v3(0.8, 0.3 + t * 0.55, 0.08),
                        paper,
                        m,
                    );
                }
            }
            bar(out, &sq, v3(4.8, 0.0, 5.0), v3(-3.6, 0.0, 3.8), 0.35, crease, m);
            bar(out, &sq, v3(0.0, 0.0, 0.0), v3(0.0, 0.0, 4.4), 0.25, crease, m);
        }
        Held::Balloon => {
            // a red balloon on a string, a little knot under it
            let red = [0.90, 0.20, 0.22];
            bar(
                out,
                &sq,
                v3(0.0, 0.0, 0.0),
                v3(0.4, 0.0, 7.0),
                0.06,
                [0.9, 0.9, 0.9],
                m,
            );
            cuboid(out, &sq, v3(0.4, 0.0, 7.3), v3(0.3, 0.3, 0.3), red, m);
            ball(out, &sq, v3(0.4, 0.0, 11.0), v3(3.0, 3.0, 3.6), red, m);
            ball(
                out,
                &sq,
                v3(1.8, 1.2, 12.4),
                v3(0.5, 0.5, 0.7),
                [0.98, 0.55, 0.55],
                m,
            );
        }
        Held::Kite => {
            // a diamond kite on two cross spars, a bow-tied tail
            let a = [0.18, 0.45, 0.85];
            let b = [0.98, 0.78, 0.18];
            let top = v3(0.0, 0.0, 12.0);
            let bottom = v3(0.0, 0.0, 2.0);
            let left = v3(0.0, 3.2, 9.0);
            let right = v3(0.0, -3.2, 9.0);
            for (p, q, col) in [
                (top, left, a),
                (left, bottom, b),
                (bottom, right, a),
                (right, top, b),
            ] {
                let mid = (p + q + v3(0.0, 0.0, 9.0)) * (1.0 / 3.0);
                bar(out, &sq, p, mid, 0.9, col, m);
                bar(out, &sq, q, mid, 0.9, col, m);
            }
            bar(out, &sq, top, bottom, 0.15, [0.55, 0.38, 0.22], m);
            bar(out, &sq, left, right, 0.15, [0.55, 0.38, 0.22], m);
            for i in 0..3 {
                let z = 1.2 - i as f32 * 1.0;
                cuboid(
                    out,
                    &sq,
                    v3(0.0, 0.0, z),
                    v3(0.1, 0.5, 0.15),
                    [0.92, 0.22, 0.22],
                    m,
                );
            }
        }
        Held::Beanie => {
            // a propeller beanie: four coloured panels, a brim, a spinning prop
            let cols = [
                [0.90, 0.22, 0.20],
                [0.98, 0.78, 0.18],
                [0.20, 0.55, 0.90],
                [0.30, 0.72, 0.36],
            ];
            for (i, col) in cols.iter().enumerate() {
                let (x, y) = [(1.0f32, 1.0f32), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)][i];
                ball(out, &sq, v3(x * 1.2, y * 1.2, 2.6), v3(1.8, 1.8, 2.2), *col, m);
            }
            cuboid(out, &sq, v3(2.8, 0.0, 1.2), v3(1.2, 2.4, 0.15), cols[0], m);
            bar(
                out,
                &sq,
                v3(0.0, 0.0, 4.6),
                v3(0.0, 0.0, 6.0),
                0.2,
                [0.85, 0.85, 0.85],
                m,
            );
            let prop = sq.at(v3(0.0, 0.0, 6.1)).yaw((1.0 - k) * 690.0);
            cuboid(out, &prop, v3(0.0, 0.0, 0.0), v3(3.2, 0.4, 0.08), cols[1], m);
        }
        Held::Eagle => {
            // an eagle plush: brown body and wings, white head, yellow beak and feet
            let brown = [0.45, 0.30, 0.18];
            let white = [0.96, 0.95, 0.92];
            let yellow = [0.98, 0.78, 0.18];
            ball(out, &sq, v3(0.0, 0.0, 4.4), v3(2.6, 2.4, 3.4), brown, m);
            for side in [-1.0f32, 1.0] {
                ball(out, &sq, v3(-0.4, side * 2.6, 4.8), v3(1.6, 0.8, 2.8), brown, m);
                cuboid(out, &sq, v3(0.8, side * 0.9, 0.4), v3(0.6, 0.4, 0.4), yellow, m);
                cuboid(
                    out,
                    &sq,
                    v3(2.0, side * 0.75, 9.6),
                    v3(0.15, 0.3, 0.3),
                    [0.08, 0.08, 0.1],
                    m,
                );
            }
            ball(out, &sq, v3(0.4, 0.0, 9.2), v3(2.0, 2.0, 2.0), white, m);
            bar(out, &sq, v3(2.0, 0.0, 9.0), v3(3.2, 0.0, 8.4), 0.5, yellow, m);
        }
        Held::PetRock => {
            // a pet rock: a grey stone with googly eyes and a little blue bow
            let grey = [0.55, 0.55, 0.58];
            ball(out, &sq, v3(0.0, 0.0, 2.6), v3(3.4, 3.0, 2.6), grey, m);
            ball(
                out,
                &sq,
                v3(-1.0, 1.0, 3.6),
                v3(1.6, 1.4, 1.2),
                [0.62, 0.62, 0.65],
                m,
            );
            for side in [-1.0f32, 1.0] {
                ball(
                    out,
                    &sq,
                    v3(2.9, side * 0.9, 3.6),
                    v3(0.5, 0.7, 0.7),
                    [0.98, 0.98, 0.98],
                    m,
                );
                cuboid(
                    out,
                    &sq,
                    v3(3.4, side * 0.9, 3.5),
                    v3(0.1, 0.25, 0.25),
                    [0.05, 0.05, 0.06],
                    m,
                );
            }
            cuboid(
                out,
                &sq,
                v3(0.6, 0.0, 5.2),
                v3(0.4, 0.9, 0.3),
                [0.20, 0.45, 0.88],
                m,
            );
        }
        Held::HardHat => {
            // a yellow hard hat: a dome, a ridge on top, a brim all round
            let yellow = [0.98, 0.80, 0.12];
            let dark = [0.86, 0.66, 0.08];
            cuboid(out, &sq, v3(0.4, 0.0, 0.8), v3(4.2, 3.6, 0.3), dark, m);
            ball(out, &sq, v3(0.0, 0.0, 1.0), v3(3.2, 3.0, 3.4), yellow, m);
            cuboid(out, &sq, v3(0.0, 0.0, 4.2), v3(2.6, 0.4, 0.3), dark, m);
            cuboid(
                out,
                &sq,
                v3(3.2, 0.0, 2.0),
                v3(0.1, 0.9, 0.6),
                [0.96, 0.96, 0.96],
                m,
            );
        }
        Held::Crystal => {
            // a glowing crystal cluster: a tall point and two short ones on a rock
            let glow = [0.55, 0.95, 0.85];
            let deep = [0.30, 0.75, 0.70];
            ball(
                out,
                &sq,
                v3(0.0, 0.0, 0.8),
                v3(2.6, 2.4, 0.8),
                [0.48, 0.42, 0.40],
                m,
            );
            bar(out, &sq, v3(0.0, 0.0, 1.0), v3(0.0, 0.0, 9.0), 1.0, glow, m);
            bar(out, &sq, v3(0.0, 0.0, 9.0), v3(0.0, 0.0, 11.0), 0.45, glow, m);
            for (y, h) in [(1.6f32, 5.0f32), (-1.5, 4.0)] {
                bar(out, &sq, v3(0.4, y * 0.6, 1.0), v3(0.6, y * 1.4, h), 0.7, deep, m);
            }
        }
        Held::Crowbar => {
            // a foam crowbar: a long orange bar, hooked and split at the top
            let orange = [0.95, 0.45, 0.12];
            bar(out, &sq, v3(0.0, 0.0, 0.0), v3(0.0, 0.0, 10.0), 0.45, orange, m);
            bar(out, &sq, v3(0.0, 0.0, 10.0), v3(1.4, 0.0, 11.6), 0.45, orange, m);
            bar(out, &sq, v3(1.4, 0.0, 11.6), v3(2.8, 0.0, 11.0), 0.45, orange, m);
            bar(out, &sq, v3(0.0, 0.0, 0.0), v3(0.9, 0.0, -0.6), 0.45, orange, m);
            cuboid(
                out,
                &sq,
                v3(0.0, 0.0, 3.0),
                v3(0.55, 0.55, 1.2),
                [0.15, 0.15, 0.18],
                m,
            );
        }
        Held::Scorpion => {
            // a scorpion plush: a sandy body, two claws forward, a curled tail
            let body = [0.86, 0.62, 0.30];
            let dark = [0.66, 0.44, 0.20];
            ball(out, &sq, v3(0.0, 0.0, 1.4), v3(2.2, 1.6, 1.0), body, m);
            for side in [-1.0f32, 1.0] {
                bar(
                    out,
                    &sq,
                    v3(1.6, side * 1.0, 1.4),
                    v3(3.4, side * 2.2, 1.6),
                    0.4,
                    dark,
                    m,
                );
                ball(out, &sq, v3(4.0, side * 2.4, 1.7), v3(0.9, 0.6, 0.5), body, m);
                for x in [-1.0f32, 0.0, 1.0] {
                    bar(
                        out,
                        &sq,
                        v3(x, side * 1.2, 1.2),
                        v3(x - 0.4, side * 2.4, 0.2),
                        0.2,
                        dark,
                        m,
                    );
                }
                cuboid(
                    out,
                    &sq,
                    v3(2.1, side * 0.5, 1.9),
                    v3(0.15, 0.2, 0.2),
                    [0.05, 0.05, 0.06],
                    m,
                );
            }
            let mut prev = v3(-2.0, 0.0, 1.6);
            for i in 1..6 {
                let (sn, c) = sin_cos_deg(i as f64 * 31.5);
                let p = v3(-2.2 - sn as f32 * 2.2, 0.0, 1.6 + (1.0 - c as f32) * 3.0);
                bar(out, &sq, prev, p, 0.55 - i as f32 * 0.05, body, m);
                prev = p;
            }
            ball(out, &sq, prev + v3(0.6, 0.0, 0.2), v3(0.5, 0.4, 0.5), dark, m);
        }
        Held::Carrot => {
            // a toy carrot: an orange taper, ridges, green leaves on top
            let orange = [0.96, 0.52, 0.12];
            for i in 0..6 {
                let t = i as f32;
                let r = 0.4 + t * 0.32;
                cuboid(out, &sq, v3(0.0, 0.0, 0.6 + t * 1.2), v3(r, r, 0.6), orange, m);
            }
            for (y, lean) in [(-0.6f32, -1.6f32), (0.0, 0.0), (0.6, 1.6)] {
                bar(
                    out,
                    &sq,
                    v3(0.0, y, 7.2),
                    v3(0.0, y + lean, 10.4),
                    0.3,
                    [0.30, 0.70, 0.30],
                    m,
                );
            }
        }
        Held::Leaf => {
            // a big maple leaf on its stem: five lobes, red and orange
            let red = [0.88, 0.25, 0.14];
            let orange = [0.95, 0.52, 0.16];
            bar(
                out,
                &sq,
                v3(0.0, 0.0, 0.0),
                v3(0.0, 0.0, 5.0),
                0.2,
                [0.45, 0.28, 0.16],
                m,
            );
            ball(out, &sq, v3(0.0, 0.0, 7.0), v3(0.25, 2.0, 2.0), red, m);
            for (y, z, col) in [
                (0.0f32, 10.0f32, red),
                (2.6, 8.4, orange),
                (-2.6, 8.4, orange),
                (2.0, 5.6, red),
                (-2.0, 5.6, red),
            ] {
                ball(out, &sq, v3(0.0, y, z), v3(0.2, 1.3, 1.6), col, m);
            }
        }
        Held::Pogo => {
            // a pogo stick: a pole, a spring that squashes with you, foot pegs, a grip bar
            let red = [0.88, 0.20, 0.20];
            let steel = [0.75, 0.77, 0.80];
            bar(
                out,
                &sq,
                v3(0.0, 0.0, 0.0),
                v3(0.0, 0.0, 2.6),
                0.25,
                [0.12, 0.12, 0.14],
                m,
            );
            for i in 0..5 {
                let z = 0.6 + i as f32 * 0.4 * k;
                cuboid(out, &sq, v3(0.0, 0.0, z), v3(0.8, 0.8, 0.08), steel, m);
            }
            bar(out, &sq, v3(0.0, 0.0, 2.6), v3(0.0, 0.0, 11.0), 0.45, red, m);
            cuboid(out, &sq, v3(0.0, 0.0, 3.4), v3(0.4, 1.8, 0.25), steel, m);
            cuboid(
                out,
                &sq,
                v3(0.0, 0.0, 11.0),
                v3(0.4, 2.4, 0.35),
                [0.12, 0.12, 0.14],
                m,
            );
        }
        Held::Acorn => {
            // an acorn: a brown nut, a scaly cap, a stalk
            let nut = [0.72, 0.48, 0.24];
            let cap = [0.48, 0.32, 0.18];
            ball(out, &sq, v3(0.0, 0.0, 3.2), v3(2.4, 2.4, 3.0), nut, m);
            cuboid(out, &sq, v3(0.0, 0.0, 0.3), v3(0.3, 0.3, 0.3), cap, m);
            ball(out, &sq, v3(0.0, 0.0, 6.0), v3(2.8, 2.8, 1.4), cap, m);
            bar(out, &sq, v3(0.0, 0.0, 7.2), v3(0.6, 0.0, 8.6), 0.25, cap, m);
            ball(
                out,
                &sq,
                v3(1.6, 1.0, 4.0),
                v3(0.5, 0.4, 0.9),
                [0.84, 0.62, 0.36],
                m,
            );
        }
        Held::Bunny => {
            // a bunny plush: white body and head, long ears with pink insides, a round tail
            let white = [0.96, 0.95, 0.94];
            let pink = [0.98, 0.62, 0.70];
            ball(out, &sq, v3(0.0, 0.0, 3.0), v3(2.6, 2.4, 3.0), white, m);
            ball(out, &sq, v3(0.6, 0.0, 7.4), v3(2.2, 2.2, 2.0), white, m);
            ball(out, &sq, v3(-2.6, 0.0, 2.0), v3(0.9, 0.9, 0.9), white, m);
            for side in [-1.0f32, 1.0] {
                ball(out, &sq, v3(0.2, side * 1.0, 11.0), v3(0.6, 0.6, 2.4), white, m);
                ball(out, &sq, v3(0.6, side * 1.0, 11.0), v3(0.3, 0.35, 1.8), pink, m);
                cuboid(
                    out,
                    &sq,
                    v3(2.6, side * 0.8, 7.8),
                    v3(0.15, 0.25, 0.3),
                    [0.05, 0.05, 0.06],
                    m,
                );
                ball(out, &sq, v3(1.6, side * 1.0, 0.6), v3(1.0, 0.7, 0.5), white, m);
            }
            cuboid(out, &sq, v3(2.8, 0.0, 7.0), v3(0.15, 0.3, 0.2), pink, m);
        }
        Held::Floaties => {
            // arm floaties: two orange inflatable rings side by side, white valves
            let orange = [0.98, 0.56, 0.14];
            for side in [-1.0f32, 1.0] {
                for k in 0..10 {
                    let (s, c) = sin_cos_deg(k as f64 * 36.0);
                    let p = v3(c as f32 * 2.2, side * 2.0, 3.4 + s as f32 * 2.2);
                    ball(out, &sq, p, v3(1.1, 1.3, 1.1), orange, m);
                }
                cuboid(
                    out,
                    &sq,
                    v3(0.0, side * 3.4, 5.6),
                    v3(0.3, 0.3, 0.3),
                    [0.96, 0.96, 0.94],
                    m,
                );
            }
        }
        Held::Whistle => {
            // a coach's whistle: a silver barrel and mouthpiece on a red lanyard
            let silver = [0.80, 0.82, 0.86];
            let red = [0.86, 0.20, 0.18];
            ball(out, &sq, v3(-0.4, 0.0, 2.6), v3(2.2, 1.6, 2.2), silver, m);
            cuboid(out, &sq, v3(2.6, 0.0, 3.6), v3(1.8, 0.9, 0.6), silver, m);
            cuboid(
                out,
                &sq,
                v3(1.2, 0.0, 4.4),
                v3(0.5, 0.6, 0.2),
                [0.10, 0.10, 0.12],
                m,
            );
            let mut prev = v3(-2.4, 0.0, 3.0);
            for i in 1..=8 {
                let (s, c) = sin_cos_deg(i as f64 * 45.0);
                let p = v3(-2.4 - s as f32 * 2.0, 0.0, 3.0 + (1.0 - c as f32) * 3.4);
                bar(out, &sq, prev, p, 0.3, red, m);
                prev = p;
            }
        }
        Held::GoldStar => {
            // a gold star sticker: five points, a smiling face
            let gold = [0.98, 0.80, 0.18];
            let ink = [0.10, 0.10, 0.12];
            let c = v3(0.0, 0.0, 5.0);
            ball(out, &sq, c, v3(0.8, 2.0, 2.0), gold, m);
            for k in 0..5 {
                let (s, co) = sin_cos_deg(90.0 + k as f64 * 72.0);
                let tip = c + v3(0.0, co as f32 * 4.6, s as f32 * 4.6);
                let mid = c + v3(0.0, co as f32 * 2.2, s as f32 * 2.2);
                bar(out, &sq, c, mid, 1.2, gold, m);
                bar(out, &sq, mid, tip, 0.55, gold, m);
            }
            for side in [-1.0f32, 1.0] {
                cuboid(out, &sq, v3(0.8, side * 0.7, 5.6), v3(0.15, 0.2, 0.3), ink, m);
            }
            bar(out, &sq, v3(0.8, -0.7, 4.3), v3(0.8, 0.7, 4.3), 0.15, ink, m);
        }
        Held::SwimRing => {
            // a swim ring standing on its edge, in red and white stripes
            for k in 0..14 {
                let (s, c) = sin_cos_deg(k as f64 * 360.0 / 14.0);
                let col = if k % 2 == 0 {
                    [0.90, 0.20, 0.18]
                } else {
                    [0.97, 0.97, 0.95]
                };
                let p = v3(0.0, c as f32 * 3.4, 4.4 + s as f32 * 3.4);
                ball(out, &sq, p, v3(1.3, 1.1, 1.1), col, m);
            }
        }
        Held::GradCap => {
            // a mortarboard: a black square board, a cap under it, a gold tassel
            let black = [0.12, 0.12, 0.15];
            let gold = [0.98, 0.78, 0.18];
            cuboid(out, &sq, v3(0.0, 0.0, 2.2), v3(2.4, 2.4, 1.8), black, m);
            cuboid(out, &sq, v3(0.0, 0.0, 4.2), v3(4.2, 4.2, 0.25), black, m);
            ball(out, &sq, v3(0.0, 0.0, 4.5), v3(0.4, 0.4, 0.25), gold, m);
            bar(out, &sq, v3(0.0, 0.0, 4.6), v3(3.6, 3.6, 4.5), 0.15, gold, m);
            bar(out, &sq, v3(3.6, 3.6, 4.5), v3(3.7, 3.7, 1.8), 0.2, gold, m);
            ball(out, &sq, v3(3.7, 3.7, 1.5), v3(0.4, 0.4, 0.7), gold, m);
        }
        Held::IceAxe => {
            // an ice axe: a blue shaft, a steel head with a pick and adze, a red leash
            let shaft = [0.20, 0.36, 0.78];
            let steel = [0.76, 0.78, 0.82];
            bar(out, &sq, v3(0.0, 0.0, 1.0), v3(0.0, 0.0, 10.0), 0.45, shaft, m);
            bar(out, &sq, v3(0.0, 0.0, 0.0), v3(0.0, 0.0, 1.2), 0.25, steel, m);
            bar(out, &sq, v3(-0.6, 0.0, 10.4), v3(2.4, 0.0, 10.4), 0.5, steel, m);
            bar(out, &sq, v3(-0.6, 0.0, 10.4), v3(-3.4, 0.0, 9.2), 0.35, steel, m);
            cuboid(out, &sq, v3(2.8, 0.0, 10.4), v3(0.6, 0.9, 0.35), steel, m);
            bar(
                out,
                &sq,
                v3(0.0, 0.0, 6.0),
                v3(0.0, 1.4, 3.6),
                0.15,
                [0.90, 0.30, 0.22],
                m,
            );
        }
        Held::Snowflake => {
            // a snowflake: six arms with branching twigs, pale blue
            let ice = [0.82, 0.92, 1.0];
            let c = v3(0.0, 0.0, 5.2);
            ball(out, &sq, c, v3(0.6, 0.9, 0.9), ice, m);
            for k in 0..6 {
                let a = k as f64 * 60.0;
                let (s, co) = sin_cos_deg(a);
                let at = |r: f32| c + v3(0.0, co as f32 * r, s as f32 * r);
                bar(out, &sq, c, at(4.6), 0.35, ice, m);
                for twig in [-40.0f64, 40.0] {
                    let (ts, tc) = sin_cos_deg(a + twig);
                    let from = at(2.6);
                    bar(
                        out,
                        &sq,
                        from,
                        from + v3(0.0, tc as f32 * 1.4, ts as f32 * 1.4),
                        0.22,
                        ice,
                        m,
                    );
                }
            }
        }
        Held::Compass => {
            // a brass compass on edge: a white face, a red and dark needle, a ring on top
            let brass = [0.82, 0.62, 0.24];
            ball(out, &sq, v3(0.0, 0.0, 4.0), v3(1.0, 3.4, 3.4), brass, m);
            ball(
                out,
                &sq,
                v3(0.7, 0.0, 4.0),
                v3(0.5, 2.8, 2.8),
                [0.97, 0.96, 0.92],
                m,
            );
            bar(
                out,
                &sq,
                v3(1.25, 0.0, 4.0),
                v3(1.25, 0.6, 6.2),
                0.25,
                [0.88, 0.18, 0.16],
                m,
            );
            bar(
                out,
                &sq,
                v3(1.25, 0.0, 4.0),
                v3(1.25, -0.6, 1.8),
                0.25,
                [0.15, 0.16, 0.20],
                m,
            );
            bar(out, &sq, v3(0.0, -0.6, 7.8), v3(0.0, 0.0, 8.6), 0.25, brass, m);
            bar(out, &sq, v3(0.0, 0.0, 8.6), v3(0.0, 0.6, 7.8), 0.25, brass, m);
        }
        Held::SummitFlag => {
            // a summit flag: a red flag on a grey pole, planted in a little cairn
            let stone = [0.56, 0.56, 0.60];
            ball(out, &sq, v3(0.0, 0.0, 0.8), v3(2.4, 2.2, 1.0), stone, m);
            ball(out, &sq, v3(0.3, 0.2, 2.0), v3(1.4, 1.3, 0.8), stone, m);
            bar(
                out,
                &sq,
                v3(0.0, 0.0, 1.0),
                v3(0.0, 0.0, 11.5),
                0.3,
                [0.72, 0.74, 0.78],
                m,
            );
            cuboid(
                out,
                &sq,
                v3(0.0, 2.6, 9.6),
                v3(0.15, 2.6, 1.7),
                [0.88, 0.20, 0.18],
                m,
            );
            cuboid(
                out,
                &sq,
                v3(0.2, 2.6, 9.6),
                v3(0.1, 1.2, 0.5),
                [0.97, 0.97, 0.95],
                m,
            );
        }
        Held::Yeti => {
            // a yeti plush: shaggy white, a blue-grey face, long arms, big feet
            let fur = [0.94, 0.96, 0.98];
            let face = [0.56, 0.66, 0.80];
            ball(out, &sq, v3(0.0, 0.0, 3.6), v3(2.8, 2.6, 3.4), fur, m);
            ball(out, &sq, v3(0.3, 0.0, 8.0), v3(2.3, 2.3, 2.1), fur, m);
            ball(out, &sq, v3(1.8, 0.0, 7.8), v3(0.8, 1.5, 1.2), face, m);
            for side in [-1.0f32, 1.0] {
                cuboid(
                    out,
                    &sq,
                    v3(2.5, side * 0.6, 8.3),
                    v3(0.15, 0.25, 0.25),
                    [0.08, 0.08, 0.10],
                    m,
                );
                bar(
                    out,
                    &sq,
                    v3(0.0, side * 2.4, 5.8),
                    v3(0.8, side * 3.4, 1.6),
                    0.8,
                    fur,
                    m,
                );
                ball(out, &sq, v3(1.0, side * 1.4, 0.4), v3(1.6, 0.9, 0.5), face, m);
            }
        }
        Held::IceCream => {
            // an ice-cream cone: a waffle cone, a pink scoop, a chocolate scoop, a cherry
            let cone = [0.86, 0.64, 0.34];
            for i in 0..6 {
                let t = i as f32;
                let r = 0.3 + t * 0.34;
                cuboid(out, &sq, v3(0.0, 0.0, 0.5 + t * 1.0), v3(r, r, 0.5), cone, m);
            }
            ball(
                out,
                &sq,
                v3(0.0, 0.0, 7.2),
                v3(2.4, 2.4, 1.9),
                [0.98, 0.66, 0.74],
                m,
            );
            ball(
                out,
                &sq,
                v3(0.0, 0.0, 9.6),
                v3(2.0, 2.0, 1.6),
                [0.48, 0.30, 0.18],
                m,
            );
            ball(
                out,
                &sq,
                v3(0.2, 0.0, 11.5),
                v3(0.6, 0.6, 0.6),
                [0.86, 0.10, 0.14],
                m,
            );
        }
        Held::BeachBall => {
            // a beach ball: white, with a red and a blue band crossing it, a yellow cap
            let c = v3(0.0, 0.0, 4.6);
            ball(out, &sq, c, v3(4.5, 4.5, 4.5), [0.97, 0.97, 0.95], m);
            ball(out, &sq, c, v3(1.5, 4.65, 4.65), [0.90, 0.20, 0.18], m);
            ball(out, &sq, c, v3(4.65, 1.5, 4.65), [0.20, 0.42, 0.88], m);
            ball(
                out,
                &sq,
                c + v3(0.0, 0.0, 4.5),
                v3(1.0, 1.0, 0.3),
                [0.98, 0.80, 0.18],
                m,
            );
        }
        Held::FlipFlop => {
            // a flip-flop: a blue sole and a pink thong strap
            let pink = [0.98, 0.50, 0.66];
            cuboid(
                out,
                &sq,
                v3(0.0, 0.0, 0.5),
                v3(4.6, 1.9, 0.5),
                [0.22, 0.52, 0.90],
                m,
            );
            bar(out, &sq, v3(2.6, 0.0, 1.0), v3(2.6, 0.0, 1.8), 0.25, pink, m);
            for side in [-1.0f32, 1.0] {
                bar(
                    out,
                    &sq,
                    v3(2.6, 0.0, 1.8),
                    v3(0.2, side * 1.7, 1.1),
                    0.35,
                    pink,
                    m,
                );
            }
        }
        Held::Watermelon => {
            // a watermelon slice standing on its point: red flesh, seeds, a green rind
            let red = [0.94, 0.30, 0.34];
            for i in 0..7 {
                let t = i as f32;
                let w = 0.4 + t * 0.62;
                cuboid(out, &sq, v3(0.0, 0.0, 0.6 + t * 1.1), v3(0.8, w, 0.55), red, m);
            }
            cuboid(
                out,
                &sq,
                v3(0.0, 0.0, 8.3),
                v3(0.8, 4.5, 0.35),
                [0.82, 0.94, 0.70],
                m,
            );
            cuboid(
                out,
                &sq,
                v3(0.0, 0.0, 8.9),
                v3(0.85, 4.7, 0.4),
                [0.20, 0.56, 0.26],
                m,
            );
            for (y, z) in [(-1.6f32, 6.4f32), (0.0, 5.4), (1.6, 6.4), (-0.8, 3.6), (0.8, 3.6)] {
                cuboid(out, &sq, v3(0.85, y, z), v3(0.1, 0.2, 0.3), [0.10, 0.08, 0.08], m);
            }
        }
        Held::Crab => {
            // a crab plush: a red body, eyes on stalks, two big claws, legs
            let red = [0.90, 0.30, 0.22];
            let dark = [0.70, 0.20, 0.16];
            ball(out, &sq, v3(0.0, 0.0, 2.0), v3(2.6, 3.2, 1.5), red, m);
            for side in [-1.0f32, 1.0] {
                bar(
                    out,
                    &sq,
                    v3(1.4, side * 0.8, 3.0),
                    v3(1.6, side * 0.9, 4.6),
                    0.2,
                    dark,
                    m,
                );
                ball(
                    out,
                    &sq,
                    v3(1.6, side * 0.9, 4.8),
                    v3(0.45, 0.45, 0.45),
                    [0.10, 0.10, 0.12],
                    m,
                );
                bar(
                    out,
                    &sq,
                    v3(1.2, side * 2.6, 2.4),
                    v3(3.0, side * 3.6, 3.8),
                    0.45,
                    dark,
                    m,
                );
                ball(out, &sq, v3(3.4, side * 3.8, 4.4), v3(1.1, 0.8, 1.2), red, m);
                for x in [-1.4f32, -0.4, 0.6] {
                    bar(
                        out,
                        &sq,
                        v3(x, side * 2.8, 1.8),
                        v3(x - 0.6, side * 4.4, 0.2),
                        0.22,
                        dark,
                        m,
                    );
                }
            }
        }
        Held::AlarmClock => {
            // a twin-bell alarm clock: a red case, a white face, two bells, little legs
            let red = [0.88, 0.22, 0.20];
            let steel = [0.80, 0.82, 0.86];
            let ink = [0.10, 0.10, 0.12];
            ball(out, &sq, v3(0.0, 0.0, 4.4), v3(1.6, 3.2, 3.2), red, m);
            ball(
                out,
                &sq,
                v3(0.9, 0.0, 4.4),
                v3(1.0, 2.6, 2.6),
                [0.97, 0.97, 0.95],
                m,
            );
            bar(out, &sq, v3(1.95, 0.0, 4.4), v3(1.95, 0.0, 6.2), 0.18, ink, m);
            bar(out, &sq, v3(1.95, 0.0, 4.4), v3(1.95, 1.3, 4.4), 0.18, ink, m);
            for side in [-1.0f32, 1.0] {
                ball(out, &sq, v3(0.0, side * 2.0, 8.0), v3(1.2, 1.2, 1.0), steel, m);
                bar(
                    out,
                    &sq,
                    v3(0.0, side * 1.6, 1.4),
                    v3(0.0, side * 2.2, 0.0),
                    0.3,
                    steel,
                    m,
                );
            }
            bar(out, &sq, v3(0.0, 0.0, 7.4), v3(0.0, 0.0, 8.6), 0.25, steel, m);
        }
        Held::CoffeeMug => {
            // a coffee mug: a white mug with a blue band, a handle, coffee, steam
            let white = [0.96, 0.96, 0.94];
            let steam = [0.88, 0.90, 0.94];
            cuboid(out, &sq, v3(0.0, 0.0, 3.0), v3(2.4, 2.4, 3.0), white, m);
            cuboid(
                out,
                &sq,
                v3(0.0, 0.0, 3.6),
                v3(2.45, 2.45, 0.5),
                [0.20, 0.42, 0.88],
                m,
            );
            cuboid(
                out,
                &sq,
                v3(0.0, 0.0, 5.85),
                v3(2.0, 2.0, 0.2),
                [0.36, 0.22, 0.12],
                m,
            );
            bar(out, &sq, v3(0.0, -2.4, 4.8), v3(0.0, -4.0, 4.4), 0.45, white, m);
            bar(out, &sq, v3(0.0, -4.0, 4.4), v3(0.0, -4.0, 2.2), 0.45, white, m);
            bar(out, &sq, v3(0.0, -4.0, 2.2), v3(0.0, -2.4, 1.6), 0.45, white, m);
            for y in [-0.8f32, 0.8] {
                bar(out, &sq, v3(0.0, y, 6.4), v3(0.0, y + 0.5, 7.6), 0.15, steam, m);
                bar(out, &sq, v3(0.0, y + 0.5, 7.6), v3(0.0, y, 8.8), 0.15, steam, m);
            }
        }
        Held::Paperclip => {
            // a giant paperclip: one steel wire bent in two nested loops
            let steel = [0.76, 0.78, 0.84];
            let path = [
                v3(0.0, -0.8, 7.0),
                v3(0.0, -0.8, 2.6),
                v3(0.0, -0.3, 2.0),
                v3(0.0, 0.3, 2.0),
                v3(0.0, 0.8, 2.6),
                v3(0.0, 0.8, 9.0),
                v3(0.0, 0.2, 9.8),
                v3(0.0, -0.9, 9.8),
                v3(0.0, -1.6, 9.0),
                v3(0.0, -1.6, 1.4),
                v3(0.0, -0.9, 0.4),
                v3(0.0, 0.9, 0.4),
                v3(0.0, 1.6, 1.4),
                v3(0.0, 1.6, 6.0),
            ];
            for w in path.windows(2) {
                bar(out, &sq, w[0], w[1], 0.3, steel, m);
            }
        }
        Held::Pencil => {
            // a big pencil: yellow body, sharpened wood tip, a steel band, a pink eraser
            let wood = [0.88, 0.70, 0.48];
            bar(
                out,
                &sq,
                v3(0.0, 0.0, 2.2),
                v3(0.0, 0.0, 9.6),
                0.8,
                [0.98, 0.78, 0.16],
                m,
            );
            cuboid(out, &sq, v3(0.0, 0.0, 1.6), v3(0.55, 0.55, 0.6), wood, m);
            cuboid(out, &sq, v3(0.0, 0.0, 0.7), v3(0.3, 0.3, 0.4), wood, m);
            cuboid(
                out,
                &sq,
                v3(0.0, 0.0, 0.2),
                v3(0.15, 0.15, 0.2),
                [0.15, 0.15, 0.18],
                m,
            );
            cuboid(
                out,
                &sq,
                v3(0.0, 0.0, 10.0),
                v3(0.85, 0.85, 0.5),
                [0.76, 0.78, 0.82],
                m,
            );
            cuboid(
                out,
                &sq,
                v3(0.0, 0.0, 11.0),
                v3(0.8, 0.8, 0.6),
                [0.98, 0.56, 0.66],
                m,
            );
        }
        Held::Owl => {
            // an owl plush: a brown body, a pale belly, big round eyes, ear tufts
            let brown = [0.56, 0.38, 0.24];
            let beak = [0.96, 0.62, 0.20];
            ball(out, &sq, v3(0.0, 0.0, 3.8), v3(2.6, 2.6, 3.6), brown, m);
            ball(
                out,
                &sq,
                v3(1.2, 0.0, 3.2),
                v3(1.6, 1.8, 2.4),
                [0.90, 0.80, 0.62],
                m,
            );
            for side in [-1.0f32, 1.0] {
                ball(
                    out,
                    &sq,
                    v3(2.0, side * 1.1, 6.0),
                    v3(0.6, 1.0, 1.0),
                    [0.97, 0.97, 0.95],
                    m,
                );
                ball(
                    out,
                    &sq,
                    v3(2.5, side * 1.1, 6.0),
                    v3(0.3, 0.45, 0.45),
                    [0.10, 0.10, 0.12],
                    m,
                );
                bar(
                    out,
                    &sq,
                    v3(0.0, side * 1.4, 7.0),
                    v3(0.0, side * 2.0, 8.8),
                    0.35,
                    brown,
                    m,
                );
                cuboid(out, &sq, v3(1.4, side * 1.0, 0.3), v3(0.6, 0.4, 0.3), beak, m);
            }
            cuboid(out, &sq, v3(2.6, 0.0, 5.3), v3(0.3, 0.25, 0.35), beak, m);
        }
        Held::Sneaker => {
            // a sneaker: a white sole, a red upper rising to the heel, white laces
            let red = [0.88, 0.24, 0.22];
            let white = [0.96, 0.96, 0.96];
            cuboid(out, &sq, v3(0.0, 0.0, 0.6), v3(4.4, 1.8, 0.6), white, m);
            cuboid(out, &sq, v3(-0.6, 0.0, 2.2), v3(3.4, 1.6, 1.0), red, m);
            ball(out, &sq, v3(3.0, 0.0, 1.6), v3(1.6, 1.6, 1.0), red, m);
            cuboid(out, &sq, v3(-2.6, 0.0, 3.8), v3(1.4, 1.5, 1.4), red, m);
            cuboid(out, &sq, v3(-3.9, 0.0, 4.6), v3(0.2, 1.0, 0.6), white, m);
            for i in 0..3 {
                let x = -0.4 + i as f32 * 1.0;
                bar(out, &sq, v3(x, -1.4, 3.3), v3(x, 1.4, 3.3), 0.22, white, m);
            }
        }
        Held::Feather => {
            // a feather: a pale shaft, a long pink vane, widest in the middle
            let vane = [0.92, 0.40, 0.62];
            let shaft = [0.96, 0.94, 0.88];
            bar(out, &sq, v3(0.0, 0.0, 0.0), v3(0.0, 1.0, 12.0), 0.2, shaft, m);
            for i in 0..5 {
                let t = i as f32;
                let wide = 1.7 - (t - 2.0).abs() * 0.3;
                ball(
                    out,
                    &sq,
                    v3(0.0, 0.15 + t * 0.2, 3.4 + t * 1.8),
                    v3(0.3, wide, 1.4),
                    vane,
                    m,
                );
            }
        }
        Held::Spring => {
            // a coil spring: a steel wire wound five times
            let steel = [0.70, 0.72, 0.78];
            let mut prev = v3(2.0, 0.0, 0.4);
            for i in 1..=60 {
                let (s, c) = sin_cos_deg(i as f64 * 30.0);
                let p = v3(2.0 * c as f32, 2.0 * s as f32, 0.4 + i as f32 * 0.16);
                bar(out, &sq, prev, p, 0.3, steel, m);
                prev = p;
            }
        }
        Held::Stopwatch => {
            // a stopwatch: a silver case, a white face, a red hand, the button on top
            let silver = [0.78, 0.80, 0.84];
            let red = [0.85, 0.20, 0.20];
            ball(out, &sq, v3(0.0, 0.0, 4.0), v3(1.0, 3.2, 3.2), silver, m);
            ball(
                out,
                &sq,
                v3(0.5, 0.0, 4.0),
                v3(0.7, 2.7, 2.7),
                [0.98, 0.98, 0.96],
                m,
            );
            bar(out, &sq, v3(1.2, 0.0, 4.0), v3(1.2, -0.8, 6.0), 0.15, red, m);
            cuboid(
                out,
                &sq,
                v3(1.2, 0.0, 4.0),
                v3(0.1, 0.25, 0.25),
                [0.1, 0.1, 0.12],
                m,
            );
            cuboid(out, &sq, v3(0.0, 0.0, 7.6), v3(0.5, 0.5, 0.5), silver, m);
            cuboid(out, &sq, v3(0.0, 0.0, 8.4), v3(0.7, 0.9, 0.3), red, m);
        }
        Held::Frog => {
            // a frog plush: a squat green body, two bulging eyes, a pale belly, splayed feet
            let green = [0.36, 0.74, 0.32];
            let belly = [0.86, 0.92, 0.62];
            ball(out, &sq, v3(0.0, 0.0, 2.6), v3(3.0, 3.2, 2.6), green, m);
            ball(out, &sq, v3(1.4, 0.0, 2.2), v3(1.8, 2.4, 1.8), belly, m);
            for side in [-1.0f32, 1.0] {
                ball(out, &sq, v3(0.8, side * 1.6, 5.0), v3(1.0, 1.0, 1.0), green, m);
                ball(
                    out,
                    &sq,
                    v3(1.6, side * 1.6, 5.2),
                    v3(0.4, 0.5, 0.5),
                    [0.05, 0.05, 0.06],
                    m,
                );
                ball(out, &sq, v3(1.6, side * 3.0, 0.4), v3(1.4, 1.0, 0.4), green, m);
            }
            bar(
                out,
                &sq,
                v3(2.9, -1.0, 3.2),
                v3(2.9, 1.0, 3.2),
                0.15,
                [0.15, 0.35, 0.15],
                m,
            );
        }
        Held::ToyBrick => {
            // a toy building brick: a red block, eight round studs on top
            let red = [0.86, 0.18, 0.16];
            cuboid(out, &sq, v3(0.0, 0.0, 1.8), v3(4.0, 2.0, 1.8), red, m);
            for i in 0..4 {
                for j in 0..2 {
                    let (x, y) = (-3.0 + i as f32 * 2.0, -1.0 + j as f32 * 2.0);
                    cuboid(
                        out,
                        &sq,
                        v3(x, y, 3.9),
                        v3(0.65, 0.65, 0.3),
                        [0.92, 0.26, 0.22],
                        m,
                    );
                }
            }
        }
        Held::BlockFigure => {
            // a blocky toy figure: red legs, a blue body, a round yellow head with a smile
            let yellow = [0.98, 0.80, 0.18];
            let blue = [0.18, 0.36, 0.78];
            let red = [0.80, 0.20, 0.18];
            let ink = [0.06, 0.06, 0.08];
            for side in [-1.0f32, 1.0] {
                cuboid(out, &sq, v3(0.0, side * 0.8, 1.6), v3(0.9, 0.75, 1.6), red, m);
                bar(
                    out,
                    &sq,
                    v3(0.0, side * 2.0, 6.0),
                    v3(0.8, side * 2.4, 3.8),
                    0.55,
                    blue,
                    m,
                );
                ball(out, &sq, v3(0.9, side * 2.4, 3.4), v3(0.5, 0.5, 0.5), yellow, m);
                cuboid(out, &sq, v3(1.35, side * 0.5, 8.5), v3(0.12, 0.18, 0.2), ink, m);
            }
            cuboid(out, &sq, v3(0.0, 0.0, 3.6), v3(1.0, 1.7, 0.4), red, m);
            cuboid(out, &sq, v3(0.0, 0.0, 5.4), v3(1.0, 1.6, 1.4), blue, m);
            ball(out, &sq, v3(0.0, 0.0, 8.2), v3(1.4, 1.4, 1.5), yellow, m);
            cuboid(out, &sq, v3(0.0, 0.0, 9.9), v3(0.6, 0.6, 0.3), yellow, m);
            bar(out, &sq, v3(1.35, -0.5, 7.7), v3(1.35, 0.5, 7.7), 0.1, ink, m);
        }
        Held::ToyCar => {
            // a toy car: a blue body, a cab with windows, four black wheels with grey hubs
            let blue = [0.20, 0.50, 0.90];
            let tyre = [0.10, 0.10, 0.12];
            cuboid(out, &sq, v3(0.0, 0.0, 2.2), v3(4.4, 2.2, 1.0), blue, m);
            cuboid(out, &sq, v3(-0.6, 0.0, 4.0), v3(2.2, 1.9, 0.9), blue, m);
            cuboid(
                out,
                &sq,
                v3(-0.6, 0.0, 4.0),
                v3(2.3, 1.95, 0.5),
                [0.75, 0.88, 0.98],
                m,
            );
            for x in [-2.6f32, 2.6] {
                for side in [-1.0f32, 1.0] {
                    ball(out, &sq, v3(x, side * 2.2, 1.2), v3(1.2, 0.5, 1.2), tyre, m);
                    ball(
                        out,
                        &sq,
                        v3(x, side * 2.6, 1.2),
                        v3(0.5, 0.2, 0.5),
                        [0.7, 0.7, 0.74],
                        m,
                    );
                }
            }
            for side in [-1.0f32, 1.0] {
                cuboid(
                    out,
                    &sq,
                    v3(4.4, side * 1.4, 2.4),
                    v3(0.15, 0.4, 0.3),
                    [0.98, 0.90, 0.40],
                    m,
                );
            }
        }
        Held::ToyDino => {
            // a toy dinosaur: a green body, a long neck and tail, four stubby legs, orange back plates
            let green = [0.30, 0.70, 0.40];
            let plate = [0.96, 0.56, 0.20];
            ball(out, &sq, v3(0.0, 0.0, 3.6), v3(3.0, 2.0, 2.0), green, m);
            bar(out, &sq, v3(2.0, 0.0, 4.2), v3(3.4, 0.0, 8.4), 0.9, green, m);
            ball(out, &sq, v3(3.9, 0.0, 8.8), v3(1.3, 0.9, 0.9), green, m);
            bar(out, &sq, v3(-2.4, 0.0, 3.6), v3(-5.6, 0.0, 1.6), 0.7, green, m);
            for x in [-1.6f32, 1.6] {
                for side in [-1.0f32, 1.0] {
                    cuboid(out, &sq, v3(x, side * 1.2, 1.0), v3(0.6, 0.6, 1.0), green, m);
                }
            }
            for i in 0..4 {
                ball(
                    out,
                    &sq,
                    v3(-1.8 + i as f32 * 1.2, 0.0, 5.6),
                    v3(0.5, 0.15, 0.7),
                    plate,
                    m,
                );
            }
            for side in [-1.0f32, 1.0] {
                cuboid(
                    out,
                    &sq,
                    v3(4.6, side * 0.6, 9.2),
                    v3(0.15, 0.15, 0.2),
                    [0.05, 0.05, 0.06],
                    m,
                );
            }
        }
        Held::ToyCrown => {
            // a toy crown: a gold band, five points with balls on top, red and blue gems
            let gold = [0.96, 0.78, 0.20];
            for i in 0..10 {
                let (s, c) = sin_cos_deg(i as f64 * 36.0);
                let (s, c) = (s as f32, c as f32);
                bar(
                    out,
                    &sq,
                    v3(2.6 * c, 2.6 * s, 0.4),
                    v3(2.6 * c, 2.6 * s, 2.6),
                    0.9,
                    gold,
                    m,
                );
                if i % 2 == 0 {
                    bar(
                        out,
                        &sq,
                        v3(2.6 * c, 2.6 * s, 2.6),
                        v3(2.6 * c, 2.6 * s, 5.0),
                        0.45,
                        gold,
                        m,
                    );
                    ball(out, &sq, v3(2.6 * c, 2.6 * s, 5.2), v3(0.5, 0.5, 0.5), gold, m);
                    let gem = if i % 4 == 0 {
                        [0.86, 0.16, 0.22]
                    } else {
                        [0.20, 0.40, 0.90]
                    };
                    ball(out, &sq, v3(3.5 * c, 3.5 * s, 1.5), v3(0.45, 0.45, 0.45), gem, m);
                }
            }
        }
        Held::FoldingFan => {
            // a folding fan, open: paper panels spread in a half circle from a red pivot
            let paper = [0.96, 0.92, 0.84];
            let shade = [0.90, 0.80, 0.70];
            for i in 0..9 {
                let (s, c) = sin_cos_deg(-60.0 + i as f64 * 15.0);
                let (s, c) = (s as f32, c as f32);
                bar(
                    out,
                    &sq,
                    v3(0.0, 0.0, 1.0),
                    v3(0.0, 3.0 * s, 1.0 + 3.0 * c),
                    0.2,
                    [0.55, 0.35, 0.20],
                    m,
                );
                let col = if i % 2 == 0 { paper } else { shade };
                bar(
                    out,
                    &sq,
                    v3(0.0, 3.0 * s, 1.0 + 3.0 * c),
                    v3(0.0, 7.0 * s, 1.0 + 7.0 * c),
                    0.8,
                    col,
                    m,
                );
                ball(
                    out,
                    &sq,
                    v3(0.3, 6.6 * s, 1.0 + 6.6 * c),
                    v3(0.3, 0.4, 0.4),
                    [0.84, 0.22, 0.22],
                    m,
                );
            }
            ball(
                out,
                &sq,
                v3(0.0, 0.0, 1.0),
                v3(0.6, 0.6, 0.6),
                [0.84, 0.22, 0.22],
                m,
            );
        }
        Held::Koi => {
            // a koi plush: a white body with orange patches, a forked tail, fins and whiskers
            let white = [0.97, 0.95, 0.92];
            let orange = [0.96, 0.46, 0.16];
            ball(out, &sq, v3(0.0, 0.0, 3.0), v3(4.0, 1.8, 2.0), white, m);
            ball(out, &sq, v3(1.0, 0.0, 4.2), v3(1.6, 1.5, 1.0), orange, m);
            ball(out, &sq, v3(-1.8, 0.0, 3.8), v3(1.2, 1.4, 1.0), orange, m);
            for side in [-1.0f32, 1.0] {
                bar(
                    out,
                    &sq,
                    v3(-3.6, 0.0, 3.0),
                    v3(-6.0, 0.0, 3.0 + side * 2.0),
                    0.5,
                    orange,
                    m,
                );
                ball(out, &sq, v3(0.8, side * 1.9, 2.2), v3(0.8, 0.3, 0.5), orange, m);
                cuboid(
                    out,
                    &sq,
                    v3(3.3, side * 1.0, 3.5),
                    v3(0.2, 0.2, 0.25),
                    [0.05, 0.05, 0.06],
                    m,
                );
                bar(
                    out,
                    &sq,
                    v3(3.8, side * 0.6, 2.6),
                    v3(4.8, side * 1.4, 2.0),
                    0.1,
                    orange,
                    m,
                );
            }
        }
        Held::Daruma => {
            // a daruma doll: a round red body, a white face, one eye filled in, gold brows and band
            let red = [0.84, 0.16, 0.14];
            let white = [0.98, 0.96, 0.92];
            let gold = [0.96, 0.78, 0.20];
            ball(out, &sq, v3(0.0, 0.0, 4.0), v3(3.2, 3.2, 4.0), red, m);
            ball(out, &sq, v3(1.4, 0.0, 5.0), v3(2.0, 2.0, 1.8), white, m);
            ball(
                out,
                &sq,
                v3(3.3, -0.8, 5.4),
                v3(0.25, 0.5, 0.5),
                [0.05, 0.05, 0.06],
                m,
            );
            ball(out, &sq, v3(3.3, 0.8, 5.4), v3(0.25, 0.5, 0.5), white, m);
            for side in [-1.0f32, 1.0] {
                bar(
                    out,
                    &sq,
                    v3(3.0, side * 0.3, 6.4),
                    v3(2.6, side * 1.6, 6.8),
                    0.18,
                    gold,
                    m,
                );
            }
            bar(out, &sq, v3(3.0, -1.4, 2.0), v3(3.0, 1.4, 2.0), 0.25, gold, m);
        }
        Held::Bonsai => {
            // a bonsai: a low blue pot, a twisting trunk, three clouds of leaves
            let pot = [0.30, 0.40, 0.62];
            let trunk = [0.45, 0.30, 0.18];
            let leaf = [0.26, 0.56, 0.26];
            cuboid(out, &sq, v3(0.0, 0.0, 1.0), v3(3.4, 2.2, 1.0), pot, m);
            cuboid(
                out,
                &sq,
                v3(0.0, 0.0, 2.1),
                v3(3.0, 1.9, 0.15),
                [0.36, 0.26, 0.16],
                m,
            );
            bar(out, &sq, v3(0.0, 0.0, 2.0), v3(1.0, 0.0, 4.6), 0.6, trunk, m);
            bar(out, &sq, v3(1.0, 0.0, 4.6), v3(-0.8, 0.0, 6.8), 0.5, trunk, m);
            bar(out, &sq, v3(-0.8, 0.0, 6.8), v3(0.4, 0.0, 8.4), 0.4, trunk, m);
            bar(out, &sq, v3(1.0, 0.0, 4.6), v3(2.8, 0.0, 5.4), 0.35, trunk, m);
            ball(out, &sq, v3(0.4, 0.0, 9.0), v3(2.4, 2.0, 1.2), leaf, m);
            ball(out, &sq, v3(-2.2, 0.0, 6.8), v3(1.8, 1.6, 1.0), leaf, m);
            ball(out, &sq, v3(3.0, 0.0, 5.8), v3(1.6, 1.4, 0.9), leaf, m);
        }
        Held::PaperCrane => {
            // an origami crane in pink paper: a folded body, two flat wings, a tall neck and tail
            let paper = [0.96, 0.60, 0.70];
            let shade = [0.86, 0.48, 0.58];
            ball(out, &sq, v3(0.0, 0.0, 2.4), v3(2.0, 1.0, 1.2), paper, m);
            for side in [-1.0f32, 1.0] {
                cuboid(out, &sq, v3(-0.4, side * 2.8, 3.4), v3(1.4, 2.0, 0.12), shade, m);
                cuboid(out, &sq, v3(-0.8, side * 5.2, 4.2), v3(1.0, 0.8, 0.12), paper, m);
            }
            bar(out, &sq, v3(1.6, 0.0, 2.6), v3(3.2, 0.0, 7.0), 0.3, paper, m);
            bar(out, &sq, v3(3.2, 0.0, 7.0), v3(4.2, 0.0, 6.2), 0.25, shade, m);
            bar(out, &sq, v3(-1.6, 0.0, 2.6), v3(-3.4, 0.0, 6.4), 0.35, paper, m);
        }
        Held::BananaPeel => {
            // a banana peel: a yellow stub, four peels flopped out, a brown stalk on top
            let yellow = [0.98, 0.86, 0.26];
            let inside = [0.98, 0.94, 0.72];
            bar(out, &sq, v3(0.0, 0.0, 1.0), v3(0.0, 0.0, 5.0), 1.0, yellow, m);
            bar(
                out,
                &sq,
                v3(0.0, 0.0, 5.0),
                v3(0.0, 0.0, 6.4),
                0.35,
                [0.45, 0.30, 0.18],
                m,
            );
            for k in 0..4 {
                let (s, c) = sin_cos_deg(45.0 + k as f64 * 90.0);
                let (s, c) = (s as f32, c as f32);
                bar(
                    out,
                    &sq,
                    v3(0.8 * c, 0.8 * s, 1.4),
                    v3(2.6 * c, 2.6 * s, 2.6),
                    0.5,
                    yellow,
                    m,
                );
                bar(
                    out,
                    &sq,
                    v3(2.6 * c, 2.6 * s, 2.6),
                    v3(4.4 * c, 4.4 * s, 0.6),
                    0.45,
                    yellow,
                    m,
                );
                ball(out, &sq, v3(2.0 * c, 2.0 * s, 2.6), v3(0.4, 0.4, 0.4), inside, m);
            }
        }
        Held::Lollipop => {
            // a swirl lollipop: a white stick, a flat round sweet in rings of pink, white and blue
            let pink = [0.96, 0.42, 0.62];
            let white = [0.98, 0.98, 0.98];
            let blue = [0.36, 0.66, 0.96];
            bar(out, &sq, v3(0.0, 0.0, 0.0), v3(0.0, 0.0, 6.0), 0.3, white, m);
            for (k, (r, col)) in [
                (3.2f32, pink),
                (2.5, white),
                (1.8, blue),
                (1.1, pink),
                (0.5, white),
            ]
            .into_iter()
            .enumerate()
            {
                ball(out, &sq, v3(0.12 * k as f32, 0.0, 8.6), v3(0.6, r, r), col, m);
            }
        }
        Held::Whoopee => {
            // a whoopee cushion: a squashed pink rubber bag, a short neck with a lip
            let pink = [0.95, 0.45, 0.60];
            ball(out, &sq, v3(0.0, 0.0, 1.8), v3(3.6, 3.6, 1.8), pink, m);
            ball(
                out,
                &sq,
                v3(-0.6, 0.0, 2.4),
                v3(2.0, 2.4, 1.0),
                [0.98, 0.62, 0.74],
                m,
            );
            bar(out, &sq, v3(3.0, 0.0, 1.6), v3(5.0, 0.0, 1.8), 0.8, pink, m);
            ball(
                out,
                &sq,
                v3(5.2, 0.0, 1.8),
                v3(0.4, 1.0, 0.9),
                [0.80, 0.32, 0.46],
                m,
            );
        }
        Held::TopHat => {
            // a top hat: a tall black crown, a wide brim, a red band
            let black = [0.10, 0.10, 0.12];
            cuboid(out, &sq, v3(0.0, 0.0, 0.3), v3(3.8, 3.8, 0.3), black, m);
            cuboid(out, &sq, v3(0.0, 0.0, 4.4), v3(2.4, 2.4, 3.8), black, m);
            cuboid(
                out,
                &sq,
                v3(0.0, 0.0, 1.4),
                v3(2.5, 2.5, 0.6),
                [0.80, 0.16, 0.16],
                m,
            );
        }
        Held::Anvil => {
            // a cartoon anvil: a dark iron foot, a narrow waist, a wide top with a pointed horn
            let iron = [0.30, 0.31, 0.35];
            let lit = [0.46, 0.47, 0.52];
            cuboid(out, &sq, v3(0.0, 0.0, 0.8), v3(2.6, 2.0, 0.8), iron, m);
            cuboid(out, &sq, v3(0.0, 0.0, 2.6), v3(1.4, 1.0, 1.0), iron, m);
            cuboid(out, &sq, v3(-0.6, 0.0, 4.4), v3(3.0, 1.8, 0.8), iron, m);
            cuboid(out, &sq, v3(-0.6, 0.0, 5.25), v3(2.9, 1.7, 0.05), lit, m);
            bar(out, &sq, v3(2.4, 0.0, 4.6), v3(5.4, 0.0, 4.9), 0.6, iron, m);
            bar(out, &sq, v3(5.4, 0.0, 4.9), v3(6.4, 0.0, 5.0), 0.25, iron, m);
        }
        Held::Mummy => {
            // a mummy plush, wrapped in strips, arms out, two eyes peeking
            let wrap = [0.94, 0.92, 0.84];
            let shade = [0.80, 0.77, 0.68];
            for i in 0..6 {
                let z = 0.8 + i as f32 * 1.5;
                let col = if i % 2 == 0 { wrap } else { shade };
                cuboid(out, &sq, v3(0.0, 0.0, z), v3(1.9, 2.2, 0.75), col, m);
            }
            ball(out, &sq, v3(0.0, 0.0, 11.0), v3(2.0, 2.0, 2.0), wrap, m);
            bar(out, &sq, v3(-1.9, -1.0, 11.6), v3(2.0, 1.2, 10.6), 0.35, shade, m);
            for side in [-1.0f32, 1.0] {
                cuboid(
                    out,
                    &sq,
                    v3(1.9, side * 0.75, 11.4),
                    v3(0.2, 0.35, 0.3),
                    [0.08, 0.08, 0.1],
                    m,
                );
                bar(
                    out,
                    &sq,
                    v3(0.5, side * 2.2, 7.4),
                    v3(3.8, side * 2.6, 7.8),
                    0.6,
                    wrap,
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
