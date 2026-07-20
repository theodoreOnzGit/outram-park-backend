// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
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

//! Generic dimensionless polynomial evaluation and closed-form root finders.
//!
//! [`Polynomial`] evaluates/differentiates/integrates a fixed-degree
//! coefficient polynomial (optionally with an added `log(x)` term).
//! [`LinearEqn`], [`QuadraticEqn`], and [`CubicEqn`] find the exact roots of
//! degree-1/2/3 equations in closed form (Cardano's method for the cubic),
//! tagging each root [`Real`](RootType::Real), [`Complex`](RootType::Complex),
//! `PosInf`/`NegInf`, or `Nan` via [`Roots`]. All quantities here are bare
//! `f64` — no `uom` units — since these are pure numerical-algebra building
//! blocks used by higher-level EOS/property closures that attach units at
//! their own call sites.

/// Closed-form real roots of a cubic equation (dimensionless `f64`).
pub mod cubic_eqn;
/// Closed-form root of a linear equation (dimensionless `f64`).
pub mod linear_eqn;
/// General polynomial representation and evaluation (dimensionless `f64`).
pub mod polynomial;
/// Closed-form real roots of a quadratic equation (dimensionless `f64`).
pub mod quadratic_eqn;
/// Shared real-root utilities for the polynomial solvers (dimensionless `f64`).
pub mod roots;

pub use cubic_eqn::CubicEqn;
pub use linear_eqn::LinearEqn;
pub use polynomial::Polynomial;
pub use quadratic_eqn::QuadraticEqn;
pub use roots::{RootType, Roots};
