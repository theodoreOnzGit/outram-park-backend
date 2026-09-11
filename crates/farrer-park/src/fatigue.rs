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
//
// ---------------------------------------------------------------------------
// Ported from:
//   Project:  PRISMS-Fatigue (prisms-center/Fatigue)
//   Source:   src/calculate_FIPs.py, lines 72-100 (plastic shear strain range,
//               the clip of the plane-normal stress at zero, and the
//               `FS_FIP` expression itself)
//             src/volume_average_FIPs.py, `Al7075_band_averaging` and
//               `Al7075_sub_band_averaging` (average over a region, then take
//               the maximum over systems and regions as the grain's value)
//   Version:  commit 2c8fc9a2cc85c0b2b3dcfac987f3a9e603b7fb9f (2023-10-13)
//   Copyright (c) The Regents of the University of Michigan, PRISMS Center
//   Licence:  LGPL-2.1 upstream; relicensed to GPL-3.0-only here under
//             LGPL-2.1 section 3. See crates/farrer-park/NOTICE and
//             crates/farrer-park/docs/upstream-provenance.md.
// ---------------------------------------------------------------------------

//! **Fatigue indicator parameters** — scalar measures of how hard a
//! microstructural site is being worked over a load cycle, used to rank sites
//! by their propensity to initiate a fatigue crack.
//!
//! # STATUS: PARTIAL, AND DELIBERATELY SO. READ THIS BEFORE USING IT.
//!
//! This module implements the **Fatemi-Socie fatigue indicator parameter**
//! and the volume averaging that turns a per-quadrature-point field into one
//! number per grain. It is verified against a hand-computable case
//! (`docs/verification.md`, case 20).
//!
//! It does **not** implement, and must not be described as implementing:
//!
//! - **A fatigue life.** A FIP is an *ordering* of sites, not a number of
//!   cycles. Turning one into a life needs a calibrated
//!   structure-property relation fitted to experiments on the specific
//!   material, which this crate has none of and is not going to invent.
//! - **PRISMS-Fatigue's band and sub-band geometry.** Upstream partitions each
//!   grain into slip bands — layers of voxels parallel to each `{111}` plane —
//!   and averages within a band rather than over the whole grain, because the
//!   band is the physical length scale a crack nucleates over. That
//!   construction is tied to their voxelised microstructures and is not
//!   ported. What is here averages over **whatever set of points the caller
//!   supplies**, which reproduces upstream's reduction *given* a partition but
//!   does not build the partition. See bead `op-q1zn`.
//! - **A converged cyclic response.** The crystal law underneath has **no
//!   backstress** (see [`crate::crystal::CrystalPlasticity`]), so it has no
//!   Bauschinger effect and will not settle into a stable hysteresis loop for
//!   the right physical reason. A FIP computed from many cycles of it is
//!   therefore not trustworthy as a cyclic quantity, and the verification case
//!   is built on a single prescribed reversal rather than on a simulated
//!   cyclic history. **This is the largest gap in the fatigue work and it is
//!   in the crystal layer, not here.**
//!
//! Nothing here is validated, and none of PRISMS-Fatigue's published case
//! studies is reproduced — that is a separate piece of work, bead `op-9smz`,
//! deliberately not started.
//!
//! # The reduction, as upstream performs it
//!
//! 1. Run the last load cycle and record, at the **tension extreme** and the
//!    **compression extreme**, the accumulated signed slip on every system;
//!    and at the tension extreme, the stress normal to every slip plane.
//! 2. Per system, the plastic shear strain amplitude is
//!    `|gamma^tension - gamma^compression| / 2`.
//! 3. Clip the plane-normal stress at zero — a plane in compression gets no
//!    credit, it does not get a penalty.
//! 4. Combine, per system, and take the maximum over the systems of a region.
//!
//! # Units
//!
//! Slips and the FIP itself are dimensionless; stresses are in pascals; the
//! integration weights used for volume averaging are in cubic metres (or
//! square metres per unit thickness in a two-dimensional analysis, which
//! cancels in the average).

use crate::crystal::{CrystalState, SlipFamily, MAX_SLIP_SYSTEMS};
use crate::error::{FemError, Result};
use crate::tensor::Voigt6;

