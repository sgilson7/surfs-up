//! Your times in a file: saved, edited, damaged and loaded back (backup.rs).

use super::*;
use crate::backup;

const MAPS: &str = include_str!("../../../data/maps/maps.json");
const UTOPIA: &[u8] = include_bytes!("../../../data/maps/surf_utopia.bin");
const KITSUNE: &[u8] = include_bytes!("../../../data/maps/surf_kitsune.bin");

/// The courses, but utopia's first foam pit finishes a run instead of
/// sending you back: a whole run a test can surf with a few keys, recorded
/// by the game itself and replayed by the same rules.
fn maps() -> String {
    let mut v: Value = serde_json::from_str(MAPS).unwrap();
    for r in v["maps"][0]["roles"].as_array_mut().unwrap() {
        if r["role"] == "restart" {
            r["role"] = json!("finish");
            r["ending"] = json!(5);
        }
    }
    v.to_string()
}

/// A game that has surfed a whole run: off the ledge, sliding into the pit.
fn finished() -> Game {
    let mut g = Game::new(&maps(), "").unwrap();
    g.load_map("surf_utopia", UTOPIA).unwrap();
    g.set_name("Hana");
    g.play();
    let mut t = 0.0;
    let mut off = 0;
    for _ in 0..4000u32 {
        t += 1000.0 / 64.0;
        // walk off the ledge (the clock starts), step off the ridge it
        // would balance on, then let go and slide into the pit
        if g.recording.is_some() {
            off += 1;
        }
        let keys = match off {
            0..10 => B_FORWARD,
            10..40 => B_RIGHT,
            _ => 0,
        } as u32;
        g.advance(t, keys, 0.0, 0.0);
        if !g.bests.is_empty() {
            return g;
        }
    }
    panic!("the slide into the pit finishes");
}

fn real_run() -> RunRecord {
    finished().bests[0].clone()
}

fn course_of(id: &str, bytes: &[u8]) -> (Map, Course) {
    let mut map = Map::parse(bytes).unwrap();
    let c = Course::parse(&maps(), id, &mut map).unwrap();
    (map, c)
}

fn utopia_set() -> course::Set {
    course::sets(&maps())
        .into_iter()
        .find(|s| s.id == "surf_utopia")
        .unwrap()
}

/// A fresh browser loads a file: what it ends with, and the note it shows.
fn load_into(mut g: Game, file: &str) -> (Game, Value) {
    let need: Vec<String> = serde_json::from_str(&g.backup_open(file)).unwrap();
    for id in need {
        let bytes = if id == "surf_kitsune" { KITSUNE } else { UTOPIA };
        g.backup_check(&id, bytes);
    }
    g.backup_finish();
    let screen: Value = serde_json::from_str(&g.screen()).unwrap();
    (g, screen["backup"].clone())
}

fn load_into_fresh(file: &str) -> (Game, Value) {
    load_into(Game::new(&maps(), "").unwrap(), file)
}

/// A file holding these runs, whole (a good checksum), as someone editing
/// the runs would write it, claiming things it never earned.
fn file_of(runs: &[RunRecord]) -> String {
    backup::write(&json!({
        "name": "Hana", "seed": 5,
        "bests": runs.iter().map(|r| r.to_json()).collect::<Vec<_>>(),
        "unlocked": ["chicken", "duck", "yeti"],
    }))
}

#[test]
fn a_saved_run_comes_back_and_its_things_are_earned_from_its_time() {
    let g = finished();
    let (h, note) = load_into_fresh(&g.backup_save());
    assert_eq!(note["key"], "backup_loaded", "{note}");
    assert_eq!(note["args"]["ok"], 1);
    assert_eq!(h.bests, g.bests, "the run, whole");
    assert_eq!(h.settings.name, "Hana");
    let mut want = vec![Held::Prayer];
    want.extend(backup::earned(&utopia_set(), g.bests[0].ticks));
    assert_eq!(h.unlocked, want, "earned from the time");
    assert!(h.unlocked.contains(&Held::Noodle), "a finish earns the noodle");
    assert!(
        g.unlocked.iter().all(|x| h.unlocked.contains(x)),
        "as the finish itself earned"
    );
}

#[test]
fn the_file_cannot_hand_out_things() {
    let run = real_run();
    let (h, _) = load_into_fresh(&file_of(std::slice::from_ref(&run)));
    let earned = backup::earned(&utopia_set(), run.ticks);
    assert_eq!(
        h.unlocked.contains(&Held::Chicken),
        earned.contains(&Held::Chicken),
        "the chicken only if the time beats the gym record"
    );
    assert!(
        !h.unlocked.contains(&Held::from_id("yeti").unwrap()),
        "nothing from a course with no run"
    );
}

