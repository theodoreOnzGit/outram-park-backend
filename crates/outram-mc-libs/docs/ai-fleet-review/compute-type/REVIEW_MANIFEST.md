# AI Fleet Review Manifest — outram-mc-libs `ComputeType` backend selector

> **⚠️ UNTRUSTED AI-DRAFTED CODE — NOT YET HUMAN-REVIEWED.**
> **This work was produced by an AI agent fleet (Claude Opus lead + one Opus
> subagent). Per `RESPONSIBLE_USE.md` / `AI_USAGE.md`, it is untrusted draft
> material at the "Unit Tested" V&V stage until a human maintainer reviews the
> code, the license/provenance, and the V&V results. Do not describe it as
> validated or trusted, and do not use it for anything safety-, licensing-, or
> operations-relevant.**

- **Bead:** `op-u6s.4` (child of `op-u6s` — outram-mc-libs Monte Carlo transport epic).
- **Date:** 2026-07-17.
- **Scope touched:** `crates/outram-mc-libs/` only. Root `Cargo.toml` NOT edited
  (rayon/log were already in `[workspace.dependencies]`); no other crate touched.
- **New third-party deps in this crate:** `rayon` and `log` (both pure-Rust,
  Android-safe, versions inherited from the root `[workspace.dependencies]` —
  no version added to the root table). Declared unconditionally (no target gate).

## The `ComputeType` interface

A single enum, set on the transport driver's settings object, selects the
transport backend. Enum dispatch — no trait objects (per `CLAUDE.md`).

```rust
// crates/outram-mc-libs/src/physics/compute.rs
pub enum ComputeType {
    CpuSingleThread,             // scalar, single-thread — TRUSTED deterministic reference (default)
    CpuMultiThread(ThreadCount), // rayon-parallel over histories, dedicated pool sized to ThreadCount
    Gpu,                         // GPU-accelerated XS lookup; graceful CPU fallback, never errors
}

pub enum ThreadCount {          // how many workers CpuMultiThread uses
    Auto,          // std::thread::available_parallelism() — scales with CPU strength (default)
    Fixed(usize),  // exact worker count (clamped >= 1)
    Fraction(f64), // fraction of logical cores, e.g. 0.5 = half (rounded, clamped >= 1)
}
impl ThreadCount { pub fn resolve(self) -> usize; } // always >= 1
```

**Name mapping to the maintainer's spec.** The maintainer named the modes
`CPUSingleThread` / `CPUMultiThread` / `GPU`. This enum uses idiomatic Rust
casing (`Cpu`/`Gpu`) so the crate stays clean under clippy's
`upper_case_acronyms` lint (verified: **zero** clippy warnings from the new
code). Semantics are identical; the mapping is documented on the enum.

**Wiring.**
- `KeffSettings` gained a `pub compute: ComputeType` field (default
  `CpuSingleThread`) plus a `with_compute(...)` builder.
- `run_keff(...)` is now a thin dispatcher that matches on `settings.compute`
  and forwards to a separate function per mode.
- Re-exported from the crate prelude: `ComputeType`, `ThreadCount`.
- Compiles on **all** targets including `aarch64-linux-android` (the enum and
  dispatcher are target-independent; only the GPU *inner* path is
  `#[cfg(not(target_os = "android"))]`).

## Per-mode status — real vs partial

| Mode | Entry point | Status |
|---|---|---|
| `CpuSingleThread` | `run_keff_cpu_single` | **REAL / complete.** The pre-existing scalar `run_keff` body, moved verbatim. Raw `f64`, single sequential RNG stream — the trusted, bit-reproducible reference. Unchanged physics. |
| `CpuMultiThread(tc)` | `run_keff_cpu_multi` | **REAL / complete.** Rayon-parallel over the per-generation history bank in a **dedicated pool** sized by `tc.resolve()`. Per-history RNG streams derived by LCG jump-ahead from `(seed, gen, hist)` → **reproducible independent of thread count** (verified bit-for-bit). Measured ~7.4× speedup (below). |
| `Gpu` | `run_keff_gpu` → `run_keff_gpu_inner` | **REAL but PARTIAL GPU penetration (honest).** Genuinely uses the `wgpu` `UnionTotalXs::lookup_gpu` kernel inside the sweep, but only for the batched first-flight Sigma_t (see next section). Graceful CPU fallback. No end-to-end GPU transport (by design — the history walk is branchy). |

No `todo!()`, no fake-green, no hard-coded "expected" k values — every mode is
judged against the CPU reference computed at test time.

## How far the GPU reaches into the transport loop (honest)

