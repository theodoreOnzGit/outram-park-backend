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
//
// Translated from the GNU Scientific Library (GSL)
// <https://www.gnu.org/software/gsl/>
// Copyright (C) 1996-2024 The GSL Team (Brian Gough, Gerard Jungman, et al.)
// GSL is free software under the GNU General Public License, version 3 or
// later. Upstream file: specfunc/gamma.c
//   - `lngamma_lanczos`      -> `ln_gamma` (positive branch)
//   - `gsl_sf_lngamma_e`     -> `ln_gamma` (reflection branch)
//   - `gsl_sf_lngamma_sgn_e` -> `ln_gamma_sgn`
//   - `gsl_sf_lnbeta_e`      -> `ln_beta`
//   - `gsl_sf_choose_e`      -> `choose`
//   - `lngamma_sgn_0`, `lngamma_sgn_sing`, `gamma_xgthalf`, `gammastar_ser`,
//     `gsl_sf_gamma_e`, `gsl_sf_gammastar_e`, `gsl_sf_fact_e`,
//     `gsl_sf_lnfact_e`, `gsl_sf_lnchoose_e` (added 2026-11-12, gh:#1419,
//     gh:#1420)
// Upstream file: specfunc/beta.c (Copyright (C) 1996-2000 Gerard Jungman)
//   - `gsl_sf_lnbeta_sgn_e`, `gsl_sf_lnbeta_e`, `gsl_sf_beta_e`
//     -> `ln_beta_sgn`, `ln_beta`, `beta`
// Upstream file: specfunc/log.c
//   - `gsl_sf_log_1plusx_e`  -> `log_1plusx` (private)
//   - `lngamma_sgn_0`, `lngamma_sgn_sing`, `gamma_xgthalf`, `gammastar_ser`,
//     `gsl_sf_gamma_e`, `gsl_sf_gammastar_e`, `gsl_sf_fact_e`,
//     `gsl_sf_lnfact_e`, `gsl_sf_lnchoose_e` (added 2026-11-12, gh:#1419,
//     gh:#1420)
// Upstream file: specfunc/beta.c (Copyright (C) 1996-2000 Gerard Jungman)
//   - `gsl_sf_lnbeta_sgn_e`, `gsl_sf_lnbeta_e`, `gsl_sf_beta_e` -> `ln_beta_sgn`,
//     `ln_beta`, `beta`
// Upstream file: specfunc/log.c
//   - `gsl_sf_log_1plusx_e`  -> `log_1plusx` (private)

//! The gamma function family: [`ln_gamma`], [`gamma`], [`ln_beta`], [`beta`],
//! [`factorial`], [`ln_factorial`], [`choose`].
//! # A port, checked against the vendored source and against an independent
//! implementation
//!
//! Unlike [`crate::specfunc::erf`], this module does *not* forward to `libm`.
//! [`ln_gamma`] is ported from GSL's `specfunc/gamma.c` — the Lanczos
//! evaluation, its nine-coefficient table, the two (2,2) Pade branches at the
//! zeros, and upstream's dispatch order between them. It is built out rather
//! than delegated because `ln_gamma` is what [`ln_beta`] and [`choose`] stand
//! on, both of which need the *logarithm* to avoid overflow, and neither of
//! which `libm` provides at all.
//!
//! Provenance is checked twice over, because a transcribed coefficient table is
//! exactly the thing that fails silently:
//!
//! 1. **Against the source.** Every constant and the shape of each routine were
//!    read from the vendored GSL 2.8 tree at commit `cf180cd7` — the same clone
//!    whose licence is recorded in `NOTICE`, re-verified on 2026-09-14 by
//!    matching its `COPYING` sha256 and commit SHA against that record. Each
//!    function's doc comment names the upstream file and line.
//! 2. **Against an independent implementation.** The tests compare [`ln_gamma`]
//!    with `libm`'s fdlibm `lgamma` across five decades of argument. That is a
//!    different algorithm (Cody-Hillstrom rational minimax) from a different
//!    lineage, so agreement at 1e-15 is real evidence and not a tautology. The
//!    exact integer factorials and Pascal's rule provide a third, algorithm-free
//!    check.
//!
//! A mistyped digit in the Lanczos table would break check 2 at the 1e-8 level
//! or worse; it is not a claim resting on careful typing.
//!
//! # Algorithm
//!
//! Lanczos' approximation with `g = 7` and nine coefficients, which is the
//! parameter choice GSL ships:
//!
//! `ln Gamma(z) = (z - 0.5) ln((z + 6.5)/e) + ln(sqrt(2 pi)) + ln A_g(z) - 7`
//!
//! where `A_g(z) = c_0 + sum_{k=1}^{8} c_k / (z - 1 + k)`, valid for
//! `Re(z) > 0.5`. Below that the Euler reflection formula
//! `Gamma(z) Gamma(1 - z) = pi / sin(pi z)` moves the argument into the valid
//! half-plane.
//!
//! # Accuracy
//!
//! Measured against `libm::lgamma` over `x` in `[1e-4, 1e5]` (this module's
//! tests, x86_64, 2026-09-14):
//!
//! - **Relative error under 1.1e-15** wherever `|ln Gamma(x)| > 1`.
//! - **Absolute error under 2.1e-15** elsewhere, where no relative bound is
//!   meaningful because the function passes through zero.
//! - **Exactly zero at `x = 1` and `x = 2`**, and under 2e-17 absolute within
//!   0.01 of either — see below.
//!
//! That last line is the Pade branches doing their work. `ln Gamma` vanishes at
//! 1 and 2, and the Lanczos form reaches those zeros only by cancelling terms
//! of magnitude about 7, giving up roughly 1.5 decimal digits exactly where the
//! answer is smallest. GSL switches to (2,2) Pade approximants for
//! `ln Gamma(1 + eps)` and `ln Gamma(2 + eps)` over `|eps| < 0.01`; PETIR ports
//! both, so the zeros come out exact by construction rather than by
//! cancellation. The window is half-open at 0.01 exactly as upstream has it, so
//! Lanczos accuracy returns just outside it.
//!
//! ~~Two limits remain, stated rather than hidden. Accuracy degrades near the
//! poles at the non-positive integers, where `ln Gamma` genuinely diverges; and
//! the reflection branch has not ported GSL's fractional-part extraction, so it
//! loses accuracy for `x` below about -50. Both are filed as beads.~~
//! **CORRECTED 2026-11-12 (gh:#1419)** — the second sentence was wrong: the
//! reflection degraded within 0.02 of EVERY negative integer (measured against
//! compiled GSL 2.8: relative error `1e-16/|eps|`, 2.5e-8 at `eps = 1e-9`,
//! 0.04-0.15 absolute in `ln|Gamma|` one ulp from a pole), not only below -50.
//! It is fixed: `lngamma_sgn_sing` and `lngamma_sgn_0` are ported, and
//! `ln_gamma`, `ln_gamma_sgn` and `gamma` follow GSL's own dispatch
//! (`gsl_sf_lngamma_e`, `gsl_sf_lngamma_sgn_e`, `gsl_sf_gamma_e`). Measured
//! after the fix, against compiled GSL 2.8 on 346 near-pole values: see
//! `tests/gsl_specfunc_code_to_code.rs` (`gamma_near_negative_integer_poles_*`).
//! What is still refused: `|x| > 0.5/(eps pi) ~ 7.2e14` has no fractional part
//! left (GSL `GSL_EROUND`; PETIR returns `NaN`).
//!
//! # Units
//!
//! All arguments and results are dimensionless.

// Under a std-linked build (`cargo test`) f64's inherent sqrt/exp/... shadow
// these trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;
use crate::specfunc::gamma_data::{
    FACT_TABLE, GAMMA_5_10_DATA, GSTAR_A_DATA, GSTAR_B_DATA, LOPX_DATA,
};
use crate::specfunc::psi::{psi_1_int, psi_int, psi_n};
use crate::specfunc::{exp_mult, DBL_EPSILON, DBL_MIN, LOG_DBL_MAX, LOG_DBL_MIN};
use core::f64::consts::PI;

/// Lanczos coefficients for `g = 7`, `n = 9` — GSL's `lanczos_7_c`.
///
/// Do not "tidy" these digits. They are a matched set: the approximation's
/// error cancels only for this exact combination of `g` and coefficients, and
/// rounding any one of them degrades the whole evaluation rather than just that
/// term.
const LANCZOS_7_C: [f64; 9] = [
    0.999_999_999_999_809_9,
    676.520_368_121_885_1,
    -1_259.139_216_722_402_8,
    771.323_428_777_653_1,
    -176.615_029_162_140_6,
    12.507_343_278_686_905,
    -0.138_571_095_265_720_12,
    9.984_369_578_019_572e-6,
    1.505_632_735_149_311_6e-7,
];

/// `ln(sqrt(2 pi))` — GSL's `LogRootTwoPi_` (specfunc/gamma.c:33), with
/// upstream's own digits.
///
/// ~~`0.918_938_533_204_672_7`~~ **CORRECTED 2026-11-12 (gh:#1421):** that
/// 16-digit literal is the double one ulp BELOW upstream's
/// (`0.9189385332046727` against `0.9189385332046728`; checked by parsing both
/// literals), so every Lanczos `ln_gamma(x)`, `x >= 0.5`, carried an extra
/// ulp-level error in its last term. Found by comparing against GSL relinked
/// with PETIR's `libm`, where `ln_gamma(3.75)` still differed by 4 ulp.
const LOG_ROOT_TWO_PI: f64 = 0.9189385332046727418;

/// `ln(pi)`, used by the reflection branch.
const LN_PI: f64 = 1.144_729_885_849_400_2;

