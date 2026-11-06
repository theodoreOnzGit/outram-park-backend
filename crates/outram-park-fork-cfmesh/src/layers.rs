// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Algorithm references (re-implemented in Rust, not transcribed):
//
//   [1] OpenFOAM snappyHexMesh `addLayers` — shrink the wall points inward by
//       the total layer thickness and fill the vacated shell with a geometric
//       stack of prisms.
//       src/mesh/snappyHexMesh/snappyHexMeshDriver/snappyLayerDriver.C
//       Copyright (C) 2011-2016 OpenFOAM Foundation
//       Copyright (C) 2016-2023 OpenCFD Ltd. Licence: GPL-3.0-only.
//
//   [2] cfMesh boundary-layer generation — meshLibrary/utilities/boundaryLayers
//       (`boundaryLayers`, `refineBoundaryLayers`).
//       Copyright (C) 2014-2017 Creative Fields, Ltd. Licence: GPL-3.0-only.
//
//   [3] The smoothed-normal / thickness-limited advancing-layer variant follows
//       the published advancing-layer idea (D. J. Mavriplis; the VMTK and
//       Netgen implementations), from the literature, not from their code.
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

//! Prism **boundary layers** — insert graded near-wall inflation layers at a
//! wall patch.
//!
//! This is snappyHexMesh's *addLayers* / cfMesh's boundary-layer step. Given a
//! wall patch, it moves the interior mesh's wall points **inward** by the total
//! layer thickness and fills the vacated shell with `n_layers` stacked prism
//! cells per wall face, with a geometric thickness progression
//! (`first_thickness`, then × `expansion` each layer). The result resolves the
//! wall-normal gradients a CFD/TH solver needs (low y⁺).
//!
//! # How it stays valid
//!
//! Only the wall points move (they belong to the boundary), so the wall-adjacent
//! cells simply *shrink* — no interior topology changes and no hanging nodes.
//! The prism stack of each wall face is 1:1 with that face; adjacent stacks
//! share their side faces automatically (matched by [`from_cell_faces`]). The
//! operation is a **repartition** of the same region, so it preserves the mesh
//! volume ~~exactly~~. **CORRECTED 2026-11-05 (gh:#1270):** exactly when every
//! patch a layer ends on is planar. A border point slides along the other
//! patch's averaged normal plane; where that patch is not planar (the
//! default dual's pipe ends spread 0.47 mm along the axis at h 5 mm), the
//! end surface moves and the volume changes: −4.7e-7 relative on the
//! Foam Studio pipe, one layer (cfMesh's route: +2.5e-7, all of it in the
//! end patches; `boundary_layers::tests::diagnose_cfmesh_layer_on_the_foam_studio_pipe`).
//!
//! # v1 scope
//!
//! A single wall patch, moved along the averaged per-point inward normal (so a
//! sharp convex corner shrinks along its diagonal — adequate for modest total
//! thickness). ~~Multi-patch selection and corner-feature handling are future
//! work.~~ **CORRECTED 2026-11-04 (gh:#1202):** patch selection is ported:
//! [`add_boundary_layers_adaptive_on_patches`] grows layers on named patches
//! only and ends them on the other patches, with cfMesh's border rules (a
//! point between a layer patch and one other patch slides in that patch's
//! plane; between two others, along their shared edge). On a mesh whose
//! whole boundary is one patch, as [`add_boundary_layers`] and
//! [`add_boundary_layers_adaptive`] are usually called, nothing changes.
//! The total thickness must be smaller than the wall cells or they would
//! invert. Pure Rust, Android-safe.
//!
//! Deterministic since 2026-11-04: point and cell numbering no longer follow a
//! randomly seeded hash set, so the same input gives the same mesh in every
//! run (the reproducibility defect class of the HTR-10 neighbour grid).

pub mod boundary_layers;
pub mod refine_boundary_layers;
pub mod extrude_layer;
pub mod triangulate_non_planar_base_faces;

use crate::checks::check_quality;
use crate::math::Vec3;
use crate::volume_mesh::{cells_faces, from_cell_faces, from_cell_faces_wound, VolumeMesh};
use std::collections::{BTreeMap, BTreeSet};

/// Insert `n_layers` graded prism boundary layers at the patch named
/// `patch_name`, returning the new mesh.
///
/// `first_thickness` is the first (wall-nearest) layer thickness; each
/// subsequent layer is `expansion` times thicker. If the patch is missing or
/// `n_layers == 0`, `mesh` is returned unchanged (rebuilt).
///
/// # Examples
///
/// ```
/// use outram_park_fork_cfmesh::{math::Vec3, shapes::box_surface, carve::carve_box, layers::add_boundary_layers};
///
/// // Carve a box (single `walls` patch), then add 3 prism layers; the volume
/// // is preserved (a repartition) and the mesh stays closed.
/// let (p, t) = box_surface(Vec3::ZERO, Vec3::new(3.0, 3.0, 3.0));
/// let m = carve_box(&p, &t, 1.0);
/// let l = add_boundary_layers(&m, "walls", 3, 0.05, 1.3);
/// assert!((l.total_volume() - m.total_volume()).abs() < 1e-9);
/// assert!(l.validate().is_ok());
/// ```
pub fn add_boundary_layers(
    mesh: &VolumeMesh,
    patch_name: &str,
    n_layers: usize,
    first_thickness: f64,
    expansion: f64,
) -> VolumeMesh {
    // Which faces are the wall patch?
    let patch = mesh.patches.iter().find(|p| p.name == patch_name);
    let Some(patch) = patch else {
        return from_cell_faces(mesh.points.clone(), &cells_faces(mesh));
    };
    if n_layers == 0 || first_thickness <= 0.0 {
        return from_cell_faces(mesh.points.clone(), &cells_faces(mesh));
    }
    let wall_faces: Vec<usize> = (patch.start_face..patch.start_face + patch.n_faces).collect();

    // Wall points and their inward normal (average of adjacent wall faces'
    // inward normals = −outward area vector).
    let mut wall_pts: BTreeSet<usize> = BTreeSet::new();
    let mut inward: BTreeMap<usize, Vec3> = BTreeMap::new();
    for &f in &wall_faces {
        let out = mesh.face_area_vector(f).normalize();
        for &p in &mesh.faces[f] {
            wall_pts.insert(p);
            let e = inward.entry(p).or_insert(Vec3::ZERO);
            *e = e.sub(out); // accumulate inward direction
        }
    }
    for v in inward.values_mut() {
        *v = v.normalize();
    }

    // Cumulative layer offsets d[0..=n]; d[0] = 0 (wall), d[n] = total thickness.
    let mut d = vec![0.0f64; n_layers + 1];
    let mut t = first_thickness;
    for m in 1..=n_layers {
        d[m] = d[m - 1] + t;
        t *= expansion;
    }

    // New point set: non-wall points kept; each wall point becomes a column of
    // n+1 points from the wall (d=0) inward to the shrunk position (d=total).
    let mut new_points: Vec<Vec3> = Vec::new();
    let mut new_of = vec![usize::MAX; mesh.points.len()];
    for (op, &pt) in mesh.points.iter().enumerate() {
        if !wall_pts.contains(&op) {
            new_of[op] = new_points.len();
            new_points.push(pt);
        }
    }
    let mut column: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for &wp in &wall_pts {
        let x = mesh.points[wp];
        let nrm = inward[&wp];
        let mut col = Vec::with_capacity(n_layers + 1);
        for &dm in &d {
            col.push(new_points.len());
            new_points.push(x.add(nrm.scale(dm)));
        }
        column.insert(wp, col);
    }
    // A point's index in the interior mesh: non-wall -> kept; wall -> shrunk end.
    let remap = |op: usize| -> usize {
        if wall_pts.contains(&op) {
            column[&op][n_layers]
        } else {
            new_of[op]
        }
    };

    // Cells: the (shrunk) interior cells, then the prism cells.
    let mut cells: Vec<Vec<Vec<usize>>> = Vec::new();
    for cf in cells_faces(mesh) {
        cells.push(
            cf.iter()
                .map(|ring| ring.iter().map(|&p| remap(p)).collect())
                .collect(),
        );
    }
    for &f in &wall_faces {
        prism_stack(&mesh.faces[f], &column, n_layers, &mut cells);
    }

    from_cell_faces_wound(new_points, &cells)
}

