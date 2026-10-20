// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// PORTED (arithmetic replayed in scalar code, see `super`) from crates by
// sarah quiñones, MIT, read from the published crates.io sources on
// 2026-10-20; the full MIT notice is in `super`'s header (`dense/mod.rs`) and
// applies to this file:
//
//   faer 0.24.4, src/linalg/matmul/mod.rs
//     matmul_imp (the f64 dispatch)                      -> matmul
//     matvec_colmajor::matvec                            -> matvec_colmajor
//     matvec_rowmajor::matvec + dot::inner_prod_no_conj_simd
//                                                        -> matvec_rowmajor, dot
//     rank_update::rank_update_imp                       -> rank_update
//     NANO_GEMM_THRESHOLD = 16*16*16
//   faer 0.24.4, src/linalg/matmul/internal/mod.rs
//     spicy_matmul (its x86 AVX2 arm)                    -> spicy_matmul
//   faer 0.24.4, src/linalg/matmul/triangular.rs
//     mat_x_mat_into_lower_impl_unchecked (its x86 arm)  -> matmul_into_lower
//   pulp 0.22.3, src/x86/v3.rs: reduce_sum_f64s (AVX2)    -> dot's final sum
//   nano-gemm 0.2.2, src/lib.rs: execute_f64, Plan::new_f64_impl (AVX2),
//     direct_millikernel, small_direct_millikernel, copy_millikernel
//   nano-gemm-codegen 0.2.1, src/lib.rs: x86 RealKernel (the microkernel
//     body and its three epilogues)                       -> nano_gemm
//   private-gemm-x86 0.1.22, src/lib.rs: gemm (kc = 512, the transposition
//     and reversal rules); build.rs: microkernel (unroll = 1 when
//     need_mask, i.e. for 256-bit AVX2; acc = 0, vfmadd231 per depth step,
//     vmul by alpha, vadd into dst)                       -> private_gemm
// Subscripts are checked accesses (see `super`).

//! faer's `f64` matrix product `dst = [dst +] alpha * lhs * rhs`, its kernel
//! choice and each kernel's rounding, in scalar code. See the
//! [module docs](super) for what is and is not reproduced.

use super::{fma, rd, rdo, wr, Accum, View};
use alloc::vec::Vec;
use crate::{PetirError, Result};

/// faer's `NANO_GEMM_THRESHOLD`.
const NANO_GEMM_THRESHOLD: usize = 16 * 16 * 16;
/// private-gemm-x86's depth block: `kc = min(depth, 512)`.
const PRIVATE_KC: usize = 512;
/// Row tile of the scalar replay of private-gemm (cache blocking only).
const ROW_TILE: usize = 96;
/// nano-gemm's `copy_millikernel` depth block `K_BS`.
const NANO_K_BS: usize = 64;

/// The destination kind private-gemm-x86 writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DstKind {
    /// every element
    Full,
    /// only `i >= j` (local coordinates)
    Lower,
    /// only `i <= j`
    Upper,
}

