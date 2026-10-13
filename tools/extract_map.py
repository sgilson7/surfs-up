#!/usr/bin/env python3
"""Turn a Source map (.bsp) into the game's map file, data/maps/<id>.bin.

    python3 -I tools/extract_map.py <map.bsp> <id> [--floor Z]

surf_utopia:  python3 -I tools/extract_map.py surf_utopia_v3.bsp surf_utopia --floor -8250
surf_kitsune: python3 -I tools/extract_map.py surf_kitsune.bsp surf_kitsune

It copies the brushes (the convex solids the Source engine itself collides
the player against, bevel planes included), the trigger brushes, the
teleport destinations and the activator-name filters a map's secrets use.
It keeps no texture: each side carries a small hint (glass, light, black)
and the core picks the gymnasium look from the side's slope. What each
teleport means for a run (restart, finish, next stage) is not in the BSP;
it is in data/maps/maps.json.

What it keeps and drops (the rules for play are in the core; this only copies):
  - world brushes that block a player (solid, window, grate, player clip);
  - func_brush brushes: solid and drawn, unless their Solidity is "never"
    (then drawn only);
  - func_illusionary brushes (drawn, not solid);
  - trigger_teleport, trigger_push, and every trigger_multiple whose output
    renames its activator (AddOutput targetname), each with its filter's
    required name; triggers that start disabled are dropped;
  - nothing below --floor (surf_utopia's TF2 jail sits under the course);
  - water brushes (drawn as a ball pit, not solid: Source does not block on water).

Names are lower-cased: Source matches entity names without regard to case.

Format v2 (little endian), all counts u32:
  b"SURF" u32 2
  u32 brushes; per brush: u8 kind, u32 group, u16 sides; per side f32 nx ny nz d, u8 flags, u8 hint
     kind: 0 solid, 1 clip (solid, never drawn), 2 window (solid, see-through), 3 visual (not solid), 4 water
     flags: bit0 bevel, bit1 drawn;  hint: 0 wall, 1 glass, 2 light, 3 black, 4 metal, 5 water
  u32 names; per name: u8 len, bytes
  u32 triggers; per trigger: u8 kind, u16 a, u16 filter, f32 pitch yaw roll speed, u16 sides; per side f32 nx ny nz d
     kind 0 teleport (a: destination name), 1 push (a unused), 2 rename (a: the new name)
     filter: the name the activator must have, or 65535 for none
  u32 destinations; per destination: u16 name, f32 x y z yaw
"""
import re, struct, sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bsp  # noqa: E402

MAPS = Path(__file__).resolve().parents[1] / "data" / "maps"
NONE = 65535
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


