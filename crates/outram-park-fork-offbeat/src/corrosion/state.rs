// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to the per-boundary-face state that upstream's
// `corrosion/corrosionModel/zircaloyOuterCorrosion.C::correct(...)` gathers by
// looking `T`, `fastFlux`, `k` and the `oxideThickness`/`DOxideThickness`
// surface fields up on the mesh registry, and to the surface fields
// `corrosion.H` declares as its results.
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Inputs and results of one corrosion step, for **one boundary face**.

use crate::materials::MaterialState;

/// Everything a waterside-corrosion model needs to advance **one boundary
/// face** by one timestep.
///
/// # Why this type exists
///
/// Upstream OFFBEAT's corrosion model reaches into the OpenFOAM mesh registry
/// and looks fields up by name — `"T"`, `"fastFlux"`, `"k"` — plus the
/// timestep from the `Time` object and the previous thickness from a stored
/// old-time field. Its dependencies are invisible until you read the body, and
/// a missing field is a runtime failure. This port inverts that: a corrosion
/// model takes a `CorrosionState`, so its inputs are visible in the signature
/// and the compiler checks that they exist.
///
/// # Units — raw `f64`, strict SI
///
/// Evaluated once per boundary face per timestep, so raw `f64` rather than
/// `uom` quantities. One field is **not** what a reader of upstream would
/// expect, and is called out because getting it wrong is silent:
///
/// - [`fast_flux`](Self::fast_flux) is in **n/(m²·s)**, whereas upstream's
///   `fastFlux` field is in n/(cm²·s). The conversion happens once, inside the
///   correlation.
///
/// # This is the metal/oxide **interface** temperature
///
/// [`interface_temperature`](Self::interface_temperature) is the temperature at
/// the metal/oxide boundary, not at the oxide's outer surface and not in the
/// coolant. Use [`oxide_thermal_coupling`](super::thermal::oxide_thermal_coupling)
/// to obtain it from a surface temperature, a first-cell temperature and the
/// current oxide thickness. Feeding the surface temperature here instead
/// **underestimates** corrosion, increasingly so as the layer thickens, because
/// the insulating oxide makes the interface the hotter of the two.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CorrosionState {
    /// Metal/oxide interface temperature \[K\]. Absolute; must be > 0.
    ///
    /// Typical PWR cladding runs 570–620 K at the interface early in life and
    /// climbs as the oxide insulates.
    pub interface_temperature: f64,

    /// Oxide-layer thickness \[m\] at the **start** of the timestep — upstream's
    /// `oxideThickness.oldTime()`.
    ///
    /// Zero for fresh cladding. A full-life PWR rod reaches 40–100 µm
    /// (`4e-5`–`1e-4` m); regulatory limits are typically around 100 µm.
    pub oxide_thickness: f64,

    /// Fast-neutron flux \[**n/(m²·s)**\], conventionally E > 1 MeV.
    ///
    /// Note the unit: SI, not the n/(cm²·s) upstream's field carries. A
    /// representative PWR value is `7e17` n/(m²·s) (= 7e13 n/(cm²·s)).
    ///
    /// Only the post-transition branch of the low-temperature kinetics uses
    /// this; the high-temperature branch ignores it entirely.
    pub fast_flux: f64,

    /// Timestep \[s\] to advance over.
    ///
    /// Fuel-performance timesteps span an enormous range — a few seconds
    /// through a power ramp, days or weeks through steady irradiation. The
    /// kinetics are integrated in closed form over the whole step, so a long
    /// step is accurate as long as the interface temperature really is roughly
    /// constant across it.
    pub time_step: f64,
}

