// The data of the diffusion arc's lessons "The diffusion equation, three
// ways" (diffusion-three-ways.html) and "Is TRISO-ATOPS's analytical solution
// conservative?" (atops-conservative.html), checked under node (no browser):
// `node docs/site/science/fission-products/check-diffarc.js`. Data:
// diffarc-data.js (crates/dhoby-ghaut/examples/lesson_diffusion_arc, gh:#1209).
// Gates are the generator's predictions or the facts the pages state; each
// says what it compares. Lines marked "REPORT" print a measured value the
// page shows as it is; they are not gates. Results of 2026-11-04: all pass.
"use strict";
const fs = require("fs"), path = require("path");
global.window = {};
eval(fs.readFileSync(path.join(__dirname, "diffarc-data.js"), "utf8"));
const A = window.DIFFARC_DATA.a, B = window.DIFFARC_DATA.b;
let failed = 0;
function check(name, value, limit) {
  const ok = value <= limit;
  if (!ok) failed++;
  console.log(`${ok ? "ok  " : "FAIL"} ${name}: ${Number(value).toExponential(2)} (limit ${limit})`);
}
function report(name, text) { console.log(`REPORT ${name}: ${text}`); }
const cs = (el, t) => B.cases.find((c) => c.el === el && c.t === t);
const last = (v) => v[v.length - 1];

// 1. Part A: CRP-6 Case 1a's Crank value at 200 h, and the mesh converging.
check("1 Crank at 200 h vs the CRP-6 record's 0.5337", Math.abs(last(A.crank) - 0.5337), 1e-4);
check("1 80 cells at 200 h vs Crank (prediction A1: 3e-4)", A.fv[3].end, 3e-4);
for (let k = 0; k + 1 < 3; k++) {
  const r = A.fv[k].end / A.fv[k + 1].end;
  check(`1 error ratio ${A.fv[k].cells} -> ${A.fv[k + 1].cells} cells, distance from the 3-5 band`, r < 3 ? 3 - r : r > 5 ? r - 5 : 0, 0);
}
report("1 worst early gap at 80 cells (prediction 1e-3, MISSED)", A.fv[3].worst.toExponential(2));
A.profiles.forEach((p) => check(`2 profile at Dt/a² = ${p.tau}: worst |FV - exact| (A2)`, p.worst, 0.01));

// 3. Part B, failed particle: the bare-kernel mesh reproduces Booth (B1;
// Cs at 1600 °C is the recorded miss, 1.14e-3, so the gate is the finding
// "agrees to the mesh error", 2e-3).
B.cases.filter((c) => c.bare).forEach((c) => {
  check(`3 ${c.el} ${c.t} °C: worst |bare mesh - Booth| over time`, c.bareWorst, 2e-3);
  check(`3 ${c.el} ${c.t} °C: intact never above bare`, c.intact.some((x, i) => x > c.bare[i] + 1e-9) ? 1 : 0, 0);
});
// 4. The finding the page states: intact Cs at 1600 °C exceeds 1e-4 x 1.
check("4 Cs 1600 °C intact / 1e-4 is above 10 (page: 'about N times more')", 10 / (last(cs("Cs", 1600).intact) / 1e-4), 1);
check("4 Sr 1400 °C intact exceeds 1e-4 x Booth", 1e-4 * last(cs("Sr", 1400).atops) / last(cs("Sr", 1400).intact), 1);
check("4 Cs 1200/1400 °C intact below 1e-12 (page: 'negligible')", Math.max(last(cs("Cs", 1200).intact), last(cs("Cs", 1400).intact)), 1e-12);
// 5. Silver: breakthrough >= intact at every hold and time; well-mixed >= intact.
B.cases.filter((c) => c.mixed).forEach((c) => {
  check(`5 Ag ${c.t} °C: breakthrough below intact (count)`, c.atops.filter((a, i) => a + 1e-12 < c.intact[i]).length, 0);
  check(`5 Ag ${c.t} °C: well mixed below intact (count)`, c.mixed.filter((m, i) => m + 1e-9 < c.intact[i]).length, 0);
  const r = last(c.atops) / last(c.intact);
  check(`5 Ag ${c.t} °C: ratio at 500 h, distance from [1, 10] (B3)`, r < 1 ? 1 - r : r > 10 ? r - 10 : 0, 0);
});
report("5 Ag 1400 °C, 10 h", "breakthrough below the well-mixed run on the 500 h time step; converged with --converge-ag1400 (1.84e-8 < 2.58e-8)");
// 6. Numerical uncertainty at 500 h (B4) as the page quotes it.
check("6 worst relative change at 500 h on refining", Math.max(...B.cases.map((c) => c.refineEnd)), 0.02);
report("6 worst at any time", Math.max(...B.cases.map((c) => c.refine)).toExponential(2) + " (early rising edge; not quoted)");

// 7. The pages: every data-da key filled by the story (run with a fake
// DOM), banners, footers, lesson-nav, and the FISSION order.
global.window.DECAYWALK_DATA = null;
const widgets = {};
global.window.Lesson = { css: () => "", el: () => ({}), widget: (id, f) => { widgets[id] = f; } };
global.window.LessonPlot = { sci: (x, p) => (x === 0 ? "0" : x.toExponential((p || 3) - 1)), Box: function () {}, linTicks: () => [], decTicks: () => [], legend: () => {} };
const filled = {};
global.document = { querySelectorAll: () => Object.keys(filled).map((k) => ({ getAttribute: () => k, set textContent(v) { filled[k] = v; } })) };
eval(fs.readFileSync(path.join(__dirname, "diffarc-story.js"), "utf8"));
["diffusion-three-ways.html", "atops-conservative.html"].forEach((f) => {
  const html = fs.readFileSync(path.join(__dirname, f), "utf8");
  const keys = [...html.matchAll(/data-da="([A-Za-z0-9]+)"/g)].map((m) => m[1]);
  keys.forEach((k) => { filled[k] = null; });
  widgets["da-numbers"]();
  const missing = keys.filter((k) => filled[k] === null || filled[k] === undefined || /NaN|undefined/.test(String(filled[k])));
  if (missing.length) console.log(`  unfilled in ${f}: ${missing.join(", ")}`);
  check(`7 ${f}: data-da keys left unfilled`, missing.length, 0);
  check(`7 ${f}: banner, footer tokens, lesson-nav`, html.includes("not yet reviewed by a human") && html.includes("@@COMMIT@@") && html.includes('class="lesson-nav"') ? 0 : 1, 0);
});
report("7 Cs 1600 °C intact over 1e-4", String(filled.csOver1e4));
const kit = fs.readFileSync(path.join(__dirname, "../neutronics/lesson-kit.js"), "utf8");
const at = (s) => kit.indexOf('["' + s + '"');
check("7 FISSION: three-ways, then conservative, then the big picture",
  at("diffusion-three-ways.html") > 0 && at("diffusion-three-ways.html") < at("atops-conservative.html") && at("atops-conservative.html") < at("atops-big-picture.html") ? 0 : 1, 0);

if (failed) { console.log(`${failed} check(s) failed`); process.exit(1); }
console.log("all diffusion-arc checks passed");