/// The `n_layers` prisms of one wall face `w` (wound outward, as a boundary
/// face of a `VolumeMesh` is), each with its faces wound **outward from the
/// prism**: the wall-side face as `w`, the inner face reversed, and each side
/// face `p_i, p_{i+1}, q_{i+1}, q_i` for the wall edge `p → q` (an edge of a
/// ring wound counter-clockwise about its normal has the face on its left, so
/// the side's outward normal is `(q − p) × n`). Shared by both layerers, which
/// assemble with [`from_cell_faces_wound`].
fn prism_stack(w: &[usize], column: &BTreeMap<usize, Vec<usize>>, n_layers: usize, cells: &mut Vec<Vec<Vec<usize>>>) {
    let k = w.len();
    for i in 0..n_layers {
        let mut faces: Vec<Vec<usize>> = Vec::with_capacity(k + 2);
        faces.push(w.iter().map(|&p| column[&p][i]).collect());
        faces.push(w.iter().rev().map(|&p| column[&p][i + 1]).collect());
        for e in 0..k {
            let (p, q) = (w[e], w[(e + 1) % k]);
            faces.push(vec![column[&p][i], column[&p][i + 1], column[&q][i + 1], column[&q][i]]);
        }
        cells.push(faces);
    }
}

/// Insert graded prism boundary layers that **adapt to curved walls**.
///
/// [`add_boundary_layers`] marches every wall point inward by the same fixed
/// total thickness along its averaged normal — exact and ideal on flat (box)
/// walls, but on a **convex curved** wall those inward normals converge and the
/// layers self-intersect, invalidating the mesh. This variant makes curved-wall
/// layers work by:
///
/// 1. **Smoothing the wall-normal field** (a few Laplacian passes over the
///    wall-point adjacency) so neighbouring march directions vary gently — the
///    core idea of advancing-layer meshers (VMTK / Netgen) versus snappyHexMesh's
///    raw averaged normal.
/// 2. **Per-point thickness limiting** to a fraction of the local wall spacing,
///    so tight regions get thinner layers.
/// 3. A **global validity back-off**: it builds the layered mesh and, if it is
///    not closed (self-intersection), scales the thickness down and retries,
///    keeping the thickest layers that still produce a valid mesh.
///
/// So it returns the thickest valid graded prism layers the geometry admits,
/// rather than an invalid mesh (or, on a wall too tight for any layer, the input
/// rebuilt with none). The requested `first_thickness` / `expansion` set the
/// *profile*; the achieved absolute thickness may be smaller after back-off.
///
/// # Limitations
///
/// The back-off is global (uniform scale), so one tight feature thins the whole
/// wall; true per-point marching with local layer termination (snappyHexMesh
/// `addLayers` / AFLR advancing-layer) is future work. Verified valid + solvable
/// on snapped spheres and cylinders. Pure Rust, Android-safe.
///
/// # Examples
///
/// ```
/// use outram_park_fork_cfmesh::{math::Vec3, shapes::sphere_surface, carve::carve_box,
///     snap::snap_to_surface, dual::polyhedral_dual_min_faces, layers::add_boundary_layers_adaptive};
///
/// // A snapped, dualised sphere — a curved polyhedral wall — takes prism layers.
/// let (p, t) = sphere_surface(Vec3::ZERO, 3.0, 20, 40);
/// let poly = polyhedral_dual_min_faces(&snap_to_surface(&carve_box(&p, &t, 0.6), &p, &t));
/// let layered = add_boundary_layers_adaptive(&poly, "walls", 3, 0.04, 1.3);
/// assert!(layered.validate().is_ok());
/// assert!(layered.cell_count() > poly.cell_count()); // prism cells were added
/// ```
pub fn add_boundary_layers_adaptive(
    mesh: &VolumeMesh,
    patch_name: &str,
    n_layers: usize,
    first_thickness: f64,
    expansion: f64,
) -> VolumeMesh {
    add_boundary_layers_adaptive_on_patches(mesh, &[patch_name], n_layers, first_thickness, expansion)
}

/// How a boundary point of the layer patches moves (cfMesh's
/// `boundaryLayers::findPatchVertices` classification).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum LayerVertex {
    /// Only layer-patch faces meet here (cfMesh `PATCHNODE` alone): it moves
    /// along the smoothed inward normal.
    Patch,
    /// On the border with exactly one non-layer patch (cfMesh `EDGENODE`, "on
    /// an edge"): it moves inside that patch's plane, along the layer faces'
    /// normal with its component along the other patch's normal removed. The
    /// direction is held fixed by the smoothing.
    Edge,
    /// On the border with two non-layer patches (cfMesh's "corner"): it moves
    /// along the boundary edge both of them share.
    Corner,
}

/// Insert graded prism boundary layers on the **named patches only**, the
/// patch-selective form of [`add_boundary_layers_adaptive`] (gh:#1202).
///
/// Every boundary face outside `layer_patches` keeps its place: no prism stack
/// is extruded from it, and a layer that runs into it ends on it. This is
/// cfMesh's `boundaryLayers::addLayerForPatch` behaviour (a patch with
/// `nLayers 0` under `patchBoundaryLayers` in `meshDict`), ported from
/// `meshLibrary/utilities/boundaryLayers/boundaryLayersCreateVertices.C`
/// (`findPatchVertices`, `createNewVertex`; cfMesh, Copyright (C) 2014-2017
/// Creative Fields, Ltd., GPL-3.0-only, commit `6cf1d211f9cf`):
///
/// - a layer point that also touches **one** non-layer patch (the rim where a
///   pipe wall meets its inlet) moves along the sum of its layer faces' area
///   vectors with the component along the non-layer faces' unit normal
///   removed, so it **stays in the non-layer patch's plane** and that patch is
///   only re-partitioned, never moved (**CORRECTED 2026-11-05, gh:#1270:**
///   when that patch is planar; on a non-planar one the point leaves its
///   surface, see the module documentation); its thickness is capped at 0.9 × its
///   distance to the edge opposite it in each adjacent non-layer face
///   (upstream's "limit distances");
/// - a point that touches **two** non-layer patches moves along the boundary
///   edge they share, at most half way to the next point;
/// - a point touching **three or more** non-layer patches is refused upstream
///   ("There are more than 3 patches meeting at this vertex!"): here the mesh
///   is returned rebuilt with no layers, which the pipeline reports.
///
/// What is this crate's own and not upstream's: the layer thickness follows
/// the requested profile (`first_thickness`, `expansion`) with the per-point
/// caps and validity back-off of [`add_boundary_layers_adaptive`], where
/// upstream extrudes one layer of half the local edge length and splits it
/// later (`refineBoundaryLayers`); and the interior normals are smoothed, with
/// the border directions held fixed.
///
/// Cell count: a successful call adds exactly `n_layers` prisms per layer-patch
/// face, appended after the existing cells (whose order is kept). The returned
/// mesh has the single `walls` patch of [`from_cell_faces`]; re-assign names
/// afterwards (the pipeline does, with [`crate::patches`]). Deterministic: the
/// same input gives the same mesh, numbering included.
///
/// # Examples
///
/// ```
/// use outram_park_fork_cfmesh::{math::Vec3, shapes::box_surface, carve::carve_box,
///     patches::{SurfaceRegions, assign_patches_by_region},
///     layers::add_boundary_layers_adaptive_on_patches};
///
/// // A unit box with its +X / -X faces as outlet / inlet: layers on the
/// // four other sides only.
/// let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
/// let regions = SurfaceRegions::from_labels(&[
///     "walls", "walls", "walls", "walls", "walls", "walls",
///     "walls", "walls", "outlet", "outlet", "inlet", "inlet",
/// ]);
/// let named = assign_patches_by_region(&carve_box(&p, &t, 0.25), &p, &t, &regions).unwrap();
/// let n_wall = named.patches.iter().find(|q| q.name == "walls").unwrap().n_faces;
/// let layered = add_boundary_layers_adaptive_on_patches(&named, &["walls"], 2, 0.02, 1.2);
/// assert!(layered.validate().is_ok());
/// assert_eq!(layered.cell_count(), named.cell_count() + 2 * n_wall);
/// assert!((layered.total_volume() - 1.0).abs() < 1e-9);
/// ```
pub fn add_boundary_layers_adaptive_on_patches(
    mesh: &VolumeMesh,
    layer_patches: &[&str],
    n_layers: usize,
    first_thickness: f64,
    expansion: f64,
) -> VolumeMesh {
    add_boundary_layers_back_off(mesh, layer_patches, n_layers, first_thickness, expansion).0
}

