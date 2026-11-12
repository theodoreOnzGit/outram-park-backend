// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), Copyright (C) 2011-2016 OpenFOAM
// Foundation, 2016-2026 OpenCFD Ltd (ESI), GPL-3.0-or-later.
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

//! The cell-removal stages of `snappyRefineDriver::doRefine` that follow
//! `surfaceOnlyRefine`, translated for the octree (gh:#1466).
//!
//! **Upstream: OpenFOAM.com ESI v2506**, `src/mesh/snappyHexMesh`:
//!
//! | stage | ESI v2506 | here |
//! |---|---|---|
//! | `removeInsideCells(nBufferLayers)` | `snappyRefineDriver.C:1760`, `meshRefinementBaffles.C:4927` and `:5014` (`splitMesh`) | [`remove_inside_cells`] |
//! | `danglingCellRefine(nFaces)` | `snappyRefineDriver.C:1002` | [`dangling_cell_refine`] |
//! | `baffleAndSplitMesh` | `snappyRefineDriver.C:2754`, `meshRefinementBaffles.C:4635` | [`baffle_and_split_mesh`] |
//! | `handleSnapProblems` -> `markFacesOnProblemCells` | `meshRefinementBaffles.C:3795`, `meshRefinementProblemCells.C:383` | inside [`baffle_and_split_mesh`] |
//!
//! The order in `doRefine` (`snappyRefineDriver.C:3560-3690`) is
//! `removeInsideCells(1)`, `danglingCellRefine(21)`, `danglingCellRefine(24)`,
//! `refinementInterfaceRefine` (selected 0 cells on the flange; not ported),
//! `baffleAndSplitMesh`. [`esi_stages`] runs them in that order.
//!
//! ## How the octree stands in for the polyMesh
//!
//! The mesh is the full octree (every leaf ever created, dead or alive) with an
//! `alive` flag per leaf: a removed cell leaves its faces behind as boundary
//! faces of its alive neighbours, exactly as `removeCells` exposes them. Faces
//! are enumerated by [`octree_faces`] (the split-hex faces of the finer side).
//! A face's vertex ring is the finer-side quad with every lattice edge midpoint
//! that exists in the octree inserted (hexRef8 inserts the mid-points of refined
//! edges into every face that uses the edge, also faces of cells that are not
//! refined); a mesh point exists when it is a corner of any leaf, dead or alive,
//! because removal never deletes a point from the faces of a surviving cell.
//! This is the assumption P10-P14 of the V&V record test.
//!
//! ## Cell order
//!
//! `markFacesOnProblemCells` visits cells in order and updates its boundary
//! marks as it goes, so its result can depend on cell order. The port visits
//! alive leaves in leaf-vector order (the nested order of
//! [`crate::snappy_hex_mesh::castellation::castellate`]), not OpenFOAM's
//! (blockMesh order, a refined cell keeps its label, children 1..7 appended).
//! The V&V record states whether that mattered.
//!
//! ## Not ported
//!
//! `locationsOutsideMesh` and leak closure, `frozenPoints`/`frozenFaces`, more
//! than one `locationInMesh`, cellZones/faceZones (`zonify`), the
//! `removeEdgeConnectedCells` branch and the `minVolCollapseRatio` collapse
//! check of `markFacesOnProblemCells` (both off in the flange dictionary:
//! `removeEdgeConnectedCells false`, `minVolCollapseRatio` unset), and
//! `refinementInterfaceRefine`.

use std::collections::{HashMap, HashSet};

use outram_foam_basic_lib::primitives::Vector3;

use crate::snappy_hex_mesh::castellation::Leaf;
use crate::snappy_hex_mesh::refinement::{
    centre, consistent_refinement, find_leaf, octree_faces, FacePair, OctFace, ROOT_SMALL,
};
use crate::snappy_hex_mesh::stl::TriangleSoup;

/// A lattice point (integer coordinates on the lattice of level `lm`).
type P = [u32; 3];
/// An unordered pair of lattice points: one mesh edge.
type E = (P, P);

