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
// PORTED from the GNU Scientific Library (GSL) 2.8, specfunc/elljac.c
// <https://www.gnu.org/software/gsl/>
// Copyright (C) 1996-2000 Gerard Jungman; the current algorithm is Brian
// Gough's, 2005.
// GSL is free software under the GNU General Public License, version 3 or
// later.
//
// The algorithm is Algorithm 5 of
//   R. Bulirsch, "Numerical Calculation of Elliptic Integrals and Elliptic
//   Functions", Numerische Mathematik 7, 78-90 (1965)
// with upstream's own reflection tweak (Abramowitz & Stegun table 16.8,
// column "K-u") to avoid dividing by a vanishing sine.
//
// NO CHEBYSHEV TABLES. This is an arithmetic-geometric mean descent and a
// backward recurrence.
//
// THE ARRAY INDEXING IS DELIBERATELY NOT TRANSCRIBED. Upstream walks
// `mu[n]`, `nu[n]`, `c[n]`, `d[n]` by a runtime index; this crate's
// `no_panic_gate` allows a subscript only where it is a literal index into a
// `const` array, because a run-time index is a panic path on a
// microcontroller. The descent is written with `get`/`get_mut` and the
// recurrence with reversed zipped slices, and `c`/`d` are carried as scalars
// because only their final values are read. The arithmetic and its
// association are unchanged -- `the_recurrence_matches_a_literal_transcription`
// checks that against a direct, indexed version.

//! The Jacobi elliptic functions `sn(u|m)`, `cn(u|m)` and `dn(u|m)`.
//!
//! # What these are
//!
//! The inverses of the incomplete elliptic integral of the first kind: if
//! `u = F(phi, k)` with `m = k^2`, then
//!
//! ```text
//!     sn(u|m) = sin(phi),   cn(u|m) = cos(phi),   dn(u|m) = sqrt(1 - m sin^2 phi)
//! ```
//!
//! They are the doubly-periodic generalisation of sine and cosine, and they
//! degenerate to exactly that at the ends of their parameter range:
//! `m = 0` gives `(sin u, cos u, 1)` and `m = 1` gives
//! `(tanh u, sech u, sech u)`. Both limits are upstream's own special cases
//! and are carried.
//!
//! This is the companion of [`crate::specfunc::ellint`]: that module
//! evaluates the integrals, this one inverts them.
//!
//! # How they are computed
//!
//! By **arithmetic-geometric mean descent** (Bulirsch 1965, Algorithm 5),
//! not by a series. Starting from `mu_0 = 1`, `nu_0 = sqrt(1 - m)`, the AGM
//! converges *quadratically* — the number of correct digits doubles each
//! step — so the descent is short and its length depends on `m` only through
//! how far `sqrt(1 - m)` starts from 1. A backward recurrence then unwinds
//! the descent to give the three functions at once.
//!
//! Upstream caps the descent at `N = 16` and reports `GSL_EMAXITER` if it is
//! reached. **Measured over the whole admissible domain it takes at most
//! seven steps**, so that branch is unreachable in `f64` — see
//! `the_descent_is_shorter_than_upstreams_cap`. Quadratic convergence is why:
//! the step count grows like `log log` of the precision demanded, not like
//! `log`.
//!
//! # Argument range
//!
//! `u` is a dimensionless `f64` of any magnitude; `m` is the parameter
//! (**not** the modulus `k`; `m = k^2`) and must satisfy `|m| <= 1`.
//! Negative `m` is allowed and is upstream's behaviour.
//!
//! Outside `|m| <= 1` upstream sets all three outputs to zero *and* returns
//! `GSL_EDOM`. A bare-tuple return cannot carry that distinction, so this
//! returns `NaN` for all three — which is the convention of the rest of
//! [`crate::specfunc`], and is safer than upstream's zeros, since a zero
//! propagates silently and a `NaN` does not.
//!
//! # Accuracy
//!
//! Measured 2026-09-20:
//!
//! | check | worst | at |
//! |---|---|---|
//! | `sn^2 + cn^2 = 1` | 8.882e-16 | `u = -0.2, m = -0.6435` |
//! | `m sn^2 + dn^2 = 1` | 8.882e-16 | same |
//! | `sn(F(phi,k) \| k^2) = sin phi` | 1.221e-15 | `phi = 0.65, k = 0.95` |
//!
//! Four to five `f64` ulps. The identities involve no reference value, no
//! table and no quadrature; the inverse relation is a genuine cross-check
//! against [`crate::specfunc::ellint`], whose Carlson forms share no
//! arithmetic with the AGM descent here.

