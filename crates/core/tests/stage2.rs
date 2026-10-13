//! Can stage 2 of surf_kitsune be done in this game's physics? A beam search
//! over yaw and keys. `stage_two_with_the_speed_stage_one_carries` runs in
//! the ordinary suite; the others are for exploring, on purpose:
//!     cargo test -p surf_core --release --test stage2 -- --ignored --nocapture
//! (env: BEAM width, WY weight on progress, VY arrival speed, ASUB air
//! sub-steps, UPF rising surface friction).

use surf_core::course::Course;
use surf_core::map::Map;
use surf_core::movement::{Cmd, Mover, B_FORWARD, B_JUMP, B_LEFT, B_RIGHT};
use surf_core::run::{Phase, Surfer};

fn setup() -> (Map, Course) {
    let mut m = Map::parse(include_bytes!("../../../data/maps/surf_kitsune.bin")).unwrap();
    let c = Course::parse(
        include_str!("../../../data/maps/maps.json"),
        "surf_kitsune",
        &mut m,
    )
    .unwrap();
    (m, c)
}

fn env(k: &str, d: f32) -> f32 {
    std::env::var(k).ok().and_then(|b| b.parse().ok()).unwrap_or(d)
}

fn beam_width() -> usize {
    env("BEAM", 200.0) as usize
}

pub fn search(m: &Map, c: &Course, start: &Surfer, ticks: usize, goal_y: f32) -> Option<(usize, Surfer)> {
    let mut mv = Mover::new(m);
    if let Ok(n) = std::env::var("ASUB") {
        mv.t.air_substeps = n.parse().unwrap();
    }
    if let Ok(f) = std::env::var("UPF") {
        mv.t.up_surface_friction = f.parse().unwrap();
    }
    let mut beam: Vec<(f32, Surfer)> = vec![(0.0, start.clone())];
    let yaws = [-20.0f32, -10.0, -5.0, -2.0, 0.0, 2.0, 5.0, 10.0, 20.0];
    let keys = [
        0,
        B_LEFT,
        B_RIGHT,
        B_LEFT | B_JUMP,
        B_RIGHT | B_JUMP,
        B_FORWARD | B_JUMP,
        B_FORWARD,
    ];
    let origin_stage = start.stage;
    for t in 0..ticks {
        let mut next: Vec<(f32, Surfer)> = Vec::new();
        for (_, s) in &beam {
            for dy in yaws {
                for k in keys {
                    let mut n = s.clone();
                    let yaw = n.last_cmd.yaw_deg() + dy;
                    let tick = n.step(&mut mv, c, Cmd::new(k, yaw, 0.0));
                    if tick.fell || (tick.teleported && n.stage == origin_stage) {
                        continue;
                    }
                    // stay on the outside of the ridge, high enough to make the
                    // jump to the far platform (its top is z -720)
                    let o = n.body.origin;
                    if origin_stage == 1 && o.y > -14600.0 && o.y < -13500.0 && o.z < -760.0 {
                        continue;
                    }
                    if n.stage > origin_stage || (n.body.origin.y > goal_y && n.body.origin.z > -730.0) {
                        return Some((t, n));
                    }
                    // progress plus the energy to make the jump at the end:
                    // height, and speed along the stage
                    let v = n.body.velocity;
                    let w = env("WY", 1.0);
                    let energy = n.body.origin.z + v.dot(v) / 1600.0;
                    let score = w * n.body.origin.y + energy + 0.3 * v.y;
                    next.push((score, n));
                }
            }
        }
        next.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        next.dedup_by(|a, b| (a.1.body.origin - b.1.body.origin).length() < 2.0);
        next.truncate(beam_width());
        if next.is_empty() {
            return None;
        }
        beam = next;
        if t % 64 == 0 {
            let b = &beam[0].1;
            println!("t {t}: best {:?} v {:?}", b.body.origin, b.body.velocity);
        }
    }
    None
}

#[test]
#[ignore]
fn stage_two_can_be_surfed() {
    let (m, c) = setup();
    let mut s = Surfer::at_start(&m);
    let (p, yaw) = m.destination_named("orange").unwrap();
    s.body.origin = p;
    s.phase = Phase::Running;
    s.stage = 1;
    s.last_cmd = Cmd::new(0, yaw, 0.0);
    s.body.velocity = surf_core::vec3::v3(0.0, env("VY", 0.0), -150.0);
    let r = search(&m, &c, &s, 64 * 12, -12900.0);
    println!(
        "result: {:?}",
        r.as_ref().map(|(t, s)| (t, s.body.origin, s.stage))
    );
    assert!(r.is_some());
}

