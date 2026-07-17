# GPU Collision Physics — Development & Debug Log (bead op-u6s.8)

> **UNTRUSTED AI-DRAFT.** Drafted with AI assistance and verified by the in-crate V&V tests; still requires human review before promotion past the "Unit Tested" V&V stage (see `RESPONSIBLE_USE.md`).

This log records a completed effort in `outram-mc-libs`: moving Monte-Carlo neutron
**collision** physics onto the GPU (bead op-u6s.8) and reproducing the Godiva
k-eigenvalue timing benchmark on a real NVIDIA GeForce RTX 3050. All numbers below
are measured, not projected.

## Background — the op-u6s.7 finding this fixes

The prior GPU path, `run_keff_gpu_batched` (bead op-u6s.7), advanced a batch of
neutrons through one **flight** per GPU dispatch but resolved the branchy
**collision** physics on the CPU. That forced a CPU↔GPU round-trip **per event**:

1. upload the live batch,
2. dispatch the flight,
3. read back position + RNG + flight outcome,
4. collide on the CPU,
5. re-upload the survivors,

and repeat for every collision depth. The per-event PCIe synchronisation dominated
the wall clock, and the GPU **lost to `CpuMultiThread` at every batch size** — the
op-u6s.7 sweep measured the GPU roughly 4.7x slower at $10^6$ histories, with no
crossover anywhere in the tested range.

### op-u6s.7 reference table (historical context only)

This table is retained from op-u6s.7 for context. It was taken on a **different
machine state / different background load**, so it is **not directly comparable
second-for-second** with the op-u6s.8 numbers later in this document. Times are
wall-clock seconds (lower is better).

| Histories/gen | CpuSingleThread | CpuMultiThread | Gpu (batched, collision on CPU) |
|---|---|---|---|
| 1 000 | 0.039 | 0.140 | 1.283 |
| 10 000 | 0.369 | 0.269 | 1.831 |
| 100 000 | 2.281 | 0.843 | 4.530 |
| 1 000 000 | (n/a) | 7.172 | 34.014 |

## What was moved onto the GPU (the honest split)

The point of op-u6s.8 is to keep the whole per-event loop resident on the GPU so the
per-event round-trip disappears. The split is partial and stated plainly below.

### Now on the GPU (per event, in f32, batch-resident)

- **The flight** — per-particle 64-bit LCG advance, macroscopic $\Sigma_t$ lookup on
  the shared native-union grid, distance-to-collision sampling, and the
  bounding-sphere test.
- **Collision-nuclide sampling** — weighted by $N_j \cdot \sigma_{t,j}(E)$.
- **Reaction partition** on the microscopic total: fission | capture | inelastic |
  elastic. (The LOW tier has $(n,2n) = 0$, so that channel collapses out.)
- **Elastic kinematics** — two-body CM→lab; isotropic-CM below the CE/MG seam, or the
  maximum-entropy exponential-$\mu$ forward-scatter law (Langevin-inverse Newton
  solve) above it.
- **Continuum inelastic** — Weisskopf evaporation down-scatter (rejection loop).
- **Fission tagging** — records $\bar\nu$ (production) and the fissioning nuclide
  index, then the neutron dies.

### Stays on the CPU (once per generation, not per event)

- **Fission daughters** — daughter count via `sample_num_neutrons`, plus each
  daughter's isotropic direction and $\chi$ (Watt) birth energy. These are replayed
  from the GPU's handed-back per-particle seed so the per-history random stream stays
  coherent across the boundary.
- **Source sampling, resampling, and statistics.**

This is an honest partial port: full scatter + absorption + fission-tagging runs on
the GPU, and only fission daughter generation — variable 0–8 count, needing
per-nuclide $\chi$ — is deferred to a cheap per-generation CPU pass. The deferral is
called out here rather than hidden.

## WGSL / engineering approach

- A new WGSL kernel, `src/gpu/shaders/batched_event.wgsl`, fuses flight + collision
  into one compute entry point (workgroup size 64, one invocation per neutron).
- The wgpu `downlevel_defaults` limit allows only **4 storage buffers per stage**, so
  everything is packed into 4 storage buffers + 1 uniform:
  - **(0) `xs`** (f32) — grid `++` `macro_total` `++` per-nuclide
    `[total, fission, absorption, inelastic, nu_fission, mubar]` `++` per-nuclide
    scalars `[N, awr, e_max]`.
  - **(1) `istate`** (u32) — `seed_lo` `++` `seed_hi` `++` `alive` `++` `fiss_nuc`.
  - **(2) `fstate`** (f32) — `pos(3N)` `++` `dir(3N)` `++` `energy(N)` `++`
    `production(N)`.
  - **(3) `ctrl`** — a single `atomic<u32>` live-neutron counter.
