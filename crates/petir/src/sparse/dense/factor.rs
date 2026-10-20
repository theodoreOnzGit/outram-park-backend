// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// PORTED from faer 0.24.4 (MIT, sarah quiñones and the faer contributors),
// read from the published crates.io source on 2026-10-20; the full MIT notice
// is in `super`'s header (`dense/mod.rs`) and applies to this file:
//
//   src/linalg/lu/partial_pivoting/factor.rs
//     lu_in_place_unblocked, lu_in_place_recursion, PartialPivLuParams::auto
//     (recursion_threshold = 16)                     -> lu_in_place_unblocked,
//                                                       lu_in_place_recursion
//   src/linalg/cholesky/llt/factor.rs: cholesky_in_place (LltParams::auto =
//     LdltParams::auto: recursion_threshold 64, block_size 128; no dynamic
//     regularisation by default)                      -> llt_in_place
//   src/linalg/cholesky/ldlt/factor.rs
//     cholesky_block_left_looking (its `if true` arm), cholesky_recursion_
//     right_looking, simd_cholesky / simd_cholesky_matrix /
//     simd_cholesky_row_batch on AVX2 (4 lanes, block 4 * 4 = 16 rows)
//                                                      -> llt_recursion, llt_simd
// Subscripts are checked accesses (see `super`).

//! Dense in-place factorisations on a [`View`]: faer's partial-pivoting LU
//! (recursive, blocked at 16) and `LLᵀ` (right-looking, 64/128, with faer's
//! 16-row-band SIMD kernel below 64 columns).

#[allow(unused_imports)]
use crate::real::Real;

use super::matmul::{matmul, matmul_into_lower};
use super::trsm;
use super::{fma, rd, wr, Accum, View};
use crate::{PetirError, Result};

/// faer's `PartialPivLuParams::auto().recursion_threshold`.
const LU_RECURSION_THRESHOLD: usize = 16;
/// faer's `LdltParams::auto()` (`LltParams` copies it).
const LLT_RECURSION_THRESHOLD: usize = 64;
const LLT_BLOCK_SIZE: usize = 128;
/// `4 * lanes` with AVX2's 4 `f64` lanes.
const LLT_SIMD_BLOCK: usize = 16;

fn swap_rows(d: &mut [f64], a: View, r0: usize, r1: usize) -> Result<()> {
    for j in 0..a.ncols {
        let p = a.at(r0, j);
        let q = a.at(r1, j);
        let x = rd(d, p)?;
        let y = rd(d, q)?;
        *wr(d, p)? = y;
        *wr(d, q)? = x;
    }
    Ok(())
}

/// faer `lu_in_place_unblocked`: returns the number of row swaps.
fn lu_in_place_unblocked(
    d: &mut [f64],
    a: View,
    start: usize,
    end: usize,
    trans: &mut [usize],
) -> Result<usize> {
    let m = a.nrows;
    if start == end {
        return Ok(0);
    }
    let mut n_trans = 0usize;
    for j in start..end {
        let col = j;
        let row = j - start;
        let mut imax = row;
        let mut max = 0.0f64;
        // faer's `for i in imax..m`: the range is fixed at entry, as clippy
        // notes; kept verbatim.
        #[allow(clippy::mut_range_bound)]
        for i in imax..m {
            let abs = rd(d, a.at(i, col))?.abs();
            if abs > max {
                max = abs;
                imax = i;
            }
        }
        *trans.get_mut(row).ok_or(PetirError::Invalid)? = imax - row;
        if imax != row {
            swap_rows(d, a, row, imax)?;
            n_trans += 1;
        }
        let mat = a.cols(start, end - start);
        let inv = 1.0 / rd(d, mat.at(row, row))?;
        for i in row + 1..m {
            let x = wr(d, mat.at(i, row))?;
            *x = *x * inv;
        }
        let ncols = mat.ncols;
        let a01 = mat
            .sub(0, row + 1, row + 1, ncols - (row + 1))
            .row_as_mat(row);
        let a10 = mat.sub(row + 1, 0, m - (row + 1), row + 1).col_as_mat(row);
        let a11 = mat.sub(row + 1, row + 1, m - (row + 1), ncols - (row + 1));
        matmul(d, a11, Accum::Add, None, a10, None, a01, -1.0)?;
    }
    Ok(n_trans)
}

