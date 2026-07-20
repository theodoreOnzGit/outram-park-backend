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

//! Concrete `EquationOfState` closures, mirroring OpenFOAM's
//! `src/thermophysicalModels/specie/equationOfState/` layer: [`perfect_gas`]
//! (ideal gas law), [`rho_const`] (constant density), [`ico_polynomial`]
//! (incompressible, specific volume as a polynomial in T), and
//! [`peng_robinson`] (cubic real-gas EOS). See `EquationOfState` (defined in
//! the crate-private `traits` submodule) for the shared interface.

/// Incompressible EOS with specific volume given by a polynomial in
/// temperature. See [`ico_polynomial::IcoPolynomial`].
pub mod ico_polynomial;
/// Peng-Robinson (1976) cubic real-gas EOS. See
/// [`peng_robinson::PengRobinsonGas`].
pub mod peng_robinson;
/// Ideal perfect-gas EOS: `p = ρ·R·T`. See [`perfect_gas::PerfectGas`].
pub mod perfect_gas;
/// Constant-density (incompressible) EOS: `ρ = ρ0`. See [`rho_const::RhoConst`].
pub mod rho_const;
pub(crate) mod traits;

pub use ico_polynomial::*;
pub use peng_robinson::*;
pub use perfect_gas::*;
pub use rho_const::*;
pub use traits::*;
