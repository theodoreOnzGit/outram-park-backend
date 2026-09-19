// SPDX-License-Identifier: GPL-3.0-only
//
// TRANSCRIBED to WGSL (f32) from `petir::specfunc::gegenbauer`, which ports
// GSL 2.8's specfunc/gegenbauer.c.
//   Copyright (C) 1996-2000 Gerard Jungman.
//
//   C_n^lambda(x) -- the ultraspherical polynomials, orthogonal on [-1, 1]
//   with weight (1 - x^2)^{lambda - 1/2}.
//
// THE FOURTH TABLE-FREE SHADER, after dilog, ellint and elljac. Three closed
// forms and a three-term recurrence; nothing is fitted.
//
// A RECURRENCE IS NOT A SERIES. There is no truncation, no convergence test
// and no tolerance anywhere in this file -- the loop runs exactly n - 3
// times and stops. That makes it the best-behaved kind of kernel for a GPU:
// the trip count is a uniform function of one integer parameter, so a
// workgroup evaluating one order never diverges.
//
// WHAT IT COSTS is accumulated rounding, which grows like n. The f64 module
// measures the same recurrence against an explicit-coefficient form and
// finds the COEFFICIENT form is the weaker of the two by seven digits at
// n = 29 -- so the growth here is mild, and the comparison a reader is
// likely to reach for is the less accurate one.
//
// lambda = 0 IS A SEPARATE BRANCH, and it is upstream's. C_n^lambda vanishes
// identically at lambda = 0 for n >= 1; what upstream returns is the limit
// of C_n^lambda / lambda, which is 2 T_n(x) / n. Its own low-order closed
// forms follow the same normalisation, which is why petir_gegenpoly_1 gives
// 2x rather than 0 there.
//
// NOTE the branch is guarded on x in [-1, 1] because it calls acos. Outside
// that, upstream falls through to the recurrence, and so does this.

fn petir_gegen_nan() -> f32 { return bitcast<f32>(0x7fc00000u); }

fn petir_gegenpoly_1(lambda: f32, x: f32) -> f32 {
    if (lambda == 0.0) { return 2.0 * x; }
    return 2.0 * lambda * x;
}

fn petir_gegenpoly_2(lambda: f32, x: f32) -> f32 {
    if (lambda == 0.0) { return -1.0 + 2.0 * x * x; }
    return lambda * (-1.0 + 2.0 * (1.0 + lambda) * x * x);
}

fn petir_gegenpoly_3(lambda: f32, x: f32) -> f32 {
    if (lambda == 0.0) { return x * (-2.0 + 4.0 / 3.0 * x * x); }
    let c = 4.0 + lambda * (6.0 + 2.0 * lambda);
    return 2.0 * lambda * x * (-1.0 - lambda + c * x * x / 3.0);
}

// C_n^lambda(x). NaN for lambda <= -1/2, which is upstream's DOMAIN_ERROR.
fn petir_gegenpoly_n(n: u32, lambda: f32, x: f32) -> f32 {
    if (lambda != lambda || x != x || lambda <= -0.5) { return petir_gegen_nan(); }
    if (n == 0u) { return 1.0; }
    if (n == 1u) { return petir_gegenpoly_1(lambda, x); }
    if (n == 2u) { return petir_gegenpoly_2(lambda, x); }
    if (n == 3u) { return petir_gegenpoly_3(lambda, x); }
    if (lambda == 0.0 && x >= -1.0 && x <= 1.0) {
        let z = f32(n) * acos(x);
        return 2.0 * cos(z) / f32(n);
    }
    var gkm2 = petir_gegenpoly_2(lambda, x);
    var gkm1 = petir_gegenpoly_3(lambda, x);
    var gk = 0.0;
    for (var k: u32 = 4u; k <= n; k = k + 1u) {
        let kf = f32(k);
        gk = (2.0 * (kf + lambda - 1.0) * x * gkm1 - (kf + 2.0 * lambda - 2.0) * gkm2) / kf;
        gkm2 = gkm1;
        gkm1 = gk;
    }
    return gk;
}