/// The natural logarithm of the absolute value of the gamma function.
///
/// # Why the logarithm
///
/// `Gamma(171)` already overflows `f64`, but `ln Gamma(171)` is about 707 and
/// `ln Gamma(1e6)` is about 1.2e7 — perfectly ordinary numbers. Any quantity
/// built from ratios of gamma functions (binomial coefficients, beta functions,
/// chi-squared densities) should be assembled in log space and exponentiated
/// once at the end, if at all.
///
/// # Domain and range
///
/// Defined for every `x` except the poles at `0, -1, -2, ...`, where it returns
/// `+inf`. For negative non-integer `x` the result is `ln|Gamma(x)|`; use
/// [`ln_gamma_sgn`] when the sign is needed too.
///
/// # Accuracy
///
/// Relative error under 2e-15 over `x` in `[1e-4, 1e5]`.
///
/// # Example
///
/// ```
/// use petir::specfunc::ln_gamma;
/// // Gamma(n) = (n-1)! , so ln Gamma(6) = ln(120).
/// assert!((ln_gamma(6.0) - 120.0_f64.ln()).abs() < 1e-13);
/// ```
pub fn ln_gamma(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    // Poles at the non-positive integers. GSL refuses these (`GSL_EDOM`, value
    // 0 or NaN); PETIR's documented answer is `+inf`.
    if x <= 0.0 && x == x.trunc() {
        return f64::INFINITY;
    }
    lngamma_e(x)
}

/// The Lanczos branch, valid for `x >= 0.5`. GSL's `lngamma_lanczos`.
fn ln_gamma_lanczos(x: f64) -> f64 {
    // Lanczos is written for z! rather than Gamma(z), hence the shift.
    let z = x - 1.0;

    // The zeroth coefficient, then the eight terms `LANCZOS_7_C[k]/(z+k)`.
    let Some((&c0, tail)) = LANCZOS_7_C.split_first() else {
        return f64::NAN;
    };
    let mut ag = c0;
    for (k, c) in (1..).zip(tail.iter()) {
        ag += c / (z + k as f64);
    }

    let term1 = (z + 0.5) * Real::ln((z + 7.5) / core::f64::consts::E);
    let term2 = LOG_ROOT_TWO_PI + Real::ln(ag);
    term1 + (term2 - 7.0)
}

/// `ln Gamma(1 + eps)` for `|eps| < 0.01` — GSL's `lngamma_1_pade`.
///
/// A (2,2) Padé approximant to `ln Gamma(1 + eps) / eps`, plus a five-term
/// correction series in `eps^5`. Multiplying back by `eps` at the end is what
/// makes the zero at `eps = 0` exact by construction rather than by
/// cancellation — which is the entire point of having this branch.
///
/// Ported from GSL 2.8 `specfunc/gamma.c:897` at commit `cf180cd7`.
/// Copyright (C) 1996-2000 Gerard Jungman, GPL-3.0-or-later. See NOTICE.
fn ln_gamma_1_pade(eps: f64) -> f64 {
    const N1: f64 = -1.0017419282349508699871138440;
    const N2: f64 = 1.7364839209922879823280541733;
    const D1: f64 = 1.2433006018858751556055436011;
    const D2: f64 = 5.0456274100274010152489597514;
    let num = (eps + N1) * (eps + N2);
    let den = (eps + D1) * (eps + D2);
    let pade = 2.0816265188662692474880210318 * num / den;
    const C0: f64 = 0.004785324257581753;
    const C1: f64 = -0.01192457083645441;
    const C2: f64 = 0.01931961413960498;
    const C3: f64 = -0.02594027398725020;
    const C4: f64 = 0.03141928755021455;
    let eps5 = eps * eps * eps * eps * eps;
    let corr = eps5 * (C0 + eps * (C1 + eps * (C2 + eps * (C3 + C4 * eps))));
    eps * (pade + corr)
}

/// `ln Gamma(2 + eps)` for `|eps| < 0.01` — GSL's `lngamma_2_pade`.
///
/// The companion of [`ln_gamma_1_pade`] at the second zero of `ln Gamma`, same
/// construction and same reason.
///
/// Ported from GSL 2.8 `specfunc/gamma.c:924` at commit `cf180cd7`.
/// Copyright (C) 1996-2000 Gerard Jungman, GPL-3.0-or-later. See NOTICE.
fn ln_gamma_2_pade(eps: f64) -> f64 {
    const N1: f64 = 1.000895834786669227164446568;
    const N2: f64 = 4.209376735287755081642901277;
    const D1: f64 = 2.618851904903217274682578255;
    const D2: f64 = 10.85766559900983515322922936;
    let num = (eps + N1) * (eps + N2);
    let den = (eps + D1) * (eps + D2);
    let pade = 2.85337998765781918463568869 * num / den;
    const C0: f64 = 0.0001139406357036744;
    const C1: f64 = -0.0001365435269792533;
    const C2: f64 = 0.0001067287169183665;
    const C3: f64 = -0.0000693271800931282;
    const C4: f64 = 0.0000407220927867950;
    let eps5 = eps * eps * eps * eps * eps;
    let corr = eps5 * (C0 + eps * (C1 + eps * (C2 + eps * (C3 + C4 * eps))));
    eps * (pade + corr)
}

/// `ln|Gamma(x)|` together with the sign of `Gamma(x)`.
///
/// Translates GSL's `gsl_sf_lngamma_sgn_e`. The sign is `+1.0` or `-1.0`; it is
/// `NaN` at the poles, where `Gamma` has no defined sign.
///
/// # Why a separate function
///
/// `Gamma` alternates sign on the negative axis — negative on `(-1, 0)`,
/// positive on `(-2, -1)`, and so on — information the logarithm of the
/// absolute value necessarily loses. Reconstructing `Gamma(x)` from
/// [`ln_gamma`] alone would silently drop that sign for half the negative axis.
///
/// # Example
///
/// ```
/// use petir::specfunc::gamma::ln_gamma_sgn;
/// let (ln_abs, sgn) = ln_gamma_sgn(-0.5);
/// // Gamma(-0.5) = -2 sqrt(pi)
/// assert_eq!(sgn, -1.0);
/// assert!((sgn * ln_abs.exp() + 2.0 * core::f64::consts::PI.sqrt()).abs() < 1e-13);
/// ```
pub fn ln_gamma_sgn(x: f64) -> (f64, f64) {
    if x.is_nan() {
        return (f64::NAN, f64::NAN);
    }
    if x <= 0.0 && x == x.trunc() {
        return (f64::INFINITY, f64::NAN);
    }
    lngamma_sgn_e(x)
}

/// The gamma function, `Gamma(x)`.
///
/// # Domain and range
///
/// Defined for every `x` except the poles at `0, -1, -2, ...`, where it returns
/// `NaN` (the two one-sided limits differ in sign, so no single value is
/// correct). Overflows to `+inf` for `x > 171` (GSL's `GSL_SF_GAMMA_XMAX`;
/// ~~"beyond about 171.6"~~ **CORRECTED 2026-11-12**: `Gamma(171.5)` is
/// representable but GSL, and now PETIR, return `+inf` there).
///
/// # Algorithm
///
/// GSL's `gsl_sf_gamma_e` (specfunc/gamma.c:1255), ported 2026-11-12
/// (gh:#1419, gh:#1420). ~~Reconstructed as `sgn * exp(ln|Gamma(x)|)`; exact
/// factorials for integers up to 20.~~ Now: the exact `fact_table` entry for an
/// integer `1 <= x <= 171`; for `x >= 1/2` a series near 1 and 2, `exp` of the
/// Lanczos logarithm below 5, a Chebyshev series for `5 <= x < 10`, and
/// `x^(x/2) e^(-x) x^(x/2) sqrt(2 pi / x)` times the Stirling `gammastar`
/// series above; for `x < 1/2` the reflection `1 / (sin_term Gamma(1 - x))`
/// with `sin_term = (-1)^r sin(pi f) / pi` and `x = r + f` exact, so a pole
/// costs no precision. Only `x <= -169` goes through the logarithm.
///
/// # Accuracy
///
/// Against compiled GSL 2.8 (`tests/gsl_specfunc_code_to_code.rs`): 581 values
/// on the sweep grid, worst relative difference 1.1e-15 against stock GSL
/// (glibc `exp`/`pow`/`sin`) and 0 (all bit-identical) against GSL linked to
/// PETIR's own libm.
///
/// # Example
///
/// ```
/// use petir::specfunc::gamma;
/// assert!((gamma(5.0) - 24.0).abs() < 1e-12);       // 4!
/// assert!((gamma(0.5) - core::f64::consts::PI.sqrt()).abs() < 1e-14);
/// ```
pub fn gamma(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x <= 0.0 && x == x.trunc() {
        return f64::NAN;
    }
    gamma_e(x)
}

/// `n!` as an `f64`, from GSL's `fact_table` for every `n <= 170`.
///
/// # Domain and range
///
/// `22!` is the largest factorial that is exact in `f64` (checked with exact
/// integer arithmetic; ~~"20! is the largest"~~ **CORRECTED 2026-11-12**: 21!
/// and 22! are exact too); from `23!` the table holds GSL's correctly rounded
/// value. `170!` is the largest that fits at all. Beyond `n = 170` the result is `+inf` — use [`ln_factorial`]
/// there instead of trying to represent the value.
///
/// Translates GSL's `gsl_sf_fact_e` (specfunc/gamma.c:1474) with its
/// `fact_table`, extracted from `gamma.c` by script (`gamma_data.rs`).
/// ~~Exact only for `n <= 20`, `exp(ln_gamma)` above~~ **CORRECTED
/// 2026-11-12 (gh:#1420)**: 171/171 values bit-identical to GSL 2.8.
///
/// # Example
///
/// ```
/// use petir::specfunc::gamma::factorial;
/// assert_eq!(factorial(0), 1.0);
/// assert_eq!(factorial(10), 3_628_800.0);
/// ```
pub fn factorial(n: u32) -> f64 {
    // `gsl_sf_fact_e` (specfunc/gamma.c:1474): the table up to
    // `GSL_SF_FACT_NMAX = 170`, overflow beyond.
    match FACT_TABLE.get(n as usize) {
        Some(&f) => f,
        None => f64::INFINITY,
    }
}

