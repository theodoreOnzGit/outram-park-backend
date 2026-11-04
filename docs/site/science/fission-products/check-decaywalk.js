// The data of the Fission products lessons "An atom's clock: sampling a
// decay" (decay-clock.html) and "Walking on spheres"
// (walk-on-spheres.html), checked under node (no browser):
// `node docs/site/science/fission-products/check-decaywalk.js`. Data:
// decaywalk-data.js (crates/dhoby-ghaut/examples/lesson_decay_walk, which
// records boon-lay). Each check says what it compares against; gates set
// from the binomial sigma (4 sigma) or the record's own criterion before the
// first run. Results of 2026-11-04 are in the comments. Lines marked
// "REPORT" print a measured value the page shows as it is; they are not gates.
"use strict";
const fs = require("fs"), path = require("path");
global.window = {};
eval(fs.readFileSync(path.join(__dirname, "decaywalk-data.js"), "utf8"));
const D = window.DECAYWALK_DATA.decay, W = window.DECAYWALK_DATA.walk;
let failed = 0;
function check(name, value, limit) {
  const ok = value <= limit;
  if (!ok) failed++;
  console.log(`${ok ? "ok  " : "FAIL"} ${name}: ${Number(value).toExponential(2)} (limit ${limit})`);
}
function report(name, text) { console.log(`REPORT ${name}: ${text}`); }

// 1. The decay law the page draws is 2^(-t/T½), recomputed here.
check("1 recorded law vs 2^(-x)", Math.max(...D.xs.map((x, i) => Math.abs(D.law[i] - Math.pow(2, -x)))), 1e-5);

D.parents.forEach((P) => {
  // 2. Branching ratios sum to one (gh:#538's panic not reachable).
  check(`2 ${P.name}: |sum of branching ratios - 1|`, Math.abs(P.branches.reduce((s, b) => s + b.ratio, 0) - 1), 1e-6);
  // 3. Surviving parents (each atom's sampled first lifetime) against the law.
  check(`3 ${P.name}: worst |surviving - 2^(-t/T½)| in sigma (${P.surviveWorst[1]} points)`, P.surviveWorst[0], 4);
  // 4. The simulator stepped with advance_timestep against CRAM48, per species.
  P.worst.forEach((w, k) => { if (w[1] > 0) check(`4 ${P.name} -> ${P.species[k]}: worst |stepped - CRAM48| in sigma (${w[1]} points)`, w[0], 4); });
  check(`4 ${P.name}: stepped parent count vs clock count (atoms)`, P.stepVsClockAtoms, 1);
  // 5. Conservation: every atom is some species; CRAM48's inventory sums to one.
  let wm = 0, wc = 0;
  D.xs.forEach((x, i) => {
    wm = Math.max(wm, Math.abs(P.mc.reduce((s, r) => s + r[i], 0) - 1));
    wc = Math.max(wc, Math.abs(P.cram.reduce((s, r) => s + r[i], 0) - 1));
  });
  check(`5 ${P.name}: counted species sum to one`, wm, 1e-3);
  check(`5 ${P.name}: CRAM48 inventory sums to one`, wc, 1e-8);
  // 6. First branches against the library's ratios, 4 sigma.
  P.branches.filter((b) => b.to !== "none").forEach((b, k) => {
    const s = Math.sqrt(b.ratio * (1 - b.ratio) / D.nBig);
    if (s > 0) check(`6 ${P.name} -> ${b.to}: sampled ${P.firstBranch[k]} vs ${b.ratio.toFixed(4)} in sigma`, Math.abs(P.firstBranch[k] - b.ratio) / s, 4);
  });
  // 7. Drawn atoms: start as the parent, clocks increase, and their surviving
  //    fraction at 1 T½ is within 3 sigma of 0.5 (D6).
  let bad = 0;
  P.atoms.forEach((a) => { if (a[0] !== 0) bad++; for (let i = 1; i + 2 < a.length; i += 2) if (a[i + 2] < a[i]) bad++; });
  check(`7 ${P.name}: drawn atoms not starting as the parent or with clocks going back`, bad, 0);
  check(`7 ${P.name}: drawn surviving at 1 T½, |f - 0.5| in sigma`, Math.abs(P.show1 - 0.5) / Math.sqrt(0.25 / D.nShow), 3);
});

