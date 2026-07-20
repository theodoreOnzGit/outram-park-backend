// SPDX-License-Identifier: GPL-3.0
//
// TRISO-ATOPS fork — provenance
// -----------------------------
// Upstream project : TRISO-ATOPS (INL) — https://github.com/IdahoLabResearch/TRISO-ATOPS
// Upstream commit  : de374c8
// Upstream source  : trisoatops/utility_functions/calculation_functions.py
//                    (`release_rate`, `base_activities`)
// Original license : MIT — Copyright (c) 2026 Battelle Energy Alliance, LLC
// This port is distributed under GPL-3.0 as part of the combined boon-lay work;
// the MIT notice above is retained (see LICENSE.triso-atops / NOTICE.triso-atops).

//! # Source terms — release rate and graphite hold-up
//!
//! These two functions sit between the dimensionless release models
//! ([`crate::triso_atops_fork::release_models`]) and the coolant activity pools
//! ([`super::coolant_activity`]):
//!
//! - [`release_rate`] turns a node's radionuclide **inventory** (an [`Activity`])
//!   and its `<R/B>_fail` into the **release rate** `R` at which the nuclide
//!   leaves the fuel into the fuel-element graphite.
//! - [`base_activities`] splits `R` into the **source rate** `S` that reaches the
//!   coolant (after graphite attenuation) and the **graphite hold-up** activity
//!   `G` retained in the matrix.
//!
//! ## Units
//!
//! The inventory is a `uom` [`Activity`] (Bq). `R` and `S` are release/source
//! *rates* carried as effective-unit `f64` becquerels (`atoms/s`); `G` is an atom
//! **count** (`atoms`). See the [`super`] module docs for why the pool
//! quantities are `f64` rather than `uom`-typed.

use uom::si::f64::{DiffusionCoefficient, Length, Time};
use uom::si::frequency::hertz;
use uom::si::ratio::ratio;

use super::Activity;
use crate::triso_atops_fork::nuclide_model::ElementGroup;
use crate::triso_atops_fork::release_models::steady_state::attenuation_factor;
use crate::triso_atops_fork::{DecayConstant, ReleaseFraction};

/// Fixed graphite attenuation factor assigned to non-metal "other" nuclides.
///
/// Ports the upstream `Af = 1e8` fallback in `base_activities` (a nuclide that is
/// neither a special metal nor silver is assumed to be almost entirely held up in
/// the graphite, so its source rate `S = R/Af` is negligible).
pub const OTHER_ATTENUATION_FACTOR: f64 = 1e8;

/// The four TRISO fuel failure fractions applied to a release-to-birth ratio.
///
/// Ports the upstream `fractions = [f_hm, f_sic, f_inc, f_inc_sic]` list (User
/// Manual §2.4 keys `f_hm`, `f_sic`, `f_inc`, `f_inc_sic`). All four are
/// dimensionless fractions in `[0, 1]`; how they combine depends on the transport
/// group (see [`release_rate`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FailureFractions {
    /// Heavy-metal contamination fraction `f_hm` (uranium outside intact kernels).
    pub heavy_metal: f64,
    /// As-manufactured defective-SiC fraction `f_sic`.
    pub sic: f64,
    /// Incremental (in-service) particle failure fraction `f_inc`.
    pub incremental: f64,
    /// Incremental SiC-only failure fraction `f_inc_sic`.
    pub incremental_sic: f64,
}

impl FailureFractions {
    /// Sum of all four failure fractions, `f_hm + f_sic + f_inc + f_inc_sic`.
    ///
    /// Used for special-metal and "other" nuclides, whose entire failed-particle
    /// population contributes to release.
    #[must_use]
    pub fn sum(&self) -> f64 {
        self.heavy_metal + self.sic + self.incremental + self.incremental_sic
    }
}

