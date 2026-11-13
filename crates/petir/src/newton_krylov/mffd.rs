// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of PETIR, a component of OUTRAM PARK.
//
// PORTED from PETSc, commit e413143d55b0811f9b914603d9694184c0392685 (main),
// read 2026-11-01 from https://github.com/petsc/petsc:
//
//   src/mat/impls/mffd/mffd.c   `MatMult_MFFD` (lines 296-370): the
//                               finite-difference product
//                               y = (F(u + h a) - F(u)) / h, and the default
//                               `error_rel = PETSC_SQRT_MACHINE_EPSILON`
//                               (line 588)
//   src/mat/impls/mffd/wp.c     `MatMFFDCompute_WP`: the Walker-Pernice
//                               differencing parameter
//                               h = error_rel * sqrt(1 + ||u||) / ||a||
//   include/petscmath.h:2029    PETSC_SQRT_MACHINE_EPSILON for double
//
// Copyright (c) 1991-2025, UChicago Argonne, LLC and the PETSc Developers and
// Contributors. All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// * Redistributions of source code must retain the above copyright notice,
//   this list of conditions and the following disclaimer.
// * Redistributions in binary form must reproduce the above copyright notice,
//   this list of conditions and the following disclaimer in the documentation
//   and/or other materials provided with the distribution.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
// ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
// LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
// CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
// SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
// CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
// POSSIBILITY OF SUCH DAMAGE.
//
// BSD-2-Clause into GPL-3.0-only: one-way. The derived file is distributed
// under GPL-3.0-only.

//! The Jacobian-free (matrix-free finite-difference) product `J(u) a`.
//!
//! # The perturbation-size rule, and where it comes from
//!
//! PETSc's default `MATMFFD` type, `MATMFFD_WP` (`wp.c`), Walker & Pernice's
//! rule (H. F. Walker and M. Pernice, "NITSOL: a Newton iterative solver for
//! nonlinear systems", SIAM J. Sci. Comput. 19 (1998) 302-318):
//!
//! ```text
//! h = error_rel * sqrt(1 + ||u||_2) / ||a||_2,   error_rel = sqrt(DBL_EPSILON)
//! J(u) a ~ (F(u + h a) - F(u)) / h
//! ```
//!
//! `error_rel` is "the square root of the relative error in the function
//! evaluation"; PETSc's default is `PETSC_SQRT_MACHINE_EPSILON =
//! 1.4901161193847656e-08`. `||u||` is computed once per base point, as PETSc
//! does by default (`computenormU = PETSC_FALSE`: `sqrt(1 + ||U||)` is formed
//! on the first product after the base is set and reused). A zero direction
//! `a` gives `y = 0` without evaluating `F` (`MatMult_MFFD`, `zeroa`).
//!
//! # Arithmetic kept from `MatMult_MFFD`
//!
//! `w = h a + U` (`VecWAXPY`), `y = F(w)`, `y = y + (-1) F(U)` (`VecAXPY`),
//! `y = y * (1/h)` (`VecScale`). Kept in that form, including the multiply by
//! the reciprocal.
//!
//! # Deviations
//!
//! - **DEVIATION — the norms.** PETSc's `VecNorm(NORM_2)` goes through the
//!   linked BLAS `dnrm2`; this uses PETIR's GSL-ported
//!   [`crate::linalg::nrm2`] (scaled sum of squares). Round-off only.
//! - Not ported: the `DS` (Brown–Saad) rule, the recompute period
//!   (`-mat_mffd_period`, default 1, so every product recomputes `h` — as
//!   here), complex-step differencing, null-space removal, `checkh`.

use alloc::vec;
use alloc::vec::Vec;
use core::cell::Cell;

#[allow(unused_imports)]
use crate::real::Real;

use crate::linalg::nrm2;
use crate::newton_krylov::gmres::LinearOperator;

/// `PETSC_SQRT_MACHINE_EPSILON` for double (`petscmath.h:2029`), PETSc's
/// default `error_rel` (`mffd.c:588`).
pub const PETSC_SQRT_MACHINE_EPSILON: f64 = 1.4901161193847656e-08;

