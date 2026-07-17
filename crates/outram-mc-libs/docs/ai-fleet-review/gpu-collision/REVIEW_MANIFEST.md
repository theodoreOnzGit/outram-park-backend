# Review manifest — GPU collision physics on the MC event loop (op-u6s.8)

> # ⚠️ UNTRUSTED AI-DRAFT — HUMAN REVIEW REQUIRED
>
> **Every artifact listed here was drafted with AI assistance and is untrusted
> until a human reviews it** (per `RESPONSIBLE_USE.md` / `AI_USAGE.md`). It passes
> the in-crate V&V gates and builds clean, but has **NOT** been promoted past the
> **"Unit Tested"** V&V stage. Do **not** cite it as validated. The timing numbers
> and the k-agreement below were produced by actually running the code on real
> hardware (a real NVIDIA GeForce RTX 3050, Vulkan) — they are not fabricated —
> but they still need human inspection, licence-provenance review, and V&V
> sign-off before this work is trusted. Not for nuclear facility operation,
> reactor control, safety-critical, or licensing decisions.

## Scope / what this effort did

This is the **op-u6s.8** follow-up to **op-u6s.7**. The prior batched-flight GPU
path put **only the flight** on the GPU and kept **collision on the CPU**, which
forced a CPU↔GPU round-trip **per event** — measured at roughly **4.7x slower**
than `CpuMultiThread` with **no crossover** at any batch size. This effort ports
the **collision physics onto the GPU** so the whole generation stays resident in
GPU buffers and advances through many events without leaving GPU memory. Touched
**only** `crates/outram-mc-libs/`.

## What is now on the GPU (honest split)

**ON GPU** — per event, `f32`, whole batch resident in one dispatch chain:

- **Flight** — LCG advance (RNG), macroscopic `Sigma_t` lookup on the shared
  native-union grid (binary-search + interpolation), distance-to-collision
  sampling, sphere distance-to-boundary test.
- **Collision-nuclide sampling** — weighted by `N_j · sigma_{t,j}(E)`.
- **Reaction partition** — `fission | capture | inelastic | elastic`
  (`(n,2n) = 0` for the LOW tier).
- **Elastic kinematics** — two-body CM→lab; isotropic-CM below the CE/MG seam,
  or exponential-`mu` forward-scatter above it (Langevin-inverse Newton solve).
- **Continuum inelastic** — Weisskopf evaporation via a rejection loop.
- **Fission tagging** — records `nu-bar` + the sampled nuclide index (no daughters
  spawned on the GPU).

**ON CPU** — once per generation, not per event:

- **Fission daughters** — the count is drawn via `sample_num_neutrons`, each
  daughter gets an isotropic direction and a `chi`/Watt birth energy, and the draw
  is **replayed from the GPU's handed-back per-particle seed** so the RNG stream
  stays coherent across the boundary.
- Source sampling, resampling, and statistics.

**Honest partial:** fission-daughter generation is the **one** piece deliberately
deferred to the CPU — it produces a variable **0–8** count and needs per-nuclide
`chi`, which does not map cleanly to a fixed-width GPU event step. This is an
honest partial, **not** full collision on the GPU. Everything else in the
collision kernel is resident on the device.

## Files to review (new + modified)

| File | Kind | What to check |
|---|---|---|
| `src/gpu/shaders/batched_event.wgsl` | **NEW** | The fused flight+collision WGSL kernel. Check the 64-bit LCG emulation, the packed-buffer offset arithmetic, the reaction-partition thresholds, and the `f32` kinematics ports. |
| `src/gpu/batched_event.rs` | **NEW** | Host driver (`advance_generation_gpu`), CPU mirror (`advance_event_cpu_mirror` / `advance_generation_cpu_mirror`), packed `EventTablesF32`, SoA `EventBatch`. Check that the CPU mirror draw order matches the WGSL kernel **exactly**, and the resident event-loop read-back logic. |
| `src/gpu/collision_grid.rs` | **NEW** | `CollisionTables` — per-nuclide reaction-XS tabulation on the native union grid. Check the channel columns and the macro-total consistency. |
| `src/material/nuclide.rs` | **MODIFIED** | New `elastic_mubar` and `e_max_ev` public accessors. Check they match the CPU elastic sampler's LOW/HIGH-tier behaviour. |
| `src/physics/keff.rs` | **MODIFIED** | `run_keff_gpu_event`, `run_keff_event_cpu_mirror`, `run_event_power_iteration`, `build_event_batch`, `bank_event_fission`, GPU dispatch routing, and the `gpu_collision_before_after` benchmark. Check that the fission-daughter banking mirrors `collide_batched` and the per-history seed striding. |
| `src/gpu/mod.rs`, `src/prelude.rs` | **MODIFIED** | Module registration + public exports. Check the new modules are wired in and the re-exports are correct. |

