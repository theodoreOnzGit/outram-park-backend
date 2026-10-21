// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// PORTED from the `faer` crate, version 0.24.4, read from the published
// crates.io source (faer-0.24.4 in the cargo registry) on 2026-10-20.
//
//     https://codeberg.org/sarah-quinones/faer
//     Copyright (c) sarah quiñones <sarah@veganb.tw> and the faer contributors
//     Licensed MIT (faer's Cargo.toml `license = "MIT"`; the published crate
//     ships no LICENSE file, so the standard MIT text is reproduced here).
//
// Its notice, retained as that licence requires of a redistribution in source
// form:
//
//   Permission is hereby granted, free of charge, to any person obtaining a
//   copy of this software and associated documentation files (the
//   "Software"), to deal in the Software without restriction, including
//   without limitation the rights to use, copy, modify, merge, publish,
//   distribute, sublicense, and/or sell copies of the Software, and to permit
//   persons to whom the Software is furnished to do so, subject to the
//   following conditions:
//
//   The above copyright notice and this permission notice shall be included in
//   all copies or substantial portions of the Software.
//
//   THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//   IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//   FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
//   THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//   LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
//   FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
//   DEALINGS IN THE SOFTWARE.
//
// MIT into AGPL-3.0-only is a ONE-WAY flow: this file may not be contributed
// back to `faer` under its original licence without its authors' agreement.
// Maintainer direction for this port: gh #994, 2026-10-20 ("if faer functions
// are needed, export no_std versions of them through petir"). See the crate
// NOTICE and CLAUDE.md.
//
// Ported items (faer 0.24.4, src/sparse/csc/mod.rs):
//   SparseColMat::try_new_from_triplets
//     (try_new_from_indices_impl, new_from_argsort_impl) -> CscMatrix::try_from_triplets

