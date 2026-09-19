//! `f32` mirror of `shaders/ellint.wgsl` — Carlson's symmetric forms and the
//! Legendre elliptic integrals built on them.
//!
//! See [`crate::wgsl::mirror`] for why a mirror exists at all.
//!
//! # Written by hand, like [`crate::wgsl::mirror_dilog`] and for the same
//! reason
//!
//! Most of the specfunc mirrors here are *generated* from one parse of their
//! `f64` module, because each carries hundreds of Chebyshev coefficients that
//! must not drift between shader and mirror. **This kernel has no coefficient
//! tables at all** — the Carlson forms converge by duplication and close with
//! a short polynomial in the deviations — so there is nothing for a generator
//! to extract and both sides are written by hand. The nine shared constants
//! are pinned against the shader text by
//! `the_shader_and_this_mirror_agree_on_their_constants`, which parses the
//! WGSL rather than restating it.
//!
//! # Two parameters, both measured rather than assumed
//!
//! **`errtol = 0.03` is upstream's own single-precision value**, not a
//! retargeting. GSL's routines read exactly one thing from their `gsl_mode_t`:
//! `GSL_PREC_DOUBLE` gives `errtol = 0.001` and anything else gives `0.03`,
//! for a documented relative error of about `2e-08`. That is five orders
//! below `f32::EPSILON`, so [`crate::specfunc::ellint::Mode::Single`] is free
//! here and halves the iteration count — see that module's
//! `the_single_precision_mode_costs_what_it_was_measured_to_cost`.
//!
//! **The iteration cap is 16, against upstream's 10000.** GSL's `nmax` is a
//! can't-happen bound rather than a working one. Measured at `errtol = 0.03`
//! over the whole `f32`-reachable domain — `x` and `y` swept geometrically
//! from `5 FLT_MIN` to `0.2 FLT_MAX`, 121 x 121 pairs — `R_C` takes at most 9
//! duplication steps, `R_F` and `R_D` at most 8, and a whole `R_J` at most 13
//! including the `R_C` it calls at every step. On a GPU that ceiling is not
//! free the way it is on a CPU: a uniform loop bound is what the compiler
//! unrolls against.
//!
//! `the_iteration_cap_is_enough_and_was_measured_not_guessed` re-measures the
//! high-water mark through this mirror and fails if it ever reaches the cap,
//! so the number above cannot go stale silently.
//!
//! # `R_J` calls `R_C` inside its own loop, and that is fine
//!
//! Recorded as an open question when the `f64` module landed, and it is not
//! one: **WGSL forbids recursion, not nested calls**, and `R_C` never calls
//! back. The shader compiles and runs as written.
//!
//! # What `f32` costs
//!
//! Worst relative difference against [`crate::specfunc::ellint`] evaluated in
//! `f64` with [`Mode::Single`](crate::specfunc::ellint::Mode::Single),
//! measured 2026-09-19:
//!
//! | surface | sweep | worst relative | at |
//! |---|---|---|---|
//! | `K(k)` | 401 moduli in `[0, 0.999]` | 8.034e-07 | 0.999 |
//! | `E(k)` | same | 6.268e-07 | 0.9965 |
//! | `D(k)` | same | 1.048e-06 | 0.999 |
//! | `Pi(k, 0.3)` | same | 7.797e-07 | 0.999 |
//! | `F(phi, k)` | 41 x 41 over `phi` in `[0, pi]`, `k` in `[0, 0.99]` | 1.181e-06 | |
//! | `E(phi, k)` | same | 1.676e-06 | |
//!
//! **Every one is within fifteen `f32` ulps**, and the four complete
//! integrals within nine. That is as good as this module gets and better than
//! most of the Chebyshev shaders here, for a structural reason: the Carlson
//! forms are a short sequence of square roots and averages, not a long sum of
//! coefficient products, so there is no accumulation to lose. The worst point
//! is at the largest modulus in every case, which is where `K` and `D` are
//! climbing toward their logarithmic divergence at `k = 1`.
//!
//! Legendre's relation — four evaluations at two complementary moduli, an
//! identity with no free parameter — holds to **8.348e-07**, worst at
//! `k = 0.1`.
//!
//! Not at `k = 0.01`, which is where the `f64` module's worst point is and
//! where this was expected to be: there `k'` is nearest 1, so `K(k')` is
//! nearest its logarithmic divergence. In `f32` that is exactly the point
//! that goes through the **A&S series instead of the Carlson form** —
//! `k'^2 = 0.9999` is past the `1 - sqrt(f32::EPSILON)` switch at
//! `0.99965`, where in `f64` the switch does not come until `1 - 1.49e-08`.
//! The wider epsilon moves the branch, and the branch it moves to is the
//! more accurate one there. `f32`'s worst point for this identity is
//! therefore an ordinary interior modulus, not the awkward one.
//!
//! # The domain bounds are recomputed, not retargeted by hand
//!
//! Every one of the six is GSL's own expression evaluated at `f32`'s
//! `MIN_POSITIVE` and `MAX` rather than a rounded stand-in, and
//! `the_domain_bounds_are_what_the_formulae_give` recomputes all six and
//! compares. That matters because these are **range guards** in the taxonomy
//! of `docs/wgsl-coverage.md`: they come from the exponent range, which is
//! exactly the kind of constant that must move with the width.

