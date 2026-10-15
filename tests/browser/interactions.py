"""The project's own interactions for tests/browser/check.py.

A person's first minute: the title and its TikZ art, the help and the cubby,
surfing off the start (the clock starts), falling into the first foam pit
(back to the start), starting over with R, the scoreboard on Tab, pausing
with Escape, and leaving to the title. Selectors and waits only: Firefox's
CSP blocks Playwright's string-form wait_for_function. Headless browsers
cannot capture the mouse, so the view never turns here; the keys still work.
"""


def run(page, check, engine):
    page.wait_for_selector("body[data-mode='title']", state="attached", timeout=10000)
    check(page.is_visible("#title"), "the title screen shows")
    check(page.locator("#title img.title-art").evaluate("i => i.complete && i.naturalWidth > 0"), "the TikZ title art loaded")

    page.click("#title .open-help")
    page.wait_for_selector("#help:not([hidden])", timeout=5000)
    check(page.locator("#help img.help-art").evaluate("i => i.complete && i.naturalWidth > 0"), "the how-to-surf diagram loaded")
    page.click("#help .close")
    page.click("#title .open-locker")
    page.wait_for_selector("#locker:not([hidden])", timeout=5000)
    items = page.locator("#items .item").count()
    locked = page.locator("#items .item.locked").count()
    check(items == 41 and locked == 40, f"the cubby has forty-one things, forty still to earn ({items}, {locked} locked)")
    check("Rubber chicken" in page.inner_text("#items"), "the rubber chicken is in the cubby")
    # taller than the window, the cubby scrolls from its title to its close button
    page.locator("#locker").evaluate("s => s.scrollTop = 0")
    top = page.locator("#locker h2").evaluate("h => h.getBoundingClientRect().top")
    page.locator("#locker").evaluate("s => s.scrollTop = s.scrollHeight")
    bottom = page.locator("#locker .close").evaluate("b => b.getBoundingClientRect().bottom - innerHeight")
    check(top >= 0 and bottom <= 0, f"the whole cubby scrolls into view (title at {top:.0f}, close {bottom:.0f} past the bottom)")
    page.click("#locker .close")

    page.fill("#name", "Tester")
    page.click("#t-play")
    page.wait_for_selector("body[data-mode='play']", state="attached", timeout=5000)
    check(page.is_visible("#hud"), "surfing shows the HUD")
    check("Clasped hands" in page.inner_text(".held"), "you start with clasped hands")
    check(page.is_visible("#h-start"), "the clock waits on the ledge")

    # a slow machine (a CI runner draws without a GPU) runs the game slower
    # than real time, so walk until the clock starts rather than for a set time
    page.keyboard.down("KeyW")
    page.wait_for_timeout(800)
    speed = page.inner_text("#h-speed")
    for _ in range(100):
        if page.inner_text("#h-time") != "0:00.000":
            break
        page.wait_for_timeout(100)
    page.keyboard.up("KeyW")
    check(speed not in ("", "0"), f"walking forward moves ({speed} u/s)")
    check(page.inner_text("#h-time") != "0:00.000", f"dropping off the ledge starts the clock ({page.inner_text('#h-time')})")

    # off the ledge with no keys held, you slide off the ridge into the pit
    page.wait_for_timeout(3000)
    page.keyboard.down("KeyR")
    page.wait_for_timeout(200)
    page.keyboard.up("KeyR")
    for _ in range(50):
        if page.inner_text("#h-time") == "0:00.000":
            break
        page.wait_for_timeout(100)
    check(page.inner_text("#h-time") == "0:00.000", "R sends you back to the start and stops the clock")

    page.keyboard.down("Tab")
    page.wait_for_selector("#board:not([hidden])", timeout=10000)
    check("Coach Rita" in page.inner_text("#board"), "Tab shows the scoreboard with the gym record")
    page.keyboard.up("Tab")

    page.keyboard.press("Escape")
    page.wait_for_selector("body[data-mode='pause']", state="attached", timeout=5000)
    check(page.is_visible("#pause"), "Escape pauses")
    page.click("#p-title")
    page.wait_for_selector("body[data-mode='title']", state="attached", timeout=5000)
    check(page.input_value("#name") == "Tester", "your name is kept")

    # the second course: surf_kitsune, nine stages, the clock from the red
    # door. A CPU renderer (CI) draws kitsune's whole world slowly enough that
    # the 900-unit walk from the spawn can outlast any fair wait, so the
    # developer's ?at= shortcut sets the walker down 150 units short of the door.
    page.goto(page.url.split("?")[0].split("#")[0] + "?at=0,-700,200,90")
    page.wait_for_selector("body[data-mode='title']", state="attached", timeout=10000)
    check(page.locator("#maps .map-card").count() == 8, "eight courses on the title")
    tiers = [page.locator("#maps .map-card .tier").nth(i).inner_text() for i in range(8)]
    check(tiers[0] == "Tier 1" and "Tier 8" in tiers and tiers[-1].startswith("Bhop"), f"each course card shows its tier, easiest first, the bhop course last {tiers}")
    anubis = page.locator("#maps .map-card:has-text('surf_anubis')").inner_text()
    check("Experts only" in anubis, "anubis warns that it is for experts")
    # a name typed just before choosing: leaving the box redraws the menus,
    # which must not swallow the click on the course
    page.fill("#name", "Tester K")
    page.click("#maps .map-card:has-text('surf_kitsune')")
    for _ in range(100):
        if "surf_kitsune" in page.inner_text("#map-name"):
            break
        page.wait_for_timeout(100)
    check("surf_kitsune" in page.inner_text("#map-name"), "choosing surf_kitsune loads it")
    page.click("#t-play")
    page.wait_for_selector("body[data-mode='play']", state="attached", timeout=5000)
    # the game still runs slower than real time there, so allow three minutes
    page.keyboard.down("KeyW")
    for _ in range(1800):
        if page.inner_text("#h-time") != "0:00.000":
            break
        page.wait_for_timeout(100)
    page.keyboard.up("KeyW")
    state = page.evaluate("() => [document.body.dataset.mode, document.getElementById('h-speed').textContent, document.getElementById('h-time').textContent, document.getElementById('message').textContent]")
    check(page.inner_text("#h-time") != "0:00.000", f"walking through the red door starts the clock {state}")
    check(page.inner_text("#h-stage") == "1/9", f"stage 1 of 9 ({page.inner_text('#h-stage')})")
    page.keyboard.press("Escape")
    page.wait_for_selector("body[data-mode='pause']", state="attached", timeout=5000)
    page.click("#p-title")
    page.wait_for_selector("body[data-mode='title']", state="attached", timeout=5000)
    page.click("#maps .map-card:has-text('surf_utopia')")
