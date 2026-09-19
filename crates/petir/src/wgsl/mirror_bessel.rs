//! `f32` mirrors of `shaders/bessel.wgsl` — GSL's order-0 and order-1
//! cylindrical Bessel functions.
//!
//! Generated from the same parse of [`crate::specfunc::bessel`] that produced
//! the shader, so the two cannot drift in their 358 coefficients. See
//! [`crate::wgsl::mirror`] for why a mirror exists at all.
//!
//! # These kernels cannot be held to bit-identity, and that is not a defect
//!
//! Every other family in this module that is pure arithmetic —
//! [`crate::wgsl::mirror`]'s Horner and Clenshaw, the matrix and BLAS kernels
//! — agrees with the GPU **exactly**, because IEEE-754 pins each operation to
//! one correctly-rounded answer. The Bessel kernels call `sin`, `cos`, `sqrt`,
//! `log` and `exp`, and WGSL specifies its builtins to an **ULP bound rather
//! than correct rounding**. The device's `sin` and `libm`'s `sinf` are
//! different functions that agree to about an ulp, so a bound of zero would
//! fail on a conforming device.
//!
//! # Where the error comes from — and a prediction the measurement refuted
//!
//! The expectation written here before anything was run was that the
//! asymptotic branch would dominate: past `x = 4`, `J` and `Y` are evaluated
//! as `cos(x - pi/4 + theta/x)`, the phase is `x` itself, and an `f32` `x`
//! near 50 carries about 4e-6 of representation error, so the branch past
//! `x = 4` should be visibly worse than the Chebyshev branch below it.
//!
//! **It is not.** Measured over 400 points on `[0.125, 50]`, restricted to
//! arguments where `|f| > 0.05` so that cancellation at a zero cannot
//! contaminate a relative figure:
//!
//! | function | below `x = 4` | above `x = 4` |
//! |---|---|---|
//! | `J_0` | 1.830e-07 | 2.441e-07 |
//! | `J_1` | 6.595e-07 | 2.161e-07 |
//! | `Y_0` | 3.031e-07 | 2.525e-07 |
//! | `Y_1` | 2.382e-07 | 2.908e-07 |
//!
//! The two branches are indistinguishable, and both sit at the `f32` floor.
//! `J_1` is *worse* below `x = 4` than above it, which is the opposite of the
//! prediction. `the_asymptotic_branch_is_no_worse_than_the_chebyshev_one`
//! pins this so the claim cannot quietly revert to the guess.
//!
//! # What the error actually is: the zeros
//!
//! Unrestricted, the worst relative errors are `J_1` 7.354e-06 at `x = 3.875`
//! and `Y_1` 2.786e-05 at `x = 11.75`. Both are zeros: `J_1`'s first is at
//! 3.8317 and `Y_1` has one at 11.7492. A function passing through zero has
//! unbounded *relative* error for any implementation in any precision, because
//! the value vanishes and the error does not. `J_1`'s worst sits in the
//! **Chebyshev** branch, which is the clearest evidence that proximity to a
//! zero, not the asymptotic form, is the mechanism.
//!
//! So: **the bounded quantity for `J` and `Y` is the absolute error** —
//! 1.275e-07 (`J_1`), 7.542e-07 (`Y_1`) over the same sweep — and a caller
//! wanting a relative bound must stay away from the zeros, exactly as in
//! `f64`.
//!
//! `I` and `K` have no zeros on the positive axis and are accordingly clean:
//! 1.575e-07 to 1.748e-07 relative, which is one `f32` ulp.

// Under a std-linked build (`cargo test`) f32's inherent sqrt/exp/... shadow
// these trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

#[rustfmt::skip]
const BI0: [f32; 12] = [
    -0.0766054725, 1.92733795, 0.228264459, 0.0130489147, 0.00043442709, 9.42265769e-06,
    1.43400629e-07, 1.61384907e-09, 1.39665004e-11, 9.579451e-14, 5.3339e-16, 2.45e-18
];

#[rustfmt::skip]
const AI0: [f32; 21] = [
    0.0757599449, 0.00759138081, 0.000415313134, 1.07007646e-05, -7.90117998e-06,
    -7.8261435e-07, 2.78384994e-07, 8.2524726e-09, -1.20446394e-08, 1.55964859e-09,
    2.2925563e-10, -1.1916228e-10, 1.757854e-11, 1.12822e-12, -1.14684e-12, 2.7155e-13,
    -2.415e-14, -6.08e-15, 3.14e-15, -7.1e-16, 7e-17
];