// Under a std-linked build (`cargo test`) f32's inherent sqrt/sin/ln shadow
// these trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

/// Upstream's single-precision duplication tolerance, `GSL_PREC_SINGLE`'s
/// `errtol`. Mirrors `PETIR_ELL_ERRTOL`.
const ERRTOL: f32 = 0.03;

/// The duplication-iteration cap. **16, not upstream's 10000** — see the
/// module documentation. Mirrors `PETIR_ELL_NMAX`.
const NMAX: u32 = 16;

/// `5 f32::MIN_POSITIVE`. Mirrors `PETIR_ELL_RF_LOLIM`.
const RF_LOLIM: f32 = 5.8774718e-38;
/// `0.2 f32::MAX`. Mirrors `PETIR_ELL_RF_UPLIM`.
const RF_UPLIM: f32 = 6.8056469e37;
/// `2 / f32::MAX^(2/3)`. Mirrors `PETIR_ELL_RD_LOLIM`.
const RD_LOLIM: f32 = 4.1033357e-26;
/// `(0.1 errtol / f32::MIN_POSITIVE)^(2/3)`. Mirrors `PETIR_ELL_RD_UPLIM`.
const RD_UPLIM: f32 = 4.0234673e23;
/// `(5 f32::MIN_POSITIVE)^(1/3)`. Mirrors `PETIR_ELL_RJ_LOLIM`.
const RJ_LOLIM: f32 = 3.8880352e-13;
/// `0.3 (0.2 f32::MAX)^(1/3)`. Mirrors `PETIR_ELL_RJ_UPLIM`.
const RJ_UPLIM: f32 = 1.2248354e12;
/// `sqrt(f32::EPSILON)`, where `K` and `E` switch to the A&S series.
/// Mirrors `PETIR_ELL_SQRT_EPS`.
const SQRT_EPS: f32 = 3.4526698e-4;

/// `f32::consts::PI`, which is what the shader's `3.1415927` literal is.
const PI: f32 = core::f32::consts::PI;

/// The high-water mark of duplication steps taken by any Carlson form since
/// it was last reset, so `the_iteration_cap_is_enough_and_was_measured_not_guessed`
/// can *measure* that [`NMAX`] is not reached instead of asserting it.
#[cfg(test)]
pub(crate) static WORST_STEPS: core::sync::atomic::AtomicU32 =
    core::sync::atomic::AtomicU32::new(0);

/// Record how many duplication steps a loop took.
#[inline]
fn note_steps(_n: u32) {
    #[cfg(test)]
    WORST_STEPS.fetch_max(_n, core::sync::atomic::Ordering::Relaxed);
}

fn max3(x: f32, y: f32, z: f32) -> f32 {
    x.max(y).max(z)
}

fn max4(x: f32, y: f32, z: f32, w: f32) -> f32 {
    x.max(y).max(z).max(w)
}

/// WGSL's `sign`, which differs from [`f32::signum`] at zero: `sign(0.0)` is
/// `0.0` where `signum` is `1.0`.
///
/// Only [`ellint_e`]'s `cos^2 phi < eps` branch reads it, where `sin phi` is
/// within `sqrt(eps)` of `±1` and never zero — so the difference cannot be
/// reached. It is mirrored anyway rather than papered over, because a mirror
/// that quietly differs from its shader is the one defect this whole module
/// exists to make impossible.
fn wgsl_sign(x: f32) -> f32 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// Carlson's `R_C(x,y) = R_F(x,y,y)`. Mirrors `petir_ellint_rc`.
pub fn rc(x: f32, y: f32) -> f32 {
    rc_with(x, y, NMAX)
}

/// [`rc`] with the iteration cap supplied, so the cap can be measured.
fn rc_with(x: f32, y: f32, nmax: u32) -> f32 {
    if !(x >= 0.0) || !(y >= 0.0) {
        return f32::NAN;
    }
    if x + y < RF_LOLIM || x.max(y) >= RF_UPLIM {
        return f32::NAN;
    }
    const C1: f32 = 1.0 / 7.0;
    const C2: f32 = 9.0 / 22.0;
    let (mut xn, mut yn) = (x, y);
    let (mut mu, mut sn) = (0.0_f32, 0.0_f32);
    let mut steps = nmax;
    for n in 0..nmax {
        mu = (xn + yn + yn) / 3.0;
        sn = (yn + mu) / mu - 2.0;
        if sn.abs() < ERRTOL {
            steps = n;
            break;
        }
        let lamda = 2.0 * xn.sqrt() * yn.sqrt() + yn;
        xn = (xn + lamda) * 0.25;
        yn = (yn + lamda) * 0.25;
    }
    note_steps(steps);
    let s = sn * sn * (0.3 + sn * (C1 + sn * (0.375 + sn * C2)));
    (1.0 + s) / mu.sqrt()
}