/// From a surfer already on the ridge's left face (as after dropping onto it
/// diagonally from the platform), can the far platform be reached? Prints
/// the best exit; run with ASUB and UPF to compare rules.
#[test]
#[ignore]
fn ridge_to_platform() {
    use surf_core::vec3::v3;
    let (m, c) = setup();
    let mut mv = Mover::new(&m);
    if let Ok(n) = std::env::var("ASUB") {
        mv.t.air_substeps = n.parse().unwrap();
    }
    if let Ok(f) = std::env::var("UPF") {
        mv.t.up_surface_friction = f.parse().unwrap();
    }
    let vy0: f32 = std::env::var("VY")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or(350.0);
    let mut s = Surfer::at_start(&m);
    s.phase = Phase::Running;
    s.stage = 1;
    // above the left face, 150 units from the apex: the face is at about z -641
    s.body.origin = v3(-13462.0, -14520.0, -600.0);
    s.body.velocity = v3(0.0, vy0, 0.0);
    s.last_cmd = Cmd::new(0, 90.0, 0.0);
    let mut beam: Vec<(f32, Surfer)> = vec![(0.0, s)];
    let turns = [-16.0f32, -8.0, -4.0, -2.0, -1.0, 0.0, 1.0, 2.0, 4.0, 8.0, 16.0];
    let keys = [0, B_LEFT, B_RIGHT];
    let mut best_exit: Option<(f32, Surfer)> = None;
    for t in 0..64 * 6 {
        let mut next = Vec::new();
        for (_, s) in &beam {
            for dy in turns {
                for k in keys {
                    let mut n = s.clone();
                    let yaw = n.last_cmd.yaw_deg() + dy;
                    let tk = n.step(&mut mv, &c, Cmd::new(k, yaw, 0.0));
                    if tk.teleported {
                        if n.stage > 1 {
                            println!("REACHED the door at tick {t}");
                            return;
                        }
                        continue;
                    }
                    let o = n.body.origin;
                    if o.z < -760.0 || o.y < -14700.0 {
                        continue;
                    }
                    if o.y > -13000.0 && o.z > -725.0 {
                        println!(
                            "REACHED the far platform at tick {t}: {:?} v {:?}",
                            o, n.body.velocity
                        );
                        return;
                    }
                    let v = n.body.velocity;
                    let e = o.z + v.dot(v) / 1600.0;
                    let score = e + 0.2 * o.y + 0.2 * v.y;
                    if o.y > -13530.0 {
                        let better = best_exit.as_ref().map(|b| score > b.0).unwrap_or(true);
                        if better {
                            best_exit = Some((score, n.clone()));
                        }
                    }
                    next.push((score, n));
                }
            }
        }
        next.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        next.dedup_by(|a, b| (a.1.body.origin - b.1.body.origin).length() < 1.0);
        next.truncate(200);
        if next.is_empty() {
            break;
        }
        beam = next;
    }
    match best_exit {
        Some((_, s)) => println!(
            "best exit off the ridge: {:?} v {:?}",
            s.body.origin, s.body.velocity
        ),
        None => println!("never reached the ridge's end"),
    }
}

/// Stage 1 from the red door at walking speed, then stage 2 with whatever
/// speed the stage door carries over (Source keeps velocity on teleport).
#[test]
#[ignore]
fn stages_one_then_two() {
    let (m, c) = setup();
    let mut s = Surfer::at_start(&m);
    let (p, yaw) = m.destination_named("red").unwrap();
    s.body.origin = p;
    s.body.velocity = surf_core::vec3::v3(0.0, 250.0, 0.0);
    s.phase = Phase::Running;
    s.stage = 0;
    s.last_cmd = Cmd::new(0, yaw, 0.0);
    let Some((t1, s2)) = search(&m, &c, &s, 64 * 15, 1e9) else {
        println!("stage 1 not finished");
        panic!();
    };
    println!(
        "stage 1 done in {t1} ticks; arrive at stage 2 at {:?} with v {:?}",
        s2.body.origin, s2.body.velocity
    );
    let r = search(&m, &c, &s2, 64 * 12, 1e9);
    println!(
        "stage 2: {:?}",
        r.as_ref().map(|(t, s)| (t, s.body.velocity, s.stage))
    );
    assert!(r.is_some());
}

/// Source keeps a player's velocity through a trigger_teleport, so stage 2
/// starts with the speed stage 1's door was taken at (about 750 u/s from the
/// drop alone, more with strafing). With 1100 u/s carried over, the search
/// reaches the far platform. From a standing start it does not: that was the
/// game before the teleport kept velocity.
#[test]
fn stage_two_with_the_speed_stage_one_carries() {
    let (m, c) = setup();
    let (p, yaw) = m.destination_named("orange").unwrap();
    let start = |vy: f32| {
        let mut s = Surfer::at_start(&m);
        s.body.origin = p;
        s.body.velocity = surf_core::vec3::v3(0.0, vy, -150.0);
        s.phase = Phase::Running;
        s.stage = 1;
        s.last_cmd = Cmd::new(0, yaw, 0.0);
        s
    };
    assert!(
        search(&m, &c, &start(1100.0), 64 * 6, -12900.0).is_some(),
        "surfable with carried speed"
    );
}
