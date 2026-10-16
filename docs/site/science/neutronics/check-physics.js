// The neutronics lessons' physics, checked under node (no browser):
// `node docs/site/science/neutronics/check-physics.js`, run by
// scripts/build-pages.sh so a regression fails the Pages build. Each check
// states what it compares against; the results on 2026-10-16 are in the
// comments.
"use strict";
const fs = require("fs"), path = require("path");
global.window = {}; window.Lesson = { widget() {}, css() {}, el() {} };
eval(fs.readFileSync(path.join(__dirname, "qscatter.js"), "utf8"));
eval(fs.readFileSync(path.join(__dirname, "shapes.js"), "utf8"));
eval(fs.readFileSync(path.join(__dirname, "angular-data.js"), "utf8"));
eval(fs.readFileSync(path.join(__dirname, "angles.js"), "utf8"));
eval(fs.readFileSync(path.join(__dirname, "xs-data.js"), "utf8"));
eval(fs.readFileSync(path.join(__dirname, "energy.js"), "utf8"));
const M = window.Lesson.qscatterMath, SH = window.Lesson.shapesMath, AN = window.Lesson.anglesMath;
let failed = 0;
function check(name, value, limit) {
  const ok = value <= limit;
  if (!ok) failed++;
  console.log(`${ok ? "ok  " : "FAIL"} ${name}: ${value.toExponential(2)} (limit ${limit})`);
}

// 1. The hard-sphere phase shift is minus NJOY's facphi (reconr.f90:4441,
//    njoy-outram-park-fork reconr/slbw.rs::phase_shift), for every l NJOY
//    tabulates. 2026-10-16: 3.7e-16.
function facphi(l, r) {
  const r2 = r * r;
  return [r, r - Math.atan(r), r - Math.atan(3 * r / (3 - r2)),
    r - Math.atan((15 * r - r * r2) / (15 - 6 * r2)),
    r - Math.atan((105 * r - 10 * r2 * r) / (105 - 45 * r2 + r2 * r2))][l];
}
let worst = 0;
for (const x of [0.05, 0.3, 1, 2.5, 7]) {
  const j = new Float64Array(9), y = new Float64Array(9);
  M.sphJY(8, x, j, y);
  for (let l = 0; l <= 4; l++) worst = Math.max(worst, Math.abs(Math.sin(Math.atan(j[l] / y[l]) + facphi(l, x))));
}
check("phase shift vs -facphi, l <= 4", worst, 1e-12);

// 2. Two routes to σ agree: the partial-wave sum (4π/k²) Σ (2l+1) sin² δ_l
//    and ∫ dσ/dΩ dΩ over the plotted grid of |f(θ)|²; at low energy
//    σ -> 4πa². 2026-10-16: C-12 at 10 eV 4.7481 b, 4πa² to 5e-6; U-238 at
//    2 MeV 7.9485 b, grid integral off by 4.7e-5 (the trapezoid rule).
for (const [label, A, ap, E] of [["C-12, 10 eV", 11.89365, 6.146927, 10], ["U-238, 2 MeV", 236.0058, 9.48, 2e6], ["C-12, 20 MeV", 11.89365, 6.146927, 2e7]]) {
  const S = M.solve(A, ap, E);
  let integral = 0;
  for (let i = 0; i < S.NM - 1; i++) {
    const m0 = Math.cos(Math.PI * i / (S.NM - 1)), m1 = Math.cos(Math.PI * (i + 1) / (S.NM - 1));
    integral += 0.5 * (S.f[i] + S.f[i + 1]) * (m0 - m1);
  }
  integral *= 2 * Math.PI;
  check(label + ": integral of dσ/dΩ vs the partial-wave sum", Math.abs(integral - S.sig) / S.sig, 1e-3);
  if (E < 100) check(label + ": σ vs 4πa²", Math.abs(S.sig - 4 * Math.PI * ap * ap) / S.sig, 1e-4);
}