/// Per-node fission-product **release rate** `R` from the fuel.
///
/// Ports `release_rate(RB_fail, z, fractions, inventories, sl, t, lam)`. The
/// release-to-birth ratio applied to the birth rate depends on the transport
/// group (ports the upstream `z in noble_gases/halogens` and `z == 47/46`
/// branches):
///
/// - **Noble gas / halogen** → `(f_hm + f_inc) · <R/B>_fail` (only the fully-
///   exposed and in-service-failed fuel releases volatiles).
/// - **Silver / palladium** → `<R/B>_fail` directly (the breakthrough model
///   already embeds the SiC-failure population).
/// - **Special metal / other** → `Σfractions · <R/B>_fail`.
///
/// The birth rate is `A` for a short-lived nuclide (secular equilibrium, the
/// inventory activity *is* the production rate) and `A / (1 − e^{−λ t})` for a
/// long-lived one, where `A` is the inventory [`Activity`]. **Derivation:** step
/// 6(ii) (crate-root `TRISO_ATOPS_DERIVATION.md` §6): `R = ⟨R/B⟩ × birth rate`,
/// with `⟨R/B⟩ = failure fraction × ⟨R/B⟩_fail`. This is the explicit
/// form of the upstream `didt = inventories × 3.7e10 [ / (1 − e^{−λt})]`, with
/// the `× 3.7e10` now living only in the Ci→Bq conversion of the inventory (see
/// [`crate::triso_atops_fork::activities::becquerels_from_curies`]).
///
/// # Arguments
/// - `rb_fail` — the release-to-birth-at-failure ratio `<R/B>_fail`
///   ([`ReleaseFraction`]), from [`crate::triso_atops_fork::release_models::rb_fail`].
/// - `element_group` — the nuclide transport [`ElementGroup`].
/// - `fractions` — the fuel [`FailureFractions`].
/// - `inventory` — the node's radionuclide inventory as an [`Activity`] (Bq);
///   obtain from curies with
///   [`crate::triso_atops_fork::activities::becquerels_from_curies`].
/// - `short_lived` — the run-dependent short-lived flag `sl`
///   (`t½ / t_irad < 0.2` upstream).
/// - `time` — irradiation time `t` ([`Time`], `s`).
/// - `decay_constant` — nuclide `λ` ([`DecayConstant`], `s^-1`).
///
/// # Returns
/// The release rate `R` (effective-unit `f64`, Bq = `atoms/s`).
#[must_use]
pub fn release_rate(
    rb_fail: ReleaseFraction,
    element_group: ElementGroup,
    fractions: FailureFractions,
    inventory: Activity,
    short_lived: bool,
    time: Time,
    decay_constant: DecayConstant,
) -> f64 {
    let rb_ratio = rb_fail.get::<ratio>();
    let release_to_birth = match element_group {
        ElementGroup::NobleGas | ElementGroup::Halogen => {
            (fractions.heavy_metal + fractions.incremental) * rb_ratio
        }
        ElementGroup::Silver => rb_ratio,
        ElementGroup::SpecialMetal | ElementGroup::Other => fractions.sum() * rb_ratio,
    };

    let inventory_bq = inventory.get::<hertz>();
    let birth_rate = if short_lived {
        inventory_bq
    } else {
        let lt = (decay_constant * time).get::<ratio>(); // dimensionless, uom-checked
        inventory_bq / (1.0 - (-lt).exp())
    };
    birth_rate * release_to_birth
}

/// The [`base_activities`] result: coolant source rate and graphite hold-up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceAndGraphite {
    /// Source rate `S` that reaches the coolant, after graphite attenuation
    /// (effective-unit `f64`, Bq = `atoms/s`). Feeds
    /// [`super::coolant_activity::circulating`].
    pub source_rate: f64,
    /// Graphite hold-up activity `G` retained in the fuel-element matrix
    /// (effective-unit `f64`, atom count). Zero for volatiles.
    pub graphite_activity: f64,
}

