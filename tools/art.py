#!/usr/bin/env python3
"""Surfs Up!'s 2D art, written as TikZ and compiled, never generated as
pixels. This follows Sam's diagrams-as-code prompt
(GameAI-Fall26/KGCurriculum/csc484_rewrite_spec/inputs/tikz_figure_prompt.md):
each picture is a compilable `standalone` document, every tunable colour is a
\\definecolor at the top, repeated elements come from \\foreach, sections are
commented in drawing order, and the facts each picture must get right are
listed below with where each is enforced.

The look follows ptonkle's Ape Race: flat low-poly shapes, each solid drawn
as two or three flat facets (a lit side and a shaded side), no outlines,
saturated colours, a soft background. The colours are the 3D gym's
(crates/core/src/mesh.rs and data/shaders/scene.frag).

    python3 tools/art.py            # every picture into web/art/
    python3 tools/art.py title      # one picture

Facts the pictures must get right, and where each is enforced:

- No military look: no guns, no camouflage, nothing in khaki. The palette
  below has no olive or khaki entries, and `check` refuses a picture that
  names a colour outside the palette.
- The title's surf ramp is a triangular prism (a ridge), steeper than
  45 degrees on its riding side, with a foam pit below, as the map's first
  ramp is: `title` draws the ramp from RAMP_SLOPE (52 degrees, the map's
  normal z of 0.62) and asserts it is above 45.6.
- The how-to-surf diagram holds the key toward the ramp's high side: the
  surfer sits on the right-hand face, so the ramp top is to the surfer's
  left, and the highlighted key is A; asserted in `how_to_surf`.
- The prayer hands are two palms pressed flat together, fingers up, four
  fingers and a thumb each: `prayer` draws the fingers in a \\foreach over
  four and asserts FINGERS == 4.
- The rubber chicken is the classic one: pale yellow, long neck, red comb,
  orange beak open, thin orange legs.
- The kitsune set: a pinwheel has four blades in a \\foreach, each a
  different colour (asserted in `pinwheel`); the fox plush has two ears,
  a white tail tip and a white muzzle.
- Repeatable: every random choice comes from a seeded random.Random.
"""
import math, random, shutil, subprocess, sys, tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "web" / "art"
SRC = ROOT / "tools" / "art"

COLOURS = {
    "sky": "DCE6EE", "skyhi": "F2F6F9", "wall": "EDE6D3", "walldark": "D6CCB4",
    "stripeblue": "2F5CBF", "stripegold": "F8C537", "window": "CFE6FF", "windowlit": "F6FBFF",
    "wood": "DBA86B", "wooddark": "B98549", "woodline": "C9965A",
    "mat": "2961CC", "matlit": "4A82E8", "matdark": "1C4597", "matseam": "16377A",
    "red": "DD3D38", "redlit": "F06A5E", "reddark": "A92A27",
    "yellow": "F9C02E", "yellowlit": "FFDB6B", "yellowdark": "C9961C",
    "green": "3FA866", "greenlit": "62C98A", "greendark": "2C7D4B",
    "pink": "F27A93", "pinklit": "F7A1B3", "orange": "F28A2E", "orangelit": "F7AE66", "orangedark": "C2671A",
    "sky2": "7FB8F0", "purple": "8B5CC8",
    "skin": "F2C49A", "skinlit": "F8D8B8", "skindark": "D9A47A",
    "skinb": "B9835A", "skinbdark": "93653F",
    "shirt": "E2463F", "shirtdark": "B23430", "shirtb": "2F7DD8", "shirtbdark": "225DA6",
    "shorts": "21305A", "shortsdark": "151F3D", "shoe": "FFFFFF", "shoedark": "D8DDE3",
    "hair": "4A2F1C", "band": "F8C537",
    "chick": "FBE48F", "chicklit": "FFF2BE", "chickdark": "DCC064", "comb": "E2302E", "beak": "F28A2E",
    "noodle": "57D163", "noodledark": "3A9E45", "ball": "D8302B", "balldark": "A5211D", "balllit": "F0645D",
    "duck": "FFD12E", "duckdark": "E0A814", "foam": "2F6BE0", "foamdark": "214FAD", "white": "FFFFFF",
    "sand": "EDCB85", "sandlit": "F7E2AE", "sanddark": "C9A35E", "camel": "D6A35F", "cameldark": "A97A3F",
    "stick": "7CF25C", "sticklit": "C8FFB0", "night": "24222B", "nightlit": "3A3744",
    "gold": "F5C231", "goldlit": "FFE07A", "golddark": "B98A12", "wrap": "F0EBD6", "wrapdark": "C9C2A8",
    "ink": "1E2B45", "netline": "9AA9BF", "glow": "FFF4C2",
}

PREAMBLE = (
    "\\documentclass[tikz,border=0pt]{standalone}\n"
    "\\usetikzlibrary{calc}\n"
    "% Palette: the 3D gym's colours (crates/core/src/mesh.rs, data/shaders/scene.frag).\n"
    + "\n".join(f"\\definecolor{{{k}}}{{HTML}}{{{v}}}" for k, v in COLOURS.items())
    + "\n"
)


def f(x):
    return f"{x:.3f}".rstrip("0").rstrip(".") if abs(x) > 0.0005 else "0"


