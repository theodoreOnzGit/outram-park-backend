// SPDX-License-Identifier: AGPL-3.0-only
//! **OpenMC-compatible particle seeding** (gh:#1386, section 2 item 1).
//!
//! Port of the seed layout of OpenMC `8f79d9d0` (MIT): `init_particle_seeds`
//! (`src/random_lcg.cpp`, in PETIR as
//! [`petir::rng::lcg::init_particle_seeds`]) and `overall_generation`,
//! `compute_particle_id`, `compute_transport_seed` (`src/simulation.cpp`
//! lines 784, 790, 800). OpenMC gives particle number `id` of the whole run
//! [`N_STREAMS`] seeds, `seeds[i] = future_seed(id * stride, master_seed + i)`:
//! one stream each for tracking, source, URR probability tables and volume.
//!
//! # Which seeding runs by default
//!
//! **[`SeedingMode::Outram`] is the default and is unchanged**: one stream per
//! history, `future_seed(hist * HIST_STRIDE, future_seed(gen * GEN_STRIDE,
//! seed))`. Reasons: the maintainer's 2026-08-06 decision (statistical
//! correctness, not particle-for-particle parity; see this crate's AGENTS.md,
//! "RNG goal"), and every recorded k-eff and bit-identity pin was measured with
//! it, so flipping the default would re-pin all of them for no change of
//! physics. [`SeedingMode::OpenMc`] is the named alternative, chosen with
//! `KeffSettings::with_seeding`, for the one-history differential of #1386
//! prio 2.
//!
//! # What `SeedingMode::OpenMc` does and does not do
//!
//! Every random stream of OpenMC's run has its OpenMC seed in this mode
//! (extended 2026-11-11, gh:#1386 task 3):
//!
//! | OpenMC stream | what draws from it | in `SeedingMode::OpenMc` |
//! |---|---|---|
//! | `STREAM_TRACKING` (0) | every transport draw of a history, **including the delayed-group draw** | the history's tracking seed; the delayed group is sampled from it (it used a separate jump-ahead sub-stream in `Outram` mode) |
//! | `STREAM_SOURCE` (1) | `initialize_source`: the external source of generation 1 | [`OpenMcRunLayout::source_seed`] per source particle (`sample_box_source_openmc`) |
//! | `STREAM_URR_PTABLE` (2) | URR probability-table bands | the history's URR seed (as before) |
//! | `STREAM_VOLUME` (3) | `volume_calc` | `geometry::volume_calc` has always used `init_seed(i, STREAM_VOLUME, seed)` |
//! | tracking, at `init_seed(total_gen + overall_generation, STREAM_TRACKING)` | `synchronize_bank`'s uniform comb | [`OpenMcRunLayout::bank_comb_seed`] and [`comb_resample_openmc`] |
//!
//! All three CSG k-eigenvalue drivers (`run_keff_csg_seq`,
//! `run_keff_csg_par*`, `CsgPowerIteration::new/step/step_par`) read the mode
//! (the sequential driver and the iteration object since 2026-11-11; before
//! that only the parallel drivers did). **Extended 2026-11-11 (gh:#1386, #1456
//! task 4):** the sphere drivers of `physics::keff` (`PowerIteration`,
//! `run_keff_cpu_multi`, `run_keff_gpu_inner`, the batched and event GPU
//! drivers) and `FixedSource` read the mode too. In the sphere drivers a history
//! starts from OpenMC's tracking seed and URR seed, the first-generation source
//! particle `i` is sampled by the port's sphere sampler on OpenMC's
//! `STREAM_SOURCE` seed of `i` (the stream is OpenMC's, the sampler is not: a
//! volume sphere source is not the box source), and the bank is combed with
//! `synchronize_bank`'s comb. In `FixedSource`, particle `p` of batch `b` has its
//! source sampled from `STREAM_SOURCE`, its transport and fission progeny from
//! `STREAM_TRACKING`, its URR bands from `STREAM_URR_PTABLE`. **The GPU batched
//! drivers take the seed LAYOUT only**: the WGSL flight kernel advances the raw
//! LCG state and reads the top 24 bits, with no PCG output permutation, so its
//! variates are not OpenMC's values (`batched_flight.wgsl`, "DOCUMENTED
//! DIVERGENCE"); a draw-for-draw comparison is a CPU matter.
//!
//! **Collision kernels (2026-11-11):** transport now uses the OpenMC-faithful
//! kernels by DEFAULT (`physics::scatter::ScatterKernels::OpenMc`): `rotate_angle`,
//! the CM-to-lab block, `LevelInelastic`, `elastic_scatter` with
//! `sample_target_velocity` (CXS, RVS, DBRC; target velocity drawn before the CM
//! cosine) and OpenMC's two-draw reaction ladder (absorption check, then
//! `scatter()`'s `prn * (total - absorption)`). What still differs from OpenMC's
//! consumption order: the port picks the reaction GROUP with its ladder (absorb,
//! inelastic levels + MT91, (n,2n), (n,3n), MT5, other, elastic last) where OpenMC
//! walks elastic then the inelastic reactions in MT order; a discrete level is
//! then chosen with one more draw (`Nuclide::sample_inelastic`) where OpenMC
//! reuses the `scatter()` cutoff; a fissionable nuclide banks fission sites only
//! at a fission event where OpenMC's `create_fission_sites` runs at EVERY
//! collision (weight `nu_t = wgt/k * nu * sigma_f/sigma_t`); fission neutrons
//! draw direction before energy and isotropically with no `rotate_angle`. The
//! first track divergence on a one-nuclide non-fissile sphere is therefore the
//! level pick and the ladder order; on a fissionable one, the per-collision
//! fission banking. The streams are OpenMC's; the consumption order is closer,
//! not identical.
//!
//! Indexing: OpenMC's `current_batch` and `current_gen` start at 1 and
//! `index_source` at 1. With `gen_per_batch = 1` (this crate's generation
//! loop) generation `g` (0-based) is batch `g + 1`, so the transport seed of
//! history `i` (0-based) is `g * n_particles + (i + 1)` on a single rank.

