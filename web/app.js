// The page draws what the core returns and decides nothing. It loads the
// map and the data files, starts the core through the shim, and each
// display frame passes the clock, the held keys and the mouse movement in,
// then draws what comes out. The core says which screen is up; the page
// shows it.
import init, { Game, numbers } from "./pkg/surf_shim.js";
import { makeRenderer } from "./gl.js";
import { makeInput } from "./input.js";
import { makeHud } from "./hud.js";
import { makeScreens } from "./screens.js";
import { setup as setupSound, wake, play, toggleMute } from "./sound.js";
import { makeOnline } from "./online.js";

const BUILD = "__BUILD__";
const SAVE_KEY = "surfs-up.record";
const el = (id) => document.getElementById(id);
const fetched = (path) => fetch(`${path}?v=${BUILD}`);
const text = (path) => fetched(path).then((r) => r.text());

// Browser storage can be missing or refused; the game works without it.
const storage = {
  load() {
    try {
      return localStorage.getItem(SAVE_KEY) || "";
    } catch {
      return "";
    }
  },
  save(value) {
    try {
      localStorage.setItem(SAVE_KEY, value);
    } catch {
      // nothing to do: the record is simply not kept
    }
  },
};

function fillCopy(copy) {
  for (const node of document.querySelectorAll("[data-copy]")) node.textContent = copy[node.dataset.copy];
  document.title = copy.title;
  el("build").textContent = BUILD;
}

