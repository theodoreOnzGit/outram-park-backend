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

//! # HTR-10 code-to-code verification against the RMC paper
//!
//! **Reference.** Li Wanlin, Yu Ganglin & Wei Chunlin, *"Research on Benchmark
//! Calculation and Analysis of HTR-10 with RMC Code"*, 7th International Topical
//! Meeting on High Temperature Reactor Technology (HTR 2014), Weihai, China,
//! 27-31 October 2014.
//!
//! This is the coupling layer's job: the paper specifies one reactor, and both
//! the Monte Carlo and the deterministic ends of this suite should reproduce it
//! from **the same** geometry and composition. That shared model lives here so
//! the two ends cannot drift apart and quietly turn a composition difference
//! into an apparent transport difference.
//!
//! # What this module can verify today, and what it cannot
//!
//! Read this before quoting anything from here. The honest scope is narrower
//! than "reproduce the paper", and the reason is in the paper itself.
//!
//! ## Verifiable now — the model's construction
//!
//! Tables 1 and 2 are **over-determined**: they state quantities that are also
//! derivable from other quantities they state. Every such closure is a genuine
//! code-to-code check that our reconstruction matches theirs, and none of them
//! needs a transport solve. [`GeometryClosure`] carries them.
//!
//! ## NOT verifiable now — the k-eff curve
//!
//! The paper's Tables 3 and 4 give `k_eff` against fuel-loading height, which is
//! the headline result. **We cannot reproduce it yet, and the blocker is the
//! paper's own**:
//!
//! > *"Modeling details of reflector and structural material are referred to
//! > paper released by IAEA which is listed in reference."*
//!
//! That reference is IAEA-TECDOC-1382. The HTR-10 core is 180 cm across inside
//! roughly a metre of graphite reflector which *"house\[s\] control rods, small
//! absorber balls, helium flow channels, and irradiation channels"* — and that
//! reflector is most of the reason so small a fissile inventory reaches
//! criticality. Without it the eigenvalue is not close, and pretending otherwise
//! would be reporting a number that looks like a comparison and is not one.
//!
//! ## A defect in the reference, found while reading it
//!
//! Tables 3 and 4 are **both captioned "(vacuum)"**, while the text says
//! *"Calculations are performed for vacuum and helium."* Checking them row by
//! row:
//!
//! - the **RMC** column is byte-identical in **11 of 11** shared rows;
//! - the **MCNP** column differs in **0 of 11** — that is, in every row.
//!
//! So the paper reports **one** RMC dataset against **two** MCNP results, and
//! one of the two captions is wrong. Consequence for anyone verifying against
//! it: there is a single RMC curve, not a vacuum/helium pair, and an attempt to
//! reproduce two would be chasing an artefact. [`RMC_KEFF_VS_HEIGHT`] carries
//! that single curve.
//!
//! ## How close is close, for this reference
//!
//! The paper's own RMC-vs-MCNP relative differences reach ~0.9 %, and it states
//! plainly that *"model used in this calculation is constructed relatively
//! independently"*. These are indicative code-to-code numbers, **not** a
//! benchmark reference. Agreement to ~500 pcm would be a real success here;
//! agreement to 50 pcm would be suspicious and should prompt a search for a
//! coincidence rather than a celebration.

use outram_mc_libs::prelude::TrisoSpec;

pub mod bed;
pub mod reflector;
pub mod core_model;
pub mod control_rod;
pub mod materials;

/// The paper's single RMC `k_eff` curve against fuel-loading height, Tables 3
/// and 4 (`(height_cm, k_eff)`).
///
/// One curve, not two — see the module docs on the duplicated column. The MCNP
/// columns are deliberately **not** carried here: they are a second code's
/// results on a third model, and mixing them in would invite a comparison that
/// is not ours to make.
pub const RMC_KEFF_VS_HEIGHT: &[(f64, f64)] = &[
    (94.182, 0.894_693),
    (103.980, 0.937_122),
    (113.778, 0.972_163),
    (123.576, 1.004_288),
    (133.374, 1.030_738),
    (143.172, 1.056_866),
    (152.970, 1.078_448),
    (162.768, 1.097_370),
    (172.566, 1.114_775),
    (182.364, 1.133_878),
    (192.162, 1.147_570),
    (201.960, 1.162_230),
];