## V&V evidence (methodology + measured results)

All GPU runs were on a **real NVIDIA GeForce RTX 3050 (Vulkan backend)** — not
skipped, not simulated.

- **GPU-vs-CPU-mirror one-event gate** — `gpu_event_matches_cpu_mirror`, 4096
  Godiva LOW-tier neutrons advanced one event on the GPU and on the CPU mirror,
  compared element-by-element:
  - alive-outcome mismatches = **0 / 4096**
  - max survivor-energy relative difference = **5.07e-7**
  - max production relative difference = **2.93e-7**
  - all LCG states **bit-exact**.
- **Three-compute-mode k agreement** — `three_compute_modes_agree_on_godiva`
  (Godiva LOW-tier, 800 histories × [15 inactive + 40 active], seed 1):
  - `k_single = 1.00762 ± 0.00827` (trusted `CpuSingleThread` reference)
  - `k_multi  = 1.00715 ± 0.00768`
  - `k_gpu    = 1.01284 ± 0.00823`
  - single-vs-gpu distance = **0.45σ** (well within the asserted 5σ band).
- **CPU-mirror unit tests** — `cpu_mirror_event_makes_progress`,
  `cpu_mirror_event_is_deterministic` pass on every target.
- **`collision_grid` tests** — `per_channel_columns_match_direct_eval`,
  `macro_total_is_sum_of_micro` pass on every target.
- **Full suite** — **95 passed, 2 ignored** (the two hardware benchmarks),
  **0 warnings**. `cargo check --target aarch64-linux-android --lib` is **clean**
  (the GPU path is cfg-gated to the CPU mirror on Android).

## Before/after benchmark (measured on RTX 3050) + crossover verdict

Same-session, back-to-back comparison (Godiva LOW-tier, 3 inactive + 7 active
generations, 12-core CPU), wall-clock seconds:

| Histories/gen | BEFORE (flight GPU, collision CPU) | AFTER (flight + collision GPU) | CpuMultiThread (12 cores) | GPU speedup (before / after) |
|---|---|---|---|---|
| 10 000    | 0.538  | 0.202 | 0.024 | 2.66x |
| 100 000   | 1.955  | 0.482 | 0.223 | 4.05x |
| 1 000 000 | 19.256 | 5.409 | 2.915 | 3.56x |

**Crossover verdict (honest):** Moving collision onto the GPU made the GPU path
**2.7x–4.1x faster** than the flight-only-GPU predecessor, and cut the deficit
versus a 12-core `CpuMultiThread` from **~6.6x to ~1.9x at 1e6**. The GPU now
**beats `CpuSingleThread`** for `N >= 1e5`. But there is **NO crossover vs
`CpuMultiThread`** on this RTX 3050 + 12-core pairing at any batch size in the
1e3–1e6 range — it is still **~1.9x slower at 1e6**. Residual causes (reasoned,
not fabricated): warp divergence across the reaction branches, the sequential
per-event dispatch chain, and a modest GPU pitted against 12 CPU cores. **No
fabricated win is claimed.**

## Human-verify checklist

Before trusting any of the above, the human maintainer must personally:

1. **Read `batched_event.wgsl` against the CPU sources it cites**
   (`src/physics/scatter.rs`, `src/material/nuclide.rs`, and
   `keff.rs::collide_batched`) and confirm the reaction partition + kinematics
   match.
2. **Confirm the CPU mirror's RNG draw order is identical to the WGSL kernel** —
   any drift would silently decorrelate the GPU path from its mirror.
3. **Independently re-run** `gpu_event_matches_cpu_mirror`,
   `three_compute_modes_agree_on_godiva`, and `gpu_collision_before_after` on the
   target hardware and confirm the numbers regenerate to within run-to-run noise.
4. **Confirm the fission-daughter CPU banking** (`bank_event_fission`) faithfully
   mirrors `collide_batched`'s fission branch (count, isotropic direction, `chi`
   birth energy) and that the handed-back seed keeps the stream coherent.
5. **Confirm the packed-buffer offset arithmetic in the shader** matches
   `EventTablesF32`'s layout exactly — a wrong offset would read the wrong channel
   with **no crash**.
6. **Review the HIGH-tier fidelity caveat:** on the GPU, HIGH/Pointwise elastic
   degrades to isotropic-CM (`mubar = 0`); confirm this is acceptable / that
   HIGH-tier work uses the CPU backends.
7. **Sign off (or not) the two README `Bookkeeping status` axes** (V&V, human
   interface). AI must **not** flip these — they record *human* review.

---

The committed development log with the full narrative and tables lives at
`crates/outram-mc-libs/docs/gpu_collision_dev_log.md`.
