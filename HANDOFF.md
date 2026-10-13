# Handoff

Stage 9. Written 13 October 2026.

## What exists

| What | Where |
| --- | --- |
| The game | https://sgilson7.github.io/surfs-up/ (GitHub Pages, from `.github/workflows/deploy.yml`) |
| The repository | https://github.com/sgilson7/surfs-up (public, MIT) |
| The research | `docs/RESEARCH-surf-movement.md`: Source movement, constants with sources, sync and gain, rampbugs |
| The map | `data/maps/surf_utopia.bin`, from `tools/extract_map.py` and Panzerhandschuh's `surf_utopia_v3.bsp` |
| Deploys | a push to `main` runs `./scripts/check.sh` (Chromium, four browsers online) and deploys `dist/` to Pages only if it passes |

## Commands run, and results

| Command | Result |
| --- | --- |
| `./scripts/check.sh` (local, 13 Oct 2026) | BUILDER CHECK PASSED in 6 min: fmt, clippy (`-D warnings`), 49 Rust tests (34 core, 12 boundary, 3 surfing on the real map), release build, WebAssembly import audit, browser check in Chromium, Firefox and WebKit, and eight browsers in one room in all three engines |
| `cargo test -p surf_core --test fixtures -- --ignored` | wrote `tests/browser/fixtures/saved_best.json` |
| `python3 tools/art.py` | wrote the title, icon, how-to-surf diagram and six item icons |
| Tests broken on purpose and restored | the rampbug test (`PUSH_OFF = 0`), the chicken (`"best"` never earned); both failed, then passed |

## What was not checked

- **Nobody has surfed the whole course in this build.** A scripted surfer rides the first ridge on both sides, and the course was looked at from points along its length, but no person or bot has finished a run. The finish doors (SECOND-ORDER row 11) and the medal times (row 12) rest on reading the map, not on playing it.
- **The room-code path over public relays** was not exercised; the online test uses pasted codes on one machine. Try two computers: one presses Host a room, the other types the code.
- **Feel on a real mouse.** Pointer capture cannot happen in a headless browser, so mouse turning was tested in the core (`the_mouse_turns_the_view_the_source_way`), not by hand.

## Waiting on the person

Rows 11 to 15 of `SECOND-ORDER.md`: the finish doors, the medal times, publishing the map geometry, ptonkle's agreement to reuse his transport, and a room-code test between two computers.
