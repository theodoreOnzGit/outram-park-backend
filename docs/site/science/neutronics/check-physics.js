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
eval(fs.readFileSync(path.join(__dirname, "reaction-data.js"), "utf8"));
window.Lesson.fmtE = (e) => e + " eV";
eval(fs.readFileSync(path.join(__dirname, "reactions.js"), "utf8"));
eval(fs.readFileSync(path.join(__dirname, "resonance-data.js"), "utf8"));
eval(fs.readFileSync(path.join(__dirname, "resonance.js"), "utf8"));
eval(fs.readFileSync(path.join(__dirname, "kinematics.js"), "utf8"));
eval(fs.readFileSync(path.join(__dirname, "slowing-data.js"), "utf8"));
eval(fs.readFileSync(path.join(__dirname, "slowing.js"), "utf8"));
eval(fs.readFileSync(path.join(__dirname, "unresolved-data.js"), "utf8"));
eval(fs.readFileSync(path.join(__dirname, "unresolved.js"), "utf8"));
eval(fs.readFileSync(path.join(__dirname, "purr-data.js"), "utf8"));
eval(fs.readFileSync(path.join(__dirname, "purr.js"), "utf8"));
eval(fs.readFileSync(path.join(__dirname, "bound-data.js"), "utf8"));
eval(fs.readFileSync(path.join(__dirname, "bound.js"), "utf8"));
Object.assign(window.Lesson, { H: 6.62607015e-34, MN: 1.67492749804e-27, EV: 1.602176634e-19 });
eval(fs.readFileSync(path.join(__dirname, "bragg.js"), "utf8"));
eval(fs.readFileSync(path.join(__dirname, "doppler-data.js"), "utf8"));
eval(fs.readFileSync(path.join(__dirname, "dbrc-data.js"), "utf8"));
window.Lesson.arrow = () => {};
eval(fs.readFileSync(path.join(__dirname, "lattice.js"), "utf8"));
eval(fs.readFileSync(path.join(__dirname, "manyphonons.js"), "utf8"));
eval(fs.readFileSync(path.join(__dirname, "structure.js"), "utf8"));
eval(fs.readFileSync(path.join(__dirname, "leapr-data.js"), "utf8"));
eval(fs.readFileSync(path.join(__dirname, "spin-data.js"), "utf8"));
eval(fs.readFileSync(path.join(__dirname, "mathtrack.js"), "utf8"));
eval(fs.readFileSync(path.join(__dirname, "dbrc.js"), "utf8"));
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

// Pages 10-11 (gh:#906 design, 2026-10-16): the Q-values the design quotes,
// as read from the tapes into reaction-data.js; the 2 MeV U-238 predict
// answer; U-238's fission onset; and page 11's prose numbers against the data.
{
  const RD = window.REACTION_DATA, RX = window.Lesson.reactionsMath;
  const Q = (n, mt, f) => RD[n].channels.find((c) => c.mt === mt)[f];
  const want = [["U-238", 51, "qi", -4.4916e4], ["C-12", 51, "qi", -4.4389e6], ["U-238", 102, "qm", 4.806e6], ["C-12", 102, "qm", 4.946e6],
    ["B-10", 107, "qm", 2.789e6], ["C-12", 107, "qm", -5.702e6], ["U-238", 16, "qm", -6.153e6], ["U-235", 102, "qm", 6.544e6]];
  let wq = 0;
  for (const [n, mt, f, v] of want) wq = Math.max(wq, Math.abs(Q(n, mt, f) / v - 1));
  check("Q-values vs the design's (relative)", wq, 2e-4);
  const open = RX.channels(RD["U-238"], 2e6).filter((c) => c.open).map((c) => c.mt).sort((a, b) => a - b).join(",");
  check("2 MeV on U-238: open = elastic, inelastic, fission, capture; (n,2n) closed", open === "2,18,51,102" ? 0 : 1, 0);
  const on = RD["U-238"].sf.onset;
  check("U-238 fission onset within 1-1.5 MeV (MeV off)", on > 1e6 && on < 1.5e6 ? 0 : Math.abs(on / 1e6 - 1.25), 0);
  check("U-235 is fissile (sigma_f at 0.0253 eV over 100 b)", RD["U-235"].sf.thermal > 100 ? 0 : 1, 0);
  const html = fs.readFileSync(path.join(__dirname, "fission.html"), "utf8");
  let wp = 0;
  for (const m of html.matchAll(/data-q="(U-23[58])">([0-9.]+) MeV/g)) wp = Math.max(wp, Math.abs(+m[2] - Q(m[1], 102, "qm") / 1e6));
  check("page 11's capture Q in the prose vs the data (MeV)", wp, 0.006);
}

