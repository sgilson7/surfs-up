//! Vertices for the page to draw, and the look of the gym. Every vertex is
//! ten floats: position (3), normal (3), colour (3), material (1). The
//! material number tells the shader which pattern to paint (mat stitching,
//! maple boards, painted block, foam cubes...); the shader holds no rule, only
//! the patterns.

use crate::course::{Course, Role};
use crate::map::{
    face_polygon, BrushKind, Map, Plane, TriggerKind, HINT_GLASS, HINT_LIGHT, HINT_METAL, NO_FILTER,
};
use crate::vec3::{v3, V3};

pub const FLOATS_PER_VERTEX: usize = 10;

pub const M_FLAT: f32 = 0.0;
pub const M_MAT: f32 = 1.0;
pub const M_WOOD: f32 = 2.0;
pub const M_BLOCK: f32 = 3.0;
pub const M_CEIL: f32 = 4.0;
pub const M_NET: f32 = 5.0;
pub const M_FOAM: f32 = 6.0;
pub const M_LIGHT: f32 = 7.0;
pub const M_HALL: f32 = 8.0;
pub const M_FINISH: f32 = 9.0;
pub const M_EXIT: f32 = 10.0;
pub const M_BALLS: f32 = 11.0;
pub const M_KID: f32 = 12.0;
pub const M_GHOST: f32 = 13.0;
pub const M_HAND: f32 = 14.0;
pub const M_DOOR_EARLY: f32 = 15.0;

/// Gym mat vinyl: the colours of the mats in a school gym. One ramp (all
/// the brushes that touch along it) is one mat colour.
const MAT_COLOURS: [[f32; 3]; 7] = [
    [0.16, 0.38, 0.80],
    [0.86, 0.24, 0.22],
    [0.30, 0.62, 0.90],
    [0.98, 0.74, 0.16],
    [0.20, 0.62, 0.38],
    [0.95, 0.50, 0.16],
    [0.55, 0.34, 0.75],
];
const WALL_COLOURS: [[f32; 3]; 3] = [[0.93, 0.90, 0.82], [0.84, 0.89, 0.93], [0.92, 0.86, 0.80]];
const WOOD: [f32; 3] = [0.86, 0.66, 0.42];
const CEILING: [f32; 3] = [0.95, 0.95, 0.93];
const NET: [f32; 3] = [0.95, 0.97, 1.0];
const LIGHT: [f32; 3] = [1.0, 0.97, 0.86];
const METAL: [f32; 3] = [0.62, 0.66, 0.72];
const FOAM: [f32; 3] = [0.95, 0.55, 0.62];
const FINISH: [f32; 3] = [1.0, 0.85, 0.20];
const EXIT: [f32; 3] = [0.30, 0.80, 0.45];
const EARLY: [f32; 3] = [0.55, 0.75, 1.0];
const BALLS: [f32; 3] = [0.40, 0.75, 0.95];
const HALL: [f32; 3] = [0.80, 0.82, 0.86];

/// Below this normal z a face is a wall or the underside of something; above
/// the ground limit it is a floor; between, a surf ramp.
const WALL_Z: f32 = 0.05;
const RAMP_TOP: f32 = 0.7;

pub fn push_vertex(out: &mut Vec<f32>, p: V3, n: V3, c: [f32; 3], m: f32) {
    out.extend_from_slice(&[p.x, p.y, p.z, n.x, n.y, n.z, c[0], c[1], c[2], m]);
}

/// A convex polygon as a triangle fan, wound counter-clockwise seen from `n`.
pub fn push_polygon(out: &mut Vec<f32>, poly: &[V3], n: V3, c: [f32; 3], m: f32) {
    if poly.len() < 3 {
        return;
    }
    let flip = (poly[1] - poly[0]).cross(poly[2] - poly[0]).dot(n) < 0.0;
    for i in 1..poly.len() - 1 {
        let (a, b) = if flip {
            (poly[i + 1], poly[i])
        } else {
            (poly[i], poly[i + 1])
        };
        push_vertex(out, poly[0], n, c, m);
        push_vertex(out, a, n, c, m);
        push_vertex(out, b, n, c, m);
    }
}

fn poly_of(planes: &[Plane], i: usize) -> Vec<V3> {
    face_polygon(planes, i)
        .into_iter()
        .map(|p| v3(p[0] as f32, p[1] as f32, p[2] as f32))
        .collect()
}

fn scramble(x: u32) -> u32 {
    let mut h = x.wrapping_mul(0x9E37_79B1);
    h ^= h >> 15;
    h.wrapping_mul(0x85EB_CA77) ^ (h >> 13)
}