// 3. The drawn wave obeys the boundary condition of an impenetrable sphere:
//    the total wave e^{ikz} + ψ_sc, through the same scattered() the page
//    draws, vanishes on r = a. 2026-10-16: 6.4e-9 (U-238, 2 MeV), against an
//    incident amplitude of 1. And field() masks the inside of the sphere.
{
  const S = M.solve(236.0058, 9.48, 2e6), cf = M.coefficients(S), n = S.n;
  const j = new Float64Array(n + 1), y = new Float64Array(n + 1), p = new Float64Array(n + 1);
  let mx = 0;
  for (let t = 0; t <= 20; t++) {
    const mu = Math.cos(Math.PI * t / 20), r = 9.48 * (1 + 1e-9), w = M.scattered(S, cf, r, mu, j, y, p);
    mx = Math.max(mx, Math.hypot(Math.cos(S.k * r * mu) + w[0], Math.sin(S.k * r * mu) + w[1]));
  }
  check("U-238, 2 MeV: |total wave| on the sphere", mx, 1e-6);
  const F = M.field(S, 9.48, 30, 8, 8);
  if (!F.inside.some(Boolean)) { failed++; console.log("FAIL field(): no grid point masked inside the sphere"); }
}

// 4. Page 6's two representations converge on the curve they store, and
//    the points pass through it. 2026-10-16, worst error as a fraction of the
//    peak: points 4/8/16/32/64 -> 0.18/0.095/0.032/0.0084/0.0021; cosines
//    1/4/16 -> 0.81/0.19/8e-13. Sixteen is exact, not lucky: the curve is
//    |Σ_l c_l P_l(cos θ)|² with significant l <= 5 at this energy, a finite
//    cosine series in θ (the page says so). The Bessel function by its integral form
//    against tabulated values (Abramowitz & Stegun table 9.1): J0(2.404825558)
//    = 0, J1(1) = 0.4400505857.
{
  const f = SH.target();
  let prev = Infinity;
  for (const n of [4, 8, 16, 32, 64]) {
    const r = SH.byPoints(f, n), e = SH.worst(f, r.y);
    if (!(e < prev)) { failed++; console.log(`FAIL points: ${n} points no better than fewer (${e})`); }
    prev = e;
  }
  check("64 points: worst error / peak", prev, 0.005);
  check("16 cosines: worst error / peak", SH.worst(f, SH.bySeries(f, 16).y), 1e-6);
  check("J0 at its first zero", Math.abs(SH.besselJ(0, 2.404825558)), 1e-9);
  check("J1(1) vs 0.4400505857", Math.abs(SH.besselJ(1, 1) - 0.4400505857), 1e-9);
}

// 5. Page 7's real data. For every held energy of C-12 and U-238:
//    (a) the evaluation's Legendre density integrates to 1 (a_0 = 1);
//    (b) it agrees with ACER's table at the table's cosines within ptleg2's
//        own tolerances (0.02 % rebuild, 0.2 % thinning, then the area
//        normalisation), wherever the density is not clamped near zero;
//    (c) the lin-lin sampler's mean cosine over 400 000 draws matches the
//        table's exact mean, and the table's mean matches a_1, the
//        evaluation's mean cosine, within the thinning.
//    2026-10-16, worst over the ten: (a) 1.5e-7, (b) 1.1e-3, (c) 2.0e-3
//    for the sampler (its standard error is ~9e-4, so 2.1 sd; the seed is
//    fixed, so the check is repeatable) and 5.3e-4 for a_1.
{
  let worstInt = 0, worstTab = 0, worstMean = 0, worstA1 = 0;
  let seed = 99;
  const rand = () => { seed ^= seed << 13; seed ^= seed >>> 17; seed ^= seed << 5; return (seed >>> 0) / 4294967296; };
  for (const nuc of ["C-12", "U-238"]) {
    for (const R of window.ANGULAR_DATA[nuc]) {
      const NL = R.a.length, N = 20000;
      let integral = 0;
      for (let i = 0; i < N; i++) {
        const m0 = -1 + 2 * i / N, m1 = -1 + 2 * (i + 1) / N;
        integral += 0.5 * (AN.density(R.a, NL, m0) + AN.density(R.a, NL, m1)) * (m1 - m0);
      }
      worstInt = Math.max(worstInt, Math.abs(integral - 1));
      let pk = 0; for (const v of R.pdf) pk = Math.max(pk, v);
      for (let i = 0; i < R.mu.length; i++) {
        const d = AN.density(R.a, NL, R.mu[i]);
        if (d > 1e-3 * pk) worstTab = Math.max(worstTab, Math.abs(R.pdf[i] / d - 1));
      }
      // The exact mean of the lin-lin table: ∫ μ p(μ) dμ bin by bin.
      let tmean = 0;
      for (let i = 0; i + 1 < R.mu.length; i++) {
        const a = R.mu[i], b = R.mu[i + 1], pa = R.pdf[i], pb = R.pdf[i + 1], m = (pb - pa) / (b - a);
        tmean += (pa - m * a) * (b * b - a * a) / 2 + m * (b * b * b - a * a * a) / 3;
      }
      let sum = 0; const n = 400000;
      for (let k = 0; k < n; k++) sum += AN.sampleTabular(R.mu, R.pdf, R.cdf, rand());
      worstMean = Math.max(worstMean, Math.abs(sum / n - tmean));
      worstA1 = Math.max(worstA1, Math.abs(tmean - R.a[0]));
    }
  }
  check("Legendre density integrates to 1", worstInt, 1e-6);
  check("ACER table vs Legendre density, relative", worstTab, 0.01);
  check("sampled mean cosine vs the table's (sd of a 400k mean ~ 1e-3)", worstMean, 4e-3);
  check("table mean cosine vs a_1", worstA1, 2e-3);
}

