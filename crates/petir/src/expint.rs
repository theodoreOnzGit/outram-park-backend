// Ported from the GNU Scientific Library `specfunc/expint.c` and
// `specfunc/cheb_eval.c`, GSL 2.8 at commit
// cf180cd7fbd06039a577f9c9ff0b428784765ac1, read 2026-09-14.
//
// Copyright (C) 2007 Brian Gough
// Copyright (C) 1996, 1997, 1998, 1999, 2000 Gerard Jungman   (upstream)
// Copyright (C) 2026 Theodore Ong and the outram-park contributors  (this port)
//
// GSL is GPL-3.0-or-later (verified from its per-file headers -- see NOTICE).
// This derivative work is GPL-3.0-only. The flow is ONE-WAY.

//! The exponential integral `E_1(x)` — a port of GSL's `gsl_sf_expint_E1`.
//!
//! # Why this is here
//!
//! `E_1` is the first of the two special functions NJOY's ACER needs for the
//! MF=5 `LF = 12` (Madland-Nix) fission spectrum: `acefc.f90:9155`'s `fmn`
//! evaluates the Madland-Nix shape in terms of `e1` and the incomplete gamma.
//! See GitHub #201.
//!
//! It is also the reason PETIR's Chebyshev machinery exists in the form it
//! does: `E_1` **is** six Chebyshev series, selected by range, and nothing
//! else. Porting it is what "wiring the Chebyshev polynomials into ACER" means
//! concretely.
//!
//! # The coefficient tables were extracted mechanically
//!
//! All 150 coefficients across the six series were parsed out of
//! `specfunc/expint.c` by script and written here, rather than retyped. That
//! removes the transcription risk that made this port look expensive when it
//! was scoped against SLATEC (#201) — and the values are checked against GSL
//! compiled and run regardless, in `tests/gsl_expint.rs`.

use crate::error::{PetirError, Result};

/// `GSL_DBL_EPSILON`.
const EPS: f64 = 2.2204460492503131e-16;
/// `GSL_LOG_DBL_MIN` (`gsl_machine.h`) — `log` of the smallest normal double.
const LOG_DBL_MIN: f64 = -7.0839641853226408e+02;

/// GSL `AE11_cs` (`specfunc/expint.c`): order 38 on `[-1, 1]`.
const AE11_DATA: [f64; 39] = [
    0.121503239716065790,
    -0.065088778513550150,
    0.004897651357459670,
    -0.000649237843027216,
    0.000093840434587471,
    0.000000420236380882,
    -0.000008113374735904,
    0.000002804247688663,
    0.000000056487164441,
    -0.000000344809174450,
    0.000000058209273578,
    0.000000038711426349,
    -0.000000012453235014,
    -0.000000005118504888,
    0.000000002148771527,
    0.000000000868459898,
    -0.000000000343650105,
    -0.000000000179796603,
    0.000000000047442060,
    0.000000000040423282,
    -0.000000000003543928,
    -0.000000000008853444,
    -0.000000000000960151,
    0.000000000001692921,
    0.000000000000607990,
    -0.000000000000224338,
    -0.000000000000200327,
    -0.000000000000006246,
    0.000000000000045571,
    0.000000000000016383,
    -0.000000000000005561,
    -0.000000000000006074,
    -0.000000000000000862,
    0.000000000000001223,
    0.000000000000000716,
    -0.000000000000000024,
    -0.000000000000000201,
    -0.000000000000000082,
    0.000000000000000017,
];

/// GSL `AE12_cs` (`specfunc/expint.c`): order 24 on `[-1, 1]`.
const AE12_DATA: [f64; 25] = [
    0.582417495134726740,
    -0.158348850905782750,
    -0.006764275590323141,
    0.005125843950185725,
    0.000435232492169391,
    -0.000143613366305483,
    -0.000041801320556301,
    -0.000002713395758640,
    0.000001151381913647,
    0.000000420650022012,
    0.000000066581901391,
    0.000000000662143777,
    -0.000000002844104870,
    -0.000000000940724197,
    -0.000000000177476602,
    -0.000000000015830222,
    0.000000000002905732,
    0.000000000001769356,
    0.000000000000492735,
    0.000000000000093709,
    0.000000000000010707,
    -0.000000000000000537,
    -0.000000000000000716,
    -0.000000000000000244,
    -0.000000000000000058,
];

