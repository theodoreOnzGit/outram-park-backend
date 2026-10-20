// The Fission products lessons' physics, checked under node (no browser):
// `node docs/site/science/fission-products/check-fission.js`, run by
// scripts/build-pages.sh beside the neutronics and transport checks, so a
// regression fails the Pages build. Each check says what it compares
// against; the results of 2026-10-20 are in the comments. Lines marked
// "REPORT" print a measured disagreement that the page shows as it is; they
// are not gates.
"use strict";
const fs = require("fs"), path = require("path");
global.window = {};
for (const f of ["fission-data.js", "fission-pending.js", "fission-physics.js"]) eval(fs.readFileSync(path.join(__dirname, f), "utf8"));
const F = window.FissionPhysics, D = window.FISSION_DATA;
const page = fs.readFileSync(path.join(__dirname, "one-fission.html"), "utf8");
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
//    evaluation's prompt ν̄_p (MT=456). Two independent parts of
//    ENDF/B-VIII.0; gate 0.05 neutron. 2026-10-20: U-235 thermal 2.4000 vs
//    2.4140.
const nuYields = {};
for (const [name, P] of Object.entries(D.parents)) {
  P.yields.forEach((Y, ie) => {
    let sa = 0; for (let i = 0; i < Y.a.length; i++) sa += Y.y[i] * Y.a[i];
    const nu = P.a + 1 - sa;
    nuYields[name + ie] = nu;
    check(`${name} ${Y.e} eV: |ν(yield mass balance) ${nu.toFixed(4)} − ν̄_p ${Y.nup.toFixed(4)}|`, Math.abs(nu - Y.nup), 0.05);
  });
}

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

// 8. The numbers the page quotes are the ones computed here, to the shown
//    precision.
function quoted(key) { const m = new RegExp(`data-chk="${key}">([0-9.]+)<`).exec(page); return m ? +m[1] : NaN; }
check("page's ν from yields vs computed", Math.abs(quoted("nuYields") - nuYields["U-2350"]), 5e-4);
check("page's pair-construction ν vs computed", Math.abs(quoted("nuPairs") - nuPairs), 5e-3);
check("page's delayed per fission vs computed", Math.abs(quoted("nuDelayed") - nuDel), 5e-4);

// 9. Timescales the page computes (printed for the record).
{
  const T = F.timescales("U-235", 0);
  report("U-235 thermal timescales", `2R/v = ${T.transit.toExponential(3)} s (R ${(T.radius_m * 1e15).toFixed(2)} fm); τ = ħ/⟨Γ⟩ = ${T.tauCN.toExponential(3)} s (⟨Γ⟩ ${T.gammaMean} eV)`);
}

if (failed) { console.log(`${failed} check(s) failed`); process.exit(1); }
console.log("all fission-products checks passed");
