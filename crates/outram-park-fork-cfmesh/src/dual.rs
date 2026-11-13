// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Algorithm references (re-implemented in Rust, not transcribed):
//
//   [1] OpenFOAM `polyDualMesh` — the "one cell per primal vertex" dual and its
//       face topology (one dual face per primal edge).
//       applications/utilities/mesh/manipulation/polyDualMesh
//       Copyright (C) 2011-2016 OpenFOAM Foundation
//       Copyright (C) 2016-2023 OpenCFD Ltd. Licence: GPL-3.0-only.
//
//   [2] voro++ — https://github.com/chr1shr/voro — reference for the Voronoi /
//       polyhedral-dual construction only; no code taken.
//       Copyright (C) 2008-2015 The Regents of the University of California,
//       through Lawrence Berkeley National Laboratory (Chris H. Rycroft).
//       Licence: modified BSD (3-clause), which is GPL-3.0-compatible; see the
//       crate NOTICE. This file contains no voro++ code.
//
//   [1b] OpenFOAM-dev commit 852527377c7e, the boundary faces of the dual
//       (gh:#1240): `meshDualiser::createFacesAroundBoundaryPoint`
//       (applications/utilities/mesh/manipulation/polyDualMesh/meshDualiser.C),
//       `simpleMarkFeatures` (.../polyDualMesh/polyDualMesh.C) and
//       `polyDualMesh::splitFace` / `calcFeatures`
//       (src/conversion/polyDualMesh/polyDualMesh.C).
//       Copyright (C) 2011-2026 OpenFOAM Foundation. Licence: GPL-3.0-or-later.
//
//   [3] The median / Donald (vertex-centred) dual is standard finite-volume
//       literature, e.g. T. J. Barth, "Aspects of Unstructured Grids and
//       Finite-Volume Solvers", VKI Lecture Series 1994-05.
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

//! The **polyhedral dual** mesh — one polyhedral cell per primal *vertex*. It
//! fills the same *role* as OpenFOAM's `polyDualMesh` (voro++ is the reference
//! for the Voronoi/median-dual idea), but it is **not the same construction** —
//! see "What this dual is, and what it is not" below before transferring any
//! result from a cell-centre dual to this one.
//!
//! A finite-volume solver is usually happier on a **polyhedral** mesh than on a
//! hex/tet mesh of the same region: polyhedra pack more neighbours per cell
//! (better gradient reconstruction) at a much lower cell count. The classic way
//! to get one is to take the *dual* of a primal mesh — turn every primal
//! **vertex** into a cell.
//!
//! # What this dual is, and what it is not
//!
//! This is the **median** (a.k.a. Donald / vertex-centred) dual. Stated
//! plainly, because the distinction is routinely lost:
//!
//! - **It is not the circumcentre Voronoi dual.** Its dual-cell corners are
//!   edge *midpoints* and face/cell *centroids*, never circumcentres. The
//!   payoff is that it is well-defined for *any* primal mesh — the carved hex
//!   mesh this crate produces, not only a Delaunay tetrahedralisation. The
//!   price is that it inherits **none** of the Voronoi dual's orthogonality
//!   properties: a median-dual face is not in general perpendicular to the
//!   primal edge it separates.
//! - **It is not the same algorithm as `outram-foam-mesh`'s
//!   `poly_dual_mesh`.** That one is the **cell-centre** dual (dual vertices
//!   placed at primal *cell centroids*, one dual face per primal *edge*) — a
//!   genuinely different construction, on a different primal→dual entity map.
//!   Its measured V&V result — "dualisation does not create
//!   non-orthogonality; the dual of a uniform hex block measures exactly 0 deg
//!   and 0 skewness" — is a statement about *that* algorithm and **must not be
//!   transferred to this one**. Nothing in this module has been measured
//!   against it.
//!
//! # Honest V&V scope (what the tests in this module actually gate)
//!
//! The four tests below assert exactly four properties, and no more:
//!
//! 1. **closure** — `VolumeMesh::validate` passes (every cell is a closed
//!    surface), plus the internal/boundary face split is consistent;
//! 2. **volume conservation** — `Σ dual-cell volumes == primal domain volume`
//!    to `1e-9`;
//! 3. **boundary match** — the dual boundary area equals the primal surface
//!    area to `1e-9`;
//! 4. **face count / polyhedrality** — one cell per primal vertex, interior
//!    cells have more than 6 faces, and the face-minimal variant has strictly
//!    fewer faces than the quad-fan one.
//!
//! **There is no orthogonality or skewness test for this crate's dual.**
//! (**UPDATED 2026-11-04, gh:#1240:** still no gate, but the pipeline's
//! ignored `diagnose_dual_kinds_differential` measures both for every
//! [`DualKind`] on the pipe and duct.) No
//! test in this module calls [`crate::checks::check_quality`], so the
//! non-orthogonality and skewness of a median dual produced here are
//! **unmeasured** at the module level. The only numbers this crate records for
//! them are the whole-pipeline sphere table in [`crate::pipeline`]'s tests,
//! which measures the *composed* pipeline (carve → snap → tet → dual → smooth)
//! and therefore cannot attribute a figure to the dual step alone. Treat any
//! claim about this dual's orthogonality as unverified until such a test
//! exists.
//!
//! Each primal cell is split into one **corner sub-cell** per vertex: the part
//! of the cell nearest that vertex, bounded by
//!
//! - the primal vertex `v`,
//! - the **midpoints** of the primal edges at `v`,
//! - the **centroids** of the primal faces at `v`,
//! - the primal **cell centroid**.
//!
//! The dual cell of `v` is the union of the corner sub-cells of every primal
//! cell that touches `v`. Two kinds of quad make up a sub-cell's boundary:
//!
//! - **inner quads** `[edge-midpoint, face-centroid, cell-centroid,
//!   face-centroid]` — one per primal edge inside the cell. The inner quad of
//!   edge `(v, w)` is listed by *both* dual cells `v` and `w`, so it becomes an
//!   **internal** dual face separating them.
//! - **outer quads** `[v, edge-midpoint, face-centroid, edge-midpoint]` on the
//!   primal *boundary* faces — these tile each boundary face over its vertices
//!   and become the **boundary** faces of the dual mesh.
//!
//! Because the sub-cells partition the domain exactly, the dual tiles the same
//! region: `Σ dual-cell volumes == primal domain volume`, every internal dual
//! face is shared by exactly two dual cells, and the dual boundary surface
//! coincides with the primal boundary surface.
//!
//! # Implementation
//!
//! Winding and owner/neighbour are recovered by [`from_cell_faces`], which
//! matches shared faces by vertex set and orients every face outward from its
//! owning cell's centroid — so this routine only has to emit each dual cell's
//! quads (in any winding) and hand them over. Geometry points (edge midpoints,
//! face/cell centroids) are allocated **per primal cell**; the inner quad of an
//! edge is always built inside a single cell, so the two endpoints reference the
//! same indices and the shared-face match is exact.
//!
//! # Two variants
//!
//! [`polyhedral_dual`] (this construction) splits the face between two dual
//! cells into one quad per surrounding primal cell — robust and watertight, but
//! more, smaller faces than a minimal `polyDualMesh`. [`polyhedral_dual_min_faces`]
//! produces the **face-minimal** dual: one polygon per primal edge, built by
//! walking the edge's star of incident faces/cells (~40% fewer faces on a
//! Cartesian block), verified to conserve volume and stay closed. Prefer the
//! face-minimal variant for solver meshes; the quad-fan variant is the simpler,
//! maximally-robust fallback.
//!
//! Orientation assumes each dual cell is star-shaped about its primal vertex
//! (true for structured/graded blocks); [`VolumeMesh::validate`] is the gate
//! that catches any cell that is not. Pure Rust, Android-safe.

