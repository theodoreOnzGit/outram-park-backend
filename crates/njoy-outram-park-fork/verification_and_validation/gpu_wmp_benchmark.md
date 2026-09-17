# GPU vs CPU windowed-multipole (WMP) benchmark — methodology template

**This committed file is a methodology template, not a results table.** GPU and
CPU timings differ from one machine to the next, so a single box's numbers are
not committed here. Instead, running the benchmark **generates a fresh,
hardware-labelled report on your own machine** and writes it to the git-ignored
`verification_and_validation/local_perf/` directory. See `src/perf_report.rs`
(the reusable report generator) and `examples/gpu_wmp_bench.rs` (the benchmark
that emits it).

To generate your machine's report:

```bash
cargo run -p njoy-outram-park-fork --release --example gpu_wmp_bench
```

This prints a table + verdict to stdout and writes
`verification_and_validation/local_perf/gpu_wmp_benchmark.md` and `.csv`
(git-ignored) headed by your detected hardware — GPU adapter + backend, CPU
cores, OS (e.g. `NVIDIA GeForce RTX 3050 / Vulkan, 12 cores, linux`). With no GPU
adapter present the example exits cleanly and does nothing (it never panics).

## What is measured

The **full-fidelity windowed-multipole evaluator** — the complex Faddeeva pole
sum over each window's pole range, for all three reaction channels
(scatter / absorption / fission) — across an entire incident-energy grid:

- **CPU reference:** `njoy_outram_park_fork::gpu_wmp::wmp_evaluate_batch_cpu`
  (`f64`) — the trusted, deterministic reference.
- **GPU path:** `GpuContext::wmp_evaluate_batch` (`f32`, WGSL compute) —
  acceleration only; it does not bit-match the CPU.

Nuclide: **U-238** from the embedded CORE WMP library (602 poles), at **300 K**.
The kernel is **compute-bound** (many floating-point Faddeeva pole terms per
energy, little memory traffic), so a GPU is expected to win once the grid is
large enough to amortise dispatch/transfer overhead — the sweep locates that
crossover.

## Methodology

- Energy grid: log-spaced over `[e_min, e_max]` of the U-238 multipole range.
- Grid sizes swept: 1e3, 1e4, 1e5, 1e6 energies.
- Timing: best-of-3 wall-clock per size, warm-up run excluded (the first GPU
  dispatch pays a one-off pipeline-creation cost that should not be charged to
  the measured runs).
- Agreement metric: max relative error of the total cross section
  (`sigma_s + sigma_a`), GPU (`f32`) vs CPU (`f64`), denominator floored to avoid
  division blow-up on near-zero totals.
- Units: energy in eV, cross sections in barn, temperature in K.

## Reference / interpretation

- Trust model: the `f64` CPU path is the verification-and-validation reference.
  The GPU `f32` number quantifies the accuracy cost of the speedup, not a second
  source of truth. Single-precision agreement on U-238 at 300 K is typically a
  few times `1e-3` on a coarse grid, degrading toward `~2e-2` on a `1e6`-point
  grid because denser grids sample more sharp-resonance points where the pole sum
  cancels harder in `f32`.
- The lead's characterization run (which hardware, measured speedup curve and
  σ agreement on real GPU silicon) is recorded in
  `docs/ai-fleet-review/njoy-faddeeva-gpu/REVIEW_MANIFEST.md` as a durable review
  record — that manifest is the place to read one concrete set of numbers; this
  template is deliberately hardware-agnostic.
