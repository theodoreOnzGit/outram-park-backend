// SPDX-License-Identifier: GPL-3.0-only
//
// TRANSCRIBED to WGSL (f32) from GSL's reference CBLAS and matrix layer:
//   cblas/source_dot_r.h    cblas_sdot    -> petir_blas_dot
//   cblas/source_nrm2_r.h   cblas_snrm2   -> petir_blas_nrm2
//   cblas/source_asum_r.h   cblas_sasum   -> petir_blas_asum
//   cblas/source_iamax_r.h  cblas_isamax  -> petir_blas_iamax
//   cblas/source_gemv_r.h   cblas_sgemv   -> petir_blas_gemv_row
//   cblas/source_gemm_r.h   cblas_sgemm   -> petir_blas_gemm_element
//   matrix/, vector/        element access -> petir_mat_* below
//   Copyright (C) 1996-2007 Gerard Jungman, Brian Gough. GPL-3.0-or-later.
//
// LAYOUT. Every matrix here is ROW-MAJOR with an explicit leading dimension
// `ld`, matching GSL's `CblasRowMajor` and its `gsl_matrix.tda`. Element
// (i, j) of a matrix based at `off` is `src[off + ld*i + j]`. GSL's
// column-major path is NOT ported -- see the module docs in `wgsl/mod.rs`.
//
// WHAT A GPU CHANGES, AND WHAT IT MUST NOT.
//
// The arithmetic of each inner accumulation is upstream's, unchanged. What
// cannot be preserved is GSL's OUTER loop structure, because a compute
// invocation owns one output element and must therefore sum over the
// contracted index itself. Concretely, GSL's gemm NoTrans/NoTrans branch
// accumulates k-outer (`C[i][j] += alpha*A[i][k] * B[k][j]` swept by k),
// while a per-element kernel is forced into the k-inner dot product that
// GSL's NoTrans/Trans branch uses. Those are the same value in exact
// arithmetic and DIFFERENT in f32.
//
// This is a real, documented deviation, not an oversight. The `f32` CPU
// mirror in `wgsl::mirror_matrix` uses the k-inner order too, so GPU-vs-mirror
// stays exact and the reassociation cost shows up where it belongs: in
// mirror-vs-GSL.

// ---------------------------------------------------------------------------
// Element access -- `gsl_matrix_get` / `gsl_matrix_set` equivalents
// ---------------------------------------------------------------------------

// Element (i, j) of a row-major matrix based at `off` with leading dimension
// `ld`. Ports `gsl_matrix_get`'s indexing (`m->data[i * m->tda + j]`).
//
// NO BOUNDS CHECK. GSL's `gsl_matrix_get` range-checks under
// `GSL_RANGE_CHECK` and returns 0 on violation; WGSL clamps or discards an
// out-of-range storage access by its own rules, so the guard would be dead
// code on every conforming device. Callers pass valid indices.
fn petir_mat_get(off: u32, ld: u32, i: u32, j: u32) -> f32 {
    return src[off + ld * i + j];
}

// Linear index of element (i, j), for a caller writing into `dst`.
fn petir_mat_index(ld: u32, i: u32, j: u32) -> u32 {
    return ld * i + j;
}

// ---------------------------------------------------------------------------
// BLAS Level 1
// ---------------------------------------------------------------------------

// Dot product of two unit-stride vectors. Ports `cblas_sdot`.
//
// GSL accumulates into a single scalar in index order; that order is kept,
// because changing it changes the answer in f32 and this is the function a
// caller compares against.
fn petir_blas_dot(off_x: u32, off_y: u32, n: u32) -> f32 {
    var r: f32 = 0.0;
    for (var i: u32 = 0u; i < n; i = i + 1u) {
        r = r + src[off_x + i] * src[off_y + i];
    }
    return r;
}

// Euclidean norm. Ports `cblas_snrm2`.
//
// This is NOT `sqrt(dot(x, x))`. GSL uses the scaled sum-of-squares
// recurrence, which tracks a running maximum and rescales, so a vector whose
// squares would overflow -- or underflow to zero -- still gives the right
// answer. In f32 the range is only ~1e38, so this matters at far more ordinary
// magnitudes than it does in f64: |x| ~ 2e19 already overflows the naive form.
fn petir_blas_nrm2(off_x: u32, n: u32) -> f32 {
    if (n == 0u) { return 0.0; }
    if (n == 1u) { return abs(src[off_x]); }

    var scale: f32 = 0.0;
    var ssq: f32 = 1.0;
    for (var i: u32 = 0u; i < n; i = i + 1u) {
        let x = src[off_x + i];
        if (x != 0.0) {
            let ax = abs(x);
            if (scale < ax) {
                ssq = 1.0 + ssq * (scale / ax) * (scale / ax);
                scale = ax;
            } else {
                ssq = ssq + (ax / scale) * (ax / scale);
            }
        }
    }
    return scale * sqrt(ssq);
}

// Sum of absolute values. Ports `cblas_sasum`.
fn petir_blas_asum(off_x: u32, n: u32) -> f32 {
    var r: f32 = 0.0;
    for (var i: u32 = 0u; i < n; i = i + 1u) {
        r = r + abs(src[off_x + i]);
    }
    return r;
}

// Index of the first element of largest absolute value. Ports `cblas_isamax`.
//
// Returned as f32 so it fits the one-output-per-invocation harness; it is an
// exact integer for any length a dispatch can address.
fn petir_blas_iamax(off_x: u32, n: u32) -> f32 {
    if (n == 0u) { return 0.0; }
    var max_val: f32 = abs(src[off_x]);
    var result: u32 = 0u;
    for (var i: u32 = 1u; i < n; i = i + 1u) {
        let a = abs(src[off_x + i]);
        if (a > max_val) {
            max_val = a;
            result = i;
        }
    }
    return f32(result);
}

