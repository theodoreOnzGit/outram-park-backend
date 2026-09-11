// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! The seam between Farrer Park's matrix and the shared Krylov layer.
//!
//! # What belongs in this module
//!
//! The implementations that make [`crate::sparse::CsrMatrix`] and
//! [`crate::sparse::CsrPreconditioner`] usable by
//! `outram-foam-basic-lib`'s generic solvers, and re-exports of the contract
//! itself so a Farrer Park user need not name the other crate.
//!
//! # What does NOT belong here
//!
//! Solver *policy* — which method, which tolerance, what to do when it fails.
//! That is [`crate::solver`].
//!
//! # The whole point of the crate boundary, in one place
//!
//! `outram-foam-basic-lib` owns the Krylov methods and the finite-volume
//! `LduMatrix`. Farrer Park owns the finite-element `CsrMatrix`. They meet at
//! [`LinearOperator`] and nowhere else: no finite-volume type appears in a
//! Farrer Park signature, no finite-element type appears in a finite-volume
//! one, and neither discretisation is bent to fit the other's storage. This is
//! GitHub issue #175 in three `impl` blocks.
//!
//! # Generic bounds, not trait objects
//!
//! [`LinearOperator`] is used exclusively as a generic bound. There is no
//! `Box<dyn LinearOperator>` anywhere, so every call site monomorphises onto
//! the concrete matrix and the sparse matrix-vector product inlines.
//!
//! # Units
//!
//! Dimensionless `f64`, as the shared contract requires.

use outram_foam_basic_lib::linear_operator::{LinearOperator, OperatorPreconditioner};

use crate::sparse::{CsrMatrix, CsrPreconditioner};

pub use outram_foam_basic_lib::krylov::{KrylovResult, KrylovSettings};
pub use outram_foam_basic_lib::linear_operator::{
    bicgstab_op, cg_op, gmres_op, IdentityPreconditioner, JacobiOperatorPreconditioner,
    LinearOperator as FemLinearOperator, OperatorPreconditioner as FemPreconditioner,
};

impl LinearOperator for CsrMatrix {
    fn n_rows(&self) -> usize {
        CsrMatrix::n_rows(self)
    }

    fn n_cols(&self) -> usize {
        CsrMatrix::n_cols(self)
    }

    /// Compressed-sparse-row multiply, delegating to [`CsrMatrix::spmv`] so
    /// there is exactly one implementation of the kernel.
    fn apply(&self, x: &[f64], y: &mut [f64]) {
        self.spmv(x, y);
    }

    fn diagonal(&self, d: &mut [f64]) {
        self.diagonal_into(d);
    }
}

impl OperatorPreconditioner for CsrPreconditioner {
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        self.apply_to(r, z);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dof::SparsityPattern;

    fn dense_pattern(n: usize) -> SparsityPattern {
        let mut row_ptr = vec![0];
        let mut col_idx = Vec::new();
        for _ in 0..n {
            for j in 0..n {
                col_idx.push(j);
            }
            row_ptr.push(col_idx.len());
        }
        SparsityPattern { row_ptr, col_idx }
    }

    /// A symmetric positive-definite CSR matrix must solve through the shared
    /// conjugate-gradient driver — the end-to-end proof that a finite-element
    /// matrix reaches the finite-volume crate's Krylov layer untouched.
    #[test]
    fn csr_solves_through_the_shared_krylov_layer() {
        let n = 12;
        let p = dense_pattern(n);
        let mut a = CsrMatrix::from_pattern(&p);
        for i in 0..n {
            for j in 0..n {
                let v = if i == j {
                    4.0 + i as f64 * 0.1
                } else {
                    0.5 / (1.0 + (i as f64 - j as f64).abs())
                };
                a.add(i, j, v).unwrap();
            }
        }
        let x_true: Vec<f64> = (0..n).map(|i| ((i % 5) as f64) - 2.0).collect();
        let mut b = vec![0.0; n];
        a.spmv(&x_true, &mut b);

        let settings = KrylovSettings {
            tolerance: 1e-13,
            max_iter: 500,
            restart: 30,
        };
        for m in [
            CsrPreconditioner::Identity,
            CsrPreconditioner::jacobi(&a),
            CsrPreconditioner::ilu0(&a),
        ] {
            let (x, r) = cg_op(&a, &b, None, &m, &settings);
            assert!(r.converged, "cg_op did not converge: {:?}", r);
            for i in 0..n {
                assert!((x[i] - x_true[i]).abs() < 1e-8, "{} vs {}", x[i], x_true[i]);
            }
        }
    }

    /// The non-symmetric drivers must reach the same answer, so a future
    /// non-symmetric tangent (contact, non-associated flow) has a path.
    #[test]
    fn csr_solves_through_gmres_and_bicgstab() {
        let n = 10;
        let p = dense_pattern(n);
        let mut a = CsrMatrix::from_pattern(&p);
        for i in 0..n {
            for j in 0..n {
                let v = if i == j { 6.0 } else { 0.4 / (1.0 + j as f64) };
                a.add(i, j, v).unwrap();
            }
        }
        let x_true: Vec<f64> = (0..n).map(|i| 0.3 * i as f64 - 1.0).collect();
        let mut b = vec![0.0; n];
        a.spmv(&x_true, &mut b);
        let s = KrylovSettings {
            tolerance: 1e-12,
            max_iter: 400,
            restart: 30,
        };
        let m = CsrPreconditioner::ilu0(&a);
        for (x, r) in [
            gmres_op(&a, &b, None, &m, &s),
            bicgstab_op(&a, &b, None, &m, &s),
        ] {
            assert!(r.converged, "{:?}", r);
            for i in 0..n {
                assert!((x[i] - x_true[i]).abs() < 1e-8);
            }
        }
    }
}
