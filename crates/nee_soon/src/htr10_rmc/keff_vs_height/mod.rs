// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! **HTR-10 `k_eff` against loading height: the shared run machinery and
//! the record it writes** (gh:#501, 2026-10-02).
//!
//! Orchestration only — no physics is implemented here. One HTR-10 core
//! (Şeker & Çolak 2003's 13-ball bed, `N` layers, gh:#472) is transported
//! with `outram_mc_libs`' hybrid delta/surface tracker, and the result is
//! compared with Li, Yu & Wei (2014)'s RMC curve and the paper's two MCNP
//! columns, each read at the height where Şeker's model holds as many balls
//! as the built bed ([`super::seker_height_for_balls`]).
//!
//! Before this module, every HTR-10 example carried its own copy of the
//! majorant grid, the fissile source box, the entropy mesh and the reference
//! interpolation. Those copies now call:
//!
//! - [`bed_majorant`] — the region-local majorant over the bed's materials;
//! - [`fissile_source_box`] / [`fissile_entropy_mesh`] — both span the WHOLE
//!   fissile region, conus floor included (a mesh blind to part of the core
//!   reports convergence of the part it can see);
//! - [`run_core`] — one k-eigenvalue run with a pinned thread count, returned
//!   as a [`HeightPoint`] plus the raw [`KeffResult`];
//! - [`script`] — the deterministic standalone matplotlib script, the
//!   markdown results table and the reconstruction parameters.
//!
//! The reference curves are taken from [`super::RMC_KEFF_VS_HEIGHT`],
//! [`super::MCNP_TABLE3_KEFF_VS_HEIGHT`] and
//! [`super::MCNP_TABLE4_KEFF_VS_HEIGHT`] directly, never retyped.
//!
//! Heights cross the public API as `uom` [`Length`]s; `k_eff` and its
//! standard deviation are dimensionless `f64`.

pub mod script;

use std::time::Instant;

use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::material::material::Material;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
use outram_mc_libs::physics::keff::{ComputeType, KeffResult, KeffSettings, ThreadCount};
use outram_mc_libs::physics::transport_csg::{run_keff_csg_hybrid, SourceBox};
use outram_mc_libs::tally::mesh::RegularMesh;
use uom::si::f64::{Length, Time};
use uom::si::length::centimeter;
use uom::si::time::second;

use super::core_model::{mat, AssembledCore};
use super::{
    keff_curve_at_height, seker_height_for_balls, MCNP_TABLE3_KEFF_VS_HEIGHT,
    MCNP_TABLE4_KEFF_VS_HEIGHT, RMC_KEFF_VS_HEIGHT,
};

/// Temperature \[K\] of every material, 27 °C, as Li, Yu & Wei (2014) and
/// Şeker & Çolak (2003) state.
pub const TEMPERATURE_K: f64 = 300.15;

/// Radial ring count of the full-radius bed (the bed radius is always the
/// physical 90 cm; the ring count is a floor on the tiling).
pub const RINGS: usize = 14;

/// The Şeker layer counts a k-vs-height sweep runs: N = 10 … 20.
///
/// N = 9 (94.182 cm) is not run: at equal ball count it reads the reference
/// at 92.9 cm, below RMC's lowest tabulated point, so there is nothing to
/// compare with short of extrapolating.
pub const SWEEP_LAYERS: [usize; 11] = [10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20];

/// Particles per cycle, cycles and seed of one sweep. Fixed per example: the
/// committed source is the specification of the run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SweepStatistics {
    /// Short name, used in file names and the plot title.
    pub name: &'static str,
    /// Particles per cycle.
    pub particles: usize,
    /// Inactive (discarded) cycles.
    pub inactive: usize,
    /// Active cycles.
    pub active: usize,
    /// RNG seed (one seed per point).
    pub seed: u64,
}

impl SweepStatistics {
    /// The quick sweep: 2000 × \[30 inactive + 70 active\], the statistics of
    /// the 2026-10-01 records (`verification_and_validation/htr10_seker_2026_10_01/`).
    pub const QUICK: Self = Self {
        name: "quick",
        particles: 2000,
        inactive: 30,
        active: 70,
        seed: 20_260_917,
    };

    /// The heavy sweep: 10 000 × \[5 inactive + 135 active\], the reference
    /// paper's own statistics (Li, Yu & Wei 2014), as in
    /// `verification_and_validation/htr10_seker_2026_10_07_10k/` (the current
    /// record, bounded majorant, gh:#589) and the superseded
    /// `htr10_seker_2026_10_01_10k/`.
    pub const HEAVY: Self = Self {
        name: "heavy",
        particles: 10_000,
        inactive: 5,
        active: 135,
        seed: 20_260_917,
    };