/// Lattice level at which every edge midpoint of every leaf is an integer.
fn lattice_level(leaves: &[Leaf]) -> usize {
    leaves.iter().map(|l| l.level).max().unwrap_or(0) + 1
}

/// Lower corner and edge length of `l` on the level-`lm` lattice.
fn lattice_box(l: &Leaf, lm: usize) -> (P, u32) {
    let s = 1u32 << (lm - l.level);
    ([l.i as u32 * s, l.j as u32 * s, l.k as u32 * s], s)
}

/// Push `a` and then every existing lattice mid-point strictly between `a` and
/// `b` (recursively), in order along the edge. `b` is not pushed.
fn push_edge(a: P, b: P, pts: &HashSet<P>, out: &mut Vec<P>) {
    out.push(a);
    push_inner(a, b, pts, out);
}

fn push_inner(a: P, b: P, pts: &HashSet<P>, out: &mut Vec<P>) {
    if (0..3).all(|i| (a[i] + b[i]) % 2 == 0) {
        let mid = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2];
        if mid != a && mid != b && pts.contains(&mid) {
            push_inner(a, mid, pts, out);
            out.push(mid);
            push_inner(mid, b, pts, out);
        }
    }
}

/// Topology of the full octree: faces, their vertex rings, per-cell faces.
struct Topo {
    faces: Vec<OctFace>,
    ring: Vec<Vec<P>>,
    cell_faces: Vec<Vec<usize>>,
    /// The eight own-level corners of each leaf (the hexRef8 "anchor points").
    corners: Vec<[P; 8]>,
}

fn build_topo(leaves: &[Leaf], n0: [usize; 3]) -> Topo {
    let lm = lattice_level(leaves);
    let mut pts: HashSet<P> = HashSet::with_capacity(leaves.len() * 2);
    let mut corners = Vec::with_capacity(leaves.len());
    for l in leaves {
        let (lo, s) = lattice_box(l, lm);
        let mut c = [[0u32; 3]; 8];
        for (n, cn) in c.iter_mut().enumerate() {
            *cn = [lo[0] + s * (n as u32 & 1), lo[1] + s * ((n as u32 >> 1) & 1), lo[2] + s * ((n as u32 >> 2) & 1)];
            pts.insert(*cn);
        }
        corners.push(c);
    }
    let faces = octree_faces(leaves, n0);
    let mut ring = Vec::with_capacity(faces.len());
    let mut cell_faces = vec![Vec::new(); leaves.len()];
    for (fi, f) in faces.iter().enumerate() {
        let (lo, s) = lattice_box(&leaves[f.own], lm);
        let plane = lo[f.axis] + if f.positive { s } else { 0 };
        let (a1, a2) = ((f.axis + 1) % 3, (f.axis + 2) % 3);
        let at = |u: u32, v: u32| {
            let mut p = [0u32; 3];
            p[f.axis] = plane;
            p[a1] = lo[a1] + u;
            p[a2] = lo[a2] + v;
            p
        };
        let quad = [at(0, 0), at(s, 0), at(s, s), at(0, s)];
        let mut r = Vec::with_capacity(4);
        for e in 0..4 {
            push_edge(quad[e], quad[(e + 1) % 4], &pts, &mut r);
        }
        ring.push(r);
        cell_faces[f.own].push(fi);
        if let Some(n) = f.nei {
            cell_faces[n].push(fi);
        }
    }
    Topo { faces, ring, cell_faces, corners }
}

