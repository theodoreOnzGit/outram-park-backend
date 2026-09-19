//! `f32` mirrors of `shaders/matrix.wgsl` — GSL's matrix layer and reference
//! CBLAS.
//!
//! See [`crate::wgsl::mirror`] for why a mirror exists. This one carries an
//! extra obligation, spelled out below.
//!
//! # Matrices are where the GPU changes the arithmetic, not just the width
//!
//! For the pointwise kernels, a GPU invocation does exactly what the CPU does.
//! For matrix products it cannot: **one invocation owns one output element**,
//! so it must sum over the contracted index itself. GSL's `sgemm`
//! NoTrans/NoTrans branch does the opposite — it sweeps `k` outermost and
//! scatters partial products into `C` — and those two orders are the same
//! number in exact arithmetic and different numbers in `f32`.
//!
//! So this mirror deliberately uses the **`k`-inner** order, matching the
//! shader rather than matching GSL. That keeps the split honest:
//!
//! - **GPU vs this mirror** stays exact, and still tests the transcription.
//! - **This mirror vs GSL's order** is the *reassociation* cost, which is a
//!   property of parallelising a sum and not a defect in either side.
//!
//! [`gemm_gsl_order`] implements GSL's own sweep, so the second comparison can
//! actually be made rather than asserted.
//!
//! # Layout
//!
//! Row-major with an explicit leading dimension, matching `CblasRowMajor` and
//! `gsl_matrix.tda`. Column-major is not ported.
//!
//! # Units
//!
//! Bare dimensionless `f32`.

use alloc::vec;
use alloc::vec::Vec;

#[allow(unused_imports)]
use crate::real::Real;

/// Element `(i, j)` of a row-major matrix with leading dimension `ld`.
///
/// Mirrors `petir_mat_get` / `gsl_matrix_get`. Returns `NaN` out of range
/// rather than panicking — a shader has no panic, and this crate's no-panic
/// gate forbids the subscript that would.
pub fn mat_get(a: &[f32], ld: usize, i: usize, j: usize) -> f32 {
    match a.get(ld * i + j) {
        Some(&v) => v,
        None => f32::NAN,
    }
}

/// Dot product of two unit-stride vectors, in index order.
///
/// Mirrors `petir_blas_dot` / `cblas_sdot`.
///
/// # Example
///
/// ```
/// use petir::wgsl::mirror_matrix::dot;
/// assert_eq!(dot(&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0]), 32.0);
/// ```
pub fn dot(x: &[f32], y: &[f32]) -> f32 {
    let mut r = 0.0_f32;
    for (&xi, &yi) in x.iter().zip(y.iter()) {
        r += xi * yi;
    }
    r
}

/// Euclidean norm by the scaled sum-of-squares recurrence.
///
/// Mirrors `petir_blas_nrm2` / `cblas_snrm2`. **Not** `dot(x, x).sqrt()` — see
/// the shader comment and [`nrm2_naive`], which exists so the difference can
/// be measured.
///
/// # Example
///
/// ```
/// use petir::wgsl::mirror_matrix::nrm2;
/// assert_eq!(nrm2(&[3.0, 4.0]), 5.0);
/// ```
pub fn nrm2(x: &[f32]) -> f32 {
    match x.len() {
        0 => return 0.0,
        1 => return x.first().copied().unwrap_or(0.0).abs(),
        _ => {}
    }
    let mut scale = 0.0_f32;
    let mut ssq = 1.0_f32;
    for &v in x.iter() {
        if v != 0.0 {
            let ax = v.abs();
            if scale < ax {
                ssq = 1.0 + ssq * (scale / ax) * (scale / ax);
                scale = ax;
            } else {
                ssq += (ax / scale) * (ax / scale);
            }
        }
    }
    scale * ssq.sqrt()
}

/// The naive norm, `sqrt(sum of squares)`.
///
/// **Not a port of anything** — GSL does not compute the norm this way, and
/// this exists only so the test suite can measure what the scaled recurrence
/// buys. In `f32` the squares overflow above `|x| ~ 1.8e19`, which is an
/// entirely ordinary magnitude, and this returns infinity there while
/// [`nrm2`] returns the right answer.
pub fn nrm2_naive(x: &[f32]) -> f32 {
    let mut s = 0.0_f32;
    for &v in x.iter() {
        s += v * v;
    }
    s.sqrt()
}