/// The static world: opaque triangles first, then see-through ones. Returns
/// the vertices and how many of them are opaque.
pub fn world(map: &Map, course: &Course) -> (Vec<f32>, usize) {
    let mut opaque = Vec::new();
    let mut clear = Vec::new();
    let mats = ramp_mats(map);
    for (bi, b) in map.brushes.iter().enumerate() {
        if b.kind == BrushKind::Clip {
            continue;
        }
        let planes: Vec<Plane> = b.sides.iter().map(|s| s.plane).collect();
        let ramp = b
            .sides
            .iter()
            .any(|s| s.drawn && s.plane.n.z > WALL_Z && s.plane.n.z < RAMP_TOP);
        let pick = scramble(b.group);
        for (i, s) in b.sides.iter().enumerate() {
            if !s.drawn {
                continue;
            }
            let n = s.plane.n;
            let poly = poly_of(&planes, i);
            if poly.is_empty() {
                continue;
            }
            // See-through walls and floors are safety netting, so you can see
            // the pit and the next ramp through them as in the original; a
            // see-through ramp is a mat like any other.
            let see_through = (b.kind == BrushKind::Window || s.hint == HINT_GLASS) && !ramp;
            let (c, m) = if b.kind == BrushKind::Water {
                (BALLS, M_BALLS)
            } else if see_through {
                (NET, M_NET)
            } else if s.hint == HINT_LIGHT {
                (LIGHT, M_LIGHT)
            } else if s.hint == HINT_METAL {
                (METAL, M_FLAT)
            } else if ramp {
                (MAT_COLOURS[mats[bi] % MAT_COLOURS.len()], M_MAT)
            } else if n.z >= RAMP_TOP {
                (WOOD, M_WOOD)
            } else if n.z <= -RAMP_TOP {
                (CEILING, M_CEIL)
            } else {
                (WALL_COLOURS[(pick % WALL_COLOURS.len() as u32) as usize], M_BLOCK)
            };
            let out = if see_through { &mut clear } else { &mut opaque };
            push_polygon(out, &poly, n, c, m);
        }
    }
    // The triggers you can see: foam pits where you fall, the finish doors,
    // the early exits, the doors to the next stage and back to the start. A
    // filtered teleport is a map's secret, and stays unseen.
    for t in &map.triggers {
        let TriggerKind::Teleport(i) = t.kind else {
            continue;
        };
        if t.filter != NO_FILTER {
            continue;
        }
        let size = t.maxs - t.mins;
        let pit = size.z <= 96.0 && size.x.max(size.y) >= 256.0;
        let (c, m) = match course.role(t, i) {
            Role::Finish(_) => (FINISH, M_FINISH),
            Role::Exit(_) => (EARLY, M_DOOR_EARLY),
            _ if pit => (FOAM, M_FOAM),
            Role::Restart => (EXIT, M_EXIT),
            Role::Move => (EARLY, M_DOOR_EARLY),
        };
        for k in 0..t.planes.len() {
            let n = t.planes[k].n;
            // a pit shows only its top: the foam you land in
            if m == M_FOAM && n.z < RAMP_TOP {
                continue;
            }
            let poly = poly_of(&t.planes, k);
            push_polygon(&mut opaque, &poly, n, c, m);
        }
    }
    hall(map, &mut opaque);
    let count = opaque.len() / FLOATS_PER_VERTEX;
    opaque.extend(clear);
    (opaque, count)
}

fn is_ramp(b: &crate::map::Brush) -> bool {
    b.kind != BrushKind::Clip
        && b.sides
            .iter()
            .any(|s| s.drawn && s.plane.n.z > WALL_Z && s.plane.n.z < RAMP_TOP)
}

/// Which mat each brush belongs to: ramp brushes whose bounds touch are one
/// ramp, numbered in the order the map lists them.
fn ramp_mats(map: &Map) -> Vec<usize> {
    let n = map.brushes.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    let ramps: Vec<usize> = (0..n).filter(|&i| is_ramp(&map.brushes[i])).collect();
    let pad = 2.0;
    for (k, &i) in ramps.iter().enumerate() {
        let a = &map.brushes[i];
        for &j in &ramps[k + 1..] {
            let b = &map.brushes[j];
            let touch = a.maxs.x + pad >= b.mins.x
                && b.maxs.x + pad >= a.mins.x
                && a.maxs.y + pad >= b.mins.y
                && b.maxs.y + pad >= a.mins.y
                && a.maxs.z + pad >= b.mins.z
                && b.maxs.z + pad >= a.mins.z;
            if touch {
                let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                if ri != rj {
                    parent[rj] = ri;
                }
            }
        }
    }
    let mut label = vec![usize::MAX; n];
    let mut next = 0;
    let mut out = vec![0; n];
    for &i in &ramps {
        let r = find(&mut parent, i);
        if label[r] == usize::MAX {
            label[r] = next;
            next += 1;
        }
        out[i] = label[r];
    }
    out
}

