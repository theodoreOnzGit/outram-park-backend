// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream
// `offbeatLib/materials/materialModel/behavioralModels/relocation/`:
//   relocationModel.{C,H}    -> RelocationModel::Zero
//   relocationFRAPCON.{C,H}  -> RelocationModel::Uo2Frapcon
//     (specifically `relocationFRAPCON::setRelocation`; the surrounding
//      slice-averaging, gap-penetration and recovery logic is not a material
//      correlation and is not ported — see the module documentation.)
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Relocation models — outward movement of cracked fuel fragments,
//! **radial strain** \[-\].
//!
//! # What relocation is
//!
//! A UO2 pellet at power has a very steep radial temperature profile: hundreds
//! of kelvin between its centre and its rim across a few millimetres. The
//! resulting thermal stress cracks it, radially, within the first hours of
//! operation. The fragments then slide outward into the fuel/cladding gap.
//!
//! Nothing has swollen — the fuel's *volume* is essentially unchanged — but the
//! pellet's effective outer radius has grown and part of the gap has closed.
//! That matters a great deal, because the gap dominates the fuel's thermal
//! resistance: a pellet that has relocated runs cooler than one that has not,
//! at the same power.
//!
//! # UNITS AND SIGN CONVENTION — read this before using the number
//!
//! Upstream mixes two conventions in one model — a **percentage gap closure**
//! internally and a **dimensionless radial strain** in the field it stores — so
//! this port names all three quantities separately and never returns the
//! ambiguous one:
//!
//! | Method | Quantity | Unit | Sign |
//! |---|---|---|---|
//! | [`value`](RelocationModel::value) | radial relocation **strain** `ε` — upstream's `epsilonRelocation` | \[-\] | **positive = outward**, closing the gap |
//! | [`gap_closure_fraction`](RelocationModel::gap_closure_fraction) | fraction `f` of the as-fabricated cold gap closed | \[-\], in `[0, 1]` | positive |
//! | [`radial_displacement`](RelocationModel::radial_displacement) | outward movement of the pellet surface `Δr` | \[m\] | positive = outward |
//!
//! They are related by upstream's algebra exactly as
//!
//! `ε = f · (G_cold / D_cold)` and `Δr = ε · D_cold / 2 = f · G_cold / 2`
//!
//! where `G_cold` is [`cold_gap`](RelocationModel::Uo2Frapcon::cold_gap) and
//! `D_cold` is
//! [`cold_pellet_diameter`](RelocationModel::Uo2Frapcon::cold_pellet_diameter).
//!
//! **`cold_gap` is read here as the DIAMETRAL gap.** Upstream's header
//! documents it only as "Cold Gap Reference Thickness \[m\]", which is
//! ambiguous. Taking it as diametral — consistent with `DiamCold` being a
//! diameter — makes the algebra self-consistent: `Δr = f · G_cold/2` is then
//! `f` times the *radial* gap, so `f` is exactly the fraction of the radial gap
//! that relocation closes, and `f = 1` closes it completely. Under the other
//! reading, full closure would move the pellet only half way across the gap,
//! which no relocation model means. If your input deck quotes a radial gap,
//! double it.
//!
//! # Relocation is NOT a volumetric strain
//!
//! [`SwellingModel`](super::swelling::SwellingModel) and
//! [`DensificationModel`](super::densification::DensificationModel) return
//! volumetric strains that are summed together. **Relocation is not one of
//! them.** It is a radial displacement of a cracked, essentially
//! constant-volume body, and it acts only on the fuel's outer surface and only
//! in the radial direction. Adding it to a volumetric strain sum is a modelling
//! error, not a unit error, and nothing will complain.
//!
//! # What is not ported
//!
//! Upstream's `relocationFRAPCON::correct` does four things around the
//! correlation, none of which is a material property, and none of which is here:
//!
//! - **Slice averaging.** It averages power and burnup over an axial slice of
//!   the mesh and takes the minimum gap width on the slice. That is mesh
//!   machinery; a caller passes the slice-averaged values in.
//! - **The ratchet.** `relocation = max(old, new)` — relocation is not allowed
//!   to decrease. These variants are pure functions with no history; because
//!   the correlation is monotonically non-decreasing in burnup at fixed power,
//!   evaluating it afresh matches the ratchet on a rising-burnup, constant-power
//!   history. On a **power ramp down** it does not, and the caller must apply
//!   the `max` itself.
//! - **Relocation recovery.** When hard pellet/cladding contact would make the
//!   relocated fuel penetrate the cladding, upstream recovers part of the
//!   relocation (`recoveryFraction`, `relaxRecovery`) using the gap width and
//!   the previous timestep's recovered strain. That is a contact-mechanics
//!   feedback loop over history and gap state, not a correlation; it belongs
//!   with [`crate::gap`] and is deferred.
//! - **Sensitivity-analysis scaling.** Upstream's `F_epsilonRelocation` and
//!   `delta_epsilonRelocation` multiply and offset the result for uncertainty
//!   studies. A caller wanting that applies it to the returned number.
//!
//! # Validity ranges: `value` clamps, `value_checked` refuses
//!
//! [`value`](RelocationModel::value) **clamps** burnup to the endpoints of the
//! stated validity range before evaluating.
//! [`value_checked`](RelocationModel::value_checked) returns
//! [`OffbeatError::OutOfRange`] instead, and additionally rejects an
//! unphysical geometry with [`OffbeatError::Unphysical`]. Upstream clamps
//! nothing. The range is **this port's stated applicability**, not an upstream
//! constant.
//!
//! # Status
//!
//! AI-assisted translation, reviewed by no human yet. Per `RESPONSIBLE_USE.md`
//! this is untrusted draft material: the tests below establish internal
//! consistency with upstream's algebra, **not** validation against measured
//! gap-closure data.
//!
//! [`OffbeatError::OutOfRange`]: crate::error::OffbeatError::OutOfRange
//! [`OffbeatError::Unphysical`]: crate::error::OffbeatError::Unphysical