// Pages 12-13 (gh:#906 design, 2026-10-16): the page's SLBW and MLBW
// against OUTRAM PARK's RECONR port (resonance-data.js, written by
// examples/lesson_resonances.rs from reconr::slbw), and MLBW = SLBW for
// levels of different spin.
{
  const RS = window.Lesson.resonanceMath, RD2 = window.RES_DATA, U = RD2.u238, P = RD2.ref.pair;
  const lvl = [[U.er, U.aj, U.gn + U.gg, U.gn, U.gg, 0]];
  let ws = 0;
  RD2.ref.slbw.e.forEach((e, i) => {
    const s = RS.slbw(e, lvl, 0, U.spi, U.ap, U.awri, U.ra);
    ws = Math.max(ws, Math.abs(s.el / RD2.ref.slbw.el[i] - 1), Math.abs(s.cap / RD2.ref.slbw.cap[i] - 1));
  });
  check("page 12 SLBW vs the RECONR port (relative)", ws, 1e-12);
  const pair = (j2) => [[10, 1, 1.03, 1.0, 0.03, 0], [12, j2, 1.03, 1.0, 0.03, 0]];
  let wm = 0, wd = 0;
  P.e.forEach((e, i) => {
    const rel = (a, b) => Math.abs(a - b) / Math.max(Math.abs(b), 1e-9);
    wm = Math.max(wm, rel(RS.mlbw(e, pair(1), 0, P.spi, P.ap, P.awri, P.ra).el, P.mlbwSame[i]), rel(RS.mlbw(e, pair(0), 0, P.spi, P.ap, P.awri, P.ra).el, P.mlbwDiff[i]),
      rel(RS.slbw(e, pair(1), 0, P.spi, P.ap, P.awri, P.ra).el, P.slbwSame[i]));
    wd = Math.max(wd, rel(P.mlbwDiff[i], P.slbwDiff[i]));
  });
  check("page 13 SLBW/MLBW vs the RECONR port (relative)", wm, 1e-11);
  check("MLBW equals SLBW for levels of different spin (relative)", wd, 1e-9);
  let same = 0; P.e.forEach((e, i) => { same = Math.max(same, Math.abs(P.mlbwSame[i] - P.slbwSame[i]) / P.slbwSame[i]); });
  check("MLBW differs from SLBW for the same spin (relative difference over 1 %)", same > 0.01 ? 0 : 1, 0);
  // Pages 13-15: Reich-Moore (reconr::rm, NJOY2016 csrmat) for the same pair,
  // without and with a fission width, and MLBW/SLBW capture and fission.
  const rpair = (j2, gf) => [[10, 1, 1.0, 0.03, gf, 0], [12, j2, 1.0, 0.03, gf, 0]];
  const fpair = [[10, 1, 1.03 + P.gf, 1.0, 0.03, P.gf], [12, 1, 1.03 + P.gf, 1.0, 0.03, P.gf]];
  let wr = 0, wf = 0, mcap = 0, rmcap = 0, rmfis = 0;
  P.e.forEach((e, i) => {
    const rel = (a, b) => Math.abs(a - b) / Math.max(Math.abs(b), 1e-6);
    const a = RS.rm(e, rpair(1, 0), 0, P.spi, P.ap, P.awri, P.ra), b = RS.rm(e, rpair(0, 0), 0, P.spi, P.ap, P.awri, P.ra), c = RS.rm(e, rpair(1, P.gf), 0, P.spi, P.ap, P.awri, P.ra);
    wr = Math.max(wr, rel(a.el, P.rmSame.el[i]), rel(a.cap, P.rmSame.cap[i]), rel(b.el, P.rmDiff.el[i]), rel(b.cap, P.rmDiff.cap[i]), rel(c.el, P.rmFis.el[i]), rel(c.cap, P.rmFis.cap[i]), rel(c.fis, P.rmFis.fis[i]));
    const m = RS.mlbw(e, fpair, 0, P.spi, P.ap, P.awri, P.ra), s = RS.slbw(e, fpair, 0, P.spi, P.ap, P.awri, P.ra);
    wf = Math.max(wf, rel(m.cap, P.mlbwFis.cap[i]), rel(m.fis, P.mlbwFis.fis[i]), rel(s.cap, P.slbwFis.cap[i]), rel(s.fis, P.slbwFis.fis[i]));
    mcap = Math.max(mcap, rel(P.mlbwFis.cap[i], P.slbwFis.cap[i]), rel(P.mlbwFis.fis[i], P.slbwFis.fis[i]));
    rmcap = Math.max(rmcap, rel(P.rmFis.cap[i], P.slbwFis.cap[i]));
    rmfis = Math.max(rmfis, rel(P.rmFis.fis[i], P.slbwFis.fis[i]));
  });
  check("pages 13-15 Reich-Moore vs the RECONR port (relative)", wr, 1e-11);
  check("page 15 MLBW/SLBW capture and fission vs the RECONR port (relative)", wf, 1e-11);
  // ENDF-102 App. D / csmlbw: MLBW's capture and fission are the SLBW sums.
  check("MLBW capture and fission equal the SLBW sums (relative)", mcap, 1e-12);
  check("Reich-Moore fission differs from the SLBW sum for the same spin (over 1 %)", rmfis > 0.01 ? 0 : 1, 0);
  console.log(`     same-spin pair with Γf ${P.gf} eV: Reich-Moore vs SLBW sum up to ${(100 * rmcap).toFixed(0)} % in capture, ${(100 * rmfis).toFixed(0)} % in fission`);
  // One level per spin group: Reich-Moore, MLBW and SLBW coincide (s-wave, no shift).
  let wj = 0;
  P.e.forEach((e, i) => { wj = Math.max(wj, Math.abs(P.rmDiff.el[i] / P.slbwDiff[i] - 1)); });
  check("different spins: Reich-Moore elastic equals the SLBW sum (relative)", wj, 1e-9);
  // Pages 13-16: every census number and level count in the prose against the data.
  const C = RD2.census, tot = Object.values(C).reduce((a, b) => a + b, 0);
  let wc = 0;
  for (const f of ["fission.html", "r-matrix.html", "reich-moore.html", "mlbw.html", "adler-adler.html"]) {
    const h = fs.readFileSync(path.join(__dirname, f), "utf8");
    for (const m of h.matchAll(/data-census="([^"]+)">([0-9]+)</g)) wc += Math.abs(+m[2] - (m[1] === "total" ? tot : C[m[1]]));
    for (const m of h.matchAll(/data-levels="([01])">([0-9 ]+)</g)) wc += Math.abs(+m[2].replace(/ /g, "") - RD2.u238Levels[m[1]]);
  }
  check("pages 11 and 13-16: census and level counts in the prose vs the tapes", wc, 0);
  // The lifetime readout: τ = ħ/Γ for U-238's 6.67 eV level (Γ ≈ 24 meV, τ ≈ 27 fs).
  const tau = RS.HBAR_EV_S / (U.gn + U.gg);
  check("U-238 6.67 eV level lifetime within 20-35 fs", tau > 2e-14 && tau < 3.5e-14 ? 0 : 1, 0);
  console.log(`     U-238 level ${U.er} eV: Γn ${U.gn} eV, Γγ ${U.gg} eV, τ ${(tau * 1e15).toFixed(1)} fs; MLBW-SLBW same spin up to ${(100 * same).toFixed(0)} %`);
}

// Page 17 (gh:#906 design and amendments, 2026-10-16): slowing down on a
// log axis, against closed forms and the tapes (slowing-data.js, written by
// examples/lesson_slowing_down.rs).
{
  const KM = window.Lesson.kinematicsMath, SM = window.Lesson.slowingMath, SD = window.SLOWING_DATA;
  // Page 2's shared kept(A, muCM) against its own vector reflection, over aims.
  let wk = 0;
  for (const A of [SD.awr["H-1"], SD.awr["C-12"], SD.awr["U-238"]]) for (let b = 0; b <= 0.98; b += 0.07) {
    const nx = Math.sqrt(1 - b * b), ny = -b, vn = nx, f = 2 * A / (1 + A);
    const v1 = [1 - f * vn * nx, -f * vn * ny];
    wk = Math.max(wk, Math.abs(v1[0] * v1[0] + v1[1] * v1[1] - KM.kept(A, 1 - 2 * vn * vn)));
  }
  check("page 2's collision: kept(A, muCM) vs the vector reflection", wk, 1e-14);
  // The sampled mean step in ln E converges to xi (C-12, 10^6 collisions, seeded).
  const A = SD.awr["C-12"], r = SM.rng(7), N = 1e6;
  let s1 = 0, s2 = 0;
  for (let i = 0; i < N; i++) { const d = -Math.log(SM.collide(1, A, r())); s1 += d; s2 += d * d; }
  const mean = s1 / N, sd = Math.sqrt(s2 / N - mean * mean) / Math.sqrt(N);
  check("C-12: sampled mean ln(E/E') vs xi, in standard errors", Math.abs(mean - SM.xi(A)) / sd, 3);
  // xi's closed form against a direct average over the uniform law.
  let wx = 0;
  for (const nuc of ["He-4", "C-12", "U-238"]) {
    const a = KM.alpha(SD.awr[nuc]), M = 200000; let q = 0;
    for (let i = 0; i < M; i++) q += -Math.log(a + (1 - a) * (i + 0.5) / M);
    wx = Math.max(wx, Math.abs(q / M / SM.xi(SD.awr[nuc]) - 1));
  }
  check("xi closed form vs the average over [alpha E, E] (relative)", wx, 1e-6);
  // B-10 (n,alpha) follows 1/v: the slope of ln sigma against ln E, 1 meV to 1 eV, from the tape.
  const B = SD.curves["B-10 (n,alpha)"], fit = SM.slope(B.e, B.s, 1e-3, 1);
  check("B-10 (n,alpha) log-log slope, 1 meV to 1 eV, vs -1/2", Math.abs(fit.slope + 0.5), 0.005);
  // Every number in the page's prose against the data.
  const h = fs.readFileSync(path.join(__dirname, "log-scales.html"), "utf8"), l0 = SD.u238.levels["0"], lv = SD.u238.levels;
  const num = (t) => +t.replace(/&minus;/g, "-").replace(/ /g, "");
  const rel = (got, want) => Math.abs(got - want) / Math.max(Math.abs(want), 1e-12);
  let wp = 0, nq = 0;
  // tol "print": agrees to the last digit printed (half a unit of it);
  // a number: relative tolerance, for values the prose rounds loosely ("about").
  const halfUnit = (t) => { const d = t.replace(/&minus;/g, "").replace(/ /g, "").split(".")[1]; return 0.5 * Math.pow(10, -(d ? d.length : 0)); };
  const q = (re, f, tol) => { for (const m of h.matchAll(re)) {
    nq++; const t = m[m.length - 1], want = f(m), got = num(t);
    const bad = tol === "print" ? Math.abs(got - want) > halfUnit(t) * (1 + 1e-9) : rel(got, want) > tol;
    if (bad) console.log("     mismatch", m[0], "want", want);
    wp = Math.max(wp, bad ? 1 : 0);
  } };
  q(/data-alpha="([^"]+)">([^<]+)</g, (m) => KM.alpha(SD.awr[m[1]]), "print");
  q(/data-xi="([^"]+)">([^<]+)</g, (m) => SM.xi(SD.awr[m[1]]), "print");
  q(/data-n1="([^"]+)">([^<]+)</g, (m) => SM.collisions(2e6, 1, SD.awr[m[1]]), "print");
  q(/data-nth="([^"]+)">([^<]+)</g, (m) => SM.collisions(2e6, 0.0253, SD.awr[m[1]]), "print");
  q(/data-meanloss="([^"]+)">([^<]+)</g, (m) => 100 * (1 - KM.alpha(SD.awr[m[1]])) / 2, "print");
  q(/data-perkev="(\d),(\d+),(\d+)">([^<]+)</g, (m) => SM.count(lv[m[1]], +m[2], +m[3]) / ((m[3] - m[2]) / 1e3), "print");
  q(/data-spacing="(\d),(\d+),(\d+)">([^<]+)</g, (m) => { const v = lv[m[1]].filter((e) => e >= +m[2] && e < +m[3]); return (v[v.length - 1] - v[0]) / (v.length - 1); }, "print");
  q(/data-decade="(\d+),(\d+)">([^<]+)</g, (m) => SM.count(l0, +m[1], +m[2]), "print");
  q(/data-gap="([^"]+)">([^<]+)</g, (m) => Math.log(l0[1] / l0[0]) / SM.xi(SD.awr[m[1]]), "print");
  q(/data-jump="([^"]+)">([^<]+)</g, (m) => SM.count(l0, 1e4, 2e4) / Math.log(2) * SM.xi(SD.awr[m[1]]), "print");
  q(/data-doppler>([^<]+)</g, () => Math.sqrt(4 * 2e4 * 8.617333262e-5 * 300 / SD.awr["U-238"]), "print");
  q(/data-b10slope>([^<]+)</g, () => fit.slope, "print");
  q(/data-c12el>([^<]+)</g, () => SM.at(SD.curves["C-12 elastic"], 0.0253), "print");
  const ext = (c, lo, hi, f) => { let v = f === "min" ? Infinity : 0; c.e.forEach((e, i) => { if (e >= lo && e <= hi) v = f === "min" ? Math.min(v, c.s[i]) : Math.max(v, c.s[i]); }); return v; };
  q(/data-u238cap="(min|max)">([^<]+)</g, (m) => ext(SD.curves["U-238 capture"], 1e-5, 2e4, m[1]), 0.05);
  q(/data-b10="(min|max)">([^<]+)</g, (m) => ext(B, 1e-5, 2e7, m[1]), 0.05);
  q(/data-range="eh(1|2)">([^<]+)</g, (m) => SD.u238.ranges[+m[1] - 1].eh / 1e3, 0.005);
  check("page 17: " + nq + " numbers in the prose vs the data", wp, 0);
  console.log(`     B-10 (n,alpha) slope ${fit.slope.toFixed(5)} over ${fit.n} points; C-12 sampled <ln E/E'> ${mean.toFixed(5)} vs xi ${SM.xi(A).toFixed(5)}`);
}

