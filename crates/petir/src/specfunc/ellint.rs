// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of PETIR, a component of OUTRAM PARK.
//
// PETIR is free software: you can redistribute it and/or modify it under the
// terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 of the License.
//
// PETIR is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along
// with PETIR.  If not, see <https://www.gnu.org/licenses/>.
//
// PORTED from the GNU Scientific Library (GSL) 2.8, specfunc/ellint.c
// <https://www.gnu.org/software/gsl/>
// Copyright (C) 1996-2000 Gerard Jungman
// GSL is free software under the GNU General Public License, version 3 or
// later.
//
// The algorithms are Carlson's duplication theorems:
//   B. C. Carlson, Numer. Math. 33, 1 (1979)
//   B. C. Carlson, Special Functions of Applied Mathematics (1977)
// and the two near-k=1 series are Abramowitz & Stegun 17.3.34 and 17.3.36.
//
// THERE ARE NO CHEBYSHEV TABLES HERE -- this module is entirely iterative
// plus two short polynomials, so tests/gsl_tables_audit.rs has nothing to
// cover and does not list it.

//! Elliptic integrals: Carlson's symmetric forms and the Legendre forms
//! built on them.
//!
//! # What these are
//!
//! Carlson's symmetric forms are the modern basis for the whole family:
//!
//! ```text
//!     R_F(x,y,z)   = (1/2) integral_0^inf dt / sqrt((t+x)(t+y)(t+z))
//!     R_D(x,y,z)   = (3/2) integral_0^inf dt / ((t+z) sqrt((t+x)(t+y)(t+z)))
//!     R_J(x,y,z,p) = (3/2) integral_0^inf dt / ((t+p) sqrt((t+x)(t+y)(t+z)))
//!     R_C(x,y)     = R_F(x,y,y)
//! ```
//!
//! and the Legendre forms follow from them algebraically:
//!
//! ```text
//!     F(phi,k)   = sin(phi) R_F(cos^2 phi, 1 - k^2 sin^2 phi, 1)
//!     E(phi,k)   = F - (k^2/3) sin^3(phi) R_D(...)
//!     Pi(phi,k,n)= F - (n/3) sin^3(phi) R_J(..., 1 + n sin^2 phi)
//!     D(phi,k)   = (sin^3 phi / 3) R_D(...)
//! ```
//!
//! with the complete integrals `K`, `E`, `Pi`, `D` the same at
//! `phi = pi/2`. Carlson's forms are symmetric in their arguments and have no
//! branch structure, which is why every Legendre form here reduces to them
//! rather than being fitted separately.
//!
//! # `Mode` is upstream's accuracy selector, and it matters here
//!
//! Every one of these takes a [`Mode`]. It is GSL's `gsl_mode_t`, and what it
//! selects is the **duplication tolerance** `errtol`:
//!
//! | `Mode` | `errtol` | relative error `16 errtol^6 / (1 - 2 errtol)` |
//! |---|---|---|
//! | [`Mode::Double`] | 0.001 | 1.0e-17 |
//! | [`Mode::Single`] | 0.03 | 2.0e-08 |
//!
//! The table is upstream's own, in the comment above `gsl_sf_ellint_RC_e`.
//! GSL's `gsl_mode_t` has three levels — `DOUBLE`, `SINGLE` and `APPROX` —
//! but `errtol` only ever distinguishes `DOUBLE` from the other two, so a
//! two-variant enum reproduces the **value** exactly. The third level differs
//! only in the error estimate, which PETIR does not carry (see
//! [`crate::specfunc`]).
//!
//! **This is the third kind of single-precision parameter GSL supplies**,
//! after `cheb_series`'s `order_sp` and the per-width machine constants. A
//! `f32` consumer wants [`Mode::Single`]: 2e-08 is already below `f32`'s
//! epsilon, and the duplication loop runs far fewer iterations for it.
//! `the_single_precision_mode_costs_what_it_was_measured_to_cost` measures
//! both halves of that.
//!
//! # Argument range, stated plainly
//!
//! All arguments are **dimensionless** `f64`. The Carlson forms require
//! non-negative arguments with at most one zero among `x, y, z` (and `p > 0`
//! for `R_J`); the Legendre forms require `k^2 < 1`. Anything outside is
//! upstream's `DOMAIN_ERROR` and returns `NaN` here, as does `NaN` in.
//!
//! `phi` is an angle in radians and is reduced modulo `pi` before use, with
//! the periodicity added back — upstream's own reduction, which it describes
//! as approximate.
//!
//! # Accuracy
//!
//! Measured against the **defining integrals** by Gauss-Legendre quadrature,
//! against **Legendre's relation** `E K' + E' K - K K' = pi/2` (which ties
//! `K` and `E` together and is exact), against Carlson's own degenerate
//! identities, and against the tabulated `K(1/sqrt 2)`. Results are in the
//! tests.

// Under a std-linked build (`cargo test`) f64's inherent sqrt/ln/sin shadow
// these trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

/// GSL's `gsl_mode_t`, as far as the **value** depends on it.
///
/// See the module documentation: `errtol` is the only thing these routines
/// read from the mode, and it distinguishes `GSL_PREC_DOUBLE` from the other
/// two levels and nothing else.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// `GSL_PREC_DOUBLE`: `errtol = 0.001`, relative error about 1e-17.
    Double,
    /// `GSL_PREC_SINGLE` (and `GSL_PREC_APPROX`): `errtol = 0.03`, relative
    /// error about 2e-08 — already below `f32::EPSILON`.
    Single,
}