use crate::error::{OffbeatError, Result};
use crate::materials::MaterialState;

/// Which of the two FRAPCON relocation formulations to evaluate — upstream's
/// `modifiedRelocationModel` switch.
///
/// The two are genuinely different fits, not a refinement of one another: at
/// beginning of life they differ by roughly a factor of six in gap closure. The
/// switch defaults to [`Modified`](Self::Modified) upstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FrapconRelocationForm {
    /// The modified relocation model used from FRAPCON 3.5 onwards —
    /// upstream's `modifiedRelocationModel true`, the default.
    ///
    /// Gap closure `f = 0.055 + min(R, R·(0.5795 + 0.2447·ln Bu))` for burnup
    /// above 0.0937 MWd/kgHM, and `f = 0.055` below it, with the power-dependent
    /// amplitude
    ///
    /// - `R = 0.345` for `q' < 20` kW/m,
    /// - `R = 0.345 + (q' − 20)/200` for `20 ≤ q' ≤ 40` kW/m,
    /// - `R = 0.445` for `q' > 40` kW/m.
    ///
    /// The logarithmic burnup term reaches 1 at `Bu = 5.576` MWd/kgHM, above
    /// which the `min` freezes `f` at `0.055 + R` — relocation is complete
    /// early in life and does not evolve further. The 0.0937 MWd/kgHM cut-off
    /// is not arbitrary: it is the burnup at which the logarithmic term crosses
    /// zero, so the two branches meet (a residual step of about 5e-5 in `f`
    /// remains — measured in a unit test below).
    #[default]
    Modified,

    /// The earlier FRAPCON relocation model (the GT2R2-derived form used before
    /// FRAPCON 3.5) — upstream's `modifiedRelocationModel false`.
    ///
    /// Gap closure as a percentage, with `F_Bu = min(Bu/5, 1)` and
    /// `P = (q' − 20)·5/20`:
    ///
    /// - `q' < 20` kW/m: `100·f = 30 + 10·F_Bu`
    /// - `20 ≤ q' < 40` kW/m: `100·f = 28 + P + (12 + P)·F_Bu`
    /// - `q' ≥ 40` kW/m: `100·f = 32 + 18·F_Bu`
    ///
    /// so `f` runs from 0.28–0.32 fresh to 0.40–0.50 by 5 MWd/kgHM, and is
    /// frozen thereafter. **This form is discontinuous in power at the branch
    /// boundaries** — at exactly 20 kW/m the first branch gives `0.30 + 0.10
    /// F_Bu` and the second `0.28 + 0.12 F_Bu`. Upstream is discontinuous
    /// there too; it is reproduced rather than smoothed, so that a comparison
    /// against an OFFBEAT run is not silently shifted.
    Legacy,
}