/// Carlson's `R_D(x,y,z)`. Mirrors `petir_ellint_rd`.
pub fn rd(x: f32, y: f32, z: f32) -> f32 {
    rd_with(x, y, z, NMAX)
}

/// [`rd`] with the iteration cap supplied.
fn rd_with(x: f32, y: f32, z: f32, nmax: u32) -> f32 {
    if !(x >= 0.0) || !(y >= 0.0) || !(z >= 0.0) {
        return f32::NAN;
    }
    if (x + y).min(z) < RD_LOLIM || max3(x, y, z) >= RD_UPLIM {
        return f32::NAN;
    }
    const C1: f32 = 3.0 / 14.0;
    const C2: f32 = 1.0 / 6.0;
    const C3: f32 = 9.0 / 22.0;
    const C4: f32 = 3.0 / 26.0;
    let (mut xn, mut yn, mut zn) = (x, y, z);
    let mut sigma = 0.0_f32;
    let mut power4 = 1.0_f32;
    let (mut mu, mut xndev, mut yndev, mut zndev) = (0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32);
    let mut steps = nmax;
    for n in 0..nmax {
        mu = (xn + yn + 3.0 * zn) * 0.2;
        xndev = (mu - xn) / mu;
        yndev = (mu - yn) / mu;
        zndev = (mu - zn) / mu;
        if max3(xndev.abs(), yndev.abs(), zndev.abs()) < ERRTOL {
            steps = n;
            break;
        }
        let (xr, yr, zr) = (xn.sqrt(), yn.sqrt(), zn.sqrt());
        let lamda = xr * (yr + zr) + yr * zr;
        sigma += power4 / (zr * (zn + lamda));
        power4 *= 0.25;
        xn = (xn + lamda) * 0.25;
        yn = (yn + lamda) * 0.25;
        zn = (zn + lamda) * 0.25;
    }
    note_steps(steps);
    let ea = xndev * yndev;
    let eb = zndev * zndev;
    let ec = ea - eb;
    let ed = ea - 6.0 * eb;
    let ef = ed + ec + ec;
    let s1 = ed * (-C1 + 0.25 * C3 * ed - 1.5 * C4 * zndev * ef);
    let s2 = zndev * (C2 * ef + zndev * (-C3 * ec + zndev * C4 * ea));
    3.0 * sigma + power4 * (1.0 + s1 + s2) / (mu * mu.sqrt())
}

/// Carlson's `R_F(x,y,z)`. Mirrors `petir_ellint_rf`.
pub fn rf(x: f32, y: f32, z: f32) -> f32 {
    rf_with(x, y, z, NMAX)
}

/// [`rf`] with the iteration cap supplied.
fn rf_with(x: f32, y: f32, z: f32, nmax: u32) -> f32 {
    if !(x >= 0.0) || !(y >= 0.0) || !(z >= 0.0) {
        return f32::NAN;
    }
    if x + y < RF_LOLIM || x + z < RF_LOLIM || y + z < RF_LOLIM {
        return f32::NAN;
    }
    if max3(x, y, z) >= RF_UPLIM {
        return f32::NAN;
    }
    const C1: f32 = 1.0 / 24.0;
    const C2: f32 = 3.0 / 44.0;
    const C3: f32 = 1.0 / 14.0;
    let (mut xn, mut yn, mut zn) = (x, y, z);
    let (mut mu, mut xndev, mut yndev, mut zndev) = (0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32);
    let mut steps = nmax;
    for n in 0..nmax {
        mu = (xn + yn + zn) / 3.0;
        xndev = 2.0 - (mu + xn) / mu;
        yndev = 2.0 - (mu + yn) / mu;
        zndev = 2.0 - (mu + zn) / mu;
        if max3(xndev.abs(), yndev.abs(), zndev.abs()) < ERRTOL {
            steps = n;
            break;
        }
        let (xr, yr, zr) = (xn.sqrt(), yn.sqrt(), zn.sqrt());
        let lamda = xr * (yr + zr) + yr * zr;
        xn = (xn + lamda) * 0.25;
        yn = (yn + lamda) * 0.25;
        zn = (zn + lamda) * 0.25;
    }
    note_steps(steps);
    let e2 = xndev * yndev - zndev * zndev;
    let e3 = xndev * yndev * zndev;
    let s = 1.0 + (C1 * e2 - 0.1 - C2 * e3) * e2 + C3 * e3;
    s / mu.sqrt()
}