/// `ln(n!)`, valid for every `n` without overflow.
///
/// `ln` of the exact `fact_table` entry for `n <= 170`, `ln Gamma(n + 1)` above.
/// Translates GSL's `gsl_sf_lnfact_e` (specfunc/gamma.c:1514). ~~Computed as
/// `ln Gamma(n + 1)` for every `n`~~ **CORRECTED 2026-11-12 (gh:#1420)**.
///
/// # Example
///
/// ```
/// use petir::specfunc::gamma::ln_factorial;
/// assert!((ln_factorial(10) - 3_628_800.0_f64.ln()).abs() < 1e-12);
/// // 1000! overflows f64, but its logarithm is an ordinary number.
/// assert!((ln_factorial(1000) - 5912.128_178_488_163).abs() < 1e-9);
/// ```
pub fn ln_factorial(n: u32) -> f64 {
    // `gsl_sf_lnfact_e` (specfunc/gamma.c:1514): the log of the exact table
    // entry up to 170!, `lngamma(n + 1)` beyond.
    match FACT_TABLE.get(n as usize) {
        Some(&f) => Real::ln(f),
        None => lngamma_e(n as f64 + 1.0),
    }
}

/// The natural logarithm of the beta function, `ln B(a, b)`.
///
/// Translates GSL's `gsl_sf_lnbeta_e` (specfunc/beta.c:35) via
/// [`ln_beta_sgn`]: when one argument dominates (`min/max < 0.2`) it is built
/// from `gammastar` and `log(1 + min/max)`, so there is no cancellation;
/// otherwise `ln Gamma(a) + ln Gamma(b) - ln Gamma(a + b)` with the signs.
///
/// # Domain
///
/// Any `a`, `b` that are not zero or a negative integer. `NaN` if `B(a, b)` is
/// negative (it has no real logarithm; use [`ln_beta_sgn`]) or if `a` or `b`
/// is zero or a negative integer. ~~`a > 0` and `b > 0` only~~ **CORRECTED
/// 2026-11-12 (gh:#1420)**: negative non-integers are GSL's, e.g.
/// `ln_beta(-0.5, 0.1) = 2.2037`.
///
/// # Accuracy
///
/// `ln_beta(1.0, 1e10)` is `-ln(1e10)` to the last bit of GSL's value
/// (-23.025850929940457); ~~it was -23.02581787109375, 4e-5 off~~.
///
/// # Example
///
/// ```
/// use petir::specfunc::ln_beta;
/// // B(2, 3) = 1/12
/// assert!((ln_beta(2.0, 3.0) - (1.0_f64 / 12.0).ln()).abs() < 1e-13);
/// ```
pub fn ln_beta(a: f64, b: f64) -> f64 {
    // `gsl_sf_lnbeta_e` (specfunc/beta.c:35): a negative beta function has no
    // real logarithm, which GSL reports as `GSL_EDOM`.
    let (v, sgn) = ln_beta_sgn(a, b);
    if sgn == -1.0 {
        return f64::NAN;
    }
    v
}

/// The beta function, `B(a, b) = Gamma(a) Gamma(b) / Gamma(a + b)`.
///
/// # Domain
///
/// Translates GSL's `gsl_sf_beta_e` (specfunc/beta.c:112): the product of
/// gammas for `0 < a, b < 50`, `exp(ln_beta_sgn)` times the sign otherwise.
/// Negative non-integer arguments are allowed (`beta(-2.5, 1.0) = -0.4`);
/// `NaN` if `a` or `b` is zero or a negative integer, `0` if `a + b` is a
/// negative integer.
///
/// Underflows to zero for large arguments — `B(500, 500)` is about `1e-302`.
/// Where that matters, stay in [`ln_beta`].
///
/// # Example
///
/// ```
/// use petir::specfunc::beta;
/// assert!((beta(2.0, 3.0) - 1.0 / 12.0).abs() < 1e-15);
/// ```
pub fn beta(a: f64, b: f64) -> f64 {
    beta_e(a, b)
}

/// The binomial coefficient `n choose k`.
///
/// Translates GSL's `gsl_sf_choose_e` (specfunc/gamma.c:1584): for `n <= 170`
/// `(n! / k!) / (n-k)!` from the exact table; above that a product of
/// `n/(n-k) * (n-1)/(n-k-1) ...` when `n - k < 64`, else
/// `exp(ln_choose(n, k))`. ~~Computed through `ln_factorial` and rounded to the
/// nearest integer below 2^53~~ **CORRECTED 2026-11-12 (gh:#1420)**: GSL does
/// not round, so for `n >= 20` or so the result is within a few ulp of the
/// integer (GSL's own error estimate is `6 eps`), not always equal to it.
/// `choose(100000, 5)` was off by 3.2e-10 relative and is now bit-identical to
/// GSL.
///
/// # Domain
///
/// `k > n` gives `0.0` (GSL: `GSL_EDOM`). Overflows to `+inf` when the value
/// exceeds `f64::MAX`.
///
/// # Example
///
/// ```
/// use petir::specfunc::gamma::choose;
/// assert_eq!(choose(5, 2), 10.0);
/// assert!((choose(100, 50) - 1.008_913_445_455_642_2e29).abs() / 1e29 < 1e-12);
/// ```
pub fn choose(n: u32, k: u32) -> f64 {
    if k > n {
        // GSL refuses (`GSL_EDOM`); PETIR defines the empty selection as 0.
        return 0.0;
    }
    choose_e(n, k)
}

// ---------------------------------------------------------------------------
// Ports added 2026-11-12 (gh:#1419, gh:#1420). All value-only: GSL's `.err`
// channel is not carried (`specfunc/mod.rs`), and a GSL error status (domain,
// round-off, overflow in a refused branch) is returned as `NaN`, PETIR's way of
// refusing. Line numbers in doc comments such as "gamma.c:763" are lines in
// GSL 2.8 at commit `cf180cd7`.
// ---------------------------------------------------------------------------

/// `M_SQRTPI`.
const SQRT_PI: f64 = 1.772_453_850_905_516_027_298_167_483_34;

/// `GSL_ROOT4_DBL_EPSILON`.
const ROOT4_DBL_EPSILON: f64 = 1.220_703_125_000_000_0e-4;

/// `GSL_ROOT6_DBL_EPSILON`.
const ROOT6_DBL_EPSILON: f64 = 2.460_783_300_575_925_1e-3;

/// `GSL_SF_GAMMA_XMAX`.
const GAMMA_XMAX: f64 = 171.0;

/// GSL's `cheb_eval_e` value on `[-1, 1]`: `x` is mapped exactly as upstream
/// maps it, `(2.0*x - a - b) / (b - a)`, whose rounding is not the identity.
fn cheb_value(coefficients: &[f64], x: f64) -> f64 {
    crate::cheb_slice::eval_gsl(crate::cheb_slice::scale(x, -1.0, 1.0), coefficients)
}

/// `gsl_sf_exp_err_e`'s value (specfunc/exp.c:536) without its error term:
/// `+inf` above `GSL_LOG_DBL_MAX`, `0` below `GSL_LOG_DBL_MIN`.
///
/// **DEVIATION (error channel dropped):** upstream also refuses when `x` is
/// within its own error estimate of either limit; the estimate is not carried
/// here, so the cut is made at the limit itself.
fn exp_checked(x: f64) -> f64 {
    if x > LOG_DBL_MAX {
        f64::INFINITY
    } else if x < LOG_DBL_MIN {
        0.0
    } else {
        Real::exp(x)
    }
}

/// `GSL_IS_ODD`.
fn is_odd(n: i32) -> bool {
    n & 1 == 1
}

/// `x` near zero: `ln|Gamma(x)|` and the sign of `Gamma(x)`, GSL's
/// `lngamma_sgn_0`. Double precision for `|eps| < 0.02`.
///
/// Ported from GSL 2.8 `specfunc/gamma.c:729` (commit `cf180cd7`), copyright
/// (C) 1996-2000 Gerard Jungman, GPL-3.0-or-later.
///
/// **DEVIATION (subnormal guard):** `gee / |eps|` overflows for `|eps|` below
/// about `1e-308`, where upstream returns `+inf` for a finite answer
/// (`ln_gamma(1e-310)` is 713.8). The quotient is then taken in log space.
/// Only arguments whose quotient overflows reach the guard, so every other
/// value is upstream's, bit for bit.
fn lngamma_sgn_0(eps: f64) -> (f64, f64) {
    // series for g(eps) = Gamma(eps) eps - 1/(1+eps) - eps/2
    const C1: f64 = -0.07721566490153286061;
    const C2: f64 = -0.01094400467202744461;
    const C3: f64 = 0.09252092391911371098;
    const C4: f64 = -0.01827191316559981266;
    const C5: f64 = 0.01800493109685479790;
    const C6: f64 = -0.00685088537872380685;
    const C7: f64 = 0.00399823955756846603;
    const C8: f64 = -0.00189430621687107802;
    const C9: f64 = 0.00097473237804513221;
    const C10: f64 = -0.00048434392722255893;
    let g6 = C6 + eps * (C7 + eps * (C8 + eps * (C9 + eps * C10)));
    let g = eps * (C1 + eps * (C2 + eps * (C3 + eps * (C4 + eps * (C5 + eps * g6)))));

    // Gamma(eps) eps, a positive quantity
    let gee = g + 1.0 / (1.0 + eps) + 0.5 * eps;

    let mut lng = Real::ln(gee / eps.abs());
    if lng.is_infinite() && gee.is_finite() {
        lng = Real::ln(gee) - Real::ln(eps.abs());
    }
    // GSL_SIGN: `(x >= 0.0) ? 1 : -1`
    let sgn = if eps >= 0.0 { 1.0 } else { -1.0 };
    (lng, sgn)
}

