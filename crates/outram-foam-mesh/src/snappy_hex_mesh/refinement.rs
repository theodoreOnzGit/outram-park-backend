// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.org)
// Copyright (C) 2011-2026 OpenFOAM Foundation
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

//! Surface-intersection refinement: `snappyRefineDriver::surfaceOnlyRefine`.
//!
//! **Upstream followed:** OpenFOAM.com ESI **v2506** (maintainer decision on
//! gh:#1459, 2026-11-12, "ESI preferred"; the run that checks it is
//! `verification_and_validation/snappy_hex_mesh_esi_v2506/README.md`).
//! ~~OpenFOAM-dev (OpenFOAM Foundation) commit
//! `852527377c7e87e33d51b19ca8974b5db60af28b`~~ **CORRECTED 2026-11-12** to
//! ESI v2506, with the line numbers re-read from the v2506 sources (dev's in
//! brackets, background only):
//!
//! - `snappyHexMeshDriver/snappyRefineDriver.C:342` `surfaceOnlyRefine`
//!   [dev :179] (loop, stop rule `nCellsToRefine == 0 || (iter >=
//!   overallMaxLevel && nCellsToRefine <= minRefineCells)`, `maxIter` 100);
//! - `meshRefinement/meshRefinementRefine.C:945` `markSurfaceRefinement`
//!   [dev :908] (one segment per face from `meshRefinement.C:288`
//!   `calcCellCellRays`: owner centre to neighbour centre, or, on a non-coupled
//!   boundary face, owner centre to the face centre from `calcNeighbourData`;
//!   segment extended by `rootSmall` of its length at each end; a cell is
//!   marked when the segment hits a surface whose level exceeds the cell's);
//! - `refinementSurfaces/refinementSurfaces.C:1265` `findHigherIntersection`
//!   [dev :410] (`findLineAny` on the triSurfaceMesh);
//! - `dynamicMesh/polyTopoChange/polyTopoChange/hexRef8/hexRef8.C:1567`
//!   `faceConsistentRefinement` [dev :1426] and `:2233` `consistentRefinement`
//!   [dev :2127] (face 2:1, `maxSet = true`);
//! - `meshRefinement/meshRefinementBaffles.C:4927` `splitMesh` and
//!   `snappyRefineDriver.C:2754` `baffleAndSplitMesh` for [`baffle_region`].
//!
//! **Checked against the ESI binary (gh:#1459, 2026-11-12):** on the flange the
//! selected-cell counts per iteration and all 42788 cells and levels equal
//! ESI's, when the surface is read as upstream reads it
//! ([`TriangleSoup::from_zoned`]: every `solid`, single-precision corners).
//!
//! ## What is ported, and what is not
//!
//! Ported: one surface with a single (min = max) level; the face-segment test;
//! the 2:1 closure; the stop rule. The octree's face-adjacent leaf pairs are
//! the same pairs as upstream's split-hex faces (a coarse face adjoining four
//! fine cells is four faces, each with its own fine neighbour), so the
//! segments are the same segments.
//!
//! Not ported (still [`RefinementSelection::DistanceBand`] territory):
//! `markSurfaceCurvatureRefinement` (needs `(min max)` levels and
//! `resolveFeatureAngle`; with `level (n n)` upstream skips it too),
//! `refinementRegions`, `features`, gap refinement, `maxGlobalCells` /
//! `maxLocalCells`, `nCellsBetweenLevels > 1` (`consistentSlowRefinement`),
//! coupled (cyclic) boundary faces, which are treated as ordinary boundary
//! faces here. Cell *removal* after refinement is by default this port's inside test;
//! ~~not upstream's baffle-and-split (gh:#1466)~~ **UPDATED 2026-11-12:** upstream's
//! chain (`removeInsideCells(1)`, `danglingCellRefine`, `baffleAndSplitMesh` with
//! `markFacesOnProblemCells`) is [`crate::snappy_hex_mesh::esi_split`], opt-in
//! through `CastellationControls::with_esi_split_mesh_removal`.
//!
//! [`RefinementSelection::DistanceBand`]: crate::snappy_hex_mesh::castellation::RefinementSelection::DistanceBand

use std::collections::HashMap;

