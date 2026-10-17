"""Build a course by hand: the pieces a themed course is made of, written to
the game's map file (data/maps/<id>.bin), as tools/extract_map.py writes one
read from a Source map.

A course is described in tools/themes/<id>.py with these pieces (see
.claude/skills/themed-map/PIECES.md):

  ridge(stations, ...)   a surf ramp: a ridge of two faces (or one) swept
                         along a line of stations; a bent ridge is a ridge of
                         many stations, each segment a convex brush, adjacent
                         segments sharing their cross-section exactly
  oval / slab / box      flat things to stand on (a landing, a deck, a beach)
  chute(...)             a ramp that faces forward (a drop's landing, a
                         line to surf up), its top sloped along its heading
  catch(lo, hi, to)      a fall: back to a drop-in point, clock running
  vent(lo, hi, ...)      a push upward (Source's trigger_push)
  dest(name, at, yaw)    a named place (the spawn, each drop-in point)
  blob(...)              scenery: an ellipsoid drawn, never solid; creatures
                         are made of blobs (tools/themes/sealife.py)

Every solid face carries its palette role as its hint (THEME_HINT + role),
so the core colours it from the course's theme in maps.json. Brushes get
the bevel planes Source's compiler adds (vbsp's AddBrushBevels), as
tools/phy.py gives static props, so a box swept against them stops where
the solid is.

Format v4 is v3 (tools/extract_map.py) followed by:
  u32 blobs; per blob: u8 role, u8 flags (bit0 glows, bit1 see-through),
     f32 centre xyz, f32 radii xyz, f32 yaw pitch roll (degrees)
"""
import itertools
import math
import struct
from pathlib import Path

MAPS = Path(__file__).resolve().parents[1] / "data" / "maps"
NONE = 65535
THEME_HINT = 64
SOLID, CLIP, WINDOW, VISUAL = 0, 1, 2, 3
GLOW, CLEAR = 1, 2
# how far a bent ridge's pieces reach into each other (units)
OVERLAP = 0.5


def sub(a, b):
    return (a[0] - b[0], a[1] - b[1], a[2] - b[2])


def add(a, b):
    return (a[0] + b[0], a[1] + b[1], a[2] + b[2])


def mul(a, k):
    return (a[0] * k, a[1] * k, a[2] * k)


def dot(a, b):
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


def cross(a, b):
    return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])


def unit(v):
    n = math.sqrt(dot(v, v))
    return None if n < 1e-9 else (v[0] / n, v[1] / n, v[2] / n)


def heading(yaw):
    r = math.radians(yaw)
    return (math.cos(r), math.sin(r), 0.0)


def hull_faces(pts):
    """The face planes of a small convex point set: every plane through three
    points with all points behind it, one per distinct normal."""
    faces = {}
    for a, b, c in itertools.combinations(range(len(pts)), 3):
        n = unit(cross(sub(pts[b], pts[a]), sub(pts[c], pts[a])))
        if n is None:
            continue
        for s in (1.0, -1.0):
            m = mul(n, s)
            d = dot(m, pts[a])
            if all(dot(m, p) <= d + 0.05 for p in pts):
                key = (round(m[0], 5), round(m[1], 5), round(m[2], 5))
                if key not in faces:
                    on = [i for i, p in enumerate(pts) if abs(dot(m, p) - d) < 0.05]
                    faces[key] = (m, d, on)
    return list(faces.values())


def bevels(pts, faces):
    """The faces, then vbsp's bevels: axial planes at the bounds, and planes
    through each edge along each axis that every point lies behind. Returns
    (normal, distance, bevel) per side."""
    out = []
    keys = set()

    def put(n, d, bevel):
        key = (round(n[0], 4), round(n[1], 4), round(n[2], 4))
        if key in keys:
            return
        keys.add(key)
        out.append((n, d, bevel))

    for n, d, _ in faces:
        put(n, d, False)
    for k in range(3):
        for s in (1.0, -1.0):
            n = [0.0, 0.0, 0.0]
            n[k] = s
            put(tuple(n), max(s * p[k] for p in pts), True)
    edges = set()
    for i, j in itertools.combinations(range(len(pts)), 2):
        shared = sum(1 for _, _, on in faces if i in on and j in on)
        if shared >= 2:
            edges.add((i, j))
    for i, j in edges:
        e = sub(pts[j], pts[i])
        for k in range(3):
            axis = [0.0, 0.0, 0.0]
            axis[k] = 1.0
            for s in (1.0, -1.0):
                n = unit(cross(e, tuple(axis)))
                if n is None:
                    continue
                n = mul(n, s)
                d = dot(n, pts[i])
                if all(dot(n, p) <= d + 0.01 for p in pts):
                    put(n, d, True)
    return out


