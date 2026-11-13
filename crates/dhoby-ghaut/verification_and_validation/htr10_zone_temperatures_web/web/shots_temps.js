// Screenshots of the HTR-10 whole-core tab's zone-temperature sliders
// (gh:#1142) at phone and desktop size, with the fuel above the 1600 °C TRISO
// limit so the warning shows. Test harness, not a demo.
//   node shots_temps.js <base-url> <out-dir>
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
  const query = '?rung=htr10&mode=watch&view=core&temps=all:300,fuel:1650,coolant:500';
  for (const s of sizes) {
    const ctx = await browser.newContext({ viewport: { width: s.w, height: s.h }, deviceScaleFactor: 1, isMobile: s.mobile, hasTouch: s.mobile });
    const page = await ctx.newPage();
    page.on('console', m => { if (m.type() === 'error') console.log(`[${s.name}] console error: ${m.text()}`); });
    await page.goto(`${base}/${query}`);
    let title = '';
    for (let i = 0; i < 120; i++) {
      await page.waitForTimeout(1000);
      title = await page.title();
      if (/whole core/.test(title) && !/loading/i.test(title)) break;
    }
    await page.waitForTimeout(5000);
    if (s.mobile) {
      await page.mouse.click(70, 30); // "Controls »"
      await page.waitForTimeout(2500);
    }
    const px = s.mobile ? 180 : 160;
    for (let k = 0; k < 4; k++) {
      await page.screenshot({ path: `${out}/temps_${s.name}_${k}.png` });
      await page.mouse.move(px, Math.round(s.h / 2));
      await page.mouse.wheel(0, 500);
      await page.waitForTimeout(1500);
    }
    console.log(`${s.name}: ${title}`);
    await ctx.close();
  }
  await browser.close();
})();
