# Surfs Up!

Counter-Strike surfing in the browser, on surf_utopia, surf_kitsune, surf_egypt and surf_anubis. The ramps are gym mats, the pits are full of foam cubes, and up to eight friends can surf together.

**Play:** https://sgilson7.github.io/surfs-up/

## The game

- **Four courses, rebuilt from the originals.** surf_utopia is Panzerhandschuh's surf_utopia_v3, surf_kitsune is Arblarg's nine-stage course, surf_egypt is Seabass45's classic run down past the pyramid, and surf_anubis is Sneakyyy's three-stage technical map, each read brush by brush from the original map file: every ramp, seam and gap is where it was. On kitsune a fall sends you back to the start of your stage with the clock still running, and its secret shortcuts still remember who found them. On egypt, as in the original, a hard landing on the ground (Source's fatal fall speed) sends you back to the start; landing in water is safe. Only the look has changed: a school gym on a Saturday, where surfing is just something kids do.
- **Source movement.** The movement is ported from the Source engine's: air acceleration capped at 30 units a second in the wish direction but not in the rate, ramps too steep to stand on (the 0.7 rule), velocity clipped along a ramp, and gravity applied in half-ticks at 64 ticks a second, with CS:GO surf-server settings (airaccelerate 150). Rampbugs are fixed the way surf servers fix them. `docs/RESEARCH-surf-movement.md` gives the sources for all of it.
- **A timer that knows surfing.** The clock starts when you leave the start and stops at the course's finish. It counts sync (how often your mouse turns the way your strafe key points), strafes and gain, as the CS surf timers do.
- **Things to hold.** You start with your palms pressed together. Each course has its own set. On utopia, a finish earns a pool noodle; bronze, silver and gold times earn a kickball, a squeaky duck and a foam finger; and the best time in the gym earns the rubber chicken. On kitsune the same times earn a paper pinwheel, a gym ribbon, a paper lantern, a hand bell and a fox plush; on egypt, a sand bucket, a toy camel, a toy pyramid, a golden scarab and a mummy plush; on anubis, a jump rope, a toy cat, a glow stick, a toy sceptre and a jackal plush. Click to squeeze what you hold.
- **Online, for up to eight.** Meet by room code or pasted invitation. You see the others surfing live: each browser re-runs everyone from their inputs, and rolls back when inputs arrive late. Each finished run is also sent once, whole, and races beside you as a ghost next time, all in your own browser. Turn off live surfers and only the finished runs travel. Press V to watch someone in your gym through their eyes; A and D switch surfer, as in Ape Race.

## Playing it

Click **Surf**, then click the view to capture the mouse. Do not hold W on a ramp. Hold A when the ramp rises to your left and D when it rises to your right, and turn the mouse the same way. Space or the scroll wheel jumps, Shift or C ducks, R goes back to the start, the mouse button squeezes, Q swaps what you hold, V watches the others online, Tab shows the scoreboard, and Esc pauses.

## How it is built

A [Builder](https://github.com/sgilson7/builder-setting) project, after Raiders of the Lost Mart. Every rule (movement, collision, triggers, the timer, rollback, what you earn) is in a Rust core (`crates/core`), compiled to WebAssembly through a shim that decides nothing (`crates/shim`), and a static page (`web/`) draws what the core returns. Online play uses ptonkle's Ape Race mesh transport (`web/net/`). The art is drawn in TikZ (`tools/art.py`).

    ./scripts/setup.sh         # once: Playwright and three browser engines
    ./scripts/check.sh         # format, lint, tests, build, WebAssembly audit, browsers, eight-browser room
    python3 scripts/serve.py   # then open http://127.0.0.1:8000

`BRIEF.md` has the request as it was given, `PLAN.md` the decisions taken from it, `SECOND-ORDER.md` what came up while building, and `HANDOFF.md` what was checked.

A fan-made game. MIT licence; the map's geometry is Panzerhandschuh's, credited in `LICENSES.md`. Not affiliated with Valve.
