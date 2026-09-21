// SPDX-License-Identifier: GPL-3.0

//! Combine a released source term with dilution factors into per-receptor
//! air and ground totals.
//!
//! This is the step that turns geometry into numbers:
//!
//! ```text
//! TIC(r, n) = sum over segments of  chi/Q(r, seg, lambda_n) * Q(n, seg)
//! D(r, n)   = v_d(group_n) * TIC_ground(r, n)
//! ```
//!
//! Everything expensive happened in [`super::chi_over_q`]; this is arithmetic
//! over the result, linear in the number of nuclides.
//!
//! # Two receptor sets, paired by index
//!
//! Air concentration is wanted at breathing height; deposition is evaluated at
//! `z = 0`. `TIC` falls off with height, so one receptor set cannot serve both
//! without under-predicting deposition. [`survey`] therefore takes **two**
//! [`super::chi_over_q::DilutionFactors`] — one computed at breathing height,
//! one at ground level — with receptor `r` meaning the same ground position in
//! both. The puff loop dominates the cost either way, and running it twice at
//! different heights is cheaper than getting the pairing wrong.
//!
//! Nothing can check that the two were actually computed at different heights;
//! a `DilutionFactors` does not carry the height it was evaluated at. Passing
//! the same set twice is legal and gives deposition evaluated at breathing
//! height, which under-predicts.

use uom::si::f64::Radioactivity;
use uom::si::radioactivity::becquerel;

use super::chi_over_q::DilutionFactors;
use super::deposition::{dry_deposition, DepositionGroup, DryDepositionVelocity};
use super::source::SourceTerm;
use super::units::{GroundDeposition, TimeIntegratedAirConcentration};

/// One dry deposition velocity per [`DepositionGroup`].
///
/// See [`DryDepositionVelocity`] for why this crate ships no cited table and
/// why the caller is expected to supply values it can justify.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DepositionVelocities {
    /// For inert gases. Physically exactly zero.
    pub noble_gas: DryDepositionVelocity,
    /// For F, Cl, Br, I, At.
    pub halogen: DryDepositionVelocity,
    /// For everything else, including Se and Te.
    pub aerosol: DryDepositionVelocity,
}

impl DepositionVelocities {
    /// **NOT CITED VALUES.** Order-of-magnitude placeholders, one per group —
    /// see [`DryDepositionVelocity::order_of_magnitude_placeholder`], whose
    /// documentation is the one to read before quoting any deposition figure
    /// computed through this.
    #[must_use]
    pub fn order_of_magnitude_placeholder() -> Self {
        Self {
            noble_gas: DryDepositionVelocity::order_of_magnitude_placeholder(
                DepositionGroup::NobleGas,
            ),
            halogen: DryDepositionVelocity::order_of_magnitude_placeholder(
                DepositionGroup::Halogen,
            ),
            aerosol: DryDepositionVelocity::order_of_magnitude_placeholder(
                DepositionGroup::Aerosol,
            ),
        }
    }

    /// The velocity for one group.
    #[must_use]
    pub const fn for_group(&self, group: DepositionGroup) -> DryDepositionVelocity {
        match group {
            DepositionGroup::NobleGas => self.noble_gas,
            DepositionGroup::Halogen => self.halogen,
            DepositionGroup::Aerosol => self.aerosol,
        }
    }
}

/// One nuclide's result at one receptor.
#[derive(Debug, Clone, PartialEq)]
pub struct NuclideTotals {
    /// The nuclide's printing label, copied from the source term.
    pub label: String,
    /// Time-integrated air concentration at **breathing height**, Bq.s/m^3.
    pub air: TimeIntegratedAirConcentration,
    /// Dry ground deposition, Bq/m^2. Dry only, and **not an upper bound** —
    /// see [`super::deposition`].
    pub ground: GroundDeposition,
}

/// Air and ground totals for every receptor and nuclide.
///
/// Indexed `[receptor][nuclide]`, in the order the receptors were given to
/// [`super::chi_over_q::dilution_factors`] and the nuclides to
/// [`SourceTerm`].
#[derive(Debug, Clone, PartialEq)]
pub struct SiteSurvey {
    per_receptor: Vec<Vec<NuclideTotals>>,
}

impl SiteSurvey {
    /// Number of receptors.
    #[must_use]
    pub fn n_receptors(&self) -> usize {
        self.per_receptor.len()
    }

    /// Every nuclide's totals at one receptor.
    ///
    /// # Panics
    /// Panics if `receptor` is out of range.
    #[must_use]
    pub fn at(&self, receptor: usize) -> &[NuclideTotals] {
        &self.per_receptor[receptor]
    }

