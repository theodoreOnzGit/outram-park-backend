// SPDX-License-Identifier: GPL-3.0-only
//
// TRANSCRIBED to WGSL (f32) from `petir::expint` and
// `petir::specfunc::shint`, which port GSL 2.8's specfunc/expint.c and
// specfunc/shint.c.
//   Copyright (C) 1996-2000 Gerard Jungman; expint.c also (C) 2007 Brian
//   Gough. shint.c's small-argument series is from SLATEC's shi.f by
//   W. Fullerton.
//
//   E_1(x) = integral_x^inf e^{-t} / t  dt
//   Ei(x)  = -PV integral_{-x}^inf e^{-t} / t  dt  =  -E_1(-x)
//   Shi(x) = integral_0^x sinh(t)/t dt  =  (Ei(x) + E_1(x)) / 2
//   Chi(x) = gamma + ln|x| + int_0^x (cosh t - 1)/t dt = (Ei(x) - E_1(x))/2
//
// FIVE ENTRY POINTS FROM ONE BRANCH TREE. Ei is -E_1(-x) and nothing more --
// that is upstream's whole implementation of it -- and Shi and Chi are the
// sum and difference of the two, except for Shi's own small-argument series
// where E_1 and Ei each diverge logarithmically and their sum does not.
//
// SEVEN CHEBYSHEV SERIES AT GSL'S SINGLE-PRECISION ORDER: 99 coefficients
// where the f64 order needs 157. Six of the seven are cut; shi_cs is not,
// because its order_sp EQUALS its f64 order -- the series is seven terms
// long and its last coefficient is 4.67e-22, so there is nothing to cut.
// That is the first series in this module whose order_sp buys nothing, and
// it is recorded rather than silently passed over.
//
// THE COEFFICIENTS WERE EXTRACTED BY SCRIPT from the f64 modules and the
// same script emits `wgsl::mirror_expint`, so the shader and its CPU mirror
// cannot disagree about a constant.
//
// MACHINE CONSTANTS. xmax = -LOG_DBL_MIN - ln(-LOG_DBL_MIN) bounds E_1's
// unscaled tail in f64. Retargeted here to f32's range, 87.33654 - ln(87.33654)
// = 82.866776, which is a RANGE GUARD in the taxonomy of
// docs/wgsl-coverage.md -- and, like synchrotron's, one the arithmetic
// enforces anyway. It is kept rather than deleted because E_1's own
// 1/x * exp(-x) prefactor means the underflow point depends on x as well as
// on the exponent, so there is no single value the multiply reaches first.
//
// xsml = sqrt(f32::EPSILON) = 3.4526698e-4 is Shi's small-argument cut, a
// PRECISION constant, retargeted from f64's 1.4901161e-8.