The `src/gpu/mod.rs` contract already states the history-based transport loop is
"branchy, not GPU friendly" and does **not** belong on the GPU. So `Gpu` does
**not** run an end-to-end GPU sweep. What it genuinely does:

1. **Build once** — a dense 16 384-point log-spaced table of the material's
   macroscopic total Sigma_t over `[1e-3, 2e7]` eV
   (`UnionTotalXs::tabulate`). Temperature is fixed for the run.
2. **GPU batch per generation (the real GPU dispatch)** — at the start of each
   generation, the birth energies of **all** source sites are looked up in **one**
   `UnionTotalXs::lookup_gpu` dispatch (`f32`, on the RTX 3050). Each history
   **consumes** its GPU `f32` value as its **first-flight** total cross section.
3. **CPU table lookups thereafter** — every subsequent per-collision Sigma_t (and
   the first flight of any `(n,2n)` secondary sub-walk) is served from the *same*
   table by CPU linear interpolation (`lookup_cpu`). Single-energy GPU dispatches
   per collision would be dominated by kernel-launch latency.

The GPU transport path (`transport_history_tabulated`) is a **verbatim mirror**
of the CPU `transport_history` with the RNG draws in the same order — only the
Sigma_t *value* differs (table/GPU vs a direct `macro_xs_total` call). That is
what keeps `k_gpu` correlated with the single-thread reference.

**Honest limit:** GPU penetration = the per-generation first-flight Sigma_t
batch. The branchy per-collision random walk stays on CPU. This is a genuine but
**partial** wire-in of the GPU XS kernel into the eigenvalue loop, not an
end-to-end GPU speedup.

## CpuMultiThread thread-pool sizing

- A **dedicated** `rayon::ThreadPool` is built per run (not the implicit global
  pool) with `num_threads(thread_count.resolve())`; the whole generation loop's
  `into_par_iter()` transport runs inside `pool.install(...)`.
- `ThreadCount::Auto` (default) = `std::thread::available_parallelism()` → scales
  with the CPU: this machine resolved **12** logical cores → 12 workers; an
  Android phone would resolve to its (few) cores with no special-casing.
- `Fixed(n)` pins an exact count; `Fraction(f)` takes a fraction of the logical
  cores. Both clamp to `>= 1`.
- Reproducibility is **independent of thread count**: each history's RNG stream is
  derived only from `(seed, generation index, history index)` via LCG jump-ahead
  (mirrors OpenMC's per-particle independent streams, `src/random_lcg.cpp`
  `init_seed`/`future_seed`), and the per-history fission-bank concatenation is
  reduced in history-index order. Verified bit-for-bit (test below).

## Build / test / android output (measured 2026-07-17)

```
# release lib test suite (desktop)
cargo test -p outram-mc-libs --lib --release
  test result: ok. 80 passed; 0 failed; 0 ignored   (78 baseline + 2 new:
    physics::keff::tests::three_compute_modes_agree_on_godiva   ok
    physics::keff::tests::cpu_multi_is_reproducible             ok)

# Android cross-check (CpuMultiThread works; Gpu inner path cfg'd out → CPU fallback)
cargo check -p outram-mc-libs --target aarch64-linux-android
  Finished — clean. rayon + available_parallelism compile on Android; the
  wgpu-backed run_keff_gpu_inner / transport_history_tabulated are
  #[cfg(not(target_os="android"))], so run_keff_gpu reduces to
  log::debug! + run_keff_cpu_single there.

# clippy — zero warnings from the new code (compute.rs, keff.rs additions);
# no upper_case_acronyms. (Pre-existing warnings elsewhere are in the njoy dep.)
```

No regression: the 78 pre-existing lib tests still pass; the 2 new tests are additive.

## 3-mode k agreement (actually executed on the RTX 3050, not skipped)

`three_compute_modes_agree_on_godiva` — Godiva LOW-tier (U234/U235/U238
`from_core`), r = 8.7407 cm, 800 histories × [15 inactive + 40 active], seed 1.
Pass: pairwise `|Δk| ≤ 5·sqrt(σ_a²+σ_b²)`. GPU arm skips gracefully if no adapter.

- **Adapter:** NVIDIA GeForce RTX 3050 (real — GPU arm ran, not skipped).
- k_single = **1.00762 ± 0.00827** (trusted reference)
- k_multi  = **1.00715 ± 0.00768**
- k_gpu    = **1.00066 ± 0.00839**
- single-vs-multi: |Δk| = 0.00047 = **0.04σ** (band 0.05643)
- single-vs-gpu:   |Δk| = 0.00696 = **0.59σ** (band 0.05890)

