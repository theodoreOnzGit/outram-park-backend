// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Primitives shared by every File-6 kinematics evaluator in this module:
//! the Legendre recursion, the CM→lab two-body energy relation, the common
//! lab-distribution output type, small numeric-guard constants, and the
//! 8-point Gauss–Legendre quadrature that both [`super::lab7::ll2lab`] and
//! [`super::lab1::f6lab`] use for their mu-integrals.

/// Sentinel "no further break point" energy \[eV\] — NJOY's `emax = 1.e10`.
pub(crate) const EMAX: f64 = 1.0e10;
/// Relative smallness guard — NJOY's `small = 1.e-10`.
pub(crate) const SMALL: f64 = 1.0e-10;
/// NJOY's `tiny = 1.e-4`, used to size the `f6cm` step floor `eps = tiny/elmax`.
pub(crate) const TINY: f64 = 1.0e-4;

/// Legendre polynomials `P_0(x) .. P_np(x)` by upward recursion.
///
/// Faithful port of `legndr` (`mathm.f90`): returns a vector `p` of length
/// `np + 1` with `p[l] = P_l(x)` for `l = 0 ..= np`. `x` is a direction cosine
/// in `[-1, 1]` (dimensionless); the polynomials are dimensionless.
///
/// Uses the same three-term recurrence NJOY uses,
/// `P_{i+1} = ((2i+1) x P_i - i P_{i-1}) / (i+1)`, written in NJOY's
/// difference form to match its round-off behaviour bit-for-bit.
pub fn legndr(x: f64, np: usize) -> Vec<f64> {
    let mut p = vec![0.0_f64; np + 1];
    p[0] = 1.0;
    if np >= 1 {
        p[1] = x;
    }
    if np < 2 {
        return p;
    }
    for i in 1..=(np - 1) {
        let g = x * p[i];
        let h = g - p[i - 1];
        p[i + 1] = h + g - h / ((i + 1) as f64);
    }
    p
}

/// Lab secondary energy for a two-body-like CM→lab emission \[eV\].
///
/// This is the kinematic relation the [`super::cm::cm2lab`] integrator
/// inverts: a particle emitted with center-of-mass energy `ep_cm` \[eV\] at CM
/// cosine `mu_cm` (dimensionless, `[-1, 1]`) from a reaction at incident
/// energy `e_in` \[eV\] appears in the lab at
///
/// `E_lab = ep_cm + xc * e_in + 2 * mu_cm * sqrt(xc * e_in * ep_cm)`,
///
/// where the CM-motion factor is `xc = A' / (A + 1)^2` — `A'` the emitted
/// particle's atomic-weight ratio to the neutron and `A` the target's
/// (dimensionless). At `mu_cm = +1` this gives the forward edge
/// `(sqrt(ep_cm) + sqrt(xc*e_in))^2`; at `mu_cm = -1` the backward edge
/// `(sqrt(ep_cm) - sqrt(xc*e_in))^2` (the `elmax`/`elmin` limits used in `f6cm`,
/// `groupr.f90:8316,8330-8333`).
///
/// Valid for `ep_cm >= 0`, `e_in > 0`, `xc >= 0`.
pub fn lab_energy_from_cm(e_in: f64, ep_cm: f64, mu_cm: f64, xc: f64) -> f64 {
    ep_cm + xc * e_in + 2.0 * mu_cm * (xc * e_in * ep_cm).sqrt()
}

/// The lab-frame Legendre distribution produced by [`super::cm::cm2lab`],
/// [`super::lab7::ll2lab`], or [`super::lab1::f6lab`].
///
/// `points` are `(E'_lab \[eV\], Legendre coefficients)` pairs in **ascending**
/// lab secondary energy, each coefficient vector of length `nl`
/// (`coeffs[l] = ` the lab `P_l` moment of the double-differential emission at
/// that `E'`). `p0_integral` is the integral of the lab `P_0` moment over `E'`
/// — NJOY's `cm2lab` `sum` normalization check (`groupr.f90:8233,8244`), which
/// should be `≈ 1` for a normalized CM emission. [`ll2lab`](super::lab7::ll2lab)
/// and [`f6lab`](super::lab1::f6lab) do not compute this energy-integral check
/// (their normalization is per-point, over the angular grid instead — see
/// their own docs), so `p0_integral` is left at `0.0` for their output.
#[derive(Debug, Clone)]
pub struct LabDistribution {
    /// Incident energy `E` \[eV\] this distribution is for.
    pub e_in: f64,
    /// Number of lab Legendre coefficients per point (`P_0 .. P_{nl-1}`).
    pub nl: usize,
    /// `(E'_lab \[eV\], [P_0, .., P_{nl-1}])`, ascending in `E'_lab`.
    pub points: Vec<(f64, Vec<f64>)>,
    /// Integral of the lab `P_0` moment over `E'` (the `sum ≈ 1` check).
    /// Only populated by [`cm2lab`](super::cm::cm2lab); see the struct docs.
    pub p0_integral: f64,
}

