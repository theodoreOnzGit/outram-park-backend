# REVIEW MANIFEST — njoy full-fidelity WMP Faddeeva pole-sum on GPU (WGSL)

> **⚠️ UNTRUSTED AI-GENERATED DRAFT — NOT YET HUMAN-REVIEWED ⚠️**
>
> **This change (the `gpu_wmp` full-fidelity WMP GPU kernel, the `perf_report`
> per-machine report generator, and their wiring) was authored by an AI fleet
> (Claude Opus 4.8) and is untrusted draft material until a human reviews it,**
> per the workspace `RESPONSIBLE_USE.md` / `AI_USAGE.md` policy. It compiles, its
> tests pass, and the GPU path ran on real hardware — but that proves types + the
> measured cases, not that the design is right for the crate. Do **not** describe
> the GPU path as validated or trusted until the human-verify checklist below is
> cleared. The `f32` GPU output must never feed a trusted/validated path; V&V
> stays on the `f64` CPU reference.

Date: 2026-07-17 (Asia/Singapore). Bead: **op-0m5** (follow-up to op-wra).
Base: `origin/develop` @ `71b63e5`. Scope touched: **only**
`crates/njoy-outram-park-fork/`.

---

## What is on the GPU

The **complete windowed-multipole (WMP) cross-section evaluation**, per incident
energy, across a whole energy grid — not just the curve-fit background (that was
op-wra / `src/gpu.rs`). For each energy the WGSL compute shader:

1. locates the `√E` window (clamped),
2. adds the window's curve-fit polynomial **background** — Doppler-broadened via
   the `broaden_wmp_polynomials` recurrence (with a real `erf`) when the window
   flags it, else the raw `a/E + b/√E + …` polynomial,
3. sums the complex **Faddeeva pole contribution** of every pole assigned to the
   window — the 0 K asymptotic form `−i/(pole−√E)` when `T = 0`, otherwise the
   temperature-dependent `w(z)` form — for all three channels (scatter /
   absorption / fission).

The Faddeeva function `w(z)` is a **48-term Weideman rational approximation**
ported line-for-line from the CPU `src/wmp.rs` (`w_standard` / `faddeeva` /
`erf` / `broaden_wmp_polynomials` / the pole loops of `WindowedMultipole::
evaluate`). WGSL has no native complex type, so complex arithmetic is done
manually in `vec2<f32>` (`x = re`, `y = im`) with `cadd/csub/cmul/cdiv/…`
helpers mirroring `Cf64`. The host uploads the identical Weideman coefficient
table, scaling `L`, and constants (`K_BOLTZMANN`, `SQRT_PI`) the CPU uses, so the
only difference from the CPU is single vs double precision.

This is genuinely the hot part of WMP: **U-238 carries 602 poles**, and each
energy sums a window's poles, each pole needing a `w(z)` — a compute-heavy,
per-energy-independent workload, i.e. a real GPU target (unlike memory-bound MC
transport).

### Files changed (all inside the njoy crate)

| File | Change |
|---|---|
| `src/gpu_wmp.rs` | **NEW** — full-fidelity WMP GPU kernel: `WmpXsGpu`, CPU reference `wmp_evaluate_batch_cpu` (f64), `impl gpu::GpuContext { wmp_evaluate_batch }`, the WGSL shader, and the agreement test. |
| `src/perf_report.rs` | **NEW** — reusable per-machine performance-report generator (`HardwareInfo::detect`, `PerfRow`, `format_perf_report`, `write_local_perf_report`). Pure `std`, builds on all targets. |
| `src/gpu.rs` | `GpuContext` fields `device`/`queue` → `pub(crate)`; new `pub(crate) info: AdapterInfo` captured in `probe()`; new `adapter_label()`; byte-pack helpers → `pub(crate)`; scaffold-honesty doc + TODO now point at `gpu_wmp`. |
| `src/wmp.rs` | Exposed `pub(crate)` `weideman_coeffs`, `weideman_l`, `WEIDEMAN_N`, `K_BOLTZMANN`, `SQRT_PI` so the GPU port uploads the identical tables/constants. No logic change. |
| `src/lib.rs` | Declared `pub mod gpu_wmp;` (cfg-gated off Android) and `pub mod perf_report;` (all targets). |
| `examples/gpu_wmp_bench.rs` | **NEW** — GPU-vs-CPU sweep (U-238, 300 K), emits a per-machine report into git-ignored `verification_and_validation/local_perf/`. |
| `verification_and_validation/gpu_wmp_benchmark.md` | **NEW** — methodology **template** (no machine-specific timings). |
| `.gitignore` | Added `/verification_and_validation/local_perf/` (per-machine reports never commit). |
| `docs/ai-fleet-review/njoy-faddeeva-gpu/REVIEW_MANIFEST.md` | This file. |

