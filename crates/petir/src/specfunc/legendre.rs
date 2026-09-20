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
// PORTED from the GNU Scientific Library (GSL) 2.8, specfunc/legendre_poly.c
// <https://www.gnu.org/software/gsl/>
// Copyright (C) 1996-2000 Gerard Jungman
// GSL is free software under the GNU General Public License, version 3 or
// later.
//
// NO CHEBYSHEV TABLES. A seed and an upward recurrence in the degree.
//
// gsl_sf_legendre_sphPlm_e IS NOT PORTED, and the reason is a dependency
// rather than a decision: it needs gsl_sf_lnpoch_e (specfunc/poch.c) and
// gsl_sf_log_1plusx_e (specfunc/log.c), neither of which PETIR has. Reading
// the source is what established that -- from the outside sphPlm looks like
// a normalisation of Plm, and it is not; it carries its own recurrence in
// log space precisely so that it does NOT overflow where Plm does. See the
// module docs.

//! The associated Legendre polynomials `P_l^m(x)`.
//!
//! # What these are
//!
//! The solutions of the general Legendre equation, and the `theta`-dependent
//! half of the spherical harmonics. `m = 0` recovers the ordinary Legendre
//! polynomials `P_l(x)`, which is checked here against two other routes.
//!
//! # How they are computed
//!
//! From the closed-form seed
//!
//! ```text
//!     P_m^m(x) = (-1)^m (2m-1)!! (1 - x^2)^{m/2}
//! ```
//!
//! built as a product rather than from a factorial, and then the upward
//! recurrence in the degree:
//!
//! ```text
//!     (l - m) P_l^m = (2l - 1) x P_{l-1}^m - (l + m - 1) P_{l-2}^m
//! ```
//!
//! Upstream's own, both of them. `l = m` and `l = m + 1` are the seed and
//! one step of it.
//!
//! # The overflow guard is real, and it is not a formality
//!
//! `P_l^m` carries a `(2m-1)!!` factor, so it grows **factorially in `m`**:
//! at `m = 150` it has left `f64`'s range entirely regardless of `x` or `l`.
//! Upstream computes an approximate log-magnitude before doing any work and
//! refuses when it is below `LOG_DBL_MIN + 10`. That check is carried — and
//! **it has a hole at `l == m`, which is upstream's and is inherited
//! deliberately.**
//!
//! `legendre_poly.c:304` gates the `t_s` term on `dif == 0.0` rather than on
//! `sum == 0.0`. At `l == m` that zeroes the one term that could make the
//! estimate negative, leaving `0.5 log(2l+1)`, which is positive for every
//! `l` — so the guard cannot fire in exactly the case where `P_l^m` is
//! largest. Measured: `P_150^150(0.5)` is `1.600e+297` and correct;
//! `P_200^200(0.5)` is **`inf`, with no error reported**. At `m = 100`,
//! where `dif` is non-zero, the guard does work and first refuses at
//! `l = 1124`.
//!
//! The port stays faithful: this crate's maturity rests on agreeing with
//! GSL, and silently repairing the condition would break that agreement on
//! precisely the inputs where it matters. `the_overflow_guard_has_upstreams_hole_at_l_equals_m`
//! pins the behaviour so it is visible rather than surprising. **If you need
//! high `m`, the normalised form below is the answer**, not a patched
//! guard.
//!
//! # `sphPlm` is absent, and the reason is a dependency
//!
//! GSL's `gsl_sf_legendre_sphPlm_e` computes
//! `sqrt((2l+1)/(4 pi) (l-m)!/(l+m)!) P_l^m(x)`, the spherical-harmonic
//! normalisation, and it is the one to reach for at high degree because the
//! normalisation cancels exactly the factorial growth that overflows this
//! one. It is **not** a scale factor applied afterwards: it carries its own
//! recurrence, seeded in log space, so that no intermediate ever overflows.
//!
//! It needs `gsl_sf_lnpoch_e` (`specfunc/poch.c`) and
//! `gsl_sf_log_1plusx_e` (`specfunc/log.c`), neither of which PETIR ports.
//! That is a bead, not a gap to paper over with a post-hoc normalisation —
//! multiplying this function's output by the factor would inherit exactly
//! the overflow the separate implementation exists to avoid.
//!
//! # Argument range
//!
//! `l >= m >= 0` and `x` in `[-1, 1]`, dimensionless `f64`. `NaN` outside,
//! which is upstream's `DOMAIN_ERROR`, and `NaN` where upstream signals
//! `OVERFLOW_ERROR`.
//!
//! **[`legendre_plm_checked`] returns a [`crate::Result`] instead**, and is
//! the one to reach for if you cannot rule out large `m`: it reports
//! [`PetirError::Overflow`](crate::PetirError::Overflow) for a non-finite
//! result whether or not upstream's guard saw it coming, which closes the
//! `l == m` hole for a caller without making the ported function unfaithful.
//!
//! # Accuracy
//!
//! Measured against the **`m = 0` reduction** to the ordinary Legendre
//! polynomials by two independent routes, against the **closed forms** for
//! `l - m <= 1`, and against the **orthogonality relation** integrated by
//! Gauss-Legendre quadrature. Results are in the tests.