// Under a std-linked build (`cargo test`) f64's inherent sqrt/sin/tanh
// shadow these trait methods, leaving the import formally unused. See
// crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use crate::zip::zip_flat;

/// Upstream's `N`, the cap on the AGM descent.
const NMAX: usize = 16;

/// The three Jacobi elliptic functions at one point, in upstream's order.
///
/// Returned together because the algorithm produces all three from one
/// descent; computing them separately would triple the work for no gain.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Jacobi {
    /// `sn(u|m)`, which reduces to `sin(u)` at `m = 0`.
    pub sn: f64,
    /// `cn(u|m)`, which reduces to `cos(u)` at `m = 0`.
    pub cn: f64,
    /// `dn(u|m)`, which is identically 1 at `m = 0`.
    pub dn: f64,
}

impl Jacobi {
    /// All three `NaN` — what this module returns where upstream signals
    /// `GSL_EDOM`.
    const NAN: Jacobi = Jacobi {
        sn: f64::NAN,
        cn: f64::NAN,
        dn: f64::NAN,
    };
}

/// `sn`, `cn` and `dn` at `u` with parameter `m`, GSL's `gsl_sf_elljac_e`
/// (`specfunc/elljac.c:40`).
///
/// `u` dimensionless and unrestricted; `m = k^2` with `|m| <= 1`. All three
/// components are `NaN` outside that.
///
/// # Examples
///
/// ```
/// use petir::specfunc::elljac::elljac;
/// // m = 0 degenerates to circular sine and cosine.
/// let j = elljac(0.7, 0.0);
/// assert!((j.sn - 0.7_f64.sin()).abs() < 1e-15);
/// assert!((j.cn - 0.7_f64.cos()).abs() < 1e-15);
/// assert_eq!(j.dn, 1.0);
/// ```
pub fn elljac(u: f64, m: f64) -> Jacobi {
    if u.is_nan() || m.is_nan() || m.abs() > 1.0 {
        return Jacobi::NAN;
    }
    // Upstream's two degenerate limits, at 2 DBL_EPSILON.
    if m.abs() < 2.0 * crate::specfunc::DBL_EPSILON {
        return Jacobi {
            sn: u.sin(),
            cn: u.cos(),
            dn: 1.0,
        };
    }
    if (m - 1.0).abs() < 2.0 * crate::specfunc::DBL_EPSILON {
        let cn = 1.0 / u.cosh();
        return Jacobi {
            sn: u.tanh(),
            cn,
            dn: cn,
        };
    }

    let mut mu = [0.0_f64; NMAX];
    let mut nu = [0.0_f64; NMAX];
    let Some(m0) = mu.first_mut() else {
        return Jacobi::NAN;
    };
    *m0 = 1.0;
    let Some(n0) = nu.first_mut() else {
        return Jacobi::NAN;
    };
    *n0 = (1.0 - m).sqrt();

    // The AGM descent. Upstream indexes `mu[n]`/`nu[n]` directly; `get` is
    // used here for the reason in the file header, and the loop is otherwise
    // identical including its convergence test and its cap.
    let mut n = 0usize;
    loop {
        let (Some(&mn), Some(&nn)) = (mu.get(n), nu.get(n)) else {
            return Jacobi::NAN;
        };
        if (mn - nn).abs() <= 4.0 * crate::specfunc::DBL_EPSILON * (mn + nn).abs() {
            break;
        }
        let (next_mu, next_nu) = (0.5 * (mn + nn), (mn * nn).sqrt());
        let Some(slot) = mu.get_mut(n + 1) else {
            return Jacobi::NAN;
        };
        *slot = next_mu;
        let Some(slot) = nu.get_mut(n + 1) else {
            return Jacobi::NAN;
        };
        *slot = next_nu;
        n += 1;
        note_steps(n);
        if n >= NMAX - 1 {
            // Upstream's GSL_EMAXITER. It still returns the values computed
            // so far, and so does this.
            break;
        }
    }

    let Some(&mu_n) = mu.get(n) else {
        return Jacobi::NAN;
    };
    let sin_umu = (u * mu_n).sin();
    let cos_umu = (u * mu_n).cos();

    // Upstream switches to sn(K-u), cn(K-u), dn(K-u) when |sin| < |cos|, so
    // that the tangent it forms never divides by a vanishing sine.
    let reflected = sin_umu.abs() < cos_umu.abs();
    let t = if reflected {
        sin_umu / cos_umu
    } else {
        cos_umu / sin_umu
    };

    // The backward recurrence. `c` and `d` are carried as scalars because
    // only their final values are read; upstream stores the whole arrays.
    let (mut c, mut d) = (mu_n * t, 1.0_f64);
    let (Some(mu_lo), Some(mu_hi), Some(nu_lo)) = (mu.get(..n), mu.get(1..=n), nu.get(..n)) else {
        return Jacobi::NAN;
    };
    for (&mu_k, &mu_k1, &nu_k) in zip_flat!(mu_lo, mu_hi, nu_lo).rev() {
        let (c_next, d_next) = (c, d);
        c = d_next * c_next;
        let r = c_next * c_next / mu_k1;
        d = (r + nu_k) / (r + mu_k);
    }

    let root = (1.0 - m).sqrt();
    if reflected {
        let dn = root / d;
        let cn = dn * cos_umu.signum() / hypot(1.0, c);
        Jacobi {
            sn: cn * c / root,
            cn,
            dn,
        }
    } else {
        let sn = sin_umu.signum() / hypot(1.0, c);
        Jacobi {
            sn,
            cn: c * sn,
            dn: d,
        }
    }
}

