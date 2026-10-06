// The code map as a navigation bar (maintainer, 2026-10-06): a thin strip at
// the top of every deep dive, tutorial and API page, under site-nav.js's
// breadcrumb, that expands to the full outram-park code map (gh:#734).
//
// site-nav.js loads this script on pages with a <main> under deep-dives/,
// tutorials/ and api/ (never on demos, whose screen belongs to the demo).
// Nothing here knows the layout or the crate list. It reads, from code-map/:
//   code_map.json   crates, topics, rows, dirs (kovan-cli code-map, at build)
//   code_map.svg    the drawn map (same tool), inlined only when expanded
//   site_links.json which deep dives, tutorials and rustdoc the site has, each
//                   with its mdBook directory (scripts/build-pages.sh writes
//                   it from docs/site/deep-dives.txt, tutorials.txt and
//                   lesson-crates.txt)
// A crate owns a book when the book's directory is inside the crate's `dir`
// (crates/boon-lay/docs/lessons -> crates/boon-lay -> boon-lay).
//
// Collapsed: a toggle, one chip per topic box (the current page's crate named
// in its topic's chip), and "Full map". Expanded: the SVG in its own box with
// + / - / Fit, drag to pan; tapping a crate goes to its deep dive, else its
// API reference, else code-map/#<crate>. The open/closed state is remembered
// per viewer in localStorage when it is available.
//
// Mobile-first (docs/claude-md/mobile-first-tutorials-and-demos.md): the chip
// row scrolls inside itself, so the page never scrolls sideways; the map box
// is at most 55 % of the screen high; buttons are 36 px. No lag: the two small
// JSON files are fetched once after load; the 65 kB SVG only on first expand.
(function () {
  "use strict";
  var me = document.currentScript;
  if (!me) return;
  var root = new URL(".", me.src);
  var mapDir = new URL("code-map/", root);
  var here = new URL(location.href);
  var parts = here.pathname.slice(root.pathname.length).split("/");
  var kind = parts[0], book = parts[1] || "";
  var KEY = "op-code-map-open";

  // Topic boxes in the map's left-to-right order; a topic not listed here
  // (added later to the tags) is appended alphabetically, never dropped.
  var ORDER = ["app", "neutronics", "thermal-hydraulics", "fuel-performance",
    "structural-mechanics", "chemistry", "fuel-cycle", "granular-dem", "risk",
    "utility", "knowledge-management"];
  var TOPIC_NAME = { "app": "Apps", "utility": "Utilities", "knowledge-management": "Kovan",
    "granular-dem": "Granular DEM" };
  function topicName(t) {
    if (TOPIC_NAME[t]) return TOPIC_NAME[t];
    var s = t.replace(/-/g, " ");
    return s.charAt(0).toUpperCase() + s.slice(1);
  }

  function load(key) { try { return localStorage.getItem(key); } catch (e) { return null; } }
  function save(key, v) { try { localStorage.setItem(key, v); } catch (e) { /* private mode */ } }
  function getJson(name) {
    return fetch(new URL(name, mapDir).href).then(function (r) {
      if (!r.ok) throw new Error(name + ": " + r.status);
      return r.json();
    });
  }

  // ---- DOM ----------------------------------------------------------------
  var css = document.createElement("style");
  css.textContent =
    ".op-cm{font:13px/1.35 system-ui,-apple-system,'Segoe UI',sans-serif;margin:0 0 14px;" +
    "border:1px solid rgba(128,128,128,.35);border-radius:8px;max-width:100%;overflow:hidden}" +
    ".op-cm-strip{display:flex;align-items:center;gap:6px;padding:4px 6px}" +
    ".op-cm button{font:inherit;font-weight:600;min-height:36px;min-width:36px;padding:0 10px;" +
    "border-radius:6px;border:1px solid rgba(128,128,128,.45);background:rgba(128,128,128,.10);" +
    "color:inherit;cursor:pointer;white-space:nowrap}" +
    ".op-cm button:hover{border-color:#1f6f78}" +
    ".op-cm-chips{display:flex;gap:6px;flex:1;min-width:0;overflow-x:auto;scrollbar-width:thin;padding:2px 0}" +
    ".op-cm-chips button{flex:none;font-weight:500;font-size:12px;min-height:32px;padding:0 8px;margin:0}" +
    // Wide screens: the chips wrap (two short rows); narrow: one row that
    // scrolls inside the strip, never the page.
    "@media (min-width:900px){.op-cm-strip{flex-wrap:wrap;gap:4px}.op-cm-chips{display:contents}" +
    ".op-cm-chips button{min-height:28px;padding:0 6px}.op-cm-strip>button{min-height:28px}" +
    ".op-cm-full{min-height:28px;line-height:28px}}" +
    ".op-cm-chips button .op-cm-n{opacity:.6;margin-left:5px}" +
    ".op-cm-chips button.op-here{border-color:#d9480f;background:rgba(217,72,15,.14);font-weight:700}" +
    ".op-cm-full{white-space:nowrap;padding:0 4px;min-height:36px;line-height:36px}" +
    ".op-cm-box{position:relative;height:min(55vh,400px);border-top:1px solid rgba(128,128,128,.35);" +
    "touch-action:none;cursor:grab;overflow:hidden;background:rgba(128,128,128,.06)}" +
    ".op-cm-box.op-drag{cursor:grabbing}" +
    ".op-cm-box svg{position:absolute;inset:0;width:100%;height:100%;display:block}" +
    ".op-cm-box svg .bg{fill:transparent}" +
    ".op-cm-box .card{cursor:pointer}" +
    ".op-cm-box .card.op-here .card-bg{stroke:#d9480f;stroke-width:6}" +
    ".op-cm-box .card.op-has-page .name{text-decoration:underline}" +
    ".op-cm-tools{position:absolute;top:6px;right:6px;display:flex;gap:6px}" +
    ".op-cm-tools button{background:rgba(255,255,255,.92);color:#1d2125}" +
    ".op-cm-status{position:absolute;left:6px;right:6px;bottom:4px;font-size:12px;pointer-events:none;" +
    "color:#1d2125;text-shadow:0 0 3px #fff,0 0 3px #fff}" +
    "@media (prefers-color-scheme: dark){.op-cm-tools button{background:rgba(28,32,36,.92);color:#e6e3dd}" +
    ".op-cm-status{color:#e6e3dd;text-shadow:0 0 3px #000,0 0 3px #000}}";
  document.head.appendChild(css);

  var wrap = document.createElement("nav");
  wrap.className = "op-cm";
  wrap.setAttribute("aria-label", "Code map");
  var strip = document.createElement("div");
  strip.className = "op-cm-strip";
  var toggle = document.createElement("button");
  toggle.type = "button";
  toggle.setAttribute("aria-expanded", "false");
  toggle.title = "Show or hide the outram-park code map";
  var chips = document.createElement("div");
  chips.className = "op-cm-chips";
  var full = document.createElement("a");
  full.className = "op-cm-full";
  full.href = mapDir.href;
  full.textContent = "Full map";
  strip.appendChild(toggle);
  strip.appendChild(chips);
  strip.appendChild(full);
  wrap.appendChild(strip);

  var box = null, svg = null, whole = null, vb = null, status = null;
  var data = null, links = null, hereCrates = [];

  function place() {
    var main = document.querySelector("main");
    if (!main) return false;
    var nav = main.querySelector(".op-site-nav");
    if (nav && nav.parentNode === main) main.insertBefore(wrap, nav.nextSibling);
    else main.insertBefore(wrap, main.firstChild);
    return true;
  }

  // ---- Where a crate goes, and which crates this page is about -------------
  function ownsDir(c, dir) { return c.dir && dir.indexOf(c.dir.replace(/\/$/, "") + "/") === 0; }
  function pageOf(c) {
    var i;
    for (i = 0; i < links.deep_dives.length; i++)
      if (ownsDir(c, links.deep_dives[i].dir)) return { url: links.deep_dives[i].url, what: "deep dive" };
    var u = c.name.replace(/-/g, "_");
    for (i = 0; i < links.api.length; i++)
      if (links.api[i].replace(/-/g, "_") === u) return { url: "api/" + u + "/index.html", what: "API reference" };
    return null;
  }
  function hrefOf(c) {
    var p = pageOf(c);
    return p ? new URL(p.url, root).href : new URL("#" + encodeURIComponent(c.name), mapDir).href;
  }
  function findHere() {
    var out = [], list = kind === "deep-dives" ? links.deep_dives : kind === "tutorials" ? links.tutorials : null;
    data.crates.forEach(function (c) {
      if (list) {
        list.forEach(function (b) { if (b.url === kind + "/" + book + "/" && ownsDir(c, b.dir)) out.push(c.name); });
      } else if (kind === "api" && c.name.replace(/-/g, "_") === book) {
        out.push(c.name);
      }
    });
    return out;
  }

  function buildChips() {
    var topics = {}, names = [];
    data.crates.forEach(function (c) {
      if (!topics[c.topic]) { topics[c.topic] = []; names.push(c.topic); }
      topics[c.topic].push(c);
    });
    names.sort(function (a, b) {
      var ia = ORDER.indexOf(a), ib = ORDER.indexOf(b);
      if (ia < 0) ia = 999; if (ib < 0) ib = 999;
      return ia - ib || (a < b ? -1 : a > b ? 1 : 0);
    });
    chips.innerHTML = "";
    names.forEach(function (t) {
      var b = document.createElement("button");
      b.type = "button";
      var mine = topics[t].filter(function (c) { return hereCrates.indexOf(c.name) >= 0; });
      b.textContent = topicName(t) + (mine.length ? ": " + mine.map(function (c) { return c.name; }).join(", ") : "");
      var n = document.createElement("span");
      n.className = "op-cm-n";
      n.textContent = topics[t].length;
      b.appendChild(n);
      b.title = topics[t].map(function (c) { return c.name; }).join(", ");
      if (mine.length) b.className = "op-here";
      b.onclick = function () { open(true, topics[t].map(function (c) { return c.name; })); };
      chips.appendChild(b);
    });
    // The current page's topic first in view on a narrow strip.
    var h = chips.querySelector(".op-here");
    if (h && chips.scrollWidth > chips.clientWidth)
      chips.scrollLeft = Math.max(0, h.offsetLeft - chips.offsetLeft - 8);
  }

  // ---- The expanded map -----------------------------------------------------
  function apply() { svg.setAttribute("viewBox", vb.x + " " + vb.y + " " + vb.w + " " + vb.h); }
  function frame(x, y, w, h) {
    // Show the rectangle whole, centred, keeping the box's aspect ratio.
    var r = box.getBoundingClientRect();
    if (!r.width || !r.height) return;
    var s = Math.max(w / r.width, h / r.height);
    var W = r.width * s, H = r.height * s;
    vb = { x: x - (W - w) / 2, y: y - (H - h) / 2, w: W, h: H };
    apply();
  }
  function fit() { frame(whole.x, whole.y, whole.w, whole.h); }
  // Map units across the box at which a card's name is readable: about 1.2
  // units per pixel (a 230-unit card is ~190 px wide), whatever the screen.
  function readable() { return Math.max(400, 1.2 * box.getBoundingClientRect().width); }
  // Frame a set of crates' cards at a readable scale: all of them when they
  // fit, else their left end (the highest-fidelity column), to be panned.
  function focus(names) {
    var x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
    names.forEach(function (n) {
      var g = svg.querySelector('.card[data-crate="' + n.replace(/"/g, "") + '"]');
      if (!g) return;
      var b = g.getBBox();
      x0 = Math.min(x0, b.x); y0 = Math.min(y0, b.y);
      x1 = Math.max(x1, b.x + b.width); y1 = Math.max(y1, b.y + b.height);
    });
    if (x0 === Infinity) { fit(); return; }
    var r = box.getBoundingClientRect();
    if (!r.width || !r.height) return;
    var w = readable(), h = w * r.height / r.width, cy = (y0 + y1) / 2;
    var x = x1 - x0 <= w ? (x0 + x1) / 2 - w / 2 : x0 - 20;
    vb = { x: x, y: cy - h / 2, w: w, h: h };
    apply();
  }
  function zoom(f) {
    var cx = vb.x + vb.w / 2, cy = vb.y + vb.h / 2;
    var w = Math.min(Math.max(vb.w / f, 150), whole.w * 2), h = vb.h * w / vb.w;
    vb = { x: cx - w / 2, y: cy - h / 2, w: w, h: h };
    apply();
  }
  function home() { if (hereCrates.length) focus(hereCrates); else fit(); }

  function buildBox() {
    box = document.createElement("div");
    box.className = "op-cm-box";
    var tools = document.createElement("div");
    tools.className = "op-cm-tools";
    [["+", "Zoom in", function () { zoom(1.4); }],
     ["−", "Zoom out", function () { zoom(1 / 1.4); }],
     ["Fit", "Whole map in view", function () { fit(); }]].forEach(function (t) {
      var b = document.createElement("button");
      b.type = "button"; b.textContent = t[0]; b.title = t[1]; b.setAttribute("aria-label", t[1]);
      b.onclick = function () { if (svg) t[2](); };
      tools.appendChild(b);
    });
    status = document.createElement("div");
    status.className = "op-cm-status";
    status.textContent = "Loading the map…";
    box.appendChild(tools);
    box.appendChild(status);
    wrap.appendChild(box);

    var drag = null;
    box.addEventListener("pointerdown", function (e) {
      if (!svg || e.target.closest("button")) return;
      drag = { x: e.clientX, y: e.clientY, vb: { x: vb.x, y: vb.y }, moved: false };
      box.setPointerCapture(e.pointerId);
    });
    box.addEventListener("pointermove", function (e) {
      if (!drag) return;
      var dx = e.clientX - drag.x, dy = e.clientY - drag.y;
      if (Math.abs(dx) + Math.abs(dy) > 5) { drag.moved = true; box.classList.add("op-drag"); }
      if (!drag.moved) return;
      var r = box.getBoundingClientRect();
      vb.x = drag.vb.x - dx * vb.w / r.width; vb.y = drag.vb.y - dy * vb.h / r.height;
      apply();
    });
    box.addEventListener("pointerup", function (e) {
      if (!drag) return;
      var moved = drag.moved; drag = null; box.classList.remove("op-drag");
      if (moved) return;
      var hit = document.elementFromPoint(e.clientX, e.clientY);
      var card = hit && hit.closest ? hit.closest(".card") : null;
      if (!card) return;
      var c = byName(card.getAttribute("data-crate"));
      if (!c) return;
      var p = pageOf(c);
      status.textContent = "Opening " + c.name + (p ? " (" + p.what + ")" : " on the full map") + "…";
      location.href = hrefOf(c);
    });
    box.addEventListener("pointercancel", function () { drag = null; box.classList.remove("op-drag"); });

    fetch(new URL("code_map.svg", mapDir).href).then(function (r) {
      if (!r.ok) throw new Error("code_map.svg: " + r.status);
      return r.text();
    }).then(function (text) {
      box.insertAdjacentHTML("afterbegin", text);
      svg = box.querySelector("svg");
      svg.removeAttribute("width"); svg.removeAttribute("height");
      svg.setAttribute("preserveAspectRatio", "xMidYMid meet");
      var v = svg.getAttribute("viewBox").split(/\s+/).map(Number);
      whole = { x: v[0], y: v[1], w: v[2], h: v[3] };
      whenData(function () {
        var cards = svg.querySelectorAll(".card");
        for (var i = 0; i < cards.length; i++) {
          var c = byName(cards[i].getAttribute("data-crate"));
          if (!c) continue;
          if (hereCrates.indexOf(c.name) >= 0) cards[i].classList.add("op-here");
          if (pageOf(c)) cards[i].classList.add("op-has-page");
        }
        status.textContent = "Tap a crate: its deep dive (underlined), else its API or its place on the full map. Drag to pan.";
        if (pendingFocus) { focus(pendingFocus); pendingFocus = null; } else home();
      });
    }).catch(function (err) { status.textContent = "Could not load the map: " + err.message; });
  }
  function byName(n) {
    for (var i = 0; i < data.crates.length; i++) if (data.crates[i].name === n) return data.crates[i];
    return null;
  }

  var pendingFocus = null, waiting = [];
  function whenData(f) { if (data && links) f(); else waiting.push(f); }

  function open(on, focusNames) {
    toggle.textContent = on ? "▾ Map" : "▸ Map";
    toggle.setAttribute("aria-expanded", on ? "true" : "false");
    save(KEY, on ? "1" : "0");
    if (!on) { if (box) box.style.display = "none"; return; }
    if (!box) { pendingFocus = focusNames || null; buildBox(); return; }
    box.style.display = "";
    if (!svg) { pendingFocus = focusNames || pendingFocus; return; }
    if (focusNames) focus(focusNames); else home();
  }
  toggle.onclick = function () { open(!(box && box.style.display !== "none")); };
  window.addEventListener("resize", function () {
    if (svg && box.style.display !== "none" && vb) {
      var r = box.getBoundingClientRect(), cx = vb.x + vb.w / 2, cy = vb.y + vb.h / 2;
      var h = vb.w * r.height / r.width;
      vb = { x: vb.x, y: cy - h / 2, w: vb.w, h: h }; vb.x = cx - vb.w / 2;
      apply();
    }
  });

  function start() {
    if (!place()) return;
    var wasOpen = load(KEY) === "1";
    toggle.textContent = "▸ Map";
    chips.textContent = "";
    Promise.all([getJson("code_map.json"), getJson("site_links.json")]).then(function (res) {
      data = res[0]; links = res[1];
      hereCrates = findHere();
      buildChips();
      var w = waiting; waiting = [];
      w.forEach(function (f) { f(); });
    }).catch(function (err) {
      var s = document.createElement("span");
      s.textContent = "Code map unavailable (" + err.message + ")";
      chips.appendChild(s);
    });
    if (wasOpen) open(true);
  }
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start);
  else start();
})();
