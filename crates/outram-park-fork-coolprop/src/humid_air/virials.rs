//! Second and third **virial coefficients** for the dry-air / water-vapour
//! mixture, following ASHRAE RP-1485 (Herrmann et al., 2009) — the cross terms
//! that make humid air a *real*-gas mixture rather than two ideal gases.
//!
//! **Scaffold only (bead op-kbc.14).** All functions are `todo!()`.
//!
//! Each coefficient is a function of temperature `T` \[K\]. Naming: `a` = dry
//! air, `w` = water. The mixture second virial is
//! `B_mix = ψ_a² B_aa + 2 ψ_a ψ_w B_aw + ψ_w² B_ww`, and likewise for the third
//! virial `C_mix` in the pure/cross coefficients below.
#![allow(dead_code, unused_variables)] // TODO(op-kbc.14): drop once implemented

/// Second virial coefficient of dry air `B_aa(T)` \[m³/mol\].
pub fn b_aa(t: f64) -> f64 {
    todo!("op-kbc.14: B_aa(T)")
}

/// Second virial coefficient of water vapour `B_ww(T)` \[m³/mol\].
pub fn b_ww(t: f64) -> f64 {
    todo!("op-kbc.14: B_ww(T)")
}

/// **Cross** second virial coefficient air–water `B_aw(T)` \[m³/mol\]
/// (ASHRAE RP-1485 Eq. for the interaction term).
pub fn b_aw(t: f64) -> f64 {
    todo!("op-kbc.14: B_aw(T)")
}

/// Third virial coefficient of dry air `C_aaa(T)` \[m⁶/mol²\].
pub fn c_aaa(t: f64) -> f64 {
    todo!("op-kbc.14: C_aaa(T)")
}

/// Third virial coefficient of water vapour `C_www(T)` \[m⁶/mol²\].
pub fn c_www(t: f64) -> f64 {
    todo!("op-kbc.14: C_www(T)")
}

/// **Cross** third virial coefficient `C_aaw(T)` \[m⁶/mol²\] (two air, one
/// water).
pub fn c_aaw(t: f64) -> f64 {
    todo!("op-kbc.14: C_aaw(T)")
}

/// **Cross** third virial coefficient `C_aww(T)` \[m⁶/mol²\] (one air, two
/// water).
pub fn c_aww(t: f64) -> f64 {
    todo!("op-kbc.14: C_aww(T)")
}
