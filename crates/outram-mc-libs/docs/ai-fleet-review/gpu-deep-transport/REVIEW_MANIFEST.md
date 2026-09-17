# Review manifest — deep GPU penetration into the MC transport loop (op-u6s.7)

<!-- op-jis-historical-note -->
> ⚠️ **HISTORICAL RECORD — the statistics below predate `op-jis` (noted 2026-08-06).**
> Every measured number in this manifest was produced **before** bead `op-jis`
> added OpenMC's PCG-RXS-M-XS output permutation to `rng::lcg::prn` on
> 2026-08-06. The LCG **state recurrence was not changed**, so integer-state
> facts still hold, but every statistic derived from the sampled **uniform
> values** — k values and their σ, tallies, fractions, σ-distances — **no longer
> reflects the current generator**. This is a dated review record, so its numbers
> are deliberately **left exactly as they were measured** and are *not* rewritten
> here. Do not cite them as current; current values live in the crate's V&V docs
> and test doc comments.

> # ⚠️ UNTRUSTED AI DRAFT — NOT YET HUMAN-REVIEWED ⚠️
>
> Everything below was produced by an AI fleet (lead + Opus subagents) and is
> **untrusted draft material** per `RESPONSIBLE_USE.md` / the workspace
> `CLAUDE.md`. It compiles, the full lib test suite passes, and the GPU tests run
> on real hardware — but it still needs human inspection, licence-provenance
> review, and V&V sign-off before it is trusted. Do **not** cite the timing
> numbers or the k-agreement as validated results until a human has reviewed
> them.

## Scope

Touched **only** `crates/outram-mc-libs/`. Deepened the `ComputeType::Gpu`
transport path from a first-flight-only Sigma_t batch toward an **event-based
batched-flight** kernel, and replaced the blind dense-log union table with a
**native-breakpoint union grid**.

## Files changed / added

| File | Change | Author |
|---|---|---|
| `src/material/nuclide.rs` | `+` `Nuclide::native_energy_grid(e_min, e_max)` accessor + test | subagent (Opus) |
| `src/gpu/union_grid.rs` | `+` `UnionTotalXs::tabulate_native(...)` + 2 tests; refreshed module doc | subagent (Opus) + lead |
| `src/gpu/batched_flight.rs` | **new** — batched next-event GPU flight kernel host + CPU mirror + 5 tests | subagent (Opus) |
| `src/gpu/shaders/batched_flight.wgsl` | **new** — WGSL flight kernel (emulated 64-bit LCG, union-grid Sigma_t lookup, sphere flight) | subagent (Opus) |
| `src/gpu/mod.rs` | `+` `pub mod batched_flight;` | subagent (Opus) |
| `src/physics/keff.rs` | `+` `run_keff_gpu_batched` + `collide_batched` + `CollisionResult`; routed `ComputeType::Gpu` to it; `+` `gpu_batched_timing_sweep` benchmark | lead |
| `src/prelude.rs` | `+` batched-flight re-exports | lead |
| `verification_and_validation/gpu_batched_transport/{README.md,timing_vs_batch.csv}` | **new** — timing write-up + plottable data | lead |

## How much of the event loop is now on GPU (honest)

**On the GPU, per event, for the whole batch in one dispatch (`f32`):** per-particle
LCG advance (RNG), native-union-grid Sigma_t binary-search + interpolation,
distance-to-collision sample, sphere distance-to-boundary, streaming, leak/collide
flag.

**Still on the CPU (`f64`):** the branchy per-collision reaction physics — nuclide
sampling, fission / capture / inelastic / (n,2n) / elastic partition, secondary
energy-angle laws, fission banking, batch compaction.

So every flight of every event now goes through the GPU (deeper than the previous
first-flight-only path), but the collision kernel remains on the CPU, which forces
a CPU/GPU round-trip **per event**.

## Honest GPU-vs-CpuMultiThread timing result — NO crossover

Godiva LOW-tier, r = 8.7407 cm, 10 generations, RTX 3050, 2026-07-17
(full table + CSV in `verification_and_validation/gpu_batched_transport/`):