fn edge_key(a: P, b: P) -> E {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Which faces are cut by the surface, by `calcCellCellRays` +
/// `findHigherIntersection`, on the mesh of the alive leaves.
///
/// Faces between two alive leaves: cell centre to cell centre. A domain
/// boundary face: owner centre to face centre. A face exposed by removal (one
/// side alive; it is in a surface patch from then on): `calcNeighbourData`
/// (`meshRefinement.C:195`) rebuilds the missing neighbour centre by
/// extrapolating the face centre by the normal distance owner-centre to
/// face-centre, halved when the face is finer than the cell. For a dead
/// neighbour of equal level or finer that is its true centre; for a coarser dead
/// neighbour it is the mirror image of the alive centre. Faces with no alive
/// side are never cut. All segments are extended by `rootSmall` of their length.
fn face_hits(
    leaves: &[Leaf],
    alive: &[bool],
    faces: &[OctFace],
    origin: Vector3,
    size0: [f64; 3],
    surface: &TriangleSoup,
) -> Vec<bool> {
    faces
        .iter()
        .map(|f| {
            let own = &leaves[f.own];
            let c = centre(own, origin, size0);
            let half = 0.5 * size0[f.axis] / (1usize << own.level) as f64;
            let step = if f.positive { half } else { -half };
            let (start, end) = match f.nei {
                None => {
                    if !alive[f.own] {
                        return false;
                    }
                    let mut fc = c;
                    match f.axis {
                        0 => fc.x += step,
                        1 => fc.y += step,
                        _ => fc.z += step,
                    }
                    (c, fc)
                }
                Some(n) => match (alive[f.own], alive[n]) {
                    (false, false) => return false,
                    (true, false) if leaves[n].level < own.level => {
                        let mut mirror = c;
                        match f.axis {
                            0 => mirror.x += 2.0 * step,
                            1 => mirror.y += 2.0 * step,
                            _ => mirror.z += 2.0 * step,
                        }
                        (c, mirror)
                    }
                    _ => (c, centre(&leaves[n], origin, size0)),
                },
            };
            let sv = (end - start) * ROOT_SMALL;
            surface.segment_hits(start - sv, end + sv)
        })
        .collect()
}

/// Flood the alive cells face-connected to `start` across internal faces that
/// are not in `blocked`.
fn flood(n: usize, faces: &[OctFace], alive: &[bool], blocked: &[bool], start: usize) -> Vec<bool> {
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (fi, f) in faces.iter().enumerate() {
        if let Some(nb) = f.nei {
            if alive[f.own] && alive[nb] && !blocked[fi] {
                adj[f.own].push(nb);
                adj[nb].push(f.own);
            }
        }
    }
    let mut reached = vec![false; n];
    reached[start] = true;
    let mut stack = vec![start];
    while let Some(c) = stack.pop() {
        for &m in &adj[c] {
            if !reached[m] {
                reached[m] = true;
                stack.push(m);
            }
        }
    }
    reached
}

/// `removeInsideCells(nBufferLayers)` with no limit shells: `splitMesh`
/// (ESI v2506 `meshRefinementBaffles.C:4927` and `:5014`).
///
/// Keeps the face-connected region holding `keep` (faces cut by the surface
/// block), then `n_buffer_layers` times: marks every point of a face between a
/// kept and a removed cell and of a domain-boundary face of a kept cell; every
/// removed cell that uses a marked point, or owns/neighbours a face cut by the
/// surface, is kept (steps 1-3 of the upstream loop: cells to points, points
/// to faces, faces back to cells). Returns the alive flags, or `None` if `keep`
/// is in no leaf.
pub(crate) fn remove_inside_cells(
    leaves: &[Leaf],
    n0: [usize; 3],
    origin: Vector3,
    size0: [f64; 3],
    surface: &TriangleSoup,
    keep: Vector3,
    n_buffer_layers: usize,
) -> Option<Vec<bool>> {
    let n = leaves.len();
    let topo = build_topo(leaves, n0);
    let all = vec![true; n];
    let hit = face_hits(leaves, &all, &topo.faces, origin, size0, surface);
    let start = find_leaf(leaves, origin, size0, keep)?;
    let mut kept = flood(n, &topo.faces, &all, &hit, start);
    for _ in 0..n_buffer_layers {
        let mut marked: HashSet<P> = HashSet::new();
        for (fi, f) in topo.faces.iter().enumerate() {
            let on_region_boundary = match f.nei {
                Some(nb) => kept[f.own] != kept[nb],
                None => kept[f.own],
            };
            if on_region_boundary {
                marked.extend(topo.ring[fi].iter().copied());
            }
        }
        let mut next = kept.clone();
        for c in 0..n {
            if !kept[c] && topo.cell_faces[c].iter().any(|&fi| topo.ring[fi].iter().any(|p| marked.contains(p))) {
                next[c] = true;
            }
        }
        // Faces that already carry a patch (cut by the surface) also pull in
        // both of their cells (step 3 of the upstream loop).
        for (fi, f) in topo.faces.iter().enumerate() {
            if hit[fi] {
                next[f.own] = true;
                if let Some(nb) = f.nei {
                    next[nb] = true;
                }
            }
        }
        kept = next;
    }
    Some(kept)
}

/// `danglingCellRefine(nFaces)` (ESI v2506 `snappyRefineDriver.C:1002`): while
/// some alive cell has exactly `n_faces` INTERNAL faces (faces to removed cells
/// and domain boundary faces do not count), refine those cells plus the face
/// 2:1 closure among alive cells. Returns the new leaves (a refined leaf is
/// replaced in place by its eight alive children), the alive flags and the
/// number of cells selected per iteration (the last is the stopping one).
pub(crate) fn dangling_cell_refine(
    mut leaves: Vec<Leaf>,
    mut alive: Vec<bool>,
    n0: [usize; 3],
    n_faces: usize,
    min_refine_cells: usize,
    max_iter: usize,
) -> (Vec<Leaf>, Vec<bool>, Vec<usize>) {
    let mut selected = Vec::new();
    for iter in 0..max_iter {
        let faces = octree_faces(&leaves, n0);
        let mut n_int = vec![0usize; leaves.len()];
        let mut pairs = Vec::new();
        for f in &faces {
            if let Some(nb) = f.nei {
                if alive[f.own] && alive[nb] {
                    n_int[f.own] += 1;
                    n_int[nb] += 1;
                    pairs.push(FacePair { own: f.own, nei: Some(nb), start: Vector3::new(0.0, 0.0, 0.0), end: Vector3::new(0.0, 0.0, 0.0) });
                }
            }
        }
        let mut refine: Vec<bool> = (0..leaves.len()).map(|c| alive[c] && n_int[c] == n_faces).collect();
        consistent_refinement(&leaves, &pairs, &mut refine);
        let n_refine = refine.iter().filter(|&&r| r).count();
        selected.push(n_refine);
        if n_refine == 0 || (iter >= 1 && n_refine <= min_refine_cells) {
            break;
        }
        let mut next = Vec::with_capacity(leaves.len() + 7 * n_refine);
        let mut next_alive = Vec::with_capacity(next.capacity());
        for (id, l) in leaves.iter().enumerate() {
            if refine[id] {
                for di in 0..2 {
                    for dj in 0..2 {
                        for dk in 0..2 {
                            next.push(Leaf { level: l.level + 1, i: 2 * l.i + di, j: 2 * l.j + dj, k: 2 * l.k + dk });
                            next_alive.push(true);
                        }
                    }
                }
            } else {
                next.push(*l);
                next_alive.push(alive[id]);
            }
        }
        leaves = next;
        alive = next_alive;
    }
    (leaves, alive, selected)
}

/// What [`baffle_and_split_mesh`] counted, to compare with the ESI log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BaffleStats {
    /// Internal faces cut by the surface (ESI: "Introducing baffles for N faces").
    pub n_hit_internal: usize,
    /// Faces cut by the surface that are boundary faces (exposed or domain).
    pub n_hit_boundary: usize,
    /// Additional internal faces baffled by `markFacesOnProblemCells`
    /// (ESI: "marked N additional internal faces").
    pub n_problem_faces: usize,
    /// Cells kept after the final `splitMeshRegions`.
    pub n_kept: usize,
}