#[rustfmt::skip]
const AI02: [f32; 22] = [
    0.054490411, 0.00336911648, 6.88975835e-05, 2.89137052e-06, 2.04891859e-07,
    2.26666899e-08, 3.39623203e-09, 4.9406022e-10, 1.188914e-11, -3.149915e-11,
    -1.32158e-11, -1.79419e-12, 7.1801e-13, 3.8529e-13, 1.539e-14, -4.151e-14, -9.54e-15,
    3.82e-15, 1.76e-15, -3.4e-16, -2.7e-16, 3e-17
];

#[rustfmt::skip]
const BI1: [f32; 11] = [
    -0.00197171326, 0.407348877, 0.0348389943, 0.00154539456, 4.18885211e-05,
    7.64902676e-07, 1.00424939e-08, 9.9322077e-11, 7.6638e-13, 4.741e-15, 2.4e-17
];

#[rustfmt::skip]
const AI1: [f32; 21] = [
    -0.0284674418, -0.0192295323, -0.000611518586, -2.06997125e-05, 8.58561915e-06,
    1.04949825e-06, -2.91833892e-07, -1.55937815e-08, 1.31801237e-08, -1.44842341e-09,
    -2.9085122e-10, 1.2663889e-10, -1.664947e-11, -1.66665e-12, 1.2426e-12, -2.7315e-13,
    2.023e-14, 7.3e-15, -3.33e-15, 7.1e-16, -6e-17
];

#[rustfmt::skip]
const AI12: [f32; 22] = [
    0.028576235, -0.00976109749, -0.000110588939, -3.88256481e-06, -2.51223624e-07,
    -2.63146885e-08, -3.83538039e-09, -5.5897433e-10, -1.897495e-11, 3.252602e-11,
    1.41258e-11, 2.03564e-12, -7.1985e-13, -4.0836e-13, -2.101e-14, 4.273e-14, 1.041e-14,
    -3.82e-15, -1.86e-15, 3.3e-16, 2.8e-16, -3e-17
];

#[rustfmt::skip]
const BJ0: [f32; 13] = [
    0.100254162, -0.665223008, 0.248983703, -0.0332527232, 0.00231141793, -9.91127742e-05,
    2.89167086e-06, -6.12108587e-08, 9.83865079e-10, -1.24235515e-11, 1.265433e-13,
    -1.0619e-15, 7.4e-18
];

#[rustfmt::skip]
const BJ1: [f32; 12] = [
    -0.117261415, -0.253615218, 0.050127081, -0.00463151481, 0.000247996229,
    -8.67894869e-06, 2.14293917e-07, -3.93609308e-09, 5.5911823e-11, -6.32761e-13,
    5.84e-15, -4.4e-17
];

#[rustfmt::skip]
const BY0: [f32; 13] = [
    -0.0112778394, -0.128345238, -0.104378848, 0.0236627492, -0.00209039165,
    0.000103975454, -3.36974716e-06, 7.72938427e-08, -1.32497677e-09, 1.7648232e-11,
    -1.88105e-13, 1.641e-15, -1.1e-17
];

#[rustfmt::skip]
const BY1: [f32; 14] = [
    0.032080471, 1.2627079, 0.0064999619, -0.0893616453, 0.0132508812, -0.000897905912,
    3.64736149e-05, -1.00137438e-06, 1.99453966e-08, -3.0230656e-10, 3.60987815e-12,
    -3.487488e-14, 2.7838e-16, -1.86e-18
];

#[rustfmt::skip]
const AK0: [f32; 24] = [
    -0.0328737867, -0.0449369058, 0.00298149992, -0.000303693649, 3.91085569e-05,
    -5.86872422e-06, 9.8287371e-07, -1.78978645e-07, 3.48332307e-08, -7.1590921e-09,
    1.5401993e-09, -3.44555486e-10, 7.97356102e-11, -1.90090969e-11, 4.65295609e-12,
    -1.16614287e-12, 2.98554375e-13, -7.7927698e-14, 2.07027467e-14, -5.5898786e-15,
    1.53202966e-15, -4.25737537e-16, 1.19840239e-16, -3.41407347e-17
];

#[rustfmt::skip]
const AK02: [f32; 14] = [
    -0.0120186983, -0.00917485269, 0.000144455093, -4.01361418e-06, 1.56783181e-07,
    -7.77011044e-09, 4.61118258e-10, -3.158593e-11, 2.43501804e-12, -2.07433139e-13,
    1.92578728e-14, -1.92755481e-15, 2.06219803e-16, -2.34168512e-17
];