// Page 18 (gh:#906 design, addenda and amendments, 2026-10-16): the
// unresolved range, against unresolved-data.js (examples/lesson_unresolved.rs).
{
  const UM = window.Lesson.unresolvedMath, UD = window.UNRESOLVED_DATA, U = UD["U-238"];
  const h = fs.readFileSync(path.join(__dirname, "unresolved.html"), "utf8");
  const seq = (j) => U.seqs.find((s) => s.j === j);
  const sn = window.REACTION_DATA["U-238"].channels.find((c) => c.mt === 102).qm, A = window.SLOWING_DATA.awr["U-238"];
  const half = (t) => { const d = t.split(".")[1]; return 0.5 * Math.pow(10, -(d ? d.length : 0)) * (1 + 1e-9); };
  let bad = 0, n = 0;
  const q = (re, f) => { for (const m of h.matchAll(re)) { n++; const t = m[m.length - 1], want = f(m); if (Math.abs(+t - want) > half(t)) { bad++; console.log("     mismatch", m[0], "want", want); } } };
  q(/data-ur="(el|eh)">([^<]+)</g, (m) => U[m[1]] / 1e3);
  q(/data-d="([0-9.]+),([0-9.]+)">([0-9.]+) eV</g, (m) => UM.spacing(seq(+m[1]), +m[2]));
  q(/data-dshrink>([^<]+)</g, () => 100 * (1 - UM.spacing(seq(0.5), U.eh) / UM.spacing(seq(0.5), U.el)));
  q(/data-estar="([0-9.]+)">([^<]+)</g, (m) => (sn + +m[1] * A / (A + 1)) / 1e6);
  q(/data-meet="lo">([^<]+)</g, () => UM.avgAt(U, U.el / 1.06).s);
  q(/data-meet="hi">([^<]+)</g, () => UM.avgAt(U, U.el * 1.08).s);
  check("page 18: " + n + " numbers in the prose vs the data", bad, 0);
  // Smearing is continuous across U-238's boundary: the resolved forest's
  // average just below 20 keV against the evaluator's dilute File 3 just above.
  const lo = UM.avgAt(U, U.el / 1.06).s, hi = UM.avgAt(U, U.el * 1.08).s;
  check("U-238: smeared forest vs File 3 average across 20 keV (relative)", Math.abs(hi / lo - 1), 0.02);
  check("U-238 is LSSF = 1, U-234 LSSF = 0", Math.abs(U.lssf - 1) + Math.abs(UD["U-234"].lssf), 0);
}

// Page 19 (gh:#906 design and amendments, 2026-10-16): PURR, against
// purr-data.js (examples/lesson_purr.rs, OUTRAM PARK's PURR port).
{
  const PM = window.Lesson.purrMath, PD = window.PURR_DATA;
  const run = PM.running(PD.means.t), runc = PM.running(PD.means.c), n = PD.means.t.length;
  // The ladders taken one at a time are probability_table's ladders: their
  // running average is the table run's own mean. Bit for bit in Rust
  // (purr::tests::sample_ladder_is_the_first_ladder_of_probability_table);
  // here the limits are the rounding of purr-data.js, which prints each mean
  // to 5 decimals (0.5e-5 b on 12.9 b and on 0.32 b) and each probability to
  // 6 significant figures.
  check("PURR: mean of the per-ladder means vs probability_table's mean total (relative)", Math.abs(run[n - 1] / PD.conv.total - 1), 0.5e-5 / 12.9);
  check("PURR: mean capture likewise (relative)", Math.abs(runc[n - 1] / PD.conv.cap - 1), 0.5e-5 / 0.31);
  // The made-up ladders converge on the dilute values the parameters give.
  check("PURR: 64-ladder average total vs infd (relative)", Math.abs(run[n - 1] / PD.infd.total - 1), 0.005);
  check("PURR: 64-ladder average capture vs infd (relative)", Math.abs(runc[n - 1] / PD.infd.cap - 1), 0.02);
  const psum = PD.table.p.reduce((a, b) => a + b, 0);
  check("PURR: table probabilities sum to 1", Math.abs(psum - 1), 1e-5);
  const H = PM.histogram(PD.samples.t, PD.samples.c, Math.min(...PD.samples.t), Math.max(...PD.samples.t) * 1.0001, 24);
  check("PURR: the histogram holds every sample", Math.abs(H.n.reduce((a, b) => a + b, 0) - PD.nsamp), 0);
  const h = fs.readFileSync(path.join(__dirname, "purr.html"), "utf8");
  const half = (t) => { const d = t.split(".")[1]; return 0.5 * Math.pow(10, -(d ? d.length : 0)) * (1 + 1e-9); };
  let bad = 0, nq = 0;
  const q = (re, f) => { for (const m of h.matchAll(re)) { nq++; const t = m[m.length - 1], want = f(m); if (Math.abs(+t - want) > half(t)) { bad++; console.log("     mismatch", m[0], "want", want); } } };
  q(/data-nladr>([^<]+)</g, () => n);
  q(/data-aver="total">([^<]+)</g, () => run[n - 1]);
  q(/data-aver="cap">([^<]+)</g, () => runc[n - 1]);
  q(/data-infd="(total|cap)">([^<]+)</g, (m) => PD.infd[m[1]]);
  check("page 19: " + nq + " numbers in the prose vs the data", bad, 0);
  console.log(`     PURR U-238 ${PD.e} eV: aver ${run[n - 1].toFixed(4)} b vs infd ${PD.infd.total.toFixed(4)} b; one ladder scatters ${PD.conv.pct.toFixed(2)} %`);
}