use outram_foam_basic_lib::primitives::Vector3;

use crate::snappy_hex_mesh::castellation::Leaf;
use crate::snappy_hex_mesh::stl::TriangleSoup;

/// OpenFOAM's `rootSmall`, `sqrt(small)` with `small = 2.2204e-16` double
/// (`doubleScalarRootSmall`, `OpenFOAM/primitives/Scalar/doubleScalar`).
pub(crate) const ROOT_SMALL: f64 = 1.490_116_119_384_765_6e-8;

/// One mesh face of the current octree: an owner leaf, an optional neighbour
/// leaf (`None` = domain boundary face) and the segment tested against the
/// surface.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FacePair {
    pub(crate) own: usize,
    pub(crate) nei: Option<usize>,
    pub(crate) start: Vector3,
    pub(crate) end: Vector3,
}

/// Centre of `leaf` `[m]` on a background of `size0` level-0 cell sizes.
pub(crate) fn centre(leaf: &Leaf, origin: Vector3, size0: [f64; 3]) -> Vector3 {
    let f = 1.0 / (1usize << leaf.level) as f64;
    Vector3::new(
        origin.x + (leaf.i as f64 + 0.5) * size0[0] * f,
        origin.y + (leaf.j as f64 + 0.5) * size0[1] * f,
        origin.z + (leaf.k as f64 + 0.5) * size0[2] * f,
    )
}

/// One face of the octree's split-hex mesh, by topology only: the (finer-side)
/// owner leaf, the neighbour leaf (an equal-level leaf or the nearest coarser
/// ancestor; `None` on the domain boundary), and which side of `own` the face
/// lies on. The face itself is the `axis`/`positive` side of `own`, whichever of
/// the two leaves is finer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OctFace {
    pub(crate) own: usize,
    pub(crate) nei: Option<usize>,
    pub(crate) axis: usize,
    pub(crate) positive: bool,
}

/// Enumerate the faces of the octree (topology only, see [`OctFace`]).
///
/// A face between two leaves of different level is listed once per finer-side
/// leaf, exactly as the split-hex mesh lists it. Equal-level pairs are listed
/// from the lower-index side along the positive axis only. gh:#1466: split out
/// of [`face_pairs`] (same order, same faces) so the ESI cell-removal stages can
/// reuse the adjacency.
pub(crate) fn octree_faces(leaves: &[Leaf], n0: [usize; 3]) -> Vec<OctFace> {
    let mut map: HashMap<(usize, usize, usize, usize), usize> = HashMap::with_capacity(leaves.len());
    for (id, l) in leaves.iter().enumerate() {
        map.insert((l.level, l.i, l.j, l.k), id);
    }
    let mut out = Vec::new();
    for (id, l) in leaves.iter().enumerate() {
        let ijk = [l.i, l.j, l.k];
        let extent = [n0[0] << l.level, n0[1] << l.level, n0[2] << l.level];
        for axis in 0..3 {
            for positive in [false, true] {
                // Neighbour index along the axis at this level.
                let ni = if positive {
                    ijk[axis] + 1
                } else if ijk[axis] == 0 {
                    usize::MAX
                } else {
                    ijk[axis] - 1
                };
                let outside = ni == usize::MAX || ni >= extent[axis];
                if outside {
                    out.push(OctFace { own: id, nei: None, axis, positive });
                    continue;
                }
                let mut q = ijk;
                q[axis] = ni;
                // Same level, or the nearest coarser ancestor.
                let mut found = None;
                for d in 0..=l.level {
                    let key = (l.level - d, q[0] >> d, q[1] >> d, q[2] >> d);
                    if let Some(&nid) = map.get(&key) {
                        found = Some((nid, d));
                        break;
                    }
                }
                let Some((nid, d)) = found else {
                    continue; // neighbour side is finer: listed from there
                };
                if d == 0 && !positive {
                    continue; // equal level: positive direction only
                }
                out.push(OctFace { own: id, nei: Some(nid), axis, positive });
            }
        }
    }
    out
}

