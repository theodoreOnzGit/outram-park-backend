// No-lag check for the htr10 "Control rods" view (gh:#1025): with the worker
// pool processing data and then running k_eff, measure the page's frame
// intervals and that a slider drag updates the page title at once.
// node lagcheck.js <base-url> <out-dir>
const { chromium } = require('/opt/node22/lib/node_modules/playwright');
const fs = require('fs');
(async () => {
  const [base, out] = [process.argv[2], process.argv[3]];
  const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--use-gl=swiftshader', '--ignore-gpu-blocklist'] });
  const ctx = await browser.newContext({ viewport: { width: 1280, height: 900 } });
  const page = await ctx.newPage();
  await page.goto(`${base}/?rung=htr10&mode=watch&view=rods&autostart=1&particles=300&inactive=2&active=3&workers=2`);
  const frames = async (ms) => page.evaluate((ms) => new Promise(res => {
    const t = []; const t0 = performance.now();
    const f = (now) => { t.push(now); if (now - t0 < ms) requestAnimationFrame(f); else {
      const d = t.slice(1).map((x, i) => x - t[i]); d.sort((a, b) => a - b);
      res({ n: d.length, median: d[Math.floor(d.length / 2)], p95: d[Math.floor(d.length * 0.95)], max: d[d.length - 1] }); } };
    requestAnimationFrame(f); }), ms);
  const log = [];
  const t0 = Date.now();
  for (let i = 0; i < 40; i++) {
    const f = await frames(3000);
    // Drag the master slider ("all rods") a little and see the title follow.
    const x0 = 14 + (i % 2 ? 60 : 20);
    await page.mouse.move(x0, 570); await page.mouse.down(); await page.mouse.move(x0 + 40, 570, { steps: 5 }); await page.mouse.up();
    const ta = Date.now(); let title = '';
    for (let k = 0; k < 20; k++) { title = await page.title(); if (/all ten at|withdrawn/.test(title)) break; await page.waitForTimeout(50); }
    const line = `${((Date.now() - t0) / 1000).toFixed(0)} s: frames n=${f.n} median=${f.median.toFixed(1)} ms p95=${f.p95.toFixed(1)} max=${f.max.toFixed(1)}; title after drag (${Date.now() - ta} ms): ${title}`;
    console.log(line); log.push(line);
    if (/running position|generation/.test(title) && i > 6) {}
    if (i === 10 || i === 25) await page.screenshot({ path: `${out}/lagcheck_desktop_${i}.png` });
  }
  fs.writeFileSync(`${out}/lagcheck.txt`, log.join('\n') + '\n');
  await browser.close();
})();
