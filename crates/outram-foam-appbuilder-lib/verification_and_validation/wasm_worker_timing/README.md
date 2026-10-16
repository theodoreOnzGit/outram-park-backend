# The Sod `RhoCentralFoam` solve in a Web Worker: wasm vs native wall clock

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**Generated:** 2026-10-16 (UTC)
**Commit:** `bf995943b` (branch `claude/outram-foam-8ookor`)
**Issue:** gh:#893, under the CFD web-demo epic gh:#891

This is a **cost measurement**, not a physics V&V case. The physics of this
solve is validated against Sod (1978) Table II and the exact Riemann solution
in `../../tests/sod_shock_tube_validation/`. The question here is narrower:
**does a foam solver actually run as `wasm32-unknown-unknown` in a worker, and
how much slower is it than native?** `scripts/check-wasm.sh` cannot answer that,
because it only compiles libraries.

## Methodology

**What runs.** `src/lib.rs` builds the committed shock-tube case with no
filesystem access:

- The mesh comes from the case's `system/blockMeshDict` text (embedded with
  `include_str!`), through `outram_foam_mesh::block_mesh` and `to_fv_mesh`.
  That gives 100 cells on x ∈ [−5, 5] m.
- The initial fields are set in memory the way `setFieldsDict` does: x < 0 is
  p = 1e5 Pa, T = 348.432 K; x ≥ 0 is p = 1e4 Pa, T = 278.746 K; U = 0.
- `RhoCentralFoam::run` integrates to Sod's τ = 0.2 (t = 6.3246e-3 s) at a
  fixed Δt.

The same Rust function is called two ways:

- **Native:** `src/bin/native.rs`, x86_64 release build.
- **Wasm:** the `cdylib` built for `wasm32-unknown-unknown` in release, loaded by
  `run_worker.mjs` inside a Node `worker_threads` Worker. Node 22 runs it on V8,
  the engine Chrome uses for Web Workers.

Both builds use the workspace release profile, with overflow checks on.

**Instrument.** Best of 5 wall-clock runs per Δt (`Instant` natively,
`performance.now()` in the worker). Best-of-N is used because the question is
what the code costs, not what the shared CI host was doing.

**Checksum.** Each run returns the cell sum of ρ, so the two targets can be
compared. On uniform cells this is the total mass. No wave reaches the
zero-gradient tube ends by τ = 0.2: the shock is at +3.5 m and the rarefaction
head is at −2.4 m. Mass must therefore be conserved to round-off. The crate's
one test, `rho_sum_is_conserved_to_round_off`, asserts this to 1e-12 relative.

**Host.** Intel Xeon @ 2.10 GHz, 4 logical CPUs, in a shared cloud container.

## Results

The wasm module has **zero imports** (`WebAssembly.Module.imports` returns
`[]`). The solver path therefore makes no host calls at all: no clock, no
threads, no filesystem. Calls of that kind compile on `wasm32-unknown-unknown`
but panic at run time, so their absence is what needed showing.

```csv
dt_s,steps,native_best_s,wasm_worker_best_s,wasm_over_native,native_rho_sum,wasm_rho_sum
8e-6,791,0.0302,0.0391,1.29,5.62300922778907974e1,5.62300922778907903e1
4e-6,1581,0.0605,0.0767,1.27,5.62300922778907974e1,5.62300922778907974e1
2e-6,3162,0.0979,0.1556,1.59,5.62300922778908472e1,5.62300922778908330e1
1e-6,6325,0.2405,0.2609,1.08,5.62300922778908543e1,5.62300922778908046e1
```

**Interpretation.**

- **It runs, in a worker, at 1.1–1.6× native.** The whole validated Sod case
  (Δt = 1e-6 s, 6325 steps) takes **0.26 s** in the worker, or about 41 µs per
  step on 100 cells (~410 ns per cell-step).
  - A demo rung can therefore stream every step, or every few steps, to the UI
    thread well inside one 16 ms frame.
  - The `sod` rung of the `cfd_web` ladder can run live.
- **The answers agree to round-off, not bit for bit.** The largest native–wasm
  difference in the ρ sum is 5e-14 absolute (9e-16 relative), a few ulps on a
  100-term sum. This is consistent with different `libm` implementations of
  `pow`/`sqrt` in the two targets, but it was **not** traced to a specific call.
  A demo must not promise bitwise parity with the native V&V numbers.
- **The ratio is not a constant.** 1.08–1.59× across four runs on a shared host
  is noise on top of a real overhead. Quote it as "about 1.3×, within 1.1–1.6×".

**What this does not establish.**

- **Not a browser.** It is V8 under Node, not Chrome, Firefox or Safari,
  and not a phone. Safari runs on JavaScriptCore and is unmeasured.
- **Solvers with an implicit pressure solve are unmeasured.** That covers
  pimpleFoam, the cavity, and the GeN-Foam eigenvalue solve. Sod is fully
  explicit (KNP fluxes), so its cost structure is the cheapest of the set.
- **The `parallel` (rayon) feature was not enabled**, and is off by default in
  `outram-foam-basic-lib`. Whether rayon's fallback works on
  `wasm32-unknown-unknown` is unchecked.

## Reproduce

```bash
cd crates/outram-foam-appbuilder-lib/verification_and_validation/wasm_worker_timing
cp ../../../../Cargo.lock .            # pin the workspace's versions
rustup target add wasm32-unknown-unknown
cargo test  --release                  # the mass-conservation check
cargo run   --release --bin native
cargo build --release --lib --target wasm32-unknown-unknown
node run_worker.mjs target/wasm32-unknown-unknown/release/foam_wasm_timing.wasm
rm -rf Cargo.lock target
```

This folder is **not a workspace member**. It declares its own empty
`[workspace]` so the `cdylib` can be built without changing the root manifest.
`cargo publish` skips sub-folders that hold their own `Cargo.toml`.
