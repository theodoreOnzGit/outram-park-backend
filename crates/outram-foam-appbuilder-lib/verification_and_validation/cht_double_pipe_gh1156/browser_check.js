// Browser check of Foam Studio's double-pipe exchanger (gh:#1156; adapted
// from laminar_pipe_gh1155/browser_check.js): the page runs in headless
// Chromium (SwiftShader), meshes and solves the exchanger (flows, then the
// three temperatures) in its Worker,
// and stays responsive (frame intervals; zoom buttons and an orbit drag)
// while the worker solves. Phone (390x844, touch) and desktop (1280x800).
//
//   node browser_check.js <base-url> <out-dir> [cells]
//
// Prints one JSON report and writes screenshots into <out-dir>.
const { chromium } = require('/opt/node22/lib/node_modules/playwright');
const fs = require('fs');
const [base, out, cellsArg] = [process.argv[2], process.argv[3], process.argv[4] || '20000'];
const stats = (d) => { d = d.slice().sort((a, b) => a - b); if (!d.length) return null;
  return { n: d.length, median: +d[Math.floor(d.length / 2)].toFixed(1), p95: +d[Math.floor(d.length * 0.95)].toFixed(1), max: +d[d.length - 1].toFixed(1) }; };
(async () => {
  const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--use-angle=swiftshader', '--ignore-gpu-blocklist'] });
  const report = {};
  for (const [name, vp, mobile] of [['phone', { width: 390, height: 844 }, true], ['desktop', { width: 1280, height: 800 }, false]]) {
    const ctx = await browser.newContext({ viewport: vp, deviceScaleFactor: mobile ? 2 : 1, isMobile: mobile, hasTouch: mobile });
    const page = await ctx.newPage();
    const logs = [];
    page.on('console', (m) => { if (m.type() !== 'log') logs.push(m.type() + ': ' + m.text()); });
    page.on('pageerror', (e) => logs.push('pageerror: ' + e.message));
    await page.addInitScript(() => { window.__f = []; const t = (now) => { window.__f.push([now, document.title]); requestAnimationFrame(t); }; requestAnimationFrame(t); });
    const t0 = Date.now();
    await page.goto(`${base}/?autostart&shape=exchanger&cells=${cellsArg}`);
    let title = '', meshedAt = null, solvingAt = null, steadyAt = null, shots = 0;
    const inputs = [];
    while (Date.now() - t0 < 400000) {
      title = await page.title();
      const s = (Date.now() - t0) / 1000;
      if (meshedAt === null && /cells/.test(title)) meshedAt = s;
      if (solvingAt === null && /solving/.test(title)) {
        solvingAt = s;
        await page.screenshot({ path: `${out}/${name}_start.png` });
      }
      if (/ · steady/.test(title)) { steadyAt = s; break; }
      if (solvingAt !== null && inputs.length < 6) {
        // While the worker solves: press "+" (zoom in) and drag on the view.
        const a = Date.now();
        const plus = { x: vp.width - 8 - (40 + 40 + 64 + 12) + 20, y: 26 };
        if (mobile) await page.touchscreen.tap(plus.x, plus.y); else await page.mouse.click(plus.x, plus.y);
        await page.mouse.move(vp.width * 0.5, vp.height * 0.3); await page.mouse.down();
        await page.mouse.move(vp.width * 0.5 + 60, vp.height * 0.3 + 20, { steps: 6 }); await page.mouse.up();
        inputs.push(Date.now() - a);
        if (shots++ === 2) await page.screenshot({ path: `${out}/${name}_solving.png` });
      }
      await page.waitForTimeout(250);
    }
    await page.screenshot({ path: `${out}/${name}_steady.png` });
    const frames = await page.evaluate(() => window.__f);
    const solving = [];
    for (let i = 1; i < frames.length; i++) if (/solving/.test(frames[i][1])) solving.push(frames[i][0] - frames[i - 1][0]);
    const idle = [];
    for (let i = 1; i < frames.length; i++) if (/steady/.test(frames[i][1])) idle.push(frames[i][0] - frames[i - 1][0]);
    await page.waitForTimeout(3000);
    const frames2 = await page.evaluate(() => window.__f);
    for (let i = frames.length; i < frames2.length; i++) idle.push(frames2[i][0] - frames2[i - 1][0]);
    report[name] = { title, meshed_s: meshedAt, solving_s: solvingAt, steady_s: steadyAt, wall_to_steady_s: steadyAt && solvingAt ? +(steadyAt - solvingAt).toFixed(1) : null,
      frames_while_solving_ms: stats(solving), frames_after_ms: stats(idle), input_round_ms: inputs, errors: logs.slice(0, 10) };
    await ctx.close();
  }
  console.log(JSON.stringify(report, null, 1));
  fs.writeFileSync(`${out}/browser_report_${cellsArg}.json`, JSON.stringify(report, null, 1));
  await browser.close();
})();