/// Port of faer 0.24.4 `matmul_imp` for `f64` on an AVX2+FMA x86-64 CPU.
/// `ls`/`rsrc` name the operand buffers; `None` means the operand lives in
/// `d`, the destination's buffer (two views into one faer matrix).
#[allow(clippy::too_many_arguments)]
pub(crate) fn matmul(
    d: &mut [f64],
    dst: View,
    beta: Accum,
    ls: Option<&[f64]>,
    lhs: View,
    rsrc: Option<&[f64]>,
    rhs: View,
    alpha: f64,
) -> Result<()> {
    let mut dst = dst;
    let mut lhs = lhs;
    let mut rhs = rhs;
    let m = dst.nrows;
    let n = dst.ncols;
    let k = lhs.ncols;
    if lhs.nrows != m || rhs.ncols != n || rhs.nrows != k {
        return Err(PetirError::Invalid);
    }
    if m == 0 || n == 0 {
        return Ok(());
    }
    if k == 0 {
        if beta == Accum::Replace {
            for j in 0..n {
                for i in 0..m {
                    *wr(d, dst.at(i, j))? = 0.0;
                }
            }
        }
        return Ok(());
    }
    if dst.rs < 0 {
        dst = dst.rev_rows();
        lhs = lhs.rev_rows();
    }
    if dst.cs < 0 {
        dst = dst.rev_cols();
        rhs = rhs.rev_cols();
    }
    if lhs.cs < 0 {
        lhs = lhs.rev_cols();
        rhs = rhs.rev_rows();
    }
    if n == 1 {
        if dst.rs == 1 && lhs.rs == 1 {
            return matvec_colmajor(d, dst, beta, ls, lhs, rsrc, rhs, alpha);
        }
        if rhs.rs == 1 && lhs.cs == 1 {
            return matvec_rowmajor(d, dst, beta, ls, lhs, rsrc, rhs, alpha);
        }
    }
    if m == 1 {
        // dst <- dst^T, (lhs, rhs) <- (rhs^T, lhs^T)
        let dt = dst.t();
        let lt = rhs.t();
        let rt = lhs.t();
        if dt.rs == 1 && lt.rs == 1 {
            return matvec_colmajor(d, dt, beta, rsrc, lt, ls, rt, alpha);
        }
        if rt.rs == 1 && lt.cs == 1 {
            return matvec_rowmajor(d, dt, beta, rsrc, lt, ls, rt, alpha);
        }
    }
    if k == 1 {
        if dst.rs == 1 && lhs.rs == 1 {
            return rank_update(d, dst, beta, ls, lhs, rsrc, rhs.t(), alpha);
        }
        if dst.cs == 1 && rhs.cs == 1 {
            // dst^T += alpha * rhs^T (column) * lhs^T (row)
            return rank_update(d, dst.t(), beta, rsrc, rhs.t(), ls, lhs, alpha);
        }
    }
    if m.saturating_mul(n).saturating_mul(k) <= NANO_GEMM_THRESHOLD {
        nano_gemm(d, dst, beta, ls, lhs, rsrc, rhs, alpha)
    } else {
        private_gemm(
            d,
            dst,
            None,
            None,
            DstKind::Full,
            beta,
            ls,
            lhs,
            rsrc,
            rhs,
            alpha,
        )
    }
}

/// faer `matvec_colmajor::matvec` (sequential): `dst` (`m x 1`) and `lhs`
/// have unit row stride. Per column `j`: `r = rhs[j] * alpha`, then
/// `dst[i] = fma(lhs[i, j], r, dst[i])` lane by lane.
#[allow(clippy::too_many_arguments)]
fn matvec_colmajor(
    d: &mut [f64],
    dst: View,
    beta: Accum,
    ls: Option<&[f64]>,
    lhs: View,
    rsrc: Option<&[f64]>,
    rhs: View,
    alpha: f64,
) -> Result<()> {
    let m = lhs.nrows;
    if beta == Accum::Replace {
        for i in 0..m {
            *wr(d, dst.at(i, 0))? = 0.0;
        }
    }
    for j in 0..lhs.ncols {
        let r = rdo(d, rsrc, rhs.at(j, 0))? * alpha;
        for i in 0..m {
            let x = rdo(d, ls, lhs.at(i, j))?;
            let y = wr(d, dst.at(i, 0))?;
            *y = fma(x, r, *y);
        }
    }
    Ok(())
}

