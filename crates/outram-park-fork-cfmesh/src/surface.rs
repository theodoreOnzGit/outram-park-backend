// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0; the
// per-file provenance blocks of `surface/*.rs` list the upstream files.
// Source: the cfMesh v1.1 line as carried by github.com/DAFoam/cfmesh, commit
// e944f6ba695fcefce73ad306074f6b3c0ea432ae.
// Copyright (C) Creative Fields, Ltd. Licence: GPL-3.0-or-later, conveyed
// here under GPL-3.0-only.
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

//! cfMesh's **triangulated-surface tools** (`triSurfaceTools/` and the
//! surface utilities), gh:#1276 and gh:#1277, working on the crate's one
//! surface type, [`crate::mesh_octree::TriSurf`] (extended for this:
//! patches, stored feature edges, subsets).
//!
//! # What is here
//!
//! - [`features`] (gh:#1276): `triSurfaceDetectFeatureEdges`,
//!   `triSurfacePatchManipulator`, `triSurfaceClassifyEdges`,
//!   `triSurfacePartitioner`, and the `surfaceFeatureEdges`,
//!   `patchesToSubsets` and `subsetToPatch` utilities.
//! - [`checks`] (gh:#1277): `triSurfaceChecks` (angles, manifolds, holes,
//!   non-manifold edges, orientation, disconnected parts, collocated
//!   points, self-intersections, overlaps) with `help::doTrianglesIntersect`
//!   and `help::doTrianglesOverlap`, and the `checkSurfaceMesh` utility.
//! - [`cleanup`] (gh:#1277): `triSurfaceCleanupDuplicates`,
//!   `triSurfaceCleanupDuplicateTriangles`, `triSurfaceRemoveFacets`,
//!   `triSurfaceCopyParts`, `triSurfaceImportSurfaceAsSubset`, and the
//!   `removeSurfaceFacets`, `copySurfaceParts`, `importSurfaceAsSubset` and
//!   `mergeSurfacePatches` utilities.
//!
//! The utilities are library functions: reading and writing FMS, FTR or
//! STL files is not ported (none is needed to test them). Every container
//! that is a hash set upstream is ordered here, so every output is
//! deterministic.
//!
//! # Upstream defects found while porting (DAFoam/cfmesh `e944f6ba695f`)
//!
//! Each is described where the port meets it; in short:
//! - `subsetToPatch` builds the new patches and discards them, renumbers
//!   the facet subsets with a patch map, and inverts its prefix test
//!   ([`features::subset_to_patch`] ports the intent);
//! - `checkForNonManifoldEdges(triSurf&, …)` runs the hole check
//!   ([`checks::check_for_non_manifold_edges_subset`] runs the right one);
//! - `triSurfaceCleanupDuplicates` reads stale `pointFacets` after a pass
//!   that removed facets but merged no points, and does not renumber
//!   feature edges or point subsets when it merges points
//!   ([`cleanup::cleanup_duplicates`]);
//! - `triSurfaceRemoveFacets` reads whichever addressing is cached after
//!   replacing the facets ([`cleanup::TriSurfaceRemoveFacets::remove_facets`]);
//! - `triSurfaceCopyParts` indexes `edgeFacets` with `-1` for a feature edge
//!   that is not a surface edge, and `triSurfaceClassifyEdges` reads
//!   `returnLeaf(-1)` beside the root box (both skipped here);
//! - `mergeSurfacePatches` copies subsets by loop index instead of id.
//!
//! Two behaviours that are as written but not as named:
//! [`checks::check_angles`] flags near-180° angles, not small ones, and
//! [`cleanup::import_surface_as_subset`] compares a cosine with an angle in
//! radians (effective tolerance about 85°).

pub mod checks;
pub mod cleanup;
pub mod curvature;
pub mod features;
#[cfg(test)]
mod tests;

use crate::math::Vec3;
use crate::mesh_octree::TriSurf;

/// A new surface on `points` and `triangles` that keeps `old`'s patch
/// list, stored feature edges and subsets **unchanged** (the caller
/// renumbers what its operation changes).
pub(crate) fn rebuild(old: &TriSurf, points: Vec<Vec3>, triangles: Vec<[usize; 3]>, regions: Vec<usize>) -> TriSurf {
    let mut s = TriSurf::from_parts(points, triangles, regions, old.patches.clone(), Vec::new());
    s.feature_edges = old.feature_edges.clone();
    s.point_subsets = old.point_subsets.clone();
    s.facet_subsets = old.facet_subsets.clone();
    s.edge_subsets = old.edge_subsets.clone();
    s
}
