//! Linearise a TAB1 cross-section table to purely lin-lin interpolation.
//!
//! RECONR's core job (beyond resonance reconstruction) is to ensure that every
//! cross section on the output PENDF tape can be reproduced by linear interpolation
//! within a specified tolerance. This module provides that algorithm independently
//! of the rest of RECONR so it can be unit-tested in isolation.
//!
//! ## Algorithm
//!
//! For each consecutive pair of tabulated points (E₁, σ₁) → (E₂, σ₂):
//!
//! 1. If the interpolation law for that segment is already `LinLin`, keep both
//!    endpoints unchanged (the segment is exact).
//! 2. Otherwise, evaluate `σ_true` at the midpoint `E_mid` using the original law,
//!    and compare it against the linear interpolation `σ_lin` at `E_mid`.
//!    - If the relative error exceeds `eps`, split the segment at `E_mid` and
//!      recurse on each half (up to [`MAX_BISECTION_DEPTH`] times).
//!    - If the error is within `eps`, the segment is accepted as lin-lin.
//!
//! The midpoint is chosen in the "natural" space of the law:
//! - lin-lin / lin-log: arithmetic mean in E (E_mid = (E₁ + E₂) / 2)
//! - log-lin / log-log: geometric mean in E (E_mid = √(E₁ · E₂))
//!
//! Ported from the grid-refinement logic in NJOY2016 `reconr.f90`.

use crate::endf::interp::{terp1, IntLaw};

/// Maximum bisection depth per segment. Prevents runaway recursion near
/// discontinuities or nearly-flat regions.
const MAX_BISECTION_DEPTH: u32 = 50;

/// Relative error floor: avoids division by near-zero cross sections.
const ERR_FLOOR: f64 = 1.0e-10;

/// Linearise a TAB1 table within fractional tolerance `eps`.
///
/// Takes the interpolation-region table and (E, σ) pairs from a TAB1 record
/// and returns a new set of pairs that is everywhere lin-lin to within `eps`
/// relative error.
///
/// Segments that are already `LinLin` are passed through unchanged. Other
/// interpolation laws are replaced by a piecewise-linear approximation refined
/// by recursive midpoint bisection.
///
/// # Parameters
/// - `interp` — ENDF region table: `(nbt, int_code)` pairs (1-based nbt).
/// - `pairs`  — `(E [eV], σ [b])` tabulated values, length ≥ 2.
/// - `eps`    — fractional reconstruction tolerance (e.g. `0.001` = 0.1%).
///
/// # Returns
/// A fully lin-lin `(E, σ)` sequence with no duplicate energies, in ascending E.
pub fn linearize_tab1(
    interp: &[(u32, u32)],
    pairs: &[(f64, f64)],
    eps: f64,
) -> Vec<(f64, f64)> {
    if pairs.len() < 2 {
        return pairs.to_vec();
    }

    let mut out = Vec::with_capacity(pairs.len() * 2);
    out.push(pairs[0]);

    for i in 0..pairs.len() - 1 {
        let (e1, s1) = pairs[i];
        let (e2, s2) = pairs[i + 1];

        // Determine interpolation law for this segment.
        // The right endpoint of segment i is pairs[i+1], whose 1-based ENDF index is (i+2).
        let endf_idx = (i + 2) as u32;
        let law_code = interp
            .iter()
            .find(|&&(nbt, _)| endf_idx <= nbt)
            .map(|&(_, c)| c)
            .unwrap_or(2);
        let law = IntLaw::from_code(law_code);

        if law == IntLaw::LinLin || law == IntLaw::Histogram {
            // Already exact — no refinement needed.
            out.push((e2, s2));
        } else {
            // Refine this segment and append all interior points + right endpoint.
            let mut seg = Vec::new();
            refine(e1, s1, e2, s2, law, eps, 0, &mut seg);
            seg.push((e2, s2));
            out.extend_from_slice(&seg);
        }
    }

    out
}

