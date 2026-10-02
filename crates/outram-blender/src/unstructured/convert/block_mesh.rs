// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
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

//! Orchestration of `outram-foam-mesh` (feature `block-mesh`): `blockMesh`
//! and the `snappyHexMesh` driver produce an OpenFOAM `polyMesh`, which is
//! handed to the neutral mesh through [`super::foam::from_poly_mesh`].
//!
//! The meshers stay in `outram-foam-mesh`; this module only calls them, so
//! every quality report and every phase decision is theirs.

use outram_foam_mesh::driver::{mesh_from_surface, GeneratedMesh, MeshingControls};
use outram_foam_mesh::snappy_hex_mesh::stl::{Triangle, TriangleSoup};
use outram_foam_basic_lib::primitives::Vector3;

use super::foam::from_poly_mesh;
use super::ConvertError;
use crate::export::triangulate;
use crate::mesh::Mesh;
use crate::unstructured::mesh::UnstructuredMesh;

/// Run `blockMesh` on the text of a `blockMeshDict` and return the neutral
/// mesh (metres; hex cells recognised as [`Hex8`](crate::unstructured::ElementKind::Hex8)).
///
/// # Errors
/// [`ConvertError::Solver`] with `blockMesh`'s message, or a neutral-mesh
/// construction error.
pub fn block_mesh(dict_text: &str) -> Result<UnstructuredMesh, ConvertError> {
    let poly = outram_foam_mesh::block_mesh::block_mesh(dict_text).map_err(|e| ConvertError::Solver(e.to_string()))?;
    from_poly_mesh(&poly.to_foam_poly_mesh())
}

/// A blender surface [`Mesh`] (metres, closed, outward-wound) as the
/// triangle soup `snappyHexMesh` reads, fan-triangulated by
/// [`crate::export::triangulate`].
pub fn surface_to_triangle_soup(surface: &Mesh, name: &str) -> TriangleSoup {
    let t = triangulate(surface);
    let v = |i: u32| {
        let p = t.positions[i as usize];
        Vector3::new(p.x, p.y, p.z)
    };
    let triangles = t.indices.chunks_exact(3).map(|c| Triangle::new(v(c[0]), v(c[1]), v(c[2]))).collect();
    TriangleSoup::new(name, triangles)
}

/// Run the `snappyHexMesh` driver (`outram_foam_mesh::mesh_from_surface`)
/// on a closed surface and return the neutral mesh together with the
/// driver's own result (quality report, phases run, layer outcome), which a
/// caller must read before trusting the mesh.
///
/// # Errors
/// [`ConvertError::Solver`] with the driver's message, or a neutral-mesh
/// construction error.
pub fn snappy_from_surface(
    surface: &TriangleSoup,
    controls: &MeshingControls,
) -> Result<(UnstructuredMesh, GeneratedMesh), ConvertError> {
    let generated = mesh_from_surface(surface, controls).map_err(|e| ConvertError::Solver(e.to_string()))?;
    let mesh = from_poly_mesh(&generated.mesh)?;
    Ok((mesh, generated))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unstructured::ElementKind;

    /// A single-block 2 x 3 x 4 `blockMeshDict` on a 1 x 1 x 1 m box gives
    /// 24 hex cells, all recognised as Hex8, total volume 1 m^3.
    #[test]
    fn block_mesh_box_is_24_hexes() {
        let dict = r#"
convertToMeters 1;
vertices ( (0 0 0) (1 0 0) (1 1 0) (0 1 0) (0 0 1) (1 0 1) (1 1 1) (0 1 1) );
blocks ( hex (0 1 2 3 4 5 6 7) (2 3 4) simpleGrading (1 1 1) );
edges ();
boundary ( walls { type wall; faces ( (0 3 2 1) (4 5 6 7) (0 1 5 4) (1 2 6 5) (2 3 7 6) (3 0 4 7) ); } );
"#;
        let m = block_mesh(dict).unwrap();
        assert_eq!(m.n_cells(), 24);
        assert!((0..24).all(|c| m.cell_kind(c) == ElementKind::Hex8));
        assert!((m.total_volume() - 1.0).abs() < 1e-12);
    }
}
