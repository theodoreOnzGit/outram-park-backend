# GPU-vs-CPU benchmarks: Godiva k_eff and HIGH-fidelity TRISO k∞

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** This is an **AI-drafted** V&V record. All
> code and numbers here are **untrusted draft material** until a human reviews
> them (see the crate `docs/ai-fleet-review/gpu-benchmarks/REVIEW_MANIFEST.md`,
> the workspace `VERIFICATION_AND_VALIDATION.md`, and `RESPONSIBLE_USE.md`). Not
> for nuclear facility operation, reactor control, safety-critical, or licensing
> decisions.

<!-- op-jis-supersede-banner -->
> ⚠️ **Superseded in part — `op-jis` PCG output permutation (2026-08-06).**
> `rng::lcg::prn` gained OpenMC's PCG-RXS-M-XS output permutation (bead
> `op-jis`). The LCG **state recurrence is unchanged**, so every integer-state
> fact in this record still holds, but every statistic derived from the sampled
> **uniform values** has moved. Each results block below is individually
> annotated: re-measured figures are dated **2026-08-06** and carry a
> *Supersedes* note naming the old value; figures that could **not** be re-run on
> this machine are left visible, clearly marked **PENDING A RE-RUN**, and must
> not be cited as current.

**Generated:** 2026-07-17 (Asia/Singapore); partially re-measured 2026-08-06 (see per-block notes)
**Crate commit at generation:** branch `develop`, based on `8e5d1e5`
**Machine (2026-07-17 original run):** NVIDIA GeForce RTX 3050 (6 GB, driver 610.43.02), Vulkan backend via `wgpu` 29.0.3; single-thread `f64` CPU reference.
**Machine (2026-08-06 op-jis re-run):** NVIDIA RTX A5000 (NVK GA102) — a **different adapter** from the original run, so GPU-side figures re-measured on 2026-08-06 confound the `prn` change with the hardware change; this is flagged where it matters rather than explained away.
**Beads:** op-nx0 (Godiva benchmark), op-6tz.37 (second TRISO tutorial), op-u6s.4 (GPU-XS-into-transport wire-in — partial), op-jis (PCG output permutation).

## What was measured, and what was NOT

This record covers two benchmarks. In **both**, the CPU raw-`f64` transport is
the **trusted, deterministic reference**; the GPU `f32` path is **acceleration
only**, never a reference.

- **Measured:** (1) the CPU k-eigenvalue baseline (wall-clock, throughput, and
  k ± σ against a reference) for each case; (2) the **isolated
  cross-section-interpolation kernel** GPU-vs-CPU throughput and f32-vs-f64
  agreement, on the same macroscopic total cross section Σ_t(E) the transport
  loop queries.
- **NOT measured (honest disclosure):** an **end-to-end GPU-accelerated k_eff**.
  The GPU XS kernel (`src/gpu/xs_interp.rs`, op-u6s.3) is **not yet wired into
  the transport collision sweep** — that is follow-up **op-u6s.4**. The history-
  based transport loop queries one collision energy at a time (serial), so it
  cannot yet feed the batched GPU lookup. No GPU k_eff / k∞ speedup is claimed
  or fabricated here.

### The partial op-u6s.4 wire-in used by these benchmarks

A new module `src/gpu/union_grid.rs` (`UnionTotalXs`) tabulates
`Material::macro_xs_total(E)` — the exact macroscopic total the transport loop
uses — on a dense **log-spaced** energy grid, and exposes a batched lookup that
runs on CPU (`lookup_cpu`, f64 reference) and GPU (`lookup_gpu`, f32, reusing
`interp_xs_gpu`). This is the data path the throughput benchmarks exercise. It
is a **dense-log resample, not a union of the nuclides' native ENDF energy
breakpoints** — that native-breakpoint union, and feeding the batched lookup
into an event-based collision-site sweep, remain the open part of op-u6s.4.

---

## Benchmark A — Godiva bare-sphere k_eff (op-nx0)

### Methodology

- **Model:** Godiva / HEU-MET-FAST-001 bare HEU metal sphere, r = 8.7407 cm,
  vacuum boundary. Atom densities (atoms/barn·cm): U-234 4.9184e-4,
  U-235 4.4994e-2, U-238 2.4984e-3; T = 293.6 K.
