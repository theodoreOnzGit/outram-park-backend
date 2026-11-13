// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), Copyright (C) 2011-2016 OpenFOAM
// Foundation, 2016-2026 OpenCFD Ltd (ESI), GPL-3.0-or-later.
//
// Upstream (OpenFOAM-v2506):
//   src/mesh/snappyHexMesh/snappyHexMeshDriver/snappyLayerDriver.C  `writeLayerData`,
//   applications/utilities/mesh/generation/snappyHexMesh/snappyHexMesh.C  (`removeEmptyPatches`,
//   "Checking final mesh ..."),
//   src/finiteVolume/fvMesh/fvMeshTools/fvMeshTools.C  `removeEmptyPatches`.

//! gh:#1467 round 10: what ESI does after the layer loop (the tail of `doLayers` and of the
//! `snappyHexMesh` application), serial:
//!
//! * [`remove_empty_patches`]: `fvMeshTools::removeEmptyPatches` (drop zero-sized patches, keep the
//!   order of the others);
//! * [`final_check_mesh`]: the "Checking final mesh ..." quality check with the base
//!   `meshQualityControls` (never the relaxed ones), whose failing faces ESI writes as the
//!   `wrongFaces` set;
//! * [`layer_data_fields`]: the three fields `writeLayerData` writes with `writeFlags
//!   (layerFields)`: `nSurfaceLayers`, `thickness`, `thicknessFraction`.
//!
//! Results against ESI: `tests/snappy_hex_mesh_esi_layers_multi.rs`, README "Round 10".

use super::esi_layer_loop::LayerIteration;
use super::esi_layer_topo::avg_point_data_faces;
use super::esi_layers::LayerPatch;
use super::esi_merge::MergeMesh;
use crate::check_mesh::{esi_check_mesh, esi_geometry, EsiCheckResult, EsiQualityControls};

/// `fvMeshTools::removeEmptyPatches(mesh, validBoundary)` for a serial mesh without processor
/// patches: patches with no faces are removed, the others keep their relative order. Faces do not
/// move (a removed patch owns none); `face_patch`, `n_patches` and `constraint` are renumbered.
/// Returns the new mesh and `newToOld` (the old index of every surviving patch).
pub fn remove_empty_patches(m: &MergeMesh) -> (MergeMesh, Vec<usize>) {
    let mut n_faces = vec![0usize; m.n_patches];
    for &p in &m.face_patch {
        if p != usize::MAX {
            n_faces[p] += 1;
        }
    }
    let new_to_old: Vec<usize> = (0..m.n_patches).filter(|&p| n_faces[p] > 0).collect();
    let mut old_to_new = vec![usize::MAX; m.n_patches];
    for (new, &old) in new_to_old.iter().enumerate() {
        old_to_new[old] = new;
    }
    let mut out = m.clone();
    for p in out.face_patch.iter_mut() {
        if *p != usize::MAX {
            *p = old_to_new[*p];
        }
    }
    out.n_patches = new_to_old.len();
    out.constraint = new_to_old.iter().map(|&o| m.constraint[o]).collect();
    (out, new_to_old)
}

/// The "Checking final mesh ..." step of `snappyHexMesh`:
/// `motionSmoother::checkMesh(false, mesh, motionDict, wrongFaces)` on every face with the base
/// `meshQualityControls`. `wrong_faces` of the result is ESI's `wrongFaces` set (ascending); ESI
/// prints "Finished meshing with N illegal faces" when it is not empty.
pub fn final_check_mesh(m: &MergeMesh, controls: &EsiQualityControls) -> EsiCheckResult {
    let geom = esi_geometry(&m.mesh);
    let all: Vec<usize> = (0..m.mesh.faces.len()).collect();
    esi_check_mesh(&m.mesh, &geom, controls, &all)
}

/// The fields of `writeLayerData` (`layerFields`) in ESI's file order.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerFields {
    /// `nSurfaceLayers` internal field: `cellNLayers` of every cell.
    pub n_surface_layers_cells: Vec<f64>,
    /// `nSurfaceLayers` patch values: `cellNLayers` of the cell next to each patch face, per layer patch.
    pub n_surface_layers_patches: Vec<Vec<f64>>,
    /// `thickness` patch values: `faceRealThickness` per patch face, per layer patch.
    pub thickness_patches: Vec<Vec<f64>>,
    /// `thicknessFraction` patch values: real over wanted thickness (0 where wanted is not above
    /// `VSMALL`), per layer patch.
    pub thickness_fraction_patches: Vec<Vec<f64>>,
}

