# Things to hold: model, inspect, juggle

Each course has five, earned in order: finish, bronze, silver, gold, best.

**Model.** A `Held` variant (add it to `ALL`, `id`, `thing`, the squeeze sound
in game.rs). Draw with `orb(out, f, centre, radii, colour, m)` (a smooth
ellipsoid), `orb_turned` (turned by yaw then pitch) and `orb_along` (a spike
along a direction). It stands on the frame's origin, about 8 to 10 units
long; `sq.yaw(70.0)` shows a fish side on. `fish_eyes` gives the dot eyes.
The goldfish, eel, pufferfish, squishy fish and knife fish are worked
examples.

**Inspect** (tap F, 2.5 s, `Pose::Inspect`). Two kinds today:
- turned over in the hands: raised, turned once round, tilted (every thing);
- dangled from a fishing line: up out of the hands on a line, swinging and
  turning (`Held::dangles`; surf_100_leagues' five).
One in twenty is a rare one (a flip, or a wriggle and a flip), chosen from
the clock, so a replay shows the same.

**Juggle** (hold F, `Pose::Juggle`): tossed hand to hand in an arc, turning
once a toss (the knife fish twice).

A trick the brief asks for that these do not cover (a moth flying out, a
lid opening) is new code in `models::posed`, with its own test, and a
SECOND-ORDER row saying so.

**Icon.** A function in `tools/art.py` (TikZ, the palette at its top), listed
under `items/<id>`; `python3 tools/art.py items/<id>`.