/// Carlson's `R_J(x,y,z,p)`. Mirrors `petir_ellint_rj`.
///
/// Calls [`rc`] once per duplication step — a nested call, not recursion,
/// which is what WGSL forbids.
pub fn rj(x: f32, y: f32, z: f32, p: f32) -> f32 {
    rj_with(x, y, z, p, NMAX)
}

/// [`rj`] with the iteration cap supplied.
fn rj_with(x: f32, y: f32, z: f32, p: f32, nmax: u32) -> f32 {
    if !(x >= 0.0) || !(y >= 0.0) || !(z >= 0.0) || !(p >= 0.0) {
        return f32::NAN;
    }
    if x + y < RJ_LOLIM || x + z < RJ_LOLIM || y + z < RJ_LOLIM || p < RJ_LOLIM {
        return f32::NAN;
    }
    if max4(x, y, z, p) >= RJ_UPLIM {
        return f32::NAN;
    }
    const C1: f32 = 3.0 / 14.0;
    const C2: f32 = 1.0 / 3.0;
    const C3: f32 = 3.0 / 22.0;
    const C4: f32 = 3.0 / 26.0;
    let (mut xn, mut yn, mut zn, mut pn) = (x, y, z, p);
    let mut sigma = 0.0_f32;
    let mut power4 = 1.0_f32;
    let (mut mu, mut xndev, mut yndev, mut zndev, mut pndev) =
        (0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32);
    let mut steps = nmax;
    for n in 0..nmax {
        mu = (xn + yn + zn + pn + pn) * 0.2;
        xndev = (mu - xn) / mu;
        yndev = (mu - yn) / mu;
        zndev = (mu - zn) / mu;
        pndev = (mu - pn) / mu;
        if max4(xndev.abs(), yndev.abs(), zndev.abs(), pndev.abs()) < ERRTOL {
            steps = n;
            break;
        }
        let (xr, yr, zr) = (xn.sqrt(), yn.sqrt(), zn.sqrt());
        let lamda = xr * (yr + zr) + yr * zr;
        let alfa = pn * (xr + yr + zr) + xr * yr * zr;
        let alfa = alfa * alfa;
        let beta = pn * (pn + lamda) * (pn + lamda);
        let rcv = rc_with(alfa, beta, nmax);
        if rcv.is_nan() {
            return f32::NAN;
        }
        sigma += power4 * rcv;
        power4 *= 0.25;
        xn = (xn + lamda) * 0.25;
        yn = (yn + lamda) * 0.25;
        zn = (zn + lamda) * 0.25;
        pn = (pn + lamda) * 0.25;
    }
    note_steps(steps);
    let ea = xndev * (yndev + zndev) + yndev * zndev;
    let eb = xndev * yndev * zndev;
    let ec = pndev * pndev;
    let e2 = ea - 3.0 * ec;
    let e3 = eb + 2.0 * pndev * (ea - ec);
    let s1 = 1.0 + e2 * (-C1 + 0.75 * C3 * e2 - 1.5 * C4 * e3);
    let s2 = eb * (0.5 * C2 + pndev * (-C3 - C3 + pndev * C4));
    let s3 = pndev * ea * (C2 - pndev * C3) - C2 * pndev * ec;
    3.0 * sigma + power4 * (s1 + s2 + s3) / (mu * mu.sqrt())
}

/// The complete elliptic integral of the first kind `K(k)`. Mirrors
/// `petir_ellint_kcomp`. `NaN` for `k^2 >= 1`.
pub fn kcomp(k: f32) -> f32 {
    if !(k * k < 1.0) {
        return f32::NAN;
    }
    if k * k >= 1.0 - SQRT_EPS {
        // A&S 17.3.34.
        let y = 1.0 - k * k;
        let ta = 1.386_294_4 + y * (0.096_663_44 + y * 0.035_900_924);
        let tb = -y.ln() * (0.5 + y * (0.124_985_94 + y * 0.068_802_49));
        return ta + tb;
    }
    rf(0.0, 1.0 - k * k, 1.0)
}

/// The complete elliptic integral of the second kind `E(k)`. Mirrors
/// `petir_ellint_ecomp`. `NaN` for `k^2 >= 1`.
pub fn ecomp(k: f32) -> f32 {
    if !(k * k < 1.0) {
        return f32::NAN;
    }
    if k * k >= 1.0 - SQRT_EPS {
        // A&S 17.3.36.
        let y = 1.0 - k * k;
        let ta = 1.0 + y * (0.443_251_42 + y * (0.062_606_014 + 0.047_573_835 * y));
        let tb = -y * y.ln() * (0.249_983_68 + y * (0.092_001_8 + 0.040_696_975 * y));
        return ta + tb;
    }
    let y = 1.0 - k * k;
    rf(0.0, y, 1.0) - k * k / 3.0 * rd(0.0, y, 1.0)
}

/// The complete `D(k) = (K - E)/k^2`. Mirrors `petir_ellint_dcomp`.
pub fn dcomp(k: f32) -> f32 {
    if !(k * k < 1.0) {
        return f32::NAN;
    }
    (1.0 / 3.0) * rd(0.0, 1.0 - k * k, 1.0)
}

