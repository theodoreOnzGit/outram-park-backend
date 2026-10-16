// The neutronics lessons' shared kit (gh:#906): the widget frame, the
// controls, number formatting, the lesson navigation and the loader for
// demos in an iframe. Each widget lives in its topic's file
// (kinematics.js, waves.js, ...) and registers itself with
// `Lesson.widget(id, make)`; a page lists the kit first, then the files of
// the widgets it shows.
//
// Every widget follows the demo layout of
// docs/claude-md/mobile-first-tutorials-and-demos.md in miniature: the
// picture fills the widget, on-screen + / − / Reset (and full screen) buttons
// sit on it, and every control is in a panel that folds ("« Hide",
// "Controls »") and starts folded on a narrow screen. Every computation in a
// lesson widget takes well under a millisecond a frame, so nothing needs a
// worker; the heavy demos (Monte Carlo) run in an iframe that loads when it
// scrolls into view. Every demo starts when scrolled to and the animated ones
// pause when scrolled away (maintainer, 2026-10-16).
(function () {
  "use strict";


  // CODATA 2018 (Tiesinga et al., 2021): h and e exact, m_n and c.
  var H = 6.62607015e-34, HBAR = H / (2 * Math.PI), EV = 1.602176634e-19;
  var MN = 1.67492749804e-27, C = 299792458;

  function css(name, fallback) {
    var v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
    return v || fallback;
  }
  function el(tag, attrs, text) {
    var e = document.createElement(tag);
    for (var k in attrs || {}) e.setAttribute(k, attrs[k]);
    if (text !== undefined) e.textContent = text;
    return e;
  }
  function fmtE(ev) {
    // Choose the unit after rounding, so 999.99 keV reads 1.00 MeV.
    if (ev >= 0.9995e6) return (ev / 1e6).toPrecision(3) + " MeV";
    if (ev >= 0.9995e3) return (ev / 1e3).toPrecision(3) + " keV";
    if (ev >= 0.9995) return ev.toPrecision(3) + " eV";
    if (ev >= 1e-3) return (ev * 1e3).toPrecision(3) + " meV";
    return ev.toExponential(2) + " eV";
  }
  function pct(f) {
    var p = 100 * f;
    return (p >= 0.1 ? p.toFixed(1) : p.toPrecision(2)) + " %";
  }
  function fmtLen(m) {
    var a = Math.abs(m);
    if (a >= 1e-6) return (m * 1e6).toPrecision(3) + " µm";
    if (a >= 1e-9) return (m * 1e9).toPrecision(3) + " nm";
    if (a >= 1e-11) return (m * 1e10).toPrecision(3) + " Å";
    if (a >= 1e-13) return (m * 1e12).toPrecision(3) + " pm";
    return (m * 1e15).toPrecision(3) + " fm";
  }
  function fmtSpeed(v) {
    if (v >= 1e6) return (v / 1e3).toExponential(2) + " km/s";
    if (v >= 1e3) return (v / 1e3).toPrecision(3) + " km/s";
    return v.toPrecision(3) + " m/s";
  }

  // ─── The widget frame ──────────────────────────────────────────────────
  // spec: { root, draw(ctx, w, h, zoom), tick(dt) -> keep animating?, panel(div) }
  function Frame(root, spec) {
    var self = this;
    this.zoom = 1;
    this.spec = spec;
    var canvas = root.querySelector("canvas");
    this.canvas = canvas;
    var bar = el("div", { "class": "w-bar" });
    var show = el("button", { type: "button", "class": "w-btn w-show" }, "Controls »");
    var zin = el("button", { type: "button", "class": "w-btn", "aria-label": "Zoom in" }, "+");
    var zout = el("button", { type: "button", "class": "w-btn", "aria-label": "Zoom out" }, "−");
    var reset = el("button", { type: "button", "class": "w-btn" }, "Reset");
    var full = el("button", { type: "button", "class": "w-btn", "aria-label": "Full screen" }, "⛶");
    var right = el("span", { "class": "w-right" });
    right.append(zin, zout, reset, full);
    bar.append(show, right);
    var panel = el("div", { "class": "w-panel" });
    var head = el("div", { "class": "w-head" });
    var hide = el("button", { type: "button", "class": "w-btn" }, "« Hide");
    head.append(el("b", {}, "Controls"), hide);
    var body = el("div", { "class": "w-body" });
    panel.append(head, body);
    this.status = el("div", { "class": "w-status", "aria-live": "polite" });
    root.append(bar, panel, this.status);
    function fold(f) { root.classList.toggle("folded", f); show.hidden = !f; }
    fold(window.innerWidth < 700);
    show.onclick = function () { fold(false); };
    hide.onclick = function () { fold(true); };
    zin.onclick = function () { self.zoom = Math.min(self.zoom * 1.4, 40); self.redraw(); };
    zout.onclick = function () { self.zoom = Math.max(self.zoom / 1.4, 0.25); self.redraw(); };
    reset.onclick = function () { self.zoom = 1; if (spec.reset) spec.reset(); self.redraw(); };
    full.onclick = function () {
      var on = !root.classList.contains("full");
      root.classList.toggle("full", on);
      document.body.classList.toggle("w-noscroll", on);
      full.textContent = on ? "✕" : "⛶";
      full.setAttribute("aria-label", on ? "Leave full screen" : "Full screen");
      self.redraw();
    };
    spec.panel(body, function () { self.redraw(); }, function () { self.play(); });
    this.running = false;
    this.last = 0;
    this.visible = false;
    new ResizeObserver(function () { self.redraw(); }).observe(canvas);
    this.redraw();
    // Start when scrolled into view, pause when scrolled out (maintainer,
    // 2026-10-16: "Demos should start as soon as I scroll past them"). A
    // paused widget costs no frames; spec.onSeen restarts it.
    whenSeen(root, function (seen) {
      self.visible = seen;
      if (seen) { if (spec.onSeen) spec.onSeen(); self.play(); }
    });
  }
  // Call back(true) when `node` is at least 30 % on screen and back(false)
  // when it leaves; without IntersectionObserver, once with true.
  function whenSeen(node, back) {
    if (!("IntersectionObserver" in window)) { back(true); return; }
    new IntersectionObserver(function (entries) {
      entries.forEach(function (e) { back(e.isIntersecting); });
    }, { threshold: 0.3 }).observe(node);
  }
  Frame.prototype.redraw = function () {
    var c = this.canvas, dpr = window.devicePixelRatio || 1;
    var w = c.clientWidth, h = c.clientHeight;
    if (!w || !h) return;
    if (c.width !== Math.round(w * dpr) || c.height !== Math.round(h * dpr)) {
      c.width = Math.round(w * dpr); c.height = Math.round(h * dpr);
    }
    var ctx = c.getContext("2d");
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    var s = this.spec.draw(ctx, w, h, this.zoom);
    this.status.textContent = s || "";
  };
  Frame.prototype.play = function () {
    var self = this;
    if (this.running) return;
    this.running = true;
    this.last = performance.now();
    function step(now) {
      var dt = Math.min((now - self.last) / 1000, 0.05);
      self.last = now;
      var more = self.spec.tick(dt);
      self.redraw();
      if (more && self.visible) requestAnimationFrame(step); else self.running = false;
    }
    requestAnimationFrame(step);
  };

  function buttons(parent, label, options, current, onPick) {
    var g = el("div", { "class": "w-group", role: "group", "aria-label": label });
    g.append(el("span", { "class": "w-label" }, label));
    var row = el("div", { "class": "w-row" });
    options.forEach(function (o) {
      var b = el("button", { type: "button", "class": "w-opt" }, o[1]);
      if (o[0] === current) b.classList.add("on");
      b.onclick = function () {
        row.querySelectorAll(".w-opt").forEach(function (x) { x.classList.remove("on"); });
        b.classList.add("on");
        onPick(o[0]);
      };
      row.append(b);
    });
    g.append(row);
    parent.append(g);
  }
  function slider(parent, label, min, max, stepv, value, onInput) {
    var g = el("label", { "class": "w-group" });
    var out = el("span", { "class": "w-val" });
    g.append(el("span", { "class": "w-label" }, label), out);
    var s = el("input", { type: "range", min: min, max: max, step: stepv });
    s.value = value;
    s.oninput = function () { out.textContent = onInput(parseFloat(s.value)); };
    out.textContent = onInput(parseFloat(value));
    g.append(s);
    parent.append(g);
    return s;
  }
  function toggle(parent, label, value, onChange) {
    var g = el("label", { "class": "w-check" });
    var c = el("input", { type: "checkbox" });
    c.checked = value;
    c.onchange = function () { onChange(c.checked); };
    g.append(c, document.createTextNode(" " + label));
    parent.append(g);
  }
  function arrow(ctx, x0, y0, x1, y1, col) {
    var a = Math.atan2(y1 - y0, x1 - x0), L = Math.hypot(x1 - x0, y1 - y0);
    if (L < 2) return;
    ctx.strokeStyle = col; ctx.fillStyle = col; ctx.lineWidth = 2;
    ctx.beginPath(); ctx.moveTo(x0, y0); ctx.lineTo(x1, y1); ctx.stroke();
    var hs = Math.min(10, L / 2);
    ctx.beginPath();
    ctx.moveTo(x1, y1);
    ctx.lineTo(x1 - hs * Math.cos(a - 0.4), y1 - hs * Math.sin(a - 0.4));
    ctx.lineTo(x1 - hs * Math.cos(a + 0.4), y1 - hs * Math.sin(a + 0.4));
    ctx.closePath(); ctx.fill();
  }

  // ─── The lesson sequence (gh:#906 checkpoint §10–§23) ────────────────────
  // One principal question per page (checkpoint §5). The navigation on every
  // page is built from this one list, so adding a page is one line here.
  var LESSONS = [
    ["meet-the-neutron.html", "Meet the neutron"],
    ["why-it-scatters.html", "Why does a neutron bounce?"],
    ["moving-nuclei.html", "The nucleus is moving too"],
    ["matter-waves.html", "Why not billiard balls?"],
    ["quantum-scattering.html", "A neutron wave at a nucleus"],
    ["storing-shapes.html", "How do we store a shape?"],
    ["scattering-angles.html", "How are scattering angles stored?"],
    ["why-energy-matters.html", "Why does the neutron's energy matter?"],
    ["nuclear-levels.html", "Why are nuclear levels so complicated?"],
    ["what-can-happen.html", "What can happen in a nucleus?"],
    ["fission.html", "Fission: fissile and fissionable"],
    ["resonance-shape.html", "What shapes a resonance peak?"],
    ["r-matrix.html", "Do neighbouring peaks interfere?"],
    ["reich-moore.html", "Can we skip the gamma paths?"],
    ["mlbw.html", "Can we drop the matrix?"],
    ["adler-adler.html", "Cheaper still: fit the curves?"],
    ["log-scales.html", "Why are cross sections drawn on log scales?"],
    ["unresolved.html", "The double whammy: unresolved resonances"],
    ["purr.html", "PURR: make up the resonances"],
    ["bound-atoms.html", "But the atoms are bound!"],
    ["lattice-scattering.html", "What does lattice scattering depend on?"],
    ["doppler.html", "Doppler broadening: lone atoms, warm"],
    ["phonons.html", "What is a phonon?"],
    ["many-phonons.html", "Can we have more than one phonon?"],
    ["structure.html", "How does structure shape the phonons?"],
    ["alpha-beta.html", "Maths 1: two numbers for a collision"],
    ["jiggle.html", "Maths 1½: let the nucleus jiggle"],
    ["free-gas.html", "Maths 2: the free gas, exactly"],
    ["one-spring.html", "Maths 3: one swing"],
    ["convolution.html", "Maths 3½: what is a convolution?"],
    ["many-modes.html", "Maths 4: many modes, many phonons"],
    ["big-kicks.html", "Maths 5: big kicks, use the bell"],
    ["bell-shape.html", "Maths 5a: what shapes the bell curve?"],
    ["zero-point.html", "Maths 5b: why can't atoms sit still?"],
    ["stiff-soft.html", "Maths 5c: stiff springs, soft springs"],
    ["snapshot.html", "Maths 5d: what does a hard kick see?"],
    ["why-bell.html", "Maths 5e: why is it a bell curve?"],
    ["width.html", "Maths 5f: what sets the width?"],
    ["one-spring-check.html", "Maths 5g: checking it, one spring then a crystal"],
    ["coherent.html", "Maths 5h: do the waves add up?"],
    ["spin.html", "Maths 5i: why does each hydrogen scatter differently?"]
  ];
  function lessonNav() {
    var here = location.pathname.split("/").pop() || LESSONS[0][0], i = -1;
    LESSONS.forEach(function (l, j) { if (l[0] === here) i = j; });
    if (i < 0) return;
    var top = document.querySelector(".lesson-progress");
    if (top) {
      top.textContent = "Neutronics · " + (i + 1) + " of " + LESSONS.length;
    }
    var bottom = document.querySelector(".lesson-nav");
    if (!bottom) return;
    var row = el("p", { "class": "actions lesson-steps" });
    if (i > 0) row.append(el("a", { "class": "btn ghost", href: LESSONS[i - 1][0] }, "← " + LESSONS[i - 1][1]));
    var next = bottom.getAttribute("data-next-soon");
    if (i + 1 < LESSONS.length) row.append(el("a", { "class": "btn", href: LESSONS[i + 1][0] }, LESSONS[i + 1][1] + " →"));
    else if (next) row.append(el("span", { "class": "btn disabled", "aria-disabled": "true" }, next + " →"));
    var contents = el("details", { "class": "lesson-contents" });
    contents.append(el("summary", {}, "Contents"));
    var ol = el("ol");
    LESSONS.forEach(function (l, j) {
      var li = el("li");
      if (j === i) li.append(el("b", {}, l[1]));
      else li.append(el("a", { href: l[0] }, l[1]));
      ol.append(li);
    });
    contents.append(ol);
    var gl = el("p"); gl.append(el("a", { href: "glossary.html" }, "Glossary of terms")); contents.append(gl);
    bottom.append(row, contents);
  }

  // Glossary back-links (gh:#906 no-jargon rule 3): on each lesson page after
  // the one that defines a term, the term's first mention in the prose links
  // to its glossary entry. [glossary id, defining page, words to match].
  var GLOSSARY = [
    ["compound-nucleus", "why-energy-matters.html", "compound nucleus"],
    ["doppler-broadening", "moving-nuclei.html", "Doppler broadening"],
    ["lethargy", "log-scales.html", "lethargy"],
    ["infinitely-dilute", "unresolved.html", "infinitely dilute"],
    ["coherent-elastic", "lattice-scattering.html", "coherent elastic"],
    ["scattering-law", "lattice-scattering.html", "scattering law"],
    ["detailed-balance", "lattice-scattering.html", "detailed balance"],
    ["debye-waller", "lattice-scattering.html", "Debye–Waller"],
    ["normal-mode", "phonons.html", "normal mode"],
    ["quasiparticle", "phonons.html", "quasiparticle"],
    ["polarisation", "phonons.html", "polarisation"],
    ["density-of-states", "many-phonons.html", "density of states"],
    ["recoil-line", "alpha-beta.html", "recoil line"],
    ["sum-rule", "free-gas.html", "sum rule"],
    ["upscatter", "one-spring.html", "up-scatter"],
    ["phonon-expansion", "one-spring.html", "phonon expansion"],
    ["convolution", "convolution.html", "convolution"],
    ["sct", "big-kicks.html", "short-collision-time approximation"],
    ["zero-point", "zero-point.html", "zero-point motion"],
    ["effective-temperature", "zero-point.html", "effective temperature"],
    ["impulse-approximation", "one-spring-check.html", "impulse approximation"]
  ];
  function glossaryLinks(here) {
    var order = LESSONS.map(function (l) { return l[0]; }), i = order.indexOf(here);
    if (i < 0) return;
    var main = document.querySelector("main.lesson-body");
    if (!main) return;
    GLOSSARY.forEach(function (g) {
      var j = order.indexOf(g[1]);
      if (j < 0 || j >= i) return;
      var re = new RegExp("\\b" + g[2].replace(/[.*+?^${}()|[\]\\]/g, "\\$&") + "s?\\b", "i");
      var walker = document.createTreeWalker(main, NodeFilter.SHOW_TEXT, {
        acceptNode: function (n) {
          for (var p = n.parentNode; p && p !== main; p = p.parentNode) {
            var t = p.nodeName;
            if (t === "A" || t === "H1" || t === "H2" || t === "H3" || t === "CODE" || t === "NAV" || t === "FOOTER" || (p.classList && (p.classList.contains("widget") || p.classList.contains("banner") || p.classList.contains("csl-entry")))) return NodeFilter.FILTER_REJECT;
          }
          return re.test(n.nodeValue) ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_SKIP;
        }
      });
      var node = walker.nextNode();
      if (!node) return;
      var m = re.exec(node.nodeValue), after = node.splitText(m.index);
      after.splitText(m[0].length);
      var a = el("a", { href: "glossary.html#" + g[0], "class": "gl-term", title: "Glossary" }, m[0]);
      after.parentNode.replaceChild(a, after);
    });
  }

  var WIDGETS = {};
  function widget(id, make) { WIDGETS[id] = make; }

  function start() {
    lessonNav();
    glossaryLinks(location.pathname.split("/").pop());
    for (var id in WIDGETS) {
      var node = document.getElementById(id);
      if (node) WIDGETS[id](node);
    }
    // A demo in an iframe loads when it scrolls into view (maintainer,
    // 2026-10-16), not with the page: it downloads and processes nuclear
    // data, which a reader who never reaches it should not pay for. The
    // button stays as a fallback. Once loaded it keeps running.
    document.querySelectorAll(".iframe-slot").forEach(function (slot) {
      var load = function () {
        if (slot.classList.contains("live")) return;
        var f = el("iframe", { src: slot.getAttribute("data-src"), title: slot.getAttribute("data-title") || "Demo", allow: "fullscreen", loading: "eager" });
        slot.innerHTML = "";
        slot.append(f);
        slot.classList.add("live");
      };
      var b = slot.querySelector(".start-demo");
      if (b) b.onclick = load;
      whenSeen(slot, function (seen) { if (seen) load(); });
    });
  }
  // The widget files are deferred scripts after this one, and they run
  // before DOMContentLoaded but after this file; while they run,
  // readyState is already "interactive". So wait for DOMContentLoaded (or
  // "load", whichever comes first) unless the page has fully loaded.
  var started = false;
  function once() { if (!started) { started = true; start(); } }
  if (document.readyState === "complete") setTimeout(once, 0);
  else {
    document.addEventListener("DOMContentLoaded", once);
    window.addEventListener("load", once);
  }

  window.Lesson = {
    H: H, HBAR: HBAR, EV: EV, MN: MN, C: C,
    css: css, el: el, fmtE: fmtE, pct: pct, fmtLen: fmtLen, fmtSpeed: fmtSpeed,
    Frame: Frame, whenSeen: whenSeen, buttons: buttons, slider: slider, toggle: toggle, arrow: arrow,
    widget: widget
  };
})();