// GSL's AE11_cs at its SINGLE-PRECISION order: 21 coefficients.
fn petir_expint_cheb_ae11(x: f32) -> f32 {
    var c = array<f32, 21>(0.12150324136018753, -0.06508877873420715, 0.004897651262581348, -0.000649237830657512, 9.384043369209394e-05, 4.2023637547572434e-07, -8.11337486084085e-06, 2.804247742460575e-06, 5.648716339123894e-08, -3.448091661084618e-07, 5.820927384547758e-08, 3.871142695288654e-08, -1.2453234887743747e-08, -5.1185047311719245e-09, 2.1487716050927474e-09, 8.684599150932115e-10, -3.4365010836978627e-10, -1.7979660815736764e-10, 4.7442060696623045e-11, 4.0423282776647085e-11, -3.543927954915982e-12);
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 20; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's AE12_cs at its SINGLE-PRECISION order: 16 coefficients.
fn petir_expint_cheb_ae12(x: f32) -> f32 {
    var c = array<f32, 16>(0.5824174880981445, -0.15834884345531464, -0.00676427548751235, 0.005125843919813633, 0.00043523250496946275, -0.00014361336070578545, -4.1801322367973626e-05, -2.7133958155900473e-06, 1.1513818662933772e-06, 4.2065002503477444e-07, 6.658190443431522e-08, 6.621437842468936e-10, -2.8441049515492978e-09, -9.407241652326093e-10, -1.7747660285838407e-10, -1.5830222549473305e-11);
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 15; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's E11_cs at its SINGLE-PRECISION order: 14 coefficients.
fn petir_expint_cheb_e11(x: f32) -> f32 {
    var c = array<f32, 14>(-16.113462448120117, 7.79407262802124, -1.955405831336975, 0.3733729422092438, -0.056925032287836075, 0.007211077958345413, -0.000781049020588398, 7.388093217741698e-05, -6.2028620959608816e-06, 4.6816001031402266e-07, -3.209288834682411e-08, 2.015199784821675e-09, -1.1673687017044188e-10, 6.2762707357666425e-12);
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 13; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's E12_cs at its SINGLE-PRECISION order: 11 coefficients.
fn petir_expint_cheb_e12(x: f32) -> f32 {
    var c = array<f32, 11>(-0.03739021345973015, 0.04272398725152016, -0.13031820952892303, 0.014419124461710453, -0.0013461707858368754, 0.00010731029033195227, -7.4299996413174085e-06, 4.537732536391559e-07, -2.4764172934510498e-08, 1.2207658217633366e-09, -5.4851415076662136e-11);
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 10; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's AE13_cs at its SINGLE-PRECISION order: 16 coefficients.
fn petir_expint_cheb_ae13(x: f32) -> f32 {
    var c = array<f32, 16>(-0.6057732701301575, -0.11253524571657181, 0.013432266190648079, -0.001926845172420144, 0.00030911833164282143, -5.3564133850159124e-05, 9.827813300944399e-06, -1.8853689880415914e-06, 3.7494319826691935e-07, -7.682345426474058e-08, 1.6143269832014084e-08, -3.466802178664352e-09, 7.587542261155988e-10, -1.6886433917839838e-10, 3.814570534443895e-11, -8.733025587404075e-12);
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 15; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's AE14_cs at its SINGLE-PRECISION order: 14 coefficients.
fn petir_expint_cheb_ae14(x: f32) -> f32 {
    var c = array<f32, 14>(-0.1892918050289154, -0.08648117631673813, 0.007224101573228836, -0.0008097559330053627, 0.0001099913424695842, -1.717332997941412e-05, 2.9856275887141237e-06, -5.659649104927666e-07, 1.1526808663120391e-07, -2.4950304933213374e-08, 5.692324389627856e-09, -1.3599577020073639e-09, 3.384662827787821e-10, -8.737852802420676e-11);
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 13; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's SHI_cs at its SINGLE-PRECISION order: 7 coefficients.
fn petir_expint_cheb_shi(x: f32) -> f32 {
    var c = array<f32, 7>(0.007837268523871899, 0.003922766540199518, 4.134678874834208e-06, 2.470748050598104e-09, 9.379295300496193e-13, 2.4518170692597655e-16, 4.6700000133587036e-20);
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 6; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}
const PETIR_EXPINT_XMAX: f32 = 82.866776;
const PETIR_EXPINT_SHI_XSML: f32 = 3.4526698e-4;

fn petir_expint_nan() -> f32 { return bitcast<f32>(0x7fc00000u); }

// E_1(x), GSL's expint_E1_impl with scale = 0. NaN at x = 0, where E_1
// diverges; 0 past the tail where the f32 arithmetic has underflowed.
fn petir_expint_e1(x: f32) -> f32 {
    if (x != x) { return petir_expint_nan(); }
    if (x <= -10.0) {
        let s = 1.0 / x * exp(-x);
        return s * (1.0 + petir_expint_cheb_ae11(20.0 / x + 1.0));
    }
    if (x <= -4.0) {
        let s = 1.0 / x * exp(-x);
        return s * (1.0 + petir_expint_cheb_ae12((40.0 / x + 7.0) / 3.0));
    }
    if (x <= -1.0) {
        let ln_term = -log(abs(x));
        return ln_term + petir_expint_cheb_e11((2.0 * x + 5.0) / 3.0);
    }
    if (x == 0.0) { return petir_expint_nan(); }
    if (x <= 1.0) {
        let ln_term = -log(abs(x));
        // The 0.6875 is upstream's; it folds the series' own offset.
        return ln_term - 0.6875 + x + petir_expint_cheb_e12(x);
    }
    if (x <= 4.0) {
        let s = 1.0 / x * exp(-x);
        return s * (1.0 + petir_expint_cheb_ae13((8.0 / x - 5.0) / 3.0));
    }
    if (x <= PETIR_EXPINT_XMAX) {
        let s = 1.0 / x * exp(-x);
        return s * (1.0 + petir_expint_cheb_ae14(8.0 / x - 1.0));
    }
    return 0.0;
}

// exp(x) E_1(x), the scaled form -- the one to prefer on a GPU, since it
// stays in range for every x where E_1 itself has underflowed to zero.
fn petir_expint_e1_scaled(x: f32) -> f32 {
    if (x != x) { return petir_expint_nan(); }
    if (x <= -10.0) {
        let s = 1.0 / x;
        return s * (1.0 + petir_expint_cheb_ae11(20.0 / x + 1.0));
    }
    if (x <= -4.0) {
        let s = 1.0 / x;
        return s * (1.0 + petir_expint_cheb_ae12((40.0 / x + 7.0) / 3.0));
    }
    if (x <= -1.0) {
        let ln_term = -log(abs(x));
        return exp(x) * (ln_term + petir_expint_cheb_e11((2.0 * x + 5.0) / 3.0));
    }
    if (x == 0.0) { return petir_expint_nan(); }
    if (x <= 1.0) {
        let ln_term = -log(abs(x));
        return exp(x) * (ln_term - 0.6875 + x + petir_expint_cheb_e12(x));
    }
    if (x <= 4.0) {
        let s = 1.0 / x;
        return s * (1.0 + petir_expint_cheb_ae13((8.0 / x - 5.0) / 3.0));
    }
    let s = 1.0 / x;
    return s * (1.0 + petir_expint_cheb_ae14(8.0 / x - 1.0));
}

// Ei(x) = -E_1(-x). Upstream's entire definition.
fn petir_expint_ei(x: f32) -> f32 { return -petir_expint_e1(-x); }

// exp(-x) Ei(x).
fn petir_expint_ei_scaled(x: f32) -> f32 { return -petir_expint_e1_scaled(-x); }

// Shi(x) = integral_0^x sinh(t)/t dt. Odd.
fn petir_shi(x: f32) -> f32 {
    if (x != x) { return petir_expint_nan(); }
    let ax = abs(x);
    if (ax < PETIR_EXPINT_SHI_XSML) { return x; }
    if (ax <= 0.375) {
        return x * (1.0 + petir_expint_cheb_shi(128.0 * x * x / 9.0 - 1.0));
    }
    return 0.5 * (petir_expint_ei(x) + petir_expint_e1(x));
}

// Chi(x). Real for x > 0; upstream returns a value for x < 0 too and that is
// carried. NaN at x = 0, which is E_1's singularity.
fn petir_chi(x: f32) -> f32 {
    if (x != x) { return petir_expint_nan(); }
    return 0.5 * (petir_expint_ei(x) - petir_expint_e1(x));
}

// One dispatcher, in the order E_1, E_1 scaled, Ei, Ei scaled, Shi, Chi.
fn petir_expint_family(which: u32, x: f32) -> f32 {
    switch (which) {
        case 0u: { return petir_expint_e1(x); }
        case 1u: { return petir_expint_e1_scaled(x); }
        case 2u: { return petir_expint_ei(x); }
        case 3u: { return petir_expint_ei_scaled(x); }
        case 4u: { return petir_shi(x); }
        case 5u: { return petir_chi(x); }
        default: { return petir_expint_nan(); }
    }
}