/// The [`LayerQualityStrategy::BackOffOnly`] layerer with its outcome: the
/// body of [`add_boundary_layers_adaptive_on_patches`] (gh:#1215 split it in
/// two, unchanged, so the other strategies share the setup).
pub(crate) fn add_boundary_layers_back_off(
    mesh: &VolumeMesh,
    layer_patches: &[&str],
    n_layers: usize,
    first_thickness: f64,
    expansion: f64,
) -> (VolumeMesh, LayerOutcome) {
    let rebuild = || from_cell_faces(mesh.points.clone(), &cells_faces(mesh));
    let Some(s) = layer_setup(mesh, layer_patches, n_layers, first_thickness, expansion) else {
        return (rebuild(), LayerOutcome::none(LayerQualityStrategy::BackOffOnly, n_layers));
    };

    // Global validity back-off: keep the thickest layers that stay valid.
    // Valid = closed, no inverted cell AND no folded face (non-orthogonality
    // of 90° or more: the owner-to-neighbour vector against the face normal).
    // Since 2026-11-04 the prisms are assembled with their own winding
    // (`from_cell_faces_wound`), so a tangled prism stays closed and only its
    // volume or a folded face shows it; before, the orientation guess flipped
    // the faces of tangled cells and `validate` caught them by accident. The
    // two curved-wall pipeline tests (sphere, cylinder) failed on 92.4° and
    // 92.5° folds until the third condition was added.
    let counts: BTreeMap<usize, usize> = s.wall_pts.iter().map(|&wp| (wp, n_layers)).collect();
    let mut scale = 1.0;
    for attempt in 0..10 {
        let per_point: BTreeMap<usize, f64> = s.cap.iter().map(|(&wp, &c)| (wp, c * scale)).collect();
        let candidate = build_layered_variable(mesh, &s.wall_faces, &s.wall_pts, &s.inward, &s.d, &per_point, &counts);
        if candidate.validate().is_ok() {
            let q = check_quality(&candidate);
            if q.n_negative_volume_cells == 0 && q.max_non_orthogonality_deg < 90.0 && meets_layer_quality(&q) {
                let mut outcome = LayerOutcome::from_columns(LayerQualityStrategy::BackOffOnly, mesh, &s, &counts, &per_point, attempt + 1);
                if attempt > 0 {
                    outcome.notes.push(format!("uniform back-off: every column scaled by {scale:.4}"));
                }
                return (candidate, outcome);
            }
        }
        scale *= 0.6;
    }
    let mut none = LayerOutcome::none(LayerQualityStrategy::BackOffOnly, n_layers);
    none.iterations = 10;
    none.notes.push("uniform back-off found no valid thickness in 10 tries".into());
    (rebuild(), none) // too tight for any valid layer
}

/// How the layer stage meets upstream's mesh-quality limits (gh:#1215).
///
/// Three routes to the same job, kept callable side by side so they can be
/// compared on the same inputs (the differential-testing rule, `AGENTS.md`):
///
/// - [`Self::BackOffOnly`] (**the default**): this crate's own global
///   back-off. Build every layer at the per-point cap, check the whole mesh
///   against upstream's limits ([`meets_layer_quality`]), and scale *every*
///   column down by 0.6 until it passes or ten tries are spent. One bad face
///   thins the whole wall.
/// - [`Self::CfMeshOptimise`]: cfMesh's own repair, ported in
///   [`crate::boundary_layer_optimisation`] and [`crate::mesh_optimizer`]:
///   one layer at the cap, `meshOptimizer::optimizeLowQualityFaces`, the
///   `boundaryLayerOptimisation::optimiseLayer` hair-normal smoothing and
///   local thickness reduction, `untangleMeshFV`, then the split into the
///   requested layers and `untangleBoundaryLayer`. Thins locally and moves
///   interior points; never removes a layer.
/// - [`Self::SnappyCollapse`]: snappyHexMesh's per-point `extrudeMode` and
///   its add–check–unextrude loop, ported in [`crate::snappy_layers`]. Keeps
///   the full thickness where the mesh passes and removes the layer locally
///   (0 layers at a collapsed point) where it does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayerQualityStrategy {
    /// Global uniform back-off (this crate's, the behaviour before gh:#1215).
    #[default]
    BackOffOnly,
    /// cfMesh `boundaryLayerOptimisation` + `meshOptimizer` (local thinning
    /// and untangling).
    CfMeshOptimise,
    /// snappyHexMesh `snappyLayerDriver::addLayers` (local layer collapse).
    SnappyCollapse,
}

/// What a layer stage did, per wall point and per wall face (gh:#1215): the
/// instrument for coverage, achieved thickness and where layers were thinned
/// or collapsed. Points and faces are listed in the layerer's own order (wall
/// points ascending, wall faces ascending) with their positions, so a caller
/// can draw them without the mesh's numbering.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LayerOutcome {
    /// Which strategy produced it.
    pub strategy: LayerQualityStrategy,
    /// Layers requested.
    pub n_layers_requested: usize,
    /// Per wall point: (position on the wall, layers at the point, total
    /// thickness [m] of its column, the cap [m] the column was allowed).
    pub points: Vec<(Vec3, usize, f64, f64)>,
    /// Per wall face: (centre, layers of the face = the most of its points).
    pub faces: Vec<(Vec3, usize)>,
    /// Iterations the strategy's loop ran (back-off tries, snappy layer
    /// iterations, cfMesh `optimiseLayer` passes).
    pub iterations: usize,
    /// Faces still failing the strategy's own quality check at the end.
    pub remaining_bad_faces: usize,
    /// One line per notable event.
    pub notes: Vec<String>,
}

impl LayerOutcome {
    /// No layer at all.
    pub(crate) fn none(strategy: LayerQualityStrategy, n_layers_requested: usize) -> Self {
        LayerOutcome { strategy, n_layers_requested, ..Default::default() }
    }

