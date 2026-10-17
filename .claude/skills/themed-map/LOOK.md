# The look of a themed course

After Mage Tower Battle and Protection of the Spheres: what you touch stays
flat, what you look at goes round. A surf face must be a flat plane (a curve
is a row of seams that catch you), so roundness goes on scenery, things and
characters.

- **Palette.** `PALETTE` in the course file: one colour per role, in order.
  maps.json's `theme.palette` carries it; the core colours every face and
  blob by role. Role 0 is the water's surface.
- **Faces.** Material `M_SOFT`: light wrapped round the shape, a sky fill, a
  gentle shade down the face, faint ripples. No gym (no hall, mats or foam).
- **Scenery.** Blobs: smooth ellipsoids (an icosphere, finer when bigger),
  `M_BLOB`; `glow=True` lights one (a lure, embers); `clear=True` makes it
  see-through (bubbles).
- **Faces on creatures.** `sealife.eye`: a white ball, a dark pupil, a white
  catch-light. Keep cute: big eyes, round bodies, small fins.
- **Water and fog.** `WATER` is the surface's height (drawn see-through, blue
  from above, bright from below; sunlight dances on everything under it).
  `FOG` lists stops (height, colour, how far you see) from high to low; the
  core picks the eye's fog each frame and the page clears to it.
- **Friends.** A place named `pal1`, `pal2`... draws a kid in the scuba gear
  there, facing its yaw. A new costume for a new theme goes in
  `models::kid` beside `scuba`.
