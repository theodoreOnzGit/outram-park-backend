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

//! # HTTR start-up core: an explicit-TRISO, delta-tracked Monte Carlo model
//!
//! Japan's High Temperature Engineering Test Reactor (HTTR), a prismatic-block
//! HTGR, in its fully-loaded 30-fuel-column start-up core at 300 K: the
//! IRPhEP benchmark HTTR-GCR-RESR-001 (GitHub epic #888; data issues
//! #883-#887). It is HTR-10's counterpart (`crate::htr10_rmc`): the same
//! coolant, moderator and particle, in blocks instead of pebbles.
//!
//! **Status: first build, 2026-10-16. AI-assisted, not yet reviewed by a
//! human.** The geometry is drawn in
//! `verification_and_validation/httr_startup_core/` (`examples/httr_geometry_images.rs`)
//! and its `k_eff` record is in the README there. Nothing in the model was
//! adjusted towards a reference value.
//!
//! # Source
//!
//! **R1:** J. D. Bess, N. Fujimoto, B. H. Dolphin, L. Snoj, A. Zukeran,
//! *Evaluation of the Start-Up Core Physics Tests at Japan's High Temperature
//! Engineering Test Reactor (Fully-Loaded Core)*, HTTR-GCR-RESR-001,
//! INL/EXT-08-14767 Rev. 2, Idaho National Laboratory, March 2010
//! (doi:10.2172/974753), in the IRPhEP Handbook, NEA/NSC/DOC(2006)1. A US
//! DOE report, openly downloadable from the INL digital library. Every
//! dimension is in [`spec`]; every composition in [`materials`]. The
//! geometry follows R1 §3.1 and its sample MCNP5 deck (App. A.1.8).
//!
//! # What is modelled, and where it departs from R1
//!
//! Modelled as R1's benchmark model specifies: every TRISO particle (an
//! ordered lattice of 333 per 0.1 cm layer, 181 818 per rod, 867 million in
//! the core), every fuel rod (4770) with its helium hole, gap, IG-110
//! sleeve and coolant annulus, every BP pin (2.0 and 2.5 wt.% B), 33- and
//! 31-hole fuel blocks, reflector blocks with coolant channels, the 16
//! control columns with explicit 10-section rods at the measured critical
//! heights, the empty RSS and instrumentation holes, 12 solid reflector
//! columns, the 2 mm helium gaps, each column's own rotation, and the PGX
//! permanent reflector out to r = 212.5 cm. The evaluators' own
//! simplifications are kept (they are the benchmark): no dowels or handling
//! holes (a 0.5 % graphite density reduction instead), no instrumentation
//! (its removal is a bias already in the benchmark `k`), all holes 1060 mm
//! deep, the bottom-most reflector block like the others.
//!
//! Departures, each physical or a choice between R1's own conflicting
//! statements, and each stated where it is made:
//!
//! - **R1's text and its deck disagree in three places; the deck is
//!   followed**, because it produced every reference `k` in R1: reflector
//!   coolant channels are 23 mm, not the text's 21 mm
//!   ([`spec::REFLECTOR_CHANNEL_R`]); zones 3-4 have a 33-hole bottom
//!   reflector block, not 31 (`geometry`); the compact's TRISO lattice is
//!   placed so that each 0.1 cm layer is whole, where the deck's lattice
//!   origin cuts the end layers in half (both hold the specified 546 layers'
//!   worth of particles).
//! - **Impurity boron is natural boron** (B-11 added beside R1's B-10),
//!   **carbon is natural carbon** (C-12/C-13), and **each carbon carries the
//!   thermal law of its material** (SiC, B4C, graphite): see
//!   [`materials`].
//! - **Graphite S(α,β) is ENDF/B-VIII.0's 30 % porous reactor graphite**,
//!   chosen by density as HTR-10's was; R1 used crystalline-type graphite
//!   (ENDF/B-VI.8 and VII.0), which is the ablation.
//! - **ENDF/B-VIII.0 throughout.** R1's reference results are on ENDF/B-V.2
//!   to VII.0, JEFF-3.1 and JENDL-3.3; none used VIII.0.
//!
//! # Delta tracking
//!
//! The compact annulus (the TRISO lattice) is delta-tracked against a
//! majorant over the compact's materials ([`compact_majorant`]); the rest is
//! surface-tracked. `CompactTracking::Surface` is the ablation.
//!
//! # One open ambiguity in R1's deck (stated, not resolved, not priced)
//!
//! MCNP's `lat=2` second index runs at +60 degrees from the first, which is
//! how [`spec`] reads the deck. R1 §3.7.2 places the instrumentation tally
//! cylinders at (114.6005, 72.4), (5.4, -135.447) and (-120, 63.04693) cm:
//! the column centres this reading gives, but each offset at 60, 180 or 300
//! degrees, where the deck's instrumentation holes are at 0, 120 and 240.
//! Neither reading of the deck's lattice convention reproduces both, so the
//! tally positions and the deck disagree with each other. It moves the
//! empty holes of three instrumentation columns, not the fuel, and matters
//! for the axial fission-rate tally, not for `k_eff`. Not priced.

pub mod geometry;
pub mod materials;
pub mod spec;

#[cfg(test)]
mod tests;

use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;

use outram_mc_libs::material::material::Material;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::delta_tracking::majorant::Majorant;
use outram_mc_libs::physics::transport_csg::SourceBox;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::run_diagnostics::RunDiagnostics;
use outram_mc_libs::tally::mesh::RegularMesh;

use crate::htr10_rmc::data::{load_slots_with_progress, Htr10DataError, LoadProgress, TapeSource, Uo2Laws};
use crate::htr10_rmc::materials::GraphiteLaw;

