// The menus: which panel is up, and the scoreboard, cubby and settings
// filled from the core's screen(). Opening and closing a panel is the
// page's own presentation; everything shown in one comes from the core.
import { fill } from "./hud.js";

const el = (id) => document.getElementById(id);

export function makeScreens(copy) {
  function board(node, s) {
    node.replaceChildren();
    const h = document.createElement("h2");
    h.textContent = copy.board;
    node.append(h);
    const table = document.createElement("table");
    const head = document.createElement("tr");
    for (const k of ["place", "name", "sync", "time"]) {
      const th = document.createElement("th");
      th.textContent = copy[k];
      head.append(th);
    }
    table.append(head);
    for (const e of s.board) {
      const tr = document.createElement("tr");
      if (e.me) tr.className = "me";
      if (e.coach) tr.className = "coach";
      const name = e.coach ? copy.gym_record : e.name + (e.me ? " (" + copy.you + ")" : "");
      for (const [text, cls] of [[e.place, ""], [name, ""], [e.sync || "", ""], [e.time, "t"]]) {
        const td = document.createElement("td");
        td.textContent = text;
        td.className = cls;
        tr.append(td);
      }
      table.append(tr);
    }
    node.append(table);
    const p = document.createElement("h3");
    p.textContent = copy.people;
    node.append(p);
    const ul = document.createElement("ul");
    for (const person of s.people) {
      const li = document.createElement("li");
      li.textContent = person.name + " · " + copy["thing_" + person.held];
      if (person.me) {
        const t = document.createElement("span");
        t.className = "tag";
        t.textContent = copy.you;
        li.append(t);
      }
      if (person.host) {
        const t = document.createElement("span");
        t.className = "tag";
        t.textContent = copy.host_tag;
        li.append(t);
      }
      ul.append(li);
    }
    node.append(ul);
  }

  let shownItems = "";
  function items(s, onPick) {
    const box = el("items");
    const key = JSON.stringify(s.items);
    if (key === shownItems) return;
    shownItems = key;
    box.replaceChildren();
    for (const it of s.items) {
      const b = document.createElement("button");
      b.className = "item" + (it.have ? "" : " locked") + (it.held ? " held" : "");
      const img = document.createElement("img");
      img.src = "art/items/" + it.id + ".png";
      img.alt = "";
      const name = document.createElement("strong");
      name.textContent = copy["thing_" + it.id];
      const need = document.createElement("span");
      need.className = "need";
      need.textContent = it.held ? copy.holding : fill(copy["need_" + it.need], { time: it.time, course: it.course });
      b.append(img, name, need);
      b.disabled = !it.have;
      b.addEventListener("click", () => onPick(it.id));
      box.append(b);
    }
  }

  function settings(s) {
    const st = s.settings;
    el("s-sens").textContent = st.sens;
    el("s-fov").textContent = st.fov;
    el("s-volume").textContent = st.volume_pct + "%";
    el("s-live").checked = st.live;
    el("s-ghosts").checked = st.ghosts;
    el("s-own-ghost").checked = st.own_ghost;
    el("s-shirt").style.background = st.shirt;
    // what the last times file did
    el("s-backup-note").textContent = s.backup ? fill(copy[s.backup.key], s.backup.args) : "";
  }

  // the courses on the title, under tabs (tier 1, tier 2, advanced, bhop):
  // pick one, see your best on it. Which tab a course is under comes from
  // the core; which tab is open is the page's own (it opens on the chosen
  // course's). The cards are rebuilt only when they change: a redraw
  // between a press and its release (leaving the name box saves the name
  // and redraws) would otherwise take the card away and lose the click.
  let shownMaps = "";
  let shownTabs = "";
  let tab = null;
  let latest = null;
  function drawTabs(s, onPick) {
    const tabs = el("map-tabs");
    tabs.replaceChildren();
    for (const g of s.map_groups) {
      const t = document.createElement("button");
      t.className = "map-tab" + (g === tab ? " open" : "");
      t.dataset.group = g;
      t.setAttribute("role", "tab");
      t.setAttribute("aria-selected", String(g === tab));
      // the tab carries the coloured tier mark its cards no longer repeat
      const mark = document.createElement("span");
      mark.className = "tier " + s.tab_shades[g];
      mark.textContent = copy["tab_" + g];
      t.append(mark);
      t.addEventListener("click", () => {
        tab = g;
        maps(latest, onPick);
      });
      tabs.append(t);
    }
  }
  function maps(s, onPick) {
    latest = s;
    el("map-name").textContent = s.map_name;
    if (!s.map_groups.includes(tab)) {
      const chosen = s.maps.find((m) => m.chosen);
      tab = chosen ? chosen.group : s.map_groups[0];
    }
    // the tabs are rebuilt only when they change (not when a course loads),
    // so a redraw cannot take a press on a tab away either
    const tabsKey = JSON.stringify([s.map_groups, tab]);
    if (tabsKey !== shownTabs) {
      shownTabs = tabsKey;
      drawTabs(s, onPick);
    }
    const key = JSON.stringify([s.maps, tab]);
    if (key === shownMaps) return;
    shownMaps = key;
    const box = el("maps");
    box.replaceChildren();
    for (const m of s.maps.filter((m) => m.group === tab)) {
      const b = document.createElement("button");
      b.className = "map-card" + (m.chosen ? " chosen" : "");
      b.dataset.map = m.id;
      const icon = document.createElement("img");
      icon.className = "map-icon";
      icon.src = "art/maps/" + m.id + ".png";
      icon.alt = "";
      const name = document.createElement("strong");
      name.textContent = m.name;
      // the tier only where the tab holds several; a word only for the hard ones
      const rank = document.createElement("span");
      rank.className = "rank";
      if (m.tier_shown) {
        const tier = document.createElement("span");
        tier.className = "tier " + m.shade;
        tier.textContent = fill(copy["course_tier_" + m.mode], { tier: m.tier });
        rank.append(tier);
      }
      if (copy["band_" + m.band]) {
        const band = document.createElement("span");
        band.className = "band";
        band.textContent = copy["band_" + m.band];
        rank.append(band);
      }
      const by = document.createElement("span");
      by.textContent = fill(copy.course_by, { author: m.author });
      const best = document.createElement("span");
      best.textContent = m.best ? fill(copy.course_best, { time: m.best }) : copy.course_none;
      const text = document.createElement("span");
      text.className = "text";
      text.append(name, ...(rank.childElementCount ? [rank] : []), by, best);
      b.append(icon, text);
      b.addEventListener("click", () => onPick(m.id));
      box.append(b);
    }
  }

  return { board, items, settings, maps };
}
