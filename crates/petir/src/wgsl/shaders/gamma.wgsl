// SPDX-License-Identifier: GPL-3.0-only
//
// TRANSCRIBED to WGSL (f32) from `petir::specfunc::gamma`, which ports GSL's
// `specfunc/gamma.c`:
//   lanczos_7_c      -> the LANCZOS_7_C table below
//   lngamma_lanczos  -> petir_lngamma_lanczos
//   lngamma_1_pade   -> petir_lngamma_1_pade
//   lngamma_2_pade   -> petir_lngamma_2_pade
//   gsl_sf_lngamma_e -> petir_lngamma
//   gsl_sf_gamma_e   -> petir_gamma
//   Copyright (C) 1996-2007 Gerard Jungman. GPL-3.0-or-later.
//
// EVERY CONSTANT WAS EXTRACTED BY SCRIPT from the f64 source, not typed --
// nine Lanczos coefficients and eighteen Pade constants is exactly the volume
// at which a hand-transcribed digit goes unnoticed.
//
// WHAT f32 COSTS HERE, STATED UP FRONT. The Lanczos sum is a ratio of large
// nearly-cancelling terms: coefficients run to 1.26e3 and alternate in sign.
// In f64 that costs a couple of digits; in f32 it costs proportionally more,
// and `petir_lngamma` is accordingly NOT as accurate as its f64 original by
// the usual f32 factor. The measured figures are in `wgsl::mirror_gamma`.
//
// The two Pade branches near x = 1 and x = 2 are not an optimisation -- they
// are there because ln Gamma has a ZERO at each, where the Lanczos form
// computes a small difference of large numbers and loses every significant
// digit. Removing them would look like a simplification and would destroy the
// function on the interval most callers use.

// Lanczos g = 7, n = 9. GSL's `lanczos_7_c`.
fn petir_lanczos_c(k: u32) -> f32 {
    var c = array<f32, 9>(
        0.9999999999998099, 676.5203681218851, -1259.1392167224028, 771.3234287776531,
        -176.6150291621406, 12.507343278686905, -0.13857109526572012, 9.984369578019572e-6,
        1.5056327351493116e-7
    );
    return c[k];
}

const PETIR_LOG_ROOT_TWO_PI: f32 = 0.9189385332046727;
const PETIR_LN_PI: f32 = 1.1447298858494002;
const PETIR_PI: f32 = 3.141592653589793;
const PETIR_E: f32 = 2.718281828459045;

// ln Gamma(x) by the Lanczos approximation, valid for x >= 0.5.
//
// Ports `lngamma_lanczos`. The `- 7.0` at the end is the `g` parameter and
// the `z + 7.5` is `z + g + 0.5`; both are tied to the table above and none of
// the three may be changed alone.
fn petir_lngamma_lanczos(x: f32) -> f32 {
    let z = x - 1.0;
    var ag = petir_lanczos_c(0u);
    for (var k: u32 = 1u; k <= 8u; k = k + 1u) {
        ag = ag + petir_lanczos_c(k) / (z + f32(k));
    }
    let term1 = (z + 0.5) * log((z + 7.5) / PETIR_E);
    let term2 = PETIR_LOG_ROOT_TWO_PI + log(ag);
    return term1 + (term2 - 7.0);
}

// ln Gamma(1 + eps) for |eps| < 0.01. Ports `lngamma_1_pade`.
//
// A (2,2) Pade approximant to ln Gamma(1 + eps)/eps, plus a five-term
// correction in eps^5. Used because x = 1 is a zero of ln Gamma.
fn petir_lngamma_1_pade(eps: f32) -> f32 {
    let n1: f32 = -1.0017419282349508699871138440;
    let n2: f32 = 1.7364839209922879823280541733;
    let d1: f32 = 1.2433006018858751556055436011;
    let d2: f32 = 5.0456274100274010152489597514;
    let num = (eps + n1) * (eps + n2);
    let den = (eps + d1) * (eps + d2);
    let pade = 2.0816265188662692474880210318 * num / den;
    let c0: f32 = 0.004785324257581753;
    let c1: f32 = -0.01192457083645441;
    let c2: f32 = 0.01931961413960498;
    let c3: f32 = -0.02594027398725020;
    let c4: f32 = 0.03141928755021455;
    let eps5 = eps * eps * eps * eps * eps;
    let corr = eps5 * (c0 + eps * (c1 + eps * (c2 + eps * (c3 + c4 * eps))));
    return eps * (pade + corr);
}

