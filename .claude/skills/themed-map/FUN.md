# What makes a tier 1 or 2 course fun, as checkable bounds

From the design log's Entry 1 (Momentum Mod's mapping docs, GameBanana
tutorials, KSF's map pages, and the ramps of the ten tier 1 and 2 surf courses
in this game, measured from their map files on 16 October 2026). A bound
marked *ours* is a proposal from those measurements; change it here.

| # | Rule | Bound |
| --- | --- | --- |
| 1 | Steep enough to slide, not awkward to board | ramp faces 46 to 60 degrees; tier 1 aims for 49 to 55 (ours: nine of ten courses centre at 47 to 52). Source's limit is 45.573 (normal z 0.7). |
| 2 | Wide and long, not oversized | a face at least 512 long; half-width about height / tan(angle) (512 high at 52 degrees is 400 across) |
| 3 | Curves smooth | a bent ridge turns at most 2 degrees a piece (`Chain.ridge` does this); the sweep must find no new stop |
| 4 | Simple collision | one plane per surf face; scenery is never solid |
| 5 | Transfers keep speed | gaps of 400 to 700 with a drop of 150 to 300 between ridges (ours) |
| 6 | Short runs | linear tier 1 records on KSF are 35 to 55 s; a themed course's gold is the brief's "good run" |
| 7 | Linear or staged on purpose | linear: a drop-in buoy before each part; staged: 4 to 11 stages |
| 8 | Checkpoints where a new part begins | at least 10 s apart in a good run |
| 9 | Falling costs little | back to the last buoy, facing on, stopped (`fall_reset`); never a jail |
| 10 | One idea at a time | each part adds one thing (a curve, a drop, b-hops, a vent) |
| 11 | Readable | surf faces one role colour per part; floors and decks another |
| 12 | One strong identity | a palette and a fog that change with the parts |
| 13 | Something soft under a big drop | a landing ridge under any drop over about 512 |
| 14 | Up only by a push | a vent (an upward push) wherever the course climbs; a chimney with a back wall when the speed you arrive with varies, so every blast flies the same way |