/// Fuel-fragment relocation correlations — **positive** radial strain \[-\] for
/// fuel moving outward into the gap.
///
/// One variant per model compiled by upstream OFFBEAT's
/// `behavioralModels/relocation/`. Dispatch is by `match`, never by a trait
/// object, per the workspace `CLAUDE.md` "No trait objects" rule.
///
/// Read the [module documentation](self) for the unit and sign convention
/// before using [`value`](Self::value) — upstream mixes a percentage gap
/// closure with a radial strain, and this port separates them into three named
/// methods.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RelocationModel {
    /// No relocation — upstream `relocationModel`, `TypeName("none")`.
    ///
    /// Selecting this upstream still creates the `epsilonRelocation` and
    /// `epsilonRecoveredRelocation` fields and leaves them at zero. Returns
    /// exactly `0.0` at every state; no validity range.
    ///
    /// Choosing this is a real modelling decision, not a null one: without
    /// relocation the gap stays open, the fuel runs several hundred kelvin
    /// hotter at beginning of life, and a fuel-temperature comparison against
    /// measured data will be visibly wrong.
    Zero,

    /// UO2 relocation, FRAPCON form — upstream `relocationFRAPCON`,
    /// `TypeName("UO2FRAPCON")`.
    ///
    /// Empirical gap closure as a function of linear power and burnup, in
    /// either of the two formulations of [`FrapconRelocationForm`], converted
    /// to a radial strain by `ε = f · (G_cold / D_cold)`.
    ///
    /// # Why the geometry and the power live on the variant
    ///
    /// [`MaterialState`](crate::materials::MaterialState) carries the local
    /// thermodynamic and irradiation state, not rod geometry or rod power.
    /// Relocation needs all three, so the cold gap, the cold pellet diameter
    /// and the linear power sit on this variant. The first two are fixed for a
    /// given rod design; **`linear_power` is not** — it changes through life,
    /// and this variant must be reconstructed when it does.
    ///
    /// Valid range: burnup `0` to `120` MWd/kgHM. Linear power is not clamped —
    /// both formulations saturate outside 20–40 kW/m by construction — but a
    /// negative power is rejected by
    /// [`value_checked`](Self::value_checked).
    Uo2Frapcon {
        /// As-fabricated **diametral** fuel/cladding gap \[m\] at cold
        /// conditions — upstream's `GapCold`. See the [module
        /// documentation](self) on why it is read as diametral; if your input
        /// deck quotes a radial gap, double it. Typical LWR value: 1.7e-4 m.
        cold_gap: f64,
        /// As-fabricated pellet diameter \[m\] at cold conditions — upstream's
        /// `DiamCold`. Typical LWR value: 8.2e-3 m.
        cold_pellet_diameter: f64,
        /// Rod linear power \[W/m\] — the slice-averaged `q'` the correlation
        /// branches on, converted to kW/m internally.
        ///
        /// Upstream derives it from the volumetric heat source `Q` \[W/m³\] as
        /// `q' = Q · π · (D_cold/2)²`, so a caller holding a volumetric source
        /// must apply that conversion. Typical LWR value: 2.0e4 W/m
        /// (20 kW/m).
        linear_power: f64,
        /// Which of the two FRAPCON formulations to evaluate.
        form: FrapconRelocationForm,
    },
}

/// Burnup validity range \[MWd/kgHM\] of the FRAPCON relocation correlation, as
/// stated by this port. Upstream states none.
const BURNUP_RANGE: (f64, f64) = (0.0, 120.0);

/// Burnup \[MWd/kgHM\] below which the modified form's logarithmic term would
/// go negative, and upstream substitutes a flat 5.5% gap closure.
const MODIFIED_LOW_BURNUP_CUTOFF: f64 = 0.0937;

impl RelocationModel {
    /// Radial relocation **strain** `ε` \[-\] — upstream's `epsilonRelocation`.
    ///
    /// **Positive means the fuel has moved outward**, closing part of the gap.
    /// The strain is referred to the cold pellet radius, so the outward
    /// movement of the pellet surface is `ε · D_cold / 2` — use
    /// [`radial_displacement`](Self::radial_displacement) rather than doing
    /// that multiplication at the call site.
    ///
    /// This is **not** a volumetric strain and must not be summed with
    /// swelling or densification; see the [module documentation](self).
    ///
    /// **Burnup is clamped** to the stated validity range before evaluation, so
    /// this always returns a finite number, and returns the endpoint value
    /// rather than an extrapolation outside the range — which is *not* what
    /// upstream does; upstream extrapolates. Use
    /// [`value_checked`](Self::value_checked) when you need to know that the
    /// clamp fired. An unphysical geometry (non-positive diameter) yields
    /// `0.0` here and an error from `value_checked`.
    ///
    /// ```
    /// use outram_park_fork_offbeat::materials::MaterialState;
    /// use outram_park_fork_offbeat::materials::behavioral::relocation::{
    ///     FrapconRelocationForm, RelocationModel,
    /// };
    ///
    /// let model = RelocationModel::Uo2Frapcon {
    ///     cold_gap: 1.7e-4,
    ///     cold_pellet_diameter: 8.2e-3,
    ///     linear_power: 2.0e4,
    ///     form: FrapconRelocationForm::Modified,
    /// };
    ///
    /// // Relocation starts immediately — the pellet cracks in the first hours.
    /// let fresh = MaterialState::fresh(900.0);
    /// assert!(model.value(&fresh) > 0.0);
    ///
    /// // It never closes more than the physical gap.
    /// assert!(model.radial_displacement(&fresh) <= 1.7e-4 / 2.0);
    /// ```
    #[must_use]
    pub fn value(&self, state: &MaterialState) -> f64 {
        match self {
            Self::Zero => 0.0,
            Self::Uo2Frapcon {
                cold_gap,
                cold_pellet_diameter,
                ..
            } => {
                if cold_pellet_diameter.is_nan()
                    || *cold_pellet_diameter <= 0.0
                    || !cold_gap.is_finite()
                {
                    return 0.0;
                }
                self.gap_closure_fraction(state) * (cold_gap / cold_pellet_diameter)
            }
        }
    }