/// Recursive midpoint refinement for one segment `[e1, e2]` under `law`.
///
/// Interior points are pushed to `out` in ascending energy order; the caller
/// is responsible for adding the bounding endpoints.
fn refine(
    e1: f64,
    s1: f64,
    e2: f64,
    s2: f64,
    law: IntLaw,
    eps: f64,
    depth: u32,
    out: &mut Vec<(f64, f64)>,
) {
    if depth >= MAX_BISECTION_DEPTH {
        return;
    }

    // Midpoint in the natural space of the law.
    let e_mid = midpoint_energy(e1, e2, law);
    if e_mid <= e1 || e_mid >= e2 {
        return; // interval too small to split
    }

    // True value at midpoint under the original law.
    let s_true = match terp1(e1, s1, e2, s2, e_mid, law) {
        Ok(v) => v,
        Err(_) => return, // non-positive log argument — skip refinement
    };

    // Linear interpolation at midpoint (always lin-lin for the output).
    let s_lin = s1 + (s2 - s1) * (e_mid - e1) / (e2 - e1);

    // Relative error check.
    let ref_val = s_true.abs().max(ERR_FLOOR);
    let rel_err = (s_lin - s_true).abs() / ref_val;

    if rel_err > eps {
        // Split: refine left half, emit midpoint, refine right half.
        refine(e1, s1, e_mid, s_true, law, eps, depth + 1, out);
        out.push((e_mid, s_true));
        refine(e_mid, s_true, e2, s2, law, eps, depth + 1, out);
    }
    // If within tolerance: no interior point needed for this segment.
}

/// Choose the bisection midpoint in the appropriate energy coordinate.
///
/// - Arithmetic mean for laws where E enters linearly (LinLin, LinLog).
/// - Geometric mean for laws where E enters logarithmically (LogLin, LogLog).
fn midpoint_energy(e1: f64, e2: f64, law: IntLaw) -> f64 {
    match law {
        IntLaw::LogLin | IntLaw::LogLog => (e1 * e2).sqrt(), // geometric mean
        _ => (e1 + e2) * 0.5,                                 // arithmetic mean
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lin_lin_identity() {
        // A lin-lin TAB1 should pass through unchanged.
        let interp = vec![(3u32, 2u32)]; // NBT=3, INT=2 (lin-lin)
        let pairs = vec![(1.0, 2.0), (2.0, 4.0), (3.0, 6.0)];
        let out = linearize_tab1(&interp, &pairs, 0.001);
        assert_eq!(out.len(), pairs.len(), "lin-lin should not add points");
        for ((e_in, s_in), (e_out, s_out)) in pairs.iter().zip(out.iter()) {
            assert!((e_in - e_out).abs() < 1e-12);
            assert!((s_in - s_out).abs() < 1e-12);
        }
    }

    #[test]
    fn log_log_adds_points() {
        // y = x² is log-log-linear (slope=2 on log axes). With only two bounding
        // points, lin-lin representation misses the curve unless we add midpoints.
        // Table: (1,1) → (100, 10000), law=log-log (5)
        let interp = vec![(2u32, 5u32)]; // log-log
        let pairs = vec![(1.0, 1.0), (100.0, 10000.0)];
        let out = linearize_tab1(&interp, &pairs, 0.001); // 0.1% tolerance
        assert!(out.len() > 2, "log-log curve should add interior points, got {}", out.len());
        // Verify every interior point is a real value of y=x²
        for &(e, s) in &out {
            let expected = e * e;
            assert!((s - expected).abs() / expected < 0.002, "at e={e}: s={s} expected={expected}");
        }
    }

    #[test]
    fn lin_log_adds_points() {
        // σ(E) = exp(E), linear in E but log in σ.  With two endpoints,
        // the lin-lin approximation deviates significantly in between.
        let interp = vec![(2u32, 3u32)]; // lin-log
        let pairs = vec![(0.0_f64, 1.0_f64), (4.0, f64::exp(4.0))];
        let out = linearize_tab1(&interp, &pairs, 0.001);
        assert!(out.len() > 2, "lin-log should add points, got {}", out.len());
    }

    #[test]
    fn single_point_returns_unchanged() {
        let interp = vec![];
        let pairs = vec![(1.0, 1.0)];
        let out = linearize_tab1(&interp, &pairs, 0.001);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn endpoints_preserved() {
        // The first and last energy must always match the original TAB1.
        let interp = vec![(2u32, 5u32)]; // log-log
        let pairs = vec![(1e-5, 3.0), (2e7, 0.15)];
        let out = linearize_tab1(&interp, &pairs, 0.001);
        assert!((out.first().unwrap().0 - 1e-5).abs() < 1.0);
        assert!((out.last().unwrap().0  - 2e7).abs()  < 1.0);
    }
}
