// ---------------------------------------------------------------------------
// Ported from OpenDA (open data assimilation toolbox).
//
//   Upstream project: OpenDA, OpenDA Association
//   Upstream repo:    https://github.com/OpenDA-Association/OpenDA
//   Upstream files:   core/java/src/org/openda/utils/Matrix.java
//                       (multiply, mult, rightMultiply, inverse, sqrt, svd,
//                        rightSolve, diag, eye, concatVertical, Matrix(IVector[]),
//                        asVectorArray)
//                     core/java/src/org/openda/utils/VectorDouble.java
//                       (axpy, scale, norm2, pointwiseMultiply/Divide, sqrt)
//                     algorithms/.../kalmanFilter/AbstractSequentialEnsembleAlgorithm.java
//                       (ensembleAverage, removeAverage)
//   Upstream commit:  e51565c2c8def7d35f8b378a9ac118edbd98a94d
//   Accessed:         2026-11-04
//
//   Copyright (c) 2012 OpenDA Association. All rights reserved.
//   OpenDA is free software: you can redistribute it and/or modify it under
//   the terms of the GNU Lesser General Public License as published by the
//   Free Software Foundation, either version 3 of the License, or (at your
//   option) any later version. OpenDA is distributed in the hope that it will
//   be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of
//   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU Lesser
//   General Public License for more details (LICENSE-LGPL-3.0-OPENDA).
//
// This Rust translation is part of RAFFLES / Outram Park and is conveyed under
// AGPL-3.0-only as a combined work (LGPLv3 is GPLv3 plus permissions; GPLv3
// section 13 / AGPLv3 section 13). It may not be offered back under LGPL terms
// alone except as the LGPL itself allows for this file.
//
// Translation notes: Java's arithmetic is sequential IEEE without FMA, so each
// function keeps the source's loop order and association (e.g. rightMultiply's
// `(beta*L(i,j))*v(j)`). Matrices are `Vec<Vec<f64>>` (Java double[][]); an
// OpenDA vector-of-columns `Matrix(IVector[])` is `from_columns`. inverse(),
// sqrt(), svd() and rightSolve() call the JAMA port (`super::jama`) exactly as
// OpenDA does, including its absolute 1e-7 singular-value cut-off.
// ---------------------------------------------------------------------------

//! **OpenDA's `Matrix` and `Vector` kernels**, in Java's rounding order
//! (gh:#1192).

use super::jama::{self, Rows};
use crate::{RafflesError, Result};

fn mismatch(expected: usize, found: usize) -> RafflesError {
    RafflesError::DimensionMismatch { expected, found }
}

fn cols(a: &[Vec<f64>]) -> usize {
    a.first().map_or(0, Vec::len)
}

/// `m × n` zeros.
#[must_use]
pub fn zeros(m: usize, n: usize) -> Rows {
    vec![vec![0.0; n]; m]
}

/// `Matrix.eye(n)`.
#[must_use]
pub fn eye(n: usize) -> Rows {
    let mut r = zeros(n, n);
    for (i, row) in r.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    r
}

/// `Matrix.diag(IVector)` / `Matrix(double[])`: the diagonal matrix of `v`.
#[must_use]
pub fn diag_matrix(v: &[f64]) -> Rows {
    let mut r = zeros(v.len(), v.len());
    for (i, x) in v.iter().enumerate() {
        r[i][i] = *x;
    }
    r
}

/// `Matrix.diag()`: the diagonal of `a`.
#[must_use]
pub fn diag(a: &[Vec<f64>]) -> Vec<f64> {
    (0..a.len().min(cols(a))).map(|i| a[i][i]).collect()
}

/// `new Matrix(IVector[])`: each vector is a **column**.
#[must_use]
pub fn from_columns(v: &[Vec<f64>]) -> Rows {
    let m = v.first().map_or(0, Vec::len);
    (0..m).map(|i| v.iter().map(|c| c[i]).collect()).collect()
}

/// `Matrix.asVectorArray()`: the columns.
#[must_use]
pub fn columns(a: &[Vec<f64>]) -> Vec<Vec<f64>> {
    (0..cols(a)).map(|j| a.iter().map(|r| r[j]).collect()).collect()
}