    /// Fraction `f` \[-\] of the as-fabricated cold gap that relocation has
    /// closed, in `[0, 1]` — upstream's `deltaGap/100`.
    ///
    /// This is the quantity the FRAPCON correlation is actually fitted in, and
    /// the one to compare against the published model. It depends only on
    /// burnup, linear power and the formulation — not on the geometry.
    ///
    /// Neither formulation can exceed `f = 0.5`, so **relocation never closes
    /// more than half the cold gap**; that bound is asserted by a unit test
    /// below.
    ///
    /// Burnup is clamped exactly as in [`value`](Self::value).
    #[must_use]
    pub fn gap_closure_fraction(&self, state: &MaterialState) -> f64 {
        match self {
            Self::Zero => 0.0,
            Self::Uo2Frapcon {
                linear_power, form, ..
            } => {
                let burnup = state.burnup.clamp(BURNUP_RANGE.0, BURNUP_RANGE.1);
                // Upstream branches on the linear power in kW/m.
                let q_prime = linear_power / 1000.0;
                match form {
                    FrapconRelocationForm::Modified => modified_gap_closure(burnup, q_prime),
                    FrapconRelocationForm::Legacy => legacy_gap_closure(burnup, q_prime),
                }
            }
        }
    }

    /// Outward movement `Δr` \[m\] of the pellet outer surface caused by
    /// relocation — `value · D_cold / 2`, equivalently
    /// `gap_closure_fraction · G_cold / 2`.
    ///
    /// **Positive is outward.** With `cold_gap` read as the diametral gap (see
    /// the [module documentation](self)), `G_cold/2` is the radial gap, so this
    /// is the gap-closure fraction times the radial gap and is bounded above by
    /// it.
    ///
    /// Burnup is clamped exactly as in [`value`](Self::value).
    #[must_use]
    pub fn radial_displacement(&self, state: &MaterialState) -> f64 {
        match self {
            Self::Zero => 0.0,
            Self::Uo2Frapcon {
                cold_pellet_diameter,
                ..
            } => self.value(state) * cold_pellet_diameter / 2.0,
        }
    }

    /// [`value`](Self::value), but returning an error instead of clamping or
    /// silently returning zero.
    ///
    /// # Errors
    ///
    /// - [`OffbeatError::OutOfRange`] when burnup lies outside `0` to `120`
    ///   MWd/kgHM.
    /// - [`OffbeatError::Unphysical`] when the geometry or the power is
    ///   impossible: a non-positive pellet diameter, a negative cold gap, or a
    ///   negative linear power.
    pub fn value_checked(&self, state: &MaterialState) -> Result<f64> {
        match self {
            Self::Zero => Ok(0.0),
            Self::Uo2Frapcon {
                cold_gap,
                cold_pellet_diameter,
                linear_power,
                ..
            } => {
                if state.burnup < BURNUP_RANGE.0 || state.burnup > BURNUP_RANGE.1 {
                    return Err(OffbeatError::OutOfRange {
                        quantity: "UO2 FRAPCON relocation",
                        value: state.burnup,
                        low: BURNUP_RANGE.0,
                        high: BURNUP_RANGE.1,
                        unit: "MWd/kgHM",
                    });
                }
                if cold_pellet_diameter.is_nan() || *cold_pellet_diameter <= 0.0 {
                    return Err(OffbeatError::Unphysical {
                        quantity: "cold pellet diameter",
                        value: *cold_pellet_diameter,
                        unit: "m",
                        reason: "a pellet diameter must be strictly positive",
                    });
                }
                if cold_gap.is_nan() || *cold_gap < 0.0 {
                    return Err(OffbeatError::Unphysical {
                        quantity: "cold diametral gap",
                        value: *cold_gap,
                        unit: "m",
                        reason: "a fuel/cladding gap cannot be negative",
                    });
                }
                if linear_power.is_nan() || *linear_power < 0.0 {
                    return Err(OffbeatError::Unphysical {
                        quantity: "rod linear power",
                        value: *linear_power,
                        unit: "W/m",
                        reason: "linear power cannot be negative",
                    });
                }
                Ok(self.value(state))
            }
        }
    }