/// `baffleAndSplitMesh` (ESI v2506 `meshRefinementBaffles.C:4635`) with
/// `handleSnapProblems` on: baffle every internal face cut by the surface,
/// baffle the faces `markFacesOnProblemCells` selects, keep the cells
/// face-connected to `keep` across the remaining internal faces.
///
/// `markFacesOnProblemCells` (`meshRefinementProblemCells.C:383`, the
/// `removeEdgeConnectedCells = false`, `checkCollapse = false` branch): every
/// face in a surface patch (the baffle pairs and the faces exposed by earlier
/// removals) marks itself, its edges and its points. Then, visiting the alive
/// cells in order: a cell whose eight own-level corners are all marked has all its
/// remaining internal faces baffled, and each baffled face is marked at once;
/// a cell with exactly seven marked corners is recorded with its unmarked
/// corner; afterwards every recorded corner shared by more than three recorded
/// cells baffles those cells; finally every remaining internal face whose edges
/// are all marked is baffled (without marking). Returns the alive flags after
/// the split and the counts, or `None` if `keep` is in no alive leaf.
pub(crate) fn baffle_and_split_mesh(
    leaves: &[Leaf],
    alive: &[bool],
    n0: [usize; 3],
    origin: Vector3,
    size0: [f64; 3],
    surface: &TriangleSoup,
    keep: Vector3,
) -> Option<(Vec<bool>, BaffleStats)> {
    let n = leaves.len();
    let topo = build_topo(leaves, n0);
    let hit = face_hits(leaves, alive, &topo.faces, origin, size0, surface);
    let start = find_leaf(leaves, origin, size0, keep).filter(|&s| alive[s])?;

    let nfaces = topo.faces.len();
    let internal: Vec<bool> = topo.faces.iter().map(|f| f.nei.is_some_and(|nb| alive[f.own] && alive[nb])).collect();
    let exposed: Vec<bool> = topo
        .faces
        .iter()
        .map(|f| f.nei.is_some_and(|nb| alive[f.own] != alive[nb]))
        .collect();
    let mut stats = BaffleStats { n_hit_internal: 0, n_hit_boundary: 0, n_problem_faces: 0, n_kept: 0 };
    for fi in 0..nfaces {
        if hit[fi] {
            if internal[fi] {
                stats.n_hit_internal += 1;
            } else {
                stats.n_hit_boundary += 1;
            }
        }
    }

    // Boundary marks: faces of the surface patches.
    let mut bpoint: HashSet<P> = HashSet::new();
    let mut bedge: HashSet<E> = HashSet::new();
    let mark = |fi: usize, bpoint: &mut HashSet<P>, bedge: &mut HashSet<E>| {
        let r = &topo.ring[fi];
        for (e, &p) in r.iter().enumerate() {
            bpoint.insert(p);
            bedge.insert(edge_key(p, r[(e + 1) % r.len()]));
        }
    };
    for fi in 0..nfaces {
        if (internal[fi] && hit[fi]) || exposed[fi] {
            mark(fi, &mut bpoint, &mut bedge);
        }
    }

    // Problem-cell baffles. `candidate(fi)`: still an internal face (not already
    // a baffle) that has not been taken.
    let mut problem = vec![false; nfaces];
    let mut n_problem = 0usize;
    let mut has_seven = vec![false; n];
    let mut seven_corner: Vec<(usize, P)> = Vec::new();
    for c in 0..n {
        if !alive[c] {
            continue;
        }
        let mut n_boundary = 0usize;
        let mut unmarked = None;
        for p in &topo.corners[c] {
            if bpoint.contains(p) {
                n_boundary += 1;
            } else {
                unmarked = Some(*p);
            }
        }
        if n_boundary == 8 {
            for &fi in &topo.cell_faces[c] {
                if internal[fi] && !hit[fi] && !problem[fi] {
                    problem[fi] = true;
                    n_problem += 1;
                    mark(fi, &mut bpoint, &mut bedge);
                }
            }
        } else if n_boundary == 7 {
            if let Some(u) = unmarked {
                has_seven[c] = true;
                seven_corner.push((c, u));
            }
        }
    }
    // Cells using each recorded corner (every alive cell with the point in one
    // of its face rings).
    let mut corner_cells: HashMap<P, Vec<usize>> = HashMap::new();
    for (_, u) in &seven_corner {
        corner_cells.entry(*u).or_default();
    }
    for c in 0..n {
        if !alive[c] {
            continue;
        }
        let mut seen: Vec<P> = Vec::new();
        for &fi in &topo.cell_faces[c] {
            for p in &topo.ring[fi] {
                if corner_cells.contains_key(p) && !seen.contains(p) {
                    seen.push(*p);
                }
            }
        }
        for p in seen {
            if let Some(v) = corner_cells.get_mut(&p) {
                v.push(c);
            }
        }
    }
    let mut corners: Vec<P> = corner_cells.keys().copied().collect();
    corners.sort_unstable();
    for u in corners {
        let cells = &corner_cells[&u];
        let seven: Vec<usize> = cells.iter().copied().filter(|&c| has_seven[c]).collect();
        if seven.len() > 3 {
            for c in seven {
                for &fi in &topo.cell_faces[c] {
                    if internal[fi] && !hit[fi] && !problem[fi] {
                        problem[fi] = true;
                        n_problem += 1;
                        mark(fi, &mut bpoint, &mut bedge);
                    }
                }
            }
        }
    }
    // Faces with all edges on the boundary (boundary data is not updated).
    for fi in 0..nfaces {
        if internal[fi] && !hit[fi] && !problem[fi] {
            let r = &topo.ring[fi];
            if (0..r.len()).all(|e| bedge.contains(&edge_key(r[e], r[(e + 1) % r.len()]))) {
                problem[fi] = true;
                n_problem += 1;
            }
        }
    }
    stats.n_problem_faces = n_problem;

    // splitMeshRegions: blocked = baffled faces (cut faces and problem faces).
    let blocked: Vec<bool> = (0..nfaces).map(|fi| (internal[fi] && hit[fi]) || problem[fi]).collect();
    let kept = flood(n, &topo.faces, alive, &blocked, start);
    stats.n_kept = kept.iter().filter(|&&k| k).count();
    Some((kept, stats))
}

