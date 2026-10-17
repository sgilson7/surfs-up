---
name: themed-map
description: Turn a person's prose brief for a themed Surfs Up! course into a playable, deployed course - questions back, a course file built from the piece kit, generated geometry, fun lint, the bot's finish, five cubby things with inspects and juggles, a costume. Use when the person describes a new map (often the design log's "Your first map" tab pasted in).
---

# Themed map from a brief

Read `CLAUDE.md`, `BRIEF.md` and this folder's other files first. Movement
constants never change for a theme; the feel is the product.

The first course built this way is surf_100_leagues: its brief is
`briefs/surf_100_leagues.md`, its course file `tools/themes/surf_100_leagues.py`.
Copy it.

1. **Keep the brief.** Copy the person's words exactly into
   `briefs/<id>.md` under "The person's description (verbatim)". Typos stay.
2. **Ask.** List what the brief leaves open, from `QUESTIONS.md`: at most ten,
   numbered, each with the default you would pick. Stop until answered, unless
   the person says to build with your defaults; then list the defaults in the
   brief under "Answers by default". End the brief with a `- [ ]` checklist,
   one line per thing the brief asks for, ending "The other courses play
   exactly the same".
3. **Course file.** Write `tools/themes/<id>.py`: `PALETTE` (role, colour),
   `FOG`, `WATER` (or `None`), and `build(k)` placing pieces from `PIECES.md`
   on a `tools/coursekit.py` Course, scenery from `tools/themes/sealife.py`
   (or a new kit beside it), and returning the zones and roles. A comment
   above each part quotes the sentence of the brief it answers.
4. **Generate.** `python3 -I tools/make_course.py <id>` writes
   `data/maps/<id>.bin` and its maps.json entry (theme, `fall_reset`, zones).
   Fill in by hand once: name, author, tier, tier_source, medals, things.
   Add the course to the two `include_bytes!` lists (course.rs and game.rs
   tests).
5. **Lint.** Every piece inside `FUN.md`'s bounds. One outside goes back to
   the course file, or to the person as a choice.
6. **Prove it.** In this order, each until it passes:
   - `cargo test --release -p surf_core --test sweep -- --ignored --nocapture`:
     no new failure on the course (a bent ridge must ride clean both ways);
   - `RUNNER_COURSES=<id> RUNNER_SECONDS=900 cargo test --release -p surf_core
     --test runner -- --ignored find_runs --nocapture`: the bot finishes and
     its run is kept. Fix the layout, not the bot. Medals: gold is the
     brief's "good run", record about 0.8 of gold, silver about 1.3, bronze
     about 1.8; say the bot's time beside them;
   - in-game screenshots of every part (`?at=x,y,z,yaw,pitch` puts you there)
     and look at each one.
7. **Things.** Five `Held` variants in `crates/core/src/models.rs`, drawn with
   `orb` (smooth spheres, `THINGS.md`), each with its inspect and juggle; an
   icon in `tools/art.py`; a name in `data/copy.json`; a squeeze sound.
8. **Hand over.** Behaviour tests for anything new, each broken once and seen
   failing (`SECOND-ORDER.md` names the failure). `./scripts/check.sh`, then
   `tests/browser/online.py` locally. A HANDOFF entry, SECOND-ORDER rows, and
   an entry in the design log. Deploy only on the person's word.
