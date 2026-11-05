// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// NOT a port. Small dense linear algebra for the SMT kriging port (gh:#1190):
// what SMT obtains from scipy.linalg (LAPACK `dpotrf`, `dtrtrs`, `dgeqrf` +
// `dorgqr`, `dgesdd`, `dgetri`) written as plain unblocked loops, because the
// crate takes no BLAS/LAPACK (crate CLAUDE.md, "Android / Termux"). The
// Householder QR follows the textbook algorithm with LAPACK's `dlarfg` sign
// convention (`beta = -sign(alpha) * norm`), so `G`'s diagonal has LAPACK's
// signs; the operation order is not LAPACK's, so results agree to round-off,
// not bit for bit (V&V record `smt_kriging_gh1190.md`).

//! Dense helpers on `Vec<Vec<f64>>` rows: triangular solves, a Householder
//! QR, a symmetric eigenvalue routine for condition estimates, and a small
//! inverse.

use petir::mathf::RealMath;

/// Solves `L z = b` for lower-triangular `L`.
#[must_use]
pub fn forward(l: &[Vec<f64>], b: &[f64]) -> Vec<f64> {
    let n = b.len();
    let mut z = vec![0.0; n];
    for i in 0..n {
        let mut s = b[i];
        for k in 0..i {
            s -= l[i][k] * z[k];
        }
        z[i] = s / l[i][i];
    }
    z
}

/// Solves `Lᵀ x = z` for lower-triangular `L`.
#[must_use]
pub fn backward_lt(l: &[Vec<f64>], z: &[f64]) -> Vec<f64> {
    let n = z.len();
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut s = z[i];
        for k in i + 1..n {
            s -= l[k][i] * x[k];
        }
        x[i] = s / l[i][i];
    }
    x
}

/// Solves `U x = b` for upper-triangular `U`.
#[must_use]
pub fn backward_upper(u: &[Vec<f64>], b: &[f64]) -> Vec<f64> {
    let n = b.len();
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut s = b[i];
        for k in i + 1..n {
            s -= u[i][k] * x[k];
        }
        x[i] = s / u[i][i];
    }
    x
}

/// Solves `Uᵀ x = b` for upper-triangular `U` (a lower solve).
#[must_use]
pub fn forward_ut(u: &[Vec<f64>], b: &[f64]) -> Vec<f64> {
    let n = b.len();
    let mut x = vec![0.0; n];
    for i in 0..n {
        let mut s = b[i];
        for k in 0..i {
            s -= u[k][i] * x[k];
        }
        x[i] = s / u[i][i];
    }
    x
}

/// `Σ a_i b_i`, in index order.
#[must_use]
pub fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).fold(0.0, |s, (x, y)| s + x * y)
}

/// Columns of an `n × p` row-major matrix.
#[must_use]
pub fn columns(a: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let p = a.first().map_or(0, Vec::len);
    (0..p).map(|j| a.iter().map(|r| r[j]).collect()).collect()
}

