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
//! It sets each history's **tracking stream** (stream 0) to OpenMC's
//! `seeds[STREAM_TRACKING]` for that particle, and hands the URR stream
//! (`seeds[STREAM_URR_PTABLE]`) to the history in place of the jump-ahead the
//! port derives from the tracking stream. It does **not** make a history
//! consume random numbers in OpenMC's order: the port samples the delayed group
//! on a separate jump-ahead stream, where OpenMC uses the tracking stream. That is a
//! deliberate port decision, recorded in `transport_csg.rs`, and is the first place a
//! track differential will diverge. Only the CSG parallel drivers
//! (`run_keff_csg_par*`, `CsgPowerIteration::step_par`) read the mode; the
//! single-stream drivers ignore it.
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
