// ---------------------------------------------------------------------------
// Ported from JAMA, the Java matrix package of NIST and The MathWorks, as
// vendored by OpenDA (core/java/resources/jama/Jama-1.0.2.jar).
//
//   Upstream:         JAMA 1.0.2, https://math.nist.gov/javanumerics/jama/
//                     (sources: Maven Central gov.nist.math:jama:1.0.2)
//   Upstream files:   Jama/SingularValueDecomposition.java (constructor,
//                     getU, getS, getV), Jama/util/Maths.java (hypot),
//                     Jama/Matrix.java (times, transpose)
//   Accessed:         2026-11-04
//
//   JAMA is in the PUBLIC DOMAIN: "This software is a cooperative product of
//   The MathWorks and the National Institute of Standards and Technology
//   (NIST) which has been released to the public domain."
//
//   The vendored jar's bytecode for SingularValueDecomposition and Maths was
//   checked identical (javap -c, constant-pool indices removed) to these
//   1.0.2 sources; verification_and_validation/openda_gh1192.md section 1.1.
//
// This Rust translation is part of RAFFLES / Outram Park (AGPL-3.0-only).
//
// Translation notes: line-for-line, in JAMA's operation order (no FMA, the
// same loop order and the same hypot), because OpenDA's Matrix.inverse(),
// sqrt(), svd() and rightSolve() all go through it and gh:#1192 compares
// against OpenDA bit for bit. Only m >= n is accepted (JAMA itself warns that
// m < n can fail; OpenDA transposes wide matrices before calling it).
// `wantu`/`wantv` are always true in JAMA and are folded away.
// ---------------------------------------------------------------------------

//! **JAMA's singular value decomposition**, in JAMA's rounding order
//! (gh:#1192). Matrices are `Vec<Vec<f64>>`, Java's `double[][]`.

use petir::mathf::RealMath;
use crate::{RafflesError, Result};

/// A Java `double[][]`.
pub type Rows = Vec<Vec<f64>>;

/// JAMA's `Maths.hypot`: `sqrt(a² + b²)` without under/overflow.
#[must_use]
pub fn hypot(a: f64, b: f64) -> f64 {
    if a.abs() > b.abs() {
        let r = b / a;
        a.abs() * (1.0 + r * r).sqrt()
    } else if b != 0.0 {
        let r = a / b;
        b.abs() * (1.0 + r * r).sqrt()
    } else {
        0.0
    }
}

/// The decomposition `A = U·diag(s)·Vᵀ` of an `m × n` matrix, `m ≥ n`.
#[derive(Debug, Clone, PartialEq)]
pub struct JamaSvd {
    /// `m × n` left singular vectors (JAMA's `getU()` for `m ≥ n`).
    pub u: Rows,
    /// The `n` singular values, decreasing.
    pub s: Vec<f64>,
    /// `n × n` right singular vectors.
    pub v: Rows,
}

impl JamaSvd {
    /// JAMA's `getS()`: the `n × n` diagonal matrix of `s`.
    #[must_use]
    pub fn s_matrix(&self) -> Rows {
        let n = self.v.len();
        let mut x = vec![vec![0.0; n]; n];
        for i in 0..n {
            x[i][i] = self.s[i];
        }
        x
    }
}

