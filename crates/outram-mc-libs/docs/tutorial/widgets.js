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

  const kinds = { flights, surface, reaction, demo };
  document.querySelectorAll("[data-mc-widget]").forEach((e) => {
    const k = kinds[e.dataset.mcWidget];
    if (k) k(e);
  });
})();
