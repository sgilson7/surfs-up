//! The surface sweep: every drawn face of every solid brush on every course,
//! and every trigger, touched by the real mover and run rules, headless.
//!
//! * Floors (normal.z >= 0.7): a standing surfer is dropped onto sample
//!   points of each face. It must land on it and stay: not fall through, not
//!   be left in the air. Where a kill box (a teleport that is not a finish,
//!   or a hurt) catches the surfer, the sweep looks for platforms inside it:
//!   a floor it catches well above the lowest floor it catches (a step or a
//!   block standing in the kill box), or a platform it catches only part of
//!   (a kill box overlapping a platform's edge). Those are listed for a
//!   person to judge; a kill floor caught all over is what kill boxes are for.
//! * Ramps (0.1 < normal.z < 0.7): a surfer is set on each face moving along
//!   it at surfing speed both ways, holding the strafe key into the ramp. It
//!   must not stand on the ramp as ground, not fall through it, and not stop
//!   dead with nothing ahead of it (a rampbug). Displacement terrain is
//!   checked for falling through only: it is bumpy by nature.
//! * Triggers: a surfer put inside each one, alone (the others set aside),
//!   gets what the course says it does: a finish stops the clock, a restart
//!   sends you back, a move takes you to its destination, a push pushes, a
//!   hurt hurts, a rename names.
//!
//! It is ignored by plain `cargo test`; `scripts/check.sh` runs it in release:
//!
//!     cargo test --release -p surf_core --test sweep -- --ignored --nocapture
//!
//! SWEEP_ALL=1 prints every line rather than the first few of each kind.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use surf_core::course::{Course, Role};
use surf_core::map::{
    box_touches, face_polygon, BrushKind, Map, Plane, TriggerKind, HINT_TERRAIN, NO_FILTER,
};
use surf_core::movement::{Body, Cmd, Mover, B_LEFT, B_RIGHT, HULL_MAXS, HULL_MINS};
use surf_core::run::{Surfer, Tick, FULL_HEALTH};
use surf_core::vec3::{v3, V3, ZERO};

const MAPS: &str = include_str!("../../../data/maps/maps.json");

fn load(id: &str) -> (Map, Course) {
    let path = format!("{}/../../data/maps/{id}.bin", env!("CARGO_MANIFEST_DIR"));
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut map = Map::parse(&bytes).unwrap();
    let course = Course::parse(MAPS, id, &mut map).unwrap();
    (map, course)
}

