// Neutronics lessons: which nucleus was hit (free-gas target sampling), DBRC,
// and the hand-over from the bound-atom table to the free-gas one (gh:#906,
// maintainer's arc of 2026-10-18).
//
// What the JavaScript does, and what it ports:
//  - sampleCxs is a line-for-line port of crates/outram-mc-libs
//    src/physics/scatter.rs::sample_cxs_target (scatter.rs:565-600 at the
//    time of writing), itself OpenMC's sample_cxs_target_velocity
//    (src/physics.cpp:1024). The target speed y = sqrt(M/2kT)|V| comes from a
//    two-branch Maxwellian mixture, the pair (y, mu) is accepted with
//    |v - V|/(v + V), so the accepted targets are weighted by the Maxwellian
//    AND the relative speed.
//  - encounter adds DBRC as scatter.rs::sample_target_velocity_dbrc
//    (scatter.rs:500-565) does, following OpenMC's ResScatMethod::dbrc
//    (physics.cpp:870-965): E_up = (sqrt(AE/kT) + 4)^2 kT/A, candidates with
//    E_rel >= E_up are redrawn, the rest accepted with sigma_0K(E_rel)/sigma_max
//    where sigma_max is the 0 K maximum over [E_low, E_up].
//  - encounterDensity is the density those two samplers draw from, by 2-D
//    quadrature over (y, mu): noise-free and cheap. The DBRC widget draws it
//    rather than sampling, because next to the peak the rejection accepts
//    only about 4 targets in 10 000 (measured 2026-10-18, E = 6.5 eV,
//    293.6 K), which is fine for a transport code that needs one target per
//    collision and far too slow for a page that wants 20 000 of them.
// All of it is exposed on window.Lesson.dbrcMath so check-physics.js can test
// it under node: both samplers against the quadrature, DBRC with a flat cross
// section against plain sampling.
(function () {
  "use strict";
  var L = window.Lesson;
  var css = L.css, el = L.el, fmtE = L.fmtE, buttons = L.buttons, slider = L.slider, toggle = L.toggle, Frame = L.Frame;
  var KB = 8.617333262e-5; // eV/K, CODATA 2018 (exact)

  // ─── Pure maths ────────────────────────────────────────────────────────
  function rng(seed) {
    return function () {
      seed = (seed + 0x6D2B79F5) | 0;
      var t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
      t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }
  function pos(r) { var x = r(); return x > 0 ? x : 1e-300; }

  // One target in the constant-cross-section approximation: [y^2, mu].
  function sampleCxs(yn, alpha, r) {
    var y2 = 0, mu = 0;
    for (var t = 0; t < 1024; t++) {
      if (r() < alpha) y2 = -Math.log(pos(r) * pos(r));
      else { var c = Math.cos(Math.PI / 2 * r()); y2 = -Math.log(pos(r)) - Math.log(pos(r)) * c * c; }
      var y = Math.sqrt(y2);
      mu = 2 * r() - 1;
      var den = yn + y;
      var acc = den > 0 ? Math.sqrt(Math.max(0, yn * yn + y2 - 2 * yn * y * mu)) / den : 1;
      if (r() < acc) break;
    }
    return [y2, mu];
  }

  // A 0 K cross-section table with the three operations DBRC needs
  // (scatter.rs DbrcTable::xs_at, xs_max_over, has_point_inside).
  function table(e, s) {
    var n = e.length;
    function lower(x) { var lo = 0, hi = n; while (lo < hi) { var m = (lo + hi) >> 1; if (e[m] < x) lo = m + 1; else hi = m; } return lo; } // first index with e >= x
    function upper(x) { var lo = 0, hi = n; while (lo < hi) { var m = (lo + hi) >> 1; if (e[m] <= x) lo = m + 1; else hi = m; } return lo; } // first index with e > x
    function at(x) {
      if (x <= e[0]) return s[0];
      if (x >= e[n - 1]) return s[n - 1];
      var hi = Math.min(Math.max(lower(x), 1), n - 1), x0 = e[hi - 1], x1 = e[hi];
      return x1 > x0 ? s[hi - 1] + (s[hi] - s[hi - 1]) * (x - x0) / (x1 - x0) : s[hi - 1];
    }
    function maxOver(lo, hi) {
      var m = Math.max(at(lo), at(hi));
      for (var k = lower(lo); k < n && e[k] <= hi; k++) m = Math.max(m, s[k]);
      return m;
    }
    function hasPointInside(lo, hi) { var a = upper(lo); return a < n && e[a] < hi; }
    return { e: e, s: s, at: at, maxOver: maxOver, hasPointInside: hasPointInside };
  }

  // The DBRC window for a neutron of energy E on a target of mass ratio A at
  // kT (physics.cpp: E_low, E_up in terms of E_red = sqrt(A E / kT)).
  function window_(E, A, kT) {
    var yn = Math.sqrt(A * E / kT);
    return { yn: yn, lo: Math.pow(Math.max(0, yn - 4), 2) * kT / A, up: Math.pow(yn + 4, 2) * kT / A };
  }

  // One encounter. tab = null (or sigma flat) gives plain sampling; with a
  // table the DBRC rejection runs. Returns { y2, mu, erel, tries }.
  function encounter(E, A, kT, r, tab) {
    var w = window_(E, A, kT), yn = w.yn, alpha = 1 / (1 + Math.sqrt(Math.PI) * yn / 2);
    function erelOf(c) {
      var y = Math.sqrt(c[0]);
      return Math.max(0, yn * yn + c[0] - 2 * yn * y * c[1]) * kT / A;
    }
    var smax = tab && tab.hasPointInside(w.lo, w.up) ? tab.maxOver(w.lo, w.up) : 0;
    if (!(smax > 0)) { var c0 = sampleCxs(yn, alpha, r); return { y2: c0[0], mu: c0[1], erel: erelOf(c0), tries: 1 }; }
    var tries = 0;
    for (;;) {
      var c, er;
      do { c = sampleCxs(yn, alpha, r); er = erelOf(c); tries++; } while (!(er < w.up));
      if (r() < tab.at(er) / smax) return { y2: c[0], mu: c[1], erel: er, tries: tries };
    }
  }

  // The encounter-energy histograms of the two samplers by quadrature:
  // returns { plain, dbrc, accept } over nb bins on [elo, ehi], each
  // normalised to unit area, with accept = the DBRC acceptance
  // <sigma>/sigma_max over the window. Both densities share the weight
  // y^2 e^{-y^2}|v - V| and the cut E_rel < E_up; DBRC multiplies by sigma.
  function encounterDensity(E, A, kT, tab, elo, ehi, nb, ny, nmu) {
    var w = window_(E, A, kT), yn = w.yn, hp = new Float64Array(nb), hd = new Float64Array(nb), ymax = 7, sp = 0, sd = 0;
    var smax = tab.hasPointInside(w.lo, w.up) ? tab.maxOver(w.lo, w.up) : 0;
    for (var i = 0; i < ny; i++) {
      var y = (i + 0.5) * ymax / ny, wy = y * y * Math.exp(-y * y);
      for (var j = 0; j < nmu; j++) {
        var mu = -1 + (j + 0.5) * 2 / nmu, e2 = yn * yn + y * y - 2 * yn * y * mu, er = Math.max(0, e2) * kT / A;
        if (er >= w.up) continue;
        var wt = wy * Math.sqrt(Math.max(0, e2)), sg = smax > 0 ? tab.at(er) : 1;
        sp += wt; sd += wt * sg;
        var b = Math.floor((er - elo) / (ehi - elo) * nb);
        if (b >= 0 && b < nb) { hp[b] += wt; hd[b] += wt * sg; }
      }
    }
    var bw = (ehi - elo) / nb;
    for (var k = 0; k < nb; k++) { hp[k] /= sp * bw; hd[k] /= sd * bw; }
    return { plain: hp, dbrc: hd, accept: smax > 0 ? sd / sp / smax : 1 };
  }

  // Mean E_rel of the target-velocity density  y^2 e^{-y^2} |v - V| sigma(E_rel)
  // restricted to E_rel < E_up, by 2-D quadrature (midpoint rule) over y and
  // mu: an independent integral, no sampling. sigma = null means flat.
  function quadMeanErel(E, A, kT, sigma, ny, nmu) {
    var w = window_(E, A, kT), yn = w.yn, num = 0, den = 0, ymax = 7;
    for (var i = 0; i < ny; i++) {
      var y = (i + 0.5) * ymax / ny, wy = y * y * Math.exp(-y * y);
      for (var j = 0; j < nmu; j++) {
        var mu = -1 + (j + 0.5) * 2 / nmu, e2 = yn * yn + y * y - 2 * yn * y * mu;
        var er = Math.max(0, e2) * kT / A;
        if (er >= w.up) continue;
        var wt = wy * Math.sqrt(Math.max(0, e2)) * (sigma ? sigma(er) : 1);
        num += wt * er; den += wt;
      }
    }
    return num / den;
  }

  // Average over mu of |v - V| (in units of the thermal speed), closed form:
  // (1/2) int_{-1}^{1} sqrt(yn^2 + y^2 - 2 yn y mu) dmu.
  function relSpeedAvg(y, yn) {
    if (y < 1e-12 || yn < 1e-12) return Math.max(y, yn);
    return (Math.pow(yn + y, 3) - Math.pow(Math.abs(yn - y), 3)) / (6 * yn * y);
  }

  L.dbrcMath = {
    KB: KB, rng: rng, sampleCxs: sampleCxs, table: table, window: window_, encounter: encounter,
    encounterDensity: encounterDensity, quadMeanErel: quadMeanErel, relSpeedAvg: relSpeedAvg
  };

  // ─── Drawing helpers ───────────────────────────────────────────────────
  function niceTicks(lo, hi, n) {
    var raw = (hi - lo) / n, mag = Math.pow(10, Math.floor(Math.log10(raw))), f = raw / mag;
    var step = (f < 1.5 ? 1 : f < 3.5 ? 2 : f < 7.5 ? 5 : 10) * mag, out = [];
    for (var v = Math.ceil(lo / step) * step; v <= hi + step * 1e-9; v += step) out.push(+v.toPrecision(12));
    return out;
  }
  function tickLabel(v, step) {
    var d = Math.max(0, -Math.floor(Math.log10(step)) + (step < 1 ? 0 : 0));
    return v.toFixed(Math.min(d, 6));
  }
  // Linear x axis along the bottom of a plot box.
  function xAxis(ctx, x0, x1, y, lo, hi, label, col) {
    var ticks = niceTicks(lo, hi, Math.max(2, Math.floor((x1 - x0) / 70))), step = ticks.length > 1 ? ticks[1] - ticks[0] : 1;
    ctx.strokeStyle = col; ctx.fillStyle = col; ctx.lineWidth = 1; ctx.font = "12px system-ui, sans-serif";
    ctx.beginPath(); ctx.moveTo(x0, y); ctx.lineTo(x1, y); ctx.stroke();
    ticks.forEach(function (v) {
      var x = x0 + (v - lo) / (hi - lo) * (x1 - x0);
      ctx.beginPath(); ctx.moveTo(x, y); ctx.lineTo(x, y + 4); ctx.stroke();
      var t = tickLabel(v, step);
      ctx.fillText(t, x - ctx.measureText(t).width / 2, y + 16);
    });
    ctx.fillText(label, (x0 + x1) / 2 - ctx.measureText(label).width / 2, y + 30);
  }
  function yAxis(ctx, x, y0, y1, lo, hi, col) {
    var ticks = niceTicks(lo, hi, 4), step = ticks.length > 1 ? ticks[1] - ticks[0] : 1;
    ctx.strokeStyle = col; ctx.fillStyle = col; ctx.lineWidth = 1; ctx.font = "12px system-ui, sans-serif";
    ctx.beginPath(); ctx.moveTo(x, y0); ctx.lineTo(x, y1); ctx.stroke();
    ticks.forEach(function (v) {
      var y = y1 - (v - lo) / (hi - lo) * (y1 - y0);
      ctx.beginPath(); ctx.moveTo(x - 4, y); ctx.lineTo(x, y); ctx.stroke();
      var t = tickLabel(v, step);
      ctx.fillText(t, x - 8 - ctx.measureText(t).width, y + 4);
    });
  }
  function interp(e, s, x) {
    var n = e.length;
    if (x <= e[0]) return s[0];
    if (x >= e[n - 1]) return s[n - 1];
    var lo = 0, hi = n - 1;
    while (hi - lo > 1) { var m = (lo + hi) >> 1; if (e[m] <= x) lo = m; else hi = m; }
    return s[lo] + (s[hi] - s[lo]) * (x - e[lo]) / (e[hi] - e[lo]);
  }
  var TARGETS = { h1: ["H-1", 0.9991673], c12: ["C-12", 11.89365], u238: ["U-238", 236.0058] }; // ENDF/B-VIII.0 AWR, as page 3

  // ─── Widget 1: the targets that actually get hit ───────────────────────
  // The gas before the collision is Maxwellian in y (density y^2 e^{-y^2});
  // the targets a neutron meets are weighted by the relative speed too.
  function svt(root) {
    var st = { nuc: "u238", T: 293.6, e: 6.5, N: 40000 };
    var NB = 48, YMAX = 5;
    function compute() {
      var A = TARGETS[st.nuc][1], kT = KB * st.T, w = window_(st.e, A, kT), yn = w.yn;
      var alpha = 1 / (1 + Math.sqrt(Math.PI) * yn / 2), r = rng(2026), hit = new Float64Array(NB), gas = new Float64Array(NB), toward = 0, sumRel = 0;
      for (var i = 0; i < st.N; i++) {
        var c = sampleCxs(yn, alpha, r), y = Math.sqrt(c[0]), b = Math.floor(y / YMAX * NB);
        if (b < NB) hit[b]++;
        if (c[1] < 0) toward++;
      }
      return { A: A, yn: yn, hit: hit, toward: toward / st.N, kT: kT };
    }
    var cache = null;
    function get() { if (!cache) cache = compute(); return cache; }
    function dirty() { cache = null; }
    return new Frame(root, {
      reset: function () { },
      onSeen: function () { },
      tick: function () { return false; },
      draw: function (ctx, w, h, zoom) {
        var D = get(), fg = css("--fg", "#1d2125"), acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b");
        var L0 = 46, R0 = w - 12, T0 = 58, B0 = h - 44, hi = YMAX / zoom;
        var yn = D.yn;
        // Analytic curves: Maxwellian gas y^2 e^{-y^2}, and the hit density y^2 e^{-y^2} <|v-V|>.
        var nPts = 200, gasC = [], hitC = [], gz = 0, hz = 0, dy = YMAX / nPts;
        for (var i = 0; i < nPts; i++) {
          var y = (i + 0.5) * dy, g = y * y * Math.exp(-y * y), hh = g * relSpeedAvg(y, yn);
          gasC.push(g); hitC.push(hh); gz += g * dy; hz += hh * dy;
        }
        var top = 0;
        for (i = 0; i < nPts; i++) top = Math.max(top, gasC[i] / gz, hitC[i] / hz);
        top *= 1.1;
        var bw = YMAX / NB, hitTot = 0;
        for (i = 0; i < NB; i++) hitTot += D.hit[i];
        function X(y) { return L0 + y / hi * (R0 - L0); }
        function Y(v) { return B0 - v / top * (B0 - T0); }
        // Sampled bars (the targets that were hit).
        ctx.fillStyle = "rgba(31, 111, 120, 0.40)";
        for (i = 0; i < NB; i++) {
          var dens = D.hit[i] / hitTot / bw;
          ctx.fillRect(X(i * bw) + 0.5, Y(dens), Math.max(1, X((i + 1) * bw) - X(i * bw) - 1), B0 - Y(dens));
        }
        // Analytic curves.
        function curve(arr, z, col, dash) {
          ctx.strokeStyle = col; ctx.lineWidth = 2; ctx.setLineDash(dash); ctx.beginPath();
          for (var k = 0; k < nPts; k++) { var x = X((k + 0.5) * dy), yy = Y(arr[k] / z); if (k) ctx.lineTo(x, yy); else ctx.moveTo(x, yy); }
          ctx.stroke(); ctx.setLineDash([]);
        }
        curve(gasC, gz, mut, [6, 4]);
        curve(hitC, hz, acc, []);
        // The neutron's own speed in the same units.
        var xn = X(yn);
        if (xn < R0) { ctx.strokeStyle = fg; ctx.lineWidth = 1.5; ctx.beginPath(); ctx.moveTo(xn, T0); ctx.lineTo(xn, B0); ctx.stroke(); ctx.fillStyle = fg; ctx.font = "12px system-ui, sans-serif"; ctx.fillText("neutron", Math.min(xn + 4, R0 - 50), T0 + 12); }
        yAxis(ctx, L0, T0, B0, 0, top, mut);
        xAxis(ctx, L0, R0, B0, 0, hi, "target speed, in units of the target's thermal speed", mut);
        ctx.font = "13px system-ui, sans-serif"; ctx.fillStyle = mut; ctx.fillText("dashed: the gas as it sits", L0 + 8, T0 + 14);
        ctx.fillStyle = acc; ctx.fillText("solid and bars: the nuclei the neutron hits", L0 + 8, T0 + 30);
        return TARGETS[st.nuc][0] + " at " + st.T + " K, neutron " + fmtE(st.e) + " (speed " + yn.toFixed(1) + " thermal speeds of the target). Targets moving toward the neutron: " +
          (100 * D.toward).toFixed(1) + " % of the nuclei it hits, against 50 % of the gas.";
      },
      panel: function (body, redraw) {
        buttons(body, "Target nucleus", [["h1", "H-1"], ["c12", "C-12"], ["u238", "U-238"]], st.nuc, function (v) { st.nuc = v; dirty(); redraw(); });
        buttons(body, "Neutron energy", [[0.0253, "0.0253 eV"], [1, "1 eV"], [6.5, "6.5 eV"], [100, "100 eV"]], st.e, function (v) { st.e = v; dirty(); redraw(); });
        slider(body, "Temperature", 20, 1200, 10, st.T, function (v) { st.T = v; dirty(); redraw(); return v + " K"; });
        body.append(el("p", { "class": "w-note" }, "Sampled with the transport code's own two-branch method (40 000 draws, fixed seed). The curves are exact integrals. The x axis is linear; zoom in to see the low-speed end."));
      }
    });
  }

  // ─── Widget 2: DBRC next to U-238's 6.67 eV resonance ──────────────────
  function dbrc(root) {
    var D = window.DBRC_DATA.u238, A = D.awr;
    var t0 = table(D.el0k.e, D.el0k.s);
    var st = { e: 6.5, T: 293.6, bt: true };
    var NB = 70, cache = null;
    function dirty() { cache = null; }
    function get(lo, hi) {
      if (!cache || cache.lo !== lo || cache.hi !== hi) {
        var kT = KB * st.T, w = window_(st.e, A, kT), d = encounterDensity(st.e, A, kT, t0, lo, hi, NB, 500, 1000);
        var bw = (hi - lo) / NB, mp = 0, md = 0, zp = 0, zd = 0;
        for (var k = 0; k < NB; k++) { var e = lo + (k + 0.5) * bw; mp += e * d.plain[k]; zp += d.plain[k]; md += e * d.dbrc[k]; zd += d.dbrc[k]; }
        cache = { lo: lo, hi: hi, w: w, d: d, meanP: mp / zp, meanD: md / zd, smax: t0.maxOver(w.lo, w.up) };
      }
      return cache;
    }
    return new Frame(root, {
      reset: function () { },
      onSeen: function () { },
      tick: function () { return false; },
      draw: function (ctx, w, h, zoom) {
        var fg = css("--fg", "#1d2125"), acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b"), hot = "#b85c38";
        var L0 = 50, R0 = w - 12, T1 = 78, B1 = Math.round(h * 0.46), T2 = B1 + 44, B2 = h - 40;
        var win = window_(st.e, A, KB * st.T);
        // x range: the DBRC window, narrowed by zoom about the neutron energy.
        var half = Math.max(win.up - st.e, st.e - win.lo) * 1.05 / zoom, lo = st.e - half, hi = st.e + half;
        var C = get(lo, hi);
        function X(e) { return L0 + (e - lo) / (hi - lo) * (R0 - L0); }
        // Panel 1: sigma_el, 0 K and broadened.
        var smaxV = 0, i;
        for (i = 0; i < D.el0k.e.length; i++) if (D.el0k.e[i] >= lo && D.el0k.e[i] <= hi) smaxV = Math.max(smaxV, D.el0k.s[i]);
        smaxV = Math.max(smaxV, interp(D.el0k.e, D.el0k.s, lo), interp(D.el0k.e, D.el0k.s, hi)) * 1.08;
        function Y1(s) { return B1 - s / smaxV * (B1 - T1); }
        ctx.fillStyle = "rgba(31, 111, 120, 0.10)";
        ctx.fillRect(X(Math.max(C.w.lo, lo)), T1, X(Math.min(C.w.up, hi)) - X(Math.max(C.w.lo, lo)), B1 - T1);
        function line(dat, col, dash) {
          ctx.strokeStyle = col; ctx.lineWidth = 2; ctx.setLineDash(dash); ctx.beginPath();
          var first = true;
          for (var k = 0; k < dat.e.length; k++) {
            if (dat.e[k] < lo - 0.2 * half || dat.e[k] > hi + 0.2 * half) continue;
            var x = X(dat.e[k]), y = Y1(dat.s[k]);
            if (first) { ctx.moveTo(x, y); first = false; } else ctx.lineTo(x, y);
          }
          ctx.stroke(); ctx.setLineDash([]);
        }
        ctx.save(); ctx.beginPath(); ctx.rect(L0, T1, R0 - L0, B1 - T1); ctx.clip();
        if (st.bt) line(D.elT, mut, [5, 4]);
        line(D.el0k, hot, []);
        ctx.strokeStyle = hot; ctx.lineWidth = 1; ctx.setLineDash([2, 3]); ctx.beginPath(); ctx.moveTo(L0, Y1(C.smax)); ctx.lineTo(R0, Y1(C.smax)); ctx.stroke(); ctx.setLineDash([]);
        ctx.restore();
        ctx.strokeStyle = fg; ctx.lineWidth = 1.5; ctx.beginPath(); ctx.moveTo(X(st.e), T1); ctx.lineTo(X(st.e), B1); ctx.stroke();
        yAxis(ctx, L0, T1, B1, 0, smaxV, mut);
        ctx.font = "12px system-ui, sans-serif";
        ctx.fillStyle = fg; ctx.fillText("elastic σ_s (barn); dotted: σ_max", L0, T1 - 6);
        ctx.fillStyle = hot; ctx.fillText("solid: 0 K (what DBRC reads)", L0 + 6, T1 + 14);
        if (st.bt) { ctx.fillStyle = mut; ctx.fillText("dashed: 293.6 K broadened (the rate)", L0 + 6, T1 + 28); }
        xAxis(ctx, L0, R0, B1, lo, hi, "relative energy E_rel (eV)", mut);
        // Panel 2: the two encounter-energy densities.
        var top = 0; for (i = 0; i < NB; i++) top = Math.max(top, C.d.plain[i], C.d.dbrc[i]); top *= 1.1;
        var bw = (R0 - L0) / NB;
        ctx.fillStyle = "rgba(91, 99, 107, 0.45)";
        for (i = 0; i < NB; i++) { var hpx = C.d.plain[i] / top * (B2 - T2); ctx.fillRect(L0 + i * bw, B2 - hpx, bw + 0.5, hpx); }
        ctx.strokeStyle = hot; ctx.lineWidth = 2; ctx.fillStyle = "rgba(184, 92, 56, 0.35)"; ctx.beginPath(); ctx.moveTo(L0, B2);
        for (i = 0; i < NB; i++) { var hdx = B2 - C.d.dbrc[i] / top * (B2 - T2); ctx.lineTo(L0 + i * bw, hdx); ctx.lineTo(L0 + (i + 1) * bw, hdx); }
        ctx.lineTo(R0, B2); ctx.fill(); ctx.stroke();
        ctx.strokeStyle = fg; ctx.lineWidth = 1.5; ctx.beginPath(); ctx.moveTo(X(st.e), T2); ctx.lineTo(X(st.e), B2); ctx.stroke();
        [[C.meanP, mut], [C.meanD, hot]].forEach(function (m) {
          ctx.strokeStyle = m[1]; ctx.lineWidth = 2; ctx.setLineDash([4, 3]); ctx.beginPath(); ctx.moveTo(X(m[0]), T2); ctx.lineTo(X(m[0]), B2); ctx.stroke(); ctx.setLineDash([]);
        });
        ctx.font = "12px system-ui, sans-serif";
        ctx.fillStyle = mut; ctx.fillText("grey: targets by plain sampling", L0 + 6, T2 + 12);
        ctx.fillStyle = hot; ctx.fillText("orange: with the DBRC rejection", L0 + 6, T2 + 26);
        xAxis(ctx, L0, R0, B2, lo, hi, "encounter energy E_rel (eV)", mut);
        return "U-238 at " + st.T + " K, neutron " + st.e.toFixed(3) + " eV. Mean encounter energy: plain " + C.meanP.toFixed(4) + " eV, with DBRC " + C.meanD.toFixed(4) +
          " eV. DBRC accepts " + (C.d.accept < 0.01 ? (1e4 * C.d.accept).toFixed(1) + " in 10 000" : (100 * C.d.accept).toFixed(1) + " %") + " of the targets it draws (σ̄/σ_max).";
      },
      panel: function (body, redraw) {
        slider(body, "Neutron energy", 6.2, 7.1, 0.005, st.e, function (v) { st.e = v; dirty(); redraw(); return v.toFixed(3) + " eV"; });
        slider(body, "Temperature", 100, 1200, 10, st.T, function (v) { st.T = v; dirty(); redraw(); return v + " K"; });
        toggle(body, "Show the broadened σ_s too", st.bt, function (v) { st.bt = v; redraw(); });
        body.append(el("p", { "class": "w-note" }, "The two densities are numerical integrals of what the two samplers draw from (the samplers themselves reproduce them: see the checks). The shaded band is the range of relative energies a target can reach, four thermal speeds either side. Zoom changes the energy range of both plots. Linear axes."));
      }
    });
  }

  // ─── Widget 3: which file supplies the cross section? ──────────────────
  // The two curves are our port's: the continuous-energy table's elastic
  // column (free gas) and the thermal table's sigma_inel + sigma_el (bound),
  // as decoded from the ACE files by examples/lesson_dbrc.rs.
  function sigmaCoh(edges, cum, e) {
    if (e <= 0) return 0;
    var lo = 0, hi = edges.length; // first index with edge > e
    while (lo < hi) { var m = (lo + hi) >> 1; if (edges[m] <= e) lo = m + 1; else hi = m; }
    return lo === 0 ? 0 : cum[lo - 1] / e;
  }
  // sigma_bound(E) per principal atom exactly as outram-mc-libs reads the
  // table: lin-lin sigma_inel plus coherent S/E, and zero at and above the cutoff.
  function boundXs(T, e) {
    if (e >= T.cutoff) return 0;
    var s = interp(T.einel, T.sinel, e);
    if (T.elastic === "coherent") s += sigmaCoh(T.edges, T.cum, e);
    return s;
  }
  L.dbrcMath.boundXs = boundXs;
  L.dbrcMath.sigmaCoh = sigmaCoh;

  function handoff(root) {
    var DD = window.DBRC_DATA;
    var PAIR = { h2o: ["H in H2O", "H-1", "hydrogen in water"], gr: ["graphite", "C-12", "carbon in graphite"] };
    var st = { mat: "h2o", win: "th", probe: 0.0253 };
    var WINS = { th: [0.001, 0.5], cut: null, all: [0.001, 20] };
    function range() {
      var T = DD.th[PAIR[st.mat][0]];
      if (st.win === "cut") return [Math.max(0, T.cutoff - 4), T.cutoff + 4];
      return WINS[st.win];
    }
    return new Frame(root, {
      reset: function () { },
      onSeen: function () { },
      tick: function () { return false; },
      draw: function (ctx, w, h, zoom) {
        var T = DD.th[PAIR[st.mat][0]], C = DD.ce[PAIR[st.mat][1]];
        var fg = css("--fg", "#1d2125"), acc = css("--accent", "#1f6f78"), mut = css("--muted", "#5b636b"), hot = "#b85c38";
        var r = range(), mid = (r[0] + r[1]) / 2, half = (r[1] - r[0]) / 2 / zoom, lo = Math.max(0.001, mid - half), hi = mid + half;
        var L0 = 50, R0 = w - 12, T0 = 96, B0 = h - 50;
        function X(e) { return L0 + (e - lo) / (hi - lo) * (R0 - L0); }
        var N = 600, free = [], bound = [], top = 0, k;
        for (k = 0; k <= N; k++) {
          var e = lo + (hi - lo) * k / N; if (e <= 0) e = 1e-6;
          var f = interp(C.elastic.e, C.elastic.s, e), b = boundXs(T, e);
          free.push(f); bound.push(b); top = Math.max(top, f, b);
        }
        top *= 1.08;
        function Y(s) { return B0 - s / top * (B0 - T0); }
        // Which file supplies it: a band under the plot.
        var xc = X(Math.min(Math.max(T.cutoff, lo), hi));
        ctx.fillStyle = "rgba(31, 111, 120, 0.12)"; ctx.fillRect(L0, T0, Math.max(0, xc - L0), B0 - T0);
        ctx.fillStyle = "rgba(184, 92, 56, 0.10)"; ctx.fillRect(xc, T0, Math.max(0, R0 - xc), B0 - T0);
        // Free-gas elastic from the CE table, drawn across the whole window (dashed where it is not used).
        ctx.lineWidth = 2;
        ctx.strokeStyle = hot; ctx.setLineDash([5, 4]); ctx.beginPath();
        for (k = 0; k <= N; k++) { var xx = L0 + k / N * (R0 - L0), yy = Y(free[k]); if (k) ctx.lineTo(xx, yy); else ctx.moveTo(xx, yy); }
        ctx.stroke(); ctx.setLineDash([]);
        // Bound curve, solid, below the cutoff only.
        ctx.strokeStyle = acc; ctx.beginPath(); var first = true;
        for (k = 0; k <= N; k++) {
          var ee = lo + (hi - lo) * k / N; if (ee >= T.cutoff) break;
          var x2 = L0 + k / N * (R0 - L0), y2 = Y(bound[k]);
          if (first) { ctx.moveTo(x2, y2); first = false; } else ctx.lineTo(x2, y2);
        }
        ctx.stroke();
        // The cutoff.
        if (T.cutoff >= lo && T.cutoff <= hi) {
          ctx.strokeStyle = fg; ctx.lineWidth = 1.5; ctx.beginPath(); ctx.moveTo(xc, T0); ctx.lineTo(xc, B0); ctx.stroke();
          ctx.fillStyle = fg; ctx.font = "12px system-ui, sans-serif"; ctx.fillText("cutoff " + T.cutoff.toFixed(4) + " eV", Math.min(xc + 4, R0 - 120), T0 + 12);
        }
        // The probe energy.
        var pe = Math.min(Math.max(st.probe, lo), hi), px = X(pe);
        ctx.strokeStyle = mut; ctx.lineWidth = 1; ctx.setLineDash([2, 3]); ctx.beginPath(); ctx.moveTo(px, T0); ctx.lineTo(px, B0); ctx.stroke(); ctx.setLineDash([]);
        yAxis(ctx, L0, T0, B0, 0, top, mut);
        xAxis(ctx, L0, R0, B0, lo, hi, "neutron energy (eV)", mut);
        ctx.font = "12px system-ui, sans-serif";
        ctx.fillStyle = acc; ctx.fillText("solid: thermal file, below its cutoff", L0 + 6, T0 - 38);
        ctx.fillStyle = hot; ctx.fillText("dashed: continuous-energy file, free gas", L0 + 6, T0 - 22);
        ctx.fillStyle = mut; ctx.fillText("barn per atom", L0 + 6, T0 - 6);
        var below = pe < T.cutoff, b = boundXs(T, pe), f = interp(C.elastic.e, C.elastic.s, pe);
        return PAIR[st.mat][2] + " at 293.6 K, neutron " + pe.toPrecision(4) + " eV: " + (below
          ? "scattering cross section " + b.toPrecision(4) + " b from the thermal file (free gas would say " + f.toPrecision(4) + " b); the outgoing energy and angle are sampled from S(α,β), not from a moving free gas."
          : "scattering cross section " + f.toPrecision(4) + " b from the continuous-energy file; the target is a free gas" + (st.mat === "gr" ? "." : ".") + " Capture is the continuous-energy file's at every energy.");
      },
      panel: function (body, redraw) {
        buttons(body, "Material", [["h2o", "H in water"], ["gr", "C in graphite"]], st.mat, function (v) { st.mat = v; redraw(); });
        buttons(body, "Energy window", [["th", "0 to 0.5 eV"], ["cut", "around the cutoff"], ["all", "0 to 20 eV"]], st.win, function (v) { st.win = v; redraw(); });
        slider(body, "Probe energy", 0.001, 20, 0.001, st.probe, function (v) { st.probe = v; redraw(); return v.toFixed(3) + " eV"; });
        body.append(el("p", { "class": "w-note" }, "Both curves are our port's output: the continuous-energy table's elastic column and the thermal table's cross section, decoded from the ACE files it wrote. Linear axes; use + and − to zoom about the middle of the window."));
      }
    });
  }

  L.widget("svt", svt);
  L.widget("dbrc", dbrc);
  L.widget("handoff", handoff);
})();