def P(x, y):
    return f"({f(x)},{f(y)})"


def poly(points, fill, extra=""):
    return f"\\fill[{fill}{extra}] " + " -- ".join(P(x, y) for x, y in points) + " -- cycle;\n"


def rect(x0, y0, x1, y1, fill, extra=""):
    return f"\\fill[{fill}{extra}] {P(x0, y0)} rectangle {P(x1, y1)};\n"


def circle(x, y, r, fill, extra=""):
    return f"\\fill[{fill}{extra}] {P(x, y)} circle ({f(r)});\n"


def ellipse(x, y, rx, ry, fill, extra=""):
    return f"\\fill[{fill}{extra}] {P(x, y)} ellipse ({f(rx)} and {f(ry)});\n"


def facet_ball(x, y, r, lit, mid, dark):
    """A low-poly ball: an octagon in three facet shades."""
    pts = [(x + r * math.cos(math.radians(22.5 + 45 * k)), y + r * math.sin(math.radians(22.5 + 45 * k))) for k in range(8)]
    out = poly(pts, mid)
    out += poly([(x, y), pts[1], pts[2], pts[3]], lit)
    out += poly([(x, y), pts[5], pts[6], pts[7]], dark)
    return out


def document(body, w, h):
    return PREAMBLE + "\\begin{document}\n\\begin{tikzpicture}[x=1cm,y=1cm]\n" + f"\\useasboundingbox (0,0) rectangle {P(w, h)};\n" + body + "\\end{tikzpicture}\n\\end{document}\n"


def check(name, tex):
    import re
    for colour in re.findall(r"\\fill\[([a-z0-9]+)", tex):
        if colour not in COLOURS:
            raise SystemExit(f"{name}: colour {colour} is not in the palette")


# ---- people ------------------------------------------------------------------

def kid(x, y, s, shirt, shirtdark, skin, skindark, lean=0.0, arms_up=False):
    """A kid in gym clothes standing in a surf stance, feet at (x, y)."""
    out = f"\\begin{{scope}}[shift={{{P(x, y)}}},scale={f(s)},rotate={f(lean)}]\n"
    # sneakers
    for dx in (-0.35, 0.35):
        out += poly([(dx - 0.25, 0), (dx + 0.3, 0), (dx + 0.3, 0.16), (dx - 0.2, 0.2)], "shoe")
        out += poly([(dx - 0.25, 0), (dx + 0.3, 0), (dx + 0.3, 0.05), (dx - 0.25, 0.05)], "shoedark")
    # legs, slightly bent
    for dx, k in ((-0.35, -1), (0.35, 1)):
        out += poly([(dx - 0.1, 0.18), (dx + 0.1, 0.18), (dx * 0.6 + 0.1, 0.95), (dx * 0.6 - 0.12, 0.95)], "skin")
        out += poly([(dx + 0.02, 0.18), (dx + 0.1, 0.18), (dx * 0.6 + 0.1, 0.95), (dx * 0.6, 0.95)], "skindark")
    # shorts and shirt
    out += poly([(-0.42, 0.9), (0.42, 0.9), (0.38, 1.35), (-0.38, 1.35)], "shorts")
    out += poly([(0.05, 0.9), (0.42, 0.9), (0.38, 1.35), (0.05, 1.35)], "shortsdark")
    out += poly([(-0.42, 1.3), (0.42, 1.3), (0.46, 2.05), (-0.46, 2.05)], shirt)
    out += poly([(0.1, 1.3), (0.42, 1.3), (0.46, 2.05), (0.1, 2.05)], shirtdark)
    # arms out for balance (or up, cheering)
    for k in (-1, 1):
        if arms_up:
            out += poly([(k * 0.4, 2.0), (k * 0.52, 1.85), (k * 0.95, 2.65), (k * 0.8, 2.75)], skin)
        else:
            out += poly([(k * 0.42, 1.98), (k * 0.45, 1.8), (k * 1.3, 1.62), (k * 1.32, 1.78)], skin)
            out += poly([(k * 0.42, 2.0), (k * 0.46, 1.78), (k * 0.68, 1.75), (k * 0.66, 1.98)], shirt)
    # head, hair, sweatband
    out += poly([(-0.3, 2.1), (0.3, 2.1), (0.32, 2.72), (-0.32, 2.72)], skin)
    out += poly([(0.08, 2.1), (0.3, 2.1), (0.32, 2.72), (0.08, 2.72)], skindark)
    out += poly([(-0.34, 2.7), (0.34, 2.7), (0.3, 2.92), (-0.3, 2.92)], "hair")
    out += poly([(-0.33, 2.58), (0.33, 2.58), (0.33, 2.68), (-0.33, 2.68)], "band")
    out += "\\end{scope}\n"
    return out


