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

//! Specie-level thermophysics layer ported from OpenFOAM's
//! `src/thermophysicalModels/specie/` primitives: equations of state,
//! per-species thermodynamic (Cp/enthalpy/entropy) closures, and per-species
//! transport (viscosity/thermal conductivity) closures, all built on `uom`
//! dimensioned quantities.

/// Physical/reference constants shared across EOS, thermo, and transport
/// closures (universal gas constant, standard-state T/p, Newton-iteration
/// bounds).
pub mod constants;
/// Equation-of-state closures — density ρ, compressibility ψ, and
/// enthalpy/entropy/Cp departure terms as functions of pressure and
/// temperature (perfect gas, constant density, incompressible polynomial,
/// Peng-Robinson real gas).
pub mod eos;
pub(crate) mod error;
/// Common `uom` quantity/unit re-exports shared by every EOS/thermo/transport
/// implementation file in this layer.
pub mod imports;
/// `uom` type aliases for compound quantities used by this layer that are
/// not already provided by `uom::si::f64` (e.g. compressibility ψ).
pub mod quantities;
/// Per-species thermodynamic (Cp / enthalpy / entropy) closures layered on
/// top of an `EquationOfState`.
pub mod thermo;
/// Per-species transport (dynamic viscosity / thermal conductivity) closures.
pub mod transport;

pub use constants::*;
pub use eos::*;
pub use error::*;
pub use imports::*;
pub use quantities::*;
pub use thermo::*;
pub use transport::*;