/// `sqrt(x^2 + y^2)` without intermediate overflow — upstream's
/// `gsl_hypot`, which this module needs at exactly two call sites.
fn hypot(x: f64, y: f64) -> f64 {
    let (ax, ay) = (x.abs(), y.abs());
    if ax == 0.0 {
        return ay;
    }
    if ay == 0.0 {
        return ax;
    }
    let (big, small) = if ax > ay { (ax, ay) } else { (ay, ax) };
    let r = small / big;
    big * (1.0 + r * r).sqrt()
}

/// The high-water mark of AGM descent steps, so
/// `the_descent_is_shorter_than_upstreams_cap` can measure it.
#[cfg(test)]
pub(crate) static WORST_STEPS: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(0);

#[inline]
fn note_steps(_n: usize) {
    #[cfg(test)]
    WORST_STEPS.fetch_max(_n, core::sync::atomic::Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::Ordering;

    /// **The two defining identities**, which are exact and involve no
    /// reference value, no table and no quadrature:
    ///
    /// ```text
    ///     sn^2 + cn^2 = 1
    ///     m sn^2 + dn^2 = 1
    /// ```
    ///
    /// Measured 2026-09-20 over a 60 x 40 grid in `u` and `m`: **8.882e-16**
    /// for both, i.e. four `f64` ulps, worst at `u = -0.2, m = -0.6435`.
    /// These are the strongest available check here, because they tie all
    /// three outputs of one descent together and involve no reference value,
    /// no table and no quadrature.
    #[test]
    fn the_defining_identities_hold() {
        let (mut w1, mut w2, mut at) = (0.0_f64, 0.0_f64, (0.0_f64, 0.0_f64));
        for i in 0..=60 {
            let u = -6.0 + 0.2 * i as f64;
            for j in 0..=40 {
                let m = -0.99 + 1.98 * j as f64 / 40.0;
                let r = elljac(u, m);
                let e1 = (r.sn * r.sn + r.cn * r.cn - 1.0).abs();
                let e2 = (m * r.sn * r.sn + r.dn * r.dn - 1.0).abs();
                if e1 > w1 {
                    w1 = e1;
                    at = (u, m);
                }
                w2 = w2.max(e2);
            }
        }
        assert!(
            w1 < 1e-14 && w2 < 1e-14,
            "sn^2+cn^2-1 = {w1:e} at {at:?}, m sn^2+dn^2-1 = {w2:e}"
        );
    }

    /// **`sn` really inverts `F`**, which checks this module against a
    /// different one rather than against itself.
    ///
    /// For `u = F(phi, k)` with `m = k^2`, `sn(u|m) = sin(phi)`. Nothing in
    /// the AGM descent resembles the Carlson forms
    /// [`crate::specfunc::ellint`] uses, so agreement is a real
    /// cross-check of both.
    ///
    /// Measured 2026-09-20 over 30 angles and 20 moduli: worst
    /// **1.221e-15**, at `phi = 0.65, k = 0.95`, which is the largest
    /// modulus tried — where `F` itself is hardest.
    #[test]
    fn sn_inverts_the_incomplete_integral_of_the_first_kind() {
        use crate::specfunc::ellint::{ellint_f, Mode};
        let (mut worst, mut at) = (0.0_f64, (0.0_f64, 0.0_f64));
        for i in 1..=30 {
            let phi = 1.5 * i as f64 / 30.0;
            for j in 1..=20 {
                let k = 0.95 * j as f64 / 20.0;
                let u = ellint_f(phi, k, Mode::Double);
                let r = elljac(u, k * k);
                let e = (r.sn - phi.sin()).abs();
                if e > worst {
                    worst = e;
                    at = (phi, k);
                }
                // and cn = cos(phi), dn = sqrt(1 - m sin^2 phi).
                assert!(
                    (r.cn - phi.cos()).abs() < 1e-13,
                    "cn at {at:?}: {} against {}",
                    r.cn,
                    phi.cos()
                );
                let want_dn = (1.0 - k * k * phi.sin() * phi.sin()).sqrt();
                assert!((r.dn - want_dn).abs() < 1e-13, "dn at {at:?}");
            }
        }
        assert!(worst < 1e-13, "sn vs sin(phi): {worst:e} at {at:?}");
    }

    /// The two degenerate limits, which are upstream's own special cases.
    #[test]
    fn the_degenerate_limits_are_circular_and_hyperbolic() {
        for i in 0..=40 {
            let u = -4.0 + 0.2 * i as f64;
            let j0 = elljac(u, 0.0);
            assert!((j0.sn - u.sin()).abs() < 1e-15, "sn(u|0) = sin u");
            assert!((j0.cn - u.cos()).abs() < 1e-15, "cn(u|0) = cos u");
            assert_eq!(j0.dn, 1.0, "dn(u|0) = 1");

            let j1 = elljac(u, 1.0);
            assert!((j1.sn - u.tanh()).abs() < 1e-15, "sn(u|1) = tanh u");
            assert!((j1.cn - 1.0 / u.cosh()).abs() < 1e-15, "cn(u|1) = sech u");
            assert_eq!(j1.cn, j1.dn, "dn(u|1) = cn(u|1)");
        }
        // And the limits are approached, not just hit exactly at 0 and 1 --
        // so the special cases agree with the general branch beside them.
        for u in [0.3_f64, 1.1, 2.7] {
            let near0 = elljac(u, 1e-9);
            assert!(
                (near0.sn - u.sin()).abs() < 1e-8,
                "sn just off m = 0: {} against {}",
                near0.sn,
                u.sin()
            );
            let near1 = elljac(u, 1.0 - 1e-9);
            assert!(
                (near1.sn - u.tanh()).abs() < 1e-8,
                "sn just off m = 1: {} against {}",
                near1.sn,
                u.tanh()
            );
        }
    }

    /// **The AGM descent is far shorter than upstream's cap**, measured
    /// rather than asserted.
    ///
    /// The AGM converges quadratically, so the step count grows only like
    /// `log log` of the precision demanded. Measured 2026-09-20 over a
    /// 200 x 200 grid covering `|m| <= 1` right up to both endpoints, plus
    /// `m` within `1e-14` of either: the high-water mark is **7 steps**
    /// against `N = 16`. Upstream's `GSL_EMAXITER` branch is therefore
    /// unreachable in `f64` for any admissible argument — it is a
    /// can't-happen bound, exactly as `ellint`'s `nmax = 10000` was.
    ///
    /// That matters for the WGSL port, where the cap is a uniform loop bound
    /// the compiler unrolls against — the same reasoning as
    /// [`crate::specfunc::ellint`]'s `nmax`.
    #[test]
    fn the_descent_is_shorter_than_upstreams_cap() {
        WORST_STEPS.store(0, Ordering::Relaxed);
        for i in 0..=200 {
            let u = -10.0 + 0.1 * i as f64;
            for j in 0..=200 {
                // Right up to both endpoints, where the descent is longest.
                let m = -1.0 + 2.0 * j as f64 / 200.0;
                let _ = elljac(u, m);
            }
        }
        // And the hardest cases: m as close to 1 as the general branch is
        // ever asked for.
        for e in [1e-3_f64, 1e-6, 1e-9, 1e-12, 1e-14] {
            let _ = elljac(1.3, 1.0 - e);
            let _ = elljac(1.3, -1.0 + e);
        }
        let observed = WORST_STEPS.load(Ordering::Relaxed);
        assert!(
            observed < NMAX - 1,
            "the descent is documented as never reaching upstream's cap; it \
             took {observed} steps against N = {NMAX}"
        );
    }

    /// **The rewritten recurrence computes what upstream's indexed one
    /// does**, bit for bit.
    ///
    /// The file header explains why the array indexing is not transcribed:
    /// `no_panic_gate` forbids a run-time subscript. That is a real
    /// divergence from the source, so it gets a real check rather than a
    /// claim — this runs a direct, indexed transcription of upstream's
    /// backward recurrence beside the shipped one and requires exact
    /// equality.
    ///
    /// A test is allowed the subscripts the library is not.
    #[test]
    fn the_recurrence_matches_a_literal_transcription() {
        // Upstream's own form, indexed exactly as `elljac.c` writes it.
        fn upstream(u: f64, m: f64) -> Jacobi {
            let n_max = NMAX;
            let mut mu = [0.0_f64; NMAX];
            let mut nu = [0.0_f64; NMAX];
            let mut c = [0.0_f64; NMAX];
            let mut d = [0.0_f64; NMAX];
            mu[0] = 1.0;
            nu[0] = (1.0 - m).sqrt();
            let mut n = 0usize;
            while (mu[n] - nu[n]).abs() > 4.0 * crate::specfunc::DBL_EPSILON * (mu[n] + nu[n]).abs()
            {
                mu[n + 1] = 0.5 * (mu[n] + nu[n]);
                nu[n + 1] = (mu[n] * nu[n]).sqrt();
                n += 1;
                if n >= n_max - 1 {
                    break;
                }
            }
            let sin_umu = (u * mu[n]).sin();
            let cos_umu = (u * mu[n]).cos();
            let reflected = sin_umu.abs() < cos_umu.abs();
            let t = if reflected {
                sin_umu / cos_umu
            } else {
                cos_umu / sin_umu
            };
            c[n] = mu[n] * t;
            d[n] = 1.0;
            while n > 0 {
                n -= 1;
                c[n] = d[n + 1] * c[n + 1];
                let r = (c[n + 1] * c[n + 1]) / mu[n + 1];
                d[n] = (r + nu[n]) / (r + mu[n]);
            }
            let root = (1.0 - m).sqrt();
            if reflected {
                let dn = root / d[n];
                let cn = dn * cos_umu.signum() / hypot(1.0, c[n]);
                Jacobi {
                    sn: cn * c[n] / root,
                    cn,
                    dn,
                }
            } else {
                let sn = sin_umu.signum() / hypot(1.0, c[n]);
                Jacobi {
                    sn,
                    cn: c[n] * sn,
                    dn: d[n],
                }
            }
        }

        let mut checked = 0usize;
        for i in 0..=80 {
            let u = -8.0 + 0.2 * i as f64;
            for j in 1..80 {
                let m = -0.99 + 1.98 * j as f64 / 80.0;
                // Skip the two degenerate windows, which the shipped
                // function short-circuits before reaching the recurrence.
                if m.abs() < 1e-12 || (m - 1.0).abs() < 1e-12 {
                    continue;
                }
                let a = elljac(u, m);
                let b = upstream(u, m);
                assert_eq!(
                    (a.sn.to_bits(), a.cn.to_bits(), a.dn.to_bits()),
                    (b.sn.to_bits(), b.cn.to_bits(), b.dn.to_bits()),
                    "the rewritten recurrence differs from the indexed \
                     transcription at u = {u}, m = {m}: {a:?} against {b:?}"
                );
                checked += 1;
            }
        }
        assert!(checked > 6000, "only {checked} points compared");
    }

    /// Both branches of upstream's reflection are reached, so neither is
    /// dead code that nothing tests.
    ///
    /// The switch is on `|sin(u mu_n)| < |cos(u mu_n)|`, which depends on
    /// `u` in a way no caller controls directly — so this asserts the sweep
    /// actually visits both rather than assuming it.
    #[test]
    fn both_reflection_branches_are_exercised() {
        let (mut small_sin, mut large_sin) = (0usize, 0usize);
        for i in 0..=400 {
            let u = -8.0 + 0.04 * i as f64;
            let m = 0.5_f64;
            // Reproduce the branch decision, which the public API hides.
            let mut mu = 1.0_f64;
            let mut nu = (1.0 - m).sqrt();
            let mut n = 0;
            while (mu - nu).abs() > 4.0 * crate::specfunc::DBL_EPSILON * (mu + nu).abs()
                && n < NMAX - 1
            {
                let (a, b) = (0.5 * (mu + nu), (mu * nu).sqrt());
                mu = a;
                nu = b;
                n += 1;
            }
            if (u * mu).sin().abs() < (u * mu).cos().abs() {
                small_sin += 1;
            } else {
                large_sin += 1;
            }
            // And the answer is sane either way.
            let r = elljac(u, m);
            assert!((r.sn * r.sn + r.cn * r.cn - 1.0).abs() < 1e-14);
        }
        assert!(
            small_sin > 50 && large_sin > 50,
            "the reflection branches are documented as both reachable; the \
             sweep hit them {small_sin} and {large_sin} times"
        );
    }

    /// The refusals, and the one place this module deliberately differs from
    /// upstream.
    #[test]
    fn the_refusals_are_nan_where_upstream_returns_zeros() {
        for m in [1.0001_f64, 2.0, -1.5, f64::INFINITY] {
            let r = elljac(0.5, m);
            assert!(
                r.sn.is_nan() && r.cn.is_nan() && r.dn.is_nan(),
                "|m| > 1 must be NaN, not upstream's zeros; at m = {m} it is {r:?}"
            );
        }
        assert!(elljac(f64::NAN, 0.5).sn.is_nan());
        assert!(elljac(0.5, f64::NAN).sn.is_nan());
        // u = 0 is sn = 0, cn = 1, dn = 1 for every m.
        for m in [-0.9_f64, -0.3, 0.3, 0.9] {
            let r = elljac(0.0, m);
            assert!(r.sn.abs() < 1e-15, "sn(0|{m}) = {}", r.sn);
            assert!((r.cn - 1.0).abs() < 1e-15, "cn(0|{m}) = {}", r.cn);
            assert!((r.dn - 1.0).abs() < 1e-15, "dn(0|{m}) = {}", r.dn);
        }
    }
}