/// Enumerate the faces of the octree as owner/neighbour leaf pairs, with their
/// `calcNeighbourData` segments (already extended by `rootSmall`). See
/// [`octree_faces`] for which faces are listed.
pub(crate) fn face_pairs(
    leaves: &[Leaf],
    n0: [usize; 3],
    origin: Vector3,
    size0: [f64; 3],
) -> Vec<(usize, Option<usize>, Vector3, Vector3)> {
    octree_faces(leaves, n0)
        .into_iter()
        .map(|f| {
            let l = &leaves[f.own];
            let c = centre(l, origin, size0);
            let (start, end) = match f.nei {
                None => {
                    // Non-coupled boundary face: neiCc is the face centre.
                    let half = 0.5 * size0[f.axis] / (1usize << l.level) as f64;
                    let d = if f.positive { half } else { -half };
                    let mut fc = c;
                    match f.axis {
                        0 => fc.x += d,
                        1 => fc.y += d,
                        _ => fc.z += d,
                    }
                    (c, fc)
                }
                Some(n) => (c, centre(&leaves[n], origin, size0)),
            };
            let sv = (end - start) * ROOT_SMALL;
            (f.own, f.nei, start - sv, end + sv)
        })
        .collect()
}

/// Run `surfaceOnlyRefine` on the octree `leaves` (all level 0 on entry or any
/// 2:1-consistent octree) and return the refined leaves.
///
/// `n0` is the level-0 cell count per axis, `size0` the level-0 cell size,
/// `surface_level` the surface's (min = max) refinement level, and
/// `min_refine_cells` upstream's `minRefinementCells`.
pub(crate) fn surface_only_refine(
    leaves: Vec<Leaf>,
    n0: [usize; 3],
    origin: Vector3,
    size0: [f64; 3],
    surface_level: usize,
    surface: &TriangleSoup,
    min_refine_cells: usize,
) -> Vec<Leaf> {
    surface_only_refine_traced(leaves, n0, origin, size0, surface_level, surface, min_refine_cells, 100).0
}

/// [`surface_only_refine`] that also returns, per iteration, the number of
/// cells selected after the 2:1 closure: the integer upstream prints as
/// `Selected for refinement : N cells` (`snappyRefineDriver.C`
/// `surfaceOnlyRefine`, ESI v2506 line 342). The final entry is the iteration
/// that stopped the loop (0 when the loop ended on "no cells selected").
pub(crate) fn surface_only_refine_traced(
    mut leaves: Vec<Leaf>,
    n0: [usize; 3],
    origin: Vector3,
    size0: [f64; 3],
    surface_level: usize,
    surface: &TriangleSoup,
    min_refine_cells: usize,
    max_iter: usize,
) -> (Vec<Leaf>, Vec<usize>) {
    let mut selected = Vec::new();
    for iter in 0..max_iter {
        let pairs: Vec<FacePair> = face_pairs(&leaves, n0, origin, size0)
            .into_iter()
            .map(|(own, nei, start, end)| FacePair { own, nei, start, end })
            .collect();
        let mut refine = vec![false; leaves.len()];

        // markSurfaceRefinement
        for p in &pairs {
            let min_level = match p.nei {
                Some(n) => leaves[p.own].level.min(leaves[n].level),
                None => leaves[p.own].level,
            };
            if surface_level <= min_level || !surface.segment_hits(p.start, p.end) {
                continue;
            }
            if surface_level > leaves[p.own].level {
                refine[p.own] = true;
            }
            if let Some(n) = p.nei {
                if surface_level > leaves[n].level {
                    refine[n] = true;
                }
            }
        }

        consistent_refinement(&leaves, &pairs, &mut refine);

        let n_refine = refine.iter().filter(|&&r| r).count();
        selected.push(n_refine);
        if n_refine == 0 || (iter >= surface_level && n_refine <= min_refine_cells) {
            break;
        }
        leaves = split(&leaves, &refine);
    }
    (leaves, selected)
}

