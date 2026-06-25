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

/// Universal gas constant in J/(mol·K).
/// Using this value with `MolarMass` in kg/mol gives `r = R_UNIVERSAL / W` in J/(kg·K).
pub const R_UNIVERSAL: f64 = 8.314_462_618_153_24;

/// Standard thermodynamic temperature (used as entropy reference in S = Cp·ln(T/Tstd)).
pub const T_STD: f64 = 298.15; // K

/// Minimum temperature floor used in Newton T-iteration to prevent log(0).
pub const T_MIN: f64 = 100.0; // K

/// Upper JANAF coefficient range limit.
pub const T_MAX: f64 = 6000.0; // K

/// Standard-state reference pressure for entropy calculations.
pub const P_REF: f64 = 101_325.0; // Pa
