// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// NOT ported from FilterPy, and no OpenBLAS source was read or copied. The
// operation orders below were MEASURED (gh:#1191) by comparing numpy 2.4.6 /
// scipy 1.17.1 (OpenBLAS 0.3.31, SkylakeX dispatch) bit for bit against exact
// emulations: verification_and_validation/filterpy_gh1191/probe_numpy_kernels.py.

//! **A small dense matrix and numpy's rounding order** (gh:#1191).
//!
//! FilterPy is numpy: every `dot` is an OpenBLAS gemm, gemv or ddot, `inv`
//! is LAPACK `dgesv`, scipy's `cholesky` is `dpotrf`. For the filters to be
//! bit-identical with FilterPy, each operation here rounds exactly as that
//! kernel does. The orders were measured, not read from a library
//! (`verification_and_validation/filterpy_gh1191.md`, section 2.3):
//!
//! | numpy call | here | order | measured envelope |
//! |---|---|---|---|
//! | `dot(A, B)`, both matrices | [`Matrix::dot`] | one FMA chain over k per element | any size tried (≤ 5×5, 3×7·7×3) |
//! | `dot(A, x)`, A with ≥ 2 rows | [`Matrix::dot_vec`] | 4-lane FMA blocks reduced `(l0+l2)+(l1+l3)`, then a k mod 4 tail | k ≤ 7, or rows a multiple of 4 |
//! | `dot(a, x)`, one row | [`ddot`] | one FMA chain | k ≤ 15 |
//! | `dot(w, S)`, 1-D w | [`vec_dot`] | pairs of FMAs in fours, then a chain | S with 2 or 3 columns |
//! | `linalg.inv` | [`Matrix::inv`] | left-looking LU, trsm with reciprocal diagonals in power-of-two blocks | n ≤ 5 |
//! | `scipy.linalg.cholesky` (upper) | [`Matrix::cholesky_upper`] | `dpotf2` with the gemv's −1 folded into x | n ≤ 4 |
//! | `mean(axis=0)`, `sum(axis=0)` | [`mean_rows`], [`sum_matrices`] | sequential over rows; numpy's **pairwise** sum when each row is a single element (the axis is then contiguous) | rows of 1, 2, 3 and stacks of 1×1, 2×1, 1×2, 3×3 (found when D7 first missed) |
//!
//! Outside an envelope the kernels keep the same structure and are correct
//! linear algebra, but are **not claimed bit-identical** to numpy: the
//! OpenBLAS kernels there (gemv with 4x2 / 4x1 blocks at k ≥ 8, ddot at
//! k ≥ 16, gemv-N with ≥ 4 columns, `inv` beyond 5×5) were not measured
//! or did not match. The order is also **CPU-dispatch dependent**: the
//! references were made on SkylakeX.
//!
//! `f64::mul_add` is a correctly rounded fused multiply-add on every target
//! (a hardware FMA, or libm's software `fma`), so the results do not depend
//! on the build's target features.

use crate::bayesian::koh::pairwise_sum;
use crate::{RafflesError, Result};

#[inline]
fn fma(a: f64, b: f64, c: f64) -> f64 {
    a.mul_add(b, c)
}

/// A dense row-major matrix of `f64`.
#[derive(Debug, Clone, PartialEq)]
pub struct Matrix {
    rows: usize,
    cols: usize,
    data: Vec<f64>,
}

fn mismatch(expected: usize, found: usize) -> RafflesError {
    RafflesError::DimensionMismatch { expected, found }
}

impl Matrix {
    /// A `rows × cols` matrix of zeros.
    #[must_use]
    pub fn zeros(rows: usize, cols: usize) -> Self {
        Self { rows, cols, data: vec![0.0; rows * cols] }
    }

    /// The `n × n` identity.
    #[must_use]
    pub fn identity(n: usize) -> Self {
        let mut m = Self::zeros(n, n);
        for i in 0..n {
            m.data[i * n + i] = 1.0;
        }
        m
    }

    /// From row-major data.
    ///
    /// # Errors
    /// [`RafflesError::DimensionMismatch`] if `data.len() != rows * cols`.
    pub fn from_row_major(rows: usize, cols: usize, data: Vec<f64>) -> Result<Self> {
        if data.len() != rows * cols {
            return Err(mismatch(rows * cols, data.len()));
        }
        Ok(Self { rows, cols, data })
    }

