// SPDX-License-Identifier: GPL-3.0-only
//
// TRANSCRIBED to WGSL (f32) from `petir::specfunc::expint3`, which ports GSL
// 2.8's specfunc/expint3.c.
//   Copyright (C) 1996-2000 Gerard Jungman.
//
//   Ei_3(x) = integral_0^x e^{-t^3} dt
//
// the cubic member of the family whose quadratic member is (sqrt(pi)/2)
// erf(x). It rises monotonically from the origin to Gamma(4/3) and never
// exceeds it. x >= 0 only: e^{-t^3} GROWS for negative t, so the restriction
// is real and not a sign convention.
//
// EVERY COEFFICIENT WAS EXTRACTED BY SCRIPT from the f64 module and the same
// script emits `wgsl::mirror_expint3`, so the shader and its CPU mirror
// cannot disagree about a constant. Both tables carry GSL'S SINGLE-PRECISION
// ORDER, as dawson.wgsl introduced: 27 coefficients where the f64 order
// needs 47.
//
// THREE MACHINE CONSTANTS, ALL PRECISION, ALL RETARGETED:
//
//   1.6 * GSL_ROOT3_DBL_EPSILON     9.689e-06 -> 7.8745065e-3
//       Below this Ei_3(x) = x to working precision.
//
//   (-GSL_LOG_DBL_EPSILON)^(1/3)    3.303261  -> 2.5168138
//       Past this e^{-x^3} is below one ulp of Gamma(4/3) and the function
//       is declared equal to it. THE f64 VERSION OF THIS CUT IS EXACTLY
//       PLACED -- straddling it by one part in 1e13 gives a bit-identical
//       answer -- which is the standard the retargeted one is measured
//       against in mirror_expint3.
//
//   val_infinity = Gamma(4/3)       0.89297951156924921 -> 0.8929795
//       Not a threshold; the saturation VALUE. Upstream's literal is the
//       correctly rounded f64, and this is its correctly rounded f32.
//
// There is no underflow or overflow guard to decide about: the function is
// bounded by Gamma(4/3) and its only refusal is the x < 0 domain error.

// GSL's expint3_data at its SINGLE-PRECISION order: 16 of the
// 24 stored, where f64 uses 24. See the header.
fn petir_expint3_cheb_expint3(x: f32) -> f32 {
    var c = array<f32, 16>(
        1.2691984176635742, -0.24884644150733948, 0.08052621781826019,
        -0.025772733613848686, 0.007599879056215286, -0.002030695555731654,
        0.0004908345872536302, -0.00010768223728518933, 2.155172660422977e-05,
        -3.956705313612474e-06, 6.699240771013137e-07, -1.0513218029473137e-07,
        1.5362580541022908e-08, -2.0990960081235244e-09, 2.6921095908072346e-10,
        -3.25195252670607e-11,
    );
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

// GSL's expint3a_data at its SINGLE-PRECISION order: 11 of the
// 23 stored, where f64 uses 23. See the header.
fn petir_expint3_cheb_expint3a(x: f32) -> f32 {
    var c = array<f32, 11>(
        1.927046537399292, -0.03492935746908188, 0.001450338400900364,
        -8.925336442189291e-05, 7.054239176795818e-06, -6.671727419416129e-07,
        7.242675792440423e-08, -8.782582661126526e-09, 1.1672234290216466e-09,
        -1.676631333769052e-10, 2.5755016175299517e-11,
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

// 1.6 * cbrt(f32::EPSILON).
const PETIR_E3_SMALL_CUT: f32 = 7.8745065e-3;
// (-ln(f32::EPSILON))^(1/3).
const PETIR_E3_LOG_CUT: f32 = 2.5168138;
// Gamma(4/3), the value of the integral over the whole half-line.
const PETIR_E3_VAL_INFINITY: f32 = 0.8929795;

// Ei_3(x). NaN for x < 0 and for NaN -- upstream's DOMAIN_ERROR.
fn petir_expint_3(x: f32) -> f32 {
    if (!(x >= 0.0)) { return bitcast<f32>(0x7fc00000u); }
    if (x < PETIR_E3_SMALL_CUT) { return x; }
    if (x <= 2.0) {
        return x * petir_expint3_cheb_expint3(x * x * x / 4.0 - 1.0);
    }
    if (x < PETIR_E3_LOG_CUT) {
        let t = 16.0 / (x * x * x) - 1.0;
        let s = exp(-(x * x * x)) / (3.0 * x * x);
        return PETIR_E3_VAL_INFINITY - petir_expint3_cheb_expint3a(t) * s;
    }
    return PETIR_E3_VAL_INFINITY;
}
