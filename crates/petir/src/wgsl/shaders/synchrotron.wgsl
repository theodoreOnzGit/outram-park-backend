// SPDX-License-Identifier: GPL-3.0-only
//
// TRANSCRIBED to WGSL (f32) from `petir::specfunc::synchrotron`, which ports
// GSL 2.8's specfunc/synchrotron.c.
//   Copyright (C) 1996-2000 Gerard Jungman; small-argument correction terms
//   by Brian Gough.
//
// S_1(x) = x integral_x^inf K_{5/3}(t) dt   -- total synchrotron power
// S_2(x) = x K_{2/3}(x)                     -- the polarised difference
//
// Both rise like x^{1/3} and fall like sqrt(pi x / 2) e^{-x}: a spectrum with
// one broad peak. x is frequency in units of the critical frequency.
//
// EVERY COEFFICIENT WAS EXTRACTED BY SCRIPT from the f64 module -- 91
// literals across six Chebyshev series -- and the same script emits
// `wgsl::mirror_synchrotron`, so the shader and its CPU mirror cannot
// disagree about a constant.
//
// ONE MACHINE CONSTANT IS RETARGETED AND ONE IS DELIBERATELY KEPT, and
// telling them apart took a measurement that reversed the first answer.
//
//   2 sqrt(2) sqrt(DBL_EPSILON)   4.21e-08 -> 0.0009765625 (exactly 2^-10)
//       A PRECISION constant. Retargeted.
//
//   -8 ln(DBL_MIN) / 7            809.5959 -> KEPT AS IS
//       A RANGE GUARD, derived from DBL_MIN. NOT retargeted -- see below.
//
// THE RANGE GUARD WAS RETARGETED FIRST, TO 99.813194, AND THAT WAS WRONG.
// Measured in f32: the retargeted guard fires at 99.8132, but exp(c0 - x)
// does not reach zero on its own until 104.198. Between those the function is
// still returning representable denormals -- 5.65e-43 just below the cut, and
// f32 holds down to 1.4e-45. Moving the guard out changed 1461 of 4001
// answers over x in [95, 107]: the retargeted guard was DISCARDING ANSWERS
// THAT EXIST.
//
// That is airy.wgsl's case exactly, and the taxonomy in
// docs/wgsl-coverage.md already says so: a guard derived from the EXPONENT
// RANGE is enforced by the arithmetic anyway, so retargeting it can only
// take away. Keeping GSL's f64 value means the arithmetic acts at 104.198,
// which is where it should.
//
// Note the f64 module reaches the opposite conclusion about the same
// constant for the same reason: there the guard sits at 809.5959 while the
// arithmetic reaches zero at 745.3590, so it is 64 units of DEAD CODE. Dead
// at f64, harmful if retargeted at f32, correct as written at both.
//
// AND THE DEVICE PUTS IT IN A THIRD PLACE. Measured on llvmpipe (LLVM
// 20.1.2, 2026-09-19): this shader's tail reaches zero at x = 87.57, earlier
// than the f32 CPU mirror's 104.1979, because WGSL's exp() returns exactly
// zero as soon as its own result would be denormal -- exp(-87) = 1.6458e-38,
// exp(-88) = 0 -- while a plain multiply on the same device produces
// denormals down to 1e-44 without complaint. The exponential here is
// multiplied by a prefactor of order 10, so normal f32 answers are lost:
// 1.1010e-37 at x = 87.57 becomes 0.
//
// Three backends put the true underflow point in three different places and
// none of them is 809.5959. THAT is why upstream's constant is kept: a guard
// that never fires lets each backend underflow wherever its own arithmetic
// does, while a retargeted one hard-codes one of the three and is wrong on
// the other two. tests/wgsl_gpu.rs asserts the tail as a shape accordingly.