use crate::math::Vec3;
use crate::volume_mesh::{from_cell_faces, VolumeMesh};
use std::collections::{BTreeMap, HashMap};

/// Build the polyhedral **median dual** of `mesh`: one polyhedral cell per
/// primal vertex. See the module docs for the construction and its guarantees.
///
/// # Examples
///
/// ```
/// use outram_park_fork_cfmesh::{math::Vec3, shapes::box_surface, carve::carve_box, dual::polyhedral_dual};
///
/// // Carve a unit box into 2×2×2 hexes (27 vertices), then take the dual:
/// // 27 polyhedral cells that tile the same volume.
/// let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
/// let hex = carve_box(&p, &t, 0.5);
/// let dual = polyhedral_dual(&hex);
///
/// assert_eq!(dual.cell_count(), hex.point_count()); // one dual cell per vertex
/// assert!((dual.total_volume() - hex.total_volume()).abs() < 1e-9);
/// assert!(dual.validate().is_ok());
/// ```
pub fn polyhedral_dual(mesh: &VolumeMesh) -> VolumeMesh {
    let n_cells = mesh.n_cells;
    let n_faces = mesh.face_count();

    // Primal cell -> its primal face indices (owner and neighbour both count).
    let mut cell_faces: Vec<Vec<usize>> = vec![Vec::new(); n_cells];
    for f in 0..n_faces {
        cell_faces[mesh.owner[f]].push(f);
        if let Some(nb) = mesh.neighbour[f] {
            cell_faces[nb].push(f);
        }
    }

    // Dual points start with every primal vertex (only boundary vertices are
    // actually referenced — by the outer quads — but keeping them all keeps the
    // indexing trivial; unused points are harmless).
    let mut points: Vec<Vec3> = mesh.points.clone();
    let n_primal_pts = points.len();

    // One dual cell (a list of face rings) per primal vertex.
    let mut dual_cells: Vec<Vec<Vec<usize>>> = vec![Vec::new(); n_primal_pts];

    let key = |a: usize, b: usize| if a < b { (a, b) } else { (b, a) };

    for faces in &cell_faces {
        // Cell centroid: average of the cell's distinct vertices.
        let mut verts: Vec<usize> = Vec::new();
        for &f in faces {
            verts.extend_from_slice(&mesh.faces[f]);
        }
        verts.sort_unstable();
        verts.dedup();
        let mut gc = Vec3::ZERO;
        for &v in &verts {
            gc = gc.add(mesh.points[v]);
        }
        let gc = gc.scale(1.0 / verts.len().max(1) as f64);
        let gc_idx = points.len();
        points.push(gc);

        // Face centroids for this cell's faces (one dual point each).
        let mut gf_idx: HashMap<usize, usize> = HashMap::new();
        for &f in faces {
            let idx = points.len();
            points.push(mesh.face_centroid(f));
            gf_idx.insert(f, idx);
        }

        // Per-cell edge midpoints, and for each edge the two incident face
        // centroids (an edge touches exactly two faces of a manifold cell).
        let mut mid_idx: HashMap<(usize, usize), usize> = HashMap::new();
        let mut edge_faces: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
        for &f in faces {
            let ring = &mesh.faces[f];
            let k = ring.len();
            for i in 0..k {
                let a = ring[i];
                let b = ring[(i + 1) % k];
                let e = key(a, b);
                mid_idx.entry(e).or_insert_with(|| {
                    let m = mesh.points[a].add(mesh.points[b]).scale(0.5);
                    let idx = points.len();
                    points.push(m);
                    idx
                });
                edge_faces.entry(e).or_default().push(gf_idx[&f]);
            }
        }

        // Inner quads: one per primal edge of the cell, shared by the two
        // endpoints' dual cells -> internal dual faces. (Winding is fixed by
        // from_cell_faces.)
        for (&(a, b), fcs) in &edge_faces {
            if fcs.len() != 2 {
                continue; // non-manifold edge within the cell — skip defensively
            }
            let m = mid_idx[&key(a, b)];
            let quad = vec![m, fcs[0], gc_idx, fcs[1]];
            dual_cells[a].push(quad.clone());
            dual_cells[b].push(quad);
        }

        // Outer quads on boundary primal faces: split each boundary face over
        // its vertices -> the boundary faces of the dual mesh.
        for &f in faces {
            if mesh.neighbour[f].is_some() {
                continue; // interior primal face -> internal to one dual cell, omit
            }
            let ring = &mesh.faces[f];
            let k = ring.len();
            let gf = gf_idx[&f];
            for i in 0..k {
                let v = ring[i];
                let prev = ring[(i + k - 1) % k];
                let next = ring[(i + 1) % k];
                let m1 = mid_idx[&key(prev, v)];
                let m2 = mid_idx[&key(v, next)];
                dual_cells[v].push(vec![v, m1, gf, m2]);
            }
        }
    }

    from_cell_faces(points, &dual_cells)
}

