# V&V: OpenMC-compatible seeding (`init_particle_seeds`, `compute_*_seed`) (gh:#1386 section 2 item 1)

> Research, education and V&V only (`RESPONSIBLE_USE.md`). AI draft, unreviewed.

## Methodology

- **Upstream:** OpenMC `8f79d9d0` (MIT). `src/random_lcg.cpp` is compiled
  **unmodified** (g++ 13.3, `-O2 -ffp-contract=off`). `overall_generation`,
  `compute_particle_id` and `compute_transport_seed` (`src/simulation.cpp`
  lines 784, 790, 800) are extracted verbatim by `extract.sh` (commit and
  SHA-256 checked) and compiled against stubs of the globals they read
  (`settings::{n_particles, gen_per_batch, use_shared_secondary_bank}`,
  `simulation::{work_index, simulation_tracks_completed, current_batch,
  current_gen, total_gen}`, `mpi::rank`; see `driver.cpp`). OpenMC itself is not
  built. `run.sh` writes `reference.txt` (hex bits).
- **Port:** `petir::rng::lcg::init_particle_seeds` (+ `N_STREAMS`,
  `STREAM_*`, `future_seed_coefficients`) and
  `outram_mc_libs::rng::openmc_seeding` (`OpenMcRunLayout`: `overall_generation`,
  `compute_particle_id`, `compute_transport_seed`, `particle_seeds`), selected for the CSG
  parallel driver by `KeffSettings::seeding = SeedingMode::OpenMc`.
- **Compared:** all four stream seeds of `init_particle_seeds` for 5 master seeds
  (incl. negative and near `i64::MAX`, so `master + i` wraps) x 12 ids (incl. ids
  whose `id * stride` wraps `u64`), the first and third `prn` of stream 0 and the
  first of the URR stream, four ids at a non-default stride, and the particle id
  and transport seed over single rank / rank 1 of 4 / shared secondary bank,
  `gen_per_batch` 1 and 5, `total_gen` 0 and 30.
- **Pass criterion:** bit identity.

## Prediction (written before the run)

Bit-identical in every quantity: integer arithmetic, one exact multiply by 2^-64.
`init_particle_seeds` equals `init_seed(id, k)` for each stream (the identity
`multiplier*(master+k)+increment == future_seed(id*stride, master+k)`).

## Results

Toolchain: rustc 1.99.1, g++ 13.3 (-O2, -ffp-contract=off), 2026-11-11.

| quantity | values | bit-identical | worst difference |
|---|---|---|---|
| `init_particle_seeds`, 5 masters x 12 ids x 4 streams | 240 seeds | 240 (100%) | 0 |
| first/third `prn` of stream 0, first of the URR stream | 180 draws | 180 (100%) | 0 |
| non-default stride (1000003), 4 ids x 4 streams | 16 | 16 (100%) | 0 |
| `compute_particle_id`, `compute_transport_seed` (shared bank, rank, `gen_per_batch`, `total_gen`, batch, generation) | 768 | 768 (100%) | 0 |
| `init_particle_seeds == init_seed(id, k)` | 64 ids x 4 streams (driver), 2 masters (Rust) | all | 0 |

Integration (`openmc_seeding_mode_is_named_pinned_and_not_the_default`): bare HEU
sphere, `CpuMultiThread`, 400 histories x (3 + 6): the default and an explicit
`SeedingMode::Outram` are bit-identical; `SeedingMode::OpenMc` differs from them,
is reproducible and gives the same bits on 1 and 2 threads; its k agrees with the
default within 4 sigma (same physics, different streams).

Command: `CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR=/home/user/outram-park-backend/target cargo test --release -p outram-mc-libs --test openmc_seeding_vs_openmc_code_to_code` gives 6 passed.

No difference was observed, so no cause needs explaining.

## What the option does and does not do

`SeedingMode::OpenMc` sets stream 0 (tracking) and the URR stream of each history
of the CSG parallel drivers to OpenMC's `seeds[STREAM_TRACKING]` and
`seeds[STREAM_URR_PTABLE]`. It does **not** yet make a history consume random
numbers in OpenMC's order: the port samples the delayed group on its own
jump-ahead stream where OpenMC uses the tracking stream, and the sequential and
GPU drivers ignore the mode. Those are the first places a one-history track
differential (#1386 prio 2) will diverge. **Default stays `Outram`** (maintainer
decision 2026-08-06: statistical correctness, not particle parity; every pinned k
was measured with it).