def main(path, map_id, floor):
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
            out.append((pl[0:3], d, bool(s[3] & 0xFF), tex))
        return out, c

    def zmax(sides):
        polys = bsp.brush_polys(None, [(n, d, None) for n, d, _, _ in sides])
        return max((p[2] for _, _, _, poly in polys for p in poly), default=-1e9)

    brushes = []

    def add(bi, kind, group, origin=(0.0, 0.0, 0.0)):
        sides, c = sides_of(bi, origin)
        if zmax(sides) < floor:
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

    names = [""]

    def name(n):
        n = n.strip().lower()
        if n not in names:
            names.append(n)
        return names.index(n)

    filters = {e["targetname"].lower(): e for e in ents if e["classname"] == "filter_activator_name" and "targetname" in e}

    def required(e):
        f = e.get("filtername", "").lower()
        if not f:
            return NONE
        flt = filters.get(f)
        if flt is None:
            return NONE
        assert flt.get("Negated", "0") in ("0", "Allow entities that match criteria"), "negated filters are not handled"
        return name(flt.get("filtername", ""))

    triggers = []
    for e in ents:
        cls = e["classname"]
        if not e.get("model", "").startswith("*"):
            continue
        model = M[int(e["model"][1:])]
        origin = tuple(float(v) for v in e.get("origin", "0 0 0").split())
        mbr = sorted(bsp.model_brushes(B, model["headnode"]))
        if cls == "func_brush":
            # rendermode 4 is alpha-tested, which ignores renderamt: such a
            # brush is seen (surf_kitsune's stages are all func_brush "kitsune")
            kind = 3 if e.get("Solidity") == "1" else 0
            for bi in mbr:
                add(bi, kind, bi, origin)
            continue
        if cls == "func_illusionary":
            for bi in mbr:
                add(bi, 3, bi, origin)
            continue
        if e.get("StartDisabled", "0") == "1":
            continue
        entry = None
        if cls == "trigger_teleport":
            entry = (0, name(e.get("target", "")), required(e), (0.0, 0.0, 0.0, 0.0))
        elif cls == "trigger_push":
            pitch, yaw, _ = [float(x) for x in e.get("pushdir", "0 0 0").split()]
            entry = (1, NONE, required(e), (pitch, yaw, 0.0, float(e.get("speed", "40"))))
        elif cls == "trigger_multiple":
            outs = [v for k, v in re.findall(r'"([^"]*)"\s+"([^"]*)"', raw_block(B, e)) if k == "OnTrigger" or k == "OnStartTouch"]
            for v in outs:
                parts = v.split(",")
                if len(parts) >= 3 and parts[0] == "!activator" and parts[1] == "AddOutput" and parts[2].startswith("targetname"):
                    entry = (2, name(parts[2][len("targetname"):]), required(e), (0.0, 0.0, 0.0, 0.0))
        if entry is None:
            continue
        for bi in mbr:
            sides, _ = sides_of(bi, origin)
            if zmax(sides) < floor:
                continue
            triggers.append((*entry, sides))

    dests = []
    for e in ents:
        if e["classname"] == "info_teleport_destination" and "targetname" in e:
            x, y, z = [float(v) for v in e["origin"].split()]
            yaw = float(e.get("angles", "0 0 0").split()[1])
            dests.append((name(e["targetname"]), x, y, z, yaw))

    out = bytearray(b"SURF") + struct.pack("<I", 2)
    out += struct.pack("<I", len(brushes))
    for kind, group, sides in brushes:
        out += struct.pack("<BIH", kind, group, len(sides))
        for n, d, bevel, tex in sides:
            flags = (1 if bevel else 0) | (2 if (drawn(tex) and not bevel) else 0)
            out += struct.pack("<ffffBB", n[0], n[1], n[2], d, flags, hint_of(tex))
    out += struct.pack("<I", len(names))
    for t in names:
        b = t.encode()
        out += struct.pack("<B", len(b)) + b
    out += struct.pack("<I", len(triggers))
    for kind, a, flt, p, sides in triggers:
        out += struct.pack("<BHHffffH", kind, a, flt, *p, len(sides))
        for n, d, _, _ in sides:
            out += struct.pack("<ffff", n[0], n[1], n[2], d)
    out += struct.pack("<I", len(dests))
    for d in dests:
        out += struct.pack("<Hffff", *d)
    target = MAPS / f"{map_id}.bin"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(bytes(out))
    print(f"wrote {target} ({len(out)} bytes): {len(brushes)} brushes, {len(triggers)} triggers, {len(dests)} destinations")
    print("names:", names)


def raw_block(B, e):
    """The entity's text as written, so repeated keys (outputs) all survive."""
    text = B.lump(bsp.LUMP_ENTITIES).decode("latin-1")
    hid = e.get("hammerid")
    for block in re.findall(r"\{(.*?)\}", text, re.S):
        if hid and f'"hammerid" "{hid}"' in block:
            return block
    return ""


if __name__ == "__main__":
    args = sys.argv[1:]
    floor = -1e9
    if "--floor" in args:
        i = args.index("--floor")
        floor = float(args[i + 1])
        del args[i:i + 2]
    main(args[0], args[1], floor)