/// The **Fatemi-Socie** fatigue indicator parameter, in its crystallographic
/// (per-slip-system) form:
///
/// `FIP_a = (delta gamma_a / 2) (1 + k max(sigma_n_a, 0) / sigma_ref)`
///
/// where `delta gamma_a / 2` is the plastic shear strain amplitude on system
/// `a` over the cycle and `sigma_n_a` is the stress normal to that system's
/// slip plane at the tension extreme.
///
/// # What the two parameters mean
///
/// - `k` weighs the opening of a slip band against the shearing of it. A
///   larger `k` makes the parameter more sensitive to tensile mean stress.
///   PRISMS-Fatigue's `calculate_FIPs.py` defaults to `k = 10.0`.
/// - `sigma_ref` non-dimensionalises the normal stress. Upstream calls it
///   `sigma_y` and passes the macroscopic yield stress of the alloy.
///
/// Neither is a material constant in any fundamental sense; both are
/// calibration choices, and a FIP computed with one pair cannot be compared
/// with a FIP computed with another.
///
/// # Units
///
/// `k` dimensionless, `sigma_ref` in pascals, the resulting FIP dimensionless.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FatemiSocie {
    normal_stress_weight: f64,
    reference_stress: f64,
}

impl FatemiSocie {
    /// Construct, with both parameters range-checked.
    ///
    /// # Arguments
    ///
    /// - `normal_stress_weight` — `k` \[-\], finite and non-negative. `10.0`
    ///   is PRISMS-Fatigue's default. Zero reduces the parameter to the plain
    ///   plastic shear strain amplitude.
    /// - `reference_stress` — `sigma_ref` \[Pa\], finite and strictly
    ///   positive; upstream passes the macroscopic yield stress.
    ///
    /// # Errors
    ///
    /// [`FemError::MaterialOutOfRange`] outside those ranges.
    pub fn new(normal_stress_weight: f64, reference_stress: f64) -> Result<Self> {
        if !(normal_stress_weight >= 0.0) || !normal_stress_weight.is_finite() {
            return Err(FemError::MaterialOutOfRange {
                parameter: "normal_stress_weight",
                value: normal_stress_weight,
                unit: "dimensionless",
                reason: "must be finite and non-negative",
            });
        }
        if !(reference_stress > 0.0) || !reference_stress.is_finite() {
            return Err(FemError::MaterialOutOfRange {
                parameter: "reference_stress",
                value: reference_stress,
                unit: "Pa",
                reason: "must be finite and strictly positive",
            });
        }
        Ok(Self {
            normal_stress_weight,
            reference_stress,
        })
    }

    /// PRISMS-Fatigue's own default weight, `k = 10.0`, with the caller's
    /// reference stress \[Pa\].
    ///
    /// # Errors
    ///
    /// As [`FatemiSocie::new`].
    pub fn with_default_weight(reference_stress: f64) -> Result<Self> {
        FatemiSocie::new(10.0, reference_stress)
    }

    /// The weight `k` \[-\].
    #[must_use]
    pub fn normal_stress_weight(&self) -> f64 {
        self.normal_stress_weight
    }

    /// The reference stress `sigma_ref` \[Pa\].
    #[must_use]
    pub fn reference_stress(&self) -> f64 {
        self.reference_stress
    }

    /// Evaluate the parameter on one slip system.
    ///
    /// # Arguments
    ///
    /// - `shear_strain_amplitude` — `delta gamma / 2` \[-\], the half-range of
    ///   the plastic shear strain on this system over the cycle. Taken as an
    ///   absolute value here, so the sign convention of the caller's slip does
    ///   not matter.
    /// - `plane_normal_stress` — `sigma_n` \[Pa\] on this system's slip plane
    ///   at the tension extreme, tension positive. **Clipped at zero**: a
    ///   plane in compression contributes nothing, exactly as upstream's
    ///   `normal_stresses_temp.clip(lower = 0)` does. A compressive normal
    ///   stress does not *reduce* the parameter below the plain shear
    ///   amplitude.
    ///
    /// # Returns
    ///
    /// The FIP \[-\], always non-negative.
    #[must_use]
    pub fn fip(&self, shear_strain_amplitude: f64, plane_normal_stress: f64) -> f64 {
        let normal = plane_normal_stress.max(0.0);
        shear_strain_amplitude.abs()
            * (1.0 + self.normal_stress_weight * normal / self.reference_stress)
    }
}

