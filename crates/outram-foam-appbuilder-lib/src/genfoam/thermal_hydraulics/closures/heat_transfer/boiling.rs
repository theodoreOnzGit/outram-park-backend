// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/heatTransferModels/
//             FSHeatTransferCoefficientModels/{multiRegimeBoiling,
//             subModels/superpositionNucleateBoiling}/
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `boiling` — nucleate-boiling superposition & the multi-regime dispatcher
//!
//! Two functions, both faithful ports of GeN-Foam's boiling-htc combiners:
//!
//! - [`superposition_nucleate_boiling_htc`] — the simple weighted sum
//!   `h = h_FC*F + h_PB*S` (upstream `superpositionNucleateBoiling`), for
//!   flows that transition from single-phase convection straight into
//!   nucleate boiling with no sub-cooled-boiling region in between.
//! - [`multi_regime_boiling_htc`] — the fuller TRACE-philosophy regime map
//!   (upstream `multiRegimeBoiling`): condensation / optional film
//!   condensation below `T_sat`, plain forced convection between `T_sat` and
//!   the onset-of-nucleate-boiling temperature, and Chen-style flux- or
//!   htc-superposition nucleate boiling above it.
//!
//! **Not ported here** (see the parent module doc for the full rationale):
//! the TRACE CHF/post-CHF variants, and the `dmdtW` sub-cooled-boiling
//! mass-transfer side effect — [`multi_regime_boiling_htc`] returns the two
//! heat-flux ingredients that side effect needs
//! ([`MultiRegimeBoilingResult::nucleate_boiling_heat_flux`],
//! [`MultiRegimeBoilingResult::forced_convection_heat_flux`]) instead of
//! writing to shared mutable state.
//!
//! ## Two upstream quirks reproduced verbatim (not silently fixed)
//!
//! Per this repository's rule against silently changing ported physics,
//! [`multi_regime_boiling_htc`] reproduces two literal upstream expressions
//! exactly, each flagged with a `NOTE(port):` comment at the call site:
//!
//! 1. The film-condensation blend fraction in the `T_wall < T_sat` branch is
//!    upstream's literal `f = 0.9 - alpha/0.1` (division binds tighter than
//!    subtraction in C++, same as Rust), not the evidently-intended
//!    `(0.9 - alpha)/0.1` — the former leaves `[0, 1]` for any
//!    `alpha in (0.8, 0.9)` except very close to `0.89`.
//! 2. The temperature-difference floor in the flux-superposition branch is
//!    upstream's literal `dT = (dT >= 0) ? max(dT,1e-3) : min(-dT,-1e-3)`;
//!    since `-dT > 0` whenever `dT < 0`, the `else` arm always evaluates to
//!    exactly `-1e-3` regardless of `|dT|` (almost certainly meant
//!    `-max(-dT, 1e-3)`).
//!
//! Both are pure scalar arithmetic (no `uom` dimensional-consistency check
//! applies), so — unlike the two dimensionally-broken formulas documented in
//! [`super::chf`] that `uom` refuses to compile — reproducing them verbatim
//! was a choice, not a compiler requirement. It was made because "the
//! equation is wrong, not the tolerance" cuts both ways: an AI-assisted port
//! silently "improving" upstream physics without sign-off is exactly the
//! failure mode this workspace's guardrails exist to prevent.

use crate::genfoam::thermal_hydraulics::units::{HeatFlux, HeatTransferCoefficient};
use uom::si::f64::{Ratio, ThermodynamicTemperature};
use uom::si::heat_flux_density::watt_per_square_meter;
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

/// `h = h_FC * F + h_PB * S` — the simplest boiling-htc combination, for
/// flows with no sub-cooled-boiling region between single-phase convection
/// and nucleate boiling (upstream `superpositionNucleateBoiling`).
#[must_use]
pub fn superposition_nucleate_boiling_htc(
    htc_forced_convection: HeatTransferCoefficient,
    flow_enhancement_factor: Ratio,
    htc_pool_boiling: HeatTransferCoefficient,
    suppression_factor: Ratio,
) -> HeatTransferCoefficient {
    htc_forced_convection * flow_enhancement_factor.get::<uom::si::ratio::ratio>()
        + htc_pool_boiling * suppression_factor.get::<uom::si::ratio::ratio>()
}