pub use geometry::{assemble_httr, AssembledHttr, CompactTracking};
pub use materials::{httr_material_set, httr_nuclide_slots, HttrMaterialConfig, HttrThermal};
pub use spec::RodPositions;

/// The benchmark temperature, 300 K (R1 §3.1.4).
pub const BENCHMARK_TEMPERATURE_K: f64 = 300.0;

/// What a run reads.
#[derive(Debug, Clone)]
pub struct HttrDataConfig {
    /// Graphite law (default 30 % porous, see [`materials`]).
    pub graphite_law: GraphiteLaw,
    /// Bound laws or the free-gas ablation.
    pub thermal: HttrThermal,
    /// Temperature of every nuclide and law.
    pub temperature: ThermodynamicTemperature,
    /// Where the tapes are.
    pub tapes: TapeSource,
}

impl Default for HttrDataConfig {
    fn default() -> Self {
        Self {
            graphite_law: GraphiteLaw::default(),
            thermal: HttrThermal::default(),
            temperature: ThermodynamicTemperature::new::<kelvin>(BENCHMARK_TEMPERATURE_K),
            tapes: TapeSource::Workspace,
        }
    }
}

/// Read and process every HTTR nuclide (48 slots, [`materials::nuc`]) and
/// thermal law, through the same loader as HTR-10.
///
/// # Errors
/// A tape that is absent or fails to process.
pub fn load_httr_nuclides<F: FnMut(LoadProgress)>(
    cfg: &HttrDataConfig,
    diag: &mut RunDiagnostics,
    progress: F,
) -> Result<Vec<Nuclide>, Htr10DataError> {
    let (slots, notes) = httr_nuclide_slots(cfg.graphite_law, cfg.thermal);
    load_slots_with_progress(
        &slots,
        &notes,
        cfg.temperature,
        &cfg.tapes,
        &Uo2Laws::GeneratedFromLeapr,
        diag,
        progress,
    )
}

/// The compact's majorant: the kernels, coatings, overcoat and matrix
/// (`materials::mat::COMPACT`), on HTR-10's energy grid plus every nuclide
/// breakpoint, with HTR-10's 0.3 margin.
#[must_use]
pub fn compact_majorant(mats: &[Material], nucs: &[Nuclide]) -> Majorant {
    Majorant::over_indices(
        mats,
        &materials::mat::COMPACT,
        nucs,
        &crate::htr10_rmc::keff_vs_height::majorant_energy_grid(),
        0.3,
    )
}

/// The initial-source box: every fuel column (rings B-D reach r = 129.5 cm
/// at a column corner) over the fuel's height.
#[must_use]
pub fn fuel_source_box() -> SourceBox {
    let r = 130.0;
    SourceBox {
        lower: Position::new(-r, -r, spec::FUEL_BOTTOM_Z),
        upper: Position::new(r, r, -spec::FUEL_BOTTOM_Z),
    }
}

/// A 9 x 9 x 5 Shannon-entropy mesh over [`fuel_source_box`]: one axial
/// bin per fuel layer, radial bins of about one column.
#[must_use]
pub fn fuel_entropy_mesh() -> RegularMesh {
    let b = fuel_source_box();
    RegularMesh {
        lower_left: [b.lower.x, b.lower.y, b.lower.z],
        upper_right: [b.upper.x, b.upper.y, b.upper.z],
        dimension: [9, 9, 5],
    }
}

/// A reference `k` and where it is from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReferenceK {
    /// What it is.
    pub label: &'static str,
    /// `k`.
    pub k: f64,
    /// Lower 1-sigma uncertainty (equal to `plus` where symmetric).
    pub minus: f64,
    /// Upper 1-sigma uncertainty.
    pub plus: f64,
}

/// Validation: the evaluated **benchmark** `k_eff` of the fully-loaded
/// critical, 1.0025 (-0.0060 / +0.0071) (R1 §3.1.5.1). The measured delayed
/// critical (k = 1.00025, R1 Table 1.17) plus the instrumentation-removal
/// bias (+0.254 ± 0.073 % Δk), with the evaluated uncertainties, dominated
/// by IG-110 and PGX boron impurity (R1 Table 2.64).
pub const BENCHMARK_CRITICAL: ReferenceK = ReferenceK {
    label: "IRPhEP benchmark, fully-loaded critical (R1 §3.1.5.1)",
    k: 1.0025,
    minus: 0.0060,
    plus: 0.0071,
};

/// Validation: the evaluated benchmark `k` of the fully-loaded
/// **subcritical** state (all rods at -55 mm), 0.6876 ± 0.0104 (R1 §3.1.5.2,
/// Table 2.65), from an inverse-kinetics measurement.
pub const BENCHMARK_SUBCRITICAL: ReferenceK = ReferenceK {
    label: "IRPhEP benchmark, fully-loaded subcritical (R1 §3.1.5.2)",
    k: 0.6876,
    minus: 0.0104,
    plus: 0.0104,
};

/// Verification (code to code): R1's own MCNP5 results on the same benchmark
/// model, ordered TRISO lattice, 50 000 x [50 + 1000] histories, ±0.0001
/// (R1 Table 4.1 for the critical, Table 4.2 for the subcritical). Library,
/// critical, subcritical.
pub const MCNP5_R1: [(&str, f64, f64); 6] = [
    ("ENDF/B-V.2", 1.0198, 0.7016),
    ("ENDF/B-VI.8", 1.0222, 0.7025),
    ("ENDF/B-VII.0", 1.0229, 0.6999),
    ("JEFF-3.1", 1.0236, 0.7036),
    ("JENDL-3.3 (+ENDF/B thermal)", 1.0178, 0.6979),
    ("ENDF/B-VII.0 (MCNPX)", 1.0231, 0.7001),
];