    /// From rows, which must all have the same length (at least one row).
    ///
    /// # Errors
    /// [`RafflesError::DimensionMismatch`] on ragged or empty rows.
    pub fn from_rows(rows: &[Vec<f64>]) -> Result<Self> {
        let cols = rows.first().map_or(0, Vec::len);
        if rows.is_empty() || cols == 0 {
            return Err(mismatch(1, 0));
        }
        let mut data = Vec::with_capacity(rows.len() * cols);
        for r in rows {
            if r.len() != cols {
                return Err(mismatch(cols, r.len()));
            }
            data.extend_from_slice(r);
        }
        Ok(Self { rows: rows.len(), cols, data })
    }

    /// A column vector (`n × 1`), FilterPy's 2-D `x`.
    #[must_use]
    pub fn column(v: &[f64]) -> Self {
        Self { rows: v.len(), cols: 1, data: v.to_vec() }
    }

    /// A diagonal matrix.
    #[must_use]
    pub fn diagonal(d: &[f64]) -> Self {
        let n = d.len();
        let mut m = Self::zeros(n, n);
        for (i, v) in d.iter().enumerate() {
            m.data[i * n + i] = *v;
        }
        m
    }

    /// Number of rows.
    #[must_use]
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Number of columns.
    #[must_use]
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Element `(i, j)`.
    ///
    /// # Panics
    /// If out of range (an indexing bug in the caller).
    #[must_use]
    pub fn get(&self, i: usize, j: usize) -> f64 {
        assert!(i < self.rows && j < self.cols, "Matrix::get({i}, {j}) on {}x{}", self.rows, self.cols);
        self.data[i * self.cols + j]
    }

    /// Set element `(i, j)`.
    ///
    /// # Panics
    /// If out of range (an indexing bug in the caller).
    pub fn set(&mut self, i: usize, j: usize, v: f64) {
        assert!(i < self.rows && j < self.cols, "Matrix::set({i}, {j}) on {}x{}", self.rows, self.cols);
        self.data[i * self.cols + j] = v;
    }

    /// Row `i` as a slice.
    #[must_use]
    pub fn row(&self, i: usize) -> &[f64] {
        &self.data[i * self.cols..(i + 1) * self.cols]
    }

    /// Replace row `i`.
    ///
    /// # Errors
    /// [`RafflesError::DimensionMismatch`] if `v` is not a full row.
    pub fn set_row(&mut self, i: usize, v: &[f64]) -> Result<()> {
        if v.len() != self.cols || i >= self.rows {
            return Err(mismatch(self.cols, v.len()));
        }
        self.data[i * self.cols..(i + 1) * self.cols].copy_from_slice(v);
        Ok(())
    }

    /// Column `j`, copied.
    #[must_use]
    pub fn col(&self, j: usize) -> Vec<f64> {
        (0..self.rows).map(|i| self.data[i * self.cols + j]).collect()
    }

    /// The row-major data.
    #[must_use]
    pub fn as_slice(&self) -> &[f64] {
        &self.data
    }

    /// As nested rows.
    #[must_use]
    pub fn to_rows(&self) -> Vec<Vec<f64>> {
        (0..self.rows).map(|i| self.row(i).to_vec()).collect()
    }

    /// The main diagonal.
    #[must_use]
    pub fn diag(&self) -> Vec<f64> {
        (0..self.rows.min(self.cols)).map(|i| self.get(i, i)).collect()
    }

    /// The transpose.
    #[must_use]
    pub fn t(&self) -> Self {
        let mut m = Self::zeros(self.cols, self.rows);
        for i in 0..self.rows {
            for j in 0..self.cols {
                m.data[j * self.rows + i] = self.data[i * self.cols + j];
            }
        }
        m
    }

    fn same_shape(&self, other: &Self) -> Result<()> {
        if self.rows != other.rows || self.cols != other.cols {
            return Err(mismatch(self.rows * self.cols, other.rows * other.cols));
        }
        Ok(())
    }

    /// Elementwise `self + other`.
    ///
    /// # Errors
    /// On a shape mismatch.
    pub fn add(&self, other: &Self) -> Result<Self> {
        self.same_shape(other)?;
        Ok(Self { rows: self.rows, cols: self.cols, data: self.data.iter().zip(&other.data).map(|(a, b)| a + b).collect() })
    }