/// The complete elliptic integral of the third kind `Pi(k, n)`. Mirrors
/// `petir_ellint_pcomp`.
pub fn pcomp(k: f32, n: f32) -> f32 {
    if !(k * k < 1.0) {
        return f32::NAN;
    }
    let y = 1.0 - k * k;
    rf(0.0, y, 1.0) - (n / 3.0) * rj(0.0, y, 1.0, 1.0 + n)
}

/// The incomplete integral of the first kind `F(phi, k)`, `phi` in radians.
/// Mirrors `petir_ellint_f`.
pub fn ellint_f(phi_in: f32, k: f32) -> f32 {
    if phi_in.is_nan() || k.is_nan() {
        return f32::NAN;
    }
    let nc = (phi_in / PI + 0.5).floor();
    let phi = phi_in - nc * PI;
    let sp = phi.sin();
    let s2 = sp * sp;
    let v = sp * rf(1.0 - s2, 1.0 - k * k * s2, 1.0);
    if nc == 0.0 {
        v
    } else {
        v + 2.0 * nc * kcomp(k)
    }
}

/// The incomplete integral of the second kind `E(phi, k)`. Mirrors
/// `petir_ellint_e`.
pub fn ellint_e(phi_in: f32, k: f32) -> f32 {
    if phi_in.is_nan() || k.is_nan() {
        return f32::NAN;
    }
    let nc = (phi_in / PI + 0.5).floor();
    let phi = phi_in - nc * PI;
    let sp = phi.sin();
    let s2 = sp * sp;
    let x = 1.0 - s2;
    let y = 1.0 - k * k * s2;
    if x < f32::EPSILON {
        let re = ecomp(k);
        return 2.0 * nc * re + wgsl_sign(sp) * re;
    }
    let s3 = s2 * sp;
    let v = sp * rf(x, y, 1.0) - k * k / 3.0 * s3 * rd(x, y, 1.0);
    if nc == 0.0 {
        v
    } else {
        v + 2.0 * nc * ecomp(k)
    }
}

/// The incomplete integral of the third kind `Pi(phi, k, n)`. Mirrors
/// `petir_ellint_p`.
pub fn ellint_p(phi_in: f32, k: f32, n: f32) -> f32 {
    if phi_in.is_nan() || k.is_nan() || n.is_nan() {
        return f32::NAN;
    }
    let nc = (phi_in / PI + 0.5).floor();
    let phi = phi_in - nc * PI;
    let sp = phi.sin();
    let s2 = sp * sp;
    let s3 = s2 * sp;
    let x = 1.0 - s2;
    let y = 1.0 - k * k * s2;
    let v = sp * rf(x, y, 1.0) - n / 3.0 * s3 * rj(x, y, 1.0, 1.0 + n * s2);
    if nc == 0.0 {
        v
    } else {
        v + 2.0 * nc * pcomp(k, n)
    }
}

/// The incomplete integral `D(phi, k)`. Mirrors `petir_ellint_d`.
pub fn ellint_d(phi_in: f32, k: f32) -> f32 {
    if phi_in.is_nan() || k.is_nan() {
        return f32::NAN;
    }
    let nc = (phi_in / PI + 0.5).floor();
    let phi = phi_in - nc * PI;
    let sp = phi.sin();
    let s2 = sp * sp;
    let s3 = s2 * sp;
    let v = s3 / 3.0 * rd(1.0 - s2, 1.0 - k * k * s2, 1.0);
    if nc == 0.0 {
        v
    } else {
        v + 2.0 * nc * dcomp(k)
    }
}