/// Inputs to [`multi_regime_boiling_htc`]. Mirrors GeN-Foam's
/// `multiRegimeBoiling` dictionary, whose several sub-models are all
/// optional (`this->found("...")`) — the `Option` fields here play the same
/// role.
#[derive(Debug, Clone, Copy)]
pub struct MultiRegimeBoilingInputs {
    /// Wall (structure) temperature `T_wall`.
    pub t_wall: ThermodynamicTemperature,
    /// Bulk fluid temperature `T_fluid`.
    pub t_fluid: ThermodynamicTemperature,
    /// Saturation temperature `T_sat` at the local pressure.
    pub t_sat: ThermodynamicTemperature,
    /// Vapour void fraction `alpha_v = 1 - alpha_liquid,normalized`.
    pub vapour_void_fraction: Ratio,
    /// Two-phase forced-convection htc, **already** multiplied by the flow-
    /// enhancement factor `F` (upstream's cached `htc2pFCi_ =
    /// htcFCPtr_->value(celli) * FPtr_->value(celli)`).
    pub htc_forced_convection_enhanced: HeatTransferCoefficient,
    /// Pool-boiling htc evaluated at `(T_wall, T_sat)` (upstream
    /// `htcPBPtr_->value(celli)`).
    pub htc_pool_boiling: HeatTransferCoefficient,
    /// Film-condensation htc, if a `filmCondensationModel` was configured
    /// (`None` skips straight to `htc_forced_convection_enhanced` below
    /// `T_sat`, matching upstream's `htcCndPtr_.valid()` guard).
    pub htc_film_condensation: Option<HeatTransferCoefficient>,
    /// Suppression factor `S`, if a `suppressionFactorModel` was configured.
    pub suppression_factor: Option<Ratio>,
    /// Onset-of-nucleate-boiling temperature, if a `nucleateBoilingOnsetModel`
    /// was configured. `None` uses `T_sat` in its place, matching upstream's
    /// `(TONBPtr_.valid()) ? TONBPtr_->value(...) : Tsati`.
    pub t_onset_nucleate_boiling: Option<ThermodynamicTemperature>,
    /// Pool-boiling htc evaluated at `(T_onset_nucleate_boiling, T_sat)`
    /// instead of `(T_wall, T_sat)` — only read in flux-superposition mode
    /// when a suppression factor is **not** configured but an ONB
    /// temperature **is** (upstream's `qBIi` "boiling-inception" heat-flux
    /// term, which avoids a discontinuity at the boiling onset). Pass `None`
    /// to fall back to plain `qFC^n + qPB^n` superposition in that case.
    pub htc_pool_boiling_at_onset: Option<HeatTransferCoefficient>,
    /// Superposition exponent `n` (upstream dictionary entry
    /// `superpositionExponent`; TRACE-philosophy codes typically use `n` in
    /// `[1, 4]`, with higher `n` sharpening the transition between regimes).
    pub superposition_exponent: f64,
    /// `true` superposes **heat fluxes** then divides by `deltaT` to recover
    /// an htc (upstream's `heatFluxSuperposition = true`, the TRACE
    /// approach); `false` superposes **heat-transfer coefficients** directly.
    pub heat_flux_superposition: bool,
}

/// Output of [`multi_regime_boiling_htc`].
#[derive(Debug, Clone, Copy)]
pub struct MultiRegimeBoilingResult {
    /// The dispatched heat-transfer coefficient.
    pub htc: HeatTransferCoefficient,
    /// Nucleate-boiling heat flux `q_NB`, the ingredient
    /// [`super::chf::SubcooledBoilingFraction`] needs to compute the wall vapour-
    /// generation rate during sub-cooled boiling (upstream's `qNBi`, fed to
    /// its `setDmdtW` side effect). `Some` only in the flux-superposition
    /// nucleate-boiling branch (`heat_flux_superposition == true` and
    /// `T_wall >= T_onset_nucleate_boiling >= T_sat`); `None` everywhere
    /// else, including the htc-superposition nucleate-boiling branch — see
    /// the field doc on why that branch cannot supply a faithful value.
    pub nucleate_boiling_heat_flux: Option<HeatFlux>,
    /// Enhanced forced-convection heat flux `q_FC = h_2pFC * (T_wall -
    /// T_fluid)`, populated alongside `nucleate_boiling_heat_flux`.
    pub forced_convection_heat_flux: Option<HeatFlux>,
}