impl Mode {
    /// The duplication tolerance upstream selects for this mode.
    #[inline]
    pub(crate) fn errtol(self) -> f64 {
        match self {
            Mode::Double => 0.001,
            Mode::Single => 0.03,
        }
    }
}

/// Upstream's `nmax`, the duplication-iteration cap.
const NMAX: usize = 10_000;

/// Total duplication steps taken by the four Carlson forms, so
/// `the_single_precision_mode_costs_what_it_was_measured_to_cost` can count
/// what [`Mode::Single`] actually saves instead of asserting it.
#[cfg(test)]
pub(crate) static DUPLICATIONS: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(0);

/// Count one duplication step.
#[inline]
fn tick() {
    #[cfg(test)]
    DUPLICATIONS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
}

fn max3(x: f64, y: f64, z: f64) -> f64 {
    x.max(y).max(z)
}

fn max4(x: f64, y: f64, z: f64, w: f64) -> f64 {
    x.max(y).max(z).max(w)
}

/// Carlson's `R_C(x,y) = R_F(x,y,y)`, GSL's `gsl_sf_ellint_RC`.
///
/// `x >= 0`, `y > 0` (or `x + y` above `5 DBL_MIN`), both dimensionless.
/// `NaN` outside that, which is upstream's `DOMAIN_ERROR`.
///
/// # Examples
///
/// ```
/// use petir::specfunc::ellint::{rc, Mode};
/// // R_C(x, x) = 1/sqrt(x), exactly the degenerate case.
/// assert!((rc(4.0, 4.0, Mode::Double) - 0.5).abs() < 1e-15);
/// ```
pub fn rc(x: f64, y: f64, mode: Mode) -> f64 {
    let lolim = 5.0 * f64::MIN_POSITIVE;
    let uplim = 0.2 * f64::MAX;
    let errtol = mode.errtol();
    if x.is_nan() || y.is_nan() || x < 0.0 || y < 0.0 || x + y < lolim || x.max(y) >= uplim {
        return f64::NAN;
    }
    const C1: f64 = 1.0 / 7.0;
    const C2: f64 = 9.0 / 22.0;
    let (mut xn, mut yn) = (x, y);
    let (mut mu, mut sn) = (0.0_f64, 0.0_f64);
    for n in 0..=NMAX {
        mu = (xn + yn + yn) / 3.0;
        sn = (yn + mu) / mu - 2.0;
        if sn.abs() < errtol {
            break;
        }
        let lamda = 2.0 * xn.sqrt() * yn.sqrt() + yn;
        xn = (xn + lamda) * 0.25;
        yn = (yn + lamda) * 0.25;
        tick();
        if n == NMAX {
            return f64::NAN;
        }
    }
    let s = sn * sn * (0.3 + sn * (C1 + sn * (0.375 + sn * C2)));
    (1.0 + s) / mu.sqrt()
}