def chicken(x, y, s, angle=0.0, gape=1.0):
    """The rubber chicken, standing on (x, y)."""
    out = f"\\begin{{scope}}[shift={{{P(x, y)}}},scale={f(s)},rotate={f(angle)}]\n"
    # legs
    for dx in (-0.12, 0.12):
        out += poly([(dx - 0.03, -0.55), (dx + 0.03, -0.55), (dx + 0.03, 0.1), (dx - 0.03, 0.1)], "beak")
        out += poly([(dx - 0.14, -0.6), (dx + 0.14, -0.6), (dx, -0.5)], "beak")
    # body
    out += poly([(-0.45, 0.0), (0.35, -0.05), (0.45, 0.35), (0.1, 0.62), (-0.42, 0.45)], "chick")
    out += poly([(-0.45, 0.0), (0.35, -0.05), (0.2, 0.2), (-0.3, 0.18)], "chickdark")
    out += poly([(-0.42, 0.45), (0.1, 0.62), (0.0, 0.42), (-0.3, 0.35)], "chicklit")
    # long neck and head
    out += poly([(0.15, 0.5), (0.32, 0.45), (0.5, 1.45), (0.36, 1.48)], "chick")
    out += poly([(0.25, 0.47), (0.32, 0.45), (0.5, 1.45), (0.43, 1.47)], "chickdark")
    out += poly([(0.28, 1.4), (0.62, 1.38), (0.66, 1.7), (0.3, 1.74)], "chick")
    out += poly([(0.32, 1.72), (0.4, 1.9), (0.48, 1.76), (0.56, 1.92), (0.62, 1.7)], "comb")
    out += poly([(0.64, 1.55), (0.95, 1.6 + 0.08 * gape), (0.64, 1.66)], "beak")
    out += poly([(0.64, 1.5), (0.9, 1.45 - 0.08 * gape), (0.64, 1.56)], "orangedark")
    out += circle(0.52, 1.62, 0.04, "ink")
    out += "\\end{scope}\n"
    return out


# ---- the title -----------------------------------------------------------------

W, H = 25.6, 14.4
RAMP_SLOPE = math.degrees(math.acos(0.62))  # the map's first ramp: normal z 0.62


def title():
    assert RAMP_SLOPE > 45.6, "a surf ramp must be too steep to stand on"
    rng = random.Random(7)
    b = "% ---- background: the gym wall, its stripe and high windows ----\n"
    b += rect(0, 0, W, H, "wall")
    b += "\\foreach \\row in {0,...,24} {\\fill[walldark] (0,{\\row*0.6}) rectangle (" + f(W) + ",{\\row*0.6+0.04});}\n"
    b += rect(0, 6.2, W, 6.9, "stripeblue") + rect(0, 6.9, W, 7.15, "stripegold")
    b += "\\foreach \\i in {0,...,5} {\\fill[window] ({0.8+\\i*4.2},10.2) rectangle ({3.8+\\i*4.2},13.6);"
    b += "\\fill[windowlit] ({0.8+\\i*4.2},12.2) rectangle ({3.8+\\i*4.2},13.6);"
    b += "\\fill[walldark] ({2.25+\\i*4.2},10.2) rectangle ({2.35+\\i*4.2},13.6);}\n"
    b += "% pennants between the windows\n"
    cols = ["red", "stripeblue", "yellow", "green", "red", "stripeblue", "yellow"]
    for i, c in enumerate(cols):
        x = 0.2 + i * 4.2 - 0.35
        if 0 < x < W - 1:
            b += poly([(x - 0.5, 10.0), (x + 0.5, 10.0), (x, 8.5)], c)
    b += "% ---- the floor: maple boards ----\n"
    b += rect(0, 0, W, 2.6, "wood")
    b += "\\foreach \\i in {0,...,40} {\\fill[woodline] ({\\i*0.65},0) rectangle ({\\i*0.65+0.03},2.6);}\n"
    b += rect(0, 2.55, W, 2.6, "wooddark")
    b += "% ---- the foam pit: a low wall and rows of foam cubes ----\n"
    pit_x0, pit_x1 = 9.5, 25.6
    foam = ["pink", "sky2", "yellow", "green", "orange", "pinklit", "yellowlit"]
    for row in range(4):
        for k in range(int((pit_x1 - pit_x0) / 0.8) + 1):
            cx = pit_x0 + k * 0.8 + (row % 2) * 0.4 + rng.uniform(-0.08, 0.08)
            cy = 1.2 + row * 0.42 + rng.uniform(-0.05, 0.05)
            c = rng.choice(foam)
            b += poly([(cx, cy), (cx + 0.72, cy + 0.05), (cx + 0.7, cy + 0.6), (cx - 0.02, cy + 0.55)], c)
            b += poly([(cx, cy), (cx + 0.72, cy + 0.05), (cx + 0.72, cy + 0.15), (cx, cy + 0.1)], "ink", ",opacity=0.18")
    b += rect(pit_x0 - 0.4, 0.4, pit_x1, 1.25, "stripeblue")
    b += rect(pit_x0 - 0.4, 1.1, pit_x1, 1.25, "matdark")
    b += "% ---- the surf ramp: a ridge of blue mat, seen from its end ----\n"
    # end-on triangle: apex, two sloped faces at RAMP_SLOPE, running back into the gym
    apex = (17.2, 9.3)
    half = 3.0
    rise = half * math.tan(math.radians(RAMP_SLOPE))
    base_y = apex[1] - rise
    left = (apex[0] - half, base_y)
    right = (apex[0] + half, base_y)
    depth = (-6.5, 2.0)  # the ridge runs back and up to the left
    a2 = (apex[0] + depth[0], apex[1] + depth[1])
    l2 = (left[0] + depth[0], left[1] + depth[1])
    b += poly([left, apex, a2, l2], "matlit")
    b += poly([apex, right, (right[0] + depth[0], right[1] + depth[1]), a2], "mat")
    b += poly([left, apex, right], "matdark")
    b += "\\foreach \\t in {0.2,0.4,0.6,0.8} {\\draw[matseam,line width=1pt] ($" + P(*left) + "!\\t!" + P(*l2) + "$) -- ($" + P(*apex) + "!\\t!" + P(*a2) + "$);}\n"
    b += "% the ramp's frame: steel legs down to the floor\n"
    b += "\\foreach \\x in {" + ",".join(f(left[0] + 0.3 + k * (2 * half - 0.6) / 3) for k in range(4)) + "} {\\fill[netline] (\\x," + f(base_y) + ") rectangle (\\x+0.18,2.55);}\n"
    b += rect(left[0], base_y - 0.25, right[0], base_y, "matdark")
    b += "% ---- the surfers ----\n"
    face = lambda t: (left[0] + (apex[0] - left[0]) * t + depth[0] * 0.35, left[1] + (apex[1] - left[1]) * t + depth[1] * 0.35)
    fx, fy = face(0.45)
    b += kid(fx, fy, 0.95, "shirtb", "shirtbdark", "skin", "skindark", lean=-(90 - RAMP_SLOPE) * 0.6)
    t2 = 0.78
    fx2 = left[0] + (apex[0] - left[0]) * t2 + depth[0] * 0.75
    fy2 = left[1] + (apex[1] - left[1]) * t2 + depth[1] * 0.75
    b += kid(fx2, fy2, 0.8, "shirt", "shirtdark", "skinb", "skinbdark", lean=-(90 - RAMP_SLOPE) * 0.6)
    b += chicken(fx2 + 1.05, fy2 + 1.45, 0.6, angle=-25)
    b += "% a kid on the floor cheering, holding a pool noodle\n"
    b += kid(4.2, 2.65, 1.05, "green", "greendark", "skin", "skindark", arms_up=True)
    b += poly([(5.12, 5.42), (5.3, 5.5), (6.3, 8.2), (6.12, 8.25)], "noodle")
    b += "% ---- a soft light from the windows over everything ----\n"
    b += rect(0, 7.3, W, H, "white", ",opacity=0.12")
    b += "% calm space at the lower left stays plain: the page has no words over the picture\n"
    return b, (W, H)


