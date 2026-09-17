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

//! Polynomials: evaluation, and closed-form and iterative root finding.
//!
//! # What is here
//!
//! Two layers, from two different upstreams, deliberately kept side by side:
//!
//! - **Closed-form root finders**, lifted verbatim from
//!   `outram-foam-basic-lib`'s port of OpenFOAM's `polynomialEqns`:
//!   [`LinearEqn`], [`QuadraticEqn`] and [`CubicEqn`], each returning a tagged
//!   [`Roots`] container that distinguishes real, complex, infinite and NaN
//!   roots rather than collapsing them. These are exact to within round-off and
//!   involve no iteration, so they are what you want for degree <= 3.
//! - **General-degree evaluation**, translating GSL's `poly/`: [`eval`] (Horner),
//!   [`eval_derivs`] (the value and the first `k` derivatives in one sweep), and
//!   the divided-difference Newton form [`DividedDifference`].
//!
//! [`Polynomial<N>`] is the fixed-degree value/derivative/integral type from
//! OpenFOAM, carried over unchanged.
//!
//! # What is *not* here
//!
//! General-degree **complex** root finding (GSL's `gsl_poly_complex_solve`,
//! which balances a companion matrix and runs a Francis QR iteration on it) is
//! not implemented. It needs the unsymmetric eigenvalue machinery that PETIR's
//! linear-algebra layer does not yet carry, and half-porting an eigensolver is
//! exactly the failure mode the workspace CLAUDE.md warns about. Tracked in
//! beads; until then, degree <= 3 is served exactly by the closed forms above
//! and higher degrees must be handled by the caller.
//!
//! # Units
//!
//! Every coefficient and root here is a bare dimensionless `f64`. This is not
//! an oversight: the coefficient of `x^k` in a dimensioned polynomial carries
//! units of `[y]/[x]^k`, which differ term by term, so no single `uom` quantity
//! can type a coefficient vector. Forcing one would misstate the physics. The
//! same reasoning — and the same conclusion — is recorded in
//! `chem-eng-real-time-process-control-simulator`'s `z_domain` module.

// VERBATIM-LIFT LINT CARVE-OUT.
//
// The five modules marked below are byte-for-byte lifts from
// outram-foam-basic-lib (see each file's PROVENANCE block). Upstream documents
// its types but not every public struct field or enum variant, and adding those
// doc comments here would destroy the one property that makes a lift
// worth having -- that it still diffs clean against its source.
//
// So the lint is suppressed at the declaration site, never in the lifted file,
// and what those fields mean is written here instead. The real fix is to
// document them UPSTREAM and re-lift, which keeps both crates correct at once;
// that is filed as a bead against outram-foam-basic-lib.

/// Closed-form cubic root finder, `Foam::cubicEqn`. Its public fields `a`, `b`,
/// `c`, `d` are the coefficients of `a x^3 + b x^2 + c x + d`, in descending
/// powers, all dimensionless.
#[allow(missing_docs)]
pub mod cubic_eqn;
pub mod eval;
/// Closed-form linear root finder, `Foam::linearEqn`. Its public fields `a` and
/// `b` are the coefficients of `a x + b`, dimensionless.
#[allow(missing_docs)]
pub mod linear_eqn;
/// Fixed-degree polynomial value / derivative / integral type, `Foam::Polynomial`.
#[allow(missing_docs)]
pub mod polynomial;
/// Closed-form quadratic root finder, `Foam::quadraticEqn`. Its public fields
/// `a`, `b`, `c` are the coefficients of `a x^2 + b x + c`, dimensionless.
#[allow(missing_docs)]
pub mod quadratic_eqn;
/// Closed-form quartic (and biquadratic) root finder, ported from the `roots`
/// crate. Unlike the `*_eqn` types above -- which come from OpenFOAM via
/// `outram-foam-basic-lib` -- this module is a derivative of Mikhail
/// Vorotilov's BSD-2-Clause `roots` crate, and returns a [`quartic::RootSet`]
/// of the distinct real roots only. See the module header for the notice.
pub mod quartic;
/// All roots of a polynomial of **arbitrary degree**, as the eigenvalues of
/// its companion matrix — ported from the `roots` crate, by way of JAMA and
/// EISPACK. The one routine here that is iterative rather than closed-form,
/// the one that goes past the quartic, and the only one that returns complex
/// roots.
pub mod companion;
/// A heap-allocated polynomial with **algebra** — multiplication, long
/// division, translation — and the classical orthogonal families built from
/// it (Legendre, Chebyshev, Hermite, Bessel). Ported from the `peroxide`
/// crate. This is the only module here that can multiply two polynomials.
pub mod dense;
/// Tagged root container, `Foam::Roots`. [`RootType`] distinguishes `Real`,
/// `Complex`, `PosInf`, `NegInf` and `Nan` roots so that a caller can tell a
/// genuine root from a degenerate one instead of inspecting the value.
#[allow(missing_docs)]
pub mod roots;

pub use cubic_eqn::CubicEqn;
pub use eval::{eval, eval_derivs, DividedDifference};
pub use linear_eqn::LinearEqn;
pub use polynomial::Polynomial;
pub use quadratic_eqn::QuadraticEqn;
pub use roots::{RootType, Roots};
pub use dense::{bessel, chebyshev, hermite, legendre, ChebyshevKind, DensePoly};
pub use companion::{
    real_roots_companion, roots_companion, roots_companion_monic_ascending, ComplexRoot,
};
pub use quartic::{
    roots_biquadratic, roots_cubic, roots_cubic_normalized, roots_linear, roots_quadratic,
    roots_quartic, roots_quartic_depressed, RootSet,
};
