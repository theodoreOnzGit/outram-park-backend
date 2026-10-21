// SPDX-License-Identifier: GPL-3.0-only
//
// TRANSCRIBED to WGSL (f32) from `petir::specfunc::transport`, which ports
// GSL 2.8's specfunc/transport.c.
//   Copyright (C) 1996-2000 Gerard Jungman.
//
// J(n, x) = integral_0^x t^n e^t / (e^t - 1)^2 dt, for n = 2..5. The
// Bloch-Gruneisen transport integrals: J(2) in the electrical and thermal
// conductivity of a metal, J(4) and J(5) in phonon-limited resistivity. Same
// physics as debye.wgsl one derivative apart -- the integrand is the
// derivative of the Bose-Einstein occupation times t^n.
//
// EVERY COEFFICIENT WAS EXTRACTED BY SCRIPT from the f64 module -- 72
// literals across four Chebyshev series -- and the same script emits
// `wgsl::mirror_transport`, so the shader and its CPU mirror cannot disagree
// about a constant.
//
// THREE MACHINE CONSTANTS ARE RETARGETED, and all three are PRECISION
// constants rather than range guards, so by the taxonomy in
// docs/wgsl-coverage.md they must be. Measured consequences in
// mirror_transport.
//
//   GSL_LOG_DBL_EPSILON   -36.0437  ->  -15.942385   (ln f32::EPSILON)
//   3 sqrt(DBL_EPSILON)    4.47e-08 ->    1.0358e-3  (3 sqrt f32::EPSILON)
//   2 / DBL_EPSILON        9.01e+15 ->    1.6777e+7  (2 / f32::EPSILON)
//
// A PREDICTION RECORDED AND REFUTED. The first constant was expected to be
// load-bearing twice over -- it sets `numexp`, the number of exponential
// images summed in the tail, AND it is the threshold below which the tail is
// discarded and J(n, inf) returned exactly. Measured, only the first is real:
//
//   x      numexp with f32 log-eps   with GSL's f64 value
//   5                4                       8
//   10               2                       4
//   16               1                       3
//   30               1                       2
//
//   saturation point   f32 log-eps   f64 log-eps
//   J(2)                 22.25         22.25
//   J(3)                 25.05         25.05
//   J(4)                 27.25         27.25
//   J(5)                 29.60         29.60
//
//   worst relative vs the f64 module, over x in (0, 75]:
//     shipped  1.3633715693879367e-06
//     f64      1.3633715693879367e-06   <- identical to the last digit
//
// The saturation point does not move, because `vinf - exp(t)` collapses to
// `vinf` as soon as exp(t) falls below half an ulp of vinf -- which happens
// well before t reaches -36. THE ARITHMETIC ENFORCES THE GUARD BEFORE THE
// GUARD DOES, exactly as airy.wgsl's overflow threshold is enforced by f32's
// own exp overflow.
//
// So retargeting here is CORRECT but its only measurable effect is that the
// tail sums HALF AS MANY IMAGES for the same answer. Worth doing; not worth
// claiming an accuracy benefit for.

const PETIR_TRANSPORT_LOG_EPS: f32 = -15.942385;
const PETIR_TRANSPORT_SMALL_CUT: f32 = 1.0358009e-3;
const PETIR_TRANSPORT_TWO_OVER_EPS: f32 = 16777216.0;

