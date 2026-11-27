// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam) and OpenFOAM-v2506 (ESI)
//   Upstream (GeN-Foam 652b3da): src/classes/common/mergeOrSplitBaffles/mergeOrSplitBaffles.C
//     `insertDuplicateMerge` (:63-135), `removeBaffles` (:255-, the `mergePatchIDs` branch), driven by
//     src/classes/multiRegion/meshHandler/meshHandler.C `createBaffleLessMeshes` (:389-506)
//   Upstream (OpenFOAM-v2506): src/meshTools/regionSplit/localPointRegion.C `isDuplicate` (:69-105), `findDuplicateFaces` (:543-621);
//     src/meshTools/polyTopoChange/polyTopoChange.C `getFaceOrder` (:745-890), `reorderCompactFaces` (:899-)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full notice.

//! # `removeBaffles`: merging the duplicate boundary faces of a region's mesh (gh:#1236, README round 78)
//!
//! [`merge_duplicate_faces`] is `removeBaffles(mesh, runTime)` of GeN-Foam's `mergeOrSplitBaffles.C` with `split = 0`, `detectOnly = 0`,
//! `overwrite = 1` (the way `createBaffleLessMeshes` calls it), on the topology only (no fields: the port keeps the baffle-less fields as copies
//! of the region's own, README round 74):
//!
//! 1. `findDuplicateFaces(mesh, boundaryFaces)` with `boundaryFaces` every boundary face in patch order (`patchFaces(mesh, identity(patches))`):
//!    for each boundary face `f`, the faces of its first point's `pointFaces` with a larger index are tested with `isDuplicate`; the SAME point
//!    order is a fatal error, the REVERSED one a duplicate, a face already paired is a fatal error;
//! 2. `insertDuplicateMerge`: for each pair, `own0 < own1` keeps `face0`'s vertex list with owner `own0` and neighbour `own1`, otherwise `face1`'s
//!    with owner `own1` and neighbour `own0`; the other face is removed;
//! 3. `polyTopoChange::changeMesh(mesh, false)` (no inflation, no cell or point ordering): removed faces are compacted away in index order, then
//!    `getFaceOrder` numbers the internal faces cell by cell (faces to a HIGHER-numbered neighbour, sorted by that neighbour, ties in face-index
//!    order) and the boundary faces patch by patch in their old order. The faces keep their vertex lists, the cells and the points stay as
//!    they were.
//!
//! **Refused by name, not ported:** a face zone (the `faceZones` flip bookkeeping), two faces of a duplicate pair on the same cell, a pair on a
//! `processor` patch (the port has none), and every case of `split`/`detectOnly`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use crate::hash::HashMap;

/// The topology after `removeBaffles`: faces in `changeMesh` order and the patch sizes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergedFaces {
    /// Every face's point labels (internal faces first, then the patches in order).
    pub faces: Vec<Vec<usize>>,
    /// `owner` of every face.
    pub owner: Vec<usize>,
    /// `neighbour` of the internal faces.
    pub neighbour: Vec<usize>,
    /// The number of faces left in each patch, in the patch order of the input.
    pub patch_sizes: Vec<usize>,
    /// The number of boundary faces that were in a duplicate pair (`Writing 21252 duplicate faces to faceSet`): twice the faces merged.
    pub n_duplicate_faces: usize,
}

const NONE: usize = usize::MAX;

/// `localPointRegion::isDuplicate(f0, f1, forward)`: equal faces read forwards or backwards from `f0[0]`'s place in `f1`.
fn is_duplicate(f0: &[usize], f1: &[usize], forward: bool) -> bool {
    if f0.len() != f1.len() {
        return false;
    }
    let n = f0.len();
    let Some(mut fp1) = f1.iter().position(|&p| p == f0[0]) else { return false };
    for &p0 in f0 {
        if p0 != f1[fp1] {
            return false;
        }
        fp1 = if forward { (fp1 + 1) % n } else { (fp1 + n - 1) % n };
    }
    true
}

/// `localPointRegion::findDuplicateFaces` over all boundary faces: `result[b]` is the boundary-face index paired with `b`, or `usize::MAX`.
///
/// # Errors
/// The two `FatalError`s of upstream: a pair in the same point order, and a face in more than one pair.
fn find_duplicate_faces(boundary: &[&[usize]]) -> Result<Vec<usize>, String> {
    let mut point_faces: HashMap<usize, Vec<usize>> = HashMap::default();
    for (b, f) in boundary.iter().enumerate() {
        for &p in *f {
            point_faces.entry(p).or_default().push(b);
        }
    }
    let mut dup = vec![NONE; boundary.len()];
    for (b, f) in boundary.iter().enumerate() {
        let Some(list) = point_faces.get(&f[0]) else { continue };
        for &other in list {
            if other > b {
                let of = boundary[other];
                if is_duplicate(f, of, true) {
                    return Err(format!("boundary faces {b} and {other} have the same points in the same order (upstream: FatalError)"));
                } else if is_duplicate(f, of, false) {
                    if dup[b] != NONE || dup[other] != NONE {
                        return Err(format!("boundary face {b} or {other} is already in a duplicate pair: three or more faces share the same points (upstream: FatalError)"));
                    }
                    dup[b] = other;
                    dup[other] = b;
                }
            }
        }
    }
    Ok(dup)
}