// 8. The CRP-6 particle as the cell is assembled (Hales et al. 2013 radii).
check("8 radii vs CRP-6 [212.5, 312.5, 352.5, 387.5, 427.5] µm", Math.max(...W.radii.map((r, i) => Math.abs(r - [212.5, 312.5, 352.5, 387.5, 427.5][i]))), 1e-9);
// 9. One Gaussian step: fractions sum to one; jumping past the IPyC grows with sigma.
let prev = -1;
W.steps.forEach((S) => {
  check(`9 sigma ${S.sigmaUm} µm: landing fractions sum to one`, Math.abs(S.frac.reduce((a, b) => a + b, 0) - 1), 1e-3);
  const past = S.frac[3] + S.frac[4] + S.frac[5];
  check(`9 sigma ${S.sigmaUm} µm: past-IPyC share does not fall as sigma grows`, past < prev ? 1 : 0, 0);
  prev = past;
  report(`9 sigma ${S.sigmaUm} µm`, `dt ${S.dt.toExponential(3)} s, SiC or beyond ${(100 * past).toFixed(2)} %`);
});
// 10. Walk-on-Spheres: hop time mean 1/6; hop spheres inside the kernel; one D.
check("10 mean dimensionless hop time vs 1/6, in standard errors", Math.abs(W.theta[0] - 1 / 6) / W.theta[1], 3);
check("10 worst (|centre| + hop radius)/R - 1", W.insideWorst, 1e-9);
check("10 cell's kernel D vs kernel_diffusion_coefficient (relative)", Math.abs(W.cellKernelD / W.kernelD - 1), 0);
let back = 0;
W.paths.forEach((p) => { for (let i = 1; i < p.length; i++) if (p[i][4] <= p[i - 1][4]) back++; });
check("10 drawn paths: clocks that do not advance", back, 0);
// 11. Release against Crank: the curve (4 sigma) and the record's criterion.
W.release.forEach((R) => {
  check(`11 ${R.temp} C: worst |WoS - Crank| in sigma (${R.worst[1]} points)`, R.worst[0], 4);
  check(`11 ${R.temp} C: CRP-6 Case 1 |MC - Crank| at 200 h (record criterion)`, Math.abs(R.case1 - R.case1Crank), 0.02);
  report(`11 ${R.temp} C`, `Case 1 MC ${R.case1.toFixed(4)}, Crank ${R.case1Crank.toFixed(4)}, D ${R.d.toExponential(4)}`);
});
// 12. The fixed-step Gaussian walk under-releases, less as the step shrinks.
const R1 = W.release[0], last = R1.hours.length - 1, def = R1.gauss.map((g) => R1.crank[last] - g.frac[last]);
check("12 Gaussian sigma = R/10 deficit at 200 h is at least 0.05", def[0] >= 0.05 ? 0 : 1, 0);
check("12 deficits fall as the step shrinks", def[0] > def[1] && def[1] > def[2] ? 0 : 1, 0);
report("12 deficits at 200 h, sigma = R/10, R/30, R/100", def.map((x) => x.toFixed(4)).join(", "));

// 13. The pages: data file small, every data-dw key filled by the story, both lessons in the FISSION list in place.
const size = fs.statSync(path.join(__dirname, "decaywalk-data.js")).size;
check("13 decaywalk-data.js size (bytes)", size, 300000);
const story = fs.readFileSync(path.join(__dirname, "decaywalk-story.js"), "utf8");
["decay-clock.html", "walk-on-spheres.html"].forEach((f) => {
  const html = fs.readFileSync(path.join(__dirname, f), "utf8");
  const keys = [...html.matchAll(/data-dw="([A-Za-z0-9]+)"/g)].map((m) => m[1]);
  check(`13 ${f}: data-dw keys the story does not fill`, keys.filter((k) => !new RegExp("\\b" + k + ":").test(story)).length, 0);
  check(`13 ${f}: banner, footer tokens, lesson-nav`, html.includes("not yet reviewed by a human") && html.includes("@@COMMIT@@") && html.includes('class="lesson-nav"') ? 0 : 1, 0);
});
const kit = fs.readFileSync(path.join(__dirname, "../neutronics/lesson-kit.js"), "utf8");
const at = (s) => kit.indexOf('["' + s + '"');
check("13 FISSION: decay-clock right after decay-again", at("decay-clock.html") > at("decay-again.html") && at("decay-clock.html") < at("decay-or-move.html") ? 0 : 1, 0);
check("13 FISSION: walk-on-spheres right after atoms-swap", at("walk-on-spheres.html") > at("atoms-swap.html") && at("walk-on-spheres.html") < at("kernel-polycrystal.html") ? 0 : 1, 0);

if (failed) { console.log(`${failed} check(s) failed`); process.exit(1); }
console.log("all decay-clock and walk-on-spheres checks passed");