#[rustfmt::skip]
const AK1: [f32; 25] = [
    0.207996868, 0.162581565, -0.00587070424, 0.00049502152, -5.78958348e-05,
    8.1861461e-06, -1.31604832e-06, 2.32546032e-07, -4.42206518e-08, 8.92163995e-09,
    -1.89046271e-09, 4.17568808e-10, -9.55912362e-11, 2.25769353e-11, -5.48128e-12,
    1.36386123e-12, -3.46936691e-13, 9.00354564e-14, -2.37950578e-14, 6.39447504e-15,
    -1.74498363e-15, 4.82994548e-16, -1.35460928e-16, 3.84604274e-17, -1.10456856e-17
];

#[rustfmt::skip]
const AK12: [f32; 14] = [
    0.0637930834, 0.0283288781, -0.000247537067, 5.77197245e-06, -2.06893922e-07,
    9.73998344e-09, -5.58533614e-10, 3.73299663e-11, -2.82505196e-12, 2.372019e-13,
    -2.17667739e-14, 2.15791416e-15, -2.29019693e-16, 2.58288573e-17
];

#[rustfmt::skip]
const BM0: [f32; 21] = [
    0.0928496164, -0.00142987707, 2.83057927e-05, -1.43300611e-06, 1.2028628e-07,
    -1.39711301e-08, 2.04076188e-09, -3.5399669e-10, 7.024759e-11, -1.554107e-11,
    3.76226e-12, -9.8282e-13, 2.7408e-13, -8.091e-14, 2.511e-14, -8.14e-15, 2.75e-15,
    -9.6e-16, 3.4e-16, -1.2e-16, 4e-17
];

#[rustfmt::skip]
const BTH0: [f32; 24] = [
    -0.246391638, 0.00173709831, -6.21836334e-05, 4.36805017e-06, -4.5609302e-07,
    6.21974001e-08, -1.03004429e-08, 1.97952678e-09, -4.28198396e-10, 1.0203584e-10,
    -2.6363898e-11, 7.297935e-12, -2.144188e-12, 6.63693e-13, -2.15126e-13, 7.2659e-14,
    -2.5465e-14, 9.229e-15, -3.448e-15, 1.325e-15, -5.22e-16, 2.1e-16, -8.7e-17, 3.6e-17
];

#[rustfmt::skip]
const BM1: [f32; 21] = [
    0.104736251, 0.00442443894, -5.6616395e-05, 2.31349417e-06, -1.7377182e-07,
    1.89320993e-08, -2.65416023e-09, 4.4740209e-10, -8.691795e-11, 1.891492e-11,
    -4.51884e-12, 1.16765e-12, -3.2265e-13, 9.45e-14, -2.913e-14, 9.39e-15, -3.15e-15,
    1.09e-15, -3.9e-16, 1.4e-16, -5e-17
];

#[rustfmt::skip]
const BTH1: [f32; 24] = [
    0.74060141, -0.00457175566, 0.000119818511, -6.96456189e-06, 6.55495621e-07,
    -8.40662289e-08, 1.33768866e-08, -2.49956565e-09, 5.294951e-10, -1.24135944e-10,
    3.1656485e-11, -8.66864e-12, 2.523758e-12, -7.75085e-13, 2.49527e-13, -8.3773e-14,
    2.9205e-14, -1.0534e-14, 3.919e-15, -1.5e-15, 5.89e-16, -2.37e-16, 9.7e-17, -4e-17
];

#[rustfmt::skip]
const K0_POLY: [f32; 8] = [
    0.115931516, 0.278982879, 0.0252489299, 0.000846035091, 1.49147192e-05, 1.62710689e-07,
    1.20826603e-09, 6.61171047e-12
];

#[rustfmt::skip]
const I0_POLY: [f32; 7] = [
    1.0, 0.25, 0.0277777778, 0.00173611111, 6.9444476e-05, 1.92882658e-06, 3.99082206e-08
];

#[rustfmt::skip]
const K1_POLY: [f32; 9] = [
    -0.307965758, -0.0853707197, -0.00464218277, -0.00011253607, -1.55928877e-06,
    -1.40301637e-08, -8.87189986e-11, -4.16143236e-13, -1.52612934e-15
];

#[rustfmt::skip]
const I1_POLY: [f32; 6] = [
    0.0833333333, 0.00694444444, 0.000347222222, 1.1574076e-05, 2.755587e-07,
    4.97243862e-09
];

const SQRT2: f32 = 1.4142135623730951;
const ROOT_EIGHT: f32 = 2.8284271247461903;
const SQRT_DBL_EPSILON: f32 = 1.4901161193847656e-8;
const ROOT5_DBL_EPSILON: f32 = 7.4009597974140505e-4;
const TWO_OVER_PI: f32 = 0.6366197723675814;
const LN2: f32 = 0.6931471805599453;
const XMAX: f32 = 4.503599627370496e15;

