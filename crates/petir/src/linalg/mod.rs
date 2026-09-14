// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of PETIR, a component of OUTRAM PARK.
//
// PETIR is free software: you can redistribute it and/or modify it under the
// terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 of the License.
//
// PETIR is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along
// with PETIR.  If not, see <https://www.gnu.org/licenses/>.

//! Dense linear algebra — GSL's `blas/` and `linalg/` layers.
//!
//! # Contents
//!
//! - [`SquareMatrix`] — the row-major dense `n x n` matrix with Crout LU and
//!   back-substitution, lifted verbatim from `outram-foam-basic-lib`.
//! - [`blas1`] — the level-1 BLAS vector kernels GSL wraps
//!   (`ddot`, `dnrm2`, `dasum`, `daxpy`, `dscal`, `idamax`, `dswap`).
//! - [`lu`] — determinant, log-determinant and explicit inverse built on the LU
//!   factors, translating `gsl_linalg_LU_det` / `_lndet` / `_invert`.
//!
//! # No BLAS library is linked, and none may be
//!
//! Every kernel here is pure Rust. That is not a performance oversight, it is
//! the crate's contract: PETIR must build for `aarch64-linux-android`, for
//! `wasm32-unknown-unknown` and for bare-metal `thumbv7em-none-eabihf`, none of
//! which has a system BLAS to link, and the workspace rule forbids pulling a C
//! or Fortran toolchain in. Where a caller on a desktop target wants a tuned
//! BLAS, the right move is to use `ndarray-linalg` from a `std` crate — not to
//! add a backend here.
//!
//! # Scope
//!
//! `n x n` dense and real. Not covered: complex matrices, the rectangular
//! least-squares path (QR, SVD), eigenvalue problems, and banded or sparse
//! storage. Those are tracked as follow-on work rather than stubbed, because a
//! half-implemented eigensolver is worse than an absent one.

pub mod blas1;
pub mod lu;
/// Row-major dense `n x n` matrix with Crout LU, lifted verbatim from
/// outram-foam-basic-lib. `MatrixError::Singular`'s `col` field is the
/// zero-based column at which the pivot vanished; the lint is suppressed here
/// rather than in the lifted file, for the reason given in `crate::poly`.
#[allow(missing_docs)]
pub mod square_matrix;

pub use blas1::{asum, axpy, dot, iamax, nrm2, scal, swap};
pub use lu::{det, inverse, ln_det};
pub use square_matrix::{MatrixError, SquareMatrix};