// Page 20 (gh:#906 design, 2026-10-16): bound atoms, against bound-data.js
// (examples/lesson_bound.rs: LEAPR decks and the tsl tapes' MF=7 B(4)).
{
  const BM = window.Lesson.boundMath, BD = window.BOUND_DATA;
  const h = fs.readFileSync(path.join(__dirname, "bound-atoms.html"), "utf8");
  const half = (t) => { const d = t.split(".")[1]; return 0.5 * Math.pow(10, -(d ? d.length : 0)) * (1 + 1e-9); };
  let bad = 0, n = 0;
  const q = (re, f) => { for (const m of h.matchAll(re)) { n++; const t = m[m.length - 1], want = f(m); if (Math.abs(+t - want) > half(t)) { bad++; console.log("     mismatch", m[0], "want", want); } } };
  q(/data-vib="([^"]+)">([^<]+)</g, (m) => BM.spectrum(BD.decks[m[1]]).top);
  q(/data-osc="(\d)">([^<]+)</g, (m) => BD.decks["H in H2O"].osc[+m[1]][0]);
  q(/data-emax="([^"]+)">([^<]+)</g, (m) => BD.tapes[m[1]].emax);
  check("page 20: " + n + " numbers in the prose vs the data", bad, 0);
  // The prose's "up to about 270 keV" and "about 13 eV" for Si.
  const A = BD.decks["Si in alpha-quartz"].awr;
  check("Si: most a 2 MeV neutron gives, about 270 keV (relative)", Math.abs(BM.maxTransfer(2e6, A) / 2.7e5 - 1), 0.02);
  check("Si: most a 100 eV neutron gives, about 13 eV (relative)", Math.abs(BM.maxTransfer(100, A) / 13 - 1), 0.04);
}

// Page 21 (gh:#906 design, 2026-10-16): graphite's Bragg edges, the page's
// sigma_coh = S/E against the THERMR port's own cross_section, and the prose.
{
  const GM = window.Lesson.braggMath, BD = window.BOUND_DATA;
  let wr = 0;
  for (const t of ["296", "1200"]) { const B = BD.bragg[t]; B.refE.forEach((e, i) => { wr = Math.max(wr, Math.abs(GM.sigmaCoh(B, e) - B.ref[i]) / Math.max(B.ref[i], 1e-9)); }); }
  check("page 21: sigma_coh = S/E vs the THERMR port (relative)", wr, 1e-6);
  const B = BD.bragg["296"], ec = GM.cutoff(B), lc = GM.lambdaA(ec);
  check("graphite: energy <-> wavelength round trip (relative)", Math.abs(GM.energyOf(lc) / ec - 1), 1e-12);
  const h = fs.readFileSync(path.join(__dirname, "lattice-scattering.html"), "utf8");
  const half = (t) => { const d = t.split(".")[1]; return 0.5 * Math.pow(10, -(d ? d.length : 0)) * (1 + 1e-9); };
  let bad = 0, n = 0;
  const q = (re, f) => { for (const m of h.matchAll(re)) { n++; const t = m[m.length - 1], want = f(m); if (Math.abs(+t - want) > half(t)) { bad++; console.log("     mismatch", m[0], "want", want); } } };
  q(/data-nedges>([^<]+)</g, () => B.e.length);
  q(/data-ecut>([^<]+)</g, () => ec * 1e3);
  q(/data-lcut>([^<]+)</g, () => lc);
  q(/data-dmax>([^<]+)</g, () => lc / 2);
  q(/data-s01="(\d+)">([^<]+)</g, (m) => GM.sigmaCoh(BD.bragg[m[1]], 0.1));
  check("page 21: " + n + " numbers in the prose vs the data", bad, 0);
}

// Page 22 (gh:#906 design and amendments, 2026-10-16): Doppler broadening,
// against doppler-data.js (examples/lesson_doppler.rs, the RECONR and BROADR
// ports on U-238's 6.674 eV capture resonance).
{
  const DD = window.DOPPLER_DATA, U = DD.u238;
  const h = fs.readFileSync(path.join(__dirname, "doppler.html"), "utf8");
  const half = (t) => { const d = t.split(".")[1]; return 0.5 * Math.pow(10, -(d ? d.length : 0)) * (1 + 1e-9); };
  let bad = 0, n = 0;
  const q = (re, f) => { for (const m of h.matchAll(re)) { n++; const t = m[m.length - 1].replace(/ /g, ""), want = f(m); if (Math.abs(+t - want) > half(t)) { bad++; console.log("     mismatch", m[0], "want", want); } } };
  q(/data-peak="([0-9.]+)">([^<]+)</g, (m) => U[m[1] === "0" ? "0" : m[1]].peak);
  q(/data-fwhm="([0-9.]+)">([^<]+)</g, (m) => U[m[1]].fwhm * 1e3);
  q(/data-area="([0-9.]+)">([^<]+)</g, (m) => U[m[1]].area);
  check("page 22: " + n + " numbers in the prose vs the data", bad, 0);
  // The physics the page states: lower, wider, area kept (E >> kT/A), 1/v unchanged.
  check("Doppler: the peak falls and widens with temperature", U["293.6"].peak < U["0"].peak && U["1200"].peak < U["293.6"].peak && U["1200"].fwhm > U["293.6"].fwhm && U["293.6"].fwhm > U["0"].fwhm ? 0 : 1, 0);
  check("Doppler: area under the 6.67 eV resonance, 1200 K vs 0 K (relative)", Math.abs(U["1200"].area / U["0"].area - 1), 2e-4);
  check("Doppler: a 1/v cross section is unchanged (largest relative change)", Math.max(DD.onev["293.6"], DD.onev["1200"]), 1e-4);
  const kta = 8.617333262e-5 * 293.6 / DD.awr;
  check("page 22: kT/A for U-238 at 293.6 K is 1.1e-4 eV (relative)", Math.abs(kta / 1.1e-4 - 1), 0.05);
  check("page 22: E_m = 16kT/A for U-235 at 300 K is 0.0017 eV (relative)", Math.abs(16 * 8.617333262e-5 * 300 / 233.0248 / 0.0017 - 1), 0.05);
}


