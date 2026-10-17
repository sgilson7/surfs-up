"""Sea creatures and seaside things, each built of blobs (ellipsoids) for a
themed course's scenery: drawn, never solid. Every function places one
thing on a tools/coursekit.py Course; sizes are in units (a surfer is 72
tall). Faces follow Mage Tower Battle and Protection of the Spheres: dot
eyes with a white catch-light."""
import math
import random


def _at(c, f, l, u, x, y, z):
    return (c[0] + f[0] * x + l[0] * y + u[0] * z,
            c[1] + f[1] * x + l[1] * y + u[1] * z,
            c[2] + f[2] * x + l[2] * y + u[2] * z)


def _axes(yaw):
    r = math.radians(yaw)
    f = (math.cos(r), math.sin(r), 0.0)
    l = (-f[1], f[0], 0.0)
    return f, l, (0.0, 0.0, 1.0)


def eye(k, at, r, yaw):
    """A dot eye with a white catch-light, looking along `yaw`."""
    f, l, u = _axes(yaw)
    k.blob("white", at, r, yaw)
    k.blob("ink", _at(at, f, l, u, r * 0.45, 0, 0), (r * 0.6, r * 0.62, r * 0.62), yaw)
    k.blob("white", _at(at, f, l, u, r * 0.85, r * 0.2, r * 0.3), r * 0.18, yaw)


def fish(k, at, yaw, size, body, fin, pitch=0.0):
    """A round little fish swimming along `yaw`."""
    f, l, u = _axes(yaw)
    k.blob(body, at, (size, size * 0.62, size * 0.75), yaw, pitch)
    k.blob(fin, _at(at, f, l, u, -size * 1.05, 0, 0), (size * 0.38, size * 0.12, size * 0.55), yaw)
    k.blob(fin, _at(at, f, l, u, 0, 0, size * 0.7), (size * 0.4, size * 0.08, size * 0.3), yaw)
    for s in (-1, 1):
        eye(k, _at(at, f, l, u, size * 0.55, s * size * 0.42, size * 0.18), size * 0.2, yaw + s * 35)


def school(k, centre, n, spread, seed, roles, size=40.0, yaw=0.0):
    rnd = random.Random(seed)
    for _ in range(n):
        p = (centre[0] + rnd.uniform(-spread, spread), centre[1] + rnd.uniform(-spread, spread),
             centre[2] + rnd.uniform(-spread * 0.4, spread * 0.4))
        body, fin = rnd.choice(roles)
        fish(k, p, yaw + rnd.uniform(-25, 25), size * rnd.uniform(0.7, 1.3), body, fin)


def coral(k, base, height, role, seed, tip=None):
    """Branching coral: a stack of balls that splits as it rises."""
    rnd = random.Random(seed)
    tip = tip or role
    stack = [(base, 0.0)]
    for _ in range(4):
        nxt = []
        for p, a in stack:
            for _ in range(rnd.choice((1, 2, 2))):
                a2 = a + rnd.uniform(-35, 35)
                d = height / 4.0
                q = (p[0] + math.cos(math.radians(a2 + 90)) * d * 0.4 * rnd.uniform(-1, 1),
                     p[1] + math.sin(math.radians(a2 + 90)) * d * 0.4 * rnd.uniform(-1, 1), p[2] + d)
                k.blob(role, ((p[0] + q[0]) / 2, (p[1] + q[1]) / 2, (p[2] + q[2]) / 2),
                       (height * 0.06, height * 0.06, d * 0.62))
                nxt.append((q, a2))
        stack = nxt[:6]
    for p, _ in stack:
        k.blob(tip, p, height * 0.09)


def brain(k, base, r, role, ridge_role):
    k.blob(role, (base[0], base[1], base[2] + r * 0.45), (r, r, r * 0.7))
    for i in range(5):
        a = i * 72
        k.blob(ridge_role, (base[0] + math.cos(math.radians(a)) * r * 0.5, base[1] + math.sin(math.radians(a)) * r * 0.5,
                            base[2] + r * 0.85), (r * 0.42, r * 0.12, r * 0.1), a)


def anemone(k, base, role, tip, seed, size=90.0):
    rnd = random.Random(seed)
    k.blob(role, (base[0], base[1], base[2] + size * 0.35), (size * 0.6, size * 0.6, size * 0.45))
    for i in range(12):
        a = i * 30 + rnd.uniform(-8, 8)
        lean = rnd.uniform(15, 40)
        r = size * 0.42
        top = (base[0] + math.cos(math.radians(a)) * r, base[1] + math.sin(math.radians(a)) * r, base[2] + size * 1.0)
        k.blob(tip, top, (size * 0.09, size * 0.09, size * 0.42), a, -lean)