/// Economic QR of an `n × p` matrix (`n ≥ p`) by Householder reflections:
/// `(Q, G)` with `Q` `n × p` (orthonormal columns, row-major) and `G` `p × p`
/// upper triangular, `A = Q G`. scipy's `qr(mode="economic")`.
#[must_use]
pub fn householder_qr(a: &[Vec<f64>]) -> (Vec<Vec<f64>>, Vec<Vec<f64>>) {
    let n = a.len();
    let p = a.first().map_or(0, Vec::len);
    let mut w = a.to_vec();
    let mut taus = vec![0.0; p];
    for j in 0..p {
        let alpha = w[j][j];
        let xnorm = (j + 1..n).map(|i| w[i][j] * w[i][j]).sum::<f64>().sqrt();
        if xnorm == 0.0 {
            taus[j] = 0.0;
            continue;
        }
        let norm = alpha.r_hypot(xnorm);
        let beta = if alpha >= 0.0 { -norm } else { norm };
        taus[j] = (beta - alpha) / beta;
        let scale = 1.0 / (alpha - beta);
        for row in w.iter_mut().skip(j + 1) {
            row[j] *= scale;
        }
        w[j][j] = beta;
        // Apply H = I − τ v vᵀ (v₀ = 1) to the columns right of j.
        for c in j + 1..p {
            let mut s = w[j][c];
            for i in j + 1..n {
                s += w[i][j] * w[i][c];
            }
            s *= taus[j];
            w[j][c] -= s;
            for i in j + 1..n {
                let v = w[i][j];
                w[i][c] -= s * v;
            }
        }
    }
    let mut g = vec![vec![0.0; p]; p];
    for i in 0..p {
        for jj in i..p {
            g[i][jj] = w[i][jj];
        }
    }
    // Q = H₀ H₁ … H_{p−1} applied to the first p columns of I, backwards.
    let mut q = vec![vec![0.0; p]; n];
    for (i, row) in q.iter_mut().enumerate().take(p) {
        row[i] = 1.0;
    }
    for j in (0..p).rev() {
        if taus[j] == 0.0 {
            continue;
        }
        for c in j..p {
            let mut s = q[j][c];
            for i in j + 1..n {
                s += w[i][j] * q[i][c];
            }
            s *= taus[j];
            q[j][c] -= s;
            for i in j + 1..n {
                let v = w[i][j];
                q[i][c] -= s * v;
            }
        }
    }
    (q, g)
}

/// Eigenvalues of a small symmetric matrix (cyclic Jacobi), unordered.
#[must_use]
pub fn symmetric_eigenvalues(a: &[Vec<f64>]) -> Vec<f64> {
    let n = a.len();
    let mut m = a.to_vec();
    for _sweep in 0..100 {
        let off: f64 = (0..n).flat_map(|i| (0..n).filter(move |j| *j != i).map(move |j| (i, j))).map(|(i, j)| m[i][j] * m[i][j]).sum();
        if off < 1e-300 {
            break;
        }
        for pi in 0..n {
            for qi in pi + 1..n {
                if m[pi][qi] == 0.0 {
                    continue;
                }
                let theta = (m[qi][qi] - m[pi][pi]) / (2.0 * m[pi][qi]);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let t = if theta == 0.0 { 1.0 } else { t };
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for k in 0..n {
                    let (akp, akq) = (m[k][pi], m[k][qi]);
                    m[k][pi] = c * akp - s * akq;
                    m[k][qi] = s * akp + c * akq;
                }
                for k in 0..n {
                    let (apk, aqk) = (m[pi][k], m[qi][k]);
                    m[pi][k] = c * apk - s * aqk;
                    m[qi][k] = s * apk + c * aqk;
                }
            }
        }
    }
    (0..n).map(|i| m[i][i]).collect()
}

/// Ratio of the smallest to the largest singular value of `a` (`n × p`),
/// from the eigenvalues of `aᵀa`: SMT's `sv[-1] / sv[0]` threshold test. Not
/// an accurate condition number near 1e-8 (squaring), which the thresholds
/// it feeds (1e-10 on `G`, 1e15 on `F`) do not need to resolve finely.
#[must_use]
pub fn singular_value_ratio(a: &[Vec<f64>]) -> f64 {
    let cols = columns(a);
    let p = cols.len();
    let ata: Vec<Vec<f64>> = (0..p).map(|i| (0..p).map(|j| dot(&cols[i], &cols[j])).collect()).collect();
    let ev = symmetric_eigenvalues(&ata);
    let max = ev.iter().copied().fold(0.0_f64, f64::max);
    let min = ev.iter().copied().fold(f64::INFINITY, f64::min).max(0.0);
    if max <= 0.0 {
        0.0
    } else {
        (min / max).sqrt()
    }
}