// Under a std-linked build (`cargo test`) f64's inherent sqrt/ln shadow
// these trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

/// `P_m^m(x) = (-1)^m (2m-1)!! (1 - x^2)^{m/2}`, GSL's `legendre_Pmm`
/// (`specfunc/legendre_poly.c:41`).
///
/// Built as a running product of `-(2i - 1) sqrt(1-x) sqrt(1+x)` rather than
/// from a double factorial and a power — upstream's own form, and the reason
/// it stays accurate near `|x| = 1`: `sqrt(1-x) sqrt(1+x)` loses nothing
/// where `sqrt(1 - x^2)` would cancel.
fn legendre_pmm(m: u32, x: f64) -> f64 {
    if m == 0 {
        return 1.0;
    }
    let mut p_mm = 1.0_f64;
    let root_factor = (1.0 - x).sqrt() * (1.0 + x).sqrt();
    let mut fact_coeff = 1.0_f64;
    for _ in 1..=m {
        p_mm *= -fact_coeff * root_factor;
        fact_coeff += 2.0;
    }
    p_mm
}

/// `P_l^m(x)`, GSL's `gsl_sf_legendre_Plm_e`
/// (`specfunc/legendre_poly.c:295`).
///
/// `l >= m` and `|x| <= 1`, dimensionless. `NaN` outside that and `NaN`
/// where the result would overflow — see the module documentation, which
/// also says what to use instead at high `m`.
///
/// # Examples
///
/// ```
/// use petir::specfunc::legendre::legendre_plm;
/// // m = 0 is the ordinary Legendre polynomial.
/// assert!((legendre_plm(3, 0, 0.25) - petir::poly::dense::legendre(3).eval(0.25)).abs() < 1e-14);
/// // P_1^1(x) = -sqrt(1 - x^2).
/// assert!((legendre_plm(1, 1, 0.6) + 0.8).abs() < 1e-15);
/// ```
pub fn legendre_plm(l: u32, m: u32, x: f64) -> f64 {
    if x.is_nan() || l < m || !(-1.0..=1.0).contains(&x) {
        return f64::NAN;
    }
    // Upstream's approximate log-magnitude, computed before any work so that
    // a factorially large answer is refused rather than formed.
    let dif = (l - m) as f64;
    let sum = (l + m) as f64;
    let t_d = if dif == 0.0 {
        0.0
    } else {
        0.5 * dif * (dif.ln() - 1.0)
    };
    // NOTE `dif`, not `sum`. That is upstream's condition, verbatim
    // (legendre_poly.c:304), and it is what puts a hole in this guard at
    // l == m -- see the module documentation. Transcribed faithfully on
    // purpose: this crate's bar is agreement with GSL.
    let t_s = if dif == 0.0 {
        0.0
    } else {
        0.5 * sum * (sum.ln() - 1.0)
    };
    let exp_check = 0.5 * (2.0 * l as f64 + 1.0).ln() + t_d - t_s;
    if exp_check < crate::specfunc::LOG_DBL_MIN + 10.0 {
        return f64::NAN;
    }

    let p_mm = legendre_pmm(m, x);
    let p_mmp1 = x * (2 * m + 1) as f64 * p_mm;
    if l == m {
        return p_mm;
    }
    if l == m + 1 {
        return p_mmp1;
    }
    let mut p_ellm2 = p_mm;
    let mut p_ellm1 = p_mmp1;
    let mut p_ell = 0.0;
    for ell in (m + 2)..=l {
        let ellf = ell as f64;
        p_ell = (x * (2.0 * ellf - 1.0) * p_ellm1 - (ellf + m as f64 - 1.0) * p_ellm2)
            / (ellf - m as f64);
        p_ellm2 = p_ellm1;
        p_ellm1 = p_ell;
    }
    p_ell
}

