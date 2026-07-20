// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
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

//! Castellation — Phase 1 of `snappyHexMesh` (**implemented**).
//!
//! Castellation turns a uniform background hex mesh into a body-fitted "castle
//! wall" staircase around a surface, in two steps:
//!
//! 1. **Octree refinement.** Level-0 background cells near the surface are
//!    recursively split into 8 children until they reach the requested surface
//!    refinement level. Proximity is measured with
//!    [`TriangleSoup::distance_to`], so a shell of small cells forms around the
//!    STL while the far field stays coarse.
//! 2. **Region removal.** Each surviving leaf cell is kept or discarded by an
//!    inside/outside test ([`TriangleSoup::contains_point`]) against the
//!    `keep_point` (`locationInMesh` in OpenFOAM): cells on the same side of the
//!    closed surface as `keep_point` are kept, the rest are carved away. The
//!    faces newly exposed by removal become a boundary patch on the surface.
//!
//! ## Conforming split-hex output (2:1 and beyond)
//!
//! When a refined (fine) cell abuts a coarse cell, the coarse cell's shared side
//! is emitted as several smaller faces — one per fine neighbour — so the coarse
//! cell simply becomes a polyhedron with more than six faces. This is exactly
//! OpenFOAM's split-hex handling of hanging nodes, and it keeps the output a
//! valid, closed [`FvMesh`] with no T-junctions inside any single face. Faces
//! are always emitted at the **finer** of the two neighbours' resolutions.
//!
//! ## What is *not* modelled here
//!
//! Feature-edge refinement, `nCellsBetweenLevels` gap filling, and 2:1 level
//! balancing are not implemented — the face-emission scheme above stays valid
//! for arbitrary level jumps, so balancing is a mesh-quality nicety rather than
//! a correctness requirement. Snapping and layer addition are separate phases
//! (see [`crate::snappy_hex_mesh::snapping`] and
//! [`crate::snappy_hex_mesh::layers`]).

use std::collections::HashMap;

use outram_foam_basic_lib::mesh::{BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind};
use outram_foam_basic_lib::primitives::Vector3;

use crate::snappy_hex_mesh::background::{BackgroundMesh, Bounds};
use crate::snappy_hex_mesh::poly_topology::PolyPatchMesh;
use crate::snappy_hex_mesh::stl::TriangleSoup;
use crate::MeshError;

/// User controls for the castellation phase.
///
/// Mirrors the `castellatedMeshControls` block of a `snappyHexMeshDict`, reduced
/// to the parameters this Phase-1 implementation honours.
#[derive(Debug, Clone)]
pub struct CastellationControls {
    /// The uniform background hex mesh to refine.
    pub background: BackgroundMesh,
    /// Target octree refinement level applied to cells near the surface.
    /// Level `n` cells are `2ⁿ` times finer than the background per axis.
    pub surface_level: usize,
    /// Width of the refinement band around the surface [m]. A cell is refined
    /// while its centre lies within this distance of the surface. `None`
    /// auto-selects a one-cell band (the cell's half space-diagonal at its
    /// current level), which refines the cells the surface actually passes
    /// through plus their immediate shell.
    pub refinement_distance: Option<f64>,
    /// `locationInMesh` — a point in the region to KEEP. Cells on the same side
    /// of the closed surface as this point survive region removal.
    pub keep_point: Vector3,
    /// Name of the boundary patch created on the carved surface.
    pub surface_patch_name: String,
}

impl CastellationControls {
    /// Convenience constructor with the common defaults (auto band, surface
    /// patch named `"surface"`).
    pub fn new(background: BackgroundMesh, surface_level: usize, keep_point: Vector3) -> Self {
        Self {
            background,
            surface_level,
            refinement_distance: None,
            keep_point,
            surface_patch_name: "surface".to_string(),
        }
    }
}

/// One octree leaf cell: a cube on the level-`level` integer grid.
///
/// A leaf at `level` occupies `S = 2^(max_level - level)` finest-grid voxels per
/// axis, with lower voxel corner `(i, j, k) · S`.
#[derive(Debug, Clone, Copy)]
struct Leaf {
    level: usize,
    i: usize,
    j: usize,
    k: usize,
}