/// `x = -N + eps` near a negative integer (`N >= 1`): `ln|Gamma(x)|` and the
/// sign of `Gamma(x)`, GSL's `lngamma_sgn_sing`. Double precision for
/// `|eps| < 0.02`. `None` is upstream's `GSL_EDOM` at `eps == 0`.
///
/// This is the routine whose absence made `ln_gamma` lose `1e-16 / |eps|`
/// near every pole (gh:#1419): the reflection `ln(pi) - ln|sin(pi x)| -
/// ln Gamma(1 - x)` takes `sin` of a rounded `pi * x`.
///
/// Ported from GSL 2.8 `specfunc/gamma.c:763` (commit `cf180cd7`), copyright
/// (C) 1996-2000 Gerard Jungman, GPL-3.0-or-later. It calls `lnfact`,
/// `psi_int`, `psi_1_int` and `psi_n`, which are PETIR's ports of the same
/// GSL routines.
fn lngamma_sgn_sing(n: i32, eps: f64) -> Option<(f64, f64)> {
    if eps == 0.0 {
        return None;
    }
    if n == 1 {
        // series for g = eps gamma(-1+eps) + 1 + eps/2 (1+3eps)/(1-eps^2),
        // double precision for |eps| < 0.02
        const C0: f64 = 0.07721566490153286061;
        const C1: f64 = 0.08815966957356030521;
        const C2: f64 = -0.00436125434555340577;
        const C3: f64 = 0.01391065882004640689;
        const C4: f64 = -0.00409427227680839100;
        const C5: f64 = 0.00275661310191541584;
        const C6: f64 = -0.00124162645565305019;
        const C7: f64 = 0.00065267976121802783;
        const C8: f64 = -0.00032205261682710437;
        const C9: f64 = 0.00016229131039545456;
        let g5 = C5 + eps * (C6 + eps * (C7 + eps * (C8 + eps * C9)));
        let g = eps * (C0 + eps * (C1 + eps * (C2 + eps * (C3 + eps * (C4 + eps * g5)))));

        // eps gamma(-1+eps), a negative quantity
        let gam_e = g - 1.0 - 0.5 * eps * (1.0 + 3.0 * eps) / (1.0 - eps * eps);

        let lng = Real::ln(gam_e.abs() / eps.abs());
        let sgn = if eps > 0.0 { -1.0 } else { 1.0 };
        return Some((lng, sgn));
    }

    // series for sin(Pi(N+1-eps))/(Pi eps) modulo the sign,
    // double precision for |eps| < 0.02
    const CS1: f64 = -1.6449340668482264365;
    const CS2: f64 = 0.8117424252833536436;
    const CS3: f64 = -0.1907518241220842137;
    const CS4: f64 = 0.0261478478176548005;
    const CS5: f64 = -0.0023460810354558236;
    let e2 = eps * eps;
    let sin_ser = 1.0 + e2 * (CS1 + e2 * (CS2 + e2 * (CS3 + e2 * (CS4 + e2 * CS5))));

    // series for ln(gamma(1+N-eps)), double precision for |eps| < 0.02
    let aeps = eps.abs();
    let nu = n as u32;
    let nf = n as f64;
    let mut psi_2 = 0.0;
    let mut psi_3 = 0.0;
    let mut psi_4 = 0.0;
    let mut psi_5 = 0.0;
    let mut psi_6 = 0.0;
    let c0 = ln_factorial(nu);
    let psi_0 = psi_int(nu + 1);
    let psi_1 = psi_1_int(nu + 1);
    if aeps > 0.00001 {
        psi_2 = psi_n(2, nf + 1.0);
    }
    if aeps > 0.0002 {
        psi_3 = psi_n(3, nf + 1.0);
    }
    if aeps > 0.001 {
        psi_4 = psi_n(4, nf + 1.0);
    }
    if aeps > 0.005 {
        psi_5 = psi_n(5, nf + 1.0);
    }
    if aeps > 0.01 {
        psi_6 = psi_n(6, nf + 1.0);
    }
    let c1 = psi_0;
    let c2 = psi_1 / 2.0;
    let c3 = psi_2 / 6.0;
    let c4 = psi_3 / 24.0;
    let c5 = psi_4 / 120.0;
    let c6 = psi_5 / 720.0;
    let c7 = psi_6 / 5040.0;
    let lng_ser =
        c0 - eps * (c1 - eps * (c2 - eps * (c3 - eps * (c4 - eps * (c5 - eps * (c6 - eps * c7))))));

    // g = ln(|eps gamma(-N+eps)|)
    //   = -ln(gamma(1+N-eps)) + ln(|eps Pi/sin(Pi(N+1+eps))|)
    let g = -lng_ser - Real::ln(sin_ser);

    let lng = g - Real::ln(eps.abs());
    let sgn = (if is_odd(n) { -1.0 } else { 1.0 }) * (if eps > 0.0 { 1.0 } else { -1.0 });
    Some((lng, sgn))
}

/// `-0.5 / (GSL_DBL_EPSILON * M_PI)`: below this `x` has no fractional part
/// to extract and GSL refuses with `GSL_EROUND`.
fn lngamma_reflection_limit() -> f64 {
    -0.5 / (DBL_EPSILON * PI)
}

/// `gsl_sf_lngamma_e` (specfunc/gamma.c:1114), value only; `NaN` where GSL
/// returns an error status. Callers have already excluded `x == 0` and the
/// negative integers.
///
/// Ported from GSL 2.8 `specfunc/gamma.c` (commit `cf180cd7`), copyright (C)
/// 1996-2000 Gerard Jungman, GPL-3.0-or-later. Note that this entry point
/// reflects through `sin(pi (1 - x))` and [`lngamma_sgn_e`] through
/// `sin(pi x)`, exactly as upstream has them.
fn lngamma_e(x: f64) -> f64 {
    if (x - 1.0).abs() < 0.01 {
        return ln_gamma_1_pade(x - 1.0);
    }
    if (x - 2.0).abs() < 0.01 {
        return ln_gamma_2_pade(x - 2.0);
    }
    if x >= 0.5 {
        return ln_gamma_lanczos(x);
    }
    if x == 0.0 {
        return f64::NAN;
    }
    if x.abs() < 0.02 {
        return lngamma_sgn_0(x).0;
    }
    if x > lngamma_reflection_limit() {
        // Try to extract a fractional part from x.
        let z = 1.0 - x;
        let s = Real::sin(PI * z);
        let abs_s = s.abs();
        if s == 0.0 {
            return f64::NAN;
        }
        if abs_s < PI * 0.015 {
            // x is near a negative integer, -N
            if x < -2_147_483_648.0 + 2.0 {
                return f64::NAN;
            }
            let n = -((x - 0.5) as i32);
            let eps = x + n as f64;
            return match lngamma_sgn_sing(n, eps) {
                Some((lng, _)) => lng,
                None => f64::NAN,
            };
        }
        let lg_z = ln_gamma_lanczos(z);
        return LN_PI - (Real::ln(abs_s) + lg_z);
    }
    // |x| was too large to extract any fractional part
    f64::NAN
}

/// `gsl_sf_lngamma_sgn_e` (specfunc/gamma.c:1185), value only: `ln|Gamma(x)|`
/// and the sign of `Gamma(x)`; `(NaN, NaN)` where GSL returns an error status.
/// Callers have already excluded `x == 0` and the negative integers.
///
/// Ported from GSL 2.8 `specfunc/gamma.c` (commit `cf180cd7`), copyright (C)
/// 1996-2000 Gerard Jungman, GPL-3.0-or-later.
fn lngamma_sgn_e(x: f64) -> (f64, f64) {
    if (x - 1.0).abs() < 0.01 {
        return (ln_gamma_1_pade(x - 1.0), 1.0);
    }
    if (x - 2.0).abs() < 0.01 {
        return (ln_gamma_2_pade(x - 2.0), 1.0);
    }
    if x >= 0.5 {
        return (ln_gamma_lanczos(x), 1.0);
    }
    if x == 0.0 {
        return (f64::NAN, f64::NAN);
    }
    if x.abs() < 0.02 {
        return lngamma_sgn_0(x);
    }
    if x > lngamma_reflection_limit() {
        let z = 1.0 - x;
        let s = Real::sin(PI * x);
        let abs_s = s.abs();
        if s == 0.0 {
            return (f64::NAN, f64::NAN);
        }
        if abs_s < PI * 0.015 {
            // x is near a negative integer, -N
            if x < -2_147_483_648.0 + 2.0 {
                return (f64::NAN, f64::NAN);
            }
            let n = -((x - 0.5) as i32);
            let eps = x + n as f64;
            return match lngamma_sgn_sing(n, eps) {
                Some(v) => v,
                None => (f64::NAN, f64::NAN),
            };
        }
        let lg_z = ln_gamma_lanczos(z);
        let sgn = if s > 0.0 { 1.0 } else { -1.0 };
        return (LN_PI - (Real::ln(abs_s) + lg_z), sgn);
    }
    // x too large to extract a fractional part
    (f64::NAN, f64::NAN)
}

/// Series for `gammastar(x)`, double precision for `x > 10`: GSL's
/// `gammastar_ser`.
///
/// Ported from GSL 2.8 `specfunc/gamma.c:954` (commit `cf180cd7`), copyright
/// (C) 1996-2000 Gerard Jungman, GPL-3.0-or-later.
fn gammastar_ser(x: f64) -> f64 {
    // Stirling series for the correction to Log(Gamma(x))
    let y = 1.0 / (x * x);
    const C0: f64 = 1.0 / 12.0;
    const C1: f64 = -1.0 / 360.0;
    const C2: f64 = 1.0 / 1260.0;
    const C3: f64 = -1.0 / 1680.0;
    const C4: f64 = 1.0 / 1188.0;
    const C5: f64 = -691.0 / 360360.0;
    const C6: f64 = 1.0 / 156.0;
    const C7: f64 = -3617.0 / 122400.0;
    let ser = C0 + y * (C1 + y * (C2 + y * (C3 + y * (C4 + y * (C5 + y * (C6 + y * C7))))));
    Real::exp(ser / x)
}

