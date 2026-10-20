// The Fission products lessons' physics (lessons 1-3), checked under node (no browser):
// `node docs/site/science/fission-products/check-fission.js`, run by
// scripts/build-pages.sh beside the neutronics and transport checks, so a
// regression fails the Pages build. Each check says what it compares
// against; the results of 2026-10-20 are in the comments. Lines marked
// "REPORT" print a measured disagreement that the page shows as it is; they
// are not gates.
"use strict";
const fs = require("fs"), path = require("path");
global.window = {};
for (const f of ["fission-data.js", "fission-pending.js", "fission-physics.js", "fp-energy.js", "yields-data.js", "split-physics.js", "spectrum-data.js"]) eval(fs.readFileSync(path.join(__dirname, f), "utf8"));
const F = window.FissionPhysics, D = window.FISSION_DATA, SP = window.SplitPhysics, YD = window.YIELDS_DATA, SD = window.SPECTRUM_DATA;
const pages = ["one-fission.html", "split-atoms.html", "prompt-spectrum.html"].map((f) => [f, fs.readFileSync(path.join(__dirname, f), "utf8")]);
let failed = 0;
function check(name, value, limit) {
  const ok = value <= limit;
  if (!ok) failed++;
  console.log(`${ok ? "ok  " : "FAIL"} ${name}: ${value.toExponential(2)} (limit ${limit})`);
}
function report(name, text) { console.log(`REPORT ${name}: ${text}`); }

// 1. Every yield table that is present is complete: two products per
//    fission. 2026-10-20: 2.000000 for all five (U-238 has no thermal table:
//    its 0.0253 eV yields are all zero in fission-yields-data and the
//    generator leaves the table out).
for (const [name, P] of Object.entries(D.parents)) {
  P.yields.forEach((Y) => check(`${name} at ${Y.e} eV: sum of independent yields vs 2`, Math.abs(Y.sum - 2), 1e-4));
  if (name === "U-238") check("U-238 has no thermal yield table", P.yields.some((Y) => Y.e < 1) ? 1 : 0, 0);
}

// 2. Every pair the construction can draw conserves charge and mass number
//    exactly, binary and ternary.
for (const [name, P] of Object.entries(D.parents)) {
  P.yields.forEach((Y, ie) => {
    for (const tern of [false, true]) {
      const pr = F.pairs(name, ie, tern);
      let bad = 0;
      pr.list.forEach((x) => {
        if (x.z1 + x.z2 + (tern ? 2 : 0) !== P.z) bad++;
        if (x.a1 + x.a2 + x.nu + (tern ? 4 : 0) !== P.a + 1) bad++;
        if (x.nu < 0) bad++;
      });
      check(`${name} ${Y.e} eV ${tern ? "ternary" : "binary"}: pairs breaking Z or A balance (of ${pr.list.length})`, bad, 0);
    }
  });
}

// 3. Energy and momentum conservation per event, to round-off. The total
//    energy is ~2.2e5 MeV, so a few ulp over ~50 operations is ~1e-10 MeV;
//    the gates are 1e-8 MeV and 1e-9 MeV/c, set from that before running.
//    2026-10-20: worst |dE| 8.7e-11 MeV, worst |dp| 5.4e-12 MeV/c (U-235
//    thermal, 2000 events); ternary 5.8e-11 / 7.6e-14.
for (const [name, P] of Object.entries(D.parents)) {
  P.yields.forEach((Y, ie) => {
    let wp = 0, we = 0, wt = 0, wq = 0;
    for (let s = 1; s <= 2000; s++) {
      const ev = F.event(name, ie, { seed: s, ternary: s % 10 === 0 });
      wp = Math.max(wp, ev.dP); we = Math.max(we, Math.abs(ev.dE));
      if (!ev.ternary) {
        const m1 = ev.frags[0].p0, m2 = ev.frags[1].p0;
        wt = Math.max(wt, Math.abs(ev.I.Mstar - F.minv(m1) - F.minv(m2) - ev.tke));
      }
      wq = Math.max(wq, Math.abs(ev.Q - ev.qSplit));
    }
    check(`${name} ${Y.e} eV: worst |Σp − p_n| [MeV/c], 2000 events`, wp, 1e-9);
    check(`${name} ${Y.e} eV: worst |ΔE| [MeV], 2000 events`, we, 1e-8);
    check(`${name} ${Y.e} eV: worst |M* − m1* − m2* − EFR| [MeV] (TKE is EFR)`, wt, 1e-8);
    check(`${name} ${Y.e} eV: worst |Q(final particles) − Q(split)| [MeV]`, wq, 1e-8);
  });
}

