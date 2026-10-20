// The physics of Fission products lesson 2 (split atoms): the yields at any
// incident energy, the fragment pairs drawn from them, and the
// peak-to-valley ratio. Pure functions over yields-data.js (generated from
// ENDF/B-VIII.0 through fission-yields-data), no drawing, so node can check
// them (check-fission.js).
//
// Two things here are this lesson's, not the evaluation's, and the page says
// so:
// 1. Between the tabulated incident energies each Y(Z, A) is interpolated
//    linearly in E, as fission-yields-data does (check-fission.js compares
//    the result with the crate's own interpolation). That is interpolation,
//    not physics.
// 2. The pair. The independent yields give each product's probability, not
//    which two products come from the same fission. One fission here draws
//    product 1 from Y(Z, A)/2, then its partner from the yields of the
//    complementary element Z_CN - Z1 with A2 <= A_CN - A1, in proportion to
//    their yields; the prompt neutrons are what is left, A_CN - A1 - A2.
//    Because the evaluation's element yields are (nearly) symmetric under
//    Z -> Z_CN - Z, both products then follow Y(A); check-fission.js
//    measures how closely.
(function () {
  "use strict";
  var W = typeof window !== "undefined" ? window : global;
  var D = function () { return W.YIELDS_DATA; };
  var E = function () { return W.FPEnergy; };

  // The tabulated incident energies of a parent [eV].
  function energies(name) { return D()[name].tables.map(function (t) { return t.e; }); }

  // Y(Z, A) at energy e [eV] as { z: { a0, y: [...] } }, linear in E between
  // the bracketing tables, clamped to the tabulated range.
  var cache = {};
  function yieldsAt(name, e) {
    var key = name + "|" + e;
    if (cache[key]) return cache[key];
    var P = D()[name], b = E().bracket(energies(name), e), lo = P.tables[b[0]].zy, hi = P.tables[b[1]].zy, x = b[2];
    var out = {}, zs = {};
    Object.keys(lo).concat(Object.keys(hi)).forEach(function (z) { zs[z] = 1; });
    Object.keys(zs).forEach(function (z) {
      var r0 = lo[z] || [999, 0], r1 = hi[z] || [999, 0];
      var a0 = Math.min(r0[0], r1[0]), a1 = Math.max(r0[0] + r0.length - 2, r1[0] + r1.length - 2), y = [];
      for (var a = a0; a <= a1; a++) {
        var v0 = a >= r0[0] && a - r0[0] + 1 < r0.length ? r0[a - r0[0] + 1] : 0;
        var v1 = a >= r1[0] && a - r1[0] + 1 < r1.length ? r1[a - r1[0] + 1] : 0;
        y.push((1 - x) * v0 + x * v1);
      }
      out[z] = { a0: a0, y: y };
    });
    cache = {}; // keep one energy only: the slider moves through many
    return (cache[key] = out);
  }

  // Y(A) = sum over Z, as { a0, y: [...] }.
  function massYields(zy) {
    var lo = Infinity, hi = -Infinity;
    for (var z in zy) { lo = Math.min(lo, zy[z].a0); hi = Math.max(hi, zy[z].a0 + zy[z].y.length - 1); }
    var y = new Array(hi - lo + 1).fill(0);
    for (z in zy) zy[z].y.forEach(function (v, k) { y[zy[z].a0 + k - lo] += v; });
    return { a0: lo, y: y };
  }

  // The peak-to-valley numbers of Y(A), as the generator's peak_valley:
  // the highest Y on each side of a_sym, the lowest Y between them, and the
  // mean of the two peaks over the valley.
  function peakValley(m, aSym) {
    var L = null, H = null, V = null, k, a;
    for (k = 0; k < m.y.length; k++) {
      a = m.a0 + k;
      if (a < aSym && (!L || m.y[k] > L[1])) L = [a, m.y[k]];
      if (a > aSym && (!H || m.y[k] > H[1])) H = [a, m.y[k]];
    }
    for (a = L[0]; a <= H[0]; a++) { k = a - m.a0; if (!V || m.y[k] < V[1]) V = [a, m.y[k]]; }
    return { light: L, heavy: H, valley: V, ratio: 0.5 * (L[1] + H[1]) / V[1] };
  }

  // The pair sampler at one energy: every product with its cumulative
  // probability, and per element its isotopes.
  function sampler(name, e) {
    var P = D()[name], zy = yieldsAt(name, e), list = [], cum = 0, total = 0;
    for (var z in zy) zy[z].y.forEach(function (v) { total += v; });
    for (z in zy) zy[z].y.forEach(function (v, k) {
      if (v > 0) { cum += v / total; list.push({ z: +z, a: zy[z].a0 + k, c: cum }); }
    });
    return { name: name, e: e, zcn: P.z, acn: P.a + 1, zy: zy, list: list };
  }
  // One fission: product 1, its partner, and the neutrons left over.
  function samplePair(S, r) {
    var x = r(), lo = 0, hi = S.list.length - 1;
    while (lo < hi) { var mid = (lo + hi) >> 1; if (S.list[mid].c < x) lo = mid + 1; else hi = mid; }
    var p1 = S.list[lo], z2 = S.zcn - p1.z, row = S.zy[z2], amax = S.acn - p1.a;
    if (!row) return null;
    var tot = 0, k;
    for (k = 0; k < row.y.length && row.a0 + k <= amax; k++) tot += row.y[k];
    if (!(tot > 0)) return null;
    var t = r() * tot, a2 = row.a0;
    for (k = 0; k < row.y.length && row.a0 + k <= amax; k++) { t -= row.y[k]; a2 = row.a0 + k; if (t <= 0) break; }
    return { z1: p1.z, a1: p1.a, z2: z2, a2: a2, nu: S.acn - p1.a - a2 };
  }
  // The exact distribution of A that samplePair produces (both products),
  // for the check: sum over product 1 of P(1) [delta(A1) + P(A2 | 1)].
  function constructionMass(S) {
    var out = {}, prev = 0, lost = 0;
    S.list.forEach(function (p) {
      var w = p.c - prev; prev = p.c;
      var row = S.zy[S.zcn - p.z], amax = S.acn - p.a, tot = 0, k;
      if (!row) { lost += w; return; }
      for (k = 0; k < row.y.length && row.a0 + k <= amax; k++) tot += row.y[k];
      if (!(tot > 0)) { lost += w; return; }
      out[p.a] = (out[p.a] || 0) + w;
      for (k = 0; k < row.y.length && row.a0 + k <= amax; k++) out[row.a0 + k] = (out[row.a0 + k] || 0) + w * row.y[k] / tot;
    });
    return { mass: out, lost: lost };
  }

  // A small seeded generator (mulberry32), as fission-physics.js uses.
  function rng(seed) {
    var a = seed >>> 0;
    return function () {
      a = (a + 0x6D2B79F5) >>> 0;
      var t = a;
      t = Math.imul(t ^ (t >>> 15), t | 1);
      t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }

  // Two measures of the construction for the page's advanced section, U-235
  // thermal by default: the largest |construction(A) - Y(A)| over the highest
  // Y(A), and the largest |Y(Z) - Y(Z_CN - Z)| over the element yields.
  function constructionDeviation(name, e) {
    var Sm = sampler(name, e), c = constructionMass(Sm), m = massYields(Sm.zy), peak = 0, dev = 0;
    m.y.forEach(function (v, k) { peak = Math.max(peak, v); dev = Math.max(dev, Math.abs((c.mass[m.a0 + k] || 0) - v)); });
    return dev / peak;
  }
  function chargeAsymmetry(name, e) {
    var zy = yieldsAt(name, e), zcn = D()[name].z, el = {}, worst = 0;
    for (var z in zy) el[z] = zy[z].y.reduce(function (s, v) { return s + v; }, 0);
    for (z in el) worst = Math.max(worst, Math.abs(el[z] - (el[zcn - z] || 0)));
    return worst;
  }

  W.SplitPhysics = { constructionDeviation: constructionDeviation, chargeAsymmetry: chargeAsymmetry, energies: energies, yieldsAt: yieldsAt, massYields: massYields, peakValley: peakValley, sampler: sampler, samplePair: samplePair, constructionMass: constructionMass, rng: rng };
})();