//! **Sparse direct solvers** — a `no_std` port of `faer`'s sparse LU (with
//! COLAMD column ordering) and sparse Cholesky `LLᵀ` (with AMD ordering),
//! simplicial and supernodal.
//!
//! # Why this comes from `faer` and not from GSL
//!
//! GSL has no sparse direct factorisation (its `spmatrix`/`splinalg` offer
//! storage and GMRES only). Two workspace crates called `faer`'s `sp_lu` and
//! `sp_cholesky`: `bedok` (a port of MATLAB's `decomposition`, whose nodal
//! eigenvalue moved 14.33 pcm on a last-ulp change, see
//! `docs/why-petir-last-ulp-evidence.md`) and `outram-blender` (Laplacian
//! smoothing, ARAP and harmonic parameterisation, whose API reports a non-SPD
//! system as an error). `outram-foam-basic-lib`'s solvers were checked first
//! and fit neither: they are iterative (they would move `bedok`'s eigenvalue at
//! the solver-tolerance level, against a MATLAB reference that solves
//! directly), and its PCG cannot report an indefinite matrix. **Maintainer
//! direction, gh #994, 2026-10-20:** "if faer functions are needed, export
//! no_std versions of them through petir."
//!
//! # What is ported
//!
//! | here | faer 0.24.4 |
//! |---|---|
//! | [`CscMatrix::try_from_triplets`] | `SparseColMat::try_new_from_triplets` |
//! | [`colamd`] | `sparse/linalg/colamd.rs` (itself a port of SuiteSparse COLAMD) |
//! | [`amd`] | `sparse/linalg/amd.rs` (itself a port of SuiteSparse AMD) |
//! | [`etree`] | `qr::ghost_col_etree`, `cholesky::ghost_postorder`, `cholesky::simplicial::{ghost_prefactorize_symbolic_cholesky, ereach}` |
//! | [`triangular`] | `sparse/linalg/triangular_solve.rs`, the single-column arms |
//! | [`SparseLu`] | `sp_lu`: `factorize_symbolic_lu` (COLAMD, `AᵀA` column counts, the path rule), the simplicial and the **supernodal** numeric LU and their solves |
//! | [`SparseLlt`] | `sp_cholesky`: `factorize_symbolic_cholesky` (AMD, the path rule), the simplicial and the **supernodal** numeric `LLᵀ` and their solves |
//! | `supernodal` (private) | `qr::ghost_column_counts_aat`, `cholesky::supernodal::ghost_factorize_supernodal_symbolic` (fundamental supernodes, `DEFAULT_RELAX` amalgamation) |
//! | `dense` (private) | the dense kernels the supernodal paths call: `linalg::matmul` (and through it `nano-gemm` 0.2.2 and `private-gemm-x86` 0.1.22), `linalg::triangular_solve`, `linalg::lu::partial_pivoting`, `linalg::cholesky::llt` |
//!
//! **Path choice** follows faer: [`SymbolicKind::Auto`] takes the
//! supernodal path when the factor's flops per nonzero exceed 40 (faer's
//! `SupernodalThreshold::AUTO` with `{LU,CHOLESKY}_SUPERNODAL_RATIO_FACTOR =
//! 40`), and [`SparseLu::factorize_with`] / [`SparseLlt::factorize_lower_with`]
//! force either path.
//!
//! ~~**DEVIATION — simplicial only.** `faer` chooses between a *simplicial*
//! and a *supernodal* numeric factorisation from a flops-per-nonzero ratio
//! (above 40 it picks supernodal). The supernodal kernels run faer's SIMD
//! dense GEMM, whose summation order cannot be reproduced without porting
//! `gemm` as well. This port always runs the simplicial path.~~
//! **CORRECTED 2026-10-20 (gh #994, "Port lah"):** the supernodal paths are
//! ported, and faer's SIMD dense kernels with them, their arithmetic replayed
//! in scalar code (see `dense`'s module docs). **Every solution is
//! bit-identical to faer's, on both forced paths and on faer's own path
//! choice**, on all thirteen matrices of `tests/sparse_vs_faer.rs`
//! (including the 3-D two-group operator, n = 27 648, and the SPD case that
//! used to differ by 1.4e-15), and the dense kernels are bit-identical to
//! faer's compiled ones on 1 377 products, 528 triangular solves, 8 LU and
//! 11 `LLᵀ` factorisations (`dense/vs_faer.rs`). The pin holds against faer
//! on an x86-64 CPU with AVX2+FMA and without AVX-512, which is the CPU the
//! replay reproduces; PETIR's own numbers do not depend on the CPU.
//!
//! **PERFORMANCE — the supernodal path is SLOWER here, not faster.** faer's
//! supernodal factorisation wins by doing ~8 fused multiply-adds per cycle in
//! SIMD registers. PETIR has no SIMD and no hardware FMA instruction it can
//! reach without `unsafe` or `std` (`#![forbid(unsafe_code)]`,
//! `core::f64::mul_add` is unstable), so every FMA is a call to
//! `libm::fma` (correctly rounded; about 1.2-1.4 ns each, measured), and the
//! supernodal path's extra (dense, zero-padded) flops cost more than the
//! simplicial path's scattered ones. Measured 2026-10-20, i9-13900K, one
//! thread: `bedok`'s n = 10 404 flux operator, 2.13e9 FMAs in the supernodal
//! GEMMs: simplicial 0.66-0.76 s, supernodal 7.5-8.0 s, faer (supernodal,
//! AVX2) 0.17 s; the 3-D test operator n = 27 648: simplicial 20.5 s,
//! supernodal 69.5 s, faer 1.4 s. So [`SymbolicKind::Auto`] reproduces
//! faer's *numbers*, not its speed; a caller that wants speed over
//! bit-identity with faer forces [`SymbolicKind::Simplicial`].
//!
//! **DEVIATION — errors instead of panics.** Every subscript of faer's source
//! is a checked access here (PETIR's `tests/no_panic_gate.rs`): an index out of
//! range, which faer would panic on and a well-formed input cannot produce, is
//! [`PetirError::Invalid`]. faer's `LuError::SymbolicSingular` is
//! [`PetirError::Singular`], its `LltError::NonPositivePivot` is
//! [`PetirError::Domain`] (GSL's code for a Cholesky of a matrix that is not
//! positive definite), and a non-square input is [`PetirError::NotSquare`].
//!
//! **DEVIATION — duplicate triplets.** `faer` sorts each column with an
//! unstable sort, so the order in which *duplicate* `(row, col)` entries are
//! summed is an implementation detail of `core`'s sort. This port sorts stably
//! and sums duplicates in input order. With no duplicates (both consumers)
//! the two are identical.
//!
//! Everything is `f64`, `usize` indices and `alloc` only: no threads, no SIMD,
//! no `std`. faer's own `Par::Seq` is what is reproduced (its multithreaded
//! kernels split some sums differently).

pub mod amd;
pub mod colamd;
pub mod etree;
pub mod triangular;

mod cholesky;
mod cholesky_supernodal;
mod dense;
#[cfg(feature = "faer")]
mod faer_backend;
mod lu;
mod lu_supernodal;
mod supernodal;

pub use cholesky::SparseLlt;
pub use lu::SparseLu;

use crate::{PetirError, Result};
use alloc::vec;
use alloc::vec::Vec;
use core::ops::Range;

/// Which numeric factorisation to run (faer's `SupernodalThreshold`:
/// `AUTO`, `FORCE_SIMPLICIAL`, `FORCE_SUPERNODAL`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SymbolicKind {
    /// faer's own rule: supernodal when the factor's flops per nonzero exceed
    /// 40 (`SupernodalThreshold::AUTO`).
    #[default]
    Auto,
    /// Always the simplicial (column-by-column) path.
    Simplicial,
    /// Always the supernodal (dense-block) path.
    Supernodal,
}

