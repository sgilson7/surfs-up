"""Read a Source engine BSP (v20) far enough to recover brushes, planes,
texture names, models, entities and displacement surfaces. Pure reading; no
geometry decisions."""
import struct, re

LUMP_ENTITIES, LUMP_PLANES, LUMP_TEXDATA, LUMP_VERTEXES = 0, 1, 2, 3
LUMP_TEXINFO, LUMP_FACES, LUMP_EDGES, LUMP_SURFEDGES, LUMP_MODELS = 6, 7, 12, 13, 14
LUMP_BRUSHES, LUMP_BRUSHSIDES = 18, 19
LUMP_DISPINFO, LUMP_DISP_VERTS = 26, 33
LUMP_TEXDATA_STRING_DATA, LUMP_TEXDATA_STRING_TABLE = 43, 44


class Bsp:
    def __init__(self, path):
        self.raw = open(path, "rb").read()
        ident, self.version = struct.unpack_from("<4si", self.raw, 0)
        assert ident == b"VBSP", ident
        self.lumps = [struct.unpack_from("<iii4s", self.raw, 8 + 16 * i) for i in range(64)]

    def lump(self, i):
        off, ln, ver, cc = self.lumps[i]
        data = self.raw[off:off + ln]
        assert data[:4] != b"LZMA", "compressed lump"
        return data

    def entities(self):
        text = self.lump(LUMP_ENTITIES).decode("latin-1")
        out = []
        for block in re.findall(r"\{(.*?)\}", text, re.S):
            out.append({k: v for k, v in re.findall(r'"([^"]*)"\s+"([^"]*)"', block)})
        return out

    def planes(self):
        d = self.lump(LUMP_PLANES)
        return [struct.unpack_from("<ffffi", d, i) for i in range(0, len(d), 20)]

    def brushes(self):
        d = self.lump(LUMP_BRUSHES)
        return [struct.unpack_from("<iii", d, i) for i in range(0, len(d), 12)]

    def brushsides(self):
        d = self.lump(LUMP_BRUSHSIDES)
        # planenum (u16), texinfo (i16), dispinfo (i16), bevel (i16)
        return [struct.unpack_from("<Hhhh", d, i) for i in range(0, len(d), 8)]

    def texnames(self):
        sd = self.lump(LUMP_TEXDATA_STRING_DATA)
        st = self.lump(LUMP_TEXDATA_STRING_TABLE)
        table = [struct.unpack_from("<i", st, i)[0] for i in range(0, len(st), 4)]
        names = [sd[o:sd.index(b"\0", o)].decode("latin-1") for o in table]
        td = self.lump(LUMP_TEXDATA)
        texdata = []
        for i in range(0, len(td), 32):
            rx, ry, rz, sid, w, h, vw, vh = struct.unpack_from("<fffiiiii", td, i)
            texdata.append((names[sid], (rx, ry, rz)))
        ti = self.lump(LUMP_TEXINFO)
        texinfo = []
        for i in range(0, len(ti), 72):
            flags, tdi = struct.unpack_from("<ii", ti, i + 64)
            texinfo.append((flags, texdata[tdi] if tdi >= 0 else ("", (0, 0, 0))))
        return texinfo

    def models(self):
        d = self.lump(LUMP_MODELS)
        out = []
        for i in range(0, len(d), 48):
            v = struct.unpack_from("<fffffffffiii", d, i)
            out.append({"mins": v[0:3], "maxs": v[3:6], "origin": v[6:9], "headnode": v[9], "firstface": v[10], "numfaces": v[11]})
        return out


    def displacements(self):
        """Every displacement as (texture, contents, face normal, triangles,
        the base face's plane number),
        triangles in world space, built as Source's CCoreDispInfo builds them:
        the base face's corners from the one nearest startPosition, the grid
        interpolated across them, each vertex moved by its offset vector times
        its distance, and each grid square split the way the engine splits it
        (alternating with the vertex index)."""
        di = self.lump(LUMP_DISPINFO)
        dv = self.lump(LUMP_DISP_VERTS)
        faces = self.lump(LUMP_FACES)
        verts = self.lump(LUMP_VERTEXES)
        edges = self.lump(LUMP_EDGES)
        surf = self.lump(LUMP_SURFEDGES)
        planes = self.planes()
        texinfo = self.texnames()
        out = []
        for k in range(0, len(di), 176):
            start = struct.unpack_from("<fff", di, k)
            vstart, = struct.unpack_from("<i", di, k + 12)
            power, = struct.unpack_from("<i", di, k + 20)
            contents, = struct.unpack_from("<i", di, k + 32)
            mapface, = struct.unpack_from("<H", di, k + 36)
            f = mapface * 56
            planenum, side, _, firstedge, numedges, tinfo = struct.unpack_from("<HBBihh", faces, f)
            corners = []
            for j in range(numedges):
                se, = struct.unpack_from("<i", surf, (firstedge + j) * 4)
                a, b = struct.unpack_from("<HH", edges, abs(se) * 4)
                corners.append(struct.unpack_from("<fff", verts, (a if se >= 0 else b) * 12))
            if len(corners) != 4:
                continue
            near = min(range(4), key=lambda i: sum((corners[i][c] - start[c]) ** 2 for c in range(3)))
            c0, c1, c2, c3 = [corners[(near + i) % 4] for i in range(4)]
            n = (1 << power) + 1
            grid = []
            for y in range(n):
                t = y / (n - 1)
                left = [c0[c] + (c1[c] - c0[c]) * t for c in range(3)]
                right = [c3[c] + (c2[c] - c3[c]) * t for c in range(3)]
                for x in range(n):
                    u = x / (n - 1)
                    vx, vy, vz, dist, _ = struct.unpack_from("<fffff", dv, (vstart + y * n + x) * 20)
                    p = [left[c] + (right[c] - left[c]) * u for c in range(3)]
                    grid.append((p[0] + vx * dist, p[1] + vy * dist, p[2] + vz * dist))
            tris = []
            for y in range(n - 1):
                for x in range(n - 1):
                    i = y * n + x
                    if i % 2:
                        tris += [(grid[i], grid[i + n], grid[i + 1]), (grid[i + 1], grid[i + n], grid[i + n + 1])]
                    else:
                        tris += [(grid[i], grid[i + n], grid[i + n + 1]), (grid[i], grid[i + n + 1], grid[i + 1])]
            pn = planes[planenum][0:3]
            normal = tuple(-c for c in pn) if side else tuple(pn)
            tex = texinfo[tinfo][1][0] if tinfo >= 0 else ""
            out.append((tex, contents, normal, tris, planenum))
        return out