/// `Matrix.scale(f)`: `a(i,j) = a(i,j)·f`.
pub fn scale_matrix(a: &mut [Vec<f64>], f: f64) {
    for row in a.iter_mut() {
        for x in row.iter_mut() {
            *x *= f;
        }
    }
}

/// `Matrix.concatVertical(a, b)`.
#[must_use]
pub fn concat_vertical(a: &[Vec<f64>], b: &[Vec<f64>]) -> Rows {
    let n = cols(a).max(cols(b));
    a.iter()
        .chain(b.iter())
        .map(|r| {
            let mut x = r.clone();
            x.resize(n, 0.0);
            x
        })
        .collect()
}

/// `Matrix.multiply(facAB, A, B, facThis, transa, transb)`: `C = facThis·C +
/// facAB·op(A)·op(B)`, element by element: `C *= facThis`, one sum chain,
/// then `C += facAB·sum`.
///
/// # Errors
/// [`RafflesError::DimensionMismatch`] on incompatible shapes.
#[allow(clippy::many_single_char_names)]
pub fn multiply(c: &mut [Vec<f64>], fac_ab: f64, a: &[Vec<f64>], b: &[Vec<f64>], fac_c: f64, transa: bool, transb: bool) -> Result<()> {
    let (am, an, bm, bn) = (a.len(), cols(a), b.len(), cols(b));
    let (rows, inner, inner_b, outc) = match (transa, transb) {
        (false, false) => (am, an, bm, bn),
        (true, false) => (an, am, bm, bn),
        (false, true) => (am, an, bn, bm),
        (true, true) => (an, am, bn, bm),
    };
    if inner != inner_b {
        return Err(mismatch(inner, inner_b));
    }
    if c.len() != rows || cols(c) != outc {
        return Err(mismatch(rows * outc, c.len() * cols(c)));
    }
    let ga = |i: usize, k: usize| if transa { a[k][i] } else { a[i][k] };
    let gb = |k: usize, j: usize| if transb { b[j][k] } else { b[k][j] };
    for i in 0..rows {
        for j in 0..outc {
            c[i][j] *= fac_c;
            let mut sum = 0.0;
            for k in 0..inner {
                sum += ga(i, k) * gb(k, j);
            }
            c[i][j] += fac_ab * sum;
        }
    }
    Ok(())
}

/// `Matrix.mult(A, B, ta, tb)`: a zero result, then [`multiply`] with factors 1.
///
/// # Errors
/// As [`multiply`].
pub fn mult(a: &[Vec<f64>], b: &[Vec<f64>], ta: bool, tb: bool) -> Result<Rows> {
    let r = if ta { cols(a) } else { a.len() };
    let c = if tb { b.len() } else { cols(b) };
    let mut out = zeros(r, c);
    multiply(&mut out, 1.0, a, b, 1.0, ta, tb)?;
    Ok(out)
}

/// `Matrix.rightMultiply(beta, v, alpha, x)`: `x = alpha·x + beta·L·v`, in
/// OpenDA's order `temp = alpha·x(i); temp += (beta·L(i,j))·v(j)`. Like
/// OpenDA, nothing is done on a shape mismatch.
pub fn right_multiply(l: &[Vec<f64>], beta: f64, v: &[f64], alpha: f64, x: &mut [f64]) {
    if x.len() == l.len() && v.len() == cols(l) {
        for i in 0..l.len() {
            let mut temp = alpha * x[i];
            for j in 0..v.len() {
                temp += beta * l[i][j] * v[j];
            }
            x[i] = temp;
        }
    }
}

fn svd_of(a: &[Vec<f64>]) -> Result<jama::JamaSvd> {
    jama::svd(a)
}

/// `Matrix.inverse()`: JAMA SVD, `V·diag(1/s)·Uᵀ` with `|s| ≤ 1e-7` zeroed.
///
/// # Errors
/// From the SVD (an empty or wide matrix).
pub fn inverse(a: &[Vec<f64>]) -> Result<Rows> {
    let d = svd_of(a)?;
    let mut s = d.s_matrix();
    for i in 0..d.s.len() {
        s[i][i] = if d.s[i].abs() > 1e-7 { 1.0 / d.s[i] } else { 0.0 };
    }
    jama::times(&jama::times(&d.v, &s)?, &jama::transpose(&d.u))
}