// 4. E* = S_n + E_cm: the invariant mass of neutron + target against the
//    table's S_n and the non-relativistic E_cm = E A/(A + 1). 2026-10-20:
//    agree to 3e-12 MeV at thermal (the relativistic correction is ~1e-11).
{
  const P = D.parents["U-235"], I = F.initial(P, 0.0253), Ecm = 0.0253e-6 * P.a / (P.a + 1);
  check("U-235 thermal: |E* − (S_n + E_cm)| [MeV]", Math.abs(I.Ex - I.Sn - Ecm), 1e-9);
  // The table's S_n against the evaluation's capture Q (MT=102): NuDat's
  // 3-figure rounding. 2026-10-20: 6.570 against 6.544 MeV.
  check("U-235: |S_n(NuDat) − QM(MT=102)| [MeV], the table's rounding", Math.abs(I.Sn - P.sn / 1e6), 0.1);
}

// 5. ν from the yields' own mass balance, A_CN − Σ Y A, against the
//    evaluation's prompt ν̄_p (MT=456): two independent parts of
//    ENDF/B-VIII.0. The balance assumes every prompt neutron comes from the
//    fragments of A_CN, which holds below the second-chance threshold, so it
//    is gated (0.05 neutron) for U-235 at thermal and 500 keV only.
//    2026-10-20: 2.4000 vs 2.4140 and 2.4565 vs 2.4669. REPORTED, not gated:
//    U-235 at 14 MeV, 4.1308 vs 4.3796 (balance below ν̄_p, the sign of
//    pre-fission emission), and U-238 at 500 keV, 2.8942 vs 2.4428 (above
//    ν̄_p, unexplained; the page says so) and 14 MeV, 4.3924 vs 4.4213.
const nuYields = {};
for (const [name, P] of Object.entries(D.parents)) {
  P.yields.forEach((Y, ie) => {
    let sa = 0; for (let i = 0; i < Y.a.length; i++) sa += Y.y[i] * Y.a[i];
    const nu = P.a + 1 - sa;
    nuYields[name + ie] = nu;
    const what = `${name} ${Y.e} eV: |ν(yield mass balance) ${nu.toFixed(4)} − ν̄_p ${Y.nup.toFixed(4)}|`;
    if (name === "U-235" && Y.e < 1e6) check(what, Math.abs(nu - Y.nup), 0.05);
    else report(what, (nu - Y.nup).toFixed(4));
  });
}
check("U-235 14 MeV: the mass balance lies below ν̄_p (the page says so)", nuYields["U-2352"] < D.parents["U-235"].yields[2].nup ? 0 : 1, 0);
check("U-238 500 keV: the mass balance lies above ν̄_p (the page says so)", nuYields["U-2380"] > D.parents["U-238"].yields[0].nup ? 0 : 1, 0);

// 6. REPORT: the pair construction's mean ν against the same two numbers,
//    and the delayed neutrons per fission implied by NuDat's branchings
//    against MT=455. Both are disagreements the page states.
const nuPairs = F.pairs("U-235", 0, false).meanNu, nuDel = F.delayedPerFission("U-235", 0);
for (const [name, P] of Object.entries(D.parents)) {
  P.yields.forEach((Y, ie) => {
    report(`${name} ${Y.e} eV`, `pair construction ⟨ν⟩ = ${F.pairs(name, ie, false).meanNu.toFixed(4)} vs yields ${nuYields[name + ie].toFixed(4)} vs ν̄_p ${Y.nup.toFixed(4)}; ` +
      `delayed per fission (NuDat chains) ${F.delayedPerFission(name, ie).toFixed(5)} vs ν̄_d ${Y.nud.toFixed(5)}`);
  });
}