    /// Elementwise `self − other`.
    ///
    /// # Errors
    /// On a shape mismatch.
    pub fn sub(&self, other: &Self) -> Result<Self> {
        self.same_shape(other)?;
        Ok(Self { rows: self.rows, cols: self.cols, data: self.data.iter().zip(&other.data).map(|(a, b)| a - b).collect() })
    }

    /// `s * self` elementwise (numpy's scalar times array).
    #[must_use]
    pub fn scale(&self, s: f64) -> Self {
        Self { rows: self.rows, cols: self.cols, data: self.data.iter().map(|a| s * a).collect() }
    }

    /// `self / s` elementwise.
    #[must_use]
    pub fn div_scalar(&self, s: f64) -> Self {
        Self { rows: self.rows, cols: self.cols, data: self.data.iter().map(|a| a / s).collect() }
    }

    /// numpy's `dot(self, other)` for two 2-D arrays, in numpy's dispatch
    /// (`cblas_matrixproduct`): a 1-row by 1-column product is `ddot`; a
    /// product with a column vector is gemv; a 1-row matrix times a matrix
    /// is the transposed gemv; anything else is gemm.
    ///
    /// # Errors
    /// [`RafflesError::DimensionMismatch`] if `self.cols != other.rows`.
    pub fn dot(&self, other: &Self) -> Result<Self> {
        if self.cols != other.rows {
            return Err(mismatch(self.cols, other.rows));
        }
        let (m, k, n) = (self.rows, self.cols, other.cols);
        if m == 1 && n == 1 {
            return Ok(Self { rows: 1, cols: 1, data: vec![ddot(&self.data, &other.data)] });
        }
        if n == 1 {
            return Ok(Self::column(&self.dot_vec(&other.data)?));
        }
        if m == 1 {
            return Ok(Self { rows: 1, cols: n, data: vec_dot(&self.data, other)? });
        }
        let mut out = Self::zeros(m, n);
        for i in 0..m {
            for j in 0..n {
                let mut s = 0.0;
                for p in 0..k {
                    s = fma(self.data[i * k + p], other.data[p * n + j], s);
                }
                out.data[i * n + j] = s;
            }
        }
        Ok(out)
    }

    /// numpy's `dot(self, x)` with x 1-D or a column: ddot for one row,
    /// OpenBLAS gemv-T otherwise.
    ///
    /// # Errors
    /// [`RafflesError::DimensionMismatch`] if `x.len() != self.cols`.
    pub fn dot_vec(&self, x: &[f64]) -> Result<Vec<f64>> {
        if x.len() != self.cols {
            return Err(mismatch(self.cols, x.len()));
        }
        if self.rows == 1 {
            return Ok(vec![ddot(&self.data, x)]);
        }
        Ok((0..self.rows).map(|i| gemv_t_accumulate(self.row(i), x, 1.0, 0.0)).collect())
    }

    /// numpy's `linalg.inv` (LAPACK `dgesv` on the identity, OpenBLAS's
    /// `getf2` and `trsm`), in the measured order.
    ///
    /// # Errors
    /// [`RafflesError::DimensionMismatch`] if not square;
    /// [`RafflesError::InvalidParameter`] if singular (a zero pivot, numpy's
    /// "Singular matrix").
    pub fn inv(&self) -> Result<Self> {
        if self.rows != self.cols || self.rows == 0 {
            return Err(mismatch(self.rows, self.cols));
        }
        let n = self.rows;
        let (lu, piv) = lu_getf2(self)?;
        let mut x = Self::identity(n);
        for (i, &p) in piv.iter().enumerate() {
            if p != i {
                for c in 0..n {
                    x.data.swap(i * n + c, p * n + c);
                }
            }
        }
        for col in 0..n {
            let mut c = x.col(col);
            trsm_forward_unit(&lu, &mut c);
            trsm_backward(&lu, &mut c);
            for (i, v) in c.into_iter().enumerate() {
                x.data[i * n + col] = v;
            }
        }
        Ok(x)
    }