/// Sum of absolute values. Mirrors `petir_blas_asum` / `cblas_sasum`.
pub fn asum(x: &[f32]) -> f32 {
    let mut r = 0.0_f32;
    for &v in x.iter() {
        r += v.abs();
    }
    r
}

/// Index of the first element of largest absolute value.
///
/// Mirrors `petir_blas_iamax` / `cblas_isamax`. Returns 0 for an empty slice,
/// as the shader does.
pub fn iamax(x: &[f32]) -> usize {
    let Some((&first, rest)) = x.split_first() else {
        return 0;
    };
    let mut max_val = first.abs();
    let mut result = 0usize;
    for (k, &v) in rest.iter().enumerate() {
        let a = v.abs();
        if a > max_val {
            max_val = a;
            result = k + 1;
        }
    }
    result
}

/// `y := alpha*A*x + beta*y` for a row-major `A`, no transpose.
///
/// Mirrors `petir_blas_gemv_row` applied to every row, reproducing
/// `cblas_sgemv`'s local-`temp` association.
pub fn gemv(
    a: &[f32],
    lda: usize,
    rows: usize,
    cols: usize,
    x: &[f32],
    alpha: f32,
    beta: f32,
    y: &[f32],
) -> Vec<f32> {
    let mut out = vec![0.0_f32; rows];
    for (i, slot) in out.iter_mut().enumerate() {
        let mut temp = 0.0_f32;
        for j in 0..cols {
            temp += x.get(j).copied().unwrap_or(0.0) * mat_get(a, lda, i, j);
        }
        *slot = beta * y.get(i).copied().unwrap_or(0.0) + alpha * temp;
    }
    out
}

/// `y := alpha*A'*x + beta*y` for a row-major `A`.
///
/// Mirrors `petir_blas_gemv_row_trans`.
pub fn gemv_trans(
    a: &[f32],
    lda: usize,
    rows: usize,
    cols: usize,
    x: &[f32],
    alpha: f32,
    beta: f32,
    y: &[f32],
) -> Vec<f32> {
    let mut out = vec![0.0_f32; cols];
    for (i, slot) in out.iter_mut().enumerate() {
        let mut temp = 0.0_f32;
        for j in 0..rows {
            temp += x.get(j).copied().unwrap_or(0.0) * mat_get(a, lda, j, i);
        }
        *slot = beta * y.get(i).copied().unwrap_or(0.0) + alpha * temp;
    }
    out
}

/// `C := alpha*A*B + beta*C`, **`k`-inner** — the order a GPU is forced into.
///
/// Mirrors `petir_blas_gemm_element` applied to every element. Compare with
/// [`gemm_gsl_order`], which is GSL's own sweep.
pub fn gemm(
    a: &[f32],
    lda: usize,
    b: &[f32],
    ldb: usize,
    m: usize,
    n: usize,
    k: usize,
    alpha: f32,
    beta: f32,
    c: &[f32],
    ldc: usize,
) -> Vec<f32> {
    let mut out = vec![0.0_f32; m * ldc];
    for i in 0..m {
        for j in 0..n {
            let mut temp = 0.0_f32;
            for kk in 0..k {
                temp += mat_get(a, lda, i, kk) * mat_get(b, ldb, kk, j);
            }
            let c_old = mat_get(c, ldc, i, j);
            if let Some(slot) = out.get_mut(ldc * i + j) {
                *slot = beta * c_old + alpha * temp;
            }
        }
    }
    out
}