/// The cycle extremes at one quadrature point: the per-system slip at the most
/// tensile and the most compressive point of a cycle, and the per-system
/// plane-normal stress at the tensile one.
///
/// # How it is driven
///
/// The caller samples this at every load step of the cycle, passing a scalar
/// `indicator` that says how far into tension the *macroscopic* load is — the
/// applied strain, the load factor, the far-field stress, whatever the driver
/// has. The extremes are the samples at which that indicator was largest and
/// smallest. That is upstream's construction: it reads the state from the two
/// files written at the tension and compression peaks of the final cycle.
///
/// Nothing is averaged over the cycle, and no rainflow counting is done. A
/// single indicator with one maximum and one minimum is assumed, i.e. a
/// **constant-amplitude** cycle. A variable-amplitude history needs cycle
/// counting that this type does not do.
///
/// # Units
///
/// Slips dimensionless, stresses in pascals, the indicator whatever the caller
/// chooses (only its ordering is used).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CycleExtremes {
    family: SlipFamily,
    slip_at_peak_tension: [f64; MAX_SLIP_SYSTEMS],
    slip_at_peak_compression: [f64; MAX_SLIP_SYSTEMS],
    normal_stress_at_peak_tension: [f64; MAX_SLIP_SYSTEMS],
    peak_tension_indicator: f64,
    peak_compression_indicator: f64,
    samples: usize,
}

impl CycleExtremes {
    /// An empty accumulator for a point whose crystal uses `family`.
    #[must_use]
    pub fn new(family: SlipFamily) -> Self {
        Self {
            family,
            slip_at_peak_tension: [0.0; MAX_SLIP_SYSTEMS],
            slip_at_peak_compression: [0.0; MAX_SLIP_SYSTEMS],
            normal_stress_at_peak_tension: [0.0; MAX_SLIP_SYSTEMS],
            peak_tension_indicator: f64::NEG_INFINITY,
            peak_compression_indicator: f64::INFINITY,
            samples: 0,
        }
    }

    /// Record one converged load step.
    ///
    /// # Arguments
    ///
    /// - `indicator` — the macroscopic load measure \[caller's units\]; only
    ///   its ordering matters.
    /// - `state` — the committed crystal state at this point.
    /// - `stress` — the converged stress \[Pa\] at this point, in sample axes.
    ///
    /// Call it once per **converged** load step, after
    /// [`crate::assembly::System::commit`] — never inside the Newton loop,
    /// for the same reason `commit` must not be called there.
    pub fn sample(&mut self, indicator: f64, state: &CrystalState, stress: &Voigt6) {
        if indicator > self.peak_tension_indicator {
            self.peak_tension_indicator = indicator;
            self.slip_at_peak_tension = state.slip;
            self.normal_stress_at_peak_tension =
                state.plane_normal_stresses(self.family, stress);
        }
        if indicator < self.peak_compression_indicator {
            self.peak_compression_indicator = indicator;
            self.slip_at_peak_compression = state.slip;
        }
        self.samples += 1;
    }

    /// How many samples have been recorded. Dimensionless count.
    #[must_use]
    pub fn samples(&self) -> usize {
        self.samples
    }

    /// The slip family this accumulator was built for.
    #[must_use]
    pub fn family(&self) -> SlipFamily {
        self.family
    }

    /// The plastic shear strain **amplitude** on each system,
    /// `|gamma^tension - gamma^compression| / 2` \[-\].
    ///
    /// Entries beyond `family.n_systems()` are zero.
    #[must_use]
    pub fn shear_strain_amplitudes(&self) -> [f64; MAX_SLIP_SYSTEMS] {
        let mut out = [0.0_f64; MAX_SLIP_SYSTEMS];
        for a in 0..self.family.n_systems() {
            out[a] = 0.5 * (self.slip_at_peak_tension[a] - self.slip_at_peak_compression[a]).abs();
        }
        out
    }

    /// The stress normal to each slip plane at the tension extreme \[Pa\],
    /// tension positive and **not** clipped — the clipping happens in
    /// [`FatemiSocie::fip`], so this accessor reports what was measured.
    #[must_use]
    pub fn plane_normal_stresses(&self) -> [f64; MAX_SLIP_SYSTEMS] {
        self.normal_stress_at_peak_tension
    }

