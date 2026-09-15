// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com) — the `polyDualMesh` utility and its
// `Foam::meshDualiser` engine
// (applications/utilities/mesh/manipulation/polyDualMesh/,
//  src/dynamicMesh/meshCut/meshModifiers/... meshDualiser).
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
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

//! `polyDualMesh` — construct the polyhedral **dual** of a primal mesh.
//!
//! This is the OUTRAM PARK re-implementation of the OpenFOAM `polyDualMesh`
//! utility. The dual is the *cell-centre dual*: it exchanges the roles of the
//! primal mesh entities.
//!
//! | primal entity        | dual entity                                   |
//! |----------------------|-----------------------------------------------|
//! | cell                 | **point** (placed at the primal cell centroid)|
//! | point                | **cell**                                      |
//! | edge                 | **face** (a ring of dual points)              |
//! | face                 | edge (between two dual points)                |
//!
//! Concretely: every primal cell contributes one dual point at its centroid;
//! every primal point becomes one dual cell; and every primal **interior edge**
//! becomes one dual face — the closed ring of cell-centres of the cells that
//! share that edge, separating the dual cells of the edge's two endpoints.
//!
//! ## Boundary treatment
//!
//! The interior of the dual uses **only cell-centres** as vertices — that is the
//! defining property of `polyDualMesh` and it is honoured here exactly. The
//! domain boundary needs extra dual vertices so that the dual mesh reproduces
//! the primal boundary *surface* (and hence encloses exactly the same volume).
//! Three further classes of dual vertex are added on the boundary:
//!
//! - one **boundary-face centre** per primal boundary face,
//! - one **boundary-edge midpoint** per primal boundary edge,
//! - one **feature-point** vertex per primal boundary point that the
//!   feature-angle test flags as a geometric feature (an edge or corner of the
//!   surface).
//!
//! A primal **boundary edge** then becomes an *internal* dual face closed along
//! the surface through the two boundary-face centres and the edge midpoint; and
//! each primal **boundary point** contributes a dual **boundary** face — the
//! 2-D cell-centre dual of the surface mesh around that point.
//!
//! ## Feature angle
//!
//! `feature_angle_deg` is the OpenFOAM-style feature angle. A boundary point is
//! kept as an explicit dual vertex (preserving a sharp edge or corner) when the
//! surface bends across it by **more than** the feature angle; otherwise the
//! surface is locally flat there and the point is *merged* away — its ring of
//! boundary quads collapses to a single coplanar polygon. On a flat patch this
//! removes the redundant vertex without changing the enclosed volume; at a sharp
//! feature (e.g. a 90° block edge) merging it away would cut the corner and lose
//! volume, which is exactly why the feature is preserved. See the crate tests
//! for the numerical demonstration of both regimes.
//!
//! ## Scope / honesty
//!
//! - The interior construction (cell-centre rings around interior edges) is
//!   general — it works for any manifold polyhedral primal mesh.
//! - The boundary construction is implemented and verified for **manifold**
//!   surfaces (each boundary edge shared by exactly two boundary faces). It has
//!   been validated on structured hexahedral blocks; non-manifold surfaces and
//!   degenerate fans are rejected with [`MeshError::Construction`] rather than
//!   silently mis-meshed.
//! - Feature handling implements point retention/merge. It does **not** yet
//!   split coplanar boundary faces belonging to *different* patches that meet
//!   below the feature angle (not needed for the block case, where patch seams
//!   coincide with feature edges); this is the one documented boundary
//!   limitation.

use std::collections::HashMap;

use outram_foam_basic_lib::mesh::{BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind};
use outram_foam_basic_lib::primitives::Vector3;

use crate::MeshError;

// ────────────────────────────────────────────────────────────────────────────
// Primal mesh (input)
// ────────────────────────────────────────────────────────────────────────────

/// A general polyhedral primal mesh — the input to [`poly_dual_mesh`].
///
/// This is a full connectivity `polyMesh`: points, faces as **ordered vertex
/// loops**, `owner`/`neighbour` cell links, and boundary patches. (The
/// `outram-foam-basic-lib` [`FvMesh`] stores only *flat geometry* — cell/face
/// centres and areas — with no point/face-vertex connectivity, so the dual
/// construction, which is purely topological, needs this richer input type.)
///
/// ## Conventions (OpenFOAM)
///
/// - Faces are ordered **internal faces first**, then boundary faces grouped by
///   patch: `faces[0 .. n_internal_faces)` are internal, the rest boundary.
/// - `owner[f]` is defined for every face; `neighbour[f]` only for internal
///   faces (`neighbour.len() == n_internal_faces`).
/// - Each face's vertex loop is wound so its right-hand-rule normal points from
///   `owner` toward `neighbour` (internal) or **outward** from the domain
///   (boundary).
#[derive(Debug, Clone)]
pub struct PolyMesh {
    /// Mesh points `[m]`.
    pub points: Vec<Vector3>,
    /// Faces — each an ordered loop of point indices.
    pub faces: Vec<Vec<usize>>,
    /// Owning cell of each face (length `faces.len()`).
    pub owner: Vec<usize>,
    /// Neighbour cell of each internal face (length `n_internal_faces`).
    pub neighbour: Vec<usize>,
    /// Number of internal faces (faces with both owner and neighbour).
    pub n_internal_faces: usize,
    /// Boundary patch descriptors (reusing the basic-lib type).
    pub patches: Vec<BoundaryPatch>,
    /// Number of cells.
    pub n_cells: usize,
}

impl PolyMesh {
    /// Number of points.
    pub fn n_points(&self) -> usize {
        self.points.len()
    }

    /// True if face `f` is internal.
    pub fn is_internal_face(&self, f: usize) -> bool {
        f < self.n_internal_faces
    }

