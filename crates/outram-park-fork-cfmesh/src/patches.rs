// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Algorithm reference (re-implemented in Rust, not transcribed):
//   OpenFOAM snappyHexMesh — surface-region -> boundary-patch assignment
//   (`snappyHexMeshDict` `regions`/`patchInfo`; a generated boundary face takes
//   the patch of the surface region it was cut from).
//   Copyright (C) 2011-2016 OpenFOAM Foundation
//   Copyright (C) 2016-2023 OpenCFD Ltd.
//   Licence: GPL-3.0-only
//   cfMesh does the same via `meshLibrary/utilities/surfaceTools` patch
//   propagation; Copyright (C) 2014-2017 Creative Fields, Ltd., GPL-3.0-only.
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

//! **Named boundary patches** — carry the input surface's region names through
//! meshing so the output `polyMesh` has an `inlet` / `outlet` / `walls` split a
//! solver case can actually be set up against.
//!
//! # Why this module exists
//!
//! Every mesher in this crate emits its boundary into a *single* patch called
//! `"walls"`: [`crate::carve::carve_box`] hard-codes it, and
//! [`crate::volume_mesh::from_cell_faces`] — which every stage after the carve
//! (tetrahedralize, dual, layers) rebuilds through — has no patch information to
//! work from at all, because it recovers connectivity by matching face vertex
//! *sets*. A mesh with one patch is unusable for CFD: you cannot write a `0/`
//! boundary-condition directory that says "fixed velocity here, zero gradient
//! there" when there is only one "there".
//!
//! # How the names survive
//!
//! Rather than thread a patch tag through five stages that each rebuild the mesh
//! from scratch (and through the boundary-layer step, which *creates* boundary
//! faces that never existed in the input), this module recovers the assignment
//! **geometrically, once, at the end**: every boundary face of the finished mesh
//! is given the region of the input-surface triangle **closest to its centroid**.
//!
//! That is exactly how snappyHexMesh assigns a cut face to a surface region, and
//! it is well-posed here because every boundary face of a finished mesh lies on
//! (post-snap) or within roughly one cell of the input surface. It also handles
//! the faces the layer stage invents, which no tag-threading scheme could.
//!
//! # What it does not do
//!
//! - **Region resolution is limited by the mesh.** Two regions closer together
//!   than a cell can be mixed up on the cells that straddle them; features are
//!   resolved by refining, not by this classifier.
//! - **Prism layers are still grown over the whole boundary**, because the
//!   classification runs after the layer stage. Selecting which patches get
//!   layers (snappyHexMesh's per-patch `nSurfaceLayers`) is future work — see
//!   [`crate::pipeline::surface_to_tet_dual_mesh_multipatch`].
//! - **No feature-edge snapping**, so a patch boundary follows the mesh's face
//!   edges, not the surface's feature edge.
//!
//! Pure Rust, no dependencies, Android-safe.

use crate::math::Vec3;
use crate::snap::closest_point_on_surface;
use crate::volume_mesh::{BoundaryPatch, VolumeMesh};

/// A labelling of an input surface's triangles into **named regions**, one
/// region per intended boundary patch (`inlet`, `outlet`, `walls`, ...).
///
/// This is the mesher's input side of the patch story: the caller says which
/// triangle belongs to which named region, and
/// [`assign_patches_by_region`] turns that into the output mesh's
/// [`BoundaryPatch`] list.
///
/// # Invariants
///
/// - `region_of_tri.len()` must equal the triangle count of the surface it
///   labels; `region_of_tri[i]` is an index into [`Self::names`].
/// - `names` must be non-empty and should be unique; patch order in the output
///   mesh follows `names` order.
///
/// Both are checked by [`Self::validate`], which the pipeline calls before
/// meshing so a mislabelled surface is a clear error, not a silent wrong mesh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceRegions {
    /// Patch names, in the order the output mesh's patches will appear.
    pub names: Vec<String>,
    /// `region_of_tri[i]` = index into [`Self::names`] for input triangle `i`.
    pub region_of_tri: Vec<usize>,
}

impl SurfaceRegions {
    /// Every triangle in one region — the single-patch behaviour the crate had
    /// before this module existed. `n_tris` is the surface's triangle count.
    ///
    /// # Examples
    ///
    /// ```
    /// use outram_park_fork_cfmesh::patches::SurfaceRegions;
    ///
    /// let r = SurfaceRegions::single("walls", 12);
    /// assert_eq!(r.names, vec!["walls".to_string()]);
    /// assert_eq!(r.region_of_tri.len(), 12);
    /// ```
    pub fn single(name: &str, n_tris: usize) -> Self {
        SurfaceRegions { names: vec![name.to_string()], region_of_tri: vec![0; n_tris] }
    }