/// The refined octree of `surface_only_refine` as `(level, centre)` pairs plus
/// the per-iteration selected-cell counts, with no cell removal. Exists so the
/// ESI v2506 differential can compare the refinement stage on its own
/// (`verification_and_validation/snappy_hex_mesh_esi_v2506`).
pub fn refined_octree(
    surface: &TriangleSoup,
    controls: &crate::snappy_hex_mesh::castellation::CastellationControls,
    max_iter: usize,
) -> (Vec<(usize, Vector3)>, Vec<usize>) {
    let bg = &controls.background;
    let span = bg.bounds.span();
    let size0 = [span.x / bg.nx as f64, span.y / bg.ny as f64, span.z / bg.nz as f64];
    let mut leaves = Vec::with_capacity(bg.n_cells());
    for i in 0..bg.nx {
        for j in 0..bg.ny {
            for k in 0..bg.nz {
                leaves.push(Leaf { level: 0, i, j, k });
            }
        }
    }
    let (leaves, selected) = surface_only_refine_traced(
        leaves,
        [bg.nx, bg.ny, bg.nz],
        bg.bounds.min,
        size0,
        controls.surface_level,
        surface,
        controls.min_refine_cells,
        max_iter,
    );
    let cells = leaves
        .iter()
        .map(|l| (l.level, centre(l, bg.bounds.min, size0)))
        .collect();
    (cells, selected)
}

/// Index of the leaf whose half-open box holds `p` (`findCell` for
/// `locationInMesh`), or `None` outside every leaf.
pub(crate) fn find_leaf(leaves: &[Leaf], origin: Vector3, size0: [f64; 3], p: Vector3) -> Option<usize> {
    leaves.iter().position(|l| {
        let c = centre(l, origin, size0);
        let f = 0.5 / (1usize << l.level) as f64;
        (p.x >= c.x - f * size0[0] && p.x < c.x + f * size0[0])
            && (p.y >= c.y - f * size0[1] && p.y < c.y + f * size0[1])
            && (p.z >= c.z - f * size0[2] && p.z < c.z + f * size0[2])
    })
}

/// Keep the face-connected region of the octree that holds `keep`, where a
/// face is a baffle (blocks the region) when its `calcCellCellRays` segment
/// hits the surface: the cell-selection result of upstream's
/// `meshRefinement::splitMesh` / `baffleAndSplitMesh` (ESI v2506
/// `meshRefinementBaffles.C:4927`, `snappyRefineDriver.C:2754`): intersected
/// faces become baffles (`getBafflePatches`), `regionSplit` labels the
/// face-connected regions, `findRegions` keeps those holding
/// `locationInMesh`, the rest is removed.
///
/// Returns `None` if `keep` is in no leaf. Not ported here: ~~the buffer layers
/// of `removeInsideCells` (`nBufferLayers`)~~ **UPDATED 2026-11-12:** the buffer
/// layers are in [`crate::snappy_hex_mesh::esi_split::remove_inside_cells`];
/// still not ported: `locationsOutsideMesh`, more than one `locationsInMesh`,
/// faceZones/cellZones (gh:#1466).
pub(crate) fn baffle_region(
    leaves: &[Leaf],
    n0: [usize; 3],
    origin: Vector3,
    size0: [f64; 3],
    surface: &TriangleSoup,
    keep: Vector3,
) -> Option<Vec<bool>> {
    let start_leaf = find_leaf(leaves, origin, size0, keep)?;
    let mut adjacent: Vec<Vec<usize>> = vec![Vec::new(); leaves.len()];
    for (own, nei, a, b) in face_pairs(leaves, n0, origin, size0) {
        if let Some(n) = nei {
            if !surface.segment_hits(a, b) {
                adjacent[own].push(n);
                adjacent[n].push(own);
            }
        }
    }
    let mut reached = vec![false; leaves.len()];
    reached[start_leaf] = true;
    let mut stack = vec![start_leaf];
    while let Some(c) = stack.pop() {
        for &n in &adjacent[c] {
            if !reached[n] {
                reached[n] = true;
                stack.push(n);
            }
        }
    }
    Some(reached)
}

/// `hexRef8::consistentRefinement(cells, maxSet = true)`: raise `refine` until
/// no face joins cells whose post-refinement levels differ by more than one.
pub(crate) fn consistent_refinement(leaves: &[Leaf], pairs: &[FacePair], refine: &mut [bool]) {
    loop {
        let mut changed = 0usize;
        for p in pairs {
            let Some(n) = p.nei else { continue };
            let lo = leaves[p.own].level + usize::from(refine[p.own]);
            let ln = leaves[n].level + usize::from(refine[n]);
            if lo > ln + 1 {
                refine[n] = true;
                changed += 1;
            } else if ln > lo + 1 {
                refine[p.own] = true;
                changed += 1;
            }
        }
        if changed == 0 {
            break;
        }
    }
}

