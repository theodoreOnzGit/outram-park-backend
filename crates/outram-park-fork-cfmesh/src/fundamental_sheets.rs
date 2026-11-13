// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/surfaceTools/createFundamentalSheets/
//     createFundamentalSheets/createFundamentalSheets.{C,H}
//     createFundamentalSheetsJFS/createFundamentalSheetsJFS.{C,H}
//     createFundamentalSheetsFJ/createFundamentalSheetsFJ.{C,H}
// Source: the cfMesh v1.1 line as carried by github.com/DAFoam/cfmesh, commit
// e944f6ba695fcefce73ad306074f6b3c0ea432ae.
// Copyright (C) Creative Fields, Ltd. Licence: GPL-3.0-or-later (cfMesh files
// say "either version 3 of the License, or (at your option) any later
// version"), conveyed here under GPL-3.0-only.
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

//! cfMesh's **`createFundamentalSheets`** (gh:#1281): layers of cells
//! inserted with [`extrude_layer`] so that every boundary cell has one
//! boundary face and every patch's cells are wrapped by a sheet, which
//! lets the surface smoother and the patch mapper of
//! `meshSurfaceEdgeExtractorFUN` produce sharp feature edges.
//!
//! Upstream selects a variant at run time (`createFundamentalSheets` is a
//! base class with a selection table); [`SheetsKind`] is that choice, by
//! enum. `meshSurfaceEdgeExtractorFUN` constructs
//! `createFundamentalSheetsJFS` directly.
//!
//! - **JFS** (`createFundamentalSheetsJFS`):
//!   1. `createInitialSheet`: unless `createWrapperSheet`, stop if
//!      `isTopologyOk` (no cell has two boundary faces); otherwise extrude
//!      every boundary face into its owner.
//!   2. `createSheetsAtFeatureEdges` (skipped with fewer than two
//!      patches): each boundary cell is marked with its face's patch (the
//!      last boundary face wins, in patch order); every internal face of a
//!      boundary cell whose other cell is not marked with the same patch is
//!      extruded into the boundary cell. One `extrudeLayer` call.
//! - **FJ** (`createFundamentalSheetsFJ`):
//!   1. `createInitialSheet`: the wrapper sheet only when
//!      `createWrapperSheet`.
//!   2. `createSheetsAtFeatureEdges`: patch by patch, the internal faces
//!      between a cell of the patch and a cell that is not are extruded into
//!      the other cell. One `extrudeLayer` call per patch.
//!
//! **Deviations:** boundary faces are visited patch by patch (upstream's
//! contiguous patch ranges), each in ascending face label; cell faces in
//! ascending label (upstream's cell lists are in the order the mesh holds
//! them). Both change the numbering of new faces, points and cells only.

use crate::layers::extrude_layer::{extrude_layer, ExtrudeLayerError, ExtrudeLayerReport};
use crate::topology::TopoMesh;

/// Which `createFundamentalSheets` variant (upstream's run-time selection).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetsKind {
    /// `createFundamentalSheetsJFS`, the one `meshSurfaceEdgeExtractorFUN` uses.
    Jfs,
    /// `createFundamentalSheetsFJ`.
    Fj,
}

/// What a `createFundamentalSheets` run did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SheetsReport {
    /// `isTopologyOk` on the input (JFS only; `true` for FJ).
    pub topology_ok: bool,
    /// The wrapper sheet's `extrudeLayer`, if it ran.
    pub initial_sheet: Option<ExtrudeLayerReport>,
    /// The feature-edge sheets' `extrudeLayer` calls (one for JFS, one per
    /// patch for FJ; none when JFS skips with fewer than two patches).
    pub feature_edge_sheets: Vec<ExtrudeLayerReport>,
}

/// Boundary faces in upstream's order: patch by patch, then faces in no
/// patch (a patch of their own).
fn boundary_faces_in_patch_order(w: &TopoMesh) -> Vec<usize> {
    let n_patches = w.patch_names.len();
    let mut out = Vec::new();
    for p in 0..=n_patches {
        out.extend((0..w.faces.len()).filter(|&f| w.neighbour[f].is_none() && w.region[f].unwrap_or(n_patches).min(n_patches) == p));
    }
    out
}

fn patch_of(w: &TopoMesh, f: usize) -> usize {
    let n = w.patch_names.len();
    w.region[f].unwrap_or(n).min(n)
}