def icon():
    b = rect(0, 0, 2, 2, "sky")
    b += poly([(0.15, 0.55), (1.0, 1.7), (1.85, 0.55)], "mat")
    b += poly([(1.0, 1.7), (1.85, 0.55), (1.0, 0.55)], "matdark")
    for k, c in enumerate(["pink", "yellow", "sky2", "green", "orange"]):
        b += rect(0.05 + k * 0.38, 0.05, 0.4 + k * 0.38, 0.48, c)
    return b, (2, 2)


# ---- things to hold -----------------------------------------------------------

S = 3.0
FINGERS = 4


def prayer():
    b = ""
    for side in (-1, 1):
        cx = 1.5 + side * 0.17
        b += f"% {'left' if side < 0 else 'right'} palm\n"
        b += poly([(1.5, 0.9), (cx + side * 0.5, 0.85), (cx + side * 0.48, 1.9), (1.5, 1.95)], "skin" if side < 0 else "skindark")
        b += f"\\foreach \\k in {{0,...,{FINGERS - 1}}} {{\\fill[{'skin' if side < 0 else 'skindark'}] ({f(1.5)}+{f(side)}*\\k*0.11,1.9) rectangle ({f(1.5)}+{f(side)}*\\k*0.11+{f(side)}*0.1,{{2.45+0.12*sin(\\k*60)}});}}\n"
        b += poly([(cx + side * 0.45, 1.1), (cx + side * 0.62, 1.25), (cx + side * 0.5, 1.55), (cx + side * 0.4, 1.4)], "skinlit" if side < 0 else "skin")
        b += poly([(1.5 + side * 0.05, 0.9), (1.5 + side * 0.55, 0.85), (1.5 + side * 0.9, 0.1), (1.5 + side * 0.35, 0.1)], "shirtb" if side < 0 else "shirtbdark")
    b += poly([(1.49, 0.9), (1.51, 0.9), (1.51, 2.5), (1.49, 2.5)], "skindark")
    assert FINGERS == 4
    return b, (S, S)


def noodle():
    b = ""
    pts = [(0.55 + 0.2 * k, 0.35 + 0.22 * k + 0.008 * k * k) for k in range(10)]
    for (x0, y0), (x1, y1) in zip(pts, pts[1:]):
        b += poly([(x0 - 0.22, y0 + 0.1), (x0 + 0.18, y0 - 0.15), (x1 + 0.18, y1 - 0.15), (x1 - 0.22, y1 + 0.1)], "noodle")
        b += poly([(x0 + 0.05, y0 - 0.05), (x0 + 0.18, y0 - 0.15), (x1 + 0.18, y1 - 0.15), (x1 + 0.05, y1 - 0.05)], "noodledark")
    x, y = pts[-1]
    b += ellipse(x - 0.02, y - 0.02, 0.21, 0.12, "noodledark") + ellipse(x - 0.02, y - 0.02, 0.07, 0.04, "ink")
    return b, (S, S)


