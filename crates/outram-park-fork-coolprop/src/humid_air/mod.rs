//! Humid (moist) air properties — the CoolProp `HAPropsSI` backend.
//!
//! Ported from CoolProp `HumidAirProp.cpp`, following ASHRAE RP-1485 /
//! Herrmann–Kretzschmar–Gatley (2009): humid air as a **real-gas mixture** of
//! dry air and water vapour, virial-corrected via [`virials`] and closed by
//! the saturation **enhancement factor** in [`saturation`].
//!
//! # Coverage
//!
//! - **Inputs**: `(T, p, W)` or `(T, p, R)` — dry-bulb temperature, pressure,
//!   plus one humidity measure (humidity ratio or relative humidity). This is
//!   the input triple most psychrometric/HVAC and FHR-secondary-loop-air
//!   calculations use.
//! - **Outputs**: `T`, `p`, `ψ_w`, `W`, `R`, specific enthalpy `H`, specific
//!   volume `V` (both per kg dry air), and the water partial pressure `p_w`.
//! - **Range**: liquid-water branch only, `T > 273.16 K` (the IAPWS triple
//!   point) — the ice-sublimation branch is not ported (see [`saturation`]).
//!
//! **Not implemented** (bead op-kbc.14 follow-up): specific entropy `S`
//! (needs the ideal-gas reference-state offset calibration CoolProp computes
//! once against IAPWS/Lemmon reference points — a separate porting effort),
//! wet-bulb temperature `T_wb` and dew-point temperature `T_dp` (Brent-solved
//! against `T_wb`/`T_dp` in CoolProp; not ported), and any input triple that
//! is not `(T, p, {W|R|ψ_w})`. [`ha_props`] returns
//! [`HumidAirError::UnsupportedInputs`] for these — never a wrong number.
//!
//! # Units
//!
//! Mass-based SI throughout, **per kilogram of _dry_ air** for the extensive
//! specific properties (`H`, `V`), matching `HAPropsSI`:
//! temperature K, pressure Pa, humidity ratio `W` kg-water/kg-dry-air,
//! relative humidity `R` \[0, 1\], `H` J/kg-dry-air, `V` m³/kg-dry-air.

pub mod saturation;
pub mod virials;

use saturation::{MM_AIR, MM_WATER, R_BAR};

/// A humid-air intensive property — usable as either an input key or an output
/// selector, mirroring `HAPropsSI`'s string parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HumidAirParam {
    /// Dry-bulb temperature `T` \[K\].
    TDryBulb,
    /// Pressure `p` \[Pa\].
    Pressure,
    /// Humidity ratio `W` \[kg water / kg dry air\] (CoolProp `W`/`Omega`).
    HumidityRatio,
    /// Relative humidity `R` \[0, 1\] (CoolProp `R`/`RH`).
    RelativeHumidity,
    /// Water-vapour mole fraction `ψ_w` \[-\] (CoolProp `psi_w`/`Y`).
    WaterMoleFraction,
    /// Wet-bulb temperature `T_wb` \[K\] (CoolProp `B`/`Twb`). **Not
    /// implemented** — see the module doc.
    TWetBulb,
    /// Dew-point temperature `T_dp` \[K\] (CoolProp `D`/`Tdp`). **Not
    /// implemented** — see the module doc.
    TDewPoint,
    /// Mixture specific enthalpy per kg dry air \[J/kg\] (CoolProp `H`/`Hda`).
    Enthalpy,
    /// Mixture specific entropy per kg dry air \[J/(kg·K)\] (CoolProp
    /// `S`/`Sda`). **Not implemented** — see the module doc.
    Entropy,
    /// Mixture specific volume per kg dry air \[m³/kg\] (CoolProp `V`/`Vda`).
    Volume,
    /// Partial pressure of water vapour `p_w` \[Pa\] (CoolProp `P_w`).
    WaterPartialPressure,
}

/// A fully-resolved humid-air state (all properties per kg **dry** air, SI).
///
/// Produced once the three input constraints are solved for the base
/// `(T, p, ψ_w)` triple; every [`HumidAirParam`] is then a direct read.
/// `entropy` is `NaN` (not implemented — see the module doc);
/// [`ha_props`] rejects an `Entropy` output before this value is read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HumidAirState {
    /// Dry-bulb temperature \[K\].
    pub t_dry_bulb: f64,
    /// Total pressure \[Pa\].
    pub pressure: f64,
    /// Water-vapour mole fraction `ψ_w` \[-\].
    pub water_mole_fraction: f64,
    /// Humidity ratio `W` \[kg water / kg dry air\].
    pub humidity_ratio: f64,
    /// Relative humidity `R` \[0, 1\].
    pub relative_humidity: f64,
    /// Specific enthalpy \[J/kg dry air\].
    pub enthalpy: f64,
    /// Specific entropy \[J/(kg dry air·K)\]. **Always `NaN`** — see the
    /// module doc; not implemented.
    pub entropy: f64,
    /// Specific volume \[m³/kg dry air\].
    pub volume: f64,
}

