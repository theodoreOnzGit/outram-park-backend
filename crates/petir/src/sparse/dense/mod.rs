// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// PORTED from four crates by sarah quiñones, all read from the published
// crates.io sources on 2026-10-20:
//
//     faer 0.24.4           https://codeberg.org/sarah-quinones/faer
//     nano-gemm 0.2.2 (+ nano-gemm-codegen 0.2.1)
//                           https://github.com/sarah-ek/nano-gemm
//     private-gemm-x86 0.1.22
//                           https://github.com/sarah-quinones/gemm-x64-v2
//     pulp 0.22.3           https://github.com/sarah-quinones/pulp
//
//     Copyright (c) sarah quiñones <sarah@veganb.tw> and the faer contributors;
//     private-gemm-x86: Copyright (c) 2025 sarah quiñones el kazdadi.
//     All four are licensed MIT (each crate's Cargo.toml `license = "MIT"`;
//     private-gemm-x86 and pulp ship the MIT text, reproduced here).
//
// Their notice, retained as that licence requires of a redistribution in
// source form:
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
// MIT into AGPL-3.0-only is a ONE-WAY flow. Maintainer direction for this
// port: gh #994, 2026-10-20 ("Port lah"; "Whatever faer does, port into
// petir"). See the crate NOTICE.

//! **Dense kernels behind the supernodal factorisations** — faer 0.24.4's
//! `f64` matrix product, triangular solves, partial-pivoting LU and `LLᵀ`, as
//! faer runs them on an x86-64 CPU with AVX2 and FMA, reproduced in portable
//! scalar code.
//!
//! # What "reproduced" means here
//!
//! faer's dense kernels are SIMD: `pulp` (AVX2) for the matrix-vector and
//! rank-1 kernels and the small dense Cholesky, `nano-gemm`'s generated
//! microkernels for products with `m·n·k ≤ 4096`, and `private-gemm-x86`'s
//! generated assembly above that. None of that is used here. Instead each
//! kernel's **arithmetic** is read off its source and replayed one scalar at a
//! time, in the same order, with the same fused multiply-adds:
//!
//! - every vector lane in those kernels is an independent output element, so
//!   a lane is replayed as a scalar;
//! - where lanes ARE reduced (faer's dot product: four 4-lane accumulators,
//!   then `pulp`'s `reduce_sum_f64s`), the 16 partial sums and the reduction
//!   order are replayed exactly ([`matmul`]);
//! - every `vfmadd` becomes [`crate::real::Real::mul_add`], PETIR's own
//!   correctly rounded FMA (gh #1001 routes it), and every separate
//!   `vmul`/`vadd` stays a separate rounding;
//! - the depth blocking that changes a sum (private-gemm-x86's `kc = 512`,
//!   nano-gemm's 64-deep copy blocks) is reproduced; register tiling and
//!   cache blocking, which do not, are not.
//!
//! The dispatch between those kernels is faer's own, decided by shapes and
//! strides only, so it is reproduced from the strides of [`View`]. It does
//! **not** depend on the CPU: the numbers are those faer gives on an AVX2+FMA
//! machine without AVX-512, on every target PETIR builds for. On a CPU where
//! faer itself takes another path (no AVX2, or aarch64), faer and PETIR differ
//! there; PETIR does not.
//!
//! Every subscript is a checked access ([`rd`], [`wr`]); an index out of range
//! is [`crate::PetirError::Invalid`], never a panic.

pub(crate) mod factor;
pub(crate) mod matmul;
pub(crate) mod trsm;
#[cfg(test)]
mod vs_faer;

use crate::{PetirError, Result};

/// faer's `Accum`: overwrite the destination, or add to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Accum {
    /// `dst = alpha * lhs * rhs`
    Replace,
    /// `dst = dst + alpha * lhs * rhs`
    Add,
}

/// A strided dense matrix view into a flat `f64` buffer: element `(i, j)` is
/// at `off + i*rs + j*cs`. faer's `MatRef`/`MatMut` with the pointer replaced
/// by an offset, so the views can be reversed and transposed exactly as faer
/// does (negative strides included).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct View {
    pub off: isize,
    pub nrows: usize,
    pub ncols: usize,
    pub rs: isize,
    pub cs: isize,
}