fn ids() -> Vec<String> {
    let v: serde_json::Value = serde_json::from_str(MAPS).unwrap();
    v["maps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap().to_string())
        .collect()
}

fn f(p: [f64; 3]) -> V3 {
    v3(p[0] as f32, p[1] as f32, p[2] as f32)
}

fn centroid(poly: &[[f64; 3]]) -> V3 {
    let k = poly.len() as f64;
    f(poly.iter().fold([0.0; 3], |a, p| {
        [a[0] + p[0] / k, a[1] + p[1] / k, a[2] + p[2] / k]
    }))
}

fn area(poly: &[[f64; 3]]) -> f64 {
    let mut s = [0.0; 3];
    for i in 1..poly.len().saturating_sub(1) {
        let a = [0, 1, 2].map(|k| poly[i][k] - poly[0][k]);
        let b = [0, 1, 2].map(|k| poly[i + 1][k] - poly[0][k]);
        s[0] += a[1] * b[2] - a[2] * b[1];
        s[1] += a[2] * b[0] - a[0] * b[2];
        s[2] += a[0] * b[1] - a[1] * b[0];
    }
    0.5 * (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt()
}

/// Whether `p`, projected along the face's normal, falls inside the face.
fn over_face(poly: &[[f64; 3]], n: V3, p: V3) -> bool {
    let mut sign = 0.0f32;
    for i in 0..poly.len() {
        let a = f(poly[i]);
        let b = f(poly[(i + 1) % poly.len()]);
        let c = (b - a).cross(p - a).dot(n);
        if c.abs() < 1e-3 {
            continue;
        }
        if sign == 0.0 {
            sign = c.signum();
        } else if c.signum() != sign {
            return false;
        }
    }
    true
}

fn here(p: V3) -> String {
    format!("({:.0}, {:.0}, {:.0})", p.x, p.y, p.z)
}

fn normal(n: V3) -> String {
    format!("normal {:.2} {:.2} {:.2}", n.x, n.y, n.z)
}

/// Whether a tick sent the surfer anywhere or hurt them.
fn caught(t: &Tick, s: &Surfer) -> bool {
    t.finished.is_some() || t.exited.is_some() || t.fell || t.teleported || s.health < FULL_HEALTH
}

/// A kill box: a teleport anyone can touch that is not a finish, or a hurt.
fn kills(map: &Map, course: &Course, i: usize) -> bool {
    let t = &map.triggers[i];
    match t.kind {
        TriggerKind::Teleport(d) => t.filter == NO_FILTER && !matches!(course.role(t, d), Role::Finish(_)),
        TriggerKind::Hurt(_) => t.filter == NO_FILTER,
        _ => false,
    }
}

/// The kill boxes a hull standing at `p` touches (as run.rs reaches them:
/// the whole hull, and 2 units under the feet).
fn killers_at(map: &Map, course: &Course, p: V3) -> Vec<usize> {
    let lo = p + HULL_MINS - v3(0.0, 0.0, 2.0);
    let hi = p + HULL_MAXS;
    (0..map.triggers.len())
        .filter(|&i| {
            let t = &map.triggers[i];
            t.maxs.x >= lo.x
                && t.mins.x <= hi.x
                && t.maxs.y >= lo.y
                && t.mins.y <= hi.y
                && t.maxs.z >= lo.z
                && t.mins.z <= hi.z
                && kills(map, course, i)
                && box_touches(&t.planes, lo, hi)
        })
        .collect()
}

/// A floor a kill box caught: its height, its centre, caught all over, terrain.
type CaughtFloor = (f32, V3, bool, bool);

#[derive(Default)]
struct Tally {
    floors: usize,
    terrain: usize,
    floor_drops: usize,
    no_room: usize,
    caught_drops: usize,
    ramps: usize,
    ramp_rides: usize,
    triggers: usize,
    failures: Vec<String>,
    caught: BTreeMap<usize, Vec<CaughtFloor>>,
    review: Vec<String>,
    notes: Vec<String>,
}

/// A standing surfer dropped onto each sample of a floor face.
fn sweep_floor(
    map: &Map,
    mv: &mut Mover,
    course: &Course,
    poly: &[[f64; 3]],
    n: V3,
    terrain: bool,
    tally: &mut Tally,
) {
    let c = centroid(poly);
    let mut samples = vec![c];
    // halfway from the centre to each corner, where a whole hull still
    // stands on a face of any fair size
    if area(poly) > 48.0 * 48.0 {
        samples.extend(poly.iter().map(|p| (f(*p) + c) * 0.5));
    }
    // the hull's lowest corner touches a tilted floor first
    let lift = 1.0 + 16.0 * (n.x.abs() + n.y.abs()) / n.z;
    let mut tried = 0;
    let mut hits: Vec<Vec<usize>> = Vec::new();
    for p in samples {
        let start = Body::at(p + v3(0.0, 0.0, lift));
        if mv.trace(&start, start.origin, start.origin).start_solid {
            tally.no_room += 1;
            continue;
        }
        tried += 1;
        tally.floor_drops += 1;
        let mut s = Surfer::from_body(start, Cmd::new(0, 0.0, 0.0));
        let mut was_caught = false;
        let mut before = s.body.origin;
        for _ in 0..40 {
            before = s.body.origin;
            let t = s.step(mv, course, Cmd::new(0, 0.0, 0.0));
            if caught(&t, &s) {
                was_caught = true;
                break;
            }
            if s.body.on_ground {
                break;
            }
        }
        if was_caught {
            tally.caught_drops += 1;
            hits.push(killers_at(map, course, before));
            continue;
        }
        let b = s.body;
        let slid = v3(b.origin.x - p.x, b.origin.y - p.y, 0.0).length();
        let through = b.origin.z < p.z - 2.0 - slid;
        // terrain: a hull dropped near a triangle at the walkable limit can
        // meet a steeper neighbour and slide, or rest in the crease between
        // two; only falling through it is wrong
        let off = !b.on_ground || slid > 16.0;
        if terrain && off && !through {
            tally.notes.push(format!(
                "terrain floor at {} ({}): dropped from {}, ended at {} {}",
                here(p),
                normal(n),
                here(start.origin),
                here(b.origin),
                if b.on_ground {
                    "on the ground"
                } else {
                    "in the air, in a crease"
                }
            ));
        } else if off || through {
            tally.failures.push(format!(
                "floor at {} ({}{}): dropped from {}, ended at {} {}",
                here(p),
                normal(n),
                if terrain { ", terrain" } else { "" },
                here(start.origin),
                here(b.origin),
                if b.on_ground {
                    "on the ground"
                } else {
                    "in the air"
                }
            ));
        }
    }
    if tried == 0 {
        return;
    }
    let all = hits.len() == tried;
    let mut boxes: Vec<usize> = hits.into_iter().flatten().collect();
    boxes.sort_unstable();
    boxes.dedup();
    for k in boxes {
        tally.caught.entry(k).or_default().push((c.z, c, all, terrain));
    }
}

/// Whether some walkable face (normal.z >= 0.7, or the bevel Source adds
/// at a brush's top ridge) of any blocking brush lies under the hull at
/// `o`, at its feet.
fn walkable_under(map: &Map, o: V3) -> bool {
    let lo = o + HULL_MINS - v3(0.0, 0.0, 3.0);
    let hi = o + v3(16.0, 16.0, 1.0);
    let mut near = Vec::new();
    map.brushes_near(lo, hi, &mut near);
    near.iter().any(|&k| {
        let br = &map.brushes[k as usize];
        let planes: Vec<Plane> = br.sides.iter().map(|s| s.plane).collect();
        let overlaps = br.maxs.x >= lo.x && br.mins.x <= hi.x && br.maxs.y >= lo.y && br.mins.y <= hi.y;
        // a bevel has no polygon: its height is the brush's top
        let ridge = br.sides.iter().any(|s| s.bevel && s.plane.n.z >= 0.7)
            && overlaps
            && br.maxs.z >= lo.z
            && br.maxs.z <= hi.z;
        ridge
            || (0..planes.len()).filter(|&i| planes[i].n.z >= 0.7).any(|i| {
                let poly = face_polygon(&planes, i);
                let (mut a, mut b) = (v3(f32::MAX, f32::MAX, f32::MAX), v3(f32::MIN, f32::MIN, f32::MIN));
                for q in &poly {
                    let q = f(*q);
                    a = v3(a.x.min(q.x), a.y.min(q.y), a.z.min(q.z));
                    b = v3(b.x.max(q.x), b.y.max(q.y), b.z.max(q.z));
                }
                !poly.is_empty()
                    && b.x >= lo.x
                    && a.x <= hi.x
                    && b.y >= lo.y
                    && a.y <= hi.y
                    && b.z >= lo.z
                    && a.z <= hi.z
            })
    })
}

/// A point inside a convex set of planes: the mean of its corners.
fn inside(planes: &[Plane]) -> V3 {
    let mut sum = ZERO;
    let mut k = 0.0f32;
    for i in 0..planes.len() {
        for p in face_polygon(planes, i) {
            sum += f(p);
            k += 1.0;
        }
    }
    sum * (1.0 / k.max(1.0))
}

/// A surfer riding a ramp face both ways along it.
fn sweep_ramp(mv: &mut Mover, course: &Course, poly: &[[f64; 3]], n: V3, terrain: bool, tally: &mut Tally) {
    let c = centroid(poly);
    let along = v3(-n.y, n.x, 0.0).normalized();
    let uphill = v3(-n.x, -n.y, 0.0).normalized();
    // the hull corner that touches the ramp first
    let corner = v3(-16.0 * n.x.signum(), -16.0 * n.y.signum(), 0.0);
    let origin = c + n * 0.5 - corner;
    let start = Body::at(origin);
    if mv.trace(&start, origin, origin).start_solid {
        tally.no_room += 1;
        return;
    }
    let plane = c.dot(n);
    // SWEEP_RIDE=x,y,z prints each tick of the rides of the ramp centred there
    let show = std::env::var("SWEEP_RIDE").ok().is_some_and(|w| {
        let w: Vec<f32> = w.split(',').map(|x| x.parse().unwrap()).collect();
        (c - v3(w[0], w[1], w[2])).length() < 2.0
    });
    for dir in [along, along * -1.0] {
        tally.ramp_rides += 1;
        let yaw = dir.y.atan2(dir.x).to_degrees();
        let right = v3(dir.y, -dir.x, 0.0);
        let key = if right.dot(uphill) > 0.0 { B_RIGHT } else { B_LEFT };
        let mut body = start;
        body.velocity = dir * 1000.0;
        let mut s = Surfer::from_body(body, Cmd::new(key, yaw, 0.0));
        let mut pushed = false;
        for tick in 0..48 {
            let before = s.body;
            let t = s.step(mv, course, Cmd::new(key, yaw, 0.0));
            if caught(&t, &s) || t.stage.is_some() {
                break;
            }
            let b = s.body;
            if show {
                println!(
                    "ride {} tick {tick}: at {} moving ({:.0}, {:.0}, {:.0}) ground {} pushed {} {t:?}",
                    here(dir * 100.0),
                    here(b.origin),
                    b.velocity.x,
                    b.velocity.y,
                    b.velocity.z,
                    b.on_ground,
                    b.pushed
                );
            }
            // the corner's height over the ramp's plane: the ride goes on
            // while it stays on the plane, past the face's edge onto a
            // neighbour in the same plane (across the seam), and ends where
            // the plane does
            let gap = (b.origin + corner).dot(n) - plane;
            if gap.abs() >= 24.0 {
                break;
            }
            let on_face = over_face(poly, n, b.origin + corner);
            if gap < -2.0 && on_face {
                tally.failures.push(format!(
                    "ramp centred {} ({}{}): fell through at {} ({:.1} under it)",
                    here(c),
                    normal(n),
                    if terrain { ", terrain" } else { "" },
                    here(b.origin),
                    -gap
                ));
                break;
            }
            if b.on_ground {
                // on ground: fair on any walkable face under the hull, a
                // ridge or a neighbour's top edge (the mover looks under each
                // quarter of the hull, as Source's TryTouchGroundInQuadrants
                // does), wrong on a surf ramp with only the ramp under it (a
                // face at the walkable limit, normal.z 0.69, is a floor to
                // within rounding; a rock's small facets are all edges, whose
                // bevels Source stands on too)
                if !walkable_under(mv.map, b.origin) && n.z < 0.68 && area(poly) > 128.0 * 128.0 {
                    tally.review.push(format!(
                        "ramp centred {} ({}): stood on as ground at {} after {tick} ticks",
                        here(c),
                        normal(n),
                        here(b.origin)
                    ));
                }
                break;
            }
            // a push, the start zone's speed cap and water all slow you fairly
            pushed |= b.pushed || b.base_velocity != ZERO;
            let in_zone = course
                .zone
                .contains_box(b.origin + HULL_MINS, b.origin + HULL_MAXS);
            if terrain || pushed || in_zone || mv.map.in_water(b.origin + v3(0.0, 0.0, 1.0)) {
                continue;
            }
            let h0 = v3(before.velocity.x, before.velocity.y, 0.0).length();
            let h1 = v3(b.velocity.x, b.velocity.y, 0.0).length();
            if h0 > 300.0 && h1 < h0 * 0.5 {
                // stopped: by a wall or a kink ahead (fair), or by a seam or
                // lip under a unit high (a rampbug): ride the same ticks again
                // a unit off the ramp; only a rampbug lets that ride sail on
                let mut lifted = before;
                lifted.origin += n * 1.0;
                let mut r = Surfer::from_body(lifted, Cmd::new(key, yaw, 0.0));
                let mut kept = true;
                for _ in 0..4 {
                    r.step(mv, course, Cmd::new(key, yaw, 0.0));
                    kept &= v3(r.body.velocity.x, r.body.velocity.y, 0.0).length() > h0 * 0.8;
                }
                if kept {
                    tally.failures.push(format!(
                        "ramp centred {} ({}): stopped dead at {} ({h0:.0} -> {h1:.0} u/s) by a seam or lip a unit off the ramp clears",
                        here(c),
                        normal(n),
                        here(b.origin)
                    ));
                }
                break;
            }
        }
    }
}

/// A surfer put inside each trigger, alone, gets what the course says it does.
fn sweep_triggers(map: &mut Map, course: &Course, tally: &mut Tally) {
    let all = std::mem::take(&mut map.triggers);
    let half = (HULL_MAXS + HULL_MINS) * 0.5;
    for (i, tr) in all.iter().enumerate() {
        tally.triggers += 1;
        map.triggers = vec![tr.clone()];
        let mut mv = Mover::new(map);
        let mid = inside(&tr.planes);
        let mut s = Surfer::from_body(Body::at(mid - half), Cmd::new(0, 0.0, 0.0));
        if tr.filter != NO_FILTER {
            s.name = tr.filter;
        }
        let before = s.clone();
        let mut t = s.step(&mut mv, course, Cmd::new(0, 0.0, 0.0));
        if t.skipped {
            // a finish on a course that wants every stage first: try again
            // as a surfer who has come through them all
            s = before;
            s.stage = course.stages.len().saturating_sub(1) as u8;
            t = s.step(&mut mv, course, Cmd::new(0, 0.0, 0.0));
        }
        let say = |what: String| format!("trigger {i} {:?} centred {}: {what}", tr.kind, here(mid));
        match tr.kind {
            TriggerKind::Teleport(d) => {
                let role = course.role(tr, d);
                let ok = match role {
                    Role::Finish(_) => t.finished.is_some(),
                    Role::Exit(_) => t.exited.is_some(),
                    Role::Restart => t.fell,
                    Role::Move => match map.destination(d) {
                        Some((p, _)) => t.teleported || (s.body.origin - p).length() < 1.0,
                        None => {
                            tally
                                .failures
                                .push(say(format!("goes to {:?}, which the map lacks", map.name(d))));
                            continue;
                        }
                    },
                };
                if !ok {
                    tally.failures.push(say(format!("as {role:?} it did {t:?}")));
                }
            }
            // a finish zone around it outranks it (lego's last push is in its end room)
            _ if t.finished.is_some() => {}
            TriggerKind::Push(..) => {
                if !s.body.pushed || s.body.base_velocity == ZERO {
                    tally.failures.push(say("did not push".into()));
                }
            }
            TriggerKind::Hurt(_) => {
                if !(s.health < FULL_HEALTH || t.fell) {
                    tally.failures.push(say("did not hurt".into()));
                }
            }
            TriggerKind::Rename(n, wait) => {
                for _ in 0..wait {
                    s.step(&mut mv, course, Cmd::new(0, 0.0, 0.0));
                }
                if s.name != n {
                    tally.failures.push(say(format!("named {} not {}", s.name, n)));
                }
            }
        }
    }
    map.triggers = all;
    // the same touch with every trigger in place: renames that overlap
    // another rename, where which one names you depends on which is first
    let mut mv = Mover::new(map);
    for (i, tr) in map.triggers.iter().enumerate() {
        let TriggerKind::Rename(n, wait) = tr.kind else {
            continue;
        };
        let mid = inside(&tr.planes);
        let mut s = Surfer::from_body(Body::at(mid - half), Cmd::new(0, 0.0, 0.0));
        if tr.filter != NO_FILTER {
            s.name = tr.filter;
        }
        let mut moved = false;
        for _ in 0..=wait {
            moved |= s.step(&mut mv, course, Cmd::new(0, 0.0, 0.0)).teleported;
        }
        if s.name != n && !moved {
            tally.notes.push(format!(
                "trigger {i} Rename({n}, {wait}) centred {} overlaps another rename, which names you {} ({:?})",
                here(mid),
                s.name,
                map.name(s.name)
            ));
        }
    }
}

fn sweep(id: &str) -> Tally {
    let (mut map, course) = load(id);
    let mut tally = Tally::default();
    {
        let mut mv = Mover::new(&map);
        for br in &map.brushes {
            if !matches!(br.kind, BrushKind::Solid | BrushKind::Window) {
                continue;
            }
            let planes: Vec<Plane> = br.sides.iter().map(|s| s.plane).collect();
            for (i, side) in br.sides.iter().enumerate() {
                if side.bevel || !side.drawn {
                    continue;
                }
                let n = side.plane.n;
                if n.z < 0.1 {
                    continue;
                }
                let poly = face_polygon(&planes, i);
                if poly.len() < 3 || area(&poly) < 4.0 {
                    continue;
                }
                let terrain = side.hint == HINT_TERRAIN;
                if terrain {
                    tally.terrain += 1;
                }
                if n.z >= 0.7 {
                    tally.floors += 1;
                    sweep_floor(&map, &mut mv, &course, &poly, n, terrain, &mut tally);
                } else {
                    tally.ramps += 1;
                    sweep_ramp(&mut mv, &course, &poly, n, terrain, &mut tally);
                }
            }
        }
    }
    // platforms inside kill boxes: a floor a kill box catches well above the
    // lowest floor it catches, or a built (not terrain) floor it catches in part
    for (&k, floors) in &tally.caught {
        let low = floors.iter().map(|f| f.0).fold(f32::MAX, f32::min);
        let t = &map.triggers[k];
        let what = match t.kind {
            TriggerKind::Teleport(d) => format!("{:?} to {}", course.role(t, d), map.name(d)),
            k => format!("{k:?}"),
        };
        for &(z, c, all, terrain) in floors {
            // terrain dips into kill boxes all over; built floors are platforms
            let raised = z > low + 16.0 && !terrain;
            let part = !all && !terrain;
            if raised || part {
                tally.review.push(format!(
                    "kill box {k} ({what}, {} .. {}) catches {} floor at {}{}",
                    here(t.mins),
                    here(t.maxs),
                    if all { "all of a" } else { "part of a" },
                    here(c),
                    if raised {
                        format!(", {:.0} above the lowest floor it catches", z - low)
                    } else {
                        String::new()
                    }
                ));
            }
        }
    }
    sweep_triggers(&mut map, &course, &mut tally);
    tally
}

/// Where a failure is: the course, and the text before its colon (the face
/// or the trigger), the same on every run.
fn key(id: &str, failure: &str) -> String {
    format!("{id} {}", failure.split(": ").next().unwrap_or(failure))
}

/// The findings a person has looked at and left (SECOND-ORDER.md): seams
/// and lips of the original maps' own geometry. A failure not listed fails
/// the sweep; SWEEP_BLESS=1 rewrites the list.
const KNOWN: &str = include_str!("sweep_known.txt");

#[test]
#[ignore]
fn every_surface_and_trigger_on_every_course() {
    let every = std::env::var("SWEEP_ALL").is_ok();
    let cap = if every { usize::MAX } else { 12 };
    let known: Vec<&str> = KNOWN
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    let mut text = String::new();
    let mut found: Vec<String> = Vec::new();
    let mut new = 0;
    // the courses are independent: sweep them side by side
    let ids = ids();
    let tallies: Vec<Tally> = std::thread::scope(|s| {
        let handles: Vec<_> = ids.iter().map(|id| s.spawn(move || sweep(id))).collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for (id, t) in ids.iter().zip(tallies) {
        let _ = writeln!(
            text,
            "{id}: {} floors ({} drops, {} caught), {} ramps ({} rides), {} of them terrain, {} with no room; {} triggers; {} failures, {} to review, {} notes",
            t.floors,
            t.floor_drops,
            t.caught_drops,
            t.ramps,
            t.ramp_rides,
            t.terrain,
            t.no_room,
            t.triggers,
            t.failures.len(),
            t.review.len(),
            t.notes.len()
        );
        for l in &t.failures {
            let k = key(id, l);
            let listed = known.contains(&k.as_str());
            if !found.contains(&k) {
                found.push(k);
            }
            if !listed {
                new += 1;
                let _ = writeln!(text, "  FAIL {l}");
            } else if every {
                let _ = writeln!(text, "  KNOWN {l}");
            }
        }
        for l in t.review.iter().take(cap) {
            let _ = writeln!(text, "  REVIEW {l}");
        }
        for l in t.notes.iter().take(cap) {
            let _ = writeln!(text, "  NOTE {l}");
        }
    }
    for k in known.iter().filter(|k| !found.iter().any(|f| f == *k)) {
        let _ = writeln!(text, "  GONE (passes now; take it off sweep_known.txt) {k}");
    }
    println!("{text}");
    if std::env::var("SWEEP_BLESS").is_ok() {
        let path = format!("{}/tests/sweep_known.txt", env!("CARGO_MANIFEST_DIR"));
        let head: String = KNOWN
            .lines()
            .take_while(|l| l.starts_with('#'))
            .map(|l| format!("{l}\n"))
            .collect();
        std::fs::write(&path, head + &found.join("\n") + "\n").unwrap();
        return;
    }
    assert_eq!(new, 0, "surface sweep: {new} new failure(s):\n{text}");
}

/// A developer's look at one place: SWEEP_PROBE=course,x,y,z,r lists the
/// brushes and triggers within r of the point.
#[test]
#[ignore]
fn probe() {
    let Ok(arg) = std::env::var("SWEEP_PROBE") else {
        return;
    };
    let a: Vec<&str> = arg.split(',').collect();
    let (map, course) = load(a[0]);
    let p = v3(
        a[1].parse().unwrap(),
        a[2].parse().unwrap(),
        a[3].parse().unwrap(),
    );
    let r: f32 = a[4].parse().unwrap();
    let near = |lo: V3, hi: V3| {
        !(hi.x < p.x - r
            || lo.x > p.x + r
            || hi.y < p.y - r
            || lo.y > p.y + r
            || hi.z < p.z - r
            || lo.z > p.z + r)
    };
    println!("spawn {} yaw {}", here(map.spawn), map.spawn_yaw);
    for (k, br) in map
        .brushes
        .iter()
        .enumerate()
        .filter(|(_, b)| near(b.mins, b.maxs))
    {
        println!("brush {k} {:?} {} .. {}", br.kind, here(br.mins), here(br.maxs));
    }
    for (k, br) in map.brushes.iter().enumerate() {
        let planes: Vec<Plane> = br.sides.iter().map(|s| s.plane).collect();
        for (i, side) in br.sides.iter().enumerate() {
            let poly = face_polygon(&planes, i);
            if poly.len() >= 3 && (centroid(&poly) - p).length() < r {
                println!(
                    "face of brush {k} ({:?}, bounds {} .. {}) side {i}: {}, drawn {}, bevel {}, corners {:?}",
                    br.kind,
                    here(br.mins),
                    here(br.maxs),
                    normal(side.plane.n),
                    side.drawn,
                    side.bevel,
                    poly.iter().map(|q| here(f(*q))).collect::<Vec<_>>()
                );
            }
        }
    }
    for (k, t) in map
        .triggers
        .iter()
        .enumerate()
        .filter(|(_, t)| near(t.mins, t.maxs))
    {
        let role = match t.kind {
            TriggerKind::Teleport(d) => format!("{:?} to {}", course.role(t, d), map.name(d)),
            _ => String::new(),
        };
        println!(
            "trigger {k} {:?} {role} filter {} {} .. {}",
            t.kind,
            t.filter,
            here(t.mins),
            here(t.maxs)
        );
    }
}