/// JAMA's `SingularValueDecomposition(A)`.
///
/// # Errors
/// [`RafflesError::DimensionMismatch`] for `m < n`, an empty or ragged `a`.
#[allow(clippy::too_many_lines, clippy::many_single_char_names, clippy::needless_range_loop)]
pub fn svd(arg: &[Vec<f64>]) -> Result<JamaSvd> {
    let m = arg.len();
    let n = arg.first().map_or(0, Vec::len);
    if m == 0 || n == 0 || arg.iter().any(|r| r.len() != n) {
        return Err(RafflesError::DimensionMismatch { expected: n.max(1), found: 0 });
    }
    if m < n {
        return Err(RafflesError::DimensionMismatch { expected: n, found: m });
    }
    let mut a: Rows = arg.to_vec();
    let nu = m.min(n);
    let mut s = vec![0.0; (m + 1).min(n)];
    let mut u = vec![vec![0.0; nu]; m];
    let mut v = vec![vec![0.0; n]; n];
    let mut e = vec![0.0; n];
    let mut work = vec![0.0; m];

    let nct = (m - 1).min(n);
    let nrt = (n as i64 - 2).min(m as i64).max(0) as usize;
    for k in 0..nct.max(nrt) {
        if k < nct {
            s[k] = 0.0;
            for i in k..m {
                s[k] = hypot(s[k], a[i][k]);
            }
            if s[k] != 0.0 {
                if a[k][k] < 0.0 {
                    s[k] = -s[k];
                }
                for i in k..m {
                    a[i][k] /= s[k];
                }
                a[k][k] += 1.0;
            }
            s[k] = -s[k];
        }
        for j in (k + 1)..n {
            if k < nct && s[k] != 0.0 {
                let mut t = 0.0;
                for i in k..m {
                    t += a[i][k] * a[i][j];
                }
                t = -t / a[k][k];
                for i in k..m {
                    a[i][j] += t * a[i][k];
                }
            }
            e[j] = a[k][j];
        }
        if k < nct {
            for i in k..m {
                u[i][k] = a[i][k];
            }
        }
        if k < nrt {
            e[k] = 0.0;
            for i in (k + 1)..n {
                e[k] = hypot(e[k], e[i]);
            }
            if e[k] != 0.0 {
                if e[k + 1] < 0.0 {
                    e[k] = -e[k];
                }
                for i in (k + 1)..n {
                    e[i] /= e[k];
                }
                e[k + 1] += 1.0;
            }
            e[k] = -e[k];
            if k + 1 < m && e[k] != 0.0 {
                for i in (k + 1)..m {
                    work[i] = 0.0;
                }
                for j in (k + 1)..n {
                    for i in (k + 1)..m {
                        work[i] += e[j] * a[i][j];
                    }
                }
                for j in (k + 1)..n {
                    let t = -e[j] / e[k + 1];
                    for i in (k + 1)..m {
                        a[i][j] += t * work[i];
                    }
                }
            }
            for i in (k + 1)..n {
                v[i][k] = e[i];
            }
        }
    }

    let mut p = n.min(m + 1);
    if nct < n {
        s[nct] = a[nct][nct];
    }
    if m < p {
        s[p - 1] = 0.0;
    }
    if nrt + 1 < p {
        e[nrt] = a[nrt][p - 1];
    }
    e[p - 1] = 0.0;

    for j in nct..nu {
        for i in 0..m {
            u[i][j] = 0.0;
        }
        u[j][j] = 1.0;
    }
    for k in (0..nct).rev() {
        if s[k] != 0.0 {
            for j in (k + 1)..nu {
                let mut t = 0.0;
                for i in k..m {
                    t += u[i][k] * u[i][j];
                }
                t = -t / u[k][k];
                for i in k..m {
                    u[i][j] += t * u[i][k];
                }
            }
            for i in k..m {
                u[i][k] = -u[i][k];
            }
            u[k][k] = 1.0 + u[k][k];
            for i in 0..(k as i64 - 1).max(0) as usize {
                u[i][k] = 0.0;
            }
        } else {
            for i in 0..m {
                u[i][k] = 0.0;
            }
            u[k][k] = 1.0;
        }
    }
    for k in (0..n).rev() {
        if k < nrt && e[k] != 0.0 {
            for j in (k + 1)..nu {
                let mut t = 0.0;
                for i in (k + 1)..n {
                    t += v[i][k] * v[i][j];
                }
                t = -t / v[k + 1][k];
                for i in (k + 1)..n {
                    v[i][j] += t * v[i][k];
                }
            }
        }
        for i in 0..n {
            v[i][k] = 0.0;
        }
        v[k][k] = 1.0;
    }

    let pp = p - 1;
    let eps = 2.0_f64.r_powi(-52);
    let tiny = 2.0_f64.r_powi(-966);
    while p > 0 {
        let mut k: i64 = p as i64 - 2;
        while k >= -1 {
            if k == -1 {
                break;
            }
            let ku = k as usize;
            if e[ku].abs() <= tiny + eps * (s[ku].abs() + s[ku + 1].abs()) {
                e[ku] = 0.0;
                break;
            }
            k -= 1;
        }
        let kase;
        if k == p as i64 - 2 {
            kase = 4;
        } else {
            let mut ks: i64 = p as i64 - 1;
            while ks >= k {
                if ks == k {
                    break;
                }
                let ksu = ks as usize;
                let t = (if ks != p as i64 { e[ksu].abs() } else { 0.0 }) + (if ks != k + 1 { e[ksu - 1].abs() } else { 0.0 });
                if s[ksu].abs() <= tiny + eps * t {
                    s[ksu] = 0.0;
                    break;
                }
                ks -= 1;
            }
            if ks == k {
                kase = 3;
            } else if ks == p as i64 - 1 {
                kase = 1;
            } else {
                kase = 2;
                k = ks;
            }
        }
        k += 1;
        let k = k as usize;
        match kase {
            1 => {
                let mut f = e[p - 2];
                e[p - 2] = 0.0;
                for j in (k..=(p - 2)).rev() {
                    let mut t = hypot(s[j], f);
                    let cs = s[j] / t;
                    let sn = f / t;
                    s[j] = t;
                    if j != k {
                        f = -sn * e[j - 1];
                        e[j - 1] = cs * e[j - 1];
                    }
                    for i in 0..n {
                        t = cs * v[i][j] + sn * v[i][p - 1];
                        v[i][p - 1] = -sn * v[i][j] + cs * v[i][p - 1];
                        v[i][j] = t;
                    }
                }
            }
            2 => {
                let mut f = e[k - 1];
                e[k - 1] = 0.0;
                for j in k..p {
                    let mut t = hypot(s[j], f);
                    let cs = s[j] / t;
                    let sn = f / t;
                    s[j] = t;
                    f = -sn * e[j];
                    e[j] = cs * e[j];
                    for i in 0..m {
                        t = cs * u[i][j] + sn * u[i][k - 1];
                        u[i][k - 1] = -sn * u[i][j] + cs * u[i][k - 1];
                        u[i][j] = t;
                    }
                }
            }
            3 => {
                let scale = s[p - 1].abs().max(s[p - 2].abs()).max(e[p - 2].abs()).max(s[k].abs()).max(e[k].abs());
                let sp = s[p - 1] / scale;
                let spm1 = s[p - 2] / scale;
                let epm1 = e[p - 2] / scale;
                let sk = s[k] / scale;
                let ek = e[k] / scale;
                let b = ((spm1 + sp) * (spm1 - sp) + epm1 * epm1) / 2.0;
                let c = (sp * epm1) * (sp * epm1);
                let mut shift = 0.0;
                if b != 0.0 || c != 0.0 {
                    shift = (b * b + c).sqrt();
                    if b < 0.0 {
                        shift = -shift;
                    }
                    shift = c / (b + shift);
                }
                let mut f = (sk + sp) * (sk - sp) + shift;
                let mut g = sk * ek;
                for j in k..(p - 1) {
                    let mut t = hypot(f, g);
                    let mut cs = f / t;
                    let mut sn = g / t;
                    if j != k {
                        e[j - 1] = t;
                    }
                    f = cs * s[j] + sn * e[j];
                    e[j] = cs * e[j] - sn * s[j];
                    g = sn * s[j + 1];
                    s[j + 1] = cs * s[j + 1];
                    for i in 0..n {
                        t = cs * v[i][j] + sn * v[i][j + 1];
                        v[i][j + 1] = -sn * v[i][j] + cs * v[i][j + 1];
                        v[i][j] = t;
                    }
                    t = hypot(f, g);
                    cs = f / t;
                    sn = g / t;
                    s[j] = t;
                    f = cs * e[j] + sn * s[j + 1];
                    s[j + 1] = -sn * e[j] + cs * s[j + 1];
                    g = sn * e[j + 1];
                    e[j + 1] = cs * e[j + 1];
                    if j < m - 1 {
                        for i in 0..m {
                            t = cs * u[i][j] + sn * u[i][j + 1];
                            u[i][j + 1] = -sn * u[i][j] + cs * u[i][j + 1];
                            u[i][j] = t;
                        }
                    }
                }
                e[p - 2] = f;
            }
            _ => {
                let mut k = k;
                if s[k] <= 0.0 {
                    s[k] = if s[k] < 0.0 { -s[k] } else { 0.0 };
                    for i in 0..=pp {
                        v[i][k] = -v[i][k];
                    }
                }
                while k < pp {
                    if s[k] >= s[k + 1] {
                        break;
                    }
                    s.swap(k, k + 1);
                    if k < n - 1 {
                        for row in v.iter_mut() {
                            row.swap(k, k + 1);
                        }
                    }
                    if k < m - 1 {
                        for row in u.iter_mut() {
                            row.swap(k, k + 1);
                        }
                    }
                    k += 1;
                }
                p -= 1;
            }
        }
    }
    Ok(JamaSvd { u, s, v })
}

