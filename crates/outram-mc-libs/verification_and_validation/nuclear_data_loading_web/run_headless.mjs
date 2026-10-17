// Drive probe.html in headless Chromium over the DevTools protocol and print
// what the page computed. Test harness for gh #935/#936; no dependencies
// (Node's built-in WebSocket, Node 22 or later).
//
//   node run_headless.mjs <url> [timeout-seconds] [out.json]
//
// Starts its own Chromium (`chromium`, or $CHROMIUM) with a fresh profile,
// opens <url>, polls `window.__probe` until `done`, prints the page's console,
// writes the state to out.json if given, and exits 0 only if the page
// finished without an error.
import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const [url, timeoutArg, outFile] = process.argv.slice(2);
if (!url) {
  console.error('usage: node run_headless.mjs <url> [timeout-seconds] [out.json]');
  process.exit(2);
}
const timeoutMs = 1000 * Number(timeoutArg || 1800);
const profile = mkdtempSync(join(tmpdir(), 'nd-probe-'));
const chrome = spawn(process.env.CHROMIUM || 'chromium', [
  '--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profile}`,
  '--no-first-run', '--no-default-browser-check', '--disable-gpu', 'about:blank',
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
ws.addEventListener('message', (ev) => {
  const msg = JSON.parse(ev.data);
  if (msg.id && pending.has(msg.id)) {
    pending.get(msg.id)(msg);
    pending.delete(msg.id);
  } else if (msg.method === 'Runtime.consoleAPICalled') {
    console.log('[page]', msg.params.args.map((a) => a.value ?? a.description).join(' '));
  } else if (msg.method === 'Runtime.exceptionThrown') {
    console.log('[page exception]', JSON.stringify(msg.params.exceptionDetails));
  }
});
const send = (method, params = {}, sessionId) => new Promise((resolve) => {
  const id = nextId++;
  pending.set(id, resolve);
  ws.send(JSON.stringify({ id, method, params, sessionId }));
});

const { result: { targetId } } = await send('Target.createTarget', { url: 'about:blank' });
const { result: { sessionId } } = await send('Target.attachToTarget', { targetId, flatten: true });
await send('Runtime.enable', {}, sessionId);
await send('Page.enable', {}, sessionId);
await send('Page.navigate', { url }, sessionId);

const start = Date.now();
let state = null;
while (Date.now() - start < timeoutMs) {
  await new Promise((r) => setTimeout(r, 3000));
  const r = await send('Runtime.evaluate', {
    expression: 'JSON.stringify(window.__probe || null)', returnByValue: true,
  }, sessionId);
  state = JSON.parse(r.result?.result?.value ?? 'null');
  if (state && state.done) break;
}
const version = await send('Browser.getVersion');
const record = { url, browser: version.result, wallSeconds: (Date.now() - start) / 1000, state };
console.log(JSON.stringify(record, null, 2));
if (outFile) writeFileSync(outFile, JSON.stringify(record, null, 2));
ws.close();
chrome.kill();
try { rmSync(profile, { recursive: true, force: true }); } catch { /* best effort */ }
process.exit(state && state.done && !state.error ? 0 : 1);