/// GSL `E11_cs` (`specfunc/expint.c`): order 18 on `[-1, 1]`.
const E11_DATA: [f64; 19] = [
    -16.11346165557149402600,
    7.79407277874268027690,
    -1.95540581886314195070,
    0.37337293866277945612,
    -0.05692503191092901938,
    0.00721107776966009185,
    -0.00078104901449841593,
    0.00007388093356262168,
    -0.00000620286187580820,
    0.00000046816002303176,
    -0.00000003209288853329,
    0.00000000201519974874,
    -0.00000000011673686816,
    0.00000000000627627066,
    -0.00000000000031481541,
    0.00000000000001479904,
    -0.00000000000000065457,
    0.00000000000000002733,
    -0.00000000000000000108,
];

/// GSL `E12_cs` (`specfunc/expint.c`): order 15 on `[-1, 1]`.
const E12_DATA: [f64; 16] = [
    -0.03739021479220279500,
    0.04272398606220957700,
    -0.13031820798497005440,
    0.01441912402469889073,
    -0.00134617078051068022,
    0.00010731029253063780,
    -0.00000742999951611943,
    0.00000045377325690753,
    -0.00000002476417211390,
    0.00000000122076581374,
    -0.00000000005485141480,
    0.00000000000226362142,
    -0.00000000000008635897,
    0.00000000000000306291,
    -0.00000000000000010148,
    0.00000000000000000315,
];

/// GSL `AE13_cs` (`specfunc/expint.c`): order 24 on `[-1, 1]`.
const AE13_DATA: [f64; 25] = [
    -0.605773246640603460,
    -0.112535243483660900,
    0.013432266247902779,
    -0.001926845187381145,
    0.000309118337720603,
    -0.000053564132129618,
    0.000009827812880247,
    -0.000001885368984916,
    0.000000374943193568,
    -0.000000076823455870,
    0.000000016143270567,
    -0.000000003466802211,
    0.000000000758754209,
    -0.000000000168864333,
    0.000000000038145706,
    -0.000000000008733026,
    0.000000000002023672,
    -0.000000000000474132,
    0.000000000000112211,
    -0.000000000000026804,
    0.000000000000006457,
    -0.000000000000001568,
    0.000000000000000383,
    -0.000000000000000094,
    0.000000000000000023,
];