/// faer `lu_in_place_recursion` (sequential): factorises columns
/// `start..end` of `a` with partial pivoting, recording relative row swaps in
/// `trans` (`trans[k]` swaps rows `k` and `k + trans[k]`).
pub(crate) fn lu_in_place_recursion(
    d: &mut [f64],
    a: View,
    start: usize,
    end: usize,
    trans: &mut [usize],
) -> Result<usize> {
    let m = a.nrows;
    let ncols = a.ncols;
    let n = end - start;
    if n <= LU_RECURSION_THRESHOLD {
        return lu_in_place_unblocked(d, a, start, end, trans);
    }
    let half = n / 2;
    let pow = Ord::min(16, half.next_power_of_two());
    let block_size = half.next_multiple_of(pow);
    if n > m {
        return Err(PetirError::Invalid);
    }
    let mut n_trans = 0usize;
    let ab = a.cols(start, n);
    n_trans += lu_in_place_recursion(
        d,
        ab,
        0,
        block_size,
        trans.get_mut(..block_size).ok_or(PetirError::Invalid)?,
    )?;
    {
        let a00 = ab.sub(0, 0, block_size, block_size);
        let a01 = ab.sub(0, block_size, block_size, n - block_size);
        let a10 = ab.sub(block_size, 0, m - block_size, block_size);
        let a11 = ab.sub(block_size, block_size, m - block_size, n - block_size);
        trsm::unit_lower(d, None, a00, a01)?;
        matmul(d, a11, Accum::Add, None, a10, None, a01, -1.0)?;
        n_trans += lu_in_place_recursion(
            d,
            ab.rows(block_size, m - block_size),
            block_size,
            n,
            trans.get_mut(block_size..n).ok_or(PetirError::Invalid)?,
        )?;
    }
    let t = trans.get(..n).ok_or(PetirError::Invalid)?;
    for part in [a.cols(0, start), a.cols(end, ncols - end)] {
        for j in 0..part.ncols {
            for (jj, &tj) in t.iter().enumerate() {
                let p = part.at(jj, j);
                let q = part.at(tj + jj, j);
                let x = rd(d, p)?;
                let y = rd(d, q)?;
                *wr(d, p)? = y;
                *wr(d, q)? = x;
            }
        }
    }
    Ok(n_trans)
}

/// faer `simd_cholesky_row_batch` (`is_llt`, no regularisation): rows
/// `start..head` of every column `0..head` of the `head x head` block.
fn llt_row_batch(d: &mut [f64], a: View, diag: &mut [f64], start: usize) -> Result<()> {
    let head = a.ncols;
    for j in 0..head {
        for i in start..head {
            let mut aij = rd(d, a.at(i, j))?;
            for k in 0..j {
                let ajk = rd(d, a.at(j, k))? * -1.0;
                let aik = rd(d, a.at(i, k))?;
                aij = fma(ajk, aik, aij);
            }
            *wr(d, a.at(i, j))? = aij;
        }
        if j >= start {
            let mut dg = rd(d, a.at(j, j))?;
            if !(dg > 0.0) {
                *diag.get_mut(j).ok_or(PetirError::Invalid)? = dg;
                return Err(PetirError::Domain);
            }
            dg = Real::sqrt(dg);
            *diag.get_mut(j).ok_or(PetirError::Invalid)? = dg;
            if dg == 0.0 || !dg.is_finite() {
                return Err(PetirError::Domain);
            }
        }
        let dg = diag.get(j).copied().ok_or(PetirError::Invalid)?;
        let inv = 1.0 / dg;
        for i in start..head {
            let x = wr(d, a.at(i, j))?;
            *x = *x * inv;
        }
    }
    Ok(())
}

/// faer `simd_cholesky_matrix`: 16-row bands.
fn llt_simd(d: &mut [f64], a: View, diag: &mut [f64]) -> Result<()> {
    let n = a.ncols;
    let mut j = 0usize;
    while j < n {
        let bs = Ord::min(LLT_SIMD_BLOCK, n - j);
        let next = j + bs;
        llt_row_batch(
            d,
            a.sub(0, 0, next, next),
            diag.get_mut(..next).ok_or(PetirError::Invalid)?,
            j,
        )?;
        j += bs;
    }
    Ok(())
}

