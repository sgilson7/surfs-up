"""surf_100_leagues: an undersea scuba adventure, by Sami (briefs/surf_100_leagues.md).

From the shore, where friends are suiting up round a barbecue, into the
water: sand dunes, a coral reef, a big turtle, a second reef, a long drop
into the dark hadal zone, anglerfish, oil platforms to b-hop across, a
geothermal vent that blasts you out of the sea, a shark, a container
ship, a whale, and up a fishing line onto a fishing boat's deck.

Built by tools/make_course.py surf_100_leagues; the pieces are
tools/coursekit.py's, the creatures tools/themes/sealife.py's. Distances in
units (a surfer is 72 tall); the sea's surface is at z = 0.
"""
import math

import sealife as sea

ID = "surf_100_leagues"

# palette roles, in order (maps.json's theme lists their colours the same way)
PALETTE = [
    ("water", "#3fb3d6"),
    ("sand", "#f2d39a"),
    ("sand_dark", "#d9b77a"),
    ("coral_pink", "#ff7f9e"),
    ("coral_orange", "#ff9b54"),
    ("coral_purple", "#b07cf0"),
    ("coral_yellow", "#ffd45c"),
    ("anemone", "#ff6fb5"),
    ("sea_green", "#59c9a5"),
    ("turtle_shell", "#5aa35a"),
    ("turtle_skin", "#a8d98f"),
    ("fish_gold", "#ffb238"),
    ("fish_blue", "#4aa8ff"),
    ("white", "#ffffff"),
    ("ink", "#2b2140"),
    ("hadal_rock", "#3a3550"),
    ("angler", "#6a4f8f"),
    ("lure", "#fff07a"),
    ("oil_yellow", "#ffc93c"),
    ("oil_steel", "#8a8fa3"),
    ("vent_rock", "#4a3b3b"),
    ("ember", "#ff6a3d"),
    ("shark", "#8fa3b8"),
    ("shark_belly", "#eef3f8"),
    ("hull_red", "#d6453d"),
    ("hull_navy", "#2f4a7a"),
    ("container_a", "#e94f37"),
    ("container_b", "#3f88c5"),
    ("container_c", "#44bba4"),
    ("container_d", "#f6ae2d"),
    ("whale", "#5b7fa6"),
    ("whale_belly", "#d4e3f0"),
    ("boat_white", "#f7f3e8"),
    ("boat_wood", "#b07a4a"),
    ("line", "#f0e6d0"),
    ("bubble", "#dff6ff"),
    ("palm", "#4caf50"),
    ("trunk", "#9c6b3f"),
    ("grill", "#3a3a3a"),
    ("kelp", "#3f9a5a"),
    ("buoy", "#ff8a3d"),
    ("rust", "#b5653a"),
    ("deep_floor", "#1f2a44"),
]

# the fog by the eye's height: above the water, then deeper and darker
FOG = [
    [64, "#bfe9ff", 60000],
    [1, "#bfe9ff", 60000],
    [0, "#55c6dd", 20000],
    [-1500, "#2f97c4", 17000],
    [-3200, "#165f8f", 13000],
    [-5200, "#0c2a52", 10000],
    [-7500, "#07122a", 8000],
]
WATER = 0.0

RIDGE_H = 512.0


class Chain:
    """Ridges one after another: each starts `gap` past the last one's end,
    `gap_drop` lower, along the heading the last one ended on."""

    def __init__(self, k, at, yaw):
        self.k = k
        self.x, self.y, self.z = at
        self.yaw = yaw

    def ridge(self, length, drop, role, turn=0.0, angle=52.0, height=RIDGE_H, sides="both", under=None, gap=500.0,
              gap_drop=200.0):
        n = max(1, math.ceil(abs(turn) / 2.0)) if turn else 1
        st = [(self.x, self.y, self.z, self.yaw)]
        x, y = self.x, self.y
        for i in range(n):
            mid = self.yaw + turn * (i + 0.5) / n
            r = math.radians(mid)
            x += math.cos(r) * length / n
            y += math.sin(r) * length / n
            st.append((x, y, self.z - drop * (i + 1) / n, self.yaw + turn * (i + 1) / n))
        self.k.ridge(st, role, angle=angle, height=height, sides=sides, under=under)
        start = (self.x, self.y, self.z)
        end = (x, y, self.z - drop)
        self.yaw += turn
        r = math.radians(self.yaw)
        self.x = x + math.cos(r) * gap
        self.y = y + math.sin(r) * gap
        self.z = self.z - drop - gap_drop
        return {"start": start, "end": end, "stations": st, "yaw": self.yaw, "height": height}