All three land within ~1000 pcm of unity and of each other. single-vs-gpu is
slightly larger than single-vs-multi despite the shared RNG seed: the dense-table
resampling smears the resonant Sigma_t, and once a slightly different Sigma_t
flips one leak-vs-collide decision the history decorrelates — a ~700 pcm
systematic shift, well inside the band, consistent with the acceleration-only
table approximation.

`cpu_multi_is_reproducible` — 600 × [10+20], `Fixed(1)` twice + `Fixed(4)`:
all three `k_mean` bit-for-bit identical (`f64::to_bits`), zero mismatches →
thread-count-independent determinism confirmed.

## Measured wall-time (honest — no GPU speedup claimed)

Throwaway timing harness (NOT committed), Godiva LOW-tier, **20 000 histories ×
[20 + 60] generations**, warm second run, this machine (12 logical cores + RTX 3050):

| Mode | Wall time | vs single | k |
|---|---|---|---|
| `CpuSingleThread` | 2.437 s | 1.0× (ref) | 1.01183 ± 0.00137 |
| `CpuMultiThread(Auto=12)` | **0.330 s** | **~7.4× faster** | 1.01049 ± 0.00136 |
| `Gpu` | 2.657 s | ~0.92× (slightly slower) | 1.01101 ± 0.00115 |

**Interpretation (honest).** `CpuMultiThread` delivers a real ~7.4× speedup on 12
cores. **`Gpu` shows NO speedup on this case** — it is marginally slower than
single-thread and ~8× slower than multi-thread. This is expected and was
predicted: the GPU does one small `lookup_gpu` dispatch per generation
(kernel-launch-latency bound at these batch sizes) while the branchy per-collision
walk stays on CPU. The GPU wire-in is a *fidelity/plumbing* deliverable (a genuine
GPU kernel executing inside the eigenvalue loop), **not** a performance win for
this homogeneous 3-nuclide bare-sphere problem. No GPU speedup is claimed.

## Human-verify list (before promoting past "Unit Tested")

1. **RNG-stream non-overlap (multi-thread).** Confirm the strides
   `HIST_STRIDE = 152917` (OpenMC per-particle stride) and `GEN_STRIDE = 2^40`
   guarantee per-history sub-streams never overlap for the `n_particles` you run
   (argument: a history draws ≪ 152917; a generation's max offset
   `n_particles·HIST_STRIDE < 2^40` for `n_particles < ~7.5e6`). Check the edge of
   very large `n_particles`.
2. **GPU determinism / correctness.** `run_keff_gpu` is `f32` acceleration only;
   confirm the CPU single-thread path remains the sole V&V reference and that the
   ~700 pcm single-vs-gpu shift is understood as table-resampling + f32, not a
   physics error. Consider whether the 16 384-point table density is appropriate.
3. **Fallback path.** Verify on a headless / no-adapter machine that
   `ComputeType::Gpu` logs the debug line and returns the CPU single-thread result
   (never errors). The message text is exactly:
   `ComputeType::Gpu requested but no GPU adapter available — falling back to CPU`.
4. **`transport_history_tabulated` vs `transport_history`.** Diff the two — the
   only intended difference is the Sigma_t source (table/GPU first-flight vs
   `macro_xs_total`); the RNG draw order must match line-for-line. A drift here
   would silently decorrelate the GPU path from the reference.
5. **Scope of the selector.** `compute` currently only dispatches inside
   `run_keff` (the bare-sphere homogeneous driver). The CSG (`run_keff_csg`), MG
   (`run_keff_mg`), and delta (`run_keff_delta`) drivers do **not** yet honour it
   — see the follow-up bead. Confirm that's the intended staging.
6. **Remaining op-u6s.4 work.** The native-breakpoint union grid (vs the dense
   log resample) is still open, and GPU penetration is first-flight-only — both
   tracked as follow-ups (see the bead).

## Files added/changed

- `crates/outram-mc-libs/Cargo.toml` — add `rayon`, `log` (unconditional, workspace-inherited).
- `crates/outram-mc-libs/src/physics/compute.rs` — **new**: `ComputeType` + `ThreadCount` + `resolve()` (lead).
- `crates/outram-mc-libs/src/physics/mod.rs` — `pub mod compute;`.
- `crates/outram-mc-libs/src/physics/keff.rs` — `compute` field + `with_compute` + dispatcher + `run_keff_cpu_single` / `run_keff_cpu_multi` / `run_keff_gpu` / `run_keff_gpu_inner` / `transport_history_tabulated` + 2 new tests (subagent).
- `crates/outram-mc-libs/src/prelude.rs` — re-export `ComputeType`, `ThreadCount`.
- `crates/outram-mc-libs/docs/ai-fleet-review/compute-type/REVIEW_MANIFEST.md` — this file.