    /// Total cycles.
    #[must_use]
    pub fn cycles(&self) -> usize {
        self.inactive + self.active
    }

    /// Histories the run should simulate, inactive cycles included.
    #[must_use]
    pub fn planned_histories(&self) -> u64 {
        (self.particles * self.cycles()) as u64
    }

    /// The transport settings for this sweep on `threads` pinned threads.
    ///
    /// The thread count is pinned, not `Auto`: with `Auto` it follows machine
    /// load. The hybrid driver seeds per history, so the eigenvalue should not
    /// depend on it; the 2026-10-02 heavy comparison in
    /// `verification_and_validation/htr10_endf8_kvsh_heavy_*` is what tests that
    /// (5 threads then, 8 now).
    #[must_use]
    pub fn keff_settings(&self, threads: usize) -> KeffSettings {
        KeffSettings {
            n_particles: self.particles,
            n_inactive: self.inactive,
            n_active: self.active,
            temperature_k: TEMPERATURE_K,
            seed: self.seed,
            compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(threads)),
            ..KeffSettings::default()
        }
    }
}

/// The 4096-point log energy grid, 1e-4 eV to 20 MeV, on which the bed
/// majorant is tabulated.
#[must_use]
pub fn majorant_energy_grid() -> Vec<f64> {
    let (lo, hi) = (1.0e-4_f64.ln(), 2.0e7_f64.ln());
    (0..4096)
        .map(|i| (lo + (hi - lo) * i as f64 / 4095.0).exp())
        .collect()
}

/// The region-local majorant over the BED's materials (pebble layers and
/// coolant, `0..=mat::HELIUM`), with a 0.3 safety margin. The reflector is
/// surface-tracked, so it must not raise the bed's tracking cost. It depends
/// on the materials only, so a sweep builds it once for every height.
///
/// **GitHub #589 (2026-10-05):** until then `over_indices` tabulated only on
/// this 4096-point log grid, which left the UO2 kernel 14x above the majorant
/// at 661 eV on ENDF/B-VIII.0. Every k-vs-height record before that date
/// used it. `over_indices` now adds every nuclide breakpoint
/// (`Majorant::from_materials`); the old construction is the ablation
/// `Majorant::over_indices_without_breakpoints`.
#[must_use]
pub fn bed_majorant(mats: &[Material], nucs: &[Nuclide]) -> Majorant {
    let bed_mats: Vec<usize> = (0..=mat::HELIUM).collect();
    Majorant::over_indices(mats, &bed_mats, nucs, &majorant_energy_grid(), 0.3)
}

/// The initial-source box: the whole fissile region, from the conus floor
/// to the bed top, full bed radius.
#[must_use]
pub fn fissile_source_box(core: &AssembledCore) -> SourceBox {
    let (zl, zu, rb) = (core.conus_floor, core.bed_half_height, core.bed_radius);
    SourceBox {
        lower: Position::new(-rb, -rb, zl),
        upper: Position::new(rb, rb, zu),
    }
}

/// The 4 × 4 × 4 Shannon-entropy mesh over the same region as
/// [`fissile_source_box`]. Its ceiling is 6 bits.
#[must_use]
pub fn fissile_entropy_mesh(core: &AssembledCore) -> RegularMesh {
    let (zl, zu, rb) = (core.conus_floor, core.bed_half_height, core.bed_radius);
    RegularMesh {
        lower_left: [-rb, -rb, zl],
        upper_right: [rb, rb, zu],
        dimension: [4, 4, 4],
    }
}

/// One height of a sweep: the built core, its result, and the three
/// reference values at equal ball count.
#[derive(Clone, Debug, PartialEq)]
pub struct HeightPoint {
    /// Şeker layers N.
    pub layers: usize,
    /// Balls in the built bed (`None` only for a bed that does not count them;
    /// Şeker's bed always does).
    pub balls: Option<usize>,
    /// Built bed height, `2 × bed_half_height` (`9.798 N + 6` cm).
    pub built_height: Length,
    /// Height at which Şeker's model holds `balls` balls; every reference is
    /// read here (gh:#472). Equal to `built_height` if `balls` is `None`.
    pub reference_height: Length,
    /// `k_eff`, mean over active cycles.
    pub k: f64,
    /// Within-run 1σ of `k` (single seed; seed-to-seed scatter is not in it).
    pub sigma: f64,
    /// RMC (Li, Yu & Wei 2014) at `reference_height`; `None` outside its table.
    pub rmc: Option<f64>,
    /// MCNP Table 3 (Şeker vacuum column) at `reference_height`; gauge only.
    pub mcnp_t3: Option<f64>,
    /// MCNP Table 4 (Şeker helium column) at `reference_height`; gauge only.
    pub mcnp_t4: Option<f64>,
    /// Histories simulated, inactive cycles included.
    pub histories: u64,
    /// Histories whose position could not be located in any cell.
    pub lost_locate: u64,
    /// Shannon entropy \[bits\] of the first and last cycle's source.
    pub entropy_first_last: Option<(f64, f64)>,
    /// Wall-clock time of the transport call alone.
    pub transport_time: Time,
}

