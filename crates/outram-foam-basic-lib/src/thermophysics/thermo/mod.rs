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

//! Per-species thermodynamic models — specific heat Cp [J/(kg·K)],
//! sensible/absolute specific enthalpy [J/kg], specific entropy [J/(kg·K)], and
//! Newton `T`-inversion, layered on top of an
//! [`EquationOfState`](crate::thermophysics::eos::EquationOfState).
//!
//! Each model implements [`ThermoModel`]. Available models: constant-Cp
//! [`HConstThermo`], polynomial-Cp [`HPolynomialThermo`], tabulated
//! [`HTabulatedThermo`], and NASA-7 (JANAF) [`JanafThermo`].

pub mod h_const;
pub mod h_polynomial;
pub mod h_tabulated;
pub mod janaf;
pub(crate) mod traits;

pub use h_const::*;
pub use h_polynomial::*;
pub use h_tabulated::*;
pub use janaf::*;
pub use traits::*;