    /// Outcome of a layered mesh built from columns.
    pub(crate) fn from_columns(
        strategy: LayerQualityStrategy,
        mesh: &VolumeMesh,
        s: &LayerSetup,
        counts: &BTreeMap<usize, usize>,
        totals: &BTreeMap<usize, f64>,
        iterations: usize,
    ) -> Self {
        let points = s
            .wall_pts
            .iter()
            .map(|&wp| (mesh.points[wp], counts[&wp], if counts[&wp] > 0 { totals[&wp] } else { 0.0 }, s.cap[&wp]))
            .collect();
        let faces = s
            .wall_faces
            .iter()
            .map(|&f| (mesh.face_centroid(f), mesh.faces[f].iter().map(|p| counts[p]).max().unwrap_or(0)))
            .collect();
        LayerOutcome { strategy, n_layers_requested: s.n_layers, points, faces, iterations, ..Default::default() }
    }

    /// Fraction of wall faces carrying the full requested layer count.
    pub fn full_coverage(&self) -> f64 {
        if self.faces.is_empty() {
            return 0.0;
        }
        self.faces.iter().filter(|f| f.1 == self.n_layers_requested && f.1 > 0).count() as f64 / self.faces.len() as f64
    }

    /// Fraction of wall points with no layer (collapsed, or none grown).
    pub fn collapsed_point_fraction(&self) -> f64 {
        if self.points.is_empty() {
            return 1.0;
        }
        self.points.iter().filter(|p| p.1 == 0).count() as f64 / self.points.len() as f64
    }

    /// Mean total thickness over the wall points that carry a layer [m].
    pub fn mean_total_thickness(&self) -> f64 {
        let t: Vec<f64> = self.points.iter().filter(|p| p.1 > 0).map(|p| p.2).collect();
        if t.is_empty() {
            0.0
        } else {
            t.iter().sum::<f64>() / t.len() as f64
        }
    }
}

/// The geometry every layer strategy starts from (gh:#1215; split out of
/// [`add_boundary_layers_adaptive_on_patches`] unchanged): the layer faces
/// and points, cfMesh's border classification with its constrained
/// directions and caps, the smoothed inward normals, the wall-point
/// adjacency, the cumulative requested thicknesses and the per-point cap on
/// the total thickness.
#[derive(Debug, Clone)]
pub(crate) struct LayerSetup {
    /// Boundary faces on the layer patches, ascending.
    pub wall_faces: Vec<usize>,
    /// Their points.
    pub wall_pts: BTreeSet<usize>,
    /// cfMesh `findPatchVertices` class of each wall point.
    pub kind: BTreeMap<usize, LayerVertex>,
    /// Unit march direction (inward) of each wall point.
    pub inward: BTreeMap<usize, Vec3>,
    /// Cumulative requested thickness `d[0] = 0 … d[n]` [m].
    pub d: Vec<f64>,
    /// Per-point cap on the total thickness [m].
    pub cap: BTreeMap<usize, f64>,
    /// Requested layers.
    pub n_layers: usize,
}

/// Classify and measure the layer patches (see [`LayerSetup`]). `None` when
/// no layer can be grown: no layer faces, zero layers or thickness, or a
/// border point upstream refuses (three or more layer-free patches, or two
/// with no shared edge).
pub(crate) fn layer_setup(
    mesh: &VolumeMesh,
    layer_patches: &[&str],
    n_layers: usize,
    first_thickness: f64,
    expansion: f64,
) -> Option<LayerSetup> {
    if n_layers == 0 || first_thickness <= 0.0 {
        return None;
    }
    // Patch of every boundary face (usize::MAX for internal faces).
    let mut patch_of = vec![usize::MAX; mesh.face_count()];
    for (pi, p) in mesh.patches.iter().enumerate() {
        for f in p.start_face..p.start_face + p.n_faces {
            patch_of[f] = pi;
        }
    }
    let treated: Vec<bool> = mesh.patches.iter().map(|p| layer_patches.contains(&p.name.as_str())).collect();
    let wall_faces: Vec<usize> = (0..mesh.face_count())
        .filter(|&f| patch_of[f] != usize::MAX && treated[patch_of[f]])
        .collect();
    if wall_faces.is_empty() {
        return None;
    }

    // Boundary faces around each point (cfMesh `pointFaces`), in face order,
    // as mesh face labels: from the shared surface engine (gh:#1257; this
    // stage built its own map before, identical element for element:
    // `surface_engine::tests::engine_matches_the_crates_earlier_boundary_maps_on_the_pipe`).
    let engine = crate::surface_engine::SurfaceEngine::new(mesh);
    let point_faces = |v: usize| -> Vec<usize> {
        match engine.bp.get(v) {
            Some(&b) if b != crate::surface_engine::NOT_BOUNDARY => {
                engine.point_faces[b].iter().map(|&bf| engine.boundary_faces[bf]).filter(|&f| patch_of[f] != usize::MAX).collect()
            }
            _ => Vec::new(),
        }
    };

    // Wall points and their (averaged) inward normal.
    let mut wall_pts: BTreeSet<usize> = BTreeSet::new();
    let mut inward: BTreeMap<usize, Vec3> = BTreeMap::new();
    for &f in &wall_faces {
        let out = mesh.face_area_vector(f).normalize();
        for &p in &mesh.faces[f] {
            wall_pts.insert(p);
            let e = inward.entry(p).or_insert(Vec3::ZERO);
            *e = e.sub(out);
        }
    }
    for v in inward.values_mut() {
        *v = v.normalize();
    }

    // Classify the border points and fix their constrained directions and
    // thickness caps (upstream `createNewVertex`).
    let mut kind: BTreeMap<usize, LayerVertex> = BTreeMap::new();
    let mut border_cap: BTreeMap<usize, f64> = BTreeMap::new();
    for &wp in &wall_pts {
        let faces = &point_faces(wp);
        let mut others: Vec<usize> = Vec::new();
        for &f in faces {
            let pi = patch_of[f];
            if !treated[pi] && !others.contains(&pi) {
                others.push(pi);
            }
        }
        let x = mesh.points[wp];
        match others.len() {
            0 => {
                kind.insert(wp, LayerVertex::Patch);
            }
            1 => {
                let (mut n, mut v) = (Vec3::ZERO, Vec3::ZERO);
                for &f in faces {
                    if treated[patch_of[f]] {
                        n = n.add(mesh.face_area_vector(f));
                    } else {
                        v = v.add(mesh.face_area_vector(f));
                    }
                }
                let v = v.scale(1.0 / (v.length() + f64::MIN_POSITIVE));
                let n = n.sub(v.scale(n.dot(v)));
                let n = n.scale(1.0 / (n.length() + f64::MIN_POSITIVE));
                inward.insert(wp, n.scale(-1.0));
                // Upstream "limit distances": 0.9 x the distance to the edge
                // opposite the point in each adjacent non-layer face.
                let mut cap = f64::MAX;
                for &f in faces {
                    if treated[patch_of[f]] {
                        continue;
                    }
                    let ring = &mesh.faces[f];
                    let k = ring.len();
                    if let Some(pos) = ring.iter().position(|&q| q == wp) {
                        let a = mesh.points[ring[(pos + k - 1) % k]];
                        let b = mesh.points[ring[(pos + 1) % k]];
                        cap = cap.min(0.9 * distance_to_segment(x, a, b));
                    }
                }
                border_cap.insert(wp, cap);
                kind.insert(wp, LayerVertex::Edge);
            }
            2 => {
                // The neighbouring boundary point on both other patches.
                let mut target: Option<usize> = None;
                'search: for &f in faces {
                    let ring = &mesh.faces[f];
                    let k = ring.len();
                    let Some(pos) = ring.iter().position(|&q| q == wp) else { continue };
                    for q in [ring[(pos + k - 1) % k], ring[(pos + 1) % k]] {
                        let on = |pi: usize| point_faces(q).iter().any(|&g| patch_of[g] == pi);
                        if on(others[0]) && on(others[1]) {
                            target = Some(q);
                            break 'search;
                        }
                    }
                }
                let q = target?;
                let d = mesh.points[q].sub(x);
                inward.insert(wp, d.normalize());
                border_cap.insert(wp, 0.5 * d.length());
                kind.insert(wp, LayerVertex::Corner);
            }
            _ => {
                return None;
            }
        }
    }

    // Wall-point adjacency + shortest incident wall edge (the local spacing).
    let mut nbrs: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    let mut min_edge: BTreeMap<usize, f64> = BTreeMap::new();
    for &f in &wall_faces {
        let w = &mesh.faces[f];
        let k = w.len();
        for i in 0..k {
            let (a, b) = (w[i], w[(i + 1) % k]);
            nbrs.entry(a).or_default().insert(b);
            nbrs.entry(b).or_default().insert(a);
            let len = mesh.points[a].sub(mesh.points[b]).length();
            for &v in &[a, b] {
                let e = min_edge.entry(v).or_insert(f64::MAX);
                *e = e.min(len);
            }
        }
    }

    // Smooth the normal field (advancing-layer style): each pass replaces a
    // normal with the unit average of itself and its wall neighbours. Border
    // points keep their constrained direction.
    for _ in 0..3 {
        let smoothed: BTreeMap<usize, Vec3> = wall_pts
            .iter()
            .map(|&wp| {
                if kind[&wp] != LayerVertex::Patch {
                    return (wp, inward[&wp]);
                }
                let mut s = inward[&wp];
                if let Some(ns) = nbrs.get(&wp) {
                    for &nb in ns {
                        s = s.add(inward[&nb]);
                    }
                }
                let sn = s.normalize();
                // Keep the original if smoothing collapsed the direction.
                (wp, if sn.length() > 0.5 { sn } else { inward[&wp] })
            })
            .collect();
        inward = smoothed;
    }

    // Cumulative profile d[0..=n] (the layer grading).
    let mut d = vec![0.0f64; n_layers + 1];
    let mut t = first_thickness;
    for m in 1..=n_layers {
        d[m] = d[m - 1] + t;
        t *= expansion;
    }
    let total_req = d[n_layers];

    // Per-point total-thickness cap: a fraction of the local wall spacing, and
    // on the border upstream's limit.
    const BETA: f64 = 0.5;
    let cap: BTreeMap<usize, f64> = wall_pts
        .iter()
        .map(|&wp| {
            let c = total_req.min(BETA * min_edge[&wp]);
            (wp, border_cap.get(&wp).map_or(c, |&b| c.min(b)))
        })
        .collect();
    Some(LayerSetup { wall_faces, wall_pts, kind, inward, d, cap, n_layers })
}

