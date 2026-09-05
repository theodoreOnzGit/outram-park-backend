// Cargo test runner for the wasm32-wasip1 target, using Node's built-in WASI.
//
// Bead op-okqo.7. Wired up in .cargo/config.toml as
//     [target.wasm32-wasip1] runner = "node ... scripts/wasi-run.mjs"
// so that `cargo test --target wasm32-wasip1 -p <crate>` just works.
//
// WHY WASI AND NOT wasm32-unknown-unknown
//
// The workspace's compile gate (scripts/check-wasm.sh) targets
// wasm32-unknown-unknown, because that is what a browser runs. But that target
// cannot RUN a `cargo test` binary: there is no stdout, no process exit code,
// and no test harness entry point without wasm-bindgen plus a browser or Node
// shim.
//
// wasm32-wasip1 shares the same `target_arch = "wasm32"` — so every cfg gate in
// this workspace applies identically — while providing the tiny POSIX surface a
// test harness needs. That makes it the right place to answer the question the
// compile gate cannot: *does the wasm code path actually run, and does it fall
// back to single-threaded CPU rather than panicking?*
//
// WHAT THIS DOES NOT PROVE
//
// WASI is not a browser. It HAS a clock and a filesystem, so `Instant::now` and
// `std::fs` work here and would not in a browser. What it shares with the
// browser, and what these tests are really for, is the absence of threads:
// `std::thread::spawn` fails on both. A test passing here proves the
// threading fallback works; it does not prove browser-readiness.
//
// Requires Node >= 20 (uses `node:wasi`). Node 26 is what this was written on.

import { WASI } from 'node:wasi';
import { readFile } from 'node:fs/promises';
import { argv, exit, env } from 'node:process';

const wasmPath = argv[2];
if (!wasmPath) {
  console.error('wasi-run: no .wasm file given');
  exit(2);
}

// Everything after the module path is passed through to the test harness, so
// `cargo test --target wasm32-wasip1 -- --nocapture some_filter` behaves as
// usual.
const forwarded = argv.slice(3);

const wasi = new WASI({
  version: 'preview1',
  // argv[0] is conventionally the program name; the harness parses the rest.
  args: [wasmPath, ...forwarded],
  env,
  // Give the guest the workspace read-only. Some tests read committed
  // reference data (e.g. the Edwards benchmark CSV); without a preopen they
  // would fail for a reason unrelated to what is being tested.
  preopens: { '/': process.cwd() },
  returnOnExit: true,
});

try {
  const bytes = await readFile(wasmPath);
  const wasm = await WebAssembly.compile(bytes);
  const instance = await WebAssembly.instantiate(wasm, wasi.getImportObject());
  const code = wasi.start(instance);
  exit(code ?? 0);
} catch (err) {
  console.error(`wasi-run: ${err?.message ?? err}`);
  exit(1);
}