/// Zero-out lab Legendre moments that are negligible vs `P_0`
/// (`groupr.f90:8503-8510`, and the analogous `380` guard elsewhere): for
/// `l >= 1`, if `|term[l]| < tol*|term[0]|`, set 0.
pub(crate) fn remove_small_moments(term: &mut [f64], nl: usize, tol: f64) {
    if nl > 1 {
        let test = tol * term[0].abs();
        for t in term.iter_mut().take(nl).skip(1) {
            if t.abs() < test {
                *t = 0.0;
            }
        }
    }
}

/// 8-point Gauss–Legendre abscissae on `[-1, 1]`.
///
/// Shared by [`ll2lab`](super::lab7::ll2lab) (`groupr.f90:8949-8952`) and
/// [`f6lab`](super::lab1::f6lab) (`groupr.f90:9088-9091`) for their mu-space
/// quadrature (identical constants to `getdis`'s `qp` in
/// [`crate::groupr::two_body`] / [`crate::groupr::matrix`]).
pub(crate) const GAUSS8_NODES: [f64; 8] = [
    -0.960_289_856_5,
    -0.796_666_477_4,
    -0.525_532_409_9,
    -0.183_434_642_5,
    0.183_434_642_5,
    0.525_532_409_9,
    0.796_666_477_4,
    0.960_289_856_5,
];

/// 8-point Gauss–Legendre weights on `[-1, 1]`, paired with [`GAUSS8_NODES`].
pub(crate) const GAUSS8_WEIGHTS: [f64; 8] = [
    0.101_228_536_2,
    0.222_381_034_5,
    0.313_706_645_9,
    0.362_683_783_4,
    0.362_683_783_4,
    0.313_706_645_9,
    0.222_381_034_5,
    0.101_228_536_3,
];

#[cfg(test)]
mod tests {
    use super::*;

    /// `legndr` reproduces the closed-form low-order Legendre polynomials.
    ///
    /// **Methodology.** Evaluate `legndr(x, 4)` at `x = 0.3` and compare to the
    /// analytic `P_0..P_4(0.3)`. Pass if every order agrees to < 1e-12.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** All five orders match to < 1e-13.
    #[test]
    fn legndr_matches_closed_form() {
        let x = 0.3_f64;
        let p = legndr(x, 4);
        let p2 = 0.5 * (3.0 * x * x - 1.0);
        let p3 = 0.5 * (5.0 * x.powi(3) - 3.0 * x);
        let p4 = 0.125 * (35.0 * x.powi(4) - 30.0 * x * x + 3.0);
        assert!((p[0] - 1.0).abs() < 1e-13);
        assert!((p[1] - x).abs() < 1e-13);
        assert!((p[2] - p2).abs() < 1e-13);
        assert!((p[3] - p3).abs() < 1e-13);
        assert!((p[4] - p4).abs() < 1e-13);
    }

    /// The CM→lab kinematic relation reduces to the analytic forward/backward
    /// edges at `mu_cm = ±1`.
    ///
    /// **Methodology.** For `E = 2 MeV`, `E'_cm = 0.5 MeV`, `xc = 1/(51)^2`
    /// (neutron off A=50), assert `lab_energy_from_cm` at `mu = +1` equals
    /// `(sqrt(E'_cm) + sqrt(xc E))^2` and at `mu = -1` equals
    /// `(sqrt(E'_cm) - sqrt(xc E))^2` — the `elmax`/`elmin` limits `f6cm` uses.
    /// Pass if both match to < 1e-6 relative.
    ///
    /// **Result (2026-07-15).** Forward and backward edges match the closed form
    /// to < 1e-9 relative.
    #[test]
    fn cm_to_lab_edges_are_analytic() {
        let e = 2.0e6;
        let ep_cm = 0.5e6;
        let xc = 1.0 / (51.0_f64).powi(2);
        let fwd = lab_energy_from_cm(e, ep_cm, 1.0, xc);
        let bwd = lab_energy_from_cm(e, ep_cm, -1.0, xc);
        let fwd_ref = (ep_cm.sqrt() + (xc * e).sqrt()).powi(2);
        let bwd_ref = (ep_cm.sqrt() - (xc * e).sqrt()).powi(2);
        assert!(
            (fwd - fwd_ref).abs() < 1e-6 * fwd_ref,
            "fwd {fwd} vs {fwd_ref}"
        );
        assert!(
            (bwd - bwd_ref).abs() < 1e-6 * bwd_ref,
            "bwd {bwd} vs {bwd_ref}"
        );
    }
}
