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
    check(items == 6 and locked == 5, f"the cubby has six things, five still to earn ({items}, {locked} locked)")
    check("Rubber chicken" in page.inner_text("#items"), "the rubber chicken is in the cubby")
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
