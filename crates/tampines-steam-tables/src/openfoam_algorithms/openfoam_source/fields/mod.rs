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

//! OpenFOAM-style field abstraction: the flat `Field<T>` container plus the
//! mesh-coupled `VolField` (cell-centred) and `SurfaceField` (face-centred)
//! wrappers, and their boundary-condition types. These are generic numeric
//! containers — any physical unit lives in the caller-supplied element type,
//! not in this layer.

/// Boundary-condition types (`BoundaryCondition`, `PatchField`).
pub mod boundary;
/// Flat `Field<T>` container with element-wise arithmetic; the storage
/// backing both `VolField` and `SurfaceField`.
pub mod field;
/// Face-centred `SurfaceField` (`Foam::surfaceScalarField`-style) and its type aliases.
pub mod surface_field;
/// Cell-centred `VolField` (`Foam::volScalarField`-style) and its type aliases.
pub mod vol_field;

pub use boundary::*;
pub use field::Field;
pub use surface_field::*;
pub use vol_field::*;