/// Which implementation factorises: PETIR's own port (the default, `no_std`,
/// deterministic on every CPU) or, with the opt-in `faer` feature, the real
/// `faer` crate (faster where faer has SIMD kernels for the build; results
/// depend on the CPU and the build, see the crate `CLAUDE.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SparseBackend {
    /// PETIR's port of faer 0.24.4 (this module), with [`SymbolicKind`]'s
    /// path choice.
    #[default]
    Ported,
    /// faer 0.24.4 itself (`sp_lu` / `sp_cholesky(Side::Lower)`, faer's own
    /// path choice). Feature `faer` only. Caveats: `no_std` + `alloc`;
    /// **without PETIR's `std` feature faer cannot detect the CPU at run
    /// time** and runs its baseline kernels unless built with target features
    /// (`-C target-cpu=native`, `-C target-feature=+avx2,+fma`); and **its
    /// results depend on the CPU and instruction set**, so they are not
    /// bit-identical across machines — [`SparseBackend::Ported`] is the
    /// deterministic choice. Measured on `bedok`'s 3-D transient (one
    /// thread): faer + `std` 116 s, faer without `std` 265 s (205 s with
    /// `target-cpu=native`), `Ported` 2103 s.
    #[cfg(feature = "faer")]
    Faer,
}

/// faer's `NONE` sentinel (`usize::MAX`); as a signed index it is `-1`.
pub(crate) const NONE: usize = usize::MAX;

// ---------------------------------------------------------------------------
// Checked access. PETIR's library code may not subscript (tests/
// no_panic_gate.rs): every `a[i]` of faer's source is written `g(a, i)?`
// (read), `*m(a, i)? = v` (write) or `sl(a, r)?` (sub-slice) here. An index
// out of range, which faer would panic on and a correct input cannot produce,
// is reported as `PetirError::Invalid` instead. The arithmetic is untouched,
// which is what keeps the port bit-identical.
// ---------------------------------------------------------------------------

/// Read `s[i]`.
#[inline(always)]
pub(crate) fn g<T: Copy>(s: &[T], i: usize) -> Result<T> {
    s.get(i).copied().ok_or(PetirError::Invalid)
}

/// `&mut s[i]`.
#[inline(always)]
pub(crate) fn m<T>(s: &mut [T], i: usize) -> Result<&mut T> {
    s.get_mut(i).ok_or(PetirError::Invalid)
}

/// `&s[r]`.
#[inline(always)]
pub(crate) fn sl<T>(s: &[T], r: Range<usize>) -> Result<&[T]> {
    s.get(r).ok_or(PetirError::Invalid)
}

/// `&mut s[r]`.
#[inline(always)]
pub(crate) fn slm<T>(s: &mut [T], r: Range<usize>) -> Result<&mut [T]> {
    s.get_mut(r).ok_or(PetirError::Invalid)
}

/// faer's `assert!`, as an error rather than a panic.
#[inline(always)]
pub(crate) fn ensure(cond: bool) -> Result<()> {
    if cond {
        Ok(())
    } else {
        Err(PetirError::Invalid)
    }
}

/// A compressed-sparse-column matrix of `f64`: column `j` holds the entries
/// `row_idx[col_ptr[j]..col_ptr[j + 1]]`, with values at the same positions in
/// `val`.
#[derive(Debug, Clone, PartialEq)]
pub struct CscMatrix {
    nrows: usize,
    ncols: usize,
    col_ptr: Vec<usize>,
    row_idx: Vec<usize>,
    val: Vec<f64>,
}

/// A borrowed pattern-only CSC view, the shape the orderings and the
/// elimination-tree routines take.
#[derive(Debug, Clone, Copy)]
pub struct CscPattern<'a> {
    /// number of rows
    pub nrows: usize,
    /// number of columns
    pub ncols: usize,
    /// column pointers, `ncols + 1` long
    pub col_ptr: &'a [usize],
    /// row indices
    pub row_idx: &'a [usize],
}

impl CscPattern<'_> {
    /// The row indices stored in column `j`.
    ///
    /// # Errors
    /// [`PetirError::Invalid`] if `j` or the column pointers are out of range.
    pub fn col(&self, j: usize) -> Result<&[usize]> {
        sl(self.row_idx, g(self.col_ptr, j)?..g(self.col_ptr, j + 1)?)
    }

    /// Total stored entries (faer's `compute_nnz`).
    ///
    /// # Errors
    /// [`PetirError::Invalid`] if the column pointers are malformed.
    pub fn nnz(&self) -> Result<usize> {
        let end = g(self.col_ptr, self.ncols)?;
        let start = g(self.col_ptr, 0)?;
        end.checked_sub(start).ok_or(PetirError::Invalid)
    }
}