/// faer `cholesky_recursion_right_looking` (`is_llt`).
fn llt_recursion(d: &mut [f64], a: View, diag: &mut [f64]) -> Result<()> {
    let n = a.ncols;
    if n <= LLT_RECURSION_THRESHOLD {
        if a.rs != 1 {
            // faer's `cholesky_fallback`; never reached from the sparse
            // factorisations, whose blocks are column-major.
            return Err(PetirError::Invalid);
        }
        return llt_simd(d, a, diag);
    }
    let bs0 = Ord::min(n.next_power_of_two() / 2, LLT_BLOCK_SIZE);
    let mut j = 0usize;
    while j < n {
        let bs = Ord::min(bs0, n - j);
        let rest = n - j - bs;
        let s = a.sub(j, j, n - j, n - j);
        let a00 = s.sub(0, 0, bs, bs);
        let a10 = s.sub(bs, 0, rest, bs);
        let a11 = s.sub(bs, bs, rest, rest);
        llt_recursion(d, a00, diag.get_mut(j..j + bs).ok_or(PetirError::Invalid)?)?;
        trsm::lower(d, None, a00, a10.t())?;
        matmul_into_lower(d, a11, Accum::Add, None, a10, None, a10.t(), -1.0)?;
        j += bs;
    }
    Ok(())
}

/// faer `llt::factor::cholesky_in_place` with default parameters: the lower
/// triangle of the square column-major block `a` is overwritten by `L`.
///
/// # Errors
/// [`PetirError::Domain`] on a pivot that is not strictly positive (faer's
/// `LltError::NonPositivePivot`).
pub(crate) fn llt_in_place(d: &mut [f64], a: View) -> Result<()> {
    if a.nrows != a.ncols {
        return Err(PetirError::Invalid);
    }
    let mut diag = alloc::vec![0.0f64; a.nrows];
    llt_recursion(d, a, &mut diag)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn partial_pivoting_lu_reconstructs_the_matrix() {
        for &(m, n) in &[(1usize, 1usize), (6, 4), (20, 20), (45, 37)] {
            let a: Vec<f64> = (0..m * n)
                .map(|x| ((x * 7919 % 101) as f64 - 50.0) / 25.0)
                .collect();
            let mut lu = a.clone();
            let mut trans = alloc::vec![0usize; n];
            lu_in_place_recursion(&mut lu, View::col_major(0, m, n, m), 0, n, &mut trans).unwrap();
            // P A = L U
            let mut pa = a.clone();
            for (k, &t) in trans.iter().enumerate() {
                for j in 0..n {
                    pa.swap(k + j * m, k + t + j * m);
                }
            }
            for j in 0..n {
                for i in 0..m {
                    let mut s = 0.0;
                    for k in 0..=Ord::min(i, j) {
                        let l = if i == k { 1.0 } else { lu[i + k * m] };
                        s += l * lu[k + j * m];
                    }
                    assert!((s - pa[i + j * m]).abs() < 1e-10, "{m}x{n} ({i},{j})");
                }
            }
        }
    }

    #[test]
    fn llt_reconstructs_spd_matrices_and_rejects_indefinite_ones() {
        for &n in &[1usize, 5, 16, 17, 70, 150] {
            let mut a = alloc::vec![0.0; n * n];
            for j in 0..n {
                for i in 0..n {
                    let v = ((i * 31 + j * 17) % 13) as f64 * 0.01
                        + ((j * 31 + i * 17) % 13) as f64 * 0.01;
                    a[i + j * n] = if i == j { n as f64 } else { v };
                }
            }
            let mut l = a.clone();
            llt_in_place(&mut l, View::col_major(0, n, n, n)).unwrap();
            for j in 0..n {
                for i in j..n {
                    let mut s = 0.0;
                    for k in 0..=j {
                        s += l[i + k * n] * l[j + k * n];
                    }
                    assert!((s - a[i + j * n]).abs() < 1e-10 * n as f64, "n={n}");
                }
            }
        }
        let mut b = [1.0, 2.0, 2.0, 1.0];
        assert_eq!(
            llt_in_place(&mut b, View::col_major(0, 2, 2, 2)),
            Err(PetirError::Domain)
        );
    }
}
