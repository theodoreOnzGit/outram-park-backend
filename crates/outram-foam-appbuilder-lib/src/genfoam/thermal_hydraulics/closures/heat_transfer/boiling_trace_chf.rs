// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/heatTransferModels/
//             FSHeatTransferCoefficientModels/{multiRegimeBoilingTRACECHF,
//             multiRegimeBoilingVapourTRACE}/*FSHeatTransferCoefficient.{H,C}
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL), Thomas Guilbaud (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `boiling_trace_chf` — `multiRegimeBoilingTRACECHF` and `multiRegimeBoilingVapourTRACE` (gh:#1222)
//!
//! One cell of upstream's liquid-side TRACE regime map with a critical-heat-flux branch, and of its
//! vapour-side companion. Pure functions on plain scalars; the side models (ONB, pool boiling,
//! sub-cooled fraction, Leidenfrost, annular film boiling) are closures evaluated only on the branches
//! that call them, as upstream does.
//!
//! ## What upstream does that this keeps
//!
//! * `TCHF = T_sat + q_CHF^(1-n) q0^n / h0 / F / A`, the wall temperature at which the Gorenflo pool
//!   boiling coefficient delivers `q_CHF` (`pR = min(p/p_crit, 0.99999)`); the pool-boiling model is
//!   assumed to be Gorenflo whatever the dictionary says (upstream's own comment: "will not be always
//!   valid if the model for PB is changed"). Without a `criticalHeatFluxModel` `q_CHF = 1e15`.
//! * the model writes three FIELDS as a side effect (`CHFBoolMultiRegimeBoilingTRACECHF`,
//!   `wfTransitionBoiling`, `LeidenfrostTemperature`: 0 pre-CHF, 1 post-CHF film boiling, 2
//!   transition), reset to 0 at the start of every call, and `dmdtW`; the vapour-side model READS them
//!   ([`multi_regime_boiling_vapour_trace`]). [`TraceChfResult`] returns them.
//! * the film-condensation blend `0.9 - alpha/0.1` (division binds tighter) as in [`super::boiling`].
//! * in the transition branch the annular-flow model is evaluated with the wall temperature set to
//!   `T_LF` and its `hGammaCachard` field is read AFTER that evaluation (the model writes it): the
//!   annular closure therefore returns `(h_FB, h_gamma)` as one evaluation. When there is no annular
//!   model upstream uses `h_FB = 0` and a STALE `hGammaCachard` field (zero until a model writes it);
//!   pass `(0, 0)` for that.
//!
//! ## What is NOT here
//!
//! ~~The annular-flow models themselves (`CachardLiquid`, `CachardVapour`: gap-audit item 6, whose upstream
//! binds a pitch to the PRESSURE field) and `lookUpTableCHF` (the Groeneveld table, deferred to the
//! maintainer: gh:#1223). With only `constantCHF` a case reaches the post-CHF branches only through
//! its `q_CHF`.~~ **CORRECTED 2026-11-26** (gh:#1223, gh:#1224, gh:#1361 round 10): `CachardLiquid`/`CachardVapour`
//! are [`super::cachard`] (upstream's pressure-as-pitch binding is the default, the pitch a named option), `lookUpTableCHF`
//! is [`super::chf_table`] (the table is case data, maintainer decision of 2026-11-26), and both pure functions here are wired
//! into the `twoPhase` solver (`FsHtcModel::MultiRegimeBoilingTraceChf` / `MultiRegimeBoilingVapourTrace`) and compared with the
//! real binary on closed `1D_CHF` variants.
//!
//! The Leidenfrost model is [`super::chf::LeidenfrostTemperature`] and the wall temperature seen by
//! the side models is the argument of each closure.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use petir::mathf::RealMath;

/// Per-cell inputs of [`multi_regime_boiling_trace_chf`] (SI, plain scalars).
#[derive(Debug, Clone, Copy)]
pub struct TraceChfInputs {
    /// `alphai = 1 - fluid.normalized`.
    pub alpha_other: f64,
    /// `Twall`.
    pub t_wall: f64,
    /// Fluid temperature.
    pub t_fluid: f64,
    /// `T.interface`.
    pub t_sat: f64,
    /// Pressure.
    pub p: f64,
    /// Active heated area per volume `iAact`.
    pub area: f64,
    /// `mag(L)`.
    pub latent_heat: f64,
    /// `htcFC * F`.
    pub htc_2p_fc: f64,
    /// Film-condensation coefficient when a `filmCondensationModel` exists.
    pub htc_film_condensation: Option<f64>,
    /// Whether a `nucleateBoilingOnsetModel` exists.
    pub has_onb_model: bool,
    /// `criticalHeatFluxModel` value; `None` is upstream's `1e15`.
    pub q_chf: Option<f64>,
    /// `absoluteSurfaceRoughness` (default `4e-7`).
    pub roughness: f64,
}

/// Result of one cell: the coefficient and the fields the model writes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TraceChfResult {
    /// The heat-transfer coefficient.
    pub htc: f64,
    /// `dmdtW[celli]` when the branch writes it (otherwise the field is left as it was).
    pub dmdt_w: Option<f64>,
    /// `CHFBoolMultiRegimeBoilingTRACECHF`: 0 pre-CHF, 1 post-CHF, 2 transition.
    pub chf_bool: f64,
    /// `wfTransitionBoiling`.
    pub wf_tb: f64,
    /// `LeidenfrostTemperature` (0 unless post-CHF).
    pub t_leidenfrost: f64,
}

/// `TCHF` of the model: the wall temperature at which Gorenflo's `h_PB (T - T_sat)` equals `q_chf`.
#[must_use]
pub fn gorenflo_t_chf(t_sat: f64, p: f64, q_chf: f64, roughness: f64) -> f64 {
    let p_crit = 2.209e7;
    let h0 = 5600.0;
    let q0 = 20000.0;
    let r0 = 4e-7;
    let a = (roughness / r0).r_powf(0.133);
    let mut p_r = p / p_crit;
    p_r = p_r.min(0.99999);
    let f = 1.73 * p_r.r_powf(0.27) + (6.1 + 0.68 / (1.0 - p_r)) * (p_r * p_r);
    let n = 0.9 - 0.3 * p_r.r_powf(0.15);
    t_sat + q_chf.r_powf(1.0 - n) * q0.r_powf(n) / h0 / f / a
}

/// `multiRegimeBoilingTRACECHF::value` for one cell.
///
/// * `t_onb(htc2pFC)`: `TONBModel::value`;
/// * `htc_pool_boiling(t_wall)`: `htcPBPtr_->value(celli)` with the cell's wall temperature set to `t_wall`;
/// * `sub_cooled_fraction(q_nb)`: `SCBFPtr_->value`, `None` is `f = 1`;
/// * `t_leidenfrost()`: `TLFPtr_->value(celli)`, `None` is `T_LF = T_wall` (no hysteresis zone);
/// * `annular(t_wall) -> (h_FB, h_gamma)`: the annular-flow model at the given wall temperature.
#[allow(clippy::too_many_arguments)]
pub fn multi_regime_boiling_trace_chf(
    inputs: &TraceChfInputs,
    t_onb: impl FnOnce(f64) -> f64,
    htc_pool_boiling: impl Fn(f64) -> f64,
    sub_cooled_fraction: Option<impl Fn(f64) -> f64>,
    t_leidenfrost: Option<impl FnOnce() -> f64>,
    annular: impl Fn(f64) -> (f64, f64),
) -> TraceChfResult {
    let alphai = inputs.alpha_other;
    let twi = inputs.t_wall;
    let tfi = inputs.t_fluid;
    let tsati = inputs.t_sat;
    let areai = inputs.area;
    let li = inputs.latent_heat;
    let htc2p_fci = inputs.htc_2p_fc;
    let q_fci = htc2p_fci * (twi - tfi);
    let plain = |htc: f64| TraceChfResult { htc, dmdt_w: None, chf_bool: 0.0, wf_tb: 0.0, t_leidenfrost: 0.0 };
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
        return plain(htc);
    }
    let tonbi = if inputs.has_onb_model { t_onb(htc2p_fci) } else { tsati };
    if twi < tonbi {
        return plain(htc2p_fci);
    }
    let q_chfi = inputs.q_chf.unwrap_or(1e15);
    let t_chf = gorenflo_t_chf(tsati, inputs.p, q_chfi, inputs.roughness);
    let mut dt = twi - tfi;
    dt = if dt >= 0.0 { dt.max(1e-3) } else { dt.min(-1e-3) };
    if twi < t_chf {
        let hpbi = htc_pool_boiling(twi);
        let q_pbi = hpbi * (twi - tsati);
        let hpb_onbi = htc_pool_boiling(tonbi);
        let q_bii = hpb_onbi * (tonbi - tsati);
        let q_nbi = (q_fci.r_powf(3.0) + (q_pbi - q_bii).r_powf(3.0)).r_powf(1.0 / 3.0);
        let f = match sub_cooled_fraction {
            Some(fr) => fr(q_nbi),
            None => 1.0,
        };
        let q_scdmdti = f * (q_nbi - q_fci);
        let dmdt_w = areai * q_scdmdti / li;
        return TraceChfResult { htc: q_nbi / dt, dmdt_w: Some(dmdt_w), chf_bool: 0.0, wf_tb: 0.0, t_leidenfrost: 0.0 };
    }
    // post-CHF
    let tlfi = match t_leidenfrost {
        Some(m) => m(),
        None => twi,
    };
    if twi < tlfi {
        // transition boiling
        let (h_fb_min, h_gamma_min) = annular(tlfi);
        let mut d_tchf = t_chf - tlfi;
        d_tchf = if d_tchf >= 0.0 { d_tchf.max(1e-6) } else { d_tchf.min(-1e-6) };
        let r = (twi - tlfi) / d_tchf;
        let wf_tb = r * r;
        let mut dt = twi - tfi;
        dt = if dt >= 0.0 { dt.max(1e-6) } else { dt.min(-1e-6) };
        let hwl_tb = q_chfi * wf_tb / dt;
        let dmdt_w = (q_chfi * areai * wf_tb + areai * (1.0 - wf_tb) * h_gamma_min * (twi - tfi)) / li.max(1e-6);
        TraceChfResult {
            htc: hwl_tb + (1.0 - wf_tb) * h_fb_min,
            dmdt_w: Some(dmdt_w),
            chf_bool: 2.0,
            wf_tb,
            t_leidenfrost: tlfi,
        }
    } else {
        // film boiling (inverted annular flow)
        let (h_fb, h_gamma) = annular(twi);
        let dmdt_w = areai * h_gamma * (twi - tfi) / li.max(1e-6);
        TraceChfResult { htc: h_fb, dmdt_w: Some(dmdt_w), chf_bool: 1.0, wf_tb: 0.0, t_leidenfrost: tlfi }
    }
}

/// `multiRegimeBoilingVapourTRACE::value` for one cell: the vapour-side coefficient from the fields
/// the liquid-side model wrote. `annular(t_wall)` is `htcAFPtr_->value(celli)` at the given wall
/// temperature; upstream dereferences it without a null check, so it is required whenever
/// `chf_bool >= 1`.
pub fn multi_regime_boiling_vapour_trace(
    t_wall: f64,
    chf_bool: f64,
    wf_tb: f64,
    t_leidenfrost: f64,
    annular: impl Fn(f64) -> f64,
) -> f64 {
    let _ = t_wall;
    if chf_bool >= 1.0 {
        if chf_bool == 1.0 {
            annular(t_wall)
        } else {
            let h_fb_min = annular(t_leidenfrost);
            (1.0 - wf_tb) * h_fb_min
        }
    } else {
        0.0
    }
}
