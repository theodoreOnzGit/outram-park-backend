// SPDX-License-Identifier: GPL-3.0-only
//
// TRANSCRIBED to WGSL (f32) from `petir::specfunc::lambert`, which ports GSL
// 2.8's specfunc/lambert.c.
//   Copyright (C) 2007 Brian Gough; from code donated by K. Briggs.
//   Lambert-ology from Corless, Gonnet, Hare and Jeffrey, "On Lambert's W
//   Function"; the Halley step is that paper's equation 5.12.
//
// W(x) solves W e^W = x. Two real branches: W_0 on [-1/e, inf) with values
// >= -1, and W_{-1} on [-1/e, 0) with values <= -1. They meet at x = -1/e,
// where W = -1 and dW/dx is infinite.
//
// THE ITERATION IS NOT PURELY HALLEY, AND THAT IS UPSTREAM'S CHOICE. For
// w > 0 it takes a plain Newton step; only for w <= 0 does it take the Halley
// step with its second-order correction. On the positive branch p = w+1 is
// bounded away from zero and Newton converges on its own; near w = -1 the
// Halley denominator is what keeps the step finite.
//
// UPSTREAM'S STOPPING RULE IS WRONG TWICE OVER AT f32 WIDTH, in two
// different ways. The rule is  |t| < 10 * eps * max(|w|, 1/(|p| e^w)).
//
// 1. eps IS A PRECISION CONSTANT AND IS RETARGETED to f32::EPSILON
//    (1.19e-07). With DBL_EPSILON (2.2e-16) an f32 iterate can never satisfy
//    the test at all: measured over 200 W_0 probes, 142 burn the full
//    iteration budget and the other 58 stop only because their step became
//    EXACTLY ZERO -- all 58, checked -- which satisfies any positive
//    tolerance. The rule never terminates the loop; the loop runs out, or
//    the arithmetic does.
//
// 2. THE 1/(|p| e^w) TERM IS DROPPED, because it makes the tolerance O(1).
//    Once w is very negative e^w is tiny and the term explodes: at
//    x = -2.1e-06, w is about -15.8 and the tolerance comes out at 5.947e-01
//    against 1.885e-05 without it -- over a hundred times the 0.005 step it
//    then accepts, stopping a full iteration early with w wrong in the third
//    decimal. In f64 the same expression gives 2.9e-10 and is harmless,
//    which is why upstream carries it.
//
//    Measured over the whole W_{-1} domain down to x = -1e-07:
//      upstream's rule, eps retargeted:  4.769e-03 worst, 7 iterations
//      10 * eps * |w|          SHIPPED:  9.982e-07 worst, 8 iterations
//    One extra iteration for three and a half orders, with W_0 untouched at
//    3.071e-07 either way. Safe near the branch point because the iteration
//    is not entered there -- the sqrt(q) series handles it.
//
// THESE ARE TWO DIFFERENT KINDS OF DECISION, and airy.wgsl makes a third.
// Point 1 retargets a PRECISION CONSTANT whose f64 value is unreachable.
// Point 2 changes a FORMULA that is numerically inadequate at f32 width, as
// debye.wgsl does for its xk counter. airy.wgsl KEEPS GSL's f64 overflow
// threshold, because that is a RANGE GUARD and retargeting it would discard
// representable answers. Know what the constant is FOR.
//
// THE ITERATION COUNTS ARE UPSTREAM'S (10 for W_0, 32 for W_{-1}) and are
// kept. In f32 convergence is reached far sooner, so they are ceilings that
// are not approached rather than tuning.
//
// NO COEFFICIENT TABLES except the 12-term series near the branch point,
// which is held inline; see the note in airy.wgsl about what that costs in
// GPU/CPU bit-identity.

const PETIR_LAMBERT_ONE_OVER_E: f32 = 0.36787945;
const PETIR_LAMBERT_E: f32 = 2.7182817;
// f32::EPSILON -- RETARGETED from upstream's DBL_EPSILON, see the header.
const PETIR_LAMBERT_EPS: f32 = 1.1920929e-7;

