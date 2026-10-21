// Neutronics lessons, page 17 (gh:#906 design and amendments, 2026-10-16):
// why cross-section graphs are logarithmic. A neutron slows down by elastic
// collisions with a free nucleus at rest, isotropic in the centre-of-mass
// frame: each collision keeps a fraction kept(A, muCM) of its energy, with
// muCM uniform in [-1, 1], so E' is uniform in [alpha E, E] (page 2's
// kinematics, L.kinematicsMath). The mean step in ln E is
// xi = 1 + alpha ln(alpha) / (1 - alpha), whatever the energy.
// Data: slowing-data.js (mass ratios from each tape's MF=1, curves from
// OUTRAM PARK's RECONR port at 0 K, U-238's resolved resonance energies).
(function () {
  "use strict";
  var L = window.Lesson;
  var css = L.css, el = L.el, fmtE = L.fmtE, Frame = L.Frame;
  var K = L.kinematicsMath;
  var D = window.SLOWING_DATA;
  var E_BIRTH = 2e6, E_STOP = 1, E_TH = 0.0253;

  function xi(A) { var a = K.alpha(A); return a === 0 ? 1 : 1 + a * Math.log(a) / (1 - a); }
  // Average number of collisions from E0 down to E1.
  function collisions(E0, E1, A) { return Math.log(E0 / E1) / xi(A); }
  // One collision: u uniform in [0, 1) gives muCM = 2u - 1.
  function collide(E, A, u) { return E * K.kept(A, 2 * u - 1); }
  // Least-squares slope of ln s against ln e over lo <= e <= hi.
  function slope(e, s, lo, hi) {
    var n = 0, sx = 0, sy = 0, sxx = 0, sxy = 0;
    for (var i = 0; i < e.length; i++) {
      if (e[i] < lo || e[i] > hi || !(s[i] > 0)) continue;
      var x = Math.log(e[i]), y = Math.log(s[i]);
      n++; sx += x; sy += y; sxx += x * x; sxy += x * y;
    }
    return { n: n, slope: (n * sxy - sx * sy) / (n * sxx - sx * sx) };
  }
  // sigma at E by log-log interpolation in a curve.
  function at(c, E) {
    var e = c.e, s = c.s;
    if (E <= e[0]) return s[0];
    for (var i = 1; i < e.length; i++) if (e[i] >= E) {
      var t = Math.log(E / e[i - 1]) / Math.log(e[i] / e[i - 1]);
      return Math.exp(Math.log(s[i - 1]) + t * Math.log(s[i] / s[i - 1]));
    }
    return s[s.length - 1];
  }
  // How many of the sorted energies lie in [lo, hi).
  function count(sorted, lo, hi) { var n = 0; sorted.forEach(function (x) { if (x >= lo && x < hi) n++; }); return n; }
  function rng(seed) { return function () { seed |= 0; seed = seed + 0x6D2B79F5 | 0; var t = Math.imul(seed ^ seed >>> 15, 1 | seed); t = t + Math.imul(t ^ t >>> 7, 61 | t) ^ t; return ((t ^ t >>> 14) >>> 0) / 4294967296; }; }
  L.slowingMath = { xi: xi, collisions: collisions, collide: collide, slope: slope, at: at, count: count, rng: rng, E_BIRTH: E_BIRTH, E_STOP: E_STOP, E_TH: E_TH };

  var CURVES = [["U-238 capture", "#b85c38", "u238"], ["C-12 elastic", "#5b636b", "c12"], ["B-10 (n,alpha)", "#7a5ea8", "b10"]];

  function slowWidget(root) {
    var st = { nuc: "C-12", trail: [E_BIRTH], run: false, logE: true, logS: true, ticks: false, ov: { u238: true, c12: true, b10: false }, seed: 1 };
    var draw01 = rng(st.seed);
    function restart() { st.trail = [E_BIRTH]; st.run = false; st.seed++; draw01 = rng(st.seed); }
    function hop() {
      var E = st.trail[st.trail.length - 1];
      if (E <= E_STOP) { st.run = false; return; }
      st.trail.push(collide(E, D.awr[st.nuc], draw01()));
    }
    var frame = new Frame(root, {
      reset: function () { restart(); },
      tick: function (dt) {
        if (!st.run) return false;
        // About three seconds from birth to 1 eV, whatever the nuclide.
        var per = Math.max(1, Math.round(collisions(E_BIRTH, E_STOP, D.awr[st.nuc]) * dt / 3));
        for (var i = 0; i < per && st.run; i++) hop();
        return st.run;
      },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b");
        var x0 = 46, y0 = 84, pw = w - x0 - 12, ph = h - y0 - 74;
        var lx0 = -3, lx1 = Math.log10(2e7), ly0 = -3, ly1 = 5;
        var lin = { e1: 2.1e6 / zoom, s1: 0 };
        var X = st.logE ? function (e) { return x0 + (Math.log10(e) - lx0) / (lx1 - lx0) * pw * zoom - (zoom - 1) * pw; } : function (e) { return x0 + e / lin.e1 * pw; };
        // Linear sigma: up to the highest value drawn.
        if (!st.logS) CURVES.forEach(function (c) { if (st.ov[c[2]]) D.curves[c[0]].s.forEach(function (v, i) { if (D.curves[c[0]].e[i] >= 1e-3) lin.s1 = Math.max(lin.s1, v); }); });
        var Y = st.logS ? function (s) { return y0 + (ly1 - Math.log10(Math.max(s, 1e-9))) / (ly1 - ly0) * ph; } : function (s) { return y0 + ph - s / (lin.s1 || 1) * ph; };
        ctx.font = "12px system-ui, sans-serif"; ctx.strokeStyle = mut; ctx.lineWidth = 1; ctx.strokeRect(x0, y0, pw, ph);
        ctx.save(); ctx.beginPath(); ctx.rect(x0, y0, pw, ph); ctx.clip();
        // The thermal region, where the nucleus's motion matters (page 3).
        var xs = X(E_STOP);
        if (st.logE && xs > x0) { ctx.fillStyle = "rgba(128,128,128,0.12)"; ctx.fillRect(x0, y0, xs - x0, ph); }
        // Grid.
        ctx.strokeStyle = "rgba(128,128,128,0.22)";
        if (st.logE) for (var d = lx0; d <= 7; d++) { var gx = X(Math.pow(10, d)); ctx.beginPath(); ctx.moveTo(gx, y0); ctx.lineTo(gx, y0 + ph); ctx.stroke(); }
        CURVES.forEach(function (c) {
          if (!st.ov[c[2]]) return;
          var C = D.curves[c[0]]; ctx.strokeStyle = c[1]; ctx.lineWidth = 1.2; ctx.globalAlpha = 0.75; ctx.beginPath();
          for (var i = 0, first = true; i < C.e.length; i++) { if (C.e[i] < 1e-3) continue; var xx = X(C.e[i]), yy = Y(C.s[i]); if (first) { ctx.moveTo(xx, yy); first = false; } else ctx.lineTo(xx, yy); }
          ctx.stroke(); ctx.globalAlpha = 1;
          // B-10's 1/v reference: through its own value at 0.0253 eV.
          if (c[2] === "b10") {
            var s0 = at(C, E_TH); ctx.setLineDash([5, 4]); ctx.strokeStyle = c[1]; ctx.beginPath();
            for (var k = 0; k <= 60; k++) { var e = Math.pow(10, -3 + k * 9 / 60), yy2 = Y(s0 * Math.sqrt(E_TH / e)); if (k) ctx.lineTo(X(e), yy2); else ctx.moveTo(X(e), yy2); }
            ctx.stroke(); ctx.setLineDash([]);
          }
        });
        // U-238's resolved resonances, as ticks along the top: s-wave dark, p-wave faint.
        if (st.ticks) [["1", "rgba(184,92,56,0.35)", 6], ["0", "#b85c38", 12]].forEach(function (t) {
          ctx.strokeStyle = t[1]; ctx.lineWidth = 1; ctx.beginPath();
          D.u238.levels[t[0]].forEach(function (e) { var xx = X(e); ctx.moveTo(xx, y0); ctx.lineTo(xx, y0 + t[2]); });
          ctx.stroke();
        });
        // The trail: one mark per collision, on a strip at the bottom.
        var ty = y0 + ph - 10;
        ctx.fillStyle = acc;
        st.trail.forEach(function (e) { ctx.beginPath(); ctx.arc(X(e), ty, 2.2, 0, 2 * Math.PI); ctx.fill(); });
        var Enow = st.trail[st.trail.length - 1], xn = X(Enow);
        ctx.strokeStyle = acc; ctx.setLineDash([3, 3]); ctx.beginPath(); ctx.moveTo(xn, y0); ctx.lineTo(xn, y0 + ph); ctx.stroke(); ctx.setLineDash([]);
        ctx.beginPath(); ctx.arc(xn, ty, 6, 0, 2 * Math.PI); ctx.fill();
        ctx.restore();
        // Axis labels.
        ctx.fillStyle = mut;
        if (st.logE) [["1 meV", -3], ["1 eV", 0], ["1 keV", 3], ["1 MeV", 6]].forEach(function (t) { var gx = X(Math.pow(10, t[1])); if (gx < x0 - 1 || gx > x0 + pw + 1) return; ctx.fillText(t[0], gx - ctx.measureText(t[0]).width / 2, y0 + ph + 16); });
        else { ctx.fillText("0", x0, y0 + ph + 16); var tl = fmtE(lin.e1); ctx.fillText(tl, x0 + pw - ctx.measureText(tl).width, y0 + ph + 16); }
        if (st.logS) for (var b = ly0; b <= ly1; b += 2) ctx.fillText("10" + String(b).replace("-", "⁻").replace(/[0-9]/g, function (c) { return "⁰¹²³⁴⁵⁶⁷⁸⁹"[+c]; }), 6, Y(Math.pow(10, b)) + 4);
        else { ctx.fillText("0 b", 6, y0 + ph); ctx.fillText(Math.round(lin.s1 / 1e3) + " kb", 4, y0 + 10); }
        ctx.fillStyle = fg; ctx.fillText("σ [b]; " + (st.logE ? "log E" : "linear E") + (st.logS ? ", log σ" : ", linear σ") + (st.logE ? "; shaded: below 1 eV" : ""), x0, y0 - 10);
        // Legend under the axis, one line per curve so it fits a phone.
        var ly = y0 + ph + 32, lx = x0;
        CURVES.forEach(function (c) {
          if (!st.ov[c[2]]) return;
          var t = c[0].replace("alpha", "α"), tw = ctx.measureText(t).width + 30;
          if (lx + tw > x0 + pw) { lx = x0; ly += 16; }
          ctx.strokeStyle = c[1]; ctx.lineWidth = 2; ctx.beginPath(); ctx.moveTo(lx, ly - 4); ctx.lineTo(lx + 18, ly - 4); ctx.stroke();
          ctx.fillStyle = fg; ctx.fillText(t, lx + 22, ly); lx += tw;
        });
        var A = D.awr[st.nuc], a = K.alpha(A), n = st.trail.length - 1, mean = n ? Math.log(E_BIRTH / Enow) / n : 0;
        return st.nuc + " (A = " + A + "): α = " + a.toFixed(4) + ", so one collision keeps between " + (100 * a).toFixed(1) + " % and 100 % of the energy; ξ = " + xi(A).toFixed(4) + ". " +
          n + " collision" + (n === 1 ? "" : "s") + ", now at " + fmtE(Enow) + (n ? "; mean step in ln E so far " + mean.toFixed(4) : "") + ". On average " + collisions(E_BIRTH, E_STOP, A).toFixed(0) + " collisions from 2 MeV to 1 eV." +
          (Enow <= E_STOP ? " The walk stops at 1 eV: below that the nucleus's own motion matters (page 3), and a neutron can even gain energy." : "") +
          " Curves: ENDF/B-VIII.0 by OUTRAM PARK's RECONR port, 0 K.";
      },
      panel: function (body, redraw) {
        L.toggle(body, "Logarithmic σ axis", st.logS, function (v) { st.logS = v; redraw(); });
        CURVES.forEach(function (c) { L.toggle(body, c[0].replace("alpha", "α") + (c[2] === "b10" ? ", with a dashed 1/v line" : ""), st.ov[c[2]], function (v) { st.ov[c[2]] = v; redraw(); }); });
        L.toggle(body, "U-238's resolved resonances (ticks)", st.ticks, function (v) { st.ticks = v; redraw(); });
        body.append(el("p", { "class": "w-note" }, "Elastic scattering off a free nucleus at rest, isotropic in the centre-of-mass frame: each collision keeps a random fraction between α and 1, drawn from page 2's kinematics. Mass ratios from each evaluation's MF=1."));
      }
    });
    var card = root.parentNode;
    card.querySelectorAll("[data-nuc]").forEach(function (b) {
      b.onclick = function () {
        st.nuc = b.getAttribute("data-nuc"); restart();
        card.querySelectorAll("[data-nuc]").forEach(function (o) { o.classList.toggle("ghost", o !== b); });
        frame.redraw();
      };
    });
    var on = function (sel, f) { var b = card.querySelector(sel); if (b) b.onclick = f; };
    on("[data-hop]", function () { st.run = false; hop(); frame.redraw(); });
    on("[data-run]", function () { if (st.trail[st.trail.length - 1] <= E_STOP) restart(); st.run = true; frame.play(); });
    on("[data-new]", function () { restart(); frame.redraw(); });
    card.querySelectorAll("[data-axis]").forEach(function (b) {
      b.onclick = function () {
        st.logE = b.getAttribute("data-axis") === "log";
        card.querySelectorAll("[data-axis]").forEach(function (o) { o.classList.toggle("ghost", o !== b); });
        frame.redraw();
      };
    });
    return frame;
  }

  L.widget("slowdown", slowWidget);
})();
