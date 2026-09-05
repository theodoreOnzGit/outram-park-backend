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

//! Common `uom` re-exports for thermophysics implementation files.
//!
//! Every EOS / thermo / transport source file starts with
//! `use crate::openfoam_algorithms::openfoam_source::imports::*;` (the names are
//! re-exported up to `openfoam_source` by `thermophysics/mod.rs`) instead of
//! repeating the full type/unit import block.
//!
//! **This module is crate-internal.** `openfoam_algorithms::openfoam_source` is
//! `pub(crate)`, so none of the names below are reachable from outside
//! `tampines-steam-tables`.  They are plain `uom` re-exports carrying no local
//! behaviour, so an external caller imports the same types straight from `uom`:
//!
//! ```rust
//! use uom::si::f64::Pressure;
//! use uom::si::pressure::pascal;
//!
//! let p = Pressure::new::<pascal>(101325.0);
//! assert!(p.get::<pascal>() > 0.0);
//! ```
//!
//! The one exception is [`Compressibility`], which is a crate-local type.

// ── quantity types ────────────────────────────────────────────────────────────
pub use uom::si::f64::{
    AvailableEnergy, DynamicViscosity, MassDensity, MolarMass, Pressure, Ratio,
    SpecificHeatCapacity, ThermalConductivity, ThermodynamicTemperature,
};

// ── unit markers (used in ::new::<unit>() and .get::<unit>()) ────────────────
pub use uom::si::available_energy::joule_per_kilogram;
pub use uom::si::dynamic_viscosity::pascal_second;
pub use uom::si::mass_density::kilogram_per_cubic_meter;
pub use uom::si::molar_mass::{gram_per_mole, kilogram_per_mole};
pub use uom::si::pressure::pascal;
pub use uom::si::ratio::ratio;
pub use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
pub use uom::si::thermal_conductivity::watt_per_meter_kelvin;
pub use uom::si::thermodynamic_temperature::kelvin;

// ── crate-local ───────────────────────────────────────────────────────────────
pub use crate::openfoam_algorithms::openfoam_source::quantities::Compressibility;
