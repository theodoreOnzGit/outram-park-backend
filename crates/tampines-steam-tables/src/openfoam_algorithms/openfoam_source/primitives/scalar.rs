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

//! OpenFOAM-style scalar type aliases and small/great tolerance constants,
//! used throughout the FV primitives layer for numeric-safety checks
//! (avoiding division by ~0, detecting an unset/uninitialised value, etc).
//! All values here are dimensionless — mirrors `src/OpenFOAM/primitives/Scalar/`.

/// Alias for the crate's floating-point scalar type (`f64`). Mirrors
/// `Foam::scalar`.
pub type Scalar = f64;
/// Alias for the crate's integer index/label type (`i64`). Mirrors
/// `Foam::label`.
pub type Label = i64;

/// A "negligibly small" tolerance, `1e-15`. Mirrors `Foam::SMALL`.
pub const SMALL: Scalar = 1e-15;
/// A "vanishingly small" tolerance, `1e-300` (near `f64` underflow).
/// Mirrors `Foam::VSMALL`.
pub const VSMALL: Scalar = 1e-300;
/// `sqrt(SMALL)`, `≈3.162e-8`. Mirrors `Foam::ROOTSMALL`.
pub const ROOT_SMALL: Scalar = 3.162_277_660_168_379_5e-8; // sqrt(1e-15)
/// `sqrt(VSMALL)`, `1e-150`. Mirrors `Foam::ROOTVSMALL`.
pub const ROOT_VSMALL: Scalar = 1e-150; // sqrt(1e-300)
/// A "very large" bound, `1e15`. Mirrors `Foam::GREAT`.
pub const GREAT: Scalar = 1e15;
/// A "near-overflow" bound, `1e300`. Mirrors `Foam::VGREAT`.
pub const VGREAT: Scalar = 1e300;
/// `sqrt(GREAT)`, `≈3.162e7`. Mirrors `Foam::ROOTGREAT`.
pub const ROOT_GREAT: Scalar = 3.162_277_660_168_379_5e7; // sqrt(1e15)