impl CscMatrix {
    /// Build from `(row, col, value)` triplets, as faer's
    /// `SparseColMat::try_new_from_triplets` does: rows sorted within each
    /// column, duplicates summed, explicit zeros **kept**.
    ///
    /// # Errors
    /// [`PetirError::Invalid`] if any index is outside `nrows x ncols`.
    pub fn try_from_triplets(
        nrows: usize,
        ncols: usize,
        triplets: &[(usize, usize, f64)],
    ) -> Result<Self> {
        for &(row, col, _) in triplets {
            if row >= nrows || col >= ncols {
                return Err(PetirError::Invalid);
            }
        }
        // Counting sort by column, stable: input order within a column.
        let mut col_ptr = vec![0usize; ncols + 1];
        for &(_, col, _) in triplets {
            *m(&mut col_ptr, col + 1)? += 1;
        }
        for j in 0..ncols {
            let prev = g(&col_ptr, j)?;
            *m(&mut col_ptr, j + 1)? += prev;
        }
        let mut next = col_ptr.clone();
        let mut order = vec![0usize; triplets.len()];
        for (k, &(_, col, _)) in triplets.iter().enumerate() {
            let at = g(&next, col)?;
            *m(&mut order, at)? = k;
            *m(&mut next, col)? += 1;
        }
        // Within each column: stable sort by row, then merge duplicates.
        let mut out_ptr = vec![0usize; ncols + 1];
        let mut row_idx = Vec::with_capacity(triplets.len());
        let mut val: Vec<f64> = Vec::with_capacity(triplets.len());
        for j in 0..ncols {
            let seg = slm(&mut order, g(&col_ptr, j)?..g(&col_ptr, j + 1)?)?;
            seg.sort_by_key(|&k| triplets.get(k).map_or(0, |t| t.0));
            let mut prev = NONE;
            for &k in seg.iter() {
                let (row, _, v) = g(triplets, k)?;
                if row == prev {
                    if let Some(last) = val.last_mut() {
                        *last += v;
                    }
                } else {
                    row_idx.push(row);
                    val.push(v);
                    prev = row;
                }
            }
            *m(&mut out_ptr, j + 1)? = row_idx.len();
        }
        Ok(Self {
            nrows,
            ncols,
            col_ptr: out_ptr,
            row_idx,
            val,
        })
    }

    /// Number of rows.
    pub fn nrows(&self) -> usize {
        self.nrows
    }

    /// Number of columns.
    pub fn ncols(&self) -> usize {
        self.ncols
    }

    /// Number of stored entries.
    pub fn nnz(&self) -> usize {
        self.row_idx.len()
    }

    /// Column pointers (`ncols + 1` long).
    pub fn col_ptr(&self) -> &[usize] {
        &self.col_ptr
    }

    /// Row indices of every stored entry, column by column.
    pub fn row_idx(&self) -> &[usize] {
        &self.row_idx
    }

    /// Values of every stored entry, column by column.
    pub fn val(&self) -> &[f64] {
        &self.val
    }

    fn col_range(&self, j: usize) -> Result<Range<usize>> {
        Ok(g(&self.col_ptr, j)?..g(&self.col_ptr, j + 1)?)
    }

    /// The row indices stored in column `j`.
    ///
    /// # Errors
    /// [`PetirError::Invalid`] if `j >= ncols`.
    pub fn row_idx_of_col(&self, j: usize) -> Result<&[usize]> {
        sl(&self.row_idx, self.col_range(j)?)
    }

    /// The values stored in column `j`.
    ///
    /// # Errors
    /// [`PetirError::Invalid`] if `j >= ncols`.
    pub fn val_of_col(&self, j: usize) -> Result<&[f64]> {
        sl(&self.val, self.col_range(j)?)
    }

    /// The pattern of this matrix.
    pub fn pattern(&self) -> CscPattern<'_> {
        CscPattern {
            nrows: self.nrows,
            ncols: self.ncols,
            col_ptr: &self.col_ptr,
            row_idx: &self.row_idx,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triplets_sort_rows_sum_duplicates_and_keep_zeros() {
        let a = CscMatrix::try_from_triplets(
            3,
            2,
            &[(2, 0, 1.0), (0, 0, 2.0), (2, 0, 0.5), (1, 1, 0.0)],
        )
        .unwrap();
        assert_eq!(a.col_ptr(), &[0, 2, 3]);
        assert_eq!(a.row_idx(), &[0, 2, 1]);
        assert_eq!(a.val(), &[2.0, 1.5, 0.0]);
        assert_eq!(
            CscMatrix::try_from_triplets(2, 2, &[(2, 0, 1.0)]),
            Err(PetirError::Invalid)
        );
    }
}
