// SPDX-License-Identifier: GPL-3.0-only
//
// TRANSCRIBED to WGSL (f32) from `petir::specfunc::atanint`, which ports GSL
// 2.8's specfunc/atanint.c.
//   Copyright (C) 1996-2000 Gerard Jungman.
//
// Ti_2(x) = integral_0^x arctan(t)/t dt = sum (-1)^k x^{2k+1}/(2k+1)^2.
// Odd, unbounded only logarithmically, and Ti_2(1) is Catalan's constant --
// which it shares with clausen.wgsl, its even counterpart.
//
// EVERY COEFFICIENT WAS EXTRACTED BY SCRIPT from the f64 module, whose own
// table is audited bit-for-bit against GSL's source.
//
// ONE TABLE, TWO BRANCHES, RECIPROCAL ARGUMENTS. For |x| <= 1 the series is
// evaluated at 2(x^2 - 1/2); above 1 at 2(1/x^2 - 1/2), combined with
// (pi/2) ln|x|. That is the Landen-type reflection
// Ti_2(x) - Ti_2(1/x) = (pi/2) ln x built into the branch structure rather
// than applied on top, so satisfying it inside the branch is close to a
// tautology -- what is worth testing is that the branches JOIN.
//
// TWO MACHINE CONSTANTS ARE RETARGETED, both precision constants, per the
// taxonomy in docs/wgsl-coverage.md:
//
//   0.5 sqrt(DBL_EPSILON)   7.45e-09  ->  1.7263349e-4
//   1 / sqrt(DBL_EPSILON)   6.71e+07  ->  2896.3093
//
// A PREDICTION RECORDED AND REFUTED. The large cut was expected to RESHAPE
// THE DOMAIN: above it the series at 1/x^2 has collapsed and the answer is
// (pi/2) ln|x| + 1/x, and in f32 that arrives 23170 times sooner than in f64,
// so a far larger share of the domain takes the closed form -- a change in
// WHICH CODE RUNS.
//
// It changes which code runs and NOTHING ELSE. Over 4000 probes spanning the
// whole disputed window, 2896.3 to 6.711e+07:
//
//   answers that differ                    0 of 4000   (bit-identical)
//   worst rel vs f64, shipped cut          1.594855996429876e-07
//   worst rel vs f64, GSL's f64 cut        1.594855996429876e-07
//
// AND THE REASON IS NOT ROUNDING, which two drafts of this note claimed. The
// series argument 2(1/x^2 - 1/2) never reaches exactly -1 near the cut:
// -0.99999976 at x = 2900, still -0.99999994 at x = 6000, because the f32
// spacing just below 0.5 is half that just above. What actually makes the
// branches agree is that the series enters as cheb(t)/|x| against a
// (pi/2) ln|x| term that dominates, and the gap between cheb(t) and cheb(-1)
// divided by |x| falls BELOW ONE ULP of that sum. The series is not
// collapsing; its variation is being rounded away by the addition.
//
// So retargeting is correct and its only effect is that the closed-form
// branch skips a 21-term Clenshaw spent on a foregone conclusion -- the same
// WORK-ONLY category as transport.wgsl, established here by bit equality
// rather than by a bound.

const PETIR_ATANINT_SMALL_CUT: f32 = 1.7263349e-4;
const PETIR_ATANINT_LARGE_CUT: f32 = 2896.3093;
const PETIR_ATANINT_PI_2: f32 = 1.5707964;

// GSL's `atanint_data` at its SINGLE-PRECISION order: 11 of the 21 stored,
// where `f64` evaluates 21.
fn petir_atanint_cheb(x: f32) -> f32 {
    var c = array<f32, 11>(
        1.9104036092758179, -0.041763514280319214, 0.002753925509750843,
        -0.00025051808916032314, 2.666981345100794e-05, -3.118905169685604e-06,
        3.8833852045172534e-07, -5.0572744214605336e-08, 6.812252983934286e-09,
        -9.421255997565936e-10, 1.3307878410362406e-10,
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

// Ti_2(x) for all real x. NaN propagates; there is no domain error and no
// overflow.
fn petir_atanint(x: f32) -> f32 {
    if (!(x == x)) { return bitcast<f32>(0x7fc00000u); }
    let ax = abs(x);
    var sgn = 1.0;
    if (x < 0.0) { sgn = -1.0; }

    if (ax == 0.0) { return 0.0; }
    if (ax < PETIR_ATANINT_SMALL_CUT) {
        // Ti_2(x) ~ x, the series' first term.
        return x;
    }
    if (ax <= 1.0) {
        return x * petir_atanint_cheb(2.0 * (x * x - 0.5));
    }
    if (ax < PETIR_ATANINT_LARGE_CUT) {
        let t = 2.0 * (1.0 / (x * x) - 0.5);
        return sgn * (PETIR_ATANINT_PI_2 * log(ax) + petir_atanint_cheb(t) / ax);
    }
    // The series at 1/x^2 has collapsed to its leading term, 1.
    return sgn * (PETIR_ATANINT_PI_2 * log(ax) + 1.0 / ax);
}