/// The patches present, in patch order (upstream's `boundaries`: a patch
/// with no face still counts, as here).
fn n_boundaries(w: &TopoMesh) -> usize {
    let unassigned = w.neighbour.iter().zip(&w.region).any(|(n, r)| n.is_none() && r.is_none_or(|r| r >= w.patch_names.len()));
    w.patch_names.len() + usize::from(unassigned)
}

/// `createFundamentalSheetsJFS::isTopologyOk`: no cell has more than one
/// boundary face.
pub fn is_topology_ok(w: &TopoMesh) -> bool {
    let mut n = vec![0usize; w.n_cells];
    for f in 0..w.faces.len() {
        if w.neighbour[f].is_none() {
            n[w.owner[f]] += 1;
            if n[w.owner[f]] > 1 {
                return false;
            }
        }
    }
    true
}

/// Extrude every boundary face into its owner (`createInitialSheet`).
fn wrapper_sheet(w: &TopoMesh) -> Result<(TopoMesh, ExtrudeLayerReport), ExtrudeLayerError> {
    let front: Vec<(usize, usize)> = boundary_faces_in_patch_order(w).into_iter().map(|f| (f, w.owner[f])).collect();
    extrude_layer(w, &front, None)
}

/// `createFundamentalSheetsJFS::createSheetsAtFeatureEdges`'s front.
pub fn jfs_feature_edge_front(w: &TopoMesh) -> Vec<(usize, usize)> {
    let bfaces = boundary_faces_in_patch_order(w);
    let mut patch_cell: Vec<Option<usize>> = vec![None; w.n_cells];
    for &f in &bfaces {
        patch_cell[w.owner[f]] = Some(patch_of(w, f));
    }
    let cells = w.cell_lists();
    let mut front = Vec::new();
    for &f in &bfaces {
        let c = w.owner[f];
        let patch = patch_of(w, f);
        for &g in &cells[c] {
            let Some(n) = w.neighbour[g] else { continue };
            let nei = if w.owner[g] == c { n } else { w.owner[g] };
            if patch_cell[nei] != Some(patch) {
                front.push((g, c));
            }
        }
    }
    front
}

/// `createFundamentalSheetsFJ::createSheetsAtFeatureEdges`'s front for patch
/// `patch`.
pub fn fj_feature_edge_front(w: &TopoMesh, patch: usize) -> Vec<(usize, usize)> {
    let bfaces: Vec<usize> = (0..w.faces.len()).filter(|&f| w.neighbour[f].is_none() && patch_of(w, f) == patch).collect();
    let mut patch_cell = vec![false; w.n_cells];
    for &f in &bfaces {
        patch_cell[w.owner[f]] = true;
    }
    let cells = w.cell_lists();
    let mut front = Vec::new();
    for &f in &bfaces {
        let c = w.owner[f];
        for &g in &cells[c] {
            let Some(n) = w.neighbour[g] else { continue };
            let nei = if w.owner[g] == c { n } else { w.owner[g] };
            if !patch_cell[nei] {
                front.push((g, nei));
            }
        }
    }
    front
}