/// `Matrix.sqrt()`: `U·diag(√s)·Vᵀ` with `|s| < 1e-7` zeroed.
///
/// # Errors
/// From the SVD, or a non-square matrix.
pub fn sqrt(a: &[Vec<f64>]) -> Result<Rows> {
    if a.len() != cols(a) {
        return Err(mismatch(a.len(), cols(a)));
    }
    let d = svd_of(a)?;
    let mut s = d.s_matrix();
    for i in 0..d.s.len() {
        // OpenDA's first branch tests |s| < -eps, which is never true.
        s[i][i] = if d.s[i].abs() < 1e-7 { 0.0 } else { d.s[i].sqrt() };
    }
    jama::times(&jama::times(&d.u, &s)?, &jama::transpose(&d.v))
}

/// `Matrix.svd()`: `[U, S, V]`, a wide matrix transposed first (and `U`,
/// `V` swapped back).
///
/// # Errors
/// From the SVD.
pub fn svd(a: &[Vec<f64>]) -> Result<(Rows, Rows, Rows)> {
    let wide = cols(a) > a.len();
    let d = if wide { svd_of(&jama::transpose(a))? } else { svd_of(a)? };
    let s = d.s_matrix();
    Ok(if wide { (d.v, s, d.u) } else { (d.u, s, d.v) })
}

/// `Matrix.rightSolve(v, x)`: the least-squares `x` of `L·x = v`, as OpenDA
/// computes it: `xᵀ = vᵀ·U·diag(1/s)·Vᵀ` (1e-7 cut-off), through
/// `Jama.Matrix.times`.
///
/// # Errors
/// From the SVD (OpenDA throws for `m < n` too), or `v` of the wrong length.
pub fn right_solve(l: &[Vec<f64>], v: &[f64]) -> Result<Vec<f64>> {
    if v.len() != l.len() {
        return Err(mismatch(l.len(), v.len()));
    }
    let d = svd_of(l)?;
    let mut s = d.s_matrix();
    for i in 0..d.s.len() {
        s[i][i] = if d.s[i].abs() < 1e-7 { 0.0 } else { 1.0 / d.s[i] };
    }
    let y = vec![v.to_vec()];
    let x = jama::times(&jama::times(&jama::times(&y, &d.u)?, &s)?, &jama::transpose(&d.v))?;
    Ok(x.into_iter().next().unwrap_or_default())
}

/// `VectorDouble.axpy(alpha, x)`: `y(i) = y(i) + alpha·x(i)`.
pub fn axpy(y: &mut [f64], alpha: f64, x: &[f64]) {
    for (a, b) in y.iter_mut().zip(x) {
        *a += alpha * b;
    }
}

/// `VectorDouble.scale(alpha)`: `y(i) = alpha·y(i)`.
pub fn scale(y: &mut [f64], alpha: f64) {
    for a in y.iter_mut() {
        *a *= alpha;
    }
}

/// `VectorDouble.norm2()`: `√(Σ y(i)²)`, one chain.
#[must_use]
pub fn norm2(y: &[f64]) -> f64 {
    let mut out = 0.0;
    for a in y {
        out += a * a;
    }
    out.sqrt()
}

/// `Math.pow(norm2(y), 2.0)`, which OpenDA's costs use and which equals
/// `n·n` on every sample measured (gh:#1192 record, finding 3).
#[must_use]
pub fn norm2_squared(y: &[f64]) -> f64 {
    let n = norm2(y);
    n * n
}

/// `VectorDouble.pointwiseMultiply`.
pub fn pointwise_multiply(y: &mut [f64], x: &[f64]) {
    for (a, b) in y.iter_mut().zip(x) {
        *a *= b;
    }
}

/// `VectorDouble.pointwiseDivide`.
pub fn pointwise_divide(y: &mut [f64], x: &[f64]) {
    for (a, b) in y.iter_mut().zip(x) {
        *a /= b;
    }
}

