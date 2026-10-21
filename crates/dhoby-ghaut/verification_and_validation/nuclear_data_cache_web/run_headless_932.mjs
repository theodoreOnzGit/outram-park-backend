// Drive a built Monte Carlo demo page in headless Chromium over the DevTools
// protocol and record, for gh:#932: every console line (the pool's per-
// generation k at full precision, the cache's "made in" lines), the page's
// long tasks and frame gaps while the data are processed (the no-lag rule),
// the main thread's answer time to a probe sent every 2 s while it works, and
// at the end every record in this origin's IndexedDB nuclear-data store with
// its SHA-256 recomputed in the page from the stored words. Test harness, not
// a demo; no dependencies (Node 22 or later, built-in WebSocket).
//
//   node run_headless_932.mjs <url> <timeout-s> <out.json> [--phone] [--generations N] [--profile DIR]
//
// Ends when the page has logged N generations of the worker pool (default 25)
// or on the timeout. With --profile the browser keeps that profile (a second
// run on it is a cached load); without it a fresh one is made and removed.
// Exits 0 only if N generations were seen.
import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const argv = process.argv.slice(2);
const [url, timeoutArg, outFile] = argv;
if (!url || !outFile) {
  console.error('usage: node run_headless_932.mjs <url> <timeout-s> <out.json> [--phone] [--generations N] [--profile DIR]');
  process.exit(2);
}
const flag = (f) => argv.includes(f);
const opt = (f, d) => { const i = argv.indexOf(f); return i >= 0 ? argv[i + 1] : d; };
const phone = flag('--phone');
const wantGens = Number(opt('--generations', '25'));
const keepProfile = opt('--profile', null);
const timeoutMs = 1000 * Number(timeoutArg || 3600);
const profile = keepProfile || mkdtempSync(join(tmpdir(), 'nd932-'));

const chrome = spawn(process.env.CHROMIUM || 'chromium', [
  '--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profile}`,
  '--no-first-run', '--no-default-browser-check', '--enable-unsafe-swiftshader',
  phone ? '--window-size=390,844' : '--window-size=1280,800', 'about:blank',
], { stdio: ['ignore', 'ignore', 'pipe'] });

const wsUrl = await new Promise((resolve, reject) => {
  let buf = '';
  chrome.stderr.on('data', (d) => {
    buf += d;
    const m = buf.match(/DevTools listening on (ws:\/\/\S+)/);
    if (m) resolve(m[1]);
  });
  chrome.on('exit', (c) => reject(new Error(`chromium exited ${c}: ${buf}`)));
});

const ws = new WebSocket(wsUrl);
await new Promise((r) => ws.addEventListener('open', r, { once: true }));
let nextId = 1;
const pending = new Map();
const consoleLines = [];
const t0 = Date.now();
const since = () => (Date.now() - t0) / 1000;
ws.addEventListener('message', (ev) => {
  const msg = JSON.parse(ev.data);
  if (msg.id && pending.has(msg.id)) {
    pending.get(msg.id)(msg);
    pending.delete(msg.id);
  } else if (msg.method === 'Runtime.consoleAPICalled') {
    const text = msg.params.args.map((a) => a.value ?? a.description).join(' ');
    consoleLines.push({ t: since(), text });
    console.log(`[${since().toFixed(1)} s]`, text.slice(0, 240));
  } else if (msg.method === 'Runtime.exceptionThrown') {
    consoleLines.push({ t: since(), text: 'EXCEPTION ' + JSON.stringify(msg.params.exceptionDetails) });
  } else if (msg.method === 'Target.attachedToTarget') {
    // Workers: listen to their consoles too (the cache's lines are logged there).
    const sid = msg.params.sessionId;
    send('Runtime.enable', {}, sid);
    send('Runtime.runIfWaitingForDebugger', {}, sid);
  }
});
const send = (method, params = {}, sessionId) => new Promise((resolve) => {
  const id = nextId++;
  pending.set(id, resolve);
  ws.send(JSON.stringify({ id, method, params, sessionId }));
});