| Histories/gen | CpuMultiThread (s) | Gpu batched (s) | GPU / multi |
|---|---|---|---|
| 1 000     | 0.140 | 1.283 | 9.2x slower |
| 10 000    | 0.269 | 1.831 | 6.8x slower |
| 100 000   | 0.843 | 4.530 | 5.4x slower |
| 1 000 000 | 7.172 | 34.014 | 4.7x slower |

**The GPU never beats `CpuMultiThread`; there is no crossover.** The gap narrows
with batch size but does not close. Cause (measured, not guessed): the path is
**memory-transfer / launch bound** — each generation issues ~tens of dispatches
(one per event depth) and each dispatch round-trips the CPU/GPU boundary (upload
batch, dispatch, read back 4 arrays, CPU collide, re-upload). The trivial `f32`
flight arithmetic is dwarfed by this per-event synchronisation + PCIe transfer.
Beating multi-thread would require moving the collision physics onto the GPU too
(so a batch advances through many events without leaving GPU memory) — filed as
follow-up, out of scope here.

## k agreement (GPU within combined sigma of CPU)

All batch sizes agree within combined 1-sigma of the trusted `CpuSingleThread`
reference. Example (1e5 hist/gen): k_single = 1.00956 ± 0.00246 vs
k_gpu = 1.00936 ± 0.00281, |Δk| = 0.0002 (~0.05σ). The in-suite
`three_compute_modes_agree_on_godiva` test asserts pairwise agreement within a 5σ
band and passes with the GPU arm on the batched path (k_gpu = 1.00933 ± 0.00727
vs k_single = 1.00762 ± 0.00827, 0.16σ).

## RNG reproducibility decision

Per-history independent LCG sub-streams (jump-ahead from `(seed, gen, hist)`),
threaded through the GPU flight (state advanced **bit-exactly**, verified
1024/1024 on the RTX 3050) and continued on the CPU for collision draws.
**Reproducible run-to-run and scheduling-independent; NOT bit-identical to
`CpuSingleThread`** (the flight uniform/distance are `f32` and the stream
structure is per-history, as in `CpuMultiThread`). CPU single-thread `f64` stays
the trusted bit-reproducible reference. This is the accepted "GPU = non-bit-repro
`f32` acceleration path" decision.

## Verification performed

- `cargo test -p outram-mc-libs --lib --release`: **88 passed, 1 ignored
  (the timing sweep), 0 failed** (baseline was 80; +8 new tests). No regressions.
- `cargo check -p outram-mc-libs --target aarch64-linux-android --lib`: **clean**
  (all wgpu code cfg-gated off Android; CPU mirror + structs + native-grid build
  there; `ComputeType::Gpu` falls back to CPU on Android).
- GPU tests ran on a real NVIDIA GeForce RTX 3050 (Vulkan), not skipped.

## Human-verify checklist (before trusting)

1. **WGSL 64-bit LCG emulation** (`batched_flight.wgsl` `mul64`): confirm the
   16-bit schoolbook multiply + carry is correct on other GPUs/backends, not just
   this RTX 3050 — the `gpu_lcg_state_matches_cpu` test is the gate; run it on a
   second adapter if available.
2. **`f32` uniform derivation** (top-24-bit of the advanced state) vs the CPU
   `f64` `prn`: confirm the documented divergence is acceptable for the intended
   use and does not bias k beyond the measured combined-sigma agreement.
3. **`collide_batched` vs `transport_history`**: verify the reaction partition and
   RNG-draw order are a faithful mirror (they must stay in lockstep so the stream
   is coherent across the GPU/CPU boundary); the (n,2n) secondary seeding
   (`BATCH_SECONDARY_STRIDE` jump-ahead) is new and non-overlap should be checked.
4. **Native union grid for the LOW/WMP tier**: `native_energy_grid` emits WMP
   *window edges*, which are not resonance peaks — confirm the log backbone floor
   is dense enough that the WMP resonance range is not under-resolved for
   thermal/epithermal problems (Godiva is fast, so this was not stressed here).
5. **Timing methodology**: the sweep uses 10 generations for throughput; confirm
   this is representative and re-run at converged settings if the timing verdict
   is to be cited in a paper.
6. **V&V sign-off**: the crate README `Bookkeeping status` axes remain
   maintainer-only; nothing here flips them.