/// `createFundamentalSheets` of kind `kind` on `w` (module docs).
pub fn create_fundamental_sheets(w: &TopoMesh, kind: SheetsKind, create_wrapper_sheet: bool) -> Result<(TopoMesh, SheetsReport), ExtrudeLayerError> {
    let mut rep = SheetsReport { topology_ok: true, ..SheetsReport::default() };
    let mut cur = w.clone();
    match kind {
        SheetsKind::Jfs => {
            rep.topology_ok = is_topology_ok(&cur);
            if create_wrapper_sheet || !rep.topology_ok {
                let (m, r) = wrapper_sheet(&cur)?;
                cur = m;
                rep.initial_sheet = Some(r);
            }
            if n_boundaries(&cur) >= 2 {
                let front = jfs_feature_edge_front(&cur);
                let (m, r) = extrude_layer(&cur, &front, None)?;
                cur = m;
                rep.feature_edge_sheets.push(r);
            }
        }
        SheetsKind::Fj => {
            if create_wrapper_sheet {
                let (m, r) = wrapper_sheet(&cur)?;
                cur = m;
                rep.initial_sheet = Some(r);
            }
            for patch in 0..n_boundaries(&cur) {
                let front = fj_feature_edge_front(&cur, patch);
                let (m, r) = extrude_layer(&cur, &front, None)?;
                cur = m;
                rep.feature_edge_sheets.push(r);
            }
        }
    }
    Ok((cur, rep))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartesian::cartesian_box;
    use crate::checks::cell_geometry;
    use crate::math::Vec3;

    fn summary(w: &TopoMesh) -> (f64, f64, usize, Vec<usize>) {
        let (m, _) = w.clone().into_volume_mesh();
        m.validate().unwrap();
        let (vols, _) = cell_geometry(&m);
        let vmin = vols.iter().cloned().fold(f64::MAX, f64::min);
        (m.total_volume(), vmin, m.n_boundary_faces(), m.patches.iter().map(|p| p.n_faces).collect())
    }

    /// V&V (gh:#1281). A 3×3×3 unit box with its six patches.
    /// **Predictions (written 2026-11-05, before the first run):**
    /// - `isTopologyOk` is false (edge and corner cells have 2–3 boundary
    ///   faces), so JFS builds the wrapper sheet first: 27 + 54 cells, the
    ///   volume stays 1 (the boundary does not move);
    /// - after the wrapper every boundary cell has one boundary face, so
    ///   the topology check holds on the result of step 1;
    /// - step 2 moves boundary points (the new points at the feature edges
    ///   are pulled inside), so the volume falls below 1; the mesh stays
    ///   closed with every cell positive; every new boundary face gets a
    ///   patch (none unpatched); the six patches survive.
    ///
    /// **Results (2026-11-05):** held: topology false on input, wrapper 54
    /// faces / 56 new points, closed, every cell positive (smallest
    /// 1.06e-3), no unpatched face, six patches of 21 faces. **Missed:** the
    /// volume rises to 1.12772, not below 1. A new point on a feature edge
    /// moves against the summed normals of the copies at it (into its
    /// wrapper cell, away from the copies), and for a point on the boundary
    /// that is out through the cell's boundary face. Upstream's
    /// `meshSurfaceEdgeExtractorFUN` maps the boundary back onto the surface
    /// afterwards (`remapBoundaryPoints`). Feature-edge sheet: 126 faces,
    /// 168 new points, 72 new boundary quads, no edge or corner cell; 207
    /// cells.
    #[test]
    fn jfs_on_a_box() {
        let w = TopoMesh::from_volume_mesh(&cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [3, 3, 3]));
        assert!(!is_topology_ok(&w));
        let (out, rep) = create_fundamental_sheets(&w, SheetsKind::Jfs, false).unwrap();
        let init = rep.initial_sheet.clone().unwrap();
        assert_eq!(init.n_extruded_faces, 54);
        let fe = &rep.feature_edge_sheets[0];
        let (v, vmin, nb, per_patch) = summary(&out);
        println!("JFS box: initial {init:?}\nfeature edges {fe:?}\ncells {} volume {v} vmin {vmin:e} boundary faces {nb} per patch {per_patch:?}", out.n_cells);
        assert!(vmin > 0.0);
        assert!((v - 1.12772).abs() < 1e-5, "{v}");
        assert_eq!(out.n_cells, 207);
        assert_eq!((fe.n_extruded_faces, fe.n_new_points, fe.n_new_boundary_faces), (126, 168, 72));
        assert_eq!(fe.n_unpatched_boundary_faces, 0);
        assert_eq!(per_patch.len(), 6);
    }

    /// V&V (gh:#1281), FJ on the same box with the wrapper sheet.
    /// **Predictions (written 2026-11-05, before the first run):** closed,
    /// every cell positive, no unpatched face, six patches; each patch's
    /// sheet moves points into the interior, not the boundary, so the
    /// volume stays 1 to 1e-12 (FJ extrudes into the non-patch cell).
    ///
    /// **Results (2026-11-05):** held: closed, every cell positive
    /// (smallest 9.9e-6), no unpatched face, six patches of 25 faces; 275
    /// cells. **Missed:** the volume is 1.11451, for the reason in
    /// [`jfs_on_a_box`]: the non-patch cell of a patch's sheet is the
    /// neighbouring patch's wrapper cell at the feature edges, whose points
    /// are boundary points.
    #[test]
    fn fj_on_a_box() {
        let w = TopoMesh::from_volume_mesh(&cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [3, 3, 3]));
        let (out, rep) = create_fundamental_sheets(&w, SheetsKind::Fj, true).unwrap();
        let (v, vmin, nb, per_patch) = summary(&out);
        println!("FJ box: {rep:?}\ncells {} volume {v} vmin {vmin:e} boundary faces {nb} per patch {per_patch:?}", out.n_cells);
        assert!(vmin > 0.0);
        assert_eq!(per_patch.len(), 6);
        assert!((v - 1.11451).abs() < 1e-5, "{v}");
        assert_eq!(out.n_cells, 275);
        assert!(rep.feature_edge_sheets.iter().all(|r| r.n_unpatched_boundary_faces == 0));
    }
}
