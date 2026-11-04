// The data of the Fission products lesson "Breathing: how the release
// leaves the vessel" (atops-breathing.html), checked under node:
// `node docs/site/science/fission-products/check-breathing.js`. Data:
// breathing-data.js (crates/dhoby-ghaut/examples/lesson_breathing, gh:#1207).
// Gates are the generator's predictions or the facts the page states.
// Results of 2026-11-04: all pass.
"use strict";
const fs = require("fs"), path = require("path");
global.window = {};
eval(fs.readFileSync(path.join(__dirname, "breathing-data.js"), "utf8"));
const D = window.BREATHING_DATA;
let failed = 0;
function check(name, value, limit) {
  const ok = value <= limit;
  if (!ok) failed++;
  console.log(`${ok ? "ok  " : "FAIL"} ${name}: ${Number(value).toExponential(2)} (limit ${limit})`);
}
function report(name, text) { console.log(`REPORT ${name}: ${text}`); }
const at = (c, h) => c.hours.indexOf(h);

D.cases.forEach((c) => {
  const tag = `${c.name} ${c.peak} °C`;
  // 1. With the ideal-gas fraction the product bounds the well-mixed gas (the derivation).
  check(`1 ${tag}: samples where well-mixed > (1 - T0/Tmax) R`, c.vent.filter((v, i) => v !== null && c.mixedNoDecay[i] > c.ventExact[i] * c.release[i] + 1e-15).length, 0);
  // 1b. With the port's own fraction, the early-sample misses are where the port's fraction is below the ideal gas's.
  const miss = c.atops.map((a, i) => (a !== null && c.mixedNoDecay[i] > a + 1e-15 ? i : -1)).filter((i) => i >= 0);
  check(`1 ${tag}: port-fraction misses not explained by vent < ideal gas`, miss.filter((i) => !(c.vent[i] < c.ventExact[i])).length, 0);
  report(`1 ${tag}`, `port-fraction misses at ${miss.map((i) => c.hours[i] + " h").join(", ") || "none"}`);
  // 2. live_pools against the written-out formula (B2).
  check(`2 ${tag}: |live_pools - formula|`, Math.max(...c.mixedNoDecay.map((x, i) => Math.abs(x - c.formula[i]))), 1e-6);
  // 3. Nothing leaves after the hold (B5): last venting sample 100 h; well-mixed flat.
  const lastVent = Math.max(...c.hours.filter((h, i) => c.vent[i] !== null));
  check(`3 ${tag}: last venting sample is 100 h`, Math.abs(lastVent - 100), 0);
  check(`3 ${tag}: well-mixed change from 100 h to the end`, Math.abs(c.mixed[c.mixed.length - 1] - c.mixed[at(c, 100)]), 0);
  // 4. Decay only lowers it; refinement (B6).
  check(`4 ${tag}: decay raises the carried-out share (count)`, c.mixed.filter((m, i) => m > c.mixedNoDecay[i] + 1e-15).length, 0);
  check(`4 ${tag}: change on 400 substeps`, c.b6, 1e-6);
});
// 5. The headline the page states: the product carries out 2.5-50 times more at 100 h.
const ratios = D.cases.map((c) => c.mixed[at(c, 100)] / c.atops[at(c, 100)]);
check("5 largest well-mixed / product at 100 h is below 0.4", Math.max(...ratios), 0.4);
check("5 smallest well-mixed / product at 100 h is above 0.02", 0.02 / Math.min(...ratios), 1);
report("5 ratios", ratios.map((r) => r.toFixed(3)).join(" "));

// 6. The page: every data-br key filled, banner, footer, lesson-nav, FISSION order.
const widgets = {}, filled = {};
global.window.Lesson = { css: () => "", el: () => ({}), widget: (id, f) => { widgets[id] = f; } };
global.window.LessonPlot = { sci: (x, p) => (x === 0 ? "0" : x.toExponential((p || 3) - 1)), Box: function () {}, linTicks: () => [], decTicks: () => [], legend: () => 0 };
global.document = { querySelectorAll: () => Object.keys(filled).map((k) => ({ getAttribute: () => k, set textContent(v) { filled[k] = v; } })) };
eval(fs.readFileSync(path.join(__dirname, "breathing-story.js"), "utf8"));
const html = fs.readFileSync(path.join(__dirname, "atops-breathing.html"), "utf8");
const keys = [...html.matchAll(/data-br="([A-Za-z0-9]+)"/g)].map((m) => m[1]);
keys.forEach((k) => { filled[k] = null; });
widgets["br-numbers"]();
const missing = keys.filter((k) => filled[k] === null || /NaN|undefined/.test(String(filled[k])));
if (missing.length) console.log("  unfilled: " + missing.join(", "));
check("6 atops-breathing.html: data-br keys left unfilled", missing.length, 0);
check("6 atops-breathing.html: banner, footer tokens, lesson-nav", html.includes("not yet reviewed by a human") && html.includes("@@COMMIT@@") && html.includes('class="lesson-nav"') ? 0 : 1, 0);
const kit = fs.readFileSync(path.join(__dirname, "../neutronics/lesson-kit.js"), "utf8");
const pos = (s) => kit.indexOf('["' + s + '"');
check("6 FISSION: breathing right after the accident lesson", pos("atops-breathing.html") > pos("atops-accident.html") && pos("atops-breathing.html") < pos("atops-simplifications.html") ? 0 : 1, 0);

if (failed) { console.log(`${failed} check(s) failed`); process.exit(1); }
console.log("all breathing checks passed");