/// faer `dot::inner_prod_no_conj_simd` on AVX2 (4 lanes): four vector
/// accumulators fed in batches of four, the leftover full vectors into
/// accumulators 0, 1, 2 in turn, the masked tail into the next one; then
/// `(acc0 + acc1) + (acc2 + acc3)` lane-wise and `pulp`'s `reduce_sum_f64s`,
/// `(l0 + l2) + (l1 + l3)`.
fn dot(
    d: &[f64],
    asrc: Option<&[f64]>,
    a0: isize,
    a_stride: isize,
    bsrc: Option<&[f64]>,
    b0: isize,
    b_stride: isize,
    len: usize,
) -> Result<f64> {
    let mut acc = [0.0f64; 16];
    let lane = |acc: &mut [f64; 16], slot: usize, v: usize, l: usize| -> Result<()> {
        let e = (4 * v + l) as isize;
        let x = rdo(d, asrc, a0 + e * a_stride)?;
        let y = rdo(d, bsrc, b0 + e * b_stride)?;
        let s = acc.get_mut(4 * slot + l).ok_or(PetirError::Invalid)?;
        *s = fma(x, y, *s);
        Ok(())
    };
    let nvec = len / 4;
    let tail = len % 4;
    let nbatched = nvec / 4 * 4;
    for v in 0..nbatched {
        for l in 0..4 {
            lane(&mut acc, v % 4, v, l)?;
        }
    }
    let rest = nvec - nbatched;
    for t in 0..rest {
        for l in 0..4 {
            lane(&mut acc, t, nbatched + t, l)?;
        }
    }
    if tail > 0 {
        for l in 0..tail {
            lane(&mut acc, rest, nvec, l)?;
        }
    }
    let [a00, a01, a02, a03, a10, a11, a12, a13, a20, a21, a22, a23, a30, a31, a32, a33] = acc;
    let (s00, s01, s02, s03) = (a00 + a10, a01 + a11, a02 + a12, a03 + a13);
    let (s20, s21, s22, s23) = (a20 + a30, a21 + a31, a22 + a32, a23 + a33);
    let (l0, l1, l2, l3) = (s00 + s20, s01 + s21, s02 + s22, s03 + s23);
    Ok((l0 + l2) + (l1 + l3))
}

/// faer `matvec_rowmajor::matvec` (sequential): `lhs` has unit column stride
/// and `rhs` unit row stride. Per row: `tmp = alpha * dot`, then
/// `dst = dst + tmp` (Add) or `dst = tmp`.
#[allow(clippy::too_many_arguments)]
fn matvec_rowmajor(
    d: &mut [f64],
    dst: View,
    beta: Accum,
    ls: Option<&[f64]>,
    lhs: View,
    rsrc: Option<&[f64]>,
    rhs: View,
    alpha: f64,
) -> Result<()> {
    for i in 0..lhs.nrows {
        let mut tmp = dot(
            d,
            ls,
            lhs.at(i, 0),
            lhs.cs,
            rsrc,
            rhs.at(0, 0),
            rhs.rs,
            lhs.ncols,
        )?;
        tmp = alpha * tmp;
        let y = wr(d, dst.at(i, 0))?;
        if beta == Accum::Add {
            tmp = *y + tmp;
        }
        *y = tmp;
    }
    Ok(())
}

/// faer `rank_update::rank_update_imp`: `dst` (unit row stride) `+= alpha *
/// col * row`, with `col` an `m x 1` view and `row` an `n x 1` view (the row
/// transposed). Per column: `r = alpha * row[j]`; Add:
/// `dst = fma(col[i], r, dst)`; Replace: `dst = col[i] * r`.
#[allow(clippy::too_many_arguments)]
fn rank_update(
    d: &mut [f64],
    dst: View,
    beta: Accum,
    csrc: Option<&[f64]>,
    col: View,
    rsrc: Option<&[f64]>,
    row: View,
    alpha: f64,
) -> Result<()> {
    for j in 0..dst.ncols {
        let r = alpha * rdo(d, rsrc, row.at(j, 0))?;
        for i in 0..dst.nrows {
            let x = rdo(d, csrc, col.at(i, 0))?;
            let y = wr(d, dst.at(i, j))?;
            *y = match beta {
                Accum::Add => fma(x, r, *y),
                Accum::Replace => x * r,
            };
        }
    }
    Ok(())
}