/// Upstream's mesh-quality limits for layer addition (gh:#1202), from
/// OpenFOAM-dev `852527377c7e` `etc/caseDicts/mesh/generation/meshQualityDict`,
/// which snappyHexMesh's `addLayers` checks the layered mesh against:
/// `maxInternalSkewness 4`, `minTetQuality 1e-15` (here: no non-positive
/// face pyramid about a cell centre), `minDeterminant 0.001`. Its
/// `maxNonOrtho 65` is not applied: the tet dual itself sits at 50–57°, and
/// the caller keeps its own bound (< 90°). Before 2026-11-04 the back-off
/// checked closure, volume and non-orthogonality only, and 3-layer pipes
/// at skewness 6.6–7.2 were accepted and diverged in the solver
/// (`laminar_pipe_gh1155.md`, step 3).
pub(crate) fn meets_layer_quality(q: &crate::checks::QualityReport) -> bool {
    const MAX_INTERNAL_SKEWNESS: f64 = 4.0;
    const MIN_DETERMINANT: f64 = 0.001;
    q.max_skewness <= MAX_INTERNAL_SKEWNESS && q.n_negative_pyramid_faces == 0 && q.min_cell_determinant >= MIN_DETERMINANT
}

/// Distance from `p` to the segment `[a, b]` (cfMesh
/// `help::distanceOfPointFromTheEdge`).
fn distance_to_segment(p: Vec3, a: Vec3, b: Vec3) -> f64 {
    let e = b.sub(a);
    let l2 = e.dot(e);
    let t = if l2 > 0.0 { (p.sub(a).dot(e) / l2).clamp(0.0, 1.0) } else { 0.0 };
    p.sub(a.add(e.scale(t))).length()
}

/// Assemble a layered mesh with a **per-point layer count** (gh:#1215): wall
/// point `p` carries `counts[p]` layers over a column of total thickness
/// `per_point_total[p]`, graded by the requested profile `d` restricted to its
/// own layers (`d[j] / d[counts[p]]`). Each wall face gets as many cells as
/// the most of its points (addPatchCellLayer's rule: `nPointLayers` is the max
/// over the connected faces), and in its layer `i` a point with fewer layers
/// stands at the top of its column, so the cell's edge there collapses: a
/// prism becomes a wedge or a pyramid where a layer ends. Every face is built
/// from the same column points on both of its sides, so neighbouring stacks
/// with different counts conform; faces whose points collapse to fewer than
/// three are dropped. A point with `counts[p] = 0` does not move.
///
/// How this differs from upstream (OpenFOAM-dev `852527377c7e`,
/// `addPatchCellLayer::setRefinement`,
/// `src/polyTopoChange/polyTopoChange/addPatchCellLayer.C`): both align a
/// face's layers with the wall and keep an unextruded point where it is;
/// upstream gives a face fewer cells than its points have (`nFaceLayers`, 1 on
/// a face where a layer ends, with `nBufferCellsNoExtrude 0`) and lets that
/// face's first cell span the rest of each column, where this builder gives
/// the face the full count with collapsed edges. The cells cover the same
/// region.
///
/// With every count equal to `n` this is the uniform builder the back-off
/// always used, bit for bit (pinned by
/// `pipeline::tests::back_off_only_layers_are_pinned_bit_for_bit`). Points and
/// cells are numbered in a fixed order (wall points ascending), so the result
/// is reproducible.
pub(crate) fn build_layered_variable(
    mesh: &VolumeMesh,
    wall_faces: &[usize],
    wall_pts: &BTreeSet<usize>,
    inward: &BTreeMap<usize, Vec3>,
    d: &[f64],
    per_point_total: &BTreeMap<usize, f64>,
    counts: &BTreeMap<usize, usize>,
) -> VolumeMesh {
    build_layered_variable_with_columns(mesh, wall_faces, wall_pts, inward, d, per_point_total, counts).0
}

/// [`build_layered_variable`] with each wall point's column of new point
/// labels (wall first), for callers that name the new faces
/// ([`boundary_layers`], gh:#1270). The same code path.
pub(crate) fn build_layered_variable_with_columns(
    mesh: &VolumeMesh,
    wall_faces: &[usize],
    wall_pts: &BTreeSet<usize>,
    inward: &BTreeMap<usize, Vec3>,
    d: &[f64],
    per_point_total: &BTreeMap<usize, f64>,
    counts: &BTreeMap<usize, usize>,
) -> (VolumeMesh, BTreeMap<usize, Vec<usize>>) {
    let (points, column) = layer_columns(mesh, wall_pts, inward, d, per_point_total, counts);
    let m = assemble_layered(mesh, wall_faces, wall_pts, points, &column, counts);
    (m, column)
}

