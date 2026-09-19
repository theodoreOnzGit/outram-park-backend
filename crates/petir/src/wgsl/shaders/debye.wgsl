// SPDX-License-Identifier: GPL-3.0-only
//
// TRANSCRIBED to WGSL (f32) from `petir::specfunc::debye`, which ports GSL
// 2.8's specfunc/debye.c.
//   Copyright (C) 1996-2000 Gerard Jungman; (C) 2007 Brian Gough (orders 5-6).
//
// D_n(x) = (n / x^n) integral_0^x t^n / (e^t - 1) dt, orders 1 through 6.
// D_3 is the Debye heat-capacity function.
//
// EVERY COEFFICIENT WAS EXTRACTED BY SCRIPT from the f64 module -- 102
// literals across six Chebyshev series -- and the same script emits
// `wgsl::mirror_debye`, so the shader and its CPU mirror cannot disagree
// about a constant.
//
// ONE DELIBERATE DEPARTURE FROM UPSTREAM'S ARITHMETIC, AND IT IS MEASURED.
//
// GSL's exponential-sum branch walks a counter down the loop:
//
//     xk = nexp * x;  ...  xk -= x;     (specfunc/debye.c)
//
// so that xk is rk*x at every step. In f64 the accumulated rounding over a
// few hundred subtractions is ~1e-13 and cannot matter. In f32 it is not: at
// x = 4.1, n = 6, xk starts near 705 where one ulp is 6.1e-05, and 171
// subtractions leave it wrong by ~1e-04 by the time rk reaches the small
// values where the falling-factorial polynomial is steepest. The branch then
// subtracts two nearly equal quantities -- the leading term is 0.9170 and the
// sum contributes 0.8012 -- so an 8x cancellation amplifies that drift into
// the answer.
//
// Measured against the f64 module over x in (4.1, 15.2] at 0.01 spacing,
// worst relative, for all four combinations of the two departures:
//
//   order   xk -= x      xk -= x      xk = rk * x
//           f64 cuts     f32 cuts     EITHER cut
//   D_1     4.093e-06    1.159e-07    1.028e-07
//   D_2     3.994e-05    5.397e-07    1.423e-07
//   D_3     1.887e-04    2.640e-06    2.569e-07
//   D_4     6.720e-04    8.860e-06    3.725e-07
//   D_5     2.120e-03    2.785e-05    8.841e-07
//   D_6     6.401e-03    8.375e-05    1.940e-06
//
// Read the first column first: it is GSL's own arithmetic, transcribed
// literally, and at D_6 it is wrong in the third significant figure. So this
// recomputes xk = rk * x, at a cost of one multiply per iteration. It is a
// departure from a line-by-line transcription, taken because the f64
// formulation is not numerically adequate at f32 width, and it is recorded
// here rather than made silently.
//
// THE TWO MACHINE CONSTANTS ARE ALSO RETARGETED, and the table shows the two
// departures are NOT independent. While xk is decremented the retargeting is
// worth a factor of ~50, because a shorter loop accumulates less drift.
// Once xk is recomputed the f32 and f64 cuts give BIT-IDENTICAL answers --
// which is why the right-hand column above is one column and not two -- and
// the retargeting becomes a PERFORMANCE choice and nothing more. GSL's xcut
// is -GSL_LOG_DBL_MIN = 708.4, where exp(-x) underflows f64; f32 underflows
// at 87.34, so at x = 4.1 the loop runs 21 times instead of 172 for the same
// result.
//
//   xcut          708.4   -> 87.33654   (-ln of the smallest normal)
//   sum/closed    35.3505 -> 15.24924   (-(ln 2 + ln eps))
//
// THIS KERNEL IS NOT BIT-IDENTICAL TO ITS CPU MIRROR, although its worst
// branch is pure arithmetic. The x <= 4 Chebyshev branch calls no builtin, so
// docs/wgsl-coverage.md's general rule says it should match exactly; it misses
// by up to 4 ulp. The cause is the inline array<f32, N> literal below, not
// the recurrence -- fed the same 17 values through a storage buffer instead,
// the same Clenshaw is bit-exact at 64 of 64 points, and inline it is exact at
// 34. See the_inline_coefficient_array_is_what_costs_bit_identity in
// tests/wgsl_gpu.rs, which is the controlled A/B, and the corrected rule in
// the coverage ledger.
//
// `bessel.wgsl` keeps GSL's f64 thresholds, and that is not an inconsistency:
// there they degenerate harmlessly, so keeping them is the more literal
// transcription at no cost. Here they cost an 8x longer loop. The rule is
// "know which of upstream's constants still mean something in f32", not
// "always keep them".
//
// WHAT f32 COSTS. The Chebyshev branch (x <= 4) is pure arithmetic and could
// be held to bit-identity; the sum and closed-form branches call exp(), whose
// WGSL specification is an ULP bound rather than correct rounding, so they
// cannot. Measured figures are in `wgsl::mirror_debye`.