/// Failure modes of a humid-air solve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HumidAirError {
    /// The requested input set is not a solvable/supported triple, or the
    /// requested output is not implemented (`Entropy`, `TWetBulb`, `TDewPoint`).
    UnsupportedInputs,
    /// An inner iteration (the enhancement factor or molar-volume solve) did
    /// not converge.
    NonConvergent,
    /// An input is out of the valid range (`T ≤ 273.16 K`, `p ≤ 0`, …).
    OutOfRange,
}

/// One `(parameter, value)` input constraint for [`ha_props`].
pub type HaInput = (HumidAirParam, f64);

/// Evaluate a humid-air `output` property from three input constraints — the
/// typed analogue of CoolProp's
/// `HAPropsSI(output, k1, v1, k2, v2, k3, v3)`.
///
/// Humid air has three degrees of freedom, so exactly three inputs are
/// required. Only `(T, p, {W|R|ψ_w})` triples are supported (see the module
/// doc); `Entropy`, `TWetBulb`, `TDewPoint` are not implemented as outputs.
///
/// # Errors
/// [`HumidAirError`] if the input triple or requested output is unsupported,
/// an input is out of range, or an inner solve fails to converge.
pub fn ha_props(output: HumidAirParam, in1: HaInput, in2: HaInput, in3: HaInput) -> Result<f64, HumidAirError> {
    if matches!(output, HumidAirParam::Entropy | HumidAirParam::TWetBulb | HumidAirParam::TDewPoint) {
        return Err(HumidAirError::UnsupportedInputs);
    }
    let state = solve_state(in1, in2, in3)?;
    Ok(match output {
        HumidAirParam::TDryBulb => state.t_dry_bulb,
        HumidAirParam::Pressure => state.pressure,
        HumidAirParam::HumidityRatio => state.humidity_ratio,
        HumidAirParam::RelativeHumidity => state.relative_humidity,
        HumidAirParam::WaterMoleFraction => state.water_mole_fraction,
        HumidAirParam::Enthalpy => state.enthalpy,
        HumidAirParam::Volume => state.volume,
        HumidAirParam::WaterPartialPressure => state.water_mole_fraction * state.pressure,
        HumidAirParam::Entropy | HumidAirParam::TWetBulb | HumidAirParam::TDewPoint => unreachable!(),
    })
}

/// Solve the base humid-air state from three input constraints.
///
/// Recognises `(T, p, {W|R|ψ_w})` (in any order), inverts the humidity
/// measure to the water mole fraction `ψ_w`, then fills every field of
/// [`HumidAirState`] except `entropy` (`NaN`, not implemented).
fn solve_state(in1: HaInput, in2: HaInput, in3: HaInput) -> Result<HumidAirState, HumidAirError> {
    let inputs = [in1, in2, in3];
    let t = find(&inputs, HumidAirParam::TDryBulb).ok_or(HumidAirError::UnsupportedInputs)?;
    let p = find(&inputs, HumidAirParam::Pressure).ok_or(HumidAirError::UnsupportedInputs)?;
    if !(t.is_finite() && t > saturation::T_TRIPLE) {
        return Err(HumidAirError::OutOfRange);
    }
    if !(p.is_finite() && p > 0.0) {
        return Err(HumidAirError::OutOfRange);
    }

    let epsilon = MM_WATER / MM_AIR;
    let psi_w = if let Some(w) = find(&inputs, HumidAirParam::HumidityRatio) {
        w / (epsilon + w)
    } else if let Some(r) = find(&inputs, HumidAirParam::RelativeHumidity) {
        r * saturation_mole_fraction(t, p)?
    } else if let Some(psi) = find(&inputs, HumidAirParam::WaterMoleFraction) {
        psi
    } else {
        return Err(HumidAirError::UnsupportedInputs);
    };
    if !(psi_w.is_finite() && (0.0..1.0).contains(&psi_w)) {
        return Err(HumidAirError::OutOfRange);
    }

    let w = humidity_ratio_from_psi_w(psi_w);
    let psi_ws = saturation_mole_fraction(t, p)?;
    let r = psi_w / psi_ws;

    let vbar = molar_volume(t, p, psi_w)?;
    let hbar = molar_enthalpy(t, psi_w, vbar);
    let m_ha = MM_WATER * psi_w + (1.0 - psi_w) * MM_AIR; // kg_ha / mol_ha

    Ok(HumidAirState {
        t_dry_bulb: t,
        pressure: p,
        water_mole_fraction: psi_w,
        humidity_ratio: w,
        relative_humidity: r,
        enthalpy: hbar * (1.0 + w) / m_ha,
        entropy: f64::NAN,
        volume: vbar * (1.0 + w) / m_ha,
    })
}