/// Build the **face-minimal** polyhedral dual of `mesh`: like
/// [`polyhedral_dual`], one polyhedral cell per primal vertex, but the face
/// between two dual cells is a **single polygon** rather than a fan of quads.
///
/// This is the proper `polyDualMesh` face topology. For each primal **edge**
/// `(a, b)` the dual face separating cells `a` and `b` is one ring built by
/// walking the edge's star of incident faces and cells:
///
/// - **interior edge** — the star closes, giving the ring
///   `[faceCentroid, cellCentroid, faceCentroid, cellCentroid, …]`;
/// - **boundary edge** — the star is an open fan; the ring is closed on the
///   domain-boundary side through the **edge midpoint**:
///   `[edgeMidpoint, faceCentroid₀, cellCentroid₀, …, faceCentroidₙ]`.
///
/// The domain boundary is tiled by the same per-vertex outer quads as
/// [`polyhedral_dual`] (`[vertex, edgeMidpoint, faceCentroid, edgeMidpoint]`),
/// so the dual boundary coincides with the primal boundary surface and the
/// **volume is conserved** (a shared internal face contributes equally and
/// oppositely to its two cells; only the boundary sets the volume).
///
/// Far fewer faces than [`polyhedral_dual`] (one per primal edge, not one per
/// (edge, surrounding cell)). Assumes a **manifold** primal mesh (each interior
/// edge ringed by a single closed cell cycle, each boundary edge by exactly two
/// boundary faces) — the meshes this crate generates. Owner/neighbour and
/// winding are recovered by [`from_cell_faces`].
///
/// # Examples
///
/// ```
/// use outram_park_fork_cfmesh::{math::Vec3, shapes::box_surface, carve::carve_box, dual::polyhedral_dual_min_faces};
///
/// let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
/// let hex = carve_box(&p, &t, 0.5);
/// let dual = polyhedral_dual_min_faces(&hex);
///
/// assert_eq!(dual.cell_count(), hex.point_count()); // one cell per primal vertex
/// assert!((dual.total_volume() - hex.total_volume()).abs() < 1e-9);
/// assert!(dual.validate().is_ok());
/// ```
pub fn polyhedral_dual_min_faces(mesh: &VolumeMesh) -> VolumeMesh {
    min_faces_dual(mesh, None)
}

