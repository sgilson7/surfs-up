//! Surfing the real map: drop onto ramps and ride them.

use surf_core::map::{face_polygon, Map, Plane};
use surf_core::movement::{Body, Cmd, Mover, B_LEFT, B_RIGHT};
use surf_core::vec3::{v3, V3};

fn utopia() -> Map {
    Map::parse(include_bytes!("../../../data/maps/surf_utopia.bin")).expect("parse")
}

/// The centre of the first steep face (a surf ramp) found under `near`.
fn ramp_under(m: &Map, near: V3, radius: f32) -> (V3, V3) {
    let mut mv = Mover::new(m);
    let mut best: Option<(f32, V3, V3)> = None;
    for br in m.brushes.iter().filter(|b| b.kind.blocks()) {
        let planes: Vec<Plane> = br.sides.iter().map(|s| s.plane).collect();
        for (i, s) in br.sides.iter().enumerate() {
            if s.bevel || s.plane.n.z <= 0.3 || s.plane.n.z >= 0.7 {
                continue;
            }
            let poly = face_polygon(&planes, i);
            if poly.is_empty() {
                continue;
            }
            let k = poly.len() as f64;
            let c = poly.iter().fold([0.0; 3], |a, p| {
                [a[0] + p[0] / k, a[1] + p[1] / k, a[2] + p[2] / k]
            });
            let c = v3(c[0] as f32, c[1] as f32, c[2] as f32);
            let d = (c - near).length();
            let probe = Body::at(c + s.plane.n * 40.0 + v3(0.0, 0.0, 10.0));
            let free = mv.trace(&probe, probe.origin, probe.origin);
            if free.start_solid || s.plane.n.y.abs() < 0.5 {
                continue;
            }
            if d < radius && best.map(|b| d < b.0).unwrap_or(true) {
                best = Some((d, c, s.plane.n));
            }
        }
    }
    let (_, c, n) = best.expect("a ramp nearby");
    (c, n)
}

#[test]
fn a_ramp_is_never_ground_and_you_slide_down_it() {
    let m = utopia();
    let mut mv = Mover::new(&m);
    let (c, n) = ramp_under(&m, v3(-13000.0, 250.0, 11450.0), 800.0);
    let start = c + n * 40.0 + v3(0.0, 0.0, 10.0);
    let mut b = Body::at(start);
    for tick in 0..96 {
        mv.player_move_same(&mut b, Cmd::new(0, 0.0, 0.0));
        assert!(
            !b.on_ground,
            "tick {tick}: a ramp became ground at {:?}",
            b.origin
        );
    }
    let free_fall = 800.0 * 1.5;
    println!("after 1.5 s: {:?} speed {}", b.origin, b.velocity.length());
    assert!(b.velocity.length() < free_fall * 0.95, "held up by the ramp");
    assert!(start.z - b.origin.z > 200.0, "went down");
    assert!(
        (b.origin - start).length2d() > 100.0,
        "slid sideways down the slope"
    );
}

#[test]
fn strafing_into_a_ramp_holds_you_on_it_and_builds_speed() {
    let m = utopia();
    let mut mv = Mover::new(&m);
    let (c, n) = ramp_under(&m, v3(-13000.0, 250.0, 11450.0), 800.0);
    // face along the ramp (horizontal, perpendicular to its normal) and hold
    // the strafe key toward the ramp
    let along = v3(-n.y, n.x, 0.0).normalized();
    let mut b = Body::at(c + n * 40.0 + v3(0.0, 0.0, 10.0));
    b.velocity = along * 600.0;
    let yaw_of = |v: V3| {
        // the yaw whose forward is closest to v, by search (no atan2 in tests of the core rules either)
        let mut best = (0.0f32, -2.0f32);
        let mut y = 0.0f32;
        while y < 360.0 {
            let (f, _, _) = surf_core::vec3::angle_vectors(0.0, y);
            let d = f.dot(v.normalized());
            if d > best.1 {
                best = (y, d);
            }
            y += 0.25;
        }
        best.0
    };
    let yaw = yaw_of(along);
    let (_, right, _) = surf_core::vec3::angle_vectors(0.0, yaw);
    // which strafe key pushes into the ramp?
    let key = if right.dot(n) < 0.0 { B_RIGHT } else { B_LEFT };
    let start_speed = b.velocity.length();
    for _ in 0..128 {
        mv.player_move_same(&mut b, Cmd::new(key, yaw, 0.0));
    }
    println!(
        "held {key} for 2 s: {:?} speed {} (from {start_speed})",
        b.origin,
        b.velocity.length()
    );
    assert!(
        b.velocity.length() > start_speed,
        "a ramp turns falling into speed"
    );
}

/// The first ridge of surf_utopia bends where two brushes meet. Riding it
/// while holding the key into the ramp once froze the surfer at the seam
/// (a rampbug: the same plane hit twice in one tick). Both sides now run
/// clean to the bottom of the ridge.
#[test]
fn the_first_ridge_has_no_rampbug_on_either_side() {
    use surf_core::course::Course;
    use surf_core::run::{Phase, Surfer};
    let mut m = utopia();
    let zone = Course::parse(
        include_str!("../../../data/maps/maps.json"),
        "surf_utopia",
        &mut m,
    )
    .unwrap();
    let mut mv = Mover::new(&m);
    for (side, key) in [(1.0f32, B_RIGHT), (-1.0, B_LEFT)] {
        let mut s = Surfer::at_start(&m);
        s.body.origin = v3(-13000.0, side * 120.0, 12600.0);
        s.phase = Phase::Running;
        let mut last = s.body.origin;
        for tick in 0..360 {
            let t = s.step(&mut mv, &zone, Cmd::new(key, 0.0, 0.0));
            assert!(!t.teleported, "side {side}: fell off at tick {tick}");
            let moved = (s.body.origin - last).length();
            if s.body.velocity.length() > 200.0 {
                assert!(
                    moved > 1.0,
                    "side {side}: frozen at {:?}, tick {tick}",
                    s.body.origin
                );
            }
            last = s.body.origin;
        }
        assert!(
            s.body.origin.x > -8000.0,
            "side {side}: rode the ridge to {:?}",
            s.body.origin
        );
        assert!(s.body.velocity.length() > 1500.0);
    }
}
