// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to the per-cell state that upstream's
// `materialModel/.../correct(scalarField&, const scalarField& T, const labelList&)`
// gathers by looking fields up on the mesh registry.
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! The per-cell state every material correlation is evaluated against.

/// Everything a material property correlation may depend on, for **one cell**.
///
/// # Why this type exists
///
/// Upstream OFFBEAT correlations reach into the OpenFOAM mesh registry and look
/// up whatever fields they need by name — `"burnup"`, `"porosity"`,
/// `"Intragranular_gas_swelling"` and so on — so the dependencies of a given
/// correlation are invisible until you read its body, and a missing field is a
/// runtime failure. This port inverts that: every correlation takes a
/// `MaterialState`, so its inputs are visible in the signature and the compiler
/// checks that they exist.
///
/// # Units — raw `f64`, strict SI unless stated
///
/// This struct is evaluated once per cell per property per timestep, deep inside
/// the numerical loops, so it carries raw `f64` rather than `uom` quantities.
/// Every field documents its unit; the two that are **not** plain SI are called
/// out explicitly because getting them wrong is the classic fuel-performance
/// error:
///
/// - [`burnup`](Self::burnup) is in **MWd/kgHM**, not J/kg.
/// - [`fast_fluence`](Self::fast_fluence) is in **n/m²** with E > 1 MeV.
///
/// # Defaults
///
/// [`MaterialState::fresh`] gives unirradiated material at a chosen temperature:
/// zero burnup, zero fluence, zero swelling and densification, fully dense,
/// stoichiometric. Build a state with that and override what the case needs,
/// rather than filling twelve fields by hand.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialState {
    /// Temperature \[K\]. Absolute; must be > 0.
    pub temperature: f64,

    /// Burnup \[**MWd/kgHM**\] — megawatt-days per kilogram of initial heavy
    /// metal.
    ///
    /// Note the unit. Upstream stores burnup in J/kgHM on the mesh and each
    /// correlation converts locally (`Bu/1000/0.881` appears throughout, which
    /// is J/kgHM → MWd/kgU for a UO2 heavy-metal fraction of 0.881). That
    /// conversion is done **once**, at the boundary of this crate, so
    /// correlations receive MWd/kgHM directly and cannot each get it slightly
    /// wrong.
    pub burnup: f64,

    /// Fast-neutron fluence \[n/m²\], conventionally for E > 1 MeV.
    ///
    /// Drives irradiation hardening, irradiation creep and — for TRISO coating
    /// layers — anisotropic dimensional change in pyrolytic carbon.
    pub fast_fluence: f64,

    /// Porosity \[-\], the void volume fraction in `[0, 1)`.
    ///
    /// As-fabricated porosity plus its in-service evolution. Fuel conductivity
    /// falls steeply with porosity, so this is a first-order input, not a
    /// correction.
    pub porosity: f64,

    /// Volumetric swelling strain \[-\] from solid and gaseous fission products.
    ///
    /// Volumetric, not linear: divide by three for the linear equivalent.
    pub swelling: f64,

    /// Volumetric densification strain \[-\], **negative** for the usual
    /// early-life sintering of as-fabricated porosity.
    pub densification: f64,

    /// Plutonium fraction \[-\] of the heavy metal, `Pu/(U+Pu)`.
    ///
    /// Zero for UO2, ~0.05–0.1 for LWR MOX, higher for fast-reactor fuel.
    pub pu_fraction: f64,

    /// Deviation from stoichiometry \[-\], the `x` in `(U,Pu)O_{2+x}`.
    ///
    /// Negative is hypostoichiometric (oxygen-deficient), which is the normal
    /// condition for fast-reactor MOX. Strongly affects conductivity and
    /// melting temperature.
    pub oxygen_deviation: f64,

    /// Gadolinia (Gd2O3) weight fraction \[-\] in the fuel, for burnable-poison
    /// fuel. Zero for unpoisoned UO2.
    pub gadolinia_fraction: f64,

    /// Cold work \[-\] in the cladding, the retained cold-work fraction from
    /// fabrication. Zero for fully recrystallised material.
    pub cold_work: f64,

    /// Oxygen concentration \[-\] in Zircaloy as a weight fraction, raised by
    /// waterside oxidation and by oxygen uptake at high temperature.
    pub oxygen_content: f64,

    /// Hydrogen concentration \[wt-ppm\] in the cladding, from corrosion-driven
    /// hydrogen pickup. Drives hydride embrittlement.
    pub hydrogen_content: f64,
}

impl MaterialState {
    /// Unirradiated, as-fabricated, fully dense, stoichiometric material at
    /// `temperature` \[K\].
    ///
    /// Every irradiation-history field is zero: no burnup, no fluence, no
    /// swelling, no densification, no cold work, no absorbed oxygen or
    /// hydrogen; `porosity` is zero and the fuel is pure UO2 (`pu_fraction` and
    /// `gadolinia_fraction` zero, `oxygen_deviation` zero).
    ///
    /// This is the state to build a beginning-of-life case from, and the state
    /// most correlation unit tests should use, because it is the condition the
    /// published reference values for a fresh material are quoted at.
    ///
    /// ```
    /// use outram_park_fork_offbeat::materials::MaterialState;
    ///
    /// let s = MaterialState::fresh(300.0);
    /// assert_eq!(s.temperature, 300.0);
    /// assert_eq!(s.burnup, 0.0);
    /// assert_eq!(s.porosity, 0.0);
    /// ```
    #[must_use]
    pub fn fresh(temperature: f64) -> Self {
        Self {
            temperature,
            burnup: 0.0,
            fast_fluence: 0.0,
            porosity: 0.0,
            swelling: 0.0,
            densification: 0.0,
            pu_fraction: 0.0,
            oxygen_deviation: 0.0,
            gadolinia_fraction: 0.0,
            cold_work: 0.0,
            oxygen_content: 0.0,
            hydrogen_content: 0.0,
        }
    }

    /// Fraction of theoretical density \[-\], i.e. `1 - porosity`.
    ///
    /// Clamped below at 0.05 to match upstream's 95%-porosity cutoff, which
    /// exists to stop porosity-correction factors blowing up in a nearly-void
    /// cell rather than to describe real material.
    #[must_use]
    pub fn density_fraction(&self) -> f64 {
        (1.0 - self.porosity).max(0.05)
    }

    /// Linear (one-dimensional) swelling strain \[-\], i.e. `swelling / 3`.
    ///
    /// Provided because correlations quote swelling both ways and the factor of
    /// three is easy to drop.
    #[must_use]
    pub fn linear_swelling(&self) -> f64 {
        self.swelling / 3.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_state_is_unirradiated_and_fully_dense() {
        let s = MaterialState::fresh(600.0);
        assert_eq!(s.temperature, 600.0);
        assert_eq!(s.burnup, 0.0);
        assert_eq!(s.fast_fluence, 0.0);
        assert_eq!(s.density_fraction(), 1.0);
    }

    #[test]
    fn density_fraction_is_clamped_at_the_upstream_cutoff() {
        let mut s = MaterialState::fresh(600.0);
        s.porosity = 0.99;
        // 1 - 0.99 = 0.01, below the 0.05 floor.
        assert_eq!(s.density_fraction(), 0.05);
    }

    #[test]
    fn linear_swelling_is_one_third_of_volumetric() {
        let mut s = MaterialState::fresh(600.0);
        s.swelling = 0.03;
        assert!((s.linear_swelling() - 0.01).abs() < 1e-15);
    }
}
