// ---------------------------------------------------------------------------
// Ported from scikit-learn 1.9.1 (BSD-3-Clause), the routine SMT's KPLS calls.
//
//   Upstream repo:    https://github.com/scikit-learn/scikit-learn (tag 1.9.1)
//   Upstream file:    sklearn/cross_decomposition/_pls.py
//                     (_PLS.fit with PLSRegression's settings: deflation
//                      "regression", mode "A", algorithm "nipals", scale True,
//                      max_iter 500, tol 1e-6; _center_scale_xy;
//                      _get_first_singular_vectors_power_method; _svd_flip_1d)
//   Called from:      SMT smt/surrogate_models/kpls.py (KPLS._compute_pls),
//                     commit 151a3ff3c13733d168b0ca8844ff0a4ff072252a
//   Accessed:         2026-11-04
//
//   Copyright (c) 2007-2026 The scikit-learn developers. All rights reserved.
//   BSD 3-Clause License; the text (identical apart from the year range) is
//   this crate's LICENSE-BSD-SKLEARN, and NOTICE records this version.
//
// This Rust translation is part of RAFFLES / Outram Park and is distributed
// under AGPL-3.0-only. BSD-3-Clause -> (A)GPLv3 is ONE-WAY: this file may NOT
// be contributed back to scikit-learn or redistributed under BSD terms.
//
// Translation notes:
//   * Only what SMT reads: `x_rotations_` for ONE target column. With one
//     target the power method stops after its first iteration (sklearn's
//     `y.shape[1] == 1` break), which is all that is ported; no y rotations,
//     coefficients, predict or transform.
//   * `pinv(P^T W)` is the plain inverse (`linalg::inverse`): equal to the
//     pseudo-inverse for the full-rank n_comp x n_comp matrix NIPALS gives;
//     a singular one is refused rather than pseudo-inverted.
//   * "y residual is constant" (sklearn warns and stops adding components)
//     leaves the remaining columns zero, as sklearn's preallocated arrays do.
// ---------------------------------------------------------------------------

//! `PLSRegression(n_comp).fit(X, y).x_rotations_` for a single output.

use super::linalg::inverse;
use crate::{RafflesError, Result};

