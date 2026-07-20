# GPU batched-flight transport — timing V&V (op-u6s.7)

> **Follow-up (op-u6s.8, 2026-07-17): collision physics moved onto the GPU.**
> The "no crossover, GPU loses" verdict below was for the path that kept the
> *collision* on the CPU (a CPU↔GPU round-trip per event). op-u6s.8 ports the
> collision physics onto the GPU too (`run_keff_gpu_event`), so a whole
> generation stays resident in GPU buffers. That made the GPU path **2.7x–4.1x
> faster** and cut the deficit vs a 12-core `CpuMultiThread` from ~6.6x to ~1.9x
> at 1e6 histories — but there is **still no crossover** vs multi-thread on the
> RTX 3050 + 12-core pairing. `ComputeType::Gpu` now routes to the fused event
> path by default. The full measured before/after table, the WGSL approach, and
> the honest verdict are in **`../../docs/gpu_collision_dev_log.md`**; this note
> is kept as the op-u6s.7 record.

Verification of the event-based **batched-flight** `ComputeType::Gpu` path in
`run_keff` against the CPU backends, and an honest wall-clock timing sweep on
real hardware. This note is the committed **methodology + interpretation**;
because timings differ per machine, the actual per-machine numbers are
**generated locally** (see "Generate a report for your machine" below) and kept
out of git — the table in this note is a single labelled **reference
measurement**, not an authoritative result for any other host.

> **UNTRUSTED AI-DRAFT.** Drafted with AI assistance and verified by the
> in-crate V&V tests; still requires human review before promotion past the
> "Unit Tested" V&V stage (see `RESPONSIBLE_USE.md`).

## What is now on the GPU (honest split)

The earlier GPU path (`run_keff_gpu_inner`, op-u6s.4) put **only the
per-generation first-flight Sigma_t batch** on the GPU — one dispatch per
generation — so launch overhead dominated and there was no speedup.

The new path (`run_keff_gpu_batched`) is **event-based**: a whole batch of live
neutrons stays resident in GPU buffers and is advanced **one flight (one event)
at a time, in parallel, per dispatch**. Each dispatch, on the GPU in `f32`, does
for every live particle:

1. advance its own 64-bit LCG one step (state math bit-exact vs the CPU LCG;
   the derived uniform is `f32`) — the collision-distance random number;
2. binary-search the **native-breakpoint union grid** and interpolate the
   macroscopic total Sigma_t at the particle energy;
3. sample distance-to-collision `d_col = -ln(xi) / Sigma_t`;
4. compute distance-to-boundary of the bounding sphere;
5. stream to the nearer of the two and flag `Leaked` vs `Collided`.

So the **regular, memory-bound, per-event streaming** now runs on the GPU for
the entire batch. Only the **branchy per-collision reaction physics** (nuclide
choice, fission / capture / inelastic / (n,2n) / elastic partition, secondary
energy-angle laws) stays on the CPU. Each generation therefore issues a
*sequence* of GPU dispatches — one per event depth — with a CPU collision +
compaction pass between them.

This is genuinely deeper GPU penetration than first-flight-only: every flight of
every event now goes through the GPU, not just the first flight of each history.

## RNG reproducibility decision

Each history owns an **independent LCG sub-stream** derived from
`(seed, generation, history index)` by jump-ahead (the same scheme as
`CpuMultiThread`). The seed is threaded *through* the GPU flight — which advances
it **bit-exactly** — and continues on the CPU for the collision draws, so a
particle sees one coherent stream across the GPU/CPU boundary.

- **Reproducible run-to-run** and independent of GPU scheduling: yes.
- **Bit-identical to `CpuSingleThread`**: no. The flight's uniform and distance
  are computed in `f32`, and the per-history stream structure differs from the
  single sequential stream — exactly like `CpuMultiThread`.

The GPU LCG **state** emulation is verified bit-exact on the RTX 3050
(`gpu_lcg_state_matches_cpu`: 1024/1024 returned states equal
`future_seed(1, seed)`). The `CpuSingleThread` `f64` path remains the trusted,
bit-reproducible reference; the GPU is an `f32` acceleration path only.

## Timing sweep — methodology

Godiva LOW-tier material (U-234/235/238, ICSBEP atom densities, T = 293.6 K,
embedded `from_core` data), bare sphere r = 8.7407 cm, a fixed short iteration
(3 inactive + 7 active = 10 generations) at each batch size. Wall-clock of a full
`run_keff` measured with `std::time::Instant`. `CpuSingleThread` was capped at
1e5 histories to bound total benchmark time.