/// A boundary face lying on the carved surface, retained (with its corner
/// points) so the Phase-2 snapping stub has explicit geometry to project.
#[derive(Debug, Clone)]
pub struct SurfaceFace {
    /// Owning kept-cell index in the output [`FvMesh`].
    pub owner_cell: usize,
    /// The four corner point indices into [`CastellatedMesh::points`],
    /// counter-clockwise as seen from outside the mesh.
    pub corners: [usize; 4],
}

/// Result of castellation: a valid refined [`FvMesh`] plus enough octree/point
/// bookkeeping for the later phases to build on.
#[derive(Debug, Clone)]
pub struct CastellatedMesh {
    /// The refined, region-removed finite-volume mesh (validated).
    pub fv_mesh: FvMesh,
    /// Deduplicated mesh points [m] (corners of the kept cells). `FvMesh` itself
    /// stores only geometry, so these are carried separately for snapping.
    pub points: Vec<Vector3>,
    /// Full point + face-connectivity view of the same mesh, in OpenFOAM face
    /// order (internal faces first, then boundary faces by patch). This is the
    /// moving-mesh substrate the snapping and layer phases mutate and rebuild
    /// via [`PolyPatchMesh::build_fvmesh`]. It shares [`points`](Self::points)'
    /// indexing.
    pub topology: PolyPatchMesh,
    /// Boundary faces on the carved surface, with their corner points.
    pub surface_faces: Vec<SurfaceFace>,
    /// Number of kept cells at each refinement level (`cells_by_level[l]`).
    pub cells_by_level: Vec<usize>,
    /// The finest refinement level reached.
    pub max_level: usize,
}

impl CastellatedMesh {
    /// Total kept cell count (same as `fv_mesh.n_cells`).
    pub fn n_cells(&self) -> usize {
        self.fv_mesh.n_cells
    }
}