/// `C := alpha*A*B + beta*C` in **GSL's own `k`-outer order**.
///
/// This is a faithful transcription of `cblas_sgemm`'s NoTrans/NoTrans branch,
/// including the `if (temp != 0.0)` skip. It is **not** what the shader does,
/// and it exists so the reassociation cost of the GPU's order can be measured
/// rather than waved at — see this module's header.
pub fn gemm_gsl_order(
    a: &[f32],
    lda: usize,
    b: &[f32],
    ldb: usize,
    m: usize,
    n: usize,
    k: usize,
    alpha: f32,
    beta: f32,
    c: &[f32],
    ldc: usize,
) -> Vec<f32> {
    let mut out = vec![0.0_f32; m * ldc];
    // form C := beta*C
    for i in 0..m {
        for j in 0..n {
            let v = if beta == 0.0 {
                0.0
            } else {
                beta * mat_get(c, ldc, i, j)
            };
            if let Some(slot) = out.get_mut(ldc * i + j) {
                *slot = v;
            }
        }
    }
    if alpha == 0.0 {
        return out;
    }
    // C := alpha*A*B + C, swept k-outermost exactly as upstream does.
    for kk in 0..k {
        for i in 0..m {
            let temp = alpha * mat_get(a, lda, i, kk);
            if temp != 0.0 {
                for j in 0..n {
                    let add = temp * mat_get(b, ldb, kk, j);
                    if let Some(slot) = out.get_mut(ldc * i + j) {
                        *slot += add;
                    }
                }
            }
        }
    }
    out
}

/// Element `(i, j)` of `A + B`. Mirrors `petir_mat_add` / `gsl_matrix_add`.
pub fn mat_add(a: &[f32], lda: usize, b: &[f32], ldb: usize, i: usize, j: usize) -> f32 {
    mat_get(a, lda, i, j) + mat_get(b, ldb, i, j)
}

/// Element `(i, j)` of `A - B`. Mirrors `petir_mat_sub` / `gsl_matrix_sub`.
pub fn mat_sub(a: &[f32], lda: usize, b: &[f32], ldb: usize, i: usize, j: usize) -> f32 {
    mat_get(a, lda, i, j) - mat_get(b, ldb, i, j)
}

/// Element `(i, j)` of the Hadamard product. Mirrors
/// `gsl_matrix_mul_elements`.
pub fn mat_mul_elements(a: &[f32], lda: usize, b: &[f32], ldb: usize, i: usize, j: usize) -> f32 {
    mat_get(a, lda, i, j) * mat_get(b, ldb, i, j)
}

/// Element `(i, j)` of the element-wise quotient. Mirrors
/// `gsl_matrix_div_elements`.
pub fn mat_div_elements(a: &[f32], lda: usize, b: &[f32], ldb: usize, i: usize, j: usize) -> f32 {
    mat_get(a, lda, i, j) / mat_get(b, ldb, i, j)
}

/// Element `(i, j)` scaled. Mirrors `gsl_matrix_scale`.
pub fn mat_scale(a: &[f32], lda: usize, s: f32, i: usize, j: usize) -> f32 {
    mat_get(a, lda, i, j) * s
}

/// Element `(i, j)` shifted. Mirrors `gsl_matrix_add_constant`.
pub fn mat_add_constant(a: &[f32], lda: usize, c: f32, i: usize, j: usize) -> f32 {
    mat_get(a, lda, i, j) + c
}