def kickball():
    b = facet_ball(1.5, 1.5, 1.1, "balllit", "ball", "balldark")
    b += "\\draw[balldark,line width=1.4pt] (0.45,1.6) .. controls (1.2,1.25) and (1.8,1.25) .. (2.55,1.6);\n"
    return b, (S, S)


def duck():
    b = facet_ball(1.35, 1.1, 0.85, "yellowlit", "duck", "duckdark")
    b += facet_ball(2.0, 2.0, 0.5, "yellowlit", "duck", "duckdark")
    b += poly([(2.42, 2.05), (2.85, 1.95), (2.42, 1.85)], "beak")
    b += circle(2.1, 2.15, 0.07, "ink")
    b += poly([(0.6, 1.2), (1.1, 1.5), (0.95, 0.95)], "duckdark")
    return b, (S, S)


def foam_finger():
    b = poly([(0.9, 0.3), (2.1, 0.3), (2.15, 1.6), (0.85, 1.6)], "foam")
    b += poly([(1.6, 0.3), (2.1, 0.3), (2.15, 1.6), (1.6, 1.6)], "foamdark")
    b += poly([(1.25, 1.55), (1.75, 1.55), (1.7, 2.8), (1.3, 2.8)], "foam")
    b += poly([(1.55, 1.55), (1.75, 1.55), (1.7, 2.8), (1.55, 2.8)], "foamdark")
    b += poly([(0.85, 1.2), (0.55, 1.65), (0.75, 1.85), (1.0, 1.5)], "foam")
    b += rect(1.0, 0.75, 2.0, 0.95, "white")
    return b, (S, S)


def chicken_icon():
    return chicken(0.95, 0.75, 1.15, angle=-6), (S, S)


PINWHEEL_COLOURS = ["red", "yellow", "stripeblue", "green"]


def pinwheel():
    b = "% the stick\n"
    b += poly([(1.46, 0.15), (1.54, 0.15), (1.54, 1.85), (1.46, 1.85)], "walldark")
    b += "% four folded paper blades, one colour each, round the pin\n"
    assert len(set(PINWHEEL_COLOURS)) == 4
    for k, c in enumerate(PINWHEEL_COLOURS):
        a0 = math.radians(20 + 90 * k)
        a1 = math.radians(20 + 90 * k + 90)
        cx, cy = 1.5, 1.95
        tip = (cx + 0.95 * math.cos(a0), cy + 0.95 * math.sin(a0))
        side = (cx + 0.7 * math.cos(a1), cy + 0.7 * math.sin(a1))
        mid = (cx + 0.5 * math.cos((a0 + a1) / 2), cy + 0.5 * math.sin((a0 + a1) / 2))
        b += poly([(cx, cy), tip, mid], c)
        b += poly([(cx, cy), mid, side], c, ",opacity=0.75")
    b += circle(1.5, 1.95, 0.09, "white")
    return b, (S, S)


def ribbon():
    b = "% the wand\n"
    b += poly([(0.45, 0.3), (0.55, 0.25), (1.15, 1.45), (1.05, 1.5)], "white")
    b += poly([(0.5, 0.27), (0.55, 0.25), (1.15, 1.45), (1.1, 1.47)], "walldark")
    b += "% the ribbon, looping up and away\n"
    b += "\\draw[pink,line width=5pt,line cap=round] (1.1,1.48) .. controls (2.6,1.2) and (2.4,2.5) .. (1.6,2.3) .. controls (0.9,2.1) and (1.6,2.95) .. (2.6,2.7);\n"
    b += "\\draw[pinklit,line width=1.6pt,line cap=round] (1.2,1.5) .. controls (2.5,1.3) and (2.35,2.4) .. (1.65,2.25);\n"
    return b, (S, S)


def lantern():
    b = "% the handle and tassel\n"
    b += poly([(1.47, 2.4), (1.53, 2.4), (1.53, 2.9), (1.47, 2.9)], "ink")
    b += poly([(1.42, 0.15), (1.58, 0.15), (1.55, 0.6), (1.45, 0.6)], "yellow")
    b += "% the paper body, in two shades, with its ribs\n"
    b += ellipse(1.5, 1.5, 0.95, 0.85, "red")
    b += "\\fill[reddark] (1.5,0.65) arc (-90:90:0.95 and 0.85) -- cycle;\n"
    b += "\\foreach \\y in {1.05,1.5,1.95} {\\fill[reddark] (0.6,\\y-0.025) rectangle (2.4,\\y+0.025);}\n"
    b += rect(1.0, 2.25, 2.0, 2.45, "ink") + rect(1.0, 0.55, 2.0, 0.75, "ink")
    return b, (S, S)


def bell():
    b = "% the wooden handle\n"
    b += poly([(1.35, 1.7), (1.65, 1.7), (1.62, 2.85), (1.38, 2.85)], "wood")
    b += poly([(1.5, 1.7), (1.65, 1.7), (1.62, 2.85), (1.5, 2.85)], "wooddark")
    b += "% the brass bell, flaring to its lip, and the clapper\n"
    b += circle(1.5, 0.45, 0.17, "yellowdark")
    b += poly([(1.15, 1.75), (1.85, 1.75), (2.2, 0.75), (2.45, 0.55), (0.55, 0.55), (0.8, 0.75)], "yellow")
    b += poly([(1.5, 1.75), (1.85, 1.75), (2.2, 0.75), (2.45, 0.55), (1.5, 0.55)], "yellowdark")
    b += poly([(1.2, 1.6), (1.35, 1.6), (1.05, 0.75), (0.9, 0.75)], "yellowlit")
    return b, (S, S)