// GSL's synch1_cs, 13 coefficients.
fn petir_synch_cheb_synch1(x: f32) -> f32 {
    var c = array<f32, 13>(
        30.364683151245117, 17.079395294189453, 4.560132026672363, 0.5492812395095825,
        0.037297606468200684, 0.00161362427752465, 4.8191675887210295e-05,
        1.0512425205888576e-06, 1.7463850809917858e-08, 2.2815486999672174e-10,
        2.4044308211124132e-12, 2.0865880773430004e-14, 1.5166999746804084e-16,
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

// GSL's synch2_cs, 12 coefficients.
fn petir_synch_cheb_synch2(x: f32) -> f32 {
    var c = array<f32, 12>(
        0.4490721523761749, 0.08983536809682846, 0.008104457519948483, 0.0004261716967448592,
        1.4760963495064061e-05, 3.628633749030996e-07, 6.663480878188466e-09,
        9.490771363251937e-11, 1.079124962131972e-12, 1.0021999811255736e-14,
        7.69999999087401e-17, 5.000000229068525e-19,
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

// GSL's synch1a_cs, 23 coefficients.
fn petir_synch_cheb_synch1a(x: f32) -> f32 {
    var c = array<f32, 23>(
        2.1329305171966553, 0.07413528859615326, 0.008696810342371464, 0.0011703826021403074,
        0.00016451058036182076, 2.4020102500799112e-05, 3.5827756619255524e-06,
        5.447747639664158e-07, 8.388028760464294e-08, 1.3069882953686829e-08,
        2.0530990241240943e-09, 3.2518754355947976e-10, 5.179140449840247e-11,
        8.30029881632166e-12, 1.3352728211318832e-12, 2.1591500018024873e-13,
        3.4996701039511843e-14, 5.6994000990039095e-15, 9.290999578105198e-16,
        1.519999962103352e-16, 2.4899999248585602e-17, 4.100000115457887e-18,
        6.9999999071056285e-19,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 22; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's synch21_cs, 13 coefficients.
fn petir_synch_cheb_synch21(x: f32) -> f32 {
    var c = array<f32, 13>(
        38.61783981323242, 23.037715911865234, 5.380249977111816, 0.6156793832778931,
        0.040668800473213196, 0.0017296274891123176, 5.106125900056213e-05,
        1.1045959809052874e-06, 1.8235530419019597e-08, 2.370769691673047e-10,
        2.4887294915870717e-12, 2.1529000844978767e-14, 1.5600000342462522e-16,
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

// GSL's synch22_cs, 13 coefficients.
fn petir_synch_cheb_synch22(x: f32) -> f32 {
    var c = array<f32, 13>(
        7.906314849853516, 3.1353464126586914, 0.4854879379272461, 0.039481665939092636,
        0.0019661621190607548, 6.59078941680491e-05, 1.5857560811127769e-06,
        2.8686530484378636e-08, 4.0412023727398605e-10, 4.556844485081868e-12,
        4.2045898961942316e-14, 3.2320000587343883e-16, 2.100000023830477e-18,
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

// GSL's synch2a_cs, 17 coefficients.
fn petir_synch_cheb_synch2a(x: f32) -> f32 {
    var c = array<f32, 17>(
        2.0203371047973633, 0.010956237092614174, 0.0008542384603060782,
        7.234302756842226e-05, 6.312443019851344e-06, 5.648192882290459e-07,
        5.128324787051497e-08, 4.719653112772448e-09, 4.3807441008070214e-10,
        4.102681389062113e-11, 3.862307175472868e-12, 3.66132308932815e-13,
        3.4802300201853403e-14, 3.3301000794931515e-15, 3.189999930044784e-16,
        3.070000110662777e-17, 3.0000000340435383e-18,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 16; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

const PETIR_SYNCH_SMALL_CUT: f32 = 0.0009765625;
// GSL's f64 value, KEPT -- retargeting it to the f32 analogue (99.813194)
// discards representable denormals. See the header and mirror_synchrotron.
const PETIR_SYNCH_UNDERFLOW_CUT: f32 = 809.5959;
const PETIR_SYNCH_PI_OVER_SQRT3: f32 = 1.8137994;
const PETIR_SYNCH_LOG_SQRT_PI_2: f32 = 0.22579135;

// x^n for the small integer powers upstream takes with gsl_sf_pow_int --
// 11 for S_1, 5 for S_2.
fn petir_synch_pow_int(x: f32, n: u32) -> f32 {
    var out = 1.0;
    for (var k: u32 = 0u; k < n; k = k + 1u) { out = out * x; }
    return out;
}

// S_1(x). NaN for x < 0 and for NaN; 0 past the underflow cut.
fn petir_synchrotron_1(x: f32) -> f32 {
    if (!(x >= 0.0)) { return bitcast<f32>(0x7fc00000u); }
    if (x < PETIR_SYNCH_SMALL_CUT) {
        // Brian Gough's first-order correction to the x^{1/3} series.
        let z = pow(x, 1.0 / 3.0);
        return 2.1495283 * z * (1.0 - 0.84381276 * z * z);
    }
    if (x <= 4.0) {
        let px = pow(x, 1.0 / 3.0);
        let t = x * x / 8.0 - 1.0;
        return px * petir_synch_cheb_synch1(t)
             - petir_synch_pow_int(px, 11u) * petir_synch_cheb_synch2(t)
             - PETIR_SYNCH_PI_OVER_SQRT3 * x;
    }
    if (x < PETIR_SYNCH_UNDERFLOW_CUT) {
        let t = (12.0 - x) / (x + 4.0);
        return sqrt(x) * petir_synch_cheb_synch1a(t) * exp(PETIR_SYNCH_LOG_SQRT_PI_2 - x);
    }
    // Upstream's UNDERFLOW_ERROR. With the f64 bound kept, exp() has already
    // reached zero by the time this is reached, so it is dead code here just
    // as it is in f64 -- which is the point.
    return 0.0;
}

// S_2(x) = x K_{2/3}(x). NaN for x < 0 and for NaN; 0 past the cut.
fn petir_synchrotron_2(x: f32) -> f32 {
    if (!(x >= 0.0)) { return bitcast<f32>(0x7fc00000u); }
    if (x < PETIR_SYNCH_SMALL_CUT) {
        let z = pow(x, 1.0 / 3.0);
        return 1.0747641 * z * (1.0 - 1.1776716 * z * x);
    }
    if (x <= 4.0) {
        let px = pow(x, 1.0 / 3.0);
        let t = x * x / 8.0 - 1.0;
        return px * petir_synch_cheb_synch21(t)
             - petir_synch_pow_int(px, 5u) * petir_synch_cheb_synch22(t);
    }
    if (x < PETIR_SYNCH_UNDERFLOW_CUT) {
        let t = (10.0 - x) / (x + 2.0);
        return sqrt(x) * exp(PETIR_SYNCH_LOG_SQRT_PI_2 - x) * petir_synch_cheb_synch2a(t);
    }
    return 0.0;
}
