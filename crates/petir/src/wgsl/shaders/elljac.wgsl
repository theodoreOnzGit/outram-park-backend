// SPDX-License-Identifier: GPL-3.0-only
//
// TRANSCRIBED to WGSL (f32) from `petir::specfunc::elljac`, which ports
// GSL 2.8's specfunc/elljac.c.
//   Copyright (C) 1996-2000 Gerard Jungman; the current algorithm is
//   Brian Gough's, 2005.
//   Algorithm 5 of R. Bulirsch, Numer. Math. 7, 78-90 (1965), with
//   upstream's reflection tweak (Abramowitz & Stegun table 16.8, "K-u").
//
//   sn(u|m), cn(u|m), dn(u|m) -- the inverses of F(phi, k), m = k^2.
//
// THE THIRD TABLE-FREE SHADER, after dilog and ellint. An
// arithmetic-geometric mean descent and a backward recurrence; nothing is
// fitted, so there is no coefficient array and no order_sp decision.
//
// ALL THREE FUNCTIONS COME FROM ONE DESCENT, so the entry point returns a
// vec3<f32> rather than being called three times. That is not a
// convenience: computing them separately would run the AGM three times for
// one answer each.
//
// THE DESCENT CAP IS 8, NOT UPSTREAM'S 16.
//
//   The AGM converges QUADRATICALLY -- the number of correct digits doubles
//   every step -- so the step count grows like log log of the precision
//   demanded, not like log. Measured in f64 over a 200 x 200 grid spanning
//   |m| <= 1 to both endpoints, plus m within 1e-14 of either: 7 steps at
//   worst. f32 asks for half the digits, so it needs fewer still; measured
//   over the same domain at f32 it is 5. Halving the digits removed two
//   steps, not half of them -- that is what log log convergence looks like.
//   mirror_elljac's the_descent_is_shorter_in_f32_than_in_f64 re-measures
//   it and fails if the cap is ever reached.
//
//   8 therefore has room and upstream's 16 is twice the ceiling needed. As
//   with ellint's nmax, on a GPU that ceiling is not free the way it is on
//   a CPU: a uniform loop bound is what the compiler unrolls against, and
//   here it also sets the size of the two arrays held in registers.
//
// THE DEGENERATE-LIMIT WINDOWS ARE RETARGETED, and they are PRECISION
// constants rather than range guards. Upstream switches to (sin, cos, 1) at
// |m| < 2 DBL_EPSILON and to (tanh, sech, sech) at |m - 1| < 2 DBL_EPSILON.
// Retargeted to 2 f32::EPSILON = 2.3841858e-7. Keeping f64's 4.44e-16 would
// make both branches unreachable in f32 -- no f32 value lies strictly
// between 0 and 4.44e-16 other than denormals -- so the special cases would
// be dead code and the general branch would be asked for m = 0, where
// nu_0 = 1 = mu_0 and the descent exits immediately with a division that is
// fine, but for m = 1 exactly it would form 0/0.

const PETIR_ELLJAC_NMAX: u32 = 8u;
const PETIR_ELLJAC_EPS: f32 = 1.1920929e-7;
const PETIR_ELLJAC_DEGEN: f32 = 2.3841858e-7;

fn petir_elljac_nan3() -> vec3<f32> {
    let n = bitcast<f32>(0x7fc00000u);
    return vec3<f32>(n, n, n);
}

fn petir_elljac_hypot(x: f32, y: f32) -> f32 {
    let ax = abs(x);
    let ay = abs(y);
    if (ax == 0.0) { return ay; }
    if (ay == 0.0) { return ax; }
    let big = max(ax, ay);
    let small = min(ax, ay);
    let r = small / big;
    return big * sqrt(1.0 + r * r);
}

// (sn, cn, dn) at u with parameter m = k^2. NaN in all three for |m| > 1.
fn petir_elljac(u: f32, m: f32) -> vec3<f32> {
    if (u != u || m != m || abs(m) > 1.0) { return petir_elljac_nan3(); }
    if (abs(m) < PETIR_ELLJAC_DEGEN) {
        return vec3<f32>(sin(u), cos(u), 1.0);
    }
    if (abs(m - 1.0) < PETIR_ELLJAC_DEGEN) {
        let c = 1.0 / cosh(u);
        return vec3<f32>(tanh(u), c, c);
    }

    var mu = array<f32, 8>(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    var nu = array<f32, 8>(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    mu[0] = 1.0;
    nu[0] = sqrt(1.0 - m);

    var n: u32 = 0u;
    loop {
        let mn = mu[n];
        let nn = nu[n];
        if (abs(mn - nn) <= 4.0 * PETIR_ELLJAC_EPS * abs(mn + nn)) { break; }
        mu[n + 1u] = 0.5 * (mn + nn);
        nu[n + 1u] = sqrt(mn * nn);
        n = n + 1u;
        if (n >= PETIR_ELLJAC_NMAX - 1u) { break; }
    }

    let mu_n = mu[n];
    let sin_umu = sin(u * mu_n);
    let cos_umu = cos(u * mu_n);

    // Upstream switches to sn(K-u) etc. when |sin| < |cos|, so the tangent
    // it forms never divides by a vanishing sine.
    let reflected = abs(sin_umu) < abs(cos_umu);
    var t = cos_umu / sin_umu;
    if (reflected) { t = sin_umu / cos_umu; }

    // The backward recurrence. c and d are carried as scalars because only
    // their final values are read; upstream stores the whole arrays.
    var c = mu_n * t;
    var d = 1.0;
    var k: u32 = n;
    loop {
        if (k == 0u) { break; }
        k = k - 1u;
        let c_next = c;
        let d_next = d;
        c = d_next * c_next;
        let r = c_next * c_next / mu[k + 1u];
        d = (r + nu[k]) / (r + mu[k]);
    }

    let root = sqrt(1.0 - m);
    if (reflected) {
        let dn = root / d;
        let cn = dn * sign(cos_umu) / petir_elljac_hypot(1.0, c);
        return vec3<f32>(cn * c / root, cn, dn);
    }
    let sn = sign(sin_umu) / petir_elljac_hypot(1.0, c);
    return vec3<f32>(sn, c * sn, d);
}

// Scalar selectors, so the test harness -- which maps one f32 in to one f32
// out -- can reach each component. `which` is 0 = sn, 1 = cn, 2 = dn.
fn petir_elljac_component(which: u32, u: f32, m: f32) -> f32 {
    let r = petir_elljac(u, m);
    switch (which) {
        case 0u: { return r.x; }
        case 1u: { return r.y; }
        case 2u: { return r.z; }
        default: { return bitcast<f32>(0x7fc00000u); }
    }
}