/// The point list of a layered mesh: the non-wall points in order, then each
/// wall point's column (wall first) — `counts[p] + 1` points, or the point
/// itself when `counts[p] = 0`. Returns the points and each wall point's
/// column of new indices.
pub(crate) fn layer_columns(
    mesh: &VolumeMesh,
    wall_pts: &BTreeSet<usize>,
    inward: &BTreeMap<usize, Vec3>,
    d: &[f64],
    per_point_total: &BTreeMap<usize, f64>,
    counts: &BTreeMap<usize, usize>,
) -> (Vec<Vec3>, BTreeMap<usize, Vec<usize>>) {
    let mut new_points: Vec<Vec3> = Vec::new();
    for (op, &pt) in mesh.points.iter().enumerate() {
        if !wall_pts.contains(&op) {
            new_points.push(pt);
        }
    }
    let mut column: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for &wp in wall_pts {
        let x = mesh.points[wp];
        let nrm = inward[&wp];
        let n_p = counts[&wp];
        let mut col = Vec::with_capacity(n_p + 1);
        if n_p == 0 {
            col.push(new_points.len());
            new_points.push(x);
        } else {
            let tot = per_point_total[&wp];
            for dj in &d[..=n_p] {
                let frac = dj / d[n_p];
                col.push(new_points.len());
                new_points.push(x.add(nrm.scale(tot * frac)));
            }
        }
        column.insert(wp, col);
    }
    (new_points, column)
}

/// Assemble the layered mesh from its points and columns (see
/// [`build_layered_variable`]): the original cells with every wall point
/// replaced by the top of its column, then the layer cells of each wall face.
pub(crate) fn assemble_layered(
    mesh: &VolumeMesh,
    wall_faces: &[usize],
    wall_pts: &BTreeSet<usize>,
    points: Vec<Vec3>,
    column: &BTreeMap<usize, Vec<usize>>,
    counts: &BTreeMap<usize, usize>,
) -> VolumeMesh {
    let mut new_of = vec![usize::MAX; mesh.points.len()];
    let mut next = 0usize;
    for (op, slot) in new_of.iter_mut().enumerate() {
        if !wall_pts.contains(&op) {
            *slot = next;
            next += 1;
        }
    }
    let remap = |op: usize| -> usize {
        if wall_pts.contains(&op) {
            *column[&op].last().unwrap()
        } else {
            new_of[op]
        }
    };
    let mut cells: Vec<Vec<Vec<usize>>> = Vec::new();
    for cf in cells_faces(mesh) {
        cells.push(
            cf.iter()
                .map(|ring| ring.iter().map(|&p| remap(p)).collect())
                .collect(),
        );
    }
    for &f in wall_faces {
        let m_f = mesh.faces[f].iter().map(|p| counts[p]).max().unwrap_or(0);
        prism_stack_variable(&mesh.faces[f], column, m_f, &mut cells);
    }
    from_cell_faces_wound(points, &cells)
}

/// The `m_f` layer cells of wall face `w` over columns of different lengths
/// (see [`build_layered_variable`]); with equal columns, exactly the cells of
/// [`prism_stack`]. Faces are wound as there; cyclically repeated points are
/// removed and faces left with fewer than three points dropped.
fn prism_stack_variable(w: &[usize], column: &BTreeMap<usize, Vec<usize>>, m_f: usize, cells: &mut Vec<Vec<Vec<usize>>>) {
    let at = |p: usize, i: usize| -> usize {
        let c = &column[&p];
        c[i.min(c.len() - 1)]
    };
    let k = w.len();
    for i in 0..m_f {
        let mut faces: Vec<Vec<usize>> = Vec::with_capacity(k + 2);
        let mut push = |ring: Vec<usize>| {
            let ring = dedupe_ring(ring);
            if ring.len() >= 3 {
                faces.push(ring);
            }
        };
        push(w.iter().map(|&p| at(p, i)).collect());
        push(w.iter().rev().map(|&p| at(p, i + 1)).collect());
        for e in 0..k {
            let (p, q) = (w[e], w[(e + 1) % k]);
            push(vec![at(p, i), at(p, i + 1), at(q, i + 1), at(q, i)]);
        }
        cells.push(faces);
    }
}

/// [`build_layered_variable`] with **upstream's per-face layer counts**
/// (gh:#1241): wall face `wall_faces[i]` gets `face_layers[i]` cells, as
/// `addPatchCellLayer::setRefinement` gives a patch face `nFaceLayers`
/// cells (OpenFOAM-dev `852527377c7e`,
/// `src/polyTopoChange/polyTopoChange/addPatchCellLayer.C`: the face's
/// layer `i` uses `addedPoints[i + offset]`, `offset = addedPoints.size() −
/// nFaceLayers`). In this crate's column order (wall first) that is: the
/// face's cells `0 … L−2` are the wall-side layers of each column, and its
/// last cell spans the rest of every column up to the column's top. Where a
/// neighbouring face has more layers the last cell's side is split at the
/// neighbour's levels (upstream's side faces per edge layer, `edgeLayers` =
/// the most of the edge's faces), so the faces conform. Every extruded
/// point must carry at least the layers of each of its faces
/// (`setupLayerInfoTruncation` guarantees it). With every face's count equal
/// to its points' it is [`build_layered_variable`] exactly.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_layered_faces(
    mesh: &VolumeMesh,
    wall_faces: &[usize],
    wall_pts: &BTreeSet<usize>,
    inward: &BTreeMap<usize, Vec3>,
    d: &[f64],
    per_point_total: &BTreeMap<usize, f64>,
    counts: &BTreeMap<usize, usize>,
    face_layers: &[usize],
) -> VolumeMesh {
    let (points, column) = layer_columns(mesh, wall_pts, inward, d, per_point_total, counts);
    let mut new_of = vec![usize::MAX; mesh.points.len()];
    let mut next = 0usize;
    for (op, slot) in new_of.iter_mut().enumerate() {
        if !wall_pts.contains(&op) {
            *slot = next;
            next += 1;
        }
    }
    let remap = |op: usize| -> usize {
        if wall_pts.contains(&op) {
            *column[&op].last().unwrap()
        } else {
            new_of[op]
        }
    };
    let mut cells: Vec<Vec<Vec<usize>>> = cells_faces(mesh).iter().map(|cf| cf.iter().map(|ring| ring.iter().map(|&p| remap(p)).collect()).collect()).collect();
    // edgeLayers: the most layers of the wall faces on each wall edge.
    let mut edge_layers: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    for (i, &f) in wall_faces.iter().enumerate() {
        let w = &mesh.faces[f];
        for e in 0..w.len() {
            let (a, b) = (w[e], w[(e + 1) % w.len()]);
            let v = edge_layers.entry((a.min(b), a.max(b))).or_insert(0);
            *v = (*v).max(face_layers[i]);
        }
    }
    for (i, &f) in wall_faces.iter().enumerate() {
        let w = &mesh.faces[f];
        let l = face_layers[i];
        // Position of point p at the face's level s (0 = wall, l = top).
        let lv = |p: usize, s: usize| -> usize {
            let c = &column[&p];
            let n_p = c.len() - 1;
            if n_p == 0 {
                c[0]
            } else if s < l {
                c[s]
            } else {
                c[n_p]
            }
        };
        let k = w.len();
        for layer in 0..l {
            let mut faces: Vec<Vec<usize>> = Vec::with_capacity(k + 2);
            let mut push = |ring: Vec<usize>| {
                let ring = dedupe_ring(ring);
                if ring.len() >= 3 {
                    faces.push(ring);
                }
            };
            push(w.iter().map(|&p| lv(p, layer)).collect());
            push(w.iter().rev().map(|&p| lv(p, layer + 1)).collect());
            for e in 0..k {
                let (p, q) = (w[e], w[(e + 1) % k]);
                if layer + 1 < l {
                    push(vec![lv(p, layer), lv(p, layer + 1), lv(q, layer + 1), lv(q, layer)]);
                    continue;
                }
                // The face's last cell: split at the edge's levels.
                let le = edge_layers[&(p.min(q), p.max(q))];
                let seq = |x: usize| -> Vec<usize> {
                    let c = &column[&x];
                    let n_x = c.len() - 1;
                    if n_x == 0 {
                        return vec![c[0]; le - layer + 1];
                    }
                    let mut v: Vec<usize> = (layer..le).map(|j| c[j.min(n_x)]).collect();
                    v.push(c[n_x]);
                    v
                };
                let (sp, sq) = (seq(p), seq(q));
                for j in 0..sp.len() - 1 {
                    push(vec![sp[j], sp[j + 1], sq[j + 1], sq[j]]);
                }
            }
            cells.push(faces);
        }
    }
    from_cell_faces_wound(points, &cells)
}