    /// [`gap_closure_fraction`](Self::gap_closure_fraction), but returning an
    /// error instead of clamping.
    ///
    /// # Errors
    ///
    /// The same errors as [`value_checked`](Self::value_checked).
    pub fn gap_closure_fraction_checked(&self, state: &MaterialState) -> Result<f64> {
        self.value_checked(state)?;
        Ok(self.gap_closure_fraction(state))
    }

    /// [`radial_displacement`](Self::radial_displacement), but returning an
    /// error instead of clamping.
    ///
    /// # Errors
    ///
    /// The same errors as [`value_checked`](Self::value_checked).
    pub fn radial_displacement_checked(&self, state: &MaterialState) -> Result<f64> {
        self.value_checked(state)?;
        Ok(self.radial_displacement(state))
    }
}

/// Gap-closure fraction `f` \[-\] of the FRAPCON 3.5 modified relocation model.
///
/// `burnup` in MWd/kgHM, `q_prime` in kW/m. See
/// [`FrapconRelocationForm::Modified`] for the formulation. Always in
/// `[0.055, 0.5]`.
fn modified_gap_closure(burnup: f64, q_prime: f64) -> f64 {
    /// The floor of the correlation: 5.5% of the gap is closed as soon as the
    /// pellet cracks, independent of power and burnup.
    const FLOOR: f64 = 0.055;

    if burnup < MODIFIED_LOW_BURNUP_CUTOFF {
        return FLOOR;
    }
    let amplitude = if q_prime < 20.0 {
        0.345
    } else if q_prime <= 40.0 {
        0.345 + (q_prime - 20.0) / 200.0
    } else {
        0.445
    };
    let burnup_term = 0.5795 + 0.2447 * burnup.ln();
    FLOOR + amplitude.min(amplitude * burnup_term)
}