- **Data tier:** LOW / offline (`Nuclide::from_core`, embedded WMP + fast MGXS)
  so the CPU baseline is fully reproducible with no network. (The HIGH-fidelity
  continuous-energy ENDF variant is documented separately in the
  `godiva_keff_endf` example, which reached 1.00367 ± 0.00182 — **superseded by
  `op-jis` and PENDING A RE-RUN**: that example needs `--features net-fetch` to
  download ENDF tapes and there is no local cache on this machine, so no
  post-`op-jis` replacement was measured. Do not cite 1.00367 ± 0.00182 as
  current.)
- **Power iteration:** 5000 histories/generation, 40 inactive + 120 active.
  Fixed RNG seed ⇒ bit-reproducible k.
- **Reference:** ICSBEP HEU-MET-FAST-001, k_eff = 1.0000 ± 0.0010.
- **XS-kernel benchmark:** Σ_t(E) tabulated on 4096 log-spaced points over
  [1e-3, 2e7] eV via `UnionTotalXs`; query energies Watt-sampled at batch sizes
  2^16, 2^18, 2^20; CPU `lookup_cpu` (f64) timed against GPU `lookup_gpu` (f32),
  end-to-end host-visible time (includes upload + dispatch + readback).
- **Pass criteria:** k within combined σ of the benchmark is the physics goal
  (this is a LOW-tier run, so a data-fidelity bias is expected and reported, not
  hidden); GPU-vs-CPU agreement judged as f32 accumulation tolerance.

### Results (CURRENT — re-measured 2026-08-06 under the `op-jis` PCG output permutation)

> **Supersedes** the 2026-07-17 figures **k_eff = 1.01024 ± 0.00171 / Δk =
> +1024 pcm**, CSV final cumulative mean **1.010242**, and the XS-kernel
> agreement columns **2.561e-6 / 7.031e-9**, **1.245e-5 / 7.197e-9**,
> **1.854e-5 / 7.119e-9** (max \|Δ\| / mean \|Δ\|, cm⁻¹). Those were measured
> with the **pre-`op-jis` output function** — the raw top-52 bits of the LCG
> state — and no longer reflect the generator. The state recurrence did not
> change, so the geometry, material, and iteration settings below are untouched.

**CPU k_eff baseline (trusted reference), 2026-08-06:**

- **k_eff = 1.01107 ± 0.00170**, Δk = **+1107 pcm** vs ICSBEP 1.0000 ± 0.0010.
- Interpretation: the +1107 pcm is the known LOW-tier (group-data, no fast
  self-shielding, no (n,2n) column, no MF=5 χ) bias documented for this model;
  it is a data-fidelity offset, not a transport error. The combined σ is 0.00197
  (0.00170 ⊕ 0.0010), so +1107 pcm is a resolved bias (5.6σ), as expected for
  LOW tier. The pre-`op-jis` run gave +1024 pcm at 5.2σ — the same physical
  conclusion, shifted by one realisation of a different uniform stream.
- **Wall-clock / throughput NOT re-measured on 2026-08-06.** The 2026-07-17
  figures (1.224 s; **653 725 source-histories/s** over 800 000 histories, RTX
  3050 host, reproduced bit-for-bit across 3 runs) are retained as the
  2026-07-17 record only; the PCG permutation adds arithmetic per draw, so treat
  them as **not current** for throughput purposes.

**Isolated XS-kernel f32-vs-f64 agreement, 2026-08-06 (NVIDIA RTX A5000 / NVK GA102):**

The query energies are Watt-sampled, so these agreement figures depend directly
on `prn` and moved with `op-jis`. Throughput was **not** re-recorded in this run.

| batch | max \|Δ\| (cm⁻¹) | mean \|Δ\| (cm⁻¹) |
|---|---|---|
| 65 536 (2^16) | 1.797990e-6 | 7.556093e-9 |
| 262 144 (2^18) | 3.410e-6 | 7.507e-9 |
| 1 048 576 (2^20) | 6.110e-5 | 7.626e-9 |

