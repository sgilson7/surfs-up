# Second-order notebook

Stage 6, kept by the agent as the work happened. `B3 Build in self-questioning`.

One row per assumption, surprise or open item. **Kind** is *divergence*, *finding*, or *worklist* (the person's to do). **Status** is *done*, *open*, or *person*.

| # | Kind | What was assumed or found | What is true | What changed | Status |
| --- | --- | --- | --- | --- | --- |
| 1 | finding | The map would have to be rebuilt by eye from videos. | GameBanana still serves Panzerhandschuh's `surf_utopia_v3.bsp` (TF2, 2010; uncompressed lumps). Its brushes, bevels, triggers and start can be read directly. | `tools/extract_map.py`; P1. | done |
| 2 | finding | Brush-entity brushes (triggers, func_illusionary) are in world space. | They are stored about the entity's origin; the first extraction put the jail's triggers at the course. | The extractor moves each plane by the origin (`d + n·origin`). | done |
| 3 | divergence | Builder's default D2 bans floats in the core. | Source's feel is float clipping; a fixed-point port would change it. Determinism needs only the same module everywhere and no platform maths. | D2 changed: floats allowed, platform sine/cosine/powers banned by the boundary test; sine and cosine computed in `vec3.rs`. Replay to the bit is tested. | done |
| 4 | finding | The research agent's 900 (u/s)² per tick ideal strafe gain. | My first air-strafe test turned at a fixed 1.5°/tick and failed (441 u/s, expected more): the velocity outran the view. Perpendicular wish gives exactly sqrt(v² + 64·900). | The test now holds the wish perpendicular and checks the exact value. | done |
| 5 | finding | A ramp-bot dropped on the first ridge would ride it. | On the y > 0 side it froze at x = -11710 with speed growing: the ridge bends where brushes 3213 and 3210 meet, clipping left a sliver of velocity into the ramp, and every later bump hit the same plane at fraction 0. That is the rampbug. | Same-plane hits are clipped again with a 0.25 u/s push-off (`PUSH_OFF`). `the_first_ridge_has_no_rampbug_on_either_side` fails with `PUSH_OFF = 0` (its "frozen at" assertion) and passes with it. | done |
| 6 | finding | A test body placed above a "ramp" face would slide. | The nearest steep faces to the start are inside solid geometry (a lip, decor): the body was stuck in solid with growing velocity, which exposed that `player_move` never ran CheckStuck. | `player_move` now unsticks first and does not move a body it cannot free. Tests pick faces with free space above them. | done |
| 7 | divergence | The boundary scan allows a core module named `net`. | `net` is on the I/O identifier list. | The module is `online.rs`. | done |
| 8 | finding | Live-surfer snapshots went out every 32 ticks. | They never did: ticks start at 1 and batches are two, so the batch's first tick was always odd. | A batch counter decides; a newcomer forces one at once. `two_games_meet_and_see_each_other_surf` caught it. | done |
| 9 | divergence | All eight surfers would be seen at the start. | They spawn on one point (the map has one start), so name tags overlap the eye and are not drawn until people move. | Kept: surf servers do not collide players. The online test checks a walker ahead instead. | done |
| 10 | finding | The prayer hands, modelled palm to palm, would read as hands. | From the eye they were edge-on: a thin column. | Turned 28° and tilted forward; a test keeps them low and centred and in front of the near plane. | done |
| 11 | divergence | Which ending finishes a run. | The TF2 map has six endings into different jail rooms. By height, three doors on the final (west) wall come last; the other three are partway down. | Those three finish a run; the rest are early exits with no time (P4). Not confirmed by playing the whole course. | open |
| 12 | worklist | Medal times. | Nobody has surfed the whole course in this build; the times are estimates from path length (about 180,000 units) and typical speeds. | Provisional values in `game.rs` (P5). | person |
| 13 | worklist | Publishing the map's geometry. | It is Panzerhandschuh's, credited in `LICENSES.md`, without textures, non-commercial. | Whether that is acceptable is the person's call. | person |
| 14 | worklist | Reusing ptonkle's `online.js`. | The brief asks for Ape Race's netcode; his repository has no licence file. | Credited in `LICENSES.md`; ask him. | person |
| 15 | worklist | The room-code path over public relays. | Checked here only by pasted codes on one machine (`online.py`, eight browsers). | Try a room code between two real computers (HANDOFF.md). | person |