// Pages 42-45 (gh:#906, maintainer's arc of 2026-10-18): free-gas target
// sampling, DBRC, and the hand-over between the two ACE files. Maths in
// dbrc.js (a port of outram-mc-libs scatter.rs sample_cxs_target and
// sample_target_velocity_dbrc), data in dbrc-data.js (examples/lesson_dbrc.rs).
{
  const DM = window.Lesson.dbrcMath, DB = window.DBRC_DATA, U8 = DB.u238, A = U8.awr, kT = DM.KB * 293.6;
  const tab = DM.table(U8.el0k.e, U8.el0k.s);
  // Two-sample chi-square per degree of freedom between two sample sets, bins of at least 10 pooled counts.
  const chi2 = (xa, xb, lo, hi, nb) => {
    const a = new Array(nb).fill(0), b = new Array(nb).fill(0);
    for (const x of xa) { const k = Math.floor((x - lo) / (hi - lo) * nb); if (k >= 0 && k < nb) a[k]++; }
    for (const x of xb) { const k = Math.floor((x - lo) / (hi - lo) * nb); if (k >= 0 && k < nb) b[k]++; }
    let c = 0, n = 0;
    for (let k = 0; k < nb; k++) if (a[k] + b[k] >= 10) { c += (a[k] - b[k]) ** 2 / (a[k] + b[k]); n++; }
    return c / (n - 1);
  };
  const N = 200000, E = 6.5, w = DM.window(E, A, kT);
  // 1. With a flat cross section the rejection accepts every target, so DBRC
  //    must reproduce plain sampling statistically (independent random
  //    streams). 2026-10-18: chi2/dof 0.8-1.2 over 40 bins (see the log line).
  const flat = DM.table([0, 1e3], [3, 3]), rA = DM.rng(1), rB = DM.rng(2);
  const pa = [], pb = [];
  for (let i = 0; i < N; i++) { pa.push(DM.encounter(E, A, kT, rA, null).erel); pb.push(DM.encounter(E, A, kT, rB, flat).erel); }
  const cflat = chi2(pa, pb, E - 0.3, E + 0.3, 40);
  check("DBRC with a flat cross section vs plain sampling: chi2 per dof", cflat, 1.6);
  // 2. The plain sampler against an independent integral: the mean encounter
  //    energy of the density y^2 e^{-y^2}|v - V|, by quadrature over y and mu.
  let s1 = 0, s2 = 0; for (const x of pa) { s1 += x; s2 += x * x; }
  const mP = s1 / N, seP = Math.sqrt((s2 / N - mP * mP) / N), qP = DM.quadMeanErel(E, A, kT, null, 1400, 2000);
  check("plain sampler: mean E_rel vs quadrature, in standard errors", Math.abs(mP - qP) / seP, 4);
  // 3. The DBRC sampler against quadrature of the density with sigma_0K(E_rel) in it, on the real data.
  const rD = DM.rng(3), pd = []; for (let i = 0; i < N; i++) pd.push(DM.encounter(E, A, kT, rD, tab).erel);
  let d1 = 0, d2 = 0; for (const x of pd) { d1 += x; d2 += x * x; }
  const mD = d1 / N, seD = Math.sqrt((d2 / N - mD * mD) / N), qD = DM.quadMeanErel(E, A, kT, (e) => tab.at(e), 1400, 6000);
  check("DBRC sampler on U-238's 0 K elastic: mean E_rel vs quadrature, in standard errors", Math.abs(mD - qD) / seD, 4);
  // 4. The correction does what the page says: it moves the encounters toward the peak (6.675 eV).
  check("DBRC moves the mean encounter energy toward the 6.67 eV peak", mD > mP + 5 * seP ? 0 : 1, 0);
  // 5. The target-speed density of the targets hit: y^2 e^{-y^2} <|v - V|>, against the sampler.
  { const yn = w.yn, nb = 50, ymax = 5, r = DM.rng(5), al = 1 / (1 + Math.sqrt(Math.PI) * yn / 2), cnt = new Array(nb).fill(0), M = 200000;
    for (let i = 0; i < M; i++) { const c = DM.sampleCxs(yn, al, r), k = Math.floor(Math.sqrt(c[0]) / ymax * nb); if (k < nb) cnt[k]++; }
    const pr = new Array(nb).fill(0); let z = 0;
    for (let k = 0; k < nb; k++) { for (let j = 0; j < 40; j++) { const y = (k + (j + 0.5) / 40) * ymax / nb; pr[k] += y * y * Math.exp(-y * y) * DM.relSpeedAvg(y, yn); } z += pr[k]; }
    let tot = 0; cnt.forEach((c) => tot += c);
    let c2 = 0, n = 0; for (let k = 0; k < nb; k++) { const ex = tot * pr[k] / z; if (ex >= 10) { c2 += (cnt[k] - ex) ** 2 / ex; n++; } }
    check("sampled target speeds vs y^2 e^{-y^2}<|v-V|>: chi2 per dof", c2 / (n - 1), 1.6);
    // relSpeedAvg is the mu-average it claims to be.
    let q = 0; const y = 1.7; for (let j = 0; j < 20000; j++) { const mu = -1 + (j + 0.5) * 2 / 20000; q += Math.sqrt(yn * yn + y * y - 2 * yn * y * mu) / 20000; }
    check("relSpeedAvg vs numerical mean of |v-V| over mu (relative)", Math.abs(q / DM.relSpeedAvg(y, yn) - 1), 1e-6);
  }
  // 6. The widget's noise-free density against the two samplers, bin by bin (z-scores), and its acceptance
  //    against the sampler's own acceptance (candidates per accepted target).
  { const lo = E - 0.3, hi = E + 0.3, nb = 40, d = DM.encounterDensity(E, A, kT, tab, lo, hi, nb, 500, 1000), bw = (hi - lo) / nb;
    const cnt = (xs) => { const c = new Array(nb).fill(0); for (const x of xs) { const k = Math.floor((x - lo) / bw); if (k >= 0 && k < nb) c[k]++; } return c; };
    const cp = cnt(pa), cd = cnt(pd);
    let wz = 0, wzd = 0;
    for (let k = 0; k < nb; k++) {
      const ep = N * d.plain[k] * bw, ed = N * d.dbrc[k] * bw;
      if (ep >= 25) wz = Math.max(wz, Math.abs(cp[k] - ep) / Math.sqrt(ep));
      if (ed >= 25) wzd = Math.max(wzd, Math.abs(cd[k] - ed) / Math.sqrt(ed));
    }
    check("encounter density vs plain sampler, worst bin z-score", wz, 4.5);
    check("encounter density vs DBRC sampler, worst bin z-score", wzd, 4.5);
    const r = DM.rng(11); let tries = 0; const M = 4000; for (let i = 0; i < M; i++) tries += DM.encounter(E, A, kT, r, tab).tries;
    const accS = M / tries, se = accS * Math.sqrt((1 - accS) / M);
    check("DBRC acceptance: quadrature vs the sampler's candidates per accepted target, in standard errors", Math.abs(accS - d.accept) / se, 4);
    console.log(`     DBRC acceptance at ${E} eV, 293.6 K: quadrature ${d.accept.toExponential(2)}, sampled ${accS.toExponential(2)}`);
  }
  // 7. DBRC acceptance never exceeds 1: sigma_max really bounds the window.
  { let over = 0; for (let k = 0; k <= 4000; k++) { const e = w.lo + (w.up - w.lo) * k / 4000; if (tab.at(e) > tab.maxOver(w.lo, w.up) * (1 + 1e-12)) over++; }
    check("sigma_max bounds sigma_0K over the window (points above it)", over, 0); }
  // 8. The data file. Structure, and the same resonance as page 22's capture data.
  check("0 K elastic grid is strictly ascending", U8.el0k.e.some((e, i) => i && e <= U8.el0k.e[i - 1]) ? 1 : 0, 0);
  let pk = 0, pe = 0; U8.el0k.e.forEach((e, i) => { if (U8.el0k.s[i] > pk) { pk = U8.el0k.s[i]; pe = e; } });
  check("0 K elastic peak sits at page 22's capture peak energy (eV)", Math.abs(pe - window.DOPPLER_DATA.u238["0"].ep), 5e-3);
  // Single-level formula from the tape's own parameters (resonance-data.js, ENDF MF=2) vs the RECONR 0 K elastic
  // at the peak and on its flanks. Not a gate on the whole curve: other levels add a smooth background.
  { const RS = window.Lesson.resonanceMath, R = window.RES_DATA.u238, lvl = [[R.er, R.aj, R.gn + R.gg, R.gn, R.gg, 0]];
    let worstRel = 0;
    for (const dE of [-0.02, -0.01, 0, 0.01, 0.02]) {
      const e = R.er + dE, sl = RS.slbw(e, lvl, 0, R.spi, R.ap, R.awri, R.ra).el;
      worstRel = Math.max(worstRel, Math.abs(tab.at(e) / sl - 1));
    }
    check("0 K elastic data vs the lone-level formula from the tape's parameters, peak +- 0.02 eV (relative)", worstRel, 0.05);
    // 2026-10-18: 0.1 % at the peak, 3.3 % at -0.02 eV. Further out the other levels (and the interference
    // dip, 0.05 b at 6.518 eV) make the lone-level formula wrong by a factor of two, which is why the gate is the peak region only.
    console.log(`     lone-level vs RECONR 0 K elastic, worst relative gap over peak +- 0.02 eV ${(100 * worstRel).toFixed(2)} %`);
  }
  // 9. The thermal and continuous-energy tables: the hand-over the page states.
  const H = DB.th["H in H2O"], G = DB.th["graphite"];
  check("H in H2O thermal table: cutoff is its last inelastic energy (eV)", Math.abs(H.cutoff - H.einel[H.einel.length - 1]), 0);
  check("thermal tables: the bound cross section is zero at and above the cutoff", DM.boundXs(H, H.cutoff) + DM.boundXs(G, G.cutoff + 1) === 0 ? 0 : 1, 0);
  check("graphite: coherent elastic from the Bragg edges is the S/E form (b, at 0.01 eV vs table)", Math.abs(DM.sigmaCoh(G.edges, G.cum, 0.01) - G.cum[G.edges.filter((e) => e <= 0.01).length - 1] / 0.01), 1e-12);
  const gc = DM.boundXs(G, 0.0253), gi = G.sinel.length ? G.sinel : [];
  console.log(`     graphite at 0.0253 eV: bound ${gc.toFixed(3)} b per atom (coherent ${DM.sigmaCoh(G.edges, G.cum, 0.0253).toFixed(3)} b); H in H2O ${DM.boundXs(H, 0.0253).toFixed(2)} b per H`);
  console.log(`     DBRC on U-238 at ${E} eV, 293.6 K: mean E_rel ${mP.toFixed(4)} -> ${mD.toFixed(4)} eV (quadrature ${qP.toFixed(4)} -> ${qD.toFixed(4)}); flat-sigma chi2/dof ${cflat.toFixed(2)}`);
}

