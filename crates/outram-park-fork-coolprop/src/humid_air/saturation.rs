//! Water-side saturation quantities for the humid-air model: the saturation
//! pressure of water, the **enhancement factor** `f`, and the Henry's-law
//! solubility of air in liquid water.
//!
//! Ported from CoolProp `HumidAirProp.cpp` (`f_factor`, `HenryConstant`,
//! `isothermal_compressibility`). **Liquid-water branch only** (`T > 273.16 K`
//! — the IAPWS triple-point temperature); the ice-sublimation branch
//! (`psub_Ice`/`dg_dp_Ice`) is not ported, so [`enhancement_factor`] and
//! [`p_ws`] return [`super::HumidAirError::OutOfRange`] below the triple point.
//! `k_T` uses CoolProp's polynomial `isothermal_compressibility` surrogate
//! (its `FlagUseIsothermCompressCorrelation` branch) unconditionally, rather
//! than the near-saturation IF97 special case — the same simplification
//! CoolProp itself offers as an equivalent alternative.

use super::HumidAirError;
use crate::fluid::Fluid;
use crate::vle::saturation_at_temperature;

/// IAPWS triple-point temperature of water \[K\].
pub(super) const T_TRIPLE: f64 = 273.16;
/// Molar mass of water \[kg/mol\] (RP-1485 convention, matches the virial fits).
pub(super) const MM_WATER: f64 = 0.018015268;
/// Molar mass of dry air \[kg/mol\] (RP-1485 convention, Lemmon et al. 2000).
pub(super) const MM_AIR: f64 = 0.028966;
/// Universal gas constant used throughout RP-1485 \[J/(mol·K)\].
pub(super) const R_BAR: f64 = 8.314472;

/// Saturation pressure of liquid water `p_ws(T)` \[Pa\] (`T > 273.16 K`).
///
/// Delegates to the crate's own IAPWS-95 water VLE
/// ([`crate::vle::saturation_at_temperature`]) rather than IF97 — both are
/// full Helmholtz-EOS saturation solves and agree to IAPWS tolerances.
pub fn p_ws(t: f64) -> Result<f64, HumidAirError> {
    if t <= T_TRIPLE {
        return Err(HumidAirError::OutOfRange);
    }
    saturation_at_temperature(Fluid::Water, t)
        .map(|s| s.pressure)
        .ok_or(HumidAirError::NonConvergent)
}

/// Saturated-liquid molar volume `v̄_ws(T)` \[m³/mol\] (`T > 273.16 K`).
fn vbar_ws(t: f64) -> Result<f64, HumidAirError> {
    if t <= T_TRIPLE {
        return Err(HumidAirError::OutOfRange);
    }
    let s = saturation_at_temperature(Fluid::Water, t).ok_or(HumidAirError::NonConvergent)?;
    Ok(MM_WATER / s.rho_liquid)
}

/// Henry's-law constant for the solubility of dry air in liquid water
/// `k_H(T)` \[1/Pa\] (RP-1485 Eq. 3.36, via N₂/O₂/Ar Henry constants and the
/// standard dry-air composition 0.7812/0.2095/0.0093).
pub fn henry_constant(t: f64) -> f64 {
    let tc = 647.096;
    let tr = t / tc;
    let tau = 1.0 - tr;
    let p_ws = crate::vle::saturation_at_temperature(Fluid::Water, t.min(tc - 1e-6)).map(|s| s.pressure).unwrap_or(f64::NAN);
    let beta_n2 = p_ws * (-9.67578 / tr + 4.72162 * tau.powf(0.355) / tr + 11.70585 * tr.powf(-0.41) * tau.exp()).exp();
    let beta_o2 = p_ws * (-9.44833 / tr + 4.43822 * tau.powf(0.355) / tr + 11.42005 * tr.powf(-0.41) * tau.exp()).exp();
    let beta_ar = p_ws * (-8.40954 / tr + 4.29587 * tau.powf(0.355) / tr + 10.52779 * tr.powf(-0.41) * tau.exp()).exp();
    let beta_a = 1.0 / (0.7812 / beta_n2 + 0.2095 / beta_o2 + 0.0093 / beta_ar);
    1.0 / (1.01325 * beta_a)
}

/// Isothermal compressibility of liquid water `k_T(T)` \[1/Pa\] — CoolProp's
/// polynomial surrogate (`FlagUseIsothermCompressCorrelation`).
fn isothermal_compressibility(t: f64) -> f64 {
    1.6261876614e-22 * t.powi(6) - 3.3016385196e-19 * t.powi(5) + 2.7978984577e-16 * t.powi(4) - 1.2672392901e-13 * t.powi(3)
        + 3.2382864853e-11 * t.powi(2)
        - 4.4318979503e-09 * t
        + 2.5455947289e-07
}

