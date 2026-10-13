// Online play, as the page sees it: opens the transport (web/net/mesh.js,
// ptonkle's Ape Race mesh), hands the core every arrival, departure and
// message, and sends what the core leaves in its outbox. The core decides
// everything; this file only carries.
import * as mesh from "./net/mesh.js";

export function makeOnline(game, build) {
  let open = false;
  // the build is in the room's name, so two different builds never meet
  const roomName = (code) => "surfs-up-" + build + "-" + code;

  function onEvent(ev) {
    if (ev.kind === "connected") game.net_peer(ev.peer, Boolean(ev.host));
    else if (ev.kind === "message") game.net_message(ev.peer, ev.text);
    else if (ev.kind === "disconnected") game.net_left(ev.peer);
    else if (ev.kind === "error") game.net_error(ev.text);
  }

  function pump() {
    if (!open) return;
    for (const [to, text] of JSON.parse(game.net_outbox())) {
      if (to) mesh.send(to, text);
      else mesh.broadcast(text);
    }
  }

  return {
    host() {
      const code = game.host_online(Date.now());
      open = true;
      mesh.hostRoom(roomName(code), onEvent);
      return code;
    },
    // false if the text cannot be a room code
    join(code) {
      if (!game.join_online(code)) return false;
      open = true;
      mesh.joinRoom(roomName(game.room_code()), onEvent);
      return true;
    },
    // Pasted codes: no relay at all. The host makes an invitation per
    // joiner and pastes back each reply; the mesh introduces joiners to
    // each other through the host.
    hostByCode() {
      game.host_online(Date.now());
      open = true;
      return mesh.hostByCode(onEvent);
    },
    invite: () => mesh.inviteByCode(),
    accept: (reply) => mesh.acceptAnswer(reply),
    joinByCode(invitation) {
      game.join_online("");
      open = true;
      return mesh.joinByCode(invitation, onEvent);
    },
    leave() {
      game.leave_online();
      mesh.leave();
      open = false;
    },
    pump,
    active: () => open,
  };
}