    /// Build from a **per-triangle name**: `labels[i]` is the patch name for
    /// triangle `i`. Distinct names become distinct regions, numbered in order
    /// of first appearance — so patch order in the output mesh follows the order
    /// the names first occur in `labels`.
    ///
    /// This is the ergonomic constructor: author a surface, then say what each
    /// triangle is, without hand-managing region indices.
    ///
    /// # Examples
    ///
    /// ```
    /// use outram_park_fork_cfmesh::patches::SurfaceRegions;
    ///
    /// let r = SurfaceRegions::from_labels(&["inlet", "walls", "walls", "outlet"]);
    /// assert_eq!(r.names, vec!["inlet".to_string(), "walls".to_string(), "outlet".to_string()]);
    /// assert_eq!(r.region_of_tri, vec![0, 1, 1, 2]);
    /// ```
    pub fn from_labels(labels: &[&str]) -> Self {
        let mut names: Vec<String> = Vec::new();
        let mut region_of_tri = Vec::with_capacity(labels.len());
        for &l in labels {
            let idx = match names.iter().position(|n| n == l) {
                Some(i) => i,
                None => {
                    names.push(l.to_string());
                    names.len() - 1
                }
            };
            region_of_tri.push(idx);
        }
        SurfaceRegions { names, region_of_tri }
    }

    /// Number of named regions.
    pub fn region_count(&self) -> usize {
        self.names.len()
    }

    /// Check the invariants against a surface of `n_tris` triangles: names
    /// non-empty and unique, one region index per triangle, every index in
    /// range. Returns `Ok(())` or a message naming the first problem.
    pub fn validate(&self, n_tris: usize) -> Result<(), String> {
        if self.names.is_empty() {
            return Err("SurfaceRegions has no names — at least one patch name is required".into());
        }
        if self.region_of_tri.len() != n_tris {
            return Err(format!(
                "SurfaceRegions labels {} triangles but the surface has {n_tris}",
                self.region_of_tri.len()
            ));
        }
        for (i, r) in self.region_of_tri.iter().enumerate() {
            if *r >= self.names.len() {
                return Err(format!("triangle {i} names region {r}, but only {} regions exist", self.names.len()));
            }
        }
        for (i, n) in self.names.iter().enumerate() {
            if self.names[..i].contains(n) {
                return Err(format!("duplicate patch name '{n}'"));
            }
        }
        Ok(())
    }
}

/// Re-bucket `mesh`'s boundary faces into **named patches**, one per region of
/// the input surface, and return the re-ordered mesh.
///
/// Each boundary face is assigned the region of the input triangle **nearest to
/// its centroid** (exact point-to-triangle distance, the same one
/// [`crate::snap::snap_to_surface`] projects with, so a snapped face lands on
/// the region it was snapped onto). Faces are then re-ordered so that:
///
/// - all internal faces come first (the OpenFOAM prefix rule), keeping their
///   relative order and their owner/neighbour pairing;
/// - each non-empty patch is a **contiguous run** of boundary faces, in
///   [`SurfaceRegions::names`] order, with a matching
///   [`BoundaryPatch::start_face`] / [`BoundaryPatch::n_faces`].
///
/// Empty patches are dropped (a region no boundary face landed on produces no
/// patch), so the caller should not assume a 1:1 patch/region correspondence.
///
/// Points, cells and topology are untouched — only the face *ordering* and the
/// patch list change — so the mesh's volume, closure and quality are identical
/// to the input's.
///
/// # Inputs and units
///
/// - `mesh` — a finished [`VolumeMesh`]; must satisfy the usual invariant that
///   `owner` / `neighbour` are per-face and in range.
/// - `points` / `tris` — the **input surface** the mesh was generated from,
///   positions in metres. Must be the same surface, in the same frame.
/// - `regions` — labels for `tris`; see [`SurfaceRegions::validate`].
///
/// # Errors
///
/// `Err` if `regions` does not describe `tris` ([`SurfaceRegions::validate`]),
/// or if the surface has no triangles (nothing to classify against).
///
/// # Cost
///
/// `O(boundary_faces x triangles)` exact distance evaluations, with a
/// bounding-box reject that skips triangles which cannot beat the current best.
/// There is no spatial index yet, so a large surface with a fine mesh is slow;
/// the single-region case is short-circuited by the pipeline and costs nothing.
///
/// # Examples
///
/// ```
/// use outram_park_fork_cfmesh::{math::Vec3, shapes::box_surface, carve::carve_box,
///     patches::{SurfaceRegions, assign_patches_by_region}};
///
/// // box_surface emits its 12 triangles face by face: -Z, +Z, -Y, +Y, +X, -X.
/// let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
/// let regions = SurfaceRegions::from_labels(&[
///     "walls", "walls", "walls", "walls", "walls", "walls",
///     "walls", "walls", "outlet", "outlet", "inlet", "inlet",
/// ]);
///
/// let mesh = carve_box(&p, &t, 0.5);
/// let named = assign_patches_by_region(&mesh, &p, &t, &regions).unwrap();
///
/// // Three patches, and they tile the boundary contiguously.
/// assert_eq!(named.patches.len(), 3);
/// assert!((named.total_volume() - 1.0).abs() < 1e-9); // geometry untouched
/// ```
pub fn assign_patches_by_region(
    mesh: &VolumeMesh,
    points: &[Vec3],
    tris: &[[usize; 3]],
    regions: &SurfaceRegions,
) -> Result<VolumeMesh, String> {
    regions.validate(tris.len())?;
    if tris.is_empty() {
        return Err("cannot assign patches against a surface with no triangles".into());
    }

    // Internal faces keep their order and come first.
    let mut order: Vec<usize> = (0..mesh.face_count()).filter(|&f| mesh.neighbour[f].is_some()).collect();
    let n_internal = order.len();

    // Bucket every boundary face by the region of its nearest input triangle.
    let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); regions.region_count()];
    for f in 0..mesh.face_count() {
        if mesh.neighbour[f].is_some() {
            continue;
        }
        let c = mesh.face_centroid(f);
        let t = nearest_triangle(c, points, tris);
        buckets[regions.region_of_tri[t]].push(f);
    }

    let mut patches: Vec<BoundaryPatch> = Vec::new();
    for (r, bucket) in buckets.iter().enumerate() {
        if bucket.is_empty() {
            continue;
        }
        patches.push(BoundaryPatch {
            name: regions.names[r].clone(),
            start_face: order.len(),
            n_faces: bucket.len(),
        });
        order.extend(bucket.iter().copied());
    }
    debug_assert_eq!(order.len(), mesh.face_count());
    debug_assert_eq!(n_internal, mesh.n_internal_faces());

    Ok(VolumeMesh {
        points: mesh.points.clone(),
        faces: order.iter().map(|&f| mesh.faces[f].clone()).collect(),
        owner: order.iter().map(|&f| mesh.owner[f]).collect(),
        neighbour: order.iter().map(|&f| mesh.neighbour[f]).collect(),
        n_cells: mesh.n_cells,
        patches,
    })
}