**No new dependency.** `wgpu`/`log` were already wired (op-wra); root `Cargo.toml`
untouched. No `bytemuck`/`pollster` — manual LE byte packing + the existing
`block_on` are reused.

---

## CPU–GPU σ agreement + f32 precision (ran on real silicon, NOT a SKIP)

The dev box has a real **NVIDIA GeForce RTX 3050** (Vulkan, NVIDIA proprietary
driver), so `probe()` returned `Some` and both GPU tests **ran on hardware** (no
llvmpipe, no skip):

- `gpu_wmp::tests::gpu_wmp_agrees_with_cpu_or_skips` — U-238 (602 poles), 2000
  log-spaced energies over `[e_min, e_max]`, 300 K: **max relative error of the
  total (scatter + absorption) = 2.98e-3 (~0.3 %)** at E ≈ 8.77e3 eV, vs the
  `f64` CPU reference. Gate `< 5e-2` (kept loose so it never flakes across
  drivers). Data: embedded CORE U-238 WMP blob (ENDF/B-VII.1).

The single-precision error grows with grid density (the benchmark sweep):

| n_energy | max_rel_err_total (GPU f32 vs CPU f64) |
|---|---|
| 1 000    | 2.7e-3 |
| 10 000   | 4.6e-3 |
| 100 000  | 9.0e-3 |
| 1 000 000| 2.2e-2 |

Denser grids sample more sharp-resonance points where the pole sum cancels
harder in `f32`, so the worst-case error climbs to ~2 % at 1e6 energies. **This
is the accuracy cost of the speedup, not a second source of truth — V&V stays on
the CPU `f64` path.**

---

## The GPU-vs-CPU speedup curve — the headline (compute-bound kernel wins)

Measured on the **NVIDIA RTX 3050 (Vulkan)**, U-238 at 300 K, best-of-3
wall-clock, warm-up excluded (representative run; absolute ms vary run-to-run,
GPU ms especially — two runs gave 69x and 61x at 1e6):

| n_energy | cpu_ms | gpu_ms | speedup |
|---|---|---|---|
| 1 000     | 0.52   | 1.67   | **0.31x** (CPU wins — launch overhead dominates) |
| 10 000    | 5.16   | 1.72   | **3.0x** |
| 100 000   | 51.6   | 2.20   | **23x** |
| 1 000 000 | 515    | 7.5    | **~61–69x** |

**Verdict: yes — this compute-bound kernel genuinely wins on the GPU.** The GPU
overtakes the CPU at **N ≈ 10 000 energies** (the crossover), and reaches
**~60–69x** at 1e6 energies. Below the crossover (N = 1000) the CPU is faster:
the fixed per-dispatch cost (buffer upload, pipeline setup, submit, readback)
outweighs ~1000 energies of arithmetic. This is the honest, expected shape for a
compute-bound kernel with launch overhead — unlike a memory-bound workload, the
arithmetic-per-byte here is high enough that the GPU pays off strongly once the
grid is large.

Per-machine numbers are **not committed**: the benchmark generates a fresh,
hardware-labelled report (`perf_report`) into git-ignored
`verification_and_validation/local_perf/` on whatever machine runs it. The
committed `gpu_wmp_benchmark.md` is a methodology template only.