    /// Patch index owning boundary face `f`, or `None` for an internal face.
    fn patch_of_face(&self, f: usize) -> Option<usize> {
        if self.is_internal_face(f) {
            return None;
        }
        self.patches
            .iter()
            .position(|p| f >= p.start && f < p.start + p.size)
    }

    /// Total enclosed volume `[m³]` — sum of primal cell volumes.
    ///
    /// Used as the reference in the volume-preservation V&V check.
    pub fn total_volume(&self) -> f64 {
        let (face_c, face_s) = face_geometry(&self.points, &self.faces);
        let cell_faces = self.cell_faces_signed();
        (0..self.n_cells)
            .map(|c| {
                cell_volume_centroid(&cell_faces[c], &self.faces, &self.points, &face_c, &face_s).0
            })
            .sum()
    }

    /// Build `cell -> [(face, outward_sign)]`. `+1` for a face owned by the
    /// cell (its stored normal already points outward), `-1` for a neighbour.
    fn cell_faces_signed(&self) -> Vec<Vec<(usize, f64)>> {
        let mut cf = vec![Vec::new(); self.n_cells];
        for f in 0..self.faces.len() {
            cf[self.owner[f]].push((f, 1.0));
            if self.is_internal_face(f) {
                cf[self.neighbour[f]].push((f, -1.0));
            }
        }
        cf
    }

    /// Construct a uniform structured hexahedral block: `nx × ny × nz` cells
    /// filling `[0, lx] × [0, ly] × [0, lz]` `[m]`. Six boundary patches are
    /// created (`xMin`, `xMax`, `yMin`, `yMax`, `zMin`, `zMax`).
    ///
    /// This is the primary V&V fixture — a mesh whose dual is analytically
    /// known (interior dual cells are cubes of the cell spacing).
    pub fn structured_hex_block(
        nx: usize,
        ny: usize,
        nz: usize,
        lx: f64,
        ly: f64,
        lz: f64,
    ) -> Self {
        assert!(
            nx >= 1 && ny >= 1 && nz >= 1,
            "block needs >=1 cell per axis"
        );
        let (hx, hy, hz) = (lx / nx as f64, ly / ny as f64, lz / nz as f64);
        let pid = |i: usize, j: usize, k: usize| i + (nx + 1) * (j + (ny + 1) * k);
        let cid = |i: usize, j: usize, k: usize| i + nx * (j + ny * k);

        let mut points = vec![Vector3::default(); (nx + 1) * (ny + 1) * (nz + 1)];
        for k in 0..=nz {
            for j in 0..=ny {
                for i in 0..=nx {
                    points[pid(i, j, k)] =
                        Vector3::new(i as f64 * hx, j as f64 * hy, k as f64 * hz);
                }
            }
        }

        // Accumulate internal + boundary faces separately, then splice.
        let mut int_faces: Vec<Vec<usize>> = Vec::new();
        let mut int_owner: Vec<usize> = Vec::new();
        let mut int_neigh: Vec<usize> = Vec::new();
        // Boundary: (patch_index, loop, owner)
        let mut bnd: Vec<(usize, Vec<usize>, usize)> = Vec::new();

        // +X-normal quad (in the y-z plane), low cell on -x, high cell on +x.
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..=nx {
                    let loop_pos = vec![
                        pid(i, j, k),
                        pid(i, j + 1, k),
                        pid(i, j + 1, k + 1),
                        pid(i, j, k + 1),
                    ];
                    let low = if i > 0 { Some(cid(i - 1, j, k)) } else { None };
                    let high = if i < nx { Some(cid(i, j, k)) } else { None };
                    match (low, high) {
                        (Some(o), Some(n)) => {
                            int_faces.push(loop_pos);
                            int_owner.push(o);
                            int_neigh.push(n);
                        }
                        (None, Some(o)) => bnd.push((0, rev(&loop_pos), o)), // xMin, outward -x
                        (Some(o), None) => bnd.push((1, loop_pos, o)),       // xMax, outward +x
                        (None, None) => unreachable!(),
                    }
                }
            }
        }
        // +Y-normal quad (z-x plane).
        for k in 0..nz {
            for j in 0..=ny {
                for i in 0..nx {
                    let loop_pos = vec![
                        pid(i, j, k),
                        pid(i, j, k + 1),
                        pid(i + 1, j, k + 1),
                        pid(i + 1, j, k),
                    ];
                    let low = if j > 0 { Some(cid(i, j - 1, k)) } else { None };
                    let high = if j < ny { Some(cid(i, j, k)) } else { None };
                    match (low, high) {
                        (Some(o), Some(n)) => {
                            int_faces.push(loop_pos);
                            int_owner.push(o);
                            int_neigh.push(n);
                        }
                        (None, Some(o)) => bnd.push((2, rev(&loop_pos), o)), // yMin
                        (Some(o), None) => bnd.push((3, loop_pos, o)),       // yMax
                        (None, None) => unreachable!(),
                    }
                }
            }
        }
        // +Z-normal quad (x-y plane).
        for k in 0..=nz {
            for j in 0..ny {
                for i in 0..nx {
                    let loop_pos = vec![
                        pid(i, j, k),
                        pid(i + 1, j, k),
                        pid(i + 1, j + 1, k),
                        pid(i, j + 1, k),
                    ];
                    let low = if k > 0 { Some(cid(i, j, k - 1)) } else { None };
                    let high = if k < nz { Some(cid(i, j, k)) } else { None };
                    match (low, high) {
                        (Some(o), Some(n)) => {
                            int_faces.push(loop_pos);
                            int_owner.push(o);
                            int_neigh.push(n);
                        }
                        (None, Some(o)) => bnd.push((4, rev(&loop_pos), o)), // zMin
                        (Some(o), None) => bnd.push((5, loop_pos, o)),       // zMax
                        (None, None) => unreachable!(),
                    }
                }
            }
        }

        let n_internal_faces = int_faces.len();
        let mut faces = int_faces;
        let mut owner = int_owner;
        let neighbour = int_neigh;

        // Splice boundary faces grouped by patch, building patch descriptors.
        let patch_names = ["xMin", "xMax", "yMin", "yMax", "zMin", "zMax"];
        let mut patches = Vec::new();
        for (pi, name) in patch_names.iter().enumerate() {
            let start = faces.len();
            let mut size = 0;
            for (p, lp, o) in bnd.iter() {
                if *p == pi {
                    faces.push(lp.clone());
                    owner.push(*o);
                    size += 1;
                }
            }
            patches.push(BoundaryPatch::new(*name, start, size, PatchKind::Wall));
        }

        PolyMesh {
            points,
            faces,
            owner,
            neighbour,
            n_internal_faces,
            patches,
            n_cells: nx * ny * nz,
        }
    }
}