    /// scipy's `linalg.cholesky(self)` (upper, `U` with `Uᵀ U = self`),
    /// in OpenBLAS `dpotf2`'s measured order. Only the upper triangle is
    /// read; the lower triangle of the result is zero.
    ///
    /// # Errors
    /// [`RafflesError::DimensionMismatch`] if not square;
    /// [`RafflesError::InvalidParameter`] if not positive definite.
    pub fn cholesky_upper(&self) -> Result<Self> {
        if self.rows != self.cols || self.rows == 0 {
            return Err(mismatch(self.rows, self.cols));
        }
        let n = self.rows;
        let mut u = Self::zeros(n, n);
        for j in 0..n {
            let cj: Vec<f64> = (0..j).map(|p| u.data[p * n + j]).collect();
            let d = self.data[j * n + j] - ddot(&cj, &cj);
            if !(d > 0.0) {
                return Err(RafflesError::InvalidParameter {
                    parameter: "matrix".into(),
                    value: d,
                    reason: format!("not positive definite (leading minor {})", j + 1),
                });
            }
            let ajj = d.sqrt();
            u.data[j * n + j] = ajj;
            let rinv = 1.0 / ajj;
            for i in (j + 1)..n {
                let ci: Vec<f64> = (0..j).map(|p| u.data[p * n + i]).collect();
                let y = gemv_t_accumulate(&ci, &cj, -1.0, self.data[j * n + i]);
                u.data[j * n + i] = y * rinv;
            }
        }
        Ok(u)
    }
}

/// numpy's `dot` of two 1-D arrays (OpenBLAS ddot): one FMA chain from 0.
/// Measured bit-identical for k ≤ 15 (the vector kernel above that is not
/// reproduced).
///
/// # Panics
/// If the lengths differ (a caller bug; every caller checks first).
#[must_use]
pub fn ddot(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len(), "ddot length mismatch");
    a.iter().zip(b).fold(0.0, |s, (x, y)| fma(*x, *y, s))
}

/// One output of OpenBLAS gemv-T: `y + alpha (a · x)`, the dot in 4-lane FMA
/// blocks reduced `(l0 + l2) + (l1 + l3)` and added as `fma(s, alpha, y)`,
/// then the k mod 4 tail with x pre-scaled by alpha.
fn gemv_t_accumulate(a: &[f64], x: &[f64], alpha: f64, y0: f64) -> f64 {
    let k = a.len();
    let m1 = k - k % 4;
    let mut y = y0;
    if m1 > 0 {
        let mut l = [0.0f64; 4];
        for i in (0..m1).step_by(4) {
            for (lane, acc) in l.iter_mut().enumerate() {
                *acc = fma(a[i + lane], x[i + lane], *acc);
            }
        }
        let s = (l[0] + l[2]) + (l[1] + l[3]);
        y = fma(s, alpha, y);
    }
    let a = &a[m1..];
    let xt: Vec<f64> = x[m1..].iter().map(|v| v * alpha).collect();
    match a.len() {
        0 => y,
        1 => fma(a[0], xt[0], y),
        2 => y + fma(a[0], xt[0], a[1] * xt[1]),
        _ => y + fma(a[2], xt[2], fma(a[0], xt[0], a[1] * xt[1])),
    }
}

/// numpy's `dot(w, s)` with 1-D `w` (k) and `s` (k × m): ddot for one
/// column, otherwise OpenBLAS gemv-N's per-column order (pairs of FMAs in
/// fours, then a chain); measured for m = 2, 3.
///
/// # Errors
/// [`RafflesError::DimensionMismatch`] if `w.len() != s.rows()`.
pub fn vec_dot(w: &[f64], s: &Matrix) -> Result<Vec<f64>> {
    if w.len() != s.rows {
        return Err(mismatch(s.rows, w.len()));
    }
    let (k, m) = (s.rows, s.cols);
    if m == 1 {
        return Ok(vec![ddot(&s.data, w)]);
    }
    let at = |i: usize, j: usize| s.data[i * m + j];
    Ok((0..m)
        .map(|j| {
            let mut t = 0.0;
            let mut i = 0;
            while i < (k & !3) {
                t += fma(at(i, j), w[i], at(i + 1, j) * w[i + 1]);
                t += fma(at(i + 2, j), w[i + 2], at(i + 3, j) * w[i + 3]);
                i += 4;
            }
            while i < k {
                t = fma(at(i, j), w[i], t);
                i += 1;
            }
            t
        })
        .collect())
}