---

## Build / test / Android (measured 2026-07-17)

```
cargo build  -p njoy-outram-park-fork --lib --release                       # clean
cargo build  -p njoy-outram-park-fork --release --example gpu_wmp_bench      # clean
cargo check  -p njoy-outram-park-fork --lib --target aarch64-linux-android   # clean (NO wgpu, gpu_wmp absent)
crates/njoy-outram-park-fork/scripts/test.sh                                 # full suite, capped, no regression
```

- **Unit tests:** `381 passed; 0 failed` in the lib (was 377 pre-change: 375 +
  the 2 `gpu` background tests; this change adds **+1** `gpu_wmp` agreement test
  and **+3** `perf_report` tests → 381). The two GPU tests run on real hardware;
  the three `perf_report` tests are pure `std`. All integration suites pass; the
  full `scripts/test.sh` run finished with **exit 0, 0 failures**.
- **Android (`aarch64-linux-android`):** clean. `gpu_wmp` is `cfg`-gated off
  Android at the `pub mod` site, `wgpu` is not pulled, and `perf_report` is pure
  `std` (compiles). Android stays lean and pure-CPU.

---

## Design-rule compliance (self-checked, still needs human review)

- No `Box<T>`, no `dyn`/trait objects, no lifetime parameters. `GpuContext` owns
  `device`/`queue`/`info` by value.
- No new dependency; root `Cargo.toml` untouched.
- `///` doc on every public item; `//!` module docs on `gpu_wmp` and
  `perf_report`. The WGSL doc cites the mirrored CPU source (`src/wmp.rs`
  functions). V&V test doc carries **methodology + measured results**.
- File lengths: `src/gpu_wmp.rs` ≈ 690 lines, `src/perf_report.rs` ≈ 230 lines
  (< 1000-line cap).
- GPLv3 provenance untouched (no ported upstream file changed;
  `LICENSE.njoy`/`NOTICE` intact).

---

## Human-verify checklist (before this is trusted)

1. **Faddeeva `f32` accuracy — TOP ASK.** Confirm the 48-term Weideman `w(z)` in
   `f32` is accurate enough for the intended use across the whole `√E`/pole range
   (not just U-238 at 300 K). The worst case measured here is ~2 % at 1e6
   energies; decide whether that bound is acceptable and whether any regime
   (very sharp/overlapping resonances, near-window-edge, `T = 0` asymptotic
   branch) needs a tighter look or a mixed-precision fallback.
2. **WGSL ↔ CPU line-for-line fidelity.** Review the shader against
   `src/wmp.rs` (`w_standard`, `faddeeva`, `erf`, `broaden_wmp_polynomials`, the
   two pole branches) — especially the broadening recurrence (fixed
   `array<f32,16>`; `n_coeff ≤ 16` asserted) and the window-clamp truncation
   (`i32(...)` vs the CPU `as isize`).
3. **Precision policy.** Confirm the `f32` GPU output is never wired into a
   trusted/validated path; V&V stays on CPU `f64`. `WmpXsGpu` is deliberately a
   separate type from `WmpXs`.
4. **`perf_report` privacy/scope.** Confirm the git-ignore of
   `verification_and_validation/local_perf/` is correct and that no machine-
   specific numbers are committed (the template `.md` carries none). Confirm
   `HardwareInfo` (GPU name, core count, OS) is acceptable to write locally.
5. **Buffer/edge cases.** `n_energy == 0` early-returns; zero-pole nuclides pad
   buffers to one dummy element (U-238 has 602, never triggered); very large
   grids single-submit (no chunking) — confirm 1e6 energies (48 MB output) is
   within the target device limits generally, not just the RTX 3050.
6. **Run-to-run timing variance.** GPU ms vary (61x vs 69x at 1e6 seen);
   confirm the benchmark's best-of-3 + warm-up-excluded methodology is the
   intended one and whether a fixed clock / more repeats is wanted for a paper.