def fox():
    b = "% the tail, curled up behind, white at the tip\n"
    b += poly([(1.9, 0.5), (2.75, 0.9), (2.85, 1.7), (2.45, 1.75), (2.2, 1.0)], "orangedark")
    b += poly([(2.85, 1.7), (2.45, 1.75), (2.6, 2.1)], "white")
    b += "% the body, the white chest\n"
    b += facet_ball(1.45, 1.0, 0.75, "orangelit", "orange", "orangedark")
    b += ellipse(1.35, 0.95, 0.32, 0.45, "white")
    b += "% the head: two ears, a white muzzle, eyes and nose\n"
    for x0 in (1.0, 1.75):
        b += poly([(x0, 2.35), (x0 + 0.25, 2.85), (x0 + 0.45, 2.3)], "orange")
        b += poly([(x0 + 0.13, 2.6), (x0 + 0.25, 2.85), (x0 + 0.35, 2.6)], "ink")
    b += facet_ball(1.47, 2.05, 0.55, "orangelit", "orange", "orangedark")
    b += poly([(1.1, 1.95), (1.85, 1.95), (1.47, 1.55)], "white")
    b += circle(1.47, 1.62, 0.07, "ink")
    b += circle(1.27, 2.15, 0.06, "ink") + circle(1.67, 2.15, 0.06, "ink")
    return b, (S, S)


def bucket():
    b = "% the handle, then the pail flaring to its rim, sand heaped on top, a red spade\n"
    b += "\\draw[netline,line width=2pt] (0.85,1.9) .. controls (0.9,2.9) and (2.1,2.9) .. (2.15,1.9);\n"
    b += poly([(0.95, 0.35), (2.05, 0.35), (2.25, 1.9), (0.75, 1.9)], "sky2")
    b += poly([(1.5, 0.35), (2.05, 0.35), (2.25, 1.9), (1.5, 1.9)], "stripeblue")
    b += rect(0.7, 1.85, 2.3, 2.0, "stripeblue")
    b += ellipse(1.5, 2.02, 0.72, 0.22, "sand") + ellipse(1.35, 2.1, 0.35, 0.1, "sandlit")
    b += poly([(1.9, 2.0), (1.98, 2.0), (2.35, 2.75), (2.27, 2.78)], "red")
    b += poly([(2.2, 2.7), (2.55, 2.6), (2.65, 2.95), (2.35, 3.0)], "red")
    return b, (S, S)


def camel():
    b = "% legs, body, two humps, the neck and head\n"
    for x in (0.85, 1.15, 1.85, 2.1):
        b += rect(x, 0.3, x + 0.13, 1.25, "cameldark" if x in (1.15, 2.1) else "camel")
    b += poly([(0.7, 1.15), (2.3, 1.15), (2.35, 1.6), (0.65, 1.6)], "camel")
    b += poly([(0.7, 1.15), (2.3, 1.15), (2.3, 1.3), (0.7, 1.3)], "cameldark")
    b += ellipse(1.1, 1.65, 0.32, 0.3, "camel") + ellipse(1.75, 1.65, 0.32, 0.3, "camel")
    b += poly([(2.2, 1.4), (2.4, 1.45), (2.55, 2.35), (2.38, 2.4)], "camel")
    b += poly([(2.3, 2.3), (2.85, 2.3), (2.85, 2.55), (2.35, 2.62)], "camel")
    b += circle(2.62, 2.48, 0.04, "ink")
    b += poly([(0.65, 1.5), (0.45, 1.05), (0.52, 1.02), (0.7, 1.4)], "cameldark")
    return b, (S, S)


PYRAMID_STEPS = 4


def pyramid():
    b = "% four stepped blocks of sandstone, a gold cap\n"
    for k in range(PYRAMID_STEPS):
        h = 1.15 - k * 0.27
        y0 = 0.35 + k * 0.5
        b += rect(1.5 - h, y0, 1.5 + h, y0 + 0.5, "sand")
        b += rect(1.5, y0, 1.5 + h, y0 + 0.5, "sanddark")
        b += rect(1.5 - h, y0 + 0.42, 1.5 + h, y0 + 0.5, "sandlit")
    b += poly([(1.3, 2.35), (1.7, 2.35), (1.5, 2.75)], "gold")
    b += poly([(1.5, 2.35), (1.7, 2.35), (1.5, 2.75)], "golddark")
    assert PYRAMID_STEPS == 4
    return b, (S, S)


def scarab():
    b = "% six legs, the shell split down the middle, the head\n"
    for side in (-1, 1):
        for k, y in enumerate((1.1, 1.5, 1.9)):
            x0 = 1.5 + side * 0.55
            b += poly([(x0, y), (x0 + side * 0.55, y - 0.25 + k * 0.1), (x0 + side * 0.58, y - 0.17 + k * 0.1), (x0, y + 0.08)], "golddark")
    b += ellipse(1.5, 1.45, 0.62, 0.78, "gold")
    b += "\\fill[goldlit] (1.5,2.2) arc (90:270:0.62 and 0.75) -- cycle;\n"
    b += poly([(1.48, 0.7), (1.52, 0.7), (1.52, 2.2), (1.48, 2.2)], "golddark")
    b += ellipse(1.5, 2.35, 0.3, 0.22, "golddark")
    return b, (S, S)