def forward(p, yaw, d, dz=0.0, side=0.0):
    r = math.radians(yaw)
    return (p[0] + math.cos(r) * d - math.sin(r) * side, p[1] + math.sin(r) * d + math.cos(r) * side, p[2] + dz)


def catch_under(k, piece, to, below=900.0, above=250.0, pad=1400.0, ahead=0.0):
    """A fall under a piece: a box spanning its stations (and `ahead` past
    its end), from its faces' foot down."""
    xs = [s[0] for s in piece["stations"]]
    ys = [s[1] for s in piece["stations"]]
    end = forward(piece["end"], piece["yaw"], ahead)
    xs.append(end[0])
    ys.append(end[1])
    base = min(s[2] for s in piece["stations"]) - piece["height"]
    k.catch((min(xs) - pad, min(ys) - pad, base - below), (max(xs) + pad, max(ys) + pad, base - above), to)


def drop_in(k, name, piece, rise=450.0):
    """A drop-in point: a buoy over the start of a piece, facing along it;
    step off its front onto the ridge."""
    s = piece["stations"][0]
    yaw = s[3]
    c = forward((s[0], s[1], s[2]), yaw, -60)
    top = s[2] + rise
    k.oval(c, 200, 200, top - 40, top, "buoy", yaw=yaw, side="white")
    k.dest(name, (c[0], c[1], top + 1), yaw)
    for i in range(3):
        k.blob("white", (c[0], c[1], top - 60 - i * 70), (150 - i * 30, 150 - i * 30, 30))


