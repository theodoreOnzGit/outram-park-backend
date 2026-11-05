// Browser check for Foam Studio's unstructured pipe (cfMesh tet -> polyhedral
// dual -> wall prism layers), gh:#1202 / #1240 / #1241: in headless Chromium,
// load the page with ?mesh=unstructured&h=..&layers=..&autostart=1, wait for
// the mesh and the steady solve (the page title carries "N cells", "step k"
// and "steady"), measure frame intervals while it works (the no-lag rule) and
// take screenshots at desktop and phone width.
//
//   node browser_check.js <base-url> <out-dir>
//
// Results go to <out-dir>/browser_check.txt and *.png.
const { chromium } = require('/opt/node22/lib/node_modules/playwright');
const fs = require('fs');

const CASES = [
  { name: 'structured_desktop', q: 'autostart=1', vp: [1280, 900] },
  { name: 'dual_h5_0layers_desktop', q: 'mesh=unstructured&h=5&layers=0&autostart=1', vp: [1280, 900] },
  { name: 'dual_h5_1layer_desktop', q: 'mesh=unstructured&h=5&layers=1&autostart=1', vp: [1280, 900] },
  { name: 'dual_h5_2layers_desktop', q: 'mesh=unstructured&h=5&layers=2&autostart=1', vp: [1280, 900] },
  { name: 'dual_h5_2layers_phone', q: 'mesh=unstructured&h=5&layers=2&autostart=1', vp: [390, 844] },
];
const TIMEOUT_MS = 15 * 60 * 1000;

(async () => {
  const [base, out] = [process.argv[2], process.argv[3]];
  fs.mkdirSync(out, { recursive: true });
  const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--use-gl=swiftshader', '--ignore-gpu-blocklist'] });
  const log = [];
  const say = (s) => { console.log(s); log.push(s); fs.writeFileSync(`${out}/browser_check.txt`, log.join('\n') + '\n'); };
  for (const c of CASES) {
    const ctx = await browser.newContext({ viewport: { width: c.vp[0], height: c.vp[1] } });
    const page = await ctx.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
    const t0 = Date.now();
    await page.goto(`${base}/?${c.q}`);
    // Frame intervals over `ms` milliseconds, measured on the UI thread.
    const frames = (ms) => page.evaluate((ms) => new Promise((res) => {
      const t = []; const s = performance.now();
      const f = (now) => { t.push(now); if (now - s < ms) requestAnimationFrame(f); else {
        const d = t.slice(1).map((x, i) => x - t[i]).sort((a, b) => a - b);
        res({ n: d.length, median: d[Math.floor(d.length / 2)] || 0, p95: d[Math.floor(d.length * 0.95)] || 0, max: d[d.length - 1] || 0 }); } };
      requestAnimationFrame(f); }), ms);
    let title = '', tMeshed = null, tSteady = null, worst = 0, shotMid = false;
    while (Date.now() - t0 < TIMEOUT_MS) {
      const f = await frames(2000);
      worst = Math.max(worst, f.max);
      title = await page.title();
      if (tMeshed === null && /cells/.test(title)) tMeshed = Date.now() - t0;
      if (tMeshed !== null && !shotMid && /step/.test(title)) {
        await page.screenshot({ path: `${out}/${c.name}_solving.png` });
        shotMid = true;
      }
      say(`${c.name} ${((Date.now() - t0) / 1000).toFixed(0)} s: ${title} | frames median ${f.median.toFixed(1)} p95 ${f.p95.toFixed(1)} max ${f.max.toFixed(1)} ms`);
      if (/steady/.test(title)) { tSteady = Date.now() - t0; break; }
    }
    await page.screenshot({ path: `${out}/${c.name}_final.png` });
    say(`RESULT ${c.name}: meshed ${tMeshed === null ? 'NO' : (tMeshed / 1000).toFixed(0) + ' s'}, steady ${tSteady === null ? 'NO' : (tSteady / 1000).toFixed(0) + ' s'}, worst frame ${worst.toFixed(0)} ms, title "${title}", errors ${errors.length}${errors.length ? ': ' + errors.slice(0, 3).join(' | ') : ''}`);
    await ctx.close();
  }
  await browser.close();
})();
