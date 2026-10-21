// SPDX-License-Identifier: GPL-3.0-only
//
// TRANSCRIBED to WGSL (f32) from `petir::poly::eval`, which ports GSL's
// `poly/eval.c` (`gsl_poly_eval`). GSL is
//   Copyright (C) 1996-2025 the GSL contributors, GPL-3.0-or-later.
//
// The arithmetic is unchanged: the same Horner sweep, in the same order, over
// the same ASCENDING coefficient convention. Only the float width differs,
// and that is the whole point of this file -- see `petir::wgsl`.

// Horner evaluation of a polynomial with ASCENDING coefficients:
//   c[0] + c[1] x + c[2] x^2 + ... + c[n-1] x^(n-1)
//
// Ports `gsl_poly_eval` / `petir::poly::eval::eval`. The loop runs from the
// highest coefficient down, accumulating `acc * x + c_i`, which is the
// association upstream uses and the one the f64 reference uses -- changing it
// would change the rounding and make the comparison meaningless.
//
// `c` is a slice of `src` starting at `off`, of length `n`.
fn petir_poly_eval(off: u32, n: u32, x: f32) -> f32 {
    var acc: f32 = 0.0;
    var i: u32 = n;
    loop {
        if (i == 0u) { break; }
        i = i - 1u;
        acc = acc * x + src[off + i];
    }
    return acc;
}

// Horner with a compensated (Kahan-style) accumulation of the running sum.
//
// NOT a port -- this has no GSL counterpart and is provided because f32 Horner
// loses accuracy fast on long polynomials, and a GPU caller usually cannot
// escape to f64. It is marked clearly so nobody mistakes it for GSL's
// behaviour: `petir_poly_eval` is the one that reproduces the reference.
fn petir_poly_eval_comp(off: u32, n: u32, x: f32) -> f32 {
    var acc: f32 = 0.0;
    var err: f32 = 0.0;
    var i: u32 = n;
    loop {
        if (i == 0u) { break; }
        i = i - 1u;
        let p = acc * x;
        let e = fma(acc, x, -p);
        let s = p + src[off + i];
        let t = s - p;
        err = err * x + (e + ((p - (s - t)) + (src[off + i] - t)));
        acc = s;
    }
    return acc + err;
}