/// Run the castellation phase, producing a refined, region-removed [`FvMesh`].
///
/// # Errors
/// - [`MeshError::Construction`] if the surface is empty, or if the assembled
///   mesh fails [`FvMesh::validate`] (a bug guard — should not happen).
pub fn castellate(
    surface: &TriangleSoup,
    controls: &CastellationControls,
) -> Result<CastellatedMesh, MeshError> {
    if surface.is_empty() {
        return Err(MeshError::Construction(
            "castellation surface has no triangles".to_string(),
        ));
    }

    let bg = &controls.background;
    let max_level = controls.surface_level;

    // Finest-grid dimensions (level-max voxels along each axis).
    let scale = 1usize << max_level;
    let gdim = [bg.nx * scale, bg.ny * scale, bg.nz * scale];
    // Finest voxel size per axis [m].
    let span = bg.bounds.span();
    let h = [
        span.x / gdim[0] as f64,
        span.y / gdim[1] as f64,
        span.z / gdim[2] as f64,
    ];
    let origin = bg.bounds.min;

    // ── Step 1: octree refinement ───────────────────────────────────────────
    // Seed with all level-0 cells, then split near-surface cells level by level.
    let mut leaves: Vec<Leaf> = Vec::with_capacity(bg.n_cells());
    for i in 0..bg.nx {
        for j in 0..bg.ny {
            for k in 0..bg.nz {
                leaves.push(Leaf {
                    level: 0,
                    i,
                    j,
                    k,
                });
            }
        }
    }

    for _ in 0..max_level {
        let mut next: Vec<Leaf> = Vec::with_capacity(leaves.len());
        for leaf in &leaves {
            if leaf.level < max_level && should_refine(leaf, origin, h, max_level, surface, controls)
            {
                // Split into 8 children at level+1.
                let (l, i, j, k) = (leaf.level + 1, leaf.i * 2, leaf.j * 2, leaf.k * 2);
                for di in 0..2 {
                    for dj in 0..2 {
                        for dk in 0..2 {
                            next.push(Leaf {
                                level: l,
                                i: i + di,
                                j: j + dj,
                                k: k + dk,
                            });
                        }
                    }
                }
            } else {
                next.push(*leaf);
            }
        }
        leaves = next;
    }

    // ── Voxel ownership grid (all leaves, kept or not) ────────────────────────
    let nvox = gdim[0] * gdim[1] * gdim[2];
    let idx = |g: [usize; 3]| -> usize { (g[0] * gdim[1] + g[1]) * gdim[2] + g[2] };
    let mut voxel_leaf = vec![usize::MAX; nvox];
    for (lid, leaf) in leaves.iter().enumerate() {
        let s = 1usize << (max_level - leaf.level);
        let lo = [leaf.i * s, leaf.j * s, leaf.k * s];
        for gx in lo[0]..lo[0] + s {
            for gy in lo[1]..lo[1] + s {
                for gz in lo[2]..lo[2] + s {
                    voxel_leaf[idx([gx, gy, gz])] = lid;
                }
            }
        }
    }

    // ── Step 2: region removal (keep the `keep_point` side) ───────────────────
    let keep_inside = surface.contains_point(controls.keep_point);
    let mut kept = vec![false; leaves.len()];
    let mut cell_id = vec![usize::MAX; leaves.len()];
    let mut n_kept = 0usize;
    let mut cells_by_level = vec![0usize; max_level + 1];
    for (lid, leaf) in leaves.iter().enumerate() {
        let centre = leaf_centre(leaf, origin, h, max_level);
        if surface.contains_point(centre) == keep_inside {
            kept[lid] = true;
            cell_id[lid] = n_kept;
            cells_by_level[leaf.level] += 1;
            n_kept += 1;
        }
    }
    if n_kept == 0 {
        return Err(MeshError::Construction(
            "region removal discarded every cell — check keep_point".to_string(),
        ));
    }

    // ── Assemble the FvMesh ───────────────────────────────────────────────────
    let mut builder = MeshAssembler::new(gdim, h, origin, controls.surface_patch_name.clone());

    for (lid, leaf) in leaves.iter().enumerate() {
        if !kept[lid] {
            continue;
        }
        let owner = cell_id[lid];
        let s = 1usize << (max_level - leaf.level);
        let lo = [leaf.i * s, leaf.j * s, leaf.k * s];
        let hi = [lo[0] + s, lo[1] + s, lo[2] + s];

        for axis in 0..3 {
            // Positive side: emit internal faces (kept neighbours) + boundary
            // faces (removed neighbours / domain edge).
            builder.emit_side(
                axis, true, lo, hi, owner, &voxel_leaf, &idx, &kept, &cell_id,
            );
            // Negative side: emit boundary faces only; internal faces there are
            // owned by the neighbour's positive-side pass.
            builder.emit_side(
                axis, false, lo, hi, owner, &voxel_leaf, &idx, &kept, &cell_id,
            );
        }

        // Cell geometry (axis-aligned hex).
        let cmin = pos_of(origin, h, [lo[0] as f64, lo[1] as f64, lo[2] as f64]);
        let cmax = pos_of(origin, h, [hi[0] as f64, hi[1] as f64, hi[2] as f64]);
        let cell_box = Bounds::new(cmin, cmax);
        builder.push_cell(cell_box.centre(), cell_box.volume());
    }

    let (fv_mesh, points, topology, surface_faces) = builder.finish(n_kept)?;

    Ok(CastellatedMesh {
        fv_mesh,
        points,
        topology,
        surface_faces,
        cells_by_level,
        max_level,
    })
}

/// Refinement predicate: refine `leaf` if its centre lies within the refinement
/// band of the surface.
fn should_refine(
    leaf: &Leaf,
    origin: Vector3,
    h: [f64; 3],
    max_level: usize,
    surface: &TriangleSoup,
    controls: &CastellationControls,
) -> bool {
    let centre = leaf_centre(leaf, origin, h, max_level);
    let s = (1usize << (max_level - leaf.level)) as f64;
    // Half the cell's space diagonal at this level.
    let half_diag = 0.5 * Vector3::new(h[0] * s, h[1] * s, h[2] * s).mag();
    let band = controls.refinement_distance.unwrap_or(half_diag);
    surface.distance_to(centre) <= band
}

/// Physical centre [m] of a leaf cell.
fn leaf_centre(leaf: &Leaf, origin: Vector3, h: [f64; 3], max_level: usize) -> Vector3 {
    let s = (1usize << (max_level - leaf.level)) as f64;
    let g = [
        (leaf.i as f64 + 0.5) * s,
        (leaf.j as f64 + 0.5) * s,
        (leaf.k as f64 + 0.5) * s,
    ];
    pos_of(origin, h, g)
}

/// Map finest-grid (fractional) voxel coordinates to a physical point [m].
fn pos_of(origin: Vector3, h: [f64; 3], g: [f64; 3]) -> Vector3 {
    Vector3::new(
        origin.x + g[0] * h[0],
        origin.y + g[1] * h[1],
        origin.z + g[2] * h[2],
    )
}

