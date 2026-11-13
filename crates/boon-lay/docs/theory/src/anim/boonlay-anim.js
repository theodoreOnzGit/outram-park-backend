// Small in-page illustrations for the boon-lay lessons (gh:#531).
//
// Each <div class="bl-anim" data-anim="NAME"> becomes one illustration. They
// are JAVASCRIPT RE-IMPLEMENTATIONS OF THE FORMULAS ON THE PAGE, labelled as
// illustrations; the checked code is the Rust each page walks through. The
// work per frame is tiny (a few hundred points), animations run only while
// visible, and nothing blocks the page (no-lagging rule,
// docs/claude-md/mobile-first-tutorials-and-demos.md).
// Inline $...$ math: mdBook's MathJax 2 recognises only \( \) inline by
// default. Same approach as the Monte Carlo tutorial's widgets.js: configure
// it (or leave the configuration for it to find) and typeset again.
(function () {
  var cfg = { tex2jax: { inlineMath: [["$", "$"], ["\\(", "\\)"]], processEscapes: true } };
  if (window.MathJax && window.MathJax.Hub) {
    window.MathJax.Hub.Config(cfg);
    window.MathJax.Hub.Queue(["Typeset", window.MathJax.Hub]);
  } else {
    window.MathJax = cfg;
  }
})();

