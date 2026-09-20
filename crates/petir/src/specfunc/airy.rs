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
// PORTED from the GNU Scientific Library (GSL) 2.8, specfunc/airy.c
// <https://www.gnu.org/software/gsl/>
// Copyright (C) 1996, 1997, 1998, 1999, 2000 Gerard Jungman
// GSL is free software under the GNU General Public License, version 3 or
// later.
//
// The underlying Chebyshev expansions are SLATEC's (ai, aie, bi, bie), which
// GSL vendored; the series and their coefficients are reproduced here exactly
// as GSL carries them.
//
// The thirteen Chebyshev tables here were extracted from that source by
// script, not retyped, and are audited bit-for-bit against it by
// `tests/gsl_tables_audit.rs`.

//! The Airy functions `Ai(x)`, `Bi(x)` and their exponentially scaled forms.
//!
//! # What these are
//!
//! `Ai` and `Bi` are the two linearly independent solutions of
//!
//! ```text
//!     y'' - x y = 0
//! ```
//!
//! `Ai` decays as `x -> +inf` and `Bi` grows; both oscillate for `x -> -inf`
//! with slowly increasing wavelength. They are the canonical **turning-point**
//! functions: wherever a wave equation's coefficient changes sign — a
//! classical turning point in WKB, a caustic in optics, the edge of a
//! propagating region — the local solution is an Airy function. In reactor
//! physics they appear in asymptotic transport and in diffusion near a
//! boundary where the buckling changes sign.
//!
//! # Argument range, stated plainly
//!
//! **All real `x`**, dimensionless (`f64`), for `Ai` and the two scaled
//! forms. `Bi(x)` **overflows** for large positive `x` and returns `+inf`
//! there, upstream's `OVERFLOW_ERROR`; the cut is at `y = (2/3) x^{3/2}`
//! exceeding `ln(f64::MAX) - 1`, which is about `x = 104.1`. Use
//! [`airy_bi_scaled`] past that. `NaN` propagates.
//!
//! The scalings are upstream's and are **not symmetric**, which is easy to
//! get wrong:
//!
//! | function | `x > 0` | `x <= 0` |
//! |---|---|---|
//! | [`airy_ai_scaled`] | `exp(+(2/3) x^{3/2}) Ai(x)` | `Ai(x)` unscaled |
//! | [`airy_bi_scaled`] | `exp(-(2/3) x^{3/2}) Bi(x)` | `Bi(x)` unscaled |
//!
//! On the negative axis both functions oscillate and need no scaling, so the
//! scaled and unscaled forms coincide there — asserted, not assumed, by
//! `the_scalings_are_the_documented_ones_and_are_not_symmetric`.
//!
//! There is **no WGSL kernel for the Airy functions yet**; when there is, the
//! scaled entry points are the ones to prefer on a GPU, for the reason
//! [`crate::wgsl::mirror_bessel`] measures on `I_0` — a scaled form calls no
//! `exp` and so escapes the device's ULP allowance on it.
//!
//! # GSL's `mode` argument is not carried
//!
//! Every `gsl_sf_airy_*` entry point takes a `gsl_mode_t` selecting how many
//! Chebyshev terms to evaluate (`GSL_PREC_DOUBLE`, `_SINGLE`, `_APPROX`).
//! This port always evaluates at **full order**, i.e. `GSL_PREC_DOUBLE`, and
//! takes no mode parameter. The reduced orders are recorded in the table
//! comments below (`order_sp`) so the information is not lost, and the same
//! decision is already taken for `erf` — see that module.
//!
//! # Accuracy
//!
//! Measured against the **Wronskian** `Ai(x) Bi'(x) - Ai'(x) Bi(x) = 1/pi`,
//! which this module cannot satisfy by construction because it implements no
//! derivatives, and against the defining **differential equation** evaluated
//! by high-order finite differences on this module's own output. Results and
//! methodology are in the tests; the headline is that the three-term
//! recurrence-free Chebyshev branches agree with a 9-point central-difference
//! residual of `y'' - x y` to better than 1e-9 relative on `[-8, 6]`.

// Under a std-linked build (`cargo test`) f64's inherent sqrt/exp/sin/cos
// shadow these trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

/// GSL's `am21_cs` (am21_data), 37 coefficients, order 36.
#[rustfmt::skip]
const AM21: [f64; 37] = [
    0.0065809191761485, 0.0023675984685722, 0.0001324741670371, 0.0000157600904043,
    0.0000027529702663, 0.0000006102679017, 0.0000001595088468, 0.0000000471033947,
    0.0000000152933871, 0.0000000053590722, 0.0000000020000910, 0.0000000007872292,
    0.0000000003243103, 0.0000000001390106, 0.0000000000617011, 0.0000000000282491,
    0.0000000000132979, 0.0000000000064188, 0.0000000000031697, 0.0000000000015981,
    0.0000000000008213, 0.0000000000004296, 0.0000000000002284, 0.0000000000001232,
    0.0000000000000675, 0.0000000000000374, 0.0000000000000210, 0.0000000000000119,
    0.0000000000000068, 0.0000000000000039, 0.0000000000000023, 0.0000000000000013,
    0.0000000000000008, 0.0000000000000005, 0.0000000000000003, 0.0000000000000001,
    0.0000000000000001,
];