// GSL's transport2_cs, 18 coefficients.
fn petir_transport_cheb2(x: f32) -> f32 {
    var c = array<f32, 18>(
        1.6717604398727417, -0.1477353572845459, 0.014821382239460945, -0.0014195330440998077,
        0.00013065413804724813, -1.1715579603333026e-05, 1.0333498039472033e-06,
        -9.019112923169814e-08, 7.817717140312652e-09, -6.744565461680452e-10,
        5.799464034006441e-11, -4.9747619218498684e-12, 4.259609971603989e-13,
        -3.6421999991724865e-14, 3.110999972851013e-15, -2.6500000149068143e-16,
        2.3000000123137025e-17, -1.89999995262919e-18,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 17; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's transport3_cs, 18 coefficients.
fn petir_transport_cheb3(x: f32) -> f32 {
    var c = array<f32, 18>(
        0.7620125412940979, -0.1056743860244751, 0.01197780855000019, -0.001214401563629508,
        0.00011550997442100197, -1.0581598871794995e-05, 9.474663329456234e-07,
        -8.362211900703187e-08, 7.310910099533885e-09, -6.350595049831043e-10,
        5.4911828556436504e-11, -4.732139593371931e-12, 4.067695074001787e-13,
        -3.4897100129310105e-14, 2.9892000231676336e-15, -2.5600001172830847e-16,
        2.1900000455312983e-17, -1.89999995262919e-18,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 17; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's transport4_cs, 18 coefficients.
fn petir_transport_cheb4(x: f32) -> f32 {
    var c = array<f32, 18>(
        0.48075708746910095, -0.08175379037857056, 0.010027006268501282,
        -0.0010599339148029685, 0.00010345062764827162, -9.64427090366371e-06,
        8.745544164412422e-07, -7.793212120077442e-08, 6.864988577603981e-09,
        -5.999570840131696e-10, 5.213662487846271e-11, -4.511838385540257e-12,
        3.89215894756878e-13, -3.349360041702762e-14, 2.876700071728633e-15,
        -2.466999870141503e-16, 2.1099999343327223e-17, -1.800000020426123e-18,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 17; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

// GSL's transport5_cs, 18 coefficients.
fn petir_transport_cheb5(x: f32) -> f32 {
    var c = array<f32, 18>(
        0.347777783870697, -0.06645698845386505, 0.008611072786152363, -0.0009396682144142687,
        9.363247954752296e-05, -8.857132343109697e-06, 8.119150152197108e-07,
        -7.29576541402821e-08, 6.469714541879057e-09, -5.68490310381975e-10,
        4.9625598769198476e-11, -4.3109400597873826e-12, 3.7309998840440173e-13,
        -3.2197999149698175e-14, 2.7720000231844233e-15, -2.3800000573378295e-16,
        2.0999999824714462e-17, -1.800000020426123e-18,
    );
    var d = 0.0;
    var dd = 0.0;
    let y2 = 2.0 * x;
    for (var j: i32 = 17; j >= 1; j = j - 1) {
        let temp = d;
        d = y2 * d - dd + c[j];
        dd = temp;
    }
    return x * d - dd + 0.5 * c[0];
}

fn petir_transport_cheb(n: u32, x: f32) -> f32 {
    if (n == 2u) { return petir_transport_cheb2(x); }
    if (n == 3u) { return petir_transport_cheb3(x); }
    if (n == 4u) { return petir_transport_cheb4(x); }
    if (n == 5u) { return petir_transport_cheb5(x); }
    return bitcast<f32>(0x7fc00000u);
}

// J(n, inf) = n! zeta(n).
fn petir_transport_vinf(n: u32) -> f32 {
    if (n == 2u) { return 3.2898681; }
    if (n == 3u) { return 7.2123413; }
    if (n == 4u) { return 25.975758; }
    if (n == 5u) { return 124.43133; }
    return bitcast<f32>(0x7fc00000u);
}

// Upstream's transport_sumexp: the sum over numexp exponential images that
// carries the large-x tail. The inner loop builds an asymptotic series in
// 1/(rk x) to `order` terms; the outer accumulates them Horner-wise in t.
fn petir_transport_sumexp(numexp: u32, order: u32, t: f32, x: f32) -> f32 {
    var rk = f32(numexp);
    var out = 0.0;
    for (var k: u32 = 1u; k <= numexp; k = k + 1u) {
        var sum2 = 1.0;
        let xk = 1.0 / (rk * x);
        var xk1 = 1.0;
        for (var j: u32 = 1u; j <= order; j = j + 1u) {
            sum2 = sum2 * xk1 * xk + 1.0;
            xk1 = xk1 + 1.0;
        }
        out = out * t;
        out = out + sum2;
        rk = rk - 1.0;
    }
    return out;
}

// J(n, x) for n in 2..=5. NaN for x < 0, for NaN, and for an untabulated
// order.
fn petir_transport(n: u32, x: f32) -> f32 {
    if (n < 2u || n > 5u) { return bitcast<f32>(0x7fc00000u); }
    if (!(x >= 0.0)) { return bitcast<f32>(0x7fc00000u); }
    let nf = f32(n);

    // x^{n-1}, shared by the two small branches.
    var p = 1.0;
    for (var k: u32 = 1u; k < n; k = k + 1u) { p = p * x; }

    if (x < PETIR_TRANSPORT_SMALL_CUT) {
        return p / (nf - 1.0);
    }
    if (x <= 4.0) {
        // Upstream writes (x^2/8 - 0.5) - 0.5, not x^2/8 - 1.0: the two
        // differ in the last bit near zero, and the grouping is kept.
        let t = (x * x / 8.0 - 0.5) - 0.5;
        return p * petir_transport_cheb(n, t);
    }

    // Past 4: the limit MINUS the tail, not further integration.
    var t: f32;
    if (x < -PETIR_TRANSPORT_LOG_EPS) {
        let numexp = u32((-PETIR_TRANSPORT_LOG_EPS) / x) + 1u;
        let s = petir_transport_sumexp(numexp, n, exp(-x), x);
        t = nf * log(x) - x + log(s);
    } else if (x < PETIR_TRANSPORT_TWO_OVER_EPS) {
        let s = petir_transport_sumexp(1u, n, 1.0, x);
        t = nf * log(x) - x + log(s);
    } else {
        t = nf * log(x) - x;
    }

    let vinf = petir_transport_vinf(n);
    if (t < PETIR_TRANSPORT_LOG_EPS) {
        return vinf;
    }
    return vinf - exp(t);
}