/// One nano-gemm microkernel accumulator over depths `k0..k1`: the
/// fixed-depth kernels (total depth <= 16) start with a `vmulpd`, the `dyn`
/// kernel with a zeroed register and `vfmadd` throughout.
#[allow(clippy::too_many_arguments)]
fn nano_acc(
    d: &[f64],
    ls: Option<&[f64]>,
    lhs: View,
    rsrc: Option<&[f64]>,
    rhs: View,
    i: usize,
    j: usize,
    k0: usize,
    k1: usize,
    fixed: bool,
) -> Result<f64> {
    let mut acc = 0.0f64;
    for kk in k0..k1 {
        let a = rdo(d, ls, lhs.at(i, kk))?;
        let b = rdo(d, rsrc, rhs.at(kk, j))?;
        acc = if fixed && kk == k0 {
            a * b
        } else {
            fma(a, b, acc)
        };
    }
    Ok(acc)
}

/// The three epilogues of nano-gemm's `RealKernel`: with nano's own
/// `alpha` (the destination scale, 0 or 1 from faer) and `beta` (faer's
/// `alpha`): `alpha == 1`: `fma(beta, acc, dst)`; `alpha == 0`:
/// `beta * acc`; else `fma(beta, acc, dst * alpha)`.
fn nano_epilogue(nalpha: f64, nbeta: f64, acc: f64, dst: f64) -> f64 {
    if nalpha == 1.0 {
        fma(nbeta, acc, dst)
    } else if nalpha == 0.0 {
        nbeta * acc
    } else {
        fma(nbeta, acc, dst * nalpha)
    }
}

/// nano-gemm 0.2.2 `execute_f64` with the AVX2 plan (faer calls it for
/// `m*n*k <= 4096` with `alpha = {Replace: 0, Add: 1}` and `beta = alpha`).
#[allow(clippy::too_many_arguments)]
fn nano_gemm(
    d: &mut [f64],
    dst: View,
    beta: Accum,
    ls: Option<&[f64]>,
    lhs: View,
    rsrc: Option<&[f64]>,
    rhs: View,
    alpha: f64,
) -> Result<()> {
    let nalpha = match beta {
        Accum::Replace => 0.0,
        Accum::Add => 1.0,
    };
    let nbeta = alpha;
    let (mut dst, mut lhs, mut rhs, mut ls, mut rsrc) = (dst, lhs, rhs, ls, rsrc);
    if dst.cs.unsigned_abs() < dst.rs.unsigned_abs() {
        // transpose the problem
        dst = dst.t();
        let (l, r) = (rhs.t(), lhs.t());
        lhs = l;
        rhs = r;
        core::mem::swap(&mut ls, &mut rsrc);
    }
    if dst.rs == -1 && dst.nrows > 0 {
        dst = dst.rev_rows();
        lhs = lhs.rev_rows();
    }
    let (m, n, k) = (dst.nrows, dst.ncols, lhs.ncols);
    let fixed = k <= 16;
    let colmajor = lhs.rs == 1 && dst.rs == 1;
    if colmajor {
        // small_direct / direct millikernel: one microkernel call per tile
        // over the whole depth.
        for j in 0..n {
            for i in 0..m {
                let acc = nano_acc(d, ls, lhs, rsrc, rhs, i, j, 0, k, fixed)?;
                let y = wr(d, dst.at(i, j))?;
                *y = nano_epilogue(nalpha, nbeta, acc, *y);
            }
        }
        return Ok(());
    }
    // copy_millikernel: depth in blocks of 64.
    let mut a = nalpha;
    let mut depth = 0usize;
    while depth < k {
        let bs = Ord::min(NANO_K_BS, k - depth);
        for j in 0..n {
            for i in 0..m {
                let acc = nano_acc(d, ls, lhs, rsrc, rhs, i, j, depth, depth + bs, fixed)?;
                let y = wr(d, dst.at(i, j))?;
                if dst.rs == 1 {
                    *y = nano_epilogue(a, nbeta, acc, *y);
                } else {
                    // microkernel into a temporary with alpha = 0, then
                    // `dst = alpha * dst + tmp` (or a copy when alpha == 0).
                    let tmp = nano_epilogue(0.0, nbeta, acc, 0.0);
                    *y = if a == 0.0 { tmp } else { a * *y + tmp };
                }
            }
        }
        a = 1.0;
        depth += bs;
    }
    Ok(())
}