def mummy():
    b = "% a plush mummy: wrapped body, arms out, wrapped head, two eyes peeking\n"
    b += "\\foreach \\k in {0,...,4} {\\fill[wrap] (1.05,0.3+\\k*0.32) rectangle (1.95,0.58+\\k*0.32);\\fill[wrapdark] (1.05,0.56+\\k*0.32) rectangle (1.95,0.62+\\k*0.32);}\n"
    b += poly([(1.0, 1.4), (0.35, 1.55), (0.35, 1.75), (1.0, 1.7)], "wrap")
    b += poly([(2.0, 1.4), (2.65, 1.55), (2.65, 1.75), (2.0, 1.7)], "wrapdark")
    b += circle(1.5, 2.25, 0.5, "wrap")
    b += poly([(1.0, 2.35), (2.0, 2.15), (2.0, 2.25), (1.0, 2.45)], "wrapdark")
    b += circle(1.32, 2.25, 0.08, "ink") + circle(1.68, 2.2, 0.08, "ink")
    return b, (S, S)


def jumprope():
    b = "% the rope looping between two red handles\n"
    b += "\\draw[yellow,line width=2.4pt] (0.75,1.55) .. controls (0.6,3.2) and (2.4,3.2) .. (2.25,1.55);\n"
    for x in (0.62, 2.12):
        b += poly([(x, 0.35), (x + 0.26, 0.35), (x + 0.24, 1.6), (x + 0.02, 1.6)], "red")
        b += poly([(x + 0.13, 0.35), (x + 0.26, 0.35), (x + 0.24, 1.6), (x + 0.13, 1.6)], "reddark")
    return b, (S, S)


def cat():
    b = "% a sitting black cat: body, chest, head, two ears, gold collar, green eyes\n"
    b += poly([(0.95, 0.3), (2.05, 0.3), (1.9, 1.6), (1.1, 1.6)], "night")
    b += poly([(1.5, 0.3), (2.05, 0.3), (1.9, 1.6), (1.5, 1.6)], "nightlit")
    b += poly([(2.05, 0.45), (2.6, 0.55), (2.6, 0.7), (2.0, 0.65)], "night")
    b += rect(1.08, 1.55, 1.92, 1.72, "gold")
    b += poly([(1.1, 1.72), (1.9, 1.72), (1.85, 2.45), (1.15, 2.45)], "night")
    b += poly([(1.15, 2.4), (1.3, 2.9), (1.45, 2.4)], "night") + poly([(1.55, 2.4), (1.7, 2.9), (1.85, 2.4)], "night")
    b += circle(1.33, 2.12, 0.07, "green") + circle(1.67, 2.12, 0.07, "green")
    return b, (S, S)


def glowstick():
    b = "% a glow stick, glowing green, with a cap\n"
    b += poly([(0.95, 0.4), (1.25, 0.3), (2.15, 2.5), (1.85, 2.6)], "stick")
    b += poly([(1.05, 0.42), (1.12, 0.39), (2.02, 2.56), (1.95, 2.58)], "sticklit")
    b += poly([(1.85, 2.6), (2.15, 2.5), (2.25, 2.75), (1.95, 2.85)], "netline")
    return b, (S, S)


SCEPTRE_BANDS = 5


def sceptre():
    b = "% a striped staff, gold and blue, with a hooked top\n"
    for k in range(SCEPTRE_BANDS):
        y0 = 0.3 + k * 0.4
        b += rect(1.38, y0, 1.62, y0 + 0.4, "gold" if k % 2 == 0 else "stripeblue")
    b += "\\draw[gold,line width=5pt,line cap=round] (1.5,2.3) .. controls (1.5,2.85) and (2.2,2.9) .. (2.2,2.45);\n"
    assert SCEPTRE_BANDS % 2 == 1
    return b, (S, S)


def jackal():
    b = "% a black jackal plush: body, gold collar, head, long snout, tall ears\n"
    b += facet_ball(1.4, 0.95, 0.7, "nightlit", "night", "night")
    b += rect(1.0, 1.55, 1.8, 1.7, "gold")
    b += facet_ball(1.4, 2.05, 0.48, "nightlit", "night", "night")
    b += poly([(1.75, 2.15), (2.55, 1.95), (2.55, 1.75), (1.75, 1.85)], "night")
    b += circle(2.55, 1.85, 0.07, "ink")
    for x in (1.05, 1.45):
        b += poly([(x, 2.35), (x + 0.12, 3.0), (x + 0.28, 2.38)], "night")
        b += poly([(x + 0.08, 2.45), (x + 0.12, 2.8), (x + 0.2, 2.47)], "gold")
    b += circle(1.62, 2.12, 0.06, "gold")
    return b, (S, S)