/// sklearn's `x_rotations_` (`nx × n_comp`) of `PLSRegression(n_comp)`
/// fitted to `x` (`n × nx`) and one output `y`.
///
/// # Errors
/// `n_comp` above `min(n, nx)` (sklearn's bound), fewer than two rows, or a
/// singular `Pᵀ W`.
pub fn x_rotations(x: &[Vec<f64>], y: &[f64], n_comp: usize) -> Result<Vec<Vec<f64>>> {
    let n = x.len();
    let p = x.first().map_or(0, Vec::len);
    if n < 2 || n_comp == 0 || n_comp > n.min(p) || y.len() != n {
        return Err(RafflesError::InvalidParameter {
            parameter: "n_comp".to_string(),
            value: n_comp as f64,
            reason: format!("PLS needs 1 <= n_comp <= min(n, nx) = {} and n >= 2", n.min(p)),
        });
    }
    let eps = f64::EPSILON;
    // _center_scale_xy (sequential column sums, (n, 1) y as a column of a 2-D
    // array: sklearn reshapes y to (n, 1), so its mean is a pairwise sum).
    let nf = n as f64;
    let mut xk = x.to_vec();
    for j in 0..p {
        let mut s = 0.0;
        for r in &xk {
            s += r[j];
        }
        let mean = s / nf;
        for r in &mut xk {
            r[j] -= mean;
        }
        let mut s2 = 0.0;
        let mut m2 = 0.0;
        for r in &xk {
            m2 += r[j];
        }
        m2 /= nf;
        for r in &xk {
            let c = r[j] - m2;
            s2 += c * c;
        }
        let mut sd = (s2 / (nf - 1.0)).sqrt();
        if sd == 0.0 {
            sd = 1.0;
        }
        for r in &mut xk {
            r[j] /= sd;
        }
    }
    use crate::surrogate::polynomial_chaos::special::pairwise_sum;
    let ym = pairwise_sum(y) / nf;
    let mut yk: Vec<f64> = y.iter().map(|v| v - ym).collect();
    let m2 = pairwise_sum(&yk) / nf;
    let sq: Vec<f64> = yk.iter().map(|v| (v - m2) * (v - m2)).collect();
    let mut ysd = (pairwise_sum(&sq) / (nf - 1.0)).sqrt();
    if ysd == 0.0 {
        ysd = 1.0;
    }
    for v in &mut yk {
        *v /= ysd;
    }
    let mut w = vec![vec![0.0; n_comp]; p];
    let mut pl = vec![vec![0.0; n_comp]; p];
    let dotv = |a: &[f64], b: &[f64]| a.iter().zip(b).fold(0.0, |s, (u, v)| s + u * v);
    let mut found = 0;
    for k in 0..n_comp {
        if yk.iter().all(|v| v.abs() < 10.0 * eps) {
            break;
        }
        if !yk.iter().any(|v| v.abs() > eps) {
            break;
        }
        found = k + 1;
        // Power method, mode A, first (and, for one target, only) iteration.
        let yy = dotv(&yk, &yk);
        let mut xw: Vec<f64> = (0..p).map(|j| xk.iter().zip(&yk).fold(0.0, |s, (r, v)| s + r[j] * v) / yy).collect();
        let norm = dotv(&xw, &xw).sqrt() + eps;
        for v in &mut xw {
            *v /= norm;
        }
        // _svd_flip_1d on x_weights (the y weight's sign is not needed).
        let mut idx = 0;
        for (j, v) in xw.iter().enumerate() {
            if v.abs() > xw[idx].abs() {
                idx = j;
            }
        }
        let sign = if xw[idx] > 0.0 { 1.0 } else if xw[idx] < 0.0 { -1.0 } else { 0.0 };
        for v in &mut xw {
            *v *= sign;
        }
        let xs: Vec<f64> = xk.iter().map(|r| dotv(r, &xw)).collect();
        let tt = dotv(&xs, &xs);
        let xl: Vec<f64> = (0..p).map(|j| xs.iter().zip(&xk).fold(0.0, |s, (t, r)| s + t * r[j]) / tt).collect();
        for (r, t) in xk.iter_mut().zip(&xs) {
            for j in 0..p {
                r[j] -= t * xl[j];
            }
        }
        let yl = dotv(&xs, &yk) / tt;
        for (v, t) in yk.iter_mut().zip(&xs) {
            *v -= t * yl;
        }
        for j in 0..p {
            w[j][k] = xw[j];
            pl[j][k] = xl[j];
        }
    }
    // x_rotations = W pinv(Pᵀ W). When the output was exhausted after
    // `found` components, the later columns of W and P are zero, Pᵀ W is
    // block-diagonal [A 0; 0 0] and its pseudo-inverse is [A⁻¹ 0; 0 0].
    if found == 0 {
        return Err(RafflesError::InvalidParameter {
            parameter: "PLS".to_string(),
            value: f64::NAN,
            reason: "y residual is constant: no PLS component".to_string(),
        });
    }
    let ptw: Vec<Vec<f64>> = (0..found).map(|a| (0..found).map(|b| (0..p).fold(0.0, |s, j| s + pl[j][a] * w[j][b])).collect()).collect();
    let inv_a = inverse(&ptw).ok_or(RafflesError::InvalidParameter {
        parameter: "PLS".to_string(),
        value: f64::NAN,
        reason: "P^T W is singular (the output is constant, or n_comp exceeds the rank)".to_string(),
    })?;
    let inv: Vec<Vec<f64>> = (0..n_comp).map(|a| (0..n_comp).map(|b| if a < found && b < found { inv_a[a][b] } else { 0.0 }).collect()).collect();
    Ok((0..p).map(|j| (0..n_comp).map(|b| (0..n_comp).fold(0.0, |s, a| s + w[j][a] * inv[a][b])).collect()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One component on data where y depends on x₀ only: the rotation is
    /// (±1, 0) up to the sign flip (positive largest entry); two components
    /// where the first exhausts y give sklearn's pseudo-inverse (a zero second
    /// column); bad `n_comp` and a constant output are refused.
    #[test]
    fn pls_rotation_on_a_one_direction_problem() {
        let x: Vec<Vec<f64>> = (0..6).map(|i| vec![f64::from(i), f64::from((i * 7) % 5)]).collect();
        let y: Vec<f64> = x.iter().map(|r| 3.0 * r[0]).collect();
        let r = x_rotations(&x, &y, 1).unwrap();
        assert!(r[0][0] > 0.5 && r[0][0] <= 1.0 + 1e-12);
        let r2 = x_rotations(&x, &y, 2).unwrap();
        assert_eq!(r2.len(), 2);
        assert!(x_rotations(&x, &y, 3).is_err());
        assert!(x_rotations(&x, &[1.0; 6], 1).is_err());
    }
}