def kelp(k, base, height, role, seed):
    rnd = random.Random(seed)
    n = int(height // 120)
    for i in range(n):
        sway = math.sin(i * 0.8 + seed) * 30
        k.blob(role, (base[0] + sway, base[1] + sway * 0.5, base[2] + i * 120 + 60), (26, 50, 80), rnd.uniform(0, 180))


def bubbles(k, base, height, n, seed, role="bubble"):
    rnd = random.Random(seed)
    for i in range(n):
        z = base[2] + height * i / max(n - 1, 1)
        k.blob(role, (base[0] + rnd.uniform(-60, 60), base[1] + rnd.uniform(-60, 60), z), rnd.uniform(10, 30), clear=True)


def turtle(k, front, back, crest_z, height, shell, skin):
    """A sea turtle under a shell ridge running from `back` to `front`."""
    yaw = math.degrees(math.atan2(front[1] - back[1], front[0] - back[0]))
    f, l, u = _axes(yaw)
    head = _at(front, f, l, u, 380, 0, crest_z - height * 0.75 - front[2])
    k.blob(skin, head, (260, 200, 190), yaw)
    for s in (-1, 1):
        eye(k, _at(head, f, l, u, 150, s * 120, 70), 46, yaw + s * 30)
    k.blob("ink", _at(head, f, l, u, 230, 0, -40), (40, 90, 14), yaw)
    mid = ((front[0] + back[0]) / 2, (front[1] + back[1]) / 2, crest_z - height - 40)
    span = math.hypot(front[0] - back[0], front[1] - back[1])
    for s in (-1, 1):
        for x, w in ((span * 0.28, 1.0), (-span * 0.3, 0.7)):
            k.blob(skin, _at(mid, f, l, u, x, s * (height * 1.15), 0), (360 * w, 160 * w, 46), yaw + s * 35 * (1 if x > 0 else -1))
    k.blob(skin, _at(back, f, l, u, -260, 0, crest_z - height * 0.85 - back[2]), (200, 90, 70), yaw)
    k.blob(shell, mid, (span * 0.55, height * 1.05, 70), yaw)


def angler(k, front, crest_z, height, yaw, body, lure, teeth="white"):
    """An anglerfish whose back is a surf ridge ending at `front`: its big
    head past the ridge's end, low, its jaw full of teeth, its lure lit."""
    f, l, u = _axes(yaw)
    head = _at(front, f, l, u, 420, 0, crest_z - height * 1.25 - front[2])
    k.blob(body, head, (520, 560, 420), yaw)
    k.blob(body, _at(head, f, l, u, 260, 0, -320), (420, 470, 180), yaw, -12)
    for i in range(7):
        y = -330 + i * 110
        k.blob(teeth, _at(head, f, l, u, 520, y, -170), (22, 22, 70), yaw, 15)
    for s in (-1, 1):
        eye(k, _at(head, f, l, u, 340, s * 300, 200), 70, yaw + s * 35)
    stalk = _at(head, f, l, u, 180, 0, 420)
    for i in range(6):
        k.blob(body, _at(stalk, f, l, u, i * 80, 120 + i * 30, i * 70 - i * i * 9), 24, yaw)
    k.blob(lure, _at(stalk, f, l, u, 520, 320, 60), 70, yaw, glow=True)


def shark(k, back, front, crest_z, height, body, belly):
    yaw = math.degrees(math.atan2(front[1] - back[1], front[0] - back[0]))
    f, l, u = _axes(yaw)
    span = math.hypot(front[0] - back[0], front[1] - back[1])
    mid = ((front[0] + back[0]) / 2, (front[1] + back[1]) / 2, crest_z - height * 0.9)
    k.blob(belly, _at(mid, f, l, u, 0, 0, -height * 0.25), (span * 0.55, height * 0.95, height * 0.45), yaw)
    nose = _at(front, f, l, u, 340, 0, crest_z - height * 0.6 - front[2])
    k.blob(body, nose, (520, 300, 260), yaw)
    for s in (-1, 1):
        eye(k, _at(nose, f, l, u, 260, s * 200, 80), 40, yaw + s * 40)
        k.blob(body, _at(mid, f, l, u, span * 0.15, s * height * 1.15, -height * 0.2), (300, 160, 30), yaw - s * 30, 0, s * 20)
    k.blob(body, _at(mid, f, l, u, 0, 0, height * 1.05), (220, 30, 260), yaw, -25)
    tail = _at(back, f, l, u, -300, 0, crest_z - height * 0.6 - back[2])
    k.blob(body, tail, (200, 40, 380), yaw, 20)


def whale(k, back, front, crest_z, height, body, belly, side=1):
    yaw = math.degrees(math.atan2(front[1] - back[1], front[0] - back[0]))
    f, l, u = _axes(yaw)
    span = math.hypot(front[0] - back[0], front[1] - back[1])
    mid = ((front[0] + back[0]) / 2, (front[1] + back[1]) / 2, crest_z - height * 0.5)
    k.blob(body, _at(mid, f, l, u, 0, -side * height * 0.8, 0), (span * 0.62, height * 1.3, height * 1.05), yaw)
    k.blob(belly, _at(mid, f, l, u, 0, -side * height * 0.8, -height * 0.55), (span * 0.55, height * 1.1, height * 0.55), yaw)
    k.blob(body, _at(back, f, l, u, -span * 0.12, -side * height * 0.8, 0), (260, 700, 60), yaw)
    eye(k, _at(front, f, l, u, -200, -side * height * 0.15, -height * 0.4), 60, yaw + side * 70)
    k.blob(body, _at(mid, f, l, u, span * 0.2, side * height * 0.1, -height * 0.5), (380, 60, 140), yaw, 0, side * 30)
    for i in range(6):
        k.blob("bubble", _at(front, f, l, u, -160, -side * height * 0.8, height * 0.6 + i * 110), 26 + i * 6, clear=True)


def palm(k, base, height, seed):
    rnd = random.Random(seed)
    lean = rnd.uniform(-12, 12)
    for i in range(6):
        k.blob("trunk", (base[0] + i * lean * 0.6, base[1], base[2] + i * height / 6 + 30), (26, 26, height / 10))
    top = (base[0] + 6 * lean * 0.6, base[1], base[2] + height)
    for i in range(6):
        a = i * 60 + rnd.uniform(-10, 10)
        k.blob("palm", (top[0] + math.cos(math.radians(a)) * 90, top[1] + math.sin(math.radians(a)) * 90, top[2] - 10),
               (120, 34, 10), a, 18)
    k.blob("trunk", (top[0], top[1], top[2] - 10), 22)


def grill(k, at):
    k.blob("grill", (at[0], at[1], at[2] + 40), (40, 40, 24))
    for s in (-1, 1):
        for t in (-1, 1):
            k.blob("grill", (at[0] + s * 24, at[1] + t * 24, at[2] + 16), (4, 4, 18))
    for i in range(3):
        k.blob("hull_red", (at[0] - 18 + i * 18, at[1], at[2] + 66), (7, 16, 7))
    for i in range(4):
        k.blob("bubble", (at[0] + i * 6, at[1], at[2] + 90 + i * 30), 10 + i * 4, clear=True)


def oil_platform(k, deck_c, deck_r, deck_z, floor_z, steel, rust):
    for s in (-1, 1):
        for t in (-1, 1):
            x, y = deck_c[0] + s * deck_r * 0.6, deck_c[1] + t * deck_r * 0.6
            h = deck_z - floor_z
            n = max(int(h // 260), 1)
            for i in range(n):
                k.blob(steel if i % 2 == 0 else rust, (x, y, floor_z + (i + 0.5) * h / n), (40, 40, h / n * 0.55))
    k.blob(rust, (deck_c[0], deck_c[1], deck_z + 160), (60, 60, 160))
    k.blob("lure", (deck_c[0], deck_c[1], deck_z + 340), 36, glow=True)


def rift(k, at, length, yaw, seed):
    rnd = random.Random(seed)
    f, l, u = _axes(yaw)
    for i in range(10):
        x = -length / 2 + length * i / 9
        for s in (-1, 1):
            k.blob("vent_rock", _at(at, f, l, u, x, s * rnd.uniform(220, 320), rnd.uniform(-40, 80)),
                   (rnd.uniform(140, 220), rnd.uniform(110, 160), rnd.uniform(90, 200)), rnd.uniform(0, 180))
        k.blob("ember", _at(at, f, l, u, x, rnd.uniform(-60, 60), -30), (rnd.uniform(60, 110), 40, 24), yaw, glow=True)