- **Resident event loop** (host `advance_generation_gpu` in
  `src/gpu/batched_event.rs`): upload once; then per event reset the atomic to 0,
  dispatch over all $N$ neutrons, and read back only the 4-byte live count; stop when
  it hits 0 (or a 100 000-event safety cap). The full particle state is read back
  only **once per generation**. That removes the per-event round-trip entirely.
- The 64-bit LCG integer state is emulated **bit-exactly** (u32 pairs, 16-bit
  schoolbook multiply) as in the earlier flight kernel. The uniform draw is the
  top-24 bits of the advanced state (an f32) — a **documented divergence** from the
  CPU f64 `prn` *value*, while the integer state *stream* stays bit-exact.
- Per-nuclide reaction cross sections are pre-tabulated once on the CPU
  (`src/gpu/collision_grid.rs`, `CollisionTables`) on the same native-breakpoint
  union grid the flight kernel uses, then packed to f32 (`EventTablesF32`). The kernel
  does binary-search + linear interpolation per channel — **the GPU never evaluates
  WMP/MGXS data directly**.
- A byte-for-byte CPU mirror, `advance_event_cpu_mirror`, runs the **same** f32
  arithmetic and draw order and serves as the bit-level logic reference for the
  kernel.

## Problems hit and how they were solved

1. **Wrong checkout.** Building and testing were accidentally run against the main
   repo checkout instead of the isolated git worktree. This was caught because the
   test count did not increase after adding new tests. Fixed by running `cargo` from
   the worktree directory. (A real debugging lesson: verify the expected test-count
   delta, and confirm which working tree the build actually used.)
2. **4-storage-buffer downlevel limit.** The `downlevel_defaults` cap of 4 storage
   buffers per stage forced the packed-buffer layout above — all per-nuclide tables
   and scalars concatenated into one buffer, particle state split into one u32 and one
   f32 buffer, and the atomic counter as the 4th buffer.
3. **Warp divergence.** Neutrons in the same workgroup take different reactions
   (fission / capture / inelastic / elastic), handled by in-kernel branching. Because
   the LOW tier has $(n,2n) = 0$, there are no same-generation secondaries, so the
   resident batch only ever **shrinks**; dead neutrons early-return. No
   sort-by-reaction or stream-compaction was needed at these batch sizes — stated
   honestly. Divergence still costs occupancy, which is one reason the GPU does not
   yet beat a 12-core CPU.
4. **Variable fission-daughter count.** Fission daughters have a variable count
   (0–8) and need per-nuclide $\chi$ sampling, so they are deferred to the
   per-generation CPU pass, with the post-collision seed handed back so the random
   stream stays coherent.
5. **f32 vs f64.** The trusted reference is the raw-f64 CPU transport loop. The GPU is
   f32 acceleration only, judged against the CPU reference and never trusted above it.

## Verification & validation (methodology + results)

### GPU-vs-CPU-mirror one-event gate

- **Test:** `gpu_event_matches_cpu_mirror`, measured on NVIDIA GeForce RTX 3050
  (Vulkan).
- **Methodology:** 4096 Godiva LOW-tier neutrons, one event through each path (fused
  GPU kernel vs `advance_event_cpu_mirror`, same f32 arithmetic and draw order).
  Pass criterion: identical alive outcomes and per-particle state agreeing to f32
  rounding, with bit-exact LCG states.
- **Results:** alive-outcome mismatches = **0 / 4096**; max survivor-energy relative
  difference = **5.07e-7**; max production relative difference = **2.93e-7**; all LCG
  states bit-exact.
- **Interpretation:** the fused GPU collision kernel reproduces the CPU mirror's logic
  to f32 rounding.

### Three-compute-mode k agreement

- **Test:** `three_compute_modes_agree_on_godiva`, RTX 3050, Godiva LOW-tier, 800
  histories × [15 inactive + 40 active], seed 1.
- **Methodology:** run the same eigenvalue problem through `CpuSingleThread` (the
  trusted f64 reference), `CpuMultiThread`, and the fused collision-on-GPU path;
  compare $k$ against the single-thread reference within combined uncertainty.