/// numpy's `mean(rows, axis=0)`: a sequential sum over rows, then ÷ N; for
/// a one-column matrix numpy's pairwise sum
/// ([`crate::bayesian::koh::pairwise_sum`], reused), because that axis is
/// contiguous.
///
/// # Errors
/// [`RafflesError::DimensionMismatch`] on an empty matrix.
pub fn mean_rows(m: &Matrix) -> Result<Vec<f64>> {
    if m.rows == 0 {
        return Err(mismatch(1, 0));
    }
    if m.cols == 1 {
        return Ok(vec![pairwise_sum(&m.data) / m.rows as f64]);
    }
    let mut acc = m.row(0).to_vec();
    for i in 1..m.rows {
        for (a, b) in acc.iter_mut().zip(m.row(i)) {
            *a += b;
        }
    }
    let n = m.rows as f64;
    Ok(acc.into_iter().map(|a| a / n).collect())
}

/// numpy's `sum(stack, axis=0)` over a stack of equal-shaped matrices:
/// sequential, or numpy's pairwise sum for a stack of 1×1 matrices.
///
/// # Errors
/// On an empty stack or a shape mismatch.
pub fn sum_matrices(stack: &[Matrix]) -> Result<Matrix> {
    let mut acc = stack.first().ok_or_else(|| mismatch(1, 0))?.clone();
    if acc.data.len() == 1 && stack.iter().all(|m| m.data.len() == 1) {
        let v: Vec<f64> = stack.iter().map(|m| m.data[0]).collect();
        acc.data[0] = pairwise_sum(&v);
        return Ok(acc);
    }
    for m in &stack[1..] {
        acc = acc.add(m)?;
    }
    Ok(acc)
}

/// numpy's `outer(a, b)` (exact products).
#[must_use]
pub fn outer(a: &[f64], b: &[f64]) -> Matrix {
    let mut m = Matrix::zeros(a.len(), b.len());
    for (i, x) in a.iter().enumerate() {
        for (j, y) in b.iter().enumerate() {
            m.data[i * b.len() + j] = x * y;
        }
    }
    m
}

/// OpenBLAS `getf2` (left-looking LU, partial pivoting on the first
/// maximum, reciprocal scaling): the packed factors (unit L below the
/// diagonal, U on and above) and the pivot rows.
fn lu_getf2(m: &Matrix) -> Result<(Matrix, Vec<usize>)> {
    let n = m.rows;
    let mut a = m.clone();
    let mut piv: Vec<usize> = (0..n).collect();
    for j in 0..n {
        let mut b = a.col(j);
        for i in 0..j {
            if piv[i] != i {
                b.swap(i, piv[i]);
            }
        }
        for i in 1..j {
            let row: Vec<f64> = (0..i).map(|k| a.get(i, k)).collect();
            b[i] -= ddot(&row, &b[..i]);
        }
        for r in j..n {
            let row: Vec<f64> = (0..j).map(|k| a.get(r, k)).collect();
            b[r] -= ddot(&row, &b[..j]);
        }
        let mut jp = j;
        for r in (j + 1)..n {
            if b[r].abs() > b[jp].abs() {
                jp = r;
            }
        }
        piv[j] = jp;
        for (i, v) in b.iter().enumerate() {
            a.set(i, j, *v);
        }
        if b[jp] == 0.0 {
            return Err(RafflesError::InvalidParameter { parameter: "matrix".into(), value: 0.0, reason: "singular (zero pivot)".into() });
        }
        let t1 = 1.0 / b[jp];
        if jp != j {
            for c in 0..=j {
                let (x, y) = (a.get(j, c), a.get(jp, c));
                a.set(j, c, y);
                a.set(jp, c, x);
            }
        }
        for r in (j + 1)..n {
            a.set(r, j, a.get(r, j) * t1);
        }
    }
    Ok((a, piv))
}

const TRSM_UNROLL: usize = 16;

