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

// ─── Chapter 3, "TRISO kernel" (lessons 10-28) ───────────────────────────
// Data: kernel-data.js (crates/dhoby-ghaut/examples/lesson_triso_kernel),
// kernel-recorded.js (cited / pending), kernel-physics.js (the page models).
// PREDICTIONS, written 2026-10-20 BEFORE the first run of 26-34:
// - 26: the page's Booth series and R/B agree with boon-lay's to 1e-9.
// - 27: R/B falls as mu^-1 (slope -1/2 against lambda) at large mu: the
//   log-log slope over the last two decades is -1 within 0.01.
// - 31: the page's fixed-step sphere walk sits BELOW Booth (it misses
//   excursions within a step), by less than 0.03 at D t / a^2 = 0.01.
// - 32: the recoil escape share for range/radius = 0.01 is three quarters
//   of 0.01 within 5 %.
// - 34: a Potts run at kT' = 0.3 has less boundary after 50 sweeps.
for (const f of ["kernel-data.js", "kernel-recorded.js", "kernel-physics.js"]) eval(fs.readFileSync(path.join(__dirname, f), "utf8"));
const KD = window.KERNEL_DATA, KR = window.KERNEL_RECORDED, KP = window.KernelPhysics;
// 26. The page's formulas against boon-lay's values. The FIRST run failed
//     (1.1e-7, 3.2e-7): kernel-data.js held the curves at 7 figures. The
//     generator now writes them at full precision; 2026-10-20: 4.1e-15 and 0.
{
  let wf = 0, wr = 0;
  KD.booth.x.forEach((x, i) => { wf = Math.max(wf, Math.abs(KP.boothF(x) - KD.booth.f[i])); });
  KD.booth.mu.forEach((m, i) => { wr = Math.max(wr, Math.abs(KP.boothRB(m) - KD.booth.rb[i]) / KD.booth.rb[i]); });
  check("26: |page Booth f − boon-lay booth_longlived| (worst)", wf, 1e-9);
  check("26: |page R/B − boon-lay booth_shortlived_fast_diffuse| / R/B (worst)", wr, 1e-9);
}
// 27. The R/B fingerprint's slope.
{
  const n = KD.booth.mu.length, i0 = n - 21, i1 = n - 1;
  const slope = Math.log(KD.booth.rb[i1] / KD.booth.rb[i0]) / Math.log(KD.booth.mu[i1] / KD.booth.mu[i0]);
  report("27 R/B slope against mu (last two decades)", slope.toFixed(5) + ", so " + (slope / 2).toFixed(5) + " against lambda");
  check("27: |slope + 1| (R/B ∝ mu^-1 = lambda^-1/2)", Math.abs(slope + 1), 0.01);
}
// 28. boon-lay's CRP-6 Monte Carlo against Booth (generator), within 3 se.
{
  let worst = 0;
  KD.mcBooth.forEach((p) => { worst = Math.max(worst, Math.abs(p.mc - p.booth) / Math.sqrt(p.booth * (1 - p.booth) / p.n)); });
  report("28 CRP-6 MC vs Booth", `worst ${worst.toFixed(2)} standard errors over ${KD.mcBooth.length} points`);
  check("28: worst |MC − Booth| in standard errors", worst, 3);
}
// 29. The code's lumping: five classes; the big one is the nominal 1e-19.
{
  check("29: number of diffusion classes in boon-lay's code", Math.abs(KD.dClasses.length - 5), 0);
  const big = KD.dClasses.reduce((m, c) => (c.members.length > m.members.length ? c : m));
  check("29: nominal class D is one constant, kernel = graphite", big.kernel.every((v) => v === big.kernel[0]) && big.kernel[0] === big.graphite[0] ? 0 : 1, 0);
  report("29 nominal class", `${big.members.length} elements at ${big.kernel[0]} m²/s`);
  check("29: noble gases are the only non-plating elements", KD.elements.filter((e) => !e.plates).every((e) => e.group === "NobleGas") ? 0 : 1, 0);
}
// 30. The kernel's temperature rise against tampines' particle solution.
KD.profiles.forEach((p) => {
  const rise = p.t[0] - p.tSurf;
  report(`30 kernel rise at ${p.tSurf} K`, `${rise.toFixed(4)} K here, ${p.tampinesRise.toFixed(4)} K tampines`);
  check(`30: |rise − tampines| at ${p.tSurf} K [K]`, Math.abs(rise - p.tampinesRise), 0.5);
});
// 31. The page's sphere walk against Booth.
{
  const x = 0.01, w = KP.walkSphere(4000, x, 400, 31, 0), b = KP.boothF(x);
  report("31 sphere walk", `walk ${w.f.toFixed(4)} vs Booth ${b.toFixed(4)} at D t / a² = ${x}`);
  check("31: Booth − walk (predicted 0 to 0.03)", (b - w.f) >= -3 * Math.sqrt(b * (1 - b) / 4000) && (b - w.f) < 0.03 ? 0 : 1, 0);
}
// 32. The recoil skin, and 33 the lognormal helper's median.
{
  const r = KP.recoilEscape(0.01, 400000, 32);
  report("32 recoil escape at range/radius 0.01", r.toExponential(4));
  check("32: |escape / (0.75 × 0.01) − 1|", Math.abs(r / 0.0075 - 1), 0.05);
  const rr = KP.lognormalRadii(5e-6, 0.4, 61);
  check("33: lognormal middle radius = median (relative)", Math.abs(rr[30] / 5e-6 - 1), 1e-9);
  check("33: erf(erfinv(0.3)) − 0.3", Math.abs(KP.erf(KP.erfinv(0.3)) - 0.3), 1e-12);
}
// 34. The Potts coarsening coarsens.
{
  const s = KP.pottsInit(64, 32, 34), b0 = KP.boundaryFraction(s);
  for (let i = 0; i < 50; i++) KP.pottsSweep(s, 0.3);
  const b1 = KP.boundaryFraction(s);
  report("34 Potts", `boundary fraction ${b0.toFixed(3)} → ${b1.toFixed(3)} after 50 sweeps`);
  check("34: boundary did not shrink", b1 < b0 ? 0 : 1, 0);
}
// 35. The chapter's pages: banner and folded section.
for (const f of fs.readdirSync(__dirname).filter((f) => f.startsWith("kernel-") && f.endsWith(".html"))) {
  const html = fs.readFileSync(path.join(__dirname, f), "utf8");
  check(`${f}: banner or "Learn more advanced stuff" missing`, html.includes("Research, education and V&amp;V only") && html.includes("Learn more advanced stuff") ? 0 : 1, 0);
}

