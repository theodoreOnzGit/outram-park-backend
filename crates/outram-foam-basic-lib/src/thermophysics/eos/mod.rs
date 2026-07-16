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

//! Per-species equations of state — `(p, T)` → density ρ [kg/m³],
//! compressibility ψ = ∂ρ/∂p|_T [s²/m²], compressibility factor Z [-], and the
//! enthalpy / entropy / internal-energy departures from the ideal-gas value.
//!
//! Each model implements [`EquationOfState`]. Available models: ideal
//! [`PerfectGas`], constant-density [`RhoConst`], incompressible specific-volume
//! polynomial [`IcoPolynomial`], and real-gas [`PengRobinsonGas`].

pub mod ico_polynomial;
pub mod peng_robinson;
pub mod perfect_gas;
pub mod rho_const;
pub(crate) mod traits;

pub use ico_polynomial::*;
pub use peng_robinson::*;
pub use perfect_gas::*;
pub use rho_const::*;
pub use traits::*;