/// Row blocks of OpenBLAS's forward trsm kernel: full blocks of 16 from
/// the top, then the remainder as powers of two, largest first.
fn forward_blocks(n: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut r = 0;
    for _ in 0..n / TRSM_UNROLL {
        out.push((r, TRSM_UNROLL));
        r += TRSM_UNROLL;
    }
    let mut i = TRSM_UNROLL >> 1;
    while i > 0 {
        if n & i != 0 {
            out.push((r, i));
            r += i;
        }
        i >>= 1;
    }
    out
}

/// Row blocks of the backward kernel: the remainder from the bottom,
/// smallest first, then the full blocks of 16 upwards.
fn backward_blocks(n: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 1;
    while i < TRSM_UNROLL {
        if n & i != 0 {
            out.push(((n & !(i - 1)) - i, i));
        }
        i *= 2;
    }
    let full = n & !(TRSM_UNROLL - 1);
    let mut top = full;
    while top >= TRSM_UNROLL {
        out.push((top - TRSM_UNROLL, TRSM_UNROLL));
        top -= TRSM_UNROLL;
    }
    out
}

/// Solve `L y = c` in place (unit lower triangle of `lu`).
fn trsm_forward_unit(lu: &Matrix, c: &mut [f64]) {
    for (r0, sz) in forward_blocks(c.len()) {
        if r0 > 0 {
            for r in r0..r0 + sz {
                let row: Vec<f64> = (0..r0).map(|k| lu.get(r, k)).collect();
                c[r] = fma(-1.0, ddot(&row, &c[..r0]), c[r]);
            }
        }
        for i in r0..r0 + sz {
            let bb = c[i];
            for k in (i + 1)..r0 + sz {
                c[k] = fma(-bb, lu.get(k, i), c[k]);
            }
        }
    }
}