fn rev(v: &[usize]) -> Vec<usize> {
    v.iter().rev().copied().collect()
}

// ────────────────────────────────────────────────────────────────────────────
// Geometry helpers
// ────────────────────────────────────────────────────────────────────────────

/// Area-weighted centroid and (signed) area vector of a polygon, via a fan
/// triangulation from its first vertex. The area-vector sign follows the loop
/// winding (right-hand rule). Works for slightly non-planar loops.
fn polygon_centroid_area(pts: &[Vector3], loop_ids: &[usize]) -> (Vector3, Vector3) {
    let n = loop_ids.len();
    let p0 = pts[loop_ids[0]];
    let mut area = Vector3::default();
    let mut cent = Vector3::default();
    let mut atot = 0.0;
    for i in 1..n - 1 {
        let a = pts[loop_ids[i]] - p0;
        let b = pts[loop_ids[i + 1]] - p0;
        let tri_av = a.cross(b) * 0.5;
        let tri_a = tri_av.mag();
        let tri_c = (p0 + pts[loop_ids[i]] + pts[loop_ids[i + 1]]) * (1.0 / 3.0);
        area += tri_av;
        cent += tri_c * tri_a;
        atot += tri_a;
    }
    let centroid = if atot > 0.0 { cent / atot } else { p0 };
    (centroid, area)
}

/// Per-face centroids and area vectors for a whole face list.
fn face_geometry(pts: &[Vector3], faces: &[Vec<usize>]) -> (Vec<Vector3>, Vec<Vector3>) {
    let mut c = Vec::with_capacity(faces.len());
    let mut s = Vec::with_capacity(faces.len());
    for f in faces {
        let (cf, sf) = polygon_centroid_area(pts, f);
        c.push(cf);
        s.push(sf);
    }
    (c, s)
}

/// Signed volume and centroid of a cell from its oriented faces, by tetrahedral
/// decomposition about the mean of the cell's face centres. `sign` flips a
/// face's outward orientation (`+1` owner, `-1` neighbour).
fn cell_volume_centroid(
    cell_faces: &[(usize, f64)],
    faces: &[Vec<usize>],
    pts: &[Vector3],
    face_c: &[Vector3],
    _face_s: &[Vector3],
) -> (f64, Vector3) {
    if cell_faces.is_empty() {
        return (0.0, Vector3::default());
    }
    let mut apex = Vector3::default();
    for (f, _) in cell_faces {
        apex += face_c[*f];
    }
    apex /= cell_faces.len() as f64;

    let mut vol = 0.0;
    let mut cen = Vector3::default();
    for (f, sign) in cell_faces {
        let lp = &faces[*f];
        let n = lp.len();
        let v0 = pts[lp[0]];
        for i in 1..n - 1 {
            let (mut a, mut b) = (pts[lp[i]], pts[lp[i + 1]]);
            if *sign < 0.0 {
                std::mem::swap(&mut a, &mut b);
            }
            let da = v0 - apex;
            let db = a - apex;
            let dc = b - apex;
            let tv = da.dot(db.cross(dc)) / 6.0;
            let tc = (apex + v0 + a + b) * 0.25;
            vol += tv;
            cen += tc * tv;
        }
    }
    let centroid = if vol.abs() > 0.0 { cen / vol } else { apex };
    (vol, centroid)
}

// ────────────────────────────────────────────────────────────────────────────
// Primal topology (edges, boundary classification)
// ────────────────────────────────────────────────────────────────────────────

/// Undirected-edge key (sorted pair of point ids).
fn ekey(a: usize, b: usize) -> (usize, usize) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

struct PrimalTopo {
    /// Edge endpoints (sorted) per edge id.
    edge_pts: Vec<(usize, usize)>,
    /// Faces incident to each edge.
    edge_faces: Vec<Vec<usize>>,
    /// Edge id for a sorted point pair.
    edge_id: HashMap<(usize, usize), usize>,
    /// Boundary edge flag per edge.
    edge_is_boundary: Vec<bool>,
    /// Boundary point flag per point.
    point_is_boundary: Vec<bool>,
    /// Boundary faces incident to each boundary point.
    point_bfaces: Vec<Vec<usize>>,
    /// Cell centroids (dual point positions) and primal geometry.
    cell_centres: Vec<Vector3>,
    face_c: Vec<Vector3>,
    face_s: Vec<Vector3>,
}

