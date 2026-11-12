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

## Completeness extension (2026-11-11, gh:#1386 task 3)

`extract.sh` now also extracts, verbatim and SHA-checked, `synchronize_bank`'s
uniform comb (`src/eigenvalue.cpp:150-175`, wrapped over a stub bank
`bank[i] = i`) and `isotropic_direction` (`src/distribution_multi.cpp:160-166`);
`run.sh` also links `src/random_dist.cpp` unmodified.

| quantity | values | bit-identical |
|---|---|---|
| `synchronize_bank` comb, bank sizes 937/999/1000/1203/2999/5000, n = 1000, `total_gen` 0/30, batches 1/2/7/50 | 48 index lists (~1000 sites each) | **48 of 48**: `comb_resample_openmc` on `OpenMcRunLayout::bank_comb_seed` equals upstream element for element |
| the older `physics::fission::comb_resample` (offset + i x teeth) on the same seed | 48 | 48 of 48 (the accumulated tooth position and `offset + i x teeth` agree on these cases; they can differ where a rounding error crosses an integer, which is why the faithful version exists) |
| `isotropic_direction` (azimuth drawn first, then the cosine), 24 seeds | 24 | **24 of 24** (all three components and the end state) |
| older `rng::distributions::isotropic_direction` (cosine first) | 24 | cosine **0 of 24** (a different draw order) |

`SeedingMode::OpenMc` now gives every random stream of OpenMC's run its OpenMC
seed (table in `rng/openmc_seeding.rs`): tracking (and the **delayed-group draw,
now on the tracking stream**, where `Outram` mode uses a jump-ahead sub-stream),
`STREAM_SOURCE` for the first-generation source (`sample_box_source_openmc`: one
seed per source particle, x/y/z of `SpatialBox::sample`, the fissionable
rejection, OpenMC's isotropic direction), URR, the comb of `synchronize_bank`
(`bank_comb_seed`, `comb_resample_openmc`), and volume (`geometry::volume_calc` has
always used `init_seed(i, STREAM_VOLUME, seed)`). It is read by **all three CSG
k-eigenvalue drivers**: `run_keff_csg_par*` and `CsgPowerIteration::step_par` as
before, and now also `run_keff_csg_seq` and `CsgPowerIteration::step`.

Integration (`openmc_seeding_makes_sequential_and_parallel_drivers_agree`): bare
HEU sphere, 400 histories x (3 + 6): under `SeedingMode::OpenMc` the scalar
driver (`CpuSingleThread`) and the parallel driver give **bit-identical k in every
generation** (k = 1.01384 +/- 0.01638 both), which the default mode cannot (its
sequential driver shares one stream across a generation); the default sequential
driver is bit-identical to itself with the mode named, and differs from the
OpenMc run.

Not covered: the GPU batched drivers (`run_keff_batched*`, WGSL) and the sphere
drivers of `physics::keff` (their own layout; the mode is a field of
`KeffSettings` they do not read), a `FixedSource` run (its source stream is not
OpenMC's), and the ORDER of draws inside a history (see below).

## What the option does and does not do

It gives each stream OpenMC's seed. It does **not** make a history consume random
numbers in OpenMC's order: the port draws a fission neutron's direction before its
energy and independently of the incident direction (OpenMC: delayed/prompt, then
the energy law, then a cosine and `rotate_angle`), the transport loops call the
older scatter kernels (`two_body_scatter_with_mu`, `free_gas_elastic_scatter_dbrc`;
the OpenMC-faithful `elastic_scatter_openmc` and friends exist in
`physics::scatter` but are not wired in, see `upstream_openmc_samplers/README.md`),
and so on. Those are the first places a one-history track differential
(#1386 prio 2) will diverge. **Default stays `Outram`** (maintainer decision
2026-08-06: statistical correctness, not particle parity; every pinned k was
measured with it).