/// GSL `AE14_cs` (`specfunc/expint.c`): order 25 on `[-1, 1]`.
const AE14_DATA: [f64; 26] = [
    -0.18929180007530170,
    -0.08648117855259871,
    0.00722410154374659,
    -0.00080975594575573,
    0.00010999134432661,
    -0.00001717332998937,
    0.00000298562751447,
    -0.00000056596491457,
    0.00000011526808397,
    -0.00000002495030440,
    0.00000000569232420,
    -0.00000000135995766,
    0.00000000033846628,
    -0.00000000008737853,
    0.00000000002331588,
    -0.00000000000641148,
    0.00000000000181224,
    -0.00000000000052538,
    0.00000000000015592,
    -0.00000000000004729,
    0.00000000000001463,
    -0.00000000000000461,
    0.00000000000000148,
    -0.00000000000000048,
    0.00000000000000016,
    -0.00000000000000005,
];
/// GSL's `cheb_eval_e` (`specfunc/cheb_eval.c:8-34`): Clenshaw on `[a, b]`
/// carrying a running absolute-error estimate.
///
/// This is *not* the same as [`crate::eval_gsl`]: the specfunc variant
/// accumulates `e` as it goes, which the `cheb/` module's evaluator does not.
/// Both are upstream code; they are different upstream functions.
fn cheb_eval(c: &[f64], order: usize, a: f64, b: f64, x: f64) -> (f64, f64) {
    let (mut d, mut dd) = (0.0f64, 0.0f64);
    let y = (2.0 * x - a - b) / (b - a);
    let y2 = 2.0 * y;
    let mut e = 0.0f64;

    // One split supplies all three reads upstream makes by subscript: the
    // zeroth coefficient for the closing half-term, coefficients 1 through
    // `order` for the recurrence (walked backwards), and the `order`-th for
    // the truncation estimate. Every caller here passes a constant `order`
    // matching its own table, so the `else` arms are unreachable; they yield
    // NaN with an infinite error bound -- which propagates loudly -- rather
    // than ending the program on a microcontroller (see
    // tests/no_panic_gate.rs).
    let (Some((&c0, c_tail)), Some(&c_order)) = (c.split_first(), c.get(order)) else {
        return (f64::NAN, f64::INFINITY);
    };
    let Some(used) = c_tail.get(..order) else {
        return (f64::NAN, f64::INFINITY);
    };

    for &c_j in used.iter().rev() {
        let temp = d;
        d = y2 * d - dd + c_j;
        e += libm::fabs(y2 * temp) + libm::fabs(dd) + libm::fabs(c_j);
        dd = temp;
    }
    let temp = d;
    d = y * d - dd + 0.5 * c0;
    e += libm::fabs(y * temp) + libm::fabs(dd) + 0.5 * libm::fabs(c0);

    (d, EPS * e + libm::fabs(c_order))
}

/// `E_1(x)` with an absolute-error estimate — `gsl_sf_expint_E1_e`
/// (`specfunc/expint.c:466`, via `expint_E1_impl` at `:290`).
///
/// Returns `(value, abserr)`.
///
/// # Errors
/// - [`PetirError::Domain`] at `x == 0`, where `E_1` diverges
///   (`DOMAIN_ERROR`, `:329`).
/// - [`PetirError::ZeroDivide`] for `x < -xmax`, upstream's `OVERFLOW_ERROR`
///   (`:297`) — the result would exceed the double range.
/// - [`PetirError::Tolerance`] where upstream signals `UNDERFLOW_ERROR`
///   (`:365`, `:370`): the result has fallen below the smallest normal double.
pub fn expint_e1(x: f64) -> Result<(f64, f64)> {
    expint_e1_impl(x, false)
}

/// `exp(x) E_1(x)` — the scaled form, `gsl_sf_expint_E1_scaled_e`.
///
/// Same branches, with the exponential factored out so the result stays in
/// range for large `x`.
pub fn expint_e1_scaled(x: f64) -> Result<(f64, f64)> {
    expint_e1_impl(x, true)
}

/// `Ei(x)`, the exponential integral, with an absolute-error estimate —
/// `gsl_sf_expint_Ei_e` (`specfunc/expint.c:501`).
///
/// ```text
///     Ei(x) = -PV integral_{-x}^{inf} e^{-t} / t  dt
/// ```
///
/// **Upstream defines it as `-E_1(-x)` and nothing more**, so this is that
/// one line rather than a second branch tree. Reading the source is what
/// establishes there is no separate implementation to port; the six
/// Chebyshev series above are the whole of it.
///
/// Returns `(value, abserr)`. `x` is dimensionless.
///
/// # Errors
///
/// Whatever [`expint_e1`] returns for `-x`: [`PetirError::Domain`] at
/// `x == 0` where `Ei` diverges, and the overflow/underflow conditions with
/// their signs mirrored.
///
/// # Examples
///
/// ```
/// use petir::expint::expint_ei;
/// // Ei(1) = 1.8951178163559368...
/// let (v, _) = expint_ei(1.0).unwrap();
/// assert!((v - 1.895_117_816_355_936_8).abs() < 1e-14);
/// ```
pub fn expint_ei(x: f64) -> Result<(f64, f64)> {
    let (v, e) = expint_e1_impl(-x, false)?;
    Ok((-v, e))
}