/// `Gamma(x)` for `x >= 1/2`: GSL's `gamma_xgthalf`.
///
/// Ported from GSL 2.8 `specfunc/gamma.c:1019` (commit `cf180cd7`), copyright
/// (C) 1996-2000 Gerard Jungman, GPL-3.0-or-later. Integer arguments up to
/// `GSL_SF_FACT_NMAX + 1 = 171` come from the exact factorial table.
fn gamma_xgthalf(x: f64) -> f64 {
    if x == 0.5 {
        return 1.77245385090551602729817;
    }
    if x <= 171.0 && x == Real::floor(x) {
        // fact_table[n - 1]
        let n = x as usize;
        return match n.checked_sub(1).and_then(|i| FACT_TABLE.get(i)) {
            Some(&f) => f,
            None => f64::NAN,
        };
    }
    if (x - 1.0).abs() < 0.01 {
        // series for Gamma[1+eps] - 1/(1+eps)
        let eps = x - 1.0;
        const C1: f64 = 0.4227843350984671394;
        const C2: f64 = -0.01094400467202744461;
        const C3: f64 = 0.09252092391911371098;
        const C4: f64 = -0.018271913165599812664;
        const C5: f64 = 0.018004931096854797895;
        const C6: f64 = -0.006850885378723806846;
        const C7: f64 = 0.003998239557568466030;
        return 1.0 / x
            + eps * (C1 + eps * (C2 + eps * (C3 + eps * (C4 + eps * (C5 + eps * (C6 + eps * C7))))));
    }
    if (x - 2.0).abs() < 0.01 {
        // series for Gamma[1 + eps]
        let eps = x - 2.0;
        const C1: f64 = 0.4227843350984671394;
        const C2: f64 = 0.4118403304264396948;
        const C3: f64 = 0.08157691924708626638;
        const C4: f64 = 0.07424901075351389832;
        const C5: f64 = -0.00026698206874501476832;
        const C6: f64 = 0.011154045718130991049;
        const C7: f64 = -0.002852645821155340816;
        const C8: f64 = 0.0021039333406973880085;
        return 1.0
            + eps
                * (C1
                    + eps
                        * (C2 + eps * (C3 + eps * (C4 + eps * (C5 + eps * (C6 + eps * (C7 + eps * C8)))))));
    }
    if x < 5.0 {
        // Exponentiating the logarithm is fine, as long as the exponential is
        // not so large that it greatly amplifies the error.
        return Real::exp(ln_gamma_lanczos(x));
    }
    if x < 10.0 {
        // This is a sticky area. The logarithm is too large and the gammastar
        // series is not good.
        const GAMMA_8: f64 = 5040.0;
        let t = (2.0 * x - 15.0) / 5.0;
        let c = cheb_value(&GAMMA_5_10_DATA, t);
        return Real::exp(c) * GAMMA_8;
    }
    if x < GAMMA_XMAX {
        // We do not want to exponentiate the logarithm if x is large because
        // of the inevitable inflation of the error. So we carefully use pow()
        // and exp() with exact quantities.
        let p = Real::powf(x, 0.5 * x);
        let e = Real::exp(-x);
        let q = (p * e) * p;
        let pre = core::f64::consts::SQRT_2 * SQRT_PI * q / Real::sqrt(x);
        return pre * gammastar_ser(x);
    }
    // OVERFLOW_ERROR
    f64::INFINITY
}

/// `gsl_sf_gamma_e` (specfunc/gamma.c:1255), value only. `NaN` where GSL
/// returns an error status (`sin_term == 0`, i.e. a pole).
///
/// Near a negative integer this does NOT use `lngamma_sgn_sing`: it writes
/// `Gamma(x) = 1 / (sin_term * Gamma(1 - x))` with `sin_term = (-1)^rint(x)
/// sin(pi (x - rint(x))) / pi`, where `x - rint(x)` is exact, so the pole costs
/// no precision. Only `x <= -169` goes through the logarithm.
///
/// Ported from GSL 2.8 `specfunc/gamma.c` (commit `cf180cd7`), copyright (C)
/// 1996-2000 Gerard Jungman, GPL-3.0-or-later.
fn gamma_e(x: f64) -> f64 {
    if x < 0.5 {
        // GSL: `(int)floor(x+0.5)`, which is undefined beyond the int range.
        // The nearest integer is kept as a float; every |x| >= 2^53 is an
        // integer, i.e. a pole, and ends at the `sin_term == 0` refusal below.
        let rint_x = Real::floor(x + 0.5);
        let f_x = x - rint_x;
        let half = rint_x * 0.5;
        let sgn_gamma = if Real::floor(half) == half { 1.0 } else { -1.0 };
        let sin_term = sgn_gamma * Real::sin(PI * f_x) / PI;

        if sin_term == 0.0 {
            return f64::NAN;
        }
        if x > -169.0 {
            let g = gamma_xgthalf(1.0 - x);
            if sin_term.abs() * g * DBL_MIN < 1.0 {
                return 1.0 / (sin_term * g);
            }
            // UNDERFLOW_ERROR
            return 0.0;
        }
        // It is hard to control it here. We can only exponentiate the
        // logarithm and eat the loss of precision.
        let (lng, sgn) = lngamma_sgn_e(x);
        return exp_mult(lng, sgn);
    }
    gamma_xgthalf(x)
}

/// `gsl_sf_log_1plusx_e` (specfunc/log.c:165), value only; `NaN` for
/// `x <= -1`.
///
/// Ported from GSL 2.8 `specfunc/log.c` (commit `cf180cd7`), copyright (C)
/// 1996-2000 Gerard Jungman, GPL-3.0-or-later.
fn log_1plusx(x: f64) -> f64 {
    if x <= -1.0 {
        return f64::NAN;
    }
    if x.abs() < ROOT6_DBL_EPSILON {
        const C1: f64 = -0.5;
        const C2: f64 = 1.0 / 3.0;
        const C3: f64 = -1.0 / 4.0;
        const C4: f64 = 1.0 / 5.0;
        const C5: f64 = -1.0 / 6.0;
        const C6: f64 = 1.0 / 7.0;
        const C7: f64 = -1.0 / 8.0;
        const C8: f64 = 1.0 / 9.0;
        const C9: f64 = -1.0 / 10.0;
        let t = C5 + x * (C6 + x * (C7 + x * (C8 + x * C9)));
        return x * (1.0 + x * (C1 + x * (C2 + x * (C3 + x * (C4 + x * t)))));
    }
    if x.abs() < 0.5 {
        let t = 0.5 * (8.0 * x + 1.0) / (x + 2.0);
        return x * cheb_value(&LOPX_DATA, t);
    }
    Real::ln(1.0 + x)
}

/// `gsl_sf_gammastar_e` (specfunc/gamma.c:1299), value only:
/// `Gamma(x) / (sqrt(2 pi) x^(x - 1/2) e^(-x))`, `NaN` for `x <= 0`.
///
/// Ported from GSL 2.8 `specfunc/gamma.c` (commit `cf180cd7`), copyright (C)
/// 1996-2000 Gerard Jungman, GPL-3.0-or-later. (`gamma_inc.rs` holds a
/// private partial `gammastar` returning `Result`, which refuses `x < 0.5` and
/// the `gammastar_ser` range; it could not be reused without editing it.)
fn gammastar(x: f64) -> f64 {
    if x <= 0.0 {
        return f64::NAN;
    }
    if x < 0.5 {
        let lg = lngamma_e(x);
        let lx = Real::ln(x);
        let c = 0.5 * (core::f64::consts::LN_2 + LN_PI);
        let lnr_val = lg - (x - 0.5) * lx + x - c;
        return exp_checked(lnr_val);
    }
    if x < 2.0 {
        let t = 4.0 / 3.0 * (x - 0.5) - 1.0;
        return cheb_value(&GSTAR_A_DATA, t);
    }
    if x < 10.0 {
        let t = 0.25 * (x - 2.0) - 1.0;
        let c = cheb_value(&GSTAR_B_DATA, t);
        return c / (x * x) + 1.0 + 1.0 / (12.0 * x);
    }
    if x < 1.0 / ROOT4_DBL_EPSILON {
        return gammastar_ser(x);
    }
    if x < 1.0 / DBL_EPSILON {
        // Stirling formula for Gamma(x).
        let xi = 1.0 / x;
        return 1.0
            + xi / 12.0 * (1.0 + xi / 24.0 * (1.0 - xi * (139.0 / 180.0 + 571.0 / 8640.0 * xi)));
    }
    1.0
}

/// `isnegint` of specfunc/beta.c:30.
fn is_neg_int(x: f64) -> bool {
    x < 0.0 && x == Real::floor(x)
}

/// `ln|B(x, y)|` and the sign of `B(x, y)`: GSL's `gsl_sf_lnbeta_sgn_e`.
/// `(NaN, NaN)` where GSL returns `GSL_EDOM` (`x` or `y` zero or a negative
/// integer) or a round-off error.
///
/// When one argument dominates (`min/max < 0.2`) the three log-gammas would
/// cancel catastrophically, so GSL assembles the result from `gammastar` and
/// `log(1 + min/max)` instead (the issue text of gh:#1420 names `lnpoch`; the
/// source uses `gammastar`).
///
/// Ported from GSL 2.8 `specfunc/beta.c:42` (commit `cf180cd7`), copyright (C)
/// 1996-2000 Gerard Jungman, GPL-3.0-or-later.
///
/// # Example
///
/// ```
/// use petir::specfunc::gamma::ln_beta_sgn;
/// // B(a, 1) = 1/a, so B(-2.5, 1) = -0.4.
/// let (ln_abs, sgn) = ln_beta_sgn(-2.5, 1.0);
/// assert_eq!(sgn, -1.0);
/// assert!((ln_abs - 0.4_f64.ln()).abs() < 1e-14);
/// ```
pub fn ln_beta_sgn(x: f64, y: f64) -> (f64, f64) {
    if x.is_nan() || y.is_nan() {
        return (f64::NAN, f64::NAN);
    }
    if x == 0.0 || y == 0.0 || is_neg_int(x) || is_neg_int(y) {
        return (f64::NAN, f64::NAN);
    }

    // See if we can handle the positive case with min/max < 0.2
    if x > 0.0 && y > 0.0 {
        let max = if x > y { x } else { y };
        let min = if x < y { x } else { y };
        let rat = min / max;

        if rat < 0.2 {
            // min << max, so be careful with the subtraction
            let gsx = gammastar(x);
            let gsy = gammastar(y);
            let gsxy = gammastar(x + y);
            let lnopr = log_1plusx(rat);
            let lnpre_val = Real::ln(gsx * gsy / gsxy * core::f64::consts::SQRT_2 * SQRT_PI);
            let t1 = min * Real::ln(rat);
            let t2 = 0.5 * Real::ln(min);
            let t3 = (x + y - 0.5) * lnopr;
            let lnpow_val = t1 - t2 - t3;
            return (lnpre_val + lnpow_val, 1.0);
        }
    }

    // General case - fallback
    let xy = x + y;
    let (lgx, sgx) = ln_gamma_sgn_checked(x);
    let (lgy, sgy) = ln_gamma_sgn_checked(y);
    let (lgxy, sgxy) = ln_gamma_sgn_checked(xy);
    let sgn = sgx * sgy * sgxy;
    if sgn.is_nan() {
        return (f64::NAN, f64::NAN);
    }
    (lgx + lgy - lgxy, sgn)
}

