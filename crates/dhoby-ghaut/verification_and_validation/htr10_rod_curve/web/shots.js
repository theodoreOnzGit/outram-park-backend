// Screenshots of the htr10 "Control rods" view (gh:#1025) at phone and desktop size.
// node shots.js <base-url> <out-dir>
const { chromium } = require('/opt/node22/lib/node_modules/playwright');
const fs = require('fs');
(async () => {
  const base = process.argv[2];
  const out = process.argv[3];
  fs.mkdirSync(out, { recursive: true });
  const browser = await chromium.launch({
    executablePath: fs.existsSync('/opt/pw-browsers/chromium-1194/chrome-linux/chrome')
      ? '/opt/pw-browsers/chromium-1194/chrome-linux/chrome' : undefined,
    args: ['--enable-unsafe-swiftshader', '--use-gl=swiftshader', '--ignore-gpu-blocklist'],
  });
  const cases = [
    { name: 'phone_390x844', w: 390, h: 844, mobile: true },
    { name: 'desktop_1280x900', w: 1280, h: 900, mobile: false },
  ];
  const extra = process.argv[4] || '';
  for (const c of cases) {
    const ctx = await browser.newContext({ viewport: { width: c.w, height: c.h }, deviceScaleFactor: 1, isMobile: c.mobile, hasTouch: c.mobile });
    const page = await ctx.newPage();
    page.on('console', m => { if (m.type() === 'error') console.log(`[${c.name}] console error: ${m.text()}`); });
    const url = `${base}/?rung=htr10&mode=watch&view=rods${extra}`;
    await page.goto(url);
    let title = '';
    for (let i = 0; i < 90; i++) {
      await page.waitForTimeout(1000);
      title = await page.title();
      if (title.includes('control rods')) break;
    }
    await page.waitForTimeout(8000);
    await page.screenshot({ path: `${out}/${c.name}${extra.replace(/[^a-z0-9]+/gi, '_')}.png` });
    console.log(`${c.name}: ${title}`);
    await ctx.close();
  }
  await browser.close();
})();