// The series in r = sqrt(q) near the branch point, q = x + 1/e. Both branches
// share it and differ only in the sign of r: W_0 takes +sqrt(q), W_{-1} takes
// -sqrt(q), which is the two sides of the square-root branch point.
// Upstream's nested grouping is reproduced; a different association would
// give a different last bit.
fn petir_lambert_series(r: f32) -> f32 {
    let c0  = -1.0;
    let c1  = 2.331644;
    let c2  = -1.8121879;
    let c3  = 1.9366311;
    let c4  = -2.3535512;
    let c5  = 3.066859;
    let c6  = -4.1753354;
    let c7  = 5.8580236;
    let c8  = -8.401032;
    let c9  = 12.250754;
    let c10 = -18.100697;
    let c11 = 27.029045;
    let t8 = c8 + r * (c9 + r * (c10 + r * c11));
    let t5 = c5 + r * (c6 + r * (c7 + r * t8));
    let t1 = c1 + r * (c2 + r * (c3 + r * (c4 + r * t5)));
    return c0 + r * t1;
}

// Upstream's halley_iteration. Returns NaN if it does not converge, which
// upstream reports as GSL_EMAXITER and comments "should never get here".
fn petir_lambert_halley(x: f32, w_initial: f32, max_iters: i32) -> f32 {
    var w = w_initial;
    for (var i: i32 = 0; i < max_iters; i = i + 1) {
        let e = exp(w);
        let p = w + 1.0;
        var t = w * e - x;
        if (w > 0.0) {
            t = (t / p) / e;                       // Newton
        } else {
            t = t / (e * p - 0.5 * (p + 1.0) * t / p);  // Halley
        }
        w = w - t;
        // UPSTREAM'S RULE IS 10 * eps * max(|w|, 1/(|p| e^w)). The second
        // term is DROPPED -- see the header and mirror_lambert. It makes the
        // tolerance O(1) once w is very negative, and the loop then stops one
        // iteration early with w wrong in the third decimal.
        let tol = 10.0 * PETIR_LAMBERT_EPS * abs(w);
        if (abs(t) < tol) {
            return w;
        }
    }
    return bitcast<f32>(0x7fc00000u);
}

// The principal branch W_0(x), for x >= -1/e. NaN below that -- upstream
// returns -1.0 with GSL_EDOM, and a caller of its natural-prototype form
// cannot tell that from an answer. See mirror_lambert.
fn petir_lambert_w0(x: f32) -> f32 {
    if (!(x == x)) { return bitcast<f32>(0x7fc00000u); }
    let q = x + PETIR_LAMBERT_ONE_OVER_E;
    if (x == 0.0) { return 0.0; }
    if (q < 0.0)  { return bitcast<f32>(0x7fc00000u); }
    if (q == 0.0) { return -1.0; }
    if (q < 1.0e-3) { return petir_lambert_series(sqrt(q)); }
    var w: f32;
    if (x < 1.0) {
        let p = sqrt(2.0 * PETIR_LAMBERT_E * q);
        w = -1.0 + p * (1.0 + p * (-1.0 / 3.0 + p * 11.0 / 72.0));
    } else {
        w = log(x);
        if (x > 3.0) { w = w - log(w); }
    }
    return petir_lambert_halley(x, w, 10);
}

// The secondary real branch W_{-1}(x), for -1/e <= x < 0. Delegates to W_0
// for x > 0, which is upstream's behaviour -- there is no second real branch
// there. x == 0 returns 0 by the same convention, although the true limit
// along this branch is -inf.
fn petir_lambert_wm1(x: f32) -> f32 {
    if (!(x == x)) { return bitcast<f32>(0x7fc00000u); }
    if (x > 0.0)  { return petir_lambert_w0(x); }
    if (x == 0.0) { return 0.0; }
    let q = x + PETIR_LAMBERT_ONE_OVER_E;
    if (q < 0.0) { return bitcast<f32>(0x7fc00000u); }
    var w: f32;
    if (x < -1.0e-6) {
        let v = petir_lambert_series(-sqrt(q));
        if (q < 3.0e-3) { return v; }
        w = v;
    } else {
        let l1 = log(-x);
        let l2 = log(-l1);
        w = l1 - l2 + l2 / l1;
    }
    return petir_lambert_halley(x, w, 32);
}