impl CorrosionState {
    /// Fresh cladding — zero oxide — at `interface_temperature` \[K\], with no
    /// fast flux, over `time_step` \[s\].
    ///
    /// This is the beginning-of-life state, and the state most correlation unit
    /// tests should start from, because a corrosion law's one exactly known
    /// value is that it produces no oxide from nothing in no time.
    ///
    /// ```
    /// use outram_park_fork_offbeat::corrosion::CorrosionState;
    ///
    /// let s = CorrosionState::fresh(600.0, 86_400.0);
    /// assert_eq!(s.oxide_thickness, 0.0);
    /// assert_eq!(s.fast_flux, 0.0);
    /// ```
    #[must_use]
    pub fn fresh(interface_temperature: f64, time_step: f64) -> Self {
        Self {
            interface_temperature,
            oxide_thickness: 0.0,
            fast_flux: 0.0,
            time_step,
        }
    }

    /// A copy of this state advanced to the end of `step`, ready for the next
    /// timestep.
    ///
    /// Only [`oxide_thickness`](Self::oxide_thickness) changes — temperature,
    /// flux and timestep are the caller's to update from the rest of the
    /// simulation. Chaining this is how a life history is integrated:
    ///
    /// ```
    /// use outram_park_fork_offbeat::corrosion::{CorrosionModel, CorrosionState};
    ///
    /// let model = CorrosionModel::zircaloy_outer_default();
    /// let mut state = CorrosionState::fresh(600.0, 86_400.0);
    /// for _ in 0..30 {
    ///     state = state.advanced(&model.step(&state));
    /// }
    /// assert!(state.oxide_thickness > 0.0);
    /// ```
    #[must_use]
    pub fn advanced(&self, step: &CorrosionStep) -> Self {
        Self {
            oxide_thickness: step.oxide_thickness,
            ..*self
        }
    }
}

/// The result of advancing one boundary face by one corrosion timestep.
///
/// Corresponds to the three surface fields upstream's `corrosion` class owns —
/// `oxideThickness`, `DOxideThickness` and `DMetalThickness` — plus the
/// hydrogen ingress that upstream's `oxidePickupFraction` boundary condition
/// computes from the first two.
///
/// # Units — raw `f64`, strict SI except hydrogen
///
/// Lengths in metres. [`hydrogen_pickup`](Self::hydrogen_pickup) is in
/// **wt-ppm**, matching
/// [`MaterialState::hydrogen_content`](crate::materials::MaterialState::hydrogen_content) —
/// a mass fraction times 1e6, which is the unit the hydride literature uses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CorrosionStep {
    /// Oxide-layer thickness \[m\] at the **end** of the step — upstream's
    /// `oxideThickness`.
    pub oxide_thickness: f64,

    /// Increase in oxide thickness \[m\] over the step — upstream's
    /// `DOxideThickness`. Always `>= 0`.
    pub oxide_growth: f64,

    /// Metal wall thickness \[m\] consumed over the step — upstream's
    /// `DMetalThickness`, equal to
    /// [`oxide_growth`](Self::oxide_growth) divided by the Pilling–Bedworth
    /// ratio 1.56. Always `>= 0`.
    ///
    /// **This is the number a moving-mesh driver needs.** Upstream displaces
    /// the boundary points inward by exactly this much each step. This port
    /// does not own a mesh — see the [module documentation](super) on the
    /// deferred layer addition/removal topology changer — so it reports the
    /// displacement and leaves applying it to the caller.
    pub metal_loss: f64,

    /// Increase in the wall-average hydrogen concentration \[wt-ppm\] over the
    /// step.
    ///
    /// Zero when the model carries no hydrogen-pickup submodel. Bounded above
    /// by the hydrogen the reaction actually liberated — see
    /// [`super::hydrogen`] — because the pickup fraction is a fraction.
    pub hydrogen_pickup: f64,
}

impl CorrosionStep {
    /// A step in which nothing happened: no growth, no metal loss, no pickup,
    /// and the oxide still `oxide_thickness` \[m\] thick.
    ///
    /// This is what a [`CorrosionModel::Constant`](super::model::CorrosionModel::Constant)
    /// returns, and what any model returns for a zero timestep.
    #[must_use]
    pub fn unchanged(oxide_thickness: f64) -> Self {
        Self {
            oxide_thickness,
            oxide_growth: 0.0,
            metal_loss: 0.0,
            hydrogen_pickup: 0.0,
        }
    }

