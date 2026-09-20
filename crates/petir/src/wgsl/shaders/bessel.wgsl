// SPDX-License-Identifier: GPL-3.0-only
//
// TRANSCRIBED to WGSL (f32) from `petir::specfunc::bessel`, which ports GSL
// 2.8's specfunc/bessel_{J,Y,I,K}{0,1}.c, bessel_amp_phase.c and the
// cos_pi4/sin_pi4 helpers in bessel.c.
//   Copyright (C) 1996-2000 Gerard Jungman. GPL-3.0-or-later.
//   Copyright (C) 2010 Brian Gough (the K0/K1 polynomial branches).
//
// EVERY COEFFICIENT WAS EXTRACTED BY SCRIPT from the f64 module -- 358
// literals across 22 tables, which is far past the volume at which a
// hand-transcribed digit goes unnoticed. The same script emits
// `wgsl::mirror_bessel`, so the shader and its CPU mirror cannot disagree
// about a constant.
//
// THE BRANCH THRESHOLDS ARE GSL'S OWN f64 VALUES, DELIBERATELY. Several are
// derived from DBL_EPSILON or DBL_MIN and look wrong beside an f32 kernel.
// They are kept because each one degenerates harmlessly, and checking that
// was cheaper than defending a second set of numbers:
//
//   y < 2*SQRT_DBL_EPSILON (3.0e-8)  -- unreachable above f32 denormals, so
//     the Chebyshev branch handles the small arguments instead. It evaluates
//     at x = -1 there, which is the series' own endpoint and accurate.
//   y < 2*DBL_MIN (4.5e-308)         -- flushes to zero in f32, so the test
//     is never taken. The branch it guards returns 0.5*x, which is what the
//     next branch produces anyway.
//   y < LOG_DBL_MAX - 1 (708.0)      -- I_0 and I_1 overflow f32 near x = 88
//     long before this, and exp() returns inf there of its own accord. The
//     guard is therefore redundant rather than wrong.
//   xmax = 1/DBL_EPSILON (4.5e15)    -- representable in f32 and kept as is.
//
// ONE FUNCTION IS DELIBERATELY NOT TRANSCRIBED. GSL's exp_mult_err_e splits
// each exponent into integer and fractional parts so that neither exp() call
// overflows on its own, buying K_0/K_1 the range between 0.5*LOG_DBL_MAX and
// LOG_DBL_MAX. In f32 both of those bounds are the same 88, so the split buys
// nothing and `petir_bessel_k0` simply multiplies by exp(-x).
//
// WHAT f32 COSTS HERE. The amplitude/phase branches past x = 4 (J, Y) and
// x = 8 (I, K) call sin, cos, sqrt and exp, whose WGSL specification is an
// ULP BOUND rather than correct rounding -- so these kernels cannot be held
// to bit-identity with the mirror, unlike the pure-arithmetic ones. Worse,
// the phase is y itself: at x = 50 an f32 y already carries an absolute
// error of ~4e-6 radians, which the cos() turns into a relative error of the
// same order wherever J_0 is not near its peak. The measured figures are in
// `wgsl::mirror_bessel`. Do not read the f64 module's 1e-16 figures as
// applying here.

