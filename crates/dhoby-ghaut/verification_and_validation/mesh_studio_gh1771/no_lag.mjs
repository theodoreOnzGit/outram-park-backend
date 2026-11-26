// Usage (serve the build as in render.mjs first):
//   node no_lag.mjs http://localhost:8765 "shape=pebble" 1100 760 [shot.png]
// Frame gaps while Mesh Studio builds its mesh in the worker, with the view
// being dragged the whole time (the no-lag rule). Prints the gaps in the
// build window (from 1 s after load, when the app has started, to the title
// reporting cells) and in the 2 s after it (the result arriving and its cut).
import { chromium } from "playwright";
const [base, query, w, h, shot] = process.argv.slice(2);
const browser = await chromium.launch({});
const page = await browser.newPage({ viewport: { width: +w, height: +h } });
await page.addInitScript(() => {
  window.__f = []; const tick = (t) => { window.__f.push(t); requestAnimationFrame(tick); }; requestAnimationFrame(tick);
});
await page.goto(`${base}/?${query}`);
await page.waitForTimeout(1000);
const tStart = await page.evaluate(() => performance.now());
let title = "", x = +w * 0.7, dir = 1;
await page.mouse.move(x, +h * 0.4); await page.mouse.down();
for (let i = 0; i < 20000; i++) {
  title = await page.title(); if (/cells/.test(title)) break;
  x += 15 * dir; if (x > +w * 0.9 || x < +w * 0.5) dir = -dir;
  await page.mouse.move(x, +h * 0.4); await page.waitForTimeout(50);
}
await page.mouse.up();
const tDone = await page.evaluate(() => performance.now());
await page.waitForTimeout(2000);
// The same drag for as long again with no build running: the drawing's own cost.
const tB = await page.evaluate(() => performance.now());
await page.mouse.down();
const nB = Math.max(20, Math.round((tDone - tStart) / 50));
for (let i = 0; i < nB; i++) { x += 15 * dir; if (x > +w * 0.9 || x < +w * 0.5) dir = -dir; await page.mouse.move(x, +h * 0.4); await page.waitForTimeout(50); }
await page.mouse.up();
const tBe = await page.evaluate(() => performance.now());
const f = await page.evaluate(() => window.__f);
const stats = (a, b) => {
  const g = []; for (let i = 1; i < f.length; i++) if (f[i] >= a && f[i] <= b) g.push(f[i] - f[i - 1]);
  g.sort((p, q) => p - q); const q = (p) => g[Math.min(g.length - 1, Math.floor(p * g.length))]?.toFixed(1);
  return `${g.length} frames, median ${q(0.5)} ms, p99 ${q(0.99)} ms, max ${g[g.length - 1]?.toFixed(1)} ms`;
};
console.log(`${query} @${w}x${h} | ${title} | build window ${((tDone - tStart) / 1000).toFixed(1)} s: ${stats(tStart, tDone)} | after: ${stats(tDone, tDone + 2000)} | same drag, idle: ${stats(tB, tBe)}`);
if (shot) await page.screenshot({ path: shot });
await browser.close();