// ---------------------------------------------------------------------------
// BLAS Level 2 -- one invocation per output element of y
// ---------------------------------------------------------------------------

// Row `i` of `y := alpha*A*x + beta*y`, row-major, no transpose.
//
// Ports the `CblasRowMajor && CblasNoTrans` branch of `cblas_sgemv`,
// including its accumulation into a local `temp` before the single
// `y[i] += alpha*temp` -- that is upstream's association and it is kept.
//
// `y_old` is the incoming y[i]; a kernel cannot read-modify-write the same
// buffer it maps for output without a race, so beta is applied here.
fn petir_blas_gemv_row(
    off_a: u32, lda: u32, off_x: u32, n_cols: u32,
    alpha: f32, beta: f32, y_old: f32, i: u32
) -> f32 {
    var temp: f32 = 0.0;
    for (var j: u32 = 0u; j < n_cols; j = j + 1u) {
        temp = temp + src[off_x + j] * src[off_a + lda * i + j];
    }
    return beta * y_old + alpha * temp;
}

// Row `i` of `y := alpha*A'*x + beta*y`, row-major.
//
// Ports the `CblasRowMajor && CblasTrans` branch. Upstream sweeps j-outer and
// scatters into y; a per-element kernel must gather instead, which walks the
// same products in the same order for a fixed output index.
fn petir_blas_gemv_row_trans(
    off_a: u32, lda: u32, off_x: u32, n_rows: u32,
    alpha: f32, beta: f32, y_old: f32, i: u32
) -> f32 {
    var temp: f32 = 0.0;
    for (var j: u32 = 0u; j < n_rows; j = j + 1u) {
        temp = temp + src[off_x + j] * src[off_a + lda * j + i];
    }
    return beta * y_old + alpha * temp;
}

// ---------------------------------------------------------------------------
// BLAS Level 3 -- one invocation per output element of C
// ---------------------------------------------------------------------------

// Element (i, j) of `C := alpha*A*B + beta*C`, all row-major, no transpose.
//
// Ports `cblas_sgemm`'s arithmetic. See the header: the k-inner dot product
// here is GSL's NoTrans/TRANS association, not its NoTrans/NoTrans one, and
// that substitution is forced by one invocation owning one output element.
fn petir_blas_gemm_element(
    off_a: u32, lda: u32, off_b: u32, ldb: u32,
    k_dim: u32, alpha: f32, beta: f32, c_old: f32, i: u32, j: u32
) -> f32 {
    var temp: f32 = 0.0;
    for (var k: u32 = 0u; k < k_dim; k = k + 1u) {
        temp = temp + src[off_a + lda * i + k] * src[off_b + ldb * k + j];
    }
    return beta * c_old + alpha * temp;
}

// Element (i, j) of `C := alpha*A*B' + beta*C`.
//
// Ports the NoTrans/Trans branch directly -- this one IS upstream's own
// association, because that branch already computes an explicit dot product
// per output element.
fn petir_blas_gemm_element_nt(
    off_a: u32, lda: u32, off_b: u32, ldb: u32,
    k_dim: u32, alpha: f32, beta: f32, c_old: f32, i: u32, j: u32
) -> f32 {
    var temp: f32 = 0.0;
    for (var k: u32 = 0u; k < k_dim; k = k + 1u) {
        temp = temp + src[off_a + lda * i + k] * src[off_b + ldb * j + k];
    }
    return beta * c_old + alpha * temp;
}

// ---------------------------------------------------------------------------
// Matrix element-wise operations -- `gsl_matrix_*`
// ---------------------------------------------------------------------------

// Element (i, j) of `A + B`. Ports `gsl_matrix_add`.
fn petir_mat_add(off_a: u32, lda: u32, off_b: u32, ldb: u32, i: u32, j: u32) -> f32 {
    return src[off_a + lda * i + j] + src[off_b + ldb * i + j];
}

// Element (i, j) of `A - B`. Ports `gsl_matrix_sub`.
fn petir_mat_sub(off_a: u32, lda: u32, off_b: u32, ldb: u32, i: u32, j: u32) -> f32 {
    return src[off_a + lda * i + j] - src[off_b + ldb * i + j];
}

// Element (i, j) of the Hadamard product. Ports `gsl_matrix_mul_elements`.
fn petir_mat_mul_elements(off_a: u32, lda: u32, off_b: u32, ldb: u32, i: u32, j: u32) -> f32 {
    return src[off_a + lda * i + j] * src[off_b + ldb * i + j];
}

// Element (i, j) of the element-wise quotient. Ports `gsl_matrix_div_elements`.
fn petir_mat_div_elements(off_a: u32, lda: u32, off_b: u32, ldb: u32, i: u32, j: u32) -> f32 {
    return src[off_a + lda * i + j] / src[off_b + ldb * i + j];
}

// Element (i, j) scaled. Ports `gsl_matrix_scale`.
fn petir_mat_scale(off_a: u32, lda: u32, s: f32, i: u32, j: u32) -> f32 {
    return src[off_a + lda * i + j] * s;
}

// Element (i, j) shifted. Ports `gsl_matrix_add_constant`.
fn petir_mat_add_constant(off_a: u32, lda: u32, c: f32, i: u32, j: u32) -> f32 {
    return src[off_a + lda * i + j] + c;
}

// Element (i, j) of the transpose, i.e. A(j, i). Ports
// `gsl_matrix_transpose_memcpy`.
fn petir_mat_transpose(off_a: u32, lda: u32, i: u32, j: u32) -> f32 {
    return src[off_a + lda * j + i];
}