/// Incrementally assembles the output mesh's faces, patches, points, and cells.
struct MeshAssembler {
    gdim: [usize; 3],
    h: [f64; 3],
    origin: Vector3,
    surface_patch_name: String,

    // Internal faces.
    int_owner: Vec<usize>,
    int_neighbour: Vec<usize>,
    int_sf: Vec<Vector3>,
    int_cf: Vec<Vector3>,
    /// Internal-face corner point ids, wound owner→neighbour (normal +axis).
    int_pts: Vec<[usize; 4]>,

    // Boundary faces bucketed per patch (7 buckets: 6 outer + surface).
    bnd_owner: [Vec<usize>; 7],
    bnd_sf: [Vec<Vector3>; 7],
    bnd_cf: [Vec<Vector3>; 7],
    /// Boundary-face corner point ids per bucket, wound outward.
    bnd_pts: [Vec<[usize; 4]>; 7],

    // Cell geometry.
    cell_centres: Vec<Vector3>,
    cell_volumes: Vec<f64>,

    // Points (deduplicated by finest-grid voxel coordinate).
    point_ids: HashMap<[usize; 3], usize>,
    points: Vec<Vector3>,
    surface_faces: Vec<SurfaceFace>,
}

/// Outer-patch bucket indices; bucket 6 is the carved surface.
const PATCH_NAMES: [&str; 6] = ["xMin", "xMax", "yMin", "yMax", "zMin", "zMax"];
const SURFACE_BUCKET: usize = 6;

impl MeshAssembler {
    fn new(gdim: [usize; 3], h: [f64; 3], origin: Vector3, surface_patch_name: String) -> Self {
        Self {
            gdim,
            h,
            origin,
            surface_patch_name,
            int_owner: Vec::new(),
            int_neighbour: Vec::new(),
            int_sf: Vec::new(),
            int_cf: Vec::new(),
            int_pts: Vec::new(),
            bnd_owner: Default::default(),
            bnd_sf: Default::default(),
            bnd_cf: Default::default(),
            bnd_pts: Default::default(),
            cell_centres: Vec::new(),
            cell_volumes: Vec::new(),
            point_ids: HashMap::new(),
            points: Vec::new(),
            surface_faces: Vec::new(),
        }
    }

    fn push_cell(&mut self, centre: Vector3, volume: f64) {
        self.cell_centres.push(centre);
        self.cell_volumes.push(volume);
    }

    /// Get-or-create a deduplicated point id for a finest-grid corner.
    fn point_id(&mut self, g: [usize; 3]) -> usize {
        if let Some(&id) = self.point_ids.get(&g) {
            return id;
        }
        let id = self.points.len();
        self.points
            .push(pos_of(self.origin, self.h, [g[0] as f64, g[1] as f64, g[2] as f64]));
        self.point_ids.insert(g, id);
        id
    }