pub use petir::rng::lcg::{
    future_seed_coefficients, init_particle_seeds, DEFAULT_STRIDE, N_STREAMS, STREAM_SOURCE,
    STREAM_TRACKING, STREAM_URR_PTABLE, STREAM_VOLUME,
};

/// How a CSG parallel run derives each history's random streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SeedingMode {
    /// This crate's layout (the default): `future_seed(hist * HIST_STRIDE,
    /// future_seed(gen * GEN_STRIDE, seed))`, one stream per history.
    #[default]
    Outram,
    /// OpenMC's layout: stream 0 and the URR stream come from
    /// [`init_particle_seeds`] at the history's transport seed, with the master
    /// seed `KeffSettings::seed` and `total_gen` previously run generations.
    OpenMc {
        /// OpenMC `simulation::total_gen` (generations of earlier runs; 0 for
        /// a fresh run).
        total_gen: i64,
    },
}

/// The pieces of OpenMC's run state that fix a particle's seeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenMcRunLayout {
    /// `openmc_get_seed()` (`master_seed`).
    pub master_seed: i64,
    /// `openmc_get_stride()` (`prn_stride`).
    pub stride: u64,
    /// `settings::n_particles`.
    pub n_particles: i64,
    /// `settings::gen_per_batch`.
    pub gen_per_batch: i64,
    /// `simulation::total_gen`.
    pub total_gen: i64,
    /// `simulation::work_index[mpi::rank]` (0 on a single rank).
    pub rank_work_index: i64,
    /// `settings::use_shared_secondary_bank`.
    pub shared_secondary_bank: bool,
    /// `simulation::simulation_tracks_completed`.
    pub simulation_tracks_completed: i64,
}

impl OpenMcRunLayout {
    /// A fresh single-rank run: `total_gen = 0`, default stride, no shared bank.
    pub fn single_rank(master_seed: i64, n_particles: i64, gen_per_batch: i64) -> Self {
        Self {
            master_seed,
            stride: DEFAULT_STRIDE,
            n_particles,
            gen_per_batch,
            total_gen: 0,
            rank_work_index: 0,
            shared_secondary_bank: false,
            simulation_tracks_completed: 0,
        }
    }

    /// `overall_generation()` (`simulation.cpp:784`): `gen_per_batch *
    /// (current_batch - 1) + current_gen`, both 1-based.
    pub fn overall_generation(&self, current_batch: i64, current_gen: i64) -> i64 {
        self.gen_per_batch * (current_batch - 1) + current_gen
    }

    /// `compute_particle_id(index_source)` (`simulation.cpp:790`);
    /// `index_source` is 1-based.
    pub fn compute_particle_id(&self, index_source: i64) -> i64 {
        if self.shared_secondary_bank {
            self.rank_work_index + index_source + self.simulation_tracks_completed
        } else {
            self.rank_work_index + index_source
        }
    }

    /// `compute_transport_seed(particle_id)` (`simulation.cpp:800`).
    pub fn compute_transport_seed(
        &self,
        particle_id: i64,
        current_batch: i64,
        current_gen: i64,
    ) -> i64 {
        if self.shared_secondary_bank {
            particle_id
        } else {
            (self.total_gen + self.overall_generation(current_batch, current_gen) - 1)
                * self.n_particles
                + particle_id
        }
    }

