use std::path::Path;
use openfoam_basic_lib::prelude::{VolScalarField, VolVectorField};
use crate::error::AppBuilderError;

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