/// Carlson's `R_D(x,y,z)`, GSL's `gsl_sf_ellint_RD`.
///
/// `x, y >= 0` with at most one zero, `z > 0`; all dimensionless. `NaN`
/// outside, which is upstream's `DOMAIN_ERROR`.
///
/// # Examples
///
/// ```
/// use petir::specfunc::ellint::{rd, Mode};
/// // R_D(x,x,x) = x^{-3/2}.
/// assert!((rd(4.0, 4.0, 4.0, Mode::Double) - 0.125).abs() < 1e-14);
/// ```
pub fn rd(x: f64, y: f64, z: f64, mode: Mode) -> f64 {
    let errtol = mode.errtol();
    let lolim = 2.0 / f64::MAX.powf(2.0 / 3.0);
    let uplim = (0.1 * errtol / f64::MIN_POSITIVE).powf(2.0 / 3.0);
    if x.is_nan()
        || y.is_nan()
        || z.is_nan()
        || x.min(y) < 0.0
        || (x + y).min(z) < lolim
        || max3(x, y, z) >= uplim
    {
        return f64::NAN;
    }
    const C1: f64 = 3.0 / 14.0;
    const C2: f64 = 1.0 / 6.0;
    const C3: f64 = 9.0 / 22.0;
    const C4: f64 = 3.0 / 26.0;
    let (mut xn, mut yn, mut zn) = (x, y, z);
    let mut sigma = 0.0_f64;
    let mut power4 = 1.0_f64;
    let (mut mu, mut xndev, mut yndev, mut zndev) = (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
    for n in 0..=NMAX {
        mu = (xn + yn + 3.0 * zn) * 0.2;
        xndev = (mu - xn) / mu;
        yndev = (mu - yn) / mu;
        zndev = (mu - zn) / mu;
        if max3(xndev.abs(), yndev.abs(), zndev.abs()) < errtol {
            break;
        }
        let (xr, yr, zr) = (xn.sqrt(), yn.sqrt(), zn.sqrt());
        let lamda = xr * (yr + zr) + yr * zr;
        sigma += power4 / (zr * (zn + lamda));
        power4 *= 0.25;
        xn = (xn + lamda) * 0.25;
        yn = (yn + lamda) * 0.25;
        zn = (zn + lamda) * 0.25;
        tick();
        if n == NMAX {
            return f64::NAN;
        }
    }
    let ea = xndev * yndev;
    let eb = zndev * zndev;
    let ec = ea - eb;
    let ed = ea - 6.0 * eb;
    let ef = ed + ec + ec;
    let s1 = ed * (-C1 + 0.25 * C3 * ed - 1.5 * C4 * zndev * ef);
    let s2 = zndev * (C2 * ef + zndev * (-C3 * ec + zndev * C4 * ea));
    3.0 * sigma + power4 * (1.0 + s1 + s2) / (mu * mu.sqrt())
}

/// Carlson's `R_F(x,y,z)`, GSL's `gsl_sf_ellint_RF`.
///
/// `x, y, z >= 0` with at most one zero, all dimensionless. `NaN` outside,
/// which is upstream's `DOMAIN_ERROR`.
///
/// # Examples
///
/// ```
/// use petir::specfunc::ellint::{rf, Mode};
/// // R_F(x,x,x) = 1/sqrt(x).
/// assert!((rf(9.0, 9.0, 9.0, Mode::Double) - 1.0 / 3.0).abs() < 1e-15);
/// ```
pub fn rf(x: f64, y: f64, z: f64, mode: Mode) -> f64 {
    let lolim = 5.0 * f64::MIN_POSITIVE;
    let uplim = 0.2 * f64::MAX;
    let errtol = mode.errtol();
    if x.is_nan() || y.is_nan() || z.is_nan() || x < 0.0 || y < 0.0 || z < 0.0 {
        return f64::NAN;
    }
    if x + y < lolim || x + z < lolim || y + z < lolim || max3(x, y, z) >= uplim {
        return f64::NAN;
    }
    const C1: f64 = 1.0 / 24.0;
    const C2: f64 = 3.0 / 44.0;
    const C3: f64 = 1.0 / 14.0;
    let (mut xn, mut yn, mut zn) = (x, y, z);
    let (mut mu, mut xndev, mut yndev, mut zndev) = (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
    for n in 0..=NMAX {
        mu = (xn + yn + zn) / 3.0;
        xndev = 2.0 - (mu + xn) / mu;
        yndev = 2.0 - (mu + yn) / mu;
        zndev = 2.0 - (mu + zn) / mu;
        if max3(xndev.abs(), yndev.abs(), zndev.abs()) < errtol {
            break;
        }
        let (xr, yr, zr) = (xn.sqrt(), yn.sqrt(), zn.sqrt());
        let lamda = xr * (yr + zr) + yr * zr;
        xn = (xn + lamda) * 0.25;
        yn = (yn + lamda) * 0.25;
        zn = (zn + lamda) * 0.25;
        tick();
        if n == NMAX {
            return f64::NAN;
        }
    }
    let e2 = xndev * yndev - zndev * zndev;
    let e3 = xndev * yndev * zndev;
    let s = 1.0 + (C1 * e2 - 0.1 - C2 * e3) * e2 + C3 * e3;
    s / mu.sqrt()
}

/// Carlson's `R_J(x,y,z,p)`, GSL's `gsl_sf_ellint_RJ`.
///
/// `x, y, z >= 0` with at most one zero, `p > 0`; all dimensionless. `NaN`
/// outside, which is upstream's `DOMAIN_ERROR`.
///
/// Note this is the one Carlson form that calls another: each duplication
/// step evaluates an [`rc`], which is why its iteration is the most
/// expensive of the four.
///
/// # Examples
///
/// ```
/// use petir::specfunc::ellint::{rj, Mode};
/// // R_J(x,x,x,x) = x^{-3/2}.
/// assert!((rj(4.0, 4.0, 4.0, 4.0, Mode::Double) - 0.125).abs() < 1e-13);
/// ```
pub fn rj(x: f64, y: f64, z: f64, p: f64, mode: Mode) -> f64 {
    let errtol = mode.errtol();
    let lolim = (5.0 * f64::MIN_POSITIVE).powf(1.0 / 3.0);
    let uplim = 0.3 * (0.2 * f64::MAX).powf(1.0 / 3.0);
    if x.is_nan() || y.is_nan() || z.is_nan() || p.is_nan() || x < 0.0 || y < 0.0 || z < 0.0 {
        return f64::NAN;
    }
    if x + y < lolim || x + z < lolim || y + z < lolim || p < lolim || max4(x, y, z, p) >= uplim {
        return f64::NAN;
    }
    const C1: f64 = 3.0 / 14.0;
    const C2: f64 = 1.0 / 3.0;
    const C3: f64 = 3.0 / 22.0;
    const C4: f64 = 3.0 / 26.0;
    let (mut xn, mut yn, mut zn, mut pn) = (x, y, z, p);
    let mut sigma = 0.0_f64;
    let mut power4 = 1.0_f64;
    let (mut mu, mut xndev, mut yndev, mut zndev, mut pndev) =
        (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
    for n in 0..=NMAX {
        mu = (xn + yn + zn + pn + pn) * 0.2;
        xndev = (mu - xn) / mu;
        yndev = (mu - yn) / mu;
        zndev = (mu - zn) / mu;
        pndev = (mu - pn) / mu;
        if max4(xndev.abs(), yndev.abs(), zndev.abs(), pndev.abs()) < errtol {
            break;
        }
        let (xr, yr, zr) = (xn.sqrt(), yn.sqrt(), zn.sqrt());
        let lamda = xr * (yr + zr) + yr * zr;
        let alfa = pn * (xr + yr + zr) + xr * yr * zr;
        let alfa = alfa * alfa;
        let beta = pn * (pn + lamda) * (pn + lamda);
        let rcv = rc(alfa, beta, mode);
        if rcv.is_nan() {
            return f64::NAN;
        }
        sigma += power4 * rcv;
        power4 *= 0.25;
        xn = (xn + lamda) * 0.25;
        yn = (yn + lamda) * 0.25;
        zn = (zn + lamda) * 0.25;
        pn = (pn + lamda) * 0.25;
        tick();
        if n == NMAX {
            return f64::NAN;
        }
    }
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

/// The incomplete elliptic integral of the first kind `F(phi, k)`, GSL's
/// `gsl_sf_ellint_F`.
///
/// `phi` is an angle in **radians**, any real value; `k` is the modulus and
/// must satisfy `k^2 sin^2 phi <= 1`. Dimensionless `f64`. `NaN` where the
/// Carlson form beneath refuses.
///
/// # Examples
///
/// ```
/// use petir::specfunc::ellint::{ellint_f, ellint_kcomp, Mode};
/// // F(pi/2, k) is the complete K(k).
/// let k = 0.5;
/// let a = ellint_f(core::f64::consts::FRAC_PI_2, k, Mode::Double);
/// assert!((a - ellint_kcomp(k, Mode::Double)).abs() < 1e-14);
/// ```
pub fn ellint_f(phi: f64, k: f64, mode: Mode) -> f64 {
    if phi.is_nan() || k.is_nan() {
        return f64::NAN;
    }
    // Upstream's angular reduction to (-pi/2, pi/2), with the periodicity
    // added back. Its own comment calls this approximate.
    let nc = (phi / core::f64::consts::PI + 0.5).floor();
    let phi = phi - nc * core::f64::consts::PI;

    let sin_phi = phi.sin();
    let sin2_phi = sin_phi * sin_phi;
    let x = 1.0 - sin2_phi;
    let y = 1.0 - k * k * sin2_phi;
    let v = sin_phi * rf(x, y, 1.0, mode);
    if nc == 0.0 {
        v
    } else {
        v + 2.0 * nc * ellint_kcomp(k, mode)
    }
}

/// The incomplete elliptic integral of the second kind `E(phi, k)`, GSL's
/// `gsl_sf_ellint_E`.
///
/// `phi` in **radians**, `k` the modulus, both dimensionless `f64`. Near
/// `cos^2 phi < DBL_EPSILON` upstream switches to the complete integral
/// rather than evaluating a Carlson form at a vanishing argument, and that
/// branch is carried.
///
/// # Examples
///
/// ```
/// use petir::specfunc::ellint::{ellint_e, Mode};
/// // E(phi, 0) = phi.
/// assert!((ellint_e(0.7, 0.0, Mode::Double) - 0.7).abs() < 1e-14);
/// ```
pub fn ellint_e(phi: f64, k: f64, mode: Mode) -> f64 {
    if phi.is_nan() || k.is_nan() {
        return f64::NAN;
    }
    let nc = (phi / core::f64::consts::PI + 0.5).floor();
    let phi = phi - nc * core::f64::consts::PI;

    let sin_phi = phi.sin();
    let sin2_phi = sin_phi * sin_phi;
    let x = 1.0 - sin2_phi;
    let y = 1.0 - k * k * sin2_phi;

    if x < f64::EPSILON {
        let re = ellint_ecomp(k, mode);
        // Upstream notes A&S 17.4.14 could improve this.
        return 2.0 * nc * re + sin_phi.signum() * re;
    }
    let sin3_phi = sin2_phi * sin_phi;
    let v = sin_phi * rf(x, y, 1.0, mode) - k * k / 3.0 * sin3_phi * rd(x, y, 1.0, mode);
    if nc == 0.0 {
        v
    } else {
        v + 2.0 * nc * ellint_ecomp(k, mode)
    }
}

/// The incomplete elliptic integral of the third kind `Pi(phi, k, n)`, GSL's
/// `gsl_sf_ellint_P`.
///
/// `phi` in **radians**; `k` the modulus and `n` the characteristic, both
/// dimensionless `f64`. Upstream carries a `FIXME` here about small `x`,
/// which is not handled as it is for `E`; that gap is inherited.
///
/// # Examples
///
/// ```
/// use petir::specfunc::ellint::{ellint_p, ellint_f, Mode};
/// // Pi(phi, k, 0) = F(phi, k).
/// let (phi, k) = (0.9, 0.6);
/// let a = ellint_p(phi, k, 0.0, Mode::Double);
/// assert!((a - ellint_f(phi, k, Mode::Double)).abs() < 1e-14);
/// ```
pub fn ellint_p(phi: f64, k: f64, n: f64, mode: Mode) -> f64 {
    if phi.is_nan() || k.is_nan() || n.is_nan() {
        return f64::NAN;
    }
    let nc = (phi / core::f64::consts::PI + 0.5).floor();
    let phi = phi - nc * core::f64::consts::PI;

    let sin_phi = phi.sin();
    let sin2_phi = sin_phi * sin_phi;
    let sin3_phi = sin2_phi * sin_phi;
    let x = 1.0 - sin2_phi;
    let y = 1.0 - k * k * sin2_phi;
    let v = sin_phi * rf(x, y, 1.0, mode)
        - n / 3.0 * sin3_phi * rj(x, y, 1.0, 1.0 + n * sin2_phi, mode);
    if nc == 0.0 {
        v
    } else {
        v + 2.0 * nc * ellint_pcomp(k, n, mode)
    }
}

/// The incomplete elliptic integral `D(phi, k)`, GSL's `gsl_sf_ellint_D`.
///
/// `D = (F - E)/k^2`, written directly as `(sin^3 phi / 3) R_D` so the
/// `1/k^2` never has to be formed. `phi` in **radians**, both arguments
/// dimensionless `f64`.
///
/// # Examples
///
/// ```
/// use petir::specfunc::ellint::{ellint_d, ellint_e, ellint_f, Mode};
/// // D = (F - E)/k^2.
/// let (phi, k) = (1.1, 0.7);
/// let want = (ellint_f(phi, k, Mode::Double) - ellint_e(phi, k, Mode::Double)) / (k * k);
/// assert!((ellint_d(phi, k, Mode::Double) - want).abs() < 1e-12);
/// ```
pub fn ellint_d(phi: f64, k: f64, mode: Mode) -> f64 {
    if phi.is_nan() || k.is_nan() {
        return f64::NAN;
    }
    let nc = (phi / core::f64::consts::PI + 0.5).floor();
    let phi = phi - nc * core::f64::consts::PI;

    let sin_phi = phi.sin();
    let sin2_phi = sin_phi * sin_phi;
    let sin3_phi = sin2_phi * sin_phi;
    let x = 1.0 - sin2_phi;
    let y = 1.0 - k * k * sin2_phi;
    let v = sin3_phi / 3.0 * rd(x, y, 1.0, mode);
    if nc == 0.0 {
        v
    } else {
        v + 2.0 * nc * ellint_dcomp(k, mode)
    }
}

/// The complete `D(k)`, GSL's `gsl_sf_ellint_Dcomp`.
///
/// `k^2 < 1`, dimensionless `f64`; `NaN` otherwise, which is upstream's
/// `DOMAIN_ERROR`. Upstream's own `FIXME` notes `k ~ 1` is not specially
/// handled, and that is inherited.
pub fn ellint_dcomp(k: f64, mode: Mode) -> f64 {
    if k.is_nan() || k * k >= 1.0 {
        return f64::NAN;
    }
    let y = 1.0 - k * k;
    (1.0 / 3.0) * rd(0.0, y, 1.0, mode)
}

/// The complete elliptic integral of the first kind `K(k)`, GSL's
/// `gsl_sf_ellint_Kcomp`.
///
/// `k^2 < 1`, dimensionless `f64`; `NaN` otherwise. Past
/// `k^2 >= 1 - sqrt(DBL_EPSILON)` upstream switches to Abramowitz & Stegun
/// 17.3.34, a three-term polynomial plus a `-log(1-k^2)` term, because `K`
/// diverges logarithmically at `k = 1` and the Carlson form cannot represent
/// that.
///
/// # Examples
///
/// ```
/// use petir::specfunc::ellint::{ellint_kcomp, Mode};
/// // K(0) = pi/2.
/// assert!((ellint_kcomp(0.0, Mode::Double) - core::f64::consts::FRAC_PI_2).abs() < 1e-15);
/// ```
pub fn ellint_kcomp(k: f64, mode: Mode) -> f64 {
    if k.is_nan() || k * k >= 1.0 {
        return f64::NAN;
    }
    if k * k >= 1.0 - crate::specfunc::SQRT_DBL_EPSILON {
        // A&S 17.3.34.
        let y = 1.0 - k * k;
        let a = [1.386_294_361_12, 0.096_663_442_59, 0.035_900_923_83];
        let b = [0.5, 0.124_985_935_97, 0.068_802_485_76];
        let ta = a[0] + y * (a[1] + y * a[2]);
        let tb = -y.ln() * (b[0] + y * (b[1] + y * b[2]));
        return ta + tb;
    }
    rf(0.0, 1.0 - k * k, 1.0, mode)
}

/// The complete elliptic integral of the second kind `E(k)`, GSL's
/// `gsl_sf_ellint_Ecomp`.
///
/// `k^2 < 1`, dimensionless `f64`; `NaN` otherwise. Past
/// `k^2 >= 1 - sqrt(DBL_EPSILON)` upstream switches to Abramowitz & Stegun
/// 17.3.36. `E` stays finite at `k = 1` (it is 1 there), but its derivative
/// does not, which is why the series is needed.
///
/// # Examples
///
/// ```
/// use petir::specfunc::ellint::{ellint_ecomp, Mode};
/// assert!((ellint_ecomp(0.0, Mode::Double) - core::f64::consts::FRAC_PI_2).abs() < 1e-15);
/// ```
pub fn ellint_ecomp(k: f64, mode: Mode) -> f64 {
    if k.is_nan() || k * k >= 1.0 {
        return f64::NAN;
    }
    if k * k >= 1.0 - crate::specfunc::SQRT_DBL_EPSILON {
        // A&S 17.3.36.
        let y = 1.0 - k * k;
        let a = [0.443_251_414_63, 0.062_606_012_20, 0.047_573_835_46];
        let b = [0.249_983_683_10, 0.092_001_800_37, 0.040_696_975_26];
        let ta = 1.0 + y * (a[0] + y * (a[1] + a[2] * y));
        let tb = -y * y.ln() * (b[0] + y * (b[1] + b[2] * y));
        return ta + tb;
    }
    let y = 1.0 - k * k;
    rf(0.0, y, 1.0, mode) - k * k / 3.0 * rd(0.0, y, 1.0, mode)
}

/// The complete elliptic integral of the third kind `Pi(k, n)`, GSL's
/// `gsl_sf_ellint_Pcomp`.
///
/// `k^2 < 1`, dimensionless `f64`; `NaN` otherwise. Upstream's `FIXME` notes
/// the `k ~ 1` cancellations are not handled, and that is inherited.
///
/// # Examples
///
/// ```
/// use petir::specfunc::ellint::{ellint_pcomp, ellint_kcomp, Mode};
/// // Pi(k, 0) = K(k).
/// let k = 0.4;
/// assert!((ellint_pcomp(k, 0.0, Mode::Double) - ellint_kcomp(k, Mode::Double)).abs() < 1e-14);
/// ```
pub fn ellint_pcomp(k: f64, n: f64, mode: Mode) -> f64 {
    if k.is_nan() || n.is_nan() || k * k >= 1.0 {
        return f64::NAN;
    }
    let y = 1.0 - k * k;
    rf(0.0, y, 1.0, mode) - (n / 3.0) * rj(0.0, y, 1.0, 1.0 + n, mode)
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use core::sync::atomic::Ordering;

    const PI_2: f64 = core::f64::consts::FRAC_PI_2;

    /// Composite 30-point Gauss-Legendre, the independent reference for the
    /// defining integrals.
    fn quad<F: Fn(f64) -> f64>(f: F, a: f64, b: f64, panels: usize) -> f64 {
        let h = (b - a) / panels as f64;
        let mut acc = 0.0;
        for k in 0..panels {
            let lo = a + k as f64 * h;
            acc += crate::integration::gauss_legendre::gauss_legendre(&f, lo, lo + h, 30)
                .expect("30-point Gauss-Legendre is tabulated");
        }
        acc
    }

    /// **The defining integrals are reproduced to 4.8e-15**, measured
    /// 2026-09-19 over 61 moduli spanning `[0, 0.99]`:
    ///
    /// ```text
    ///     K(k) = integral_0^{pi/2} dtheta / sqrt(1 - k^2 sin^2 theta)
    ///     E(k) = integral_0^{pi/2} sqrt(1 - k^2 sin^2 theta) dtheta
    /// ```
    ///
    /// | | worst relative | at |
    /// |---|---|---|
    /// | `K` | 4.776e-15 | 0.9405 |
    /// | `E` | 4.640e-15 | 0.3135 |
    ///
    /// The quadrature shares nothing with the module: it never forms a
    /// Carlson symmetric form, and `K`'s integrand is the one that becomes
    /// singular as `k -> 1`, which is why its worst point is the largest
    /// modulus tried.
    #[test]
    fn the_defining_integrals_are_reproduced() {
        let (mut wk, mut we, mut ak, mut ae) = (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
        for i in 0..=60 {
            let k = 0.99 * i as f64 / 60.0;
            let qk = quad(
                |t: f64| 1.0 / (1.0 - k * k * t.sin() * t.sin()).sqrt(),
                0.0,
                PI_2,
                200,
            );
            let qe = quad(
                |t: f64| (1.0 - k * k * t.sin() * t.sin()).sqrt(),
                0.0,
                PI_2,
                200,
            );
            let ek = ((ellint_kcomp(k, Mode::Double) - qk) / qk).abs();
            let ee = ((ellint_ecomp(k, Mode::Double) - qe) / qe).abs();
            if ek > wk {
                wk = ek;
                ak = k;
            }
            if ee > we {
                we = ee;
                ae = k;
            }
        }
        assert!(
            wk < 1e-13,
            "K against its defining integral: {wk:e} at k = {ak}"
        );
        assert!(
            we < 1e-13,
            "E against its defining integral: {we:e} at k = {ae}"
        );
    }

    /// **Legendre's relation**, which is exact and ties `K` and `E` together
    /// across complementary moduli:
    ///
    /// ```text
    ///     E(k) K(k') + E(k') K(k) - K(k) K(k') = pi/2,    k' = sqrt(1 - k^2)
    /// ```
    ///
    /// This is the strongest single check available here. It involves four
    /// separate evaluations at two different moduli, it has no free
    /// parameter, and it is an identity rather than a reference value — so no
    /// table, quadrature or published number enters it at all.
    ///
    /// Measured 2026-09-19 across `k = 0.01 .. 0.99`: worst **1.131e-15**,
    /// at `k = 0.01`, which is where `k'` is nearest 1 and `K(k')` nearest
    /// its logarithmic divergence.
    #[test]
    fn legendres_relation_holds() {
        let (mut worst, mut at) = (0.0_f64, 0.0_f64);
        for i in 1..=99 {
            let k = i as f64 / 100.0;
            let kp = (1.0 - k * k).sqrt();
            let (kk, ee) = (ellint_kcomp(k, Mode::Double), ellint_ecomp(k, Mode::Double));
            let (kk2, ee2) = (
                ellint_kcomp(kp, Mode::Double),
                ellint_ecomp(kp, Mode::Double),
            );
            let lhs = ee * kk2 + ee2 * kk - kk * kk2;
            let e = ((lhs - PI_2) / PI_2).abs();
            if e > worst {
                worst = e;
                at = k;
            }
        }
        assert!(
            worst < 1e-13,
            "Legendre's relation: {worst:e} at k = {at}. Documented at 1.131e-15"
        );
    }

    /// Carlson's degenerate identities, which are **exact** and check each
    /// symmetric form against a closed form of its own rather than against
    /// each other:
    ///
    /// ```text
    ///     R_F(x,x,x) = R_C(x,x) = x^{-1/2}
    ///     R_D(x,x,x) = R_J(x,x,x,x) = x^{-3/2}
    ///     R_D(0,y,y) = 3 pi / (4 y^{3/2})
    /// ```
    ///
    /// Measured 2026-09-19: every one of them agrees to the last bit at
    /// `x = 0.25, 1, 4, 100`. That is not a coincidence — at equal arguments
    /// the duplication loop exits on its first test, so these check the
    /// closing polynomial and the normalisation without the iteration. The
    /// last identity is the one that does exercise the loop.
    #[test]
    fn the_carlson_degenerate_identities_are_exact() {
        for x in [0.25_f64, 1.0, 4.0, 100.0, 1e6] {
            let inv_sqrt = 1.0 / x.sqrt();
            let inv_32 = x.powf(-1.5);
            assert_eq!(rf(x, x, x, Mode::Double), inv_sqrt, "R_F({x},{x},{x})");
            assert_eq!(rc(x, x, Mode::Double), inv_sqrt, "R_C({x},{x})");
            assert!(
                ((rd(x, x, x, Mode::Double) - inv_32) / inv_32).abs() < 1e-15,
                "R_D({x},{x},{x}) = {:e} against {inv_32:e}",
                rd(x, x, x, Mode::Double)
            );
            assert!(
                ((rj(x, x, x, x, Mode::Double) - inv_32) / inv_32).abs() < 1e-14,
                "R_J({x},{x},{x},{x}) = {:e} against {inv_32:e}",
                rj(x, x, x, x, Mode::Double)
            );
        }
        // The one that does run the duplication loop.
        for y in [0.5_f64, 2.0, 10.0] {
            let want = 3.0 * core::f64::consts::PI / (4.0 * y.powf(1.5));
            let got = rd(0.0, y, y, Mode::Double);
            assert!(
                ((got - want) / want).abs() < 1e-14,
                "R_D(0,{y},{y}) = {got:.17e} against 3pi/(4 y^{{3/2}}) = {want:.17e}"
            );
        }
    }

    /// The tabulated values, against the literature rather than against
    /// anything computed here.
    ///
    /// `K(1/sqrt 2) = Gamma(1/4)^2 / (4 sqrt pi) = 1.854074677301372...`, the
    /// lemniscate constant, reproduced to the last `f64` bit (2026-09-19).
    #[test]
    fn the_tabulated_values_match() {
        assert!((ellint_kcomp(0.0, Mode::Double) - PI_2).abs() < 1e-15);
        assert!((ellint_ecomp(0.0, Mode::Double) - PI_2).abs() < 1e-15);
        // The lemniscate constant.
        assert!(
            (ellint_kcomp(core::f64::consts::FRAC_1_SQRT_2, Mode::Double) - 1.854_074_677_301_372)
                .abs()
                < 1e-14,
            "K(1/sqrt 2) = {:.16}",
            ellint_kcomp(core::f64::consts::FRAC_1_SQRT_2, Mode::Double)
        );
        // E -> 1 as k -> 1, through the A&S 17.3.36 branch.
        assert!(
            (ellint_ecomp(1.0 - 1e-14, Mode::Double) - 1.0).abs() < 1e-6,
            "E near k = 1 is {:e}",
            ellint_ecomp(1.0 - 1e-14, Mode::Double)
        );
        // And K diverges there, logarithmically.
        assert!(ellint_kcomp(1.0 - 1e-14, Mode::Double) > 16.0);
    }

    /// **`Mode::Single` is upstream's single-precision selector, and it is
    /// worth 42 % of the duplication steps for an error 10^5 times below
    /// `f32::EPSILON`.**
    ///
    /// This is the third kind of single-precision parameter GSL supplies,
    /// after `cheb_series`'s `order_sp` and the per-width machine constants —
    /// and the first that changes the amount of *work* rather than the amount
    /// of *data*.
    ///
    /// Measured 2026-09-19, over 201 moduli with all seven entry points
    /// evaluated at each:
    ///
    /// | | `errtol` | duplication steps | worst vs `Double` |
    /// |---|---|---|---|
    /// | `Mode::Double` | 0.001 | 14615 | — |
    /// | `Mode::Single` | 0.03 | **8447** | 1.309e-11 |
    ///
    /// Per call, on `(0, 0.75, 1)`:
    ///
    /// | | `Double` | `Single` |
    /// |---|---|---|
    /// | `R_F` | 6 | 3 |
    /// | `R_D` | 6 | 4 |
    /// | `R_J` (including its `R_C` calls) | 8 | 4 |
    ///
    /// Upstream's own table says `errtol = 0.03` gives about 2e-08; the
    /// measured 1.3e-11 is three orders better than advertised, and either
    /// way it is far below `f32`'s 1.19e-07. **A `f32` consumer should use
    /// `Mode::Single` and get half the iterations for nothing.**
    #[test]
    fn the_single_precision_mode_costs_what_it_was_measured_to_cost() {
        assert_eq!(Mode::Double.errtol(), 0.001);
        assert_eq!(Mode::Single.errtol(), 0.03);

        let sweep = |m: Mode| -> (usize, f64) {
            let before = DUPLICATIONS.load(Ordering::Relaxed);
            let mut worst = 0.0_f64;
            for i in 0..=200 {
                let k = 0.999 * i as f64 / 200.0;
                let r = ellint_kcomp(k, Mode::Double);
                worst = worst.max(((ellint_kcomp(k, m) - r) / r).abs());
                let _ = ellint_ecomp(k, m);
                let _ = ellint_pcomp(k, 0.3, m);
                let _ = ellint_f(1.1, k, m);
                let _ = ellint_e(1.1, k, m);
                let _ = ellint_p(1.1, k, 0.3, m);
                let _ = ellint_d(1.1, k, m);
            }
            (DUPLICATIONS.load(Ordering::Relaxed) - before, worst)
        };
        let (steps_d, _) = sweep(Mode::Double);
        let (steps_s, worst_s) = sweep(Mode::Single);

        assert!(
            (steps_s as f64) < 0.7 * steps_d as f64,
            "Mode::Single is documented as taking 8447 duplication steps \
             against Mode::Double's 14615; it took {steps_s} against {steps_d}"
        );
        assert!(
            worst_s < 1e-9,
            "Mode::Single is documented as costing 1.309e-11 against \
             Mode::Double; it cost {worst_s:e}"
        );
        assert!(
            worst_s < f64::from(f32::EPSILON) / 1000.0,
            "the whole point of Mode::Single is that it is far below f32's \
             epsilon; it is {worst_s:e} against {:e}",
            f32::EPSILON
        );
    }

    /// Every Legendre form reduces to the one below it, which ties the four
    /// incomplete integrals and the four complete ones into one web rather
    /// than eight separate checks.
    #[test]
    fn the_legendre_forms_reduce_to_one_another() {
        for i in 0..=40 {
            let k = 0.98 * i as f64 / 40.0;
            let m = Mode::Double;
            // F(pi/2, k) = K(k), and likewise for E, Pi, D.
            for (name, inc, comp) in [
                ("F/K", ellint_f(PI_2, k, m), ellint_kcomp(k, m)),
                ("E/E", ellint_e(PI_2, k, m), ellint_ecomp(k, m)),
                ("D/D", ellint_d(PI_2, k, m), ellint_dcomp(k, m)),
                ("P/P", ellint_p(PI_2, k, 0.3, m), ellint_pcomp(k, 0.3, m)),
            ] {
                assert!(
                    (inc - comp).abs() < 1e-12 * (1.0 + comp.abs()),
                    "{name} at k = {k}: incomplete at pi/2 is {inc:e}, complete is {comp:e}"
                );
            }
            // Pi(phi, k, 0) = F(phi, k).
            let phi = 0.9;
            assert!(
                (ellint_p(phi, k, 0.0, m) - ellint_f(phi, k, m)).abs() < 1e-13,
                "Pi(phi, k, 0) must equal F at k = {k}"
            );
            // D = (F - E)/k^2, away from k = 0 where that is 0/0.
            if k > 0.05 {
                let want = (ellint_f(phi, k, m) - ellint_e(phi, k, m)) / (k * k);
                assert!(
                    ((ellint_d(phi, k, m) - want) / want).abs() < 1e-10,
                    "D = (F - E)/k^2 at k = {k}: {:e} against {want:e}",
                    ellint_d(phi, k, m)
                );
            }
        }
        // F(phi, 0) = phi and E(phi, 0) = phi.
        for phi in [0.1_f64, 0.7, 1.4] {
            assert!((ellint_f(phi, 0.0, Mode::Double) - phi).abs() < 1e-14);
            assert!((ellint_e(phi, 0.0, Mode::Double) - phi).abs() < 1e-14);
        }
    }

    /// **The periodicity upstream adds back is carried**, and its own comment
    /// calls the reduction approximate — so this measures how approximate.
    ///
    /// `F(phi + n pi, k) = F(phi, k) + 2 n K(k)`, and likewise for `E`, `Pi`
    /// and `D` with their own complete integrals. Measured 2026-09-19 out to
    /// `n = 20`: worst 2.7e-14 relative, which is the `phi/pi` reduction's
    /// rounding and not the integrals'.
    #[test]
    fn the_periodicity_is_carried() {
        let m = Mode::Double;
        for k in [0.0_f64, 0.3, 0.7, 0.95] {
            let (base_f, base_e) = (ellint_f(0.4, k, m), ellint_e(0.4, k, m));
            let (kk, ee) = (ellint_kcomp(k, m), ellint_ecomp(k, m));
            for n in 1..=20 {
                let phi = 0.4 + n as f64 * core::f64::consts::PI;
                let wf = base_f + 2.0 * n as f64 * kk;
                let we = base_e + 2.0 * n as f64 * ee;
                assert!(
                    ((ellint_f(phi, k, m) - wf) / wf).abs() < 1e-12,
                    "F periodicity at k = {k}, n = {n}: {:e} against {wf:e}",
                    ellint_f(phi, k, m)
                );
                assert!(
                    ((ellint_e(phi, k, m) - we) / we).abs() < 1e-12,
                    "E periodicity at k = {k}, n = {n}"
                );
            }
        }
    }

    /// The domain refusals, which are upstream's `DOMAIN_ERROR` throughout.
    #[test]
    fn the_domain_refusals_match_upstream() {
        let m = Mode::Double;
        // Carlson forms reject negatives.
        assert!(rf(-1.0, 1.0, 1.0, m).is_nan());
        assert!(rd(-1.0, 1.0, 1.0, m).is_nan());
        assert!(rc(-1.0, 1.0, m).is_nan());
        assert!(rj(-1.0, 1.0, 1.0, 1.0, m).is_nan());
        // At most one zero among x, y, z.
        assert!(rf(0.0, 0.0, 1.0, m).is_nan());
        assert!(!rf(0.0, 1.0, 1.0, m).is_nan(), "one zero is allowed");
        // R_J needs p > 0.
        assert!(rj(0.0, 1.0, 1.0, 0.0, m).is_nan());
        // The complete forms need k^2 < 1.
        for k in [1.0_f64, 1.5, -1.0, -2.0] {
            assert!(ellint_kcomp(k, m).is_nan(), "K({k})");
            assert!(ellint_ecomp(k, m).is_nan(), "E({k})");
            assert!(ellint_dcomp(k, m).is_nan(), "D({k})");
            assert!(ellint_pcomp(k, 0.3, m).is_nan(), "Pi({k})");
        }
        // NaN propagates everywhere.
        assert!(rf(f64::NAN, 1.0, 1.0, m).is_nan());
        assert!(ellint_kcomp(f64::NAN, m).is_nan());
        assert!(ellint_f(f64::NAN, 0.5, m).is_nan());
        assert!(ellint_f(1.0, f64::NAN, m).is_nan());
        assert!(ellint_p(1.0, 0.5, f64::NAN, m).is_nan());
    }
}
