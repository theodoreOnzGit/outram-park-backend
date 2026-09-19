// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of PETIR, a component of OUTRAM PARK.
//
// PETIR is free software: you can redistribute it and/or modify it under the
// terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 of the License.
//
// PETIR is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along
// with PETIR.  If not, see <https://www.gnu.org/licenses/>.

//! Special functions — GSL's `specfunc/`, plus the OpenFOAM-derived kernels.
//!
//! # What is here
//!
//! - **Error function family**: [`erf`], [`erfc`] and the scaled complementary
//!   form [`erfc_scaled`], translated from GSL's `specfunc/erfc.c`; and
//!   [`erf_inv`], the Winitzki approximation lifted verbatim from
//!   `outram-foam-basic-lib`.
//! - **Bessel family**: the cylindrical functions of orders 0 and 1 —
//!   [`bessel_j0`], [`bessel_j1`], [`bessel_y0`], [`bessel_y1`],
//!   [`bessel_i0`], [`bessel_i1`], [`bessel_k0`], [`bessel_k1`] and the
//!   exponentially scaled modified forms — translated from GSL's
//!   `specfunc/bessel_{J,Y,I,K}{0,1}.c`.
//! - **Digamma family**: [`psi`], [`psi_1`] and the general polygamma
//!   [`psi_n`], with the integer-argument forms, from GSL's `specfunc/psi.c`.
//! - **Zeta family**: [`zeta`], the Hurwitz [`hzeta`], the cancellation-free
//!   [`zetam1`] and the Dirichlet [`eta`], from GSL's `specfunc/zeta.c`.
//! - **Gamma family**: [`ln_gamma`] and [`gamma`] (GSL's Lanczos form), the
//!   regularised incomplete gamma functions
//!   [`inc_gamma_ratio_p`] / [`inc_gamma_ratio_q`] and their unnormalised
//!   counterparts, and the inverse [`inv_inc_gamma`] — the last three lifted
//!   verbatim from `outram-foam-basic-lib`.
//!
//! # Accuracy, and how to read the two lineages
//!
//! The two sources are held to different standards and this matters when
//! choosing a routine:
//!
//! - GSL's `erf`/`erfc`/`ln_gamma` target close to machine precision and are
//!   documented with their error bounds.
//! - [`erf_inv`] is an *approximation* with maximum relative error of order
//!   `1e-4`. That is what OpenFOAM uses and it is fine for the sampling work it
//!   was written for, but it is emphatically not a machine-precision inverse.
//!   Do not reach for it where accuracy matters without refining the result —
//!   a couple of Newton steps against [`erf`] will do it.
//!
//! Each function's own doc comment states its accuracy and its upstream.

// Under a std-linked build (`cargo test`) f64's inherent exp/ln shadow these
// trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

/// `GSL_DBL_EPSILON`.
pub(crate) const DBL_EPSILON: f64 = 2.220_446_049_250_313_1e-16;
/// `GSL_SQRT_DBL_EPSILON`.
pub(crate) const SQRT_DBL_EPSILON: f64 = 1.490_116_119_384_765_6e-8;
/// `GSL_ROOT5_DBL_EPSILON` — the fifth root of [`DBL_EPSILON`].
pub(crate) const ROOT5_DBL_EPSILON: f64 = 7.400_959_797_414_050_5e-4;
/// `GSL_LOG_DBL_MAX`.
pub(crate) const LOG_DBL_MAX: f64 = 7.097_827_128_933_839_7e2;
/// `GSL_LOG_DBL_MIN`.
pub(crate) const LOG_DBL_MIN: f64 = -7.083_964_185_322_640_8e2;
/// `GSL_SQRT_DBL_MAX`.
pub(crate) const SQRT_DBL_MAX: f64 = 1.340_780_792_994_259_6e154;
/// `GSL_SQRT_DBL_MIN`.
pub(crate) const SQRT_DBL_MIN: f64 = 1.491_668_146_240_041_3e-154;
/// `GSL_DBL_MIN`.
pub(crate) const DBL_MIN: f64 = f64::MIN_POSITIVE;
/// `M_EULER`, the Euler-Mascheroni constant.
pub(crate) const EULER: f64 = 0.577_215_664_901_532_9;

