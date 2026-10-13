// Online transport settings, ptonkle's Ape Race web/online-config.js, read
// only by web/net/mesh.js, and only after a player presses Host or Join.
//
// The relay list is Raiders of the Lost Mart's (web/net/config.js there),
// which was checked against live relays in October 2026: Trystero's own
// default list, which Ape Race pinned, included relay.mostr.pub, and that
// relay then redirected every handshake. The page's CSP allows wss: for
// connect-src, so this list can change without touching the page.
// None of these numbers is a game rule; each is a transport timing or id.

export const ONLINE = {
  // The vendored Trystero Nostr bundle, pinned by sha256 in
  // web/net/vendor/SHA256SUMS and checked by scripts/build.sh. Imported only
  // when a player hosts or joins by room code.
  bundle: "./vendor/trystero-nostr-0.25.4.js",

  // Namespaces this game's rooms away from other Trystero apps on the same
  // public relays.
  appId: "surfs-up-p2p",

  // Encrypts session descriptions as they cross relays this project does not
  // run, so a relay cannot tamper with a handshake. Not a secret: it ships in
  // this file. Game traffic is DTLS-encrypted end to end regardless.
  password: "surfs-up",

  relayUrls: [
    "wss://nos.lol",
    "wss://nostr.mom",
    "wss://relay.damus.io",
    "wss://relay.primal.net",
    "wss://nostr-01.yakihonne.com",
    "wss://nostr.sathoarder.com",
    "wss://relay.mostro.network",
  ],

  // STUN only: public servers that tell a browser its own public address.
  // Without TURN, two players both behind strict NATs cannot connect.
  stunServers: [
    { urls: "stun:stun.l.google.com:19302" },
    { urls: "stun:stun.cloudflare.com:3478" },
  ],

  // The negotiated data channel's stream id. Both ends create it with this id,
  // so there is no ondatachannel event and no race over who listened first.
  // Not 0 or 1: Trystero opens its own in-band channel, and in-band ids are
  // handed out from zero upwards.
  channelId: 40,

  // How long to wait for ICE gathering before a pasted code is shown anyway.
  gatherTimeoutMs: 6000,

  // How often a link repeats its hello until the other end acknowledges it.
  helloRepeatMs: 250,

  // How long a room code waits for any relay to answer before reporting that
  // the relays look blocked.
  relayTimeoutMs: 15000,
};

// `?ice=none` drops the STUN servers, so tabs on one machine connect on host
// candidates alone and nothing leaves the origin. tests/browser/online.py
// uses it; a player never needs it.
export function iceServers() {
  const ice = new URLSearchParams(location.search).get("ice");
  return ice === "none" ? [] : ONLINE.stunServers;
}