// GSL's `adeb1_data` at its SINGLE-PRECISION order: 10 of the 17 stored,
// where `f64` evaluates 17.
fn petir_debye_cheb1(x: f32) -> f32 {
    var c = array<f32, 10>(
        2.40065972, 0.193721304, -0.00623291246, 0.000351117477, -2.28222467e-05,
        1.58054679e-06, -1.1353782e-07, 8.35833612e-09, -6.26442479e-10, 4.76033489e-11,
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

// GSL's `adeb2_data` at its SINGLE-PRECISION order: 11 of the 18 stored,
// where `f64` evaluates 18.
fn petir_debye_cheb2(x: f32) -> f32 {
    var c = array<f32, 11>(
        2.59438102, 0.28633572, -0.0102062656, 0.000604910978, -4.05257659e-05,
        2.86338263e-06, -2.0863943e-07, 1.55237876e-08, -1.17312801e-09, 8.97358589e-11,
        -6.9317614e-12,
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

// GSL's `adeb3_data` at its SINGLE-PRECISION order: 11 of the 17 stored,
// where `f64` evaluates 17.
fn petir_debye_cheb3(x: f32) -> f32 {
    var c = array<f32, 11>(
        2.70773707, 0.340068135, -0.0129451502, 0.000796375538, -5.4636001e-05,
        3.92430196e-06, -2.89403282e-07, 2.17317614e-08, -1.65421e-09, 1.27279619e-10,
        -9.8796346e-12,
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

// GSL's `adeb4_data` at its SINGLE-PRECISION order: 11 of the 17 stored,
// where `f64` evaluates 17.
fn petir_debye_cheb4(x: f32) -> f32 {
    var c = array<f32, 11>(
        2.78186942, 0.374976784, -0.0149409074, 0.000945679811, -6.61329161e-05,
        4.81563298e-06, -3.58808396e-07, 2.71601187e-08, -2.08070991e-09, 1.60938387e-10,
        -1.25470979e-11,
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

// GSL's `adeb5_data` at its SINGLE-PRECISION order: 11 of the 17 stored,
// where `f64` evaluates 17.
fn petir_debye_cheb5(x: f32) -> f32 {
    var c = array<f32, 11>(
        2.83402695, 0.399409886, -0.0164566765, 0.00106521383, -7.56730375e-05,
        5.57459852e-06, -4.19069233e-07, 3.19456144e-08, -2.46133182e-09, 1.91280163e-10,
        -1.49720049e-11,
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

// GSL's `adeb6_data` at its SINGLE-PRECISION order: 11 of the 17 stored,
// where `f64` evaluates 17.
fn petir_debye_cheb6(x: f32) -> f32 {
    var c = array<f32, 11>(
        2.87267271, 0.417437535, -0.0176453849, 0.00116298527, -8.37118027e-05,
        6.22836116e-06, -4.71864447e-07, 3.61950398e-08, -2.8030368e-09, 2.18768198e-10,
        -1.71857387e-11,
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

// Dispatch to the right Chebyshev series. WGSL has no function pointers.
fn petir_debye_cheb(n: u32, x: f32) -> f32 {
    if (n == 1u) { return petir_debye_cheb1(x); }
    if (n == 2u) { return petir_debye_cheb2(x); }
    if (n == 3u) { return petir_debye_cheb3(x); }
    if (n == 4u) { return petir_debye_cheb4(x); }
    if (n == 5u) { return petir_debye_cheb5(x); }
    if (n == 6u) { return petir_debye_cheb6(x); }
    return bitcast<f32>(0x7fc00000u);
}

// n!/(n-k)! for k = 0 ..= n, ascending -- GSL's own falling-factorial rows,
// written out per order as debye.c writes them, not derived from a formula.
fn petir_debye_fall(n: u32, u: f32) -> f32 {
    // Horner descending in u: 1 + n u + n(n-1) u^2 + ...
    if (n == 1u) { return ((0.0 * u + 1.0) * u + 1.0); }
    if (n == 2u) { return (((0.0 * u + 2.0) * u + 2.0) * u + 1.0); }
    if (n == 3u) { return ((((0.0 * u + 6.0) * u + 6.0) * u + 3.0) * u + 1.0); }
    if (n == 4u) { return (((((0.0 * u + 24.0) * u + 24.0) * u + 12.0) * u + 4.0) * u + 1.0); }
    if (n == 5u) { return ((((((0.0 * u + 120.0) * u + 120.0) * u + 60.0) * u + 20.0) * u + 5.0) * u + 1.0); }
    if (n == 6u) { return (((((((0.0 * u + 720.0) * u + 720.0) * u + 360.0) * u + 120.0) * u + 30.0) * u + 6.0) * u + 1.0); }
    return bitcast<f32>(0x7fc00000u);
}

// Same coefficients as a polynomial in x, for the closed-form branch.
fn petir_debye_fall_x(n: u32, x: f32) -> f32 {
    if (n == 1u) { return ((0.0 * x + 1.0) * x + 1.0); }
    if (n == 2u) { return (((0.0 * x + 1.0) * x + 2.0) * x + 2.0); }
    if (n == 3u) { return ((((0.0 * x + 1.0) * x + 3.0) * x + 6.0) * x + 6.0); }
    if (n == 4u) { return (((((0.0 * x + 1.0) * x + 4.0) * x + 12.0) * x + 24.0) * x + 24.0); }
    if (n == 5u) { return ((((((0.0 * x + 1.0) * x + 5.0) * x + 20.0) * x + 60.0) * x + 120.0) * x + 120.0); }
    if (n == 6u) { return (((((((0.0 * x + 1.0) * x + 6.0) * x + 30.0) * x + 120.0) * x + 360.0) * x + 720.0) * x + 720.0); }
    return bitcast<f32>(0x7fc00000u);
}

const PETIR_DEBYE_XCUT: f32 = 87.33654470871734;
const PETIR_DEBYE_SUMCUT: f32 = 15.249237968550476;

fn petir_debye_vinf(n: u32) -> f32 {
    if (n == 1u) { return 1.64493407; }
    if (n == 2u) { return 4.80822761; }
    if (n == 3u) { return 19.4818182; }
    if (n == 4u) { return 99.5450645; }
    if (n == 5u) { return 610.405837; }
    if (n == 6u) { return 4356.06888; }
    return bitcast<f32>(0x7fc00000u);
}

fn petir_debye_lin(n: u32) -> f32 {
    if (n == 1u) { return 0.25; }
    if (n == 2u) { return 0.333333333; }
    if (n == 3u) { return 0.375; }
    if (n == 4u) { return 0.4; }
    if (n == 5u) { return 0.416666667; }
    if (n == 6u) { return 0.428571429; }
    return bitcast<f32>(0x7fc00000u);
}

fn petir_debye_quad(n: u32) -> f32 {
    if (n == 1u) { return 0.0277777778; }
    if (n == 2u) { return 0.0416666667; }
    if (n == 3u) { return 0.05; }
    if (n == 4u) { return 0.0555555556; }
    if (n == 5u) { return 0.0595238095; }
    if (n == 6u) { return 0.0625; }
    return bitcast<f32>(0x7fc00000u);
}

// The f32 small-argument cut. D_1 uses 2 sqrt(eps); the rest 2 sqrt(2)
// sqrt(eps) -- upstream's own asymmetry, retargeted to f32's epsilon.
fn petir_debye_small_cut(n: u32) -> f32 {
    let se = 3.4526698e-4;  // sqrt(f32::EPSILON)
    if (n == 1u) { return 2.0 * se; }
    return 2.0 * 1.4142135623730951 * se;
}

fn petir_debye_pow_n(x: f32, n: u32) -> f32 {
    var out = 1.0;
    for (var k: u32 = 0u; k < n; k = k + 1u) { out = out * x; }
    return out;
}

// D_n(x) for n in 1 ..= 6. NaN for x < 0 or an untabulated order.
fn petir_debye(n: u32, x: f32) -> f32 {
    if (n == 0u || n > 6u) { return bitcast<f32>(0x7fc00000u); }
    if (!(x >= 0.0)) { return bitcast<f32>(0x7fc00000u); }
    let lin = petir_debye_lin(n);
    if (x < petir_debye_small_cut(n)) {
        return 1.0 - lin * x + petir_debye_quad(n) * x * x;
    }
    if (x <= 4.0) {
        return petir_debye_cheb(n, x * x / 8.0 - 1.0) - lin * x;
    }
    let vinf = petir_debye_vinf(n);
    if (x < PETIR_DEBYE_SUMCUT) {
        let nexp = floor(PETIR_DEBYE_XCUT / x);
        let ex = exp(-x);
        var sum = 0.0;
        var rk = nexp;
        var i = nexp;
        loop {
            if (i < 1.0) { break; }
            // xk = rk * x, RECOMPUTED rather than decremented -- see the
            // header. Upstream carries `xk -= x` down the loop, which is
            // exact enough in f64 and drifts badly in f32.
            let xk = rk * x;
            sum = sum * ex;
            sum = sum + petir_debye_fall(n, 1.0 / xk) / rk;
            rk = rk - 1.0;
            i = i - 1.0;
        }
        return vinf / petir_debye_pow_n(x, n) - f32(n) * sum * ex;
    }
    if (x < PETIR_DEBYE_XCUT) {
        return (vinf - f32(n) * petir_debye_fall_x(n, x) * exp(-x))
             / petir_debye_pow_n(x, n);
    }
    return vinf / petir_debye_pow_n(x, n);
}
