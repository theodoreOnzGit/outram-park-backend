// Small illustrative widgets for the Monte Carlo tutorial (gh:#521).
//
// Each one animates ONE idea of a lesson step, with JavaScript's own
// Math.random: they are pictures of the formula on the page, not the
// transport code. The real thing runs in the demo (outram-mc-libs compiled to
// WebAssembly) and in the code each step links to. Every widget is
// touch-sized and scales to the page width (mobile-first rule).
// Inline $...$ math: mdBook's MathJax 2 recognises only \( \) inline by
// default. If MathJax has not started yet, a `window.MathJax` object is its
// configuration; if it has, configure it and typeset again.
(function () {
  const cfg = { tex2jax: { inlineMath: [["$", "$"], ["\\(", "\\)"]], processEscapes: true } };
  if (window.MathJax && window.MathJax.Hub) {
    window.MathJax.Hub.Config(cfg);
    window.MathJax.Hub.Queue(["Typeset", window.MathJax.Hub]);
  } else {
    window.MathJax = cfg;
  }
})();

(function () {
  "use strict";
  const css =(name) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  const fg = () => css("--fg") || "#222";
  const accent = () => css("--links") || "#1f6f78";

  function setupCanvas(cv, hCss) {
    const dpr = window.devicePixelRatio || 1;
    const w = cv.parentElement.clientWidth;
    cv.style.width = w + "px";
    cv.style.height = hCss + "px";
    cv.width = Math.round(w * dpr);
    cv.height = Math.round(hCss * dpr);
    const g = cv.getContext("2d");
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    return { g, w, h: hCss };
  }
  function el(tag, attrs, parent, text) {
    const e = document.createElement(tag);
    for (const k in attrs || {}) e.setAttribute(k, attrs[k]);
    if (text) e.textContent = text;
    if (parent) parent.appendChild(e);
    return e;
  }
  function button(parent, label, fn) {
    const b = el("button", { type: "button" }, parent, label);
    b.addEventListener("click", fn);
    return b;
  }
  function slider(parent, label, min, max, step, value, fn) {
    const wrap = el("label", { class: "mcw-slider" }, parent);
    el("span", {}, wrap, label + " ");
    const s = el("input", { type: "range", min, max, step, value }, wrap);
    const out = el("output", {}, wrap);
    const upd = () => { out.textContent = fn(parseFloat(s.value)); };
    s.addEventListener("input", upd);
    upd();
    return s;
  }

  // ── Free flights: histogram of d = -ln(xi)/Sigma_t against exp(-Sigma_t d) ──
  function flights(root) {
    let sigma = parseFloat(root.dataset.sigma || "0.332");
    const dMax = 15, nBins = 50;
    let bins = new Array(nBins).fill(0), n = 0, sum = 0;
    const ctl = el("div", { class: "mcw-controls" }, root);
    const cv = el("canvas", {}, root);
    const info = el("p", { class: "mcw-info" }, root);
    const draw = () => {
      const { g, w, h } = setupCanvas(cv, 220);
      g.clearRect(0, 0, w, h);
      const L = 36, B = h - 24, W = w - L - 8, H = B - 10;
      const bw = dMax / nBins;
      const peak = Math.max(1, ...bins, n * bw * sigma);
      g.fillStyle = accent();
      bins.forEach((c, i) => {
        const x = L + (i / nBins) * W, hh = (c / peak) * H;
        g.fillRect(x, B - hh, W / nBins - 1, hh);
      });
      // Expected count per bin: n * Sigma_t * exp(-Sigma_t d) * bin width.
      g.strokeStyle = fg(); g.lineWidth = 2; g.beginPath();
      for (let i = 0; i <= 200; i++) {
        const d = (i / 200) * dMax, y = B - ((n * bw * sigma * Math.exp(-sigma * d)) / peak) * H;
        i ? g.lineTo(L + (d / dMax) * W, y) : g.moveTo(L + (d / dMax) * W, y);
      }
      if (n > 0) g.stroke();
      g.fillStyle = fg(); g.font = "12px system-ui, sans-serif";
      g.fillText("0", L - 4, B + 16); g.fillText(dMax + " cm", L + W - 34, B + 16);
      g.fillText("flight length d", L + W / 2 - 40, B + 16);
      g.beginPath(); g.moveTo(L, 10); g.lineTo(L, B); g.lineTo(L + W, B); g.stroke();
      info.textContent = n
        ? `${n} flights. Mean flight ${(sum / n).toFixed(3)} cm; the mean free path 1/Σt is ${(1 / sigma).toFixed(3)} cm. Bars: sampled. Line: n·Σt·e^(−Σt·d)·Δd.`
        : "No flights yet.";
    };
    const fly = (k) => {
      for (let i = 0; i < k; i++) {
        const d = -Math.log(1 - Math.random()) / sigma;
        n++; sum += d;
        if (d < dMax) bins[Math.floor((d / dMax) * nBins)]++;
      }
      draw();
    };
    slider(ctl, "Σt (cm⁻¹)", 0.05, 1.5, 0.001, sigma, (v) => { sigma = v; bins.fill(0); n = 0; sum = 0; draw(); return v.toFixed(3); });
    const row = el("div", { class: "mcw-buttons" }, ctl);
    button(row, "Fly 1", () => fly(1));
    button(row, "Fly 100", () => fly(100));
    button(row, "Fly 10 000", () => fly(10000));
    button(row, "Clear", () => { bins.fill(0); n = 0; sum = 0; draw(); });
    window.addEventListener("resize", draw);
    draw();
  }

  // ── The surface: the shorter of d_col and d_bound wins ─────────────────────
  function surface(root) {
    const R = parseFloat(root.dataset.radius || "8.7407");
    let sigma = parseFloat(root.dataset.sigma || "0.332"), r0 = 0;
    let last = null, n = 0, leaks = 0;
    const ctl = el("div", { class: "mcw-controls" }, root);
    const cv = el("canvas", {}, root);
    const info = el("p", { class: "mcw-info" }, root);
    const draw = () => {
      const { g, w, h } = setupCanvas(cv, 260);
      g.clearRect(0, 0, w, h);
      const s = (Math.min(w, h) * 0.45) / R, cx = w / 2, cy = h / 2;
      g.fillStyle = "rgba(150,120,90,0.35)"; g.beginPath(); g.arc(cx, cy, R * s, 0, 7); g.fill();
      g.strokeStyle = "#8a63d2"; g.lineWidth = 2; g.stroke();
      g.fillStyle = fg(); g.beginPath(); g.arc(cx + r0 * s, cy, 4, 0, 7); g.fill();
      if (last) {
        const { u, dc, db } = last, d = Math.min(dc, db);
        g.strokeStyle = dc < db ? "#e0a020" : "#8a63d2"; g.lineWidth = 2.5;
        g.beginPath(); g.moveTo(cx + r0 * s, cy); g.lineTo(cx + (r0 + u[0] * d) * s, cy - u[1] * d * s); g.stroke();
        g.fillStyle = g.strokeStyle; g.beginPath(); g.arc(cx + (r0 + u[0] * d) * s, cy - u[1] * d * s, 5, 0, 7); g.fill();
      }
      info.textContent = (last
        ? `Last flight: d_col = ${last.dc.toFixed(2)} cm, d_bound = ${last.db.toFixed(2)} cm → ${last.dc < last.db ? "a COLLISION inside" : "it LEAKS out"}. `
        : "") + (n ? `${leaks} of ${n} first flights from here leaked (${((100 * leaks) / n).toFixed(1)} %).` : "");
    };
    const fly = (k) => {
      for (let i = 0; i < k; i++) {
        // Isotropic direction in 3D; the picture shows its x-y projection.
        const mu = 2 * Math.random() - 1, phi = 2 * Math.PI * Math.random(), st = Math.sqrt(1 - mu * mu);
        const u = [st * Math.cos(phi), st * Math.sin(phi), mu];
        const dc = -Math.log(1 - Math.random()) / sigma;
        // |o + d u|^2 = R^2 with o = (r0, 0, 0): d^2 + 2 k d + c = 0.
        const kk = r0 * u[0], c = r0 * r0 - R * R, db = -kk + Math.sqrt(kk * kk - c);
        last = { u, dc, db }; n++; if (dc >= db) leaks++;
      }
      draw();
    };
    slider(ctl, "start radius (cm)", 0, R - 0.01, 0.01, 0, (v) => { r0 = v; n = 0; leaks = 0; last = null; draw(); return v.toFixed(2); });
    slider(ctl, "Σt (cm⁻¹)", 0.05, 1.5, 0.001, sigma, (v) => { sigma = v; n = 0; leaks = 0; last = null; draw(); return v.toFixed(3); });
    const row = el("div", { class: "mcw-buttons" }, ctl);
    button(row, "Fly 1", () => fly(1));
    button(row, "Fly 1000", () => fly(1000));
    window.addEventListener("resize", draw);
    draw();
  }

  // ── Which reaction? xi * Sigma_t lands on a partition of the total ─────────
  function reaction(root) {
    const table = JSON.parse(root.dataset.table);
    const names = ["fission", "capture", "inelastic + (n,xn)", "elastic"];
    const cols = ["#e0a020", "#d04040", "#3a9a8a", "#5a7ad0"];
    let row = table[0], counts = [0, 0, 0, 0], lastXi = null;
    const ctl = el("div", { class: "mcw-controls" }, root);
    const cv = el("canvas", {}, root);
    const info = el("p", { class: "mcw-info" }, root);
    const draw = () => {
      const { g, w, h } = setupCanvas(cv, 110);
      g.clearRect(0, 0, w, h);
      const L = 8, W = w - 16, top = 30, bh = 34;
      let x = L;
      row.f.forEach((f, i) => {
        g.fillStyle = cols[i]; g.fillRect(x, top, f * W, bh);
        g.fillStyle = "#fff"; g.font = "12px system-ui, sans-serif";
        if (f * W > 60) g.fillText(names[i], x + 4, top + 21);
        x += f * W;
      });
      if (lastXi !== null) {
        const px = L + lastXi * W;
        g.strokeStyle = fg(); g.lineWidth = 3; g.beginPath(); g.moveTo(px, top - 10); g.lineTo(px, top + bh + 10); g.stroke();
        g.fillStyle = fg(); g.fillText("ξ·Σt", Math.min(px - 12, w - 40), top - 14);
      }
      const n = counts.reduce((a, b) => a + b, 0);
      g.fillStyle = fg(); g.font = "12px system-ui, sans-serif";
      g.fillText(`E = ${row.label} · Σt = ${row.st} cm⁻¹`, L, h - 12);
      info.textContent = n
        ? `${n} collisions: ` + names.map((nm, i) => `${nm} ${((100 * counts[i]) / n).toFixed(1)} % (bar ${(100 * row.f[i]).toFixed(1)} %)`).join(" · ")
        : "No collisions yet.";
    };
    const collide = (k) => {
      for (let i = 0; i < k; i++) {
        const xi = Math.random();
        let acc = 0, j = 0;
        for (; j < 3; j++) { acc += row.f[j]; if (xi < acc) break; }
        counts[j]++; lastXi = xi;
      }
      draw();
    };
    const sel = el("select", {}, ctl);
    table.forEach((r, i) => el("option", { value: i }, sel, r.label));
    sel.value = String(table.findIndex((r) => r.label === "1 MeV"));
    row = table[+sel.value];
    sel.addEventListener("change", () => { row = table[+sel.value]; counts = [0, 0, 0, 0]; lastXi = null; draw(); });
    const b = el("div", { class: "mcw-buttons" }, ctl);
    button(b, "Collide", () => collide(1));
    button(b, "Collide 1000", () => collide(1000));
    window.addEventListener("resize", draw);
    draw();
  }

  // Targets for the slowing-down widgets: AWR from each ENDF/B-VIII.0 tape
  // (the table on the rung-2 and rung-4 pages, computed 2026-10-04).
  const TARGETS = [
    { label: "H-1 (water)", awr: 0.9991673 },
    { label: "C-12 (graphite)", awr: 11.89365 },
    { label: "U-238", awr: 236.0058 },
  ];
  const alphaOf = (a) => ((a - 1) / (a + 1)) ** 2;
  const xiOf = (a) => { const al = alphaOf(a); return al < 1e-12 ? 1 : 1 + (al * Math.log(al)) / (1 - al); };
  function targetSelect(parent, init, fn) {
    const sel = el("select", {}, parent);
    TARGETS.forEach((t, i) => el("option", { value: i }, sel, t.label));
    sel.value = String(init);
    sel.addEventListener("change", () => fn(TARGETS[+sel.value]));
    return TARGETS[init];
  }

  // ── One elastic collision: the centre-of-mass circle and the lab energy ───
  // Target at rest, isotropic in the CM. In velocity space the neutron's
  // outgoing velocity is v_cm + (A/(A+1)) v on a circle; E'/E is the squared
  // length of that vector, which lands uniformly in [alpha, 1].
  function collision(root) {
    let t = TARGETS[+(root.dataset.target || 1)], last = null;
    const nBins = 40;
    let bins = new Array(nBins).fill(0), n = 0;
    const ctl = el("div", { class: "mcw-controls" }, root);
    const cv = el("canvas", {}, root);
    const info = el("p", { class: "mcw-info" }, root);
    const reset = () => { bins.fill(0); n = 0; last = null; };
    const draw = () => {
      const { g, w, h } = setupCanvas(cv, 260);
      g.clearRect(0, 0, w, h);
      const A = t.awr, al = alphaOf(A);
      // Left: velocity space, incident v = 1 along +x.
      const s = Math.min(w * 0.45, h - 40) / 2.2, ox = 16 + s * 0.2, oy = h / 2;
      const vcm = 1 / (A + 1), rc = A / (A + 1);
      g.strokeStyle = fg(); g.lineWidth = 1;
      g.beginPath(); g.arc(ox + vcm * s, oy, rc * s, 0, 7); g.stroke();
      g.fillStyle = fg(); g.font = "12px system-ui, sans-serif";
      g.fillText("CM circle", ox + vcm * s - 24, oy - rc * s - 6);
      g.strokeStyle = "#888"; g.beginPath(); g.moveTo(ox, oy); g.lineTo(ox + s, oy); g.stroke();
      g.fillText("v in", ox + s - 20, oy + 14);
      if (last) {
        const px = ox + (vcm + rc * last.mu) * s, py = oy - rc * Math.sqrt(1 - last.mu * last.mu) * s;
        g.strokeStyle = accent(); g.lineWidth = 2.5;
        g.beginPath(); g.moveTo(ox, oy); g.lineTo(px, py); g.stroke();
        g.fillStyle = accent(); g.beginPath(); g.arc(px, py, 4, 0, 7); g.fill();
      }
      // Right: the lab energy bar [alpha E, E] and the histogram of E'/E.
      const L = w * 0.52, W = w - L - 10, B = h - 30, H = B - 40;
      g.fillStyle = "rgba(128,128,128,0.25)"; g.fillRect(L + al * W, 14, (1 - al) * W, 10);
      g.fillStyle = fg(); g.fillText("αE", L + al * W - 8, 40); g.fillText("E", L + W - 6, 40);
      if (last) { g.fillStyle = accent(); g.fillRect(L + last.r * W - 2, 8, 4, 22); }
      const peak = Math.max(1, ...bins);
      g.fillStyle = accent();
      bins.forEach((c, i) => { const hh = (c / peak) * H; g.fillRect(L + (i / nBins) * W, B - hh, W / nBins - 1, hh); });
      g.strokeStyle = fg(); g.lineWidth = 1; g.beginPath(); g.moveTo(L, B); g.lineTo(L + W, B); g.stroke();
      g.fillStyle = fg(); g.fillText("0", L - 4, B + 14); g.fillText("E′/E = 1", L + W - 44, B + 14);
      info.textContent = `${t.label}: A = ${A}, α = ${al.toPrecision(4)}, so one collision leaves between ${(100 * al).toFixed(al > 0.01 ? 1 : 5)} % and 100 % of the energy; ξ = ${xiOf(A).toPrecision(4)}.` +
        (last ? ` Last: μ_cm = ${last.mu.toFixed(3)}, E′/E = ${last.r.toFixed(4)}.` : "") +
        (n ? ` ${n} collisions, mean E′/E ${(sum / n).toFixed(3)} (exact (1+α)/2 = ${((1 + al) / 2).toFixed(3)}).` : "");
    };
    let sum = 0;
    const collide = (k, muFixed) => {
      const al = alphaOf(t.awr);
      for (let i = 0; i < k; i++) {
        const mu = muFixed === undefined ? 2 * Math.random() - 1 : muFixed;
        const r = 0.5 * ((1 + al) + (1 - al) * mu);
        last = { mu, r }; n++; sum += r;
        bins[Math.min(nBins - 1, Math.floor(r * nBins))]++;
      }
      draw();
    };
    targetSelect(ctl, +(root.dataset.target || 1), (nt) => { t = nt; reset(); sum = 0; draw(); });
    const row = el("div", { class: "mcw-buttons" }, ctl);
    button(row, "Collide", () => collide(1));
    button(row, "Collide 1000", () => collide(1000));
    button(row, "μ_cm = −1 (head-on)", () => collide(1, -1));
    button(row, "μ_cm = +1 (grazing)", () => collide(1, 1));
    button(row, "Clear", () => { reset(); sum = 0; draw(); });
    window.addEventListener("resize", draw);
    draw();
  }

  // ── Slowing down: energy against collision number, and how many it takes ──
  function slowdown(root) {
    let t = TARGETS[+(root.dataset.target || 1)], path = [], counts = [];
    const E0 = 2.0e6, ET = 0.025;
    const ctl = el("div", { class: "mcw-controls" }, root);
    const cv = el("canvas", {}, root);
    const info = el("p", { class: "mcw-info" }, root);
    const walk = () => {
      const al = alphaOf(t.awr);
      let e = E0, p = [e], k = 0;
      while (e > ET && k < 20000) { e *= 0.5 * ((1 + al) + (1 - al) * (2 * Math.random() - 1)); p.push(e); k++; }
      return p;
    };
    const draw = () => {
      const { g, w, h } = setupCanvas(cv, 240);
      g.clearRect(0, 0, w, h);
      const L = 58, B = h - 24, W = w - L - 10, H = B - 10;
      const nExp = Math.log(E0 / ET) / xiOf(t.awr);
      const nMax = Math.max(20, 1.6 * nExp, path.length);
      const y = (e) => B - ((Math.log10(e) - Math.log10(ET / 3)) / (Math.log10(E0) - Math.log10(ET / 3))) * H;
      g.strokeStyle = fg(); g.lineWidth = 1; g.beginPath(); g.moveTo(L, 10); g.lineTo(L, B); g.lineTo(L + W, B); g.stroke();
      g.fillStyle = fg(); g.font = "12px system-ui, sans-serif";
      [[2e6, "2 MeV"], [1e3, "1 keV"], [1, "1 eV"], [ET, "0.025 eV"]].forEach(([e, lb]) => g.fillText(lb, 2, y(e) + 4));
      g.fillText("collision number", L + W / 2 - 40, B + 16); g.fillText(String(Math.round(nMax)), L + W - 24, B + 16);
      // The average line: ln E falls by xi per collision.
      g.strokeStyle = "#999"; g.setLineDash([5, 4]); g.beginPath(); g.moveTo(L, y(E0)); g.lineTo(L + (nExp / nMax) * W, y(ET)); g.stroke(); g.setLineDash([]);
      if (path.length) {
        g.strokeStyle = accent(); g.lineWidth = 2; g.beginPath();
        path.forEach((e, i) => { const X = L + (i / nMax) * W; i ? g.lineTo(X, y(e)) : g.moveTo(X, y(e)); });
        g.stroke();
      }
      const m = counts.length ? counts.reduce((a, b) => a + b, 0) / counts.length : 0;
      info.textContent = `${t.label}: ξ = ${xiOf(t.awr).toPrecision(4)}, so on average n = ln(2 MeV / 0.025 eV)/ξ = ${nExp.toFixed(1)} collisions (dashed line).` +
        (path.length ? ` This neutron took ${path.length - 1}.` : "") +
        (counts.length ? ` ${counts.length} neutrons: mean ${m.toFixed(1)} collisions, fewest ${Math.min(...counts)}, most ${Math.max(...counts)}.` : "") +
        " (Target at rest, isotropic in the centre of mass: below a few eV this is wrong, which is step 4.)";
    };
    targetSelect(ctl, +(root.dataset.target || 1), (nt) => { t = nt; path = []; counts = []; draw(); });
    const row = el("div", { class: "mcw-buttons" }, ctl);
    button(row, "One neutron", () => { path = walk(); counts.push(path.length - 1); draw(); });
    button(row, "1000 neutrons", () => { for (let i = 0; i < 1000; i++) { const p = walk(); counts.push(p.length - 1); path = p; } draw(); });
    button(row, "Clear", () => { path = []; counts = []; draw(); });
    window.addEventListener("resize", draw);
    draw();
  }

  // ── Spatial self-shielding: neutrons entering a lump, at one energy ───────
  // A sphere of radius r and absorption cross section Sigma (one energy, pure
  // absorber). Neutrons enter through the surface from an isotropic flux
  // outside (cosine law: mu = sqrt(xi) to the inward normal), so the chord is
  // 2 r mu, and each is absorbed if its sampled flight -ln(xi)/Sigma is
  // shorter. The dilute limit absorbs Sigma * (mean chord 4r/3) per entering
  // neutron; the ratio of the two is how effective each atom still is.
  function lump(root) {
    let tau = parseFloat(root.dataset.tau || "1"); // Sigma * r
    let shots = [], n = 0, absorbed = 0;
    const ctl = el("div", { class: "mcw-controls" }, root);
    const cv = el("canvas", {}, root);
    const info = el("p", { class: "mcw-info" }, root);
    const exact = (t) => {
      // P_abs for a sphere in an isotropic flux: 1 - escape over the
      // cosine-weighted chords, P = 1 - (1 - (1 + 2t) e^{-2t}) / (2 t^2).
      if (t < 1e-6) return (4 / 3) * t;
      return 1 - (1 - (1 + 2 * t) * Math.exp(-2 * t)) / (2 * t * t);
    };
    const draw = () => {
      const { g, w, h } = setupCanvas(cv, 250);
      g.clearRect(0, 0, w, h);
      const R = Math.min(w, h) * 0.4, cx = Math.min(w * 0.3, R + 20), cy = h / 2;
      g.fillStyle = "rgba(214,120,46,0.35)"; g.beginPath(); g.arc(cx, cy, R, 0, 7); g.fill();
      g.strokeStyle = "#d6782e"; g.lineWidth = 2; g.stroke();
      shots.slice(-300).forEach((s) => {
        const y0 = cy - s.b * R, x0 = cx - Math.sqrt(Math.max(0, 1 - s.b * s.b)) * R;
        g.strokeStyle = s.abs ? "rgba(208,64,64,0.55)" : "rgba(90,122,208,0.35)"; g.lineWidth = 1;
        g.beginPath(); g.moveTo(x0, y0); g.lineTo(x0 + s.d * R, y0); g.stroke();
        if (s.abs) { g.fillStyle = "#d04040"; g.beginPath(); g.arc(x0 + s.d * R, y0, 2.2, 0, 7); g.fill(); }
      });
      const P = exact(tau), dil = (4 / 3) * tau;
      const L = cx + R + 30, W = w - L - 10;
      if (W > 60) {
        g.fillStyle = fg(); g.font = "12px system-ui, sans-serif";
        g.fillText("per atom, vs dilute", L, 30);
        g.fillStyle = accent(); g.fillRect(L, 40, W * Math.min(1, P / dil), 16);
        g.strokeStyle = fg(); g.strokeRect(L, 40, W, 16);
        g.fillStyle = fg(); g.fillText(`${(100 * P / dil).toFixed(1)} %`, L, 72);
      }
      info.textContent = `Σr = ${tau.toFixed(2)} (the lump's radius in mean free paths). Exact for a sphere: an entering neutron is absorbed with probability ${P.toFixed(3)}; ` +
        `if the same atoms were spread thin they would absorb Σ·(4r/3) = ${dil.toFixed(3)} per entering neutron, so each atom in the lump is ${(100 * P / dil).toFixed(1)} % as effective.` +
        (n ? ` Sampled: ${absorbed} of ${n} absorbed (${(absorbed / n).toFixed(3)}).` : "") +
        " Red dots: absorptions. At a resonance peak (Σr large) they crowd the surface.";
    };
    const shoot = (k) => {
      for (let i = 0; i < k; i++) {
        const mu = Math.sqrt(Math.random()); // cosine-law entry
        const chord = 2 * mu; // in units of r
        const d = -Math.log(1 - Math.random()) / tau; // in units of r
        const abs = d < chord;
        shots.push({ b: Math.sqrt(1 - mu * mu) * (Math.random() < 0.5 ? 1 : -1), d: Math.min(d, chord), abs });
        n++; absorbed += abs;
      }
      if (shots.length > 2000) shots = shots.slice(-1000);
      draw();
    };
    slider(ctl, "Σr", -2, 2, 0.01, Math.log10(tau), (v) => { tau = 10 ** v; shots = []; n = 0; absorbed = 0; draw(); return tau.toFixed(2); });
    const row = el("div", { class: "mcw-buttons" }, ctl);
    button(row, "Send 1", () => shoot(1));
    button(row, "Send 100", () => shoot(100));
    button(row, "Send 10 000", () => shoot(10000));
    window.addEventListener("resize", draw);
    draw();
  }

  // ── The demo, embedded on demand (it downloads and processes nuclear data) ─
  function demo(root) {
    const src = root.dataset.src;
    const go = el("button", { type: "button", class: "mcw-start" }, root, root.dataset.label || "Start the demo here");
    el("span", {}, root, " or ");
    el("a", { href: src, target: "_blank", rel: "noopener" }, root, "open it full screen");
    go.addEventListener("click", () => {
      root.textContent = "";
      el("iframe", { src, title: "Monte Carlo demo", loading: "lazy", allow: "fullscreen" }, root);
      el("a", { href: src, target: "_blank", rel: "noopener", class: "mcw-full" }, root, "Open the demo full screen");
    });
  }

  const kinds = { flights, surface, reaction, collision, slowdown, lump, demo };
  document.querySelectorAll("[data-mc-widget]").forEach((e) => {
    const k = kinds[e.dataset.mcWidget];
    if (k) k(e);
  });
})();