/// Clenshaw in GSL's convention, closing with `0.5 c[0]`. Mirrors every
/// `petir_bessel_cheb_*` in the shader, which inline this loop around their
/// own table.
///
/// The shader indexes its array directly; this walks `c[1..]` in reverse,
/// which is the same sequence of operations in the same order.
fn cheb(x: f32, c: &[f32]) -> f32 {
    let Some((&c0, rest)) = c.split_first() else {
        return 0.0;
    };
    let (mut d, mut dd) = (0.0_f32, 0.0_f32);
    let y2 = 2.0 * x;
    for &ci in rest.iter().rev() {
        let temp = d;
        d = y2 * d - dd + ci;
        dd = temp;
    }
    x * d - dd + 0.5 * c0
}

/// Horner, ascending coefficients — GSL's `gsl_poly_eval`. Mirrors every
/// `petir_bessel_poly_*`.
fn poly(x: f32, c: &[f32]) -> f32 {
    let Some((&last, head)) = c.split_last() else {
        return 0.0;
    };
    let mut ans = last;
    for &ci in head.iter().rev() {
        ans = ci + x * ans;
    }
    ans
}

/// `(sin eps, cos eps)`. Mirrors `petir_bessel_sin_cos_eps`.
fn sin_cos_eps(eps: f32) -> (f32, f32) {
    if eps.abs() < ROOT5_DBL_EPSILON {
        let e2 = eps * eps;
        (
            eps * (1.0 - e2 / 6.0 * (1.0 - e2 / 20.0)),
            1.0 - e2 / 2.0 * (1.0 - e2 / 12.0),
        )
    } else {
        (eps.sin(), eps.cos())
    }
}

/// `cos(y - pi/4 + eps)`. Mirrors `petir_bessel_cos_pi4`.
fn cos_pi4(y: f32, eps: f32) -> f32 {
    let (sy, cy) = (y.sin(), y.cos());
    let (s, d) = (sy + cy, sy - cy);
    let (seps, ceps) = sin_cos_eps(eps);
    (ceps * s - seps * d) / SQRT2
}

/// `sin(y - pi/4 + eps)`. Mirrors `petir_bessel_sin_pi4`.
fn sin_pi4(y: f32, eps: f32) -> f32 {
    let (sy, cy) = (y.sin(), y.cos());
    let (s, d) = (sy + cy, sy - cy);
    let (seps, ceps) = sin_cos_eps(eps);
    (ceps * d + seps * s) / SQRT2
}

/// `J_0(x)` in `f32`. Mirrors `petir_bessel_j0`.
pub fn j0(x: f32) -> f32 {
    let y = x.abs();
    if y < 2.0 * SQRT_DBL_EPSILON {
        return 1.0;
    }
    if y <= 4.0 {
        return cheb(0.125 * y * y - 1.0, &BJ0);
    }
    let z = 32.0 / (y * y) - 1.0;
    let ca = cheb(z, &BM0);
    let ct = cheb(z, &BTH0);
    let cp = cos_pi4(y, ct / y);
    (0.75 + ca) / y.sqrt() * cp
}

/// `J_1(x)` in `f32`. Mirrors `petir_bessel_j1`.
pub fn j1(x: f32) -> f32 {
    let y = x.abs();
    if y == 0.0 {
        return 0.0;
    }
    if y < ROOT_EIGHT * SQRT_DBL_EPSILON {
        return 0.5 * x;
    }
    if y < 4.0 {
        return x * (0.25 + cheb(0.125 * y * y - 1.0, &BJ1));
    }
    let z = 32.0 / (y * y) - 1.0;
    let ca = cheb(z, &BM1);
    let ct = cheb(z, &BTH1);
    let sp = sin_pi4(y, ct / y);
    let ampl = (0.75 + ca) / y.sqrt();
    (if x < 0.0 { -ampl } else { ampl }) * sp
}

/// `Y_0(x)` in `f32`, `NaN` for `x <= 0`. Mirrors `petir_bessel_y0`.
pub fn y0(x: f32) -> f32 {
    if !(x > 0.0) {
        return f32::NAN;
    }
    if x < 4.0 {
        let j0v = j0(x);
        let c = cheb(0.125 * x * x - 1.0, &BY0);
        return TWO_OVER_PI * (-LN2 + x.ln()) * j0v + 0.375 + c;
    }
    if x < XMAX {
        let z = 32.0 / (x * x) - 1.0;
        let c1 = cheb(z, &BM0);
        let c2 = cheb(z, &BTH0);
        let sp = sin_pi4(x, c2 / x);
        return (0.75 + c1) / x.sqrt() * sp;
    }
    0.0
}

