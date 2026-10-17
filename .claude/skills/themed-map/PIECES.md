# The piece kit (`tools/coursekit.py`)

A brief's sentences become these pieces. Each is a few convex brushes with
vbsp's bevels, so the core's collision, the sweep and the runner work on them
unchanged. Every solid face carries a palette role (`THEME_HINT + role`).

| Piece | Call | Notes |
| --- | --- | --- |
| ridge | `Chain(k, at, yaw).ridge(length, drop, role, turn=0, angle=52, height=512, sides="both", under=None, gap=500, gap_drop=200)` | the next ridge starts `gap` on and `gap_drop` lower; `sides` "left"/"right" makes a one-sided ramp (a hull's side, a whale's flank); a negative `drop` climbs (a line up to a boat) |
| bent ridge | `turn=` on a ridge | 2 degrees a piece; the pieces meet where their faces meet and overlap by `OVERLAP` (0.5), so no lip opens between them |
| deck, landing, buoy | `k.oval(c, rx, ry, z0, z1, role, yaw, side=)` | flat, 20 sides |
| box | `k.box(lo, hi, role, top=)` | a beach, a sea floor |
| any convex solid | `k.hull(points, role)` or `k.planes([(n, d, role)...])` | a chimney's walls |
| fall | `k.catch(lo, hi, to)` | a teleport to a place; with `fall_reset` a fall stops you there |
| vent | `k.vent(lo, hi, pitch, yaw, speed)` | Source's trigger_push; `pitch` -90 is straight up; it accelerates you up by (speed - 800) a second and its push becomes your own when you leave; a stander is lifted off the ground |
| place | `k.dest(name, at, yaw)` | the spawn, each buoy; a place named `pal...` draws a friend in scuba gear standing there |
| scenery | `k.blob(role, centre, radii, yaw, pitch, roll, glow=, clear=)` | an ellipsoid, drawn only; creatures in `tools/themes/sealife.py` are made of them |

Helpers in the course file: `catch_under(k, piece, to)` puts a fall under a
ridge; `drop_in(k, name, piece)` puts a buoy over a ridge's start.

The finish is a `finish_zone` box returned by `build`; the start is a
`start_zone` box round the spawn.