/// Split a release rate into coolant **source rate** `S` and **graphite** `G`.
///
/// Ports `base_activities(z, lam, t, a_graph, D_graph, R)`. **Derivation:** step
/// 6(iii) (crate-root `TRISO_ATOPS_DERIVATION.md` §6): splits `R` into the
/// coolant source rate `S = R/Af` and the graphite hold-up
/// `G = R·(1 − 1/Af)·(1 − e^{−λt})/λ`, using the step-5d attenuation factor `Af`.
///
/// - **Noble gas / halogen** → `S = R`, `G = 0` (volatiles are not retained in
///   graphite; they pass straight to the coolant).
/// - **Special metal / silver** → attenuation factor `Af` from the graphite
///   diffusion series ([`attenuation_factor`]); `S = R / Af`,
///   `G = R·(1 − 1/Af)·(1 − e^{−λ t}) / λ`.
/// - **Other** metals → fixed `Af = ` [`OTHER_ATTENUATION_FACTOR`] (`1e8`), same
///   `S`/`G` formulas.
///
/// # Arguments
/// - `element_group` — the nuclide transport [`ElementGroup`].
/// - `decay_constant` — nuclide `λ` ([`DecayConstant`], `s^-1`); must be non-zero.
/// - `time` — irradiation time `t` ([`Time`], `s`).
/// - `graphite_thickness` — graphite layer thickness `a_graph` ([`Length`], `m`).
/// - `graphite_diffusion_coefficient` — graphite diffusion coefficient `D_graph`
///   ([`DiffusionCoefficient`], `m^2/s`), from
///   [`crate::triso_atops_fork::diffusion::diffusion_coefficient`].
/// - `release_rate` — the release rate `R` from [`release_rate`] (effective `f64`, Bq).
///
/// # Returns
/// [`SourceAndGraphite`] with the coolant source rate `S` and graphite hold-up `G`.
#[must_use]
pub fn base_activities(
    element_group: ElementGroup,
    decay_constant: DecayConstant,
    time: Time,
    graphite_thickness: Length,
    graphite_diffusion_coefficient: DiffusionCoefficient,
    release_rate: f64,
) -> SourceAndGraphite {
    match element_group {
        ElementGroup::NobleGas | ElementGroup::Halogen => SourceAndGraphite {
            source_rate: release_rate,
            graphite_activity: 0.0,
        },
        _ => {
            let af = match element_group {
                ElementGroup::SpecialMetal | ElementGroup::Silver => {
                    attenuation_factor(graphite_diffusion_coefficient, time, graphite_thickness)
                        .get::<ratio>()
                }
                _ => OTHER_ATTENUATION_FACTOR,
            };
            let lam = decay_constant.get::<hertz>();
            let lt = (decay_constant * time).get::<ratio>(); // dimensionless, uom-checked
            let source_rate = release_rate / af;
            let graphite_activity = release_rate * (1.0 - 1.0 / af) * (1.0 - (-lt).exp()) / lam;
            SourceAndGraphite {
                source_rate,
                graphite_activity,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::triso_atops_fork::activities::becquerels_from_curies;
    use approx::assert_relative_eq;
    use uom::si::diffusion_coefficient::square_meter_per_second;
    use uom::si::length::meter;
    use uom::si::time::second;

    const FRACTIONS: FailureFractions = FailureFractions {
        heavy_metal: 1e-4,
        sic: 1e-4,
        incremental: 2.3e-5,
        incremental_sic: 3.6e-5,
    };
    // 3 years of irradiation, matching the upstream driver run (2026-07-15).
    fn t_irad() -> Time {
        Time::new::<second>(3.0 * 3.155_76e7)
    }

    /// Release rate for a short-lived noble gas (Kr-88) matches upstream.
    ///
    /// Methodology: `R = A · (f_hm + f_inc) · <R/B>_fail` with `A = 44.5 Ci` in
    /// Bq, `<R/B>_fail = 3.14e-6`. Reference (upstream `release_rate`,
    /// 2026-07-15): 635.91123. Pass tol 1e-10 rel.
    #[test]
    fn release_rate_shortlived_noble_gas_matches_upstream() {
        let rb = ReleaseFraction::new::<ratio>(3.14e-6);
        let r = release_rate(
            rb,
            ElementGroup::NobleGas,
            FRACTIONS,
            becquerels_from_curies(44.5),
            true,
            t_irad(),
            DecayConstant::new::<hertz>(std::f64::consts::LN_2 / 10152.0),
        );
        assert_relative_eq!(r, 635.911_23, max_relative = 1e-10);
    }

    /// Release rate for a long-lived special metal (Cs-137) matches upstream.
    ///
    /// Methodology: `R = A/(1−e^{−λt}) · Σfractions · <R/B>_fail`,
    /// `<R/B>_fail = 1e-5`. Reference (upstream `release_rate`, 2026-07-15):
    /// 63842.31838143877. Pass tol 1e-10 rel.
    #[test]
    fn release_rate_longlived_special_metal_matches_upstream() {
        let rb = ReleaseFraction::new::<ratio>(1e-5);
        let r = release_rate(
            rb,
            ElementGroup::SpecialMetal,
            FRACTIONS,
            becquerels_from_curies(44.5),
            false,
            t_irad(),
            DecayConstant::new::<hertz>(std::f64::consts::LN_2 / 949_232_333.0),
        );
        assert_relative_eq!(r, 63842.318_381_438_77, max_relative = 1e-10);
    }

    /// `base_activities` for Cs-137 (special metal) matches upstream S and G.
    ///
    /// Methodology: `D_graph = 1e-16 m^2/s`, `a_graph = 0.0045 m`, `t = 3 yr`, so
    /// the attenuation factor saturates to its 1e8 cap; `S = R/Af`,
    /// `G = R(1−1/Af)(1−e^{−λt})/λ`. References (upstream `base_activities`,
    /// 2026-07-15): `S = 0.0006384231838143877`, `G = 5839942305222.119`.
    #[test]
    fn base_activities_special_metal_matches_upstream() {
        let lam = DecayConstant::new::<hertz>(std::f64::consts::LN_2 / 949_232_333.0);
        let sg = base_activities(
            ElementGroup::SpecialMetal,
            lam,
            t_irad(),
            Length::new::<meter>(0.0045),
            DiffusionCoefficient::new::<square_meter_per_second>(1e-16),
            63842.318_381_438_77,
        );
        assert_relative_eq!(
            sg.source_rate,
            6.384_231_838_143_877e-4,
            max_relative = 1e-9
        );
        assert_relative_eq!(
            sg.graphite_activity,
            5.839_942_305_222_119e12,
            max_relative = 1e-9
        );
    }

    /// Volatiles pass straight to the coolant: `S = R`, `G = 0`.
    #[test]
    fn base_activities_volatile_no_graphite() {
        let sg = base_activities(
            ElementGroup::NobleGas,
            DecayConstant::new::<hertz>(1e-4),
            t_irad(),
            Length::new::<meter>(0.0045),
            DiffusionCoefficient::new::<square_meter_per_second>(1e-16),
            635.911_23,
        );
        assert_eq!(sg.source_rate, 635.911_23);
        assert_eq!(sg.graphite_activity, 0.0);
    }
}
