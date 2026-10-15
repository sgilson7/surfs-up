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

## Fifth request (13 October 2026): things to hold, a kitsune set, spectating

"unlocking the pool noodle and stuff doesn't seem to work at all, also add a new set you can unlock for kitsune" and "there should be a spectator mode like ape race to watch other people in your gym online". Unlocking was broken by the order of the finish and its teleport (row 28); each course now has its own set (row 29); V watches someone in your gym (row 30); the kitsune browser walk starts near the door (row 31). `./scripts/check.sh`: BUILDER CHECK PASSED.

## Sixth request (13 October 2026): kitsune online, edges and platforms

"fix the weird see-through edges on kitsune, and change the platforms to more the original kitsune style. also i dont think kitsune works online properly". The title lost a course choice made right after typing a name, so friends ended up on different courses (row 32); live snapshots now keep the map-given name and stage (row 33); kitsune's platforms are solid neon grids in each stage's colour, with neon edges (row 34). `./scripts/check.sh`: BUILDER CHECK PASSED.

## Seventh request (13 October 2026): kitsune's first wedge

"the first wedge is like kind of see through and hard to tell what youre looking at". Row 35: the doubled glass shells are drawn once, lit so the faces read apart, with the hollow ends closed. Collision is unchanged. `./scripts/check.sh`: BUILDER CHECK PASSED.

## Eighth request (13 October 2026): surf_egypt

"theres this egyptian themed pretty technical map we liked from back in the day, could you find that and add it as a new map?" Sam chose surf_egypt (Seabass45). Rows 36 to 40: the extractor follows Source's entity-named teleport targets; the run starts at the spawn and finishes at the three bottom doors; Source's fatal fall is on for egypt only; egypt's own set of five things. Medal times are provisional and nobody has played it through yet. `./scripts/check.sh`: BUILDER CHECK PASSED.

## Ninth request (13 October 2026): surf_anubis, and utopia's flashing start

Utopia's start flickered because the kitsune glass-ramp look reached its glass side panes; it now needs a kitsune grid texture (row 41, deployed first on its own). surf_anubis is the fourth course: stages 1 to 3, finishing in its end zone (rows 42 to 46). Bonuses are not timed; medal times are provisional; nobody has played it through yet. `./scripts/check.sh`: BUILDER CHECK PASSED.

## Tenth request (13 October 2026): online on every course, an Egyptian multi-parter

"make sure all the online still works with the new map": every course is checked online in the core and in browsers (row 47). anubis is tier 7+; after a survey Sam chose surf_sandtrap, added as the fifth course with displacement collision (rows 48 to 52).
`./scripts/check.sh`: BUILDER CHECK PASSED (the online check now includes a pair of browsers on every course).

## Eleventh request (14 October 2026): tiers, surf_aircontrol, surf_mesa, bhop_ambience

From Sam's friend, a long-time surfer: add bhop_ambience and surf_aircontrol, show tiers on the course choice "so people who don't know how to surf dont imemdiately ruin themselves", and add surf_mesa. Every card now shows its tier from the server lists (row 55) in a coloured badge with a plain word ("Good for beginners" to "Experts only"), easiest first, bhop last. anubis is tier 8 by KSF and SharpTimer, not 5. The three maps are courses six to eight, each with a set of five things: map format v3 carries displacement grids (mesa went from 38 MB to 2.2 MB, row 56); Source's trigger_hurt is honoured (aircontrol's kill floors, and egypt's ground Killzone, rows 57 and 58); the zones are in rows 59 to 61. Medal times are provisional; nobody has played the three through.
`./scripts/check.sh`: BUILDER CHECK PASSED (eight courses, tiers shown in chromium, firefox and webkit; online pairs on every course).

## Twelfth request (15 October 2026): the cubby scrolls again

"fix the cubby cause you cant scroll it fully anymore": with 41 things its top was out of reach (row 65); every screen now starts at its top when it is taller than the window, and the browser check scrolls the cubby from title to close button.
`./scripts/check.sh`: BUILDER CHECK PASSED.

## Thirteenth request (15 October 2026): eight more maps, tier tabs

"find two more of the best, highly rated, fondly remembered surf maps that are tier 1, then find two more that are tier 2, then do the same for bhop maps ... then, change the menu to be tabs of tiers ... with a lil tikz icon ... then deploy". Chosen by SurfHeaven play counts and SharpTimer tiers (row 67): surf_beginner and surf_summit (tier 1), surf_summer and surf_deadline (tier 2), bhop_eazy and bhop_lego (bhop tier 1), bhop_japan and bhop_cartoons (bhop tier 2), each with a set of five things (81 things in all). The title now has tabs (Tier 1, Tier 2, Advanced, Bhop; row 66) of compact course chips, each with a TikZ icon. Bhop blocks are held still (row 68); a rename's output delay is honoured (row 70); triggers no plain player can pass are dropped (row 73); lego's stand-still teleports are left out (row 75, for Sam to try). Medal times are provisional; nobody has played the eight through.
The first push failed CI (a tab press lost to a redraw on the slow runner, row 80); fixed and checked again.
`./scripts/check.sh`: BUILDER CHECK PASSED (sixteen courses under four tabs and 81 things in chromium, firefox and webkit; online pairs on every course).
