// SPDX-License-Identifier: GPL-3.0-only
//
// TRANSCRIBED to WGSL (f32) from `petir::specfunc::dilog`, which ports GSL
// 2.8's specfunc/dilog.c.
//   Copyright (C) 1996-2000, 2004 Gerard Jungman.
//
// Li_2(x) = sum_{k>=1} x^k / k^2  =  -integral_0^x ln(1-t)/t dt, for real x.
// GSL's convention: Li_2(1) = pi^2/6, Li_2(0) = 0. Conventions differ; check
// before comparing against a table.
//
// NO COEFFICIENT TABLES. Unlike bessel/psi_zeta/debye this kernel has none --
// it is two convergent series and seven branch identities. So there is no
// generator and no table audit for it, and `mirror_dilog` is written by hand
// beside this file rather than emitted from one parse. The three constants
// that do appear (pi^2/6, pi^2/3, and the 0.01 cut) are checked against each
// other by `every_generated_shader_and_its_mirror_hold_the_same_constants`'s
// sibling test rather than by the table audit.
//
// A PREDICTION THAT WAS REFUTED, AND UPSTREAM'S CONSTANT KEPT BECAUSE OF IT.
//
// petir_dilog_series_2 switches at x = 0.01 between evaluating
// (1-x)ln(1-x)/x directly and using its Taylor expansion through x^8. The
// expectation, by analogy with debye.wgsl where two f64 thresholds HAD to be
// retargeted, was that 0.01 would be far too small in f32: the branch then
// forms 1 + t with t near -1, an 8th-order cancellation, and f32 has seven
// digits to lose rather than sixteen. The Taylor truncation error is ~x^9/10,
// which reaches f32 epsilon only near x = 0.22, so there looked to be two
// decades of headroom.
//
// MEASURED, and it is not so. Worst relative error against the f64 module
// over x in [-15, 15] where |Li_2| > 0.1, as a function of the cut:
//
//   0.01 (upstream, shipped)   4.786e-06
//   0.05                       4.786e-06
//   0.1                        4.063e-06
//   0.2                        4.063e-06
//   0.3                        2.472e-05
//   0.5                        2.472e-05
//
// The best available gain is 1.18x, and past 0.25 the Taylor branch is being
// asked for arguments it was never meant to cover and the error grows
// five-fold. So UPSTREAM'S 0.01 IS KEPT: a 1.18x gain does not justify
// departing from a literal transcription, and the two decades of headroom the
// prediction was built on do not exist.
//
// The general rule this and debye.wgsl together make is "know which of
// upstream's constants still mean something in f32", not "always retarget"
// and not "always keep". debye.wgsl's xcut is derived from f64's EXPONENT
// RANGE, which f32 does not share, so it had to move. This cut is derived
// from a TRUNCATION ORDER against epsilon, and moving it buys nothing because
// the error is dominated elsewhere.
//
// WHERE THE f32 ERROR ACTUALLY IS: the x > 2 branch, and Li_2's zero.
//
// Per branch, worst relative against the f64 module where |Li_2| > 0.1:
//
//   x in (0, 0.25]     3.021e-07      direct power series
//   x in (0.25, 0.5]   6.982e-07      accelerated series
//   x in (0.5, 1)      3.553e-07      Landen reflection
//   x in (1, 1.01]     4.244e-08      the eps/ln(eps) expansion
//   x in (1.01, 2]     1.561e-07      1 - 1/x Landen
//   x in (2, 40]       5.710e-06      inversion            <- the outlier
//   x in [-40, 0)      4.510e-07      duplication
//
// Six of the seven are one f32 ulp. The inversion branch is twenty times
// worse and it is CANCELLATION, not transcription: it forms
// pi^2/3 - Li_2(1/x) - (1/2)ln^2 x, and 0.5*ln^2(x) passes through pi^2/3 at
// x = 12.5952 -- where Li_2 has its real zero. Every implementation in every
// precision loses relative accuracy there; f64 simply has more digits to
// spend. The bounded quantity is the ABSOLUTE error, measured at 1.871e-06
// over x in [-15, 15].
//
// A CALLER WANTING Li_2 NEAR x = 12.6 IN f32 SHOULD EXPECT NO CORRECT DIGITS,
// and that is a property of the formula, not of this port.

const PETIR_DILOG_ZETA2: f32 = 1.6449341;      // pi^2 / 6
const PETIR_DILOG_PI2_3: f32 = 3.2898681;      // pi^2 / 3
const PETIR_DILOG_EPS:   f32 = 1.1920929e-7;   // f32::EPSILON
// The logarithmic/Taylor cut inside petir_dilog_series_2. UPSTREAM'S VALUE,
// kept deliberately -- see the header.
const PETIR_DILOG_TCUT:  f32 = 0.01;