/// Which polyhedral dual the pipeline's dual stage builds (gh:#1240).
///
/// All three make one dual cell per primal vertex. They differ in where the
/// dual's corners sit and in how the domain boundary is tiled; the
/// differential comparison of the three is in the gh:#1240 section of
/// `crates/outram-foam-appbuilder-lib/verification_and_validation/laminar_pipe_gh1155.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DualKind {
    /// The **median** dual ([`polyhedral_dual_min_faces`], with
    /// [`polyhedral_dual`] as fallback), one boundary face per primal
    /// boundary-face **corner** (`[v, edge-mid, face-centroid, edge-mid]`).
    /// The default: what every recorded number before gh:#1240 used. A wall
    /// cell of a tetrahedral primal gets 4–8 boundary faces, each at a median
    /// 69° to its owner (gh:#1215).
    #[default]
    MedianPerCorner,
    /// The median dual with **one boundary face per primal boundary point
    /// per feature-bounded sector** ([`polyhedral_dual_per_point`]), the
    /// boundary rule of OpenFOAM's `polyDualMesh` ported onto the median
    /// dual (Route A of gh:#1240).
    MedianPerPoint,
    /// OpenFOAM's **cell-centre** dual (`polyDualMesh`), through
    /// `outram-foam-mesh`'s `poly_dual_mesh` (Route B of gh:#1240); needs
    /// this crate's `cell-centre-dual` feature, and falls back to
    /// [`DualKind::MedianPerCorner`] with a stage note without it.
    CellCentre,
}

/// The median dual of `mesh` (interior exactly as [`polyhedral_dual_min_faces`])
/// with the boundary tiled by **one dual face per primal boundary point per
/// feature-bounded sector**, the rule of OpenFOAM's `polyDualMesh` (gh:#1240,
/// Route A).
///
/// Upstream (OpenFOAM-dev `852527377c7e`), ported:
///
/// - **Feature edges** (`simpleMarkFeatures`, polyDualMesh.C): a boundary edge
///   whose two boundary faces are in different `mesh.patches` (always), or
///   whose unit face normals satisfy `n0 · n1 < cos(feature_angle_deg)`;
///   every point on a feature edge is a feature point.
/// - **Boundary faces** (`meshDualiser::createFacesAroundBoundaryPoint`): the
///   boundary faces around a point are walked in order; at a non-feature point
///   they give one polygon `[fc, em, fc, em, …]` (the point itself dropped);
///   at a feature point a face is closed at every feature edge, one per sector,
///   `[v, em(feature), fc, em, …, fc, em(feature)]`, so a point on a sharp
///   line keeps the line and no face spans two patches. Fewer than two
///   feature edges at a point: the ring is kept whole
///   (`polyDualMesh::splitFace`).
///
/// **What differs from upstream, and why.** The interior is this crate's
/// median dual, not the cell-centre dual; every boundary edge carries an
/// edge-midpoint dual vertex (the median dual's boundary-edge face closes
/// through it), where upstream has one only on feature edges.
///
/// **Shared code.** The same boundary rule is in `outram-foam-mesh`'s
/// `poly_dual_mesh::build_boundary_faces_for_point` (Route B). It cannot be
/// called from here: this crate builds with no dependency by default and
/// `outram-foam-mesh` is optional (`cell-centre-dual`), so the ~40 lines of
/// sector walking are written in both, each citing the other and upstream.
///
/// Volume: merging the corner quads drops the point `v` from the face, so on
/// a curved patch the face cuts below `v` and the volume is no longer the
/// primal's exactly (it is on a flat patch). The non-feature case assumes the
/// boundary faces around a point form one ring (a manifold surface); a point
/// where they do not keeps its per-corner quads.
///
/// # Examples
///
/// ```
/// use outram_park_fork_cfmesh::{math::Vec3, shapes::box_surface, carve::carve_box, dual::polyhedral_dual_per_point};
///
/// let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
/// let hex = carve_box(&p, &t, 0.5);
/// let dual = polyhedral_dual_per_point(&hex, 45.0);
/// assert_eq!(dual.cell_count(), hex.point_count());
/// assert!((dual.total_volume() - 1.0).abs() < 1e-9); // flat sides: exact
/// assert!(dual.validate().is_ok());
/// ```
pub fn polyhedral_dual_per_point(mesh: &VolumeMesh, feature_angle_deg: f64) -> VolumeMesh {
    polyhedral_dual_per_point_with(mesh, feature_angle_deg, FeaturePointRule::OnFeatureEdge)
}

/// Which boundary points keep their own dual vertex (gh:#1240). OpenFOAM has
/// two `polyDualMesh`s, and they differ here; both are kept callable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FeaturePointRule {
    /// Every point on a feature edge (the `polyDualMesh` **utility**:
    /// `simpleMarkFeatures`' `singleCellFeaturePoints`, then
    /// `meshDualiser`). A point on a sharp line keeps the line exactly: its
    /// faces are `[v, em, fc, …, em]`, one per side.
    #[default]
    OnFeatureEdge,
    /// Only points with **more than two** feature edges (the `polyDualMesh`
    /// **class**, `src/conversion/polyDualMesh/polyDualMesh.C`
    /// `calcFeatures`): a point on a feature line drops out and its ring is
    /// split along the line's two feature edges (`splitFace`, "multiple
    /// feature edges but no feature point"), the sides meeting on the chord
    /// between the two feature-edge midpoints. On a straight line the chord
    /// passes through the point, so nothing is cut; where the line bends, the
    /// chord cuts the corner by its sagitta.
    Corners,
}

/// [`polyhedral_dual_per_point`] with the feature-point rule named.
pub fn polyhedral_dual_per_point_with(mesh: &VolumeMesh, feature_angle_deg: f64, rule: FeaturePointRule) -> VolumeMesh {
    min_faces_dual(mesh, Some((feature_angle_deg, rule)))
}