/// private-gemm-x86 0.1.22 `gemm` with 256-bit AVX2 kernels: depth in blocks
/// of `kc = 512`; per block `acc = 0`, `acc = fma(lhs, rhs, acc)` per depth
/// step, `acc *= alpha`, then `dst = acc` (first block of a Replace) or
/// `dst = acc + dst`. Optional `row_idx`/`col_idx` scatter the result, and
/// `kind` restricts it to a triangle.
#[allow(clippy::too_many_arguments)]
pub(crate) fn private_gemm(
    d: &mut [f64],
    dst: View,
    row_idx: Option<&[usize]>,
    col_idx: Option<&[usize]>,
    kind: DstKind,
    beta: Accum,
    ls: Option<&[f64]>,
    lhs: View,
    rsrc: Option<&[f64]>,
    rhs: View,
    alpha: f64,
) -> Result<()> {
    let (mut nrows, mut ncols, depth) = (lhs.nrows, rhs.ncols, lhs.ncols);
    if nrows == 0 || ncols == 0 || (depth == 0 && beta == Accum::Add) {
        return Ok(());
    }
    let (mut dst, mut lhs, mut rhs, mut ls, mut rsrc) = (dst, lhs, rhs, ls, rsrc);
    let (mut row_idx, mut col_idx, mut kind) = (row_idx, col_idx, kind);
    if dst.rs.unsigned_abs() > dst.cs.unsigned_abs() {
        // The problem is transposed (index lists and triangle with it).
        dst = dst.t();
        core::mem::swap(&mut row_idx, &mut col_idx);
        kind = match kind {
            DstKind::Lower => DstKind::Upper,
            DstKind::Upper => DstKind::Lower,
            DstKind::Full => DstKind::Full,
        };
        let (l, r) = (rhs.t(), lhs.t());
        lhs = l;
        rhs = r;
        core::mem::swap(&mut ls, &mut rsrc);
        core::mem::swap(&mut nrows, &mut ncols);
    }
    if dst.rs < 0 && kind == DstKind::Full && row_idx.is_none() {
        dst = dst.rev_rows();
        lhs = lhs.rev_rows();
    }
    if lhs.cs < 0 && depth > 0 {
        lhs = lhs.rev_cols();
        rhs = rhs.rev_rows();
    }
    let place = |i: usize, j: usize| -> Result<isize> {
        let ri = match row_idx {
            Some(r) => r.get(i).copied().ok_or(PetirError::Invalid)?,
            None => i,
        };
        let cj = match col_idx {
            Some(c) => c.get(j).copied().ok_or(PetirError::Invalid)?,
            None => j,
        };
        Ok(dst.at(ri, cj))
    };
    // Every output element is an independent FMA chain over the depth (the
    // microkernel's vector lanes and register tiles never mix elements), so
    // the loops run column by column with the depth outside the rows: the
    // same chain per element, with many chains in flight at once.
    // Rows are taken in tiles so a tile of `lhs` stays in cache across the
    // columns (cache blocking, which faer's kernels also do and which does
    // not touch the per-element arithmetic).
    let mut acc: Vec<f64> = Vec::new();
    let mut r0 = 0usize;
    while r0 < nrows {
        let r1 = Ord::min(nrows, r0 + ROW_TILE);
        for j in 0..ncols {
            let (i0, i1) = match kind {
                DstKind::Full => (0, nrows),
                DstKind::Lower => (j, nrows),
                DstKind::Upper => (0, Ord::min(j + 1, nrows)),
            };
            let (i0, i1) = (Ord::max(i0, r0), Ord::min(i1, r1));
            if i0 >= i1 {
                continue;
            }
            let rows = i1 - i0;
            let mut k0 = 0usize;
            let mut first = true;
            while k0 < depth {
                let kc = Ord::min(depth - k0, PRIVATE_KC);
                acc.clear();
                acc.resize(rows, 0.0);
                {
                    let lsl: &[f64] = ls.unwrap_or(d);
                    let rsl: &[f64] = rsrc.unwrap_or(d);
                    for kk in k0..k0 + kc {
                        let b = rd(rsl, rhs.at(kk, j))?;
                        let base = lhs.at(i0, kk);
                        // bounds of this column segment, checked once
                        rd(lsl, base)?;
                        rd(lsl, base + (rows as isize - 1) * lhs.rs)?;
                        if lhs.rs == 1 {
                            let col = lsl
                                .get(base as usize..base as usize + rows)
                                .ok_or(PetirError::Invalid)?;
                            for (a, &x) in acc.iter_mut().zip(col) {
                                *a = fma(x, b, *a);
                            }
                        } else {
                            for (t, a) in acc.iter_mut().enumerate() {
                                let x = rd(lsl, base + t as isize * lhs.rs)?;
                                *a = fma(x, b, *a);
                            }
                        }
                    }
                }
                for (t, &a) in acc.iter().enumerate() {
                    let a = a * alpha;
                    let p = place(i0 + t, j)?;
                    let y = wr(d, p)?;
                    *y = if first && beta == Accum::Replace {
                        a
                    } else {
                        a + *y
                    };
                }
                first = false;
                k0 += kc;
            }
        }
        r0 = r1;
    }
    Ok(())
}

