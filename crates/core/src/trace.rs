//! Sweeping the player's box through the map: Source's CM_ClipBoxToBrush
//! (engine/cmodel.cpp in the 2013 SDK, from Quake 2's CM_ClipBoxToBrush).
//! Each brush plane is pushed out by the box corner that meets it first, so a
//! box sweep becomes a point sweep against bigger planes. Bevel planes are
//! used, as they are for box traces in Source, so a box cannot snag on the
//! sharp edges of a ramp.

use crate::map::{Map, Plane};
use crate::vec3::{v3, V3};

/// Source's DIST_EPSILON (1/32 unit): a trace stops this far short of a plane.
pub const DIST_EPSILON: f32 = 0.03125;

#[derive(Clone, Copy, Debug)]
pub struct Trace {
    pub fraction: f32,
    pub end: V3,
    pub normal: V3,
    pub start_solid: bool,
    pub all_solid: bool,
}

impl Trace {
    pub fn hit(&self) -> bool {
        self.fraction < 1.0
    }
}

/// Scratch space a tracer reuses between traces.
#[derive(Default)]
pub struct Scratch {
    near: Vec<u32>,
}

/// Sweep the box [mins, maxs] (relative to the origin) from `start` to `end`.
pub fn trace_box(map: &Map, scratch: &mut Scratch, start: V3, end: V3, mins: V3, maxs: V3) -> Trace {
    let mut t = Trace {
        fraction: 1.0,
        end,
        normal: V3::default(),
        start_solid: false,
        all_solid: false,
    };
    let lo = v3(start.x.min(end.x), start.y.min(end.y), start.z.min(end.z)) + mins - v3(1.0, 1.0, 1.0);
    let hi = v3(start.x.max(end.x), start.y.max(end.y), start.z.max(end.z)) + maxs + v3(1.0, 1.0, 1.0);
    map.brushes_near(lo, hi, &mut scratch.near);
    for &i in &scratch.near {
        let b = &map.brushes[i as usize];
        clip_box_to_brush(&mut t, b.sides.iter().map(|s| &s.plane), start, end, mins, maxs);
        if t.all_solid {
            break;
        }
    }
    if t.fraction == 1.0 {
        t.end = end;
    } else {
        t.end = start + (end - start) * t.fraction;
    }
    t
}

fn clip_box_to_brush<'a>(
    t: &mut Trace,
    planes: impl Iterator<Item = &'a Plane>,
    start: V3,
    end: V3,
    mins: V3,
    maxs: V3,
) {
    let mut enter = -1.0f32;
    let mut leave = 1.0f32;
    let mut clip: Option<V3> = None;
    let mut start_out = false;
    let mut get_out = false;
    for p in planes {
        // the corner of the box that reaches furthest against this plane
        let ofs = v3(
            if p.n.x < 0.0 { maxs.x } else { mins.x },
            if p.n.y < 0.0 { maxs.y } else { mins.y },
            if p.n.z < 0.0 { maxs.z } else { mins.z },
        );
        let dist = p.d - ofs.dot(p.n);
        let d1 = start.dot(p.n) - dist;
        let d2 = end.dot(p.n) - dist;
        if d2 > 0.0 {
            get_out = true;
        }
        if d1 > 0.0 {
            start_out = true;
        }
        // completely in front of this face: no intersection with the brush
        if d1 > 0.0 && (d2 >= DIST_EPSILON || d2 >= d1) {
            return;
        }
        // completely behind this face
        if d1 <= 0.0 && d2 <= 0.0 {
            continue;
        }
        if d1 > d2 {
            // entering
            let f = ((d1 - DIST_EPSILON) / (d1 - d2)).max(0.0);
            if f > enter {
                enter = f;
                clip = Some(p.n);
            }
        } else {
            // leaving
            let f = ((d1 + DIST_EPSILON) / (d1 - d2)).min(1.0);
            if f < leave {
                leave = f;
            }
        }
    }
    if !start_out {
        t.start_solid = true;
        if !get_out {
            t.all_solid = true;
            t.fraction = 0.0;
        }
        return;
    }
    if enter < leave && enter > -1.0 && enter < t.fraction {
        if let Some(n) = clip {
            t.fraction = enter.max(0.0);
            t.normal = n;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::tests::utopia;

    const MINS: V3 = v3(-16.0, -16.0, 0.0);
    const MAXS: V3 = v3(16.0, 16.0, 72.0);

    #[test]
    fn falling_at_the_start_lands_on_its_floor() {
        let m = utopia();
        let mut s = Scratch::default();
        let a = m.spawn + v3(0.0, 0.0, 10.0);
        let t = trace_box(m, &mut s, a, a - v3(0.0, 0.0, 1000.0), MINS, MAXS);
        assert!(t.hit(), "no floor under the start");
        assert!(t.normal.z > 0.99, "floor faces up: {:?}", t.normal);
        assert!(!t.start_solid);
        let floor = t.end.z;
        let t2 = trace_box(m, &mut s, t.end, t.end - v3(0.0, 0.0, 1.0), MINS, MAXS);
        assert!(t2.fraction < 0.1, "resting on the floor, {floor}");
    }

    #[test]
    fn a_box_inside_a_wall_is_all_solid() {
        let m = utopia();
        let mut s = Scratch::default();
        // the first ramp's body, well inside: find a blocking brush and sweep from its centre
        let b = m
            .brushes
            .iter()
            .find(|b| {
                b.kind.blocks()
                    && (b.maxs - b.mins).z > 200.0
                    && (b.maxs - b.mins).x > 200.0
                    && (b.maxs - b.mins).y > 200.0
            })
            .expect("a big brush");
        let c = (b.mins + b.maxs) * 0.5;
        let t = trace_box(
            m,
            &mut s,
            c,
            c + v3(1.0, 0.0, 0.0),
            v3(-1.0, -1.0, -1.0),
            v3(1.0, 1.0, 1.0),
        );
        assert!(t.start_solid);
    }
}