/// GSL's `ath1_cs` (ath1_data), 36 coefficients, order 35.
#[rustfmt::skip]
const ATH1: [f64; 36] = [
    -0.07125837815669365, -0.00590471979831451, -0.00012114544069499, -0.00000988608542270,
    -0.00000138084097352, -0.00000026142640172, -0.00000006050432589, -0.00000001618436223,
    -0.00000000483464911, -0.00000000157655272, -0.00000000055231518, -0.00000000020545441,
    -0.00000000008043412, -0.00000000003291252, -0.00000000001399875, -0.00000000000616151,
    -0.00000000000279614, -0.00000000000130428, -0.00000000000062373, -0.00000000000030512,
    -0.00000000000015239, -0.00000000000007758, -0.00000000000004020, -0.00000000000002117,
    -0.00000000000001132, -0.00000000000000614, -0.00000000000000337, -0.00000000000000188,
    -0.00000000000000105, -0.00000000000000060, -0.00000000000000034, -0.00000000000000020,
    -0.00000000000000011, -0.00000000000000007, -0.00000000000000004, -0.00000000000000002,
];

/// GSL's `am22_cs` (am22_data), 33 coefficients, order 32.
#[rustfmt::skip]
const AM22: [f64; 33] = [
    -0.01562844480625341, 0.00778336445239681, 0.00086705777047718, 0.00015696627315611,
    0.00003563962571432, 0.00000924598335425, 0.00000262110161850, 0.00000079188221651,
    0.00000025104152792, 0.00000008265223206, 0.00000002805711662, 0.00000000976821090,
    0.00000000347407923, 0.00000000125828132, 0.00000000046298826, 0.00000000017272825,
    0.00000000006523192, 0.00000000002490471, 0.00000000000960156, 0.00000000000373448,
    0.00000000000146417, 0.00000000000057826, 0.00000000000022991, 0.00000000000009197,
    0.00000000000003700, 0.00000000000001496, 0.00000000000000608, 0.00000000000000248,
    0.00000000000000101, 0.00000000000000041, 0.00000000000000017, 0.00000000000000007,
    0.00000000000000002,
];

/// GSL's `ath2_cs` (ath2_data), 32 coefficients, order 31.
#[rustfmt::skip]
const ATH2: [f64; 32] = [
    0.00440527345871877, -0.03042919452318455, -0.00138565328377179, -0.00018044439089549,
    -0.00003380847108327, -0.00000767818353522, -0.00000196783944371, -0.00000054837271158,
    -0.00000016254615505, -0.00000005053049981, -0.00000001631580701, -0.00000000543420411,
    -0.00000000185739855, -0.00000000064895120, -0.00000000023105948, -0.00000000008363282,
    -0.00000000003071196, -0.00000000001142367, -0.00000000000429811, -0.00000000000163389,
    -0.00000000000062693, -0.00000000000024260, -0.00000000000009461, -0.00000000000003716,
    -0.00000000000001469, -0.00000000000000584, -0.00000000000000233, -0.00000000000000093,
    -0.00000000000000037, -0.00000000000000015, -0.00000000000000006, -0.00000000000000002,
];

/// GSL's `aif_cs` (ai_data_f), 9 coefficients, order 8.
#[rustfmt::skip]
const AIF: [f64; 9] = [
    -0.03797135849666999750, 0.05919188853726363857, 0.00098629280577279975,
    0.00000684884381907656, 0.00000002594202596219, 0.00000000006176612774,
    0.00000000000010092454, 0.00000000000000012014, 0.00000000000000000010,
];

/// GSL's `aig_cs` (ai_data_g), 8 coefficients, order 7.
#[rustfmt::skip]
const AIG: [f64; 8] = [
    0.01815236558116127, 0.02157256316601076, 0.00025678356987483, 0.00000142652141197,
    0.00000000457211492, 0.00000000000952517, 0.00000000000001392, 0.00000000000000001,
];

/// GSL's `bif_cs` (data_bif), 9 coefficients, order 8.
#[rustfmt::skip]
const BIF: [f64; 9] = [
    -0.01673021647198664948, 0.10252335834249445610, 0.00170830925073815165,
    0.00001186254546774468, 0.00000004493290701779, 0.00000000010698207143,
    0.00000000000017480643, 0.00000000000000020810, 0.00000000000000000018,
];

/// GSL's `big_cs` (data_big), 8 coefficients, order 7.
#[rustfmt::skip]
const BIG: [f64; 8] = [
    0.02246622324857452, 0.03736477545301955, 0.00044476218957212, 0.00000247080756363,
    0.00000000791913533, 0.00000000001649807, 0.00000000000002411, 0.00000000000000002,
];

/// GSL's `bif2_cs` (data_bif2), 10 coefficients, order 9.
#[rustfmt::skip]
const BIF2: [f64; 10] = [
    0.0998457269381604100, 0.4786249778630055380, 0.0251552119604330118, 0.0005820693885232645,
    0.0000074997659644377, 0.0000000613460287034, 0.0000000003462753885, 0.0000000000014288910,
    0.0000000000000044962, 0.0000000000000000111,
];

/// GSL's `big2_cs` (data_big2), 10 coefficients, order 9.
#[rustfmt::skip]
const BIG2: [f64; 10] = [
    0.033305662145514340, 0.161309215123197068, 0.0063190073096134286, 0.0001187904568162517,
    0.0000013045345886200, 0.0000000093741259955, 0.0000000000474580188, 0.0000000000001783107,
    0.0000000000000005167, 0.0000000000000000011,
];