fn build_topo(m: &PolyMesh) -> Result<PrimalTopo, MeshError> {
    let (face_c, face_s) = face_geometry(&m.points, &m.faces);

    // Edges from face loops.
    let mut edge_id: HashMap<(usize, usize), usize> = HashMap::new();
    let mut edge_pts: Vec<(usize, usize)> = Vec::new();
    let mut edge_faces: Vec<Vec<usize>> = Vec::new();
    for (f, lp) in m.faces.iter().enumerate() {
        let n = lp.len();
        for i in 0..n {
            let (a, b) = (lp[i], lp[(i + 1) % n]);
            let key = ekey(a, b);
            let id = *edge_id.entry(key).or_insert_with(|| {
                edge_pts.push(key);
                edge_faces.push(Vec::new());
                edge_pts.len() - 1
            });
            if !edge_faces[id].contains(&f) {
                edge_faces[id].push(f);
            }
        }
    }

    let mut point_is_boundary = vec![false; m.n_points()];
    let mut point_bfaces = vec![Vec::new(); m.n_points()];
    for f in m.n_internal_faces..m.faces.len() {
        for &p in &m.faces[f] {
            point_is_boundary[p] = true;
            if !point_bfaces[p].contains(&f) {
                point_bfaces[p].push(f);
            }
        }
    }

    let mut edge_is_boundary = vec![false; edge_pts.len()];
    for (e, fs) in edge_faces.iter().enumerate() {
        let n_bnd = fs.iter().filter(|&&f| !m.is_internal_face(f)).count();
        edge_is_boundary[e] = n_bnd > 0;
        // Manifold surface: a boundary edge must sit on exactly two boundary faces.
        if n_bnd != 0 && n_bnd != 2 {
            return Err(MeshError::Construction(format!(
                "non-manifold boundary edge {e} lies on {n_bnd} boundary faces (expected 2)"
            )));
        }
    }

    // Cell centroids as dual point positions.
    let cell_faces = m.cell_faces_signed();
    let mut cell_centres = vec![Vector3::default(); m.n_cells];
    for c in 0..m.n_cells {
        cell_centres[c] =
            cell_volume_centroid(&cell_faces[c], &m.faces, &m.points, &face_c, &face_s).1;
    }

    Ok(PrimalTopo {
        edge_pts,
        edge_faces,
        edge_id,
        edge_is_boundary,
        point_is_boundary,
        point_bfaces,
        cell_centres,
        face_c,
        face_s,
    })
}

// ────────────────────────────────────────────────────────────────────────────
// Dual mesh (output)
// ────────────────────────────────────────────────────────────────────────────

/// The polyhedral dual mesh produced by [`poly_dual_mesh`].
///
/// Full connectivity: `points` (dual vertices), `faces` (vertex loops), and per
/// face an `owner` dual-cell id (== the primal point index it surrounds) plus an
/// optional `neighbour` (`None` ⇒ boundary face) and an optional source patch
/// index. Convert to a flat `outram-foam-basic-lib` [`FvMesh`] with
/// [`DualMesh::to_fv_mesh`].
#[derive(Debug, Clone)]
pub struct DualMesh {
    /// Dual points `[m]`.
    pub points: Vec<Vector3>,
    /// Dual faces (ordered vertex loops).
    pub faces: Vec<Vec<usize>>,
    /// Owning dual cell of each face (== primal point index).
    pub owner: Vec<usize>,
    /// Neighbour dual cell of each face; `None` for boundary faces.
    pub neighbour: Vec<Option<usize>>,
    /// Source primal patch index for each boundary face; `None` for internal.
    pub face_patch: Vec<Option<usize>>,
    /// Number of dual cells (== number of primal points).
    pub n_cells: usize,
    /// Whether each dual cell corresponds to an interior primal point.
    pub cell_is_interior: Vec<bool>,
}

impl DualMesh {
    /// Number of dual cells whose primal point is interior (not on the
    /// boundary). For a `polyDualMesh` this equals the count of primal interior
    /// points — the number of *fully interior* dual cells.
    pub fn n_interior_cells(&self) -> usize {
        self.cell_is_interior.iter().filter(|&&b| b).count()
    }

    /// Build `cell -> [(face, outward_sign)]`.
    fn cell_faces_signed(&self) -> Vec<Vec<(usize, f64)>> {
        let mut cf = vec![Vec::new(); self.n_cells];
        for f in 0..self.faces.len() {
            cf[self.owner[f]].push((f, 1.0));
            if let Some(nb) = self.neighbour[f] {
                cf[nb].push((f, -1.0));
            }
        }
        cf
    }

    /// Total enclosed volume `[m³]` — sum of dual cell volumes. Equals the primal
    /// total volume to machine precision when the boundary is faithfully
    /// reproduced (all features preserved).
    pub fn total_volume(&self) -> f64 {
        let (fc, fs) = face_geometry(&self.points, &self.faces);
        let cf = self.cell_faces_signed();
        (0..self.n_cells)
            .map(|c| cell_volume_centroid(&cf[c], &self.faces, &self.points, &fc, &fs).0)
            .sum()
    }

    /// Largest closure residual over all dual cells: `max_c |Σ_f Sf_out|`.
    /// A closed polyhedron has zero net outward area, so this should be ~0
    /// (relative to a face area). Cells with no faces are skipped.
    pub fn max_closure_residual(&self) -> f64 {
        let (_, fs) = face_geometry(&self.points, &self.faces);
        let cf = self.cell_faces_signed();
        let mut worst: f64 = 0.0;
        for faces in &cf {
            if faces.is_empty() {
                continue;
            }
            let mut sum = Vector3::default();
            for (f, sign) in faces {
                sum += fs[*f] * *sign;
            }
            worst = worst.max(sum.mag());
        }
        worst
    }

    /// Verify that every non-empty dual cell is a genus-0 closed polyhedron
    /// (Euler characteristic `V - E + F == 2`). Returns the first offending
    /// `(cell, chi)` or `None` if all cells pass.
    pub fn first_bad_euler(&self) -> Option<(usize, i64)> {
        let cf = self.cell_faces_signed();
        for (c, faces) in cf.iter().enumerate() {
            if faces.is_empty() {
                continue;
            }
            let mut verts: std::collections::HashSet<usize> = std::collections::HashSet::new();
            let mut edges: std::collections::HashSet<(usize, usize)> =
                std::collections::HashSet::new();
            for (f, _) in faces {
                let lp = &self.faces[*f];
                let n = lp.len();
                for i in 0..n {
                    verts.insert(lp[i]);
                    edges.insert(ekey(lp[i], lp[(i + 1) % n]));
                }
            }
            let chi = verts.len() as i64 - edges.len() as i64 + faces.len() as i64;
            if chi != 2 {
                return Some((c, chi));
            }
        }
        None
    }