/// The face-minimal median dual; `per_point = Some(feature angle)` tiles the
/// boundary per point per sector ([`polyhedral_dual_per_point`]), `None` per
/// boundary-face corner ([`polyhedral_dual_min_faces`], unchanged to the bit).
fn min_faces_dual(mesh: &VolumeMesh, per_point: Option<(f64, FeaturePointRule)>) -> VolumeMesh {
    let n_faces = mesh.face_count();
    let n_cells = mesh.n_cells;

    // Primal cell -> its faces.
    let mut cell_faces: Vec<Vec<usize>> = vec![Vec::new(); n_cells];
    for f in 0..n_faces {
        cell_faces[mesh.owner[f]].push(f);
        if let Some(nb) = mesh.neighbour[f] {
            cell_faces[nb].push(f);
        }
    }

    // edge -> faces containing it; (cell, edge) -> the (two) faces of that cell
    // containing the edge; needed to walk each edge's star. `edge_faces` is a
    // BTreeMap because the loop below numbers edge midpoints and emits dual
    // faces in its order; as a HashMap that order (and so the mesh's
    // numbering) changed from call to call (gh:#1202, 2026-11-04). The same in
    // `polyhedral_dual`.
    let mut edge_faces: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
    let mut cell_edge_faces: HashMap<(usize, (usize, usize)), Vec<usize>> = HashMap::new();
    let key = |a: usize, b: usize| if a < b { (a, b) } else { (b, a) };
    for (c, faces) in cell_faces.iter().enumerate() {
        for &f in faces {
            let ring = &mesh.faces[f];
            let k = ring.len();
            for i in 0..k {
                let e = key(ring[i], ring[(i + 1) % k]);
                cell_edge_faces.entry((c, e)).or_default().push(f);
            }
        }
    }
    for f in 0..n_faces {
        let ring = &mesh.faces[f];
        let k = ring.len();
        for i in 0..k {
            edge_faces
                .entry(key(ring[i], ring[(i + 1) % k]))
                .or_default()
                .push(f);
        }
    }

    // Points: primal vertices, then face centroids, cell centroids, then edge
    // midpoints (added on demand).
    let mut points: Vec<Vec3> = mesh.points.clone();
    let face_base = points.len();
    for f in 0..n_faces {
        points.push(mesh.face_centroid(f));
    }
    let cell_base = points.len();
    for faces in &cell_faces {
        let mut verts: Vec<usize> = Vec::new();
        for &f in faces {
            verts.extend_from_slice(&mesh.faces[f]);
        }
        verts.sort_unstable();
        verts.dedup();
        let mut g = Vec3::ZERO;
        for &v in &verts {
            g = g.add(mesh.points[v]);
        }
        points.push(g.scale(1.0 / verts.len().max(1) as f64));
    }
    let pf = |f: usize| face_base + f;
    let pc = |c: usize| cell_base + c;

    let mut edge_mid: HashMap<(usize, usize), usize> = HashMap::new();
    let mut dual_cells: Vec<Vec<Vec<usize>>> = vec![Vec::new(); mesh.points.len()];

    // The "other" face of a cell around an edge, and the cell across a face.
    let other_face = |c: usize, e: (usize, usize), f: usize| -> usize {
        let two = &cell_edge_faces[&(c, e)];
        if two[0] == f {
            two[1]
        } else {
            two[0]
        }
    };
    let across = |f: usize, c: usize| -> Option<usize> {
        match (mesh.owner[f], mesh.neighbour[f]) {
            (o, Some(nb)) if o == c => Some(nb),
            (o, Some(nb)) if nb == c => Some(o),
            _ => None,
        }
    };

    // Type-A dual faces: one polygon per primal edge, shared by its endpoints.
    for (&(a, b), faces_e) in &edge_faces {
        let boundary_start = faces_e
            .iter()
            .copied()
            .find(|&f| mesh.neighbour[f].is_none());
        let e = (a, b);
        let ring: Vec<usize> = if let Some(fstart) = boundary_start {
            // Boundary edge: open fan, closed through the edge midpoint.
            let mid = *edge_mid.entry(e).or_insert_with(|| {
                let m = mesh.points[a].add(mesh.points[b]).scale(0.5);
                points.push(m);
                points.len() - 1
            });
            let mut poly = vec![mid, pf(fstart)];
            let mut cur_face = fstart;
            let mut cur_cell = mesh.owner[fstart];
            loop {
                poly.push(pc(cur_cell));
                let nf = other_face(cur_cell, e, cur_face);
                poly.push(pf(nf));
                match across(nf, cur_cell) {
                    None => break, // reached the other boundary face
                    Some(next) => {
                        cur_face = nf;
                        cur_cell = next;
                    }
                }
            }
            poly
        } else {
            // Interior edge: closed star ring [pf, pc, pf, pc, ...].
            let fstart = faces_e[0];
            let mut poly = Vec::new();
            let mut cur_face = fstart;
            let mut cur_cell = mesh.owner[fstart];
            for _ in 0..faces_e.len() + 1 {
                poly.push(pf(cur_face));
                poly.push(pc(cur_cell));
                let nf = other_face(cur_cell, e, cur_face);
                let next = across(nf, cur_cell).expect("interior edge star stays interior");
                cur_face = nf;
                cur_cell = next;
                if cur_face == fstart {
                    break;
                }
            }
            poly
        };
        dual_cells[a].push(ring.clone());
        dual_cells[b].push(ring);
    }

    if let Some((angle, rule)) = per_point {
        boundary_faces_per_point(mesh, angle, rule, &edge_mid, face_base, &mut dual_cells);
        return from_cell_faces(points, &dual_cells);
    }

    // Boundary dual faces: split each boundary primal face over its vertices.
    for f in 0..n_faces {
        if mesh.neighbour[f].is_some() {
            continue;
        }
        let ring = &mesh.faces[f];
        let k = ring.len();
        for i in 0..k {
            let v = ring[i];
            let prev = ring[(i + k - 1) % k];
            let next = ring[(i + 1) % k];
            let mut mid = |x: usize, y: usize| {
                *edge_mid.entry(key(x, y)).or_insert_with(|| {
                    let m = mesh.points[x].add(mesh.points[y]).scale(0.5);
                    points.push(m);
                    points.len() - 1
                })
            };
            let m1 = mid(prev, v);
            let m2 = mid(v, next);
            dual_cells[v].push(vec![v, m1, pf(f), m2]);
        }
    }

    from_cell_faces(points, &dual_cells)
}