// Pages 42-45, the prose numbers: every data-* number on the four new pages
// against the data or against a recomputation, to the digits printed.
{
  const DM = window.Lesson.dbrcMath, DB = window.DBRC_DATA, U8 = DB.u238, A = U8.awr, kT = DM.KB * 293.6;
  const tab = DM.table(U8.el0k.e, U8.el0k.s);
  const rd = (f) => fs.readFileSync(path.join(__dirname, f), "utf8");
  const half = (t) => { const d = t.split(".")[1]; return 0.5 * Math.pow(10, -(d ? d.length : 0)) * (1 + 1e-9); };
  let bad = 0, n = 0;
  const q = (h, re, f) => { for (const m of h.matchAll(re)) { n++; const t = m[m.length - 1].replace(/ /g, ""), want = f(m); if (!(Math.abs(+t - want) <= half(t))) { bad++; console.log("     mismatch", m[0], "want", want); } } };
  // dbrc.html: the U-238 0 K elastic peak and dip, and the exact mean encounter energies.
  { const h = rd("dbrc.html");
    let pk = 0, pe = 0, dp = 1e9, de = 0;
    U8.el0k.e.forEach((e, i) => { const s = U8.el0k.s[i]; if (s > pk) { pk = s; pe = e; } if (e > 6.3 && e < 6.67 && s < dp) { dp = s; de = e; } });
    q(h, /data-pk0>([^<]+)</g, () => pk); q(h, /data-pke0>([^<]+)</g, () => pe); q(h, /data-dip>([^<]+)</g, () => dp); q(h, /data-dipe>([^<]+)</g, () => de);
    q(h, /data-acc>([^<]+)</g, () => 1e4 * DM.encounterDensity(6.5, A, kT, tab, 6.2, 6.8, 40, 500, 1000).accept);
    q(h, /data-mp>([^<]+)</g, () => DM.quadMeanErel(6.5, A, kT, null, 1400, 2000));
    q(h, /data-md>([^<]+)</g, () => DM.quadMeanErel(6.5, A, kT, (e) => tab.at(e), 1400, 6000));
  }
  // which-nucleus.html: the share of hit targets moving toward the neutron, 40 000 draws, seed 2026 (the widget's).
  { const h = rd("which-nucleus.html"), M = { h1: 0.9991673, c12: 11.89365, u238: 236.0058 };
    for (const m of h.matchAll(/data-toward="([a-z0-9]+),([0-9.]+)">([^<]+)</g)) {
      n++; const w = DM.window(+m[2], M[m[1]], kT), al = 1 / (1 + Math.sqrt(Math.PI) * w.yn / 2), r = DM.rng(2026); let t = 0;
      for (let i = 0; i < 40000; i++) if (DM.sampleCxs(w.yn, al, r)[1] < 0) t++;
      const got = 100 * t / 40000;
      if (Math.abs(got - +m[3]) > 0.05) { bad++; console.log("     toward mismatch", m[0], "got", got.toFixed(2)); }
    }
  }
  // ace-files.html: counts in the two tables against dbrc-data.js.
  { const h = rd("ace-files.html"), ce = DB.ce, th = DB.th;
    q(h, /data-nes="([^"]+)">([^<]+)</g, (m) => ce[m[1]].nes); q(h, /data-ntr="([^"]+)">([^<]+)</g, (m) => ce[m[1]].ntr);
    for (const m of h.matchAll(/data-xss="([^"]+)">([^<]+)</g)) { n++; const want = (ce[m[1]] || th[m[1]]).xss; if (+m[2] !== want) { bad++; console.log("     xss mismatch", m[0], want); } }
    q(h, /data-nin="([^"]+)">([^<]+)</g, (m) => th[m[1]].einel.length); q(h, /data-cut="([^"]+)">([^<]+)</g, (m) => th[m[1]].cutoff);
    q(h, /data-edges="([^"]+)">([^<]+)</g, (m) => th[m[1]].edges.length);
    check("page 44: H-1 has a reaction block besides elastic and total and an AND block", ce["H-1"].ntr >= 1 && ce["H-1"].and > 0 ? 0 : 1, 0);
    check("page 44: the thermal tables are IFENG 0, 16 outgoing energies, 16 cosines", [th["H in H2O"], th["graphite"]].every((t) => t.ifeng === 0 && t.nieb === 16 && t.nil === 15) ? 0 : 1, 0);
    check("page 44: water has no thermal elastic (IDPNC 0), graphite coherent (IDPNC 4)", th["H in H2O"].idpnc === 0 && th["graphite"].idpnc === 4 ? 0 : 1, 0);
  }
  // two-files.html: bound and free cross sections at the stated energies.
  { const h = rd("two-files.html"), th = DB.th, ce = DB.ce;
    const interp = (e, s, x) => { let lo = 0, hi = e.length - 1; if (x <= e[0]) return s[0]; if (x >= e[hi]) return s[hi]; while (hi - lo > 1) { const m = (lo + hi) >> 1; if (e[m] <= x) lo = m; else hi = m; } return s[lo] + (s[hi] - s[lo]) * (x - e[lo]) / (e[hi] - e[lo]); };
    const cutOf = { "H in H2O": th["H in H2O"].cutoff, "graphite": th["graphite"].cutoff, "H-1": th["H in H2O"].cutoff, "C-12": th["graphite"].cutoff };
    q(h, /data-xs="([^,]+),([^,]+),([^"]+)">([^<]+)</g, (m) => {
      const E = m[2] === "cut-" ? cutOf[m[1]] - 1e-6 : m[2] === "cut+" ? cutOf[m[1]] + 1e-6 : +m[2];
      return m[3] === "bound" ? DM.boundXs(th[m[1]], E) : interp(ce[m[1]].elastic.e, ce[m[1]].elastic.s, E);
    });
    q(h, /data-coh>([^<]+)</g, () => DM.sigmaCoh(th.graphite.edges, th.graphite.cum, 0.0253));
    // The hand-over, stated as a check: the cross section moves by well under 1 % across the cutoff.
    for (const [mat, nuc] of [["H in H2O", "H-1"], ["graphite", "C-12"]]) {
      const T = th[mat], b = DM.boundXs(T, T.cutoff - 1e-6), f = interp(ce[nuc].elastic.e, ce[nuc].elastic.s, T.cutoff + 1e-6);
      check("page 45: " + mat + " scattering cross section across the cutoff, relative step", Math.abs(b / f - 1), 0.01);
    }
  }
  check("pages 42-45: " + n + " numbers in the prose vs the data", bad, 0);
}