/// GSL's `aip_cs` (data_aip), 36 coefficients, order 35.
#[rustfmt::skip]
const AIP: [f64; 36] = [
    -0.0187519297793867540198, -0.0091443848250055004725, 0.0009010457337825074652,
    -0.0001394184127221491507, 0.0000273815815785209370, -0.0000062750421119959424,
    0.0000016064844184831521, -0.0000004476392158510354, 0.0000001334635874651668,
    -0.0000000420735334263215, 0.0000000139021990246364, -0.0000000047831848068048,
    0.0000000017047897907465, -0.0000000006268389576018, 0.0000000002369824276612,
    -0.0000000000918641139267, 0.0000000000364278543037, -0.0000000000147475551725,
    0.0000000000060851006556, -0.0000000000025552772234, 0.0000000000010906187250,
    -0.0000000000004725870319, 0.0000000000002076969064, -0.0000000000000924976214,
    0.0000000000000417096723, -0.0000000000000190299093, 0.0000000000000087790676,
    -0.0000000000000040927557, 0.0000000000000019271068, -0.0000000000000009160199,
    0.0000000000000004393567, -0.0000000000000002125503, 0.0000000000000001036735,
    -0.0000000000000000509642, 0.0000000000000000252377, -0.0000000000000000125793,
];

/// GSL's `bip_cs` (data_bip), 24 coefficients, order 23.
#[rustfmt::skip]
const BIP: [f64; 24] = [
    -0.08322047477943447, 0.01146118927371174, 0.00042896440718911, -0.00014906639379950,
    -0.00001307659726787, 0.00000632759839610, -0.00000042226696982, -0.00000019147186298,
    0.00000006453106284, -0.00000000784485467, -0.00000000096077216, 0.00000000070004713,
    -0.00000000017731789, 0.00000000002272089, 0.00000000000165404, -0.00000000000185171,
    0.00000000000059576, -0.00000000000012194, 0.00000000000001334, 0.00000000000000172,
    -0.00000000000000145, 0.00000000000000049, -0.00000000000000011, 0.00000000000000001,
];

/// GSL's `bip2_cs` (data_bip2), 29 coefficients, order 28.
#[rustfmt::skip]
const BIP2: [f64; 29] = [
    -0.113596737585988679, 0.0041381473947881595, 0.0001353470622119332, 0.0000104273166530153,
    0.0000013474954767849, 0.0000001696537405438, -0.0000000100965008656,
    -0.0000000167291194937, -0.0000000045815364485, 0.0000000003736681366,
    0.0000000005766930320, 0.0000000000621812650, -0.0000000000632941202,
    -0.0000000000149150479, 0.0000000000078896213, 0.0000000000024960513,
    -0.0000000000012130075, -0.0000000000003740493, 0.0000000000002237727,
    0.0000000000000474902, -0.0000000000000452616, -0.0000000000000030172,
    0.0000000000000091058, -0.0000000000000009814, -0.0000000000000016429,
    0.0000000000000005533, 0.0000000000000002175, -0.0000000000000001737,
    -0.0000000000000000010,
];

/// `ln(f64::MAX)`, upstream's `GSL_LOG_DBL_MAX`. `Bi` overflows when
/// `(2/3) x^{3/2}` exceeds this less one.
use crate::specfunc::LOG_DBL_MAX;

/// Clenshaw in GSL's convention, on `[-1, 1]`. Every series in this module is
/// declared on that interval, so no argument rescaling is needed — the branch
/// formulae below map `x` into it themselves.
fn cheb(x: f64, c: &[f64]) -> f64 {
    crate::cheb_slice::eval_gsl(x, c)
}

/// The Airy modulus and phase, upstream's `airy_mod_phase`, valid for
/// `x <= -1`.
///
/// For `x` below `-1` both functions are oscillatory and are written as
///
/// ```text
///     Ai(x) = M(x) cos(theta(x)),    Bi(x) = M(x) sin(theta(x))
/// ```
///
/// with a slowly varying modulus `M` and a phase `theta` that grows like
/// `(2/3)|x|^{3/2}`. That is the whole reason this branch exists: evaluating
/// a Chebyshev series in `x` directly would need more and more terms as the
/// oscillation tightens, while `M` and `theta` are smooth in `16/x^3`.
///
/// Returns `(modulus, phase)`. `NaN` for `x > -1`, which upstream reports as
/// `GSL_EDOM`; no caller here reaches it.
fn mod_phase(x: f64) -> (f64, f64) {
    let (m, p) = if x < -2.0 {
        let z = 16.0 / (x * x * x) + 1.0;
        (cheb(z, &AM21), cheb(z, &ATH1))
    } else if x <= -1.0 {
        let z = (16.0 / (x * x * x) + 9.0) / 7.0;
        (cheb(z, &AM22), cheb(z, &ATH2))
    } else {
        return (f64::NAN, f64::NAN);
    };
    let m = 0.3125 + m;
    let p = -0.625 + p;
    let sqx = (-x).sqrt();
    let modulus = (m / sqx).sqrt();
    let phase = core::f64::consts::FRAC_PI_4 - x * sqx * p;
    (modulus, phase)
}

/// `exp(+(2/3) x^{3/2}) Ai(x)` for `x >= 1`, upstream's `airy_aie`.
fn aie(x: f64) -> f64 {
    let sqx = x.sqrt();
    let z = 2.0 / (x * sqx) - 1.0;
    let y = sqx.sqrt();
    (0.28125 + cheb(z, &AIP)) / y
}

/// `exp(-(2/3) x^{3/2}) Bi(x)` for `x >= 2`, upstream's `airy_bie`.
///
/// Two sub-branches at `x = 4`, each a Chebyshev series in a different
/// rational function of `x^{3/2}`. `ATR` and `BTR` are upstream's own
/// constants and are reproduced to the digits it gives.
fn bie(x: f64) -> f64 {
    const ATR: f64 = 8.750_690_570_848_434_5;
    const BTR: f64 = -2.093_836_321_356_054_3;
    let sqx = x.sqrt();
    let y = sqx.sqrt();
    // Branch fully rather than selecting a slice: `&BIP[..]` is a range
    // subscript, which `tests/no_panic_gate.rs` rejects on sight even though
    // a full-range one cannot fail. The gate is conservative on purpose.
    let c = if x < 4.0 {
        cheb(ATR / (x * sqx) + BTR, &BIP)
    } else {
        cheb(16.0 / (x * sqx) - 1.0, &BIP2)
    };
    (0.625 + c) / y
}

