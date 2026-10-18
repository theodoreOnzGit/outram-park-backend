/* Small illustrations for the fission-product theory book (2026-10-18).
   Same pattern as docs/tutorial/anim/boonlay-anim.js: a <div class="bl-anim"
   data-anim="..."> is filled in, canvas fills the column, sliders wrap.
   Research, education and V&V only. The numbers are NOT HTR-10 data except
   where a slider label says so. The curves are cheap closed forms (no lag).
   Only widget: "building" (a lumped reactor-building control volume). */
(function () {
  function el(tag, attrs, parent, text) {
    var e = document.createElement(tag);
    for (var k in (attrs || {})) e.setAttribute(k, attrs[k]);
    if (text) e.textContent = text;
    if (parent) parent.appendChild(e);
    return e;
  }
  function colors() {
    var dark = /(navy|coal|ayu)/.test(document.documentElement.className);
    return dark ? { fg: "#ddd", grid: "#555", a: "#6cb6ff", b: "#f0a050" }
                : { fg: "#222", grid: "#bbb", a: "#1f6fbf", b: "#d9731a" };
  }
  function building(root) {
    el("p", { class: "bl-label" }, root,
      "Illustration (JavaScript, not the crate's code): one puff of activity enters a well-mixed " +
      "building at t = 0. dB/dt = -(lambda + k_dep + k_ex) B; stack release = (1 - capture) k_ex B. " +
      "Solid: with the credit sliders. Dashed: no credit (the bounding case).");
    var cv = el("canvas", { width: 600, height: 300 }, root);
    var ctl = el("div", { class: "bl-controls" }, root);
    var out = el("div", { class: "bl-readout" }, root);
    var st = { halfH: 8, kex: 1.0, kdep: 0.0, cap: 0.0, zoom: 1 };
    var specs = [
      ["half-life (h)", "halfH", 0.5, 200, 0.5, function (v) { return v.toFixed(1) + " h"; }],
      ["exhaust turnover k_ex (volumes/day; 1 is the allowable leakage in the HTR-10 vented-confinement note)", "kex", 0.1, 5, 0.1, function (v) { return v.toFixed(1) + " /day"; }],
      ["deposition k_dep (/day; illustrative, not HTR-10)", "kdep", 0, 10, 0.1, function (v) { return v.toFixed(1) + " /day"; }],
      ["filter capture (fraction; illustrative, not HTR-10)", "cap", 0, 0.99, 0.01, function (v) { return v.toFixed(2); }]
    ];
    var labels = {};
    specs.forEach(function (s) {
      var w = el("label", {}, ctl);
      labels[s[1]] = el("span", {}, w, s[0] + ": " + s[5](st[s[1]]));
      var i = el("input", { type: "range", min: s[2], max: s[3], step: s[4], value: st[s[1]] }, w);
      i.style.width = "100%";
      i.addEventListener("input", function () { st[s[1]] = parseFloat(i.value); labels[s[1]].textContent = s[0] + ": " + s[5](st[s[1]]); draw(); });
    });
    var bar = el("div", { class: "bl-controls" }, root);
    [["zoom in", function () { st.zoom = Math.min(8, st.zoom * 2); }],
     ["zoom out", function () { st.zoom = Math.max(1, st.zoom / 2); }],
     ["reset", function () { root.innerHTML = ""; building(root); throw new Error("reset"); }]
    ].forEach(function (b) {
      var x = el("button", { type: "button" }, bar, b[0]);
      x.style.minHeight = "36px"; x.style.marginRight = "6px";
      x.addEventListener("click", function () { try { b[1](); } catch (e) { return; } draw(); });
    });
    function curve(lamD, kex, kdep, cap, t) { // t in days; cumulative fraction released
      var beta = lamD + kdep + kex;
      return (1 - cap) * kex / beta * (1 - Math.exp(-beta * t));
    }
    function draw() {
      var c = colors(), ctx = cv.getContext("2d"), W = cv.width, H = cv.height;
      ctx.clearRect(0, 0, W, H);
      var lamD = Math.LN2 / (st.halfH / 24), T = 5 / st.zoom, x0 = 46, y0 = 10, w = W - 56, h = H - 46;
      var yTop = 1.0;
      ctx.strokeStyle = c.grid; ctx.fillStyle = c.fg; ctx.font = "12px sans-serif"; ctx.lineWidth = 1;
      ctx.strokeRect(x0, y0, w, h);
      ctx.fillText("t (0 to " + T.toFixed(2) + " days)", x0 + w / 2 - 50, H - 8);
      ctx.fillText("1.0", 12, y0 + 10); ctx.fillText("0", 28, y0 + h);
      ctx.save(); ctx.translate(14, y0 + h / 2 + 60); ctx.rotate(-Math.PI / 2); ctx.fillText("fraction of the puff released to the stack", 0, 0); ctx.restore();
      function line(kdep, cap, col, dash) {
        ctx.strokeStyle = col; ctx.lineWidth = 2; ctx.setLineDash(dash); ctx.beginPath();
        for (var i = 0; i <= 200; i++) {
          var t = T * i / 200, y = curve(lamD, st.kex, kdep, cap, t);
          var X = x0 + w * i / 200, Y = y0 + h * (1 - y / yTop);
          if (i === 0) ctx.moveTo(X, Y); else ctx.lineTo(X, Y);
        }
        ctx.stroke(); ctx.setLineDash([]);
      }
      line(0, 0, c.b, [6, 4]);
      line(st.kdep, st.cap, c.a, []);
      var a = curve(lamD, st.kex, 0, 0, 5), b = curve(lamD, st.kex, st.kdep, st.cap, 5);
      out.textContent = "at 5 days: no credit " + a.toFixed(3) + ", with credit " + b.toFixed(3) +
        (b > 0 ? ", so the bound over-predicts by x" + (a / b).toFixed(2) : ", credit removes it all");
    }
    draw();
  }
  function init() {
    var nodes = document.querySelectorAll(".bl-anim");
    for (var i = 0; i < nodes.length; i++) {
      if (nodes[i].getAttribute("data-anim") === "building" && !nodes[i].firstChild) building(nodes[i]);
    }
  }
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", init); else init();
})();
