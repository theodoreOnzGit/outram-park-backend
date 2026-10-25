// No-lag check for the merged HTR-10 whole-core tab (maintainer 2026-10-25):
// with the worker pool processing the data and then running k_eff, measure
// the page's frame intervals; while the data process (the rods are locked
// during a run, by design), drag the master rod slider and see the page
// title follow. Screenshots once the run has generations: k and entropy in
// the bottom dock.
// node lagcheck.js <base-url> <out-dir>
const { chromium } = require('/opt/node22/lib/node_modules/playwright');
const fs = require('fs');
(async () => {
  const [base, out] = [process.argv[2], process.argv[3]];
  fs.mkdirSync(out, { recursive: true });
  const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--use-gl=swiftshader', '--ignore-gpu-blocklist'] });
  const ctx = await browser.newContext({ viewport: { width: 1280, height: 900 } });
  const page = await ctx.newPage();
  await page.goto(`${base}/?rung=htr10&mode=watch&view=core&autostart=1&particles=300&inactive=2&active=6&workers=2`);
  const frames = async (ms) => page.evaluate((ms) => new Promise(res => {
    const t = []; const t0 = performance.now();
    const f = (now) => { t.push(now); if (now - t0 < ms) requestAnimationFrame(f); else {
      const d = t.slice(1).map((x, i) => x - t[i]); d.sort((a, b) => a - b);
      res({ n: d.length, median: d[Math.floor(d.length / 2)], p95: d[Math.floor(d.length * 0.95)], max: d[d.length - 1] }); } };
    requestAnimationFrame(f); }), ms);
  // Where the master "all rods" slider is (desktop, controls open), found on
  // the 2026-10-25 screenshot.
  const slider = { x: 20, y: 678 };
  const log = [];
  const t0 = Date.now();
  let shot = 0;
  for (let i = 0; i < 60; i++) {
    const f = await frames(3000);
    let title = await page.title();
    let drag = '';
    if (!/generation|k_eff gen/.test(title)) {
      const x0 = slider.x + (i % 2 ? 60 : 0);
      await page.mouse.move(x0, slider.y); await page.mouse.down(); await page.mouse.move(x0 + 40, slider.y, { steps: 5 }); await page.mouse.up();
      const ta = Date.now();
      for (let k = 0; k < 20; k++) { title = await page.title(); if (/all ten at|withdrawn/.test(title)) break; await page.waitForTimeout(50); }
      drag = ` title ${Date.now() - ta} ms after a rod drag;`;
    }
    const line = `${((Date.now() - t0) / 1000).toFixed(0)} s: frames n=${f.n} median=${f.median.toFixed(1)} ms p95=${f.p95.toFixed(1)} max=${f.max.toFixed(1)};${drag} ${title}`;
    console.log(line); log.push(line);
    if (/k_eff generation (\d+)\/8/.test(title) && shot < 2) {
      await page.screenshot({ path: `${out}/lagcheck_run_${shot}.png` }); shot++;
    }
    if (/mean/.test(title) && /generation 8\/8/.test(title)) { await page.screenshot({ path: `${out}/lagcheck_run_done.png` }); break; }
  }
  fs.writeFileSync(`${out}/lagcheck.txt`, log.join('\n') + '\n');
  await browser.close();
})();
