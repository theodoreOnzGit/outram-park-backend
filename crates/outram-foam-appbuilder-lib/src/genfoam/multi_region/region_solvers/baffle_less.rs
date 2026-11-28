// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Upstream: GeN-Foam 652b3da `src/classes/multiRegion/meshHandler/meshHandler.C:389-506` (`createBaffleLessMeshes`)
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full notice.

//! # The baffle-less copy of a region's mesh, read from the case (gh:#1935, README round 69)
//!
//! [`read_baffle_less`] gathers what `polyMesh::writeObject` of `<region>.baffleLess` writes: the region's points, faces, owner,
//! neighbour, its patches as plain `polyPatch`es (type `patch`, `inGroups` kept), the cell zones and the number of face and point zones.
//! A refusal (a patch type outside `patch`/`wall`/`empty`, an unreadable file) is kept as the `Err` text and surfaces only when the
//! directory is written, so a case that never writes is unaffected.

use std::path::Path;

use super::DeformableMesh;
use crate::genfoam::multi_region::baffle_merge::merge_duplicate_faces;
use crate::io::foam_ascii_mesh::{parse_boundary_patches, PolyMeshOut};
use crate::io::poly_mesh::read_cell_zones;

/// The number of entries of a `faceZones`/`pointZones` file: its first token after the header (`0()` or `N(`), 0 when the file is absent.
fn zone_count(poly_mesh_dir: &Path, name: &str) -> Result<usize, String> {
    let Ok(text) = std::fs::read_to_string(poly_mesh_dir.join(name)) else { return Ok(0) };
    let at = text.find("}\n// * *").ok_or_else(|| format!("{name}: no header end"))?;
    let rest = text[at..].split_once("//\n").map_or("", |(_, r)| r);
    let digits: String = rest.trim_start().chars().take_while(char::is_ascii_digit).collect();
    digits.parse::<usize>().map_err(|_| format!("{name}: no zone count"))
}

/// What `<region>.baffleLess/polyMesh` holds, from the case's own `constant/<region>/polyMesh` and the region's mesh.
///
/// # Errors
/// The refusal text of a construct the writer does not lay out (a patch type, a file that does not parse).
pub fn read_baffle_less(poly_mesh_dir: &Path, dm: &DeformableMesh) -> Result<PolyMeshOut, String> {
    let btxt = std::fs::read_to_string(poly_mesh_dir.join("boundary")).map_err(|e| format!("boundary: {e}"))?;
    let patches = parse_boundary_patches(&btxt).map_err(|e| e.to_string())?;
    let zones = read_cell_zones(poly_mesh_dir).map_err(|e| format!("cellZones: {e}"))?;
    let topo = dm.topology();
    Ok(PolyMeshOut {
        points: dm.points().iter().map(|p| [p.x, p.y, p.z]).collect(),
        faces: topo.faces().to_vec(),
        owner: topo.owner().to_vec(),
        neighbour: topo.neighbour().to_vec(),
        n_cells: dm.mesh().n_cells,
        patches,
        cell_zones: zones.into_iter().map(|z| (z.name, z.cells)).collect(),
        n_face_zones: zone_count(poly_mesh_dir, "faceZones")?,
        n_point_zones: zone_count(poly_mesh_dir, "pointZones")?,
    })
}

/// `createBaffleLessMeshes` + `removeBaffles` (gh:#1236, README round 78): the region's current points, faces and patches with every pair of
/// duplicate boundary faces merged into one internal face and the faces renumbered as `polyTopoChange::changeMesh` leaves them
/// ([`merge_duplicate_faces`], upstream cited there). Zones are copied as `meshHandler.C:491-525` does (cell zones only here: a face or point
/// zone is refused by the writer, not here).
///
/// # Errors
/// The refusal text of a construct the writer or the merge does not lay out (a patch type, an unreadable file, a duplicate-face pair upstream
/// aborts on).
pub fn build_baffle_less(poly_mesh_dir: &Path, dm: &DeformableMesh) -> Result<PolyMeshOut, String> {
    let mut out = read_baffle_less(poly_mesh_dir, dm)?;
    let sizes: Vec<usize> = out.patches.iter().map(|p| p.n_faces).collect();
    let merged = merge_duplicate_faces(&out.faces, &out.owner, &out.neighbour, out.n_cells, &sizes)?;
    let mut start = merged.neighbour.len();
    for (p, &n) in out.patches.iter_mut().zip(&merged.patch_sizes) {
        p.n_faces = n;
        p.start_face = start;
        start += n;
    }
    out.faces = merged.faces;
    out.owner = merged.owner;
    out.neighbour = merged.neighbour;
    Ok(out)
}
