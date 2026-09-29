// SPDX-License-Identifier: GPL-3.0-only
//! Plume rise and building wake, which pyDOSEIA defines but never calls.
//!
//! # Provenance
//!
//! Ported from pyDOSEIA `metfunc.py` (`MetFunc.compute_plume_rise_neutral_unstable_cat`,
//! `compute_plume_rise_stable_cat`, `building_wake_effect_gifford`), upstream
//! <https://github.com/BiswajitSadhu/pyDOSEIA> at commit
//! `dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`. Copyright (c) 2024 Dr. Biswajit
//! Sadhu; MIT licence (full notice in `crates/buangkok/NOTICE`). Upstream cites
//! the AERB/NF/SG/S-1 guide (p. 44) and IAEA-TECDOC-379; neither was
//! available to check the formulas against.
//!
//! None of these is used by upstream's dose calculation (its release height is
//! the effective height, with no rise and no wake), and none is used by this
//! crate's pathways either.
//!
//! | Function | Upstream state | Here |
//! |---|---|---|
//! | [`plume_rise_neutral_unstable`] | runs, never called | ported faithfully |
//! | [`plume_rise_stable_upstream`] | runs, never called, marked TO-DO; defect **D6** | ported faithfully (always class F, second formula) |
//! | [`plume_rise_stable_both_formulas`] | — | **divergence**: D6 corrected |
//! | building wake | cannot run (`TypeError`), marked TO-DO; defect **D5** | [`building_wake_gifford`] is a **divergence** only |

/// Python's `v ** e` on floats: C `pow`, with the exponent hidden from LLVM so
/// it is not strength-reduced.
fn pw(v: f64, e: f64) -> f64 {
    v.powf(core::hint::black_box(e))
}

/// `compute_plume_rise_neutral_unstable_cat(W0, x, U, D_i, D_e)` (classes
/// A-D), m: the smaller of
/// `1.44 D_i (W0/U)^(2/3) (x/D_i)^(1/3) - 3 (1.5 - W0/U) D_e` and
/// `3 D_i W0/U`. Upstream's defaults are `W0 = 10` m/s, `x = 100` m,
/// `U = 2` m/s, `D_i = 5` m, `D_e = 8` m.
#[must_use]
pub fn plume_rise_neutral_unstable(w0: f64, x: f64, u: f64, d_i: f64, d_e: f64) -> f64 {
    let c = 3.0 * (1.5 - (w0 / u)) * d_e;
    let dh_a = 1.44 * d_i * pw(w0 / u, 2.0 / 3.0) * pw(x / d_i, 1.0 / 3.0) - c;
    let dh_b = 3.0 * d_i * (w0 / u);
    // Python's min([a, b]) returns the first on a tie and propagates the
    // first NaN only when it is first; f64::min differs only for NaN.
    if dh_b < dh_a {
        dh_b
    } else {
        dh_a
    }
}

/// Upstream's momentum flux parameter `Fm = W0^2 (D_i/2)^2`.
#[must_use]
pub fn momentum_flux(w0: f64, d_i: f64) -> f64 {
    pw(w0, 2.0) * pw(d_i / 2.0, 2.0)
}

/// `compute_plume_rise_stable_cat(W0, U, D_i)` **as upstream computes it**
/// (defect D6): the stability parameter is assigned for class E and then
/// overwritten with class F's, and the calm formula is computed and then
/// overwritten by the windy one, so the result is always
/// `1.5 S_F^(-1/6) (Fm/U)^(1/3)` with `S_F = 1.75e-3`.
#[must_use]
pub fn plume_rise_stable_upstream(w0: f64, u: f64, d_i: f64) -> f64 {
    let fm = momentum_flux(w0, d_i);
    let s = 1.75e-3;
    1.5 * pw(s, -1.0 / 6.0) * pw(fm / u, 1.0 / 3.0)
}

/// Stable class for [`plume_rise_stable_both_formulas`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StableClass {
    /// Class E, `S = 8.7e-4`.
    E,
    /// Class F, `S = 1.75e-3`.
    F,
}

/// Both stable momentum-rise formulas upstream writes, m.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StablePlumeRise {
    /// `4 (Fm/S)^(1/4)` (upstream's first, overwritten, formula).
    pub calm_formula: f64,
    /// `1.5 S^(-1/6) (Fm/U)^(1/3)`.
    pub windy_formula: f64,
}

/// **Divergence from upstream (D6 corrected):** uses the stability parameter
/// of the class asked for and returns **both** formulas upstream writes,
/// instead of discarding the first. Which one applies (Briggs' guidance takes
/// the smaller) is left to the caller: the AERB guide upstream cites was not
/// available to check what it prescribes.
#[must_use]
pub fn plume_rise_stable_both_formulas(
    w0: f64,
    u: f64,
    d_i: f64,
    class: StableClass,
) -> StablePlumeRise {
    let fm = momentum_flux(w0, d_i);
    let s = match class {
        StableClass::E => 8.7e-4,
        StableClass::F => 1.75e-3,
    };
    StablePlumeRise {
        calm_formula: 4.0 * pw(fm / s, 1.0 / 4.0),
        windy_formula: 1.5 * pw(s, -1.0 / 6.0) * pw(fm / u, 1.0 / 3.0),
    }
}

/// **Divergence from upstream (D5 corrected):** Gifford's building-wake
/// dilution factor `chi/Q = 1 / ((c A + pi sigma_y sigma_z) U)`, `c = 0.5`,
/// floored at one third of the unwaked value, s/m^3.
///
/// Upstream's `building_wake_effect_gifford` cannot run: it multiplies the
/// bound methods `self.sigmay` and `self.sigmaz` (a `TypeError`), and it
/// **multiplies** by `U` where the formula divides. This version takes the
/// sigmas (m) as arguments and divides by the wind speed (m/s); the floor is
/// upstream's. Not checked against the AERB guide or TECDOC-379.
#[must_use]
pub fn building_wake_gifford(
    chi_over_q_unwaked: f64,
    building_area_m2: f64,
    wind_speed_m_per_s: f64,
    sigma_y_m: f64,
    sigma_z_m: f64,
) -> f64 {
    let c = 0.5;
    let chi = 1.0
        / (((c * building_area_m2) + core::f64::consts::PI * sigma_y_m * sigma_z_m)
            * wind_speed_m_per_s);
    if chi < (1.0 / 3.0) * chi_over_q_unwaked {
        (1.0 / 3.0) * chi_over_q_unwaked
    } else {
        chi
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_stable_rise_ignores_class_e() {
        let both = plume_rise_stable_both_formulas(10.0, 2.0, 5.0, StableClass::F);
        assert_eq!(
            plume_rise_stable_upstream(10.0, 2.0, 5.0),
            both.windy_formula
        );
        let e = plume_rise_stable_both_formulas(10.0, 2.0, 5.0, StableClass::E);
        assert!(e.windy_formula > both.windy_formula);
    }

    #[test]
    fn wake_never_drops_below_a_third() {
        let v = building_wake_gifford(1e-3, 1.0e4, 5.0, 1.0, 1.0);
        assert!(v >= 1e-3 / 3.0);
    }
}