// 7. The prompt neutrons the demo samples, against MF=5/MT=18 at the same
//    energy: mean energy (close by construction, since T is set from
//    ENP/ν̄_p) and the fraction above 5 MeV (not constrained: a real test of
//    the shape). REPORT only.
{
  const P = D.parents["U-235"], Y = P.yields[0];
  let n = 0, sum = 0, hi = 0;
  for (let s = 1; s <= 5000; s++) F.event("U-235", 0, { seed: s }).neutrons.forEach((x) => { n++; sum += x.T; if (x.T > 5) hi++; });
  let tot = 0, above = 0;
  for (let i = 0; i + 1 < Y.chi.e.length; i++) {
    const w = 0.5 * (Y.chi.p[i] + Y.chi.p[i + 1]) * (Y.chi.e[i + 1] - Y.chi.e[i]);
    tot += w; if (Y.chi.e[i] >= 5e6) above += w;
  }
  report("U-235 thermal prompt neutrons", `demo mean ${(sum / n).toFixed(3)} MeV vs MF=5 ${(Y.chi.mean / 1e6).toFixed(3)} MeV; above 5 MeV demo ${(100 * hi / n).toFixed(2)} % vs MF=5 ${(100 * above / tot).toFixed(2)} % (thinned table)`);
}

// 8. Every number the pages bind to the data (data-chk, data-mt458, data-sp,
//    data-ts) is filled by the page's script from the data: the HTML must
//    hold only a placeholder there, never a typed number that could go stale.
for (const [f, html] of pages) {
  const typed = [...html.matchAll(/data-(?:chk|mt458|sp|ts)="[^"]*"(?: data-pct)?>([^<]*)</g)].filter((m) => /[0-9]/.test(m[1]));
  check(`${f}: bound spans holding a typed number`, typed.length, 0);
  if (typed.length) typed.forEach((m) => console.log("   ", m[0]));
}
// The pair construction's miss and the delayed-neutron disagreement the
// page states in words: the construction short of ν̄_p, NuDat's chains
// above ν̄_d. 2026-10-20: 2.0568 vs 2.4140 (15 % short), 0.02234 vs 0.01585.
check("U-235 thermal: pair construction ⟨ν⟩ below ν̄_p (the page says so)", nuPairs < D.parents["U-235"].yields[0].nup ? 0 : 1, 0);
check("U-235 thermal: NuDat chains' delayed ν above ν̄_d (the page says so)", nuDel > D.parents["U-235"].yields[0].nud ? 0 : 1, 0);

// 9. Timescales the page computes (printed for the record).
{
  const T = F.timescales("U-235", 0);
  report("U-235 thermal timescales", `2R/v = ${T.transit.toExponential(3)} s (R ${(T.radius_m * 1e15).toFixed(2)} fm); τ = ħ/⟨Γ⟩ = ${T.tauCN.toExponential(3)} s (⟨Γ⟩ ${T.gammaMean} eV)`);
}

// 9b. The page builds the pair table in slices (preparePairs); it must be
//     the table pairs() builds at once. A fresh copy of the physics (empty
//     cache), slices of 37, deferral run at once.
{
  const saved = window.FissionPhysics;
  eval(fs.readFileSync(path.join(__dirname, "fission-physics.js"), "utf8"));
  const fresh = window.FissionPhysics;
  window.FissionPhysics = saved;
  let got = null;
  fresh.preparePairs("U-235", 0, false, () => { got = fresh.pairs("U-235", 0, false); }, (f) => f(), 37);
  const want = saved.pairs("U-235", 0, false);
  const same = got && got.list.length === want.list.length && got.meanNu === want.meanNu && got.list.every((x, i) => x.c === want.list[i].c && x.a1 === want.list[i].a1 && x.a2 === want.list[i].a2);
  check("preparePairs in slices of 37 vs pairs() at once (U-235 thermal): differing tables", same ? 0 : 1, 0);
}

// ── Lesson 2 (split atoms) ───────────────────────────────────────────────
// 10. Every yield table sums to two products per fission, and the page's
//     Y(A) and peak-to-valley reproduce the generator's.
for (const [name, P] of Object.entries(YD)) {
  P.tables.forEach((t) => {
    check(`${name} ${t.e} eV: Σ Y vs 2 (yields-data)`, Math.abs(t.sum - 2), 1e-4);
    const m = SP.massYields(SP.yieldsAt(name, t.e)), pv = SP.peakValley(m, (P.a + 1) / 2);
    // The table is written to 4-5 figures: the page's ratio agrees to 1e-4.
    check(`${name} ${t.e} eV: page's peak-to-valley ${pv.ratio.toFixed(2)} vs generator's ${t.pv.ratio.toFixed(2)} (relative)`, Math.abs(pv.ratio / t.pv.ratio - 1), 1e-4);
  });
  // 11. The prediction, written before the run: peak-to-valley falls with
  //     energy for every parent. 2026-10-20: it does, for all four
  //     (U-235 635.8, 218.4, 5.34; U-238 192.1, 10.06; U-233 536.1, 126.4,
  //     5.12; Pu-239 239.7, 130.7, 99.5, 4.79).
  const r = P.tables.map((t) => t.pv.ratio);
  check(`${name}: peak-to-valley falls with energy (${r.map((x) => x.toFixed(1)).join(" > ")})`, r.every((x, i) => i === 0 || x < r[i - 1]) ? 0 : 1, 0);
  // 12. The page interpolates as the crate does, linearly in E: Y(A) at the
  //     crate's own in-between energies. Data written to 5 figures, so the
  //     gate is 1e-5 per fission (2026-10-20: worst 1.03e-6).
  P.between.forEach((b) => {
    const m = SP.massYields(SP.yieldsAt(name, b.e));
    let w = 0;
    for (let k = 1; k < b.mass.length; k++) { const a = b.mass[0] + k - 1; w = Math.max(w, Math.abs((m.y[a - m.a0] || 0) - b.mass[k])); }
    check(`${name} at ${b.e} eV: page's interpolated Y(A) vs fission-yields-data's`, w, 1e-5);
  });
}
// 13. The pair construction: charge and mass number kept in every pair it
//     draws, and its own exact mass distribution against Y(A) (REPORT; the
//     page quotes the U-235 thermal number). 2026-10-20: at most 4.4 % of
//     the highest peak, U-235 thermal.
{
  const S = SP.sampler("U-235", 0.0253), r = SP.rng(7);
  let bad = 0, nu = 0, n = 0;
  for (let k = 0; k < 20000; k++) {
    const p = SP.samplePair(S, r);
    if (!p) continue;
    n++; nu += p.nu;
    if (p.z1 + p.z2 !== 92 || p.a1 + p.a2 + p.nu !== 236 || p.nu < 0) bad++;
  }
  check("lesson 2 pairs breaking Z or A balance, 20 000 draws", bad, 0);
  for (const [name, P] of Object.entries(YD)) P.tables.forEach((t) => {
    report(`${name} ${t.e} eV lesson 2 construction`, `max |construction(A) − Y(A)| / peak = ${(100 * SP.constructionDeviation(name, t.e)).toFixed(1)} %; charge asymmetry max |Y(Z) − Y(Z_CN − Z)| = ${SP.chargeAsymmetry(name, t.e).toFixed(4)}`);
  });
  report("U-235 thermal lesson 2 pairs", `mean ν of the construction ${(nu / n).toFixed(3)} (no energy test here; lesson 1's adds one)`);
}

// ── Lesson 3 (the prompt spectrum) ───────────────────────────────────────
// 14. At every slider energy, the histogram drawn through outram-mc-libs'
//     sampler against the exact law it follows: chi-square over bins with an
//     expected count of at least 20 (the overflow included), gated at
//     dof + 5 sqrt(2 dof); and the sampled mean against the exact mean, gated
//     at 5 standard errors (the spread of E' is under 2 MeV, so 5 × 2 MeV /
//     sqrt(n)). Fixed before running. 2026-10-20: printed below.
for (const [name, P] of Object.entries(SD)) {
  let worst = 0, worstE = 0, worstMean = 0, chiSum = 0, dofSum = 0;
  P.grid.forEach((g) => {
    let c2 = 0, dof = 0;
    g.counts.concat([g.over]).forEach((c, k) => {
      const m = (k < g.prob.length ? g.prob[k] : g.pover) * g.n;
      if (m >= 20) { c2 += (c - m) * (c - m) / m; dof++; }
    });
    chiSum += c2; dofSum += dof;
    const z = (c2 - dof) / Math.sqrt(2 * dof);
    if (z > worst) { worst = z; worstE = g.e; }
    worstMean = Math.max(worstMean, Math.abs(g.sum / g.n - g.mean) / (2e6 / Math.sqrt(g.n)));
    if (c2 > dof + 5 * Math.sqrt(2 * dof)) check(`${name} at ${g.e} eV: χ² ${c2.toFixed(1)} over ${dof} bins`, c2, dof + 5 * Math.sqrt(2 * dof));
    // The exact bin probabilities and the overflow sum to one.
    if (Math.abs(g.prob.reduce((a, b) => a + b, 0) + g.pover - 1) > 1e-3) check(`${name} at ${g.e} eV: Σ p`, 1, 0);
  });
  report(`${name} spectra, ${P.grid.length} energies`, `pooled χ²/dof ${(chiSum / dofSum).toFixed(3)} over ${dofSum} bins; worst single (χ² − dof)/√(2 dof) = ${worst.toFixed(2)} at ${worstE} eV`);
  check(`${name}: worst |sampled − exact mean| in units of 2 MeV/√n`, worstMean, 5);
  // 15. What the page says in words, checked on the data: the most probable
  //     energy is below 1 MeV and the mean at least 1.5 times it at every
  //     tabulated energy, and two to three times it at thermal energy (it is
  //     more where the mean dips: 5.3 times at 6 MeV for U-235); the mean rises from the lowest
  //     tabulated energy to 5 MeV by under 10 %; and it dips (falls between
  //     two tabulated energies) somewhere in 5-8 MeV and in 11-15 MeV.
  const tab = P.grid.filter((g) => g.tab);
  check(`${name}: tabulated energies with a most probable E' at or above 1 MeV`, tab.filter((g) => g.mode >= 1e6).length, 0);
  check(`${name}: tabulated energies with a mean under 1.5 times the most probable`, tab.filter((g) => g.mean / g.mode < 1.5).length, 0);
  const th = P.grid.find((g) => Math.abs(g.e - 0.0253) < 1e-9);
  check(`${name} at thermal: mean / most probable ${(th.mean / th.mode).toFixed(2)} outside [2, 3]`, th.mean / th.mode < 2 || th.mean / th.mode > 3 ? 1 : 0, 0);
  const at5 = tab.find((g) => Math.abs(g.e - 5e6) < 1);
  check(`${name}: mean rise from ${tab[0].e} eV to 5 MeV (fraction)`, at5.mean / tab[0].mean - 1, 0.10);
  const dip = (lo, hi) => tab.some((g, i) => i > 0 && g.e > lo && g.e <= hi && g.mean < tab[i - 1].mean);
  check(`${name}: the mean dips somewhere in 5-8 MeV`, dip(5e6, 8e6) ? 0 : 1, 0);
  check(`${name}: the mean dips somewhere in 11-15 MeV`, dip(11e6, 15e6) ? 0 : 1, 0);
  // 16. The prediction, written before the run, on the mean between thermal
  //     and 5 MeV: it rises. 2026-10-20: it does, monotonically.
  const below = tab.filter((g) => g.e <= 5e6 + 1);
  check(`${name}: mean rises monotonically up to 5 MeV`, below.every((g, i) => i === 0 || g.mean > below[i - 1].mean) ? 0 : 1, 0);
  tab.forEach((g) => console.log(`       ${name} ${g.e.toExponential(3)} eV: mean ${(g.mean / 1e6).toFixed(4)} MeV (sampled ${(g.sum / g.n / 1e6).toFixed(4)}), most probable ${(g.mode / 1e6).toFixed(4)} MeV`));
}
// 17. The energy slider's own helpers: the log map round-trips and the
//     bracket matches the linear interpolation the yields use.
{
  const FE = window.FPEnergy, e = FE.toEnergy(FE.toPos(1234.5, 0.0253, 1.4e7), 0.0253, 1.4e7);
  check("slider round trip 1234.5 eV (relative, one step of 400 over 8.7 decades)", Math.abs(e / 1234.5 - 1), 0.03);
  const b = FE.bracket([0.0253, 5e5, 1.4e7], 1e6);
  check("bracket of 1 MeV in (0.0253 eV, 500 keV, 14 MeV)", Math.abs(b[0] - 1) + Math.abs(b[2] - 5e5 / 1.35e7), 1e-15);
}

// ─── Chapter 2, "A fragment's story" (lessons 4-9) ───────────────────────
// Data from crates/dhoby-ghaut/examples/lesson_fission_cascades
// (cascade-data.js), the cited records (cascade-recorded.js) and the kinetic
// Monte Carlo (hop-physics.js).
//
// PREDICTIONS, written 2026-10-20 BEFORE the first run of 18-24:
// - 21: the interstitial walk's Arrhenius fit gives back E_m = 1.00 eV to
//   within 3 %, r^2 > 0.999 (1000 walkers, ~400 hops each, 8 temperatures).
// - 22: the vacancy mechanism's slope gives E_f + E_m = 1.40 eV to within
//   5 % (thermal vacancies only).
// - 23: with a fixed irradiation vacancy fraction 1e-2 swamping the thermal
//   one (600-900 K), the slope falls to E_m = 0.80 eV, within 10 %.
// - 24: with 5 % traps of E_b = 0.5 eV, at 600-800 K the slope rises to
//   E_m + E_b = 1.50 eV, within 10 %; at 1800-2400 K it is nearer E_m
//   (REPORT only).
// - 19: every beta recoil on both chains is under 1 keV, so under 1e-5 of
//   the fragment's energy; the alpha recoils are 50-150 keV.
for (const f of ["cascade-data.js", "cascade-recorded.js", "hop-physics.js"]) eval(fs.readFileSync(path.join(__dirname, f), "utf8"));
const CD = window.CASCADE_DATA, CR = window.CASCADE_RECORDED, HP = window.HopPhysics;
const ch2 = ["fragment-flight.html", "lattice-cascade.html", "decay-again.html", "decay-or-move.html", "atom-hop.html", "atoms-swap.html"]
  .map((f) => [f, fs.readFileSync(path.join(__dirname, f), "utf8")]);
const U_KEV = 931494.10242, ME_KEV = 510.9989500;

// 18. The fragments: the momentum split sums to EFR, and the two momenta
//     are equal and opposite (to the non-relativistic split's ~v^2/c^2).
//     The sum gates (here and in 19's alpha check) were first set at 1e-12
//     and FAILED on the first run (5.9e-8, 1.9e-9, 2.8e-8): cascade-data.js
//     stores 7 significant figures, so a sum of two stored values can be off
//     by ~1e-7 relative. Corrected 2026-10-20 to 1e-6, from that precision.
{
  const [L, Hv] = CD.fragments.pair;
  check("fragments: |E_L + E_H − EFR| / EFR (7-figure file)", Math.abs(L.e + Hv.e - CD.fragments.efr) / CD.fragments.efr, 1e-6);
  check("fragments: |pc_L − pc_H| / pc_L (non-relativistic split)", Math.abs(L.pcKeV - Hv.pcKeV) / L.pcKeV, 2e-3);
}

// 19. The chains: each daughter is the next parent; every kick recomputed
//     here from Q (independently of physics.rs, with A u for the mass)
//     agrees with the generator's to 1e-3; and the prediction above.
{
  let bad = 0, worstBeta = 0, worstRel = 0;
  CD.chains.forEach((c) => {
    c.steps.forEach((s, i) => {
      if (i && c.steps[i - 1].daughter !== s.parent) bad++;
      const M = s.da * U_KEV, q = s.qKeV[0], pc2 = q * q + 2 * q * ME_KEV;
      worstRel = Math.max(worstRel, Math.abs(pc2 / (2 * M) * 1e3 / s.betaRecoilMax[0] - 1), Math.abs(q * q / (2 * M) * 1e3 / s.gammaRecoilMax[0] - 1));
      worstBeta = Math.max(worstBeta, s.betaRecoilMax[2]);
      report(`${s.parent} → ${s.daughter}`, `${s.mode}, t½ ${s.t12.toExponential(3)} s, Q ${q.toFixed(0)} keV [${s.qKeV[1].toFixed(0)}, ${s.qKeV[2].toFixed(0)}], beta recoil ≤ ${s.betaRecoilMax[0].toFixed(1)} eV, gamma recoil ≤ ${s.gammaRecoilMax[0].toFixed(2)} eV`);
    });
  });
  check("chains: daughter ≠ next parent", bad, 0);
  check("chains: recoil recomputed in JS vs generator (relative)", worstRel, 1e-3);
  check("chains: largest beta recoil (upper end of the mass interval) [eV], predicted < 1000", worstBeta, 1000);
  const frag = CD.fragments.pair[0].e;
  check("chains: largest beta recoil / light fragment energy, predicted < 1e-5", worstBeta / frag, 1e-5);
  CD.kicks.filter((k) => k.kind === "alpha").forEach((k) => {
    report(`${k.parent} alpha`, `Q ${k.qKeV[0].toFixed(0)} keV, recoil ${(k.recoil[0] / 1e3).toFixed(1)} keV [${(k.recoil[1] / 1e3).toFixed(1)}, ${(k.recoil[2] / 1e3).toFixed(1)}], alpha ${(k.eAlpha[0] / 1e3).toFixed(0)} keV`);
    check(`${k.parent} alpha recoil outside the predicted 50-150 keV`, k.recoil[0] > 5e4 && k.recoil[0] < 1.5e5 ? 0 : 1, 0);
    check(`${k.parent} alpha: |E_alpha + E_recoil − Q| / Q`, Math.abs((k.eAlpha[0] + k.recoil[0]) / 1e3 - k.qKeV[0]) / k.qKeV[0], 1e-6);
  });
  CD.kicks.filter((k) => k.kind === "neutron").forEach((k) => report(`${k.parent} beta-,n`, `Q_bn ${k.qKeV[0].toFixed(0)} keV, recoil ≤ ${(k.recoil[0] / 1e3).toFixed(2)} keV`));
  CD.inflight.forEach((f) => report(`in flight, ${f.name}`, `turn ≤ ${f.angleRad.toExponential(2)} rad, ΔE ≤ ${f.dEKeV.toFixed(1)} keV, P(decay in 1 ps) ${f.pPerPs.toExponential(2)}`));
  // Sampled histories run forward in time and start at the fragment.
  let hb = 0;
  for (const [name, hs] of Object.entries(CD.histories)) hs.forEach((h) => {
    if (h[0][0] !== name) hb++;
    h.forEach((x, i) => { if (i && x[1] !== null && h[i - 1][1] !== null && x[1] < h[i - 1][1]) hb++; });
    if (h[h.length - 1][1] !== null) hb++;
  });
  check("histories: out of order, wrong start, or not ending stable", hb, 0);
  // decay-again.html says the half-lives are "more than ten orders of
  // magnitude" longer than the flight (its lower bound, range / speed).
  const tmin = CR.rustbca.light.rangeUm * 1e-6 / CD.fragments.pair[0].v;
  const t12 = Math.min(...CD.chains.map((c) => c.steps[0].t12));
  report("flight vs first half-life", `flight ≥ ${tmin.toExponential(2)} s, first t½ ${t12} s, ratio ${(t12 / tmin).toExponential(2)}`);
  check("decay-again: first t½ / flight lower bound below 1e10", t12 / tmin > 1e10 ? 0 : 1, 0);
}

// 20. Diffusion data: positive, never falling with T, and the flat (clamped)
//     range the generator found is flat in the array too.
{
  let bad = 0;
  CD.diffusion.series.forEach((s) => {
    s.d.forEach((d, i) => { if (!(d > 0) || (i && d < s.d[i - 1])) bad++; });
    if (s.flatUpToC !== null) CD.diffusion.tC.forEach((t, i) => { if (i && t <= s.flatUpToC && s.d[i] !== s.d[i - 1]) bad++; });
  });
  check("diffusion series: non-positive, falling, or not flat where clamped", bad, 0);
  // kT at 1000 K, against the 0.086 eV the design brief quotes.
  check("kT(1000 K) − 0.0862 eV", Math.abs(HP.kT(1000) - 0.0862), 1e-4);
  const t = HP.turnbull(1500, 0);
  check("Turnbull at F = 0 is its first term", Math.abs(t.total - t.d1), 0);
  // atom-hop.html says a 1 eV barrier hops "dozens of times" as often at
  // 1500 K as at 1000 K.
  const ratio = HP.boltzmann(1, 1500) / HP.boltzmann(1, 1000);
  report("hop ratio 1500 K / 1000 K, 1 eV", ratio.toFixed(1));
  check("atom-hop: hop ratio outside 'dozens' (12-100)", ratio > 12 && ratio < 100 ? 0 : 1, 0);
}

// 21-24. The kinetic Monte Carlo: the Arrhenius slope comes back out.
{
  const lin = (a, b, n) => Array.from({ length: n }, (_, i) => a + (b - a) * i / (n - 1));
  const s21 = HP.sweep("walk", { Em: 1.0 }, lin(900, 1950, 8), 1000, 400, 21);
  report("21 interstitial", `E_fit ${s21.fit.E.toFixed(4)} eV, r² ${s21.fit.r2.toFixed(5)}`);
  check("21: |E_fit − E_m| / E_m, interstitial walk (predicted < 3 %)", Math.abs(s21.fit.E - 1.0) / 1.0, 0.03);
  check("21: 1 − r²", 1 - s21.fit.r2, 1e-3);
  const s22 = HP.sweep("vacancy", { Ef: 0.6, Em: 0.8, cIrr: 0 }, lin(900, 1950, 8), 100, 200000, 22);
  report("22 vacancy", `E_fit ${s22.fit.E.toFixed(4)} eV (E_f + E_m = 1.40), r² ${s22.fit.r2.toFixed(5)}`);
  check("22: |E_fit − (E_f + E_m)| / 1.4, vacancy mechanism (predicted < 5 %)", Math.abs(s22.fit.E - 1.4) / 1.4, 0.05);
  const s23 = HP.sweep("vacancy", { Ef: 0.6, Em: 0.8, cIrr: 1e-2 }, lin(600, 900, 6), 100, 200000, 23);
  report("23 irradiation", `E_fit ${s23.fit.E.toFixed(4)} eV (E_m = 0.80)`);
  check("23: |E_fit − E_m| / E_m with irradiation vacancies (predicted < 10 %)", Math.abs(s23.fit.E - 0.8) / 0.8, 0.10);
  const s24 = HP.sweep("walk", { Em: 1.0, Eb: 0.5, trapFrac: 0.05 }, lin(600, 800, 5), 1000, 400, 24);
  report("24 traps, 600-800 K", `E_fit ${s24.fit.E.toFixed(4)} eV (E_m + E_b = 1.50)`);
  check("24: |E_fit − (E_m + E_b)| / 1.5, traps at low T (predicted < 10 %)", Math.abs(s24.fit.E - 1.5) / 1.5, 0.10);
  const s24h = HP.sweep("walk", { Em: 1.0, Eb: 0.5, trapFrac: 0.05 }, lin(1800, 2400, 5), 1000, 400, 25);
  report("24 traps, 1800-2400 K", `E_fit ${s24h.fit.E.toFixed(4)} eV`);
}

// 25. The chapter's pages: banner, folded advanced section, and the
//     recorded numbers carry their date.
for (const [f, html] of ch2) {
  check(`${f}: education-only banner missing`, html.includes("Research, education and V&amp;V only") ? 0 : 1, 0);
  check(`${f}: "Learn more advanced stuff" section missing`, html.includes("Learn more advanced stuff") ? 0 : 1, 0);
}
check("cascade-recorded.js: the RustBCA record carries its date", CR.rustbca.date === "2026-10-16" ? 0 : 1, 0);

if (failed) { console.log(`${failed} check(s) failed`); process.exit(1); }
console.log("all fission-products checks passed");