// Pages 8-9 (gh:#906 design, 2026-10-16).
{
  const EN = window.Lesson.energyMath;
  // The toy response is 1 on each level and 1/2 at Γ/2 from it.
  let w = 0;
  for (const n of [1, 2, 3, 4, 5, 6]) {
    w = Math.max(w, Math.abs(EN.response(n * n).r - 1), Math.abs(EN.response(n * n + EN.GAMMA / 2).r - 0.5));
  }
  check("toy resonance: 1 on a level, 1/2 at half width", w, 1e-12);
  // Shell closures, computed from (N+1)(N+2) and from 2j + 1: the
  // oscillator's 2, 8, 20, 40, 70, 112, 168 and the magic numbers
  // 2, 8, 20, 28, 50, 82, 126 (Mayer 1949; Haxel, Jensen & Suess 1949).
  const same = (a, b) => (a.length === b.length && a.every((x, i) => x === b[i]) ? 0 : 1);
  check("oscillator shell closures", same(EN.oscillatorClosures(7), [2, 8, 20, 40, 70, 112, 168]), 0);
  check("spin-orbit shell closures are the magic numbers", same(EN.shellModel(), [2, 8, 20, 28, 50, 82, 126]), 0);
  // The cross sections: sorted energies, positive sigma, and the takeaway
  // as numbers. Peaks (a point over 1.5 times both neighbours) at the plot's
  // resolution on 2026-10-16: He-4 0, Mn-55 57, U-238 131; He-4's maximum
  // between 0.3 and 3 MeV at 1.125 MeV.
  const X = window.XS_DATA;
  const peaks = (R) => { let p = 0; for (let i = 1; i + 1 < R.s.length; i++) if (R.s[i] > R.s[i - 1] && R.s[i] > R.s[i + 1] && R.s[i] > 1.5 * Math.min(R.s[i - 1], R.s[i + 1])) p++; return p; };
  let bad = 0;
  for (const k of ["He-4", "Mn-55", "U-238"]) {
    const R = X[k];
    if (!R || R.e.length !== R.s.length || R.e.length < 100) { bad++; continue; }
    for (let i = 0; i < R.e.length; i++) if (!(R.s[i] > 0) || (i && !(R.e[i] > R.e[i - 1]))) bad++;
  }
  check("cross-section data well formed (bad points)", bad, 0);
  const pk = ["He-4", "Mn-55", "U-238"].map((k) => peaks(X[k]));
  check("more nucleons, more peaks (He-4 < Mn-55 < U-238)", pk[0] < pk[1] && pk[1] < pk[2] ? 0 : 1, 0);
  let he = [0, 0]; X["He-4"].e.forEach((e, i) => { if (e > 3e5 && e < 3e6 && X["He-4"].s[i] > he[1]) he = [e, X["He-4"].s[i]]; });
  check("He-4's broad maximum lies 0.8-1.5 MeV (MeV off)", he[0] > 8e5 && he[0] < 1.5e6 ? 0 : Math.abs(he[0] / 1e6 - 1.1), 0);
  console.log(`     peaks He-4 ${pk[0]}, Mn-55 ${pk[1]}, U-238 ${pk[2]}; He-4 max ${he[1].toFixed(2)} b at ${(he[0] / 1e6).toFixed(3)} MeV`);
}

if (failed) { console.log(failed + " check(s) failed"); process.exit(1); }
console.log("all lesson physics checks passed");
