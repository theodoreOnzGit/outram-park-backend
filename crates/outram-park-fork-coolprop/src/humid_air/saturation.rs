//! Water-side saturation quantities for the humid-air model: the saturation
//! pressure of water over ice/liquid, the **enhancement factor** `f`, and the
//! Henry's-law solubility of air in the condensed phase.
//!
//! **Scaffold only (bead op-kbc.14).** All functions are `todo!()`.
//!
//! These close the humid-air saturation condition. The saturated water mole
//! fraction is `ψ_{w,s} = f(T, p) · p_ws(T) / p`, where `p_ws` is the pure-water
//! saturation pressure and `f ≥ 1` corrects it for the dissolved air and the
//! non-ideality of the gas phase (ASHRAE RP-1485 / Nelson–Sauer).
#![allow(dead_code, unused_variables)] // TODO(op-kbc.14): drop once implemented

/// Saturation pressure of pure water `p_ws(T)` \[Pa\].
///
/// Over liquid water (`T ≥ 273.16 K`) this should defer to the crate's own
/// water VLE ([`crate::vle::saturation_at_temperature`] on [`crate::fluid::Fluid::Water`]);
/// over ice (`T < 273.16 K`) it uses the IAPWS ice-Ih sublimation curve.
pub fn p_ws(t: f64) -> f64 {
    // TODO(op-kbc.14): branch liquid vs ice; reuse crate::vle for the liquid arm.
    todo!("op-kbc.14: p_ws(T)")
}

/// The dimensionless **enhancement factor** `f(T, p) ≥ 1`.
///
/// Iterative in CoolProp: `f` depends on the virial coefficients
/// ([`super::virials`]), the isothermal compressibility of the condensed water,
/// and the Henry constant ([`henry_constant`]). Converges in a handful of
/// fixed-point steps.
pub fn enhancement_factor(t: f64, p: f64) -> f64 {
    // TODO(op-kbc.14): ASHRAE RP-1485 enhancement-factor fixed-point iteration.
    todo!("op-kbc.14: enhancement factor f(T, p)")
}

/// Henry's-law constant for the solubility of dry air in liquid water
/// `k_H(T)` \[1/Pa\] — used by [`enhancement_factor`] over liquid water.
pub fn henry_constant(t: f64) -> f64 {
    todo!("op-kbc.14: Henry constant k_H(T)")
}
