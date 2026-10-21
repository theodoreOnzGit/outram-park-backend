// SPDX-License-Identifier: GPL-3.0-only
//
// TRANSCRIBED to WGSL (f32) from `petir::specfunc::dawson`, which ports GSL
// 2.8's specfunc/dawson.c.
//   Copyright (C) 1996-2000 Gerard Jungman; the Chebyshev fits come from
//   SLATEC's daws.f by W. Fullerton.
//
//   F(x) = e^{-x^2} integral_0^x e^{t^2} dt
//
// the odd, bounded companion of erf: it rises to 0.5410442 at x = 0.9241389
// and decays like 1/(2x). It is what exp(x^2) erf(x) would be if that product
// did not overflow, and the imaginary part of the Faddeeva function on the
// real axis.
//
// EVERY COEFFICIENT WAS EXTRACTED BY SCRIPT from the f64 module and the same
// script emits `wgsl::mirror_dawson`, so the shader and its CPU mirror cannot
// disagree about a constant.
//
// ============================================================================
// THE FIRST SHADER HERE TO USE GSL'S SINGLE-PRECISION ORDER
// ============================================================================
//
// GSL's cheb_series struct has FIVE fields and the fifth is order_sp -- the
// order to use in single precision, which GSL_MODE_SINGLE selects inside
// cheb_eval_mode:
//
//   static cheb_series dawa_cs = { dawa_data, 34, /* 74, */ -1, 1, 12 };
//                                              ^^                    ^^
//                                          f64 order            order_sp
//
// Every earlier shader in this module ships the f64 order. This one ships
// order_sp, which is upstream's own answer for exactly this width -- porting,
// not inventing a truncation. The cost:
//
//   table       stored   f64 uses   order_sp   answer-level cost
//   daw_data        21         16         10   measured in mirror_dawson
//   daw2_data       45         33         22
//   dawa_data       75         35         13
//
// 45 coefficients where the f64 order would need 84, and where the raw arrays
// hold 141. dawa alone falls from 35 to 13.
//
// That matters beyond size: debye.wgsl measured that series held as INLINE
// array<f32, N> literals do not reproduce the CPU bit for bit where
// storage-buffer coefficients do -- 34/64 against 64/64 -- and every series
// here is inline. Shortening them is a direct experiment on that.
//
// ============================================================================
// THREE MACHINE CONSTANTS, AND THE THIRD CANNOT BE WRITTEN DOWN
// ============================================================================
//
//   1.225 * SQRT_DBL_EPSILON   1.825e-08 -> 4.2295e-4     PRECISION,
//                                                         retargeted
//   1/(sqrt2 * SQRT_DBL_EPS)   4.745e+07 -> 2048.0        PRECISION,
//                                                         retargeted --
//       and it lands on exactly 2^11, the same way fermi_dirac's small cut
//       landed on 2^-10: 2^{-1/2} * 2^{23/2} = 2^11 by algebra.
//
//   0.1 * GSL_DBL_MAX          1.798e+307 -> DELETED      RANGE GUARD, NOT
//                                                         REPRESENTABLE
//       f32::MAX is 3.4e38, so 0.1*DBL_MAX cannot be an f32 at all -- the
//       same third outcome fermi_dirac.wgsl's overflow bounds met. The branch
//       is removed and 0.5/x is left to underflow on its own, which it does
//       only past 3.4e38 and into denormals that are perfectly representable.
//       Deleting the guard therefore GAINS answers here rather than losing
//       them.

// GSL's daw_data at its SINGLE-PRECISION order: 10 of the
// 21 stored, where f64 uses 16. See the header.
fn petir_dawson_cheb_daw(x: f32) -> f32 {
    var c = array<f32, 10>(
        -0.006351734511554241, -0.2294071465730667, 0.02213050052523613,
        -0.001549265463836491, 8.49732750793919e-05, -3.828266471828101e-06,
        1.462854868350405e-07, -4.85198237143436e-09, 1.4214636412379633e-10,
        -3.728836250188605e-12,
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

// GSL's daw2_data at its SINGLE-PRECISION order: 22 of the
// 45 stored, where f64 uses 33. See the header.
fn petir_dawson_cheb_daw2(x: f32) -> f32 {
    var c = array<f32, 22>(
        -0.056886542588472366, -0.3181134760379791, 0.20873846113681793,
        -0.12475410103797913, 0.06786930561065674, -0.03365914523601532,
        0.015260781161487103, -0.006348371040076017, 0.0024326741695404053,
        -0.0008621953893452883, 0.0002837657229974866, -8.70575531735085e-05,
        2.4986849894048646e-05, -6.7319288064027205e-06, 1.7078579048757092e-06,
        -4.09175498816694e-07, 9.282829438461704e-08, -1.9991404087704723e-08,
        4.096349037752134e-09, -8.003240847820337e-10, 1.4938503212214016e-10,
        -2.668799903293717e-11,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 21; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's dawa_data at its SINGLE-PRECISION order: 13 of the
// 75 stored, where f64 uses 35. See the header.
fn petir_dawson_cheb_dawa(x: f32) -> f32 {
    var c = array<f32, 13>(
        0.016904857009649277, 0.008683252148330212, 0.00024248640693258494,
        1.2611823876795825e-05, 1.066453364728659e-06, 1.358159806841286e-07,
        2.1710423681042812e-08, 2.8670104068595492e-09, -1.9013364493947194e-10,
        -3.097780365557412e-10, -1.0294148866663022e-10, -6.260356382598031e-12,
        8.563132841699073e-12,
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

// 1.225 * sqrt(f32::EPSILON).
const PETIR_DAW_XSML: f32 = 4.2295206e-4;
// 1/(sqrt(2) * sqrt(f32::EPSILON)), exactly 2^11.
const PETIR_DAW_XBIG: f32 = 2048.0;

// F(x). NaN propagates; there is no refusal and no overflow.
fn petir_dawson(x: f32) -> f32 {
    if (x != x) { return bitcast<f32>(0x7fc00000u); }
    let y = abs(x);
    if (y < PETIR_DAW_XSML) { return x; }
    if (y < 1.0) {
        // The 0.75 is carried outside the fit so the series has no constant
        // term to lose precision on, as upstream does.
        return x * (0.75 + petir_dawson_cheb_daw(2.0 * y * y - 1.0));
    }
    if (y < 4.0) {
        return x * (0.25 + petir_dawson_cheb_daw2(0.125 * y * y - 1.0));
    }
    if (y < PETIR_DAW_XBIG) {
        return (0.5 + petir_dawson_cheb_dawa(32.0 / (y * y) - 1.0)) / x;
    }
    // Upstream's 0.1 * DBL_MAX underflow guard is not an f32; 0.5/x carries
    // on into denormals instead. See the header.
    return 0.5 / x;
}