/// Inverse of a small square matrix by Gauss–Jordan with partial pivoting;
/// `None` if singular. Stands in for scipy's `pinv` on a full-rank matrix
/// (where the two agree) and for `linalg.inv`.
#[must_use]
pub fn inverse(a: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let n = a.len();
    let mut m: Vec<Vec<f64>> = a
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut row = r.clone();
            row.extend((0..n).map(|j| if i == j { 1.0 } else { 0.0 }));
            row
        })
        .collect();
    for c in 0..n {
        let piv = (c..n).max_by(|x, y| m[*x][c].abs().total_cmp(&m[*y][c].abs()))?;
        if m[piv][c] == 0.0 || !m[piv][c].is_finite() {
            return None;
        }
        m.swap(c, piv);
        let d = m[c][c];
        for v in &mut m[c] {
            *v /= d;
        }
        for r in 0..n {
            if r != c {
                let f = m[r][c];
                if f != 0.0 {
                    for k in 0..2 * n {
                        m[r][k] -= f * m[c][k];
                    }
                }
            }
        }
    }
    Some(m.into_iter().map(|r| r[n..].to_vec()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// QR reconstructs A, Q has orthonormal columns, G is upper triangular
    /// with LAPACK's sign on the diagonal (negative for a positive leading
    /// entry); the solves, the inverse and the eigenvalues on hand cases.
    #[test]
    fn small_dense_helpers_on_hand_cases() {
        let a = vec![vec![2.0, 1.0], vec![1.0, 3.0], vec![0.5, -1.0], vec![4.0, 0.0]];
        let (q, g) = householder_qr(&a);
        assert!(g[0][0] < 0.0 && g[1][0] == 0.0);
        for i in 0..4 {
            for j in 0..2 {
                let v: f64 = (0..2).map(|k| q[i][k] * g[k][j]).sum();
                assert!((v - a[i][j]).abs() < 1e-14);
            }
        }
        let qc = columns(&q);
        assert!((dot(&qc[0], &qc[0]) - 1.0).abs() < 1e-14 && dot(&qc[0], &qc[1]).abs() < 1e-14);
        let l = vec![vec![2.0, 0.0], vec![1.0, 3.0]];
        let b = [4.0, 5.0];
        let z = forward(&l, &b);
        assert_eq!(z, vec![2.0, 1.0]);
        let x = backward_lt(&l, &z);
        assert!((2.0 * x[0] + x[1] - 2.0).abs() < 1e-15 && (3.0 * x[1] - 1.0).abs() < 1e-15);
        let u = vec![vec![2.0, 1.0], vec![0.0, 3.0]];
        let y = backward_upper(&u, &[5.0, 3.0]);
        assert_eq!(y, vec![2.0, 1.0]);
        let w = forward_ut(&u, &[4.0, 5.0]);
        assert_eq!(w, vec![2.0, 1.0]);
        let inv = inverse(&[vec![4.0, 7.0], vec![2.0, 6.0]]).unwrap();
        assert!((inv[0][0] - 0.6).abs() < 1e-15 && (inv[1][0] + 0.2).abs() < 1e-15);
        assert!(inverse(&[vec![1.0, 2.0], vec![2.0, 4.0]]).is_none());
        let mut ev = symmetric_eigenvalues(&[vec![2.0, 1.0], vec![1.0, 2.0]]);
        ev.sort_by(f64::total_cmp);
        assert!((ev[0] - 1.0).abs() < 1e-14 && (ev[1] - 3.0).abs() < 1e-14);
        assert!((singular_value_ratio(&[vec![1.0, 0.0], vec![0.0, 2.0]]) - 0.5).abs() < 1e-15);
        assert_eq!(singular_value_ratio(&[vec![0.0]]), 0.0);
        // A column already zero below the diagonal: τ = 0, Q = I.
        let (q1, g1) = householder_qr(&[vec![3.0], vec![0.0]]);
        assert_eq!((q1[0][0], g1[0][0]), (1.0, 3.0));
    }
}