- **Results:**
  - $k_{\text{single}} = 1.00762 \pm 0.00827$ (trusted reference)
  - $k_{\text{multi}} = 1.00715 \pm 0.00768$
  - $k_{\text{gpu}} = 1.01284 \pm 0.00823$ (fused collision on GPU)
  - single-vs-gpu $|\Delta k| = 0.00522 = $ **0.45σ** (combined σ 0.01167; the 5σ gate
    band is 0.0583) — well within combined uncertainty.
- **Interpretation:** moving the whole collision onto the GPU does not change the
  physics.

## Before vs after benchmark (the deliverable)

This is the authoritative comparison: both GPU drivers timed back-to-back on **one
machine state** (same session), Godiva LOW-tier, $r = 8.7407$ cm, 3 inactive + 7
active generations, from the `gpu_collision_before_after` test. Wall-clock seconds
(lower is better). Adapter: NVIDIA GeForce RTX 3050; CPU: 12 cores.

| Histories/gen | BEFORE `run_keff_gpu_batched` (flight GPU, collision CPU) | AFTER `run_keff_gpu_event` (flight + collision GPU) | CpuMultiThread (12 cores) | GPU speedup (before/after) |
|---|---|---|---|---|
| 10 000 | 0.538 | 0.202 | 0.024 | 2.66x |
| 100 000 | 1.955 | 0.482 | 0.223 | 4.05x |
| 1 000 000 | 19.256 | 5.409 | 2.915 | 3.56x |

`k` agreement across those same runs (all within combined σ):

| Histories/gen | k before | k after | k multi |
|---|---|---|---|
| 10 000 | 1.00777 ± 0.00156 | 1.00559 ± 0.00186 | 1.00614 ± 0.00503 |
| 100 000 | 1.00936 ± 0.00281 | 1.00791 ± 0.00340 | 1.00863 ± 0.00330 |
| 1 000 000 | 1.00852 ± 0.00289 | 1.00726 ± 0.00249 | 1.00796 ± 0.00289 |

### Secondary table — full AFTER sweep

From `gpu_batched_timing_sweep`, taken in a **separate session** with a slightly
different machine state (adds single-thread). Wall-clock seconds (lower is better);
not second-for-second comparable with the before/after table above.

| Histories/gen | CpuSingleThread | CpuMultiThread | Gpu (event, collision on GPU) |
|---|---|---|---|
| 1 000 | 0.015 | 0.003 | 0.399 |
| 10 000 | 0.145 | 0.022 | 0.423 |
| 100 000 | 1.490 | 0.239 | 0.693 |
| 1 000 000 | (n/a) | 2.871 | 4.683 |

## Honest verdict

- Moving collision onto the GPU eliminated the per-event CPU↔GPU round-trip and made
  the GPU transport path **2.7x–4.1x faster** than the collision-on-CPU path (same
  machine state).
- The GPU deficit versus a 12-core `CpuMultiThread` shrank from ~6.6x slower (before:
  19.256 vs 2.915 at $10^6$) to ~1.86x slower (after: 5.409 vs 2.915 at $10^6$), and
  the gap narrows further as batch size grows.
- The GPU now **beats `CpuSingleThread`** for $N \geq 10^5$ (e.g. 0.693 vs 1.490 at
  $10^5$ in the sweep run; single-thread is n/a at $10^6$ but extrapolates far above
  the GPU's 4.68 s).
- **No crossover vs `CpuMultiThread`** on this RTX 3050 + 12-core pairing at any tested
  batch size ($10^3$–$10^6$): the GPU is still ~1.9x slower than the 12-core CPU at
  $10^6$. Residual causes: warp divergence across reaction branches (occupancy loss),
  the still-sequential per-event dispatch chain (one dispatch per event depth), and
  the RTX 3050 being a modest GPU against 12 CPU cores. A stronger GPU, a
  weaker/fewer-core CPU, or larger batches would be expected to move the crossover —
  stated as expectation, not as a measured claim.

## Files changed

- `src/gpu/collision_grid.rs` — **new**; per-nuclide XS tabulation (`CollisionTables`).
- `src/gpu/shaders/batched_event.wgsl` — **new**; fused flight + collision kernel.
- `src/gpu/batched_event.rs` — **new**; packed `EventTablesF32`, SoA `EventBatch`,
  host driver, CPU mirror, and V&V tests.
- `src/gpu/mod.rs` — module registration.
- `src/material/nuclide.rs` — `elastic_mubar` + `e_max_ev` accessors.
- `src/physics/keff.rs` — `run_keff_gpu_event`, `run_keff_event_cpu_mirror`, dispatch,
  and the before/after benchmark.
- `src/prelude.rs` — exports.
