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

//! One-dimensional root finding — GSL's `roots/`.
//!
//! # Not to be confused with [`crate::poly::roots`]
//!
//! Two things in this crate are called "roots" and they are unrelated:
//!
//! - **This module** holds the *iterative solvers* — find `x` such that
//!   `f(x) = 0`, for an arbitrary `f`.
//! - [`crate::poly::roots`] holds the *tagged container type* that OpenFOAM's
//!   closed-form linear/quadratic/cubic solvers return.
//!
//! Both names come from their respective upstreams (GSL's `roots/` and
//! OpenFOAM's `Roots`), and renaming either would break the property that a
//! port reads like its source. They are kept distinct by namespace instead.
//!
//! # Which to use
//!
//! **If `f` is a polynomial of degree 3 or less, use neither.** The closed
//! forms in [`crate::poly`] are exact and need no iteration:
//! [`crate::poly::QuadraticEqn`], [`crate::poly::CubicEqn`].
//!
//! Otherwise:
//!
//! 1. **Can you bracket the root** — find `a`, `b` with `f(a)` and `f(b)` of
//!    opposite sign? Then [`BracketingSolver`] with
//!    [`BracketingMethod::Brent`]. It cannot lose the root, and it is fast.
//!    This is the right answer most of the time.
//! 2. **Only a starting guess, plus an exact derivative?**
//!    [`PolishingSolver`]. Faster, but no guarantee: it can diverge or find a
//!    different root, silently.
//! 3. **A guess and an expensive derivative?** [`PolishingMethod::Secant`],
//!    which evaluates the true `f'` once.
//!
//! # Structure
//!
//! GSL separates *stepping* from *stopping*, and this port keeps that:
//! [`BracketingSolver::iterate`] and [`PolishingSolver::iterate`] each take one
//! step, and the [`test_interval`] / [`test_delta`] / [`test_residual`]
//! functions decide whether to continue. That matters because the right
//! stopping rule depends on what the root is for — see [`convergence`].
//!
//! `solve` methods are provided on both for the common case where the default
//! loop is fine.
//!
//! # Units
//!
//! Bare dimensionless `f64`, like the rest of PETIR's numerics.

pub mod bracketing;
pub mod convergence;
pub mod polishing;

pub use bracketing::{BracketingMethod, BracketingSolver};
pub use convergence::{test_delta, test_interval, test_residual, Convergence};
pub use polishing::{PolishingMethod, PolishingSolver};