/// `VSMALL` of a double.
const VSMALL: f64 = 1e-300;

/// The layer fields of the mesh of a layer iteration. `patch_ids` are the layer patches (indices
/// into `it.layers.mesh.face_patch`); the wanted thickness of a face is `avgPointData(pp,
/// thickness)` of the patch face it descends from, as in [`super::esi_layer_loop::layer_data_table`].
pub fn layer_data_fields(it: &LayerIteration, pp: &LayerPatch, thickness: &[f64], patch_ids: &[usize]) -> LayerFields {
    let mesh = &it.layers.mesh;
    let wanted_pp = avg_point_data_faces(pp, thickness);
    let mut wanted = vec![0.0f64; mesh.mesh.faces.len()];
    for (pf, &f_old) in pp.addressing.iter().enumerate() {
        wanted[it.layers.face_map[f_old]] = wanted_pp[pf];
        for &lf in &it.layers.layer_faces[pf] {
            wanted[lf] = wanted_pp[pf];
        }
    }
    let mut out = LayerFields {
        n_surface_layers_cells: it.cell_n_layers.iter().map(|&c| c as f64).collect(),
        n_surface_layers_patches: Vec::new(),
        thickness_patches: Vec::new(),
        thickness_fraction_patches: Vec::new(),
    };
    for &patch in patch_ids {
        let faces: Vec<usize> = (0..mesh.mesh.faces.len()).filter(|&f| mesh.face_patch[f] == patch).collect();
        out.n_surface_layers_patches.push(faces.iter().map(|&f| it.cell_n_layers[mesh.mesh.owner[f]] as f64).collect());
        out.thickness_patches.push(faces.iter().map(|&f| it.face_real_thickness[f]).collect());
        out.thickness_fraction_patches
            .push(faces.iter().map(|&f| if wanted[f] > VSMALL { it.face_real_thickness[f] / wanted[f] } else { 0.0 }).collect());
    }
    out
}

/// The bytes `extract_layer_fields.py` writes for an ESI run: little-endian doubles of
/// `nSurfaceLayers` (cells, then the patches), `thickness` (patches), `thicknessFraction` (patches).
pub fn layer_field_bytes(f: &LayerFields) -> Vec<u8> {
    let mut out = Vec::new();
    let mut push = |v: &[f64]| {
        for x in v {
            out.extend_from_slice(&x.to_le_bytes());
        }
    };
    push(&f.n_surface_layers_cells);
    for p in &f.n_surface_layers_patches {
        push(p);
    }
    for p in &f.thickness_patches {
        push(p);
    }
    for p in &f.thickness_fraction_patches {
        push(p);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snappy_hex_mesh::esi_snap::SnapMesh;

    fn square_mesh() -> MergeMesh {
        // two quads sharing an edge; patches 0 (empty), 1 (one face), 2 (empty), 3 (one face)
        let mesh = SnapMesh {
            points: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
            faces: vec![vec![0, 1, 2, 3], vec![3, 2, 1, 0]],
            owner: vec![0, 0],
            neighbour: vec![],
        };
        MergeMesh { mesh, face_patch: vec![1, 3], n_patches: 4, constraint: vec![false, true, false, false] }
    }

    /// `removeEmptyPatches`: empty patches go, the order of the others and their flags stay.
    #[test]
    fn remove_empty_patches_keeps_order_and_flags() {
        let (m, new_to_old) = remove_empty_patches(&square_mesh());
        assert_eq!(new_to_old, vec![1, 3]);
        assert_eq!(m.n_patches, 2);
        assert_eq!(m.face_patch, vec![0, 1]);
        assert_eq!(m.constraint, vec![true, false]);
        // nothing empty: identity
        let (m2, n2) = remove_empty_patches(&m);
        assert_eq!(n2, vec![0, 1]);
        assert_eq!(m2.face_patch, m.face_patch);
    }
}
