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

//! # `solvers` — Layer-5 OpenFOAM solver-application ports
//!
//! Each submodule is a self-contained Rust port of one OpenFOAM solver
//! application: it owns its time-advancement loop, assembles and solves the
//! governing equations each step, and enforces boundary conditions through
//! [`outram_foam_basic_lib`]'s `FvMesh`/`FvPatch` (never re-implementing BC
//! logic — see `bc_util`).
//!
//! | Submodule | Ports | Regime |
//! |---|---|---|
//! | [`pimple_foam`] | pimpleFoam | Incompressible transient PIMPLE |
//! | [`melt_foam`] | pimpleFoam + `solidificationMelting` fvModel | Incompressible buoyant PIMPLE with phase change |
//! | [`rho_pimple_foam`] | rhoPimpleFoam | Compressible transient PIMPLE |
//! | [`sonic_foam`] | sonicFoam | Transonic/supersonic compressible |
//! | [`rho_central_foam`] | rhoCentralFoam | Density-based central-upwind (Kurganov-Tadmor) |
//! | [`hrm_foam`] | HRMFoam | Homogeneous relaxation two-phase flashing flow |
//! | [`reacting_two_phase_euler_foam`] | reactingTwoPhaseEulerFoam | Two-fluid Euler-Euler with reacting mass/heat transfer |
//!
//! `bc_util` is a crate-internal helper for capturing and re-applying patch
//! boundary conditions around a field solve.

pub(crate) mod bc_util;
pub mod hrm_foam;
pub mod melt_foam;
pub mod pimple_foam;
pub mod reacting_two_phase_euler_foam;
pub mod rho_central_foam;
pub mod rho_pimple_foam;
pub mod sonic_foam;