/// `ensembleAverage`: member 0, plus each later member (`axpy(1.0, ·)`),
/// then `scale(1.0/n)`.
#[must_use]
pub fn ensemble_average(v: &[Vec<f64>]) -> Vec<f64> {
    let mut r = v[0].clone();
    for x in &v[1..] {
        axpy(&mut r, 1.0, x);
    }
    scale(&mut r, 1.0 / v.len() as f64);
    r
}

/// `removeAverage`: `axpy(-1.0, mean)` on each member.
pub fn remove_average(v: &mut [Vec<f64>], mean: &[f64]) {
    for x in v.iter_mut() {
        axpy(x, -1.0, mean);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Inverse·A = I, sqrt² = A, rightSolve solves a consistent system,
    /// the transposed multiply paths agree, svd of a wide matrix, and the
    /// shape errors.
    #[test]
    fn kernels_behave() {
        let a = vec![vec![4.0, 1.0], vec![1.0, 3.0]];
        let i = mult(&inverse(&a).unwrap(), &a, false, false).unwrap();
        let s = sqrt(&a).unwrap();
        let ss = mult(&s, &s, false, false).unwrap();
        for r in 0..2 {
            for c in 0..2 {
                assert!((i[r][c] - f64::from(u8::from(r == c))).abs() < 1e-14);
                assert!((ss[r][c] - a[r][c]).abs() < 1e-13);
            }
        }
        let x = right_solve(&a, &[5.0, 4.0]).unwrap();
        assert!((x[0] - 1.0).abs() < 1e-14 && (x[1] - 1.0).abs() < 1e-14);
        let b = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        assert_eq!(mult(&b, &b, false, true).unwrap(), mult(&from_columns(&columns(&b)), &jama::transpose(&b), false, false).unwrap());
        assert_eq!(mult(&b, &b, true, false).unwrap(), mult(&jama::transpose(&b), &b, false, false).unwrap());
        let bt = jama::transpose(&b);
        assert_eq!(mult(&b, &bt, true, true).unwrap(), mult(&bt, &b, false, false).unwrap());
        let (u, sm, v) = svd(&b).unwrap();
        let r = mult(&mult(&u, &sm, false, false).unwrap(), &v, false, true).unwrap();
        for (p, q) in r.iter().flatten().zip(b.iter().flatten()) {
            assert!((p - q).abs() < 1e-12);
        }
        assert!(mult(&b, &b, false, false).is_err());
        assert!(sqrt(&b).is_err());
        assert!(right_solve(&a, &[1.0]).is_err());
        let mut c = zeros(1, 1);
        assert!(multiply(&mut c, 1.0, &b, &b, 0.0, false, true).is_err());
        assert_eq!(diag(&eye(3)), vec![1.0; 3]);
        assert_eq!(diag_matrix(&[2.0, 3.0])[1][1], 3.0);
        assert_eq!(concat_vertical(&a, &[vec![9.0]]), vec![vec![4.0, 1.0], vec![1.0, 3.0], vec![9.0, 0.0]]);
        let mut m = a.clone();
        scale_matrix(&mut m, 2.0);
        assert_eq!(m[0][0], 8.0);
        let mut y = vec![3.0, 4.0];
        assert_eq!(norm2(&y), 5.0);
        assert_eq!(norm2_squared(&y), 25.0);
        pointwise_multiply(&mut y, &[2.0, 2.0]);
        pointwise_divide(&mut y, &[4.0, 4.0]);
        assert_eq!(y, vec![1.5, 2.0]);
        let mut ens = vec![vec![1.0], vec![3.0]];
        let mean = ensemble_average(&ens);
        remove_average(&mut ens, &mean);
        assert_eq!(ens, vec![vec![-1.0], vec![1.0]]);
        let mut x0 = vec![1.0, 1.0];
        right_multiply(&a, 1.0, &[1.0], 1.0, &mut x0);
        assert_eq!(x0, vec![1.0, 1.0], "shape mismatch leaves x unchanged, as OpenDA");
    }
}
