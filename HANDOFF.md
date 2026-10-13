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

## Deployed

| What | Result |
| --- | --- |
| GitHub Actions on `main` | `check` (Chromium, four browsers online) and `deploy` succeeded on the third push; the first two failed on CI timing (SECOND-ORDER rows 16 and 17) |
| https://sgilson7.github.io/surfs-up/ | 200; `tests/browser/check.py` against the live origin passed in Chromium, Firefox and WebKit |
| The website listing | website commit `0686179` "List Surfs Up!", published as build `1c9bad20` by `make deploy`: https://sgilson7.github.io/projects/surfs-up/ answers 200 and /projects/ links to it; `make check`, `make packet`, `make gate` and `make gate-live` passed; `testing/engines.py` drew all sixteen games in three engines. The website's source repository was not pushed, as earlier listings were not. |

## What was not checked

- **Nobody has surfed the whole course in this build.** A scripted surfer rides the first ridge on both sides, and the course was looked at from points along its length, but no person or bot has finished a run. The finish doors (SECOND-ORDER row 11) and the medal times (row 12) rest on reading the map, not on playing it.
- **The room-code path over public relays** was not exercised; the online test uses pasted codes on one machine. Try two computers: one presses Host a room, the other types the code.
- **Feel on a real mouse.** Pointer capture cannot happen in a headless browser, so mouse turning was tested in the core (`the_mouse_turns_the_view_the_source_way`), not by hand.

## Waiting on the person

Rows 11 to 15 of `SECOND-ORDER.md`: the finish doors, the medal times, publishing the map geometry, ptonkle's agreement to reuse his transport, and a room-code test between two computers.

## Second request: surf_kitsune (13 October 2026)

surf_kitsune is the second course, picked on the title. `./scripts/check.sh` passed after it: 54 Rust tests (new: the course file resolves against both maps, kitsune's red door starts stage 1, a fall keeps the clock, its secrets remember your name, bests are kept per course, a first-version save keeps its utopia best), the browser check in three engines (now choosing kitsune and walking through its red door to stage 1 of 9), and eight browsers in one room. The secret-filter test was broken on purpose and failed. Not checked: nobody has surfed kitsune's nine stages; its medal times are provisional (SECOND-ORDER rows 20 to 22).

## Third request: dismounting and kitsune's stage 2 (13 October 2026)

`docs/ANALYSIS-dismount.md`. The cause was the teleport: Source keeps velocity through a stage door and this game zeroed it. Fixed, with three CS2 alignments (no slow-rise air cut, two air steps a tick, the snag offset). Kitsune's map is unchanged. New tests: a stage door keeps speed, two air steps gain more than one, stage 2 is surfable with stage 1's speed (a beam search), and a save from the first rules keeps everything but its best runs. The first two were broken on purpose and failed. Not checked by a person: how it feels with a mouse (SECOND-ORDER row 26).

## Fourth request (13 October 2026): take out the speed wind

"the noise when you start getting fast is awful, take that out". The wind (filtered noise rising with speed) is removed from the core's HUD, `web/sound.js`, `web/app.js` and `data/sounds.json` (`SECOND-ORDER.md` row 27). `./scripts/check.sh`: BUILDER CHECK PASSED.
