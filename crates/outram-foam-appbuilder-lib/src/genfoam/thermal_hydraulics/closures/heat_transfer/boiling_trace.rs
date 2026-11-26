// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/heatTransferModels/
//             FSHeatTransferCoefficientModels/multiRegimeBoilingTRACE/
//             multiRegimeBoilingTRACEFSHeatTransferCoefficient.{H,C}
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `boiling_trace` — `multiRegimeBoilingTRACE::value` (gh:#1222)
//!
//! One cell of upstream's `multiRegimeBoilingTRACE`, the TRACE-style wall-boiling heat-transfer
//! regime map that every `PSBT` tutorial family member selects. It differs from
//! [`super::boiling`]'s `multiRegimeBoiling` in three ways, all kept:
//!
//! * the nucleate/sub-cooled boiling heat flux is ALWAYS flux superposition with the exponent 3
//!   hard-wired (`superpositionExponent` and `heatFluxSuperposition` are read by nobody),
//!   `q_NB = (q_FC^3 + (q_PB - q_BI)^3)^(1/3)`, with `q_BI = h_PB(T_ONB) (T_ONB - T_sat)` the
//!   "boiling inception" flux and `h_PB(T_ONB)` the pool-boiling coefficient at a wall at the ONB
//!   temperature (upstream evaluates it by temporarily overwriting the wall-temperature field);
//! * the suppression factor is NOT used (its model is constructed and never evaluated);
//! * the sub-cooled boiling contribution `q_SC = f (q_NB - q_FC)` is turned into a wall
//!   mass-transfer source `dmdtW = iA_act q_SC / |L|` which the model writes into the phase-change
//!   model's `dmdtW` field. This function RETURNS `q_SC`; the caller forms `iA_act q_SC / |L|`.
//!
//! The post-CHF branch is upstream's `TCHFi = 1e69` stub and returns `1.0`.
//!
//! Upstream literals reproduced: the film-condensation blend `f = 0.9 - alpha/0.1` (division
//! binds tighter, as in [`super::boiling`]) and the temperature-difference floor
//! `dT = (dT >= 0) ? max(dT, 1e-3) : min(-dT, -1e-3)` (always `-1e-3` when `dT < 0`).
//!
//! Side-model evaluations are passed as closures so that, exactly as upstream, the ONB, pool-boiling
//! and sub-cooled-boiling-fraction models are evaluated ONLY on the branches that call them.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use petir::mathf::RealMath;

/// Per-cell inputs of [`multi_regime_boiling_trace`] (SI, plain scalars).
#[derive(Debug, Clone, Copy)]
pub struct TraceBoilingInputs {
    /// `alphai = 1 - fluid.normalized` (the other phase's fraction).
    pub alpha_other: f64,
    /// Wall temperature `Twall`.
    pub t_wall: f64,
    /// Fluid temperature.
    pub t_fluid: f64,
    /// Saturation (interface) temperature `T.interface`.
    pub t_sat: f64,
    /// `htcFC(celli) * F(celli)`: the enhanced forced-convection coefficient.
    pub htc_2p_fc: f64,
    /// Film-condensation coefficient when a `filmCondensationModel` exists.
    pub htc_film_condensation: Option<f64>,
    /// Whether a `nucleateBoilingOnsetModel` exists (otherwise `T_ONB = T_sat`).
    pub has_onb_model: bool,
}

/// Result of one cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TraceBoilingResult {
    /// The heat-transfer coefficient the model returns.
    pub htc: f64,
    /// `q_SC = f (q_NB - q_FC)` when the cell is in the boiling branch (else `None`: `dmdtW` of
    /// the cell is not written).
    pub q_subcooled_dmdt: Option<f64>,
}

/// `multiRegimeBoilingTRACE::value` for one cell.
///
/// * `t_onb(htc_2p_fc)` is `TONBModel::value(celli, htc2pFCi)`;
/// * `htc_pool_boiling(t_wall)` is `htcPBPtr_->value(celli)` with the wall temperature field of the
///   cell set to `t_wall`;
/// * `sub_cooled_fraction(q_nb)` is `SCBFPtr_->value(celli, qNBi)`, present when the dictionary has
///   a `subCooledBoilingFractionModel` (`None` is `f = 1`).
pub fn multi_regime_boiling_trace(
    inputs: &TraceBoilingInputs,
    t_onb: impl FnOnce(f64) -> f64,
    htc_pool_boiling: impl Fn(f64) -> f64,
    sub_cooled_fraction: Option<impl Fn(f64) -> f64>,
) -> TraceBoilingResult {
    let alphai = inputs.alpha_other;
    let twi = inputs.t_wall;
    let tfi = inputs.t_fluid;
    let tsati = inputs.t_sat;
    let htc2p_fci = inputs.htc_2p_fc;
    let q_fci = htc2p_fci * (twi - tfi);
    if twi < tsati {
        let htc = match inputs.htc_film_condensation {
            None => htc2p_fci,
            Some(htc_cndi) => {
                if alphai <= 0.8 {
                    htc2p_fci
                } else if alphai < 0.9 {
                    // NOTE(port): upstream's literal `0.9-alphai/0.1`.
                    let f = 0.9 - alphai / 0.1;
                    f * htc2p_fci + (1.0 - f) * htc_cndi
                } else {
                    htc_cndi
                }
            }
        };
        return TraceBoilingResult { htc, q_subcooled_dmdt: None };
    }
    let tonbi = if inputs.has_onb_model { t_onb(htc2p_fci) } else { tsati };
    if twi < tonbi {
        return TraceBoilingResult { htc: htc2p_fci, q_subcooled_dmdt: None };
    }
    let tchfi = 1e69;
    if twi < tchfi {
        let hpb_onbi = htc_pool_boiling(tonbi);
        let hpbi = htc_pool_boiling(twi);
        let q_bii = hpb_onbi * (tonbi - tsati);
        let q_pbi = hpbi * (twi - tsati);
        let q_nbi = (q_fci.r_powf(3.0) + (q_pbi - q_bii).r_powf(3.0)).r_powf(1.0 / 3.0);
        let f = match sub_cooled_fraction {
            Some(fr) => fr(q_nbi),
            None => 1.0,
        };
        let q_scdmdti = f * (q_nbi - q_fci);
        let mut dt = twi - tfi;
        dt = if dt >= 0.0 { dt.max(1e-3) } else { (-dt).min(-1e-3) };
        TraceBoilingResult { htc: q_nbi / dt, q_subcooled_dmdt: Some(q_scdmdti) }
    } else {
        TraceBoilingResult { htc: 1.0, q_subcooled_dmdt: None }
    }
}