/// `Y_1(x)` in `f32`, `NaN` for `x <= 0`. Mirrors `petir_bessel_y1`.
pub fn y1(x: f32) -> f32 {
    if !(x > 0.0) {
        return f32::NAN;
    }
    if x < 2.0 * SQRT_DBL_EPSILON {
        let j1v = j1(x);
        let c = cheb(-1.0, &BY1);
        return TWO_OVER_PI * (0.5 * x).ln() * j1v + (0.5 + c) / x;
    }
    if x < 4.0 {
        let c = cheb(0.125 * x * x - 1.0, &BY1);
        let j1v = j1(x);
        return TWO_OVER_PI * (0.5 * x).ln() * j1v + (0.5 + c) / x;
    }
    if x < XMAX {
        let z = 32.0 / (x * x) - 1.0;
        let ca = cheb(z, &BM1);
        let ct = cheb(z, &BTH1);
        let cp = cos_pi4(x, ct / x);
        return -(0.75 + ca) / x.sqrt() * cp;
    }
    0.0
}

/// `e^{-|x|} I_0(x)` in `f32`. Mirrors `petir_bessel_i0_scaled`.
pub fn i0_scaled(x: f32) -> f32 {
    let y = x.abs();
    if y < 2.0 * SQRT_DBL_EPSILON {
        return 1.0 - y;
    }
    if y <= 3.0 {
        return (-y).exp() * (2.75 + cheb(y * y / 4.5 - 1.0, &BI0));
    }
    if y <= 8.0 {
        return (0.375 + cheb((48.0 / y - 11.0) / 5.0, &AI0)) / y.sqrt();
    }
    (0.375 + cheb(16.0 / y - 1.0, &AI02)) / y.sqrt()
}

/// `I_0(x)` in `f32`; `+inf` past `|x| ~ 88`. Mirrors `petir_bessel_i0`.
pub fn i0(x: f32) -> f32 {
    let y = x.abs();
    if y < 2.0 * SQRT_DBL_EPSILON {
        return 1.0;
    }
    if y <= 3.0 {
        return 2.75 + cheb(y * y / 4.5 - 1.0, &BI0);
    }
    y.exp() * i0_scaled(x)
}

/// `e^{-|x|} I_1(x)` in `f32`. Mirrors `petir_bessel_i1_scaled`.
pub fn i1_scaled(x: f32) -> f32 {
    let y = x.abs();
    if y == 0.0 {
        return 0.0;
    }
    if y < ROOT_EIGHT * SQRT_DBL_EPSILON {
        return 0.5 * x;
    }
    if y <= 3.0 {
        return x * (-y).exp() * (0.875 + cheb(y * y / 4.5 - 1.0, &BI1));
    }
    let c = if y <= 8.0 {
        cheb((48.0 / y - 11.0) / 5.0, &AI1)
    } else {
        cheb(16.0 / y - 1.0, &AI12)
    };
    let b = (0.375 + c) / y.sqrt();
    if x > 0.0 {
        b
    } else {
        -b
    }
}

/// `I_1(x)` in `f32`; overflows past `|x| ~ 88` with the sign of its
/// argument. Mirrors `petir_bessel_i1`.
pub fn i1(x: f32) -> f32 {
    let y = x.abs();
    if y == 0.0 {
        return 0.0;
    }
    if y < ROOT_EIGHT * SQRT_DBL_EPSILON {
        return 0.5 * x;
    }
    if y <= 3.0 {
        return x * (0.875 + cheb(y * y / 4.5 - 1.0, &BI1));
    }
    y.exp() * i1_scaled(x)
}

/// `e^{x} K_0(x)` in `f32`, `NaN` for `x <= 0`. Mirrors
/// `petir_bessel_k0_scaled`.
pub fn k0_scaled(x: f32) -> f32 {
    if !(x > 0.0) {
        return f32::NAN;
    }
    if x < 1.0 {
        let x2 = x * x;
        let i0v = 1.0 + 0.25 * x2 * poly(0.25 * x2, &I0_POLY);
        return x.exp() * (poly(x2, &K0_POLY) - x.ln() * i0v);
    }
    if x <= 8.0 {
        return (1.203125 + cheb((16.0 / x - 9.0) / 7.0, &AK0)) / x.sqrt();
    }
    (1.25 + cheb(16.0 / x - 1.0, &AK02)) / x.sqrt()
}

