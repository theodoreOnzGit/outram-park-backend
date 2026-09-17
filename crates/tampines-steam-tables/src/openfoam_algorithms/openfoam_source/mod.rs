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

//! Pure-Rust port of OpenFOAM's primitive + finite-volume layer: tensor
//! algebra, polynomial/ODE solvers, interpolation, the LDU matrix + FV
//! operators, mesh, `fields` (see the `fields` sub-module), and thermophysics
//! kernels. All sub-modules here are crate-internal (`pub(crate)`); none of
//! this is part of `tampines-steam-tables`'s public API.

// These are ported-but-not-fully-wired-up OpenFOAM reference primitives (see
// this directory's CLAUDE.md, "Numerical primitives already in-tree") -- most
// of their surface has no caller yet, so they generate a large amount of
// dead_code/unused_imports noise. Silenced here (op-21g.8) rather than
// per-item so the warning count doesn't obscure real issues in the rest of
// the crate; op-21g.8 tracks revisiting this once each primitive either gets
// a real consumer or is confirmed genuinely unused and deleted.
#[allow(dead_code, unused_imports)]
pub(crate) mod fields;
#[allow(dead_code, unused_imports)]
pub(crate) mod fluid_thermo;
#[allow(dead_code, unused_imports)]
pub(crate) mod fv_operators;
#[allow(dead_code, unused_imports)]
pub(crate) mod interpolation;
#[allow(dead_code, unused_imports)]
pub(crate) mod ldu_matrix;
#[allow(dead_code, unused_imports)]
pub(crate) mod math;
#[allow(dead_code, unused_imports)]
pub(crate) mod matrix;
#[allow(dead_code, unused_imports)]
pub(crate) mod mesh;
#[allow(dead_code, unused_imports)]
pub(crate) mod ode;
#[allow(dead_code, unused_imports)]
pub(crate) mod polynomial;
#[allow(dead_code, unused_imports)]
pub(crate) mod primitives;
#[allow(dead_code, unused_imports)]
pub(crate) mod thermophysics;

#[allow(unused_imports)]
pub(crate) use fields::*;
#[allow(unused_imports)]
pub(crate) use fluid_thermo::*;
#[allow(unused_imports)]
pub(crate) use fv_operators::*;
#[allow(unused_imports)]
pub(crate) use interpolation::*;
#[allow(unused_imports)]
pub(crate) use ldu_matrix::*;
#[allow(unused_imports)]
pub(crate) use math::*;
#[allow(unused_imports)]
pub(crate) use matrix::*;
#[allow(unused_imports)]
pub(crate) use mesh::*;
#[allow(unused_imports)]
pub(crate) use ode::*;
#[allow(unused_imports)]
pub(crate) use polynomial::*;
#[allow(unused_imports)]
pub(crate) use primitives::*;
#[allow(unused_imports)]
pub(crate) use thermophysics::*;

/// this part is extension in Rust
/// Now under here, I want to expose the openfoam primitives to something
/// that can be human readable
///
/// Also useful add-ons for the underlying libraries are put here,
/// eg. generating one dimensional meshes for system code type simulations
/// in TAMPINES
#[allow(dead_code, unused_imports)]
pub(crate) mod interface;
#[allow(unused_imports)]
pub(crate) use interface::*;