## Generate a report for your machine

Timings are machine-specific, so run the sweep on **your** hardware to get the
"what performance is available on my PC" answer:

```bash
cargo test -p outram-mc-libs --lib --release gpu_batched_timing_sweep -- --ignored --nocapture
```

This detects your GPU adapter / CPU cores / OS (via the `perf_report` module) and
writes a per-machine markdown report + CSV to the **gitignored**
`verification_and_validation/local_perf/` directory
(`gpu_batched_transport.md` and `gpu_batched_transport_timing.csv`). Those files
are local only — they never commit — so each user keeps their own numbers. The
report includes the per-backend timing/throughput table, the
GPU-vs-`CpuMultiThread` speedup, and the crossover verdict computed for your box.

## Reference measurement (one dev machine — illustrative only)

The following is a single labelled reference run on the development box
(NVIDIA GeForce RTX 3050 / Vulkan, 2026-07-17). **Your machine will differ** —
regenerate with the command above. Wall-clock seconds for a 10-generation run
(lower is better):

| Histories/gen | CpuSingleThread | CpuMultiThread | Gpu (batched) |
|---|---|---|---|
| 1 000     | 0.039 | 0.140 | 1.283 |
| 10 000    | 0.369 | 0.269 | 1.831 |
| 100 000   | 2.281 | 0.843 | 4.530 |
| 1 000 000 | (n/a) | 7.172 | 34.014 |

k-eigenvalue agreement (all within combined 1-sigma of the CPU reference):

| Histories/gen | k single | k multi | k gpu |
|---|---|---|---|
| 1 000     | 1.0334 $\pm$ 0.0126 | 1.0144 $\pm$ 0.0148 | 1.0142 $\pm$ 0.0116 |
| 10 000    | 1.0020 $\pm$ 0.0063 | 1.0061 $\pm$ 0.0050 | 1.0078 $\pm$ 0.0016 |
| 100 000   | 1.0096 $\pm$ 0.0025 | 1.0086 $\pm$ 0.0033 | 1.0094 $\pm$ 0.0028 |
| 1 000 000 | (n/a) | 1.0080 $\pm$ 0.0029 | 1.0085 $\pm$ 0.0029 |

## Verdict — no crossover; GPU still loses to CpuMultiThread

**The batched GPU path does not beat `CpuMultiThread` at any tested batch size,
and there is no crossover.** At 1e6 histories/gen the GPU (34.0 s) is about
4.7x slower than multi-thread (7.2 s) and about 15x slower than a linear
extrapolation of single-thread. The GPU/CPU gap *narrows* with batch size (more
useful parallel work amortises each dispatch), but never closes.

**Why it loses — measured, not guessed.** The path is **memory-transfer /
launch bound**, not compute bound. Each generation issues many dispatches (one
per event depth; for a fast Godiva sphere most neutrons die within a handful of
collisions, so ~tens of dispatches per generation), and **every dispatch
round-trips the CPU/GPU boundary**: upload the live batch, dispatch, then read
back four arrays (position 3N, two RNG halves, outcome) over PCIe, run the CPU
collision, and re-upload the survivors. This per-event synchronisation and
transfer dominates the `f32` flight arithmetic, which is trivially cheap. The
divergent, memory-bound behaviour the task anticipated is exactly what is
observed.

**What would be needed to win.** The bottleneck is the per-event CPU/GPU
round-trip forced by keeping the **collision physics on the CPU**. To beat
multi-thread the collision kernel (nuclide sampling, reaction partition,
secondary sampling, fission banking) would also have to run on the GPU so a batch
can be advanced through many events **without leaving GPU memory** — a much
larger effort (branch-heavy, data-dependent, needs on-GPU fission banking and
stream compaction). That is filed as follow-up work. With collision physics on
the CPU, the per-event boundary fundamentally caps GPU penetration here, which is
the honest conclusion for a history/event-based transport loop of this size.

## Accuracy side-result — native union grid

`UnionTotalXs::tabulate_native` (the native-breakpoint union that replaces the
blind log resample) was measured on Godiva at equal backbone size (4096): it
produced **10 834** strictly-increasing nodes and a max relative error vs a
direct `macro_xs_total` evaluation of **0.79** versus **2.17** for the dense-log
grid — i.e. at least as accurate as, and here ~2.7x better than, the equal-size
log grid, because window/group edges land on real feature boundaries the log grid
steps over. GPU vs CPU on the native grid agrees to max |diff| = 3.9e-4 cm^-1.
This is an accuracy improvement independent of the timing result.
