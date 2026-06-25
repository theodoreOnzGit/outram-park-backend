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

/// Field-level fluid thermodynamic interface (Layer 4).
///
/// Mirrors `Foam::fluidThermo` / `Foam::psiThermo` / `Foam::rhoThermo` from
/// `src/thermophysicalModels/basic/`.
///
/// Each struct owns the primary thermodynamic fields (`p`, `T`, `he`, `rho`,
/// `psi`) and uses a per-species `TransportModel` (from Layer 1h) to evaluate
/// properties cell-by-cell.
pub mod traits;
pub mod psi_thermo;
pub mod rho_thermo;
pub mod solid_thermo;

pub use traits::FluidThermo;
pub use psi_thermo::PsiThermo;
pub use rho_thermo::RhoThermo;
pub use solid_thermo::{SolidThermo, ConstSolidThermo};