/// The gymnasium around the course: an inward-facing box at the map's edge.
fn hall(map: &Map, out: &mut Vec<f32>) {
    let lo = map.mins - v3(64.0, 64.0, 64.0);
    let hi = map.maxs + v3(64.0, 64.0, 64.0);
    let c = |x: f32, y: f32, z: f32| v3(x, y, z);
    let faces = [
        // floor, ceiling, then four walls; each wound to face inward
        (
            [
                c(lo.x, lo.y, lo.z),
                c(hi.x, lo.y, lo.z),
                c(hi.x, hi.y, lo.z),
                c(lo.x, hi.y, lo.z),
            ],
            v3(0.0, 0.0, 1.0),
        ),
        (
            [
                c(lo.x, lo.y, hi.z),
                c(lo.x, hi.y, hi.z),
                c(hi.x, hi.y, hi.z),
                c(hi.x, lo.y, hi.z),
            ],
            v3(0.0, 0.0, -1.0),
        ),
        (
            [
                c(lo.x, lo.y, lo.z),
                c(lo.x, hi.y, lo.z),
                c(lo.x, hi.y, hi.z),
                c(lo.x, lo.y, hi.z),
            ],
            v3(1.0, 0.0, 0.0),
        ),
        (
            [
                c(hi.x, lo.y, lo.z),
                c(hi.x, lo.y, hi.z),
                c(hi.x, hi.y, hi.z),
                c(hi.x, hi.y, lo.z),
            ],
            v3(-1.0, 0.0, 0.0),
        ),
        (
            [
                c(lo.x, lo.y, lo.z),
                c(lo.x, lo.y, hi.z),
                c(hi.x, lo.y, hi.z),
                c(hi.x, lo.y, lo.z),
            ],
            v3(0.0, 1.0, 0.0),
        ),
        (
            [
                c(lo.x, hi.y, lo.z),
                c(hi.x, hi.y, lo.z),
                c(hi.x, hi.y, hi.z),
                c(lo.x, hi.y, hi.z),
            ],
            v3(0.0, -1.0, 0.0),
        ),
    ];
    for (poly, n) in faces {
        push_polygon(out, &poly, n, HALL, M_HALL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::course::tests::load;

    #[test]
    fn the_world_has_ramps_floors_pits_and_doors() {
        let (km, kc) = load("surf_kitsune");
        let (kv, _) = world(&km, &kc);
        let kmats: Vec<f32> = kv.chunks(FLOATS_PER_VERTEX).map(|c| c[9]).collect();
        for m in [M_MAT, M_FOAM, M_FINISH, M_DOOR_EARLY, M_HALL] {
            assert!(kmats.contains(&m), "kitsune: material {m} missing");
        }
        let (um, uc) = load("surf_utopia");
        let (v, opaque) = world(&um, &uc);
        assert_eq!(v.len() % (3 * FLOATS_PER_VERTEX), 0, "whole triangles");
        assert!(opaque * FLOATS_PER_VERTEX <= v.len());
        let mats: Vec<f32> = v.chunks(FLOATS_PER_VERTEX).map(|c| c[9]).collect();
        for m in [M_MAT, M_WOOD, M_BLOCK, M_FOAM, M_FINISH, M_EXIT, M_HALL] {
            assert!(mats.contains(&m), "material {m} missing");
        }
        // every normal is unit length
        for c in v.chunks(FLOATS_PER_VERTEX) {
            let n = v3(c[3], c[4], c[5]);
            assert!((n.length() - 1.0).abs() < 1e-3);
        }
    }

    #[test]
    fn polygons_face_their_normal() {
        let mut out = Vec::new();
        let n = v3(0.0, 0.0, 1.0);
        // clockwise from above: must be flipped
        push_polygon(
            &mut out,
            &[v3(0.0, 0.0, 0.0), v3(0.0, 1.0, 0.0), v3(1.0, 1.0, 0.0)],
            n,
            [1.0; 3],
            0.0,
        );
        let p = |i: usize| v3(out[i * 10], out[i * 10 + 1], out[i * 10 + 2]);
        assert!((p(1) - p(0)).cross(p(2) - p(0)).dot(n) > 0.0);
    }
}
