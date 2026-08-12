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

//! Vendored pure-Rust OpenFOAM finite-volume layer (Layers 1–4): tensor-algebra
//! primitives, polynomial and ODE solvers, `xy` interpolation, thermophysics
//! kernels, cell/surface fields, the FV mesh, the LDU sparse matrices and their
//! linear solvers, and the explicit/implicit FV operators (`fvc`/`fvm`) — plus
//! the field-level fluid-thermo interface. Everything here is `pub(crate)`; it
//! is the mathematical machinery the `rhoPimpleFoam` solver runs on, not a
//! public API. `uom`-only (no `ndarray`/BLAS) so the crate stays
//! Android-buildable. Ported from OpenFOAM `src/`; see per-module docs for the
//! upstream `Foam::` correspondence.

pub(crate) mod primitives;
pub(crate) mod polynomial;
pub(crate) mod math;
pub(crate) mod matrix;
pub(crate) mod ode;
pub(crate) mod interpolation;
pub(crate) mod thermophysics;
pub(crate) mod fields;
pub(crate) mod mesh;
pub(crate) mod ldu_matrix;
pub(crate) mod fv_operators;
pub(crate) mod fluid_thermo;

pub(crate) use primitives::*;
pub(crate) use polynomial::*;
pub(crate) use math::*;
pub(crate) use matrix::*;
pub(crate) use ode::*;
pub(crate) use interpolation::*;
pub(crate) use thermophysics::*;
pub(crate) use fields::*;
pub(crate) use mesh::*;
pub(crate) use ldu_matrix::*;
pub(crate) use fv_operators::*;
pub(crate) use fluid_thermo::*;

/// this part is extension in Rust
/// Now under here, I want to expose the openfoam primitives to something
/// that can be human readable
///
/// Also useful add-ons for the underlying libraries are put here,
/// eg. generating one dimensional meshes for system code type simulations
/// in TAMPINES
pub(crate) mod interface;
pub(crate) use interface::*;
