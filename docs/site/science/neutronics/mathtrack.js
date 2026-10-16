// Neutronics lessons, the maths track from the elastic collision to LEAPR
// (gh:#906 designs and amendments, 2026-10-16): rungs 1 to 5h. Conventions,
// stated on every page that uses them:
//   beta = (E' - E)/kT: positive when the neutron GAINS energy;
//   alpha = (E' + E - 2 mu sqrt(E E'))/(A kT): the size of the kick;
//   S here is LEAPR's asymmetric scattering law (the NJOY2016 manual's script
//   S), for which detailed balance reads S(alpha, beta) = e^-beta S(alpha, -beta).
// Free gas (manual eq. 538-539): S(alpha, beta) = exp(-(alpha + beta)^2 / 4 alpha)
//   / sqrt(4 pi alpha), for either sign of beta.
// One oscillator of weight 1 and step beta_i = hbar omega/kT (eq. 542-543):
//   S = e^-(alpha lambda) sum_n I_n(alpha / (beta_i sinh(beta_i/2))) e^(-n beta_i/2)
//   delta(beta - n beta_i), lambda = coth(beta_i/2)/beta_i; n < 0 creates
//   phonons (down-scatter), n > 0 absorbs them (up-scatter).
// Numbers from OUTRAM PARK's LEAPR port are in leapr-data.js
// (examples/lesson_leapr.rs); check-physics.js compares the JS with them.
(function () {
  "use strict";
  var L = window.Lesson;
  var css = L.css, el = L.el, fmtE = L.fmtE, Frame = L.Frame;

  // ─── Maths ──────────────────────────────────────────────────────────
  function freeGas(alpha, beta) { return Math.exp(-(alpha + beta) * (alpha + beta) / (4 * alpha)) / Math.sqrt(4 * Math.PI * alpha); }
  // ln I_n(y) by its power series, summed in logs (n >= 0, y > 0).
  function lnI(n, y) {
    var lh = Math.log(y / 2), terms = [], lf = 0, lfn = 0;
    for (var j = 1; j <= n; j++) lfn += Math.log(j);
    for (var k = 0; k < 400; k++) {
      if (k > 0) { lf += Math.log(k); lfn += Math.log(k + n); }
      var t = (2 * k + n) * lh - lf - lfn;
      terms.push(t);
      if (k > 10 && t < terms[0] - 60 && t < Math.max.apply(null, terms) - 60) break;
    }
    var m = Math.max.apply(null, terms), s = 0;
    terms.forEach(function (t) { s += Math.exp(t - m); });
    return m + Math.log(s);
  }
  // Oscillator weights A_n, n = -N..N (index n + N).
  function oscWeights(alpha, bi, N) {
    var lam = 1 / Math.tanh(bi / 2) / bi, y = alpha / (bi * Math.sinh(bi / 2)), out = [];
    for (var n = -N; n <= N; n++) out.push(Math.exp(-alpha * lam + lnI(Math.abs(n), y) - n * bi / 2));
    return { lambda: lam, w: out, N: N };
  }
  // Effective temperature ratio from a spectrum rho on 0, delta, 2 delta, ... [eV]:
  // each mode's (beta/2) coth(beta/2), weighted by rho.
  function tbarFromRho(rho, delta, kT) {
    var num = 0, den = 0;
    for (var i = 1; i < rho.length; i++) { var b = i * delta / kT; num += rho[i] * (b / 2) / Math.tanh(b / 2); den += rho[i]; }
    return num / den;
  }
  function rng(seed) { return function () { seed |= 0; seed = seed + 0x6D2B79F5 | 0; var t = Math.imul(seed ^ seed >>> 15, 1 | seed); t = t + Math.imul(t ^ t >>> 7, 61 | t) ^ t; return ((t ^ t >>> 14) >>> 0) / 4294967296; }; }
  function gauss(r) { var u = Math.max(r(), 1e-12), v = r(); return Math.sqrt(-2 * Math.log(u)) * Math.cos(2 * Math.PI * v); }
  function poisson(r, m) { var Lm = Math.exp(-m), k = 0, p = 1; if (m > 30) return Math.max(0, Math.round(m + Math.sqrt(m) * gauss(r))); do { k++; p *= r(); } while (p > Lm); return k - 1; }
  // Convolution of two discrete distributions on 0..n-1 and 0..m-1.
  function convolve(a, b) { var out = new Array(a.length + b.length - 1).fill(0); for (var i = 0; i < a.length; i++) for (var j = 0; j < b.length; j++) out[i + j] += a[i] * b[j]; return out; }
  L.mathTrack = { freeGas: freeGas, lnI: lnI, oscWeights: oscWeights, tbarFromRho: tbarFromRho, convolve: convolve, rng: rng, gauss: gauss, poisson: poisson };

  function palette() { return { fg: css("--fg", "#1d2125"), acc: css("--accent", "#1f6f78"), mut: css("--muted", "#5b636b"), hot: "#b85c38", blue: "#3a6ea5" }; }
  function bindRange(card, attr, set, fmt, after) {
    var i = card.querySelector("[" + attr + "]"), o = card.querySelector("[" + attr + "-out]");
    if (!i) return;
    var go = function () { var v = parseFloat(i.value); set(v); if (o) o.textContent = fmt(v); after(); };
    i.oninput = go; go();
  }
  function pick(card, attr, set, after) {
    card.querySelectorAll("[" + attr + "]").forEach(function (b) {
      b.onclick = function () { set(b.getAttribute(attr)); card.querySelectorAll("[" + attr + "]").forEach(function (o) { o.classList.toggle("ghost", o !== b); }); after(); };
    });
  }

  // ─── Rungs 1, 1½, 2: the (alpha, beta) map ───────────────────────────
  // data-abstage on the widget picks the stage: "line" (rung 1), "jiggle"
  // (rung 1½), "freegas" (rung 2). alpha and beta in units of kT at 293.6 K so
  // the axes stay put while the temperature slider moves.
  function abWidget(root) {
    var st = { stage: root.getAttribute("data-abstage") || "line", T: 293.6, dots: [], r: rng(17), t: 0 };
    var AMAX = 6;
    function addDots(n) {
      for (var i = 0; i < n; i++) {
        var a = 0.15 + st.r() * (AMAX - 0.15);
        st.dots.push([a, -a + Math.sqrt(2 * a * st.T / 293.6) * gauss(st.r)]);
      }
      if (st.dots.length > 4000) st.dots = st.dots.slice(-4000);
    }
    var frame = new Frame(root, {
      reset: function () { st.dots = []; },
      tick: function (dt) { if (st.stage !== "jiggle") return false; st.t += dt; addDots(12); return st.dots.length < 4000; },
      draw: function (ctx, w, h, zoom) {
        var c = palette(), x0 = 46, y0 = 84, pw = w - x0 - 12, ph = h - y0 - 46;
        var bmin = -AMAX - 4, bmax = 3;
        var X = function (a) { return x0 + a / AMAX * pw; }, Y = function (b) { return y0 + (bmax - b) / (bmax - bmin) * ph; };
        ctx.font = "12px system-ui, sans-serif"; ctx.strokeStyle = c.mut; ctx.lineWidth = 1; ctx.strokeRect(x0, y0, pw, ph);
        ctx.beginPath(); ctx.moveTo(x0, Y(0)); ctx.lineTo(x0 + pw, Y(0)); ctx.stroke();
        ctx.save(); ctx.beginPath(); ctx.rect(x0, y0, pw, ph); ctx.clip();
        if (st.stage === "freegas") {
          // The exact band: shade each column by S(alpha, beta) at this temperature.
          var tr = st.T / 293.6;
          for (var i = 0; i < 90; i++) for (var j = 0; j < 90; j++) {
            var a = (i + 0.5) / 90 * AMAX, b = bmax - (j + 0.5) / 90 * (bmax - bmin);
            var s = tr > 0 ? freeGas(a / tr, b / tr) / tr : 0, v = Math.min(1, s * 2.4);
            if (v > 0.01) { ctx.fillStyle = "rgba(31,111,120," + v.toFixed(3) + ")"; ctx.fillRect(X(a - AMAX / 180), Y(b + (bmax - bmin) / 180), pw / 90 + 1, ph / 90 + 1); }
          }
        }
        if (st.stage === "jiggle") { ctx.fillStyle = c.acc; st.dots.forEach(function (d) { ctx.fillRect(X(d[0]) - 1, Y(d[1]) - 1, 2.4, 2.4); }); }
        ctx.strokeStyle = c.hot; ctx.lineWidth = 2; ctx.beginPath(); ctx.moveTo(X(0), Y(0)); ctx.lineTo(X(AMAX), Y(-AMAX)); ctx.stroke();
        ctx.restore();
        ctx.fillStyle = c.mut; ctx.fillText("α (kick) →", x0 + pw - 70, y0 + ph + 16); ctx.fillText("0", x0, y0 + ph + 16);
        ctx.fillText("β", 8, y0 + 12); ctx.fillText("gain +", 4, Y(1.5)); ctx.fillText("loss −", 4, Y(-4));
        ctx.fillStyle = c.fg; ctx.font = "13px system-ui, sans-serif";
        ctx.fillText(st.stage === "line" ? "the recoil line: β = −α" : st.stage === "jiggle" ? "each dot: one collision with a moving atom" : "free gas: S(α, β), darker = more likely", x0, y0 - 10);
        if (st.stage === "line") return "A nucleus at rest: for every kick α the neutron loses exactly the recoil energy, so β = −α. Every collision lands on this one line.";
        if (st.stage === "jiggle") return st.dots.length + " collisions with atoms jiggling at " + st.T.toFixed(0) + " K. Same kick, slightly different energy change each time: the dots scatter around the recoil line, wider when hotter. (α and β in units of kT at 293.6 K.)";
        return "The exact free-gas band at " + st.T.toFixed(0) + " K: a bell curve in β around β = −α, getting wider with the kick and with the temperature. At 0 K it shrinks back to the line.";
      },
      panel: function (body) {
        body.append(el("p", { "class": "w-note" }, "α and β are measured in units of kT at 293.6 K here, so the axes stay fixed while the temperature changes. β > 0 means the neutron gained energy."));
      }
    });
    var card = root.parentNode;
    bindRange(card, "data-abt", function (v) { st.T = v; st.dots = []; }, function (v) { return v.toFixed(0) + " K"; }, function () { frame.play(); frame.redraw(); });
    return frame;
  }

  // ─── Rung 3: one swing (one oscillator) ──────────────────────────────
  function swingWidget(root) {
    var st = { alpha: 1, hw: 0.1, T: 293.6, E: 0.0253, warm: true, h: new Array(13).fill(0), n: 0, r: rng(3), pending: 0, last: null };
    var N = 6;
    // The kick is a free-recoil energy E_R = alpha kT0 (kT0 at 293.6 K); the
    // oscillator formula wants it in units of the swing's own kT: a warm swing
    // at 293.6 K, a cold one at 1 K.
    function weights() {
      var kT0 = 8.617333262e-5 * 293.6, kT = 8.617333262e-5 * (st.warm ? 293.6 : 1), bi = st.hw / kT, W = oscWeights(st.alpha * kT0 / kT, bi, N);
      // Case B: creating n phonons needs n hbar omega <= E.
      var allowed = W.w.map(function (v, k) { var n = k - N; return n < 0 && -n * st.hw > st.E ? 0 : v; });
      return { bi: bi, raw: W.w, w: allowed, lambda: W.lambda };
    }
    var frame = new Frame(root, {
      reset: function () { st.h = new Array(2 * N + 1).fill(0); st.n = 0; st.last = null; },
      tick: function () {
        if (!st.pending) return false;
        var W = weights(), tot = W.w.reduce(function (a, b) { return a + b; }, 0), k = Math.min(st.pending, Math.max(1, Math.ceil(st.pending / 15)));
        for (var i = 0; i < k; i++) { var u = st.r() * tot, j = 0; while (j < W.w.length - 1 && (u -= W.w[j]) > 0) j++; st.h[j]++; st.n++; st.last = j - N; }
        st.pending -= k;
        return st.pending > 0;
      },
      draw: function (ctx, w, h, zoom) {
        var c = palette(), W = weights(), x0 = 40, y0 = 84, pw = w - x0 - 12, ph = h - y0 - 56, bw = pw / (2 * N + 1);
        var tot = W.w.reduce(function (a, b) { return a + b; }, 0) || 1;
        ctx.font = "12px system-ui, sans-serif"; ctx.strokeStyle = c.mut; ctx.lineWidth = 1; ctx.strokeRect(x0, y0, pw, ph);
        for (var k = 0; k <= 2 * N; k++) {
          var n = k - N, f = W.w[k] / tot, xx = x0 + k * bw;
          var forbidden = W.raw[k] > 1e-9 && W.w[k] === 0;
          ctx.fillStyle = forbidden ? "rgba(128,128,128,0.25)" : n === 0 ? c.blue : n < 0 ? c.hot : c.acc;
          var hh = (forbidden ? W.raw[k] / tot : f) * ph * 0.95 * zoom;
          ctx.fillRect(xx + 3, y0 + ph - Math.min(hh, ph), bw - 6, Math.min(hh, ph));
          if (st.n) { var sf = st.h[k] / st.n; ctx.strokeStyle = c.fg; ctx.lineWidth = 2; ctx.beginPath(); ctx.moveTo(xx + 3, y0 + ph - Math.min(sf * ph * 0.95 * zoom, ph)); ctx.lineTo(xx + bw - 3, y0 + ph - Math.min(sf * ph * 0.95 * zoom, ph)); ctx.stroke(); }
          ctx.fillStyle = c.mut; var lab = n > 0 ? "+" + n : String(n); ctx.fillText(lab, xx + bw / 2 - ctx.measureText(lab).width / 2, y0 + ph + 14);
        }
        ctx.fillStyle = c.mut; ctx.fillText("phonons: made (−)   none   absorbed (+)", x0, y0 + ph + 30);
        ctx.fillStyle = c.fg; ctx.font = "13px system-ui, sans-serif"; ctx.fillText("the phonon expansion for one swing", x0, y0 - 10);
        var mean = 0; W.w.forEach(function (v, k) { mean += (k - N) * v / tot; });
        var caseB = st.hw > st.E;
        var s = "Kick: the free-recoil energy is " + (st.alpha * 8.617333262e-5 * 293.6 * 1e3).toFixed(1) + " meV, " + (st.alpha * 8.617333262e-5 * 293.6 / st.hw).toFixed(2) + " steps of ħω = " + (st.hw * 1e3).toFixed(0) + " meV. Chance of no phonon (elastic): " + (W.w[N] / tot * 100).toFixed(1) + " %. Average net phonons made (made minus absorbed): " + (-mean).toFixed(2).replace("-0.00", "0.00") + ".";
        if (caseB) s += " The neutron has only " + (st.E * 1e3).toFixed(1) + " meV, less than one step: it cannot make a phonon (grey). It can bounce off elastically" + (st.warm ? ", or absorb a phonon already there and leave faster: upscatter." : "; the swing is cold, so there is nothing to absorb.");
        else s += " Each outcome costs or gives whole steps: a gentle kick usually makes none, and sometimes one.";
        if (st.n) s += " Black lines: " + st.n + " shoves, sampled.";
        return s;
      },
      panel: function (body) {
        body.append(el("p", { "class": "w-note" }, "One quantum oscillator with steps ħω, at the temperature shown; bars from the NJOY2016 manual's eq. 542 (computed here, checked against OUTRAM PARK's port of LEAPR's bfact). Kicks are given as the free-recoil energy. A toy, not a material."));
      }
    });
    var card = root.parentNode, redraw = function () { frame.redraw(); };
    bindRange(card, "data-kick", function (v) { st.alpha = v; st.h = new Array(2 * N + 1).fill(0); st.n = 0; }, function (v) { return (v * 8.617333262e-5 * 293.6 * 1e3).toFixed(1) + " meV"; }, redraw);
    bindRange(card, "data-hw", function (v) { st.hw = v / 1e3; st.h = new Array(2 * N + 1).fill(0); st.n = 0; }, function (v) { return v.toFixed(0) + " meV"; }, redraw);
    bindRange(card, "data-en", function (v) { st.E = v / 1e3; st.h = new Array(2 * N + 1).fill(0); st.n = 0; }, function (v) { return v.toFixed(1) + " meV"; }, redraw);
    pick(card, "data-warm", function (v) { st.warm = v === "1"; st.h = new Array(2 * N + 1).fill(0); st.n = 0; }, redraw);
    card.querySelectorAll("[data-shove]").forEach(function (b) { b.onclick = function () { st.pending += +b.getAttribute("data-shove"); frame.play(); }; });
    return frame;
  }

  // ─── Rung 3½: two dice, convolved ────────────────────────────────────
  function diceWidget(root) {
    var st = { a: [1, 1, 1, 1, 1, 1], b: [1, 1, 1, 1, 1, 1], slide: 7, h: new Array(11).fill(0), n: 0, r: rng(35) };
    function norm(v) { var s = v.reduce(function (x, y) { return x + y; }, 0); return v.map(function (x) { return x / s; }); }
    var frame = new Frame(root, {
      reset: function () { st.h = new Array(11).fill(0); st.n = 0; },
      tick: function () { return false; },
      draw: function (ctx, w, h, zoom) {
        var c = palette(), A = norm(st.a), B = norm(st.b), C = convolve(A, B);
        var x0 = 30, top = 84, bw = (w - 60) / 11, rowH = (h - top - 70) / 3;
        ctx.font = "12px system-ui, sans-serif";
        // Row 1: die 1 at faces 1..6.
        var bar = function (row, k, v, col, max) { var hh = v / max * (rowH - 18); ctx.fillStyle = col; ctx.fillRect(x0 + k * bw + 2, top + row * rowH + rowH - 14 - hh, bw - 4, hh); };
        var maxAB = Math.max.apply(null, A.concat(B)) * 1.1;
        for (var k = 0; k < 6; k++) bar(0, k, A[k], c.acc, maxAB);
        // Row 2: die 2 flipped and slid so that its face (t - k) sits under die 1's face k.
        var t = st.slide, sum = 0;
        for (k = 0; k < 6; k++) { var f2 = t - (k + 1); if (f2 >= 1 && f2 <= 6) { bar(1, k, B[f2 - 1], c.hot, maxAB); sum += A[k] * B[f2 - 1]; ctx.fillStyle = c.fg; ctx.fillText(String(f2), x0 + k * bw + bw / 2 - 4, top + 2 * rowH - 2); } }
        // Row 3: the convolution, totals 2..12, built up to the slider.
        var maxC = Math.max.apply(null, C) * 1.1;
        for (k = 0; k < 11; k++) {
          var total = k + 2, done = total <= t;
          bar(2, k, C[k], done ? (total === t ? c.blue : c.mut) : "rgba(128,128,128,0.15)", maxC);
          if (st.n) { var sf = st.h[k] / st.n, hh2 = sf / maxC * (rowH - 18); ctx.strokeStyle = c.fg; ctx.lineWidth = 2; ctx.beginPath(); ctx.moveTo(x0 + k * bw + 2, top + 3 * rowH - 14 - hh2); ctx.lineTo(x0 + (k + 1) * bw - 2, top + 3 * rowH - 14 - hh2); ctx.stroke(); }
          ctx.fillStyle = c.mut; ctx.fillText(String(total), x0 + k * bw + bw / 2 - 5, top + 3 * rowH);
        }
        ctx.fillStyle = c.fg; ctx.font = "13px system-ui, sans-serif";
        ctx.fillText("die 1", x0, top - 4); ctx.fillText("die 2, flipped and slid", x0, top + rowH - 2); ctx.fillText("the total's chances", x0, top + 2 * rowH - 2);
        var ways = []; for (k = 1; k <= 6; k++) if (t - k >= 1 && t - k <= 6) ways.push("(" + k + "," + (t - k) + ")");
        return "Total " + t + ": the ways are " + ways.join(" ") + ". Multiply each pair's chances and add: P(" + t + ") = " + sum.toFixed(4) + (st.a.every(function (x) { return x === 1; }) && st.b.every(function (x) { return x === 1; }) ? " = " + ways.length + "/36" : "") + "." + (st.n ? " Black lines: " + st.n + " rolls of the two dice, sampled." : "");
      },
      panel: function (body) { body.append(el("p", { "class": "w-note" }, "Load the dice to make them unfair: the same counting works for any two distributions.")); }
    });
    var card = root.parentNode, redraw = function () { frame.redraw(); };
    bindRange(card, "data-total", function (v) { st.slide = v; }, function (v) { return String(v); }, redraw);
    pick(card, "data-load", function (v) { st.a = v === "1" ? [1, 1, 1, 1, 1, 3] : [1, 1, 1, 1, 1, 1]; st.b = v === "1" ? [3, 1, 1, 1, 1, 1] : [1, 1, 1, 1, 1, 1]; st.h = new Array(11).fill(0); st.n = 0; }, redraw);
    card.querySelectorAll("[data-roll]").forEach(function (b) {
      b.onclick = function () {
        var A = norm(st.a), B = norm(st.b), draw1 = function (P) { var u = st.r(), k = 0; while (k < 5 && (u -= P[k]) > 0) k++; return k + 1; };
        for (var i = 0; i < +b.getAttribute("data-roll"); i++) { st.h[draw1(A) + draw1(B) - 2]++; st.n++; }
        frame.redraw();
      };
    });
    return frame;
  }

  // ─── Rung 4: the phonon expansion, sampled (dice, then quartz's rho) ──
  function samplerWidget(root) {
    var Q = window.LEAPR_DATA.quartz, KI = [0.05, 0.3, 1, 5, 20];
    var st = { stage: "dice", dice: 2, n: 3, ki: 2, h: {}, cnt: 0, r: rng(4) };
    // Sample |beta| from T1 on both sides (gain side weighted e^-beta), return signed beta.
    var t1 = Q.tn[0], db = Q.deltab, cum = [], tot = 0;
    for (var i = 0; i < t1.length; i++) { var b = i * db, wgt = t1[i] * (1 + Math.exp(-b)) * db; tot += wgt; cum.push(tot); }
    function drawBeta() { var u = st.r() * tot, k = 0; while (k < cum.length - 1 && cum[k] < u) k++; var b = (k + st.r() - 0.5) * db; b = Math.max(b, 0); return st.r() < 1 / (1 + Math.exp(-b)) ? -b : b; }
    function sample(m) {
      for (var i2 = 0; i2 < m; i2++) {
        var v;
        if (st.stage === "dice") { v = 0; for (var d = 0; d < st.dice; d++) v += 1 + Math.floor(st.r() * 6); }
        else if (st.stage === "one") v = drawBeta();
        else if (st.stage === "n") { v = 0; for (d = 0; d < st.n; d++) v += drawBeta(); }
        else { var a = KI[st.ki], nn = poisson(st.r, a * Q.lambda); v = 0; for (d = 0; d < nn; d++) v += drawBeta(); }
        var key = st.stage === "dice" ? v : Math.round(v / 0.25);
        st.h[key] = (st.h[key] || 0) + 1; st.cnt++;
      }
    }
    var frame = new Frame(root, {
      reset: function () { st.h = {}; st.cnt = 0; },
      tick: function () { return false; },
      draw: function (ctx, w, h, zoom) {
        var c = palette(), x0 = 40, y0 = 84, pw = w - x0 - 12, ph = h - y0 - 50;
        ctx.font = "12px system-ui, sans-serif"; ctx.strokeStyle = c.mut; ctx.lineWidth = 1; ctx.strokeRect(x0, y0, pw, ph);
        var keys = Object.keys(st.h).map(Number);
        if (st.stage === "dice") {
          var lo = st.dice, hi = 6 * st.dice, bw = pw / (hi - lo + 1), mx = 0;
          keys.forEach(function (k) { mx = Math.max(mx, st.h[k] / st.cnt); });
          for (var k = lo; k <= hi; k++) { var f = (st.h[k] || 0) / Math.max(st.cnt, 1), hh = mx ? f / mx * ph * 0.9 : 0; ctx.fillStyle = c.acc; ctx.fillRect(x0 + (k - lo) * bw + 1, y0 + ph - hh, Math.max(bw - 2, 1), hh); }
          ctx.fillStyle = c.mut; ctx.fillText(String(lo), x0, y0 + ph + 16); ctx.fillText(String(hi), x0 + pw - 14, y0 + ph + 16);
          ctx.fillStyle = c.fg; ctx.font = "13px system-ui, sans-serif"; ctx.fillText("totals of " + st.dice + " dice", x0, y0 - 10);
          return st.cnt + " rolls of " + st.dice + " dice. " + (st.dice === 1 ? "One die: every face equally likely." : st.dice === 2 ? "Two dice: the triangle, peaking at 7 (rung 3½)." : "More dice: the totals pile into a smoother, more bell-shaped curve: adding many random numbers does that.");
        }
        // Phonon stages: histogram in beta (energy change in kT; loss to the left).
        var bmin = st.stage === "full" ? -Math.max(8, KI[st.ki] * 2.2) : -14, bmax = st.stage === "full" ? Math.max(3, KI[st.ki] * 0.4) : 4;
        var X = function (b) { return x0 + (b - bmin) / (bmax - bmin) * pw; };
        var exact = null, ex0 = 0;
        if (st.stage === "one" || st.stage === "n") {
          var T = Q.tn[(st.stage === "one" ? 1 : st.n) - 1];
          exact = function (b) { var ab = Math.abs(b), k2 = ab / db, k0 = Math.floor(k2); if (k0 + 1 >= T.length) return 0; var v = T[k0] + (T[k0 + 1] - T[k0]) * (k2 - k0); return b > 0 ? v * Math.exp(-ab) : v; };
        } else if (st.stage === "full") {
          var col = Q.cols[st.ki];
          exact = function (b) { var ab = Math.abs(b), k2 = ab / 0.05, k0 = Math.floor(k2); if (k0 + 1 >= col.s.length) return 0; var v = col.s[k0] + (col.s[k0 + 1] - col.s[k0]) * (k2 - k0); return b > 0 ? v * Math.exp(-ab) : v; };
          ex0 = Math.exp(-KI[st.ki] * Q.lambda);
        }
        ctx.save(); ctx.beginPath(); ctx.rect(x0, y0, pw, ph); ctx.clip();
        var mxh = 0; keys.forEach(function (k) { if (k !== 0 || st.stage !== "full") mxh = Math.max(mxh, st.h[k] / st.cnt / 0.25); });
        var mxe = 0; if (exact) for (var s = 0; s <= 200; s++) mxe = Math.max(mxe, exact(bmin + (bmax - bmin) * s / 200));
        var top = Math.max(mxh, mxe) * 1.15 / zoom || 1;
        keys.forEach(function (k) {
          if (st.stage === "full" && k === 0) return;
          var d = st.h[k] / st.cnt / 0.25, xx = X((k - 0.5) * 0.25);
          ctx.fillStyle = "rgba(31,111,120,0.6)"; ctx.fillRect(xx, y0 + ph - Math.min(d / top, 1) * ph, Math.max(X(0.25) - X(0), 1), Math.min(d / top, 1) * ph);
        });
        if (exact) { ctx.strokeStyle = c.hot; ctx.lineWidth = 2; ctx.beginPath(); for (s = 0; s <= 300; s++) { var bb = bmin + (bmax - bmin) * s / 300, yy = y0 + ph - Math.min(exact(bb) / top, 1) * ph; if (s) ctx.lineTo(X(bb), yy); else ctx.moveTo(X(bb), yy); } ctx.stroke(); }
        ctx.strokeStyle = c.mut; ctx.setLineDash([3, 3]); ctx.beginPath(); ctx.moveTo(X(0), y0); ctx.lineTo(X(0), y0 + ph); ctx.stroke(); ctx.setLineDash([]);
        ctx.restore();
        ctx.fillStyle = c.mut; ctx.fillText("← energy lost", x0 + 2, y0 + ph + 16); ctx.fillText("gained →", x0 + pw - 54, y0 + ph + 16);
        ctx.fillStyle = c.fg; ctx.font = "13px system-ui, sans-serif";
        if (st.stage === "one") { ctx.fillText("one phonon's energy, drawn from quartz's ρ", x0, y0 - 10); return st.cnt + " phonons drawn at random. Brown: T₁, the one-phonon shape, from OUTRAM PARK's LEAPR port (Si in α-quartz, " + Q.t + " K). The small right-hand part is absorbing a phonon already there (upscatter)."; }
        if (st.stage === "n") { ctx.fillText(st.n + " phonons' energies, added", x0, y0 - 10); return st.cnt + " sums of " + st.n + " random phonon energies. Brown: T" + st.n + ", the " + st.n + "-fold convolution, from the LEAPR port. We sample it to see it; LEAPR computes it exactly."; }
        ctx.fillText("1. how many? (Poisson)  2. add their energies", x0, y0 - 10);
        var mean = 0; keys.forEach(function (k) { mean += k * 0.25 * st.h[k]; }); mean = st.cnt ? mean / st.cnt : 0;
        return st.cnt + " collisions at kick α = " + KI[st.ki] + ": for each, a Poisson number of phonons (mean αλ = " + (KI[st.ki] * Q.lambda).toFixed(2) + "), their energies drawn from T₁ and added. Zero phonons (elastic, not drawn): " + (ex0 * 100).toFixed(1) + " % of collisions. Brown: the port's phonon-expansion S(α, β). Average energy change so far " + mean.toFixed(2) + " kT (the sum rule says −α = " + (-KI[st.ki]).toFixed(2) + ").";
      },
      panel: function (body) { body.append(el("p", { "class": "w-note" }, "Energy change β in units of kT at " + Q.t + " K; negative = the neutron lost energy. Quartz's ρ from its LEAPR deck; the curves from OUTRAM PARK's LEAPR port. The sampling is this page's way of showing the maths, not neutron transport.")); }
    });
    var card = root.parentNode;
    var reset = function () { st.h = {}; st.cnt = 0; frame.redraw(); };
    pick(card, "data-sstage", function (v) { st.stage = v; card.querySelectorAll("[data-for]").forEach(function (r) { r.style.display = r.getAttribute("data-for") === v ? "" : "none"; }); }, reset);
    pick(card, "data-dice", function (v) { st.dice = +v; }, reset);
    pick(card, "data-nph", function (v) { st.n = +v; }, reset);
    pick(card, "data-ki", function (v) { st.ki = +v; }, reset);
    card.querySelectorAll("[data-draw]").forEach(function (b) { b.onclick = function () { sample(+b.getAttribute("data-draw")); frame.redraw(); }; });
    card.querySelectorAll("[data-for]").forEach(function (r) { r.style.display = r.getAttribute("data-for") === st.stage ? "" : "none"; });
    return frame;
  }

  // ─── Rungs 5 and 5a, 5d-5g: the big-kick bell ────────────────────────
  // data-bell on the widget: "shortcut" (5: sampled sum vs the bell), "shape"
  // (5a: sliders), "snapshot" (5d), "sample" (5e), "width" (5f), "check" (5g).
  function bellWidget(root) {
    var Q = window.LEAPR_DATA.quartz, KI = [0.05, 0.3, 1, 5, 20];
    var mode = root.getAttribute("data-bell") || "shape";
    // The snapshot pages (5d-5g) use a light, hydrogen-like atom and a hard
    // kick, so that the recoil is well above the jiggle energy: the big-kick
    // limit the picture needs.
    var st = { ki: 4, Q: mode === "shape" ? 4 : 10, A: mode === "shape" ? 12 : 1, T: 293.6, hw: 0.1, jig: "both", snaps: [], r: rng(5), shortcut: true, last: null };
    var MN = 939.56542052e6, HBARC = 1973.26980459; // MeV, eV*Angstrom
    function recoil() { return HBARC * HBARC * st.Q * st.Q / (2 * st.A * MN); } // eV
    function kTbar() {
      var kT = 8.617333262e-5 * st.T, half = st.hw / 2;
      if (st.jig === "thermal") return kT;
      if (st.jig === "zero") return half;
      return st.T > 0 ? half / Math.tanh(half / kT) : half;
    }
    var frame = new Frame(root, {
      reset: function () { st.snaps = []; st.last = null; },
      tick: function () { return false; },
      draw: function (ctx, w, h, zoom) {
        var c = palette(), x0 = 40, y0 = 84, pw = w - x0 - 12, ph = h - y0 - 50;
        ctx.font = "12px system-ui, sans-serif"; ctx.strokeStyle = c.mut; ctx.lineWidth = 1; ctx.strokeRect(x0, y0, pw, ph);
        if (mode === "shortcut") {
          var col = Q.cols[st.ki], a = KI[st.ki], bmax = Math.min(30, Math.max(4, a * 2.4)), X = function (b) { return x0 + b / bmax * pw; };
          var mx = 0; col.s.forEach(function (v, k) { if (k * 0.05 <= bmax) mx = Math.max(mx, v, col.sct[k]); });
          var Y = function (v) { return y0 + ph - v / (mx * 1.1) * ph; };
          ctx.strokeStyle = c.acc; ctx.lineWidth = 2; ctx.beginPath(); col.s.forEach(function (v, k) { var b = k * 0.05; if (b > bmax) return; if (k) ctx.lineTo(X(b), Y(v)); else ctx.moveTo(X(b), Y(v)); }); ctx.stroke();
          if (st.shortcut) { ctx.strokeStyle = c.hot; ctx.setLineDash([5, 4]); ctx.beginPath(); col.sct.forEach(function (v, k) { var b = k * 0.05; if (b > bmax) return; if (k) ctx.lineTo(X(b), Y(v)); else ctx.moveTo(X(b), Y(v)); }); ctx.stroke(); ctx.setLineDash([]); }
          ctx.fillStyle = c.mut; ctx.fillText("0", x0, y0 + ph + 16); ctx.fillText("energy lost: " + bmax.toFixed(0) + " kT", x0 + pw - 110, y0 + ph + 16);
          ctx.fillStyle = c.fg; ctx.font = "13px system-ui, sans-serif"; ctx.fillText("adding up the phonon cases vs the bell", x0, y0 - 10);
          var d = 0, s2 = 0; col.s.forEach(function (v, k) { d += Math.abs(v - col.sct[k]) * 0.05; s2 += v * 0.05; });
          return "Kick α = " + a + ": teal, adding up the phonon cases (OUTRAM PARK's LEAPR port, Si in α-quartz); brown dashed, the bell-curve shortcut at the effective temperature T̄ = " + (Q.tbar * Q.t).toFixed(0) + " K. They differ by " + (100 * d / Math.max(s2, 1e-12)).toFixed(0) + " % of the area: " + (a >= 5 ? "the shortcut is safe here." : "too far apart: for small kicks the shortcut fails.");
        }
        // The bell in physical units: energy lost, mean E_R, variance 2 E_R kTbar.
        var ER = recoil(), kTb = mode === "shape" ? 8.617333262e-5 * st.T : kTbar(), sd = Math.sqrt(2 * ER * kTb);
        var spread = Math.max(sd, 0.02 * ER), emin = Math.min(0, ER - 5 * spread), emax = Math.max(ER + 5 * spread, ER * 1.6), X2 = function (e) { return x0 + (e - emin) / (emax - emin) * pw; };
        ctx.save(); ctx.beginPath(); ctx.rect(x0, y0, pw, ph); ctx.clip();
        var gauss2 = function (e, s) { return s > 0 ? Math.exp(-(e - ER) * (e - ER) / (2 * s * s)) / (s * Math.sqrt(2 * Math.PI)) : 0; };
        var peak = sd > 0 ? 1 / (sd * Math.sqrt(2 * Math.PI)) : 1;
        if (mode === "shape") {
          var sd0 = Math.sqrt(2 * ER * 0.05); // zero-point jiggle of a 100 meV spring: kTbar = hbar omega / 2
          ctx.fillStyle = "rgba(128,128,128,0.18)"; ctx.fillRect(X2(ER - 2 * sd0), y0, X2(ER + 2 * sd0) - X2(ER - 2 * sd0), ph);
        }
        if (mode === "sample" || mode === "snapshot") {
          var bins = {}, bw = Math.max(sd, 1e-6) / 4;
          st.snaps.forEach(function (e) { var k = Math.floor(e / bw); bins[k] = (bins[k] || 0) + 1; });
          Object.keys(bins).forEach(function (k) { var dd = bins[k] / st.snaps.length / bw, hh = Math.min(dd / (peak * 1.15) * ph, ph); ctx.fillStyle = "rgba(31,111,120,0.55)"; ctx.fillRect(X2(k * bw), y0 + ph - hh, Math.max(X2(bw) - X2(0), 1), hh); });
        }
        if (mode !== "snapshot" || st.snaps.length > 30) { ctx.strokeStyle = c.hot; ctx.lineWidth = 2; ctx.beginPath(); for (var s3 = 0; s3 <= 300; s3++) { var e = emin + (emax - emin) * s3 / 300, yy = y0 + ph - Math.min(gauss2(e, sd) / (peak * 1.15) * ph, ph); if (s3) ctx.lineTo(X2(e), yy); else ctx.moveTo(X2(e), yy); } ctx.stroke(); }
        ctx.strokeStyle = c.fg; ctx.setLineDash([3, 3]); ctx.beginPath(); ctx.moveTo(X2(ER), y0); ctx.lineTo(X2(ER), y0 + ph); ctx.stroke(); ctx.setLineDash([]);
        if (mode === "snapshot" && st.last) { ctx.fillStyle = c.blue; ctx.beginPath(); ctx.arc(X2(st.last.e), y0 + ph - 10, 6, 0, 2 * Math.PI); ctx.fill(); }
        if (emin < 0) { ctx.strokeStyle = c.mut; ctx.beginPath(); ctx.moveTo(X2(0), y0); ctx.lineTo(X2(0), y0 + ph); ctx.stroke(); }
        ctx.restore();
        ctx.fillStyle = c.mut; ctx.fillText(emin < 0 ? fmtE(emin) : "0", x0, y0 + ph + 16); var t1 = fmtE(emax); ctx.fillText(t1, x0 + pw - ctx.measureText(t1).width, y0 + ph + 16); ctx.fillText("energy the neutron loses", x0 + pw / 2 - 60, y0 + ph + 30);
        ctx.fillStyle = c.fg; ctx.font = "13px system-ui, sans-serif"; ctx.fillText("dashed: the free recoil E_R; brown: the bell", x0, y0 - 10);
        var head = "Kick " + st.Q.toFixed(1) + " Å⁻¹ on mass " + st.A.toFixed(0) + ": free recoil E_R = " + fmtE(ER) + ".";
        if (mode === "shape") return head + " Width √(2 E_R kT) = " + fmtE(sd) + " at " + st.T.toFixed(0) + " K. Heavier atom or gentler kick: the centre moves left. Hotter: wider. Grey: the width a bound atom keeps even at 0 K, here for a 100 meV spring (pages 5b and 5c).";
        if (mode === "snapshot") return head + (st.last ? " This collision: the atom was moving at " + st.last.v.toFixed(0) + " m/s along the kick, so the neutron lost " + fmtE(st.last.e) + " instead of exactly E_R." : " Press Take a snapshot.") + " " + st.snaps.length + " so far.";
        var kind = st.jig === "thermal" ? "thermal jiggle only" : st.jig === "zero" ? "zero-point jiggle only" : "both";
        if (mode === "sample") return head + " " + st.snaps.length + " snapshots with " + kind + " (a spring of ħω = " + (st.hw * 1e3).toFixed(0) + " meV at " + st.T.toFixed(0) + " K): atom speeds drawn from a bell, so the energy losses pile into a bell (brown) around E_R.";
        if (mode === "width") return head + " The atoms' jiggle energy is kT̄ = " + fmtE(kTb) + " (" + kind + "), T̄ = " + (kTb / 8.617333262e-5).toFixed(0) + " K. Width √(2 E_R kT̄) = " + fmtE(sd) + ".";
        var bi = st.hw / (8.617333262e-5 * st.T);
        return "One spring, ħω = " + (st.hw * 1e3).toFixed(0) + " meV at " + st.T.toFixed(0) + " K: kT̄ = (ħω/2) coth(ħω/2kT) = " + fmtE(kTbar()) + ", so T̄/T = " + (kTbar() / (8.617333262e-5 * st.T)).toFixed(3) + "; the manual's (β/2) coth(β/2) with β = ħω/kT = " + bi.toFixed(2) + " gives " + ((bi / 2) / Math.tanh(bi / 2)).toFixed(3) + ". " + (bi < 0.5 ? "A soft spring: T̄ ≈ T." : bi > 4 ? "A stiff spring: kT̄ ≈ ħω/2, the zero-point jiggle." : "In between.");
      },
      panel: function (body) {
        body.append(el("p", { "class": "w-note" }, mode === "shortcut" ? "Si in α-quartz at " + Q.t + " K from OUTRAM PARK's LEAPR port: the phonon expansion and the short-collision-time bell at the same kicks. Energy loss in units of kT." : "A toy in physical units: a kick Q, an atom of mass A neutron masses, recoil E_R = (ħQ)²/(2M). The bell has mean E_R and variance 2 E_R kT̄, exact for free gas and the short-collision-time limit for bound atoms."));
      }
    });
    var card = root.parentNode, redraw = function () { frame.redraw(); };
    bindRange(card, "data-bq", function (v) { st.Q = v; st.snaps = []; }, function (v) { return v.toFixed(1) + " Å⁻¹"; }, redraw);
    bindRange(card, "data-ba", function (v) { st.A = v; st.snaps = []; }, function (v) { return v.toFixed(0); }, redraw);
    bindRange(card, "data-bt", function (v) { st.T = v; st.snaps = []; }, function (v) { return v.toFixed(0) + " K"; }, redraw);
    bindRange(card, "data-bhw", function (v) { st.hw = v / 1e3; st.snaps = []; }, function (v) { return v.toFixed(0) + " meV"; }, redraw);
    pick(card, "data-ki", function (v) { st.ki = +v; }, redraw);
    pick(card, "data-jig", function (v) { st.jig = v; st.snaps = []; }, redraw);
    pick(card, "data-short", function (v) { st.shortcut = v === "1"; }, redraw);
    function snap(n) {
      var M = st.A * 1.67492749804e-27, ER = recoil(), q = st.Q * 1e10, hbar = 1.054571817e-34, ev = 1.602176634e-19;
      for (var i = 0; i < n; i++) {
        // Velocity along the kick from a bell of variance kTbar/M.
        var v = Math.sqrt(kTbar() * ev / M) * gauss(st.r), e = ER + hbar * q * v / ev;
        st.snaps.push(e); st.last = { v: v, e: e };
      }
    }
    card.querySelectorAll("[data-snap]").forEach(function (b) { b.onclick = function () { snap(+b.getAttribute("data-snap")); frame.redraw(); }; });
    return frame;
  }

  // ─── Rungs 5b, 5c: the swing that never sits still ───────────────────
  function zeroWidget(root) {
    var mode = root.getAttribute("data-zero") || "zp";
    var st = { T: 0, hw: 0.1, t: 0 };
    var frame = new Frame(root, {
      reset: function () { st.t = 0; },
      tick: function (dt) { st.t += dt; return true; },
      draw: function (ctx, w, h, zoom) {
        var c = palette(), kT = 8.617333262e-5 * st.T, half = st.hw / 2, kTb = st.T > 0 ? half / Math.tanh(half / kT) : half;
        var cx = w * 0.36, base = h - 70, scale = (h - 170) / (6 * st.hw);
        // The ladder: levels (n + 1/2) hbar omega above the bottom of the well.
        ctx.strokeStyle = c.mut; ctx.lineWidth = 1.5;
        ctx.beginPath(); for (var s = -60; s <= 60; s++) { var x = s / 60, y = base - x * x * 6 * st.hw * scale; if (s > -60) ctx.lineTo(cx + x * w * 0.3, y); else ctx.moveTo(cx + x * w * 0.3, y); } ctx.stroke();
        for (var n = 0; n < 6; n++) { var e = (n + 0.5) * st.hw, xw = Math.sqrt(e / (6 * st.hw)) * w * 0.3, yy = base - e * scale; ctx.strokeStyle = n === 0 ? c.hot : c.fg; ctx.lineWidth = n === 0 ? 3 : 1; ctx.beginPath(); ctx.moveTo(cx - xw, yy); ctx.lineTo(cx + xw, yy); ctx.stroke(); }
        ctx.strokeStyle = c.mut; ctx.setLineDash([4, 4]); ctx.beginPath(); ctx.moveTo(cx - w * 0.3, base); ctx.lineTo(cx + w * 0.3, base); ctx.stroke(); ctx.setLineDash([]);
        ctx.fillStyle = c.mut; ctx.font = "11px system-ui, sans-serif"; ctx.fillText("bottom of the well (zero energy)", cx - w * 0.3, base + 14);
        ctx.fillStyle = c.hot; ctx.fillText("lowest level: ½ħω", cx + w * 0.3 * Math.sqrt(1 / 12) + 6, base - 0.5 * st.hw * scale + 4);
        // The position cloud: a Gaussian whose spread squared goes as coth(hbar omega/2kT).
        var spread = Math.sqrt(kTb / half) * 22, gx = w * 0.82, gy = 120;
        for (var k = 0; k < 40; k++) { var a = Math.sin(st.t * 3 + k * 2.1) * spread * Math.cos(k * 1.7), b = Math.cos(st.t * 2.3 + k) * 6; ctx.fillStyle = "rgba(31,111,120,0.35)"; ctx.beginPath(); ctx.arc(gx + a, gy + b, 4, 0, 2 * Math.PI); ctx.fill(); }
        ctx.fillStyle = c.hot; ctx.beginPath(); ctx.arc(gx, gy, 3, 0, 2 * Math.PI); ctx.fill();
        ctx.fillStyle = c.mut; ctx.font = "11px system-ui, sans-serif"; ctx.fillText("where the atom is", gx - 46, gy + 34);
        ctx.fillStyle = c.fg; ctx.font = "13px system-ui, sans-serif"; ctx.fillText(mode === "zp" ? "one swing, cooled" : "how stiff is the spring?", 12, 80);
        var bi = st.T > 0 ? st.hw / kT : Infinity;
        if (mode === "zp") return "At " + st.T.toFixed(0) + " K. Even at 0 K the swing sits on its lowest level, ½ħω = " + fmtE(half) + " above the bottom, and the atom's position stays spread out: the ground state still has motion. Heat it and thermal jiggle adds on top: its jiggle energy is kT̄ = " + fmtE(kTb) + ".";
        return "Step ħω = " + fmtE(st.hw) + " against kT = " + fmtE(kT) + " (" + st.T.toFixed(0) + " K): " + (st.T > 0 ? "T̄/T = " + (kTb / kT).toFixed(2) + ". " : "") + (bi > 4 ? "A step much bigger than kT: the zero-point jiggle dominates, kT̄ ≈ ħω/2." : bi < 0.5 ? "A step much smaller than kT: thermal jiggle dominates, T̄ ≈ T." : "Step and kT comparable: both matter.");
      },
      panel: function (body) { body.append(el("p", { "class": "w-note" }, "One harmonic oscillator: levels (n + ½)ħω. The position cloud's spread grows as coth(ħω/2kT), exact for the harmonic model; real bonds deviate slightly.")); }
    });
    var card = root.parentNode, redraw = function () { frame.redraw(); };
    bindRange(card, "data-zt", function (v) { st.T = v; }, function (v) { return v.toFixed(0) + " K"; }, redraw);
    bindRange(card, "data-zhw", function (v) { st.hw = v / 1e3; }, function (v) { return v.toFixed(0) + " meV"; }, redraw);
    return frame;
  }

  // ─── Rung 5h: do the waves from different atoms add up? ──────────────
  // A square patch of atoms, spacing d; incoming wave along +x. The outgoing
  // intensity at angle theta is |sum_j b_j exp(i (k_in - k_out) . r_j)|^2.
  function wavesWidget(root) {
    var st = { coherent: true, lam: 1.8, r: rng(58), b: null };
    var N = 14, d = 3.35;
    function strengths() { if (!st.b) { st.b = []; for (var i = 0; i < N * N; i++) st.b.push(st.coherent ? 1 : (st.r() < 0.5 ? -1 : 1) * (0.4 + 1.2 * st.r())); } return st.b; }
    function intensity(th) {
      var k = 2 * Math.PI / st.lam, qx = k * (1 - Math.cos(th)), qy = -k * Math.sin(th), re = 0, im = 0, b = strengths();
      for (var i = 0; i < N; i++) for (var j = 0; j < N; j++) { var ph = qx * i * d + qy * j * d, s = b[i * N + j]; re += s * Math.cos(ph); im += s * Math.sin(ph); }
      return (re * re + im * im) / (N * N);
    }
    var frame = new Frame(root, {
      reset: function () { st.b = null; },
      tick: function () { return false; },
      draw: function (ctx, w, h, zoom) {
        var c = palette(), cx = w / 2, cy = 84 + (h - 140) / 2, R = Math.min(w, h - 140) / 2 - 16;
        var M = 360, vals = [], mx = 0;
        for (var s = 0; s < M; s++) { var th = s / M * 2 * Math.PI; var v = s === 0 ? 0 : intensity(th); vals.push(v); if (s > 6 && s < M - 6) mx = Math.max(mx, v); }
        mx = Math.max(mx, 1);
        ctx.strokeStyle = c.mut; ctx.lineWidth = 1; ctx.beginPath(); ctx.arc(cx, cy, R, 0, 2 * Math.PI); ctx.stroke();
        ctx.strokeStyle = st.coherent ? c.acc : c.hot; ctx.lineWidth = 2; ctx.beginPath();
        vals.forEach(function (v, s2) { var th = s2 / M * 2 * Math.PI, rr = (s2 < 6 || s2 > M - 6) ? 0 : Math.min(v / mx, 1) * R; var xx = cx + rr * Math.cos(th), yy = cy - rr * Math.sin(th); if (s2) ctx.lineTo(xx, yy); else ctx.moveTo(xx, yy); });
        ctx.closePath(); ctx.stroke();
        L.arrow(ctx, cx - R - 6, cy, cx - R * 0.55, cy, c.blue);
        ctx.fillStyle = c.mut; ctx.font = "11px system-ui, sans-serif"; ctx.fillText("incoming", cx - R - 6, cy - 8);
        ctx.fillStyle = c.fg; ctx.font = "13px system-ui, sans-serif"; ctx.fillText("scattered intensity by direction", 12, 80);
        var peaks = 0; for (s = 8; s < M - 8; s++) if (vals[s] > vals[s - 1] && vals[s] > vals[s + 1] && vals[s] > 0.5 * mx) peaks++;
        return (st.coherent ? "Identical atoms in a perfect pattern: the scattered waves keep in step, and add up into strong beams in a few directions (Bragg)." : "Each atom scatters a little differently (random strength and sign): the waves don't keep in step, and the beams wash out into an even spray.") +
          " λ = " + st.lam.toFixed(2) + " Å, plane spacing " + d + " Å" + (st.coherent ? (st.lam > 2 * d ? ": longer than twice the spacing, so no beam at all (page 21's cutoff)." : ": " + peaks + " strong beams.") : ".") + " The straight-through direction is left out.";
      },
      panel: function (body) { body.append(el("p", { "class": "w-note" }, "A 14 × 14 patch of atoms, spacing 3.35 Å (graphite's layer spacing), incoming wave from the left; intensity |Σ b e^{i(k_in − k_out)·r}|² per direction, a toy.")); }
    });
    var card = root.parentNode;
    pick(card, "data-coh", function (v) { st.coherent = v === "1"; st.b = null; }, function () { frame.redraw(); });
    bindRange(card, "data-wlam", function (v) { st.lam = v; }, function (v) { return v.toFixed(2) + " Å"; }, function () { frame.redraw(); });
    return frame;
  }

  L.widget("abmap", abWidget);
  L.widget("swing", swingWidget);
  L.widget("dice", diceWidget);
  L.widget("sampler", samplerWidget);
  L.widget("bell", bellWidget);
  L.widget("zero", zeroWidget);
  L.widget("waves", wavesWidget);
})();
