// Runs the pipe cases in a Web Worker inside headless Chromium (Playwright),
// one Worker per case, and prints one JSON line per case.
//
//   node run_worker.mjs path/to/foam_wasm_pipe.wasm [case-name ...]
//
// Serves this folder and the wasm from a local HTTP server (a Worker cannot
// be started from file://). Needs `require('playwright')` to resolve.
import { createServer } from 'node:http';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const wasmPath = process.argv[2];
const only = process.argv.slice(3);
const require = createRequire(import.meta.url);
const { chromium } = require('playwright');

// Same cases, in the same order, as the native runs in README.md.
const cases = [
  { name: 'ogrid-5k', route: 'ogrid', a: 6, b: 4, c: 40, dt: 0.5 },
  { name: 'ogrid-20k', route: 'ogrid', a: 8, b: 6, c: 80, dt: 0.25 },
  { name: 'ogrid-50k', route: 'ogrid', a: 10, b: 8, c: 120, dt: 0.16667 },
  { name: 'cfmesh-5k', route: 'cfmesh', a: 0.004, b: 0, c: 48, dt: 0.214 },
  { name: 'cfmesh-20k', route: 'cfmesh', a: 0.0026, b: 0, c: 48, dt: 0.138, max_steps: 400 },
  { name: 'cfmesh-50k', route: 'cfmesh', a: 0.0018, b: 0, c: 48, dt: 0.102, max_steps: 400 },
  { name: 'ogrid-20k-dt1', route: 'ogrid', a: 8, b: 6, c: 80, dt: 1.0 },
  { name: 'ogrid-20k-dt2.5', route: 'ogrid', a: 8, b: 6, c: 80, dt: 2.5 },
].filter((c) => only.length === 0 || only.includes(c.name));

const files = {
  '/': ['text/html', '<!doctype html><meta charset="utf-8"><title>pipe</title>'],
  '/worker.js': ['text/javascript', readFileSync(join(here, 'worker.js'))],
  '/pipe.wasm': ['application/wasm', readFileSync(wasmPath)],
};
const server = createServer((req, res) => {
  const f = files[req.url];
  if (!f) { res.writeHead(404); res.end(); return; }
  res.writeHead(200, { 'content-type': f[0] });
  res.end(f[1]);
}).listen(0);
const port = server.address().port;

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage();
page.on('console', (m) => console.error('page:', m.text()));
await page.goto(`http://127.0.0.1:${port}/`);
console.error('browser', browser.version());
for (const c of cases) {
  const out = await page.evaluate((c) => new Promise((ok, fail) => {
    const w = new Worker('/worker.js');
    w.onmessage = (e) => { w.terminate(); ok(e.data); };
    w.onerror = (e) => { w.terminate(); fail(new Error(e.message)); };
    w.postMessage({ ...c, wasm: '/pipe.wasm', max_steps: c.max_steps ?? 3000 });
  }), c);
  console.log(JSON.stringify(out));
}
await browser.close();
server.close();
