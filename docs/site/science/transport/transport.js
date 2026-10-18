// The transport lessons' widgets (gh:#906, maintainer's second task of
// 2026-10-18). Each is a JavaScript sketch of ONE idea whose real code adds
// nothing a reader can see: the page says so next to it and embeds the real
// OutramMC demo (dhoby-ghaut, running in the browser) wherever a real run
// teaches more. The pure maths is on window.Lesson.transportMath, and
// check-transport.js tests it under node: against closed forms, against
// pinned values from the Rust crates (the RNG), and against each other.
//
// Widgets: flight (page 1: the exponential flight), roundoff (page 3: which
// side of a surface is a particle on), stream (page 4: the RNG and why
// per-history streams make a run repeatable), ladder (page 5: who and what at
// a collision, with the numbers of collision-data.js), woodcock (page 8: delta
// tracking against surface tracking in one dimension), comb (page 9: banking
// and resampling fission sites), correlated (page 10: why an error bar that
// assumes independent generations can be too small).
(function () {
  "use strict";
  var L = window.Lesson;
  var css = L.css, el = L.el, buttons = L.buttons, slider = L.slider, toggle = L.toggle, Frame = L.Frame;

  // ─── Pure maths ────────────────────────────────────────────────────────
  // The 64-bit LCG with PCG-RXS-M-XS output of petir::rng::lcg (crates/petir/
  // src/rng/lcg.rs, a port of OpenMC's random_lcg.cpp), in BigInt.
  var MASK = (1n << 64n) - 1n;
  var MULT = 6364136223846793005n, INC = 1442695040888963407n, PERM = 12605985483714917081n;
  var DEFAULT_STRIDE = 152917n, GEN_STRIDE = 1n << 40n;
  function u64(x) { return BigInt.asUintN(64, x); }
  // One step of the recurrence; returns the new state.
  function step(s) { return u64(s * MULT + INC); }
  // The output permutation of a state (lcg.rs:prn steps 2 and 3): the double in [0, 1).
  function output(s) {
    var word = u64(((s >> ((s >> 59n) + 5n)) ^ s) * PERM), res = (word >> 43n) ^ word;
    return Number(res) * (1 / 18446744073709551616);
  }
  // prn: advance and return {state, x}.
  function prn(s) { var t = step(s); return { state: t, x: output(t) }; }
  // The jump-ahead: the state n steps on, in O(log n) (lcg.rs:future_seed).
  function futureSeed(n, seed) {
    var a = MULT, c = INC, am = 1n, cm = 0n;
    n = BigInt(n);
    while (n > 0n) {
      if (n & 1n) { am = u64(am * a); cm = u64(cm * a + c); }
      c = u64((a + 1n) * c); a = u64(a * a); n >>= 1n;
    }
    return u64(am * seed + cm);
  }
  // The seed of history `hist` in generation `gen` of a run with master seed `seed`
  // (transport_csg.rs, run_keff_csg_par_progress: gen_base_seed, then the history's).
  function historySeed(seed, gen, hist) {
    return futureSeed(BigInt(hist) * DEFAULT_STRIDE, futureSeed(BigInt(gen) * GEN_STRIDE, BigInt(seed)));
  }
  // A float generator for the widgets that need a stream of their own.
  function rng(seed) {
    return function () {
      seed = (seed + 0x6D2B79F5) | 0;
      var t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
      t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }

  // Distance to the next collision in a medium of macroscopic total cross
  // section sigma [1/cm]: d = -ln(xi)/sigma, infinite in void
  // (transport_csg.rs surface_arm; xi is kept off zero as there).
  function flightDistance(xi, sigma) {
    return sigma > 0 ? -Math.log(Math.max(xi, 2.2250738585072014e-308)) / sigma : Infinity;
  }

  // Is a particle that has just been stepped onto a sphere (radius 1, at the
  // point where its ray first meets it) found inside by the sign of
  // f = |r|^2 - R^2? b is the impact parameter / R, in [0, 1).
  // Returns { signInside, tokenInside, nudgedInside } for one random ray.
  function surfaceStep(b, r) {
    // A ray in the x-z plane, aimed along +z, offset by b in x, started at z = -3, rotated randomly about z
    // so that the rounding differs from ray to ray.
    var ph = 2 * Math.PI * r(), ox = b * Math.cos(ph), oy = b * Math.sin(ph);
    var o = [ox, oy, -3], u = [0, 0, 1];
    var ou = o[0] * u[0] + o[1] * u[1] + o[2] * u[2], oo = o[0] * o[0] + o[1] * o[1] + o[2] * o[2];
    var disc = ou * ou - (oo - 1);
    var d = -ou - Math.sqrt(disc);
    var p = [o[0] + d * u[0], o[1] + d * u[1], o[2] + d * u[2]];
    var f = p[0] * p[0] + p[1] * p[1] + p[2] * p[2] - 1;
    // The particle enters, so the truth is: inside. The outward normal at p is p itself; u . n < 0 says "entering".
    var n = p, dot = u[0] * n[0] + u[1] * n[1] + u[2] * n[2];
    var q = [p[0] - 1e-9 * n[0] + 1e-9 * u[0], p[1] - 1e-9 * n[1] + 1e-9 * u[1], p[2] - 1e-9 * n[2] + 1e-9 * u[2]];
    var fq = q[0] * q[0] + q[1] * q[1] + q[2] * q[2] - 1;
    return { signInside: f < 0, tokenInside: dot < 0, nudgedInside: fq < 0, f: f };
  }

  // The reaction ladder at a collision (transport_csg.rs): xi * sigma_total
  // against cumulative fission, absorption (capture after fission), inelastic,
  // (n,2n), elastic last. parts = { fission, capture, inelastic, n2n, other, elastic }.
  function ladderPick(parts, xi) {
    var order = ["fission", "capture", "inelastic", "n2n", "other", "elastic"], tot = 0, i;
    for (i = 0; i < order.length; i++) tot += parts[order[i]] || 0;
    var t = xi * tot, acc = 0;
    for (i = 0; i < order.length; i++) { acc += parts[order[i]] || 0; if (t < acc) return order[i]; }
    return "elastic";
  }
  // The nuclide draw (material.rs sample_nuclide): xi * Sigma_t against the cumulative N_i sigma_t,i.
  function nuclidePick(totals, xi) {
    var s = 0, i; for (i = 0; i < totals.length; i++) s += totals[i];
    var t = xi * s, acc = 0;
    for (i = 0; i < totals.length; i++) { acc += totals[i]; if (t < acc) return i; }
    return totals.length - 1;
  }

  // One-dimensional two-region slab: region A on [0, a) with sigma sA, region B
  // on [a, L) with sB, vacuum beyond L, a neutron born at 0 flying along +x.
  // Both trackers return { where: "A" | "B" | "leak", x, flights, virtual, boundary }.
  function surfaceTrack(sA, sB, a, L, r) {
    var x = 0, flights = 0, boundary = 0;
    for (;;) {
      var inA = x < a, s = inA ? sA : sB, edge = inA ? a : L, d = flightDistance(r(), s);
      flights++;
      if (x + d < edge) return { where: inA ? "A" : "B", x: x + d, flights: flights, virtual: 0, boundary: boundary };
      x = edge; boundary++;
      if (edge >= L) return { where: "leak", x: L, flights: flights, virtual: 0, boundary: boundary };
    }
  }
  function deltaTrack(sA, sB, a, L, r) {
    var maj = Math.max(sA, sB), x = 0, flights = 0, virt = 0;
    for (;;) {
      x += flightDistance(r(), maj); flights++;
      if (x >= L) return { where: "leak", x: L, flights: flights, virtual: virt, boundary: 0 };
      var s = x < a ? sA : sB;
      if (r() < s / maj) return { where: x < a ? "A" : "B", x: x, flights: flights, virtual: virt, boundary: 0 };
      virt++;
    }
  }

  // Fission-site bookkeeping, ported from physics/fission.rs.
  // sample_num_neutrons: floor(nu/k) plus a Bernoulli draw on the remainder.
  function numNeutrons(nuBar, k, xi) { var e = nuBar / (k > 0 ? k : 1), n = Math.floor(e); if (xi < e - n) n++; return n; }
  // comb_resample: how many times each of `total` sites is taken when n are drawn,
  // teeth total/n apart from one random offset; returns the multiplicity array.
  function combMultiplicities(total, n, xi) {
    var teeth = total / n, off = xi * teeth, m = new Array(total).fill(0);
    for (var i = 0; i < n; i++) m[Math.min(Math.floor(off + i * teeth), total - 1)]++;
    return m;
  }
  function replacementMultiplicities(total, n, r) {
    var m = new Array(total).fill(0);
    for (var i = 0; i < n; i++) m[Math.min(Math.floor(r() * total), total - 1)]++;
    return m;
  }

  // A correlated sequence (AR(1), unit variance) of length n with lag-one
  // correlation rho: a stand-in for generation-to-generation correlation.
  function ar1(n, rho, r) {
    function normal() { return Math.sqrt(-2 * Math.log(Math.max(r(), 1e-300))) * Math.cos(2 * Math.PI * r()); }
    var x = new Array(n), prev = normal();
    for (var i = 0; i < n; i++) { prev = rho * prev + Math.sqrt(1 - rho * rho) * normal(); x[i] = prev; }
    return x;
  }
  function meanStderr(x) {
    var n = x.length, m = 0, i; for (i = 0; i < n; i++) m += x[i]; m /= n;
    var v = 0; for (i = 0; i < n; i++) v += (x[i] - m) * (x[i] - m);
    return { mean: m, se: Math.sqrt(v / (n - 1) / n) };
  }

  L.transportMath = {
    step: step, output: output, prn: prn, futureSeed: futureSeed, historySeed: historySeed, rng: rng,
    flightDistance: flightDistance, surfaceStep: surfaceStep, ladderPick: ladderPick, nuclidePick: nuclidePick,
    surfaceTrack: surfaceTrack, deltaTrack: deltaTrack, numNeutrons: numNeutrons, combMultiplicities: combMultiplicities,
    replacementMultiplicities: replacementMultiplicities, ar1: ar1, meanStderr: meanStderr,
    DEFAULT_STRIDE: DEFAULT_STRIDE, GEN_STRIDE: GEN_STRIDE
  };

  // ─── Drawing helpers (as dbrc.js) ──────────────────────────────────────
  function niceTicks(lo, hi, n) {
    var raw = (hi - lo) / n, mag = Math.pow(10, Math.floor(Math.log10(raw))), f = raw / mag;
    var st = (f < 1.5 ? 1 : f < 3.5 ? 2 : f < 7.5 ? 5 : 10) * mag, out = [];
    for (var v = Math.ceil(lo / st) * st; v <= hi + st * 1e-9; v += st) out.push(+v.toPrecision(12));
    return out;
  }
  function fmtTick(v, st) { var d = Math.max(0, -Math.floor(Math.log10(st))); return v.toFixed(Math.min(d, 6)); }
  function xAxis(ctx, x0, x1, y, lo, hi, label, col) {
    var ticks = niceTicks(lo, hi, Math.max(2, Math.floor((x1 - x0) / 70))), st = ticks.length > 1 ? ticks[1] - ticks[0] : 1;
    ctx.strokeStyle = col; ctx.fillStyle = col; ctx.lineWidth = 1; ctx.font = "12px system-ui, sans-serif";
    ctx.beginPath(); ctx.moveTo(x0, y); ctx.lineTo(x1, y); ctx.stroke();
    ticks.forEach(function (v) {
      var x = x0 + (v - lo) / (hi - lo) * (x1 - x0);
      ctx.beginPath(); ctx.moveTo(x, y); ctx.lineTo(x, y + 4); ctx.stroke();
      var t = fmtTick(v, st); ctx.fillText(t, x - ctx.measureText(t).width / 2, y + 16);
    });
    ctx.fillText(label, (x0 + x1) / 2 - ctx.measureText(label).width / 2, y + 30);
  }
  function yAxis(ctx, x, y0, y1, lo, hi, col) {
    var ticks = niceTicks(lo, hi, 4), st = ticks.length > 1 ? ticks[1] - ticks[0] : 1;
    ctx.strokeStyle = col; ctx.fillStyle = col; ctx.lineWidth = 1; ctx.font = "12px system-ui, sans-serif";
    ctx.beginPath(); ctx.moveTo(x, y0); ctx.lineTo(x, y1); ctx.stroke();
    ticks.forEach(function (v) {
      var y = y1 - (v - lo) / (hi - lo) * (y1 - y0);
      ctx.beginPath(); ctx.moveTo(x - 4, y); ctx.lineTo(x, y); ctx.stroke();
      var t = fmtTick(v, st); ctx.fillText(t, x - 8 - ctx.measureText(t).width, y + 4);
    });
  }
  function colors() { return { fg: css("--fg", "#1d2125"), acc: css("--accent", "#1f6f78"), mut: css("--muted", "#5b636b"), hot: "#b85c38" }; }
  function still() { return false; }
  function noop() { }

  // ─── Widget: the exponential flight (page 1) ───────────────────────────
  // JavaScript only, on purpose: the flight is one line, d = -ln(xi)/Sigma,
  // and the real code adds nothing to see beyond it. The real thing is on
  // page 2, in the Godiva demo.
  function flight(root) {
    var st = { sigma: 0.5, N: 30000 }, cache = null;
    function get() {
      if (!cache || cache.sigma !== st.sigma) {
        var r = rng(906), d = new Float64Array(st.N), s = 0, beyond1 = 0, beyond3 = 0;
        for (var i = 0; i < st.N; i++) { d[i] = flightDistance(r(), st.sigma); s += d[i]; if (d[i] > 1 / st.sigma) beyond1++; if (d[i] > 3 / st.sigma) beyond3++; }
        cache = { sigma: st.sigma, d: d, mean: s / st.N, b1: beyond1 / st.N, b3: beyond3 / st.N };
      }
      return cache;
    }
    return new Frame(root, {
      reset: noop, onSeen: noop, tick: still,
      draw: function (ctx, w, h, zoom) {
        var C = get(), c = colors(), L0 = 50, R0 = w - 12, T0 = 70, B0 = h - 50, mfp = 1 / st.sigma, hi = 5 * mfp / zoom, nb = 50, bw = hi / nb;
        var hist = new Float64Array(nb), i;
        for (i = 0; i < C.d.length; i++) { var k = Math.floor(C.d[i] / bw); if (k < nb) hist[k]++; }
        var top = st.sigma * 1.1;
        function X(d) { return L0 + d / hi * (R0 - L0); } function Y(v) { return B0 - v / top * (B0 - T0); }
        ctx.fillStyle = "rgba(31, 111, 120, 0.40)";
        for (i = 0; i < nb; i++) { var dens = hist[i] / C.d.length / bw; ctx.fillRect(X(i * bw) + 0.5, Y(dens), Math.max(1, X((i + 1) * bw) - X(i * bw) - 1), B0 - Y(dens)); }
        ctx.strokeStyle = c.hot; ctx.lineWidth = 2; ctx.beginPath();
        for (i = 0; i <= 200; i++) { var dd = hi * i / 200, v = st.sigma * Math.exp(-st.sigma * dd); if (i) ctx.lineTo(X(dd), Y(v)); else ctx.moveTo(X(dd), Y(v)); }
        ctx.stroke();
        ctx.strokeStyle = c.fg; ctx.lineWidth = 1.5; ctx.setLineDash([4, 3]); ctx.beginPath(); ctx.moveTo(X(mfp), T0); ctx.lineTo(X(mfp), B0); ctx.stroke(); ctx.setLineDash([]);
        yAxis(ctx, L0, T0, B0, 0, top, c.mut);
        xAxis(ctx, L0, R0, B0, 0, hi, "distance to the next collision d (cm)", c.mut);
        ctx.font = "12px system-ui, sans-serif"; ctx.fillStyle = c.hot; ctx.fillText("curve: Σ e^(−Σd)", L0 + 8, T0 - 22);
        ctx.fillStyle = c.acc; ctx.fillText("bars: 30 000 draws of −ln ξ / Σ", L0 + 8, T0 - 8);
        ctx.fillStyle = c.fg; ctx.fillText("mean free path 1/Σ", Math.min(X(mfp) + 4, R0 - 110), T0 + 12);
        return "Σ = " + st.sigma.toFixed(2) + " per cm, so 1/Σ = " + mfp.toFixed(2) + " cm. Mean of the draws " + C.mean.toFixed(3) + " cm. Beyond one mean free path: " +
          (100 * C.b1).toFixed(1) + " % (e⁻¹ = 36.8 %); beyond three: " + (100 * C.b3).toFixed(1) + " % (e⁻³ = 5.0 %).";
      },
      panel: function (body, redraw) {
        slider(body, "Macroscopic cross section Σ", 0.05, 2, 0.05, st.sigma, function (v) { st.sigma = v; cache = null; redraw(); return v.toFixed(2) + " /cm"; });
        body.append(el("p", { "class": "w-note" }, "One uniform random number ξ per flight. The histogram is the draws; the curve is the exact density. Linear axes; zoom changes the distance range."));
      }
    });
  }

  // ─── Widget: standing on a surface (page 3) ────────────────────────────
  function roundoff(root) {
    var st = { b: 0, N: 20000 }, cache = null;
    function get() {
      if (!cache || cache.b !== st.b) {
        var r = rng(168), a = 0, t = 0, n = 0;
        for (var i = 0; i < st.N; i++) { var s = surfaceStep(st.b, r); if (s.signInside) a++; if (s.tokenInside) t++; if (s.nudgedInside) n++; }
        cache = { b: st.b, sign: a / st.N, token: t / st.N, nudge: n / st.N };
      }
      return cache;
    }
    return new Frame(root, {
      reset: noop, onSeen: noop, tick: still,
      draw: function (ctx, w, h, zoom) {
        var C = get(), c = colors(), L0 = 50, R0 = w - 12, T0 = 70, B0 = h - 52;
        var rows = [["sign of the surface equation at the point", C.sign, c.hot], ["the recorded side (sign of u · n)", C.token, c.acc], ["…after a 1e-9 cm nudge along the normal", C.nudge, c.acc]];
        var bh = (B0 - T0) / rows.length * 0.55, gap = (B0 - T0) / rows.length;
        ctx.font = "12px system-ui, sans-serif";
        rows.forEach(function (rw, i) {
          var y = T0 + i * gap + (gap - bh) / 2, wid = (R0 - L0) * rw[1] / (1 / Math.max(zoom, 1));
          ctx.fillStyle = "rgba(91, 99, 107, 0.15)"; ctx.fillRect(L0, y, R0 - L0, bh);
          ctx.fillStyle = rw[2]; ctx.fillRect(L0, y, Math.min(R0 - L0, wid), bh);
          ctx.fillStyle = c.fg; ctx.fillText(rw[0], L0, y - 4); ctx.fillText((100 * rw[1]).toFixed(1) + " % found inside", L0 + 6, y + bh / 2 + 4);
        });
        ctx.strokeStyle = c.mut; ctx.beginPath(); ctx.moveTo(L0, B0 + 4); ctx.lineTo(R0, B0 + 4); ctx.stroke();
        ctx.fillStyle = c.mut; ctx.fillText("0 %", L0 - 4, B0 + 20); ctx.fillText("100 % (the truth)", R0 - 100, B0 + 20);
        return "A neutron has just been stepped onto a unit sphere from outside, with impact parameter " + st.b.toFixed(4) + " of the radius. It is entering, so it is inside. Of " + st.N.toLocaleString() + " such steps, deciding by the sign of the equation finds it inside " + (100 * C.sign).toFixed(1) + " % of the time.";
      },
      panel: function (body, redraw) {
        slider(body, "Impact parameter (0 = head on, 1 = grazing)", 0, 0.99999, 0.00001, st.b, function (v) { st.b = v; cache = null; redraw(); return v.toFixed(5); });
        body.append(el("p", { "class": "w-note" }, "Doubles, as the transport code uses. The point is placed on the surface by the flight distance, so it sits there up to rounding, about 1e-16 relative: which side the equation reports is decided by that rounding. The three bars are three ways to decide the side. Zoom does not change anything here."));
      }
    });
  }

  // ─── Widget: random-number streams (page 4) ────────────────────────────
  function hex64(x) { return "0x" + x.toString(16).padStart(16, "0"); }
  // A toy "history": the sum of its first five draws, from the state `seed`.
  function toyHistory(seed) { var s = seed, tot = 0; for (var k = 0; k < 5; k++) { var p = prn(s); s = p.state; tot += p.x; } return tot; }
  // Which history each of N finishing slots serves with W workers: a fixed shuffle of 0..N-1 for W > 1, in order for W = 1.
  function schedule(N, W) {
    var order = [], i; for (i = 0; i < N; i++) order.push(i);
    if (W > 1) { var r = rng(W * 7919); for (i = N - 1; i > 0; i--) { var j = Math.floor(r() * (i + 1)), t = order[i]; order[i] = order[j]; order[j] = t; } }
    return order;
  }
  // The result of every history, (a) with a stream per history, whatever the schedule, and
  // (b) with ONE stream shared by all, drawn from in the order the schedule serves them.
  function ownStreams(N, seed, gen) {
    var g = futureSeed(BigInt(gen) * GEN_STRIDE, BigInt(seed)), out = new Array(N);
    for (var i = 0; i < N; i++) out[i] = toyHistory(futureSeed(BigInt(i) * DEFAULT_STRIDE, g));
    return out;
  }
  function sharedStream(N, seed, W) {
    var order = schedule(N, W), s = BigInt(seed), out = new Array(N);
    for (var i = 0; i < N; i++) { out[order[i]] = toyHistory(s); for (var k = 0; k < 5; k++) s = step(s); }
    return out;
  }
  function changed(a, b) { var n = 0; for (var i = 0; i < a.length; i++) if (a[i] !== b[i]) n++; return n; }
  L.transportMath.ownStreams = ownStreams; L.transportMath.sharedStream = sharedStream; L.transportMath.changed = changed; L.transportMath.toyHistory = toyHistory;

  function stream(root) {
    var st = { seed: 1, gen: 3, hist: 5, workers: 4, N: 1000 }, cache = null;
    function get() {
      var key = [st.seed, st.gen, st.workers].join();
      if (!cache || cache.key !== key) {
        // Per-history streams are laid out by (seed, generation, history) alone, so the schedule cannot enter: the same
        // vector comes back for any W. (It is computed once; the check recomputes it under every schedule.)
        var own = ownStreams(st.N, st.seed, st.gen), s1 = sharedStream(st.N, st.seed, 1), sW = sharedStream(st.N, st.seed, st.workers);
        cache = { key: key, own: own, shared1: s1, sharedW: sW, moved: changed(s1, sW) };
      }
      return cache;
    }
    return new Frame(root, {
      reset: noop, onSeen: noop, tick: still,
      draw: function (ctx, w, h) {
        var C = get(), c = colors(), y = 70, lh = 20, hs = historySeed(st.seed, st.gen, st.hist), p1 = prn(hs), p2 = prn(p1.state);
        ctx.font = "13px system-ui, sans-serif";
        var lines = [
          ["History " + st.hist + " of generation " + st.gen + " (master seed " + st.seed + ")", c.fg],
          ["starts at state " + hex64(hs), c.acc],
          ["first draw " + p1.x.toFixed(12) + ", second " + p2.x.toFixed(12), c.acc],
          ["", c.fg],
          ["Toy run: " + st.N + " histories, each the sum of 5 draws.", c.fg],
          ["Result of history " + st.hist + ":", c.fg],
          ["   own stream, any number of workers: " + C.own[st.hist].toFixed(9), c.acc],
          ["   one shared stream, 1 worker:       " + C.shared1[st.hist].toFixed(9), c.mut],
          ["   one shared stream, " + st.workers + " workers:      " + C.sharedW[st.hist].toFixed(9), C.sharedW[st.hist] === C.shared1[st.hist] ? c.mut : c.hot],
          ["", c.fg],
          ["Histories whose result changes when 1 worker becomes " + st.workers + ":", c.fg],
          ["   own streams: 0 of " + st.N, c.acc],
          ["   shared stream: " + C.moved + " of " + st.N, st.workers === 1 ? c.mut : c.hot]
        ];
        lines.forEach(function (ln, i) { ctx.fillStyle = ln[1]; ctx.fillText(ln[0], 14, y + i * lh); });
        return "With a stream per history a run is the same run however it is scheduled; with one shared stream, which history gets which numbers depends on who asks first.";
      },
      panel: function (body, redraw) {
        slider(body, "Master seed", 1, 99, 1, st.seed, function (v) { st.seed = v; cache = null; redraw(); return "" + v; });
        slider(body, "Generation", 0, 20, 1, st.gen, function (v) { st.gen = v; cache = null; redraw(); return "" + v; });
        slider(body, "History", 0, 999, 1, st.hist, function (v) { st.hist = v; redraw(); return "" + v; });
        slider(body, "Workers", 1, 16, 1, st.workers, function (v) { st.workers = v; cache = null; redraw(); return "" + v; });
        body.append(el("p", { "class": "w-note" }, "The generator and the jump-ahead are the transport code's (crates/petir/src/rng/lcg.rs), written here with 64-bit integers; check-transport.js compares them with the values pinned in the Rust crate's own tests. The toy run is not transport: it only shows what stream layout does to which history gets which numbers."));
      }
    });
  }

  // ─── Widget: nuclide and reaction at a collision (page 5) ──────────────
  function ladder(root) {
    var D = window.COLLISION_DATA, st = { e: "1000000", xi1: 0.5, xi2: 0.5 };
    var NAMES = { fission: "fission", capture: "capture", inelastic: "inelastic", n2n: "(n,2n)", other: "other", elastic: "elastic" };
    var COLS = { fission: "#b85c38", capture: "#6b4c9a", inelastic: "#c9a227", n2n: "#8a8a2a", other: "#888", elastic: "#1f6f78" };
    function shares() {
      var E = D.energies[st.e], keys = Object.keys(E.nuclides), tots = keys.map(function (k) { return E.nuclides[k].n * E.nuclides[k].total; });
      return { keys: keys, tots: tots, E: E };
    }
    return new Frame(root, {
      reset: noop, onSeen: noop, tick: still,
      draw: function (ctx, w, h, zoom) {
        var S = shares(), c = colors(), L0 = 14, R0 = w - 14, y = 74, bh = 34, sum = S.tots.reduce(function (a, b) { return a + b; }, 0);
        ctx.font = "12px system-ui, sans-serif";
        ctx.fillStyle = c.fg; ctx.fillText("1. which nuclide: ξ₁ against N·σ_t of each (this material at " + (+st.e / 1e6) + " MeV)", L0, y - 8);
        var x = L0, cols = ["#1f6f78", "#b85c38", "#6b4c9a"], pick = nuclidePick(S.tots, st.xi1), i;
        for (i = 0; i < S.keys.length; i++) {
          var wd = (R0 - L0) * S.tots[i] / sum;
          ctx.fillStyle = cols[i % 3]; ctx.globalAlpha = i === pick ? 1 : 0.35; ctx.fillRect(x, y, wd, bh); ctx.globalAlpha = 1;
          ctx.fillStyle = c.fg; ctx.fillText(S.keys[i] + " " + (100 * S.tots[i] / sum).toFixed(1) + " %", x + 3, y + bh + 14 + (i % 2) * 14);
          x += wd;
        }
        ctx.strokeStyle = c.fg; ctx.lineWidth = 2; var mx = L0 + (R0 - L0) * st.xi1; ctx.beginPath(); ctx.moveTo(mx, y - 4); ctx.lineTo(mx, y + bh + 4); ctx.stroke();
        var nuc = S.E.nuclides[S.keys[pick]], parts = { fission: nuc.fission, capture: nuc.capture, inelastic: nuc.inelastic, n2n: nuc.n2n, other: Math.max(0, nuc.other), elastic: nuc.elastic };
        var tot = 0, k; for (k in parts) tot += parts[k];
        var y2 = y + bh + 70;
        ctx.fillStyle = c.fg; ctx.fillText("2. which reaction for " + S.keys[pick] + ": ξ₂ against the partial cross sections, in the code's order", L0, y2 - 8);
        x = L0; var which = ladderPick(parts, st.xi2), order = ["fission", "capture", "inelastic", "n2n", "other", "elastic"];
        order.forEach(function (key) {
          var wd = (R0 - L0) * parts[key] / tot;
          if (wd <= 0) return;
          ctx.fillStyle = COLS[key]; ctx.globalAlpha = key === which ? 1 : 0.35; ctx.fillRect(x, y2, wd, bh); ctx.globalAlpha = 1;
          if (wd > 38) { ctx.fillStyle = "#fff"; ctx.fillText(NAMES[key], x + 3, y2 + 20); }
          x += wd;
        });
        var mx2 = L0 + (R0 - L0) * st.xi2; ctx.strokeStyle = c.fg; ctx.beginPath(); ctx.moveTo(mx2, y2 - 4); ctx.lineTo(mx2, y2 + bh + 4); ctx.stroke();
        var yy = y2 + bh + 22;
        order.forEach(function (key) { if (parts[key] > 0) { ctx.fillStyle = COLS[key]; ctx.fillText(NAMES[key] + " " + parts[key].toPrecision(3) + " b", L0, yy); yy += 14; } });
        return "ξ₁ = " + st.xi1.toFixed(3) + " picks " + S.keys[pick] + "; ξ₂ = " + st.xi2.toFixed(3) + " picks " + NAMES[which] + ". Σ_t = " + S.E.sigma_t.toFixed(4) + " per cm, mean free path " + (1 / S.E.sigma_t).toFixed(2) + " cm.";
      },
      panel: function (body, redraw) {
        buttons(body, "Neutron energy", [["100000", "0.1 MeV"], ["1000000", "1 MeV"], ["2000000", "2 MeV"]], st.e, function (v) { st.e = v; redraw(); });
        slider(body, "ξ₁ (which nuclide)", 0, 0.9999, 0.0001, st.xi1, function (v) { st.xi1 = v; redraw(); return v.toFixed(4); });
        slider(body, "ξ₂ (which reaction)", 0, 0.9999, 0.0001, st.xi2, function (v) { st.xi2 = v; redraw(); return v.toFixed(4); });
        var b = el("button", { type: "button", "class": "w-opt wide" }, "Draw both at random");
        b.onclick = function () { st.xi1 = Math.random() * 0.9999; st.xi2 = Math.random() * 0.9999; redraw(); };
        body.append(b);
        body.append(el("p", { "class": "w-note" }, "The widths are our port's: Godiva's material and the microscopic channels of Nuclide::xs_at_energy, from collision-data.js. In the code the second draw is made against the chosen nuclide's own total, ladder order fission, capture, inelastic, (n,2n), the rest, elastic last."));
      }
    });
  }

  // ─── Widget: delta tracking against surface tracking (page 8) ──────────
  // A one-dimensional sketch of the algorithm of physics/delta_tracking
  // (flight.rs): the real code, on real geometry, is the demo embedded on the
  // page. This shows only why the two methods agree.
  function woodcock(root) {
    var st = { sA: 0.2, sB: 2.0, a: 4, Len: 10, N: 20000 }, cache = null;
    function get() {
      var key = [st.sA, st.sB, st.a, st.Len].join();
      if (!cache || cache.key !== key) {
        var rs = rng(1), rd = rng(2), res = { key: key, s: { A: 0, B: 0, leak: 0, flights: 0, boundary: 0 }, d: { A: 0, B: 0, leak: 0, flights: 0, virtual: 0 }, hs: new Float64Array(40), hd: new Float64Array(40) }, i, o;
        for (i = 0; i < st.N; i++) {
          o = surfaceTrack(st.sA, st.sB, st.a, st.Len, rs); res.s[o.where]++; res.s.flights += o.flights; res.s.boundary += o.boundary; if (o.where !== "leak") res.hs[Math.min(39, Math.floor(o.x / st.Len * 40))]++;
          o = deltaTrack(st.sA, st.sB, st.a, st.Len, rd); res.d[o.where]++; res.d.flights += o.flights; res.d.virtual += o.virtual; if (o.where !== "leak") res.hd[Math.min(39, Math.floor(o.x / st.Len * 40))]++;
        }
        cache = res;
      }
      return cache;
    }
    return new Frame(root, {
      reset: noop, onSeen: noop, tick: still,
      draw: function (ctx, w, h, zoom) {
        var C = get(), c = colors(), L0 = 50, R0 = w - 12, T0 = 96, B0 = h - 50, nb = 40, i, top = 0;
        for (i = 0; i < nb; i++) top = Math.max(top, C.hs[i], C.hd[i]); top = top / st.N * 1.15;
        var bw = (R0 - L0) / nb;
        function Y(v) { return B0 - v / top * (B0 - T0); }
        ctx.fillStyle = "rgba(31, 111, 120, 0.45)";
        for (i = 0; i < nb; i++) { var v = C.hs[i] / st.N; ctx.fillRect(L0 + i * bw, Y(v), bw / 2 - 1, B0 - Y(v)); }
        ctx.fillStyle = "rgba(184, 92, 56, 0.65)";
        for (i = 0; i < nb; i++) { var u = C.hd[i] / st.N; ctx.fillRect(L0 + i * bw + bw / 2, Y(u), bw / 2 - 1, B0 - Y(u)); }
        // The materials, drawn as a strip above the plot.
        var xa = L0 + (R0 - L0) * st.a / st.Len;
        ctx.fillStyle = "rgba(91, 99, 107, 0.25)"; ctx.fillRect(L0, T0 - 26, xa - L0, 16);
        ctx.fillStyle = "rgba(91, 99, 107, 0.55)"; ctx.fillRect(xa, T0 - 26, R0 - xa, 16);
        ctx.font = "12px system-ui, sans-serif"; ctx.fillStyle = c.fg; ctx.fillText("A: Σ = " + st.sA.toFixed(2), L0 + 4, T0 - 14); ctx.fillText("B: Σ = " + st.sB.toFixed(2), xa + 4, T0 - 14);
        yAxis(ctx, L0, T0, B0, 0, top, c.mut);
        xAxis(ctx, L0, R0, B0, 0, st.Len, "where the first real collision happens (cm)", c.mut);
        ctx.fillStyle = c.acc; ctx.fillText("blue: surface tracking", L0 + 8, 62); ctx.fillStyle = c.hot; ctx.fillText("orange: delta tracking", L0 + 8, 78);
        var pA = 1 - Math.exp(-st.sA * st.a), pLeak = Math.exp(-st.sA * st.a - st.sB * (st.Len - st.a));
        return "In A: surface " + (100 * C.s.A / st.N).toFixed(1) + " %, delta " + (100 * C.d.A / st.N).toFixed(1) + " % (exact " + (100 * pA).toFixed(1) + " %). Leak: " + (100 * C.s.leak / st.N).toFixed(2) + " % and " + (100 * C.d.leak / st.N).toFixed(2) + " % (exact " + (100 * pLeak).toFixed(2) +
          " %). Per neutron: surface tracking " + (C.s.flights / st.N).toFixed(2) + " flights, " + (C.s.boundary / st.N).toFixed(2) + " boundary stops; delta tracking " + (C.d.flights / st.N).toFixed(2) + " flights, " + (C.d.virtual / st.N).toFixed(2) + " of them virtual.";
      },
      panel: function (body, redraw) {
        slider(body, "Σ in region A (per cm)", 0.05, 4, 0.05, st.sA, function (v) { st.sA = v; cache = null; redraw(); return v.toFixed(2); });
        slider(body, "Σ in region B (per cm)", 0.05, 4, 0.05, st.sB, function (v) { st.sB = v; cache = null; redraw(); return v.toFixed(2); });
        slider(body, "Boundary position (cm)", 1, 9, 0.5, st.a, function (v) { st.a = v; cache = null; redraw(); return v.toFixed(1) + " cm"; });
        body.append(el("p", { "class": "w-note" }, "20 000 neutrons each way, born at 0 flying right, vacuum at 10 cm. Delta tracking flies on the larger Σ and accepts a real collision with probability Σ(x)/Σ_majorant. Both histograms estimate the same distribution; the exact values are closed forms."));
      }
    });
  }

  // ─── Widget: banking and resampling fission sites (page 9) ─────────────
  function comb(root) {
    var st = { nu: 2.5, k: 1.0, total: 10, n: 8, mode: "comb" };
    return new Frame(root, {
      reset: noop, onSeen: noop, tick: still,
      draw: function (ctx, w, h) {
        var c = colors(), r = rng(31), L0 = 14, R0 = w - 14, T0 = 76, i, trials = 4000;
        // Fission yield: counts of n over many fissions.
        var cnt = {}, mean = 0;
        for (i = 0; i < trials; i++) { var nn = numNeutrons(st.nu, st.k, r()); cnt[nn] = (cnt[nn] || 0) + 1; mean += nn; }
        mean /= trials;
        ctx.font = "12px system-ui, sans-serif"; ctx.fillStyle = c.fg;
        ctx.fillText("sites banked per fission, ν̄/k = " + (st.nu / st.k).toFixed(3) + " (mean of " + trials + " draws: " + mean.toFixed(3) + ")", L0, T0 - 12);
        var maxN = 0; for (var kk in cnt) maxN = Math.max(maxN, +kk);
        var bw = Math.min(40, (R0 - L0) / (maxN + 2)), hh = 54;
        for (i = 0; i <= maxN; i++) {
          var f = (cnt[i] || 0) / trials; ctx.fillStyle = "rgba(31, 111, 120, 0.55)"; ctx.fillRect(L0 + i * bw, T0 + hh - f * hh * 1.6, bw - 4, f * hh * 1.6);
          ctx.fillStyle = c.fg; ctx.fillText(i + ": " + (100 * f).toFixed(0) + " %", L0 + i * bw, T0 + hh + 14);
        }
        // Resampling.
        var y2 = T0 + hh + 54;
        var mult = st.mode === "comb" ? combMultiplicities(st.total, st.n, 0.37) : replacementMultiplicities(st.total, st.n, rng(77));
        ctx.fillStyle = c.fg; ctx.fillText("resampling " + st.total + " banked sites to " + st.n + " source sites (" + (st.mode === "comb" ? "uniform comb" : "with replacement") + "): copies of each site", L0, y2 - 10);
        var bw2 = Math.min(34, (R0 - L0) / st.total);
        for (i = 0; i < st.total; i++) {
          var m = mult[i], x0 = L0 + i * bw2;
          ctx.fillStyle = m === 0 ? c.hot : "rgba(31, 111, 120, 0.55)"; ctx.fillRect(x0, y2 + 60 - m * 16, bw2 - 4, Math.max(m * 16, 2));
          ctx.fillStyle = c.fg; ctx.fillText("" + m, x0 + 4, y2 + 76);
        }
        var lo = Math.floor(st.n / st.total), hi = Math.ceil(st.n / st.total), bad = mult.filter(function (m) { return m < lo || m > hi; }).length;
        return (st.mode === "comb" ? "Every site is taken " + lo + (hi !== lo ? " or " + hi : "") + " times, " + bad + " outside that." : "With replacement, " + bad + " of " + st.total + " sites are taken fewer than " + lo + " or more than " + hi + " times (red: dropped).");
      },
      panel: function (body, redraw) {
        slider(body, "ν̄ (neutrons per fission)", 1.5, 3.5, 0.05, st.nu, function (v) { st.nu = v; redraw(); return v.toFixed(2); });
        slider(body, "k (running)", 0.5, 1.5, 0.01, st.k, function (v) { st.k = v; redraw(); return v.toFixed(2); });
        slider(body, "Banked sites", 4, 14, 1, st.total, function (v) { st.total = v; redraw(); return "" + v; });
        slider(body, "Source sites wanted", 4, 14, 1, st.n, function (v) { st.n = v; redraw(); return "" + v; });
        buttons(body, "Resampling", [["comb", "Uniform comb"], ["rep", "With replacement"]], st.mode, function (v) { st.mode = v; redraw(); });
        body.append(el("p", { "class": "w-note" }, "numNeutrons and the comb are ports of sample_num_neutrons and comb_resample (outram-mc-libs/src/physics/fission.rs); the comb is shown for the fixed random offset 0.37. Counts are small on purpose: the real banks hold thousands."));
      }
    });
  }

  // ─── Widget: correlated generations (page 10) ──────────────────────────
  // An illustration, not a measurement of our code: generation-to-generation
  // correlation is modelled by an AR(1) sequence with a correlation the reader
  // sets. What it shows is the arithmetic of ignoring it.
  function correlated(root) {
    var st = { rho: 0.5, n: 120, reps: 600 }, cache = null;
    function get() {
      var key = [st.rho, st.n].join();
      if (!cache || cache.key !== key) {
        var r = rng(10), means = [], naive = 0, i;
        for (i = 0; i < st.reps; i++) { var x = ar1(st.n, st.rho, r), ms = meanStderr(x); means.push(ms.mean); naive += ms.se; }
        naive /= st.reps;
        var mm = meanStderr(means), sd = mm.se * Math.sqrt(st.reps);
        cache = { key: key, means: means, naive: naive, truth: sd };
      }
      return cache;
    }
    return new Frame(root, {
      reset: noop, onSeen: noop, tick: still,
      draw: function (ctx, w, h, zoom) {
        var C = get(), c = colors(), L0 = 50, R0 = w - 12, T0 = 76, B0 = h - 52, nb = 30, i;
        var span = 4 * C.truth / zoom, lo = -span, hi = span, hist = new Float64Array(nb);
        C.means.forEach(function (m) { var k = Math.floor((m - lo) / (hi - lo) * nb); if (k >= 0 && k < nb) hist[k]++; });
        var top = 0; for (i = 0; i < nb; i++) top = Math.max(top, hist[i]); top *= 1.2;
        var bw = (R0 - L0) / nb;
        ctx.fillStyle = "rgba(31, 111, 120, 0.45)";
        for (i = 0; i < nb; i++) { var hv = hist[i] / top * (B0 - T0); ctx.fillRect(L0 + i * bw + 1, B0 - hv, bw - 2, hv); }
        function X(v) { return L0 + (v - lo) / (hi - lo) * (R0 - L0); }
        [[C.naive, c.hot, "assumes independent generations"], [C.truth, c.acc, "the actual scatter of the mean"]].forEach(function (b, k) {
          ctx.strokeStyle = b[1]; ctx.lineWidth = 3; ctx.beginPath(); ctx.moveTo(X(-b[0]), T0 + 8 + k * 14); ctx.lineTo(X(b[0]), T0 + 8 + k * 14); ctx.stroke();
          ctx.fillStyle = b[1]; ctx.font = "12px system-ui, sans-serif"; ctx.fillText("±1σ " + b[2], L0 + 4, T0 - 12 + k * 14 - 0);
        });
        xAxis(ctx, L0, R0, B0, lo, hi, "mean of one run (arbitrary units)", c.mut);
        return "Lag-one correlation " + st.rho.toFixed(2) + ", " + st.n + " generations per run, " + st.reps + " runs. The error bar a run quotes, assuming independence, is on average " + C.naive.toFixed(3) + "; the real scatter of run means is " + C.truth.toFixed(3) + ", a factor " + (C.truth / C.naive).toFixed(2) + " larger (large-n limit " + Math.sqrt((1 + st.rho) / (1 - st.rho)).toFixed(2) + ").";
      },
      panel: function (body, redraw) {
        slider(body, "Correlation between neighbouring generations", 0, 0.9, 0.05, st.rho, function (v) { st.rho = v; cache = null; redraw(); return v.toFixed(2); });
        slider(body, "Generations per run", 40, 400, 20, st.n, function (v) { st.n = v; cache = null; redraw(); return "" + v; });
        body.append(el("p", { "class": "w-note" }, "A model of the arithmetic, not a measurement of OutramMC: its inter-generation correlation has not been measured (the crate's statistics gates are recorded as not yet measured). Linear axes; zoom changes the range."));
      }
    });
  }

  L.widget("flight", flight);
  L.widget("roundoff", roundoff);
  L.widget("stream", stream);
  L.widget("ladder", ladder);
  L.widget("woodcock", woodcock);
  L.widget("comb", comb);
  L.widget("correlated", correlated);
})();