    /// The [`N_STREAMS`] seeds of the particle at `index_source` in
    /// (`current_batch`, `current_gen`): `init_particle_seeds(
    /// compute_transport_seed(compute_particle_id(index_source)))`, as
    /// `initialize_particle_track` does (`simulation.cpp:744`).
    pub fn particle_seeds(
        &self,
        current_batch: i64,
        current_gen: i64,
        index_source: i64,
    ) -> [u64; N_STREAMS] {
        let id = self.compute_particle_id(index_source);
        let tseed = self.compute_transport_seed(id, current_batch, current_gen);
        init_particle_seeds(tseed, self.master_seed, self.stride)
    }

    /// The seed `initialize_source` gives the `i`-th source particle of this
    /// rank (0-based) when it samples the external source:
    /// `init_seed(total_gen * n_particles + work_index + i + 1, STREAM_SOURCE)`
    /// (`src/eigenvalue.cpp` / `src/source.cpp`), which is stream
    /// [`STREAM_SOURCE`] of [`init_particle_seeds`] at that id.
    pub fn source_seed(&self, i: i64) -> u64 {
        let id = self.total_gen * self.n_particles + self.rank_work_index + i + 1;
        init_particle_seeds(id, self.master_seed, self.stride)[STREAM_SOURCE]
    }

    /// The seed `synchronize_bank` combs the fission bank with after batch
    /// `current_batch` (1-based), generation `current_gen` (1-based):
    /// `init_seed(total_gen + overall_generation(), STREAM_TRACKING)`
    /// (`src/eigenvalue.cpp:150-151`).
    pub fn bank_comb_seed(&self, current_batch: i64, current_gen: i64) -> u64 {
        let id = self.total_gen + self.overall_generation(current_batch, current_gen);
        init_particle_seeds(id, self.master_seed, self.stride)[STREAM_TRACKING]
    }
}

/// `synchronize_bank`'s uniform comb (`src/eigenvalue.cpp:153-175`) on a single
/// rank: `n` sites from `bank`, teeth `total / n` apart from one offset
/// `prn(seed) * total / n`, the tooth position **accumulated by repeated
/// addition** (`tooth += teeth_distance`) as upstream does.
/// [`crate::physics::fission::comb_resample`] computes `offset + i * teeth`
/// instead; the two agree except where an accumulated rounding error moves a
/// tooth across an integer, so this one is the bit-exact version. Needs a bank
/// in OpenMC's order (sorted by parent, then progeny).
pub fn comb_resample_openmc<T: Copy>(bank: &[T], n: usize, seed: &mut u64) -> Vec<T> {
    let total = bank.len();
    if total == 0 || n == 0 {
        return Vec::new();
    }
    // Comb specification
    let teeth_distance = total as f64 / n as f64;
    let teeth_offset = crate::rng::lcg::prn(seed) * teeth_distance;
    // First and last hitting tooth (start = 0 on a single rank)
    let end = total as f64;
    let tooth_start = ((0.0 - teeth_offset) / teeth_distance).ceil() as i64;
    let tooth_end = ((end - teeth_offset) / teeth_distance).floor() as i64 + 1;
    let mut tooth = tooth_start as f64 * teeth_distance + teeth_offset;
    let mut out = Vec::with_capacity(n + 1);
    for _ in tooth_start..tooth_end {
        // upstream indexes `fission_bank[floor(tooth) - start]` unchecked; a
        // tooth exactly at `total` (offset 0) is clamped to the last site.
        let idx = (tooth.floor() as usize).min(total - 1);
        out.push(bank[idx]);
        // Next tooth
        tooth += teeth_distance;
    }
    out
}

impl SeedingMode {
    /// The `(tracking, URR)` start seeds of history `hist_idx` (0-based) of
    /// generation `generation` (0-based) under [`SeedingMode::OpenMc`], else
    /// `None`. `n_particles` is the generation's source size.
    pub(crate) fn openmc_history_seeds(
        &self,
        run_seed: u64,
        n_particles: usize,
        generation: usize,
        hist_idx: usize,
    ) -> Option<(u64, u64)> {
        match *self {
            SeedingMode::Outram => None,
            SeedingMode::OpenMc { total_gen } => {
                let mut layout =
                    OpenMcRunLayout::single_rank(run_seed as i64, n_particles as i64, 1);
                layout.total_gen = total_gen;
                let s = layout.particle_seeds(generation as i64 + 1, 1, hist_idx as i64 + 1);
                Some((s[STREAM_TRACKING], s[STREAM_URR_PTABLE]))
            }
        }
    }
}
