#!/usr/bin/env python3
"""Turn surf_utopia_v3.bsp into data/maps/surf_utopia.bin.

    python3 -I tools/extract_map.py <path/to/surf_utopia_v3.bsp>

The map's geometry is Panzerhandschuh's surf_utopia_v3 (see LICENSES.md). This
script copies the brushes (the convex solids the Source engine itself collides
the player against, bevel planes included), the trigger brushes, and the start.
It keeps no texture: each side carries only a small hint (glass, light, black)
and the core picks the gymnasium look from the side's slope.

What it keeps and drops (the rules for play are in the core; this only copies):
  - world brushes that block a player (solid, window, grate, player clip);
  - func_brush brushes (solid) and func_illusionary brushes (drawn, not solid);
  - every trigger_teleport and trigger_push brush above the jail;
  - nothing below z = -8250, where the TF2 version keeps its jail;
  - water brushes (drawn as a flat colour, not solid; Source does not block on water).

Format (little endian), all counts u32:
  b"SURF" u32 version
  u32 brushes; per brush: u8 kind, u32 group, u16 sides; per side f32 nx ny nz d, u8 flags, u8 hint
     kind: 0 solid, 1 clip (solid, never drawn), 2 window (solid, see-through), 3 visual (not solid), 4 water
     flags: bit0 bevel, bit1 drawn
     hint: 0 wall, 1 glass, 2 light, 3 black, 4 metal, 5 water
  u32 triggers; per trigger: u8 kind, u8 target, f32 px py pz speed, u16 sides; per side f32 nx ny nz d
     kind: 0 teleport, 1 push
     target: index into the target name list below (teleports), 255 for pushes
  u32 targets; per target: u8 len, bytes
  f32 spawn x y z yaw
"""
import struct, sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bsp  # noqa: E402

OUT = Path(__file__).resolve().parents[1] / "data" / "maps" / "surf_utopia.bin"
FLOOR = -8250.0
CONTENTS_SOLID, CONTENTS_WINDOW, CONTENTS_GRATE = 0x1, 0x2, 0x8
CONTENTS_WATER, CONTENTS_PLAYERCLIP, CONTENTS_MONSTERCLIP = 0x20, 0x10000, 0x20000


def hint_of(tex):
    t = tex.upper()
    if "GLASS" in t:
        return 1
    if t.startswith("LIGHTS/"):
        return 2
    if t == "TOOLS/TOOLSBLACK":
        return 3
    if t.startswith("METAL/"):
        return 4
    if "WATER" in t:
        return 5
    return 0


def drawn(tex):
    t = tex.upper()
    return not t.startswith("TOOLS/") or t == "TOOLS/TOOLSBLACK"


def main(path):
    B = bsp.Bsp(path)
    P, BR, BS, TI, M = B.planes(), B.brushes(), B.brushsides(), B.texnames(), B.models()
    ents = B.entities()

    def sides_of(bi, origin=(0.0, 0.0, 0.0)):
        # A brush entity's brushes are stored about its origin; move them to world space.
        f, n, c = BR[bi]
        out = []
        for s in BS[f:f + n]:
            pl = P[s[0]]
            tex = TI[s[1]][1][0] if s[1] >= 0 else "TOOLS/TOOLSNODRAW"
            d = pl[3] + sum(pl[k] * origin[k] for k in range(3))
            out.append((pl[0:3], d, bool(s[3]), tex))
        return out, c

    def zmax(sides):
        polys = bsp.brush_polys(None, [(n, d, None) for n, d, _, _ in sides])
        return max((p[2] for _, _, _, poly in polys for p in poly), default=-1e9)

    brushes = []

    def add(bi, kind, group, origin=(0.0, 0.0, 0.0)):
        sides, c = sides_of(bi, origin)
        if zmax(sides) < FLOOR:
            return
        brushes.append((kind, group, sides))

    for bi in sorted(bsp.model_brushes(B, M[0]["headnode"])):
        c = BR[bi][2]
        if c & CONTENTS_WATER:
            add(bi, 4, bi)
        elif c & CONTENTS_WINDOW:
            add(bi, 2, bi)
        elif c & (CONTENTS_SOLID | CONTENTS_GRATE):
            add(bi, 0, bi)
        elif c & CONTENTS_PLAYERCLIP:
            add(bi, 1, bi)

    targets, triggers = [], []
    for e in ents:
        cls = e["classname"]
        if not e.get("model", "").startswith("*"):
            continue
        model = M[int(e["model"][1:])]
        origin = tuple(float(v) for v in e.get("origin", "0 0 0").split())
        mbr = sorted(bsp.model_brushes(B, model["headnode"]))
        if cls in ("func_brush", "func_illusionary"):
            for bi in mbr:
                add(bi, 0 if cls == "func_brush" else 3, bi, origin)
        elif cls in ("trigger_teleport", "trigger_push"):
            for bi in mbr:
                sides, _ = sides_of(bi, origin)
                if zmax(sides) < FLOOR:
                    continue
                if cls == "trigger_teleport":
                    name = e.get("target", "")
                    if name not in targets:
                        targets.append(name)
                    triggers.append((0, targets.index(name), (0, 0, 0, 0), sides))
                else:
                    pitch, yaw, _ = [float(x) for x in e.get("pushdir", "0 0 0").split()]
                    triggers.append((1, 255, (pitch, yaw, 0, float(e.get("speed", "40"))), sides))

    start = [e for e in ents if e["classname"] == "info_teleport_destination" and e.get("targetname") == "start"][0]
    sx, sy, sz = [float(v) for v in start["origin"].split()]
    syaw = float(start.get("angles", "0 0 0").split()[1])

    out = bytearray(b"SURF") + struct.pack("<I", 1)
    out += struct.pack("<I", len(brushes))
    for kind, group, sides in brushes:
        out += struct.pack("<BIH", kind, group, len(sides))
        for n, d, bevel, tex in sides:
            flags = (1 if bevel else 0) | (2 if (drawn(tex) and not bevel) else 0)
            out += struct.pack("<ffffBB", n[0], n[1], n[2], d, flags, hint_of(tex))
    out += struct.pack("<I", len(triggers))
    for kind, target, p, sides in triggers:
        out += struct.pack("<BBffffH", kind, target, *p, len(sides))
        for n, d, _, _ in sides:
            out += struct.pack("<ffff", n[0], n[1], n[2], d)
    out += struct.pack("<I", len(targets))
    for t in targets:
        b = t.encode()
        out += struct.pack("<B", len(b)) + b
    out += struct.pack("<ffff", sx, sy, sz, syaw)
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_bytes(bytes(out))
    print(f"wrote {OUT} ({len(out)} bytes): {len(brushes)} brushes, {len(triggers)} triggers, targets {targets}")


if __name__ == "__main__":
    main(sys.argv[1])