    /// Convert the dual to the writable `outram-foam-basic-lib`
    /// [`PolyMesh`](outram_foam_basic_lib::io::PolyMesh) — the on-disk
    /// `constant/polyMesh` representation.
    ///
    /// Faces are re-ordered into OpenFOAM order (internal first, then boundary
    /// grouped by patch) exactly as [`to_fv_mesh`](Self::to_fv_mesh) does, but
    /// the point/face connectivity is kept rather than being flattened to
    /// geometry. `patch_names[i]` names the patch built from primal patch `i`;
    /// missing names default to `patch<i>`, and every patch is created as
    /// [`PatchKind::Wall`].
    ///
    /// Use it to write a polyhedral dual to an OpenFOAM case
    /// ([`PolyMesh::write`](outram_foam_basic_lib::io::PolyMesh::write)) or to
    /// grade it with [`assess_quality`](crate::mesh_quality::assess_quality) —
    /// dual meshes are the ones most likely to be badly non-orthogonal, so
    /// checking them is worthwhile.
    ///
    /// # Errors
    /// Never fails today; the `Result` is kept so a future validity check can
    /// be added without a breaking change.
    pub fn to_foam_poly_mesh(
        &self,
        patch_names: &[String],
    ) -> Result<outram_foam_basic_lib::io::PolyMesh, MeshError> {
        let nf = self.faces.len();
        let mut order: Vec<usize> = (0..nf).filter(|&f| self.neighbour[f].is_some()).collect();
        let n_internal = order.len();
        let n_patches = patch_names.len();
        let mut patch_faces: Vec<Vec<usize>> = vec![Vec::new(); n_patches.max(1)];
        for f in 0..nf {
            if self.neighbour[f].is_none() {
                let pi = self.face_patch[f].unwrap_or(0);
                patch_faces[pi].push(f);
            }
        }
        let mut patches = Vec::new();
        for (pi, pf) in patch_faces.iter().enumerate() {
            let start = order.len();
            order.extend_from_slice(pf);
            let name = patch_names
                .get(pi)
                .cloned()
                .unwrap_or_else(|| format!("patch{pi}"));
            patches.push(BoundaryPatch::new(name, start, pf.len(), PatchKind::Wall));
        }

        let faces = order
            .iter()
            .map(|&old_f| outram_foam_basic_lib::io::MeshFace {
                verts: self.faces[old_f].clone(),
                owner: self.owner[old_f],
                neighbour: self.neighbour[old_f],
            })
            .collect();

        Ok(outram_foam_basic_lib::io::PolyMesh {
            points: self.points.clone(),
            faces,
            n_internal_faces: n_internal,
            n_cells: self.n_cells,
            patches,
        })
    }