/// Replace every flagged leaf by its eight children.
fn split(leaves: &[Leaf], refine: &[bool]) -> Vec<Leaf> {
    let mut next = Vec::with_capacity(leaves.len() + 7 * refine.iter().filter(|&&r| r).count());
    for (id, l) in leaves.iter().enumerate() {
        if refine[id] {
            for di in 0..2 {
                for dj in 0..2 {
                    for dk in 0..2 {
                        next.push(Leaf {
                            level: l.level + 1,
                            i: 2 * l.i + di,
                            j: 2 * l.j + dj,
                            k: 2 * l.k + dk,
                        });
                    }
                }
            }
        } else {
            next.push(*l);
        }
    }
    next
}

#[cfg(test)]
/// Test-support: check the two invariants `surfaceOnlyRefine` leaves behind.
/// Returns `(n_unrefined_intersections, n_2to1_violations)`.
pub(crate) fn invariants(
    leaves: &[Leaf],
    n0: [usize; 3],
    origin: Vector3,
    size0: [f64; 3],
    surface_level: usize,
    surface: &TriangleSoup,
) -> (usize, usize) {
    let pairs = face_pairs(leaves, n0, origin, size0);
    let (mut unrefined, mut bad) = (0, 0);
    for (own, nei, s, e) in pairs {
        let min_level = match nei {
            Some(n) => leaves[own].level.min(leaves[n].level),
            None => leaves[own].level,
        };
        if min_level < surface_level && surface.segment_hits(s, e) {
            unrefined += 1;
        }
        if let Some(n) = nei {
            if leaves[own].level.abs_diff(leaves[n].level) > 1 {
                bad += 1;
            }
        }
    }
    (unrefined, bad)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn background(n: usize) -> Vec<Leaf> {
        let mut v = Vec::new();
        for i in 0..n {
            for j in 0..n {
                for k in 0..n {
                    v.push(Leaf { level: 0, i, j, k });
                }
            }
        }
        v
    }

    /// PREDICTIONS (written before the first run, 2026-11-11):
    /// 1. After `surface_only_refine` on a sphere (r = 0.3) in the unit cube,
    ///    8^3 background, level 2: zero face segments crossing the surface
    ///    join a cell below level 2, and zero faces differ by more than one
    ///    level (the two postconditions of the algorithm).
    /// 2. Cell counts: level-0 cells remain far from the sphere, so the leaf
    ///    count lies strictly between the background count (512) and the
    ///    uniformly refined count (512 * 64).
    #[test]
    fn sphere_postconditions_hold() {
        let surface = TriangleSoup::uv_sphere(Vector3::new(0.5, 0.5, 0.5), 0.3, 24, 24);
        let n0 = [8, 8, 8];
        let origin = Vector3::new(0.0, 0.0, 0.0);
        let size0 = [0.125; 3];
        let out = surface_only_refine(background(8), n0, origin, size0, 2, &surface, 0);
        let (unrefined, bad) = invariants(&out, n0, origin, size0, 2, &surface);
        assert_eq!(unrefined, 0);
        assert_eq!(bad, 0);
        assert!(out.len() > 512 && out.len() < 512 * 64, "{}", out.len());
        assert!(out.iter().any(|l| l.level == 2));
    }

    /// A surface far from every cell centre segment refines nothing.
    #[test]
    fn no_hit_no_refinement() {
        let surface = TriangleSoup::uv_sphere(Vector3::new(5.0, 5.0, 5.0), 0.3, 12, 12);
        let out = surface_only_refine(
            background(4),
            [4, 4, 4],
            Vector3::new(0.0, 0.0, 0.0),
            [0.25; 3],
            3,
            &surface,
            0,
        );
        assert_eq!(out.len(), 64);
    }

    /// The face enumeration lists each face of a uniform grid once:
    /// 3 n^2 (n-1) internal + 6 n^2 boundary.
    #[test]
    fn face_pairs_uniform_count() {
        let n = 3;
        let p = face_pairs(&background(n), [n; 3], Vector3::new(0.0, 0.0, 0.0), [1.0; 3]);
        assert_eq!(p.len(), 3 * n * n * (n - 1) + 6 * n * n);
    }

    /// End to end through `castellate`: the sphere case of the V&V record
    /// (`verification_and_validation/snappy_hex_mesh_1465/README.md`) gives
    /// the recorded counts, and the legacy rule's counts are unchanged.
    #[test]
    fn castellate_modes_recorded_counts() {
        use crate::snappy_hex_mesh::{castellate, BackgroundMesh, Bounds, CastellationControls};
        let surface = TriangleSoup::uv_sphere(Vector3::new(0.5, 0.5, 0.5), 0.3, 32, 32);
        let domain = Bounds::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 1.0, 1.0));
        let keep = Vector3::new(0.02, 0.02, 0.02);
        let band = CastellationControls::new(BackgroundMesh::uniform(domain, 8, 8, 8), 2, keep);
        let m = castellate(&surface, &band).unwrap();
        assert_eq!(m.cells_by_level, vec![376, 368, 2112]);
        let up = band.with_surface_intersection_refinement();
        let m = castellate(&surface, &up).unwrap();
        assert_eq!(m.cells_by_level, vec![328, 728, 2304]);
    }

    /// `refined_octree` reports the same leaves as `surface_only_refine` and a
    /// per-iteration trace that ends in 0 and sums to the cells split.
    #[test]
    fn refined_octree_trace_is_consistent() {
        use crate::snappy_hex_mesh::{BackgroundMesh, Bounds, CastellationControls};
        let surface = TriangleSoup::uv_sphere(Vector3::new(0.5, 0.5, 0.5), 0.3, 24, 24);
        let domain = Bounds::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 1.0, 1.0));
        let c = CastellationControls::new(
            BackgroundMesh::uniform(domain, 8, 8, 8),
            2,
            Vector3::new(0.02, 0.02, 0.02),
        );
        let (cells, sel) = refined_octree(&surface, &c, 100);
        let direct = surface_only_refine(background(8), [8; 3], Vector3::new(0.0, 0.0, 0.0), [0.125; 3], 2, &surface, 0);
        assert_eq!(cells.len(), direct.len());
        assert_eq!(*sel.last().unwrap(), 0);
        assert_eq!(512 + 7 * sel.iter().sum::<usize>(), cells.len());
    }

    /// gh:#1466: `baffle_region` keeps the cells reachable from the keep point
    /// across faces the surface does not cut. A sphere in a box, keep point
    /// outside: no kept cell has its centre inside, and cells inside are not
    /// reached; a keep point inside keeps only the inside; a point outside the
    /// box is `None`.
    #[test]
    fn baffle_region_separates_inside_from_outside() {
        let surface = TriangleSoup::uv_sphere(Vector3::new(0.5, 0.5, 0.5), 0.3, 24, 24);
        let n0 = [8, 8, 8];
        let origin = Vector3::new(0.0, 0.0, 0.0);
        let size0 = [0.125; 3];
        let leaves = surface_only_refine(background(8), n0, origin, size0, 2, &surface, 0);
        let outside = baffle_region(&leaves, n0, origin, size0, &surface, Vector3::new(0.02, 0.02, 0.02)).unwrap();
        let inside = baffle_region(&leaves, n0, origin, size0, &surface, Vector3::new(0.5, 0.5, 0.5)).unwrap();
        for (i, l) in leaves.iter().enumerate() {
            let deep_inside = surface.contains_point(centre(l, origin, size0)) && surface.distance_to(centre(l, origin, size0)) > 0.2;
            let deep_outside = !surface.contains_point(centre(l, origin, size0)) && surface.distance_to(centre(l, origin, size0)) > 0.2;
            if deep_inside {
                assert!(!outside[i] && inside[i]);
            }
            if deep_outside {
                assert!(outside[i] && !inside[i]);
            }
            assert!(!(outside[i] && inside[i]));
        }
        assert!(baffle_region(&leaves, n0, origin, size0, &surface, Vector3::new(5.0, 5.0, 5.0)).is_none());
    }
}