def how_to_surf():
    """A ramp seen end-on: the surfer on its right face, the ramp's top to the
    surfer's left, so the key to hold is A."""
    w, h = 16.0, 7.0
    b = rect(0, 0, w, h, "white")
    b += "% ---- the ramp, end-on: a ridge over a foam pit ----\n"
    apex = (5.0, 5.6)
    half = 3.2
    base = apex[1] - half * math.tan(math.radians(RAMP_SLOPE))
    b += poly([(apex[0] - half, base), apex, (apex[0] + half, base)], "mat")
    b += poly([apex, (apex[0] + half, base), (apex[0], base)], "matdark")
    b += "\\foreach \\i in {0,...,9} {\\fill[" + "pink" + "] ({0.3+\\i*0.95},0.25) rectangle ({1.0+\\i*0.95},0.8);}\n"
    b += "\\foreach \\i in {0,...,8} {\\fill[" + "sky2" + "] ({0.75+\\i*0.95},0.75) rectangle ({1.45+\\i*0.95},1.25);}\n"
    # the surfer on the right face, perpendicular-ish to it, leaning in
    t = 0.45
    sx, sy = apex[0] + half * t, apex[1] - (apex[1] - base) * t
    b += kid(sx + 0.45, sy + 0.1, 0.7, "shirt", "shirtdark", "skin", "skindark", lean=RAMP_SLOPE - 90 + 25)
    b += "% ---- what to do: hold the key toward the ramp's top ----\n"
    surfer_on_right_face = sx > apex[0]
    key = "A" if surfer_on_right_face else "D"
    assert key == "A", "on the right face the ramp rises to the surfer's left"
    b += f"\\draw[-latex,ink,line width=2.4pt] {P(sx + 1.6, sy + 2.2)} -- {P(apex[0] + 0.5, apex[1] + 0.3)};\n"
    b += "% gravity\n"
    b += f"\\draw[-latex,reddark,line width=2pt] {P(sx + 0.9, sy + 0.4)} -- {P(sx + 0.9, sy - 1.2)};\n"
    b += "% the keys: A lit, D dim, W crossed out\n"
    keys = [("A", 10.2, 3.2, True), ("S", 11.4, 3.2, False), ("D", 12.6, 3.2, False), ("W", 11.4, 4.4, False)]
    for name, x, y, lit in keys:
        b += f"\\fill[{'yellow' if lit else 'walldark'},rounded corners=3pt] {P(x - 0.5, y - 0.5)} rectangle {P(x + 0.5, y + 0.5)};\n"
        b += f"\\node[ink,font=\\sffamily\\bfseries\\Large] at {P(x, y)} {{{name}}};\n"
    b += f"\\draw[reddark,line width=2.5pt] {P(10.8, 3.8)} -- {P(12.0, 5.0)} {P(10.8, 5.0)} -- {P(12.0, 3.8)};\n"
    b += "% the mouse turns the same way as the key\n"
    b += f"\\fill[walldark,rounded corners=8pt] {P(14.2, 2.3)} rectangle {P(15.4, 4.2)};\n"
    b += f"\\draw[-latex,ink,line width=2.2pt] {P(15.2, 4.8)} arc (20:160:0.75);\n"
    return b, (w, h)


PICTURES = {
    "title": title,
    "icon": icon,
    "how-to-surf": how_to_surf,
    "items/prayer": prayer,
    "items/noodle": noodle,
    "items/kickball": kickball,
    "items/duck": duck,
    "items/foam_finger": foam_finger,
    "items/chicken": chicken_icon,
    "items/pinwheel": pinwheel,
    "items/ribbon": ribbon,
    "items/lantern": lantern,
    "items/bell": bell,
    "items/fox": fox,
    "items/bucket": bucket,
    "items/camel": camel,
    "items/pyramid": pyramid,
    "items/scarab": scarab,
    "items/mummy": mummy,
    "items/jumprope": jumprope,
    "items/cat": cat,
    "items/glowstick": glowstick,
    "items/sceptre": sceptre,
    "items/jackal": jackal,
}
DPI = {"title": 150, "icon": 81, "how-to-surf": 110}
CLEAR = {"items"}


def main():
    only = set(sys.argv[1:])
    for name, make in PICTURES.items():
        if only and not only & {name, name.split("/")[0]}:
            continue
        body, (w, h) = make()
        tex = document(body, w, h)
        check(name, tex)
        folder = name.split("/")[0] if "/" in name else ""
        with tempfile.TemporaryDirectory() as tmp:
            t = Path(tmp)
            (t / "art.tex").write_text(tex)
            r = subprocess.run(["pdflatex", "-interaction=nonstopmode", "-halt-on-error", "art.tex"], cwd=t, capture_output=True, text=True)
            if r.returncode != 0:
                print(r.stdout[-3000:])
                raise SystemExit(f"{name}: pdflatex failed")
            clear = ["-transp"] if folder in CLEAR else []
            dpi = DPI.get(name, 110)
            subprocess.run(["pdftocairo", "-png", *clear, "-r", str(dpi), "-singlefile", str(t / "art.pdf"), str(t / "art")], check=True)
            out = OUT / f"{name}.png"
            out.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy(t / "art.png", out)
            src = SRC / f"{name}.tex"
            src.parent.mkdir(parents=True, exist_ok=True)
            src.write_text(tex)
        print("wrote", out)


if __name__ == "__main__":
    main()