/// `removeBaffles` on the topology: see the module documentation.
///
/// `faces`, `owner` (all faces) and `neighbour` (internal faces) are the mesh as read; `patch_sizes` the sizes of its patches in order (they cover
/// the boundary faces); `n_cells` the cell count.
///
/// # Errors
/// A text naming what is refused or what upstream aborts on (see the module documentation).
pub fn merge_duplicate_faces(
    faces: &[Vec<usize>],
    owner: &[usize],
    neighbour: &[usize],
    n_cells: usize,
    patch_sizes: &[usize],
) -> Result<MergedFaces, String> {
    let n_int = neighbour.len();
    let n_faces = faces.len();
    if owner.len() != n_faces || n_int > n_faces || patch_sizes.iter().sum::<usize>() != n_faces - n_int {
        return Err("faces, owner, neighbour and the patch sizes disagree".to_string());
    }
    let boundary: Vec<&[usize]> = faces[n_int..].iter().map(Vec::as_slice).collect();
    let dup = find_duplicate_faces(&boundary)?;
    // `insertDuplicateMerge`: per face, whether it stays, its (owner, neighbour) and which vertex list is used
    let mut removed = vec![false; n_faces];
    let mut new_owner = owner.to_vec();
    let mut new_neighbour: Vec<usize> = (0..n_faces).map(|f| if f < n_int { neighbour[f] } else { NONE }).collect();
    let mut n_duplicate_faces = 0;
    for b in 0..boundary.len() {
        let other = dup[b];
        if other != NONE && other > b {
            let (face0, face1) = (n_int + b, n_int + other);
            let (own0, own1) = (owner[face0], owner[face1]);
            if own0 == own1 {
                return Err(format!("duplicate faces {face0} and {face1} are both owned by cell {own0}"));
            }
            let (keep, drop, own, nbr) = if own0 < own1 { (face0, face1, own0, own1) } else { (face1, face0, own1, own0) };
            removed[drop] = true;
            new_owner[keep] = own;
            new_neighbour[keep] = nbr;
            n_duplicate_faces += 2;
        }
    }
    // the patch of each old boundary face
    let mut patch_of = vec![NONE; n_faces];
    let mut at = n_int;
    for (pi, &sz) in patch_sizes.iter().enumerate() {
        for f in at..at + sz {
            patch_of[f] = pi;
        }
        at += sz;
    }
    // compaction (`reorderCompactFaces` after `compact`): the surviving faces in old index order
    let alive: Vec<usize> = (0..n_faces).filter(|&f| !removed[f]).collect();
    // a face is internal when it now has a neighbour; a merged boundary face loses its patch
    let internal = |f: usize| new_neighbour[f] != NONE;
    // `getFaceOrder`: internal faces cell by cell; cellFaces lists faces in compacted order
    let mut cell_faces: Vec<Vec<usize>> = vec![Vec::new(); n_cells];
    for &f in &alive {
        cell_faces[new_owner[f]].push(f);
        if internal(f) {
            cell_faces[new_neighbour[f]].push(f);
        }
    }
    let mut order: Vec<usize> = Vec::with_capacity(alive.len());
    for (celli, list) in cell_faces.iter().enumerate() {
        let mut mine: Vec<(usize, usize)> = Vec::new();
        for &f in list {
            if internal(f) {
                let nbr_cell = if new_neighbour[f] == celli { new_owner[f] } else { new_neighbour[f] };
                if celli < nbr_cell {
                    mine.push((nbr_cell, f));
                }
            }
        }
        mine.sort_by_key(|&(nbr_cell, _)| nbr_cell); // stable: ties stay in cellFaces order
        order.extend(mine.into_iter().map(|(_, f)| f));
    }
    let n_internal_new = order.len();
    // boundary faces patch by patch, old order
    let mut new_sizes = vec![0usize; patch_sizes.len()];
    for pi in 0..patch_sizes.len() {
        for &f in &alive {
            if !internal(f) && patch_of[f] == pi {
                order.push(f);
                new_sizes[pi] += 1;
            }
        }
    }
    if order.len() != alive.len() {
        return Err("a surviving face got no position (a boundary face without a patch)".to_string());
    }
    let mut out_faces = Vec::with_capacity(order.len());
    let mut out_owner = Vec::with_capacity(order.len());
    for &f in &order {
        out_faces.push(faces[f].clone());
        out_owner.push(new_owner[f]);
    }
    let out_neighbour: Vec<usize> = order[..n_internal_new].iter().map(|&f| new_neighbour[f]).collect();
    Ok(MergedFaces { faces: out_faces, owner: out_owner, neighbour: out_neighbour, patch_sizes: new_sizes, n_duplicate_faces })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two cells side by side, the wall between them a baffle: cell 0 owns `[1,2,3,4]` (patch `b0`), cell 1 owns `[4,3,2,1]` (patch `b1`);
    /// one ordinary wall face per cell (patch `w`). The two faces merge into ONE internal face, `b0`/`b1` end empty, `w` keeps both.
    #[test]
    fn two_duplicate_faces_merge_into_one_internal_face_and_the_patches_empty() {
        let faces = vec![vec![1, 2, 3, 4], vec![4, 3, 2, 1], vec![10, 11, 12, 13], vec![20, 21, 22, 23]];
        let owner = vec![0, 1, 0, 1];
        let m = merge_duplicate_faces(&faces, &owner, &[], 2, &[1, 1, 2]).unwrap();
        assert_eq!(m.n_duplicate_faces, 2);
        assert_eq!(m.patch_sizes, vec![0, 0, 2]);
        assert_eq!(m.neighbour, vec![1]);
        assert_eq!(m.owner, vec![0, 0, 1]);
        // `own0 < own1`: face 0's vertex list is kept
        assert_eq!(m.faces[0], vec![1, 2, 3, 4]);
        assert_eq!(m.faces[1], vec![10, 11, 12, 13]);
    }

    /// The face of the HIGHER-numbered owner is kept when it is `face0` (`own0 > own1`), and the internal faces are renumbered upper-triangularly:
    /// an existing internal face `0-2` and the merged `0-1` swap places.
    #[test]
    fn the_merged_face_takes_its_upper_triangular_place_and_the_lower_owners_vertices() {
        // cells 0,1,2; internal face 0 between cells 0 and 2; baffle between cells 0 (patch b1's face) and 1 (patch b0's face, owner 1)
        let faces = vec![vec![50, 51, 52, 53], vec![1, 2, 3, 4], vec![4, 3, 2, 1], vec![60, 61, 62, 63]];
        let owner = vec![0, 1, 0, 2];
        // face 1 (patch b0) is owned by cell 1, face 2 (patch b1) by cell 0: own0 = 1 > own1 = 0, so face 2's list is kept
        let m = merge_duplicate_faces(&faces, &owner, &[2], 3, &[1, 1, 1]).unwrap();
        assert_eq!(m.neighbour, vec![1, 2]);
        assert_eq!(m.owner[..2], [0, 0]);
        assert_eq!(m.faces[0], vec![4, 3, 2, 1]);
        assert_eq!(m.faces[1], vec![50, 51, 52, 53]);
        assert_eq!(m.patch_sizes, vec![0, 0, 1]);
        assert_eq!(m.faces[2], vec![60, 61, 62, 63]);
    }

    /// The two fatal errors of `findDuplicateFaces` and the same-cell refusal are errors, not silent merges.
    #[test]
    fn the_upstream_fatal_errors_and_the_same_cell_pair_are_refused() {
        let same_order = vec![vec![1, 2, 3, 4], vec![1, 2, 3, 4]];
        assert!(merge_duplicate_faces(&same_order, &[0, 1], &[], 2, &[2]).unwrap_err().contains("same order"));
        let three = vec![vec![1, 2, 3, 4], vec![4, 3, 2, 1], vec![3, 2, 1, 4]];
        assert!(merge_duplicate_faces(&three, &[0, 1, 2], &[], 3, &[3]).is_err());
        let one_cell = vec![vec![1, 2, 3, 4], vec![4, 3, 2, 1]];
        assert!(merge_duplicate_faces(&one_cell, &[0, 0], &[], 1, &[2]).unwrap_err().contains("owned by cell"));
        assert!(merge_duplicate_faces(&one_cell, &[0, 0], &[], 1, &[1]).is_err());
    }

    /// A mesh with no duplicates is only renumbered into `changeMesh` order: an out-of-order internal face list is sorted, boundary kept.
    #[test]
    fn a_mesh_without_duplicates_is_put_in_change_mesh_order() {
        let faces = vec![vec![1, 2, 3], vec![4, 5, 6], vec![7, 8, 9]];
        let owner = vec![1, 0, 0];
        // internal faces: 0 between (1,2); 1 between (0,2); boundary face 2 owned by cell 0
        let m = merge_duplicate_faces(&faces, &owner, &[2, 2], 3, &[1]).unwrap();
        assert_eq!(m.n_duplicate_faces, 0);
        assert_eq!(m.owner[..2], [0, 1]);
        assert_eq!(m.neighbour, vec![2, 2]);
        assert_eq!(m.faces[0], vec![4, 5, 6]);
        assert_eq!(m.faces[1], vec![1, 2, 3]);
        assert_eq!(m.faces[2], vec![7, 8, 9]);
    }
}