/// [`lngamma_sgn_e`] with the poles refused (`NaN`) instead of `(inf, NaN)`.
fn ln_gamma_sgn_checked(x: f64) -> (f64, f64) {
    if x.is_nan() || (x <= 0.0 && x == x.trunc()) {
        return (f64::NAN, f64::NAN);
    }
    lngamma_sgn_e(x)
}

/// `gsl_sf_beta_e` (specfunc/beta.c:112), value only.
///
/// Ported from GSL 2.8 `specfunc/beta.c` (commit `cf180cd7`), copyright (C)
/// 1996-2000 Gerard Jungman, GPL-3.0-or-later.
fn beta_e(x: f64, y: f64) -> f64 {
    if x.is_nan() || y.is_nan() {
        return f64::NAN;
    }
    if (x > 0.0 && y > 0.0) && x < 50.0 && y < 50.0 {
        // Handle the easy case
        let gx = gamma_e(x);
        let gy = gamma_e(y);
        let gxy = gamma_e(x + y);
        return (gx * gy) / gxy;
    }
    if is_neg_int(x) || is_neg_int(y) {
        return f64::NAN;
    }
    if is_neg_int(x + y) {
        // infinity in the denominator
        return 0.0;
    }
    let (lb, sgn) = ln_beta_sgn(x, y);
    if sgn.is_nan() {
        return f64::NAN;
    }
    exp_checked(lb) * sgn
}

/// `ln(n choose m)`: GSL's `gsl_sf_lnchoose_e` (specfunc/gamma.c:1556).
/// `NaN` for `m > n` (GSL's `GSL_EDOM`).
///
/// Ported from GSL 2.8 `specfunc/gamma.c` (commit `cf180cd7`), copyright (C)
/// 1996-2000 Gerard Jungman, GPL-3.0-or-later.
///
/// # Example
///
/// ```
/// use petir::specfunc::gamma::ln_choose;
/// assert!((ln_choose(10, 5) - 252.0_f64.ln()).abs() < 1e-14);
/// assert_eq!(ln_choose(7, 0), 0.0);
/// ```
pub fn ln_choose(n: u32, m: u32) -> f64 {
    if m > n {
        return f64::NAN;
    }
    if m == n || m == 0 {
        return 0.0;
    }
    // C unsigned arithmetic: `m*2` wraps modulo 2^32 exactly as upstream's.
    let m = if m.wrapping_mul(2) > n { n - m } else { m };
    let nf = ln_factorial(n);
    let mf = ln_factorial(m);
    let nmmf = ln_factorial(n - m);
    nf - mf - nmmf
}

