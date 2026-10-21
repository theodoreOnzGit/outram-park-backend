// Runs the Sod solve inside a node worker_thread (V8, same engine as a Chrome
// Web Worker) and reports best-of-5 wall time per dt plus the rho checksum.
import { Worker, isMainThread, parentPort, workerData } from 'node:worker_threads';
import { readFileSync } from 'node:fs';
import { performance } from 'node:perf_hooks';
if (isMainThread) {
  const w = new Worker(new URL(import.meta.url), { workerData: process.argv[2] });
  w.on('message', m => console.log(m));
  w.on('error', e => { console.error('worker error:', e); process.exit(1); });
} else {
  const bytes = readFileSync(workerData);
  const imports = WebAssembly.Module.imports(new WebAssembly.Module(bytes));
  parentPort.postMessage('imports: ' + JSON.stringify(imports));
  const { instance } = await WebAssembly.instantiate(bytes, {});
  const ex = instance.exports;
  parentPort.postMessage('n_cells ' + ex.sod_n_cells());
  for (const dt of [8e-6, 4e-6, 2e-6, 1e-6]) {
    let best = Infinity, sum = 0;
    for (let i = 0; i < 5; i++) { const t = performance.now(); sum = ex.sod_rho_sum(dt); best = Math.min(best, performance.now() - t); }
    parentPort.postMessage(`wasm-worker dt=${dt} best_of_5=${(best/1000).toFixed(4)}s rho_sum=${sum.toExponential(17)}`);
  }
}