#[test]
fn edited_runs_do_not_count() {
    let (map, c) = course_of("surf_utopia", UTOPIA);
    let good = real_run();
    assert_eq!(backup::verify(&good, &map, &c), Ok(()));
    // a faster time claimed, with inputs dropped to match it
    let mut quick = good.clone();
    quick.ticks -= 64;
    quick.cmds.truncate(quick.ticks as usize);
    assert_eq!(backup::verify(&quick, &map, &c), Err("it never finishes"));
    // a slower time claimed than the inputs take
    let mut slow = good.clone();
    slow.cmds.push(slow.cmds[slow.cmds.len() - 1]);
    slow.ticks += 1;
    assert_eq!(
        backup::verify(&slow, &map, &c),
        Err("it finishes before its last input")
    );
    // started part way down the course
    let mut ahead = good.clone();
    ahead.start.origin.x += 1500.0;
    assert_eq!(
        backup::verify(&ahead, &map, &c),
        Err("it starts away from the start")
    );
    // started with a running jump
    let mut fast = good.clone();
    fast.start.velocity.x += 800.0;
    assert_eq!(backup::verify(&fast, &map, &c), Err("it starts too fast"));
    // inputs changed part way: walking backwards off the ledge
    let mut turned = good.clone();
    for k in 0..turned.cmds.len() {
        turned.cmds[k].yaw = turned.cmds[k].yaw.wrapping_add(32768);
    }
    assert!(
        backup::verify(&turned, &map, &c).is_err(),
        "turned around, it does not finish there"
    );
    // claimed for another course
    let (km, kc) = course_of("surf_kitsune", KITSUNE);
    assert_eq!(backup::verify(&good, &km, &kc), Err("a run for another course"));
    let mut moved = good.clone();
    moved.map = "surf_kitsune".into();
    assert!(
        backup::verify(&moved, &km, &kc).is_err(),
        "utopia's inputs do not finish kitsune"
    );
    // and through the whole path, none of them counts
    let (h, note) = load_into_fresh(&file_of(&[quick]));
    assert!(h.bests.is_empty());
    assert_eq!(
        (note["key"].as_str(), note["args"]["bad"].as_u64()),
        (Some("backup_loaded_some"), Some(1))
    );
    let (h, _) = load_into_fresh(&file_of(&[moved]));
    assert!(h.bests.is_empty(), "a run moved to kitsune");
    assert_eq!(h.unlocked, vec![Held::Prayer]);
}

#[test]
fn a_damaged_file_changes_nothing() {
    let g = finished();
    let ticks = g.bests[0].ticks;
    let file = g.backup_save().replacen(
        &format!("\\\"ticks\\\":{ticks}"),
        &format!("\\\"ticks\\\":{}", ticks - 1),
        1,
    );
    assert_ne!(file, g.backup_save(), "the edit landed");
    let (h, note) = load_into_fresh(&file);
    assert_eq!(note["key"], "backup_damaged");
    assert!(h.bests.is_empty());
    assert_eq!(h.settings.name, Settings::default().name);
}

#[test]
fn a_loaded_run_only_replaces_a_slower_best() {
    let g = finished();
    let file = g.backup_save();
    let mut h = Game::new(&maps(), "").unwrap();
    let mut better = g.bests[0].clone();
    better.ticks -= 1;
    h.bests.push(better.clone());
    let (h, _) = load_into(h, &file);
    assert_eq!(h.bests, vec![better], "a faster best here stays");
    let mut slower = g.bests[0].clone();
    slower.ticks += 1;
    let mut k = Game::new(&maps(), "").unwrap();
    k.bests.push(slower);
    let (k, _) = load_into(k, &file);
    assert_eq!(k.bests, g.bests, "a slower one is replaced");
}

/// Every course's real clock start passes the start checks: a wanderer on
/// each course, taken from the tick its clock starts.
#[test]
fn real_starts_pass_the_start_checks_on_every_course() {
    let mut started = Vec::new();
    for course::Listed { id, .. } in course::listed(MAPS).unwrap() {
        let mut g = Game::new(MAPS, "").unwrap();
        g.load_map(&id, super::tests::map_bytes(&id)).unwrap();
        g.play();
        let mut t = 0.0;
        for i in 0..1500u32 {
            t += 1000.0 / 64.0;
            let keys = match (i / 48) % 4 {
                0 => B_FORWARD,
                1 => B_FORWARD | B_LEFT,
                2 => B_FORWARD | if i % 40 == 0 { B_JUMP } else { 0 },
                _ => B_FORWARD | B_RIGHT,
            } as u32;
            g.advance(t, keys, if (i / 48) % 2 == 1 { 5.0 } else { 0.0 }, 0.0);
            if let Some((start, _)) = &g.recording {
                let run = RunRecord {
                    map: id.clone(),
                    name: "x".into(),
                    seed: 0,
                    ticks: 1,
                    ending: 0,
                    start: start.body,
                    start_name: start.name,
                    start_stage: start.stage,
                    prev: start.last_cmd,
                    cmds: vec![Cmd::default()],
                    sync: 0.0,
                    strafes: 0,
                };
                assert_eq!(backup::start_ok(&run, &g.map, &g.course), Ok(()), "{id}");
                started.push(id.clone());
                break;
            }
        }
    }
    assert!(
        started.len() >= 10,
        "most courses start from a wander ({started:?})"
    );
}