const { result: { targetId } } = await send('Target.createTarget', { url: 'about:blank' });
const { result: { sessionId } } = await send('Target.attachToTarget', { targetId, flatten: true });
await send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: false, flatten: true }, sessionId);
await send('Runtime.enable', {}, sessionId);
await send('Page.enable', {}, sessionId);
if (phone) {
  await send('Emulation.setDeviceMetricsOverride', { width: 390, height: 844, deviceScaleFactor: 3, mobile: true }, sessionId);
  await send('Emulation.setTouchEmulationEnabled', { enabled: true }, sessionId);
}
// Long tasks and frame gaps, from the first script on.
await send('Page.addScriptToEvaluateOnNewDocument', {
  source: `
    window.__lt = []; window.__gaps = { max: 0, over100: 0, frames: 0 };
    try { new PerformanceObserver((l) => { for (const e of l.getEntries()) window.__lt.push([e.startTime, e.duration]); })
      .observe({ type: 'longtask', buffered: true }); } catch (e) {}
    let last = 0;
    const tick = (t) => { if (last && document.visibilityState === 'visible') { const g = t - last; window.__gaps.frames++;
      if (g > window.__gaps.max) { window.__gaps.max = g; window.__gaps.maxAt = t; } if (g > 100) window.__gaps.over100++; }
      last = t; requestAnimationFrame(tick); };
    requestAnimationFrame(tick);`,
}, sessionId);
await send('Page.navigate', { url }, sessionId);

// The probe: every 2 s, a wheel event over the page's centre (a zoom, as a
// reader would) and a round trip to the main thread, timed.
const probes = [];
let gens = 0;
let title = '';
while (Date.now() - t0 < timeoutMs) {
  await new Promise((r) => setTimeout(r, 2000));
  const p0 = Date.now();
  await send('Input.dispatchMouseEvent', { type: 'mouseWheel', x: phone ? 195 : 640, y: phone ? 420 : 400, deltaX: 0, deltaY: (probes.length % 2 ? 120 : -120) }, sessionId);
  const r = await send('Runtime.evaluate', { expression: 'document.title', returnByValue: true }, sessionId);
  probes.push({ t: since(), ms: Date.now() - p0 });
  title = r.result?.result?.value ?? title;
  gens = consoleLines.filter((l) => /worker pool: .*: generation \d+ k = /.test(l.text)).length;
  if (gens >= wantGens) break;
}

const report = await send('Runtime.evaluate', {
  awaitPromise: true, returnByValue: true,
  expression: `(async () => {
    const hex = (b) => [...new Uint8Array(b)].map((x) => x.toString(16).padStart(2, '0')).join('');
    const db = await new Promise((res, rej) => { const r = indexedDB.open('outram-park-nuclear-data'); r.onsuccess = () => res(r.result); r.onerror = () => rej(r.error); });
    if (!db.objectStoreNames.contains('records')) return { records: [] };
    const all = await new Promise((res, rej) => { const r = db.transaction('records').objectStore('records').getAll(); r.onsuccess = () => res(r.result); r.onerror = () => rej(r.error); });
    const out = [];
    for (const rec of all) {
      const w = rec.words instanceof Float64Array ? rec.words : new Float64Array(rec.words);
      out.push({ key: rec.key, label: rec.label, code: rec.code, words: w.length, stored: rec.sha256,
        recomputed: hex(await crypto.subtle.digest('SHA-256', w.buffer.slice(w.byteOffset, w.byteOffset + w.byteLength))) });
    }
    return { records: out, longTasks: window.__lt, gaps: window.__gaps, title: document.title };
  })()`,
}, sessionId);
const version = await send('Browser.getVersion');
const record = {
  url, phone, browser: version.result?.product, wallSeconds: since(), generations: gens, title,
  probes, page: report.result?.result?.value ?? report.result, console: consoleLines,
};
writeFileSync(outFile, JSON.stringify(record, null, 2));
const recs = record.page?.records ?? [];
console.log(`generations ${gens}; records ${recs.length}; sha mismatches ${recs.filter((r) => r.stored !== r.recomputed).length}; ` +
  `max probe ${Math.max(0, ...probes.map((p) => p.ms))} ms; max frame gap ${record.page?.gaps?.max?.toFixed?.(0)} ms`);
ws.close();
chrome.kill();
if (!keepProfile) { try { rmSync(profile, { recursive: true, force: true }); } catch { /* best effort */ } }
process.exit(gens >= wantGens ? 0 : 1);
