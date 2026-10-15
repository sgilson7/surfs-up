"""Read a Source model's collision (.phy): the convex pieces it is built of.

A .phy holds one or more solids, each an IVP compact surface (Havok's
predecessor, which Source's vphysics uses): a list of convex "ledges", each a
closed hull of triangles over a shared array of points. Points are in metres
in IVP's axes; Source's are inches with y and z swapped (IVP y points down):
source = (x, z, -y) / 0.0254.

Layout (Source SDK public/phyfile.h, vphysics' ivp_compact_surface.hxx and
ivp_compact_ledge.hxx; the same reading as SourceIO and Crowbar):
  phyheader_t: i32 size (16), i32 id, i32 solidCount, i32 checksum
  per solid: i32 byte size, then either
    "VPHY" i16 version, i16 modelType, i32 surfaceSize, f32 x3 dragAxisAreas,
      i32 axisMapSize (28 bytes), then the compact surface; or
    the compact surface itself (older models)
  IVP_Compact_Surface (48 bytes): f32 x3 mass centre, f32 x3 rotation
    inertia, f32 radius, i32 deviation:8 | byte_size:24,
    i32 offset_ledgetree_root, i32 x3 dummy ("IVPS")
  the ledges follow it; IVP_Compact_Ledge (16 bytes): i32 c_point_offset
    (from the ledge), i32 client data, u32 flags | size_div_16 << 8,
    i16 n_triangles, i16 unused; then per triangle 16 bytes: u32 indices,
    then three edges u32: start_point_index:16 | opposite:15 | virtual:1
  each point 16 bytes: f32 x y z, f32 unused
"""
import struct

INCH = 1.0 / 0.0254


def hulls(data):
    """Every convex piece of every solid, as a list of points (Source
    inches, model space) and its triangles (point indices)."""
    size, _id, count, _sum = struct.unpack_from("<iiii", data, 0)
    p = size
    out = []
    for _ in range(count):
        solid_size, = struct.unpack_from("<i", data, p)
        start = p + 4
        s = start
        if data[s:s + 4] == b"VPHY":
            s += 28
        out.extend(_surface(data, s))
        p = start + solid_size
    return out


def _surface(data, s):
    root, = struct.unpack_from("<i", data, s + 32)
    pieces = []
    # the ledge tree (IVP_Compact_Ledgetree_Node, 28 bytes: i32
    # offset_right_node, i32 offset_compact_ledge, f32 x4 sphere, u8 x4):
    # the left child follows its node; a leaf (no right child) names a ledge
    stack = [s + root]
    seen = set()
    while stack:
        node = stack.pop()
        right, ledge_off = struct.unpack_from("<ii", data, node)
        if right == 0:
            ledge = node + ledge_off
            if ledge not in seen:
                seen.add(ledge)
                pieces.append(_ledge(data, ledge))
        else:
            stack.append(node + 28)
            stack.append(node + right)
    return pieces


def _ledge(data, ledge):
    c_point, _client, _flags, ntri = struct.unpack_from("<iiIh", data, ledge)
    points_at = ledge + c_point
    tris = []
    for t in range(ntri):
        e = struct.unpack_from("<III", data, ledge + 16 + 16 * t + 4)
        tris.append([x & 0xFFFF for x in e])
    pts = {}
    for i in {i for tri in tris for i in tri}:
        x, y, z = struct.unpack_from("<fff", data, points_at + 16 * i)
        pts[i] = (x * INCH, z * INCH, -y * INCH)
    return pts, tris


def rotate(angles):
    """Source's AngleMatrix: the columns forward, left, up of a pitch, yaw,
    roll in degrees (mathlib.cpp)."""
    import math
    p, y, r = (math.radians(a) for a in angles)
    sp, cp, sy, cy, sr, cr = math.sin(p), math.cos(p), math.sin(y), math.cos(y), math.sin(r), math.cos(r)
    forward = (cp * cy, cp * sy, -sp)
    left = (sr * sp * cy - cr * sy, sr * sp * sy + cr * cy, sr * cp)
    up = (cr * sp * cy + sr * sy, cr * sp * sy - sr * cy, cr * cp)
    return forward, left, up


def place(point, origin, axes):
    f, l, u = axes
    return tuple(origin[k] + point[0] * f[k] + point[1] * l[k] + point[2] * u[k] for k in range(3))


def _cross(a, b):
    return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])


def _unit(v):
    n = (v[0] ** 2 + v[1] ** 2 + v[2] ** 2) ** 0.5
    return None if n < 1e-6 else (v[0] / n, v[1] / n, v[2] / n)


def planes(points, tris):
    """A convex piece as a brush: (normal, distance, bevel) per side, its
    faces outward, then the bevels Source's map compiler adds to a brush so
    a box swept against it stops where the solid is (axial planes at its
    bounds, and planes through each edge along each axis that every point
    lies behind), as vbsp's AddBrushBevels does."""
    pts = list(points.values())
    centre = tuple(sum(p[k] for p in pts) / len(pts) for k in range(3))
    out = []
    keys = set()

    def add(n, bevel):
        d = max(sum(n[k] * p[k] for k in range(3)) for p in pts)
        key = (round(n[0], 4), round(n[1], 4), round(n[2], 4))
        if key in keys:
            return
        keys.add(key)
        out.append((n, d, bevel))

    edges = set()
    for a, b, c in tris:
        n = _unit(_cross(tuple(points[b][k] - points[a][k] for k in range(3)),
                         tuple(points[c][k] - points[a][k] for k in range(3))))
        if n is None:
            continue
        if sum(n[k] * (centre[k] - points[a][k]) for k in range(3)) > 0:
            n = (-n[0], -n[1], -n[2])
        add(n, False)
        for i, j in ((a, b), (b, c), (c, a)):
            edges.add((min(i, j), max(i, j)))
    for k in range(3):
        for s in (1.0, -1.0):
            n = [0.0, 0.0, 0.0]
            n[k] = s
            add(tuple(n), True)
    for i, j in edges:
        e = tuple(points[j][k] - points[i][k] for k in range(3))
        for k in range(3):
            axis = [0.0, 0.0, 0.0]
            axis[k] = 1.0
            for s in (1.0, -1.0):
                n = _unit(_cross(e, axis))
                if n is None:
                    continue
                n = tuple(s * x for x in n)
                d = sum(n[m] * points[i][m] for m in range(3))
                if all(sum(n[m] * p[m] for m in range(3)) <= d + 0.01 for p in pts):
                    add(n, True)
    return out