def build(k):
    # ---- the beach -------------------------------------------------------
    beach_top = 256.0
    k.box((-1500, -800, beach_top - 96), (400, 800, beach_top), "sand_dark", top="sand")
    k.dest("start", (-1100, 0, beach_top + 1), 0.0)
    start_zone = [[-1500, -800, beach_top], [400, 800, beach_top + 200]]
    # friends suiting up round the barbecue, palms, the sea ahead
    for i, (x, y, yaw) in enumerate([(-1350, 520, -60), (-1180, 620, -90), (-950, 600, -110), (-1380, -480, 60),
                                     (-1150, -600, 80), (-820, -560, 100)]):
        k.dest(f"pal{i + 1}", (x, y, beach_top + 1), yaw)
    sea.grill(k, (-1080, 470, beach_top))
    sea.grill(k, (-1000, -470, beach_top))
    for i, (x, y) in enumerate([(-1440, 760), (-600, 740), (-1450, -760), (-500, -720), (200, 760), (240, -740)]):
        sea.palm(k, (x, y, beach_top), 420 + (i % 3) * 60, 30 + i)

    # ---- leg A: sand dunes and the first reef, along +x --------------------
    a = Chain(k, (300, 0, 180), 0.0)
    dunes = [a.ridge(3200, 350, "sand", under="sand_dark"),
             a.ridge(3000, 350, "sand", turn=14, under="sand_dark"),
             a.ridge(3000, 350, "sand", turn=-14, under="sand_dark")]
    reef1 = [a.ridge(2800, 350, "coral_pink", under="coral_purple"),
             a.ridge(2800, 350, "coral_orange", under="coral_pink")]
    # ---- the turtle, curving left, and the second reef, curving back --------
    tur = a.ridge(3800, 350, "turtle_shell", turn=90, height=600, under="turtle_skin")
    reef2 = [a.ridge(3600, 350, "coral_purple", turn=90, under="coral_pink"),
             a.ridge(3000, 350, "coral_yellow", under="coral_orange", gap=1200, gap_drop=1300)]
    # ---- the drop into the hadal zone: anglerfish, then the speedy ramp ------
    anglers = [a.ridge(3400, 500, "angler", height=760, angle=54, under="hadal_rock"),
               a.ridge(3000, 450, "angler", turn=-8, under="hadal_rock"),
               a.ridge(3000, 450, "angler", turn=8, under="hadal_rock", gap=450, gap_drop=150)]
    speedy = a.ridge(4200, 1000, "oil_steel", height=640, angle=55, under="hadal_rock", gap=380, gap_drop=700)

    # ---- the oil platforms: b-hop across ---------------------------------------
    yaw = a.yaw
    deck_z = a.z
    p = (a.x, a.y, deck_z)
    decks = []
    for i, (length, gap) in enumerate([(1500, 440), (1200, 440), (1300, 0)]):
        c = forward(p, yaw, length / 2)
        k.oval(c, length / 2, 420, deck_z - 64, deck_z, "oil_yellow", yaw=yaw, side="rust")
        decks.append((c, length))
        p = forward(p, yaw, length + gap)
    floor_z = deck_z - 700

    # ---- the geothermal vent: a rock chimney you run into, which blasts you
    # straight up and out of the sea (its back wall takes your speed, so every
    # blast flies the same way) ----------------------------------------------
    inner = 450.0
    vc = forward(p, yaw, inner + 40)
    v_lo_z, v_hi_z = deck_z - 64, deck_z + 3400
    f = math.radians(yaw)
    fx, fy = math.cos(f), math.sin(f)
    # walls: back (far side), left, right, and the front above the doorway
    wall = 160.0

    def slab_along(d0, d1, s0, s1, z0, z1, role):
        pts = []
        for d in (d0, d1):
            for s in (s0, s1):
                q = forward(vc, yaw, d, side=s)
                pts.append((q[0], q[1], z0))
                pts.append((q[0], q[1], z1))
        k.hull(pts, role)

    slab_along(inner, inner + wall, -inner - wall, inner + wall, v_lo_z, v_hi_z, "vent_rock")
    slab_along(-inner - wall, inner + wall, inner, inner + wall, v_lo_z, v_hi_z, "vent_rock")
    slab_along(-inner - wall, inner + wall, -inner - wall, -inner, v_lo_z, v_hi_z, "vent_rock")
    slab_along(-inner - wall, -inner, -inner, inner, deck_z + 700, v_hi_z, "vent_rock")
    k.oval(vc, inner + 40, inner + 40, v_lo_z - 64, v_lo_z, "vent_rock", yaw=yaw)
    # the push: up, and a little on along the course
    up, on = 3000.0, 700.0
    k.vent((vc[0] - inner, vc[1] - inner, v_lo_z), (vc[0] + inner, vc[1] + inner, v_hi_z),
           -math.degrees(math.atan2(up, on)), yaw, math.hypot(up, on))
    sea.rift(k, (vc[0], vc[1], v_lo_z), inner * 1.6, yaw, 7)
    sea.bubbles(k, (vc[0], vc[1], v_lo_z), v_hi_z - v_lo_z + 2200, 26, 9)

    # ---- leg C, above the water: shark, container ship, whale, fishing boat --
    c_start = forward(vc, yaw, 3200)
    c = Chain(k, (c_start[0], c_start[1], 1800), yaw)
    shk = c.ridge(4800, 450, "shark", height=560, under="shark_belly", gap=500, gap_drop=150)
    hull = c.ridge(3400, 300, "hull_red", sides="left", height=700, angle=56, under="hull_navy", gap=380, gap_drop=0)
    # the ship's funnel steam lifts you onto the containers
    steam_at = forward((c.x, c.y, c.z), c.yaw, -190)
    k.vent((steam_at[0] - 320, steam_at[1] - 320, c.z - 900), (steam_at[0] + 320, steam_at[1] + 320, c.z + 200),
           -90.0, 0.0, 3600.0)
    sea.bubbles(k, (steam_at[0], steam_at[1], c.z - 900), 1400, 12, 21, role="white")
    c.z += 380
    boxes = []
    for i, role in enumerate(["container_a", "container_b", "container_c", "container_d", "container_a", "container_c"]):
        last = i == 5
        boxes.append(c.ridge(560, 60, role, height=480, under="hull_navy", gap=0 if not last else 700,
                             gap_drop=0))
    whl = c.ridge(4200, 420, "whale", sides="right", height=620, angle=53, under="whale_belly", gap=520, gap_drop=120)
    # up the fishing line to the boat's stern
    line = c.ridge(2000, -380, "line", height=420, angle=54, under="boat_wood", gap=0, gap_drop=0)
    boat_c = forward(line["end"], c.yaw, 1000)
    deck_top = line["end"][2] - 40
    k.oval(boat_c, 1000, 520, deck_top - 300, deck_top, "boat_wood", yaw=c.yaw, side="boat_white")
    finish_zone = [[boat_c[0] - 700, boat_c[1] - 700, deck_top], [boat_c[0] + 700, boat_c[1] + 700, deck_top + 300]]
    finish_zone = [[min(finish_zone[0][0], finish_zone[1][0]), min(finish_zone[0][1], finish_zone[1][1]), deck_top],
                   [max(finish_zone[0][0], finish_zone[1][0]), max(finish_zone[0][1], finish_zone[1][1]), deck_top + 300]]

    # ---- drop-in points and falls -----------------------------------------------
    drop_in(k, "cp_reef", reef1[0])
    drop_in(k, "cp_turtle", tur)
    drop_in(k, "cp_reef2", reef2[0])
    drop_in(k, "cp_deep", anglers[1])
    drop_in(k, "cp_speedy", speedy)
    k.dest("cp_vent", (vc[0], vc[1], v_lo_z + 1), yaw)
    drop_in(k, "cp_shark", shk)
    drop_in(k, "cp_ship", hull)
    drop_in(k, "cp_whale", whl)
    drop_in(k, "cp_line", line)

    for d in dunes:
        catch_under(k, d, "start")
    for d in reef1:
        catch_under(k, d, "cp_reef")
    catch_under(k, tur, "cp_turtle")
    catch_under(k, reef2[0], "cp_reef2")
    # the long drop: falls past the first anglerfish land in its catch
    catch_under(k, reef2[1], "cp_reef2", below=4200, above=2400, pad=1800)
    catch_under(k, anglers[0], "cp_deep", pad=1800)
    catch_under(k, anglers[1], "cp_deep")
    catch_under(k, anglers[2], "cp_speedy")
    catch_under(k, speedy, "cp_speedy")
    # the abyss under the decks, and a sea floor under it
    span_lo = forward(decks[0][0], yaw, -decks[0][1] / 2 - 400)
    span_hi = forward(decks[-1][0], yaw, decks[-1][1] / 2)
    lo = (min(span_lo[0], span_hi[0]) - 900, min(span_lo[1], span_hi[1]) - 900)
    hi = (max(span_lo[0], span_hi[0]) + 900, max(span_lo[1], span_hi[1]) + 900)
    # the catch covers the whole sea floor, so nobody stands on it
    k.catch((lo[0] - 2000, lo[1] - 2000, floor_z), (hi[0] + 2000, hi[1] + 2000, deck_z - 300), "cp_speedy")
    k.box((lo[0] - 2000, lo[1] - 2000, floor_z - 200), (hi[0] + 2000, hi[1] + 2000, floor_z), "deep_floor")
    # leg C: the water itself catches you

    def leg_c_catch(piece, to):
        xs2 = [s[0] for s in piece["stations"]]
        ys2 = [s[1] for s in piece["stations"]]
        k.catch((min(xs2) - 900, min(ys2) - 1500, WATER - 1500), (max(xs2) + 900, max(ys2) + 1500, WATER - 150), to)

    for p2, to in [(shk, "cp_shark"), (hull, "cp_ship"), *[(b, "cp_ship") for b in boxes], (whl, "cp_whale"), (line, "cp_line")]:
        leg_c_catch(p2, to)
    # a missed vent falls back into the chimney
    k.catch((vc[0] - 3500, vc[1] - 3500, deck_z - 1400), (vc[0] + 3500, vc[1] + 3500, deck_z - 1000), "cp_vent")

    # ---- scenery -------------------------------------------------------------------
    for i, d in enumerate(dunes):
        base = d["start"][2] - RIDGE_H - 500
        for j in range(6):
            p2 = forward(d["start"], d["yaw"], 400 + j * 520, side=(-1) ** j * (1100 + 200 * (j % 3)))
            sea.kelp(k, (p2[0], p2[1], base), 700 + 120 * (j % 3), "kelp", 100 + i * 10 + j)
        sea.school(k, forward(d["end"], d["yaw"], 0, 300, side=900), 6, 300, 200 + i,
                   [("fish_gold", "coral_orange"), ("fish_blue", "white")])
    for i, d in enumerate(reef1 + reef2):
        base = min(s[2] for s in d["stations"]) - RIDGE_H - 300
        for j in range(8):
            p2 = forward(d["start"], d["stations"][0][3], 300 + j * 340, side=(-1) ** j * (800 + 160 * (j % 4)))
            q = (p2[0], p2[1], base)
            pick = (i + j) % 4
            if pick == 0:
                sea.coral(k, q, 900, "coral_pink", 300 + i * 20 + j, tip="coral_yellow")
            elif pick == 1:
                sea.brain(k, q, 260, "coral_purple", "coral_pink")
            elif pick == 2:
                sea.anemone(k, q, "anemone", "coral_yellow", 400 + j, size=140)
            else:
                sea.coral(k, q, 760, "coral_orange", 500 + i * 20 + j, tip="white")
        sea.school(k, forward(d["start"], d["stations"][0][3], 1200, 200, side=-1000), 8, 380, 600 + i,
                   [("fish_gold", "fish_blue"), ("coral_yellow", "coral_purple"), ("fish_blue", "fish_gold")])
        # coral arches over the second reef: in and out of them
        if d in reef2:
            for j in range(3):
                mid = forward(d["start"], d["stations"][0][3], 700 + j * 900)
                for t in range(9):
                    ang = math.radians(t * 22.5)
                    p3 = forward(mid, d["stations"][0][3] + 90, math.cos(ang) * 700, dz=math.sin(ang) * 560 - 320)
                    k.blob("coral_pink" if t % 2 else "coral_purple", p3, (110, 110, 110))
    sea.turtle(k, tur["end"], tur["stations"][len(tur["stations"]) // 2], tur["end"][2], 600,
               "turtle_shell", "turtle_skin")
    for i, d in enumerate(anglers):
        sea.angler(k, d["end"], d["end"][2], d["height"], d["yaw"], "angler", "lure")
    for c2, length in decks:
        sea.oil_platform(k, c2, min(length / 2, 420), deck_z - 64, floor_z, "oil_steel", "rust")
    sea.shark(k, shk["start"], shk["end"], shk["start"][2], 560, "shark", "shark_belly")
    # the ship: hull below its red side, a funnel by the steam
    hs, he = hull["start"], hull["end"]
    mid = ((hs[0] + he[0]) / 2, (hs[1] + he[1]) / 2, hs[2] - 900)
    k.blob("hull_navy", forward(mid, hull["yaw"], 0, side=-900), (2600, 900, 560), hull["yaw"])
    k.blob("white", forward(steam_at, hull["yaw"], 0, dz=-700, side=-700), (260, 260, 520), hull["yaw"])
    sea.whale(k, whl["start"], whl["end"], whl["start"][2], 620, "whale", "whale_belly", side=-1)
    # the fishing boat: a cabin, a rod, and its line down to where you ride up
    k.blob("boat_white", forward(boat_c, c.yaw, 300, dz=deck_top - boat_c[2] + 200), (320, 300, 220), c.yaw)
    k.blob("fish_blue", forward(boat_c, c.yaw, 300, dz=deck_top - boat_c[2] + 260, side=0), (330, 310, 40), c.yaw)
    rod_tip = forward(boat_c, c.yaw, -900, dz=deck_top - boat_c[2] + 700)
    for i in range(10):
        q = forward(boat_c, c.yaw, -300 - i * 66, dz=deck_top - boat_c[2] + 80 + i * 68)
        k.blob("boat_wood", q, 12)
    ls = line["start"]
    for i in range(18):
        t = i / 17
        q = (rod_tip[0] + (ls[0] - rod_tip[0]) * t, rod_tip[1] + (ls[1] - rod_tip[1]) * t,
             rod_tip[2] + (ls[2] + 60 - rod_tip[2]) * t - math.sin(t * math.pi) * 180)
        k.blob("line", q, 7)
    k.blob("oil_steel", (ls[0], ls[1], ls[2] + 40), (18, 50, 60), c.yaw)
    for i in range(10):
        q = forward(boat_c, c.yaw, -500 + i * 110, dz=deck_top - boat_c[2] + 20, side=(-1) ** i * 120)
        sea.fish(k, q, c.yaw + 90 * (-1) ** i, 34, "fish_gold" if i % 2 else "fish_blue", "white")

    return {
        "start_zone": start_zone,
        "finish_zone": finish_zone,
        "roles": [{"target": "start", "role": "restart"}],
        "spawn": "start",
    }
