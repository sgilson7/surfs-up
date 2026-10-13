// The heads-up display and the name tags: text the core has already worked
// out (times, speed, sync), put in place. The page computes none of it.

const el = (id) => document.getElementById(id);

export function fill(template, args) {
  let text = template || "";
  for (const [k, v] of Object.entries(args || {})) text = text.split("{" + k + "}").join(String(v));
  return text;
}

export function makeHud(copy) {
  let lastBanner = "";
  let lastLabels = "";
  const keys = ["w", "a", "s", "d", "jump", "duck"];

  function banner(b) {
    const node = el("banner");
    const key = JSON.stringify(b);
    if (key === lastBanner) return;
    lastBanner = key;
    node.replaceChildren();
    if (!b) {
      node.hidden = true;
      return;
    }
    const line = (text, cls) => {
      const d = document.createElement("div");
      d.className = cls;
      d.textContent = text;
      node.append(d);
    };
    const a = b.args || {};
    if (b.key === "banner_finish") {
      line(fill(copy[a.pb ? "banner_finish_pb" : "banner_finish"], a), "big-line");
      if (a.pb && a.was) line(fill(copy.banner_finish_was, a), "line");
      line(fill(copy.banner_finish_stats, a), "line");
      for (const id of a.new || []) line(fill(copy.banner_new_thing, { thing: copy["thing_" + id] }), "line");
    } else if (b.key === "banner_net_error") {
      line(fill(copy.banner_net_error, { key: copy[a.key] || a.key }), "big-line");
    } else {
      line(fill(copy[b.key], a), "big-line");
    }
    node.hidden = false;
  }

  function labels(list) {
    const text = JSON.stringify(list);
    if (text === lastLabels) {
      return;
    }
    lastLabels = text;
    const box = el("labels");
    while (box.children.length < list.length) {
      const d = document.createElement("div");
      d.className = "label";
      box.append(d);
    }
    while (box.children.length > list.length) box.lastChild.remove();
    list.forEach((l, i) => {
      const d = box.children[i];
      d.textContent = l.name;
      d.className = "label" + (l.ghost ? " ghost" : "") + (l.far ? " far" : "");
      d.style.left = l.x + "px";
      d.style.top = l.y + "px";
    });
  }

  return {
    update(h, labelList) {
      el("h-time").textContent = h.time;
      el("h-best").textContent = h.best || "-";
      el("h-stage-pair").hidden = !h.stage;
      el("h-stage").textContent = h.stage || "";
      el("h-speed").textContent = h.speed;
      el("h-speed").className = "speed " + h.trend;
      el("h-sync").textContent = h.sync;
      el("h-strafes").textContent = h.strafes;
      el("h-gain").textContent = h.gain;
      el("h-start").hidden = h.running;
      for (const k of keys) el("k-" + k).classList.toggle("on", h.keys[k]);
      el("h-held").textContent = copy["thing_" + h.held];
      const icon = "art/items/" + h.held + ".png";
      if (el("h-held-icon").getAttribute("src") !== icon) el("h-held-icon").setAttribute("src", icon);
      el("h-people").textContent = h.people > 1 ? fill(copy.hud_people, { n: h.people }) : "";
      banner(h.banner);
      labels(labelList);
    },
  };
}