/// `exp(-x) Ei(x)` — the scaled form, `gsl_sf_expint_Ei_scaled_e`
/// (`specfunc/expint.c:514`). Same one-line relation to
/// [`expint_e1_scaled`].
pub fn expint_ei_scaled(x: f64) -> Result<(f64, f64)> {
    let (v, e) = expint_e1_impl(-x, true)?;
    Ok((-v, e))
}

fn expint_e1_impl(x: f64, scale: bool) -> Result<(f64, f64)> {
    let xmaxt = -LOG_DBL_MIN;
    let xmax = xmaxt - libm::log(xmaxt);

    if x < -xmax && !scale {
        return Err(PetirError::ZeroDivide); // OVERFLOW_ERROR
    }
    if x <= -10.0 {
        let s = 1.0 / x * if scale { 1.0 } else { libm::exp(-x) };
        let (c, ce) = cheb_eval(&AE11_DATA, 38, -1.0, 1.0, 20.0 / x + 1.0);
        let val = s * (1.0 + c);
        let err = s * ce + 2.0 * EPS * (libm::fabs(x) + 1.0) * libm::fabs(val);
        Ok((val, err))
    } else if x <= -4.0 {
        let s = 1.0 / x * if scale { 1.0 } else { libm::exp(-x) };
        let (c, ce) = cheb_eval(&AE12_DATA, 24, -1.0, 1.0, (40.0 / x + 7.0) / 3.0);
        let val = s * (1.0 + c);
        Ok((val, s * ce + 2.0 * EPS * libm::fabs(val)))
    } else if x <= -1.0 {
        let ln_term = -libm::log(libm::fabs(x));
        let sf = if scale { libm::exp(x) } else { 1.0 };
        let (c, ce) = cheb_eval(&E11_DATA, 18, -1.0, 1.0, (2.0 * x + 5.0) / 3.0);
        let val = sf * (ln_term + c);
        let err = sf * (ce + EPS * libm::fabs(ln_term)) + 2.0 * EPS * libm::fabs(val);
        Ok((val, err))
    } else if x == 0.0 {
        Err(PetirError::Domain)
    } else if x <= 1.0 {
        let ln_term = -libm::log(libm::fabs(x));
        let sf = if scale { libm::exp(x) } else { 1.0 };
        let (c, ce) = cheb_eval(&E12_DATA, 15, -1.0, 1.0, x);
        // The 0.6875 is upstream's (`:337`); it folds the series' own offset.
        let val = sf * (ln_term - 0.6875 + x + c);
        let err = sf * (ce + EPS * libm::fabs(ln_term)) + 2.0 * EPS * libm::fabs(val);
        Ok((val, err))
    } else if x <= 4.0 {
        let s = 1.0 / x * if scale { 1.0 } else { libm::exp(-x) };
        let (c, ce) = cheb_eval(&AE13_DATA, 24, -1.0, 1.0, (8.0 / x - 5.0) / 3.0);
        let val = s * (1.0 + c);
        Ok((val, s * ce + 2.0 * EPS * libm::fabs(val)))
    } else if x <= xmax || scale {
        let s = 1.0 / x * if scale { 1.0 } else { libm::exp(-x) };
        let (c, ce) = cheb_eval(&AE14_DATA, 25, -1.0, 1.0, 8.0 / x - 1.0);
        let val = s * (1.0 + c);
        let err = s * (EPS + ce) + 2.0 * (x + 1.0) * EPS * libm::fabs(val);
        if val == 0.0 {
            Err(PetirError::Tolerance) // UNDERFLOW_ERROR
        } else {
            Ok((val, err))
        }
    } else {
        Err(PetirError::Tolerance) // UNDERFLOW_ERROR
    }
}
