// Screenshots of the merged HTR-10 whole-core tab (layers N + control rods,
// k_eff/entropy bottom dock, views right dock) and of Godiva's Run mode with
// the same bottom dock, at phone and desktop size (maintainer 2026-10-25).
// node shots.js <base-url> <out-dir>
const { chromium } = require('/opt/node22/lib/node_modules/playwright');
const fs = require('fs');
(async () => {
  const [base, out] = [process.argv[2], process.argv[3]];
  fs.mkdirSync(out, { recursive: true });
  const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--use-gl=swiftshader', '--ignore-gpu-blocklist'] });
  const sizes = [
    { name: 'phone_390x844', w: 390, h: 844, mobile: true },
    { name: 'desktop_1280x900', w: 1280, h: 900, mobile: false },
  ];
  const cases = [
    { name: 'htr10_core', query: '?rung=htr10&mode=watch&view=core', ready: /whole core/ },
    { name: 'htr10_rods_link', query: '?rung=htr10&mode=watch&view=rods', ready: /whole core/ },
    { name: 'godiva_run', query: '?rung=godiva&mode=run', ready: /Godiva/i },
  ];
  for (const c of cases) {
    for (const s of sizes) {
      const ctx = await browser.newContext({ viewport: { width: s.w, height: s.h }, deviceScaleFactor: 1, isMobile: s.mobile, hasTouch: s.mobile });
      const page = await ctx.newPage();
      page.on('console', m => { if (m.type() === 'error') console.log(`[${c.name} ${s.name}] console error: ${m.text()}`); });
      await page.goto(`${base}/${c.query}`);
      let title = '';
      for (let i = 0; i < 120; i++) {
        await page.waitForTimeout(1000);
        title = await page.title();
        if (c.ready.test(title) && !/loading/i.test(title)) break;
      }
      await page.waitForTimeout(6000);
      await page.screenshot({ path: `${out}/${c.name}_${s.name}.png` });
      console.log(`${c.name} ${s.name}: ${title}`);
      if (c.name === 'htr10_core' && s.mobile) {
        // Open the folded right dock from its tab (top right, below the zoom buttons).
        await page.mouse.click(s.w - 8 - 66, 56 + 22);
        await page.waitForTimeout(6000);
        await page.screenshot({ path: `${out}/${c.name}_${s.name}_views_open.png` });
        // Its second half.
        await page.mouse.click(s.w - Math.round(0.88 * s.w * 0.25), 44 + 22);
        await page.waitForTimeout(3000);
        await page.screenshot({ path: `${out}/${c.name}_${s.name}_views_graphs.png` });
        // Hide it again, open the controls.
        await page.mouse.click(s.w - Math.round(0.88 * s.w / 2), 22);
        await page.waitForTimeout(1500);
        await page.mouse.click(70, 30);
        await page.waitForTimeout(2500);
        await page.screenshot({ path: `${out}/${c.name}_${s.name}_controls.png` });
      }
      await ctx.close();
    }
  }
  await browser.close();
})();
