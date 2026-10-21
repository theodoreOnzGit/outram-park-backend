// SPDX-License-Identifier: GPL-3.0-only
//
// TRANSCRIBED to WGSL (f32) from `petir::specfunc::sinint`, which ports GSL
// 2.8's specfunc/sinint.c.
//   Copyright (C) 1996-2000 Gerard Jungman; the two small-argument fits come
//   from SLATEC's si.f and ci.f by W. Fullerton.
//
//   Si(x) = integral_0^x sin(t)/t dt
//   Ci(x) = -integral_x^inf cos(t)/t dt
//
// the pair that describes diffraction at a straight edge. Si rises to pi/2
// with a decaying oscillation; Ci starts at -infinity, crosses zero at
// x = 0.6165, and oscillates towards zero.
//
// SELF-CONTAINED: no argument reduction, and that was MEASURED rather than
// assumed. See the note at the foot of this header.
//
// All six tables carry GSL'S SINGLE-PRECISION ORDER: 81 coefficients where
// the f64 order needs 129.
//
// FOUR OF THE SIX TABLES BELONG TO f AND g, NOT TO Si AND Ci. Past x = 4,
//
//   Si(x) = pi/2 - f(x) cos x - g(x) sin x
//   Ci(x) =        f(x) sin x - g(x) cos x
//
// with f ~ 1/x and g ~ 1/x^2. That is not a convenience: it moves the
// oscillation entirely into the sin/cos factors, so what gets fitted is
// smooth and monotone. It also means this kernel's accuracy at large x is
// whatever the device's sin and cos can deliver, and nothing this shader
// does can improve on that.
//
// THREE MACHINE CONSTANTS AND TWO DELETED GUARDS:
//
//   GSL_SQRT_DBL_EPSILON        1.490e-08 -> 3.4526698e-4   PRECISION
//   sqrt(50), the f1/f2 split   7.07106781187 -> 7.0710678  BRANCH BOUNDARY,
//                                                           kept
//   1/GSL_SQRT_DBL_EPSILON      6.711e+07 -> 2896.3093      PRECISION
//
//   1/GSL_DBL_MIN               4.494e+307 -> DELETED       NOT AN f32
//   1/GSL_SQRT_DBL_MIN          6.704e+153 -> DELETED       NOT AN f32
//
// The last two are fermi_dirac.wgsl's third outcome again. Here deleting
// them GAINS answers: the arithmetic reaches the same zeros by itself -- x*x
// overflows above 1.8e19 so 1/x^2 becomes 0 -- and does so later than either
// guard's f32 analogue would.
//
// ============================================================================
// THE ARGUMENT REDUCTION WAS TRIED AND IS ABSENT ON THE EVIDENCE
// ============================================================================
//
// clausen.wgsl's petir_clausen_reduce is the f32 three-way 2*pi split, and
// reusing it here looked obviously right. Measured, it is worse on both
// sides, so this shader calls sin and cos directly.
//
//   ON THE CPU, against the f64 module, Ci's error relative to its own 1/x
//   envelope over [1e4, 1e5]: 1.955e-07 raw against 1.182e-06 reduced, and
//   over [1e5, 5.2e5] 1.652e-07 against 7.993e-06. Si is unaffected either
//   way, because there sin and cos are multiplied by f ~ 1/x against a
//   leading pi/2; Ci IS f*sin - g*cos, so it carries their error at full
//   weight. That asymmetry is why Ci is the instrument here.
//
//   ON THE DEVICE (llvmpipe, LLVM 20.1.2), sin(x) against f64: within 1e-7
//   to about x = 1e7, then 7.4e-4 at 1e8. The reduction returns NaN past its
//   own loss cut of 524288, so above 5.2e5 it is not even available, and
//   below it it is slightly the worse of the two (0.0357484 against the
//   correct 0.0357488 at x = 1e5).
//
// Both libraries already do a proper multi-word reduction internally. What
// actually ends the usable range is neither of them: at x = 1e7 one f32 ulp
// of the ARGUMENT is 1 radian, so sin(x) is not determined by the f32 x at
// all. See mirror_sinint for the range this kernel is honest over.