// GSL's `bi0_cs`, 12 coefficients, Clenshaw in GSL's convention
// (closing with 0.5*c[0]). Mirrors `cheb_bi0` in wgsl::mirror_bessel.
fn petir_bessel_cheb_bi0(x: f32) -> f32 {
    var c = array<f32, 12>(
        -0.0766054725, 1.92733795, 0.228264459, 0.0130489147, 0.00043442709, 9.42265769e-06,
        1.43400629e-07, 1.61384907e-09, 1.39665004e-11, 9.579451e-14, 5.3339e-16, 2.45e-18
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 11; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's `ai0_data` at its SINGLE-PRECISION order: 14 of the 21 stored,
// where `f64` evaluates 21.
// (closing with 0.5*c[0]). Mirrors `cheb_ai0` in wgsl::mirror_bessel.
fn petir_bessel_cheb_ai0(x: f32) -> f32 {
    var c = array<f32, 14>(
        0.0757599449, 0.00759138081, 0.000415313134, 1.07007646e-05, -7.90117998e-06,
        -7.8261435e-07, 2.78384994e-07, 8.2524726e-09, -1.20446394e-08, 1.55964859e-09,
        2.2925563e-10, -1.1916228e-10, 1.757854e-11, 1.12822e-12,
    );
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

// GSL's `ai02_data` at its SINGLE-PRECISION order: 12 of the 22 stored,
// where `f64` evaluates 22.
// (closing with 0.5*c[0]). Mirrors `cheb_ai02` in wgsl::mirror_bessel.
fn petir_bessel_cheb_ai02(x: f32) -> f32 {
    var c = array<f32, 12>(
        0.054490411, 0.00336911648, 6.88975835e-05, 2.89137052e-06, 2.04891859e-07,
        2.26666899e-08, 3.39623203e-09, 4.9406022e-10, 1.188914e-11, -3.149915e-11,
        -1.32158e-11, -1.79419e-12,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 11; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's `bi1_cs`, 11 coefficients, Clenshaw in GSL's convention
// (closing with 0.5*c[0]). Mirrors `cheb_bi1` in wgsl::mirror_bessel.
fn petir_bessel_cheb_bi1(x: f32) -> f32 {
    var c = array<f32, 11>(
        -0.00197171326, 0.407348877, 0.0348389943, 0.00154539456, 4.18885211e-05,
        7.64902676e-07, 1.00424939e-08, 9.9322077e-11, 7.6638e-13, 4.741e-15, 2.4e-17
    );
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

// GSL's `ai1_data` at its SINGLE-PRECISION order: 12 of the 21 stored,
// where `f64` evaluates 21.
// (closing with 0.5*c[0]). Mirrors `cheb_ai1` in wgsl::mirror_bessel.
fn petir_bessel_cheb_ai1(x: f32) -> f32 {
    var c = array<f32, 12>(
        -0.0284674418, -0.0192295323, -0.000611518586, -2.06997125e-05, 8.58561915e-06,
        1.04949825e-06, -2.91833892e-07, -1.55937815e-08, 1.31801237e-08, -1.44842341e-09,
        -2.9085122e-10, 1.2663889e-10,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 11; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's `ai12_data` at its SINGLE-PRECISION order: 10 of the 22 stored,
// where `f64` evaluates 22.
// (closing with 0.5*c[0]). Mirrors `cheb_ai12` in wgsl::mirror_bessel.
fn petir_bessel_cheb_ai12(x: f32) -> f32 {
    var c = array<f32, 10>(
        0.028576235, -0.00976109749, -0.000110588939, -3.88256481e-06, -2.51223624e-07,
        -2.63146885e-08, -3.83538039e-09, -5.5897433e-10, -1.897495e-11, 3.252602e-11,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 9; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's `bj0_data` at its SINGLE-PRECISION order: 10 of the 13 stored,
// where `f64` evaluates 13.
// (closing with 0.5*c[0]). Mirrors `cheb_bj0` in wgsl::mirror_bessel.
fn petir_bessel_cheb_bj0(x: f32) -> f32 {
    var c = array<f32, 10>(
        0.100254162, -0.665223008, 0.248983703, -0.0332527232, 0.00231141793,
        -9.91127742e-05, 2.89167086e-06, -6.12108587e-08, 9.83865079e-10, -1.24235515e-11,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 9; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's `bj1_data` at its SINGLE-PRECISION order: 9 of the 12 stored,
// where `f64` evaluates 12.
// (closing with 0.5*c[0]). Mirrors `cheb_bj1` in wgsl::mirror_bessel.
fn petir_bessel_cheb_bj1(x: f32) -> f32 {
    var c = array<f32, 9>(
        -0.117261415, -0.253615218, 0.050127081, -0.00463151481, 0.000247996229,
        -8.67894869e-06, 2.14293917e-07, -3.93609308e-09, 5.5911823e-11,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 8; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's `by0_data` at its SINGLE-PRECISION order: 9 of the 13 stored,
// where `f64` evaluates 13.
// (closing with 0.5*c[0]). Mirrors `cheb_by0` in wgsl::mirror_bessel.
fn petir_bessel_cheb_by0(x: f32) -> f32 {
    var c = array<f32, 9>(
        -0.0112778394, -0.128345238, -0.104378848, 0.0236627492, -0.00209039165,
        0.000103975454, -3.36974716e-06, 7.72938427e-08, -1.32497677e-09,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 8; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's `by1_data` at its SINGLE-PRECISION order: 11 of the 14 stored,
// where `f64` evaluates 14.
// (closing with 0.5*c[0]). Mirrors `cheb_by1` in wgsl::mirror_bessel.
fn petir_bessel_cheb_by1(x: f32) -> f32 {
    var c = array<f32, 11>(
        0.032080471, 1.2627079, 0.0064999619, -0.0893616453, 0.0132508812, -0.000897905912,
        3.64736149e-05, -1.00137438e-06, 1.99453966e-08, -3.0230656e-10, 3.60987815e-12,
    );
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

// GSL's `ak0_data` at its SINGLE-PRECISION order: 11 of the 24 stored,
// where `f64` evaluates 24.
// (closing with 0.5*c[0]). Mirrors `cheb_ak0` in wgsl::mirror_bessel.
fn petir_bessel_cheb_ak0(x: f32) -> f32 {
    var c = array<f32, 11>(
        -0.0328737867, -0.0449369058, 0.00298149992, -0.000303693649, 3.91085569e-05,
        -5.86872422e-06, 9.8287371e-07, -1.78978645e-07, 3.48332307e-08, -7.1590921e-09,
        1.5401993e-09,
    );
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

// GSL's `ak02_data` at its SINGLE-PRECISION order: 9 of the 14 stored,
// where `f64` evaluates 14.
// (closing with 0.5*c[0]). Mirrors `cheb_ak02` in wgsl::mirror_bessel.
fn petir_bessel_cheb_ak02(x: f32) -> f32 {
    var c = array<f32, 9>(
        -0.0120186983, -0.00917485269, 0.000144455093, -4.01361418e-06, 1.56783181e-07,
        -7.77011044e-09, 4.61118258e-10, -3.158593e-11, 2.43501804e-12,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 8; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's `ak1_data` at its SINGLE-PRECISION order: 10 of the 25 stored,
// where `f64` evaluates 25.
// (closing with 0.5*c[0]). Mirrors `cheb_ak1` in wgsl::mirror_bessel.
fn petir_bessel_cheb_ak1(x: f32) -> f32 {
    var c = array<f32, 10>(
        0.207996868, 0.162581565, -0.00587070424, 0.00049502152, -5.78958348e-05,
        8.1861461e-06, -1.31604832e-06, 2.32546032e-07, -4.42206518e-08, 8.92163995e-09,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 9; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's `ak12_data` at its SINGLE-PRECISION order: 8 of the 14 stored,
// where `f64` evaluates 14.
// (closing with 0.5*c[0]). Mirrors `cheb_ak12` in wgsl::mirror_bessel.
fn petir_bessel_cheb_ak12(x: f32) -> f32 {
    var c = array<f32, 8>(
        0.0637930834, 0.0283288781, -0.000247537067, 5.77197245e-06, -2.06893922e-07,
        9.73998344e-09, -5.58533614e-10, 3.73299663e-11,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 7; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's `bm0_data` at its SINGLE-PRECISION order: 11 of the 21 stored,
// where `f64` evaluates 21.
// (closing with 0.5*c[0]). Mirrors `cheb_bm0` in wgsl::mirror_bessel.
fn petir_bessel_cheb_bm0(x: f32) -> f32 {
    var c = array<f32, 11>(
        0.0928496164, -0.00142987707, 2.83057927e-05, -1.43300611e-06, 1.2028628e-07,
        -1.39711301e-08, 2.04076188e-09, -3.5399669e-10, 7.024759e-11, -1.554107e-11,
        3.76226e-12,
    );
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

// GSL's `bth0_data` at its SINGLE-PRECISION order: 13 of the 24 stored,
// where `f64` evaluates 24.
// (closing with 0.5*c[0]). Mirrors `cheb_bth0` in wgsl::mirror_bessel.
fn petir_bessel_cheb_bth0(x: f32) -> f32 {
    var c = array<f32, 13>(
        -0.246391638, 0.00173709831, -6.21836334e-05, 4.36805017e-06, -4.5609302e-07,
        6.21974001e-08, -1.03004429e-08, 1.97952678e-09, -4.28198396e-10, 1.0203584e-10,
        -2.6363898e-11, 7.297935e-12, -2.144188e-12,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 12; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's `bm1_data` at its SINGLE-PRECISION order: 11 of the 21 stored,
// where `f64` evaluates 21.
// (closing with 0.5*c[0]). Mirrors `cheb_bm1` in wgsl::mirror_bessel.
fn petir_bessel_cheb_bm1(x: f32) -> f32 {
    var c = array<f32, 11>(
        0.104736251, 0.00442443894, -5.6616395e-05, 2.31349417e-06, -1.7377182e-07,
        1.89320993e-08, -2.65416023e-09, 4.4740209e-10, -8.691795e-11, 1.891492e-11,
        -4.51884e-12,
    );
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

// GSL's `bth1_data` at its SINGLE-PRECISION order: 13 of the 24 stored,
// where `f64` evaluates 24.
// (closing with 0.5*c[0]). Mirrors `cheb_bth1` in wgsl::mirror_bessel.
fn petir_bessel_cheb_bth1(x: f32) -> f32 {
    var c = array<f32, 13>(
        0.74060141, -0.00457175566, 0.000119818511, -6.96456189e-06, 6.55495621e-07,
        -8.40662289e-08, 1.33768866e-08, -2.49956565e-09, 5.294951e-10, -1.24135944e-10,
        3.1656485e-11, -8.66864e-12, 2.523758e-12,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 12; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's `k0_poly`, 8 coefficients, Horner (gsl_poly_eval).
fn petir_bessel_poly_k0(x: f32) -> f32 {
    var c = array<f32, 8>(
        0.115931516, 0.278982879, 0.0252489299, 0.000846035091, 1.49147192e-05, 1.62710689e-07,
        1.20826603e-09, 6.61171047e-12
    );
    var ans = c[7];
    for (var j: i32 = 6; j >= 0; j = j - 1) {
        ans = c[j] + x * ans;
    }
    return ans;
}

// GSL's `i0_poly`, 7 coefficients, Horner (gsl_poly_eval).
fn petir_bessel_poly_i0(x: f32) -> f32 {
    var c = array<f32, 7>(
        1.0, 0.25, 0.0277777778, 0.00173611111, 6.9444476e-05, 1.92882658e-06, 3.99082206e-08
    );
    var ans = c[6];
    for (var j: i32 = 5; j >= 0; j = j - 1) {
        ans = c[j] + x * ans;
    }
    return ans;
}

// GSL's `k1_poly`, 9 coefficients, Horner (gsl_poly_eval).
fn petir_bessel_poly_k1(x: f32) -> f32 {
    var c = array<f32, 9>(
        -0.307965758, -0.0853707197, -0.00464218277, -0.00011253607, -1.55928877e-06,
        -1.40301637e-08, -8.87189986e-11, -4.16143236e-13, -1.52612934e-15
    );
    var ans = c[8];
    for (var j: i32 = 7; j >= 0; j = j - 1) {
        ans = c[j] + x * ans;
    }
    return ans;
}

// GSL's `i1_poly`, 6 coefficients, Horner (gsl_poly_eval).
fn petir_bessel_poly_i1(x: f32) -> f32 {
    var c = array<f32, 6>(
        0.0833333333, 0.00694444444, 0.000347222222, 1.1574076e-05, 2.755587e-07,
        4.97243862e-09
    );
    var ans = c[5];
    for (var j: i32 = 4; j >= 0; j = j - 1) {
        ans = c[j] + x * ans;
    }
    return ans;
}

const PETIR_BESSEL_SQRT2: f32 = 1.4142135623730951;
const PETIR_BESSEL_ROOT_EIGHT: f32 = 2.8284271247461903;
const PETIR_BESSEL_SQRT_DBL_EPSILON: f32 = 1.4901161193847656e-8;
const PETIR_BESSEL_ROOT5_DBL_EPSILON: f32 = 7.4009597974140505e-4;
const PETIR_BESSEL_TWO_OVER_PI: f32 = 0.6366197723675814;
const PETIR_BESSEL_LN2: f32 = 0.6931471805599453;
const PETIR_BESSEL_XMAX: f32 = 4.503599627370496e15;

// sin(eps) and cos(eps) as (x, y), by GSL's own branch: a two-term series
// below ROOT5_DBL_EPSILON, the builtins above it. Ports `sin_cos_eps`.
//
// The series threshold is left at GSL's f64 value, which is CONSERVATIVE in
// f32 rather than wrong -- it switches to the builtins earlier than an f32
// analysis would, so the series is never used outside its valid range.
fn petir_bessel_sin_cos_eps(eps: f32) -> vec2<f32> {
    if (abs(eps) < PETIR_BESSEL_ROOT5_DBL_EPSILON) {
        let e2 = eps * eps;
        return vec2<f32>(
            eps * (1.0 - e2 / 6.0 * (1.0 - e2 / 20.0)),
            1.0 - e2 / 2.0 * (1.0 - e2 / 12.0)
        );
    }
    return vec2<f32>(sin(eps), cos(eps));
}

// cos(y - pi/4 + eps), never formed by adding pi/4 to y. Ports `cos_pi4`.
fn petir_bessel_cos_pi4(y: f32, eps: f32) -> f32 {
    let sy = sin(y);
    let cy = cos(y);
    let s = sy + cy;
    let d = sy - cy;
    let se = petir_bessel_sin_cos_eps(eps);
    return (se.y * s - se.x * d) / PETIR_BESSEL_SQRT2;
}

// sin(y - pi/4 + eps). Ports `sin_pi4`.
fn petir_bessel_sin_pi4(y: f32, eps: f32) -> f32 {
    let sy = sin(y);
    let cy = cos(y);
    let s = sy + cy;
    let d = sy - cy;
    let se = petir_bessel_sin_cos_eps(eps);
    return (se.y * d + se.x * s) / PETIR_BESSEL_SQRT2;
}

// J_0(x). Ports `bessel_j0`.
fn petir_bessel_j0(x: f32) -> f32 {
    let y = abs(x);
    if (y < 2.0 * PETIR_BESSEL_SQRT_DBL_EPSILON) {
        return 1.0;
    }
    if (y <= 4.0) {
        return petir_bessel_cheb_bj0(0.125 * y * y - 1.0);
    }
    let z = 32.0 / (y * y) - 1.0;
    let ca = petir_bessel_cheb_bm0(z);
    let ct = petir_bessel_cheb_bth0(z);
    let cp = petir_bessel_cos_pi4(y, ct / y);
    return (0.75 + ca) / sqrt(y) * cp;
}

// J_1(x). Ports `bessel_j1`.
fn petir_bessel_j1(x: f32) -> f32 {
    let y = abs(x);
    if (y == 0.0) {
        return 0.0;
    }
    if (y < PETIR_BESSEL_ROOT_EIGHT * PETIR_BESSEL_SQRT_DBL_EPSILON) {
        return 0.5 * x;
    }
    if (y < 4.0) {
        return x * (0.25 + petir_bessel_cheb_bj1(0.125 * y * y - 1.0));
    }
    let z = 32.0 / (y * y) - 1.0;
    let ca = petir_bessel_cheb_bm1(z);
    let ct = petir_bessel_cheb_bth1(z);
    let sp = petir_bessel_sin_pi4(y, ct / y);
    let ampl = (0.75 + ca) / sqrt(y);
    return select(ampl, -ampl, x < 0.0) * sp;
}

// Y_0(x), x > 0. Ports `bessel_y0`. Returns NaN for x <= 0.
fn petir_bessel_y0(x: f32) -> f32 {
    if (!(x > 0.0)) {
        return bitcast<f32>(0x7fc00000u);
    }
    if (x < 4.0) {
        let j0 = petir_bessel_j0(x);
        let c = petir_bessel_cheb_by0(0.125 * x * x - 1.0);
        return PETIR_BESSEL_TWO_OVER_PI * (-PETIR_BESSEL_LN2 + log(x)) * j0 + 0.375 + c;
    }
    if (x < PETIR_BESSEL_XMAX) {
        let z = 32.0 / (x * x) - 1.0;
        let c1 = petir_bessel_cheb_bm0(z);
        let c2 = petir_bessel_cheb_bth0(z);
        let sp = petir_bessel_sin_pi4(x, c2 / x);
        return (0.75 + c1) / sqrt(x) * sp;
    }
    return 0.0;
}

// Y_1(x), x > 0. Ports `bessel_y1`. Returns NaN for x <= 0.
fn petir_bessel_y1(x: f32) -> f32 {
    if (!(x > 0.0)) {
        return bitcast<f32>(0x7fc00000u);
    }
    if (x < 2.0 * PETIR_BESSEL_SQRT_DBL_EPSILON) {
        let j1 = petir_bessel_j1(x);
        let c = petir_bessel_cheb_by1(-1.0);
        return PETIR_BESSEL_TWO_OVER_PI * log(0.5 * x) * j1 + (0.5 + c) / x;
    }
    if (x < 4.0) {
        let c = petir_bessel_cheb_by1(0.125 * x * x - 1.0);
        let j1 = petir_bessel_j1(x);
        return PETIR_BESSEL_TWO_OVER_PI * log(0.5 * x) * j1 + (0.5 + c) / x;
    }
    if (x < PETIR_BESSEL_XMAX) {
        let z = 32.0 / (x * x) - 1.0;
        let ca = petir_bessel_cheb_bm1(z);
        let ct = petir_bessel_cheb_bth1(z);
        let cp = petir_bessel_cos_pi4(x, ct / x);
        return -(0.75 + ca) / sqrt(x) * cp;
    }
    return 0.0;
}

// e^{-|x|} I_0(x). Ports `bessel_i0_scaled`.
fn petir_bessel_i0_scaled(x: f32) -> f32 {
    let y = abs(x);
    if (y < 2.0 * PETIR_BESSEL_SQRT_DBL_EPSILON) {
        return 1.0 - y;
    }
    if (y <= 3.0) {
        return exp(-y) * (2.75 + petir_bessel_cheb_bi0(y * y / 4.5 - 1.0));
    }
    if (y <= 8.0) {
        return (0.375 + petir_bessel_cheb_ai0((48.0 / y - 11.0) / 5.0)) / sqrt(y);
    }
    return (0.375 + petir_bessel_cheb_ai02(16.0 / y - 1.0)) / sqrt(y);
}

// I_0(x). Ports `bessel_i0`. Overflows to +inf near |x| = 88 in f32.
fn petir_bessel_i0(x: f32) -> f32 {
    let y = abs(x);
    if (y < 2.0 * PETIR_BESSEL_SQRT_DBL_EPSILON) {
        return 1.0;
    }
    if (y <= 3.0) {
        return 2.75 + petir_bessel_cheb_bi0(y * y / 4.5 - 1.0);
    }
    return exp(y) * petir_bessel_i0_scaled(x);
}

// e^{-|x|} I_1(x). Ports `bessel_i1_scaled`.
fn petir_bessel_i1_scaled(x: f32) -> f32 {
    let y = abs(x);
    if (y == 0.0) {
        return 0.0;
    }
    if (y < PETIR_BESSEL_ROOT_EIGHT * PETIR_BESSEL_SQRT_DBL_EPSILON) {
        return 0.5 * x;
    }
    if (y <= 3.0) {
        return x * exp(-y) * (0.875 + petir_bessel_cheb_bi1(y * y / 4.5 - 1.0));
    }
    var c = 0.0;
    if (y <= 8.0) {
        c = petir_bessel_cheb_ai1((48.0 / y - 11.0) / 5.0);
    } else {
        c = petir_bessel_cheb_ai12(16.0 / y - 1.0);
    }
    let b = (0.375 + c) / sqrt(y);
    return select(-b, b, x > 0.0);
}

// I_1(x). Ports `bessel_i1`. Overflows near |x| = 88 in f32, with the sign of
// its argument -- see the f64 module for why that differs from GSL.
fn petir_bessel_i1(x: f32) -> f32 {
    let y = abs(x);
    if (y == 0.0) {
        return 0.0;
    }
    if (y < PETIR_BESSEL_ROOT_EIGHT * PETIR_BESSEL_SQRT_DBL_EPSILON) {
        return 0.5 * x;
    }
    if (y <= 3.0) {
        return x * (0.875 + petir_bessel_cheb_bi1(y * y / 4.5 - 1.0));
    }
    return exp(y) * petir_bessel_i1_scaled(x);
}

// e^{x} K_0(x), x > 0. Ports `bessel_k0_scaled`. Returns NaN for x <= 0.
fn petir_bessel_k0_scaled(x: f32) -> f32 {
    if (!(x > 0.0)) {
        return bitcast<f32>(0x7fc00000u);
    }
    if (x < 1.0) {
        let x2 = x * x;
        let i0 = 1.0 + 0.25 * x2 * petir_bessel_poly_i0(0.25 * x2);
        return exp(x) * (petir_bessel_poly_k0(x2) - log(x) * i0);
    }
    if (x <= 8.0) {
        // 1.203125 = 77/64, upstream's own note.
        return (1.203125 + petir_bessel_cheb_ak0((16.0 / x - 9.0) / 7.0)) / sqrt(x);
    }
    return (1.25 + petir_bessel_cheb_ak02(16.0 / x - 1.0)) / sqrt(x);
}

// K_0(x), x > 0. Ports `bessel_k0`. Returns NaN for x <= 0 and underflows to
// zero near x = 88 in f32.
fn petir_bessel_k0(x: f32) -> f32 {
    if (!(x > 0.0)) {
        return bitcast<f32>(0x7fc00000u);
    }
    if (x < 1.0) {
        let x2 = x * x;
        let i0 = 1.0 + 0.25 * x2 * petir_bessel_poly_i0(0.25 * x2);
        return petir_bessel_poly_k0(x2) - log(x) * i0;
    }
    return exp(-x) * petir_bessel_k0_scaled(x);
}

// e^{x} K_1(x), x > 0. Ports `bessel_k1_scaled`. Returns NaN for x <= 0.
fn petir_bessel_k1_scaled(x: f32) -> f32 {
    if (!(x > 0.0)) {
        return bitcast<f32>(0x7fc00000u);
    }
    if (x < 1.0) {
        let x2 = x * x;
        let t = 0.25 * x2;
        let i1 = 0.5 * x * (1.0 + t * (0.5 + t * petir_bessel_poly_i1(t)));
        return exp(x) * (x2 * petir_bessel_poly_k1(x2) + x * log(x) * i1 + 1.0) / x;
    }
    if (x <= 8.0) {
        // 1.375 = 11/8, upstream's own note.
        return (1.375 + petir_bessel_cheb_ak1((16.0 / x - 9.0) / 7.0)) / sqrt(x);
    }
    return (1.25 + petir_bessel_cheb_ak12(16.0 / x - 1.0)) / sqrt(x);
}

// K_1(x), x > 0. Ports `bessel_k1`. Returns NaN for x <= 0.
fn petir_bessel_k1(x: f32) -> f32 {
    if (!(x > 0.0)) {
        return bitcast<f32>(0x7fc00000u);
    }
    if (x < 1.0) {
        let x2 = x * x;
        let t = 0.25 * x2;
        let i1 = 0.5 * x * (1.0 + t * (0.5 + t * petir_bessel_poly_i1(t)));
        return (x2 * petir_bessel_poly_k1(x2) + x * log(x) * i1 + 1.0) / x;
    }
    return exp(-x) * petir_bessel_k1_scaled(x);
}