- f32-vs-f64 agreement stays single-precision-tight on the mean (≈ 7.6e-9 cm⁻¹
  against Σ_t of O(0.05–40 cm⁻¹), ≲ 1e-6 relative). The **max** \|Δ\| at 2^20
  rose from 1.854e-5 to 6.110e-5 cm⁻¹; a max over a Watt-sampled batch is a
  tail statistic of the drawn energies, so it is expected to be volatile between
  streams. **Confound, stated plainly:** this re-run was on a *different
  adapter* (RTX A5000 / NVK) from the 2026-07-17 RTX 3050 run, so the change
  cannot be attributed to `op-jis` alone.

**Throughput record (2026-07-17, RTX 3050 — NOT re-measured post-`op-jis`):**

| batch | CPU Mq/s | GPU Mq/s | GPU speedup |
|---|---|---|---|
| 65 536 (2^16) | 30.4 | 75.7 | 2.49× |
| 262 144 (2^18) | 30.0 | 178.8 | 5.95× |
| 1 048 576 (2^20) | 29.7 | 219.4 | 7.39× |

- On that hardware the GPU beat the single-thread f64 CPU reference at every
  batch size, and the speedup **grew with batch size** (2.49× → 7.39×) — the
  signature of a kernel bounded by fixed upload/launch/readback overhead that
  amortises as the batch grows.
- **Caveat on the speedup baseline:** the CPU side is a single-thread scalar f64
  loop. Against a vectorised / multithreaded CPU baseline the small-batch case
  could fall below 1×. The honest claim is: *"GPU wins for large batches on this
  hardware against this single-thread reference,"* not an unconditional speedup.

**Plottable CSVs** (committed, see note at end):

- `gpu_benchmarks/godiva_keff_convergence.csv` — columns
  `generation,k_generation,k_cumulative_mean,k_cumulative_sigma,phase`
  (160 generations; inactive rows carry blank cumulative columns; the first
  active generation has a blank σ). **Regenerated 2026-08-06**: final row is
  generation 159, `k_cumulative_mean` = 1.011075, σ = 0.001695, matching the
  reported k_eff. (Supersedes the pre-`op-jis` final cumulative mean 1.010242.)
- `gpu_benchmarks/godiva_xs_throughput.csv` — one row per batch size, columns
  `batch_size,cpu_ms,gpu_ms,cpu_queries_per_s,gpu_queries_per_s,gpu_speedup,max_abs_err_cm^-1,mean_abs_err_cm^-1`.
  **Regenerated 2026-08-06** by the same re-run.

Reproduce: `cargo run -p outram-mc-libs --release --example godiva_gpu_benchmark`.

---

## Benchmark B — Second TRISO tutorial, HIGH-fidelity ENDF (op-6tz.37)

### Methodology

- **Case:** a SECOND, distinct TRISO doubly-heterogeneous k∞ tutorial (the
  existing `triso_delta_tracking` example is the LOW-tier one). HEU fuel kernels
  randomly packed (RSA) into a reflective cube of moderator, transported by
  delta (Woodcock) tracking with a bin-maximum majorant.
- **Distinct geometry** (vs the LOW example's pf 0.30 / r 0.04 cm / 1.0 cm cube,
  seed 20240715): packing fraction 0.25, kernel radius 0.05 cm, 1.2 cm cube,
  seed 20260717 — 825 kernels packed (realized pf 0.2500).
- **Data tier — HIGH / net-fetched ENDF (opt-in `net-fetch` feature):** all four
  nuclides reconstructed on device from **ENDF/B-VII.1** via `Nuclide::from_endf`
  (RECONR 0.1% tol + BROADR @ 293.6 K): U-234, U-235, U-238, **and the moderator
  H-1** — all HIGH-fidelity, no LOW-tier fallback on any of them.
- **Power iteration:** 2000 histories/generation, 25 inactive + 75 active,
  delta-tracking driver (`run_keff_delta`).
- **XS-kernel benchmark:** identical protocol to Benchmark A, on the HEU fuel
  material's Σ_t.
- **Reference / pass criterion:** this is a k∞ of a fissile HEU infinite medium
  (reflective, no leakage), so k∞ ≫ 1 is the physical expectation; the pass
  criterion is a converged, stationary source and a physically sensible k∞, plus
  GPU-vs-CPU f32 agreement on the XS kernel. It is NOT a benchmark-k comparison
  (no ICSBEP number for this synthetic unit cell).

