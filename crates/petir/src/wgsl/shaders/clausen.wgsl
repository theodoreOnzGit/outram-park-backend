// SPDX-License-Identifier: GPL-3.0-only
//
// TRANSCRIBED to WGSL (f32) from `petir::specfunc::clausen` and
// `petir::specfunc::trig`, which port GSL 2.8's specfunc/clausen.c and the
// angle reductions in specfunc/trig.c.
//   Copyright (C) 1996-2000 Gerard Jungman.
//
// Cl_2(x) = -integral_0^x ln|2 sin(t/2)| dt = sum_{k>=1} sin(kx)/k^2.
// Odd, 2pi-periodic, zero at 0 and pi, maximum 1.0149416 at pi/3. It is the
// imaginary part of Li_2(e^{ix}), which makes it dilog.wgsl's trigonometric
// sibling.
//
// THE ARGUMENT REDUCTION IS REDESIGNED FOR f32, NOT TRANSCRIBED.
//
// GSL splits 2pi into three f64 pieces P1+P2+P3 so that each y*Pk subtraction
// is exact. Those constants do not carry over: P1 has about 30 significant
// bits, which leaves no room in an f32 mantissa for the period count y. So
// this uses a COARSER HEAD -- and takes it from upstream itself, because
// clausen.c already carries exactly such a split for its own pi-reflection
// step:
//
//     p0 = 6.28125                    (201/32 -- EIGHT significant bits)
//     p1 = 0.19353071795864769253e-2
//
// plus a third piece so the three sum to 2pi to f64 accuracy. An 8-bit head
// leaves 16 bits for y, i.e. exact subtraction out to about 65000 periods.
//
// MEASURED, worst |sin(reduce(theta)) - sin(theta mod 2pi)| over 2000 probes
// per decade, against a naive single-f32 2pi reduction:
//
//   theta            split      naive      ratio   theta's own ulp
//   1e0              3.937e-09  1.748e-07     44   1.192e-07
//   1e2              2.384e-07  4.682e-05    196   7.629e-06
//   1e3              3.052e-07  4.091e-04   1341   6.104e-05
//   1e4              9.783e-07  5.350e-03   5468   9.766e-04
//   1e5              7.234e-06  2.897e-02   4004   7.812e-03
//   2.6e5 .. 5.2e5   7.774e-06  2.893e-02  ~3700   --
//
// THE SPLIT HOLDS NEAR 8e-06 ALL THE WAY TO THE REFUSAL, three to four
// thousand times better than the naive form, which instead tracks theta's
// own ulp. There is no collapse inside the usable range.
//
// That is not luck. An eight-bit head keeps y*P0 exact while y fits in
// sixteen bits, i.e. out to 65536 periods -- 4.12e5 in theta, against a
// refusal at 5.24e5. THE HEAD WIDTH AND THE CUT ARE MATCHED to within a
// factor of 1.3, and the small rise from 7.23e-06 to 7.77e-06 in the last
// row is that limit starting to bite.
//
// THE REFUSAL IS AT 0.0625/f32::EPSILON = 524288, the correct f32 analogue of
// upstream's threshold -- a PRECISION constant, retargeted, unlike
// airy.wgsl's range guard which is kept. Here, unusually, the cut and the
// usable range nearly coincide; in the f64 module they are seven decades
// apart.
//
// A note on how this was measured: |reduce(theta) - (theta mod 2pi)| is the
// obvious instrument and it is WRONG -- at a period boundary the reduced
// value jumps between 0 and 2pi, so a probe landing either side reports an
// error of 2pi when nothing is amiss. The comparison goes through sin, which
// is continuous there.

const PETIR_CLAUSEN_P0: f32 = 6.28125;        // 201/32, 8 significant bits
const PETIR_CLAUSEN_P1: f32 = 1.9353072e-3;
const PETIR_CLAUSEN_P2: f32 = 1.0253132e-11;
const PETIR_CLAUSEN_TWO_PI: f32 = 6.2831855;  // = P0 + P1 + P2 in f32
const PETIR_CLAUSEN_PI: f32 = 3.1415927;
// 0.0625 / f32::EPSILON -- upstream's threshold, retargeted.
const PETIR_CLAUSEN_LOSS_CUT: f32 = 524288.0;
// pi * sqrt(f32::EPSILON) -- the small-argument cut, also retargeted.
const PETIR_CLAUSEN_X_CUT: f32 = 1.0846882e-3;

// GSL's aclaus_cs, 15 coefficients, order 14 -- the whole array is used.
fn petir_clausen_cheb(x: f32) -> f32 {
    var c = array<f32, 15>(
        2.1426945, 0.07233243, 0.0010164248, 3.2452503e-5, 1.3331519e-6,
        6.2132406e-8, 3.1300413e-9, 1.6635723e-10, 9.196593e-12, 5.240046e-13,
        3.05804e-14, 1.8197e-15, 1.1e-16, 6.8e-18, 4e-19
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

// Reduce theta >= 0 to [0, 2pi) with the coarse-head split. NaN past the
// loss cut -- see the header, and note the usable range is far below it.
fn petir_clausen_reduce(theta: f32) -> f32 {
    if (!(theta == theta) || abs(theta) > PETIR_CLAUSEN_LOSS_CUT) {
        return bitcast<f32>(0x7fc00000u);
    }
    let y = floor(theta / PETIR_CLAUSEN_TWO_PI);
    var r = ((theta - y * PETIR_CLAUSEN_P0) - y * PETIR_CLAUSEN_P1) - y * PETIR_CLAUSEN_P2;
    if (r > PETIR_CLAUSEN_TWO_PI) {
        r = ((r - PETIR_CLAUSEN_P0) - PETIR_CLAUSEN_P1) - PETIR_CLAUSEN_P2;
    } else if (r < 0.0) {
        r = ((r + PETIR_CLAUSEN_P0) + PETIR_CLAUSEN_P1) + PETIR_CLAUSEN_P2;
    }
    return r;
}

// Cl_2(x) for all real x. NaN past the reduction's loss cut, and for NaN.
fn petir_clausen(x_in: f32) -> f32 {
    if (!(x_in == x_in)) { return bitcast<f32>(0x7fc00000u); }
    var x = x_in;
    var sgn = 1.0;
    if (x < 0.0) { x = -x; sgn = -1.0; }

    x = petir_clausen_reduce(x);
    if (!(x == x)) { return bitcast<f32>(0x7fc00000u); }

    if (x > PETIR_CLAUSEN_PI) {
        // Upstream's own simulated extra precision for pi - x: the same two
        // constants the reduction above is built from.
        x = (PETIR_CLAUSEN_P0 - x) + PETIR_CLAUSEN_P1;
        sgn = -sgn;
    }

    var val: f32;
    if (x == 0.0) {
        val = 0.0;
    } else if (x < PETIR_CLAUSEN_X_CUT) {
        // Cl_2(x) ~ x(1 - ln x). Not an optimisation: the Chebyshev branch
        // forms a difference of two large quantities there.
        val = x * (1.0 - log(x));
    } else {
        let t = 2.0 * (x * x / (PETIR_CLAUSEN_PI * PETIR_CLAUSEN_PI) - 0.5);
        val = x * (petir_clausen_cheb(t) - log(x));
    }
    return sgn * val;
}