// GSL's f1_data at its SINGLE-PRECISION order: 11 of the
// 20 stored, where f64 uses 20. See the header.
fn petir_sinint_cheb_f1(x: f32) -> f32 {
    var c = array<f32, 11>(
        -0.11910820007324219, -0.024782314896583557, 0.0011910281609743834,
        -9.270277223549783e-05, 9.337313713331241e-06, -1.1058288009735406e-06,
        1.464772054760033e-07, -2.1069450184540983e-08, 3.229349232469758e-09,
        -5.20652965185775e-10, 8.748789193102624e-11,
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

// GSL's f2_data at its SINGLE-PRECISION order: 15 of the
// 29 stored, where f64 uses 29. See the header.
fn petir_sinint_cheb_f2(x: f32) -> f32 {
    var c = array<f32, 15>(
        -0.03484092652797699, -0.016684221103787422, 0.0006752901244908571,
        -5.350666106096469e-05, 6.269342065934325e-06, -9.526638677925803e-07,
        1.7456292766837578e-07, -3.687954119868664e-08, 8.720268063200365e-09,
        -2.260197140557807e-09, 6.324624712839011e-10, -1.8889119435261392e-10,
        5.967746435908694e-11, -1.980443066484927e-11, 6.86413946515696e-12,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 14; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's g1_data at its SINGLE-PRECISION order: 14 of the
// 21 stored, where f64 uses 21. See the header.
fn petir_sinint_cheb_g1(x: f32) -> f32 {
    var c = array<f32, 14>(
        -0.30405786633491516, -0.056689098477363586, 0.0039046157617121935,
        -0.00037460759631358087, 4.3543153878999874e-05, -5.741729637520621e-06,
        8.282552244054386e-07, -1.2782459180016303e-07, 2.079783456565565e-08,
        -3.531320569294394e-09, 6.210824077257371e-10, -1.1252154763496947e-10,
        2.0908890938087232e-11, -3.971583045075944e-12,
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

// GSL's g2_data at its SINGLE-PRECISION order: 21 of the
// 34 stored, where f64 uses 34. See the header.
fn petir_sinint_cheb_g2(x: f32) -> f32 {
    var c = array<f32, 21>(
        -0.09673293679952621, -0.04520779103040695, 0.0028190005104988813,
        -0.0002899167884606868, 4.074446769664064e-05, -7.105638360371813e-06,
        1.453472350476659e-06, -3.364116594184452e-07, 8.597744027838417e-08,
        -2.3843766072673134e-08, 7.08319047859618e-09, -2.2318067394166974e-09,
        7.401087520619853e-10, -2.567171197842555e-10, 9.267070444352044e-11,
        -3.4669329906922286e-11, 1.339505763253701e-11, -5.32907528869031e-12,
        2.177531158858992e-12, -9.118620572165503e-13, 3.9058640761806263e-13,
    );
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

// GSL's si_data at its SINGLE-PRECISION order: 10 of the
// 12 stored, where f64 uses 12. See the header.
fn petir_sinint_cheb_si(x: f32) -> f32 {
    var c = array<f32, 10>(
        -0.131564661860466, -0.2776578664779663, 0.035441406071186066,
        -0.002563163172453642, 0.0001162365369964391, -3.5904326978197787e-06,
        8.023421571579092e-08, -1.356299739185829e-09, 1.794407157584832e-11,
        -1.90838704973266e-13,
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

// GSL's ci_data at its SINGLE-PRECISION order: 10 of the
// 13 stored, where f64 uses 13. See the header.
fn petir_sinint_cheb_ci(x: f32) -> f32 {
    var c = array<f32, 10>(
        -0.3400428295135498, -1.0330216884613037, 0.19388222694396973,
        -0.019182603806257248, 0.0011078924871981144, -4.1572344343876466e-05,
        1.0927852827080642e-06, -2.123285902655425e-08, 3.1733482508400357e-10,
        -3.761415658110057e-12,
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

const PETIR_SI_SMALL_CUT: f32 = 3.4526698e-4;
// sqrt(50), upstream's rounded decimal, kept verbatim: it is a branch
// boundary, not a machine constant.
const PETIR_SI_XBND: f32 = 7.0710678;
const PETIR_SI_XBIG: f32 = 2896.3093;

// (sin x, cos x), straight from the device. A 2*pi reduction was measured
// and is worse on both CPU and GPU -- see the header.
fn petir_si_sin_cos(x: f32) -> vec2<f32> {
    return vec2<f32>(sin(x), cos(x));
}

// The asymptotic pair (f, g) for x >= 4. Upstream's two far-field guards are
// absent -- neither is an f32; see the header.
fn petir_si_fg_asymp(x: f32) -> vec2<f32> {
    let x2 = x * x;
    if (x <= PETIR_SI_XBND) {
        let t = (1.0 / x2 - 0.04125) / 0.02125;
        return vec2<f32>((1.0 + petir_sinint_cheb_f1(t)) / x,
                         (1.0 + petir_sinint_cheb_g1(t)) / x2);
    }
    if (x <= PETIR_SI_XBIG) {
        let t = 100.0 / x2 - 1.0;
        return vec2<f32>((1.0 + petir_sinint_cheb_f2(t)) / x,
                         (1.0 + petir_sinint_cheb_g2(t)) / x2);
    }
    return vec2<f32>(1.0 / x, 1.0 / x2);
}

// Si(x), odd, bounded by 1.8519. NaN propagates.
fn petir_si(x: f32) -> f32 {
    if (x != x) { return bitcast<f32>(0x7fc00000u); }
    let ax = abs(x);
    if (ax < PETIR_SI_SMALL_CUT) { return x; }
    if (ax <= 4.0) {
        // The 0.75 is carried outside the fit, as in GSL.
        return x * (0.75 + petir_sinint_cheb_si((x * x - 8.0) * 0.125));
    }
    let fg = petir_si_fg_asymp(ax);
    let sc = petir_si_sin_cos(ax);
    let v = 1.5707964 - fg.x * sc.y - fg.y * sc.x;
    if (x < 0.0) { return -v; }
    return v;
}

// Ci(x). NaN for x <= 0 -- upstream's DOMAIN_ERROR, and a real one: Ci has a
// logarithmic singularity at the origin and a branch cut below it.
fn petir_ci(x: f32) -> f32 {
    if (!(x > 0.0)) { return bitcast<f32>(0x7fc00000u); }
    if (x <= 4.0) {
        // ln x carries the singularity; the fit carries the rest.
        return log(x) - 0.5 + petir_sinint_cheb_ci((x * x - 8.0) * 0.125);
    }
    let fg = petir_si_fg_asymp(x);
    let sc = petir_si_sin_cos(x);
    return fg.x * sc.x - fg.y * sc.y;
}