async function start() {
  const names = ["copy", "controls", "sounds", "display"];
  const [copyText, controlsText, soundsText, displayText, vertex, fragment, mapsText] = await Promise.all(
    names
      .map((n) => text(`data/${n}.json`))
      .concat([text("data/shaders/scene.vert"), text("data/shaders/scene.frag"), text("data/maps/maps.json")]),
  );
  const copy = JSON.parse(copyText);
  const display = JSON.parse(displayText);
  fillCopy(copy);
  el("message").textContent = copy.loading;
  setupSound(JSON.parse(soundsText));
  await init();
  const N = JSON.parse(numbers());

  let game;
  try {
    game = new Game(mapsText, storage.load());
  } catch (e) {
    el("message").textContent = String(e);
    document.body.dataset.ready = "true";
    return;
  }

  const canvas = el("view");
  const renderer = makeRenderer(canvas, N, { vertex, fragment }, display);
  if (!renderer) {
    el("message").textContent = copy.no_webgl;
    document.body.dataset.ready = "true";
    return;
  }
  // The core says which course it wants; the page fetches that map file
  // (once each) and hands it over, then uploads the world the core built.
  const mapFiles = new Map();
  let loading = null;
  function ensureMap() {
    const want = game.map_wanted();
    if (want === game.map_loaded() || loading === want) return loading ? loadingPromise : Promise.resolve();
    loading = want;
    el("message").textContent = copy.loading;
    const got = mapFiles.has(want)
      ? Promise.resolve(mapFiles.get(want))
      : fetched(`data/maps/${want}.bin`)
          .then((r) => r.arrayBuffer())
          .then((b) => {
            const bytes = new Uint8Array(b);
            mapFiles.set(want, bytes);
            return bytes;
          });
    loadingPromise = got.then((bytes) => {
      game.load_map(want, bytes);
      renderer.setWorld(game.world_vertices(), game.world_counts());
      loading = null;
      el("message").textContent = "";
      redraw();
    });
    return loadingPromise;
  }
  let loadingPromise = Promise.resolve();

  const hud = makeHud(copy);
  const screens = makeScreens(copy);
  const online = makeOnline(game, BUILD);
  const save = () => storage.save(game.saved());
  let boardHeld = false;
  let shownMode = "";
  let dialog = null;

  const input = makeInput(canvas, JSON.parse(controlsText), N.actions, (command, down) => {
    if (command === "board") boardHeld = down;
    if (command === "next_held" && down) {
      game.next_held();
      save();
    }
    if (command === "mute" && down) toggleMute();
    if (command === "watch" && down && shownMode === "play") game.watch();
    // Escape reaches the page only when the mouse is not captured (a
    // captured mouse is freed by the browser, which pauses below)
    if (command === "pause" && down) {
      game.pause();
      input.free();
      save();
      redraw();
    }
  });

  function resize() {
    const scale = renderer.software ? display.software_scale : window.devicePixelRatio || 1;
    renderer.resize(game.resize(window.innerWidth, window.innerHeight, scale));
  }
  window.addEventListener("resize", resize);
  resize();

  function openDialog(id) {
    closeDialog();
    dialog = id;
    redraw();
  }
  function closeDialog() {
    dialog = null;
    redraw();
  }

  // a button: wake the sound, act, keep the record, redraw
  const act = (fn) => () => {
    wake();
    fn();
    if (document.activeElement && document.activeElement.blur) document.activeElement.blur();
    save();
    redraw();
  };
  const on = (id, fn) => el(id).addEventListener("click", act(fn));
  const onAll = (sel, fn) => document.querySelectorAll(sel).forEach((b) => b.addEventListener("click", act(fn)));

  // ?at=x,y,z,yaw,pitch: a developer's shortcut onto the course (the core ignores it online)
  const devAt = new URLSearchParams(location.search).get("at");
  function surf() {
    if (loading || game.map_loaded() !== game.map_wanted()) return;
    game.set_name(el("name").value);
    game.play();
    if (devAt) game.dev_at(devAt);
    input.capture();
  }
  on("t-play", surf);
  on("t-host", () => {
    online.host();
    surf();
  });
  on("p-host", () => {
    if (!online.active()) online.host();
  });
  on("t-join", () => {
    el("join-error").textContent = "";
    openDialog("join");
  });
  const joinNow = () => {
    if (!online.join(el("room-code").value)) {
      el("join-error").textContent = copy.join_bad;
      return;
    }
    closeDialog();
    surf();
  };
  on("join-go", joinNow);
  el("room-code").addEventListener("keydown", (e) => {
    if (e.key === "Enter") act(joinNow)();
  });
  on("p-resume", () => {
    game.play();
    input.capture();
  });
  on("p-watch", () => {
    game.watch();
    game.play();
    input.capture();
  });
  on("p-restart", () => {
    game.play();
    input.capture();
    pendingReset = true;
  });
  on("p-title", () => {
    online.leave();
    game.to_title();
  });
  on("p-leave", () => online.leave());
  on("p-leave-code", () => {
    online.leave();
    byCode = false;
  });
  // pasted codes: the host's invitation and the joiner's reply
  let byCode = false;
  const showOffer = (p) =>
    p.then(
      (code) => {
        el("inv-offer").value = code;
        el("inv-error").textContent = "";
      },
      () => {
        el("inv-error").textContent = copy.code_bad;
      },
    );
  on("p-host-code", () => {
    byCode = true;
    showOffer(online.hostByCode());
  });
  on("inv-new", () => showOffer(online.invite()));
  on("inv-accept", () => {
    online.accept(el("inv-reply").value).then(
      () => {
        el("inv-reply").value = "";
        el("inv-error").textContent = "";
      },
      () => {
        el("inv-error").textContent = copy.code_bad;
      },
    );
  });
  on("j-make", () => {
    online.joinByCode(el("j-offer").value).then(
      (reply) => {
        el("j-reply").value = reply;
        el("j-error").textContent = "";
      },
      () => {
        el("j-error").textContent = copy.code_bad;
      },
    );
  });
  on("p-copy", () => {
    const link = location.origin + location.pathname + "#room=" + game.room_code();
    if (navigator.clipboard) navigator.clipboard.writeText(link).catch(() => {});
    el("p-copy").textContent = copy.copied;
  });
  onAll(".open-locker", () => openDialog("locker"));
  onAll(".open-help", () => openDialog("help"));
  onAll(".open-settings", () => openDialog("settings"));
  onAll(".close", closeDialog);
  document.querySelectorAll(".nudge").forEach((b) =>
    b.addEventListener("click", act(() => game.nudge(b.dataset.key, b.dataset.up === "true"))),
  );
  on("s-shirt", () => game.cycle_shirt());
  const check = (id, key) => el(id).addEventListener("change", act(() => game.set_setting(key, el(id).checked ? 1 : 0)));
  check("s-live", "live");
  check("s-ghosts", "ghosts");
  check("s-own-ghost", "own_ghost");
  el("name").addEventListener("change", act(() => game.set_name(el("name").value)));

  // Your times in a file. The core writes it; loading hands it to the core,
  // which names the courses its runs need, replays each run on its course's
  // map as that map arrives, and only then counts what finished.
  on("s-backup-save", () => {
    const link = document.createElement("a");
    link.href = URL.createObjectURL(new Blob([game.backup_save()], { type: "application/json" }));
    link.download = copy.backup_file;
    link.click();
    setTimeout(() => URL.revokeObjectURL(link.href));
  });
  on("s-backup-load", () => el("s-backup-file").click());
  const mapBytes = (id) =>
    mapFiles.has(id)
      ? Promise.resolve(mapFiles.get(id))
      : fetched(`data/maps/${id}.bin`)
          .then((r) => r.arrayBuffer())
          .then((b) => new Uint8Array(b));
  el("s-backup-file").addEventListener("change", async () => {
    const file = el("s-backup-file").files[0];
    el("s-backup-file").value = "";
    if (!file) return;
    for (const id of JSON.parse(game.backup_open(await file.text()))) {
      // a map that does not arrive leaves its run unreplayed, which does not count
      const bytes = await mapBytes(id).catch(() => null);
      if (bytes) game.backup_check(id, bytes);
    }
    game.backup_finish();
    save();
    redraw();
  });

  // clicking the view while playing takes the mouse back
  canvas.addEventListener("click", () => {
    wake();
    if (shownMode === "pause" && !dialog) {
      game.play();
      input.capture();
    }
  });
  // Losing a mouse we held pauses. A request the browser refused (a window
  // in the background) is not a loss, and must not pause.
  let hadLock = false;
  document.addEventListener("pointerlockchange", () => {
    if (input.locked()) {
      hadLock = true;
      return;
    }
    if (!hadLock) return;
    hadLock = false;
    input.release();
    game.pause();
    save();
    redraw();
  });
  window.addEventListener("pagehide", save);

  // an invite link: #room=CODE
  const invite = new URLSearchParams(location.hash.slice(1)).get("room");
  if (invite) {
    el("room-code").value = invite;
    openDialog("join");
  }

  let pendingReset = false;

  function redraw() {
    const s = JSON.parse(game.screen());
    shownMode = s.mode;
    document.body.dataset.mode = s.mode;
    el("title").hidden = s.mode !== "title" || dialog !== null;
    el("pause").hidden = s.mode !== "pause" || dialog !== null;
    el("hud").hidden = s.mode === "title";
    for (const id of ["locker", "help", "settings", "join"]) el(id).hidden = dialog !== id;
    el("p-room").hidden = !s.room || byCode;
    el("p-invite").hidden = !s.room || !byCode;
    el("p-host").hidden = Boolean(s.room);
    el("p-host-code").hidden = Boolean(s.room);
    el("p-code").textContent = s.room || "";
    el("p-copy").hidden = !s.room;
    el("p-watch").hidden = !s.room;
    el("p-watch").textContent = copy[s.watching ? "stop_watching" : "watch"];
    if (document.activeElement !== el("name")) el("name").value = s.settings.name;
    screens.board(el("p-board"), s);
    screens.items(s, (id) => act(() => game.hold(id))());
    screens.settings(s);
    screens.maps(s, (id) =>
      act(() => {
        game.choose_map(id);
        ensureMap();
      })(),
    );
    el("click-to-surf").hidden = !(s.mode === "pause" && dialog === null);
  }

  let frames = 0;
  function frame(now) {
    const read = input.read();
    let bits = read.bits;
    if (pendingReset) {
      bits |= N.actions.reset;
      pendingReset = false;
    }
    game.advance(now, bits, read.dx, read.dy);
    renderer.frame(game.view(), game.others_vertices(), game.others_opaque(), game.hands_vertices());
    const h = JSON.parse(game.hud());
    hud.update(h, shownMode === "title" ? [] : JSON.parse(game.labels()));
    const s = JSON.parse(game.screen());
    if (s.mode !== shownMode) redraw();
    el("board").hidden = !(boardHeld && s.mode === "play");
    if (boardHeld) screens.board(el("b-board"), s);
    for (const [kind, gain] of JSON.parse(game.events())) {
      play(kind, gain);
      if (kind === "finish" || kind === "record") {
        save();
        redraw();
      }
    }
    online.pump();
    frames += 1;
    if (frames >= display.save_every) {
      frames = 0;
      save();
    }
    requestAnimationFrame(frame);
  }

  await ensureMap();
  redraw();
  document.body.dataset.ready = "true";
  requestAnimationFrame(frame);
}

start().catch((e) => {
  el("message").textContent = String(e && e.message ? e.message : e);
  document.body.dataset.ready = "true";
});
