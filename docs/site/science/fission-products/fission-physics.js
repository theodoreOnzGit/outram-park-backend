// The physics of Fission products lesson 1: one fission, sampled and
// conserved event by event. Pure functions, no drawing, so node can check
// them (check-fission.js). Every number comes from fission-data.js
// (generated from ENDF/B-VIII.0 and NNDC NuDat; see that file's header),
// from the constants cited below, or from fission-pending.js (the values a
// source is still being found for).
//
// Kinematics are special-relativistic in 2D, in MeV with c = 1: every
// particle is [E, px, py] with its rest mass, every break-up is an exact
// two-body decay in its own rest frame followed by a Lorentz boost, so the
// event conserves energy and momentum to round-off. Rest masses are atomic
// masses A u + Δ from the NuDat table (Δ rounded there to 3 significant
// figures), with u = m_n − Δ_n so the neutron's own mass is CODATA 2018's.
//
// The deliberate liberties (each also listed on the page):
// 1. The pair: fragment 1 drawn from the independent yields; its partner
//    from the yields of the complementary element (Z_CN − Z1, or − 2 more in
//    a ternary event) with A2 ≤ A_CN − A1, weighted by its yield; ν is what
//    is left, A_CN − A1 − A2. A pair that cannot pay for its neutrons is
//    not drawn. The yield library has no pair correlation, so this is a
//    construction, not data.
// 2. TKE is the evaluated average EFR(E) of MF=1/MT=458 for every event:
//    the split-to-split spread of TKE is not modelled.
// 3. The excitation energy is shared in proportion to the primary fragments'
//    mass numbers (equal temperatures in a Fermi gas whose level-density
//    parameter grows with A): an assumption.
// 4. Each prompt neutron's energy in its fragment's frame is drawn from the
//    evaporation shape ε exp(−ε/T), with T set so the mean laboratory energy
//    of the prompt neutrons is the evaluation's ENP/ν̄_p (MT=458 over
//    MT=456): ⟨E_lab⟩ = 2T + E_frag/A_frag for isotropic emission.
// 5. What is left of each fragment's excitation after its neutrons leaves as
//    one photon (the split among several photons is not modelled).
// 6. Delayed neutrons: a precursor's NuDat beta-delayed-neutron branching
//    decides whether one is emitted; its energy is drawn from the ENDF
//    MF=5/MT=455 group whose decay constant is nearest the precursor's
//    ln 2 / t½ (an assignment rule of this lesson, not of the evaluation).
(function () {
  "use strict";
  var W = typeof window !== "undefined" ? window : global;
  var D = function () { return W.FISSION_DATA; };
  var PENDING = function () { return W.FISSION_PENDING || {}; };

  // CODATA 2018 (Tiesinga et al., 2021): m_n, c and e, as lesson-kit.js.
  var MN_KG = 1.67492749804e-27, C = 299792458, EV = 1.602176634e-19;
  var MN = MN_KG * C * C / EV / 1e6;                 // neutron rest energy [MeV]
  var HBAR_EVS = 6.62607015e-34 / (2 * Math.PI) / EV; // ħ [eV s]
  // Sourced constants (references on the page):
  var R0_FM = 1.2;        // fm, R = r0 A^(1/3): OpenStax University Physics 3, §10.1, p. 436
  var PROMPT_BOUND_S = 1e-13; // s, "over 99 %" of fission neutrons within it: DOE-HDBK-1019/1-93 v1, NP-02 p. 29

  // ── Masses ────────────────────────────────────────────────────────────
  function deltaKeV(z, a) {
    var row = D().masses[String(z)];
    if (!row) return NaN;
    var v = row[a - row[0] + 1];
    return v === null || v === undefined ? NaN : v;
  }
  function u() { return MN - deltaKeV(0, 1) / 1000; }
  // Atomic rest energy [MeV] of the ground state of (Z, A); the neutron is (0, 1).
  function mass(z, a) {
    if (z === 0 && a === 1) return MN;
    return a * u() + deltaKeV(z, a) / 1000;
  }
  // Neutron separation energy of (Z, A) [MeV], from the same table.
  function sn(z, a) { return mass(z, a - 1) + MN - mass(z, a); }
  // The beta-stable isobar at A: lowest mass excess among the table's isobars.
  var stableCache = {};
  function stableZ(a) {
    if (stableCache[a] !== undefined) return stableCache[a];
    var best = null, bz = null, M = D().masses;
    for (var k in M) {
      var z = +k; if (z < 1) continue;
      var d = deltaKeV(z, a);
      if (!isNaN(d) && (best === null || d < best)) { best = d; bz = z; }
    }
    return (stableCache[a] = bz);
  }

  // ── Interpolation of the evaluated tables ──────────────────────────────
  function linlin(xs, ys, x) {
    if (x <= xs[0]) return ys[0];
    var n = xs.length;
    if (x >= xs[n - 1]) return ys[n - 1];
    var k = 1; while (xs[k] < x) k++;
    return ys[k - 1] + (ys[k] - ys[k - 1]) * (x - xs[k - 1]) / (xs[k] - xs[k - 1]);
  }
  // MT=458 component ifc (1 EFR … 9 ET) at incident energy e [eV], in MeV.
  function mt458(P, ifc, e) {
    var t = P.mt458.tab[String(ifc)];
    return (t ? linlin(t[0], t[1], e) : P.mt458.thermal[ifc - 1][0]) / 1e6;
  }

  // ── Kinematics: [E, px, py] with c = 1 ──────────────────────────────────
  function vec(m, px, py) { return [Math.sqrt(m * m + px * px + py * py), px, py]; }
  function minv(p) { return Math.sqrt(Math.max(p[0] * p[0] - p[1] * p[1] - p[2] * p[2], 0)); }
  function add(a, b) { return [a[0] + b[0], a[1] + b[1], a[2] + b[2]]; }
  // Lorentz boost of p by velocity (bx, by).
  // Lorentz boost into the lab of p, given in the rest frame of a body moving
  // with velocity (bx, by): E' = γ(E + β·p), p' = p + [(γ−1)(β·p)/β² + γE] β.
  function boost(p, bx, by) {
    var b2 = bx * bx + by * by;
    if (b2 === 0) return p.slice();
    var g = 1 / Math.sqrt(1 - b2), bp = bx * p[1] + by * p[2];
    var c = (g - 1) * bp / b2 + g * p[0];
    return [g * (p[0] + bp), p[1] + c * bx, p[2] + c * by];
  }
  // Two-body decay of mass M at rest into m1 (along angle th) and m2.
  function twoBody(M, m1, m2, th) {
    var s = (M * M - (m1 + m2) * (m1 + m2)) * (M * M - (m1 - m2) * (m1 - m2));
    var p = Math.sqrt(Math.max(s, 0)) / (2 * M);
    var cx = Math.cos(th), cy = Math.sin(th);
    return [vec(m1, p * cx, p * cy), vec(m2, -p * cx, -p * cy)];
  }
  function velocity(p) { return [p[1] / p[0], p[2] / p[0]]; }
  function kinetic(p, m) { return p[0] - m; }

  // ── The random stream: mulberry32, seeded per event (reproducible) ─────
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

  // ── The compound nucleus ───────────────────────────────────────────────
  function initial(P, e) {
    var Tn = e / 1e6, mt = mass(P.z, P.a);
    var pn = Math.sqrt(Tn * Tn + 2 * Tn * MN);
    var tot = [Tn + MN + mt, pn, 0];
    var Mstar = minv(tot), mcn = mass(P.z, P.a + 1);
    return { tot: tot, Mstar: Mstar, mcn: mcn, Ex: Mstar - mcn, Sn: mt + MN - mcn, pn: pn, Tn: Tn };
  }

  // The excitation split of a pair: ν1 + ν2 = ν, shares ∝ primary A.
  // Returns null when no split pays for every fragment's neutrons.
  function split(Mstar, tke, z1, a1, z2, a2, nu, ternary) {
    var best = null, mal = ternary ? mass(2, 4) : 0;
    for (var n1 = 0; n1 <= nu; n1++) {
      var n2 = nu - n1, A1 = a1 + n1, A2 = a2 + n2;
      var M1 = mass(z1, A1), M2 = mass(z2, A2);
      var exTot = Mstar - mal - M1 - M2 - tke;
      if (!(exTot >= 0)) continue;
      var e1 = exTot * A1 / (A1 + A2), e2 = exTot - e1;
      var b1 = mass(z1, a1) + n1 * MN - M1, b2 = mass(z2, a2) + n2 * MN - M2;
      var L = Math.min(e1 - b1, e2 - b2);
      if (!(L >= 0)) continue;
      if (!best || L > best.L) best = { L: L, nu1: n1, nu2: n2, ex1: e1, ex2: e2, M1: M1, M2: M2 };
    }
    return best;
  }

  // Every pair the construction can draw, with its probability (liberty 1).
  var pairCache = {};
  function pairs(name, ie, ternary) {
    var key = name + "|" + ie + "|" + (ternary ? 3 : 2);
    if (pairCache[key]) return pairCache[key];
    var P = D().parents[name], Y = P.yields[ie], e = Y.e;
    var I = initial(P, e), tke = mt458(P, 1, e);
    var zcn = P.z, acn = P.a + 1, dz = ternary ? 2 : 0, da = ternary ? 4 : 0;
    var byZ = {};
    for (var i = 0; i < Y.z.length; i++) (byZ[Y.z[i]] = byZ[Y.z[i]] || []).push(i);
    var list = [], wsum = 0;
    for (i = 0; i < Y.z.length; i++) {
      var z1 = Y.z[i], a1 = Y.a[i], z2 = zcn - z1 - dz, cands = byZ[z2] || [], ok = [], cw = 0;
      for (var j = 0; j < cands.length; j++) {
        var k = cands[j], a2 = Y.a[k], nu = acn - da - a1 - a2;
        if (nu < 0) continue;
        var s = split(I.Mstar, tke, z1, a1, z2, a2, nu, ternary);
        if (!s) continue;
        ok.push([k, s]); cw += Y.y[k];
      }
      if (!ok.length) continue;
      for (j = 0; j < ok.length; j++) {
        var pr = Y.y[i] * Y.y[ok[j][0]] / cw;
        list.push({ z1: z1, a1: a1, z2: z2, a2: Y.a[ok[j][0]], nu: acn - da - a1 - Y.a[ok[j][0]], s: ok[j][1], p: pr });
        wsum += pr;
      }
    }
    var cum = 0;
    list.forEach(function (x) { x.p /= wsum; cum += x.p; x.c = cum; });
    var mean = list.reduce(function (s, x) { return s + x.p * x.nu; }, 0);
    return (pairCache[key] = { list: list, meanNu: mean, tke: tke, I: I });
  }

  // Draw ε from ε exp(−ε/T) on [0, emax].
  function evaporation(T, emax, r) {
    if (!(emax > 0)) return 0;
    for (var t = 0; t < 64; t++) {
      var x = -T * Math.log(Math.max(r(), 1e-300) * Math.max(r(), 1e-300));
      if (x <= emax) return x;
    }
    return r() * emax;
  }

  // ── One fission event ─────────────────────────────────────────────────
  // opts: { seed, ternary } → the event, with every particle's final
  // 4-momentum and the conservation residual.
  function event(name, ie, opts) {
    opts = opts || {};
    var P = D().parents[name], Y = P.yields[ie], e = Y.e, r = rng(opts.seed || 1);
    var ternary = !!opts.ternary, PR = pairs(name, ie, ternary), I = PR.I, tke = PR.tke;
    var x = r(), list = PR.list, lo = 0, hi = list.length - 1;
    while (lo < hi) { var mid = (lo + hi) >> 1; if (list[mid].c < x) lo = mid + 1; else hi = mid; }
    var pr = list[lo], s = pr.s;
    var th = 2 * Math.PI * r();
    var m1 = s.M1 + s.ex1, m2 = s.M2 + s.ex2, f1, f2, alpha = null, ma = mass(2, 4);
    var Talpha = null, alphaRule = false;
    if (!ternary) {
      var cm = twoBody(I.Mstar, m1, m2, th); f1 = cm[0]; f2 = cm[1];
    } else {
      // The alpha leaves perpendicular to the fission axis in the CM frame.
      // Its energy: PENDING; the stand-in rule (EFR × 4 / A_CN, the alpha's
      // mass share of the kinetic energy) is a placeholder, not physics.
      Talpha = PENDING().ternaryAlphaEnergyMeV;
      if (Talpha === null || Talpha === undefined) { Talpha = tke * 4 / (P.a + 1); alphaRule = true; }
      var pa = Math.sqrt(Talpha * Talpha + 2 * Talpha * ma), ta = th + Math.PI / 2;
      alpha = vec(ma, pa * Math.cos(ta), pa * Math.sin(ta));
      var rest = [I.Mstar - alpha[0], -alpha[1], -alpha[2]], Wm = minv(rest);
      var sub = twoBody(Wm, m1, m2, th), bv = velocity(rest);
      f1 = boost(sub[0], bv[0], bv[1]); f2 = boost(sub[1], bv[0], bv[1]);
    }
    var bcm = velocity(I.tot);
    f1 = boost(f1, bcm[0], bcm[1]); f2 = boost(f2, bcm[0], bcm[1]);
    if (alpha) alpha = boost(alpha, bcm[0], bcm[1]);
    var frags = [
      { z: pr.z1, a: pr.a1, A: pr.a1 + s.nu1, nu: s.nu1, ex: s.ex1, p0: f1 },
      { z: pr.z2, a: pr.a2, A: pr.a2 + s.nu2, nu: s.nu2, ex: s.ex2, p0: f2 }
    ];
    var meanLab = mt458(P, 2, e) / Y.nup; // ENP / ν̄_p [MeV]
    var neutrons = [], gammas = [];
    frags.forEach(function (f, fi) {
      var m = s["M" + (fi + 1)] + f.ex, p = f.p0, A = f.A, Ex = f.ex;
      f.T0 = kinetic(p, m);
      f.v0 = velocity(p);
      f.temp = Math.max((meanLab - f.T0 / f.A) / 2, 0.01);
      f.tempFloored = (meanLab - f.T0 / f.A) / 2 < 0.01;
      f.emissions = [];
      for (var k = 0; k < f.nu; k++) {
        var later = 0;
        for (var j = 1; j < f.nu - k; j++) later += sn(f.z, A - j);
        var S = sn(f.z, A), emax = Ex - S - later;
        var eps = evaporation(f.temp, emax, r);
        var mr = mass(f.z, A - 1) + (Ex - S - eps);
        var d = twoBody(m, MN, mr, 2 * Math.PI * r()), bv = velocity(p);
        var pn = boost(d[0], bv[0], bv[1]), pres = boost(d[1], bv[0], bv[1]);
        neutrons.push({ frag: fi, k: k, p: pn, T: kinetic(pn, MN), eps: eps });
        f.emissions.push({ p: pn, after: pres, S: S, eps: eps });
        p = pres; m = mr; A -= 1; Ex = mr - mass(f.z, A);
      }
      f.gamma = null;
      if (Ex > 0) {
        var mg = mass(f.z, A), dg = twoBody(m, 0, mg, 2 * Math.PI * r()), bg = velocity(p);
        var pg = boost(dg[0], bg[0], bg[1]); p = boost(dg[1], bg[0], bg[1]); m = mg;
        f.gamma = { p: pg, E: pg[0] };
        gammas.push({ frag: fi, p: pg, E: pg[0] });
      }
      f.pFinal = p; f.mFinal = m; f.Tfinal = kinetic(p, m);
    });
    // Conservation: everything at the end against the neutron + target.
    var sum = [0, 0, 0];
    frags.forEach(function (f) { sum = add(sum, f.pFinal); });
    neutrons.forEach(function (n) { sum = add(sum, n.p); });
    gammas.forEach(function (g) { sum = add(sum, g.p); });
    if (alpha) sum = add(sum, alpha);
    var dP = Math.hypot(sum[1] - I.tot[1], sum[2] - I.tot[2]), dE = sum[0] - I.tot[0];
    var restFinal = frags[0].mFinal + frags[1].mFinal + neutrons.length * MN + (alpha ? ma : 0);
    var Q = I.tot[0] - restFinal; // kinetic + photon energy at the end [MeV]
    return {
      parent: name, e: e, seed: opts.seed, ternary: ternary, alpha: alpha, Talpha: Talpha, alphaRule: alphaRule,
      I: I, tke: tke, pair: pr, frags: frags, neutrons: neutrons, gammas: gammas,
      Q: Q, qSplit: I.Mstar - mass(pr.z1, pr.a1) - mass(pr.z2, pr.a2) - pr.nu * MN - (alpha ? ma : 0),
      dP: dP, dE: dE, pScale: Math.max(I.pn, Math.hypot(frags[0].p0[1], frags[0].p0[2])), eScale: I.tot[0]
    };
  }

  // ── After the prompt stage: beta decay ───────────────────────────────
  // A chain from (Z, A) in the NuDat ground-state table, up to tMax [s].
  function nearestGroup(P, half) {
    var lam = Math.log(2) / half, best = 0, bd = Infinity;
    P.lambda.forEach(function (l, g) { var d = Math.abs(Math.log(l / lam)); if (d < bd) { bd = d; best = g; } });
    return best;
  }
  function sampleSpectrum(g, r) {
    // A histogram group spectrum (LF=5, INT=1 on the tape): pick a bin by its
    // area, then uniformly inside it.
    var e = g.e, p = g.p, tot = 0, i;
    for (i = 0; i + 1 < e.length; i++) tot += p[i] * (e[i + 1] - e[i]);
    var x = r() * tot;
    for (i = 0; i + 1 < e.length; i++) {
      var w = p[i] * (e[i + 1] - e[i]);
      if (x <= w) return e[i] + (e[i + 1] - e[i]) * (w > 0 ? x / w : 0);
      x -= w;
    }
    return e[e.length - 1];
  }
  function betaChain(name, z, a, seed, tMax) {
    var P = D().parents[name], r = rng(seed), t = 0, out = [], steps = 0;
    while (steps++ < 12) {
      var d = D().decays[z + "," + a];
      if (!d || !d[0]) { out.push({ z: z, a: a, t: t, stable: !d }); break; }
      var dt = -Math.log(Math.max(r(), 1e-300)) * d[0] / Math.LN2;
      if (t + dt > tMax) { out.push({ z: z, a: a, t: t, half: d[0], pending: true }); break; }
      t += dt;
      var bn = r() * 100 < d[2], q;
      if (bn) {
        var g = nearestGroup(P, d[0]);
        q = (deltaKeV(z, a) - deltaKeV(z + 1, a - 1) - deltaKeV(0, 1)) / 1000;
        out.push({ z: z, a: a, t: t, half: d[0], to: [z + 1, a - 1], q: q, dn: { group: g, E: sampleSpectrum(P.groups[g], r) } });
        z = z + 1; a = a - 1;
      } else {
        q = (deltaKeV(z, a) - deltaKeV(z + 1, a)) / 1000;
        out.push({ z: z, a: a, t: t, half: d[0], to: [z + 1, a], q: q });
        z = z + 1;
      }
    }
    return out;
  }
  // The delayed neutrons per fission this construction implies: Σ over the
  // independent yields of the chance a chain from each product emits one
  // (NuDat branchings, every decay followed, no time cut).
  function delayedPerFission(name, ie) {
    var Y = D().parents[name].yields[ie], memo = {};
    function pn(z, a, depth) {
      var k = z + "," + a;
      if (memo[k] !== undefined) return memo[k];
      var d = D().decays[k];
      if (!d || !d[0] || depth > 20) return (memo[k] = 0);
      var b = d[2] / 100;
      return (memo[k] = b * (1 + pn(z + 1, a - 1, depth + 1)) + (1 - b) * pn(z + 1, a, depth + 1));
    }
    var s = 0;
    for (var i = 0; i < Y.z.length; i++) s += Y.y[i] * pn(Y.z[i], Y.a[i], 0);
    return s;
  }

  // ── Timescales ─────────────────────────────────────────────────────────
  function timescales(name, ie) {
    var P = D().parents[name], e = P.yields[ie].e, I = initial(P, e);
    var R = R0_FM * Math.pow(P.a, 1 / 3) * 1e-15, vn = C * I.pn / (I.Tn + MN);
    var inRes = e >= P.res.el && e <= P.res.eh;
    return {
      radius_m: R, vn: vn, transit: 2 * R / vn,
      tauCN: inRes ? HBAR_EVS / P.res.mean : null, gammaMean: P.res.mean,
      promptBound: PROMPT_BOUND_S
    };
  }

  var api = {
    MN: MN, C: C, HBAR_EVS: HBAR_EVS, R0_FM: R0_FM, PROMPT_BOUND_S: PROMPT_BOUND_S,
    deltaKeV: deltaKeV, mass: mass, u: u, sn: sn, stableZ: stableZ, linlin: linlin, mt458: mt458,
    vec: vec, minv: minv, boost: boost, twoBody: twoBody, velocity: velocity, rng: rng,
    initial: initial, split: split, pairs: pairs, event: event, betaChain: betaChain,
    nearestGroup: nearestGroup, sampleSpectrum: sampleSpectrum, delayedPerFission: delayedPerFission, timescales: timescales
  };
  W.FissionPhysics = api;
})();