    /// Convert the dual to a flat `outram-foam-basic-lib` [`FvMesh`]: faces are
    /// re-ordered internal-first then by patch, geometry (centres, area vectors,
    /// volumes) is computed from the connectivity, and validity is checked by
    /// [`FvMesh::validate`].
    pub fn to_fv_mesh(&self, patch_names: &[String]) -> Result<FvMesh, MeshError> {
        let nf = self.faces.len();
        // Order: internal faces first, then boundary grouped by patch index.
        let mut order: Vec<usize> = (0..nf).filter(|&f| self.neighbour[f].is_some()).collect();
        let n_internal = order.len();
        let n_patches = patch_names.len();
        let mut patch_faces: Vec<Vec<usize>> = vec![Vec::new(); n_patches.max(1)];
        for f in 0..nf {
            if self.neighbour[f].is_none() {
                let pi = self.face_patch[f].unwrap_or(0);
                patch_faces[pi].push(f);
            }
        }
        let mut patches = Vec::new();
        for (pi, pf) in patch_faces.iter().enumerate() {
            let start = order.len();
            order.extend_from_slice(pf);
            let name = patch_names
                .get(pi)
                .cloned()
                .unwrap_or_else(|| format!("patch{pi}"));
            patches.push(BoundaryPatch::new(name, start, pf.len(), PatchKind::Wall));
        }
        // Drop empty trailing patches so validate()'s contiguous coverage holds
        // (empty patches are fine — start==prev end, size 0 — so keep them).

        let ordered_loops: Vec<Vec<usize>> = order.iter().map(|&f| self.faces[f].clone()).collect();
        let (fc, fs) = face_geometry(&self.points, &ordered_loops);

        let mut owner = Vec::with_capacity(nf);
        let mut neighbour = Vec::with_capacity(n_internal);
        let mut face_area_vectors = Vec::with_capacity(nf);
        let mut face_centres = Vec::with_capacity(nf);
        for (new_f, &old_f) in order.iter().enumerate() {
            owner.push(self.owner[old_f]);
            if let Some(nb) = self.neighbour[old_f] {
                neighbour.push(nb);
            }
            face_area_vectors.push(fs[new_f]);
            face_centres.push(fc[new_f]);
        }

        // Cell geometry from the re-ordered faces.
        let mut cell_faces: Vec<Vec<(usize, f64)>> = vec![Vec::new(); self.n_cells];
        for new_f in 0..nf {
            cell_faces[owner[new_f]].push((new_f, 1.0));
            if new_f < n_internal {
                cell_faces[neighbour[new_f]].push((new_f, -1.0));
            }
        }
        let mut cell_volumes = vec![0.0; self.n_cells];
        let mut cell_centres = vec![Vector3::default(); self.n_cells];
        for c in 0..self.n_cells {
            let (v, cc) =
                cell_volume_centroid(&cell_faces[c], &ordered_loops, &self.points, &fc, &fs);
            cell_volumes[c] = v;
            cell_centres[c] = cc;
        }

        FvMeshBuilder::new()
            .n_cells(self.n_cells)
            .n_internal_faces(n_internal)
            .owner(owner)
            .neighbour(neighbour)
            .patches(patches)
            .cell_volumes(cell_volumes)
            .cell_centres(cell_centres)
            .face_area_vectors(face_area_vectors)
            .face_centres(face_centres)
            .build()
            .map_err(|e| MeshError::Construction(e.to_string()))
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Dual construction
// ────────────────────────────────────────────────────────────────────────────

/// Construct the polyhedral dual of `primal`.
///
/// `feature_angle_deg` controls boundary-point retention (see the module docs):
/// a boundary point is kept as an explicit dual vertex when the surface bends
/// across it by more than this angle; otherwise it is merged into a coplanar
/// boundary polygon.
///
/// # Errors
/// Returns [`MeshError::Construction`] for a non-manifold boundary or an
/// inconsistent edge fan.
pub fn poly_dual_mesh(primal: &PolyMesh, feature_angle_deg: f64) -> Result<DualMesh, MeshError> {
    let topo = build_topo(primal)?;

    // Feature classification of boundary points.
    let is_feature_point = classify_feature_points(primal, &topo, feature_angle_deg);

    // ── Dual point layout ───────────────────────────────────────────────────
    // [0 .. n_cells)                         : cell centres
    // boundary-face centres, boundary-edge midpoints, feature points appended.
    let n_cells = primal.n_cells;
    let mut points: Vec<Vector3> = topo.cell_centres.clone();

    let mut dp_bface: HashMap<usize, usize> = HashMap::new();
    for f in primal.n_internal_faces..primal.faces.len() {
        dp_bface.insert(f, points.len());
        points.push(topo.face_c[f]);
    }
    let mut dp_bedge: HashMap<usize, usize> = HashMap::new();
    for (e, &is_b) in topo.edge_is_boundary.iter().enumerate() {
        if is_b {
            let (a, b) = topo.edge_pts[e];
            dp_bedge.insert(e, points.len());
            points.push((primal.points[a] + primal.points[b]) * 0.5);
        }
    }
    let mut dp_bpoint: HashMap<usize, usize> = HashMap::new();
    #[allow(clippy::needless_range_loop)]
    for p in 0..primal.n_points() {
        if topo.point_is_boundary[p] && is_feature_point[p] {
            dp_bpoint.insert(p, points.len());
            points.push(primal.points[p]);
        }
    }

    let _ = n_cells; // clarity: cell dual point id == cell index
    let dp_cell = |c: usize| c;

    let mut faces: Vec<Vec<usize>> = Vec::new();
    let mut owner: Vec<usize> = Vec::new();
    let mut neighbour: Vec<Option<usize>> = Vec::new();
    let mut face_patch: Vec<Option<usize>> = Vec::new();

    // ── (1) Interior edges → cell-centre ring faces (internal) ───────────────
    for e in 0..topo.edge_pts.len() {
        if topo.edge_is_boundary[e] {
            continue;
        }
        let ring = interior_edge_cell_ring(primal, &topo, e)?;
        if ring.len() < 3 {
            return Err(MeshError::Construction(format!(
                "interior edge {e} produced a degenerate {}-cell ring",
                ring.len()
            )));
        }
        let (p0, p1) = topo.edge_pts[e];
        let loop_ids: Vec<usize> = ring.iter().map(|&c| dp_cell(c)).collect();
        push_internal(
            &primal.points,
            &points,
            &mut faces,
            &mut owner,
            &mut neighbour,
            &mut face_patch,
            loop_ids,
            p0,
            p1,
        );
    }

    // ── (2) Boundary edges → internal dual faces closed along the surface ────
    for e in 0..topo.edge_pts.len() {
        if !topo.edge_is_boundary[e] {
            continue;
        }
        let (bf_a, cells, bf_b) = boundary_edge_fan(primal, &topo, e)?;
        let mut loop_ids = vec![dp_bedge[&e], dp_bface[&bf_a]];
        loop_ids.extend(cells.iter().map(|&c| dp_cell(c)));
        loop_ids.push(dp_bface[&bf_b]);
        let (p0, p1) = topo.edge_pts[e];
        push_internal(
            &primal.points,
            &points,
            &mut faces,
            &mut owner,
            &mut neighbour,
            &mut face_patch,
            loop_ids,
            p0,
            p1,
        );
    }

    // ── (3) Boundary points → dual boundary faces (surface cell-centre dual) ─
    #[allow(clippy::needless_range_loop)]
    for p in 0..primal.n_points() {
        if !topo.point_is_boundary[p] {
            continue;
        }
        build_boundary_faces_for_point(
            primal,
            &topo,
            p,
            is_feature_point[p],
            &dp_bface,
            &dp_bedge,
            &dp_bpoint,
            &points,
            &mut faces,
            &mut owner,
            &mut neighbour,
            &mut face_patch,
        )?;
    }

    let cell_is_interior: Vec<bool> = (0..primal.n_points())
        .map(|p| !topo.point_is_boundary[p])
        .collect();

    Ok(DualMesh {
        points,
        faces,
        owner,
        neighbour,
        face_patch,
        n_cells: primal.n_points(),
        cell_is_interior,
    })
}

/// A boundary point is a *feature* if the boundary faces meeting there are not
/// all coplanar within `feature_angle_deg` — i.e. the maximum angle between any
/// two incident boundary-face outward normals exceeds the threshold. Such a
/// point sits on a surface edge or corner and is preserved as a dual vertex.
fn classify_feature_points(m: &PolyMesh, topo: &PrimalTopo, feature_angle_deg: f64) -> Vec<bool> {
    let thr = feature_angle_deg.to_radians().cos(); // dot >= thr  ⇔  angle <= feature
    let mut is_feature = vec![false; m.n_points()];
    #[allow(clippy::needless_range_loop)]
    for p in 0..m.n_points() {
        if !topo.point_is_boundary[p] {
            continue;
        }
        let normals: Vec<Vector3> = topo.point_bfaces[p]
            .iter()
            .map(|&f| topo.face_s[f].normalise(1e-30))
            .collect();
        let mut feature = false;
        'outer: for i in 0..normals.len() {
            for j in (i + 1)..normals.len() {
                if normals[i].dot(normals[j]) < thr {
                    feature = true;
                    break 'outer;
                }
            }
        }
        is_feature[p] = feature;
    }
    is_feature
}

/// Ordered cyclic ring of cells around an **interior** edge.
fn interior_edge_cell_ring(
    m: &PolyMesh,
    topo: &PrimalTopo,
    e: usize,
) -> Result<Vec<usize>, MeshError> {
    let faces = &topo.edge_faces[e];
    let other_cell = |f: usize, c: usize| -> usize {
        if m.owner[f] == c {
            m.neighbour[f]
        } else {
            m.owner[f]
        }
    };
    let borders = |f: usize, c: usize| -> bool {
        m.owner[f] == c || (m.is_internal_face(f) && m.neighbour[f] == c)
    };

    let start_face = faces[0];
    let first_cell = m.owner[start_face];
    let mut cur_face = start_face;
    let mut cur_cell = first_cell;
    let mut ring = Vec::new();
    for _ in 0..faces.len() + 1 {
        ring.push(cur_cell);
        let next = faces
            .iter()
            .copied()
            .find(|&f| f != cur_face && borders(f, cur_cell))
            .ok_or_else(|| {
                MeshError::Construction(format!("interior edge {e}: broken cell fan"))
            })?;
        let nxt_cell = other_cell(next, cur_cell);
        cur_face = next;
        cur_cell = nxt_cell;
        if cur_cell == first_cell {
            break;
        }
    }
    Ok(ring)
}

/// Open fan around a **boundary** edge: `(boundary_face_a, [cells...],
/// boundary_face_b)`, walking from one boundary face through the interior cells
/// to the other.
fn boundary_edge_fan(
    m: &PolyMesh,
    topo: &PrimalTopo,
    e: usize,
) -> Result<(usize, Vec<usize>, usize), MeshError> {
    let faces = &topo.edge_faces[e];
    let bnd: Vec<usize> = faces
        .iter()
        .copied()
        .filter(|&f| !m.is_internal_face(f))
        .collect();
    if bnd.len() != 2 {
        return Err(MeshError::Construction(format!(
            "boundary edge {e}: expected 2 boundary faces, found {}",
            bnd.len()
        )));
    }
    let bf_a = *bnd.iter().min().unwrap();
    let bf_b = *bnd.iter().max().unwrap();
    let other_cell = |f: usize, c: usize| -> usize {
        if m.owner[f] == c {
            m.neighbour[f]
        } else {
            m.owner[f]
        }
    };
    let borders = |f: usize, c: usize| -> bool {
        m.owner[f] == c || (m.is_internal_face(f) && m.neighbour[f] == c)
    };

    let mut cur_face = bf_a;
    let mut cur_cell = m.owner[bf_a];
    let mut cells = Vec::new();
    for _ in 0..faces.len() + 1 {
        cells.push(cur_cell);
        let next = faces
            .iter()
            .copied()
            .find(|&f| f != cur_face && borders(f, cur_cell))
            .ok_or_else(|| {
                MeshError::Construction(format!("boundary edge {e}: broken cell fan"))
            })?;
        if !m.is_internal_face(next) {
            // Reached the other boundary face.
            if next != bf_b {
                return Err(MeshError::Construction(format!(
                    "boundary edge {e}: fan closed on the wrong boundary face"
                )));
            }
            return Ok((bf_a, cells, bf_b));
        }
        cur_cell = other_cell(next, cur_cell);
        cur_face = next;
    }
    Err(MeshError::Construction(format!(
        "boundary edge {e}: fan did not terminate"
    )))
}

/// Push an internal dual face, orienting the loop so its area vector points from
/// the dual cell of `p0` toward that of `p1` (owner→neighbour), matching the
/// primal edge direction `P(p1) - P(p0)`.
#[allow(clippy::too_many_arguments)]
fn push_internal(
    primal_pts: &[Vector3],
    dual_pts: &[Vector3],
    faces: &mut Vec<Vec<usize>>,
    owner: &mut Vec<usize>,
    neighbour: &mut Vec<Option<usize>>,
    face_patch: &mut Vec<Option<usize>>,
    mut loop_ids: Vec<usize>,
    p0: usize,
    p1: usize,
) {
    let dir = primal_pts[p1] - primal_pts[p0];
    let (_, area) = polygon_centroid_area(dual_pts, &loop_ids);
    if area.dot(dir) < 0.0 {
        loop_ids.reverse();
    }
    faces.push(loop_ids);
    owner.push(p0);
    neighbour.push(Some(p1));
    face_patch.push(None);
}

/// Emit the dual **boundary** face(s) for boundary point `p`.
///
/// If `p` is a feature point, one quad `[p, em(e1), fc(bf), em(e2)]` per incident
/// boundary face is emitted (sharp edges/corners preserved). Otherwise the
/// surface is locally flat and the quads are merged into a single ring polygon
/// `[fc, em, fc, em, …]` around `p`, dropping `p`.
#[allow(clippy::too_many_arguments)]
fn build_boundary_faces_for_point(
    m: &PolyMesh,
    topo: &PrimalTopo,
    p: usize,
    feature: bool,
    dp_bface: &HashMap<usize, usize>,
    dp_bedge: &HashMap<usize, usize>,
    dp_bpoint: &HashMap<usize, usize>,
    dual_pts: &[Vector3],
    faces: &mut Vec<Vec<usize>>,
    owner: &mut Vec<usize>,
    neighbour: &mut Vec<Option<usize>>,
    face_patch: &mut Vec<Option<usize>>,
) -> Result<(), MeshError> {
    let bfaces = &topo.point_bfaces[p];
    // Two boundary edges of face bf at point p → (edge_before, edge_after).
    let edges_at = |bf: usize| -> Result<(usize, usize), MeshError> {
        let lp = &m.faces[bf];
        let n = lp.len();
        let pos = lp.iter().position(|&q| q == p).ok_or_else(|| {
            MeshError::Construction(format!("point {p} not on its boundary face {bf}"))
        })?;
        let prev = lp[(pos + n - 1) % n];
        let next = lp[(pos + 1) % n];
        Ok((topo.edge_id[&ekey(p, prev)], topo.edge_id[&ekey(p, next)]))
    };

    if feature {
        for &bf in bfaces {
            let (e1, e2) = edges_at(bf)?;
            let loop_ids = vec![dp_bpoint[&p], dp_bedge[&e1], dp_bface[&bf], dp_bedge[&e2]];
            push_boundary(
                dual_pts,
                topo.face_s[bf],
                faces,
                owner,
                neighbour,
                face_patch,
                loop_ids,
                p,
                m.patch_of_face(bf),
            );
        }
    } else {
        // Walk the boundary faces around p into a single cycle through their
        // shared boundary edges, then build the alternating ring.
        let ring = order_faces_around_point(m, topo, p, bfaces)?;
        let mut loop_ids = Vec::with_capacity(ring.len() * 2);
        for i in 0..ring.len() {
            let bf = ring[i];
            let bf_next = ring[(i + 1) % ring.len()];
            loop_ids.push(dp_bface[&bf]);
            // shared boundary edge between bf and bf_next at p
            let shared = shared_boundary_edge(m, topo, p, bf, bf_next)?;
            loop_ids.push(dp_bedge[&shared]);
        }
        // Use the mean incident normal for outward orientation.
        let mut nrm = Vector3::default();
        for &bf in bfaces {
            nrm += topo.face_s[bf];
        }
        push_boundary(
            dual_pts,
            nrm,
            faces,
            owner,
            neighbour,
            face_patch,
            loop_ids,
            p,
            m.patch_of_face(ring[0]),
        );
    }
    Ok(())
}

/// Order the boundary faces incident to `p` into a single cycle, consecutive
/// faces sharing a boundary edge at `p`.
fn order_faces_around_point(
    m: &PolyMesh,
    topo: &PrimalTopo,
    p: usize,
    bfaces: &[usize],
) -> Result<Vec<usize>, MeshError> {
    // adjacency: face -> its two neighbour faces (sharing a boundary edge at p)
    let mut adj: HashMap<usize, Vec<usize>> = HashMap::new();
    for &a in bfaces {
        for &b in bfaces {
            if a < b && shared_boundary_edge(m, topo, p, a, b).is_ok() {
                adj.entry(a).or_default().push(b);
                adj.entry(b).or_default().push(a);
            }
        }
    }
    let start = bfaces[0];
    let mut ring = vec![start];
    let mut prev = usize::MAX;
    let mut cur = start;
    for _ in 0..bfaces.len() {
        let nbrs = adj.get(&cur).cloned().unwrap_or_default();
        let next = nbrs.into_iter().find(|&f| f != prev);
        match next {
            Some(nf) if nf != start => {
                ring.push(nf);
                prev = cur;
                cur = nf;
            }
            _ => break,
        }
    }
    if ring.len() != bfaces.len() {
        return Err(MeshError::Construction(format!(
            "point {p}: boundary faces do not form a single ring ({} of {})",
            ring.len(),
            bfaces.len()
        )));
    }
    Ok(ring)
}

/// The boundary edge at `p` shared by boundary faces `a` and `b`, if any.
fn shared_boundary_edge(
    m: &PolyMesh,
    topo: &PrimalTopo,
    p: usize,
    a: usize,
    b: usize,
) -> Result<usize, MeshError> {
    let edges_of = |bf: usize| -> Vec<usize> {
        let lp = &m.faces[bf];
        let n = lp.len();
        let pos = lp.iter().position(|&q| q == p);
        match pos {
            Some(pos) => {
                let prev = lp[(pos + n - 1) % n];
                let next = lp[(pos + 1) % n];
                vec![topo.edge_id[&ekey(p, prev)], topo.edge_id[&ekey(p, next)]]
            }
            None => Vec::new(),
        }
    };
    let ea = edges_of(a);
    let eb = edges_of(b);
    ea.into_iter()
        .find(|e| eb.contains(e))
        .ok_or_else(|| MeshError::Construction(format!("faces {a},{b} share no edge at {p}")))
}

/// Push a boundary dual face, orienting the loop so its area vector points
/// outward (along `outward_ref`, the primal boundary normal).
#[allow(clippy::too_many_arguments)]
fn push_boundary(
    dual_pts: &[Vector3],
    outward_ref: Vector3,
    faces: &mut Vec<Vec<usize>>,
    owner: &mut Vec<usize>,
    neighbour: &mut Vec<Option<usize>>,
    face_patch: &mut Vec<Option<usize>>,
    mut loop_ids: Vec<usize>,
    p: usize,
    patch: Option<usize>,
) {
    let (_, area) = polygon_centroid_area(dual_pts, &loop_ids);
    if area.dot(outward_ref) < 0.0 {
        loop_ids.reverse();
    }
    faces.push(loop_ids);
    owner.push(p);
    neighbour.push(None);
    face_patch.push(patch);
}