### Data provenance

- **Case provenance:** OpenMC `triso.ipynb`, openmc-notebooks @ commit
  `cf1e5db` (MIT-licensed, OpenMC project). Adapted to a distinct HIGH-tier
  geometry; cited per `RESPONSIBLE_USE.md`.
- **Nuclear data:** ENDF/B-VII.1 neutron sublibrary (U-234/-235/-238, H-1),
  downloaded from the IAEA Nuclear Data Services `download-endf` tree
  (`https://www-nds.iaea.org/public/download-endf`), **accessed 2026-07-17**
  (U tapes were already present in the local cache from prior runs; the pinned
  upstream URL lives in `njoy-outram-park-fork::acquire::IAEA_BASE_URL`).
  ENDF/B-VII.1 is used (not VIII.0) because its U resonances are Reich-Moore
  (LRF=3), which the RECONR port reconstructs.

### Known fidelity limitation — moderator

A graphite (carbon) moderator would be the textbook TRISO matrix, but C-12 /
C-nat is **not** in the port's `well_known_mat` MAT/Z/A table (currently
H/O/Fe/Th/U/Pu only), so `from_endf("C12")` cannot address it by name. The
moderator therefore uses **H-1** (still HIGH-fidelity ENDF/B-VII.1, but a lighter
scatterer than graphite). This is a documented, honest substitution — the case
demonstrates the HIGH-fidelity net-fetch + GPU-XS path end to end, not a
graphite-moderated TRISO physics benchmark. Adding carbon to `well_known_mat` is
follow-up work (candidate bead).

### Results (2026-07-17, RTX 3050) — SUPERSEDED BY `op-jis`, PENDING A RE-RUN

> ⚠️ **Every sampled statistic in this Benchmark-B block is superseded by the
> `op-jis` PCG output permutation (2026-08-06) and has NOT been replaced.**
> `examples/triso_gpu_benchmark` requires `--features net-fetch` to download the
> ENDF/B-VII.1 tapes it reconstructs from, and this machine has no local cache
> for it, so the example **could not be re-run**. The numbers below were measured
> with the **pre-`op-jis` output function** (raw top-52 state bits). They are left
> visible as the historical record; **do not cite them as current**, and do not
> substitute a guessed replacement. Re-run with
> `cargo run -p outram-mc-libs --release --features net-fetch --example triso_gpu_benchmark`
> on a networked machine to retire this note.
>
> The **two tracked CSVs** this benchmark writes —
> `gpu_benchmarks/triso_keff_convergence.csv` and
> `gpu_benchmarks/triso_xs_throughput.csv` — were likewise **not** regenerated
> and still hold **pre-`op-jis`** numbers.

**HIGH-fidelity reconstruction (on device):** U-234 0.4 s, U-235 12.4 s,
U-238 28.4 s, H-1 0.9 s — total 42.1 s (U tapes warm in cache). *(Reconstruction
is RNG-free deterministic data processing, so these timings are not a `prn`
statistic; they are still 2026-07-17 / RTX-3050-host figures.)*

**CPU k∞ baseline (trusted reference) — SUPERSEDED, PENDING RE-RUN:**

- **k∞ = 1.86062 ± 0.00268** over 100 generations (delta tracking).
  *Pre-`op-jis` value; no post-`op-jis` measurement exists.*
- Transport wall-clock 15.3 s; throughput **1.311e4 source-histories/s**.
- Interpretation (still the qualitative expectation, independent of the RNG
  change): a reflective fissile HEU infinite medium has no leakage, so
  k∞ ≫ 1 is expected; the source converged and stayed stationary — evidence
  delta tracking transports correctly through the packed doubly-heterogeneous
  medium on HIGH-fidelity data.

**Isolated XS-kernel throughput (GPU vs CPU f64 reference) — the agreement
columns are Watt-sampled and therefore SUPERSEDED, PENDING RE-RUN:**

