// Keyboard and mouse, turned into the action bits the core defines.
// data/controls.json names the action for each key and mouse button, and
// the commands that are the page's own (the board, the next thing to hold);
// the core's numbers() gives each action its bit. With the pointer captured,
// mouse movement turns the view. Nothing here decides what an action does.
// Raiders of the Lost Mart's web/input.js, with the scroll wheel added: surf
// players bind jump to it.

export function makeInput(canvas, controls, actions, onCommand) {
  // `tapped` keeps a press that is released before the next frame reads it
  const state = { held: 0, tapped: 0, dx: 0, dy: 0 };
  const bitOf = (code) => actions[controls.actions[code]] || 0;
  const locked = () => document.pointerLockElement === canvas;

  function release() {
    state.held = 0;
    state.tapped = 0;
  }

  function down(code) {
    const bit = bitOf(code);
    state.held |= bit;
    state.tapped |= bit;
    return bit;
  }

  window.addEventListener("keydown", (e) => {
    if (e.target && (e.target.tagName === "INPUT" || e.target.tagName === "TEXTAREA")) return;
    const command = controls.commands[e.code];
    if (command) {
      e.preventDefault();
      if (!e.repeat) onCommand(command, true);
      return;
    }
    if (down(e.code)) e.preventDefault();
  });
  window.addEventListener("keyup", (e) => {
    const command = controls.commands[e.code];
    if (command) onCommand(command, false);
    state.held &= ~bitOf(e.code);
  });
  canvas.addEventListener("mousedown", (e) => {
    if (!locked()) return;
    down("Mouse" + e.button);
    e.preventDefault();
  });
  window.addEventListener("mouseup", (e) => {
    state.held &= ~bitOf("Mouse" + e.button);
  });
  window.addEventListener(
    "wheel",
    (e) => {
      if (!locked()) return;
      state.tapped |= bitOf("Wheel");
      e.preventDefault();
    },
    { passive: false },
  );
  window.addEventListener("mousemove", (e) => {
    if (!locked()) return;
    state.dx += e.movementX;
    state.dy += e.movementY;
  });
  canvas.addEventListener("contextmenu", (e) => e.preventDefault());
  window.addEventListener("blur", release);

  return {
    release,
    locked,
    capture() {
      if (locked() || !canvas.requestPointerLock) return;
      // raw mouse counts where the browser offers them, as a surfer expects
      let p = null;
      try {
        p = canvas.requestPointerLock({ unadjustedMovement: true });
      } catch (e) {
        p = null;
      }
      if (p && p.catch) {
        p.catch(() => {
          const q = canvas.requestPointerLock();
          if (q && q.catch) q.catch(() => {});
        });
      }
    },
    free() {
      if (locked()) document.exitPointerLock();
    },
    // The held bits, and the movement since the last read.
    read() {
      const out = { bits: state.held | state.tapped, dx: state.dx, dy: state.dy };
      state.tapped = 0;
      state.dx = 0;
      state.dy = 0;
      return out;
    },
  };
}