/// `K_0(x)` in `f32`, `NaN` for `x <= 0`. Mirrors `petir_bessel_k0`.
pub fn k0(x: f32) -> f32 {
    if !(x > 0.0) {
        return f32::NAN;
    }
    if x < 1.0 {
        let x2 = x * x;
        let i0v = 1.0 + 0.25 * x2 * poly(0.25 * x2, &I0_POLY);
        return poly(x2, &K0_POLY) - x.ln() * i0v;
    }
    (-x).exp() * k0_scaled(x)
}

/// `e^{x} K_1(x)` in `f32`, `NaN` for `x <= 0`. Mirrors
/// `petir_bessel_k1_scaled`.
pub fn k1_scaled(x: f32) -> f32 {
    if !(x > 0.0) {
        return f32::NAN;
    }
    if x < 1.0 {
        let x2 = x * x;
        let t = 0.25 * x2;
        let i1v = 0.5 * x * (1.0 + t * (0.5 + t * poly(t, &I1_POLY)));
        return x.exp() * (x2 * poly(x2, &K1_POLY) + x * x.ln() * i1v + 1.0) / x;
    }
    if x <= 8.0 {
        return (1.375 + cheb((16.0 / x - 9.0) / 7.0, &AK1)) / x.sqrt();
    }
    (1.25 + cheb(16.0 / x - 1.0, &AK12)) / x.sqrt()
}