/// Index of the surface triangle closest to `p`, by exact point-to-triangle
/// distance. Triangles whose bounding box is already further than the current
/// best are rejected without the full distance evaluation.
fn nearest_triangle(p: Vec3, points: &[Vec3], tris: &[[usize; 3]]) -> usize {
    let mut best = 0usize;
    let mut best_d2 = f64::MAX;
    for (i, t) in tris.iter().enumerate() {
        let (a, b, c) = (points[t[0]], points[t[1]], points[t[2]]);
        // Cheap lower bound: squared distance to the triangle's bounding box.
        let lo = Vec3::new(a.x.min(b.x).min(c.x), a.y.min(b.y).min(c.y), a.z.min(b.z).min(c.z));
        let hi = Vec3::new(a.x.max(b.x).max(c.x), a.y.max(b.y).max(c.y), a.z.max(b.z).max(c.z));
        let dx = (lo.x - p.x).max(0.0).max(p.x - hi.x);
        let dy = (lo.y - p.y).max(0.0).max(p.y - hi.y);
        let dz = (lo.z - p.z).max(0.0).max(p.z - hi.z);
        if dx * dx + dy * dy + dz * dz >= best_d2 {
            continue;
        }
        let q = closest_point_on_surface(p, points, std::slice::from_ref(t));
        let d2 = q.sub(p).dot(q.sub(p));
        if d2 < best_d2 {
            best_d2 = d2;
            best = i;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carve::carve_box;
    use crate::shapes::box_surface;

    /// The 12 triangles [`box_surface`] emits, in order, are two per face in the
    /// order `-Z, +Z, -Y, +Y, +X, -X`. Label `-X` as `inlet`, `+X` as `outlet`
    /// and the rest `walls` — the canonical duct set-up.
    fn duct_regions() -> SurfaceRegions {
        SurfaceRegions::from_labels(&[
            "walls", "walls", // -Z
            "walls", "walls", // +Z
            "walls", "walls", // -Y
            "walls", "walls", // +Y
            "outlet", "outlet", // +X
            "inlet", "inlet", // -X
        ])
    }

    /// V&V — `SurfaceRegions` constructors and validation. Methodology: build
    /// via [`SurfaceRegions::single`] and [`SurfaceRegions::from_labels`] and
    /// check the invariants the pipeline relies on, plus each rejection path.
    /// Measured 2026-08-07: `single` gives 1 region over all triangles;
    /// `from_labels` numbers regions in first-appearance order; a wrong triangle
    /// count, an out-of-range index and a duplicate name each return `Err`.
    #[test]
    fn surface_regions_construct_and_validate() {
        let s = SurfaceRegions::single("walls", 12);
        assert_eq!(s.region_count(), 1);
        s.validate(12).expect("single is valid");
        assert!(s.validate(11).is_err(), "wrong triangle count rejected");

        let d = duct_regions();
        assert_eq!(d.names, vec!["walls".to_string(), "outlet".to_string(), "inlet".to_string()]);
        d.validate(12).expect("duct labels are valid");

        let bad = SurfaceRegions { names: vec!["a".into()], region_of_tri: vec![0, 3] };
        assert!(bad.validate(2).is_err(), "out-of-range region index rejected");
        let dup = SurfaceRegions { names: vec!["a".into(), "a".into()], region_of_tri: vec![0, 1] };
        assert!(dup.validate(2).is_err(), "duplicate patch name rejected");
    }

    /// V&V — headline for the classifier on a mesh whose answer is exactly
    /// known. Methodology: carve the unit box `[0,1]^3` at `cell_size = 0.5 m`
    /// (a grid-aligned carve: 8 cells, 24 boundary faces, 4 per box side), label
    /// the surface `inlet` (-X) / `outlet` (+X) / `walls` (the other four
    /// sides), and classify. Pass criteria: exactly 3 patches; `inlet` and
    /// `outlet` hold 4 faces each and `walls` 16; the patches are **contiguous**
    /// and start immediately after the internal faces; every `inlet` face really
    /// lies in the plane `x = 0` and every `outlet` face in `x = 1`; geometry
    /// (volume, closure) unchanged. Measured 2026-08-07: 24 boundary faces split
    /// 16 / 4 / 4 as predicted, contiguous from face 12 (12 internal faces),
    /// volume 1.0 m^3, `validate()` Ok.
    #[test]
    fn duct_box_carve_splits_into_inlet_outlet_walls() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let mesh = carve_box(&p, &t, 0.5);
        let named = assign_patches_by_region(&mesh, &p, &t, &duct_regions()).expect("classifies");

        named.validate().expect("re-ordering keeps every cell closed");
        assert!((named.total_volume() - 1.0).abs() < 1e-9, "volume {}", named.total_volume());
        assert_eq!(named.cell_count(), mesh.cell_count());
        assert_eq!(named.face_count(), mesh.face_count());

        assert_eq!(named.patches.len(), 3, "walls + outlet + inlet");
        let get = |n: &str| named.patches.iter().find(|q| q.name == n).unwrap_or_else(|| panic!("patch {n}"));
        assert_eq!(get("inlet").n_faces, 4, "2x2 faces on the -X side");
        assert_eq!(get("outlet").n_faces, 4, "2x2 faces on the +X side");
        assert_eq!(get("walls").n_faces, 16, "4 remaining sides x 4 faces");

        // Contiguous, internal-first, covering every boundary face exactly once.
        let mut sorted = named.patches.clone();
        sorted.sort_by_key(|q| q.start_face);
        let mut expect = named.n_internal_faces();
        for q in &sorted {
            assert_eq!(q.start_face, expect, "patch '{}' starts contiguously", q.name);
            expect += q.n_faces;
        }
        assert_eq!(expect, named.face_count(), "patches cover every boundary face");

        // The classification is geometrically right, not merely well-counted.
        for q in &named.patches {
            for f in q.start_face..q.start_face + q.n_faces {
                assert!(named.neighbour[f].is_none(), "patch face {f} is a boundary face");
                let c = named.face_centroid(f);
                match q.name.as_str() {
                    "inlet" => assert!(c.x.abs() < 1e-12, "inlet face at x=0, got {}", c.x),
                    "outlet" => assert!((c.x - 1.0).abs() < 1e-12, "outlet face at x=1, got {}", c.x),
                    _ => assert!(c.x > 1e-12 && c.x < 1.0 - 1e-12, "walls face off the X caps, got {}", c.x),
                }
            }
        }
    }

    /// V&V — a single region reproduces the crate's historical single-`walls`
    /// boundary exactly: one patch, all boundary faces, unchanged geometry.
    /// Measured 2026-08-07: 1 patch of 24 faces starting at face 12, volume 1.0.
    #[test]
    fn single_region_reproduces_the_walls_patch() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let mesh = carve_box(&p, &t, 0.5);
        let regions = SurfaceRegions::single("walls", t.len());
        let named = assign_patches_by_region(&mesh, &p, &t, &regions).expect("classifies");
        assert_eq!(named.patches.len(), 1);
        assert_eq!(named.patches[0].name, "walls");
        assert_eq!(named.patches[0].n_faces, mesh.n_boundary_faces());
        assert_eq!(named.patches[0].start_face, named.n_internal_faces());
        assert!((named.total_volume() - 1.0).abs() < 1e-9);
    }
}