fn find(inputs: &[HaInput; 3], key: HumidAirParam) -> Option<f64> {
    inputs.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

/// Humidity ratio `W` \[kg water / kg dry air\] from the water mole fraction
/// `psi_w` \[-\]. `W = ψ_w·(M_w/M_a) / (1 − ψ_w)` (RP-1485 Eq. 3.2).
fn humidity_ratio_from_psi_w(psi_w: f64) -> f64 {
    psi_w * (MM_WATER / MM_AIR) / (1.0 - psi_w)
}

/// Saturation water mole fraction `ψ_{w,s}(T, p)` \[-\] including the
/// enhancement factor `f` (RP-1485 Eq. 3.24): `ψ_{w,s} = f(T,p) · p_ws(T)/p`.
fn saturation_mole_fraction(t: f64, p: f64) -> Result<f64, HumidAirError> {
    let p_ws = saturation::p_ws(t)?;
    let f = saturation::enhancement_factor(t, p)?;
    Ok(f * p_ws / p)
}

/// Molar volume of humid air `v̄(T, p, ψ_w)` \[m³/mol_ha\] — secant solve of
/// the virial-truncated real-gas equation of state
/// `p = R̄T/v̄ · (1 + B_m/v̄ + C_m/v̄²)` (RP-1485 Eq. 3.3).
fn molar_volume(t: f64, p: f64, psi_w: f64) -> Result<f64, HumidAirError> {
    let bm = virials::b_mix(t, psi_w);
    let cm = virials::c_mix(t, psi_w);
    let residual = |v: f64| (p - R_BAR * t / v * (1.0 + bm / v + cm / (v * v))) / p;

    let v0 = R_BAR * t / p;
    let mut x1 = v0;
    let mut x2 = v0 + 1e-6;
    let mut y1 = residual(x1);
    let mut v = x2;
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
        v = x2;
        if change < 1e-11 {
            break;
        }
    }
    if v.is_finite() && v > 0.0 {
        Ok(v)
    } else {
        Err(HumidAirError::NonConvergent)
    }
}

/// Ideal-gas molar enthalpy of water vapour \[J/mol\] — CoolProp's simplified
/// polynomial correlation (`FlagUseIdealGasEnthalpyCorrelations`), used in
/// place of the full IAPWS-95 `α⁰` + reference-state-offset evaluation.
fn ideal_gas_molar_enthalpy_water(t: f64) -> f64 {
    2.7030251618e-03 * t * t + 3.1994361015e+01 * t + 3.6123174929e+04
}

/// Ideal-gas molar enthalpy of dry air \[J/mol\] — CoolProp's simplified
/// polynomial correlation (`FlagUseIdealGasEnthalpyCorrelations`).
fn ideal_gas_molar_enthalpy_air(t: f64) -> f64 {
    9.2486716590e-04 * t * t + 2.8557221776e+01 * t - 7.8616129429e+03
}

/// Mixture molar enthalpy `h̄(T, ψ_w, v̄)` \[J/mol_ha\] (RP-1485 Eq. 3.4):
/// mole-fraction-weighted ideal-gas enthalpies plus the virial correction.
fn molar_enthalpy(t: f64, psi_w: f64, vbar: f64) -> f64 {
    let hbar_w = ideal_gas_molar_enthalpy_water(t);
    let hbar_a = ideal_gas_molar_enthalpy_air(t);
    let bm = virials::b_mix(t, psi_w);
    let d_bm_dt = virials::d_b_mix_dt(t, psi_w);
    let cm = virials::c_mix(t, psi_w);
    let d_cm_dt = virials::d_c_mix_dt(t, psi_w);
    (1.0 - psi_w) * hbar_a
        + psi_w * hbar_w
        + R_BAR * t * ((bm - t * d_bm_dt) / vbar + (cm - t / 2.0 * d_cm_dt) / (vbar * vbar))
}