/// The four complete integrals behind one selector, in the order
/// `K, E, D, Pi`. Mirrors `petir_ellint_comp`, which is the entry point the
/// GPU test dispatches. `n` is read only by `Pi`; any other `which` is `NaN`.
pub fn ellint_comp(which: u32, k: f32, n: f32) -> f32 {
    match which {
        0 => kcomp(k),
        1 => ecomp(k),
        2 => dcomp(k),
        3 => pcomp(k, n),
        _ => f32::NAN,
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::specfunc::ellint as f64_ellint;
    use crate::specfunc::ellint::Mode;
    use core::sync::atomic::Ordering;

    /// Worst relative difference of a `f32` surface against the `f64` module
    /// in [`Mode::Single`], over a sweep of moduli.
    fn worst<F, G>(f: F, g: G) -> (f64, f32)
    where
        F: Fn(f32) -> f32,
        G: Fn(f64) -> f64,
    {
        let (mut w, mut at) = (0.0_f64, 0.0_f32);
        for i in 0..=400 {
            let k = 0.999 * i as f32 / 400.0;
            let r = g(k as f64);
            if !r.is_finite() || r.abs() < 1e-6 {
                continue;
            }
            let e = (((f(k) as f64) - r) / r).abs();
            if e > w {
                w = e;
                at = k;
            }
        }
        (w, at)
    }

    /// **What `f32` costs, per surface.** The `f64` reference is evaluated in
    /// [`Mode::Single`], so this isolates the *width* rather than also
    /// measuring the tolerance.
    ///
    /// Measured 2026-09-19 over 401 moduli in `[0, 0.999]`:
    ///
    /// | | worst relative | at |
    /// |---|---|---|
    /// | `K` | 8.034e-07 | 0.999 |
    /// | `E` | 6.268e-07 | 0.9965025 |
    /// | `D` | 1.048e-06 | 0.999 |
    /// | `Pi(k, 0.3)` | 7.797e-07 | 0.999 |
    ///
    /// and over the two-dimensional `(phi, k)` sweep, `F` 1.181e-06 and
    /// `E(phi, k)` 1.676e-06. The budget asserted is an order above each, so
    /// this fails on a regression rather than on the last bit.
    #[test]
    fn the_f32_cost_is_what_it_was_measured_to_be() {
        let m = Mode::Single;
        let cases: [(&str, f64); 4] = [("K", 0.0), ("E", 0.0), ("D", 0.0), ("Pi", 0.0)];
        let measured = [
            worst(kcomp, |k| f64_ellint::ellint_kcomp(k, m)),
            worst(ecomp, |k| f64_ellint::ellint_ecomp(k, m)),
            worst(dcomp, |k| f64_ellint::ellint_dcomp(k, m)),
            worst(|k| pcomp(k, 0.3), |k| f64_ellint::ellint_pcomp(k, 0.3, m)),
        ];
        for ((name, _), (w, at)) in cases.iter().zip(measured.iter()) {
            std::println!("{name}: {w:e} at k = {at}");
            assert!(*w < 1e-4, "{name}: {w:e} at k = {at}");
        }
        // The incomplete forms, over a two-dimensional sweep.
        let mut wf = 0.0_f64;
        let mut we = 0.0_f64;
        for i in 0..=40 {
            let phi = core::f32::consts::PI * i as f32 / 40.0;
            for j in 0..=40 {
                let k = 0.99 * j as f32 / 40.0;
                let rf64 = f64_ellint::ellint_f(phi as f64, k as f64, m);
                if rf64.abs() > 1e-3 {
                    wf = wf.max((((ellint_f(phi, k) as f64) - rf64) / rf64).abs());
                }
                let re64 = f64_ellint::ellint_e(phi as f64, k as f64, m);
                if re64.abs() > 1e-3 {
                    we = we.max((((ellint_e(phi, k) as f64) - re64) / re64).abs());
                }
            }
        }
        std::println!("F: {wf:e}   E(phi,k): {we:e}");
        assert!(wf < 1e-3, "F(phi,k): {wf:e}");
        assert!(we < 1e-3, "E(phi,k): {we:e}");
    }

    /// **The iteration cap is 16 and the loop never reaches it**, measured
    /// rather than asserted.
    ///
    /// High-water mark over the moduli sweep plus the corners of the
    /// representable domain, 2026-09-19: **9 duplication steps** against a
    /// cap of 16 — the same 9 the `f64` probe found for `R_C`, which is the
    /// longest of the four loops. Upstream's cap is 10000.
    ///
    /// Falling out of the loop at the cap is *not* an error here, unlike in
    /// the `f64` module which returns `NaN`: the shader has no error channel
    /// and a `NaN` returned for an argument that merely needed one more step
    /// would be worse than a slightly unconverged answer. That makes this
    /// test the only thing standing between the cap and a silent loss of
    /// accuracy, which is why it sweeps the corners of the domain rather
    /// than a comfortable interval.
    #[test]
    fn the_iteration_cap_is_enough_and_was_measured_not_guessed() {
        WORST_STEPS.store(0, Ordering::Relaxed);
        for i in 0..=200 {
            let k = 0.9999 * i as f32 / 200.0;
            let _ = kcomp(k);
            let _ = ecomp(k);
            let _ = dcomp(k);
            let _ = pcomp(k, 0.3);
            let _ = ellint_f(1.1, k);
            let _ = ellint_p(1.1, k, 0.3);
        }
        // The extremes of the representable domain, which is where the
        // duplication loop is longest.
        let lo = 5.0 * f32::MIN_POSITIVE;
        let hi = 0.2 * f32::MAX;
        let mut x = lo;
        while x < hi {
            let mut y = lo;
            while y < hi {
                let _ = rc(x, y);
                y *= 1e6;
            }
            x *= 1e6;
        }
        let observed = WORST_STEPS.load(Ordering::Relaxed);
        std::println!("worst duplication steps observed: {observed} against NMAX {NMAX}");
        assert!(
            observed < NMAX,
            "the cap is documented as having room: {observed} steps against a \
             cap of {NMAX}. If the loop is now reaching the cap the answers \
             are silently unconverged"
        );
    }

    /// The six domain bounds are GSL's own expressions evaluated at `f32`'s
    /// range, not hand-rounded stand-ins.
    ///
    /// This is the check that caught a hand-rounded constant in
    /// `mirror_synchrotron` — a literal that both the shader and the mirror
    /// shared, so every accuracy comparison between them stayed green while
    /// both were wrong. A constant that is *computed* here cannot do that.
    #[test]
    fn the_domain_bounds_are_what_the_formulae_give() {
        let min = f32::MIN_POSITIVE as f64;
        let max = f32::MAX as f64;
        let close = |got: f32, want: f64, name: &str| {
            let e = ((got as f64 - want) / want).abs();
            assert!(
                e < 1e-6,
                "{name} is {got:e}, the formula gives {want:e} ({e:e} apart)"
            );
        };
        close(RF_LOLIM, 5.0 * min, "RF_LOLIM = 5 FLT_MIN");
        close(RF_UPLIM, 0.2 * max, "RF_UPLIM = 0.2 FLT_MAX");
        close(RD_LOLIM, 2.0 / max.powf(2.0 / 3.0), "RD_LOLIM");
        close(
            RD_UPLIM,
            (0.1 * ERRTOL as f64 / min).powf(2.0 / 3.0),
            "RD_UPLIM",
        );
        close(RJ_LOLIM, (5.0 * min).powf(1.0 / 3.0), "RJ_LOLIM");
        close(RJ_UPLIM, 0.3 * (0.2 * max).powf(1.0 / 3.0), "RJ_UPLIM");
        close(SQRT_EPS, (f32::EPSILON as f64).sqrt(), "SQRT_EPS");
    }

    /// **Legendre's relation in `f32`** — `E K' + E' K - K K' = pi/2`, exact,
    /// with no free parameter and nothing from a table.
    ///
    /// Four evaluations at two complementary moduli, so it ties `K` and `E`
    /// together rather than checking either against itself.
    #[test]
    fn legendres_relation_survives_f32() {
        let (mut worst, mut at) = (0.0_f32, 0.0_f32);
        for i in 1..=99 {
            let k = i as f32 / 100.0;
            let kp = (1.0 - k * k).sqrt();
            let (kk, ee) = (kcomp(k), ecomp(k));
            let (kk2, ee2) = (kcomp(kp), ecomp(kp));
            let lhs = ee * kk2 + ee2 * kk - kk * kk2;
            let e = ((lhs - core::f32::consts::FRAC_PI_2) / core::f32::consts::FRAC_PI_2).abs();
            if e > worst {
                worst = e;
                at = k;
            }
        }
        std::println!("Legendre's relation in f32: {worst:e} at k = {at}");
        assert!(worst < 1e-4, "Legendre's relation: {worst:e} at k = {at}");
    }

    /// Carlson's degenerate identities, which are closed forms rather than
    /// comparisons against the `f64` module.
    #[test]
    fn the_carlson_degenerate_identities_survive_f32() {
        for x in [0.25_f32, 1.0, 4.0, 100.0] {
            let inv_sqrt = 1.0 / x.sqrt();
            let inv_32 = x.powf(-1.5);
            assert!(((rf(x, x, x) - inv_sqrt) / inv_sqrt).abs() < 1e-6, "R_F");
            assert!(((rc(x, x) - inv_sqrt) / inv_sqrt).abs() < 1e-6, "R_C");
            assert!(((rd(x, x, x) - inv_32) / inv_32).abs() < 1e-5, "R_D");
            assert!(((rj(x, x, x, x) - inv_32) / inv_32).abs() < 1e-5, "R_J");
        }
        // The one that runs the duplication loop.
        for y in [0.5_f32, 2.0, 10.0] {
            let want = 3.0 * core::f32::consts::PI / (4.0 * y.powf(1.5));
            let got = rd(0.0, y, y);
            assert!(
                ((got - want) / want).abs() < 1e-5,
                "R_D(0,{y},{y}) = {got:e} against {want:e}"
            );
        }
    }

    /// The closed-form values and the reductions between the Legendre forms.
    #[test]
    fn the_closed_forms_and_reductions_survive_f32() {
        let pi2 = core::f32::consts::FRAC_PI_2;
        assert!((kcomp(0.0) - pi2).abs() < 1e-6, "K(0)");
        assert!((ecomp(0.0) - pi2).abs() < 1e-6, "E(0)");
        // The lemniscate constant.
        assert!(
            (kcomp(core::f32::consts::FRAC_1_SQRT_2) - 1.854_074_7).abs() < 1e-5,
            "K(1/sqrt 2) = {}",
            kcomp(core::f32::consts::FRAC_1_SQRT_2)
        );
        for i in 0..=40 {
            let k = 0.98 * i as f32 / 40.0;
            for (name, inc, comp) in [
                ("F/K", ellint_f(pi2, k), kcomp(k)),
                ("E/E", ellint_e(pi2, k), ecomp(k)),
                ("D/D", ellint_d(pi2, k), dcomp(k)),
                ("P/P", ellint_p(pi2, k, 0.3), pcomp(k, 0.3)),
            ] {
                assert!(
                    (inc - comp).abs() < 1e-4 * (1.0 + comp.abs()),
                    "{name} at k = {k}: {inc:e} against {comp:e}"
                );
            }
            // Pi(phi, k, 0) = F(phi, k).
            assert!(
                (ellint_p(0.9, k, 0.0) - ellint_f(0.9, k)).abs() < 1e-5,
                "Pi(phi,k,0) = F at k = {k}"
            );
        }
        // The dispatcher agrees with the four it dispatches to.
        for k in [0.0_f32, 0.3, 0.7, 0.95] {
            assert_eq!(ellint_comp(0, k, 0.3), kcomp(k));
            assert_eq!(ellint_comp(1, k, 0.3), ecomp(k));
            assert_eq!(ellint_comp(2, k, 0.3), dcomp(k));
            assert_eq!(ellint_comp(3, k, 0.3), pcomp(k, 0.3));
            assert!(ellint_comp(4, k, 0.3).is_nan());
        }
    }

    /// The domain refusals, which are upstream's `DOMAIN_ERROR` throughout.
    #[test]
    fn the_domain_refusals_match_the_f64_module() {
        assert!(rf(-1.0, 1.0, 1.0).is_nan());
        assert!(rd(-1.0, 1.0, 1.0).is_nan());
        assert!(rc(-1.0, 1.0).is_nan());
        assert!(rj(-1.0, 1.0, 1.0, 1.0).is_nan());
        assert!(rf(0.0, 0.0, 1.0).is_nan(), "two zeros");
        assert!(!rf(0.0, 1.0, 1.0).is_nan(), "one zero is allowed");
        assert!(rj(0.0, 1.0, 1.0, 0.0).is_nan(), "R_J needs p > 0");
        for k in [1.0_f32, 1.5, -1.0, -2.0] {
            assert!(kcomp(k).is_nan(), "K({k})");
            assert!(ecomp(k).is_nan(), "E({k})");
            assert!(dcomp(k).is_nan(), "D({k})");
            assert!(pcomp(k, 0.3).is_nan(), "Pi({k})");
        }
        assert!(rf(f32::NAN, 1.0, 1.0).is_nan());
        assert!(kcomp(f32::NAN).is_nan());
        assert!(ellint_f(f32::NAN, 0.5).is_nan());
        assert!(ellint_f(1.0, f32::NAN).is_nan());
        assert!(ellint_p(1.0, 0.5, f32::NAN).is_nan());
    }

    /// The shader and this mirror agree on all nine shared constants.
    ///
    /// There is no generator and no table audit for `ellint` — it has no
    /// coefficient tables — so this parses `ellint.wgsl` for its `const`
    /// declarations and compares them here, exactly as `mirror_dilog` does.
    #[test]
    fn the_shader_and_this_mirror_agree_on_their_constants() {
        let src = crate::wgsl::ELLINT;
        let read = |name: &str| -> f32 {
            let at = src
                .find(name)
                .unwrap_or_else(|| panic!("{name} not declared in ellint.wgsl"));
            let rest = &src[at..];
            let eq = rest.find('=').expect("no = after the constant name");
            let end = rest[eq..].find(';').expect("no ; after the value");
            rest[eq + 1..eq + end]
                .trim()
                .trim_end_matches('u')
                .parse::<f32>()
                .unwrap_or_else(|_| panic!("{name} is not a numeric literal"))
        };
        assert_eq!(read("PETIR_ELL_ERRTOL: f32"), ERRTOL);
        assert_eq!(read("PETIR_ELL_NMAX: u32"), NMAX as f32);
        assert_eq!(read("PETIR_ELL_RF_LOLIM: f32"), RF_LOLIM);
        assert_eq!(read("PETIR_ELL_RF_UPLIM: f32"), RF_UPLIM);
        assert_eq!(read("PETIR_ELL_RD_LOLIM: f32"), RD_LOLIM);
        assert_eq!(read("PETIR_ELL_RD_UPLIM: f32"), RD_UPLIM);
        assert_eq!(read("PETIR_ELL_RJ_LOLIM: f32"), RJ_LOLIM);
        assert_eq!(read("PETIR_ELL_RJ_UPLIM: f32"), RJ_UPLIM);
        assert_eq!(read("PETIR_ELL_SQRT_EPS: f32"), SQRT_EPS);
        // And errtol is upstream's own single-precision value, not ours.
        // Compared at f32: 0.03 is not representable, and `0.03f32 as f64` is
        // 0.029999999329447746 — a difference that says nothing about
        // provenance, which is what this line is checking.
        assert_eq!(ERRTOL, Mode::Single.errtol() as f32);
        assert!(ERRTOL != Mode::Double.errtol() as f32);
    }
}