impl View {
    /// A column-major `nrows x ncols` block at `off` with leading dimension
    /// `ld` (faer's `from_column_major_slice` uses `ld = nrows`).
    pub fn col_major(off: usize, nrows: usize, ncols: usize, ld: usize) -> Self {
        Self {
            off: off as isize,
            nrows,
            ncols,
            rs: 1,
            cs: ld as isize,
        }
    }

    /// Buffer offset of `(i, j)`.
    #[inline(always)]
    pub fn at(&self, i: usize, j: usize) -> isize {
        self.off + (i as isize) * self.rs + (j as isize) * self.cs
    }

    /// `transpose`.
    pub fn t(self) -> Self {
        Self {
            off: self.off,
            nrows: self.ncols,
            ncols: self.nrows,
            rs: self.cs,
            cs: self.rs,
        }
    }

    /// `submatrix(i, j, nrows, ncols)`.
    pub fn sub(self, i: usize, j: usize, nrows: usize, ncols: usize) -> Self {
        Self {
            off: self.at(i, j),
            nrows,
            ncols,
            rs: self.rs,
            cs: self.cs,
        }
    }

    /// `subrows(i, n)`.
    pub fn rows(self, i: usize, n: usize) -> Self {
        self.sub(i, 0, n, self.ncols)
    }

    /// `subcols(j, n)`.
    pub fn cols(self, j: usize, n: usize) -> Self {
        self.sub(0, j, self.nrows, n)
    }

    /// `reverse_rows`.
    pub fn rev_rows(self) -> Self {
        if self.nrows == 0 {
            return Self {
                rs: -self.rs,
                ..self
            };
        }
        Self {
            off: self.at(self.nrows - 1, 0),
            rs: -self.rs,
            ..self
        }
    }

    /// `reverse_cols`.
    pub fn rev_cols(self) -> Self {
        if self.ncols == 0 {
            return Self {
                cs: -self.cs,
                ..self
            };
        }
        Self {
            off: self.at(0, self.ncols - 1),
            cs: -self.cs,
            ..self
        }
    }

    /// `reverse_rows_and_cols`.
    pub fn rev_both(self) -> Self {
        self.rev_rows().rev_cols()
    }

    /// `col(j).as_mat()`: an `nrows x 1` view whose column stride is 0, as
    /// faer's `ColRef::as_mat` sets it.
    pub fn col_as_mat(self, j: usize) -> Self {
        Self {
            off: self.at(0, j),
            nrows: self.nrows,
            ncols: 1,
            rs: self.rs,
            cs: 0,
        }
    }

    /// `row(i).as_mat()`: a `1 x ncols` view whose row stride is 0.
    pub fn row_as_mat(self, i: usize) -> Self {
        Self {
            off: self.at(i, 0),
            nrows: 1,
            ncols: self.ncols,
            rs: 0,
            cs: self.cs,
        }
    }
}

/// Read `s[k]` (checked).
#[inline(always)]
pub(crate) fn rd(s: &[f64], k: isize) -> Result<f64> {
    usize::try_from(k)
        .ok()
        .and_then(|k| s.get(k).copied())
        .ok_or(PetirError::Invalid)
}

/// `&mut s[k]` (checked).
#[inline(always)]
pub(crate) fn wr(s: &mut [f64], k: isize) -> Result<&mut f64> {
    usize::try_from(k)
        .ok()
        .and_then(|k| s.get_mut(k))
        .ok_or(PetirError::Invalid)
}

/// Read element `k` of an operand that lives either in its own buffer
/// (`Some`) or in the destination buffer `d` (`None`, faer's case of two views
/// into one matrix).
#[inline(always)]
pub(crate) fn rdo(d: &[f64], src: Option<&[f64]>, k: isize) -> Result<f64> {
    rd(src.unwrap_or(d), k)
}

/// faer's `f64` FMA lane: `a * b + c` rounded once, through PETIR's FMA.
#[inline(always)]
pub(crate) fn fma(a: f64, b: f64, c: f64) -> f64 {
    crate::real::Real::mul_add(a, b, c)
}