    /// Air concentration summed over every nuclide at one receptor.
    ///
    /// Summing activity across nuclides is meaningful only as a gross
    /// inventory figure — a becquerel of Kr-85 and a becquerel of Cs-137 are
    /// not interchangeable in any consequence sense. Report the per-nuclide
    /// breakdown alongside it.
    ///
    /// # Panics
    /// Panics if `receptor` is out of range.
    #[must_use]
    pub fn total_air(&self, receptor: usize) -> TimeIntegratedAirConcentration {
        self.per_receptor[receptor].iter().map(|n| n.air).sum()
    }

    /// Ground deposition summed over every nuclide at one receptor. The same
    /// caveat as [`Self::total_air`] applies.
    ///
    /// # Panics
    /// Panics if `receptor` is out of range.
    #[must_use]
    pub fn total_ground(&self, receptor: usize) -> GroundDeposition {
        self.per_receptor[receptor].iter().map(|n| n.ground).sum()
    }
}

/// Scale dilution factors by a source term, per nuclide, and deposit.
///
/// # Arguments
/// - `source` — what was released, over which windows.
/// - `air_factors` — dilution factors at **breathing height**.
/// - `ground_factors` — dilution factors at **`z = 0`**, for deposition. See
///   the module docs on why these are separate.
/// - `velocities` — one dry deposition velocity per group, supplied by the
///   caller.
///
/// # Panics
/// Panics if the two factor sets disagree on the number of receptors or
/// segments, or if either has a different number of segments than the source
/// term has release windows.
#[must_use]
pub fn survey(
    source: &SourceTerm,
    air_factors: &DilutionFactors,
    ground_factors: &DilutionFactors,
    velocities: &DepositionVelocities,
) -> SiteSurvey {
    source.validate();
    assert_eq!(
        air_factors.n_receptors(),
        ground_factors.n_receptors(),
        "the breathing-height and ground-level factor sets must cover the same receptors"
    );
    assert_eq!(
        air_factors.n_segments(),
        ground_factors.n_segments(),
        "the two factor sets must share a segmentation"
    );
    assert_eq!(
        air_factors.n_segments(),
        source.windows.len(),
        "the dilution factors have {} segments but the source term has {} release windows; \
         they must be built from the same segmentation (see SourceTerm::segment_boundaries)",
        air_factors.n_segments(),
        source.windows.len()
    );

    let mut per_receptor = Vec::with_capacity(air_factors.n_receptors());
    for r in 0..air_factors.n_receptors() {
        let mut row = Vec::with_capacity(source.nuclides.len());
        for n in &source.nuclides {
            let mut air = TimeIntegratedAirConcentration::default();
            let mut ground_air = TimeIntegratedAirConcentration::default();
            for (seg, released) in n.released.iter().enumerate() {
                if released.get::<becquerel>() == 0.0 {
                    continue;
                }
                air += air_factors.dilution(r, seg, n.decay_constant) * *released;
                ground_air += ground_factors.dilution(r, seg, n.decay_constant) * *released;
            }
            row.push(NuclideTotals {
                label: n.label.clone(),
                air,
                ground: dry_deposition(ground_air, velocities.for_group(n.deposition_group)),
            });
        }
        per_receptor.push(row);
    }
    SiteSurvey { per_receptor }
}

/// A dimensionless helper for reporting: how much of a nuclide's release
/// survived to a receptor, relative to a stable nuclide released identically.
///
/// Useful for showing the effect of decay in transit without quoting two
/// absolute numbers. Returns `None` when the stable reference is zero.
#[must_use]
pub fn surviving_fraction_against_stable(
    decayed: TimeIntegratedAirConcentration,
    stable: TimeIntegratedAirConcentration,
) -> Option<f64> {
    let s = stable.becquerel_seconds_per_cubic_meter();
    if s == 0.0 {
        return None;
    }
    Some(decayed.becquerel_seconds_per_cubic_meter() / s)
}

/// Total activity released across every window and nuclide.
///
/// The same "a becquerel is not a becquerel" caveat as [`SiteSurvey::total_air`]
/// applies — this is an inventory figure, not a consequence one.
#[must_use]
pub fn total_released(source: &SourceTerm) -> Radioactivity {
    Radioactivity::new::<becquerel>(
        source
            .nuclides
            .iter()
            .map(|n| n.total_released().get::<becquerel>())
            .sum::<f64>(),
    )
}
