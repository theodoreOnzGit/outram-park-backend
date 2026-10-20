// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// DELEGATED, not ported: with the opt-in `faer` feature this file calls the
// real `faer` 0.24.4 crate (MIT, sarah quiñones and the faer contributors)
// for the sparse LU and Cholesky. Maintainer direction, gh #994,
// 2026-10-20: "bring faer into petir under a feature, then wire it into
// bedok", corrected the same day to keep PETIR `no_std` + `alloc` with the
// feature on (faer with `default-features = false`, `sparse-linalg` only).

//! The **`faer` backend** of [`super::SparseLu`] / [`super::SparseLlt`]
//! (feature `faer`, off by default): faer's own `sp_lu` and
//! `sp_cholesky(Side::Lower)`, simplicial or supernodal by faer's rule.
//!
//! Unlike the ported path, its results depend on how faer was built and on
//! the CPU: faer picks its dense kernels by instruction set, and without its
//! `std` feature (as here) it can only see the target features the build
//! enables at compile time. See the crate `CLAUDE.md` ("Dependencies").

use super::CscMatrix;
use crate::{PetirError, Result};
use faer::linalg::solvers::SolveCore;
use faer::sparse::{SparseColMatRef, SymbolicSparseColMatRef};
use faer::{Conj, MatMut, Side};

/// faer's factorisations, held by value. (`Box` is forbidden by the
/// workspace rules, so the size difference clippy flags is accepted: one
/// factor per `SparseLu`/`SparseLlt`.)
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub(crate) enum FaerFactor {
    /// `sp_lu`
    Lu(faer::sparse::linalg::solvers::Lu<usize, f64>),
    /// `sp_cholesky(Side::Lower)`
    Llt(faer::sparse::linalg::solvers::Llt<usize, f64>),
}

fn view(a: &CscMatrix) -> Result<SparseColMatRef<'_, usize, f64>> {
    let sym =
        SymbolicSparseColMatRef::new_checked(a.nrows(), a.ncols(), a.col_ptr(), None, a.row_idx());
    if a.val().len() != a.row_idx().len() {
        return Err(PetirError::Invalid);
    }
    Ok(SparseColMatRef::new(sym, a.val()))
}

/// faer `SparseColMatRef::sp_lu`.
pub(crate) fn lu(a: &CscMatrix) -> Result<FaerFactor> {
    use faer::sparse::linalg::LuError;
    match view(a)?.sp_lu() {
        Ok(f) => Ok(FaerFactor::Lu(f)),
        Err(LuError::SymbolicSingular { index }) => Err(PetirError::Singular { col: index }),
        Err(LuError::Generic(_)) => Err(PetirError::Invalid),
    }
}

/// faer `SparseColMatRef::sp_cholesky(Side::Lower)`.
pub(crate) fn llt(a: &CscMatrix) -> Result<FaerFactor> {
    use faer::sparse::linalg::LltError;
    match view(a)?.sp_cholesky(Side::Lower) {
        Ok(f) => Ok(FaerFactor::Llt(f)),
        Err(LltError::Numeric(_)) => Err(PetirError::Domain),
        Err(LltError::Generic(_)) => Err(PetirError::Invalid),
    }
}

impl FaerFactor {
    /// Solve in place for one right-hand side.
    pub(crate) fn solve_in_place(&self, rhs: &mut [f64]) -> Result<()> {
        let n = rhs.len();
        let x = MatMut::from_column_major_slice_mut(rhs, n, 1);
        match self {
            Self::Lu(f) => f.solve_in_place_with_conj(Conj::No, x),
            Self::Llt(f) => f.solve_in_place_with_conj(Conj::No, x),
        }
        Ok(())
    }
}