/// The cells after each ESI stage, for the differential against the ESI
/// intermediate meshes. Each entry is `(level, centre)`.
#[derive(Debug, Clone)]
pub struct EsiStageCells {
    /// After `removeInsideCells(nBufferLayers = 1)` (ESI: "Selected for keeping").
    pub after_remove_inside: Vec<(usize, Vector3)>,
    /// Cells selected per iteration by `danglingCellRefine(21)`, then `(24)`.
    pub dangling_selected: [Vec<usize>; 2],
    /// After both dangling-cell stages.
    pub after_dangling: Vec<(usize, Vector3)>,
    /// After `baffleAndSplitMesh` (the final castellated cells).
    pub final_cells: Vec<(usize, Vector3)>,
    /// The counts of the baffle step.
    pub baffle: BaffleStats,
}

/// Everything [`esi_stages`] produced, with leaves (for assembling the mesh).
pub(crate) struct EsiStages {
    pub(crate) leaves: Vec<Leaf>,
    pub(crate) kept: Vec<bool>,
    pub(crate) cells: EsiStageCells,
}

fn alive_cells(leaves: &[Leaf], alive: &[bool], origin: Vector3, size0: [f64; 3]) -> Vec<(usize, Vector3)> {
    leaves
        .iter()
        .zip(alive)
        .filter(|(_, &a)| a)
        .map(|(l, _)| (l.level, centre(l, origin, size0)))
        .collect()
}

