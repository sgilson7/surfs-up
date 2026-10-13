#!/usr/bin/env python3
"""Eight browsers surf together online, introduced by pasted codes.

After Raiders of the Lost Mart's tests/browser/online.py. `?ice=none` drops
the STUN servers, so the browsers connect on host candidates alone and no
request leaves the origin; the room-code path (Trystero over public relays)
is checked by hand (HANDOFF.md). The host invites seven joiners one at a
time; the mesh introduces the joiners to each other through the host.

The host's saved game holds a finished run (tests/browser/fixtures/
saved_best.json, a real recording), which it sends once to each newcomer:
every joiner must show it on the scoreboard. Then one joiner walks off the
ledge, and every other browser must draw them (a name tag) and see them move.

It fails on: a joiner never connecting, a browser not seeing all eight, the
run not reaching every joiner, a live surfer not moving on the others'
screens, a page error or console error, or a request off the origin.

    python3 tests/browser/online.py [chromium firefox ...]   (default: chromium)
    SURF_PEOPLE=4 python3 tests/browser/online.py             (fewer browsers)
"""
import functools
import http.server
import json
import os
import pathlib
import sys
import threading

from playwright.sync_api import sync_playwright

ROOT = pathlib.Path(__file__).resolve().parents[2]
DIST = ROOT / "dist"
COPY = json.loads((ROOT / "data" / "copy.json").read_text())
SAVED = (ROOT / "tests" / "browser" / "fixtures" / "saved_best.json").read_text()
PEOPLE = int(os.environ.get("SURF_PEOPLE", "8"))
failures = []


def check(ok, what):
    print(("  PASS " if ok else "  FAIL ") + what)
    if not ok:
        failures.append(what)


class Quiet(http.server.SimpleHTTPRequestHandler):
    extensions_map = {**http.server.SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm", ".js": "text/javascript"}

    def log_message(self, *a):
        pass


def serve():
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), functools.partial(Quiet, directory=str(DIST)))
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv


def label_positions(page):
    return page.locator("#labels .label:not(.ghost)").evaluate_all("ls => ls.map(l => [l.textContent, l.style.left, l.style.top])")