/// Gap-closure fraction `f` \[-\] of the earlier (GT2R2-derived) FRAPCON
/// relocation model.
///
/// `burnup` in MWd/kgHM, `q_prime` in kW/m. See
/// [`FrapconRelocationForm::Legacy`] for the formulation, including its
/// discontinuities in power. Always in `[0.28, 0.50]`.
fn legacy_gap_closure(burnup: f64, q_prime: f64) -> f64 {
    let power_factor = (q_prime - 20.0) * 5.0 / 20.0;
    let burnup_factor = (burnup / 5.0).min(1.0);

    // Upstream's `+ 1e-6` slack on the branch boundaries, kept verbatim.
    let percent = if q_prime < 20.0 + 1.0e-6 {
        30.0 + 10.0 * burnup_factor
    } else if q_prime < 40.0 + 1.0e-6 {
        28.0 + power_factor + (12.0 + power_factor) * burnup_factor
    } else {
        32.0 + 18.0 * burnup_factor
    };
    percent / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Representative LWR rod geometry: 8.2 mm pellet, 170 µm diametral gap,
    /// 20 kW/m.
    const COLD_GAP: f64 = 1.7e-4;
    const COLD_DIAMETER: f64 = 8.2e-3;

    fn model(linear_power: f64, form: FrapconRelocationForm) -> RelocationModel {
        RelocationModel::Uo2Frapcon {
            cold_gap: COLD_GAP,
            cold_pellet_diameter: COLD_DIAMETER,
            linear_power,
            form,
        }
    }

    fn at(burnup: f64) -> MaterialState {
        let mut state = MaterialState::fresh(900.0);
        state.burnup = burnup;
        state
    }

    #[test]
    fn zero_model_never_relocates() {
        let model = RelocationModel::Zero;
        assert_eq!(model.value(&at(50.0)), 0.0);
        assert_eq!(model.gap_closure_fraction(&at(50.0)), 0.0);
        assert_eq!(model.radial_displacement(&at(50.0)), 0.0);
        assert_eq!(model.value_checked(&at(50.0)).unwrap(), 0.0);
    }

    /// **Self-consistency check against the physical gap, not external
    /// validation.** Relocation moves cracked fragments into an existing gap;
    /// it can never move them further than the gap is wide. Both FRAPCON
    /// formulations bound the closure fraction below 0.5 by construction, so
    /// the pellet surface must never move more than half the cold diametral
    /// gap — i.e. never more than the radial gap.
    ///
    /// Swept over the whole validity range in burnup and 0–60 kW/m in power.
    ///
    /// # Results (measured 2026-07-29, this port)
    ///
    /// Maximum closure fraction over the sweep: `0.500` (legacy form, ≥ 40
    /// kW/m, ≥ 5 MWd/kgHM); maximum radial displacement `4.250e-5` m against a
    /// radial gap of `8.500e-5` m.
    #[test]
    fn relocation_is_bounded_by_the_physical_gap() {
        for form in [
            FrapconRelocationForm::Modified,
            FrapconRelocationForm::Legacy,
        ] {
            for power_kw in [0.0, 5.0, 15.0, 20.0, 25.0, 40.0, 45.0, 60.0] {
                let model = model(power_kw * 1000.0, form);
                for burnup in [0.0, 0.05, 0.1, 1.0, 5.0, 6.0, 40.0, 120.0] {
                    let fraction = model.gap_closure_fraction(&at(burnup));
                    assert!(
                        (0.0..=1.0).contains(&fraction),
                        "{form:?} at {power_kw} kW/m, {burnup} MWd/kgHM gives \
                         closure {fraction}"
                    );
                    assert!(
                        fraction <= 0.5 + 1.0e-12,
                        "{form:?} closes {fraction} of the gap, above the 0.5 bound"
                    );
                    let displacement = model.radial_displacement(&at(burnup));
                    assert!(
                        (0.0..=COLD_GAP / 2.0).contains(&displacement),
                        "displacement {displacement} escapes the radial gap \
                         {}",
                        COLD_GAP / 2.0
                    );
                }
            }
        }
    }

    /// **Verification of the low-burnup cut-off against the correlation's own
    /// algebra.** Upstream's 0.0937 MWd/kgHM threshold exists because the
    /// logarithmic burnup term `0.5795 + 0.2447·ln Bu` crosses zero there; a
    /// port that got the constants wrong would show a visible step.
    ///
    /// # Methodology
    ///
    /// - Inputs: modified form, 20 kW/m, burnup approached from both sides of
    ///   0.0937 MWd/kgHM (±1e-9).
    /// - Reference: continuity — the two branches are designed to meet.
    /// - Pass criterion: step in the closure fraction below 1e-3.
    ///
    /// # Results (measured 2026-07-29, this port)
    ///
    /// - `f` below the cut-off: `0.055000000`
    /// - `f` above the cut-off: `0.055046`
    /// - step: `4.6e-5` in `f`, i.e. 0.08% of the value.
    ///
    /// # Interpretation
    ///
    /// The residual step is upstream's, not the port's: `0.5795 + 0.2447 ·
    /// ln(0.0937) = 1.34e-4` rather than exactly zero, because 0.0937 is a
    /// rounded root. The threshold and the constants are therefore consistent
    /// with each other to four significant figures, which is the precision the
    /// published constants carry.
    #[test]
    fn modified_form_is_continuous_at_its_low_burnup_cutoff() {
        let model = model(20_000.0, FrapconRelocationForm::Modified);
        let below = model.gap_closure_fraction(&at(MODIFIED_LOW_BURNUP_CUTOFF - 1.0e-9));
        let above = model.gap_closure_fraction(&at(MODIFIED_LOW_BURNUP_CUTOFF + 1.0e-9));
        assert!(
            (below - 0.055).abs() < 1.0e-15,
            "the floor is 5.5% gap closure"
        );
        assert!(
            (above - below).abs() < 1.0e-3,
            "step of {} at the cut-off",
            above - below
        );
        assert!(
            above >= below,
            "the correlation must not drop at the cut-off"
        );
    }

    /// Self-consistency check, not external validation: relocation is a
    /// one-way process — cracked fragments do not move back — so the closure
    /// fraction must never decrease with burnup at fixed power. This is also
    /// what makes it safe to drop upstream's `max(old, new)` ratchet for a
    /// rising-burnup history.
    #[test]
    fn gap_closure_is_monotonic_in_burnup() {
        for form in [
            FrapconRelocationForm::Modified,
            FrapconRelocationForm::Legacy,
        ] {
            for power_kw in [10.0, 20.0, 30.0, 50.0] {
                let model = model(power_kw * 1000.0, form);
                let mut previous = f64::NEG_INFINITY;
                for burnup in [0.0, 0.05, 0.0937, 0.5, 1.0, 3.0, 5.0, 5.6, 20.0, 120.0] {
                    let fraction = model.gap_closure_fraction(&at(burnup));
                    assert!(
                        fraction >= previous - 1.0e-15,
                        "{form:?} at {power_kw} kW/m drops from {previous} to \
                         {fraction} at {burnup} MWd/kgHM"
                    );
                    previous = fraction;
                }
            }
        }
    }

    /// Self-consistency check, not external validation: both formulations
    /// saturate early in life — the modified form at `Bu = 5.576` MWd/kgHM
    /// where its logarithmic term reaches 1, the legacy form at exactly
    /// 5 MWd/kgHM where `F_Bu` reaches 1. After that, relocation is complete.
    #[test]
    fn gap_closure_saturates_early_in_life() {
        let modified = model(30_000.0, FrapconRelocationForm::Modified);
        let saturated = modified.gap_closure_fraction(&at(10.0));
        assert!((modified.gap_closure_fraction(&at(60.0)) - saturated).abs() < 1.0e-15);
        assert!((modified.gap_closure_fraction(&at(120.0)) - saturated).abs() < 1.0e-15);
        // 0.055 + amplitude, with amplitude = 0.345 + (30-20)/200 = 0.395
        assert!((saturated - (0.055 + 0.395)).abs() < 1.0e-12);
        // Still climbing just below the saturation burnup.
        assert!(modified.gap_closure_fraction(&at(5.0)) < saturated);

        let legacy = model(30_000.0, FrapconRelocationForm::Legacy);
        let saturated = legacy.gap_closure_fraction(&at(5.0));
        assert!((legacy.gap_closure_fraction(&at(120.0)) - saturated).abs() < 1.0e-15);
        // 28 + P + (12 + P), with P = (30-20)*5/20 = 2.5  ->  45%
        assert!((saturated - 0.45).abs() < 1.0e-12);
    }

    /// Self-consistency check on the power dependence of the modified form: the
    /// amplitude ramps linearly from 0.345 to 0.445 across 20–40 kW/m and is
    /// flat outside. Checked at saturation, where `f = 0.055 + amplitude`.
    #[test]
    fn modified_form_power_ramp_matches_the_correlation() {
        let saturated = |power_kw: f64| {
            model(power_kw * 1000.0, FrapconRelocationForm::Modified)
                .gap_closure_fraction(&at(20.0))
        };
        assert!((saturated(5.0) - (0.055 + 0.345)).abs() < 1.0e-12);
        assert!((saturated(19.9) - (0.055 + 0.345)).abs() < 1.0e-12);
        assert!((saturated(20.0) - (0.055 + 0.345)).abs() < 1.0e-12);
        assert!((saturated(30.0) - (0.055 + 0.395)).abs() < 1.0e-12);
        assert!((saturated(40.0) - (0.055 + 0.445)).abs() < 1.0e-12);
        assert!((saturated(60.0) - (0.055 + 0.445)).abs() < 1.0e-12);
        // Monotone in power.
        let mut previous = 0.0;
        for power_kw in [0.0, 10.0, 20.0, 25.0, 30.0, 35.0, 40.0, 80.0] {
            let value = saturated(power_kw);
            assert!(value >= previous - 1.0e-15);
            previous = value;
        }
    }

    /// The three exported quantities must be related by upstream's algebra
    /// exactly: `ε = f·(G/D)` and `Δr = ε·D/2 = f·G/2`. Pins the unit
    /// convention documented at the top of this module — the single thing most
    /// likely to be misread by a caller.
    #[test]
    fn strain_fraction_and_displacement_are_mutually_consistent() {
        for form in [
            FrapconRelocationForm::Modified,
            FrapconRelocationForm::Legacy,
        ] {
            let model = model(25_000.0, form);
            for burnup in [0.0, 1.0, 5.0, 40.0] {
                let state = at(burnup);
                let fraction = model.gap_closure_fraction(&state);
                let strain = model.value(&state);
                let displacement = model.radial_displacement(&state);

                assert!((strain - fraction * (COLD_GAP / COLD_DIAMETER)).abs() < 1.0e-18);
                assert!((displacement - strain * COLD_DIAMETER / 2.0).abs() < 1.0e-18);
                assert!((displacement - fraction * COLD_GAP / 2.0).abs() < 1.0e-18);
            }
        }
    }

    /// The two formulations are genuinely different fits, not variations of
    /// one another. At beginning of life the legacy form closes about six times
    /// as much of the gap; by saturation they are within 12%. Recorded so that
    /// swapping the form is never mistaken for a rounding difference.
    ///
    /// Measured at 20 kW/m (2026-07-29, this port): fresh fuel `f = 0.055`
    /// (modified) against `0.300` (legacy), a ratio of 5.45; at 40 MWd/kgHM
    /// `0.400` against `0.400` — identical at this power, by coincidence of the
    /// two fits.
    #[test]
    fn the_two_formulations_differ_substantially_at_beginning_of_life() {
        let modified = model(20_000.0, FrapconRelocationForm::Modified);
        let legacy = model(20_000.0, FrapconRelocationForm::Legacy);

        let fresh_modified = modified.gap_closure_fraction(&at(0.0));
        let fresh_legacy = legacy.gap_closure_fraction(&at(0.0));
        assert!((fresh_modified - 0.055).abs() < 1.0e-15);
        assert!((fresh_legacy - 0.300).abs() < 1.0e-15);
        assert!(fresh_legacy > 5.0 * fresh_modified);
    }

    /// `value` clamps and `value_checked` refuses — the documented contract.
    #[test]
    fn out_of_range_and_unphysical_inputs_are_reported() {
        let model = model(20_000.0, FrapconRelocationForm::Modified);

        assert_eq!(model.value(&at(500.0)), model.value(&at(120.0)));
        assert!(matches!(
            model.value_checked(&at(500.0)),
            Err(OffbeatError::OutOfRange { .. })
        ));
        assert_eq!(model.value(&at(-5.0)), model.value(&at(0.0)));
        assert!(matches!(
            model.value_checked(&at(-5.0)),
            Err(OffbeatError::OutOfRange { .. })
        ));

        let degenerate = RelocationModel::Uo2Frapcon {
            cold_gap: COLD_GAP,
            cold_pellet_diameter: 0.0,
            linear_power: 2.0e4,
            form: FrapconRelocationForm::Modified,
        };
        assert_eq!(degenerate.value(&at(10.0)), 0.0);
        assert!(matches!(
            degenerate.value_checked(&at(10.0)),
            Err(OffbeatError::Unphysical { .. })
        ));

        let negative_gap = RelocationModel::Uo2Frapcon {
            cold_gap: -1.0e-4,
            cold_pellet_diameter: COLD_DIAMETER,
            linear_power: 2.0e4,
            form: FrapconRelocationForm::Modified,
        };
        assert!(matches!(
            negative_gap.value_checked(&at(10.0)),
            Err(OffbeatError::Unphysical { .. })
        ));

        let negative_power = RelocationModel::Uo2Frapcon {
            cold_gap: COLD_GAP,
            cold_pellet_diameter: COLD_DIAMETER,
            linear_power: -1.0,
            form: FrapconRelocationForm::Modified,
        };
        assert!(matches!(
            negative_power.value_checked(&at(10.0)),
            Err(OffbeatError::Unphysical { .. })
        ));
        assert!(matches!(
            negative_power.gap_closure_fraction_checked(&at(10.0)),
            Err(OffbeatError::Unphysical { .. })
        ));
        assert!(matches!(
            negative_power.radial_displacement_checked(&at(10.0)),
            Err(OffbeatError::Unphysical { .. })
        ));
    }

    /// The legacy form's power branches are discontinuous upstream; this pins
    /// the discontinuity so that anyone smoothing it has to notice. It
    /// documents an upstream artefact, and does not endorse it.
    ///
    /// Measured 2026-07-29, this port: at zero burnup the 20 kW/m boundary
    /// steps DOWN from `f = 0.300000000` (19.999 kW/m) to `f = 0.280002500`
    /// (20.001 kW/m) — a 6.7% discontinuity. At 5 MWd/kgHM (`F_Bu = 1`) the
    /// same boundary is continuous (`0.400000` either side) and the 40 kW/m
    /// boundary is continuous to 1e-5 (`0.499995` against `0.500000`), because
    /// the branches were fitted to agree at saturation and not at beginning of
    /// life.
    #[test]
    fn legacy_form_power_branches_are_discontinuous_reproducing_upstream() {
        let fresh_below = legacy_gap_closure(0.0, 19.999);
        let fresh_above = legacy_gap_closure(0.0, 20.001);
        assert!((fresh_below - 0.300).abs() < 1.0e-9);
        assert!((fresh_above - 0.280).abs() < 1.0e-5);
        assert!(
            fresh_above < fresh_below,
            "upstream's 20 kW/m branch steps DOWN at zero burnup"
        );

        // The 40 kW/m boundary steps too, in the burnup slope.
        let hot_below = legacy_gap_closure(5.0, 39.999);
        let hot_above = legacy_gap_closure(5.0, 40.001);
        assert!((hot_below - 0.50).abs() < 1.0e-5);
        assert!((hot_above - 0.50).abs() < 1.0e-9);
    }
}