/// `K_1(x)` in `f32`, `NaN` for `x <= 0`. Mirrors `petir_bessel_k1`.
pub fn k1(x: f32) -> f32 {
    if !(x > 0.0) {
        return f32::NAN;
    }
    if x < 1.0 {
        let x2 = x * x;
        let t = 0.25 * x2;
        let i1v = 0.5 * x * (1.0 + t * (0.5 + t * poly(t, &I1_POLY)));
        return (x2 * poly(x2, &K1_POLY) + x * x.ln() * i1v + 1.0) / x;
    }
    (-x).exp() * k1_scaled(x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::specfunc::bessel as f64_bessel;

    /// A sweep spanning every branch: the small-argument cuts, the `3`/`4`/`8`
    /// Chebyshev boundaries, and the asymptotic tails.
    fn sweep() -> impl Iterator<Item = f32> {
        (1..=400).map(|k| 0.125 * k as f32)
    }

    /// Worst relative difference between a mirror function and its `f64`
    /// original, over a filtered sweep. Returns `(worst, x)`.
    fn worst_vs_f64(
        mirror: impl Fn(f32) -> f32,
        exact: impl Fn(f64) -> f64,
        keep: impl Fn(f32) -> bool,
    ) -> (f32, f32) {
        let (mut worst, mut at) = (0.0_f32, 0.0_f32);
        for x in sweep().filter(|x| keep(*x)) {
            let r = exact(x as f64);
            if r == 0.0 || !r.is_finite() {
                continue;
            }
            let e = ((mirror(x) as f64 - r) / r).abs() as f32;
            if e > worst {
                worst = e;
                at = x;
            }
        }
        (worst, at)
    }

    /// The `f32` mirror against PETIR's `f64` Bessel functions — what `f32`
    /// costs, measured rather than assumed.
    ///
    /// # Results, measured 2026-09-19 over `x` in `[0.125, 50]`, 400 points
    ///
    /// | function | worst relative | at |
    /// |---|---|---|
    /// | `I_0` | 1.576e-07 | 12.625 |
    /// | `I_1` | 1.748e-07 | 33.250 |
    /// | `K_0` | 1.629e-07 | 17.500 |
    /// | `K_1` | 1.736e-07 | 24.000 |
    /// | `J_0` | 2.744e-06 | 30.625 |
    /// | `J_1` | 7.354e-06 | 3.875 |
    /// | `Y_0` | 1.288e-05 | 16.500 |
    /// | `Y_1` | 2.786e-05 | 11.750 |
    ///
    /// Every location in the lower half is a zero: `J_0` has one at 30.635,
    /// `J_1` at 3.8317, `Y_0` at 16.5366, `Y_1` at 11.7492. See the module
    /// documentation — away from the zeros all eight sit at one `f32` ulp.
    #[test]
    fn the_f32_cost_is_what_it_was_measured_to_be() {
        let cases: [(&str, fn(f32) -> f32, fn(f64) -> f64, f32); 8] = [
            ("I_0", i0, f64_bessel::bessel_i0, 3e-7),
            ("I_1", i1, f64_bessel::bessel_i1, 3e-7),
            ("K_0", k0, f64_bessel::bessel_k0, 3e-7),
            ("K_1", k1, f64_bessel::bessel_k1, 3e-7),
            ("J_0", j0, f64_bessel::bessel_j0, 5e-5),
            ("J_1", j1, f64_bessel::bessel_j1, 5e-5),
            ("Y_0", y0, f64_bessel::bessel_y0, 5e-5),
            ("Y_1", y1, f64_bessel::bessel_y1, 5e-5),
        ];
        for (name, m, e, budget) in cases {
            let (worst, at) = worst_vs_f64(m, e, |_| true);
            assert!(
                worst < budget,
                "{name}: worst f32 relative error {worst:e} at x = {at} exceeds {budget:e}"
            );
        }
    }

    /// Away from the zeros every function is at one `f32` ulp — which is the
    /// claim that makes the table above readable rather than alarming.
    ///
    /// `|f| > 0.05` is the filter. It is not tuned: any threshold that
    /// excludes the immediate neighbourhood of a zero gives the same answer,
    /// because the error is a cliff at the zero and flat everywhere else.
    #[test]
    fn away_from_the_zeros_every_function_is_at_the_f32_floor() {
        let cases: [(&str, fn(f32) -> f32, fn(f64) -> f64); 8] = [
            ("I_0", i0, f64_bessel::bessel_i0),
            ("I_1", i1, f64_bessel::bessel_i1),
            ("K_0", k0, f64_bessel::bessel_k0),
            ("K_1", k1, f64_bessel::bessel_k1),
            ("J_0", j0, f64_bessel::bessel_j0),
            ("J_1", j1, f64_bessel::bessel_j1),
            ("Y_0", y0, f64_bessel::bessel_y0),
            ("Y_1", y1, f64_bessel::bessel_y1),
        ];
        for (name, m, e) in cases {
            let mut worst = 0.0_f32;
            let mut at = 0.0_f32;
            for x in sweep() {
                let r = e(x as f64);
                if !r.is_finite() || r.abs() <= 0.05 {
                    continue;
                }
                let rel = (((m(x) as f64) - r) / r).abs() as f32;
                if rel > worst {
                    worst = rel;
                    at = x;
                }
            }
            assert!(
                worst < 1e-6,
                "{name} away from its zeros: {worst:e} at {at}, not the f32 floor"
            );
        }
    }

    /// The asymptotic branch past `x = 4` is **not** worse than the Chebyshev
    /// branch below it, contradicting the prediction recorded in this module's
    /// documentation.
    ///
    /// The prediction was that the phase `x` carrying `f32` representation
    /// error would degrade the asymptotic branch measurably by `x = 50`. It
    /// does not: the two branches agree to within a factor of four, and `J_1`
    /// is worse *below* the cut. Keeping the assertion means the documented
    /// explanation stays falsifiable.
    #[test]
    fn the_asymptotic_branch_is_no_worse_than_the_chebyshev_one() {
        let cases: [(&str, fn(f32) -> f32, fn(f64) -> f64); 4] = [
            ("J_0", j0, f64_bessel::bessel_j0),
            ("J_1", j1, f64_bessel::bessel_j1),
            ("Y_0", y0, f64_bessel::bessel_y0),
            ("Y_1", y1, f64_bessel::bessel_y1),
        ];
        for (name, m, e) in cases {
            let (mut lo, mut hi) = (0.0_f32, 0.0_f32);
            for x in sweep() {
                let r = e(x as f64);
                if !r.is_finite() || r.abs() <= 0.05 {
                    continue;
                }
                let rel = (((m(x) as f64) - r) / r).abs() as f32;
                if x < 4.0 {
                    lo = lo.max(rel);
                } else {
                    hi = hi.max(rel);
                }
            }
            assert!(
                hi < 4.0 * lo.max(1e-7),
                "{name}: the asymptotic branch ({hi:e}) is documented as no \
                 worse than the Chebyshev one ({lo:e}); if that has changed, \
                 re-measure and rewrite the module docs rather than deleting \
                 this assertion"
            );
        }
    }

    /// For `J` and `Y` the **absolute** error is the bounded quantity, since
    /// the relative one is unbounded at a zero. Measured over the same sweep:
    /// `J_0` 9.927e-08, `J_1` 1.275e-07, `Y_0` 2.396e-07, `Y_1` 7.542e-07.
    #[test]
    fn the_oscillatory_functions_are_bounded_in_absolute_error() {
        let cases: [(&str, fn(f32) -> f32, fn(f64) -> f64); 4] = [
            ("J_0", j0, f64_bessel::bessel_j0),
            ("J_1", j1, f64_bessel::bessel_j1),
            ("Y_0", y0, f64_bessel::bessel_y0),
            ("Y_1", y1, f64_bessel::bessel_y1),
        ];
        for (name, m, e) in cases {
            let mut worst = 0.0_f32;
            for x in sweep() {
                let r = e(x as f64);
                if !r.is_finite() {
                    continue;
                }
                worst = worst.max((((m(x) as f64) - r).abs()) as f32);
            }
            assert!(worst < 2e-6, "{name} absolute error {worst:e}");
        }
    }

    /// The `J`/`Y` Wronskian in `f32`. It constrains all six tables at once
    /// and cannot be satisfied by a mirror that copied one of them wrongly.
    #[test]
    fn the_j_y_wronskian_holds_in_f32() {
        let mut worst = 0.0_f32;
        for x in sweep() {
            let w = j0(x) * y1(x) - j1(x) * y0(x);
            let exact = -2.0 / (core::f32::consts::PI * x);
            worst = worst.max(((w - exact) / exact).abs());
        }
        assert!(worst < 1e-3, "J/Y Wronskian in f32: {worst:e}");
    }

    /// The `I`/`K` Wronskian in `f32`.
    #[test]
    fn the_i_k_wronskian_holds_in_f32() {
        let mut worst = 0.0_f32;
        for x in sweep() {
            let w = i0(x) * k1(x) + i1(x) * k0(x);
            worst = worst.max((w * x - 1.0).abs());
        }
        assert!(worst < 1e-5, "I/K Wronskian in f32: {worst:e}");
    }

    /// The scaled forms agree with the unscaled ones where both are
    /// representable in `f32` — a much narrower window than in `f64`, since
    /// `e^{x}` leaves `f32` at `x = 88`.
    #[test]
    fn the_scaled_forms_agree_with_the_unscaled_ones() {
        let mut worst = 0.0_f32;
        for x in sweep().filter(|&x| x <= 40.0) {
            for (scaled, plain, factor) in [
                (i0_scaled(x), i0(x), (-x).exp()),
                (i1_scaled(x), i1(x), (-x).exp()),
                (k0_scaled(x), k0(x), x.exp()),
                (k1_scaled(x), k1(x), x.exp()),
            ] {
                let expect = plain * factor;
                if expect != 0.0 && expect.is_finite() {
                    worst = worst.max(((scaled - expect) / expect).abs());
                }
            }
        }
        assert!(
            worst < 1e-5,
            "scaled/unscaled disagreement in f32: {worst:e}"
        );
    }

    /// The second-kind functions reject non-positive arguments, and the
    /// shader's `!(x > 0.0)` spelling of that test propagates `NaN` too.
    #[test]
    fn the_second_kind_functions_reject_non_positive_arguments_and_nan() {
        for x in [-1.0, 0.0, f32::NAN] {
            assert!(y0(x).is_nan(), "Y_0({x})");
            assert!(y1(x).is_nan(), "Y_1({x})");
            assert!(k0(x).is_nan(), "K_0({x})");
            assert!(k1(x).is_nan(), "K_1({x})");
        }
    }

    /// Parity is carried by explicit sign branches rather than by the series,
    /// so it is pinned exactly.
    #[test]
    fn the_parities_hold_exactly() {
        for x in sweep() {
            assert_eq!(j0(x), j0(-x));
            assert_eq!(i0(x), i0(-x));
            assert_eq!(j1(x), -j1(-x));
            assert_eq!(i1(x), -i1(-x));
        }
    }

    /// `I_0` and `I_1` leave `f32` near `x = 88`, far earlier than the
    /// `f64` module's `x = 709`. The scaled forms must still work there,
    /// which is the whole reason they are in the shader.
    ///
    /// `K_0(100)` does **not** flush to zero as expected — it lands on
    /// 4e-45, a `f32` subnormal, and the assertion below records the measured
    /// value rather than the guess. Subnormals carry only a few bits of
    /// significand, so this is a value to detect and not to compute with;
    /// that is what `k0_scaled` is for.
    #[test]
    fn the_modified_functions_overflow_f32_near_88_and_the_scaled_ones_do_not() {
        assert!(i0(100.0).is_infinite());
        let k = k0(100.0);
        assert!(k > 0.0 && k < 1e-44, "K_0(100) in f32 is {k:e}");
        let x = 100.0_f32;
        let i0s = i0_scaled(x);
        let k0s = k0_scaled(x);
        assert!(i0s.is_finite() && i0s > 0.0);
        assert!(k0s.is_finite() && k0s > 0.0);
        // Their product tends to 1/(2x).
        assert!((i0s * k0s * 2.0 * x - 1.0).abs() < 1e-2);
    }
}