/// `y * exp(x)`, computed as GSL's `gsl_sf_exp_mult_err_e` (`specfunc/exp.c`)
/// computes its value: directly where both factors are comfortably in range,
/// and otherwise by splitting each exponent into its integer and fractional
/// parts so that neither `exp` call can overflow on its own.
///
/// Only the value is translated; GSL's error propagation is not carried here
/// because PETIR's special functions return a bare `f64` (see the module
/// documentation of [`crate::specfunc`]).
#[inline]
pub(crate) fn exp_mult(x: f64, y: f64) -> f64 {
    let ay = y.abs();
    if y == 0.0 {
        return 0.0;
    }
    if x < 0.5 * LOG_DBL_MAX
        && x > 0.5 * LOG_DBL_MIN
        && ay < 0.8 * SQRT_DBL_MAX
        && ay > 1.2 * SQRT_DBL_MIN
    {
        return y * x.exp();
    }
    let ly = ay.ln();
    let lnr = x + ly;
    if lnr > LOG_DBL_MAX - 0.01 {
        return if y < 0.0 {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
    }
    if lnr < LOG_DBL_MIN + 0.01 {
        return 0.0;
    }
    let sy = if y < 0.0 { -1.0 } else { 1.0 };
    let m = x.floor();
    let n = ly.floor();
    let a = x - m;
    let b = ly - n;
    sy * (m + n).exp() * (a + b).exp()
}

#[cfg(test)]
mod exp_mult_tests {
    use super::exp_mult;
    // Under a std-linked test build f64's inherent exp shadows the trait
    // method, leaving the import formally unused. See crate::real.
    #[allow(unused_imports)]
    use crate::real::Real;

    /// `exp_mult`'s split branch must agree with the direct product wherever
    /// the direct product is representable, and must stay finite where it is
    /// not. The split is only reached for arguments `K_0`/`K_1` see past
    /// `x ~ 354`, so it is otherwise untested by the suite.
    #[test]
    fn exp_mult_agrees_with_the_direct_product_and_extends_past_it() {
        for (x, y) in [(-10.0_f64, 3.0_f64), (-100.0, 0.05), (5.0, -2.0)] {
            let direct = y * x.exp();
            assert!(((exp_mult(x, y) - direct) / direct).abs() < 1e-15);
        }
        // Past the fast branch: -400 is below 0.5 * LOG_DBL_MIN.
        let v = exp_mult(-400.0, 0.05);
        assert!(v > 0.0 && v.is_finite(), "exp_mult(-400, 0.05) = {v:e}");
        assert!(((v - 0.05 * (-400.0_f64).exp()) / v).abs() < 1e-14);
        assert_eq!(exp_mult(-10.0, 0.0), 0.0);
    }
}

pub mod airy;
pub mod atanint;
pub mod bessel;
pub mod clausen;
pub mod dawson;
pub mod debye;
pub mod dilog;
pub mod ellint;
pub mod elljac;
pub mod erf;
/// Inverse error function (Winitzki approximation), lifted verbatim from
/// outram-foam-basic-lib. Maximum relative error of order 1e-4 -- see the
/// module documentation above before relying on it.
pub mod erf_inv;
pub mod expint3;
pub mod fermi_dirac;
pub mod gamma;
pub mod gegenbauer;
/// Regularised and unnormalised incomplete gamma functions, lifted verbatim
/// from outram-foam-basic-lib (DiDonato & Morris, ACM TOMS 1986).
pub mod inc_gamma;
/// Inverse of the regularised lower incomplete gamma function, lifted verbatim
/// from outram-foam-basic-lib.
pub mod inv_inc_gamma;
pub mod lambert;
pub mod psi;
pub mod shint;
pub mod sinint;
pub mod synchrotron;
pub mod transport;
pub mod trig;
pub mod zeta;

pub use bessel::{
    bessel_i0, bessel_i0_scaled, bessel_i1, bessel_i1_scaled, bessel_j0, bessel_j1, bessel_k0,
    bessel_k0_scaled, bessel_k1, bessel_k1_scaled, bessel_y0, bessel_y1,
};
pub use debye::{debye_1, debye_2, debye_3, debye_4, debye_5, debye_6, debye_n};
pub use erf::{erf, erfc, erfc_scaled, erfcx};
pub use erf_inv::erf_inv;
pub use gamma::{beta, choose, factorial, gamma, ln_beta, ln_factorial, ln_gamma, ln_gamma_sgn};
pub use inc_gamma::{inc_gamma_p, inc_gamma_q, inc_gamma_ratio_p, inc_gamma_ratio_q};
pub use inv_inc_gamma::inv_inc_gamma;
pub use psi::{psi, psi_1, psi_1_int, psi_1piy, psi_int, psi_n};
pub use zeta::{eta, eta_int, hzeta, zeta, zeta_int, zetam1, zetam1_int};
