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

//! Specie-level thermophysics: mesh-independent per-species property kernels.
//!
//! Ports the OpenFOAM `thermophysicalModels/specie` layer. Properties are built
//! in three stacked layers, each wrapping the one below:
//! - [`eos`](crate::thermophysics::eos) — equation of state: density ρ, compressibility ψ, compressibility
//!   factor Z, and enthalpy/entropy/internal-energy departures from `(p, T)`.
//! - [`thermo`](crate::thermophysics::thermo) — specific heat Cp, enthalpy, entropy, and Newton `T`-inversion.
//! - [`transport`](crate::thermophysics::transport) — dynamic viscosity μ and thermal conductivity κ.
//!
//! Supporting modules: [`constants`](crate::thermophysics::constants) (physical
//! constants), [`error`](crate::thermophysics::error) (the
//! [`ThermoError`](crate::thermophysics::error::ThermoError) type),
//! [`quantities`](crate::thermophysics::quantities) (uom type aliases), and
//! [`imports`](crate::thermophysics::imports) (shared uom re-exports used by
//! every implementation file).

pub mod constants;
pub mod eos;
pub mod error;
pub mod imports;
pub mod quantities;
pub mod thermo;
pub mod transport;