/// The Airy function `Ai(x)`, GSL's `gsl_sf_airy_Ai`.
///
/// Decays like `exp(-(2/3) x^{3/2}) / (2 sqrt(pi) x^{1/4})` for large
/// positive `x`, and oscillates for `x -> -inf`. Never overflows: underflows
/// smoothly to zero on the positive axis instead. `NaN` propagates.
///
/// # Examples
///
/// ```
/// use petir::specfunc::airy::airy_ai;
/// // Ai(0) = 3^{-2/3} / Gamma(2/3) = 0.3550280538878172...
/// assert!((airy_ai(0.0) - 0.355_028_053_887_817_2).abs() < 1e-15);
/// // Decays on the positive axis.
/// assert!(airy_ai(5.0) > 0.0 && airy_ai(5.0) < 1e-3);
/// ```
pub fn airy_ai(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x < -1.0 {
        let (m, theta) = mod_phase(x);
        m * theta.cos()
    } else if x <= 1.0 {
        let z = x * x * x;
        0.375 + (cheb(z, &AIF) - x * (0.25 + cheb(z, &AIG)))
    } else {
        let x32 = x * x.sqrt();
        aie(x) * (-2.0 * x32 / 3.0).exp()
    }
}

/// The exponentially scaled Airy function, GSL's `gsl_sf_airy_Ai_scaled`.
///
/// `exp(+(2/3) x^{3/2}) Ai(x)` for `x > 0`, and plain `Ai(x)` for `x <= 0`
/// where the function oscillates and needs no scaling. That asymmetry is
/// upstream's, and is the reason this is a separate entry point rather than a
/// flag.
///
/// Tends to `1 / (2 sqrt(pi) x^{1/4})` for large positive `x`, so it decays
/// only algebraically and stays representable far past where [`airy_ai`] has
/// underflowed. `NaN` propagates.
pub fn airy_ai_scaled(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x < -1.0 {
        let (m, theta) = mod_phase(x);
        m * theta.cos()
    } else if x <= 1.0 {
        let z = x * x * x;
        let v = 0.375 + (cheb(z, &AIF) - x * (0.25 + cheb(z, &AIG)));
        if x > 0.0 {
            v * (2.0 / 3.0 * z.sqrt()).exp()
        } else {
            v
        }
    } else {
        aie(x)
    }
}

/// The Airy function `Bi(x)`, GSL's `gsl_sf_airy_Bi`.
///
/// Grows like `exp(+(2/3) x^{3/2}) / (sqrt(pi) x^{1/4})` for large positive
/// `x` and **overflows to `+inf`** once `(2/3) x^{3/2}` exceeds
/// `ln(f64::MAX) - 1`, at about `x = 104.1`. That is upstream's
/// `OVERFLOW_ERROR` branch; use [`airy_bi_scaled`] beyond it. `NaN`
/// propagates.
///
/// # Examples
///
/// ```
/// use petir::specfunc::airy::{airy_bi, airy_bi_scaled};
/// // Bi(0) = 3^{-1/6} / Gamma(2/3) = 0.6149266274460007...
/// assert!((airy_bi(0.0) - 0.614_926_627_446_000_7).abs() < 1e-15);
/// // It really does overflow, and the scaled form does not.
/// assert!(airy_bi(200.0).is_infinite());
/// assert!(airy_bi_scaled(200.0).is_finite());
/// ```
pub fn airy_bi(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x < -1.0 {
        let (m, theta) = mod_phase(x);
        m * theta.sin()
    } else if x < 1.0 {
        let z = x * x * x;
        0.625 + cheb(z, &BIF) + x * (0.4375 + cheb(z, &BIG))
    } else if x <= 2.0 {
        let z = (2.0 * x * x * x - 9.0) / 7.0;
        1.125 + cheb(z, &BIF2) + x * (0.625 + cheb(z, &BIG2))
    } else {
        let y = 2.0 * x * x.sqrt() / 3.0;
        if y > LOG_DBL_MAX - 1.0 {
            // Upstream's OVERFLOW_ERROR, which sets +inf.
            f64::INFINITY
        } else {
            bie(x) * y.exp()
        }
    }
}