/// Dispatch the boiling regime and return its heat-transfer coefficient.
///
/// Faithful port of `multiRegimeBoiling::value()`'s regime map, minus the
/// post-CHF branch (upstream itself hardcodes `TCHFi = 1e69`, i.e.
/// unreachable — "needs dedicated model for its setting", per its own
/// comment) and the `dmdtW` mass-transfer side effect (see the module doc).
///
/// Regime map, matching upstream exactly:
/// 1. `T_wall < T_sat`: either plain enhanced forced convection, or — if a
///    film-condensation model is configured and `alpha_vapour > 0.8` — a
///    blend into (or full) film condensation as `alpha_vapour -> 0.9`.
/// 2. `T_sat <= T_wall < T_onset_nucleate_boiling`: plain enhanced forced
///    convection (nothing "special" happens yet).
/// 3. `T_wall >= T_onset_nucleate_boiling`: nucleate/sub-cooled boiling,
///    combining `htc_forced_convection_enhanced` and `htc_pool_boiling` via
///    `superposition_exponent`-power superposition, optionally moderated by
///    `suppression_factor` or `htc_pool_boiling_at_onset`.
#[must_use]
pub fn multi_regime_boiling_htc(inputs: MultiRegimeBoilingInputs) -> MultiRegimeBoilingResult {
    let tw = inputs.t_wall.get::<kelvin>();
    let tf = inputs.t_fluid.get::<kelvin>();
    let tsat = inputs.t_sat.get::<kelvin>();
    let alpha = inputs.vapour_void_fraction.get::<uom::si::ratio::ratio>();
    let htc2p_fc = inputs.htc_forced_convection_enhanced;

    let no_flux = MultiRegimeBoilingResult {
        htc: htc2p_fc,
        nucleate_boiling_heat_flux: None,
        forced_convection_heat_flux: None,
    };

    if tw < tsat {
        let htc = match inputs.htc_film_condensation {
            None => htc2p_fc,
            Some(htc_cnd) => {
                if alpha <= 0.8 {
                    htc2p_fc
                } else if alpha < 0.9 {
                    // NOTE(port): upstream's literal C++ is `f = 0.9 -
                    // alphai/0.1` (division binds tighter than subtraction),
                    // not the evidently-intended `(0.9 - alphai)/0.1`.
                    // Reproduced verbatim — see the module doc.
                    let f = 0.9 - alpha / 0.1;
                    htc2p_fc * f + htc_cnd * (1.0 - f)
                } else {
                    htc_cnd
                }
            }
        };
        return MultiRegimeBoilingResult { htc, ..no_flux };
    }

    let t_onb = inputs
        .t_onset_nucleate_boiling
        .map(|t| t.get::<kelvin>())
        .unwrap_or(tsat);
    if tw < t_onb {
        return no_flux;
    }

    // Nucleate / sub-cooled boiling branch.
    let n = inputs.superposition_exponent;
    let one_by_n = 1.0 / n;
    let q_fc = htc2p_fc.get::<watt_per_square_meter_kelvin>() * (tw - tf);
    let htc_pb = inputs
        .htc_pool_boiling
        .get::<watt_per_square_meter_kelvin>();
    let q_pb = htc_pb * (tw - tsat);

    if inputs.heat_flux_superposition {
        let q_nb = if let Some(s) = inputs.suppression_factor {
            let s_v = s.get::<uom::si::ratio::ratio>();
            (q_fc.powf(n) + (s_v * q_pb).powf(n)).powf(one_by_n)
        } else if let (Some(t_onb_typed), Some(htc_pb_onb)) = (
            inputs.t_onset_nucleate_boiling,
            inputs.htc_pool_boiling_at_onset,
        ) {
            let q_bi = htc_pb_onb.get::<watt_per_square_meter_kelvin>()
                * (t_onb_typed.get::<kelvin>() - tsat);
            (q_fc.powf(n) + (q_pb - q_bi).powf(n)).powf(one_by_n)
        } else {
            (q_fc.powf(n) + q_pb.powf(n)).powf(one_by_n)
        };

        let raw_dt = tw - tf;
        let dt_safe = if raw_dt >= 0.0 {
            raw_dt.max(1e-3)
        } else {
            // NOTE(port): upstream literal is `min(-dT, -1e-3)`; since `-dT`
            // is positive whenever `dT < 0`, this always evaluates to exactly
            // `-1e-3` regardless of `|dT|` (likely meant `-max(-dT, 1e-3)`).
            // Reproduced verbatim — see the module doc.
            (-raw_dt).min(-1e-3)
        };

        MultiRegimeBoilingResult {
            htc: HeatTransferCoefficient::new::<watt_per_square_meter_kelvin>(q_nb / dt_safe),
            nucleate_boiling_heat_flux: Some(HeatFlux::new::<watt_per_square_meter>(q_nb)),
            forced_convection_heat_flux: Some(HeatFlux::new::<watt_per_square_meter>(q_fc)),
        }
    } else {
        // htc-superposition mode. Upstream's `qNBi` here uses the *previous
        // call's* cached `htcNBi_` member (a stateful quantity with no
        // faithful pure-function equivalent) and is, in this branch, fed only
        // to the excluded `setDmdtW` side effect — irrelevant to the returned
        // htc either way. Not reproduced; the heat-flux fields are `None`.
        let htc2p_fc_v = htc2p_fc.get::<watt_per_square_meter_kelvin>();
        let htc_nb_v = if let Some(s) = inputs.suppression_factor {
            let s_v = s.get::<uom::si::ratio::ratio>();
            (htc2p_fc_v.powf(n) + (s_v * htc_pb).powf(n)).powf(one_by_n)
        } else {
            (htc2p_fc_v.powf(n) + htc_pb.powf(n)).powf(one_by_n)
        };
        MultiRegimeBoilingResult {
            htc: HeatTransferCoefficient::new::<watt_per_square_meter_kelvin>(htc_nb_v),
            nucleate_boiling_heat_flux: None,
            forced_convection_heat_flux: None,
        }
    }
}