// Page 23 (gh:#906 designs, 2026-10-16): the toy phonon rules.
{
  const LM = window.Lesson.latticeMath;
  check("phonons: the toy's highest phonon is scaled to 152 meV", Math.abs(LM.mev(LM.OMAX) - 152), 1e-9);
  // A kick inside the first zone makes a phonon with q = Q: longitudinal
  // strength 1, transverse 0 (Q.e = 0).
  const a = LM.match(0.6 * Math.PI, 0, 0);
  check("phonons: a first-zone kick pushes only the longitudinal style", Math.abs(a.L.strength - 1) + a.T.strength, 1e-12);
  // A kick folded back by a reciprocal-lattice vector can push a transverse phonon.
  const b = LM.match(1.5 * Math.PI, 0.6 * Math.PI, 0);
  check("phonons: a folded kick gives the transverse style a share", b.fold.gx !== 0 && b.T.strength > 0.05 ? 0 : 1, 0);
  check("phonons: (Q.e)^2 over the two styles adds to 1", Math.abs(b.L.strength + b.T.strength - 1), 1e-12);
  // The energy rule: a match exactly at the branch energy, none far off it.
  const c = LM.match(0.6 * Math.PI, 0, 0), hit = LM.match(0.6 * Math.PI, 0, c.L.mev), miss = LM.match(0.6 * Math.PI, 0, c.L.mev + 40);
  check("phonons: energy on the longitudinal branch matches, 40 meV off does not", hit.L.hit && !miss.L.hit ? 0 : 1, 0);
  // Periodicity: Q and Q + G make the same phonon energy.
  const d = LM.match(0.3 * Math.PI, 0.2 * Math.PI, 0), e = LM.match(2.3 * Math.PI, 0.2 * Math.PI, 0);
  check("phonons: the dispersion repeats with the reciprocal lattice (meV)", Math.abs(d.L.mev - e.L.mev) + Math.abs(d.T.mev - e.T.mev), 1e-9);
}

// Page 24 (gh:#906 design, 2026-10-16): random kicks build the toy's density
// of states; the real spectra's low-energy slopes and lines.
{
  const MM = window.Lesson.manyMath, BD = window.BOUND_DATA, NB = MM.NB, rho = MM.rhoToy(NB), r = MM.rng(906), N = 300000, h = new Array(NB).fill(0);
  for (let i = 0; i < N; i++) h[Math.min(NB - 1, Math.floor(MM.kick(r).mev / 152 * NB))]++;
  let l1 = 0; for (let b = 0; b < NB; b++) l1 += Math.abs(h[b] / N - rho[b]);
  // Sampling noise alone gives about sum sqrt(p/N) ~ 0.01 here.
  check("page 24: 3e5 random kicks vs the toy density of states (L1)", l1, 0.02);
  const slope = (d) => { const p = []; for (let i = 1; i <= 6; i++) if (d.rho[i] > 0) p.push([Math.log(i * d.delta), Math.log(d.rho[i])]); const n = p.length, sx = p.reduce((a, q) => a + q[0], 0), sy = p.reduce((a, q) => a + q[1], 0), sxx = p.reduce((a, q) => a + q[0] * q[0], 0), sxy = p.reduce((a, q) => a + q[0] * q[1], 0); return (n * sxy - sx * sy) / (n * sxx - sx * sx); };
  const Q = BD.decks["Si in alpha-quartz"], W = BD.decks["H in H2O"];
  const hh = fs.readFileSync(path.join(__dirname, "many-phonons.html"), "utf8");
  const num = (re) => +hh.match(re)[1];
  check("page 24: quartz low-energy slope in the prose vs the deck", Math.abs(num(/data-qslope>([^<]+)</) - slope(Q)), 0.005);
  check("page 24: water low-energy slope in the prose vs the deck", Math.abs(num(/data-wslope>([^<]+)</) - slope(W)), 0.005);
  check("page 24: water's two lines in the prose vs the deck", Math.abs(num(/data-osc0>([^<]+)</) - W.osc[0][0]) + Math.abs(num(/data-osc1>([^<]+)</) - W.osc[1][0]), 1e-12);
  console.log(`     kicks vs rho L1 ${l1.toFixed(4)}; quartz slope ${slope(Q).toFixed(3)}, water ${slope(W).toFixed(3)}`);
}

// Page 25 (gh:#906 design, 2026-10-16): structure shapes rho (toy maths).
{
  const SM = window.Lesson.structureMath, LM = window.Lesson.latticeMath, BD = window.BOUND_DATA;
  // With equal masses the checkerboard reproduces page 23's dispersion.
  let miss = 0;
  for (let i = 0; i < 64; i++) {
    const qx = -Math.PI + (i % 8 + 0.3) * Math.PI / 4, qy = -Math.PI + (Math.floor(i / 8) + 0.6) * Math.PI / 4;
    const m = SM.modes(qx, qy, SM.ISO, 1, 1), sx = Math.sin(qx / 2), sy = Math.sin(qy / 2);
    for (const want of [4 * (LM.KL * sx * sx + LM.KT * sy * sy), 4 * (LM.KT * sx * sx + LM.KL * sy * sy)]) if (!m.some((u) => Math.abs(u - want) < 1e-9)) miss++;
  }
  check("page 25: equal masses reproduce page 23's frequencies (misses)", miss, 0);
  const wmax = Math.sqrt(Math.max(...SM.modes(Math.PI, Math.PI, SM.ISO, 1, 1), ...SM.modes(0, 0, SM.ISO, 1, 3))) * 1.02;
  const one = SM.dos(SM.ISO, 1, 1, wmax, 40), two = SM.dos(SM.ISO, 1, 3, wmax, 40);
  check("page 25: each density of states sums to 1", Math.abs(one.reduce((a, b) => a + b) - 1) + Math.abs(two.reduce((a, b) => a + b) - 1), 1e-12);
  const gap = two.slice(17, 27).every((v) => v === 0) && one.slice(17, 27).every((v) => v > 0);
  check("page 25: two masses open a gap that one mass does not have", gap ? 0 : 1, 0);
  const q = 0.6 * Math.PI;
  check("page 25: the even lattice's frequency does not depend on direction here", Math.abs(SM.alongDir(0, q, SM.ISO) / SM.alongDir(Math.PI / 2, q, SM.ISO) - 1), 1e-12);
  check("page 25: the layered toy is faster along its layers", SM.alongDir(0, q, SM.LAYER) / SM.alongDir(Math.PI / 2, q, SM.LAYER) > 2 ? 0 : 1, 0);
  const h = fs.readFileSync(path.join(__dirname, "structure.html"), "utf8"), W = BD.decks["H in H2O"];
  check("page 25: water's lines in the prose vs the deck", Math.abs(+h.match(/data-osc0>([^<]+)</)[1] - W.osc[0][0]) + Math.abs(+h.match(/data-osc1>([^<]+)</)[1] - W.osc[1][0]), 1e-12);
}