    /// Add this step's [`hydrogen_pickup`](Self::hydrogen_pickup) to a cell's
    /// [`MaterialState::hydrogen_content`] \[wt-ppm\].
    ///
    /// This is the one place corrosion writes back into the material state, and
    /// it is why [`MaterialState::hydrogen_content`] exists: downstream
    /// correlations — hydride-embrittlement failure criteria, hydrogen-affected
    /// creep and plasticity laws — read hydrogen content and have no idea where
    /// it came from.
    ///
    /// Note that this is a **wall average**. Real hydrogen is concentrated near
    /// the cold outer wall by thermal diffusion (the Soret effect), which needs
    /// a hydrogen transport solve this port does not contain; a wall average
    /// therefore under-predicts the local concentration at the rim.
    ///
    /// ```
    /// use outram_park_fork_offbeat::corrosion::CorrosionStep;
    /// use outram_park_fork_offbeat::materials::MaterialState;
    ///
    /// let mut material = MaterialState::fresh(600.0);
    /// let step = CorrosionStep {
    ///     oxide_thickness: 1.0e-5,
    ///     oxide_growth: 1.0e-6,
    ///     metal_loss: 1.0e-6 / 1.56,
    ///     hydrogen_pickup: 12.5,
    /// };
    /// step.apply_to(&mut material);
    /// assert_eq!(material.hydrogen_content, 12.5);
    /// ```
    ///
    /// [`MaterialState::hydrogen_content`]: crate::materials::MaterialState::hydrogen_content
    pub fn apply_to(&self, state: &mut MaterialState) {
        state.hydrogen_content += self.hydrogen_pickup;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_state_has_no_oxide_and_no_flux() {
        let s = CorrosionState::fresh(600.0, 86_400.0);
        assert_eq!(s.interface_temperature, 600.0);
        assert_eq!(s.oxide_thickness, 0.0);
        assert_eq!(s.fast_flux, 0.0);
        assert_eq!(s.time_step, 86_400.0);
    }

    #[test]
    fn advancing_carries_only_the_thickness_forward() {
        let mut s = CorrosionState::fresh(600.0, 86_400.0);
        s.fast_flux = 7.0e17;
        let step = CorrosionStep {
            oxide_thickness: 3.0e-6,
            oxide_growth: 3.0e-6,
            metal_loss: 3.0e-6 / 1.56,
            hydrogen_pickup: 4.0,
        };
        let next = s.advanced(&step);
        assert_eq!(next.oxide_thickness, 3.0e-6);
        assert_eq!(next.interface_temperature, s.interface_temperature);
        assert_eq!(next.fast_flux, s.fast_flux);
        assert_eq!(next.time_step, s.time_step);
    }

    #[test]
    fn an_unchanged_step_does_nothing() {
        let step = CorrosionStep::unchanged(5.0e-6);
        assert_eq!(step.oxide_thickness, 5.0e-6);
        assert_eq!(step.oxide_growth, 0.0);
        assert_eq!(step.metal_loss, 0.0);
        assert_eq!(step.hydrogen_pickup, 0.0);

        let mut material = MaterialState::fresh(600.0);
        material.hydrogen_content = 30.0;
        step.apply_to(&mut material);
        assert_eq!(material.hydrogen_content, 30.0);
    }

    /// Self-consistency check: hydrogen pickup accumulates rather than
    /// replacing, because a rod's hydrogen content is the integral of a life's
    /// worth of corrosion.
    #[test]
    fn hydrogen_pickup_accumulates_into_the_material_state() {
        let mut material = MaterialState::fresh(600.0);
        let step = CorrosionStep {
            oxide_thickness: 1.0e-5,
            oxide_growth: 1.0e-6,
            metal_loss: 1.0e-6 / 1.56,
            hydrogen_pickup: 2.5,
        };
        for _ in 0..4 {
            step.apply_to(&mut material);
        }
        assert!((material.hydrogen_content - 10.0).abs() < 1.0e-12);
    }
}
