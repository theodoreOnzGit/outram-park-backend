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
//! **Upstream followed:** OpenFOAM-dev (OpenFOAM Foundation) commit
//! `852527377c7e87e33d51b19ca8974b5db60af28b`:
//!
//! - `snappyHexMeshDriver/snappyRefineDriver.C:179` `surfaceOnlyRefine`
//!   (loop, stop rule `nCellsToRefine == 0 || (iter >= overallMaxLevel &&
//!   nCellsToRefine <= minRefineCells)`, `maxIter` 100);
//! - `meshRefinement/meshRefinementRefine.C:908` `markSurfaceRefinement`
//!   (one segment per face: owner centre to neighbour centre, or, on a
//!   non-coupled boundary face, owner centre to the face centre, from
//!   `meshRefinement.C:97` `calcNeighbourData`; segment extended by
//!   `rootSmall` of its length at each end; a cell is marked when the
//!   segment hits a surface whose level exceeds the cell's level and the
//!   face's lower cell level);
//! - `refinementSurfaces/refinementSurfaces.C:410` `findHigherIntersection`;
//! - `polyTopoChange/polyTopoChange/hexRef8/hexRef8.C:1426`
//!   `faceConsistentRefinement` and `:2127` `consistentRefinement` (face 2:1,
//!   `maxSet = true`).
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
//! faces here. Cell *removal* after refinement stays this port's inside test,
//! not upstream's baffle-and-split (gh:#1466).
//!
//! [`RefinementSelection::DistanceBand`]: crate::snappy_hex_mesh::castellation::RefinementSelection::DistanceBand

use std::collections::HashMap;

use outram_foam_basic_lib::primitives::Vector3;

use crate::snappy_hex_mesh::castellation::Leaf;
use crate::snappy_hex_mesh::stl::TriangleSoup;

/// OpenFOAM's `rootSmall`, `sqrt(small)` with `small = 2.2204e-16` double
/// (`doubleScalarRootSmall`, `OpenFOAM/primitives/Scalar/doubleScalar`).
const ROOT_SMALL: f64 = 1.490_116_119_384_765_6e-8;

/// One mesh face of the current octree: an owner leaf, an optional neighbour
/// leaf (`None` = domain boundary face) and the segment tested against the
/// surface.
#[derive(Debug, Clone, Copy)]
struct FacePair {
    own: usize,
    nei: Option<usize>,
    start: Vector3,
    end: Vector3,
}

/// Centre of `leaf` `[m]` on a background of `size0` level-0 cell sizes.
fn centre(leaf: &Leaf, origin: Vector3, size0: [f64; 3]) -> Vector3 {
    let f = 1.0 / (1usize << leaf.level) as f64;
    Vector3::new(
        origin.x + (leaf.i as f64 + 0.5) * size0[0] * f,
        origin.y + (leaf.j as f64 + 0.5) * size0[1] * f,
        origin.z + (leaf.k as f64 + 0.5) * size0[2] * f,
    )
}

/// Enumerate the faces of the octree as owner/neighbour leaf pairs, with their
/// `calcNeighbourData` segments (already extended by `rootSmall`).
///
/// A face between two leaves of different level is listed once per finer-side
/// leaf, exactly as the split-hex mesh lists it. Equal-level pairs are listed
/// from the lower-index side along the positive axis only.
pub(crate) fn face_pairs(
    leaves: &[Leaf],
    n0: [usize; 3],
    origin: Vector3,
    size0: [f64; 3],
) -> Vec<(usize, Option<usize>, Vector3, Vector3)> {
    let mut map: HashMap<(usize, usize, usize, usize), usize> = HashMap::with_capacity(leaves.len());
    for (id, l) in leaves.iter().enumerate() {
        map.insert((l.level, l.i, l.j, l.k), id);
    }
    let mut out = Vec::new();
    for (id, l) in leaves.iter().enumerate() {
        let c = centre(l, origin, size0);
        let half = [
            0.5 * size0[0] / (1usize << l.level) as f64,
            0.5 * size0[1] / (1usize << l.level) as f64,
            0.5 * size0[2] / (1usize << l.level) as f64,
        ];
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
                let (start, end, nei) = if outside {
                    // Non-coupled boundary face: neiCc is the face centre.
                    let mut fc = c;
                    let d = if positive { half[axis] } else { -half[axis] };
                    match axis {
                        0 => fc.x += d,
                        1 => fc.y += d,
                        _ => fc.z += d,
                    }
                    (c, fc, None)
                } else {
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
                    (c, centre(&leaves[nid], origin, size0), Some(nid))
                };
                let sv = (end - start) * ROOT_SMALL;
                out.push((id, nei, start - sv, end + sv));
            }
        }
    }
    out
}

/// Run `surfaceOnlyRefine` on the octree `leaves` (all level 0 on entry or any
/// 2:1-consistent octree) and return the refined leaves.
///
/// `n0` is the level-0 cell count per axis, `size0` the level-0 cell size,
/// `surface_level` the surface's (min = max) refinement level, and
/// `min_refine_cells` upstream's `minRefinementCells`.
pub(crate) fn surface_only_refine(
    mut leaves: Vec<Leaf>,
    n0: [usize; 3],
    origin: Vector3,
    size0: [f64; 3],
    surface_level: usize,
    surface: &TriangleSoup,
    min_refine_cells: usize,
) -> Vec<Leaf> {
    const MAX_ITER: usize = 100;
    for iter in 0..MAX_ITER {
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
        if n_refine == 0 || (iter >= surface_level && n_refine <= min_refine_cells) {
            break;
        }
        leaves = split(&leaves, &refine);
    }
    leaves
}

/// `hexRef8::consistentRefinement(cells, maxSet = true)`: raise `refine` until
/// no face joins cells whose post-refinement levels differ by more than one.
fn consistent_refinement(leaves: &[Leaf], pairs: &[FacePair], refine: &mut [bool]) {
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
}