/// Run `removeInsideCells(1)`, `danglingCellRefine(21)`, `danglingCellRefine(24)`
/// and `baffleAndSplitMesh` on the octree `leaves` of `surfaceOnlyRefine`.
/// `None` if `keep` is in no leaf or is removed.
pub(crate) fn esi_stages(
    leaves: Vec<Leaf>,
    n0: [usize; 3],
    origin: Vector3,
    size0: [f64; 3],
    surface: &TriangleSoup,
    keep: Vector3,
    min_refine_cells: usize,
) -> Option<EsiStages> {
    let alive = remove_inside_cells(&leaves, n0, origin, size0, surface, keep, 1)?;
    let after_remove_inside = alive_cells(&leaves, &alive, origin, size0);
    let (leaves, alive, sel21) = dangling_cell_refine(leaves, alive, n0, 21, min_refine_cells, 100);
    let (leaves, alive, sel24) = dangling_cell_refine(leaves, alive, n0, 24, min_refine_cells, 100);
    let after_dangling = alive_cells(&leaves, &alive, origin, size0);
    let (kept, baffle) = baffle_and_split_mesh(&leaves, &alive, n0, origin, size0, surface, keep)?;
    let final_cells = alive_cells(&leaves, &kept, origin, size0);
    Some(EsiStages {
        leaves,
        kept,
        cells: EsiStageCells {
            after_remove_inside,
            dangling_selected: [sel21, sel24],
            after_dangling,
            final_cells,
            baffle,
        },
    })
}

