//! A headless runner: a bot that finishes the courses with real inputs only
//! (keys and view angles each tick, through the same `Surfer::step` a person
//! surfs with), and the runs it found, kept as fixtures and replayed here.
//!
//! The search is Go-Explore (Ecoffet et al., 2021): it keeps an archive of
//! the places it has reached (a cell is a 192-unit box, the stage, the name
//! the map gave you and a coarse speed), each with the quickest inputs that
//! reached it; it picks a cell, favouring the furthest stage and the cells it
//! has tried least, puts the surfer back there exactly (the simulation is
//! deterministic), and tries a few seconds of random surfing: a held key, a
//! steady turn, a jump now and then. A fall ends the try. The first finish
//! is kept as the run: the inputs from the spawn to the clock starting
//! ("lead"), and from there to the finish (a run record, as a ghost is sent).
//!
//! `cargo test --release -p surf_core --test runner -- --ignored find_runs`
//! searches every course again and rewrites `tests/runs/<course>.json`
//! (`RUNNER_COURSES=surf_utopia,surf_egypt` for some; `RUNNER_SECONDS` caps
//! each course's search, 600 by default). The plain test replays the kept runs:
//! a change to the movement or a map that stops one finishing fails it.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::{json, Value};
use surf_core::course::{Course, Role};
use surf_core::map::{box_touches, Map, Plane, TriggerKind};
use surf_core::movement::{Cmd, Mover, B_DUCK, B_FORWARD, B_JUMP, B_LEFT, B_RIGHT, HULL_MINS};
use surf_core::online::{start_of, RunRecord};
use surf_core::replay::{pack_cmds, unpack_cmds};
use surf_core::run::{Phase, Surfer};
use surf_core::trace::{trace_box, Scratch};
use surf_core::vec3::{v3, V3};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn runs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("runs")
}

fn maps_json() -> String {
    fs::read_to_string(root().join("data/maps/maps.json")).expect("maps.json")
}