def run(p, engine, origin):
    print(f"\n[{engine}] online, {PEOPLE} browsers")
    # Browsers hide a machine's own addresses behind mDNS names, which a CI
    # runner may not resolve; the browsers here share one machine, so plain
    # local addresses are what lets them meet without STUN.
    options = {
        "chromium": {"args": ["--disable-features=WebRtcHideLocalIpsWithMdns"]},
        "firefox": {"firefox_user_prefs": {"media.peerconnection.ice.obfuscate_host_addresses": False}},
    }.get(engine, {})
    browser = getattr(p, engine).launch(**options)
    seen = {"console": [], "errors": [], "foreign": []}
    pages = []

    def open_page(i, saved=None):
        ctx = browser.new_context()
        ctx.on("request", lambda r: None if r.url.startswith(origin) else seen["foreign"].append(r.url))
        if saved is not None:
            ctx.add_init_script("localStorage.setItem('surfs-up.record', " + json.dumps(saved) + ");")
        page = ctx.new_page()
        page.on("console", lambda m: seen["console"].append(f"{i}: {m.text}") if m.type == "error" else None)
        page.on("pageerror", lambda e: seen["errors"].append(f"{i}: {e}"))
        page.goto(origin + "/index.html?ice=none")
        page.wait_for_selector("body[data-ready='true']", state="attached", timeout=30000)
        pages.append(page)
        return page

    host = open_page(0, SAVED)
    host.wait_for_selector("body[data-mode='title']", state="attached")
    host.click("#t-play")
    host.keyboard.press("Escape")
    host.wait_for_selector("body[data-mode='pause']", state="attached")
    host.click("#p-host-code")
    for _ in range(200):
        if host.input_value("#inv-offer"):
            break
        host.wait_for_timeout(100)
    check(bool(host.input_value("#inv-offer")), "the host has an invitation")

    for i in range(1, PEOPLE):
        j = open_page(i)
        j.fill("#name", f"J{i}")
        j.click("#t-join")
        j.click("#join-paste summary")
        j.fill("#j-offer", host.input_value("#inv-offer"))
        j.click("#j-make")
        for _ in range(100):
            if j.input_value("#j-reply"):
                break
            j.wait_for_timeout(100)
        reply = j.input_value("#j-reply")
        check(bool(reply), f"joiner {i} made a reply")
        host.fill("#inv-reply", reply)
        host.click("#inv-accept")
        j.click("#join .close")
        j.click("#t-play")
        # wait until the host counts this joiner, then make the next invitation
        want = COPY["hud_people"].replace("{n}", str(i + 1))
        for _ in range(150):
            if host.inner_text("#h-people") == want:
                break
            host.wait_for_timeout(100)
        check(host.inner_text("#h-people") == want, f"the host sees joiner {i} ({host.inner_text('#h-people')})")
        if i < PEOPLE - 1:
            before = host.input_value("#inv-offer")
            host.click("#inv-new")
            for _ in range(100):
                if host.input_value("#inv-offer") and host.input_value("#inv-offer") != before:
                    break
                host.wait_for_timeout(100)

    # the host may stay paused: a paused browser still runs the room and draws
    # the others; resume only if the button is there to press
    if host.is_visible("#p-resume"):
        try:
            host.click("#p-resume", timeout=5000)
        except Exception:
            pass
    everyone = COPY["hud_people"].replace("{n}", str(PEOPLE))
    for _ in range(200):
        if all(pg.inner_text("#h-people") == everyone for pg in pages):
            break
        pages[0].wait_for_timeout(100)
    for i, pg in enumerate(pages):
        check(pg.inner_text("#h-people") == everyone, f"browser {i} sees all {PEOPLE} ({pg.inner_text('#h-people')})")

    # the host's finished run reached everyone, once, whole
    for i, pg in enumerate(pages[1:], 1):
        pg.keyboard.down("Tab")
        pg.wait_for_timeout(150)
        board = pg.inner_text("#b-board")
        pg.keyboard.up("Tab")
        check("Hana" in board, f"joiner {i} has the host's run on the scoreboard")

    # one joiner walks forward off the ledge; the others, who all stand on
    # the start and face the same way, draw them ahead and moving
    walker = pages[1]
    watchers = [pg for pg in pages if pg is not walker]
    # a browser that held the mouse and lost it to another window pauses
    # (Firefox grants the lock to a window in front); bring the walker back
    walker.bring_to_front()
    if walker.is_visible("#pause"):
        walker.click("#p-resume")
    walker.keyboard.down("KeyW")
    walker.wait_for_timeout(700)
    first = [dict((n, (x, y)) for n, x, y in label_positions(pg)) for pg in watchers]
    walker.wait_for_timeout(700)
    second = [dict((n, (x, y)) for n, x, y in label_positions(pg)) for pg in watchers]
    walker.keyboard.up("KeyW")
    for k, (a, b) in enumerate(zip(first, second)):
        check("J1" in a and "J1" in b, f"watcher {k} draws J1 surfing ahead")
        check(a.get("J1") != b.get("J1"), f"watcher {k} sees J1 move ({a.get('J1')} -> {b.get('J1')})")

    # spectating, as in Ape Race: V watches someone in the gym, Space stops
    spectator = watchers[1]
    spectator.bring_to_front()
    if spectator.is_visible("#pause"):
        spectator.click("#p-resume")
    spectator.keyboard.press("KeyV")
    spectator.wait_for_timeout(300)
    shown = spectator.is_visible("#h-watch")
    text = spectator.inner_text("#h-watch") if shown else ""
    watching = COPY["hud_watching"].split("{")[0]
    check(shown and watching in text, f"V watches someone in the gym ({text!r})")
    spectator.keyboard.press("Space")
    spectator.wait_for_timeout(300)
    check(not spectator.is_visible("#h-watch"), "Space goes back to surfing")

    check(not seen["errors"], f"no page error {seen['errors'][:3]}")
    check(not seen["console"], f"no console error {seen['console'][:3]}")
    check(not seen["foreign"], f"no request off the origin {seen['foreign'][:3]}")
    browser.close()


def main():
    engines = sys.argv[1:] or ["chromium"]
    srv = serve()
    origin = f"http://127.0.0.1:{srv.server_address[1]}"
    with sync_playwright() as p:
        for e in engines:
            run(p, e, origin)
    srv.shutdown()
    if failures:
        print(f"\nONLINE CHECK FAILED: {len(failures)} problem(s)")
        sys.exit(1)
    print("\nONLINE CHECK PASSED")


if __name__ == "__main__":
    main()