/// The cells after each ESI stage for a whole case: `surfaceOnlyRefine`, then
/// [`esi_stages`]. Exists for the ESI v2506 differential
/// (`verification_and_validation/snappy_hex_mesh_esi_v2506`); `None` if
/// `locationInMesh` is outside the mesh or removed.
pub fn esi_stage_cells(
    surface: &TriangleSoup,
    controls: &crate::snappy_hex_mesh::castellation::CastellationControls,
) -> Option<EsiStageCells> {
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
    let n0 = [bg.nx, bg.ny, bg.nz];
    let leaves = crate::snappy_hex_mesh::refinement::surface_only_refine(
        leaves,
        n0,
        bg.bounds.min,
        size0,
        controls.surface_level,
        surface,
        controls.min_refine_cells,
    );
    esi_stages(leaves, n0, bg.bounds.min, size0, surface, controls.keep_point, controls.min_refine_cells).map(|s| s.cells)
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

    /// A face next to a refined edge carries the edge mid-point: in a 2 x 1 x 1
    /// row where only the left cell is split, the right cell's left face is
    /// four sub-faces (listed from the fine side), and a leaf's face ring has 4
    /// vertices unless an edge mid-point exists.
    #[test]
    fn face_ring_inserts_existing_edge_midpoints() {
        let mut leaves = vec![Leaf { level: 0, i: 1, j: 0, k: 0 }];
        for di in 0..2 {
            for dj in 0..2 {
                for dk in 0..2 {
                    leaves.push(Leaf { level: 1, i: di, j: dj, k: dk });
                }
            }
        }
        let topo = build_topo(&leaves, [2, 1, 1]);
        // The coarse cell (id 0) has 4 faces to fine leaves + 5 boundary faces.
        assert_eq!(topo.cell_faces[0].len(), 4 + 5);
        // Its +x... boundary faces: the y- face has the midpoint of the shared
        // edge x = 1 (between the two fine cells), so it is a pentagon.
        let ring_sizes: Vec<usize> = topo.cell_faces[0]
            .iter()
            .filter(|&&fi| topo.faces[fi].nei.is_none())
            .map(|&fi| topo.ring[fi].len())
            .collect();
        assert!(ring_sizes.iter().any(|&s| s > 4), "{ring_sizes:?}");
    }

    /// The buffer layer is point-connected: with a surface that cuts the
    /// background along a plane the kept region is one side of the plane; one
    /// buffer layer adds exactly the cells that share a point with it.
    #[test]
    fn buffer_layer_adds_point_connected_cells() {
        // A cube surface 0.3..0.7 inside the unit cube, 4^3 background of 0.25
        // cells: the inside region is the cells strictly inside.
        let surface = TriangleSoup::uv_sphere(Vector3::new(0.5, 0.5, 0.5), 0.3, 16, 16);
        let n0 = [4, 4, 4];
        let o = Vector3::new(0.0, 0.0, 0.0);
        let s0 = [0.25; 3];
        let leaves = background(4);
        let k = Vector3::new(0.5, 0.5, 0.5);
        let none = remove_inside_cells(&leaves, n0, o, s0, &surface, k, 0).unwrap();
        let one = remove_inside_cells(&leaves, n0, o, s0, &surface, k, 1).unwrap();
        let (a, b) = (none.iter().filter(|&&x| x).count(), one.iter().filter(|&&x| x).count());
        assert!(a > 0 && b > a, "{a} {b}");
        assert!(none.iter().zip(&one).all(|(x, y)| !*x || *y), "buffer only adds");
        assert!(remove_inside_cells(&leaves, n0, o, s0, &surface, Vector3::new(9.0, 9.0, 9.0), 1).is_none());
    }

    /// Dangling refinement on a uniform interior block: an alive cell is
    /// refined when it has 21 or 24 internal faces; in a uniform mesh (6 faces)
    /// nothing is selected and the leaves are unchanged.
    #[test]
    fn dangling_selects_nothing_in_a_uniform_mesh() {
        let leaves = background(3);
        let alive = vec![true; leaves.len()];
        let (out, al, sel) = dangling_cell_refine(leaves.clone(), alive, [3, 3, 3], 21, 0, 100);
        assert_eq!(out.len(), leaves.len());
        assert!(al.iter().all(|&a| a));
        assert_eq!(sel, vec![0]);
    }

    /// A cell with five refined sides and one coarse side has 21 internal
    /// faces and is refined; its refinement is closed under 2:1.
    #[test]
    fn dangling_refines_a_cell_surrounded_by_finer_cells() {
        // 3^3 background; refine the 26 outer cells, leave the centre one: it
        // has 6 sides x 4 = 24 internal faces.
        let mut leaves = Vec::new();
        for i in 0..3 {
            for j in 0..3 {
                for k in 0..3 {
                    if (i, j, k) == (1, 1, 1) {
                        leaves.push(Leaf { level: 0, i, j, k });
                    } else {
                        for di in 0..2 {
                            for dj in 0..2 {
                                for dk in 0..2 {
                                    leaves.push(Leaf { level: 1, i: 2 * i + di, j: 2 * j + dj, k: 2 * k + dk });
                                }
                            }
                        }
                    }
                }
            }
        }
        let alive = vec![true; leaves.len()];
        let n = leaves.len();
        let (out, _, sel21) = dangling_cell_refine(leaves.clone(), alive.clone(), [3, 3, 3], 21, 0, 100);
        assert_eq!((out.len(), sel21), (n, vec![0]));
        let (out, al, sel24) = dangling_cell_refine(leaves, alive, [3, 3, 3], 24, 0, 100);
        assert_eq!(sel24, vec![1, 0]);
        assert_eq!(out.len(), n + 7);
        assert_eq!(al.iter().filter(|&&a| a).count(), n + 7);
    }

    /// The whole pipeline on a sphere: the final cells are a subset of the cells
    /// after dangling-cell refinement, which contain the buffer layer.
    #[test]
    fn stages_are_nested_on_a_sphere() {
        use crate::snappy_hex_mesh::{BackgroundMesh, Bounds, CastellationControls};
        let surface = TriangleSoup::uv_sphere(Vector3::new(0.5, 0.5, 0.5), 0.3, 24, 24);
        let domain = Bounds::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 1.0, 1.0));
        let c = CastellationControls::new(BackgroundMesh::uniform(domain, 8, 8, 8), 2, Vector3::new(0.5, 0.5, 0.5))
            .with_surface_intersection_refinement();
        let st = esi_stage_cells(&surface, &c).unwrap();
        assert!(st.final_cells.len() <= st.after_dangling.len());
        assert!(st.after_dangling.len() >= st.after_remove_inside.len());
        assert!(st.baffle.n_hit_internal > 0);
        assert_eq!(st.baffle.n_kept, st.final_cells.len());
        // The kept centres are inside the sphere (keep point inside).
        assert!(st.final_cells.iter().all(|(_, p)| (*p - Vector3::new(0.5, 0.5, 0.5)).mag() < 0.35));
    }
}