// ─── Chapter 4, "TRISO layer migration and particle failure" (29-38) ─────
// Data: barrier-data.js and failure-data.js (crates/dhoby-ghaut/examples/
// lesson_barriers), barrier-recorded.js (cited / pending), barrier-physics.js
// (the page models). The predictions P1-P15 were written in the generator's
// docs BEFORE the runs they test; P4 and P5 could not be tested (boon-lay's
// walker reached no release; see 38) and were replaced by P13-P14, written
// before their own run. Results 2026-10-22 in the comments.
for (const f of ["barrier-data.js", "failure-data.js", "barrier-recorded.js", "barrier-physics.js"]) eval(fs.readFileSync(path.join(__dirname, f), "utf8"));
const BD = window.BARRIER_DATA, FD = window.FAILURE_DATA, BR = window.BARRIER_RECORDED, BP = window.BarrierPhysics;
const wrow = (el, ly) => BD.walker.find((w) => w.el === el && w.layer === ly);
const wD = (el, ly, t) => BP.dAt(t, BD.tC, wrow(el, ly).d);
// 36. The gaps are where the code has todo!(): Kr in the buffer only, and
//     every other (element, layer) pair has a full curve.
{
  let bad = 0;
  BD.walker.forEach((w) => { const gap = w.el === "Kr" && w.layer === "buffer"; if (gap !== (w.d === null)) bad++; if (w.d && w.d.some((x) => !(x > 0))) bad++; });
  check("36: walker gaps exactly at Kr/buffer, every other curve positive", bad, 0);
}
// 37. P1, P2. 2026-10-22: SiC/IPyC crossing-time ratio >= 16.7 (Cs, 800 °C)
//     for Ag, Cs, Sr over 800-1600 °C; Kr IPyC at 1000 °C 4.79e10 s.
{
  const th = BD.thickness;
  let worst = Infinity;
  for (const el of ["Ag", "Cs", "Sr"]) for (let t = 800; t <= 1600; t += 50) worst = Math.min(worst, (th.SiC * th.SiC / wD(el, "SiC", t)) / (th.IPyC * th.IPyC / wD(el, "PyC", t)));
  report("37 P1", `smallest SiC/IPyC crossing-time ratio ${worst.toFixed(1)}`);
  check("37 P1: 10 / smallest SiC-over-IPyC ratio (Ag, Cs, Sr, 800-1600 °C)", 10 / worst, 1);
  const kr = th.IPyC * th.IPyC / wD("Kr", "PyC", 1000);
  report("37 P2", `Kr IPyC crossing at 1000 °C ${kr.toExponential(3)} s`);
  check("37 P2: one year / Kr IPyC crossing time at 1000 °C", 3.15576e7 / kr, 1);
}
// 38. P15: boon-lay's own walker, 2e7 steps at 1600 °C, never reached the
//     SiC. 2026-10-22: Cs 100.0 %, Ag 100.0 %, Sr 99.9 % of steps in the
//     buffer; 8, 60 and 1558 entries into the IPyC; none released.
BD.wos.probes.forEach((p) => {
  const tot = p.perLayer.reduce((a, b) => a + b, 0);
  report(`38 P15 ${p.el}`, `${(100 * p.perLayer[1] / tot).toFixed(2)} % in buffer, ${p.ipycEntries} IPyC entries, max r ${(p.rMax * 1e6).toFixed(1)} µm`);
  check(`38 P15 ${p.el}: steps in SiC/OPyC or released`, p.perLayer[3] + p.perLayer[4] + (p.released ? 1 : 0), 0);
  check(`38 P15 ${p.el}: 0.9 / buffer share of steps`, 0.9 / (p.perLayer[1] / tot), 1);
});
// 39. The exact mean exit time: the page's formula against the generator's
//     (1e-12), and P13 (one layer carries more than half). 2026-10-22: SiC
//     93-99 % for Cs and Ag, kernel 96-100 % for Sr, 1000-1800 °C.
{
  let wr = 0, low = 1;
  BD.wos.exact.forEach((x) => {
    const D = [wD(x.el, "kernel", x.tC), wD(x.el, "buffer", x.tC), wD(x.el, "PyC", x.tC), wD(x.el, "SiC", x.tC), wD(x.el, "PyC", x.tC)];
    wr = Math.max(wr, Math.abs(BP.exactMean(BD.radii, D) - x.mean) / x.mean);
    const want = x.el === "Sr" ? 0 : 3;
    low = Math.min(low, x.terms[want] / x.mean);
  });
  check("39: |page exact mean − generator| / mean (worst)", wr, 1e-12);
  report("39 P13", `smallest share of the predicted layer ${(100 * low).toFixed(1)} %`);
  check("39 P13: 0.5 / smallest share of the predicted dominant layer", 0.5 / low, 1);
}
// 40. P14: the page's lattice walk against the exact mean, 400 atoms each.
//     2026-10-22 (node): worst Ag 1200 °C −14.2 % (s.e. 3.6 %); others
//     within 9.6 %.
for (const el of ["Cs", "Ag", "Sr"]) for (const t of [1200, 1400, 1600, 1800]) {
  const D = [wD(el, "kernel", t), wD(el, "buffer", t), wD(el, "PyC", t), wD(el, "SiC", t), wD(el, "PyC", t)];
  const ex = BP.exactMean(BD.radii, D), r = BP.latticeWalk(BD.radii, D, 400, 14 + t, 1e9);
  check(`40 P14 ${el} ${t} °C: atoms capped`, r.capped, 0);
  check(`40 P14 ${el} ${t} °C: |lattice mean − exact| / exact`, Math.abs(BP.mean(r.times) - ex) / ex, 0.15);
}
// 41. P6: re-measure the fuel_failure::htr10 docs' tables (2026-09-24),
//     within the figures they print. 2026-10-22: sigma 50.2/178.5/370.3/
//     536.5 MPa; phi1 4.529e-9/3.103e-5/4.794e-3/6.069e-2; phi2 3.442e-15
//     and 0.9770; end of irradiation 2.776e-15, 1.214e-12, 4.988e-9, 1.577e-6.
{
  const f = FD.rowFields, at = (t) => { const H = FD.holds.find((x) => x.tC === t && !x.gb); return H.rows.find((r) => r[0] === 200); };
  const doc = [[1200, 50, 4.53e-9], [1600, 178, 3.10e-5], [2000, 370, 4.79e-3], [2200, 536, 6.07e-2]];
  doc.forEach(([t, s, p]) => {
    const r = at(t);
    check(`41 P6 ${t} °C: |sigma − ${s} MPa| [MPa]`, Math.abs(r[f.indexOf("sigma")] / 1e6 - s), 0.6);
    check(`41 P6 ${t} °C: |phi1 / ${p} − 1| (3 figures)`, Math.abs(r[f.indexOf("phi1")] / p - 1), 0.002);
  });
  check("41 P6 1600 °C: |phi2 / 3.4e-15 − 1| (2 figures)", Math.abs(at(1600)[f.indexOf("phi2")] / 3.4e-15 - 1), 0.02);
  check("41 P6 2200 °C: |phi2 − 0.977|", Math.abs(at(2200)[f.indexOf("phi2")] - 0.977), 0.0005);
  [[700, 2.8e-15], [776, 1.2e-12], [900, 5.0e-9], [1000, 1.6e-6]].forEach(([t, p], k) => check(`41 P6 end of irradiation T_B ${t} °C: |phi1 / ${p} − 1| (2 figures)`, Math.abs(FD.eoi.phi1[k] / p - 1), 0.03));
}
// 42. P7 and the page's ports. 2026-10-22: stress and Weibull exact to
//     round-off; phi1(sigma_o) = 0.5 exactly; p_gas + p_CO = p to 1e-16.
{
  const g = FD.geometry;
  let ws = 0, wsp = 0;
  FD.pressure.rows.forEach((row) => row.p.forEach((p, i) => {
    ws = Math.max(ws, Math.abs(BP.stress(p, g.rMean, g.d0, 1) - row.sigma[i]) / row.sigma[i]);
    wsp = Math.max(wsp, Math.abs(row.pGas[i] + row.pCo[i] - p) / p);
  }));
  check("42: |page stress − induced_stress_with_thinning_factor| / sigma (worst)", ws, 1e-12);
  check("42 P7: |p_gas + p_CO − p| / p (worst)", wsp, 1e-12);
  let ww = 0;
  FD.weibullCheck.forEach(([s, s0, m, phi]) => { ww = Math.max(ww, Math.abs(BP.weibull(s, s0, m) - phi) / Math.max(phi, 1e-300)); });
  check("42: |page Weibull − weibull_failure_fraction| / phi (worst)", ww, 1e-12);
  check("42 P7: |phi1(sigma_o) − 0.5|", Math.abs(FD.weibullCheck[0][3] - 0.5), 1e-12);
  let wt = 0, f = FD.rowFields;
  FD.holds.forEach((H) => H.rows.forEach((r) => { wt = Math.max(wt, Math.abs(BP.total(0, r[f.indexOf("phi1")], r[f.indexOf("phi2")]) - r[f.indexOf("total")])); }));
  check("42: |page total(0, phi1, phi2) − total_failure_fraction| (worst)", wt, 1e-15);
}
// 43. P8-P12. 2026-10-22: GB on/off at 1600 °C, 500 h = 62.8; phi2 > phi1
//     at 2200 and not at 2000 °C; heat-ups never fall; Cs Booth 0.9999996,
//     Sr 0.0827 at 1600 °C, 200 h; P12 MISSED: p(0.12)/p(0.04) = 3.000
//     exactly, because in this model neither the released gas fraction nor
//     the CO depends on burnup, so the pressure is linear in it.
{
  const f = FD.rowFields, last = (t, gb) => { const H = FD.holds.find((x) => x.tC === t && x.gb === gb); return H.rows[H.rows.length - 1]; };
  const gb = last(1600, true)[f.indexOf("phi1")] / last(1600, false)[f.indexOf("phi1")];
  report("43 P8", `grain-boundary attack raises phi1 at 1600 °C, 500 h by ${gb.toFixed(1)}`);
  check("43 P8: 3 / (GB on / GB off)", 3 / gb, 1);
  const at = (t) => { const H = FD.holds.find((x) => x.tC === t && !x.gb); return H.rows.find((r) => r[0] === 200); };
  check("43 P9: phi2 > phi1 at 2200 °C and not at 2000 °C", at(2200)[f.indexOf("phi2")] > at(2200)[f.indexOf("phi1")] && at(2000)[f.indexOf("phi2")] < at(2000)[f.indexOf("phi1")] ? 0 : 1, 0);
  let falls = 0;
  FD.ramps.forEach((R) => R.rows.forEach((r, k) => { if (k && (r[f.indexOf("phi1")] < R.rows[k - 1][f.indexOf("phi1")] || r[f.indexOf("phi2")] < R.rows[k - 1][f.indexOf("phi2")])) falls++; }));
  check("43 P10: steps where phi1 or phi2 fall on the heat-ups", falls, 0);
  const H = FD.holds.find((x) => x.tC === 1600 && !x.gb), j = H.rows.findIndex((r) => r[0] === 200);
  report("43 P11", `Booth kernel release at 1600 °C, 200 h: Cs ${H.release.Cs[j].toPrecision(7)}, Sr ${H.release.Sr[j].toPrecision(4)}`);
  check("43 P11: 1 − Cs Booth release (≤ 0.01)", 1 - H.release.Cs[j], 0.01);
  check("43 P11: Sr Booth release outside 0.05-0.15", H.release.Sr[j] >= 0.05 && H.release.Sr[j] <= 0.15 ? 0 : 1, 0);
  const b = FD.burnup, ratio = b.p1600at200h[5] / b.p1600at200h[1];
  report("43 P12 (MISSED: predicted 3 < ratio < 6)", `p(0.12 FIMA) / p(0.04 FIMA) = ${ratio.toFixed(6)}: pressure is linear in burnup in this model`);
}
// 44. The chapter's pages: banner, folded section, nav entry; the required
//     boon-lay fuel failure wording on lessons 33 and 38; no page attributes a
//     number to PANAMA.
{
  const kit = fs.readFileSync(path.join(__dirname, "../neutronics/lesson-kit.js"), "utf8");
  const ch4 = ["barriers-why.html", "barriers-migration.html", "barriers-sic.html", "barriers-whole.html", "failure-vessel.html", "failure-weibull.html", "failure-thinning.html", "failure-accident.html", "failure-release.html", "failure-leaves-out.html"];
  const wording = "boon-lay fuel failure is inspired by PANAMA: a clean-room, agentic implementation of the equations published in the PANAMA-I report (Verfondern &amp; Nabielek, HTA-IB-03/90). It is not a port: the PANAMA source code was not available.";
  ch4.forEach((f) => {
    const html = fs.readFileSync(path.join(__dirname, f), "utf8");
    check(`44 ${f}: banner or "Learn more advanced stuff" missing`, html.includes("Research, education and V&amp;V only") && html.includes("Learn more advanced stuff") ? 0 : 1, 0);
    check(`44 ${f}: not in the lesson list`, kit.includes(`"${f}"`) ? 0 : 1, 0);
    check(`44 ${f}: "PANAMA's" result or "PANAMA gives/computed"`, /PANAMA(&rsquo;|')s (number|result|value)|PANAMA (gives|computes|computed)/.test(html.replace("&ldquo;PANAMA gives this for HTR-10&rdquo;", "")) ? 1 : 0, 0);
  });
  ["failure-vessel.html", "failure-leaves-out.html"].forEach((f) => check(`44 ${f}: the required boon-lay fuel failure wording`, fs.readFileSync(path.join(__dirname, f), "utf8").includes(wording) ? 0 : 1, 0));
  check("44: barrier-recorded.js cites Kugeler Table 5 on pp. 32-33", BR.layerJobs.page === "32-33" && BR.layerJobs.sic.length === 2 ? 0 : 1, 0);
}

// ─── Chapter 5, "Integrating it in TRISO-ATOPS" (lessons 39-46) ──────────
// Data: atops-data.js (crates/dhoby-ghaut/examples/lesson_triso_atops),
// atops-physics.js. Predictions Q1-Q9 written in the generator's docs
// before the first run. Results 2026-10-22 in the comments.
for (const f of ["atops-data.js", "atops-physics.js"]) eval(fs.readFileSync(path.join(__dirname, f), "utf8"));
const AT = window.ATOPS_DATA, AP = window.AtopsPhysics;
// 45. Q1: 84 nuclides, 5 groups, and each group sits in one diffusion class
//     (special metals in Cs-like or Sr-like). 2026-10-22: no exception.
{
  check("45 Q1: number of nuclides vs 84", Math.abs(AT.nuclides.length - 84), 0);
  const want = { NobleGas: [0], Halogen: [0], SpecialMetal: [1, 2], Silver: [3], Other: [4] };
  check("45 Q1: number of groups vs 5", Math.abs(Object.keys(AT.nuclides.reduce((m, n) => (m[n.group] = 1, m), {})).length - 5), 0);
  check("45 Q1: nuclides whose diffusion class is outside their group's", AT.nuclides.filter((n) => !want[n.group].includes(n.dClass)).length, 0);
  check("45: the short-lived rule (t1/2 < 0.2 t_irrad)", AT.nuclides.filter((n) => n.short !== (n.t12 / AT.caseA.tIrrad < 0.2)).length, 0);
}
// 46. Q2: clamps read from the output. 2026-10-22: Cs 700/550, Sr 700/800,
//     Ag graphite 475 on the 25 °C grid, none for Kr, the Ag kernel, Ag-SiC.
{
  const c = (z) => AT.dCurves.find((x) => x.z === z);
  check("46 Q2: Cs clamps 700/550 °C", c(55).clampKernel === 700 && c(55).clampGraphite === 550 ? 0 : 1, 0);
  check("46 Q2: Sr clamps 700/800 °C", c(38).clampKernel === 700 && c(38).clampGraphite === 800 ? 0 : 1, 0);
  check("46 Q2: Ag graphite clamp 475-500 °C, no Ag kernel clamp", c(47).clampGraphite >= 475 && c(47).clampGraphite <= 500 && c(47).clampKernel === null ? 0 : 1, 0);
  check("46 Q2: no clamp on the Kr kernel", c(36).clampKernel === null ? 0 : 1, 0);
  let flat = 0; for (let i = 1; i < AT.dSicAg.length; i++) if (AT.dSicAg[i] === AT.dSicAg[0]) flat++;
  check("46 Q2: Ag in SiC never flat", flat, 0);
}
// 47. Q3: the Booth floor. 2026-10-22: 1.215976e-4 from both, at 1e-16.
//     The page's truncated series against the port's to 1e-12 (relative to
//     the value, worst over the grid).
{
  const floor = 6 / (Math.PI * Math.PI) * (1 / 4999.5); // the tail sum_{n>=5000} 1/n^2 ~ 1/(N - 1/2)
  report("47 Q3", `booth_longlived(1e-16) = ${AT.booth.long[0].toExponential(6)}, booth_transient = ${AT.booth.transient[0].toExponential(6)}, tail estimate ${floor.toExponential(6)}`);
  check("47 Q3: |floor / 1.215976e-4 − 1|", Math.abs(AT.booth.long[0] / 1.215976e-4 - 1), 1e-3);
  check("47 Q3: |transient floor / 1.215976e-4 − 1|", Math.abs(AT.booth.transient[0] / 1.215976e-4 - 1), 1e-3);
  let w = 0; AT.booth.x.forEach((x, i) => { w = Math.max(w, Math.abs(AP.boothTrunc(x, AT.booth.terms) - AT.booth.long[i]) / Math.max(AT.booth.long[i], 1e-300)); });
  check("47: |page truncated Booth − booth_longlived| / value (worst)", w, 1e-9);
}
// 48. Q4: the Booth radius sqrt(2 a_grain r). 2026-10-22: 6.5269e-5 m.
check("48 Q4: |a_booth − sqrt(2 a_grain r)| / a_booth", Math.abs(AT.caseA.aBooth - Math.sqrt(2 * AT.caseA.aGrain * AT.caseA.r)) / AT.caseA.aBooth, 1e-15);
check("48 Q4: |a_booth / 6.53e-5 − 1|", Math.abs(AT.caseA.aBooth / 6.53e-5 - 1), 0.001);
// 49. Q5, Q6, and the page's rescaling against the code. 2026-10-22: ratio
//     2.000000000000 (Ag 1); bridged Kr-88 0.465116, Cs-137 0.871508.
{
  const c = AT.caseA, f0 = { hm: c.fHM, sic: c.fSiC, inc: c.fInc, incSic: c.fIncSiC };
  const f2 = { hm: 2 * c.fHM, sic: 2 * c.fSiC, inc: 2 * c.fInc, incSic: 2 * c.fIncSiC }, fb = { hm: c.fHM, sic: c.fSiC, inc: AT.phi1Eoi776, incSic: c.fIncSiC };
  let wd = 0, wb = 0;
  AT.nodes.forEach((n) => {
    wd = Math.max(wd, Math.abs(n.doubled[0] / n.base[0] - AP.releaseScale(n.group, f2, f0)));
    wb = Math.max(wb, Math.abs(n.bridged[0] / n.base[0] - AP.releaseScale(n.group, fb, f0)));
  });
  check("49 Q5: |doubled / base − page scale| (worst)", wd, 1e-12);
  check("49: |bridged / base − page scale| (worst)", wb, 1e-12);
  const kr = AT.nodes.find((n) => n.name === "Kr-88");
  check("49 Q6: |Kr-88 bridged ratio − 0.465|", Math.abs(kr.bridged[0] / kr.base[0] - 0.465), 0.001);
  check("49 Q5: Ag-110m unchanged by doubling", AT.nodes.find((n) => n.name === "Ag-110m").doubled[0] === AT.nodes.find((n) => n.name === "Ag-110m").base[0] ? 0 : 1, 0);
}
// 50. Q7: live pools (NOT a port) from empty reproduce the circulating pool.
//     2026-10-22: relative difference 0 for all five.
AT.nodes.forEach((n) => check(`50 Q7 ${n.name}: |live circulating − node| / node`, Math.abs(n.live[0] - n.base[3]) / n.base[3], 1e-9));
// 51. Q8, Q9: venting and the accident. 2026-10-22: vent 0.356 at the end of
//     the hold against 1 − T0/Tpeak = 0.374 (Q8 MISSED, half the first step
//     dropped); never defined while cooling (as predicted); Cs kernel 0.9979.
{
  const a = AT.accidents.find((x) => x.peak === 1600), cs = a.nuclides.find((n) => n.name === "Cs-137");
  const ideal = 1 - (900 + 273.15) / (1600 + 273.15);
  report("51 Q8 (MISSED: predicted within 2 %)", `vent ${cs.vent[20].toFixed(4)} vs ideal ${ideal.toFixed(4)} (${(100 * (cs.vent[20] / ideal - 1)).toFixed(1)} %)`);
  let cool = 0; a.nuclides.forEach((n) => n.vent.forEach((v, k) => { if (k > 20 && v !== null) cool++; }));
  check("51 Q8: vent samples defined while cooling", cool, 0);
  check("51 Q9: 0.99 / Cs kernel release at the end of the hold", 0.99 / cs.rfKernel[20], 1);
  let bad = 0; AT.accidents.forEach((x) => x.nuclides.forEach((n) => { n.rfKernel.forEach((v, k) => { if (v < 0 || v > 1 || (k && v < n.rfKernel[k - 1] - 1e-15)) bad++; }); if ((n.group === "NobleGas" || n.group === "Halogen") && n.rfGraphite.some((v) => v !== 0)) bad++; }));
  check("51: kernel release outside [0,1] or falling; gas held in graphite", bad, 0);
}
// 52. The chapter's pages: banner, folded section, nav entry, a Code Review
//     link, and the paper cited, never quoted at length.
{
  const kit = fs.readFileSync(path.join(__dirname, "../neutronics/lesson-kit.js"), "utf8");
  ["atops-big-picture.html", "atops-groups.html", "atops-diffusion.html", "atops-release-models.html", "atops-normal-operation.html", "atops-coolant.html", "atops-accident.html", "atops-simplifications.html"].forEach((f) => {
    const html = fs.readFileSync(path.join(__dirname, f), "utf8");
    check(`52 ${f}: banner or "Learn more advanced stuff" missing`, html.includes("Research, education and V&amp;V only") && html.includes("Learn more advanced stuff") ? 0 : 1, 0);
    check(`52 ${f}: not in the lesson list`, kit.includes(`"${f}"`) ? 0 : 1, 0);
    check(`52 ${f}: no Code Review link`, f === "atops-simplifications.html" || html.includes("../../code-review/#") ? 0 : 1, 0);
    // A quotation from the restricted paper would sit in &ldquo;...&rdquo;; allow
    // only single terms (at most three words), titles and the tool's name.
    const quotes = (html.match(/&ldquo;([^&]*)&rdquo;/g) || []).filter((q) => q.split(" ").length > 3 && !/step|Through the layers|Simplify|TRISO-ATOPS: lumped|FRESCO-style|Levels of fidelity|Why each simplification|Getting out|PANAMA gives|Analysis Tool for Predictive|A Mechanistic Source Term Model/.test(q));
    check(`52 ${f}: long quotations (restricted paper)`, quotes.length, 0);
  });
}

if (failed) { console.log(`${failed} check(s) failed`); process.exit(1); }
console.log("all fission-products checks passed");
