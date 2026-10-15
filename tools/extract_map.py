#!/usr/bin/env python3
"""Turn a Source map (.bsp) into the game's map file, data/maps/<id>.bin.

    python3 -I tools/extract_map.py <map.bsp> <id> [--floor Z]

surf_utopia:  python3 -I tools/extract_map.py surf_utopia_v3.bsp surf_utopia --floor -8250
surf_kitsune: python3 -I tools/extract_map.py surf_kitsune.bsp surf_kitsune
surf_egypt:   python3 -I tools/extract_map.py surf_egypt.bsp surf_egypt --player-start
surf_aircontrol: python3 -I tools/extract_map.py surf_AirControl_v3.bsp surf_aircontrol --player-start
Every other course: python3 -I tools/extract_map.py <map.bsp> <id>

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
  - water brushes (drawn as a ball pit, not solid: Source does not block on water);
  - displacements (curved terrain, solid in Source), as their grids: the
    core makes each triangle a thin solid wedge under it (its top drawn,
    4 units deep, with axial bevels), so its brush collision holds them;
  - func_wall brushes (solid and drawn);
  - func_door brushes (solid and drawn where they rest: a bhop map's
    blocks, which the timer servers hold still);
  - trigger_hurt (Source's damage triggers: what kills a fall on old maps);
  - a func_button fired by shooting it whose output renames the activator
    (bhop_lego's boost pads): touching its pad renames;
  - no trigger filtered by a filter_multi (bhop_lego's anti-stand teleports).

Names are lower-cased: Source matches entity names without regard to case.

Format v3 (little endian), all counts u32:
  b"SURF" u32 3
  u32 brushes; per brush: u8 kind, u32 group, u16 sides; per side f32 nx ny nz d, u8 flags, u8 hint
     kind: 0 solid, 1 clip (solid, never drawn), 2 window (solid, see-through), 3 visual (not solid), 4 water
     flags: bit0 bevel, bit1 drawn;  hint: 0 wall, 1 glass, 2 light, 3 black, 4 metal, 5 water, 6 terrain (a displacement's top),
       16 + k a neon strip and 32 + k a grid, in colour k of COLOURS (kitsune's stage colours)
  u32 names; per name: u8 len, bytes
  u32 triggers; per trigger: u8 kind, u16 a, u16 filter, f32 pitch yaw roll speed, u16 sides; per side f32 nx ny nz d
     kind 0 teleport (a: destination name), 1 push (a unused), 2 rename (a: the new name),
       3 hurt (a unused, speed: damage a second)
     filter: the name the activator must have, or 65535 for none
  u32 destinations; per destination: u16 name, f32 x y z yaw
  u32 displacements; per displacement: u8 power, f32 face normal xyz, then
     (2^power + 1)^2 points f32 xyz, row by row from the corner nearest its
     start position (the core builds the solid wedges from them)
"""
import re, struct, sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bsp  # noqa: E402

MAPS = Path(__file__).resolve().parents[1] / "data" / "maps"
NONE = 65535
CONTENTS_SOLID, CONTENTS_WINDOW, CONTENTS_GRATE = 0x1, 0x2, 0x8
CONTENTS_WATER, CONTENTS_PLAYERCLIP, CONTENTS_MONSTERCLIP = 0x20, 0x10000, 0x20000


# kitsune's stage colours, as its NEONS/NEON_<colour> and GRIDS/GRID_<colour> name them
COLOURS = ["RED", "ORANGE", "YELLOW", "GREEN", "TEAL", "BLUE", "PURPLE", "PINK", "WHITE"]


def colour_of(t, prefix):
    rest = t[len(prefix):].split("_")[0]
    return COLOURS.index(rest) if rest in COLOURS else None


def hint_of(tex):
    t = tex.upper()
    if t.startswith("NEONS/NEON_") and colour_of(t, "NEONS/NEON_") is not None:
        return 16 + colour_of(t, "NEONS/NEON_")
    if t.startswith("GRIDS/GRID_") and colour_of(t, "GRIDS/GRID_") is not None:
        return 32 + colour_of(t, "GRIDS/GRID_")
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
    if t.startswith("TERRAIN:"):
        return 6
    return 0


def drawn(tex):
    t = tex.upper()
    return not t.startswith("TOOLS/") or t == "TOOLS/TOOLSBLACK"