/// The boundary faces of [`polyhedral_dual_per_point`]: per primal boundary
/// point, one ring per feature-bounded sector (see there). `edge_mid` holds
/// the dual vertex of every boundary edge's midpoint (the type-A loop made
/// them), `face_base + f` is the dual vertex of face `f`'s centroid. A point
/// whose boundary faces do not form a single ring keeps its per-corner quads.
fn boundary_faces_per_point(
    mesh: &VolumeMesh,
    feature_angle_deg: f64,
    rule: FeaturePointRule,
    edge_mid: &HashMap<(usize, usize), usize>,
    face_base: usize,
    dual_cells: &mut [Vec<Vec<usize>>],
) {
    use petir::mathf::RealMath;
    let key = |a: usize, b: usize| if a < b { (a, b) } else { (b, a) };
    let n_faces = mesh.face_count();
    // Patch of every boundary face (patch index; faces outside every patch
    // share one id).
    let mut patch_of = vec![usize::MAX; n_faces];
    for (pi, pa) in mesh.patches.iter().enumerate() {
        for f in pa.start_face..(pa.start_face + pa.n_faces).min(n_faces) {
            patch_of[f] = pi;
        }
    }
    // Boundary edge -> its boundary faces; boundary point -> its boundary
    // faces (both in face order, so the walk below is deterministic).
    let mut edge_bfaces: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
    let mut point_bfaces: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for f in 0..n_faces {
        if mesh.neighbour[f].is_some() {
            continue;
        }
        let ring = &mesh.faces[f];
        let k = ring.len();
        for i in 0..k {
            edge_bfaces.entry(key(ring[i], ring[(i + 1) % k])).or_default().push(f);
            point_bfaces.entry(ring[i]).or_default().push(f);
        }
    }
    // Feature edges (`simpleMarkFeatures`): patch seams, and normals more than
    // the feature angle apart.
    let min_cos = feature_angle_deg.to_radians().r_cos();
    let unit = |f: usize| {
        let a = mesh.face_area_vector(f);
        a.scale(1.0 / a.length().max(1e-300))
    };
    let mut feature_edge: BTreeMap<(usize, usize), bool> = BTreeMap::new();
    let mut n_feature_edges = vec![0usize; mesh.points.len()];
    for (&e, fs) in &edge_bfaces {
        let feat = fs.len() != 2 || patch_of[fs[0]] != patch_of[fs[1]] || unit(fs[0]).dot(unit(fs[1])) < min_cos;
        feature_edge.insert(e, feat);
        if feat {
            n_feature_edges[e.0] += 1;
            n_feature_edges[e.1] += 1;
        }
    }
    let feature_point: Vec<bool> = n_feature_edges
        .iter()
        .map(|&n| match rule {
            FeaturePointRule::OnFeatureEdge => n > 0,
            FeaturePointRule::Corners => n > 2,
        })
        .collect();
    // The two boundary edges of face f at point v: (before v, after v).
    let edges_at = |f: usize, v: usize| -> ((usize, usize), (usize, usize)) {
        let ring = &mesh.faces[f];
        let k = ring.len();
        let i = ring.iter().position(|&q| q == v).expect("point on its face");
        (key(ring[(i + k - 1) % k], v), key(v, ring[(i + 1) % k]))
    };
    for (&v, bfaces) in &point_bfaces {
        // Order the faces into a ring: ring[i] and ring[i + 1] share
        // shared[i] (walking across the edge after v of each face).
        let mut ring = vec![bfaces[0]];
        let mut shared = Vec::new();
        let mut ok = true;
        loop {
            let cur = *ring.last().unwrap();
            let e = edges_at(cur, v).1;
            let next = edge_bfaces[&e].iter().copied().find(|&g| g != cur);
            let Some(next) = next else {
                ok = false;
                break;
            };
            shared.push(e);
            if next == ring[0] {
                break;
            }
            if ring.len() > bfaces.len() || ring.contains(&next) {
                ok = false;
                break;
            }
            ring.push(next);
        }
        if !ok || ring.len() != bfaces.len() {
            // Not a single ring (non-manifold point): keep the corner quads.
            for &f in bfaces {
                let (e1, e2) = edges_at(f, v);
                dual_cells[v].push(vec![v, edge_mid[&e1], face_base + f, edge_mid[&e2]]);
            }
            continue;
        }
        let k = ring.len();
        let cuts: Vec<usize> = (0..k).filter(|&i| feature_edge[&shared[i]]).collect();
        if cuts.len() >= 2 {
            // One face per sector between consecutive feature edges; through
            // the point itself when it is a feature point, else closed on the
            // chord between the two feature-edge midpoints (`splitFace`).
            for c in 0..cuts.len() {
                let (start, end) = (cuts[c], cuts[(c + 1) % cuts.len()]);
                let mut face = if feature_point[v] { vec![v, edge_mid[&shared[start]]] } else { vec![edge_mid[&shared[start]]] };
                let mut i = (start + 1) % k;
                loop {
                    face.push(face_base + ring[i]);
                    face.push(edge_mid[&shared[i]]);
                    if i == end {
                        break;
                    }
                    i = (i + 1) % k;
                }
                dual_cells[v].push(face);
            }
        } else {
            let mut face = Vec::with_capacity(2 * k);
            for i in 0..k {
                face.push(face_base + ring[i]);
                face.push(edge_mid[&shared[i]]);
            }
            dual_cells[v].push(face);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carve::carve_box;
    use crate::shapes::box_surface;
    use crate::volume_mesh::cells_faces;

    /// V&V — headline. Methodology: a unit box carved into 2×2×2 hexes (27
    /// vertices), then its median dual. Pass criteria (from the bead): one dual
    /// cell per primal vertex; the dual tiles the same region so `Σ vols ==`
    /// domain volume (exact); every cell is closed (`validate` Ok); every
    /// internal dual face is shared by exactly two cells; the dual is genuinely
    /// **polyhedral** (the interior vertex's cell has far more than 6 faces).
    /// Result: 27 cells; volume 1.0 (exact); validate Ok; interior cell has 24
    /// faces.
    #[test]
    fn dual_of_hex_block_is_closed_and_conserves_volume() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let hex = carve_box(&p, &t, 0.5); // 8 hexes, 27 vertices
        let dual = polyhedral_dual(&hex);

        assert_eq!(
            dual.cell_count(),
            hex.point_count(),
            "one dual cell per primal vertex"
        );
        assert!(
            (dual.total_volume() - hex.total_volume()).abs() < 1e-9,
            "dual conserves the domain volume: {} vs {}",
            dual.total_volume(),
            hex.total_volume()
        );
        dual.validate().expect("every dual cell is closed");

        // Each internal dual face has a neighbour; each boundary face does not —
        // i.e. every face is shared by exactly two cells or is a single boundary.
        for f in 0..dual.face_count() {
            let internal = f < dual.n_internal_faces();
            assert_eq!(
                dual.neighbour[f].is_some(),
                internal,
                "face {f} internal/boundary split"
            );
        }

        // Polyhedral: the cell around the interior vertex has > 6 faces (6 edges
        // × 4 surrounding hexes = 24 inner quads).
        let max_faces = cells_faces(&dual).iter().map(|c| c.len()).max().unwrap();
        assert!(
            max_faces > 6,
            "polyhedral cells present (max faces/cell = {max_faces})"
        );
    }

    /// V&V — the dual boundary surface coincides with the primal boundary, so
    /// the dual boundary faces enclose the same volume and every boundary vertex
    /// contributes. Methodology: a 3×3×3 carve, dual, compare boundary-face area
    /// sum to the unit cube's surface area (6). Result: total boundary area 6.0
    /// (exact); volume preserved.
    #[test]
    fn dual_boundary_matches_primal_surface() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let hex = carve_box(&p, &t, 1.0 / 3.0); // 27 hexes
        let dual = polyhedral_dual(&hex);

        let mut area = 0.0;
        for f in 0..dual.face_count() {
            if dual.neighbour[f].is_none() {
                area += dual.face_area_vector(f).length();
            }
        }
        assert!(
            (area - 6.0).abs() < 1e-9,
            "dual boundary area == cube surface area: {area}"
        );
        assert!((dual.total_volume() - 1.0).abs() < 1e-9);
        dual.validate().expect("closed");
    }

    /// V&V — the **face-minimal** merged dual is a correct, watertight dual with
    /// far fewer faces than the quad-fan median dual. Methodology: 2×2×2 carve,
    /// take both duals. Pass criteria: same cell count (one per vertex); the
    /// merged dual conserves volume exactly and every cell is closed (the hard
    /// gate — a wrong edge-star walk or boundary closure breaks one of these);
    /// its boundary area still equals the cube surface (6); and it really is
    /// leaner (fewer total faces than the quad-fan dual). Result: 27 cells;
    /// volume 1.0; validate Ok; boundary area 6.0; merged face count < median.
    #[test]
    fn merged_dual_is_closed_conserves_volume_and_has_fewer_faces() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let hex = carve_box(&p, &t, 0.5); // 8 hexes, 27 vertices
        let median = polyhedral_dual(&hex);
        let merged = polyhedral_dual_min_faces(&hex);

        assert_eq!(
            merged.cell_count(),
            hex.point_count(),
            "one dual cell per primal vertex"
        );
        assert!(
            (merged.total_volume() - hex.total_volume()).abs() < 1e-9,
            "merged dual conserves volume: {} vs {}",
            merged.total_volume(),
            hex.total_volume()
        );
        merged.validate().expect("merged dual cell is closed");

        // Internal/boundary split is consistent (every face shared by 2 cells or 1).
        for f in 0..merged.face_count() {
            let internal = f < merged.n_internal_faces();
            assert_eq!(
                merged.neighbour[f].is_some(),
                internal,
                "face {f} internal/boundary split"
            );
        }

        // Boundary coincides with the primal surface.
        let mut area = 0.0;
        for f in 0..merged.face_count() {
            if merged.neighbour[f].is_none() {
                area += merged.face_area_vector(f).length();
            }
        }
        assert!(
            (area - 6.0).abs() < 1e-9,
            "merged dual boundary area == cube surface: {area}"
        );

        // Leaner: one face per primal edge instead of one per (edge, cell).
        assert!(
            merged.face_count() < median.face_count(),
            "merged dual is face-minimal: {} < {}",
            merged.face_count(),
            median.face_count()
        );
    }

    /// V&V — the merged dual holds up on a larger block and stays genuinely
    /// polyhedral. Methodology: 3×3×3 carve -> merged dual; check exact volume,
    /// closure, and that the interior vertices' cells have > 6 faces. Result:
    /// volume 1.0; validate Ok; max faces/cell > 6.
    #[test]
    fn merged_dual_scales_and_stays_polyhedral() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let hex = carve_box(&p, &t, 1.0 / 3.0); // 27 hexes
        let merged = polyhedral_dual_min_faces(&hex);
        assert!(
            (merged.total_volume() - 1.0).abs() < 1e-9,
            "volume: {}",
            merged.total_volume()
        );
        merged.validate().expect("closed");
        let max_faces = cells_faces(&merged).iter().map(|c| c.len()).max().unwrap();
        assert!(
            max_faces > 6,
            "polyhedral cells present (max faces/cell = {max_faces})"
        );
    }

    /// V&V (gh:#1240) — **Route A: one boundary face per point per sector**.
    /// Methodology: a unit box carved into 3×3×3 hexes, its boundary in one
    /// `walls` patch, median dual with per-point boundary faces at 45°.
    /// Predictions (written before the first run, 2026-11-04): exactly as
    /// `outram-foam-mesh`'s `boundary_faces_one_per_point_per_sector` on the
    /// same block: a face-interior point's cell has 1 boundary face, an
    /// edge point 2, a corner 3 (the 90° edges are features at 45°); the
    /// interior faces are the per-corner dual's to the bit; volume 1 and
    /// boundary area 6 to 1e-9 (flat sides); every cell closed.
    #[test]
    fn per_point_dual_has_one_boundary_face_per_sector() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let hex = carve_box(&p, &t, 1.0 / 3.0);
        let corner = polyhedral_dual_min_faces(&hex);
        let point = polyhedral_dual_per_point(&hex, 45.0);
        point.validate().expect("closed");
        assert!((point.total_volume() - 1.0).abs() < 1e-9);
        let mut area = 0.0;
        let mut per_cell = vec![0usize; point.n_cells];
        for f in 0..point.face_count() {
            if point.neighbour[f].is_none() {
                area += point.face_area_vector(f).length();
                per_cell[point.owner[f]] += 1;
            }
        }
        assert!((area - 6.0).abs() < 1e-9, "boundary area {area}");
        for (v, &n) in per_cell.iter().enumerate() {
            let x = hex.points[v];
            let on = |c: f64| (c.abs() < 1e-12 || (c - 1.0).abs() < 1e-12) as usize;
            assert_eq!(n, on(x.x) + on(x.y) + on(x.z), "point {x:?}");
        }
        assert_eq!(point.n_internal_faces(), corner.n_internal_faces());
        assert_eq!(&point.faces[..point.n_internal_faces()], &corner.faces[..corner.n_internal_faces()]);
        // The class's rule: only the 8 box corners keep their vertex; the
        // straight edges are split on their chords, which pass through the
        // dropped points, so the volume is still exact.
        let corners = polyhedral_dual_per_point_with(&hex, 45.0, FeaturePointRule::Corners);
        corners.validate().expect("closed");
        assert!((corners.total_volume() - 1.0).abs() < 1e-9, "{}", corners.total_volume());
        assert_eq!(corners.n_boundary_faces(), point.n_boundary_faces());
        // A larger angle merges the 90° edges away: the corners are cut.
        let merged = polyhedral_dual_per_point(&hex, 120.0);
        assert!(merged.total_volume() < 1.0 - 1e-3, "merged {}", merged.total_volume());
    }
}