/// The dimensionless **enhancement factor** `f(T, p) ≥ 1` (RP-1485 Eq. 3.25).
///
/// Solved by secant iteration (matching CoolProp's own solve) on
/// `ln f = RHS(f; T, p)`, where the right-hand side collects the
/// isothermal-compressibility, Henry's-law and virial-coefficient
/// corrections. Liquid-water branch only (`T > 273.16 K`).
pub fn enhancement_factor(t: f64, p: f64) -> Result<f64, HumidAirError> {
    if t <= T_TRIPLE {
        return Err(HumidAirError::OutOfRange);
    }
    let p_ws = p_ws(t)?;
    let vbar_ws = vbar_ws(t)?;
    let mut k_t = isothermal_compressibility(t);
    let mut beta_h = henry_constant(t);
    if p_ws > p {
        k_t = 0.0;
        beta_h = 0.0;
    }

    let b_aa = super::virials::b_aa(t);
    let b_aw = super::virials::b_aw(t);
    let b_ww = super::virials::b_ww(t);
    let c_aaa = super::virials::c_aaa(t);
    let c_aaw = super::virials::c_aaw(t);
    let c_aww = super::virials::c_aww(t);
    let c_www = super::virials::c_www(t);

    let rhs = |f: f64| -> f64 {
        let psi_ws = f * p_ws / p;
        let rt = R_BAR * t;
        let line1 = ((1.0 + k_t * p_ws) * (p - p_ws) - k_t * (p * p - p_ws * p_ws) / 2.0) / rt * vbar_ws
            + (1.0 - beta_h * (1.0 - psi_ws) * p).ln();
        let line2 = (1.0 - psi_ws).powi(2) * p / rt * b_aa - 2.0 * (1.0 - psi_ws).powi(2) * p / rt * b_aw
            - (p - p_ws - (1.0 - psi_ws).powi(2) * p) / rt * b_ww;
        let line3 = (1.0 - psi_ws).powi(3) * p * p / (rt * rt) * c_aaa
            + (3.0 * (1.0 - psi_ws).powi(2) * (1.0 - 2.0 * (1.0 - psi_ws)) * p * p) / (2.0 * rt * rt) * c_aaw;
        let line4 = -3.0 * (1.0 - psi_ws).powi(2) * psi_ws * p * p / (rt * rt) * c_aww
            - ((3.0 - 2.0 * psi_ws) * psi_ws * psi_ws * p * p - p_ws * p_ws) / (2.0 * rt * rt) * c_www;
        let line5 = -((1.0 - psi_ws).powi(2) * (-2.0 + 3.0 * psi_ws) * psi_ws * p * p) / (rt * rt) * b_aa * b_ww;
        let line6 = -(2.0 * (1.0 - psi_ws).powi(3) * (-1.0 + 3.0 * psi_ws) * p * p) / (rt * rt) * b_aa * b_aw;
        let line7 = (6.0 * (1.0 - psi_ws).powi(2) * psi_ws * psi_ws * p * p) / (rt * rt) * b_ww * b_aw
            - (3.0 * (1.0 - psi_ws).powi(4) * p * p) / (2.0 * rt * rt) * b_aa * b_aa;
        let line8 = -(2.0 * (1.0 - psi_ws).powi(2) * psi_ws * (-2.0 + 3.0 * psi_ws) * p * p) / (rt * rt) * b_aw * b_aw
            - (p_ws * p_ws - (4.0 - 3.0 * psi_ws) * psi_ws.powi(3) * p * p) / (2.0 * rt * rt) * b_ww * b_ww;
        line1 + line2 + line3 + line4 + line5 + line6 + line7 + line8
    };

    let residual = |f: f64| f.ln() - rhs(f);
    let mut x1 = 1.0_f64;
    let mut x2 = 1.0 + 1e-6;
    let mut y1 = residual(x1);
    let mut f = x2;
    for _ in 0..100 {
        let y2 = residual(x2);
        if (y2 - y1).abs() < 1e-300 {
            break;
        }
        let x3 = x2 - y2 / (y2 - y1) * (x2 - x1);
        let change = (y2 / (y2 - y1) * (x2 - x1)).abs();
        x1 = x2;
        y1 = y2;
        x2 = x3;
        f = x2;
        if change < 1e-8 {
            break;
        }
    }
    Ok(if f >= 1.0 { f } else { 1.0 })
}