class Course:
    def __init__(self, roles):
        """`roles`: the palette role names, in the order maps.json's theme
        lists their colours."""
        self.roles = list(roles)
        self.brushes = []
        self.triggers = []
        self.dests = []
        self.names = [""]
        self.blobs = []

    def role(self, name):
        return self.roles.index(name)

    def name(self, n):
        n = n.lower()
        if n not in self.names:
            self.names.append(n)
        return self.names.index(n)

    # ---- solids ----------------------------------------------------------

    def hull(self, pts, role, kind=SOLID, face_roles=None):
        """A convex brush from its corner points. `face_roles(normal)` may
        give a face its own role (a ridge's underside, a deck's sides)."""
        faces = hull_faces(pts)
        sides = []
        for n, d, bevel in bevels(pts, faces):
            r = role if bevel or face_roles is None else face_roles(n)
            sides.append((n, d, bevel, self.role(r)))
        self.brushes.append((kind, len(self.brushes), sides))

    def box(self, lo, hi, role, kind=SOLID, top=None):
        sides = []
        for k in range(3):
            for s in (1.0, -1.0):
                n = [0.0, 0.0, 0.0]
                n[k] = s
                d = hi[k] if s > 0 else -lo[k]
                r = top if (top and k == 2 and s > 0) else role
                sides.append((tuple(n), d, False, self.role(r)))
        self.brushes.append((kind, len(self.brushes), sides))

    def oval(self, c, rx, ry, z0, z1, role, yaw=0.0, k=20, side=None):
        """A flat oval deck: its top at z1, `k` sides around."""
        f = heading(yaw)
        l = (-f[1], f[0], 0.0)
        sides = [((0.0, 0.0, 1.0), z1, False, self.role(role)), ((0.0, 0.0, -1.0), -z0, False, self.role(side or role))]
        ring = []
        for i in range(k):
            a = 2 * math.pi * (i + 0.5) / k
            ring.append((c[0] + f[0] * rx * math.cos(a) + l[0] * ry * math.sin(a), c[1] + f[1] * rx * math.cos(a) + l[1] * ry * math.sin(a)))
        for i in range(k):
            p, q = ring[i], ring[(i + 1) % k]
            n = unit((q[1] - p[1], -(q[0] - p[0]), 0.0))
            if dot(n, (p[0] - c[0], p[1] - c[1], 0.0)) < 0:
                n = mul(n, -1.0)
            sides.append((n, dot(n, (p[0], p[1], 0.0)), False, self.role(side or role)))
        xs = [p[0] for p in ring]
        ys = [p[1] for p in ring]
        for k2, (lo, hi) in enumerate([(min(xs), max(xs)), (min(ys), max(ys))]):
            n = [0.0, 0.0, 0.0]
            n[k2] = 1.0
            sides.append((tuple(n), hi, True, self.role(role)))
            n = [0.0, 0.0, 0.0]
            n[k2] = -1.0
            sides.append((tuple(n), -lo, True, self.role(role)))
        self.brushes.append((SOLID, len(self.brushes), sides))

    def ridge(self, stations, role, angle=52.0, height=512.0, sides="both", under=None):
        """A surf ridge swept along `stations`: (x, y, crest z, heading).
        Its faces fall `height` from the crest at `angle` degrees from
        horizontal; `sides` is "both", "left" or "right" (a one-sided ramp's
        other side is a wall straight down). Each pair of stations makes one
        brush of exact planes: its faces hold its straight piece of crest,
        and its ends are the vertical planes that halve the turn at each
        station, so neighbouring pieces meet face to face along a line, with
        no lip between them (a bent ridge as Hammer's arch tool builds one)."""
        under = under or role
        t = math.tan(math.radians(angle))
        segs = []
        for i in range(len(stations) - 1):
            (x0, y0, z0, _), (x1, y1, z1, _) = stations[i], stations[i + 1]
            a, b = (x0, y0, z0), (x1, y1, z1)
            d = unit(sub(b, a))
            h = unit((d[0], d[1], 0.0))
            l = (-h[1], h[0], 0.0)
            sides_ = []
            for side, sign in (("left", 1.0), ("right", -1.0)):
                if sides in ("both", side):
                    # down the face: across the crest, falling at the angle
                    w = add(mul(l, sign), (0.0, 0.0, -t))
                    n = unit(cross(d, w)) if sign > 0 else unit(cross(w, d))
                    if n[2] < 0:
                        n = mul(n, -1.0)
                    sides_.append((n, dot(n, a), role))
                else:
                    # the other side of a one-sided ramp: a wall straight down
                    n = mul(l, sign)
                    sides_.append((n, dot(n, a), under))
            segs.append({"a": a, "b": b, "h": h, "sides": sides_, "low": min(z0, z1) - height})

        def cap(i):
            """The plane at station i between segment i-1 and segment i:
            through the lines where their left faces meet and where their
            right faces meet, so both faces join with no lip. Its normal
            points along the course."""
            if i == 0 or i == len(segs):
                s_ = segs[0] if i == 0 else segs[-1]
                p = s_["a"] if i == 0 else s_["b"]
                return s_["h"], dot(s_["h"], p)
            A, B = segs[i - 1], segs[i]
            p = A["b"]
            lines = []
            for (na, _, _), (nb, _, _) in zip(A["sides"], B["sides"]):
                ln = cross(na, nb)
                if math.sqrt(dot(ln, ln)) > 1e-6:
                    lines.append(unit(ln))
            if len(lines) == 2:
                n = unit(cross(lines[0], lines[1]))
            else:
                n = None
            if n is None or len(lines) < 2:
                n = unit(add(A["h"], B["h"]))
            if dot(n, add(A["h"], B["h"])) < 0:
                n = mul(n, -1.0)
            return n, dot(n, p)

        caps = [cap(i) for i in range(len(segs) + 1)]
        for i, sg in enumerate(segs):
            planes = list(sg["sides"])
            planes.append(((0.0, 0.0, -1.0), -sg["low"], under))
            # neighbours overlap by a hair, so rounding (the map file keeps
            # f32) cannot open a lip where their faces meet
            n0, d0 = caps[i]
            planes.append((mul(n0, -1.0), -(d0 - (OVERLAP if i > 0 else 0.0)), under))
            n1, d1 = caps[i + 1]
            planes.append((n1, d1 + (OVERLAP if i + 1 < len(segs) else 0.0), under))
            self.planes(planes)

    def planes(self, planes, kind=SOLID):
        """A convex brush from its face planes (normal, distance, role),
        with vbsp's bevels from its corners."""
        pts = []
        for p, q, r in itertools.combinations(planes, 3):
            n1, n2, n3 = p[0], q[0], r[0]
            det = dot(n1, cross(n2, n3))
            if abs(det) < 1e-9:
                continue
            x = mul(add(add(mul(cross(n2, n3), p[1]), mul(cross(n3, n1), q[1])), mul(cross(n1, n2), r[1])), 1.0 / det)
            if all(dot(pl[0], x) <= pl[1] + 0.01 for pl in planes):
                if not any(abs(x[0] - y[0]) + abs(x[1] - y[1]) + abs(x[2] - y[2]) < 0.01 for y in pts):
                    pts.append(x)
        faces = []
        for n, d, _ in planes:
            on = [i for i, x in enumerate(pts) if abs(dot(n, x) - d) < 0.05]
            faces.append((n, d, on))
        role_of = {(round(n[0], 4), round(n[1], 4), round(n[2], 4)): r for n, _, r in planes}
        sides = []
        for n, d, bevel in bevels(pts, faces):
            r = role_of.get((round(n[0], 4), round(n[1], 4), round(n[2], 4)), planes[0][2])
            sides.append((n, d, bevel, self.role(r)))
        self.brushes.append((kind, len(self.brushes), sides))

    def chute(self, start, yaw, length, pitch, width, role, thick=64.0, under=None):
        """A ramp facing along `yaw`, falling `pitch` degrees over `length`
        (its top's horizontal run), `width` across, from `start` (the top
        edge's middle)."""
        f = heading(yaw)
        l = (-f[1], f[0], 0.0)
        run = length
        drop = run * math.tan(math.radians(pitch))
        a = start
        b = (start[0] + f[0] * run, start[1] + f[1] * run, start[2] - drop)
        pts = []
        for p in (a, b):
            for s in (-0.5, 0.5):
                q = (p[0] + l[0] * width * s, p[1] + l[1] * width * s, p[2])
                pts.append(q)
                pts.append((q[0], q[1], q[2] - thick))
        under = under or role
        self.hull(pts, role, face_roles=lambda n: role if n[2] > 0.05 else under)
        return b

    # ---- triggers and places ---------------------------------------------

    def _trigger(self, kind, a, lo, hi, pitch=0.0, yaw=0.0, speed=0.0):
        planes = []
        for k in range(3):
            n = [0.0, 0.0, 0.0]
            n[k] = 1.0
            planes.append((tuple(n), hi[k]))
            n = [0.0, 0.0, 0.0]
            n[k] = -1.0
            planes.append((tuple(n), -lo[k]))
        self.triggers.append((kind, a, NONE, (pitch, yaw, 0.0, speed), planes))

    def catch(self, lo, hi, to):
        self._trigger(0, self.name(to), lo, hi)

    def vent(self, lo, hi, pitch, yaw, speed):
        """Source's trigger_push: `pitch` negative pushes up."""
        self._trigger(1, 0, lo, hi, pitch, yaw, speed)

    def dest(self, name, at, yaw):
        self.dests.append((self.name(name), at[0], at[1], at[2], yaw))

    def blob(self, role, c, r, yaw=0.0, pitch=0.0, roll=0.0, glow=False, clear=False):
        if isinstance(r, (int, float)):
            r = (r, r, r)
        flags = (GLOW if glow else 0) | (CLEAR if clear else 0)
        self.blobs.append((self.role(role), flags, c, r, (yaw, pitch, roll)))

    # ---- the file ---------------------------------------------------------

    def write(self, map_id):
        out = bytearray(b"SURF") + struct.pack("<I", 4)
        out += struct.pack("<I", len(self.brushes))
        for kind, group, sides in self.brushes:
            out += struct.pack("<BIH", kind, group, len(sides))
            for n, d, bevel, role in sides:
                flags = 1 if bevel else 2
                out += struct.pack("<ffffBB", n[0], n[1], n[2], d, flags, THEME_HINT + role)
        out += struct.pack("<I", len(self.names))
        for t in self.names:
            b = t.encode()
            out += struct.pack("<B", len(b)) + b
        out += struct.pack("<I", len(self.triggers))
        for kind, a, flt, p, planes in self.triggers:
            out += struct.pack("<BHHffffH", kind, a, flt, *p, len(planes))
            for n, d in planes:
                out += struct.pack("<ffff", n[0], n[1], n[2], d)
        out += struct.pack("<I", len(self.dests))
        for d in self.dests:
            out += struct.pack("<Hffff", *d)
        out += struct.pack("<I", 0)
        out += struct.pack("<I", len(self.blobs))
        for role, flags, c, r, a in self.blobs:
            out += struct.pack("<BBfffffffff", role, flags, *c, *r, *a)
        target = MAPS / f"{map_id}.bin"
        target.write_bytes(bytes(out))
        print(f"wrote {target} ({len(out)} bytes): {len(self.brushes)} brushes, {len(self.triggers)} triggers, "
              f"{len(self.dests)} places, {len(self.blobs)} blobs")