fn course_ids() -> Vec<String> {
    let v: Value = serde_json::from_str(&maps_json()).unwrap();
    v["maps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap().to_string())
        .collect()
}

fn load(id: &str) -> (Map, Course) {
    let bytes = fs::read(root().join(format!("data/maps/{id}.bin"))).expect("map file");
    let mut m = Map::parse(&bytes).expect("parse map");
    let c = Course::parse(&maps_json(), id, &mut m).expect("course");
    (m, c)
}

// ---- the kept runs -------------------------------------------------------

/// A kept run: the inputs from the spawn until the clock starts, and the run.
struct Found {
    lead: Vec<Cmd>,
    run: RunRecord,
}

fn found_to_json(f: &Found) -> Value {
    let mut v = f.run.to_json();
    v["lead"] = json!(pack_cmds(Cmd::default(), &f.lead));
    v["lead_ticks"] = json!(f.lead.len());
    v
}

fn found_from_json(v: &Value) -> Found {
    let run = RunRecord::from_json(v).expect("a run record");
    let lead = unpack_cmds(Cmd::default(), v["lead"].as_str().expect("lead")).expect("lead inputs");
    assert_eq!(lead.len() as u64, v["lead_ticks"].as_u64().unwrap());
    Found { lead, run }
}

/// Replays a kept run from the spawn: the lead must start the clock exactly
/// where the run begins, and the run must finish on its last input.
fn replay(m: &Map, c: &Course, f: &Found) -> Result<u32, String> {
    let mut mv = Mover::new(m);
    let mut s = Surfer::at_start(m);
    let mut started = false;
    for (i, cmd) in f.lead.iter().enumerate() {
        let t = s.step(&mut mv, c, *cmd);
        if t.started {
            if i + 1 != f.lead.len() {
                return Err(format!("the clock started early, at lead tick {i}"));
            }
            started = true;
        }
    }
    if !started {
        return Err("the lead never starts the clock".into());
    }
    if s.body != f.run.start || s.stage != f.run.start_stage || s.name != f.run.start_name {
        return Err("the run does not begin where the lead left the surfer".into());
    }
    let mut g = start_of(&f.run);
    assert_eq!(g.phase, Phase::Running);
    for (i, cmd) in f.run.cmds.iter().enumerate() {
        let t = g.step(&mut mv, c, *cmd);
        if let Some((ending, ticks)) = t.finished {
            if i + 1 != f.run.cmds.len() {
                return Err(format!("finished early, at tick {i}"));
            }
            if ending != f.run.ending || ticks != f.run.ticks {
                return Err(format!(
                    "finished by ending {ending} in {ticks} ticks, not as kept"
                ));
            }
            return Ok(ticks);
        }
        if t.fell && c.stages.is_empty() {
            return Err(format!("fell at tick {i} ({:?})", g.body.origin));
        }
    }
    Err(format!(
        "never finished; ended at {:?}, stage {}",
        g.body.origin,
        g.stage + 1
    ))
}

#[test]
fn every_kept_run_still_finishes_from_the_spawn() {
    let ids = course_ids();
    let mut checked = Vec::new();
    let mut problems = Vec::new();
    for id in &ids {
        let path = runs_dir().join(format!("{id}.json"));
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let v: Value = serde_json::from_str(&text).expect("run json");
        let f = found_from_json(&v);
        assert_eq!(f.run.map, *id);
        let (m, c) = load(id);
        match replay(&m, &c, &f) {
            Ok(t) => checked.push(format!("{id} {:.2}s", t as f32 / 64.0)),
            Err(e) => problems.push(format!("{id}: {e}")),
        }
    }
    println!("replayed: {}", checked.join(", "));
    assert!(
        problems.is_empty(),
        "kept runs that no longer finish:\n{}",
        problems.join("\n")
    );
    assert!(
        !checked.is_empty(),
        "no kept runs found in {}",
        runs_dir().display()
    );
}

// ---- which way is forward ------------------------------------------------

/// How far (in grid steps through open space) every place is from a goal:
/// a breadth-first search over a grid of the map from the goal's triggers,
/// stepping only where a small box passes and never into a trigger that
/// would end the try. One per stage: its door to the next, or the finish.
struct Field {
    lo: V3,
    g: f32,
    n: [usize; 3],
    dist: Vec<u16>,
}

const FAR: u16 = u16::MAX;

impl Field {
    fn index(&self, p: V3) -> Option<usize> {
        let f = |v: f32, lo: f32, n: usize| {
            let i = ((v - lo) / self.g).floor();
            if i < 0.0 || i >= n as f32 {
                None
            } else {
                Some(i as usize)
            }
        };
        let (x, y, z) = (
            f(p.x, self.lo.x, self.n[0])?,
            f(p.y, self.lo.y, self.n[1])?,
            f(p.z, self.lo.z, self.n[2])?,
        );
        Some((z * self.n[1] + y) * self.n[0] + x)
    }
    /// The way forward from here (a yaw in degrees), down the field.
    fn heading(&self, p: V3) -> Option<f32> {
        let here = self.at(p);
        if here == FAR {
            return None;
        }
        let mut best: Option<(u16, f32)> = None;
        for k in 0..16 {
            let a = k as f32 * 22.5;
            let r = a.to_radians();
            for reach in [1.0f32, 2.0, 3.0] {
                let q = p + v3(r.cos(), r.sin(), 0.0) * (self.g * reach);
                let d = self.at(q);
                if d < here && best.map(|b| d < b.0).unwrap_or(true) {
                    best = Some((d, a));
                }
            }
        }
        best.map(|b| b.1)
    }

    fn at(&self, p: V3) -> u16 {
        // the middle of the body, not the feet
        self.index(p + v3(0.0, 0.0, 36.0))
            .map(|i| self.dist[i])
            .unwrap_or(FAR)
    }
}

/// The open grid: per cell, bit 0 the cell is open, bits 1..3 the step to
/// +x, +y, +z is open, bit 4 there is ground (or a ramp) close below.
struct Grid {
    lo: V3,
    g: f32,
    n: [usize; 3],
    open: Vec<u8>,
}

fn grid(m: &Map) -> Grid {
    let size = m.maxs - m.mins;
    let mut g = 48.0f32;
    while (size.x / g).ceil() * (size.y / g).ceil() * (size.z / g).ceil() > 2.4e7 {
        g += 8.0;
    }
    let n = [
        (size.x / g).ceil() as usize,
        (size.y / g).ceil() as usize,
        (size.z / g).ceil() as usize,
    ];
    let lo = m.mins;
    let b = v3(4.0, 4.0, 4.0);
    let layers: Vec<Vec<u8>> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..n[2])
            .map(|z| {
                sc.spawn(move || {
                    let mut scratch = Scratch::default();
                    let mut out = vec![0u8; n[0] * n[1]];
                    let c = |x: usize, y: usize, z: usize| {
                        lo + v3((x as f32 + 0.5) * g, (y as f32 + 0.5) * g, (z as f32 + 0.5) * g)
                    };
                    // a passage narrower than a cell can run between the
                    // centres: try offsets a third of a cell across the step
                    let third = g / 3.0;
                    let offs = |axis: usize| -> Vec<V3> {
                        let mut v = vec![v3(0.0, 0.0, 0.0)];
                        for a in [-1.0f32, 0.0, 1.0] {
                            for bb in [-1.0f32, 0.0, 1.0] {
                                if a == 0.0 && bb == 0.0 {
                                    continue;
                                }
                                v.push(match axis {
                                    0 => v3(0.0, a * third, bb * third),
                                    1 => v3(a * third, 0.0, bb * third),
                                    _ => v3(a * third, bb * third, 0.0),
                                });
                            }
                        }
                        v
                    };
                    let per_axis = [offs(0), offs(1), offs(2)];
                    for y in 0..n[1] {
                        for x in 0..n[0] {
                            let p = c(x, y, z);
                            let mut bits = 0u8;
                            for (k, q) in [(1u8, c(x + 1, y, z)), (2, c(x, y + 1, z)), (3, c(x, y, z + 1))] {
                                for o in &per_axis[k as usize - 1] {
                                    let t = trace_box(m, &mut scratch, p + *o, q + *o, -b, b);
                                    if !t.start_solid && t.fraction >= 1.0 {
                                        bits |= 1 << k;
                                        break;
                                    }
                                }
                            }
                            // open if any of its points is
                            let free = bits != 0 || {
                                let t = trace_box(m, &mut scratch, p, p, -b, b);
                                !t.start_solid
                            };
                            if free {
                                bits |= 1;
                                // something to stand or surf on near below:
                                // only from here can you go up
                                let t = trace_box(m, &mut scratch, p, p - v3(0.0, 0.0, g * 2.2), -b, b);
                                if t.hit() {
                                    bits |= 16;
                                }
                            }
                            out[y * n[0] + x] = bits;
                        }
                    }
                    out
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    Grid {
        lo,
        g,
        n,
        open: layers.concat(),
    }
}

/// The goals of each stage, and the triggers that end a try on it.
fn fields(m: &Map, c: &Course, gr: &Grid) -> Vec<Field> {
    // one field, to the finish: a stage's door is a teleport, and so a
    // portal to the next stage's start (the per-stage doors are not always
    // doors: surf_deadline's stages run into each other)
    let stages = 1;
    (0..stages)
        .map(|k| {
            let last = k + 1 >= stages;
            let mut goals: Vec<(V3, V3)> = Vec::new();
            let mut bad: Vec<(V3, V3, &[Plane])> = Vec::new();
            // a teleport that keeps the run: its trigger is as near the goal
            // as where it sends you
            let mut portals: Vec<(V3, V3, V3, &[Plane])> = Vec::new();
            for t in &m.triggers {
                match t.kind {
                    TriggerKind::Teleport(d) => match c.role(t, d) {
                        Role::Finish(_) if last => goals.push((t.mins, t.maxs)),
                        Role::Move if !last && c.stage_of(d) == Some(k as u8 + 1) => goals.push((t.mins, t.maxs)),
                        Role::Move => {
                            if let Some((to, _)) = m.destination(d) {
                                portals.push((to, t.mins, t.maxs, &t.planes[..]));
                            }
                        }
                        Role::Restart | Role::Exit(_) => bad.push((t.mins, t.maxs, &t.planes[..])),
                        _ => {}
                    },
                    TriggerKind::Hurt(_) => bad.push((t.mins, t.maxs, &t.planes[..])),
                    _ => {}
                }
            }
            if last {
                if let Some(z) = &c.finish_zone {
                    goals.push((z.mins, z.maxs));
                }
            }
            let f = bfs(gr, &goals, &bad, &portals);
            if std::env::var("RUNNER_DEBUG").is_ok() {
                let open = gr.open.iter().filter(|b| **b & 1 != 0).count();
                let edges = gr.open.iter().filter(|b| **b & 14 != 0).count();
                let reached = f.dist.iter().filter(|d| **d != FAR).count();
                eprintln!(
                    "{} stage {k}: grid {:?} at {}, {open} open, {edges} with an open step, {} goals, {} bad, {reached} reached, spawn {}",
                    c.id,
                    gr.n, gr.g, goals.len(), bad.len(), f.at(m.spawn)
                );
                eprintln!("{}  spawn {:?}, map {:?}..{:?}", c.id, m.spawn, m.mins, m.maxs);
                for g in &goals {
                    eprintln!("{}  goal {:?}..{:?}", c.id, g.0, g.1);
                }
                for t in &m.triggers {
                    if let TriggerKind::Teleport(d) = t.kind {
                        if goals.iter().any(|g| g.0 == t.mins && g.1 == t.maxs) {
                            eprintln!("{}  goal trigger filter {} to {}", c.id, t.filter, m.name(d));
                        }
                    }
                }
                // the way the field leads from the spawn
                let mut q = m.spawn;
                let mut line = Vec::new();
                for step in 0..400 {
                    let d = f.at(q);
                    if d == 0 || d == FAR {
                        break;
                    }
                    let mut best = (d, q);
                    for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
                        let r = q + v3(dx as f32, dy as f32, dz as f32) * gr.g;
                        let e = f.at(r);
                        if e < best.0 {
                            best = (e, r);
                        }
                    }
                    q = best.1;
                    if step % 4 == 0 {
                        line.push(format!("({:.0},{:.0},{:.0})", q.x, q.y, q.z));
                    }
                }
                eprintln!("{}  path {}", c.id, line.join(" "));
                for (to, lo, hi, _) in &portals {
                    eprintln!("{}  portal {:?}..{:?} -> {:?} field there {}", c.id, lo, hi, to, f.at(*to));
                }
                let nob = bfs(gr, &goals, &[], &portals);
                eprintln!("{}  without bad: spawn {}, reached {}", c.id, nob.at(m.spawn), nob.dist.iter().filter(|d| **d != FAR).count());
            }
            f
        })
        .collect()
}

type Portal<'a> = (V3, V3, V3, &'a [Plane]);

fn bfs(gr: &Grid, goals: &[(V3, V3)], bad: &[(V3, V3, &[Plane])], portals: &[Portal]) -> Field {
    let n = gr.n;
    let total = n[0] * n[1] * n[2];
    let mut dist = vec![FAR; total];
    let mut blocked = vec![false; total];
    let range = |lo: V3, hi: V3| {
        let r = |a: f32, b: f32, o: f32, k: usize| {
            let i0 = ((a - o) / gr.g).floor().max(0.0) as usize;
            let i1 = (((b - o) / gr.g).floor().max(-1.0) as isize).min(k as isize - 1);
            (i0, i1)
        };
        (
            r(lo.x, hi.x, gr.lo.x, n[0]),
            r(lo.y, hi.y, gr.lo.y, n[1]),
            r(lo.z, hi.z, gr.lo.z, n[2]),
        )
    };
    let each = |lo: V3, hi: V3, f: &mut dyn FnMut(usize)| {
        let ((x0, x1), (y0, y1), (z0, z1)) = range(lo, hi);
        for z in z0 as isize..=z1 {
            for y in y0 as isize..=y1 {
                for x in x0 as isize..=x1 {
                    f((z as usize * n[1] + y as usize) * n[0] + x as usize);
                }
            }
        }
    };
    let centre = |i: usize| {
        let x = i % n[0];
        let y = (i / n[0]) % n[1];
        let z = i / (n[0] * n[1]);
        gr.lo
            + v3(
                (x as f32 + 0.5) * gr.g,
                (y as f32 + 0.5) * gr.g,
                (z as f32 + 0.5) * gr.g,
            )
    };
    for (lo, hi, planes) in bad {
        let b = v3(4.0, 4.0, 4.0);
        each(*lo, *hi, &mut |i| {
            let c = centre(i);
            if box_touches(planes, c - b, c + b) {
                blocked[i] = true;
            }
        });
    }
    // destination cell -> the cells of the triggers that send you there
    let mut sends: HashMap<usize, Vec<usize>> = HashMap::new();
    let probe = Field {
        lo: gr.lo,
        g: gr.g,
        n,
        dist: Vec::new(),
    };
    for (to, lo, hi, planes) in portals {
        let Some(at) = probe.index(*to + v3(0.0, 0.0, 36.0)) else {
            continue;
        };
        let h = gr.g * 0.5;
        let mut cells = Vec::new();
        each(*lo, *hi, &mut |i| {
            let c = centre(i);
            if box_touches(planes, c - v3(h, h, h), c + v3(h, h, h)) {
                cells.push(i);
            }
        });
        sends.entry(at).or_default().extend(cells);
    }
    let mut queue = std::collections::VecDeque::new();
    // a goal thinner than a cell (a door in a wall) marks every cell it
    // touches, and opens onto their open neighbours; a big one (a finish
    // room) marks only its open cells, so it cannot leak through a wall
    let mut portal_goal = vec![false; total];
    for (lo, hi) in goals {
        let e = *hi - *lo;
        let thin = e.x.min(e.y).min(e.z) < gr.g * 1.5;
        each(*lo, *hi, &mut |i| {
            let c = centre(i);
            let inside =
                c.x >= lo.x && c.x <= hi.x && c.y >= lo.y && c.y <= hi.y && c.z >= lo.z && c.z <= hi.z;
            if thin || (inside && gr.open[i] & 1 != 0) {
                if dist[i] != 0 {
                    dist[i] = 0;
                    queue.push_back(i);
                }
                portal_goal[i] = thin;
            }
        });
    }
    while let Some(i) = queue.pop_front() {
        let d = dist[i];
        if let Some(from) = sends.get(&i) {
            for &j in from {
                if dist[j] == FAR && !blocked[j] {
                    dist[j] = d.saturating_add(1);
                    queue.push_back(j);
                }
            }
        }
        let x = i % n[0];
        let y = (i / n[0]) % n[1];
        let z = i / (n[0] * n[1]);
        let sx = 1;
        let sy = n[0];
        let sz = n[0] * n[1];
        // (neighbour, the step is open): an open step is marked on the lower cell
        let mut steps: [(usize, bool); 6] = [(usize::MAX, false); 6];
        if x + 1 < n[0] {
            steps[0] = (i + sx, gr.open[i] & 2 != 0);
        }
        if x > 0 {
            steps[1] = (i - sx, gr.open[i - sx] & 2 != 0);
        }
        if y + 1 < n[1] {
            steps[2] = (i + sy, gr.open[i] & 4 != 0);
        }
        if y > 0 {
            steps[3] = (i - sy, gr.open[i - sy] & 4 != 0);
        }
        // (the search runs backwards from the goal: a step to the cell
        // below is a climb from it, which needs ground close under it)
        if z + 1 < n[2] {
            steps[4] = (i + sz, gr.open[i] & 8 != 0);
        }
        if z > 0 {
            steps[5] = (i - sz, gr.open[i - sz] & 8 != 0 && gr.open[i - sz] & 16 != 0);
        }
        // a goal (a door, often in a wall) opens onto every open neighbour
        let portal = d == 0 && portal_goal[i];
        for (j, ok) in steps {
            if j == usize::MAX {
                continue;
            }
            let ok = ok || (portal && gr.open[j] & 1 != 0);
            if ok && dist[j] == FAR && !blocked[j] {
                dist[j] = d.saturating_add(1);
                queue.push_back(j);
            }
        }
    }
    Field {
        lo: gr.lo,
        g: gr.g,
        n,
        dist,
    }
}

// ---- the search ------------------------------------------------------------

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// One step of the path: a parent and the inputs from it.
struct Node {
    parent: usize,
    cmds: Vec<Cmd>,
}
const ROOT: usize = usize::MAX;

#[derive(Clone)]
struct Cell {
    node: usize,
    surfer: Surfer,
    /// Ticks from the spawn.
    ticks: u32,
    /// Ticks from the clock starting (None before it starts).
    start_at: Option<u32>,
    chosen: u32,
    /// Tries from here that reached nothing new: a dead end, near a pit.
    fails: u32,
    /// How far along: the stage, then how close to its goal.
    progress: f32,
}

type Key = (u8, u16, i32, i32, i32, u8, bool);

fn key(s: &Surfer) -> Key {
    let o = s.body.origin;
    let sp = s.body.velocity.length2d();
    let speed = if sp < 450.0 {
        0
    } else if sp < 900.0 {
        1
    } else if sp < 1500.0 {
        2
    } else {
        3
    };
    (
        s.stage,
        s.name,
        (o.x / 192.0).floor() as i32,
        (o.y / 192.0).floor() as i32,
        (o.z / 128.0).floor() as i32,
        speed,
        s.phase == Phase::Running,
    )
}

/// The surf ramp the surfer is on, if any: the normal of a steep plane the
/// body's box meets within a few units.
fn ramp_under(m: &Map, scratch: &mut Scratch, s: &Surfer) -> Option<V3> {
    let o = s.body.origin;
    let maxs = s.body.maxs();
    for k in 0..9 {
        let dir = if k == 0 {
            v3(0.0, 0.0, -1.0)
        } else {
            let a = (k as f32 * 45.0).to_radians();
            v3(a.cos(), a.sin(), -0.6)
        };
        let t = trace_box(m, scratch, o, o + dir * 6.0, HULL_MINS, maxs);
        if t.hit() && !t.start_solid && t.normal.z > 0.05 && t.normal.z < 0.7 {
            return Some(t.normal);
        }
    }
    None
}

/// A coarse place (four cells a side), for counting dead ends.
fn region(k: &Key) -> (u8, i32, i32, i32) {
    (k.0, k.2.div_euclid(4), k.3.div_euclid(4), k.4.div_euclid(6))
}

fn path_of(nodes: &[Node], mut n: usize) -> Vec<Cmd> {
    let mut parts = Vec::new();
    while n != ROOT {
        parts.push(n);
        n = nodes[n].parent;
    }
    parts
        .iter()
        .rev()
        .flat_map(|&i| nodes[i].cmds.iter().copied())
        .collect()
}

/// What the search reports for a course.
struct Outcome {
    found: Option<Found>,
    cells: usize,
    ticks_simulated: u64,
    best_stage: u8,
    seconds: f32,
    /// The closest it came to the furthest stage's goal, in grid steps.
    closest: f32,
}

/// Searches one course for a finished run, for at most `budget` seconds.
fn search(id: &str, seed: u64, budget: f32) -> Outcome {
    let (m, c) = load(id);
    let mut mv = Mover::new(&m);
    let mut rng = Rng(seed | 1);
    let gr = grid(&m);
    let fs = fields(&m, &c, &gr);
    drop(gr);
    let progress = |s: &Surfer| {
        let f = &fs[(s.stage as usize).min(fs.len() - 1)];
        let d = f.at(s.body.origin);
        if d == FAR {
            0.0
        } else {
            70_000.0 - d as f32
        }
    };
    let mut nodes: Vec<Node> = Vec::new();
    let mut archive: HashMap<Key, Cell> = HashMap::new();
    let first = Surfer::at_start(&m);
    archive.insert(
        key(&first),
        Cell {
            node: ROOT,
            surfer: first.clone(),
            ticks: 0,
            start_at: None,
            chosen: 0,
            fails: 0,
            progress: progress(&first),
        },
    );
    if std::env::var("RUNNER_DEBUG").is_ok() {
        let mut w = Surfer::at_start(&m);
        let yaw = m.spawn_yaw;
        for i in 0..200 {
            let t = w.step(&mut mv, &c, Cmd::new(B_FORWARD, yaw, 0.0));
            if i < 3 || t.teleported || t.started || t.fell || t.stage.is_some() || i % 50 == 0 {
                eprintln!("{id} walk {i}: {:?} phase {:?} {:?}", w.body.origin, w.phase, t);
            }
        }
    }
    let clock = Instant::now();
    let mut simulated = 0u64;
    let mut best_stage = 0u8;
    let mut keys: Vec<Key> = archive.keys().copied().collect();
    let mut iter = 0u64;
    let mut best = f32::MIN;
    let mut region_fails: HashMap<(u8, i32, i32, i32), u32> = HashMap::new();
    while clock.elapsed().as_secs_f32() < budget {
        iter += 1;
        if iter % 128 == 1 {
            // the most promising first: furthest along, least tried
            let mut ranked: Vec<(f32, Key)> = archive
                .iter()
                .map(|(k, c)| {
                    let rf = region_fails.get(&region(k)).copied().unwrap_or(0) as f32;
                    let speed = c.surfer.body.velocity.length2d();
                    (
                        c.progress - 4.0 * (c.chosen as f32).sqrt() - 25.0 * c.fails as f32 - 2.0 * rf
                            + speed / 100.0,
                        *k,
                    )
                })
                .collect();
            ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
            keys = ranked.into_iter().map(|(_, k)| k).collect();
        }
        if iter.is_multiple_of(20000) && std::env::var("RUNNER_DEBUG").is_ok() {
            let (k, c) = archive
                .iter()
                .max_by(|a, b| a.1.progress.total_cmp(&b.1.progress))
                .unwrap();
            eprintln!(
                "{id} {:.0}s: best {:.0} at {:?} v {:?} ground {} ticks {} chosen {} fails {} key {:?}",
                clock.elapsed().as_secs_f32(),
                c.progress,
                c.surfer.body.origin,
                c.surfer.body.velocity,
                c.surfer.body.on_ground,
                c.ticks,
                c.chosen,
                c.fails,
                k
            );
        }
        let u = rng.unit();
        let pick = keys[((u * u * u) * keys.len() as f32) as usize % keys.len()];
        let cell = archive.get_mut(&pick).unwrap();
        cell.chosen += 1;
        let mut s = cell.surfer.clone();
        let mut node = cell.node;
        let mut ticks = cell.ticks;
        let mut start_at = cell.start_at;
        let mut since: Vec<Cmd> = Vec::new();
        let mut yaw = s.last_cmd.yaw_deg();
        let mut gained = false;
        // a try: up to five seconds of random surfing in held actions
        let mut left = 64 + rng.below(256) as u32;
        let mut hold = 0u32;
        let mut buttons = 0u8;
        let mut turn = 0.0f32;
        let mut jump_every = 0u32;
        let mut aim: Option<f32> = None;
        let mut surf: Option<f32> = None;
        let mut scratch = Scratch::default();
        while left > 0 {
            left -= 1;
            if hold == 0 {
                hold = 6 + rng.below(40) as u32;
                buttons = match rng.below(9) {
                    0 => 0,
                    1 => B_FORWARD,
                    2 | 3 => B_LEFT,
                    4 | 5 => B_RIGHT,
                    6 => B_FORWARD | B_LEFT,
                    7 => B_FORWARD | B_RIGHT,
                    _ => B_FORWARD,
                };
                aim = if rng.below(3) == 0 {
                    Some((rng.unit() - 0.5) * 120.0)
                } else {
                    None
                };
                // on a ramp, often surf it: along the ramp, the strafe key
                // toward the side it rises on, the view a little into it
                surf = if rng.below(2) == 0 {
                    Some((rng.unit() - 0.3) * 25.0)
                } else {
                    None
                };
                // a strafe key is turned into, as a surfer turns the mouse
                let sign = if buttons & B_LEFT != 0 {
                    1.0
                } else if buttons & B_RIGHT != 0 {
                    -1.0
                } else if rng.below(2) == 0 {
                    1.0
                } else {
                    -1.0
                };
                turn = sign * rng.unit() * rng.unit() * 6.0;
                if rng.below(5) == 0 {
                    // a sudden look somewhere else
                    yaw += (rng.unit() - 0.5) * 180.0;
                }
                jump_every = match rng.below(4) {
                    0 => 1,
                    1 => 0,
                    _ => 0,
                };
                if rng.below(12) == 0 {
                    buttons |= B_DUCK;
                }
            }
            hold -= 1;
            yaw += turn;
            let mut surfing = false;
            if let Some(into) = surf {
                if let Some(n) = ramp_under(&m, &mut scratch, &s) {
                    let h = v3(n.x, n.y, 0.0);
                    let hl = h.length();
                    if hl > 0.01 {
                        let h = h * (1.0 / hl);
                        // the two ways along the ramp; the field picks one
                        let along = [v3(-h.y, h.x, 0.0), v3(h.y, -h.x, 0.0)];
                        let f = &fs[(s.stage as usize).min(fs.len() - 1)];
                        let ahead = |a: V3| f.at(s.body.origin + a * (f.g * 2.0));
                        let mut dir = if ahead(along[0]) <= ahead(along[1]) {
                            along[0]
                        } else {
                            along[1]
                        };
                        // keep going the way you are already going, if fast
                        let v = s.body.velocity;
                        if v.length2d() > 400.0 && (v.x * dir.x + v.y * dir.y) < 0.0 {
                            dir = dir * -1.0;
                        }
                        let base = dir.y.atan2(dir.x).to_degrees();
                        // the ramp rises on the side away from its normal
                        let left = v3(-dir.y, dir.x, 0.0);
                        let rises_left = -(h.x * left.x + h.y * left.y) > 0.0;
                        let key = if rises_left { B_LEFT } else { B_RIGHT };
                        let into_deg = if rises_left { into } else { -into };
                        yaw = base + into_deg;
                        buttons = key;
                        surfing = true;
                    }
                }
            }
            if let (Some(off), false) = (aim, surfing) {
                // look down the way forward, a little to one side
                let f = &fs[(s.stage as usize).min(fs.len() - 1)];
                if let Some(h) = f.heading(s.body.origin) {
                    yaw = h + off;
                }
            }
            let mut b = buttons;
            if jump_every == 1 {
                b |= B_JUMP;
            }
            let cmd = Cmd::new(b, yaw, 0.0);
            let t = s.step(&mut mv, &c, cmd);
            simulated += 1;
            since.push(cmd);
            ticks += 1;
            if t.started {
                start_at = Some(ticks);
            }
            if let Some((ending, run_ticks)) = t.finished {
                // the run: the path to here, split where the clock started
                let mut all = path_of(&nodes, node);
                all.extend(since.iter().copied());
                let at = start_at.expect("finished with a clock") as usize;
                let lead = all[..at].to_vec();
                let cmds = all[at..].to_vec();
                assert_eq!(cmds.len() as u32, run_ticks);
                // the surfer as the clock started, for the record
                let mut r = Surfer::at_start(&m);
                for cmd in &lead {
                    r.step(&mut mv, &c, *cmd);
                }
                let run = RunRecord {
                    map: id.to_string(),
                    name: "Runner".into(),
                    seed: 0,
                    ticks: run_ticks,
                    ending,
                    start: r.body,
                    start_name: r.name,
                    start_stage: r.stage,
                    prev: r.last_cmd,
                    cmds,
                    sync: 0.0,
                    strafes: 0,
                };
                return Outcome {
                    found: Some(Found { lead, run }),
                    cells: archive.len(),
                    ticks_simulated: simulated,
                    best_stage,
                    seconds: clock.elapsed().as_secs_f32(),
                    closest: 0.0,
                };
            }
            if t.fell || t.exited.is_some() {
                break;
            }
            if t.teleported {
                // a teleport that keeps the run turns the view as it lands
                if let Some(y) = s.snap_yaw.take() {
                    yaw = y;
                }
            }
            if t.stage.is_some() {
                // a stage door (or the clock starting at stage 1) turns the view
                if let Some(y) = s.snap_yaw.take() {
                    yaw = y;
                }
            }
            if s.phase == Phase::Running && start_at.is_none() {
                break;
            }
            best_stage = best_stage.max(s.stage);
            let k = key(&s);
            let better = archive.get(&k).map(|old| ticks < old.ticks).unwrap_or(true);
            if better {
                nodes.push(Node {
                    parent: node,
                    cmds: std::mem::take(&mut since),
                });
                node = nodes.len() - 1;
                let chosen = archive.get(&k).map(|o| o.chosen).unwrap_or(0);
                let p = progress(&s);
                best = best.max(p);
                let fails = archive.get(&k).map(|o| o.fails).unwrap_or(0);
                archive.insert(
                    k,
                    Cell {
                        node,
                        surfer: s.clone(),
                        ticks,
                        start_at,
                        chosen,
                        fails,
                        progress: p,
                    },
                );
                gained = true;
            }
        }
        if !gained {
            if let Some(c) = archive.get_mut(&pick) {
                c.fails += 1;
            }
            *region_fails.entry(region(&pick)).or_insert(0) += 1;
        }
    }
    Outcome {
        found: None,
        cells: archive.len(),
        ticks_simulated: simulated,
        best_stage,
        seconds: clock.elapsed().as_secs_f32(),
        closest: 70_000.0 - best,
    }
}

#[test]
#[ignore]
fn find_runs() {
    let wanted: Option<Vec<String>> = std::env::var("RUNNER_COURSES")
        .ok()
        .map(|s| s.split(',').map(|x| x.to_string()).collect());
    let budget: f32 = std::env::var("RUNNER_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(600.0);
    let ids: Vec<String> = course_ids()
        .into_iter()
        .filter(|id| wanted.as_ref().map(|w| w.contains(id)).unwrap_or(true))
        .collect();
    fs::create_dir_all(runs_dir()).unwrap();
    let handles: Vec<_> = ids
        .into_iter()
        .map(|id| {
            std::thread::spawn(move || {
                let o = search(&id, 0x5eed_0000 + id.len() as u64 * 7919, budget);
                let line = match &o.found {
                    Some(f) => {
                        let (m, c) = load(&id);
                        let checked = replay(&m, &c, f);
                        assert!(checked.is_ok(), "{id}: the found run replays: {checked:?}");
                        let text = serde_json::to_string(&found_to_json(f)).unwrap();
                        fs::write(runs_dir().join(format!("{id}.json")), text + "\n").unwrap();
                        format!("{id}: FINISHED in {:.2}s", f.run.ticks as f32 / 64.0)
                    }
                    None => format!(
                        "{id}: no finish (furthest stage {}, {} steps from its goal)",
                        o.best_stage + 1,
                        o.closest
                    ),
                };
                println!(
                    "{line} [searched {:.0}s, {} cells, {} ticks simulated]",
                    o.seconds, o.cells, o.ticks_simulated
                );
                line
            })
        })
        .collect();
    for h in handles {
        let _ = h.join();
    }
}

/// What is around a place: ramps and floors, and triggers (for tuning the search).
#[test]
#[ignore]
fn look_around() {
    let id = std::env::var("LOOK_COURSE").unwrap_or("surf_utopia".into());
    let at: Vec<f32> = std::env::var("LOOK_AT")
        .unwrap()
        .split(',')
        .map(|x| x.parse().unwrap())
        .collect();
    let r: f32 = std::env::var("LOOK_R")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or(1500.0);
    let p = v3(at[0], at[1], at[2]);
    let (m, c) = load(&id);
    for b in &m.brushes {
        if !b.kind.blocks()
            || b.maxs.x < p.x - r
            || b.mins.x > p.x + r
            || b.maxs.y < p.y - r
            || b.mins.y > p.y + r
            || b.maxs.z < p.z - r
            || b.mins.z > p.z + r
        {
            continue;
        }
        let ups: Vec<String> = b
            .sides
            .iter()
            .filter(|s| !s.bevel && s.plane.n.z > 0.05)
            .map(|s| format!("({:.2},{:.2},{:.2})", s.plane.n.x, s.plane.n.y, s.plane.n.z))
            .collect();
        eprintln!("brush {:?}..{:?} up faces {}", b.mins, b.maxs, ups.join(" "));
    }
    for t in &m.triggers {
        if t.maxs.x < p.x - r
            || t.mins.x > p.x + r
            || t.maxs.y < p.y - r
            || t.mins.y > p.y + r
            || t.maxs.z < p.z - r
            || t.mins.z > p.z + r
        {
            continue;
        }
        let what = match t.kind {
            TriggerKind::Teleport(d) => format!("teleport {:?} to {}", c.role(t, d), m.name(d)),
            k => format!("{k:?}"),
        };
        eprintln!("trigger {:?}..{:?} {what}", t.mins, t.maxs);
        if std::env::var("LOOK_PLANES").is_ok() {
            for pl in &t.planes {
                eprintln!(
                    "    plane n ({:.2},{:.2},{:.2}) d {:.1}",
                    pl.n.x, pl.n.y, pl.n.z, pl.d
                );
            }
        }
    }
}

/// A kept run, tick by tick: where it went, and when it ducked or jumped.
#[test]
#[ignore]
fn describe_run() {
    let id = std::env::var("LOOK_COURSE").unwrap_or("bhop_eazy".into());
    let every: usize = std::env::var("LOOK_EVERY")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or(64);
    let v: Value =
        serde_json::from_str(&fs::read_to_string(runs_dir().join(format!("{id}.json"))).unwrap()).unwrap();
    let f = found_from_json(&v);
    let (m, c) = load(&id);
    let mut mv = Mover::new(&m);
    let mut g = start_of(&f.run);
    let n = f.run.cmds.len();
    let ducks = f.run.cmds.iter().filter(|c| c.has(B_DUCK)).count();
    eprintln!(
        "{id}: {} lead ticks, {n} run ticks, {ducks} ticks ducking",
        f.lead.len()
    );
    for (i, cmd) in f.run.cmds.iter().enumerate() {
        let t = g.step(&mut mv, &c, *cmd);
        if i % every == 0 || t.teleported || i + 1 == n || (cmd.has(B_DUCK) && i + 400 > n) {
            eprintln!(
                "{i:5} ({:.0},{:.0},{:.0}) v ({:.0},{:.0},{:.0}) ground {} ducked {} keys {:#04x} {}",
                g.body.origin.x,
                g.body.origin.y,
                g.body.origin.z,
                g.body.velocity.x,
                g.body.velocity.y,
                g.body.velocity.z,
                g.body.on_ground,
                g.body.ducked,
                cmd.buttons,
                if t.teleported { "TELEPORT" } else { "" }
            );
        }
    }
}

/// The human utopia run kept for the browser check, with the field along it.
#[test]
#[ignore]
fn trace_the_human_utopia_run() {
    let v: Value = serde_json::from_str(
        &fs::read_to_string(root().join("tests/browser/fixtures/saved_best.json")).unwrap(),
    )
    .unwrap();
    let run = RunRecord::from_json(&v["best"]).expect("run");
    let (m, c) = load("surf_utopia");
    let gr = grid(&m);
    let fs = fields(&m, &c, &gr);
    let mut mv = Mover::new(&m);
    let mut g = start_of(&run);
    for (i, cmd) in run.cmds.iter().enumerate() {
        let t = g.step(&mut mv, &c, *cmd);
        if i % 64 == 0 || t.finished.is_some() {
            eprintln!(
                "{i:5} {:?} v {:?} ground {} keys {} yaw {:.0} field {}",
                g.body.origin,
                g.body.velocity,
                g.body.on_ground,
                cmd.buttons,
                cmd.yaw_deg(),
                fs[0].at(g.body.origin)
            );
        }
    }
}