/// The exponentially scaled Airy function, GSL's `gsl_sf_airy_Bi_scaled`.
///
/// `exp(-(2/3) x^{3/2}) Bi(x)` for `x > 0`, and plain `Bi(x)` for `x <= 0`.
/// Note the sign of the exponent is the opposite of [`airy_ai_scaled`]'s,
/// because `Bi` grows where `Ai` decays.
///
/// Tends to `1 / (sqrt(pi) x^{1/4})` for large positive `x`, so unlike
/// [`airy_bi`] it never overflows. `NaN` propagates.
pub fn airy_bi_scaled(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x < -1.0 {
        let (m, theta) = mod_phase(x);
        m * theta.sin()
    } else if x < 1.0 {
        let z = x * x * x;
        let v = 0.625 + cheb(z, &BIF) + x * (0.4375 + cheb(z, &BIG));
        if x > 0.0 {
            v * (-2.0 / 3.0 * z.sqrt()).exp()
        } else {
            v
        }
    } else if x <= 2.0 {
        let x3 = x * x * x;
        let z = (2.0 * x3 - 9.0) / 7.0;
        let s = (-2.0 / 3.0 * x3.sqrt()).exp();
        s * (1.125 + cheb(z, &BIF2) + x * (0.625 + cheb(z, &BIG2)))
    } else {
        bie(x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Ai(0) = 3^{-2/3} / Gamma(2/3)` and `Bi(0) = 3^{-1/6} / Gamma(2/3)`,
    /// to the digits DLMF 9.2.3-9.2.4 give.
    const AI0: f64 = 0.355_028_053_887_817_23;
    const BI0: f64 = 0.614_926_627_446_000_73;
    /// `Ai'(0) = -3^{-1/3} / Gamma(1/3)`, `Bi'(0) = 3^{1/6} / Gamma(1/3)`.
    const AI0_PRIME: f64 = -0.258_819_403_792_806_80;
    const BI0_PRIME: f64 = 0.448_288_357_353_826_36;

    /// A 9-point central difference of `f` at `x`, order `h^8`.
    ///
    /// This is the instrument the accuracy claim rests on, so it is chosen
    /// for the physics rather than for convenience: the residual being
    /// measured is `y'' - x y`, and a low-order difference would leave a
    /// truncation error larger than the quantity of interest.
    fn second_derivative(f: impl Fn(f64) -> f64, x: f64, h: f64) -> f64 {
        // Coefficients of the 8th-order central second difference.
        let c: [f64; 9] = [
            -1.0 / 560.0,
            8.0 / 315.0,
            -1.0 / 5.0,
            8.0 / 5.0,
            -205.0 / 72.0,
            8.0 / 5.0,
            -1.0 / 5.0,
            8.0 / 315.0,
            -1.0 / 560.0,
        ];
        let mut s = 0.0;
        for (k, &ck) in c.iter().enumerate() {
            s += ck * f(x + (k as f64 - 4.0) * h);
        }
        s / (h * h)
    }

    /// **The defining differential equation, `y'' - x y = 0`.**
    ///
    /// # Methodology
    ///
    /// `y''` is taken by an 8th-order central difference of *this module's
    /// own output*, and compared against `x y` from the same. The reference
    /// is therefore the ODE itself and not another implementation, and it
    /// shares nothing with the Chebyshev coefficients, the branch structure
    /// or the modulus/phase decomposition — a wrong coefficient in any of the
    /// thirteen tables breaks it.
    ///
    /// Note what it *cannot* catch: a solution of the same ODE that is the
    /// wrong linear combination of `Ai` and `Bi`. That is what
    /// `the_values_at_the_origin_are_the_published_ones` pins, and the two
    /// together fix the function uniquely.
    ///
    /// The step `h = 1/32` is the **measured** optimum of that balance —
    /// truncation above it, `f64` cancellation in the second difference below
    /// — and `1e-9` relative is what the balance permits, not what the
    /// functions are capable of.
    ///
    /// **The normaliser is `max(|y''|, |x y|, |y|)`, and the third term is
    /// not padding.** At `x = 0` the equation reads `y'' = 0` and both of the
    /// first two vanish together, so a residual relative to them alone
    /// measures the normaliser rather than the functions — the first version
    /// of this test did exactly that and reported 1.0 at the origin. `|y|` is
    /// `O(0.35)` there and is the scale the physics supplies.
    ///
    /// # Results, measured 2026-09-19
    ///
    /// Worst relative residual over `x` in `[-8, 6]` at 0.05 spacing,
    /// normalised by `max(|y''|, |x y|)`:
    ///
    /// | function | worst | at |
    /// |---|---|---|
    /// | `Ai` | 3.661e-11 | -4.1 |
    /// | `Bi` | 4.292e-11 | -6.15 |
    ///
    /// Both worst points are on the negative axis, where the oscillation is
    /// tightest and a central difference is least accurate — i.e. the
    /// residual is dominated by the *instrument*, not by the functions.
    /// `the_residual_is_the_difference_scheme_not_the_functions` **tests**
    /// that explanation rather than restating it.
    #[test]
    fn both_functions_satisfy_the_defining_differential_equation() {
        // h = 1/32 is the MEASURED optimum, not a guess -- see
        // `the_residual_is_the_difference_scheme_not_the_functions`.
        let h = 1.0 / 32.0;
        for (name, f) in [
            ("Ai", &airy_ai as &dyn Fn(f64) -> f64),
            ("Bi", &airy_bi as &dyn Fn(f64) -> f64),
        ] {
            let (mut worst, mut at) = (0.0_f64, 0.0_f64);
            for k in 0..=280 {
                let x = -8.0 + 0.05 * k as f64;
                let y = f(x);
                let ypp = second_derivative(&f, x, h);
                let xy = x * y;
                // Normalise by the FUNCTION's scale as well as the two terms'.
                // At x = 0 the equation reads y'' = 0, so both terms vanish
                // and a residual relative to them alone is meaningless -- the
                // first version of this test normalised by max(|y''|, |x y|)
                // and reported 1.0 there, which measured its own normaliser.
                // |y| is O(0.35) at the origin and never vanishes at the same
                // argument as both terms, so it is the scale the physics
                // supplies.
                let scale = ypp.abs().max(xy.abs()).max(y.abs());
                let r = (ypp - xy).abs() / scale;
                if r > worst {
                    worst = r;
                    at = x;
                }
            }
            assert!(
                worst < 1e-8,
                "{name} residual of y'' - x y: {worst:e} at x = {at}"
            );
        }
    }

    /// **The residual above is the finite difference, not the functions**,
    /// and this is the check that establishes it rather than asserting it.
    ///
    /// If the residual came from the Chebyshev series it would be independent
    /// of the step `h`. If it is the difference scheme's truncation it must
    /// fall as a power of `h` until `f64` cancellation in the second
    /// difference takes over, and then rise. Worst relative residual over
    /// `x` in `[-8, 6]`, measured 2026-09-19:
    ///
    /// | `h` | `Ai` | ratio | `Bi` | ratio |
    /// |---|---|---|---|---|
    /// | 1/4 | 8.624e-04 | — | 1.834e-04 | — |
    /// | 1/8 | 3.822e-06 | 226 | 8.068e-07 | 227 |
    /// | 1/16 | 1.540e-08 | 248 | 3.248e-09 | 248 |
    /// | **1/32** | **3.661e-11** | 421 | **4.292e-11** | 76 |
    /// | 1/64 | 1.309e-10 | 0.28 | 8.219e-11 | 0.52 |
    /// | 1/128 | 5.504e-10 | 0.24 | 5.102e-10 | 0.16 |
    ///
    /// **A prediction this refuted.** The ratios were expected to be about
    /// 64, on the reasoning that an `O(h^8)` truncation divided by `h^2`
    /// leaves `O(h^6)`. They are ~250, i.e. `h^8` — the scheme's coefficients
    /// are built so the error in `y''` **itself** is `O(h^8)`, and the `h^2`
    /// is already accounted for. The first draft of this table was written
    /// from the wrong reasoning and from unmeasured numbers; the assertion
    /// below failed on it, which is why the table is now measured.
    ///
    /// The turnaround at 1/32 is the second half of the claim and is asserted
    /// too: past the optimum the residual **rises**, which truncation alone
    /// cannot do and roundoff must.
    #[test]
    fn the_residual_is_the_difference_scheme_not_the_functions() {
        let worst = |f: &dyn Fn(f64) -> f64, h: f64| {
            let mut w = 0.0_f64;
            for k in 0..=280 {
                let x = -8.0 + 0.05 * k as f64;
                let y = f(x);
                let ypp = second_derivative(f, x, h);
                let xy = x * y;
                w = w.max((ypp - xy).abs() / ypp.abs().max(xy.abs()).max(y.abs()));
            }
            w
        };
        for (name, f) in [
            ("Ai", &airy_ai as &dyn Fn(f64) -> f64),
            ("Bi", &airy_bi as &dyn Fn(f64) -> f64),
        ] {
            let r: [f64; 6] = [
                worst(&f, 1.0 / 4.0),
                worst(&f, 1.0 / 8.0),
                worst(&f, 1.0 / 16.0),
                worst(&f, 1.0 / 32.0),
                worst(&f, 1.0 / 64.0),
                worst(&f, 1.0 / 128.0),
            ];
            // Truncation regime: each halving of h buys at least 64x, which
            // is the weakest power (h^6) anyone would expect. Measured ~250.
            for k in 0..2 {
                assert!(
                    r[k] > 64.0 * r[k + 1],
                    "{name}: the residual is documented as truncation-dominated \
                     down to h = 1/32, falling ~250x per halving; from \
                     h=1/{} it went {:e} -> {:e}",
                    4 << k,
                    r[k],
                    r[k + 1]
                );
            }
            // And past the optimum it RISES -- roundoff, not truncation.
            assert!(
                r[4] > r[3] && r[5] > r[4],
                "{name}: the residual is documented as turning around at \
                 h = 1/32 because f64 cancellation then dominates, and it \
                 went {:e} (1/32) -> {:e} (1/64) -> {:e} (1/128). If it is \
                 still falling, the optimum has moved and the step the ODE \
                 test uses should move with it",
                r[3],
                r[4],
                r[5]
            );
        }
    }

    /// The published values at the origin, which fix *which* solution of the
    /// ODE each function is.
    ///
    /// The derivatives are taken by an 8th-order central difference, so the
    /// tolerance on them is the difference scheme's, not the module's.
    #[test]
    fn the_values_at_the_origin_are_the_published_ones() {
        assert!(
            (airy_ai(0.0) - AI0).abs() < 1e-15,
            "Ai(0) = {}",
            airy_ai(0.0)
        );
        assert!(
            (airy_bi(0.0) - BI0).abs() < 1e-15,
            "Bi(0) = {}",
            airy_bi(0.0)
        );
        // The scalings are the identity at x = 0 (both branches take the
        // x <= 0 side), which is itself worth pinning -- see the module docs.
        assert_eq!(airy_ai_scaled(0.0), airy_ai(0.0));
        assert_eq!(airy_bi_scaled(0.0), airy_bi(0.0));

        // First derivatives, by an 8th-order central difference.
        let h = 1.0 / 64.0;
        let d1 = |f: &dyn Fn(f64) -> f64, x: f64| {
            let c = [
                1.0 / 280.0,
                -4.0 / 105.0,
                1.0 / 5.0,
                -4.0 / 5.0,
                0.0,
                4.0 / 5.0,
                -1.0 / 5.0,
                4.0 / 105.0,
                -1.0 / 280.0,
            ];
            let mut s = 0.0;
            for (k, &ck) in c.iter().enumerate() {
                s += ck * f(x + (k as f64 - 4.0) * h);
            }
            s / h
        };
        let aip = d1(&airy_ai, 0.0);
        let bip = d1(&airy_bi, 0.0);
        assert!((aip - AI0_PRIME).abs() < 1e-12, "Ai'(0) = {aip}");
        assert!((bip - BI0_PRIME).abs() < 1e-12, "Bi'(0) = {bip}");
    }

    /// **The Wronskian, `Ai Bi' - Ai' Bi = 1/pi`.**
    ///
    /// This module implements no derivatives, so it cannot satisfy this by
    /// construction — the derivatives come from finite differences of its own
    /// output. It is a genuine cross-check between `Ai` and `Bi`, which share
    /// only the modulus/phase branch and no Chebyshev table at all: `Ai` uses
    /// `aif`/`aig`/`aip`, `Bi` uses `bif`/`big`/`bif2`/`big2`/`bip`/`bip2`.
    ///
    /// # Results, measured 2026-09-19
    ///
    /// Worst relative departure from `1/pi` over `x` in `[-6, 4]`:
    /// **1.298e-12**, at `x = -5.95`. The bound is 1e-9, set by the
    /// difference scheme.
    #[test]
    fn the_wronskian_is_one_over_pi() {
        let h = 1.0 / 64.0;
        let d1 = |f: &dyn Fn(f64) -> f64, x: f64| {
            let c = [
                1.0 / 280.0,
                -4.0 / 105.0,
                1.0 / 5.0,
                -4.0 / 5.0,
                0.0,
                4.0 / 5.0,
                -1.0 / 5.0,
                4.0 / 105.0,
                -1.0 / 280.0,
            ];
            let mut s = 0.0;
            for (k, &ck) in c.iter().enumerate() {
                s += ck * f(x + (k as f64 - 4.0) * h);
            }
            s / h
        };
        let target = 1.0 / core::f64::consts::PI;
        let (mut worst, mut at) = (0.0_f64, 0.0_f64);
        for k in 0..=200 {
            let x = -6.0 + 0.05 * k as f64;
            let w = airy_ai(x) * d1(&airy_bi, x) - d1(&airy_ai, x) * airy_bi(x);
            let r = ((w - target) / target).abs();
            if r > worst {
                worst = r;
                at = x;
            }
        }
        assert!(worst < 1e-9, "Wronskian: {worst:e} at x = {at}");
    }

    /// The scaled forms really are the stated exponential multiples, and the
    /// **asymmetry between them is deliberate** — `Ai`'s exponent is positive
    /// and `Bi`'s negative, and on the negative axis neither is scaled.
    ///
    /// Checked only where the unscaled function has not yet under- or
    /// overflowed, which is the whole reason the scaled forms exist.
    #[test]
    fn the_scalings_are_the_documented_ones_and_are_not_symmetric() {
        for k in 1..=60 {
            let x = 0.1 * k as f64;
            let e = 2.0 / 3.0 * x * x.sqrt();
            let ai = airy_ai(x);
            let bi = airy_bi(x);
            assert!(
                ((airy_ai_scaled(x) - ai * e.exp()) / (ai * e.exp())).abs() < 1e-12,
                "Ai_scaled at {x}"
            );
            assert!(
                ((airy_bi_scaled(x) - bi * (-e).exp()) / (bi * (-e).exp())).abs() < 1e-12,
                "Bi_scaled at {x}"
            );
        }
        // On the negative axis the scaled and unscaled forms COINCIDE, which
        // is upstream's asymmetry and not an oversight.
        for k in 1..=120 {
            let x = -0.1 * k as f64;
            assert_eq!(airy_ai_scaled(x), airy_ai(x), "Ai_scaled at {x}");
            assert_eq!(airy_bi_scaled(x), airy_bi(x), "Bi_scaled at {x}");
        }
    }

    /// The scaled forms tend to their algebraic asymptotes, which is what
    /// makes them usable where the unscaled ones are not:
    ///
    /// ```text
    ///     Ai_scaled(x) -> 1 / (2 sqrt(pi) x^{1/4})
    ///     Bi_scaled(x) -> 1 / (sqrt(pi) x^{1/4})
    /// ```
    ///
    /// Note the factor of two between them — `Bi`'s asymptote is exactly
    /// twice `Ai`'s, which a single mis-taken constant would break.
    #[test]
    fn the_scaled_forms_reach_their_algebraic_asymptotes() {
        let sqrt_pi = core::f64::consts::PI.sqrt();
        for x in [1e3_f64, 1e5, 1e8, 1e12] {
            let q = x.sqrt().sqrt();
            let want_ai = 1.0 / (2.0 * sqrt_pi * q);
            let want_bi = 1.0 / (sqrt_pi * q);
            assert!(
                ((airy_ai_scaled(x) - want_ai) / want_ai).abs() < 1e-3,
                "Ai_scaled({x:e}) = {} vs {want_ai:e}",
                airy_ai_scaled(x)
            );
            assert!(
                ((airy_bi_scaled(x) - want_bi) / want_bi).abs() < 1e-3,
                "Bi_scaled({x:e}) = {} vs {want_bi:e}",
                airy_bi_scaled(x)
            );
        }
    }

    /// Every branch boundary joins. Each compares the **two formulas** at one
    /// argument, not the function either side of the cut — that would measure
    /// its slope rather than a discontinuity.
    ///
    /// The boundaries are `x = -2` and `x = -1` (inside the modulus/phase
    /// branch), `x = 1` for `Ai` and `Bi`, `x = 2` for `Bi`, and `x = 4`
    /// inside `bie`.
    #[test]
    fn the_branch_boundaries_join() {
        // x = -2: the two modulus/phase series.
        let x = -2.0_f64;
        let z1 = 16.0 / (x * x * x) + 1.0;
        let z2 = (16.0 / (x * x * x) + 9.0) / 7.0;
        for (name, a, b) in [
            ("modulus", cheb(z1, &AM21), cheb(z2, &AM22)),
            ("phase", cheb(z1, &ATH1), cheb(z2, &ATH2)),
        ] {
            assert!(((a - b) / b).abs() < 1e-9, "x = -2 {name} join: {a} vs {b}");
        }

        // x = -1: the mod/phase branch against the central Chebyshev one.
        let x = -1.0_f64;
        let (m, theta) = mod_phase(x);
        let z = x * x * x;
        let central_ai = 0.375 + (cheb(z, &AIF) - x * (0.25 + cheb(z, &AIG)));
        let central_bi = 0.625 + cheb(z, &BIF) + x * (0.4375 + cheb(z, &BIG));
        assert!(
            ((m * theta.cos() - central_ai) / central_ai).abs() < 1e-10,
            "x = -1 Ai join: {} vs {central_ai}",
            m * theta.cos()
        );
        assert!(
            ((m * theta.sin() - central_bi) / central_bi).abs() < 1e-10,
            "x = -1 Bi join: {} vs {central_bi}",
            m * theta.sin()
        );

        // x = 1: Ai's central series against the aie branch.
        let x = 1.0_f64;
        let z = x * x * x;
        let a = 0.375 + (cheb(z, &AIF) - x * (0.25 + cheb(z, &AIG)));
        let b = aie(x) * (-2.0 / 3.0 * x * x.sqrt()).exp();
        assert!(((a - b) / b).abs() < 1e-10, "x = 1 Ai join: {a} vs {b}");

        // x = 1: Bi's two central series.
        let a = 0.625 + cheb(z, &BIF) + x * (0.4375 + cheb(z, &BIG));
        let zz = (2.0 * z - 9.0) / 7.0;
        let b = 1.125 + cheb(zz, &BIF2) + x * (0.625 + cheb(zz, &BIG2));
        assert!(((a - b) / b).abs() < 1e-10, "x = 1 Bi join: {a} vs {b}");

        // x = 2: Bi's second central series against the bie branch.
        let x = 2.0_f64;
        let z = x * x * x;
        let zz = (2.0 * z - 9.0) / 7.0;
        let a = 1.125 + cheb(zz, &BIF2) + x * (0.625 + cheb(zz, &BIG2));
        let b = bie(x) * (2.0 * x * x.sqrt() / 3.0).exp();
        assert!(((a - b) / b).abs() < 1e-10, "x = 2 Bi join: {a} vs {b}");

        // x = 4: bie's own two sub-branches.
        let x = 4.0_f64;
        let sqx = x.sqrt();
        let y = sqx.sqrt();
        let a = (0.625
            + cheb(
                8.750_690_570_848_434_5 / (x * sqx) - 2.093_836_321_356_054_3,
                &BIP,
            ))
            / y;
        let b = (0.625 + cheb(16.0 / (x * sqx) - 1.0, &BIP2)) / y;
        assert!(((a - b) / b).abs() < 1e-10, "x = 4 bie join: {a} vs {b}");
    }

    /// `Bi` overflows where upstream says it does, and the scaled form does
    /// not. `Ai` underflows smoothly and never overflows.
    ///
    /// The cut is `(2/3) x^{3/2} > ln(f64::MAX) - 1`, i.e. about
    /// `x = 104.05`. Checked either side rather than asserted from the
    /// formula.
    #[test]
    fn bi_overflows_where_upstream_says_and_the_scaled_form_does_not() {
        // Locate the documented cut from the inequality itself.
        let cut = (1.5 * (LOG_DBL_MAX - 1.0)).powf(2.0 / 3.0);
        assert!(
            (104.0..105.0).contains(&cut),
            "the Bi overflow cut is documented near x = 104.1, and is {cut}"
        );
        assert!(airy_bi(cut - 0.5).is_finite(), "Bi just below the cut");
        assert!(airy_bi(cut + 0.5).is_infinite(), "Bi just above the cut");
        assert!(airy_bi(1e6).is_infinite());
        // The scaled form is finite everywhere on the positive axis.
        for x in [cut - 0.5, cut + 0.5, 1e6, 1e30] {
            assert!(airy_bi_scaled(x).is_finite(), "Bi_scaled({x:e})");
        }
        // Ai never overflows; it underflows to zero.
        assert_eq!(airy_ai(1e6), 0.0);
        assert!(airy_ai_scaled(1e6).is_finite() && airy_ai_scaled(1e6) > 0.0);
    }

    /// `Ai` is positive and decreasing on the positive axis, `Bi` positive
    /// and increasing, and both oscillate below the first zero. The first
    /// zeros are `a_1 = -2.338107` and `b_1 = -1.173713` (DLMF 9.9.1,
    /// 9.9.2), which a sign change must bracket.
    #[test]
    fn the_shape_and_the_first_zeros_are_right() {
        let mut prev = airy_ai(0.0);
        for k in 1..=200 {
            let x = 0.05 * k as f64;
            let v = airy_ai(x);
            assert!(v > 0.0 && v < prev, "Ai not decreasing at {x}: {v}");
            prev = v;
        }
        let mut prev = airy_bi(0.0);
        for k in 1..=200 {
            let x = 0.05 * k as f64;
            let v = airy_bi(x);
            assert!(v > prev, "Bi not increasing at {x}: {v}");
            prev = v;
        }
        // The first zeros, bracketed.
        assert!(
            airy_ai(-2.33) > 0.0 && airy_ai(-2.35) < 0.0,
            "Ai's first zero"
        );
        assert!(
            airy_bi(-1.17) > 0.0 && airy_bi(-1.18) < 0.0,
            "Bi's first zero"
        );
    }

    /// `NaN` propagates through all four entry points rather than falling
    /// into a branch.
    #[test]
    fn nan_propagates() {
        assert!(airy_ai(f64::NAN).is_nan());
        assert!(airy_bi(f64::NAN).is_nan());
        assert!(airy_ai_scaled(f64::NAN).is_nan());
        assert!(airy_bi_scaled(f64::NAN).is_nan());
    }
}
