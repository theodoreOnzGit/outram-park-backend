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

//! **This is OUTRAM PARK's independent Rust translation of selected
//! OpenFOAM® turbulence-model algorithms — it is not the official
//! OpenFOAM® software and is not affiliated with, endorsed by, or
//! sanctioned by OpenCFD Ltd. or the ESI Group.** OpenFOAM® is a registered
//! trademark of OpenCFD Limited. See `TRADEMARKS.md` (this crate's
//! directory, mirrored from the workspace root) for the full attribution
//! and non-affiliation notice.
//!
//! # Overview
//!
//! Pure-Rust translation of the OpenFOAM turbulence-closure library: RAS
//! (Reynolds-Averaged Simulation) and LES (Large-Eddy Simulation) models that
//! supply the turbulent-stress and effective-viscosity terms a momentum solver
//! needs. Every model implements the [`traits::TurbulenceModel`] trait; dispatch
//! is static (generics), never `dyn`.
//!
//! # Implementation status (read before depending on a model)
//!
//! Only **k-ω SST is implemented and unit-tested**. The other closures are
//! scaffolds — the struct and its coefficients exist, but their trait methods
//! `todo!()`-panic if called. Constructing them is safe; driving them is not.
//!
//! | Module | Model | Status |
//! |---|---|---|
//! | [`k_omega_sst`] | Menter (1994) k-ω SST | Implemented + unit-tested |
//! | [`laminar`] | No-op laminar | Partial — `div_dev_rho_reff` is `todo!()` |
//! | [`k_epsilon`] | Jones & Launder (1972) k-ε | Scaffold — trait methods `todo!()` |
//! | [`k_omega`] | Wilcox (1988) k-ω | Scaffold — trait methods `todo!()` |
//! | [`spalart_allmaras`] | Spalart-Allmaras (1992) | Scaffold — trait methods `todo!()` |
//! | [`les`] | Smagorinsky (1963) LES | Scaffold — trait methods `todo!()` |
//!
//! [`wall_functions`] provides standalone log-law helpers (`y_plus`, `u_tau`,
//! `nu_t_wall`); they are not yet wired into any model as boundary conditions.
//! See `README.md` ("Limitations") for the full scope/validation caveats.

pub mod error;
pub mod k_epsilon;
pub mod k_omega;
pub mod k_omega_sst;
pub mod laminar;
pub mod les;
pub mod prelude;
pub mod spalart_allmaras;
pub mod traits;
pub mod wall_functions;