    /// The Fatemi-Socie parameter on every system \[-\].
    ///
    /// Entries beyond `family.n_systems()` are zero.
    ///
    /// # Errors
    ///
    /// [`FemError::LengthMismatch`] if fewer than two samples have been
    /// recorded, since a cycle with one sample has no range and the answer
    /// would silently be zero.
    pub fn fips(&self, parameter: &FatemiSocie) -> Result<[f64; MAX_SLIP_SYSTEMS]> {
        if self.samples < 2 {
            return Err(FemError::LengthMismatch {
                context: "CycleExtremes::fips: a cycle needs at least two sampled load steps",
                expected: 2,
                actual: self.samples,
            });
        }
        let amplitude = self.shear_strain_amplitudes();
        let mut out = [0.0_f64; MAX_SLIP_SYSTEMS];
        for a in 0..self.family.n_systems() {
            out[a] = parameter.fip(amplitude[a], self.normal_stress_at_peak_tension[a]);
        }
        Ok(out)
    }
}

/// The volume-averaged fatigue indicator parameter of a region, and which slip
/// system attains it.
///
/// # Units
///
/// `fip` dimensionless, `volume` in cubic metres (or square metres per unit
/// thickness in two dimensions).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegionFip {
    /// Index of the slip system carrying the largest averaged parameter.
    pub system: usize,
    /// That system's volume-averaged parameter \[-\].
    pub fip: f64,
    /// The region's total integration weight \[m^3\], reported so that a
    /// caller comparing regions can see whether it is comparing like with
    /// like — upstream's own output records the element count of each band for
    /// exactly that reason.
    pub volume: f64,
}

/// Average the per-system fatigue parameters of a region and return its
/// largest system.
///
/// This is PRISMS-Fatigue's band reduction: average **within** the region,
/// then maximise over slip systems. The order matters and is not
/// interchangeable — maximising first and averaging second would let a single
/// hot point set the region's value, which is exactly the mesh-dependence the
/// averaging exists to remove.
///
/// # Arguments
///
/// - `fips` — one twelve-entry parameter array per point of the region \[-\],
///   as returned by [`CycleExtremes::fips`].
/// - `weights` — the integration weight of each point \[m^3\], as returned by
///   [`crate::assembly::System::quadrature_weights`]. Must be the same length
///   as `fips` and must not sum to zero.
/// - `n_systems` — how many entries of each array are live, normally
///   `family.n_systems()`.
///
/// # Errors
///
/// [`FemError::LengthMismatch`] if the two slices differ in length or the
/// region is empty; [`FemError::MaterialOutOfRange`] if the weights sum to
/// zero or a non-finite number.
pub fn region_fip(
    fips: &[[f64; MAX_SLIP_SYSTEMS]],
    weights: &[f64],
    n_systems: usize,
) -> Result<RegionFip> {
    if fips.len() != weights.len() || fips.is_empty() {
        return Err(FemError::LengthMismatch {
            context: "region_fip: one weight per point, and at least one point",
            expected: fips.len().max(1),
            actual: weights.len(),
        });
    }
    let volume: f64 = weights.iter().sum();
    if !(volume > 0.0) || !volume.is_finite() {
        return Err(FemError::MaterialOutOfRange {
            parameter: "region integration weight",
            value: volume,
            unit: "m^3",
            reason: "must be finite and strictly positive",
        });
    }
    let mut best = RegionFip {
        system: 0,
        fip: f64::NEG_INFINITY,
        volume,
    };
    for a in 0..n_systems.min(MAX_SLIP_SYSTEMS) {
        let mut sum = 0.0;
        for (f, w) in fips.iter().zip(weights.iter()) {
            sum += w * f[a];
        }
        let averaged = sum / volume;
        if averaged > best.fip {
            best.system = a;
            best.fip = averaged;
        }
    }
    Ok(best)
}

/// Rank a set of regions by their volume-averaged parameter, largest first.
///
/// The output is what a fatigue screening actually consumes: the ordering of
/// candidate initiation sites. It is an **ordering**, not a life — see the
/// module documentation.
///
/// # Arguments
///
/// - `regions` — one [`RegionFip`] per region, in the caller's own region
///   order (grain index, band index, whatever the partition is).
///
/// # Returns
///
/// The indices into `regions`, sorted by descending parameter. Ties keep their
/// original relative order, so the result is deterministic.
#[must_use]
pub fn rank_regions(regions: &[RegionFip]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..regions.len()).collect();
    order.sort_by(|a, b| {
        regions[*b]
            .fip
            .partial_cmp(&regions[*a].fip)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    order
}