LUMP_NODES, LUMP_LEAFS, LUMP_LEAFBRUSHES = 5, 10, 17


def model_brushes(b, headnode):
    """Brush indices reachable from a model's head node."""
    nodes = b.lump(LUMP_NODES)
    leafs = b.lump(LUMP_LEAFS)
    lb = b.lump(LUMP_LEAFBRUSHES)
    leafsize = 56 if b.version <= 19 else 32
    out, stack = set(), [headnode]
    while stack:
        n = stack.pop()
        if n >= 0:
            pl, c0, c1 = struct.unpack_from("<iii", nodes, n * 32)
            stack += [c0, c1]
        else:
            leaf = -1 - n
            o = leaf * leafsize
            first, num = struct.unpack_from("<HH", leafs, o + 24)
            for k in range(first, first + num):
                out.add(struct.unpack_from("<H", lb, k * 2)[0])
    return out


def brush_polys(planes, sides):
    """Polygon (list of xyz) for each side of a convex brush, from its planes.
    `sides` is a list of (normal, dist, tag). Returns [(tag, normal, dist, poly)]."""
    out = []
    for i, (n, d, tag) in enumerate(sides):
        poly = base_winding(n, d)
        for j, (n2, d2, _) in enumerate(sides):
            if i == j or not poly:
                continue
            poly = clip(poly, n2, d2)
        if poly and len(poly) >= 3:
            out.append((tag, n, d, poly))
    return out


def base_winding(n, d):
    ax = max(range(3), key=lambda k: abs(n[k]))
    up = (0, 0, 1) if ax != 2 else (1, 0, 0)
    dot = sum(up[k] * n[k] for k in range(3))
    u = [up[k] - n[k] * dot for k in range(3)]
    ln = sum(x * x for x in u) ** 0.5
    u = [x / ln for x in u]
    r = [u[1] * n[2] - u[2] * n[1], u[2] * n[0] - u[0] * n[2], u[0] * n[1] - u[1] * n[0]]
    S = 65536.0
    o = [n[k] * d for k in range(3)]
    return [[o[k] - r[k] * S + u[k] * S for k in range(3)], [o[k] + r[k] * S + u[k] * S for k in range(3)],
            [o[k] + r[k] * S - u[k] * S for k in range(3)], [o[k] - r[k] * S - u[k] * S for k in range(3)]]


def clip(poly, n, d, eps=0.01):
    """Keep the part of poly behind plane (n.x <= d)."""
    out = []
    m = len(poly)
    ds = [sum(p[k] * n[k] for k in range(3)) - d for p in poly]
    for i in range(m):
        p, q = poly[i], poly[(i + 1) % m]
        a, bq = ds[i], ds[(i + 1) % m]
        if a <= eps:
            out.append(p)
        if (a < -eps and bq > eps) or (a > eps and bq < -eps):
            t = a / (a - bq)
            out.append([p[k] + (q[k] - p[k]) * t for k in range(3)])
    return out