/// [`legendre_plm`] with the overflow **actually checked**, returning
/// [`crate::Result`] rather than a bare `f64`.
///
/// # Why this exists beside the bare form
///
/// [`legendre_plm`] is a faithful transcription, and upstream's guard has a
/// hole at `l == m` — see the module documentation. The consequence is that
/// `legendre_plm(200, 200, 0.5)` returns `inf` and says nothing about it, so
/// a caller who does not know about the hole gets an infinity propagating
/// silently through whatever comes next.
///
/// This entry point closes that without touching the faithful one. It runs
/// the same computation and then **checks the result it actually got**:
///
/// | outcome | returned |
/// |---|---|
/// | finite | `Ok(value)` |
/// | `inf` | `Err(`[`PetirError::Overflow`](crate::PetirError::Overflow)`)` |
/// | `NaN` from the domain test | `Err(`[`PetirError::Domain`](crate::PetirError::Domain)`)` |
/// | `NaN` from upstream's guard | `Err(`[`PetirError::Overflow`](crate::PetirError::Overflow)`)` |
///
/// Checking the output rather than re-deriving a better magnitude estimate
/// is deliberate: it cannot disagree with the bare function about *what was
/// computed*, only about how that is reported. A second, cleverer guard
/// could drift from the one that ships.
///
/// **This is not a substitute for the normalised form.** Where `P_l^m` is
/// genuinely too large for `f64`, no error convention makes the value
/// available — see the module documentation on `sphPlm`. What this buys is
/// that the caller finds out.
///
/// # Errors
///
/// - [`PetirError::Domain`](crate::PetirError::Domain) for `l < m`,
///   `|x| > 1`, or a `NaN` argument.
/// - [`PetirError::Overflow`](crate::PetirError::Overflow) where the result
///   is not finite, whether upstream's guard caught it or not.
///
/// # Examples
///
/// ```
/// use petir::specfunc::legendre::{legendre_plm, legendre_plm_checked};
/// use petir::PetirError;
///
/// // An ordinary case agrees with the bare form exactly.
/// assert_eq!(legendre_plm_checked(8, 3, 0.4).unwrap(), legendre_plm(8, 3, 0.4));
///
/// // The case upstream's guard cannot see: the bare form returns infinity,
/// // this one says why.
/// assert!(legendre_plm(200, 200, 0.5).is_infinite());
/// assert_eq!(legendre_plm_checked(200, 200, 0.5), Err(PetirError::Overflow));
///
/// // And a domain error stays a domain error.
/// assert_eq!(legendre_plm_checked(2, 3, 0.5), Err(PetirError::Domain));
/// ```
pub fn legendre_plm_checked(l: u32, m: u32, x: f64) -> crate::Result<f64> {
    // The domain test first, so a refusal here is reported as a domain error
    // rather than being folded into the NaN the guard also produces.
    if x.is_nan() || l < m || !(-1.0..=1.0).contains(&x) {
        return Err(crate::PetirError::Domain);
    }
    let v = legendre_plm(l, m, x);
    if v.is_finite() {
        Ok(v)
    } else {
        // Past the domain test, the only remaining ways out are upstream's
        // magnitude guard (NaN) and the factorial growth escaping f64 (inf).
        // Both are the same condition reported two ways, so both are
        // Overflow.
        Err(crate::PetirError::Overflow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **`m = 0` gives the ordinary Legendre polynomials**, checked by two
    /// independent routes at once.
    ///
    /// [`crate::specfunc::gegenbauer::gegenpoly_n`] at `lambda = 1/2` is
    /// another value-recurrence, and [`crate::poly::dense::legendre`] is an
    /// explicit coefficient vector. Agreeing with both is a stronger
    /// statement than agreeing with either.
    ///
    /// The dense route is used only to `l = 20`, where it is still sound —
    /// it loses about a digit every three orders past that, which
    /// `gegenbauer`'s own tests measure.
    #[test]
    fn m_zero_gives_the_ordinary_legendre_polynomials() {
        let (mut worst, mut at) = (0.0_f64, (0u32, 0.0_f64));
        for l in 0..=30u32 {
            for i in 0..=40 {
                let x = -1.0 + 2.0 * i as f64 / 40.0;
                let a = legendre_plm(l, 0, x);
                let b = crate::specfunc::gegenbauer::gegenpoly_n(l, 0.5, x);
                let e = (a - b).abs();
                if e > worst {
                    worst = e;
                    at = (l, x);
                }
                if l <= 20 && x.abs() < 0.95 {
                    let c = crate::poly::dense::legendre(l as usize).eval(x);
                    assert!(
                        (a - c).abs() < 1e-10,
                        "P_{l}^0({x}) against the coefficient form: {a:e} vs {c:e}"
                    );
                }
            }
        }
        assert!(
            worst < 1e-12,
            "P_l^0 against C_l^{{1/2}}: {worst:e} at l = {}, x = {}",
            at.0,
            at.1
        );
        // And the endpoints exactly, as both recurrences manage.
        for l in 0..=40u32 {
            assert_eq!(legendre_plm(l, 0, 1.0), 1.0, "P_{l}(1)");
            assert_eq!(
                legendre_plm(l, 0, -1.0),
                if l % 2 == 0 { 1.0 } else { -1.0 },
                "P_{l}(-1)"
            );
        }
    }

    /// The closed forms for the seed and its first step, which are what the
    /// whole recurrence stands on.
    ///
    /// ```text
    ///     P_1^1 = -sqrt(1 - x^2)
    ///     P_2^1 = -3 x sqrt(1 - x^2)
    ///     P_2^2 = 3 (1 - x^2)
    ///     P_3^3 = -15 (1 - x^2)^{3/2}
    /// ```
    #[test]
    fn the_closed_forms_for_the_seed_are_right() {
        for i in 0..=40 {
            let x = -1.0 + 2.0 * i as f64 / 40.0;
            let s = (1.0 - x * x).max(0.0).sqrt();
            for (name, got, want) in [
                ("P_1^1", legendre_plm(1, 1, x), -s),
                ("P_2^1", legendre_plm(2, 1, x), -3.0 * x * s),
                ("P_2^2", legendre_plm(2, 2, x), 3.0 * (1.0 - x * x)),
                ("P_3^3", legendre_plm(3, 3, x), -15.0 * s * s * s),
            ] {
                assert!(
                    (got - want).abs() < 1e-13,
                    "{name}({x}) = {got:e} against {want:e}"
                );
            }
        }
    }

    /// **Orthogonality**, integrated rather than asserted:
    ///
    /// ```text
    ///     integral_{-1}^{1} P_l^m P_k^m dx = 0            for l != k
    ///                                      = 2 (l+m)! / ((2l+1) (l-m)!)   for l = k
    /// ```
    ///
    /// This is the property that defines the family, it involves no
    /// reference implementation at all, and it exercises the recurrence at
    /// every degree from `m` up. The quadrature is the same 30-point
    /// Gauss-Legendre composite the other modules use.
    #[test]
    fn the_orthogonality_relation_holds() {
        let quad = |f: &dyn Fn(f64) -> f64| -> f64 {
            let panels = 200;
            let h = 2.0 / panels as f64;
            let mut acc = 0.0;
            for k in 0..panels {
                let lo = -1.0 + k as f64 * h;
                acc += crate::integration::gauss_legendre::gauss_legendre(f, lo, lo + h, 30)
                    .expect("30-point Gauss-Legendre is tabulated");
            }
            acc
        };
        for m in 0..=3u32 {
            for l in m..=(m + 4) {
                for k in m..=(m + 4) {
                    let v = quad(&|x: f64| legendre_plm(l, m, x) * legendre_plm(k, m, x));
                    if l == k {
                        // 2 (l+m)! / ((2l+1) (l-m)!), built as a ratio so no
                        // factorial is formed.
                        let mut ratio = 1.0_f64;
                        for j in (l - m + 1)..=(l + m) {
                            ratio *= j as f64;
                        }
                        let want = 2.0 * ratio / (2.0 * l as f64 + 1.0);
                        assert!(
                            ((v - want) / want).abs() < 1e-10,
                            "norm of P_{l}^{m}: {v:e} against {want:e}"
                        );
                    } else {
                        assert!(
                            v.abs() < 1e-10,
                            "P_{l}^{m} and P_{k}^{m} are not orthogonal: {v:e}"
                        );
                    }
                }
            }
        }
    }

    /// **The overflow guard has a hole at `l == m`, and it is upstream's.**
    ///
    /// This test was written expecting to locate the smallest `m` at which
    /// `P_m^m` is refused. There is none: the guard cannot fire there at
    /// all, and `P_200^200(0.5)` returns `inf` rather than an error.
    ///
    /// Upstream, `specfunc/legendre_poly.c:303-304`, verbatim:
    ///
    /// ```text
    ///   const double t_d = ( dif == 0.0 ? 0.0 : 0.5*dif*(log(dif)-1.0) );
    ///   const double t_s = ( dif == 0.0 ? 0.0 : 0.5*sum*(log(sum)-1.0) );
    /// ```
    ///
    /// **`t_s` is gated on `dif`, not on `sum`.** When `l == m`, `dif` is
    /// zero and `t_s` is forced to zero with it — although `sum = 2m` is
    /// large and `t_s` is the term that makes `exp_check` negative. What
    /// remains is `exp_check = 0.5 log(2l+1)`, which is positive for every
    /// `l`, so the guard never refuses. Written as `sum == 0.0` it would
    /// behave; as written, the one case where `P_l^m` is largest is the one
    /// case the check cannot see.
    ///
    /// Measured 2026-09-20:
    ///
    /// | | |
    /// |---|---|
    /// | `P_150^150(0.5)` | `1.600e+297` — finite, and correct |
    /// | `P_200^200(0.5)` | **`inf`, with no error reported** |
    /// | first refused `l` at `m = 100` | 1124 |
    /// | first refused `l` at `m = 50` | never |
    ///
    /// **The port is faithful and stays faithful.** This crate's maturity
    /// rests on agreeing with GSL, and quietly correcting the condition
    /// would break that agreement on exactly the inputs where it matters.
    /// The behaviour is pinned here instead, so it is visible rather than
    /// surprising, and a caller who needs high `m` is pointed at the
    /// normalised form in the module documentation — which does not
    /// overflow at all and is the right answer to this problem.
    #[test]
    fn the_overflow_guard_has_upstreams_hole_at_l_equals_m() {
        // The hole: no order of P_m^m is ever refused, and the answer
        // eventually becomes infinite without complaint.
        for m in [0u32, 50, 150, 200, 400] {
            assert!(
                !legendre_plm(m, m, 0.5).is_nan(),
                "the guard is documented as UNABLE to fire at l == m; it \
                 refused m = {m}, which would mean upstream's condition has \
                 changed"
            );
        }
        assert!(
            legendre_plm(150, 150, 0.5).is_finite(),
            "P_150^150 is documented as finite at 1.600e+297"
        );
        assert!(
            legendre_plm(200, 200, 0.5).is_infinite(),
            "P_200^200 is documented as overflowing to infinity with no \
             error reported -- the inherited upstream hole. If this is now \
             finite or NaN, upstream's guard has been fixed and the module \
             documentation needs rewriting"
        );

        // And the guard DOES work where `dif` is non-zero, which is what
        // shows the expression is otherwise transcribed correctly.
        let mut first = None;
        for l in 100..3000u32 {
            if legendre_plm(l, 100, 0.5).is_nan() {
                first = Some(l);
                break;
            }
        }
        assert_eq!(
            first,
            Some(1124),
            "at m = 100 the guard is documented as first refusing l = 1124"
        );
        // It does not fire on ordinary arguments.
        for (l, m) in [(40u32, 5u32), (100, 2), (10, 10), (60, 50)] {
            assert!(
                legendre_plm(l, m, 0.3).is_finite(),
                "the guard fired on an ordinary case, l = {l}, m = {m}"
            );
        }
    }

    /// **The checked form agrees with the bare one everywhere, and reports
    /// what the bare one cannot.**
    ///
    /// Two claims, both swept rather than spot-checked:
    ///
    /// 1. Wherever `legendre_plm` returns a finite value,
    ///    `legendre_plm_checked` returns `Ok` of **exactly** that value —
    ///    bit equality, not a tolerance, because it is the same computation.
    /// 2. Wherever the bare form returns `inf` or a guard `NaN`, the checked
    ///    form returns `Err(Overflow)`; wherever the arguments are out of
    ///    domain, `Err(Domain)`.
    ///
    /// The second is what closes upstream's `l == m` hole for a caller. The
    /// first is what stops the checked form from silently becoming a
    /// *different* function — which is the real risk in adding a second
    /// entry point, and the reason this asserts equality rather than
    /// agreement.
    #[test]
    fn the_checked_form_agrees_exactly_and_reports_the_overflow() {
        use crate::PetirError;

        let (mut finite, mut overflowed, mut domain) = (0usize, 0usize, 0usize);
        // m to 200 so the l == m overflow near m = 160 is inside the sweep,
        // and an l far above m so upstream's guard (which needs dif != 0)
        // fires too. A narrower sweep reached NEITHER, and the assertions
        // below are what caught that.
        for m in (0..=200u32).step_by(4) {
            // m.saturating_sub(1) gives l < m for every m > 0, so the
            // domain branch is exercised by the sweep and not only by the
            // spot checks below.
            for l in [
                m.saturating_sub(1),
                m,
                m + 1,
                m + 3,
                m + 17,
                m + 60,
                m + 1200,
            ] {
                for i in 0..=8 {
                    let x = -1.0 + 2.0 * i as f64 / 8.0;
                    let bare = legendre_plm(l, m, x);
                    match legendre_plm_checked(l, m, x) {
                        Ok(v) => {
                            assert_eq!(
                                v.to_bits(),
                                bare.to_bits(),
                                "checked and bare disagree at l = {l}, m = {m}, \
                                 x = {x}: {v:e} against {bare:e}"
                            );
                            assert!(v.is_finite());
                            finite += 1;
                        }
                        Err(PetirError::Overflow) => {
                            assert!(
                                !bare.is_finite(),
                                "checked reported Overflow at l = {l}, m = {m} \
                                 where the bare form returned a finite {bare:e}"
                            );
                            overflowed += 1;
                        }
                        Err(PetirError::Domain) => {
                            assert!(l < m, "Domain reported for l = {l} >= m = {m}");
                            domain += 1;
                        }
                        Err(e) => panic!("unexpected error {e} at l = {l}, m = {m}"),
                    }
                }
            }
        }
        // All three outcomes must actually occur, or the sweep is not
        // exercising what it claims to.
        assert!(finite > 1000, "only {finite} finite results");
        assert!(overflowed > 0, "the sweep never reached an overflow");
        assert!(domain > 0, "the sweep never reached a domain refusal");

        // The specific case upstream's guard cannot see. The bare form is
        // infinite and silent; the checked form says Overflow.
        assert!(legendre_plm(200, 200, 0.5).is_infinite());
        assert_eq!(
            legendre_plm_checked(200, 200, 0.5),
            Err(PetirError::Overflow)
        );
        // And the case the guard DOES catch, which must report the same
        // thing -- a caller should not have to know which mechanism fired.
        assert!(legendre_plm(1124, 100, 0.5).is_nan());
        assert_eq!(
            legendre_plm_checked(1124, 100, 0.5),
            Err(PetirError::Overflow)
        );
        // A domain refusal is NOT folded into Overflow.
        assert_eq!(legendre_plm_checked(2, 3, 0.5), Err(PetirError::Domain));
        assert_eq!(legendre_plm_checked(3, 1, 1.5), Err(PetirError::Domain));
        assert_eq!(
            legendre_plm_checked(3, 1, f64::NAN),
            Err(PetirError::Domain)
        );
    }

    /// The domain refusals, which are upstream's.
    #[test]
    fn the_refusals_match_upstream() {
        // l < m.
        assert!(legendre_plm(2, 3, 0.5).is_nan(), "l < m");
        // |x| > 1.
        for x in [1.0001_f64, -1.0001, 2.0, f64::INFINITY] {
            assert!(legendre_plm(3, 1, x).is_nan(), "x = {x}");
        }
        assert!(legendre_plm(3, 1, f64::NAN).is_nan());
        // The endpoints themselves are allowed, and P_l^m(±1) = 0 for m > 0.
        for l in 1..=6u32 {
            for m in 1..=l {
                assert_eq!(legendre_plm(l, m, 1.0), 0.0, "P_{l}^{m}(1)");
                assert_eq!(legendre_plm(l, m, -1.0).abs(), 0.0, "P_{l}^{m}(-1)");
            }
        }
        // P_0^0 = 1 everywhere.
        for i in 0..=10 {
            let x = -1.0 + 0.2 * i as f64;
            assert_eq!(legendre_plm(0, 0, x), 1.0);
        }
    }
}