/// The finite-difference Jacobian-vector product of `F` at a base point `u`,
/// as a [`LinearOperator`] for [`gmres`](crate::newton_krylov::gmres).
///
/// `G` is the residual closure `F(x, out)` — typically `&F`, so one closure
/// serves many base points. Construct with [`MffdOperator::new`], passing
/// `F(u)` already evaluated (PETSc's `MatMFFDSetBase(J, U, F)`).
///
/// # Example
///
/// ```
/// use petir::newton_krylov::{LinearOperator, MffdOperator, PETSC_SQRT_MACHINE_EPSILON};
/// // F(x) = (x0^2, x0 x1): J(u) = [[2 u0, 0], [u1, u0]].
/// let f = |x: &[f64], out: &mut [f64]| { out[0] = x[0] * x[0]; out[1] = x[0] * x[1]; };
/// let u = vec![3.0, 2.0];
/// let mut fu = vec![0.0; 2];
/// f(&u, &mut fu);
/// let op = MffdOperator::new(&f, u, fu, PETSC_SQRT_MACHINE_EPSILON);
/// let mut y = [0.0; 2];
/// op.apply(&[1.0, 0.0], &mut y);
/// assert!((y[0] - 6.0).abs() < 1e-6 && (y[1] - 2.0).abs() < 1e-6);
/// ```
#[derive(Debug, Clone)]
pub struct MffdOperator<G> {
    /// The residual function `F`.
    f: G,
    /// The base point `U`.
    u: Vec<f64>,
    /// `F(U)`.
    fu: Vec<f64>,
    /// `error_rel`.
    error_rel: f64,
    /// `sqrt(1 + ||U||)` (PETSc `normUfact`).
    norm_u_fact: f64,
    /// Evaluations of `F` made by [`LinearOperator::apply`] so far.
    evals: Cell<usize>,
}

impl<G> MffdOperator<G>
where
    G: Fn(&[f64], &mut [f64]),
{
    /// Set the base point `u` with `fu = F(u)` and the differencing error
    /// `error_rel` (pass [`PETSC_SQRT_MACHINE_EPSILON`] unless `F` is noisier
    /// than round-off). `fu.len()` must equal `u.len()`; the product is
    /// square.
    pub fn new(f: G, u: Vec<f64>, fu: Vec<f64>, error_rel: f64) -> Self {
        let norm_u = nrm2(&u);
        Self {
            f,
            u,
            fu,
            error_rel,
            norm_u_fact: Real::sqrt(1.0 + norm_u),
            evals: Cell::new(0),
        }
    }

    /// The differencing parameter `MatMFFDCompute_WP` gives for direction
    /// `a`, or `None` for `a = 0`.
    pub fn differencing_parameter(&self, a: &[f64]) -> Option<f64> {
        let norma = nrm2(a);
        if norma == 0.0 {
            return None;
        }
        Some(self.error_rel * self.norm_u_fact / norma)
    }

    /// How many times [`LinearOperator::apply`] has evaluated `F`.
    pub fn function_evaluations(&self) -> usize {
        self.evals.get()
    }
}

impl<G> LinearOperator for MffdOperator<G>
where
    G: Fn(&[f64], &mut [f64]),
{
    fn n_rows(&self) -> usize {
        self.u.len()
    }

    fn n_cols(&self) -> usize {
        self.u.len()
    }

    /// `MatMult_MFFD`: `y = (F(U + h a) - F(U)) / h`.
    fn apply(&self, a: &[f64], y: &mut [f64]) {
        let Some(h) = self.differencing_parameter(a) else {
            for yi in y.iter_mut() {
                *yi = 0.0;
            }
            return;
        };
        // w = u + h a
        let mut w = vec![0.0_f64; self.u.len()];
        for (wi, (&ai, &ui)) in w.iter_mut().zip(a.iter().zip(self.u.iter())) {
            *wi = h * ai + ui;
        }
        (self.f)(&w, y);
        self.evals.set(self.evals.get() + 1);
        // `VecAXPY(y, -1.0, F)`, kept as written.
        #[allow(clippy::neg_multiply)]
        for (yi, &fi) in y.iter_mut().zip(self.fu.iter()) {
            *yi += -1.0 * fi;
        }
        let rh = 1.0 / h;
        for yi in y.iter_mut() {
            *yi *= rh;
        }
    }
}