/// `gsl_sf_choose_e` (specfunc/gamma.c:1584), value only, for `m <= n`.
///
/// Ported from GSL 2.8 `specfunc/gamma.c` (commit `cf180cd7`), copyright (C)
/// 1996-2000 Gerard Jungman, GPL-3.0-or-later.
fn choose_e(n: u32, m: u32) -> f64 {
    if m == n || m == 0 {
        return 1.0;
    }
    if let (Some(&fnn), Some(&fm), Some(&fnm)) = (
        FACT_TABLE.get(n as usize),
        FACT_TABLE.get(m as usize),
        FACT_TABLE.get((n - m) as usize),
    ) {
        return (fnn / fm) / fnm;
    }
    // C unsigned arithmetic: `m*2` wraps modulo 2^32 exactly as upstream's.
    let m = if m.wrapping_mul(2) < n { n - m } else { m };

    if n - m < 64 {
        // compute product for a manageable number of terms
        let mut prod = 1.0_f64;
        let mut k = n;
        while k >= m + 1 {
            let tk = k as f64 / (k - m) as f64;
            if tk > f64::MAX / prod {
                // OVERFLOW_ERROR
                return f64::INFINITY;
            }
            prod *= tk;
            k -= 1;
        }
        return prod;
    }
    exp_checked(ln_choose(n, m))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE transcription gate for the Lanczos coefficient table.
    ///
    /// `libm::lgamma` is fdlibm's implementation — a completely independent
    /// algorithm (Cody-Hillstrom rational minimax, not Lanczos) from a
    /// different lineage. Agreement to 2e-15 across five decades therefore says
    /// the table above was copied correctly; a single mistyped digit would
    /// break this at the 1e-8 level or worse.
    #[test]
    fn ln_gamma_agrees_with_independent_fdlibm_lgamma() {
        let mut x = 1.0e-4_f64;
        let mut worst_rel = 0.0_f64;
        let mut worst_abs_near_zeros = 0.0_f64;
        while x < 1.0e5 {
            let ours = ln_gamma(x);
            let theirs = libm::lgamma(x);
            let abs = (ours - theirs).abs();
            if theirs.abs() > 1.0 {
                // Away from the zeros, hold the result to a relative bound.
                let rel = abs / theirs.abs();
                if rel > worst_rel {
                    worst_rel = rel;
                }
                assert!(
                    rel < 2e-15,
                    "x={x}: ours={ours}, fdlibm={theirs}, rel={rel}"
                );
            } else {
                // Near the zeros at x = 1 and x = 2, ln Gamma passes through
                // zero and no relative bound is meaningful; the absolute error
                // is what is bounded. See the module docs.
                if abs > worst_abs_near_zeros {
                    worst_abs_near_zeros = abs;
                }
                assert!(
                    abs < 5e-15,
                    "x={x}: ours={ours}, fdlibm={theirs}, abs={abs}"
                );
            }
            x *= 1.037;
        }
        // Measured 2026-09-14 on x86_64: 1.04e-15 and 2.06e-15 respectively.
        // These are recorded as regression bounds, not aspirations -- a change
        // that loosens either is a change in the numerics.
        assert!(worst_rel < 2e-15, "worst relative deviation {worst_rel:e}");
        assert!(
            worst_abs_near_zeros < 5e-15,
            "worst absolute deviation near the zeros {worst_abs_near_zeros:e}"
        );
    }
    /// The Pade branches are the reason `ln Gamma` is accurate at its zeros.
    ///
    /// `ln Gamma` vanishes at x = 1 and x = 2. Lanczos reaches those zeros by
    /// cancelling terms of magnitude about 7, so it gives up roughly 1.5
    /// decimal digits precisely where the answer is smallest. GSL switches to
    /// (2,2) Pade approximants for `|x - 1| < 0.01` and `|x - 2| < 0.01`, and
    /// PETIR ports both -- this test is what demonstrates the port is doing its
    /// job rather than sitting there unused.
    #[test]
    fn pade_branches_make_the_zeros_of_ln_gamma_exact() {
        // The Pade form is eps * (pade + corr), so eps = 0 gives exactly zero
        // by construction rather than by cancellation.
        assert_eq!(ln_gamma(1.0), 0.0, "ln Gamma(1) must vanish EXACTLY");
        assert_eq!(ln_gamma(2.0), 0.0, "ln Gamma(2) must vanish EXACTLY");

        // Inside the Pade windows, agreement with fdlibm is at the level of
        // fdlibm's own error.
        for &eps in &[-1e-3_f64, -1e-4, -1e-6, 1e-6, 1e-4, 1e-3, 9e-3] {
            for &base in &[1.0_f64, 2.0] {
                let x = base + eps;
                let abs = (ln_gamma(x) - libm::lgamma(x)).abs();
                assert!(
                    abs < 2e-17,
                    "x={x} (inside the Pade window): absolute error {abs:e}"
                );
            }
        }
    }

    /// The Pade window is half-open at 0.01, exactly as upstream has it, so
    /// just outside it the Lanczos cancellation returns. This is not a defect
    /// -- it is GSL's own boundary -- but it is worth pinning so that nobody
    /// "fixes" the window width without realising it changes the dispatch.
    #[test]
    fn just_outside_the_pade_window_lanczos_accuracy_returns() {
        for &base in &[1.0_f64, 2.0] {
            let x = base + 0.01;
            let abs = (ln_gamma(x) - libm::lgamma(x)).abs();
            assert!(
                abs < 5e-15,
                "x={x} (just outside the window): absolute error {abs:e}"
            );
        }
    }

    /// The reflection branch, checked on the negative axis where fdlibm also
    /// returns ln|Gamma|.
    #[test]
    fn ln_gamma_reflection_agrees_with_fdlibm_on_the_negative_axis() {
        for &x in &[-0.5_f64, -1.5, -2.5, -3.25, -10.7, -20.3] {
            let ours = ln_gamma(x);
            let theirs = libm::lgamma(x);
            let rel = (ours - theirs).abs() / theirs.abs().max(1.0);
            assert!(
                rel < 1e-14,
                "x={x}: ours={ours}, fdlibm={theirs}, rel={rel}"
            );
        }
    }

    /// Exact values that need no reference implementation at all.
    #[test]
    fn gamma_reproduces_the_exact_closed_forms() {
        // Gamma(n) = (n-1)!
        assert!((gamma(1.0) - 1.0).abs() < 1e-15);
        assert!((gamma(5.0) - 24.0).abs() < 1e-12);
        assert!((gamma(11.0) - 3_628_800.0).abs() / 3_628_800.0 < 1e-13);
        // Gamma(1/2) = sqrt(pi); Gamma(3/2) = sqrt(pi)/2
        assert!((gamma(0.5) - PI.sqrt()).abs() < 1e-14);
        assert!((gamma(1.5) - 0.5 * PI.sqrt()).abs() < 1e-14);
        // Gamma(-1/2) = -2 sqrt(pi)
        assert!((gamma(-0.5) + 2.0 * PI.sqrt()).abs() < 1e-13);
    }

    #[test]
    fn gamma_has_no_value_at_the_poles() {
        for &x in &[0.0, -1.0, -2.0, -17.0] {
            assert!(gamma(x).is_nan(), "gamma({x}) should be NaN at a pole");
            assert_eq!(ln_gamma(x), f64::INFINITY);
        }
    }

    #[test]
    fn gamma_alternates_sign_on_the_negative_axis() {
        assert!(gamma(-0.5) < 0.0, "Gamma is negative on (-1, 0)");
        assert!(gamma(-1.5) > 0.0, "Gamma is positive on (-2, -1)");
        assert!(gamma(-2.5) < 0.0, "Gamma is negative on (-3, -2)");
    }

    #[test]
    fn factorials_are_exact_where_f64_allows() {
        assert_eq!(factorial(0), 1.0);
        assert_eq!(factorial(1), 1.0);
        assert_eq!(factorial(12), 479_001_600.0);
        assert_eq!(factorial(20), 2_432_902_008_176_640_000.0);
        // The recurrence must hold across the table boundary at 20 -> 21.
        let rel = (factorial(21) - 21.0 * factorial(20)).abs() / factorial(21);
        assert!(rel < 1e-13, "factorial is discontinuous at the table edge");
    }

    #[test]
    fn ln_factorial_survives_where_factorial_overflows() {
        assert_eq!(factorial(200), f64::INFINITY);
        assert!(ln_factorial(200).is_finite());
        // Stirling: ln(n!) ~ n ln n - n + 0.5 ln(2 pi n)
        let n = 200.0_f64;
        let stirling = n * n.ln() - n + 0.5 * (2.0 * PI * n).ln();
        assert!((ln_factorial(200) - stirling).abs() < 1e-3);
    }

    #[test]
    fn beta_matches_its_closed_forms() {
        // B(1, 1) = 1; B(2, 3) = 1/12; B(1/2, 1/2) = pi
        // 1e-14 rather than 1e-15: beta(1,1) routes through ln_gamma(1) and
        // ln_gamma(2), which are exactly the two points where the Lanczos form
        // loses relative accuracy (see the module docs and
        // `ln_gamma_loses_relative_accuracy_near_its_zeros_as_documented`).
        assert!((beta(1.0, 1.0) - 1.0).abs() < 1e-14);
        assert!((beta(2.0, 3.0) - 1.0 / 12.0).abs() < 1e-15);
        assert!((beta(0.5, 0.5) - PI).abs() < 1e-13);
        // Symmetry
        assert!((beta(2.7, 4.1) - beta(4.1, 2.7)).abs() < 1e-16);
    }

    #[test]
    fn beta_rejects_non_positive_arguments() {
        assert!(beta(0.0, 1.0).is_nan());
        assert!(beta(1.0, -1.0).is_nan());
        assert!(ln_beta(-2.0, 3.0).is_nan());
    }

    #[test]
    fn choose_is_exact_for_small_arguments() {
        assert_eq!(choose(5, 0), 1.0);
        assert_eq!(choose(5, 5), 1.0);
        assert_eq!(choose(5, 2), 10.0);
        assert_eq!(choose(10, 5), 252.0);
        assert_eq!(choose(52, 5), 2_598_960.0);
        assert_eq!(choose(3, 7), 0.0, "k > n is the empty selection");
    }

    #[test]
    fn choose_is_symmetric_and_survives_the_naive_overflow() {
        assert_eq!(choose(100, 50), choose(100, 50));
        // 100! overflows f64; C(100, 50) does not.
        let c = choose(100, 50);
        assert!(c.is_finite());
        assert!((c - 1.008_913_445_455_642_2e29).abs() / 1e29 < 1e-12);
        // Pascal's rule, as an independent check. ~~`assert_eq!`~~ **CHANGED
        // 2026-11-12 (gh:#1420):** `choose` is now GSL's table quotient
        // `(n!/k!)/(n-k)!`, which is not rounded to the integer (GSL's own
        // error estimate is `6 eps`; e.g. `C(23, 2)` is 253.00000000000003).
        // Each side is therefore held to GSL's bound, `6 eps` per operand.
        for n in 1..25u32 {
            for k in 1..n {
                let lhs = choose(n, k);
                let rhs = choose(n - 1, k - 1) + choose(n - 1, k);
                let bound = 18.0 * DBL_EPSILON * lhs;
                assert!((lhs - rhs).abs() <= bound, "Pascal's rule failed at C({n},{k}): {lhs} vs {rhs}");
            }
        }
    }

    // ---- gh:#1419 / gh:#1420 ports (2026-11-12) --------------------------
    //
    // The independent references are `libm`'s fdlibm `lgamma_r` / `tgamma`
    // (a Cody-Hillstrom rational minimax, not GSL's series) and exact
    // recurrences. The comparison against compiled GSL itself is
    // `tests/gsl_specfunc_code_to_code.rs`.

    /// `ln|Gamma|` and sign of `Gamma` near every pole `-N + eps`, N = 1..=40,
    /// from `eps = 1e-16` (one ulp at N = 1) to the edge of GSL's series
    /// window (0.02), against fdlibm's `lgamma_r`. Before the port the relative
    /// error was `1e-16/|eps|` (gh:#1419). The bound is 1e-14 for `|eps| <=
    /// 0.01` and 1e-13 relative to `max(1, |ln Gamma|)` above it: GSL's series claims double precision for
    /// `|eps| < 0.02` but is truncated after the sixth derivative, and glibc's
    /// `lgamma` (0.709617711420476) agrees with fdlibm (0.7096177114204767),
    /// not with GSL (0.7096177114204909, rel 2.0e-14), at `x = -4.0199`, and the
    /// gap reaches 3.8e-14 absolute at `x = -4.9801` and 1.35e-13 at `x = -20.0199`. The
    /// port reproduces GSL's value (`tests/gsl_specfunc_code_to_code.rs`), so
    /// this is an upstream accuracy limit, recorded here and not copied.
    #[test]
    fn ln_gamma_and_sign_near_every_pole_agree_with_fdlibm() {
        let mut worst = 0.0_f64;
        for n in 1..=40u32 {
            for &eps in &[1e-16, 1e-13, 1e-9, 1e-6, 1e-4, 1e-3, 5e-3, 0.01, 0.0199] {
                for &sign in &[1.0_f64, -1.0] {
                    let x = -(n as f64) + sign * eps;
                    if x == -(n as f64) {
                        continue; // eps below one ulp: this is the pole itself
                    }
                    let (reference, ref_sign) = libm::lgamma_r(x);
                    let ours = ln_gamma(x);
                    // the bound is relative to |ln Gamma| (1e-14), or absolute
                    // (1e-13, floor |ln Gamma| at 1) in the outer half of the series window
                    let scale = if eps <= 0.01 { reference.abs() } else { reference.abs().max(1.0) };
                    let bound = if eps <= 0.01 { 1e-14 } else { 1e-13 };
                    let rel = (ours - reference).abs() / scale;
                    worst = worst.max(rel);
                    assert!(rel < bound, "ln_gamma({x:e}) = {ours}, fdlibm {reference}, err {rel:e}");
                    let (ln_abs, sgn) = ln_gamma_sgn(x);
                    assert!((ln_abs - reference).abs() / scale < bound);
                    assert_eq!(sgn, ref_sign as f64, "sign of Gamma({x:e})");
                }
            }
        }
        assert!(worst < 1e-13, "worst error {worst:e}");
    }

    /// `Gamma(-N + eps)` against the exact recurrence
    /// `Gamma(x) = Gamma(1 + eps') / prod_{k=0..N} (x + k)` with
    /// `eps' = x + N` (exact in floating point), which shares nothing with the
    /// reflection or the `lngamma_sgn_sing` series.
    #[test]
    fn gamma_near_poles_agrees_with_the_exact_recurrence() {
        for n in 1..=30u32 {
            for &eps in &[1e-16, 1e-12, 1e-8, 1e-4, 0.003, 0.0199] {
                for &sign in &[1.0_f64, -1.0] {
                    let x = -(n as f64) + sign * eps;
                    if x == -(n as f64) {
                        continue;
                    }
                    let eps_exact = x + n as f64;
                    let mut prod = 1.0_f64;
                    for k in 0..=n {
                        prod *= x + k as f64;
                    }
                    let recurrence = gamma(1.0 + eps_exact) / prod;
                    let ours = gamma(x);
                    let rel = (ours - recurrence).abs() / recurrence.abs();
                    assert!(
                        rel < 4e-14,
                        "gamma({x:e}) = {ours:e}, recurrence {recurrence:e}, rel {rel:e}"
                    );
                }
            }
        }
        // One ulp from -1: Gamma(-1 + eps) = -1/eps (1 + O(eps)) to a relative
        // 4e-15 (gh:#1419 measured 10 % here before the port).
        let x = -1.0000000000000002_f64;
        assert!(((gamma(x) + 1.0 / (x + 1.0)) * (x + 1.0)).abs() < 4e-15);
    }

    /// `lngamma_sgn_0`: |x| < 0.02 on both sides, including the subnormal guard.
    #[test]
    fn ln_gamma_near_zero_agrees_with_fdlibm_and_the_subnormal_guard_holds() {
        for &x in &[1e-8, 1e-4, 1e-3, 0.0101, 0.019, -1e-8, -1e-4, -0.0101, -0.0199] {
            let (reference, ref_sign) = libm::lgamma_r(x);
            let (ours, sgn) = ln_gamma_sgn(x);
            assert!((ours - reference).abs() / reference.abs() < 1e-14, "x={x}");
            assert_eq!(sgn, ref_sign as f64, "sign at {x}");
            assert!((ln_gamma(x) - reference).abs() / reference.abs() < 1e-14, "x={x}");
        }
        // DEVIATION recorded at lngamma_sgn_0: upstream gives +inf here.
        for &x in &[1e-310_f64, -1e-310] {
            let (reference, _) = libm::lgamma_r(x);
            assert!((ln_gamma(x) - reference).abs() / reference < 1e-14, "x={x}");
        }
    }

    /// The refusal at `|x| > 0.5/(eps pi)`.
    #[test]
    fn ln_gamma_without_a_fractional_part_is_refused_not_wrong() {
        let x = -7.5e14_f64 - 0.5;
        assert!(ln_gamma(x).is_nan());
        assert!(ln_gamma_sgn(x).0.is_nan());
        // Just inside the limit the fractional part is still extracted.
        assert!(ln_gamma(-1.0e9 - 0.5).is_finite());
        // Beyond the int range GSL's own refusal, `x < INT_MIN + 2`, near a pole.
        assert!(ln_gamma(-3.0e9 + 0.001).is_nan());
    }

    /// `gamma_xgthalf`: every branch against fdlibm's `tgamma`, and the exact
    /// table for integers.
    #[test]
    fn gamma_branches_agree_with_fdlibm_tgamma() {
        for &x in &[
            0.5, 0.75, 0.995, 1.004, 1.5, 1.995, 2.004, 2.5, 4.75, 5.0001, 7.3, 9.99, 10.5, 25.25, 60.1,
            100.5, 150.75, 170.9,
        ] {
            let reference = libm::tgamma(x);
            let ours = gamma(x);
            let rel = (ours - reference).abs() / reference;
            assert!(rel < 1e-13, "gamma({x}) = {ours:e}, fdlibm {reference:e}, rel {rel:e}");
        }
        for n in 1..=171u32 {
            assert_eq!(gamma(n as f64), factorial(n - 1), "Gamma({n}) must be the table entry");
        }
        assert!(gamma(171.5).is_infinite(), "GSL_SF_GAMMA_XMAX: overflow above 171");
        // The logarithm route, x <= -169.
        for &x in &[-169.5_f64, -171.25, -180.75] {
            let (l, s) = libm::lgamma_r(x);
            let reference = s as f64 * libm::exp(l);
            if reference != 0.0 && reference.is_normal() {
                let rel = (gamma(x) - reference).abs() / reference.abs();
                assert!(rel < 1e-11, "gamma({x}) = {:e}, reference {reference:e}", gamma(x));
            }
        }
        assert!(gamma(-0.5) < 0.0);
    }

    #[test]
    fn factorial_table_is_exact_then_correctly_rounded() {
        // Exact integer arithmetic for the first 23 entries (22! is the last
        // one exact in f64).
        let mut exact: u128 = 1;
        for n in 0..=22u32 {
            if n > 0 {
                exact *= n as u128;
            }
            assert_eq!(factorial(n), exact as f64, "{n}!");
            assert_eq!(factorial(n) as u128, exact, "{n}! must be exact in f64");
        }
        // The recurrence n! = n (n-1)! to one rounding, over the whole table.
        for n in 1..=170u32 {
            let rel = (factorial(n) - n as f64 * factorial(n - 1)).abs() / factorial(n);
            assert!(rel <= 2.3e-16, "recurrence at {n}: {rel:e}");
        }
        assert!(factorial(170).is_finite() && factorial(171).is_infinite());
        // lnfact: the log of the table entry, then lngamma(n+1) beyond 170.
        assert_eq!(ln_factorial(0), 0.0);
        let (l, _) = libm::lgamma_r(172.0);
        assert!((ln_factorial(171) - l).abs() / l < 1e-15);
    }

    /// `choose` against exact 128-bit integer arithmetic on all three of GSL's
    /// branches, and `ln_choose` against fdlibm.
    #[test]
    fn choose_agrees_with_exact_integers_on_every_branch() {
        // Exact C(n, k) by the multiplicative formula in u128.
        let exact = |n: u128, k: u128| -> u128 {
            let k = if k > n - k { n - k } else { k };
            let mut r: u128 = 1;
            for i in 1..=k {
                r = r * (n - k + i) / i;
            }
            r
        };
        // fact_table branch (n <= 170): table quotient, GSL err 6 eps.
        for &(n, k) in &[(23u32, 2u32), (52, 5), (60, 30), (100, 3)] {
            let e = exact(n as u128, k as u128) as f64;
            assert!((choose(n, k) - e).abs() <= 6.0 * DBL_EPSILON * e, "C({n},{k})");
        }
        // Product branch (n > 170, n - k < 64): GSL err 2 eps (n-k).
        for &(n, k) in &[(100_000u32, 5u32), (171, 3), (200, 20), (250, 15), (1_000_000, 4)] {
            let e = exact(n as u128, k as u128) as f64;
            let m = k.min(n - k) as f64;
            let rel = (choose(n, k) - e).abs() / e;
            assert!(rel <= 2.0 * DBL_EPSILON * m, "C({n},{k}): rel {rel:e}");
        }
        // The headline value of gh:#1420: 8.332500029166249e22, GSL's bits.
        assert_eq!(choose(100_000, 5), 8.332500029166249e22);
        assert_eq!(choose(100_000, 99_995), 8.332500029166249e22);
        // Log branch (n - m >= 64): fdlibm lgamma combination.
        let ln_c = ln_choose(1000, 500);
        let (a, _) = libm::lgamma_r(1001.0);
        let (b, _) = libm::lgamma_r(501.0);
        assert!((ln_c - (a - 2.0 * b)).abs() / ln_c < 1e-13);
        let c = choose(1000, 500);
        assert!((Real::ln(c) - ln_c).abs() / ln_c < 1e-13);
        assert!(ln_choose(5, 6).is_nan());
        assert_eq!(ln_choose(9, 9), 0.0);
        // The product loop reports overflow as +inf.
        assert!(choose(u32::MAX, 40).is_infinite());
        assert_eq!(choose(100, 101), 0.0, "documented API difference: GSL refuses");
    }

    /// `ln_beta` / `beta` where `lnG(a) + lnG(b) - lnG(a+b)` cancels
    /// (gh:#1420 item 1): `B(1, b) = 1/b` and `B(2, b) = 1/(b (b+1))` are
    /// exact, so the dominant-argument branch (`gammastar` + `log_1plusx`)
    /// has a closed form to be checked against.
    #[test]
    fn ln_beta_with_one_dominant_argument_has_no_cancellation() {
        for &b in &[1e3_f64, 1e6, 1e10, 1e15] {
            let want = -Real::ln(b);
            let got = ln_beta(1.0, b);
            assert!((got - want).abs() <= 4.0 * DBL_EPSILON * want.abs(), "ln_beta(1,{b:e}) = {got}");
            let want2 = -Real::ln(b * (b + 1.0));
            let got2 = ln_beta(2.0, b);
            assert!((got2 - want2).abs() <= 4.0 * DBL_EPSILON * want2.abs(), "ln_beta(2,{b:e})");
        }
        // The case the issue measured: -23.025850929940457 (4e-5 off before).
        assert!((ln_beta(1.0, 1e10) + 23.025850929940457).abs() < 2e-14);
        // Small first argument against the large-b expansion
        // lnB = lnG(a) - a ln b - a (a-1) / (2 b) + O(b^-2).
        for &a in &[1e-8_f64, 0.01, 0.3, 0.9] {
            let b = 1e10_f64;
            let (lg, _) = libm::lgamma_r(a);
            let want = lg - a * Real::ln(b) - a * (a - 1.0) / (2.0 * b);
            let got = ln_beta(a, b);
            assert!(
                (got - want).abs() <= 1e-14 * want.abs().max(1.0),
                "ln_beta({a},{b:e}) = {got}, {want}"
            );
        }
        assert!(((beta(1e-8, 1e10) - 99_999_976.397) / 99_999_976.397).abs() < 1e-9);
    }

    /// Negative non-integer arguments (gh:#1420 item 2) against fdlibm's
    /// `lgamma_r` triple, and the sign.
    #[test]
    fn beta_for_negative_non_integers_agrees_with_fdlibm() {
        for &(a, b) in &[
            (-0.5_f64, 0.1_f64),
            (-2.5, 1.0),
            (-0.5, 10.0),
            (-1.5, 1.7),
            (0.01, -0.5),
            (-3.25, -0.5),
            (30.5, -0.25),
        ] {
            let (la, sa) = libm::lgamma_r(a);
            let (lb, sb) = libm::lgamma_r(b);
            let (lab, sab) = libm::lgamma_r(a + b);
            let want = la + lb - lab;
            let want_sign = (sa * sb * sab) as f64;
            let (got, sgn) = ln_beta_sgn(a, b);
            assert_eq!(sgn, want_sign, "sign of B({a},{b})");
            assert!(
                (got - want).abs() <= 2e-14 * want.abs().max(1.0),
                "ln|B({a},{b})| = {got}, {want}"
            );
            let value = beta(a, b);
            let expected = want_sign * Real::exp(want);
            assert!((value - expected).abs() <= 1e-13 * expected.abs(), "B({a},{b}) = {value}");
            if want_sign < 0.0 {
                assert!(ln_beta(a, b).is_nan(), "a negative B has no real logarithm");
            }
        }
        // Exact: B(a, 1) = 1/a.
        assert!((beta(-2.5, 1.0) + 0.4).abs() < 4e-16);
        // x or y zero / negative integer: refused. x + y a negative integer: 0.
        assert!(beta(0.0, 1.0).is_nan() && beta(-2.0, 0.5).is_nan());
        assert!(ln_beta_sgn(-3.0, 1.5).0.is_nan());
        assert_eq!(beta(-2.5, 0.5), 0.0);
        assert!(beta(f64::NAN, 1.0).is_nan() && ln_beta_sgn(1.0, f64::NAN).0.is_nan());
        assert!(beta(3.0, 80.0).is_finite());
        assert_eq!(beta(2000.0, 2000.0), 0.0, "underflow is reported as 0, as GSL does");
    }
}