/// JAMA's `Matrix.times`: column of `B` copied, then one chain per element.
///
/// # Errors
/// [`RafflesError::DimensionMismatch`] if the inner dimensions differ.
#[allow(clippy::needless_range_loop)]
pub fn times(a: &[Vec<f64>], b: &[Vec<f64>]) -> Result<Rows> {
    let (m, n) = (a.len(), a.first().map_or(0, Vec::len));
    if b.len() != n {
        return Err(RafflesError::DimensionMismatch { expected: n, found: b.len() });
    }
    let bn = b.first().map_or(0, Vec::len);
    let mut c = vec![vec![0.0; bn]; m];
    let mut bcolj = vec![0.0; n];
    for j in 0..bn {
        for k in 0..n {
            bcolj[k] = b[k][j];
        }
        for i in 0..m {
            let mut s = 0.0;
            for k in 0..n {
                s += a[i][k] * bcolj[k];
            }
            c[i][j] = s;
        }
    }
    Ok(c)
}

/// JAMA's `Matrix.transpose` (exact).
#[must_use]
pub fn transpose(a: &[Vec<f64>]) -> Rows {
    let (m, n) = (a.len(), a.first().map_or(0, Vec::len));
    (0..n).map(|j| (0..m).map(|i| a[i][j]).collect()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// U·S·Vᵀ reproduces A, UᵀU = I, s decreasing, on a 3×2 and a 3×3 with a
    /// zero singular value; m < n and ragged input are refused.
    #[test]
    fn decomposition_reconstructs() {
        for a in [vec![vec![1.0, 2.0], vec![3.0, 4.0], vec![5.0, 6.5]], vec![vec![1.0, 2.0, 3.0], vec![2.0, 4.0, 6.0], vec![1.0, 0.0, 1.0]]] {
            let d = svd(&a).unwrap();
            let us = times(&d.u, &d.s_matrix()).unwrap();
            let r = times(&us, &transpose(&d.v)).unwrap();
            for i in 0..a.len() {
                for j in 0..a[0].len() {
                    assert!((r[i][j] - a[i][j]).abs() < 1e-12, "{r:?}");
                }
            }
            assert!(d.s.windows(2).all(|w| w[0] >= w[1]));
            let utu = times(&transpose(&d.u), &d.u).unwrap();
            for i in 0..utu.len() {
                assert!((utu[i][i] - 1.0).abs() < 1e-12 || d.s[i] == 0.0);
            }
        }
        assert!(svd(&[vec![1.0, 2.0]]).is_err());
        assert!(svd(&[vec![1.0, 2.0], vec![1.0]]).is_err());
        assert!(times(&[vec![1.0, 2.0]], &[vec![1.0]]).is_err());
        assert_eq!(hypot(0.0, 0.0), 0.0);
        assert_eq!(hypot(3.0, 4.0), 5.0);
    }
}
