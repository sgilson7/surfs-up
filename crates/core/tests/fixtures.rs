//! Writes the fixtures the browser tests load. Run on purpose:
//!     cargo test -p surf_core --test fixtures -- --ignored
//! tests/browser/fixtures/saved_best.json is a saved game whose best "run"
//! is a short walk off the start ledge: a real recording through the same
//! step every surfer runs, so another browser can replay it as a ghost.

use std::fs;
use surf_core::course::Course;
use surf_core::map::Map;
use surf_core::movement::{Cmd, Mover, B_FORWARD};
use surf_core::online::RunRecord;
use surf_core::run::Surfer;

#[test]
#[ignore]
fn write_saved_best() {
    let mut m = Map::parse(include_bytes!("../../../data/maps/surf_utopia.bin")).unwrap();
    let zone = Course::parse(
        include_str!("../../../data/maps/maps.json"),
        "surf_utopia",
        &mut m,
    )
    .unwrap();
    let mut mv = Mover::new(&m);
    let mut s = Surfer::at_start(&m);
    let mut rec = None;
    for _ in 0..400 {
        let c = Cmd::new(B_FORWARD, 0.0, 0.0);
        let t = s.step(&mut mv, &zone, c);
        if let Some((_, _, cmds)) = &mut rec {
            let cmds: &mut Vec<Cmd> = cmds;
            cmds.push(c);
        }
        if t.started {
            rec = Some((s.body, c, Vec::new()));
        }
    }
    let (start, prev, cmds) = rec.expect("left the ledge");
    let run = RunRecord {
        map: "surf_utopia".into(),
        start_name: 0,
        start_stage: 0,
        name: "Hana".into(),
        seed: 5,
        ticks: cmds.len() as u32,
        ending: 5,
        start,
        prev,
        cmds,
        sync: 88.5,
        strafes: 4,
    };
    let saved = serde_json::json!({
        "name": "Hana", "seed": 5, "unlocked": ["prayer", "noodle"], "held": "noodle",
        "finishes": 1, "best": run.to_json(),
    });
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/browser/fixtures/saved_best.json");
    fs::write(path, serde_json::to_string_pretty(&saved).unwrap()).unwrap();
}