/// Solve `U x = c` in place (upper triangle of `lu`, reciprocal diagonal).
fn trsm_backward(lu: &Matrix, c: &mut [f64]) {
    let n = c.len();
    for (r0, sz) in backward_blocks(n) {
        let end = r0 + sz;
        if end < n {
            for r in r0..end {
                let row: Vec<f64> = (end..n).map(|k| lu.get(r, k)).collect();
                c[r] = fma(-1.0, ddot(&row, &c[end..]), c[r]);
            }
        }
        for i in (r0..end).rev() {
            let bb = c[i] * (1.0 / lu.get(i, i));
            c[i] = bb;
            for k in r0..i {
                c[k] = fma(-bb, lu.get(k, i), c[k]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: &Matrix, b: &Matrix, tol: f64) -> bool {
        a.rows == b.rows && a.cols == b.cols && a.data.iter().zip(&b.data).all(|(x, y)| (x - y).abs() <= tol)
    }

    /// Plain linear algebra, independent of the rounding order: products,
    /// inverse, Cholesky and the reductions on fixed matrices, including
    /// sizes outside the bit-identity envelope (6×6, 20×20 inverse; k = 9
    /// gemv). The numpy bit-identity itself is pinned by
    /// `tests/filterpy_gh1191.rs` through the filters.
    #[test]
    fn the_kernels_are_correct_linear_algebra() {
        for n in [1usize, 2, 3, 5, 6, 20] {
            let mut a = Matrix::zeros(n, n);
            for i in 0..n {
                for j in 0..n {
                    a.set(i, j, 1.0 / (1.0 + (i + j) as f64) + if i == j { 2.0 } else { 0.0 });
                }
            }
            let ai = a.inv().unwrap();
            assert!(close(&a.dot(&ai).unwrap(), &Matrix::identity(n), 1e-12), "inv n={n}");
            let u = a.cholesky_upper().unwrap();
            assert!(close(&u.t().dot(&u).unwrap(), &a, 1e-12), "chol n={n}");
            for i in 1..n {
                assert_eq!(u.get(i, 0), 0.0);
            }
        }
        // A pivoting case.
        let p = Matrix::from_rows(&[vec![0.0, 2.0, 1.0], vec![1.0, 0.0, 0.0], vec![3.0, 1.0, 4.0]]).unwrap();
        assert!(close(&p.dot(&p.inv().unwrap()).unwrap(), &Matrix::identity(3), 1e-14));
        assert!(Matrix::from_rows(&[vec![1.0, 2.0], vec![2.0, 4.0]]).unwrap().inv().is_err());
        assert!(Matrix::from_rows(&[vec![1.0, 2.0], vec![2.0, 1.0]]).unwrap().cholesky_upper().is_err());
        assert!(Matrix::zeros(2, 3).inv().is_err() && Matrix::zeros(2, 3).cholesky_upper().is_err());
        // Products through every dispatch branch.
        let a = Matrix::from_rows(&[(0..9).map(|v| v as f64).collect(), (0..9).map(|v| (v * v) as f64).collect()]).unwrap();
        let x: Vec<f64> = (0..9).map(|v| 0.5 + v as f64).collect();
        let want: Vec<f64> = (0..2).map(|i| a.row(i).iter().zip(&x).map(|(p, q)| p * q).sum()).collect();
        assert_eq!(a.dot_vec(&x).unwrap(), want);
        assert_eq!(a.dot(&Matrix::column(&x)).unwrap().col(0), want);
        let row = Matrix::from_rows(&[vec![1.0, 2.0, 3.0]]).unwrap();
        assert_eq!(row.dot(&Matrix::column(&[1.0, 1.0, 1.0])).unwrap().get(0, 0), 6.0);
        let s = Matrix::from_rows(&[vec![1.0, 2.0], vec![3.0, 4.0], vec![5.0, 6.0]]).unwrap();
        assert_eq!(row.dot(&s).unwrap().row(0), &[22.0, 28.0]);
        assert_eq!(vec_dot(&[1.0, 1.0, 1.0], &s).unwrap(), vec![9.0, 12.0]);
        let w: Vec<f64> = (0..9).map(|v| v as f64).collect();
        let tall = Matrix::from_row_major(9, 3, (0..27).map(|v| v as f64).collect()).unwrap();
        let want: Vec<f64> = (0..3).map(|j| (0..9).map(|i| w[i] * tall.get(i, j)).sum()).collect();
        assert_eq!(vec_dot(&w, &tall).unwrap(), want);
        assert_eq!(vec_dot(&[2.0, 3.0], &Matrix::column(&[1.0, 1.0])).unwrap(), vec![5.0]);
        assert_eq!(s.t().dot(&s).unwrap().to_rows(), vec![vec![35.0, 44.0], vec![44.0, 56.0]]);
        assert!(s.dot(&s).is_err() && s.dot_vec(&[1.0]).is_err() && vec_dot(&[1.0], &s).is_err());
        // Reductions and helpers.
        assert_eq!(mean_rows(&s).unwrap(), vec![3.0, 4.0]);
        assert_eq!(mean_rows(&Matrix::column(&[1.0, 2.0, 6.0])).unwrap(), vec![3.0]);
        let ones: Vec<Matrix> = (0..10).map(|i| Matrix::diagonal(&[i as f64])).collect();
        assert_eq!(sum_matrices(&ones).unwrap().get(0, 0), 45.0);
        assert_eq!(sum_matrices(&[s.clone(), s.clone()]).unwrap(), s.scale(2.0));
        assert!(sum_matrices(&[]).is_err() && mean_rows(&Matrix::zeros(0, 2)).is_err());
        assert_eq!(outer(&[1.0, 2.0], &[3.0]).col(0), vec![3.0, 6.0]);
        assert_eq!(Matrix::diagonal(&[1.0, 2.0]).diag(), vec![1.0, 2.0]);
        assert_eq!(s.div_scalar(2.0).get(2, 1), 3.0);
        assert!(s.add(&row).is_err() && s.sub(&row).is_err());
        assert_eq!(s.sub(&s).unwrap(), Matrix::zeros(3, 2));
        let mut m = Matrix::zeros(2, 2);
        m.set_row(1, &[7.0, 8.0]).unwrap();
        assert!(m.set_row(1, &[1.0]).is_err());
        assert_eq!(m.as_slice(), &[0.0, 0.0, 7.0, 8.0]);
        assert!(Matrix::from_rows(&[vec![1.0], vec![1.0, 2.0]]).is_err() && Matrix::from_rows(&[]).is_err());
        assert!(Matrix::from_row_major(2, 2, vec![1.0]).is_err());
        assert_eq!(forward_blocks(37), vec![(0, 16), (16, 16), (32, 4), (36, 1)]);
        assert_eq!(backward_blocks(37), vec![(36, 1), (32, 4), (16, 16), (0, 16)]);
    }
}