(function () {
  "use strict";

  // ---------- shared helpers ----------
  function css(name, fallback) {
    var v = getComputedStyle(document.documentElement).getPropertyValue(name);
    return v && v.trim() ? v.trim() : fallback;
  }
  function theme() {
    return {
      fg: css("--fg", "#222"),
      bg: css("--bg", "#fff"),
      link: css("--links", "#2e86c1"),
      grid: "rgba(127,127,127,0.35)",
      red: "#c0392b",
      green: "#27ae60",
      orange: "#e67e22",
      blue: "#2e86c1",
      grey: "rgba(127,127,127,0.6)",
    };
  }
  function el(tag, attrs, parent) {
    var e = document.createElement(tag);
    if (attrs) for (var k in attrs) {
      if (k === "text") e.textContent = attrs[k]; else e.setAttribute(k, attrs[k]);
    }
    if (parent) parent.appendChild(e);
    return e;
  }
  function makeCanvas(root, aspect) {
    var c = el("canvas", null, root);
    var ctx = c.getContext("2d");
    var state = { w: 0, h: 0 };
    function fit() {
      var w = Math.max(240, Math.min(root.clientWidth - 2, 760));
      var h = Math.round(w / aspect);
      var dpr = window.devicePixelRatio || 1;
      c.width = Math.round(w * dpr);
      c.height = Math.round(h * dpr);
      c.style.height = h + "px";
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      state.w = w; state.h = h;
    }
    fit();
    return { canvas: c, ctx: ctx, size: state, fit: fit };
  }
  function button(parent, text, onClick) {
    var b = el("button", { type: "button", text: text }, parent);
    b.addEventListener("click", onClick);
    return b;
  }
  function slider(parent, label, min, max, step, value, onInput) {
    var l = el("label", null, parent);
    var span = el("span", { text: label }, l);
    var s = el("input", { type: "range", min: min, max: max, step: step, value: value }, l);
    s.addEventListener("input", function () { onInput(parseFloat(s.value), span); });
    onInput(parseFloat(s.value), span);
    return s;
  }
  function header(root, text) {
    el("p", { class: "bl-label", text: "Illustration (JavaScript, not the crate's code): " + text }, root);
  }
  // Run `frame(dt)` with requestAnimationFrame only while the element is on screen.
  function animate(root, frame) {
    var visible = false, last = 0, running = false;
    function tick(t) {
      if (!visible) { running = false; return; }
      var dt = last ? Math.min(0.1, (t - last) / 1000) : 0;
      last = t;
      frame(dt);
      requestAnimationFrame(tick);
    }
    function start() { if (!running) { running = true; last = 0; requestAnimationFrame(tick); } }
    if ("IntersectionObserver" in window) {
      new IntersectionObserver(function (es) {
        visible = es[0].isIntersecting; if (visible) start();
      }).observe(root);
    } else { visible = true; start(); }
  }
  // Mulberry32: a tiny seeded PRNG so a "Restart" replays something comparable.
  function rng(seed) {
    var a = seed >>> 0;
    return function () {
      a |= 0; a = (a + 0x6D2B79F5) | 0;
      var t = Math.imul(a ^ (a >>> 15), 1 | a);
      t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }
  function axes(ctx, x0, y0, w, h, th, xl, yl) {
    ctx.strokeStyle = th.grid; ctx.lineWidth = 1;
    ctx.strokeRect(x0, y0, w, h);
    ctx.fillStyle = th.fg; ctx.font = "12px sans-serif";
    ctx.fillText(xl, x0 + w / 2 - ctx.measureText(xl).width / 2, y0 + h + 15);
    ctx.save(); ctx.translate(x0 - 6, y0 + h / 2); ctx.rotate(-Math.PI / 2);
    ctx.fillText(yl, -ctx.measureText(yl).width / 2, 0); ctx.restore();
  }
  function fmt(x) {
    if (x === 0) return "0";
    var a = Math.abs(x);
    return (a >= 1e-3 && a < 1e4) ? x.toPrecision(3) : x.toExponential(2);
  }

  // ---------- 1. TRISO layers (rung 1) ----------
  var GEOM = {
    "CRP-6": [212.5, 312.5, 352.5, 387.5, 427.5],
    "HTR-10": [250, 340, 380, 415, 455],
  };
  var LAYERS = [
    ["kernel (UO2)", "#c0392b"], ["buffer", "#d9c38c"], ["IPyC", "#5d6d7e"],
    ["SiC", "#2e86c1"], ["OPyC", "#2c3e50"],
  ];
  function layers(root) {
    header(root, "the five layers drawn from the published radii; tap a layer to name it.");
    var cv = makeCanvas(root, 1.25);
    var ctl = el("div", { class: "bl-controls" }, root);
    var out = el("div", { class: "bl-readout" }, root);
    var which = "CRP-6", zoom = 1, picked = -1;
    var bC = button(ctl, "CRP-6", function () { which = "CRP-6"; bC.classList.add("bl-on"); bH.classList.remove("bl-on"); draw(); });
    var bH = button(ctl, "HTR-10", function () { which = "HTR-10"; bH.classList.add("bl-on"); bC.classList.remove("bl-on"); draw(); });
    bC.classList.add("bl-on");
    button(ctl, "+", function () { zoom = Math.min(8, zoom * 1.5); draw(); });
    button(ctl, "−", function () { zoom = Math.max(0.5, zoom / 1.5); draw(); });
    button(ctl, "Reset", function () { zoom = 1; picked = -1; draw(); });
    function scale() { var s = cv.size; return zoom * 0.45 * Math.min(s.w, s.h) / 455; }
    function draw() {
      var th = theme(), ctx = cv.ctx, s = cv.size, r = GEOM[which], k = scale();
      ctx.clearRect(0, 0, s.w, s.h);
      var cx = s.w / 2, cy = s.h / 2;
      for (var i = 4; i >= 0; i--) {
        ctx.beginPath(); ctx.arc(cx, cy, r[i] * k, 0, 2 * Math.PI);
        ctx.fillStyle = LAYERS[i][1]; ctx.fill();
        if (i === picked) { ctx.lineWidth = 3; ctx.strokeStyle = th.orange; ctx.stroke(); }
      }
      // 100 um scale bar
      ctx.strokeStyle = th.fg; ctx.lineWidth = 3;
      ctx.beginPath(); ctx.moveTo(12, s.h - 14); ctx.lineTo(12 + 100 * k, s.h - 14); ctx.stroke();
      ctx.fillStyle = th.fg; ctx.font = "12px sans-serif"; ctx.fillText("100 µm", 12, s.h - 20);
      var lines = [which + " radii (µm): " + r.join(", ")];
      if (picked >= 0) {
        var inner = picked === 0 ? 0 : r[picked - 1];
        lines.push("tapped: " + LAYERS[picked][0] + ", " + inner + "–" + r[picked] + " µm, thickness " + (r[picked] - inner).toFixed(1) + " µm");
      }
      out.textContent = lines.join("\n");
    }
    cv.canvas.addEventListener("click", function (e) {
      var b = cv.canvas.getBoundingClientRect();
      var dx = e.clientX - b.left - cv.size.w / 2, dy = e.clientY - b.top - cv.size.h / 2;
      var rr = Math.sqrt(dx * dx + dy * dy) / scale(), g = GEOM[which];
      picked = -1;
      for (var i = 0; i < 5; i++) if (rr < g[i]) { picked = i; break; }
      draw();
    });
    window.addEventListener("resize", function () { cv.fit(); draw(); });
    draw();
  }

  // ---------- 2. decay (rung 2) ----------
  function decay(root) {
    header(root, "400 atoms, each with lifetime t = T½·|ln ξ|/ln 2; the curve is 2^(−t/T½).");
    var cv = makeCanvas(root, 1.6);
    var ctl = el("div", { class: "bl-controls" }, root);
    var out = el("div", { class: "bl-readout" }, root);
    var N = 400, half = 3, t = 0, paused = false, life = [], seed = 1, pts = [];
    function reset() {
      var r = rng(seed++); life = []; pts = []; t = 0;
      for (var i = 0; i < N; i++) life.push(half * Math.abs(Math.log(1 - r())) / Math.LN2);
    }
    slider(ctl, "half-life T½ (s of animation)", 1, 8, 0.5, half, function (v, sp) { half = v; sp.textContent = "half-life T½ = " + v + " s (animation time)"; reset(); });
    var bp = button(ctl, "Pause", function () { paused = !paused; bp.textContent = paused ? "Play" : "Pause"; });
    button(ctl, "Restart", reset);
    function draw() {
      var th = theme(), ctx = cv.ctx, s = cv.size;
      ctx.clearRect(0, 0, s.w, s.h);
      var gw = Math.floor(s.w * 0.42), cols = 20, cell = gw / cols, alive = 0;
      for (var i = 0; i < N; i++) {
        var on = life[i] > t; if (on) alive++;
        ctx.fillStyle = on ? th.green : th.grey;
        ctx.beginPath();
        ctx.arc(8 + (i % cols + 0.5) * cell, 8 + (Math.floor(i / cols) + 0.5) * cell, cell * (on ? 0.35 : 0.18), 0, 2 * Math.PI);
        ctx.fill();
      }
      var x0 = gw + 40, y0 = 10, w = s.w - x0 - 10, h = s.h - 40, tmax = 5 * half;
      axes(ctx, x0, y0, w, h, th, "t (5 half-lives)", "surviving");
      ctx.strokeStyle = th.blue; ctx.lineWidth = 2; ctx.beginPath();
      for (var k = 0; k <= 100; k++) {
        var tt = tmax * k / 100, y = Math.pow(2, -tt / half);
        var X = x0 + w * tt / tmax, Y = y0 + h * (1 - y);
        if (k === 0) ctx.moveTo(X, Y); else ctx.lineTo(X, Y);
      }
      ctx.stroke();
      if (t <= tmax) pts.push([t, alive / N]);
      ctx.fillStyle = th.red;
      for (var j = 0; j < pts.length; j += 3) {
        ctx.fillRect(x0 + w * pts[j][0] / tmax - 1.5, y0 + h * (1 - pts[j][1]) - 1.5, 3, 3);
      }
      var exact = Math.pow(2, -t / half), sig = Math.sqrt(exact * (1 - exact) / N);
      out.textContent = "t = " + t.toFixed(1) + " s   counted " + (alive / N).toFixed(3) +
        "   2^(−t/T½) = " + exact.toFixed(3) + "   one binomial σ = " + sig.toFixed(3);
    }
    reset();
    animate(root, function (dt) { if (!paused && t < 5 * half) t += dt; draw(); });
    window.addEventListener("resize", function () { cv.fit(); });
  }

  // ---------- 3. walk: Gaussian step vs Walk-on-Spheres (rung 3) ----------
  function walk(root) {
    header(root, "a 2-D slice of the CRP-6 particle with ILLUSTRATIVE diffusion ratios (buffer 1, PyC 0.1, SiC 0.001, kernel 0.001). 2-D hops, not the crate's 3-D sampler.");
    var cv = makeCanvas(root, 1.25);
    var ctl = el("div", { class: "bl-controls" }, root);
    var out = el("div", { class: "bl-readout" }, root);
    var R = GEOM["CRP-6"], Drel = [0.001, 1, 0.1, 0.001, 0.1];
    var mode = "gauss", dtRel = 1, p, path, r = rng(7), crossings = 0, hops = 0, circle = null, acc = 0;
    function layerOf(rad) { for (var i = 0; i < 5; i++) if (rad < R[i]) return i; return 5; }
    function reset() { p = [0, 262.5]; path = [p.slice()]; crossings = 0; hops = 0; circle = null; }
    var bg = button(ctl, "Gaussian step", function () { mode = "gauss"; bg.classList.add("bl-on"); bw.classList.remove("bl-on"); reset(); });
    var bw = button(ctl, "Walk-on-Spheres", function () { mode = "wos"; bw.classList.add("bl-on"); bg.classList.remove("bl-on"); reset(); });
    bg.classList.add("bl-on");
    button(ctl, "Restart", reset);
    slider(ctl, "Gaussian σ in the buffer", 10, 160, 5, 80, function (v, sp) { dtRel = v; sp.textContent = "Gaussian σ in the buffer = " + v + " µm (buffer is 100 µm)"; });
    function gauss() {
      var u1 = Math.max(1e-12, r()), u2 = r();
      return Math.sqrt(-2 * Math.log(u1)) * Math.cos(2 * Math.PI * u2);
    }
    function step() {
      var rad = Math.hypot(p[0], p[1]), L = layerOf(rad);
      if (L === 5) { reset(); return; }
      if (mode === "gauss") {
        var sig = dtRel * Math.sqrt(Drel[L] / Drel[1]);
        var q = [p[0] + sig * gauss(), p[1] + sig * gauss()];
        var L2 = layerOf(Math.hypot(q[0], q[1]));
        if (Math.abs(L2 - L) > 1) crossings++; // jumped clean over a whole layer
        p = q; circle = null;
      } else {
        var inner = L === 0 ? -1 : R[L - 1], outer = R[L];
        var din = inner < 0 ? Infinity : rad - inner, dout = outer - rad, d = Math.min(din, dout);
        if (d < 1.0) {
          // at an interface: transmit with p = D2/(D1 + D2) (K = 1)
          var toOuter = dout <= din, L2 = toOuter ? L + 1 : L - 1;
          if (L2 === 5) { p = [p[0] * 1.02, p[1] * 1.02]; }
          else {
            var pt = Drel[L2] / (Drel[L] + Drel[L2]);
            var f = (r() < pt) === toOuter ? 1 + 2 / rad : 1 - 2 / rad;
            p = [p[0] * f, p[1] * f];
          }
          circle = null;
        } else {
          var a = 2 * Math.PI * r();
          circle = [p[0], p[1], d];
          p = [p[0] + d * Math.cos(a), p[1] + d * Math.sin(a)];
        }
      }
      hops++;
      path.push(p.slice()); if (path.length > 60) path.shift();
    }
    function draw() {
      var th = theme(), ctx = cv.ctx, s = cv.size, k = 0.45 * Math.min(s.w, s.h) / 470;
      var cx = s.w / 2, cy = s.h / 2;
      ctx.clearRect(0, 0, s.w, s.h);
      for (var i = 4; i >= 0; i--) {
        ctx.beginPath(); ctx.arc(cx, cy, R[i] * k, 0, 2 * Math.PI);
        ctx.fillStyle = LAYERS[i][1]; ctx.globalAlpha = 0.45; ctx.fill(); ctx.globalAlpha = 1;
      }
      if (circle) {
        ctx.strokeStyle = th.orange; ctx.lineWidth = 1.5; ctx.beginPath();
        ctx.arc(cx + circle[0] * k, cy - circle[1] * k, circle[2] * k, 0, 2 * Math.PI); ctx.stroke();
      }
      ctx.strokeStyle = th.fg; ctx.lineWidth = 1; ctx.beginPath();
      for (var j = 0; j < path.length; j++) {
        var X = cx + path[j][0] * k, Y = cy - path[j][1] * k;
        if (j === 0) ctx.moveTo(X, Y); else ctx.lineTo(X, Y);
      }
      ctx.stroke();
      ctx.fillStyle = th.red; ctx.beginPath(); ctx.arc(cx + p[0] * k, cy - p[1] * k, 4, 0, 2 * Math.PI); ctx.fill();
      var L = layerOf(Math.hypot(p[0], p[1]));
      out.textContent = (mode === "gauss" ? "Gaussian step" : "Walk-on-Spheres") + ": " + hops +
        " steps, now in " + (L < 5 ? LAYERS[L][0] : "outside") +
        (mode === "gauss" ? ", whole layers jumped over: " + crossings : ", hop circles never cross an interface");
    }
    reset();
    animate(root, function (dt) { acc += dt; while (acc > 0.12) { step(); acc -= 0.12; } draw(); });
    window.addEventListener("resize", function () { cv.fit(); });
  }

  // ---------- 4. interface transmission (rung 4) ----------
  function interfaceAnim(root) {
    header(root, "p_transmit = K·D₂/(D₁ + K·D₂).");
    var cv = makeCanvas(root, 2.0);
    var ctl = el("div", { class: "bl-controls" }, root);
    var out = el("div", { class: "bl-readout" }, root);
    var lr = -2.6, K = 1;
    function p(l) { var d2 = Math.pow(10, l); return K * d2 / (1 + K * d2); }
    function draw() {
      var th = theme(), ctx = cv.ctx, s = cv.size;
      ctx.clearRect(0, 0, s.w, s.h);
      var x0 = 40, y0 = 10, w = s.w - 50, h = s.h - 40;
      axes(ctx, x0, y0, w, h, th, "log₁₀(D₂/D₁)  from −6 to +2", "p");
      ctx.strokeStyle = th.blue; ctx.lineWidth = 2; ctx.beginPath();
      for (var i = 0; i <= 200; i++) {
        var l = -6 + 8 * i / 200, X = x0 + w * i / 200, Y = y0 + h * (1 - p(l));
        if (i === 0) ctx.moveTo(X, Y); else ctx.lineTo(X, Y);
      }
      ctx.stroke();
      var X = x0 + w * (lr + 6) / 8, Y = y0 + h * (1 - p(lr));
      ctx.fillStyle = th.red; ctx.beginPath(); ctx.arc(X, Y, 5, 0, 2 * Math.PI); ctx.fill();
      var pp = p(lr);
      out.textContent = "D₂/D₁ = " + fmt(Math.pow(10, lr)) + ", K = " + K + "   p = " + fmt(pp) +
        "   about " + fmt(1 / pp) + " arrivals per crossing";
    }
    slider(ctl, "log₁₀(D₂/D₁)", -6, 2, 0.1, lr, function (v, sp) { lr = v; sp.textContent = "log₁₀(D₂/D₁) = " + v.toFixed(1); draw(); });
    slider(ctl, "partition ratio K", 0.1, 10, 0.1, 1, function (v, sp) { K = v; sp.textContent = "partition ratio K = " + v.toFixed(1); draw(); });
    window.addEventListener("resize", function () { cv.fit(); draw(); });
    draw();
  }

  // ---------- 5. Booth sphere (rung 4) ----------
  function booth(root) {
    header(root, "left: F(τ) = 1 − (6/π²)Σ e^(−n²π²τ)/n² (200 terms); right: ⟨R/B⟩ = (3/μ)(coth μ − 1/μ).");
    var cv = makeCanvas(root, 2.0);
    var ctl = el("div", { class: "bl-controls" }, root);
    var out = el("div", { class: "bl-readout" }, root);
    var lt = -1.44, lm = 1;
    function F(tau) { var s = 0; for (var n = 1; n <= 200; n++) s += Math.exp(-n * n * Math.PI * Math.PI * tau) / (n * n); return Math.max(0, 1 - 6 / (Math.PI * Math.PI) * s); }
    function RB(mu) { if (mu < 1e-3) return 1; return 3 / mu * (1 / Math.tanh(mu) - 1 / mu); }
    function panel(ctx, th, x0, y0, w, h, f, lo, hi, mark, xl, yl) {
      axes(ctx, x0, y0, w, h, th, xl, yl);
      ctx.strokeStyle = th.blue; ctx.lineWidth = 2; ctx.beginPath();
      for (var i = 0; i <= 120; i++) {
        var l = lo + (hi - lo) * i / 120, v = f(Math.pow(10, l));
        var X = x0 + w * i / 120, Y = y0 + h * (1 - v);
        if (i === 0) ctx.moveTo(X, Y); else ctx.lineTo(X, Y);
      }
      ctx.stroke();
      var v2 = f(Math.pow(10, mark));
      ctx.fillStyle = th.red; ctx.beginPath(); ctx.arc(x0 + w * (mark - lo) / (hi - lo), y0 + h * (1 - v2), 5, 0, 2 * Math.PI); ctx.fill();
    }
    function draw() {
      var th = theme(), ctx = cv.ctx, s = cv.size;
      ctx.clearRect(0, 0, s.w, s.h);
      var w = (s.w - 90) / 2, h = s.h - 40;
      panel(ctx, th, 40, 10, w, h, F, -4, 0.5, lt, "log₁₀ τ", "F");
      panel(ctx, th, 80 + w, 10, w, h, RB, -1, 3, lm, "log₁₀ μ", "⟨R/B⟩");
      out.textContent = "τ = " + fmt(Math.pow(10, lt)) + ": F = " + fmt(F(Math.pow(10, lt))) +
        "     μ = " + fmt(Math.pow(10, lm)) + ": ⟨R/B⟩ = " + fmt(RB(Math.pow(10, lm))) + " (3/μ = " + fmt(3 / Math.pow(10, lm)) + ")";
    }
    slider(ctl, "log₁₀ τ", -4, 0.5, 0.02, lt, function (v, sp) { lt = v; sp.textContent = "log₁₀ τ = " + v.toFixed(2) + " (CRP-6 1a: −1.44)"; draw(); });
    slider(ctl, "log₁₀ μ", -1, 3, 0.05, lm, function (v, sp) { lm = v; sp.textContent = "log₁₀ μ = " + v.toFixed(2); draw(); });
    window.addEventListener("resize", function () { cv.fit(); draw(); });
    draw();
  }

  // ---------- 6. Weibull (rung 5) ----------
  function weibull(root) {
    header(root, "φ₁ = 1 − exp[−ln2·(σₜ/σₒ)^m]; 400 particles with strengths drawn from that law.");
    var cv = makeCanvas(root, 2.0);
    var ctl = el("div", { class: "bl-controls" }, root);
    var out = el("div", { class: "bl-readout" }, root);
    var sr = 0.8, m = 8, u = [], r = rng(11);
    for (var i = 0; i < 400; i++) u.push(r());
    function phi(x) { return 1 - Math.exp(-Math.LN2 * Math.pow(x, m)); }
    function draw() {
      var th = theme(), ctx = cv.ctx, s = cv.size;
      ctx.clearRect(0, 0, s.w, s.h);
      var gw = Math.floor(s.w * 0.4), cols = 20, cell = gw / cols, failed = 0;
      for (var i = 0; i < 400; i++) {
        var strength = Math.pow(-Math.log(1 - u[i]) / Math.LN2, 1 / m); // median 1
        var f = strength < sr; if (f) failed++;
        ctx.fillStyle = f ? th.red : th.green;
        ctx.beginPath(); ctx.arc(8 + (i % cols + 0.5) * cell, 8 + (Math.floor(i / cols) + 0.5) * cell, cell * 0.33, 0, 2 * Math.PI); ctx.fill();
      }
      var x0 = gw + 40, y0 = 10, w = s.w - x0 - 10, h = s.h - 40;
      axes(ctx, x0, y0, w, h, th, "σₜ/σₒ from 0 to 2", "φ₁");
      ctx.strokeStyle = th.blue; ctx.lineWidth = 2; ctx.beginPath();
      for (var k = 0; k <= 120; k++) {
        var x = 2 * k / 120, X = x0 + w * k / 120, Y = y0 + h * (1 - phi(x));
        if (k === 0) ctx.moveTo(X, Y); else ctx.lineTo(X, Y);
      }
      ctx.stroke();
      ctx.strokeStyle = th.grid; ctx.beginPath(); ctx.moveTo(x0 + w / 2, y0); ctx.lineTo(x0 + w / 2, y0 + h); ctx.stroke();
      ctx.fillStyle = th.red; ctx.beginPath(); ctx.arc(x0 + w * sr / 2, y0 + h * (1 - phi(sr)), 5, 0, 2 * Math.PI); ctx.fill();
      out.textContent = "σₜ/σₒ = " + sr.toFixed(2) + ", m = " + m + ":  φ₁ = " + fmt(phi(sr)) +
        "   counted " + failed + "/400   (at σₜ = σₒ exactly half fail: the ln 2)";
    }
    slider(ctl, "stress over median strength", 0.2, 2, 0.01, sr, function (v, sp) { sr = v; sp.textContent = "σₜ/σₒ = " + v.toFixed(2); draw(); });
    slider(ctl, "Weibull modulus m", 2, 12, 0.1, m, function (v, sp) { m = v; sp.textContent = "Weibull modulus m = " + v.toFixed(1); draw(); });
    window.addEventListener("resize", function () { cv.fit(); draw(); });
    draw();
  }

  // ---------- 7. decomposition: read phi2 off zeta vs accumulate (rung 5) ----------
  function decomp(root) {
    header(root, "Eqs (11)–(13), dₒ = 35 µm, sphere calibration α = 10⁻⁴, β = 4, over a heat-up and cool-down. Blue: φ₂ read off ζ (the report's rule). Orange: the WRONG rule, summing positive increments of an isothermal φ₂ at each step's temperature.");
    var cv = makeCanvas(root, 2.0);
    var ctl = el("div", { class: "bl-controls" }, root);
    var out = el("div", { class: "bl-readout" }, root);
    var peak = 2200;
    function T(th_) { // hours -> deg C: 1600 -> peak by 50 h, hold to 80 h, back to 1600 by 150 h
      if (th_ < 50) return 1600 + (peak - 1600) * th_ / 50;
      if (th_ < 80) return peak;
      if (th_ < 150) return peak - (peak - 1600) * (th_ - 80) / 70;
      return 1600;
    }
    function k(tc) { return 375 / 35e-6 * Math.exp(-556000 / (8.3143 * (tc + 273.15))); }
    function phi2(z) { return 1 - Math.exp(-1e-4 * Math.pow(z, 4)); }
    function draw() {
      var th = theme(), ctx = cv.ctx, s = cv.size;
      ctx.clearRect(0, 0, s.w, s.h);
      var x0 = 40, y0 = 10, w = s.w - 50, h = s.h - 40, N = 400, tmax = 200, dt = tmax / N * 3600;
      axes(ctx, x0, y0, w, h, th, "t (0–200 h); grey: T from 1600 °C to the peak", "φ₂");
      var z = 0, wrong = 0, elapsed = 0, A = [], B = [], C = [];
      for (var i = 0; i < N; i++) {
        var tm = T((i + 0.5) * tmax / N), kk = k(tm);
        var a = phi2(kk * elapsed), b = phi2(kk * (elapsed + dt));
        wrong = Math.min(1, wrong + Math.max(0, b - a));
        z += kk * dt; elapsed += dt;
        A.push(phi2(z)); B.push(wrong); C.push((tm - 1600) / Math.max(1, peak - 1600));
      }
      function line(arr, col) {
        ctx.strokeStyle = col; ctx.lineWidth = 2; ctx.beginPath();
        for (var j = 0; j < N; j++) { var X = x0 + w * j / N, Y = y0 + h * (1 - arr[j]); if (j === 0) ctx.moveTo(X, Y); else ctx.lineTo(X, Y); }
        ctx.stroke();
      }
      line(C, th.grid); line(B, th.orange); line(A, th.blue);
      out.textContent = "peak " + peak + " °C:  φ₂ read off ζ = " + fmt(A[N - 1]) +
        "   summed increments = " + fmt(B[N - 1]) + "   (ζ = " + fmt(z) + ")";
    }
    slider(ctl, "peak temperature", 1900, 2400, 10, peak, function (v, sp) { peak = v; sp.textContent = "peak temperature = " + v + " °C"; draw(); });
    window.addEventListener("resize", function () { cv.fit(); draw(); });
    draw();
  }

  // ---------- 8. graphite oxidation in air (rung 6) ----------
  function oxidation(root) {
    header(root, "Contescu 2011 (ORNL/TM-2022/1839 Table 3): k = e^13·exp(−191 kJ/mol / RT) in air; 5000 kg of graphite; carbon gasified = min(kinetic, O₂ supply).");
    var cv = makeCanvas(root, 2.0);
    var ctl = el("div", { class: "bl-controls" }, root);
    var out = el("div", { class: "bl-readout" }, root);
    var tc = 650, ls = 0;
    function kin(t) { return Math.exp(13) * Math.exp(-191000 / (8.314 * (t + 273.15))) * 5000 / 12.011e-3; }
    function draw() {
      var th = theme(), ctx = cv.ctx, s = cv.size;
      ctx.clearRect(0, 0, s.w, s.h);
      var x0 = 50, y0 = 10, w = s.w - 60, h = s.h - 40, lo = -4, hi = 6;
      axes(ctx, x0, y0, w, h, th, "T from 400 to 1200 °C", "log₁₀ mol C/s");
      var a0 = x0 + w * (597 - 400) / 800, a1 = x0 + w * (694 - 400) / 800;
      ctx.fillStyle = "rgba(39,174,96,0.15)"; ctx.fillRect(a0, y0, a1 - a0, h);
      ctx.strokeStyle = th.blue; ctx.lineWidth = 2; ctx.beginPath();
      for (var i = 0; i <= 160; i++) {
        var t = 400 + 800 * i / 160, v = Math.log10(kin(t));
        var X = x0 + w * i / 160, Y = y0 + h * (1 - (v - lo) / (hi - lo));
        if (i === 0) ctx.moveTo(X, Y); else ctx.lineTo(X, Y);
      }
      ctx.stroke();
      var Ys = y0 + h * (1 - (ls - lo) / (hi - lo));
      ctx.strokeStyle = th.orange; ctx.beginPath(); ctx.moveTo(x0, Ys); ctx.lineTo(x0 + w, Ys); ctx.stroke();
      var X = x0 + w * (tc - 400) / 800, v = Math.log10(kin(tc));
      ctx.fillStyle = th.red; ctx.beginPath(); ctx.arc(X, y0 + h * (1 - (Math.min(v, ls) - lo) / (hi - lo)), 5, 0, 2 * Math.PI); ctx.fill();
      var kinetic = kin(tc), supply = Math.pow(10, ls);
      out.textContent = "T = " + tc + " °C: kinetic " + fmt(kinetic) + " mol/s, O₂ supply " + fmt(supply) +
        " mol/s → " + (kinetic <= supply ? "kinetics binds" : "supply binds") +
        (tc >= 597 && tc <= 694 ? " (inside the measured range, shaded)" : " (Arrhenius extrapolated)");
    }
    slider(ctl, "temperature", 400, 1200, 5, tc, function (v, sp) { tc = v; sp.textContent = "temperature = " + v + " °C"; draw(); });
    slider(ctl, "log₁₀ O₂ supply", -3, 3, 0.1, ls, function (v, sp) { ls = v; sp.textContent = "O₂ supply = 10^" + v.toFixed(1) + " mol/s"; draw(); });
    window.addEventListener("resize", function () { cv.fit(); draw(); });
    draw();
  }

  // ---------- 9. coolant pools (rung 7) ----------
  function pools(root) {
    header(root, "dC/dt = S − (λ + k_plate + k_clean)C, dP/dt = k_plate·C − λP, dH/dt = k_clean·C − λH, from empty with S = 1 (rates per hour).");
    var cv = makeCanvas(root, 2.0);
    var ctl = el("div", { class: "bl-controls" }, root);
    var out = el("div", { class: "bl-readout" }, root);
    var lam = 0.05, kp = 0.3, kc = 0.1;
    function draw() {
      var th = theme(), ctx = cv.ctx, s = cv.size;
      ctx.clearRect(0, 0, s.w, s.h);
      var x0 = 40, y0 = 10, w = s.w - 50, h = s.h - 40, tmax = 100, N = 500, dt = tmax / N;
      var C = 0, P = 0, H = 0, sc = [], sp = [], sh = [], beta = lam + kp + kc;
      for (var i = 0; i < N; i++) {
        // exact one-step update for C, then trapezoid source for P and H
        var Cn = 1 / beta + (C - 1 / beta) * Math.exp(-beta * dt);
        var Cm = 0.5 * (C + Cn), e = Math.exp(-lam * dt);
        P = P * e + kp * Cm * dt; H = H * e + kc * Cm * dt; C = Cn;
        sc.push(C); sp.push(P); sh.push(H);
      }
      var top = Math.max(sc[N - 1], sp[N - 1], sh[N - 1], 1e-9) * 1.15;
      axes(ctx, x0, y0, w, h, th, "t (0–100 h)", "atoms (S = 1/h)");
      [[sc, th.blue, "C"], [sp, th.orange, "P"], [sh, th.green, "H"]].forEach(function (q) {
        ctx.strokeStyle = q[1]; ctx.lineWidth = 2; ctx.beginPath();
        for (var j = 0; j < N; j++) { var X = x0 + w * j / N, Y = y0 + h * (1 - q[0][j] / top); if (j === 0) ctx.moveTo(X, Y); else ctx.lineTo(X, Y); }
        ctx.stroke();
      });
      out.textContent = "at 100 h: circulating C = " + fmt(sc[N - 1]) + " (steady 1/β = " + fmt(1 / beta) +
        "), plate-out P = " + fmt(sp[N - 1]) + ", clean-up H = " + fmt(sh[N - 1]) + "   (blue C, orange P, green H)";
    }
    slider(ctl, "decay constant λ (1/h)", 0.001, 0.5, 0.001, lam, function (v, s) { lam = v; s.textContent = "λ = " + v.toFixed(3) + " /h"; draw(); });
    slider(ctl, "k_plate (1/h)", 0, 2, 0.01, kp, function (v, s) { kp = v; s.textContent = "k_plate = " + v.toFixed(2) + " /h"; draw(); });
    slider(ctl, "k_clean (1/h)", 0, 2, 0.01, kc, function (v, s) { kc = v; s.textContent = "k_clean = " + v.toFixed(2) + " /h"; draw(); });
    window.addEventListener("resize", function () { cv.fit(); draw(); });
    draw();
  }

  var REGISTRY = {
    layers: layers, decay: decay, walk: walk, "interface": interfaceAnim, booth: booth,
    weibull: weibull, decomp: decomp, oxidation: oxidation, pools: pools,
  };
  function init() {
    var nodes = document.querySelectorAll("div.bl-anim[data-anim]");
    for (var i = 0; i < nodes.length; i++) {
      var f = REGISTRY[nodes[i].getAttribute("data-anim")];
      if (f && !nodes[i].getAttribute("data-ready")) {
        nodes[i].setAttribute("data-ready", "1");
        try { f(nodes[i]); } catch (e) { nodes[i].textContent = "Illustration failed to start: " + e; }
      }
    }
  }
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", init); else init();
})();