// The maths track, rungs 1-5h (gh:#906 designs, 2026-10-16), against
// leapr-data.js (examples/lesson_leapr.rs, OUTRAM PARK's LEAPR port).
{
  const MT = window.Lesson.mathTrack, LD = window.LEAPR_DATA, BD = window.BOUND_DATA;
  // Free gas (rung 2): normalised, mean beta = -alpha (the sum rule), variance 2 alpha.
  let wfg = 0;
  for (const a of [0.1, 1, 5]) {
    let s0 = 0, s1 = 0, s2 = 0; const db = 0.001;
    for (let b = -a - 40 * Math.sqrt(a) - 5; b < -a + 40 * Math.sqrt(a) + 5; b += db) { const v = MT.freeGas(a, b) * db; s0 += v; s1 += b * v; s2 += b * b * v; }
    wfg = Math.max(wfg, Math.abs(s0 - 1), Math.abs(s1 + a), Math.abs(s2 - s1 * s1 - 2 * a) / (2 * a));
  }
  check("rung 2: free gas normalised, mean -alpha, variance 2 alpha", wfg, 1e-6);
  // Rung 3: the page's eq. 542 against the LEAPR port's bfact.
  let wo = 0;
  LD.osc.forEach((o) => {
    const W = MT.oscWeights(o.a, o.bi, 10), rel = (x, y) => Math.abs(x - y) / Math.max(Math.abs(y), 1e-300);
    wo = Math.max(wo, rel(W.w[10], o.b0));
    for (let k = 1; k <= 10; k++) { if (o.up[k - 1] > 1e-200) wo = Math.max(wo, rel(W.w[10 + k], o.up[k - 1])); if (o.down[k - 1] > 1e-200) wo = Math.max(wo, rel(W.w[10 - k], o.down[k - 1])); }
  });
  check("rung 3: eq. 542 vs the LEAPR port's bfact (relative)", wo, 1e-5);
  let wsr = 0;
  for (const [a, bi] of [[0.2, 8], [1.3, 2], [6, 0.7]]) { const W = MT.oscWeights(a, bi, 60); let s0 = 0, s1 = 0; W.w.forEach((v, k) => { s0 += v; s1 += (k - 60) * bi * v; }); wsr = Math.max(wsr, Math.abs(s0 - 1), Math.abs(s1 + a)); }
  check("rung 3: one swing normalised, mean beta = -alpha (sum rule)", wsr, 1e-9);
  // Rung 3 1/2: two fair dice.
  const two = MT.convolve([1, 1, 1, 1, 1, 1].map((x) => x / 6), [1, 1, 1, 1, 1, 1].map((x) => x / 6));
  check("rung 3 1/2: P(7) = 6/36 for two dice", Math.abs(two[5] - 6 / 36), 1e-15);
  // Rung 4: the port's S(alpha, -beta) plus the zero-phonon spike obeys the sum rule.
  const Q = LD.quartz;
  let w4 = 0;
  Q.cols.forEach((c) => {
    if (c.a > 5) return; // beyond the 30 kT grid the loss side is truncated
    let s0 = Math.exp(-c.a * Q.lambda), s1 = 0;
    c.s.forEach((v, k) => { const b = Q.beta[k], wq = (k === 0 || k === c.s.length - 1 ? 0.5 : 1) * 0.05; s0 += v * (1 + Math.exp(-b)) * wq; s1 += -b * v * (1 - Math.exp(-b)) * wq; });
    w4 = Math.max(w4, Math.abs(s0 - 1), Math.abs(s1 + c.a) / c.a);
  });
  check("rung 4: the LEAPR port's S (with the elastic spike) normalised and obeying the sum rule", w4, 0.02);
  // Rung 4: sampling n phonons from T1 and averaging the energy change gives -alpha.
  { const r = MT.rng(44), t1 = Q.tn[0], db = Q.deltab, cum = []; let tot = 0;
    for (let i = 0; i < t1.length; i++) { const b = i * db; tot += t1[i] * (1 + Math.exp(-b)) * db; cum.push(tot); }
    const draw = () => { const u = r() * tot; let k = 0; while (k < cum.length - 1 && cum[k] < u) k++; const b = Math.max((k + r() - 0.5) * db, 0); return r() < 1 / (1 + Math.exp(-b)) ? -b : b; };
    const a = 1, N = 200000; let s1 = 0, s2 = 0;
    for (let i = 0; i < N; i++) { const n = MT.poisson(r, a * Q.lambda); let v = 0; for (let d = 0; d < n; d++) v += draw(); s1 += v; s2 += v * v; }
    const mean = s1 / N, se = Math.sqrt((s2 / N - mean * mean) / N);
    check("rung 4: sampled mean energy change vs -alpha, in standard errors", Math.abs(mean + a) / se, 4);
  }
  // Rungs 5c, 5g: rho-weighted (beta/2)coth(beta/2) reproduces the port's Tbar.
  const tb = MT.tbarFromRho(BD.decks["Si in alpha-quartz"].rho, BD.decks["Si in alpha-quartz"].delta, Q.tev);
  check("rungs 5c/5g: rho-weighted coth vs the LEAPR port's Tbar/T (relative)", Math.abs(tb / Q.tbar - 1), 1e-8);
  // Rung 5: the shortcut agrees with the sum for the biggest kick and not the smallest.
  const diff = (c) => { let d = 0, s = 0; c.s.forEach((v, k) => { d += Math.abs(v - c.sct[k]); s += v; }); return d / s; };
  check("rung 5: the bell shortcut fits kick 20 (within 10 %) and not kick 0.05", diff(Q.cols[4]) < 0.1 && diff(Q.cols[0]) > 0.3 ? 0 : 1, 0);
  // Prose numbers.
  const num = (f, re) => +fs.readFileSync(path.join(__dirname, f), "utf8").match(re)[1];
  check("rung 5c: quartz Tbar/T in the prose", Math.abs(num("stiff-soft.html", /data-qtbar>([^<]+)</) - Q.tbar), 0.0005);
  check("rung 5c: quartz Tbar in kelvin in the prose", Math.abs(num("stiff-soft.html", /data-qtbark>([^<]+)</) - Q.tbar * Q.t), 0.5);
  check("rung 5g: quartz Tbar/T in the prose", Math.abs(num("one-spring-check.html", /data-qtbar2>([^<]+)</) - Q.tbar), 0.0005);
  console.log(`     rung 5 shortcut misfit: kick 20 ${(100 * diff(Q.cols[4])).toFixed(1)} %, kick 0.05 ${(100 * diff(Q.cols[0])).toFixed(0)} %; quartz Tbar/T ${Q.tbar.toFixed(4)}`);
}

// Rung 5i (gh:#906 design, 2026-10-16): the spin page's numbers against
// spin-data.js (the NIST NCNR table), and the 3:1 spin-state weighting.
{
  const SD = window.SPIN_DATA, H = SD["H-1"], h = fs.readFileSync(path.join(__dirname, "spin.html"), "utf8");
  const avg = 0.75 * H.bplus + 0.25 * H.bminus, msq = 0.75 * H.bplus * H.bplus + 0.25 * H.bminus * H.bminus;
  check("rung 5i: 3/4 triplet + 1/4 singlet reproduces H-1's coherent length (fm)", Math.abs(avg - H.bc), 0.01);
  const sc = 4 * Math.PI * avg * avg / 100, si = 4 * Math.PI * (msq - avg * avg) / 100;
  check("rung 5i: coherent and incoherent cross sections from the two lengths vs the table (relative)", Math.max(Math.abs(sc / H.sc - 1), Math.abs(si / H.si - 1)), 0.01);
  const half = (t) => { const d = t.split(".")[1]; return 0.5 * Math.pow(10, -(d ? d.length : 0)) * (1 + 1e-9); };
  let bad = 0, n = 0;
  const q = (re, want) => { const m = h.match(re); n++; const t = m[1].replace("&minus;", "-"); if (Math.abs(+t - want) > half(t)) { bad++; console.log("     mismatch", m[0], "want", want); } };
  q(/data-bplus>([^<]+)</, H.bplus); q(/data-bminus>([^<]+)</, H.bminus); q(/data-bavg>([^<]+)</, avg); q(/data-bc>([^<]+)</, H.bc);
  q(/data-sc>([^<]+)</, H.sc); q(/data-si>([^<]+)</, H.si); q(/data-csi>([^<]+)</, SD["C-12"].si); q(/data-scc>([^<]+)</, sc); q(/data-sic>([^<]+)</, si);
  check("rung 5i: " + n + " numbers in the prose vs spin-data.js", bad, 0);
  console.log(`     H-1 from b+/b-: coherent ${sc.toFixed(3)} b (table ${H.sc}), incoherent ${si.toFixed(2)} b (table ${H.si})`);
}

// The glossary: every entry links to a page that exists and names the term.
{
  const g = fs.readFileSync(path.join(__dirname, "glossary.html"), "utf8");
  const plain = (t) => t.replace(/<[^>]+>/g, " ").replace(/&ndash;/g, "-").replace(/&[a-z]+;/g, " ").replace(/\s+/g, " ").toLowerCase();
  let bad = 0, n = 0;
  for (const m of g.matchAll(/<a href="([a-z0-9-]+\.html)" data-term="([^"]+)"/g)) {
    n++;
    const f = path.join(__dirname, m[1]);
    if (!fs.existsSync(f) || !plain(fs.readFileSync(f, "utf8")).includes(m[2].toLowerCase().replace(/-/g, "-"))) { bad++; console.log("     glossary: " + m[1] + " does not name '" + m[2] + "'"); }
  }
  check("glossary: " + n + " links reach a page that names the term", bad, 0);
}

if (failed) { console.log(failed + " check(s) failed"); process.exit(1); }
console.log("all lesson physics checks passed");