// ln Gamma(2 + eps) for |eps| < 0.01. Ports `lngamma_2_pade`.
//
// The companion of the above at the second zero of ln Gamma.
fn petir_lngamma_2_pade(eps: f32) -> f32 {
    let n1: f32 = 1.000895834786669227164446568;
    let n2: f32 = 4.209376735287755081642901277;
    let d1: f32 = 2.618851904903217274682578255;
    let d2: f32 = 10.85766559900983515322922936;
    let num = (eps + n1) * (eps + n2);
    let den = (eps + d1) * (eps + d2);
    let pade = 2.85337998765781918463568869 * num / den;
    let c0: f32 = 0.0001139406357036744;
    let c1: f32 = -0.0001365435269792533;
    let c2: f32 = 0.0001067287169183665;
    let c3: f32 = -0.0000693271800931282;
    let c4: f32 = 0.0000407220927867950;
    let eps5 = eps * eps * eps * eps * eps;
    let corr = eps5 * (c0 + eps * (c1 + eps * (c2 + eps * (c3 + c4 * eps))));
    return eps * (pade + corr);
}

// ln |Gamma(x)|. Ports `gsl_sf_lngamma_e`.
//
// DEVIATION, and a necessary one: the f64 original RECURSES into itself for
// the reflection at x < 0.5. WGSL has no recursion at all, so the reflection
// is inlined -- it can only ever reach the x >= 0.5 branch, so one level of
// manual expansion is exact, not an approximation.
//
// Poles at x = 0, -1, -2, ... return +inf, as upstream does.
fn petir_lngamma(x: f32) -> f32 {
    if (x <= 0.0 && x == trunc(x)) {
        // WGSL has no inf or NaN literal, and `1.0 / 0.0` is constant-folded
        // and rejected at parse time. The bit pattern is the honest way to
        // say it -- an earlier draft returned f32::MAX here, which is a
        // different number and would have compared equal to nothing.
        return bitcast<f32>(0x7f800000u);   // +inf
    }
    if (abs(x - 1.0) < 0.01) { return petir_lngamma_1_pade(x - 1.0); }
    if (abs(x - 2.0) < 0.01) { return petir_lngamma_2_pade(x - 2.0); }
    if (x >= 0.5) { return petir_lngamma_lanczos(x); }

    // Reflection: ln Gamma(x) = ln(pi) - ln|sin(pi x)| - ln Gamma(1 - x).
    // 1 - x > 0.5 whenever x < 0.5, so the inlined call takes one of the
    // three branches above and never reflects again.
    let y = 1.0 - x;
    var inner: f32;
    if (abs(y - 1.0) < 0.01) {
        inner = petir_lngamma_1_pade(y - 1.0);
    } else if (abs(y - 2.0) < 0.01) {
        inner = petir_lngamma_2_pade(y - 2.0);
    } else {
        inner = petir_lngamma_lanczos(y);
    }
    let sin_term = abs(sin(PETIR_PI * x));
    return PETIR_LN_PI - log(sin_term) - inner;
}

// Gamma(x). Ports `gsl_sf_gamma_e`'s exp(lngamma) path.
//
// OVERFLOWS EARLY IN f32, and that is arithmetic rather than a defect:
// Gamma(35) is already 3e38, at the top of f32's range, so anything past
// about x = 35 is +inf. The f64 original reaches x = 171. A caller wanting
// large arguments wants `petir_lngamma` and should stay in log space.
fn petir_gamma(x: f32) -> f32 {
    if (x <= 0.0 && x == trunc(x)) {
        return bitcast<f32>(0x7fc00000u);   // quiet NaN, as upstream gives
    }
    let lg = petir_lngamma(x);
    if (x > 0.0) {
        return exp(lg);
    }
    // Negative non-integer: sign follows sin(pi x).
    let s = sin(PETIR_PI * x);
    if (s < 0.0) { return -exp(lg); }
    return exp(lg);
}

// ln B(a, b) = ln Gamma(a) + ln Gamma(b) - ln Gamma(a + b).
// Ports `gsl_sf_lnbeta_e`'s straightforward branch.
fn petir_lnbeta(a: f32, b: f32) -> f32 {
    return petir_lngamma(a) + petir_lngamma(b) - petir_lngamma(a + b);
}

// B(a, b). Ports `gsl_sf_beta_e`.
fn petir_beta(a: f32, b: f32) -> f32 {
    return exp(petir_lnbeta(a, b));
}
