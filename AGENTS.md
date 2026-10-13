# Instructions for a coding agent in Surfs Up!

`CLAUDE.md` holds the same rules for agents that read that file instead.

1. Read `BRIEF.md` and `BUILDER-SPEC.md` before changing code.
2. The core (`crates/core`) owns every game decision and performs no input or output.
3. The shim (`crates/shim`) translates values and decides no game behaviour.
4. The page (`web/`) draws what the core returns and does not reproduce a core rule.
5. Run `cargo test` after each meaningful core change.
6. For each new behaviour test, break the behaviour it covers once, watch the test fail, then restore it. Note the failure message in `SECOND-ORDER.md`.
7. Keep `SECOND-ORDER.md` while you work: one row per assumption, surprise, divergence from the plan, or item the person must do.
8. Do not deploy on your own judgement. Deploying is the person's decision (stage 8).
9. Before handoff, run `./scripts/check.sh` and record the result in `HANDOFF.md`.

Where things are: movement is `crates/core/src/movement.rs` (a port of Source's CGameMovement; every constant is in `TUNING`, with its source in `docs/RESEARCH-surf-movement.md`), collision is `trace.rs`, the map is `map.rs` reading `data/maps/surf_utopia.bin` (written by `tools/extract_map.py` from the original BSP), the timer and triggers are `run.rs`, online play is `online.rs`, and the game is `game.rs`. Art is TikZ: `python3 tools/art.py` writes `web/art/`. Do not change movement constants without a source; the feel is the product.