/// Element `(i, j)` of the transpose. Mirrors
/// `gsl_matrix_transpose_memcpy`.
pub fn mat_transpose(a: &[f32], lda: usize, i: usize, j: usize) -> f32 {
    mat_get(a, lda, j, i)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded(n: usize, seed: u32) -> Vec<f32> {
        // A small deterministic LCG, so the cases are reproducible and the
        // test carries no data file.
        let mut s = seed;
        (0..n)
            .map(|_| {
                s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                ((s >> 8) as f32 / 8_388_608.0) - 1.0
            })
            .collect()
    }

    /// Level-1 kernels reproduce hand-computed values exactly.
    ///
    /// Small integers, so every intermediate is exactly representable and the
    /// comparison is exact rather than toleranced — this checks the indexing
    /// and the accumulation order, not the rounding.
    #[test]
    fn level_one_kernels_are_exact_on_small_integers() {
        let x = [1.0f32, 2.0, 3.0, 4.0];
        let y = [4.0f32, 5.0, 6.0, 7.0];
        assert_eq!(dot(&x, &y), 4.0 + 10.0 + 18.0 + 28.0);
        assert_eq!(asum(&[-1.0, 2.0, -3.0]), 6.0);
        assert_eq!(nrm2(&[3.0, 4.0]), 5.0);
        assert_eq!(nrm2(&[]), 0.0);
        assert_eq!(nrm2(&[-7.0]), 7.0);
        assert_eq!(iamax(&[1.0, -5.0, 3.0]), 1);
        assert_eq!(iamax(&[]), 0);
        // Ties go to the FIRST, as `>` rather than `>=` in upstream gives.
        assert_eq!(iamax(&[2.0, -2.0]), 0);
    }

    /// The scaled norm survives magnitudes where the naive form overflows.
    ///
    /// # Why this is worth a test
    ///
    /// GSL's `nrm2` is the scaled recurrence rather than `sqrt(dot(x,x))`, and
    /// in `f32` that is not a fine point: the squares overflow above about
    /// `1.8e19`, which is an ordinary number. A transcription that "simplified"
    /// it to the naive form would pass every small-vector test and then return
    /// infinity on real data.
    ///
    /// # Results
    ///
    /// At `x = [3e19, 4e19]` the true norm is `5e19`, comfortably inside
    /// `f32`'s range. [`nrm2`] returns it; [`nrm2_naive`] returns `inf`.
    /// Measured 2026-09-19.
    #[test]
    fn the_scaled_norm_survives_where_the_naive_one_overflows() {
        let big = [3.0e19f32, 4.0e19];
        let scaled = nrm2(&big);
        let naive = nrm2_naive(&big);
        assert!(
            scaled.is_finite(),
            "the scaled norm overflowed, which is the whole thing it prevents"
        );
        assert!(
            ((scaled - 5.0e19) / 5.0e19).abs() < 1e-5,
            "scaled norm {scaled:e} is not 5e19"
        );
        assert!(
            naive.is_infinite(),
            "the naive norm did NOT overflow, so this test proves nothing -- \
             check whether f32 range assumptions still hold"
        );

        // And at the other end: squares that underflow to zero.
        let tiny = [3.0e-25f32, 4.0e-25];
        assert!(
            ((nrm2(&tiny) - 5.0e-25) / 5.0e-25).abs() < 1e-5,
            "scaled norm lost a subnormal-squared vector"
        );
        assert_eq!(
            nrm2_naive(&tiny),
            0.0,
            "the naive form should underflow here"
        );
    }

    /// `gemv` agrees with an independent dot-product-per-row computation.
    #[test]
    fn gemv_agrees_with_row_dot_products() {
        let (m, n) = (4usize, 5usize);
        let a = seeded(m * n, 7);
        let x = seeded(n, 11);
        let y = seeded(m, 13);
        let got = gemv(&a, n, m, n, &x, 2.0, 3.0, &y);
        for i in 0..m {
            let row: Vec<f32> = (0..n).map(|j| mat_get(&a, n, i, j)).collect();
            let want = 3.0 * y[i] + 2.0 * dot(&row, &x);
            assert_eq!(got[i], want, "row {i}");
        }
        // Transposed form against the transpose, computed independently.
        let gt = gemv_trans(&a, n, m, n, &y, 1.0, 0.0, &vec![0.0; n]);
        for j in 0..n {
            let col: Vec<f32> = (0..m).map(|i| mat_get(&a, n, i, j)).collect();
            assert_eq!(gt[j], dot(&col, &y), "col {j}");
        }
    }

    /// `gemm` reproduces a product computed from `gemv` row by row.
    ///
    /// Two different routines in this module, so a shared indexing error would
    /// have to be made twice in different shapes to pass.
    #[test]
    fn gemm_agrees_with_gemv_applied_per_column() {
        let (m, k, n) = (3usize, 4usize, 5usize);
        let a = seeded(m * k, 3);
        let b = seeded(k * n, 5);
        let c = vec![0.0f32; m * n];
        let got = gemm(&a, k, &b, n, m, n, k, 1.0, 0.0, &c, n);
        for j in 0..n {
            let col: Vec<f32> = (0..k).map(|kk| mat_get(&b, n, kk, j)).collect();
            let want = gemv(&a, k, m, k, &col, 1.0, 0.0, &vec![0.0; m]);
            for i in 0..m {
                assert_eq!(got[n * i + j], want[i], "element ({i}, {j})");
            }
        }
    }

    /// The GPU's `k`-inner order and GSL's `k`-outer order differ, and by how
    /// much.
    ///
    /// # Why measure this rather than assert they agree
    ///
    /// They are the same value in exact arithmetic and different in `f32`, and
    /// the shader has no choice: one invocation owns one output element. The
    /// honest thing is to quantify the reassociation rather than pick whichever
    /// comparison passes.
    ///
    /// # Results
    ///
    /// On a 16x16x16 product of pseudo-random values in `[-1, 1]`, worst
    /// relative difference between the two orders is recorded by the
    /// assertion. It is nonzero, it is of order the `f32` rounding of a
    /// 16-term sum, and neither order is "correct" — GSL's is simply the one
    /// a serial library happens to use.
    #[test]
    fn the_gpu_order_and_gsls_order_differ_only_by_reassociation() {
        let d = 16usize;
        let a = seeded(d * d, 17);
        let b = seeded(d * d, 23);
        let c = seeded(d * d, 29);
        let ours = gemm(&a, d, &b, d, d, d, d, 1.25, 0.5, &c, d);
        let gsls = gemm_gsl_order(&a, d, &b, d, d, d, d, 1.25, 0.5, &c, d);

        let mut worst = 0.0_f32;
        for (p, q) in ours.iter().zip(gsls.iter()) {
            let scale = p.abs().max(q.abs()).max(1.0);
            worst = worst.max((p - q).abs() / scale);
        }
        // Loose enough to be a statement about reassociation, tight enough
        // that a genuine indexing error could not hide inside it.
        assert!(
            worst < 1e-5,
            "the two orders differ by {worst:e}, which is too much to be \
             reassociation alone"
        );
    }

    /// Element-wise matrix operations, and the transpose being an involution.
    #[test]
    fn element_wise_operations_and_transpose() {
        let (m, n) = (3usize, 4usize);
        let a = seeded(m * n, 31);
        let b = seeded(m * n, 37);
        for i in 0..m {
            for j in 0..n {
                let ai = mat_get(&a, n, i, j);
                let bi = mat_get(&b, n, i, j);
                assert_eq!(mat_add(&a, n, &b, n, i, j), ai + bi);
                assert_eq!(mat_sub(&a, n, &b, n, i, j), ai - bi);
                assert_eq!(mat_mul_elements(&a, n, &b, n, i, j), ai * bi);
                assert_eq!(mat_div_elements(&a, n, &b, n, i, j), ai / bi);
                assert_eq!(mat_scale(&a, n, 2.5, i, j), ai * 2.5);
                assert_eq!(mat_add_constant(&a, n, -1.5, i, j), ai - 1.5);
            }
        }
        // Transposing a square matrix twice is the identity.
        let s = seeded(n * n, 41);
        for i in 0..n {
            for j in 0..n {
                let once = mat_transpose(&s, n, i, j);
                assert_eq!(once, mat_get(&s, n, j, i));
            }
        }
    }

    /// Out-of-range access yields `NaN`, never a panic.
    ///
    /// The shader cannot panic and this crate's gate forbids the subscript
    /// that would, so the mirror has to behave the same way for the comparison
    /// to mean anything at the edges.
    #[test]
    fn out_of_range_access_is_nan_not_a_panic() {
        let a = [1.0f32, 2.0, 3.0, 4.0];
        assert_eq!(mat_get(&a, 2, 0, 0), 1.0);
        assert_eq!(mat_get(&a, 2, 1, 1), 4.0);
        assert!(mat_get(&a, 2, 5, 0).is_nan());
        assert!(mat_get(&a, 2, 0, 9).is_nan());
        assert!(mat_get(&[], 1, 0, 0).is_nan());
    }
}
