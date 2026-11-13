// Browser check of the double-pipe twin demo (gh:#1182): the page runs in
// headless Chromium (SwiftShader), its worker evaluates the 1-D model, and
// the page is screenshotted in every view at phone (390x844, touch) and
// desktop (1280x800) widths. The lag check runs "Play" (Re_hot drifting, so
// the worker is asked for a new slice continuously) and, meanwhile, presses
// "+", drags the plot and opens the panel, recording frame intervals.
//
//   node browser_check.js <base-url> <out-dir>
//
// Prints one JSON report and writes it and the screenshots into <out-dir>.
const { chromium } = require('playwright');
const fs = require('fs');
const [base, out] = [process.argv[2], process.argv[3]];
const stats = (d) => { d = d.slice().sort((a, b) => a - b); if (!d.length) return null;
  return { n: d.length, median: +d[Math.floor(d.length / 2)].toFixed(1), p95: +d[Math.floor(d.length * 0.95)].toFixed(1), max: +d[d.length - 1].toFixed(1) }; };
const waitReady = async (page, ms) => { const t0 = Date.now(); while (Date.now() - t0 < ms) { if (/ready/.test(await page.title())) return (Date.now() - t0) / 1000; await page.waitForTimeout(200); } return null; };
(async () => {
  const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--use-angle=swiftshader', '--ignore-gpu-blocklist'] });
  const report = {};
  for (const [name, vp, mobile] of [['phone', { width: 390, height: 844 }, true], ['desktop', { width: 1280, height: 800 }, false]]) {
    const ctx = await browser.newContext({ viewport: vp, deviceScaleFactor: mobile ? 2 : 1, isMobile: mobile, hasTouch: mobile });
    const r = { shots: [], errors: [] };
    for (const view of ['twin', 'delta', 'sweep', 'filter']) {
      const page = await ctx.newPage();
      page.on('console', (m) => { if (m.type() === 'error' || m.type() === 'warning') r.errors.push(view + ' ' + m.type() + ': ' + m.text()); });
      page.on('pageerror', (e) => r.errors.push(view + ' pageerror: ' + e.message));
      await page.goto(`${base}/?view=${view}`);
      r[`${view}_ready_s`] = await waitReady(page, 120000);
      await page.waitForTimeout(800);
      const f = `${out}/${name}_${vp.width}x${vp.height}_${view}.png`;
      await page.screenshot({ path: f }); r.shots.push(f);
      if (view === 'twin') {
        r.title = await page.title();
        await page.goto(`${base}/?view=twin&model=calibrated`);
        await waitReady(page, 120000); await page.waitForTimeout(800);
        const g = `${out}/${name}_${vp.width}x${vp.height}_twin_calibrated.png`;
        await page.screenshot({ path: g }); r.shots.push(g);
        // Baseline: `?spin` redraws every frame with the worker idle, so its
        // frame intervals are SwiftShader's rendering cost alone.
        await page.goto(`${base}/?view=twin&spin`);
        await waitReady(page, 120000);
        await page.evaluate(() => { window.__f = []; const t = (now) => { window.__f.push(now); requestAnimationFrame(t); }; requestAnimationFrame(t); });
        await page.waitForTimeout(5000);
        { const fr = await page.evaluate(() => window.__f); const d = []; for (let i = 1; i < fr.length; i++) d.push(fr[i] - fr[i - 1]); r.frames_spin_worker_idle_ms = stats(d); }
        // Lag check: Play, then interact while the worker is kept busy.
        await page.goto(`${base}/?view=twin&play`);
        await page.waitForTimeout(5000); // busy most frames while playing, so no "ready" wait
        await page.evaluate(() => { window.__f = []; const t = (now) => { window.__f.push(now); requestAnimationFrame(t); }; requestAnimationFrame(t); });
        const inputs = [];
        for (let i = 0; i < 4; i++) {
          const a = Date.now();
          const plus = { x: vp.width - 8 - (40 + 40 + 64 + 12) + 20, y: 26 };
          if (mobile) await page.touchscreen.tap(plus.x, plus.y); else await page.mouse.click(plus.x, plus.y);
          await page.mouse.move(vp.width * 0.5, vp.height * 0.7); await page.mouse.down();
          await page.mouse.move(vp.width * 0.5 + 50, vp.height * 0.7 - 20, { steps: 6 }); await page.mouse.up();
          inputs.push(Date.now() - a);
          await page.waitForTimeout(700);
        }
        if (mobile) { // open the folded panel with "Controls »" while playing
          await page.touchscreen.tap(78, 30); await page.waitForTimeout(800);
          const h = `${out}/${name}_${vp.width}x${vp.height}_panel_open_playing.png`;
          await page.screenshot({ path: h }); r.shots.push(h);
        } else {
          const h = `${out}/${name}_${vp.width}x${vp.height}_playing_zoomed.png`;
          await page.screenshot({ path: h }); r.shots.push(h);
        }
        const fr = await page.evaluate(() => window.__f);
        const d = []; for (let i = 1; i < fr.length; i++) d.push(fr[i] - fr[i - 1]);
        r.frames_while_playing_ms = stats(d);
        r.input_round_ms = inputs;
        r.title_playing = await page.title();
      }
      await page.close();
    }
    report[name] = r;
    await ctx.close();
  }
  console.log(JSON.stringify(report, null, 1));
  fs.writeFileSync(`${out}/browser_report.json`, JSON.stringify(report, null, 1));
  await browser.close();
})();