/// `ring` with cyclically consecutive repeats removed.
fn dedupe_ring(ring: Vec<usize>) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::with_capacity(ring.len());
    for v in ring {
        if out.last() != Some(&v) {
            out.push(v);
        }
    }
    while out.len() > 1 && out.first() == out.last() {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carve::carve_box;
    use crate::shapes::box_surface;

    /// V&V (gh:#1241) — **upstream's per-face layer counts**
    /// (`build_layered_faces`, `addPatchCellLayer`'s `nFaceLayers`). A unit
    /// box carved 3×3×3, 3 layers on `walls`, one wall point not extruded:
    /// the faces touching it get 1 cell (spanning the columns), every other
    /// face 3, and every other point 3 layers. Predicted before the first
    /// run: closed; no inverted cell; the volume is the box's (a
    /// repartition) to 1e-12; cells = 27 + 3 × (54 − k) + k for the k faces
    /// at the point; and with every face at 3 the mesh is
    /// `build_layered_variable`'s bit for bit.
    #[test]
    fn per_face_layer_counts_conform_and_repartition() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let base = carve_box(&p, &t, 1.0 / 3.0);
        let s = layer_setup(&base, &["walls"], 3, 0.02, 1.3).unwrap();
        let totals: BTreeMap<usize, f64> = s.wall_pts.iter().map(|&w| (w, 0.05)).collect();
        let all3: BTreeMap<usize, usize> = s.wall_pts.iter().map(|&w| (w, 3)).collect();
        let f3 = vec![3usize; s.wall_faces.len()];
        let a = build_layered_faces(&base, &s.wall_faces, &s.wall_pts, &s.inward, &s.d, &totals, &all3, &f3);
        let b = build_layered_variable(&base, &s.wall_faces, &s.wall_pts, &s.inward, &s.d, &totals, &all3);
        assert_eq!((a.points.clone(), a.faces.clone(), a.owner.clone(), a.neighbour.clone()), (b.points, b.faces, b.owner, b.neighbour));
        // Unextrude the first wall point (a box corner in 3 wall faces).
        let x = *s.wall_pts.iter().next().unwrap();
        let mut counts = all3.clone();
        counts.insert(x, 0);
        let fl: Vec<usize> = s.wall_faces.iter().map(|&f| if base.faces[f].contains(&x) { 1 } else { 3 }).collect();
        let k = fl.iter().filter(|&&l| l == 1).count();
        let m = build_layered_faces(&base, &s.wall_faces, &s.wall_pts, &s.inward, &s.d, &totals, &counts, &fl);
        m.validate().expect("closed");
        assert_eq!(crate::checks::check_quality(&m).n_negative_volume_cells, 0);
        assert!((m.total_volume() - 1.0).abs() < 1e-12, "{}", m.total_volume());
        assert_eq!(m.cell_count(), 27 + 3 * (54 - k) + k);
        assert!(k >= 1);
    }

    /// V&V — headline. Methodology: a unit box carved 3×3×3 (a single `walls`
    /// patch), then 3 graded boundary layers added on `walls`. Pass criteria:
    /// the operation is a repartition, so it preserves the exact volume and
    /// stays closed; the cell count grows by exactly `n_layers × wall_faces`;
    /// the outer wall faces sit back at the original box faces. Result: volume 1
    /// (exact); validate() Ok; +3·54 cells; the mesh's bounding box is still the
    /// unit box.
    #[test]
    fn box_boundary_layers_preserve_volume_and_close() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let base = carve_box(&p, &t, 1.0 / 3.0); // 3×3×3 = 27 cells, single `walls` patch
        let wall_faces = base.n_boundary_faces(); // 6 * 9 = 54
        let layered = add_boundary_layers(&base, "walls", 3, 0.02, 1.3);

        assert!(
            (layered.total_volume() - 1.0).abs() < 1e-9,
            "volume preserved: {}",
            layered.total_volume()
        );
        layered.validate().expect("layered mesh is closed");
        assert_eq!(
            layered.cell_count(),
            base.cell_count() + 3 * wall_faces,
            "n_layers × wall faces added"
        );

        // The outer wall still bounds the original unit box.
        let (mut lo, mut hi) = (layered.points[0], layered.points[0]);
        for p in &layered.points {
            lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
            hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
        }
        assert!(
            lo.length() < 1e-9 && hi.sub(Vec3::new(1.0, 1.0, 1.0)).length() < 1e-9,
            "outer wall at the box"
        );
    }

    /// V&V — a single layer is the minimal case, and the first-layer thickness
    /// is honoured: with n = 1, the shrunk interior wall sits `first_thickness`
    /// inside the original wall (measured on a face-centre column, where the
    /// inward normal is purely axial).
    #[test]
    fn single_layer_thickness_is_honoured() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let base = carve_box(&p, &t, 0.5); // 2×2×2 = 8 cells, single `walls` patch
        let layered = add_boundary_layers(&base, "walls", 1, 0.05, 1.0);
        assert!((layered.total_volume() - 1.0).abs() < 1e-9);
        layered.validate().expect("closed");
        // The inner shell is inset by ~0.05 on flat faces; the −X wall's
        // face-centre point (0, 0.5, 0.5) has a pure +X inward normal, so it
        // moves to exactly x = 0.05.
        let near = layered
            .points
            .iter()
            .filter(|p| (p.x - 0.05).abs() < 1e-6)
            .count();
        assert!(
            near > 0,
            "wall points moved inward by the first-layer thickness"
        );
    }

    /// V&V — **curved-wall** layers (the headline for the adaptive variant).
    /// Methodology: a snapped, face-minimal-dualised sphere (radius 3) — a curved
    /// polyhedral wall on which the fixed-thickness [`add_boundary_layers`]
    /// self-intersects and invalidates the mesh — takes prism layers via
    /// [`add_boundary_layers_adaptive`]. Pass criteria: the result is **closed**
    /// (`validate` Ok), has **no inverted cells**, adds prism cells, conserves
    /// the volume (a repartition — the outer wall is unmoved), and keeps
    /// non-orthogonality within the practical boundary-layer limit (< 85°).
    ///
    /// Note on the 85° bar: near-wall prism layers are *intrinsically*
    /// non-orthogonal and high-aspect-ratio, so a boundary-layer mesh legitimately
    /// exceeds `checkMesh`'s conservative 70° warning (which
    /// [`check_quality`](crate::checks::check_quality)`::is_solvable` uses); such
    /// meshes are solved with non-orthogonal correctors. Result (cell size 0.6):
    /// closed; 0 inverted; volume conserved; max non-orthogonality ≈ 79°.
    #[test]
    fn adaptive_layers_work_on_a_curved_sphere() {
        use crate::carve::carve_box;
        use crate::checks::check_quality;
        use crate::dual::polyhedral_dual_min_faces;
        use crate::shapes::sphere_surface;
        use crate::snap::snap_to_surface;

        let (p, t) = sphere_surface(Vec3::ZERO, 3.0, 24, 48);
        let poly = polyhedral_dual_min_faces(&snap_to_surface(&carve_box(&p, &t, 0.6), &p, &t));
        let vol0 = poly.total_volume();

        let layered = add_boundary_layers_adaptive(&poly, "walls", 3, 0.04, 1.3);
        layered
            .validate()
            .expect("curved-wall layered mesh is closed");
        assert!(
            layered.cell_count() > poly.cell_count(),
            "prism layers added"
        );
        assert!(
            (layered.total_volume() - vol0).abs() < 1e-6,
            "volume conserved: {} vs {vol0}",
            layered.total_volume()
        );
        let q = check_quality(&layered);
        assert_eq!(q.n_negative_volume_cells, 0, "no inverted cells");
        assert!(
            q.max_non_orthogonality_deg < 85.0,
            "non-orthogonality within BL limit: {}",
            q.max_non_orthogonality_deg
        );
    }

    /// V&V — the adaptive variant also handles a cylinder (mixed flat caps +
    /// curved side). Methodology: snapped, dualised cylinder → adaptive layers.
    /// Pass criteria: closed, prism cells added, volume conserved.
    #[test]
    fn adaptive_layers_work_on_a_cylinder() {
        use crate::carve::carve_box;
        use crate::dual::polyhedral_dual_min_faces;
        use crate::shapes::cylinder_surface;
        use crate::snap::snap_to_surface;

        let (p, t) = cylinder_surface(Vec3::ZERO, 2.0, 5.0, 48);
        let poly = polyhedral_dual_min_faces(&snap_to_surface(&carve_box(&p, &t, 0.6), &p, &t));
        let vol0 = poly.total_volume();

        let layered = add_boundary_layers_adaptive(&poly, "walls", 3, 0.04, 1.3);
        layered.validate().expect("cylinder layered mesh is closed");
        assert!(
            layered.cell_count() > poly.cell_count(),
            "prism layers added"
        );
        assert!(
            (layered.total_volume() - vol0).abs() < 1e-6,
            "volume conserved"
        );
    }

    /// V&V — the adaptive variant matches the exact one on a **flat box** wall
    /// (where no back-off is needed): still valid, volume conserved, and prism
    /// cells added (`+n_layers × wall_faces`).
    #[test]
    fn adaptive_layers_match_on_flat_box() {
        use crate::carve::carve_box;
        use crate::shapes::box_surface;

        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let base = carve_box(&p, &t, 1.0 / 3.0);
        let wall_faces = base.n_boundary_faces();
        let layered = add_boundary_layers_adaptive(&base, "walls", 3, 0.02, 1.3);
        assert!(
            (layered.total_volume() - 1.0).abs() < 1e-9,
            "volume conserved on box"
        );
        layered.validate().expect("box layered mesh is closed");
        assert_eq!(
            layered.cell_count(),
            base.cell_count() + 3 * wall_faces,
            "prism cells added"
        );
    }

    /// V&V — `n_layers = 0` is a faithful rebuild (no change in volume or cells).
    #[test]
    fn zero_layers_is_a_noop_on_patches_too() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let base = carve_box(&p, &t, 0.5);
        let same = add_boundary_layers_adaptive_on_patches(&base, &["walls"], 0, 0.05, 1.2);
        assert_eq!(same.cell_count(), base.cell_count());
        let none = add_boundary_layers_adaptive_on_patches(&base, &["no-such-patch"], 2, 0.05, 1.2);
        assert_eq!(none.cell_count(), base.cell_count());
        assert!(distance_to_segment(Vec3::new(0.0, 1.0, 0.0), Vec3::ZERO, Vec3::new(2.0, 0.0, 0.0)) == 1.0);
        assert!(distance_to_segment(Vec3::new(3.0, 0.0, 0.0), Vec3::ZERO, Vec3::new(2.0, 0.0, 0.0)) == 1.0);
    }

    /// V&V — **wall-only layers on a box duct** (gh:#1202), the case with
    /// truly flat ends. Methodology: the unit box carved 4×4×4, its −X / +X
    /// sides named `inlet` / `outlet`, the other four `walls`; 2 layers (first
    /// 0.02, expansion 1.2, total t = 0.044) on `walls` only.
    ///
    /// Predictions, written before the first run (2026-11-04): exactly
    /// `2 × 64` prisms added (16 faces on each of 4 walls); closed, no
    /// inverted cell, volume 1 to 1e-9 (a repartition); every inlet point at
    /// x = 0 and outlet point at x = 1 exactly, each end's area 1 to 1e-9 (the
    /// ends are re-partitioned in their planes, cfMesh's border rule); the
    /// prisms' share of the inlet a frame of width t, `1 − (1 − 2t)² ≈ 0.168`
    /// less the corner chamfers (the corner points slide along the diagonal),
    /// so in [0.15, 0.18].
    #[test]
    fn wall_only_layers_on_a_box_keep_the_ends() {
        use crate::patches::{assign_patches_by_region, SurfaceRegions};
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let regions = SurfaceRegions::from_labels(&[
            "walls", "walls", "walls", "walls", "walls", "walls", "walls", "walls", "outlet", "outlet", "inlet", "inlet",
        ]);
        let named = assign_patches_by_region(&carve_box(&p, &t, 0.25), &p, &t, &regions).unwrap();
        let n0 = named.cell_count();
        let l = add_boundary_layers_adaptive_on_patches(&named, &["walls"], 2, 0.02, 1.2);
        assert_eq!(l.cell_count(), n0 + 2 * 64);
        l.validate().unwrap();
        assert_eq!(check_quality(&l).n_negative_volume_cells, 0);
        assert!((l.total_volume() - 1.0).abs() < 1e-9);
        // The rebuilt mesh has one patch; the ends are the boundary faces in
        // the x = 0 / x = 1 planes, and nothing else may touch those planes.
        let (mut a_in, mut a_out, mut a_prism) = (0.0, 0.0, 0.0);
        for f in l.n_internal_faces()..l.face_count() {
            let a = l.face_area_vector(f);
            let xs: Vec<f64> = l.faces[f].iter().map(|&v| l.points[v].x).collect();
            if a.x < -0.999 * a.length() {
                assert!(xs.iter().all(|&x| x == 0.0), "inlet face off x = 0: {xs:?}");
                a_in += a.length();
                if l.owner[f] >= n0 {
                    a_prism += a.length();
                }
            } else if a.x > 0.999 * a.length() {
                assert!(xs.iter().all(|&x| x == 1.0), "outlet face off x = 1: {xs:?}");
                a_out += a.length();
            }
        }
        assert!((a_in - 1.0).abs() < 1e-9 && (a_out - 1.0).abs() < 1e-9, "{a_in} {a_out}");
        assert!((0.15..0.18).contains(&a_prism), "prism share of the inlet {a_prism}");
    }

    /// V&V — `n_layers = 0` is a faithful rebuild (no change in volume or cells).
    #[test]
    fn zero_layers_is_a_noop() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let base = carve_box(&p, &t, 0.5); // 2×2×2 = 8 cells
        let same = add_boundary_layers(&base, "walls", 0, 0.05, 1.2);
        assert_eq!(same.cell_count(), base.cell_count());
        assert!((same.total_volume() - 1.0).abs() < 1e-9);
        same.validate().expect("closed");
    }
}
