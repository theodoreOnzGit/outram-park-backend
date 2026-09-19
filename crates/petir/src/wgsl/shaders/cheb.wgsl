// SPDX-License-Identifier: GPL-3.0-only
//
// TRANSCRIBED to WGSL (f32) from `petir::cheb`, which ports GSL's
// `cheb/eval.c` (`gsl_cheb_eval`, `gsl_cheb_eval_n`). GSL is
//   Copyright (C) 1996-2025 Gerard Jungman, GPL-3.0-or-later.
//
// The Clenshaw recurrence, the argument mapping and the final
// `y*d1 - d2 + 0.5*c0` are unchanged from upstream. Only the float width
// differs.

// Clenshaw evaluation of a Chebyshev series on [a, b].
//
// Ports `gsl_cheb_eval` / `petir::cheb::ChebSeries::eval`. `c[0]` is the
// GSL convention coefficient that enters HALVED at the end -- this is the
// single most common thing to get wrong when hand-rolling Clenshaw, and it is
// why this is a transcription rather than a fresh implementation.
//
// Coefficients are `src[off .. off + n]`, so `n` is the coefficient COUNT and
// the series order is `n - 1`.
fn petir_cheb_eval(off: u32, n: u32, a: f32, b: f32, x: f32) -> f32 {
    if (n == 0u) { return 0.0; }

    var d1: f32 = 0.0;
    var d2: f32 = 0.0;
    let y = (2.0 * x - a - b) / (b - a);
    let y2 = 2.0 * y;

    // Upstream sweeps i = order down to 1, i.e. c[1 ..= order], backwards.
    var i: u32 = n;
    loop {
        if (i <= 1u) { break; }
        i = i - 1u;
        let temp = d1;
        d1 = y2 * d1 - d2 + src[off + i];
        d2 = temp;
    }

    return y * d1 - d2 + 0.5 * src[off];
}

// Clenshaw truncated to the first `eval_order` terms.
//
// Ports `gsl_cheb_eval_n`. `eval_order` is clamped to the series order, as
// upstream's `GSL_MIN` does.
fn petir_cheb_eval_n(off: u32, n: u32, eval_order: u32, a: f32, b: f32, x: f32) -> f32 {
    if (n == 0u) { return 0.0; }
    let order = n - 1u;
    let use_order = min(eval_order, order);

    var d1: f32 = 0.0;
    var d2: f32 = 0.0;
    let y = (2.0 * x - a - b) / (b - a);
    let y2 = 2.0 * y;

    var i: u32 = use_order + 1u;
    loop {
        if (i <= 1u) { break; }
        i = i - 1u;
        let temp = d1;
        d1 = y2 * d1 - d2 + src[off + i];
        d2 = temp;
    }

    return y * d1 - d2 + 0.5 * src[off];
}

// Map `x` on [lo, hi] to the Chebyshev reference interval [-1, 1].
//
// Ports `petir::cheb_slice::scale`.
fn petir_cheb_scale(v: f32, lo: f32, hi: f32) -> f32 {
    return (2.0 * v - lo - hi) / (hi - lo);
}