// sum_{k>=1} x^k / k^2, converging usefully only for |x| < 1/2. Upstream's
// dilog_series_1. The term is carried by the ratio ((k-1)/k)^2 rather than
// recomputed, which is upstream's own structure.
fn petir_dilog_series_1(x: f32) -> f32 {
    var sum = x;
    var term = x;
    for (var k: i32 = 2; k < 1000; k = k + 1) {
        let rk = (f32(k) - 1.0) / f32(k);
        term = term * x;
        term = term * rk * rk;
        sum = sum + term;
        if (abs(term / sum) < PETIR_DILOG_EPS) { break; }
    }
    return sum;
}

// sum_{k>=1} r^k / (k^2 (k+1)), upstream's series_2. The first eight terms are
// unconditional before the convergence test begins, which is upstream's own
// two-loop structure: a ratio test against a part-summed series is meaningless
// while the sum is still small.
fn petir_dilog_series_2_raw(r: f32) -> f32 {
    var rk = r;
    var sum = 0.5 * r;
    for (var k: i32 = 2; k < 10; k = k + 1) {
        rk = rk * r;
        sum = sum + rk / (f32(k) * f32(k) * (f32(k) + 1.0));
    }
    for (var k: i32 = 10; k < 100; k = k + 1) {
        rk = rk * r;
        let ds = rk / (f32(k) * f32(k) * (f32(k) + 1.0));
        sum = sum + ds;
        if (abs(ds / sum) < 0.5 * PETIR_DILOG_EPS) { break; }
    }
    return sum;
}

// Li_2(x) = 1 + (1-x) ln(1-x) / x + series_2(x), for -1 < x < 1.
// Every call site below supplies x in [0, 1/2].
fn petir_dilog_series_2(x: f32) -> f32 {
    let s = petir_dilog_series_2_raw(x);
    var t: f32;
    if (x > PETIR_DILOG_TCUT) {
        t = (1.0 - x) * log(1.0 - x) / x;
    } else {
        // Taylor of (1-x)ln(1-x)/x about 0, through x^8.
        let t68 = 1.0 / 6.0 + x * (1.0 / 7.0 + x * (1.0 / 8.0));
        let t38 = 1.0 / 3.0 + x * (1.0 / 4.0 + x * (1.0 / 5.0 + x * t68));
        t = (x - 1.0) * (1.0 + x * (0.5 + x * t38));
    }
    return s + 1.0 + t;
}

// Li_2(x) for x >= 0. Seven branches, upstream's dilog_xge0.
fn petir_dilog_xge0(x: f32) -> f32 {
    if (x > 2.0) {
        let log_x = log(x);
        return PETIR_DILOG_PI2_3 - petir_dilog_series_2(1.0 / x) - 0.5 * log_x * log_x;
    }
    if (x > 1.01) {
        let log_x = log(x);
        let log_term = log_x * (log(1.0 - 1.0 / x) + 0.5 * log_x);
        return PETIR_DILOG_ZETA2 + petir_dilog_series_2(1.0 - 1.0 / x) - log_term;
    }
    if (x > 1.0) {
        // Series about x = 1, in eps AND ln(eps): Li_2 is not analytic there,
        // so a plain Taylor series cannot work.
        let eps = x - 1.0;
        let lne = log(eps);
        let c1 = 1.0 - lne;
        let c2 = -(1.0 - 2.0 * lne) / 4.0;
        let c3 = (1.0 - 3.0 * lne) / 9.0;
        let c4 = -(1.0 - 4.0 * lne) / 16.0;
        let c5 = (1.0 - 5.0 * lne) / 25.0;
        let c6 = -(1.0 - 6.0 * lne) / 36.0;
        let c7 = (1.0 - 7.0 * lne) / 49.0;
        let c8 = -(1.0 - 8.0 * lne) / 64.0;
        return PETIR_DILOG_ZETA2 + eps * (c1 + eps * (c2 + eps * (c3 + eps * (c4
             + eps * (c5 + eps * (c6 + eps * (c7 + eps * c8)))))));
    }
    if (x == 1.0) { return PETIR_DILOG_ZETA2; }
    if (x > 0.5) {
        let log_x = log(x);
        return PETIR_DILOG_ZETA2 - petir_dilog_series_2(1.0 - x) - log_x * log(1.0 - x);
    }
    if (x > 0.25) { return petir_dilog_series_2(x); }
    if (x > 0.0)  { return petir_dilog_series_1(x); }
    return 0.0;
}

// Li_2(x) for real x. Negative arguments go through upstream's duplication
// formula, Li_2(-y) = (1/2) Li_2(y^2) - Li_2(y), so both inner calls take a
// non-negative argument and the recursion is one level deep.
//
// Returns the REAL PART of the principal branch for x > 1; Li_2 is complex
// there and neither GSL nor this reports the imaginary part. NaN propagates.
fn petir_dilog(x: f32) -> f32 {
    if (!(x == x)) { return bitcast<f32>(0x7fc00000u); }
    if (x >= 0.0) { return petir_dilog_xge0(x); }
    return -petir_dilog_xge0(-x) + 0.5 * petir_dilog_xge0(x * x);
}