| batch | CPU Mq/s | GPU Mq/s | GPU speedup | max \|Δ\| (cm⁻¹) | mean \|Δ\| (cm⁻¹) |
|---|---|---|---|---|---|
| 65 536 (2^16) | 30.5 | 57.3 | 1.88× | 1.374e-6 | 9.878e-9 |
| 262 144 (2^18) | 32.3 | 186.7 | 5.77× | 1.104e-5 | 9.967e-9 |
| 1 048 576 (2^20) | 28.5 | 171.1 | 6.01× | 2.082e-5 | 9.965e-9 |

- Same pattern as Benchmark A: GPU wins at every batch size (1.88× → 6.01×),
  speedup grows and plateaus ~6× as fixed overhead amortises. f32-vs-f64
  agreement ~1e-8 cm⁻¹ mean (≲ 1e-6 relative) against Σ_t of O(0.1–40 cm⁻¹).
  The same single-thread-baseline caveat as Benchmark A applies.

**Plottable CSVs** (committed) — **still pre-`op-jis`, not regenerated:**

- `gpu_benchmarks/triso_keff_convergence.csv` — columns
  `generation,k_generation,k_cumulative_mean,k_cumulative_sigma,phase` (100
  generations; final cumulative mean matches the reported pre-`op-jis` k∞).
- `gpu_benchmarks/triso_xs_throughput.csv` — one row per batch size (same
  columns as the Godiva throughput CSV).

Reproduce (needs network on first run for the moderator tape; U tapes cache):
`cargo run -p outram-mc-libs --release --features net-fetch --example triso_gpu_benchmark`.

---

## Cross-cutting: GPU-vs-CPU agreement gate

The `UnionTotalXs` GPU path is additionally gated by the unit test
`gpu::union_grid::tests::gpu_matches_cpu_union_grid`, which compares 8195 queries
against real Godiva macroscopic Σ_t within tolerance
`|gpu − cpu| ≤ 3e-3·(1 + |cpu|)`.

- **Re-measured 2026-08-06** (NVIDIA RTX A5000 / NVK GA102): **max \|Δ\| =
  1.948e-4 cm⁻¹, mean \|Δ\| = 2.401e-6 cm⁻¹**. The native-union variant of the
  same gate gave **max \|Δ\| = 3.866e-4 cm⁻¹, mean \|Δ\| = 3.521e-6 cm⁻¹**.
- **This is NOT a `prn` effect and must not be reported as one.** The probe
  energies for this gate are **log-spaced and RNG-free**, so `op-jis` cannot move
  them. The max is **unchanged** at 1.948e-4 cm⁻¹. The mean drifted from the
  2026-07-17 record of **2.342e-6 cm⁻¹** (measured on the RTX 3050) to 2.401e-6
  cm⁻¹; the most likely cause is the different adapter/driver, but that is an
  **unresolved discrepancy flagged here rather than explained away**. Both values
  sit far inside the tolerance.

On CPU-only machines (no GPU adapter) that
test prints a SKIP and the analytical CPU-reference tests still cover the
algorithm. All 78 `--lib` tests pass in release; the Android `--lib` check is
clean (the GPU path is `cfg(not(target_os = "android"))`-gated out).

## Commit-vs-gitignore decision for the CSVs

The crate's default V&V convention gitignores standalone `.csv` files
(`/verification_and_validation/*.csv`) and keeps notebook-comparison CSVs local.
**These GPU-benchmark CSVs are deliberately committed instead**, at the
maintainer's explicit request that the plottable data be usable for plots. They
live in the subdirectory `verification_and_validation/gpu_benchmarks/`, which the
top-level `*.csv` gitignore rule (direct children only) does not match, so they
are tracked by git. They are excluded from the packaged crate via `Cargo.toml`'s
`exclude` (they are reproducible generated outputs, not shipped crate content).
Regenerate them by re-running the two example commands above; do not hand-edit.

## Software provenance

- Transport / XS interpolation mirror OpenMC C++
  (`/home/teddy0/Documents/research/openmc/`): the CPU interpolation reference in
  `src/gpu/xs_interp.rs` mirrors `Nuclide::calculate_xs`
  (`src/nuclide.cpp:716-760`); the k-eigenvalue and delta-tracking drivers cite
  their OpenMC sources in their own module docs. OpenMC is MIT-licensed; this
  port is GPL-3.0-only.
- Benchmark code AI-drafted 2026-07-17, verified by running on real hardware;
  untrusted until human review (see the review manifest).