    /// Emit all faces on one side (`axis`, `positive`) of a kept cell.
    #[allow(clippy::too_many_arguments)]
    fn emit_side<F>(
        &mut self,
        axis: usize,
        positive: bool,
        lo: [usize; 3],
        hi: [usize; 3],
        owner: usize,
        voxel_leaf: &[usize],
        idx: &F,
        kept: &[bool],
        cell_id: &[usize],
    ) where
        F: Fn([usize; 3]) -> usize,
    {
        let (t0, t1) = transverse(axis);
        // Plane voxel-coordinate along `axis`, and the neighbour voxel layer's
        // coordinate along `axis`.
        let plane_g = if positive { hi[axis] } else { lo[axis] };
        let outside_domain = if positive {
            hi[axis] >= self.gdim[axis]
        } else {
            lo[axis] == 0
        };

        if outside_domain {
            // Whole side lies on the domain boundary → one outer-patch face.
            let bucket = 2 * axis + if positive { 1 } else { 0 };
            self.push_boundary(
                bucket, axis, positive, plane_g, lo[t0], hi[t0], lo[t1], hi[t1], owner,
            );
            return;
        }

        // Neighbour voxel layer coordinate along the axis.
        let nb_layer = if positive { hi[axis] } else { lo[axis] - 1 };

        // Group the face's voxels by neighbour leaf, accumulating the covered
        // transverse rectangle for each.
        let mut groups: HashMap<usize, [usize; 4]> = HashMap::new();
        for a in lo[t0]..hi[t0] {
            for b in lo[t1]..hi[t1] {
                let mut g = [0usize; 3];
                g[axis] = nb_layer;
                g[t0] = a;
                g[t1] = b;
                let nb = voxel_leaf[idx(g)];
                let e = groups.entry(nb).or_insert([a, a + 1, b, b + 1]);
                e[0] = e[0].min(a);
                e[1] = e[1].max(a + 1);
                e[2] = e[2].min(b);
                e[3] = e[3].max(b + 1);
            }
        }

        for (nb_leaf, rect) in groups {
            let (a0, a1, b0, b1) = (rect[0], rect[1], rect[2], rect[3]);
            if nb_leaf != usize::MAX && kept[nb_leaf] {
                if positive {
                    // Internal face, owner → neighbour along +axis.
                    self.push_internal(
                        axis,
                        true,
                        plane_g,
                        a0,
                        a1,
                        b0,
                        b1,
                        owner,
                        cell_id[nb_leaf],
                    );
                }
                // Negative side internal faces are emitted by the neighbour.
            } else {
                // Removed neighbour (or void) → carved surface boundary face.
                self.push_boundary(
                    SURFACE_BUCKET,
                    axis,
                    positive,
                    plane_g,
                    a0,
                    a1,
                    b0,
                    b1,
                    owner,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn push_internal(
        &mut self,
        axis: usize,
        positive: bool,
        plane_g: usize,
        a0: usize,
        a1: usize,
        b0: usize,
        b1: usize,
        owner: usize,
        neighbour: usize,
    ) {
        let (sf, cf) = self.face_geometry(axis, positive, plane_g, a0, a1, b0, b1);
        // Internal faces are always emitted on the owner's +axis side, so the
        // owner→neighbour normal points +axis; wind the corners to match.
        let pts = self.wound_corners(axis, plane_g, a0, a1, b0, b1, true);
        self.int_owner.push(owner);
        self.int_neighbour.push(neighbour);
        self.int_sf.push(sf);
        self.int_cf.push(cf);
        self.int_pts.push(pts);
    }

    #[allow(clippy::too_many_arguments)]
    fn push_boundary(
        &mut self,
        bucket: usize,
        axis: usize,
        positive: bool,
        plane_g: usize,
        a0: usize,
        a1: usize,
        b0: usize,
        b1: usize,
        owner: usize,
    ) {
        let (sf, cf) = self.face_geometry(axis, positive, plane_g, a0, a1, b0, b1);
        // Boundary face: outward normal is +axis on the positive side, −axis on
        // the negative side; wind the corners so the polygon normal is outward.
        let pts = self.wound_corners(axis, plane_g, a0, a1, b0, b1, positive);
        self.bnd_owner[bucket].push(owner);
        self.bnd_sf[bucket].push(sf);
        self.bnd_cf[bucket].push(cf);
        self.bnd_pts[bucket].push(pts);

        if bucket == SURFACE_BUCKET {
            self.surface_faces.push(SurfaceFace {
                owner_cell: owner,
                corners: pts,
            });
        }
    }

    /// Area vector [m²] (owner→neighbour / outward) and centre [m] of an
    /// axis-aligned rectangular face on the `axis` plane at `plane_g`,
    /// spanning transverse voxel ranges `[a0,a1] × [b0,b1]`.
    #[allow(clippy::too_many_arguments)]
    fn face_geometry(
        &self,
        axis: usize,
        positive: bool,
        plane_g: usize,
        a0: usize,
        a1: usize,
        b0: usize,
        b1: usize,
    ) -> (Vector3, Vector3) {
        let (t0, t1) = transverse(axis);
        let la = self.h[t0] * (a1 - a0) as f64;
        let lb = self.h[t1] * (b1 - b0) as f64;
        let area = la * lb;
        let mut normal = [0.0f64; 3];
        normal[axis] = if positive { area } else { -area };
        let sf = Vector3::new(normal[0], normal[1], normal[2]);

        let mut gc = [0.0f64; 3];
        gc[axis] = plane_g as f64;
        gc[t0] = 0.5 * (a0 + a1) as f64;
        gc[t1] = 0.5 * (b0 + b1) as f64;
        let cf = pos_of(self.origin, self.h, gc);
        (sf, cf)
    }

    /// The four corner point ids (deduplicated) of a rectangular face.
    fn face_corner_points(
        &mut self,
        axis: usize,
        plane_g: usize,
        a0: usize,
        a1: usize,
        b0: usize,
        b1: usize,
    ) -> [usize; 4] {
        let (t0, t1) = transverse(axis);
        let corner = |s: &mut Self, a: usize, b: usize| -> usize {
            let mut g = [0usize; 3];
            g[axis] = plane_g;
            g[t0] = a;
            g[t1] = b;
            s.point_id(g)
        };
        [
            corner(self, a0, b0),
            corner(self, a1, b0),
            corner(self, a1, b1),
            corner(self, a0, b1),
        ]
    }

    /// Corner point ids of a rectangular face, wound so the polygon's
    /// right-hand-rule normal points along `+axis` (when `desired_positive`) or
    /// `−axis` (otherwise).
    ///
    /// The base ordering from [`face_corner_points`](Self::face_corner_points) is
    /// counter-clockwise in the `(t0, t1)` transverse plane; its natural normal
    /// is `t0 × t1`, which equals `+axis` for `axis ∈ {0, 2}` but `−axis` for
    /// `axis == 1` (since `x × z = −y`). The winding is reversed when it does not
    /// match the requested sign, keeping [`PolyPatchMesh`] face normals
    /// consistent with the `FvMesh` area-vector convention.
    #[allow(clippy::too_many_arguments)]
    fn wound_corners(
        &mut self,
        axis: usize,
        plane_g: usize,
        a0: usize,
        a1: usize,
        b0: usize,
        b1: usize,
        desired_positive: bool,
    ) -> [usize; 4] {
        let c = self.face_corner_points(axis, plane_g, a0, a1, b0, b1);
        let natural_positive = axis != 1;
        if desired_positive == natural_positive {
            c
        } else {
            // Reverse orientation: [a, b, c, d] → [a, d, c, b].
            [c[0], c[3], c[2], c[1]]
        }
    }

    /// Concatenate internal + per-patch boundary faces into a validated
    /// [`FvMesh`], returning it with the points and surface-face records.
    fn finish(
        self,
        n_cells: usize,
    ) -> Result<(FvMesh, Vec<Vector3>, PolyPatchMesh, Vec<SurfaceFace>), MeshError> {
        let n_internal = self.int_owner.len();

        let mut owner = self.int_owner.clone();
        let mut sf = self.int_sf.clone();
        let mut cf = self.int_cf.clone();
        let neighbour = self.int_neighbour.clone();

        // Point-connectivity faces in the same order as `owner`/`sf`: internal
        // faces first, then boundary faces bucket by bucket.
        let mut topo_faces: Vec<Vec<usize>> =
            self.int_pts.iter().map(|c| c.to_vec()).collect();

        let mut patches = Vec::new();
        let mut start = n_internal;

        // Outer patches (6), then the carved surface patch.
        #[allow(clippy::needless_range_loop)]
        for bucket in 0..7 {
            let size = self.bnd_owner[bucket].len();
            if size == 0 {
                continue;
            }
            let (name, kind) = if bucket == SURFACE_BUCKET {
                (self.surface_patch_name.clone(), PatchKind::Wall)
            } else {
                (PATCH_NAMES[bucket].to_string(), PatchKind::Patch)
            };
            owner.extend_from_slice(&self.bnd_owner[bucket]);
            sf.extend_from_slice(&self.bnd_sf[bucket]);
            cf.extend_from_slice(&self.bnd_cf[bucket]);
            topo_faces.extend(self.bnd_pts[bucket].iter().map(|c| c.to_vec()));
            patches.push(BoundaryPatch::new(name, start, size, kind));
            start += size;
        }

        let fv_mesh = FvMeshBuilder::new()
            .n_cells(n_cells)
            .n_internal_faces(n_internal)
            .owner(owner.clone())
            .neighbour(neighbour.clone())
            .patches(patches.clone())
            .cell_volumes(self.cell_volumes.clone())
            .cell_centres(self.cell_centres.clone())
            .face_area_vectors(sf)
            .face_centres(cf)
            .build()
            .map_err(|e| MeshError::Construction(format!("assembled mesh invalid: {e}")))?;

        let topology = PolyPatchMesh {
            points: self.points.clone(),
            faces: topo_faces,
            owner,
            neighbour,
            n_internal_faces: n_internal,
            n_cells,
            patches,
        };

        Ok((fv_mesh, self.points, topology, self.surface_faces))
    }
}

/// The two transverse axis indices for a given face axis (0=x → (1,2), etc.).
fn transverse(axis: usize) -> (usize, usize) {
    match axis {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    }
}
