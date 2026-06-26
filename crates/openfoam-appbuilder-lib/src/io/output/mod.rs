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

use crate::error::AppBuilderError;
use openfoam_basic_lib::prelude::{VolScalarField, VolVectorField};
use std::path::Path;

/// Write a scalar field to `<time_dir>/<field_name>` in OpenFOAM ASCII format.
///
/// The output follows the standard OpenFOAM field file layout:
/// ```text
/// FoamFile { version 2.0; format ascii; class volScalarField; object p; }
/// dimensions [kg m-1 s-2];
/// internalField nonuniform List<scalar> N ( v0 v1 … vN-1 );
/// boundaryField { … }
/// ```
pub fn write_scalar_field(
    time_dir: &Path,
    field: &VolScalarField,
    dimensions: &str,
) -> Result<(), AppBuilderError> {
    let _ = (time_dir, field, dimensions);
    todo!("write_scalar_field — serialise VolScalarField to OpenFOAM ASCII format")
}

/// Write a vector field to `<time_dir>/<field_name>` in OpenFOAM ASCII format.
pub fn write_vector_field(
    time_dir: &Path,
    field: &VolVectorField,
    dimensions: &str,
) -> Result<(), AppBuilderError> {
    let _ = (time_dir, field, dimensions);
    todo!("write_vector_field — serialise VolVectorField to OpenFOAM ASCII format")
}

/// Write a legacy VTK unstructured grid file for ParaView.
///
/// Includes mesh geometry and all provided scalar fields.
pub fn write_vtk(
    out_path: &Path,
    mesh_points: &[[f64; 3]],
    scalar_fields: &[(&str, &VolScalarField)],
) -> Result<(), AppBuilderError> {
    let _ = (out_path, mesh_points, scalar_fields);
    todo!("write_vtk — serialise mesh + fields to legacy VTK ASCII format")
}