impl HeightPoint {
    /// Built height in cm.
    #[must_use]
    pub fn built_height_cm(&self) -> f64 {
        self.built_height.get::<centimeter>()
    }

    /// Reference height in cm.
    #[must_use]
    pub fn reference_height_cm(&self) -> f64 {
        self.reference_height.get::<centimeter>()
    }

    /// `(k − reference) × 1e5` \[pcm\], or `None` when there is no reference.
    #[must_use]
    pub fn residual_pcm(&self, reference: Option<f64>) -> Option<f64> {
        reference.map(|r| (self.k - r) * 1.0e5)
    }

    /// The point's reference values, read from the curves in
    /// [`super`] at the equal-ball-count height.
    fn with_references(mut self) -> Self {
        let h = self.reference_height_cm();
        self.rmc = keff_curve_at_height(RMC_KEFF_VS_HEIGHT, h);
        self.mcnp_t3 = keff_curve_at_height(MCNP_TABLE3_KEFF_VS_HEIGHT, h);
        self.mcnp_t4 = keff_curve_at_height(MCNP_TABLE4_KEFF_VS_HEIGHT, h);
        self
    }
}

/// Transport one assembled core and compare it with the references.
///
/// `layers` is the Şeker layer count the core was built with (it is
/// recorded, not used to build). The majorant is the caller's, built once by
/// [`bed_majorant`]; the source box and entropy mesh come from the core.
#[must_use]
pub fn run_core(
    layers: usize,
    core: &AssembledCore,
    mats: &[Material],
    nucs: &[Nuclide],
    majorant: &Majorant,
    stats: &SweepStatistics,
    threads: usize,
) -> (HeightPoint, KeffResult) {
    let settings = stats.keff_settings(threads);
    let mesh = fissile_entropy_mesh(core);
    let t = Instant::now();
    let res = run_keff_csg_hybrid(
        &core.geometry,
        mats,
        nucs,
        std::slice::from_ref(majorant),
        Some(&mesh),
        fissile_source_box(core),
        &settings,
        None,
    );
    let secs = t.elapsed().as_secs_f64();
    let built_cm = 2.0 * core.bed_half_height;
    let balls = core.bed.as_ref().and_then(|b| b.core_balls());
    let ref_cm = balls.map_or(built_cm, seker_height_for_balls);
    let point = HeightPoint {
        layers,
        balls,
        built_height: Length::new::<centimeter>(built_cm),
        reference_height: Length::new::<centimeter>(ref_cm),
        k: res.k_mean,
        sigma: res.k_std,
        rmc: None,
        mcnp_t3: None,
        mcnp_t4: None,
        histories: res.histories,
        lost_locate: res.lost_locate,
        entropy_first_last: res
            .entropy
            .first()
            .copied()
            .zip(res.entropy.last().copied()),
        transport_time: Time::new::<second>(secs),
    }
    .with_references();
    (point, res)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statistics_are_the_issue_501_settings() {
        let q = SweepStatistics::QUICK;
        assert_eq!((q.particles, q.inactive, q.active), (2000, 30, 70));
        assert_eq!(q.planned_histories(), 200_000);
        let h = SweepStatistics::HEAVY;
        assert_eq!((h.particles, h.inactive, h.active), (10_000, 5, 135));
        assert_eq!(h.planned_histories(), 1_400_000);
        assert_eq!(q.seed, h.seed);
    }

    #[test]
    fn every_sweep_height_has_all_three_references() {
        // Şeker's whole-ball bed holds 721 + 609 balls per layer pair
        // (gh:#472): 1330 N + 721. Every swept N must read all three curves
        // inside their tables, or a point silently drops out of the plot.
        for n in SWEEP_LAYERS {
            let h = seker_height_for_balls(1330 * n + 721);
            for c in [
                RMC_KEFF_VS_HEIGHT,
                MCNP_TABLE3_KEFF_VS_HEIGHT,
                MCNP_TABLE4_KEFF_VS_HEIGHT,
            ] {
                assert!(keff_curve_at_height(c, h).is_some(), "N = {n}, h = {h}");
            }
        }
    }

    #[test]
    fn curve_reads_rows_exactly_and_refuses_to_extrapolate() {
        let c = RMC_KEFF_VS_HEIGHT;
        assert_eq!(keff_curve_at_height(c, 123.576), Some(1.004_288));
        assert!(keff_curve_at_height(c, 94.0).is_none());
        assert!(keff_curve_at_height(c, 202.0).is_none());
    }
}