def main(path, map_id, floor, player_start=False):
    B = bsp.Bsp(path)
    P, BR, BS, TI, M = B.planes(), B.brushes(), B.brushsides(), B.texnames(), B.models()
    ents = B.entities()

    # a brush face a displacement was built on is replaced by the
    # displacement: it is not drawn (it would lie on the sand and flicker)
    def plane_key(pl):
        return (round(pl[0], 3), round(pl[1], 3), round(pl[2], 3), round(pl[3]))

    disp_planes = {(plane_key(P[d[4]]), d[0].upper()) for d in B.displacements()}

    def sides_of(bi, origin=(0.0, 0.0, 0.0)):
        # A brush entity's brushes are stored about its origin; move them to world space.
        f, n, c = BR[bi]
        out = []
        for s in BS[f:f + n]:
            pl = P[s[0]]
            tex = TI[s[1]][1][0] if s[1] >= 0 else "TOOLS/TOOLSNODRAW"
            if (plane_key(pl), tex.upper()) in disp_planes:
                tex = "TOOLS/TOOLSNODRAW"
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

    # displacements: their grids, which the core turns into thin solid
    # wedges (one per triangle, its top the surface)
    terrain = []
    for tex, contents, face_n, tris, _, power, grid in B.displacements():
        if not contents & CONTENTS_SOLID and contents != 0:
            continue
        if max(p[2] for p in grid) < floor:
            continue
        terrain.append((power, face_n, grid))

    names = [""]

    def name(n):
        n = n.strip().lower()
        if n not in names:
            names.append(n)
        return names.index(n)

    filters = {e["targetname"].lower(): e for e in ents if e["classname"] == "filter_activator_name" and "targetname" in e}
    multis = {e["targetname"].lower() for e in ents if e["classname"] == "filter_multi" and "targetname" in e}

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
        if cls == "func_wall":
            # a func_wall is a solid, drawn brush (surf_aircontrol's)
            for bi in mbr:
                add(bi, 0, bi, origin)
            continue
        if cls == "func_door":
            # a bhop map's blocks are func_doors that sink when stood on; the
            # timer servers hold them still, and so does this: solid and
            # drawn where they rest (bhop_eazy's, bhop_cartoons')
            for bi in mbr:
                add(bi, 0, bi, origin)
            continue
        if cls == "func_illusionary":
            for bi in mbr:
                add(bi, 3, bi, origin)
            continue
        if e.get("StartDisabled", "0") == "1":
            continue
        if e.get("filtername", "").lower() in multis:
            # a filter_multi passes any of several names: bhop_lego's
            # anti-stand teleports on every block (named as you land, fired
            # if you stay). Its blocks are held still, like a bhop map's
            # doors, so these are left out; the floor under them still
            # sends a miss back to its checkpoint.
            continue
        entry = None
        if cls == "trigger_teleport":
            # a teleport with no target does nothing in Source (surf_egypt's
            # destination markers are targetless teleports)
            if not e.get("target", "").strip():
                continue
            entry = (0, name(e.get("target", "")), required(e), (0.0, 0.0, 0.0, 0.0))
        elif cls == "trigger_hurt":
            # damage a second; the core deals it as Source's HurtThink does
            entry = (3, NONE, required(e), (0.0, 0.0, 0.0, float(e.get("damage", "10"))))
        elif cls == "trigger_push":
            pitch, yaw, _ = [float(x) for x in e.get("pushdir", "0 0 0").split()]
            entry = (1, NONE, required(e), (pitch, yaw, 0.0, float(e.get("speed", "40"))))
        elif cls == "func_button" and int(e.get("spawnflags", "0")) & 512:
            # a button shot to fire it (bhop_lego's boost pads, at the foot of
            # each lift): with nothing to shoot, touching its pad fires it
            outs = [v for k, v in re.findall(r'"([^"]*)"\s+"([^"]*)"', raw_block(B, e)) if k == "OnDamaged"]
            for v in outs:
                parts = v.split(",")
                at_once = len(parts) < 4 or float(parts[3] or 0) == 0
                if len(parts) >= 3 and parts[0] == "!activator" and parts[1] == "AddOutput" and parts[2].startswith("targetname") and at_once:
                    entry = (2, name(parts[2][len("targetname"):]), NONE, (0.0, 0.0, 0.0, 0.0))
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
    # Source sends a teleport to any entity of its target's name, not only an
    # info_teleport_destination (surf_egypt aims at other teleports' origins)
    known = {d[0] for d in dests}
    for e in ents:
        if e["classname"] != "trigger_teleport" or not e.get("target", "").strip():
            continue
        t = name(e["target"])
        if t in known:
            continue
        for o in ents:
            if o.get("targetname", "").strip().lower() == names[t] and "origin" in o:
                x, y, z = [float(v) for v in o["origin"].split()]
                yaw = float(o.get("angles", "0 0 0").split()[1])
                dests.append((t, x, y, z, yaw))
                known.add(t)
                break
    # --player-start: the first terrorist spawn as a destination named
    # player_start, for a course that starts where the map spawns players
    if player_start:
        for e in ents:
            if e["classname"] == "info_player_terrorist":
                x, y, z = [float(v) for v in e["origin"].split()]
                yaw = float(e.get("angles", "0 0 0").split()[1])
                dests.append((name("player_start"), x, y, z, yaw))
                break

    out = bytearray(b"SURF") + struct.pack("<I", 3)
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
    out += struct.pack("<I", len(terrain))
    for power, face_n, grid in terrain:
        out += struct.pack("<Bfff", power, *face_n)
        for p in grid:
            out += struct.pack("<fff", *p)
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
    player_start = "--player-start" in args
    if player_start:
        args.remove("--player-start")
    main(args[0], args[1], floor, player_start)
