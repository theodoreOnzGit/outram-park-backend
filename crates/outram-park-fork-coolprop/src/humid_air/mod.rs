//! Humid (moist) air properties — the CoolProp `HAPropsSI` backend.
//!
//! **Scaffold only (bead op-kbc.14).** Types and signatures are laid out; the
//! bodies are `todo!()`. Nothing here is wired into the crate's public API yet.
//!
//! # What this models
//!
//! Humid air is treated as a **real-gas mixture of dry air and water vapour**
//! following ASHRAE RP-1485 / Herrmann–Kretzschmar–Gatley (2009), the same
//! formulation CoolProp's `HumidAirProp.cpp` implements:
//!
//! - dry air and water each from their pure-fluid Helmholtz EOS
//!   ([`crate::fluid::Fluid::Air`], [`crate::fluid::Fluid::Water`]),
//! - cross-interaction through the second/third **virial coefficients**
//!   (`B_aw`, `C_aaw`, `C_aww`; see [`virials`]) and the **enhancement factor**
//!   `f` that corrects the saturation water mole fraction for the presence of
//!   air (see [`saturation`]).
//!
//! A humid-air state has **three** independent intensive properties (dry air is
//! a second component), so — unlike a pure fluid — every query fixes three
//! inputs, typically `(T, p, W)` or `(T, p, R)`.
//!
//! # Units
//!
//! Mass-based SI throughout, **per kilogram of _dry_ air** for the extensive
//! specific properties (`H`, `S`, `V`), matching `HAPropsSI`:
//! temperature K, pressure Pa, humidity ratio `W` kg-water/kg-dry-air,
//! relative humidity `R` \[0, 1\], `H` J/kg-dry-air, `S` J/(kg-dry-air·K),
//! `V` m³/kg-dry-air.
#![allow(dead_code, unused_variables, unused_imports)] // TODO(op-kbc.14): drop once implemented

pub mod saturation;
pub mod virials;

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
    /// Wet-bulb temperature `T_wb` \[K\] (CoolProp `B`/`Twb`).
    TWetBulb,
    /// Dew-point temperature `T_dp` \[K\] (CoolProp `D`/`Tdp`).
    TDewPoint,
    /// Mixture specific enthalpy per kg dry air \[J/kg\] (CoolProp `H`/`Hda`).
    Enthalpy,
    /// Mixture specific entropy per kg dry air \[J/(kg·K)\] (CoolProp `S`/`Sda`).
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
    /// Specific entropy \[J/(kg dry air·K)\].
    pub entropy: f64,
    /// Specific volume \[m³/kg dry air\].
    pub volume: f64,
}

/// Failure modes of a humid-air solve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HumidAirError {
    /// The requested input set is not a solvable/supported triple.
    UnsupportedInputs,
    /// An inner iteration (wet-bulb / dew-point / `W`-from-`H`) did not converge.
    NonConvergent,
    /// An input is out of the valid range (`T ∈ [~200, 473] K`, `p` > 0, …).
    OutOfRange,
}

/// One `(parameter, value)` input constraint for [`ha_props`].
pub type HaInput = (HumidAirParam, f64);

/// Evaluate a humid-air `output` property from three input constraints — the
/// typed analogue of CoolProp's
/// `HAPropsSI(output, k1, v1, k2, v2, k3, v3)`.
///
/// Humid air has three degrees of freedom, so exactly three inputs are
/// required (conventionally two of `T`/`p` plus one humidity measure). Returns
/// the `output` property in the SI units documented on [`HumidAirParam`].
///
/// # Errors
/// [`HumidAirError`] if the input triple is unsupported, out of range, or an
/// inner solve fails to converge.
pub fn ha_props(
    output: HumidAirParam,
    in1: HaInput,
    in2: HaInput,
    in3: HaInput,
) -> Result<f64, HumidAirError> {
    // TODO(op-kbc.14): normalise the three inputs to a base (T, p, ψ_w) triple
    // (solving wet-bulb / dew-point / R / W / H as needed), then read `output`
    // off the resolved `HumidAirState`.
    let _state = solve_state(in1, in2, in3)?;
    todo!("op-kbc.14: read `output` from the resolved HumidAirState")
}

/// Solve the base humid-air state from three input constraints.
///
/// The core dispatcher: recognises the supported input combinations, inverts
/// the humidity measure to the water mole fraction `ψ_w`, and fills every field
/// of [`HumidAirState`].
fn solve_state(in1: HaInput, in2: HaInput, in3: HaInput) -> Result<HumidAirState, HumidAirError> {
    // TODO(op-kbc.14): match on the (sorted) input-parameter set; for the common
    // (T, p, {W|R|Tdp|Twb|ψ_w}) cases compute ψ_w, then derive W, R, H, S, V via
    // the real-gas mixing model (virials + enhancement factor).
    todo!("op-kbc.14: humid-air input dispatch + state assembly")
}

/// Humidity ratio `W` \[kg water / kg dry air\] from the water mole fraction
/// `psi_w` \[-\]. `W = (M_w/M_a) · ψ_w/(1 − ψ_w)`.
fn humidity_ratio_from_psi_w(psi_w: f64) -> f64 {
    // TODO(op-kbc.14): use molar masses of water and dry air from the EOS.
    todo!("op-kbc.14: W(ψ_w)")
}

/// Saturation water mole fraction `ψ_{w,s}(T, p)` \[-\] including the
/// enhancement factor `f` — the anchor for relative humidity `R = ψ_w/ψ_{w,s}`.
fn saturation_mole_fraction(t: f64, p: f64) -> Result<f64, HumidAirError> {
    // TODO(op-kbc.14): ψ_{w,s} = f(T,p) · p_ws(T) / p, with f from
    // `saturation::enhancement_factor` and p_ws from `saturation::p_ws`.
    todo!("op-kbc.14: saturated water mole fraction")
}