/// faer `spicy_matmul` on AVX2 (straight to private-gemm-x86), for the
/// `Rectangular` and `TriangularLower` destinations the sparse Cholesky uses.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spicy_matmul(
    d: &mut [f64],
    c: View,
    kind: DstKind,
    row_idx: Option<&[usize]>,
    col_idx: Option<&[usize]>,
    beta: Accum,
    asrc: Option<&[f64]>,
    a: View,
    bsrc: Option<&[f64]>,
    b: View,
    alpha: f64,
) -> Result<()> {
    if a.nrows == 0 || b.ncols == 0 {
        return Ok(());
    }
    if a.ncols != b.nrows {
        return Err(PetirError::Invalid);
    }
    private_gemm(d, c, row_idx, col_idx, kind, beta, asrc, a, bsrc, b, alpha)
}

/// faer `triangular::matmul` into a `TriangularLower` destination with dense
/// operands (`mat_x_mat_into_lower_impl_unchecked`'s x86 arm: private-gemm
/// with `DstKind::Lower` when every dimension is non-zero).
#[allow(clippy::too_many_arguments)]
pub(crate) fn matmul_into_lower(
    d: &mut [f64],
    dst: View,
    beta: Accum,
    ls: Option<&[f64]>,
    lhs: View,
    rsrc: Option<&[f64]>,
    rhs: View,
    alpha: f64,
) -> Result<()> {
    if dst.nrows == 0 || dst.ncols == 0 || lhs.ncols == 0 {
        // faer's non-x86 recursion would run here; with a zero dimension it
        // writes nothing an Add could change.
        if beta == Accum::Replace && lhs.ncols == 0 {
            for j in 0..dst.ncols {
                for i in j..dst.nrows {
                    *wr(d, dst.at(i, j))? = 0.0;
                }
            }
        }
        return Ok(());
    }
    private_gemm(
        d,
        dst,
        None,
        None,
        DstKind::Lower,
        beta,
        ls,
        lhs,
        rsrc,
        rhs,
        alpha,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    /// A plain triple loop to compare the dispatch against (agreement to
    /// rounding; bit-identity with faer is in `tests/sparse_vs_faer.rs`).
    fn naive(a: &[f64], b: &[f64], m: usize, n: usize, k: usize) -> Vec<f64> {
        let mut c = alloc::vec![0.0; m * n];
        for j in 0..n {
            for i in 0..m {
                let mut s = 0.0;
                for kk in 0..k {
                    s += a[i + kk * m] * b[kk + j * k];
                }
                c[i + j * m] = s;
            }
        }
        c
    }

    #[test]
    fn every_kernel_computes_a_product() {
        for &(m, n, k) in &[
            (1, 1, 1),
            (5, 1, 7),
            (1, 6, 9),
            (4, 3, 1),
            (7, 5, 3),
            (20, 20, 20),
            (3, 2, 600),
        ] {
            let a: Vec<f64> = (0..m * k)
                .map(|x| ((x * 7 % 11) as f64 - 5.0) * 0.37)
                .collect();
            let b: Vec<f64> = (0..k * n)
                .map(|x| ((x * 5 % 13) as f64 - 6.0) * 0.11)
                .collect();
            let want = naive(&a, &b, m, n, k);
            // column-major dst, lhs, rhs
            let mut c = alloc::vec![1.0; m * n];
            matmul(
                &mut c,
                View::col_major(0, m, n, m),
                Accum::Replace,
                Some(&a),
                View::col_major(0, m, k, m),
                Some(&b),
                View::col_major(0, k, n, k),
                1.0,
            )
            .unwrap();
            for (x, y) in c.iter().zip(&want) {
                assert!((x - y).abs() < 1e-12 * (1.0 + y.abs()), "{m}x{n}x{k}");
            }
            // row-major lhs (transposed storage), Add with alpha = -1
            let mut at = alloc::vec![0.0; m * k];
            for i in 0..m {
                for kk in 0..k {
                    at[kk + i * k] = a[i + kk * m];
                }
            }
            let mut c2 = want.clone();
            matmul(
                &mut c2,
                View::col_major(0, m, n, m),
                Accum::Add,
                Some(&at),
                View::col_major(0, k, m, k).t(),
                Some(&b),
                View::col_major(0, k, n, k),
                -1.0,
            )
            .unwrap();
            for x in &c2 {
                assert!(x.abs() < 1e-12, "{m}x{n}x{k} add");
            }
        }
    }

    #[test]
    fn lower_and_scatter_destinations() {
        let (m, k) = (6, 4);
        let a: Vec<f64> = (0..m * k).map(|x| x as f64 * 0.5 - 3.0).collect();
        let mut c = alloc::vec![0.0; m * m];
        let av = View::col_major(0, m, k, m);
        matmul_into_lower(
            &mut c,
            View::col_major(0, m, m, m),
            Accum::Add,
            Some(&a),
            av,
            Some(&a),
            av.t(),
            1.0,
        )
        .unwrap();
        let full = {
            let mut at = alloc::vec![0.0; k * m];
            for i in 0..m {
                for kk in 0..k {
                    at[kk + i * k] = a[i + kk * m];
                }
            }
            naive(&a, &at, m, m, k)
        };
        for j in 0..m {
            for i in 0..m {
                let want = if i >= j { full[i + j * m] } else { 0.0 };
                assert!((c[i + j * m] - want).abs() < 1e-12);
            }
        }
        let mut s = alloc::vec![0.0; 9];
        spicy_matmul(
            &mut s,
            View::col_major(0, 3, 3, 3),
            DstKind::Full,
            Some(&[2, 0]),
            Some(&[1]),
            Accum::Add,
            Some(&[1.0, 2.0]),
            View::col_major(0, 2, 1, 2),
            Some(&[3.0]),
            View::col_major(0, 1, 1, 1),
            -1.0,
        )
        .unwrap();
        // local (0,0) -> dst(2,1) = -3, local (1,0) -> dst(0,1) = -6
        assert_eq!(s, [0.0, 0.0, 0.0, -6.0, 0.0, -3.0, 0.0, 0.0, 0.0]);
    }
}