/// Design characteristics from the paper's **Table 1**.
pub mod table1 {
    /// Thermal power \[MW\].
    pub const THERMAL_POWER_MW: f64 = 10.0;
    /// Mean core height \[cm\].
    pub const CORE_HEIGHT_CM: f64 = 197.0;
    /// Core diameter \[cm\].
    pub const CORE_DIAMETER_CM: f64 = 180.0;
    /// Fuel-to-moderator ball ratio, as the paper writes it (`0.57/0.43`).
    pub const FUEL_BALL_FRACTION: f64 = 0.57;
    /// Moderator (graphite) ball fraction.
    pub const MODERATOR_BALL_FRACTION: f64 = 0.43;
    /// Total fuel elements in the core, from the paper's body text.
    pub const FUEL_ELEMENTS: f64 = 27_000.0;
    /// Fuel-ball loading step, i.e. one layer \[cm\] — the paper's own
    /// quantisation, *"selected as the height of a layer ... in order to avoid
    /// fractional fuel or moderator balls"*.
    pub const LAYER_HEIGHT_CM: f64 = 9.798;
    /// Ball filling fraction in the core region, from the paper's body text.
    pub const BALL_FILLING_FRACTION: f64 = 0.61;
    /// Ball diameter \[cm\] (Table 2, both fuel and moderator).
    pub const BALL_DIAMETER_CM: f64 = 6.0;
}

/// One over-determined quantity in the paper: a value the paper **states**,
/// beside the value our reconstruction **derives** from other stated quantities.
///
/// A closure is only evidence if the derivation does not use the stated value —
/// otherwise it is a tautology. Each entry below says what it was derived from.
#[derive(Debug, Clone, Copy)]
pub struct GeometryClosure {
    /// What is being closed.
    pub quantity: &'static str,
    /// What the paper states.
    pub stated: f64,
    /// What our reconstruction implies.
    pub derived: f64,
    /// Units, for reporting.
    pub units: &'static str,
    /// What the derivation used — so a reader can check it is independent of
    /// `stated`.
    pub derived_from: &'static str,
}

impl GeometryClosure {
    /// Relative difference `(derived - stated)/stated`, dimensionless.
    #[must_use]
    pub fn relative(&self) -> f64 {
        (self.derived - self.stated) / self.stated
    }
}

/// Every geometry closure the paper supports, computed from
/// [`bed::HexBedCell`] and [`TrisoSpec::HTR10_LI2014`].
#[must_use]
pub fn geometry_closures() -> Vec<GeometryClosure> {
    let cell = bed::HexBedCell::from_paper();
    let spec = TrisoSpec::HTR10_LI2014;

    vec![
        GeometryClosure {
            quantity: "layer height",
            stated: table1::LAYER_HEIGHT_CM,
            derived: bed::close_packed_layer_spacing(table1::BALL_DIAMETER_CM) * 2.0,
            units: "cm",
            derived_from: "two close-packed layers of 6 cm spheres",
        },
        GeometryClosure {
            quantity: "ball filling fraction",
            stated: table1::BALL_FILLING_FRACTION,
            derived: cell.packing_fraction(),
            units: "-",
            derived_from: "2 balls in the reconstructed hex cell",
        },
        GeometryClosure {
            quantity: "fuel elements in the core",
            stated: table1::FUEL_ELEMENTS,
            derived: cell.balls_in_core(table1::CORE_DIAMETER_CM, table1::CORE_HEIGHT_CM),
            units: "balls",
            derived_from: "the cell tiled through a 180 x 197 cm core",
        },
        GeometryClosure {
            quantity: "heavy metal per fuel ball",
            stated: 5.0,
            derived: heavy_metal_per_ball(),
            units: "g",
            derived_from: "8335 kernels, 250 um radius, 10.4 g/cm3, 17 wt%",
        },
        GeometryClosure {
            quantity: "TRISO packing fraction",
            stated: spec.packing_fraction,
            derived: 8335.0 * (spec.opyc / 2.5).powi(3),
            units: "-",
            derived_from: "8335 particles of radius 455 um in a 2.5 cm fuel zone",
        },
    ]
}

/// Heavy metal per fuel ball \[g\], from Table 2's kernel count, radius, density
/// and enrichment — none of which is the stated 5 g.
#[must_use]
pub fn heavy_metal_per_ball() -> f64 {
    use outram_mc_libs::pebble_beds::htr10::{u235_atom_fraction, RHO_UO2};
    let spec = TrisoSpec::HTR10_LI2014;
    let x5 = u235_atom_fraction();
    let m_u = x5 * 235.043_930 + (1.0 - x5) * 238.050_788;
    let m_uo2 = m_u + 2.0 * 15.994_914_6;
    let v_kernel = 4.0 / 3.0 * std::f64::consts::PI * spec.kernel.powi(3);
    8335.0 * v_kernel * RHO_UO2 * m_u / m_uo2
}

#[cfg(test)]
mod tests;
