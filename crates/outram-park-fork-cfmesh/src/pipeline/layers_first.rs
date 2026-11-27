// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Provenance: original OUTRAM PARK code (gh:#1829). It composes this crate's
// existing pieces: the layer columns of `crate::layers` (`layer_setup`,
// `layer_columns`, `assemble_layered`: cfMesh `boundaryLayers`'
// `findPatchVertices` / `createNewVertex` border rules), the TetGen 1.6.0
// port (`crate::tetgen`, AGPL-3.0-or-later, Hang Si / WIAS) through
// `tet_source`, and cfMesh's `triSurfaceChecks::checkSelfIntersections`
// port (`crate::surface::checks`). The order of work (grow the layers from
// the wall triangulation first, then mesh the region the layers leave) is
// Gmsh's boundary-layer extrusion (`src/mesh/BoundaryLayers.cpp`, Gmsh
// 4.x, github.com/live-clones/gmsh commit c89709a591d0, GPL-2.0-or-later,
// Copyright (C) 1997-2026 C. Geuzaine, J.-F. Remacle), read for this
// module; no Gmsh code is translated.

//! **Layers first (gh:#1829): grow the prism layers from the wall, then let
//! TetGen mesh what is left.** Since 2026-11-26 the only boundary-layer
//! route of the tet → dual → layers pipeline (maintainer: "Don't make it
//! opt in, make it default. Hard rule for this crate. Take out the old
//! option.").
//!
//! # Why this order
//!
//! Every earlier route grew the layers *after* the volume mesh, by pushing
//! existing wall cells inward (cfMesh's and snappyHexMesh's order). On
//! TetGen's tetrahedra that grew nothing (back-off: the bare tets already
//! fail `minDeterminant 0.001`), or upstream's thickness rather than the
//! requested one (cfMesh `tetMesh`), or refused the pebble. Growing the
//! prisms into empty space first, from the wall triangulation, gives the
//! requested layer count and thickness wherever the geometry has room, and
//! TetGen then meshes the inner region bounded by the layers' top surface
//! with `-Y` (no Steiner point on the boundary), so every tetrahedron meets
//! the prisms face to face.
//!
//! # Steps ([`grow_layers_first`])
//!
//! 1. **The wall triangulation.** TetGen meshes the input surface once
//!    (`-pq<r>/<d>[a<V>][m]`, the pipeline's [`TetSource::TetGen`],
//!    [`super::tet_source::tetgen_tets_graded`]); its boundary triangles
//!    (the input surface refined by TetGen's Steiner points, named by
//!    region) are the wall the layers grow from. Its tetrahedra are
//!    discarded. The input surface's own triangles are no use for this
//!    (Mesh Studio's pipe wall is 32 triangles 6 m long).
//! 2. **Columns** ([`crate::layers::layer_setup`], unchanged): every wall
//!    point gets an inward direction, the unit average of its wall faces'
//!    inward normals smoothed three times over its wall neighbours; a point
//!    on the border with one non-layer patch (an inlet) moves inside that
//!    patch's plane (cfMesh `createNewVertex`'s `EDGENODE` rule), so the
//!    layers' side faces lie in the inlet; a point between two non-layer
//!    patches moves along their shared edge.
//!    Gmsh (`BoundaryLayers.cpp`, `addExtrudeNormals`) instead weights each
//!    face normal by the face's angle at the vertex and does not smooth;
//!    on the uniform wall triangulations here the two differ little, and
//!    the existing columns were reused rather than written again.
//! 3. **Thickness: the request, thinned only where it must be.** Each
//!    column starts at the requested total (`first × Σ ratioᵏ`, the profile
//!    kept, so every column carries exactly `n_layers` layers) capped by
//!    - **proximity**: [`PROXIMITY_FRACTION`] of the distance to the first
//!      surface triangle the column's ray meets (the far wall of the
//!      pebble-to-channel or pin-to-channel gap, a concave corner), so two
//!      facing walls leave at least a fifth of the gap to the tetrahedra;
//!    - ~~**the border rule**: an inlet point slides at most 0.9 of its
//!      distance to the opposite edge of each inlet triangle at it (cfMesh's
//!      "limit distances")~~ (replaced before the first commit: on the 1 cm
//!      pipe it held 54 rim columns to 54 % of the request). Instead the
//!      **inlet slides out of the way** (step 3b): the points of a planar
//!      non-layer patch within [`SLIDE_RINGS`] edges of a wall point are
//!      Laplace-smoothed in their plane ([`SLIDE_PASSES`] Jacobi passes)
//!      around the moved rim, as Gmsh re-meshes its non-source surfaces
//!      after moving the layer points (`BoundaryLayers.cpp`: "remesh
//!      non-source edges", "mesh non-source surfaces"); an inlet triangle
//!      that still flips thins the column it came from. Corner points (two
//!      non-layer patches) keep `layer_setup`'s cap, half the edge to their
//!      target;
//!    - **a slope limit**: a column is at most its neighbour's thickness
//!      plus the distance between them, so a cap does not leave a step.
//!
//!    Every free direction must also make at least [`MIN_VISIBILITY`] with
//!    each wall face at its point ([`make_directions_visible`]: Gmsh's
//!    angle-weighted normal, then pushed off the faces it misses).
//!    Then, until nothing is wrong (at most [`MAX_THINNING_PASSES`]), the
//!    columns of any prism with a non-positive face pyramid, any inlet
//!    triangle that flips, and any triangle of the layers' top surface that
//!    intersects another (cfMesh's `checkSelfIntersections` port,
//!    [`crate::surface::checks::check_self_intersections`]) are thinned by
//!    [`THINNING_FACTOR`]. A column is never stopped (a stopped column
//!    would leave a quadrilateral side face on the inner region, which
//!    TetGen's triangles cannot match); below [`MIN_FRACTION`] of the
//!    request the route gives up with an error rather than a mesh it
//!    cannot vouch for. Gmsh's extrusion has no such step: its documentation
//!    leaves non-intersecting layers to the user.
//! 4. **The inner region**: the layered shell's first cell (the surface
//!    with every wall point replaced by the top of its column) becomes a
//!    PLC; TetGen meshes it with the pipeline's switches plus `-Y` (keep
//!    the boundary), `-M` (do not merge coplanar facets: a merged facet is
//!    re-triangulated, and its triangles would no longer be the prisms'),
//!    `-J` (keep the input points' numbers). Every boundary triangle of the
//!    tetrahedra is checked to be a top or inlet triangle of the layered
//!    shell, else the route errors.
//! 5. **Join**: the tetrahedra (cells `0..n_tets`) and the prisms after them
//!    ([`LayersFirstMesh::first_layer_cell`]), one boundary patch per
//!    region (named by [`crate::patches::assign_patches_by_region`]).
//!
//! The dual, when the pipeline asks for one, is taken of the joined mesh
//! (prisms and tetrahedra together); see the pipeline docs.
//!
//! **Untrusted AI-assisted draft pending human V&V.** Education, research
//! and V&V only, per the workspace `RESPONSIBLE_USE.md`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use alloc::collections::{BTreeMap, BTreeSet};
use petir::mathf::RealMath;

use crate::layers::{assemble_layered, layer_columns, layer_setup, LayerOutcome, LayerQualityStrategy, LayerSetup, LayerVertex};
use crate::math::Vec3;
use crate::mesh_octree::{help, BoundBox, TriSurf};
use crate::patches::{assign_patches_by_region, SurfaceRegions};
use crate::tetgen::{tetrahedralize_plc, Switches};
use crate::volume_mesh::{cells_faces, from_cell_faces_wound, VolumeMesh};

use super::tet_source::{tetgen_plc, tetgen_tets_graded, TetSource};
use super::TetDualOptions;

/// The share of the distance to the facing surface a column may take
/// (step 3): two facing walls then leave at least `1 - 2 × 0.4 = 0.2` of
/// the gap to the tetrahedra.
pub const PROXIMITY_FRACTION: f64 = 0.4;

/// The factor a column is thinned by in each pass that finds it wrong.
pub const THINNING_FACTOR: f64 = 0.7;

/// The most thinning passes before the route gives up.
pub const MAX_THINNING_PASSES: usize = 40;

/// The thinnest column allowed, as a fraction of the requested total.
pub const MIN_FRACTION: f64 = 0.01;

/// The layers-first mesh and what it took.
#[derive(Debug, Clone)]
pub struct LayersFirstMesh {
    /// Tetrahedra (cells `0..first_layer_cell`), then the prisms, layer by
    /// layer within each wall face's stack; boundary named by region (or one
    /// patch named after `wall_patch`).
    pub mesh: VolumeMesh,
    /// The label of the first prism (the number of tetrahedra).
    pub first_layer_cell: usize,
    /// The patches the layers grew on.
    pub layer_patches: Vec<String>,
    /// TetGen's mesh of the input surface (step 1), whose boundary
    /// triangulation the layers grow from.
    pub surface_mesh: VolumeMesh,
    /// Per point of [`Self::mesh`]: `j + 1` for the `j`-th point of a column
    /// counted from the wall (`j < n_layers`), `0` otherwise (a column's top
    /// point, a point of the inner region). A dual cell built around point
    /// `p` lies in layer `point_layer[p]`.
    pub point_layer: Vec<u32>,
    /// Per wall point and face: layers and thickness (strategy
    /// [`LayerQualityStrategy::LayersFirst`]).
    pub outcome: LayerOutcome,
    /// One line per step for the report.
    pub notes: Vec<String>,
}

/// The boundary of `m` as a one-cell mesh (the "shell"): only the boundary
/// points, renumbered in ascending order, every boundary face kept with its
/// patch. The layer columns are grown on it as on any mesh.
pub(crate) fn boundary_shell(m: &VolumeMesh) -> VolumeMesh {
    let n_int = m.n_internal_faces();
    let mut used: BTreeSet<usize> = BTreeSet::new();
    for f in n_int..m.face_count() {
        used.extend(m.faces[f].iter().copied());
    }
    let mut new_of = alloc::vec![usize::MAX; m.points.len()];
    let mut points = Vec::with_capacity(used.len());
    for &p in &used {
        new_of[p] = points.len();
        points.push(m.points[p]);
    }
    let faces: Vec<Vec<usize>> = (n_int..m.face_count()).map(|f| m.faces[f].iter().map(|&p| new_of[p]).collect()).collect();
    let n = faces.len();
    let patches = m
        .patches
        .iter()
        .map(|p| crate::volume_mesh::BoundaryPatch { name: p.name.clone(), start_face: p.start_face - n_int, n_faces: p.n_faces })
        .collect();
    VolumeMesh { points, faces, owner: alloc::vec![0; n], neighbour: alloc::vec![None; n], n_cells: 1, patches, cells: None }
}

/// The distance from `x` along the unit direction `dir` to the first
/// triangle of `surf` the ray meets within `reach`, skipping triangles at
/// the point `skip` and triangles the ray runs along (`|n̂·dir| < 1e-3`: an
/// inlet point's ray lies in the inlet plane). `None` when nothing is met.
fn ray_distance(surf: &TriSurf, octree: &crate::mesh_octree::MeshOctree, skip: usize, x: Vec3, dir: Vec3, reach: f64) -> Option<f64> {
    let e = x.add(dir.scale(reach));
    let lo = Vec3::new(x.x.min(e.x), x.y.min(e.y), x.z.min(e.z));
    let hi = Vec3::new(x.x.max(e.x), x.y.max(e.y), x.z.max(e.z));
    let mut best: Option<f64> = None;
    let mut seen: BTreeSet<usize> = BTreeSet::new();
    for l in octree.find_leaves_contained_in_box(&BoundBox::new(lo, hi)) {
        for t in octree.contained_triangles(l) {
            if !seen.insert(t) {
                continue;
            }
            let tri = surf.triangles[t];
            if tri.contains(&skip) {
                continue;
            }
            let (a, b, c) = (surf.points[tri[0]], surf.points[tri[1]], surf.points[tri[2]]);
            let n = b.sub(a).cross(c.sub(a)).normalize();
            if n.dot(dir).abs() < 1e-3 {
                continue;
            }
            if let Some(h) = help::tri_line_intersection(a, b, c, x, e) {
                let d = h.sub(x).length();
                if d > 0.0 && best.map_or(true, |b| d < b) {
                    best = Some(d);
                }
            }
        }
    }
    best
}

/// The least cosine a column's direction may make with the inward normal of
/// any wall face at its point ([`make_directions_visible`]).
pub const MIN_VISIBILITY: f64 = 0.1;

/// Gmsh's vertex normal (`BoundaryLayers.cpp`, `addExtrudeNormals`, the
/// Gouraud branch): the sum of the faces' unit normals, each weighted by
/// the face's angle at the vertex × 2/π, normalised. Here inward.
fn angle_weighted_inward(shell: &VolumeMesh, faces: &[usize], p: usize) -> Vec3 {
    let mut s = Vec3::ZERO;
    for &f in faces {
        let ring = &shell.faces[f];
        let k = ring.len();
        let Some(pos) = ring.iter().position(|&q| q == p) else { continue };
        let x = shell.points[p];
        let a = shell.points[ring[(pos + k - 1) % k]].sub(x).normalize();
        let b = shell.points[ring[(pos + 1) % k]].sub(x).normalize();
        let angle = a.dot(b).clamp(-1.0, 1.0).r_acos();
        let n = crate::surface_engine::ring_area_vector(&shell.points, ring).normalize();
        s = s.sub(n.scale(angle * 2.0 / core::f64::consts::PI));
    }
    s.normalize()
}

/// Turn every free (`Patch`) column whose direction makes less than
/// [`MIN_VISIBILITY`] with the inward normal of a wall face at its point:
/// first to Gmsh's angle-weighted normal, then, while a face still misses,
/// pushed along that face's normal. The smoothing in `layer_setup`
/// averages over neighbours, and at a sharp edge whose two sides are
/// triangulated at different densities it can tip a direction past 90° from
/// the coarser side (found on a 1 × 1 × 0.1 box: −0.013 against the top face,
/// a prism inverted at every thickness). Returns how many were turned.
fn make_directions_visible(shell: &VolumeMesh, s: &mut LayerSetup, engine: &crate::surface_engine::SurfaceEngine, is_wall_face: &[bool]) -> usize {
    let mut turned = 0usize;
    for &p in &s.wall_pts {
        if s.kind[&p] != LayerVertex::Patch {
            continue;
        }
        let b = engine.bp[p];
        let faces: Vec<usize> = engine.point_faces[b].iter().map(|&bf| engine.boundary_faces[bf]).filter(|&f| is_wall_face[f]).collect();
        let normals: Vec<Vec3> = faces.iter().map(|&f| crate::surface_engine::ring_area_vector(&shell.points, &shell.faces[f]).normalize().scale(-1.0)).collect();
        let worst = |d: Vec3| normals.iter().map(|n| d.dot(*n)).fold(f64::INFINITY, f64::min);
        let d0 = s.inward[&p];
        if worst(d0) >= MIN_VISIBILITY {
            continue;
        }
        let mut d = angle_weighted_inward(shell, &faces, p);
        for _ in 0..50 {
            let Some(n) = normals.iter().copied().min_by(|a, b| d.dot(*a).total_cmp(&d.dot(*b))) else { break };
            let w = d.dot(n);
            if w >= MIN_VISIBILITY {
                break;
            }
            d = d.add(n.scale(MIN_VISIBILITY - w + 0.05)).normalize();
        }
        s.inward.insert(p, d);
        turned += 1;
    }
    turned
}

/// Is the closed cell with these outward rings valid: every face-fan
/// pyramid about the cell's vertex average has positive volume
/// (OpenFOAM's `minTetQuality > 0`, as the pipeline's gate counts it).
fn cell_is_valid(points: &[Vec3], rings: &[Vec<usize>]) -> bool {
    let mut c = Vec3::ZERO;
    let mut k = 0usize;
    for r in rings {
        for &p in r {
            c = c.add(points[p]);
            k += 1;
        }
    }
    let c = c.scale(1.0 / k as f64);
    rings.iter().all(|r| {
        let mut fc = Vec3::ZERO;
        for &p in r {
            fc = fc.add(points[p]);
        }
        let fc = fc.scale(1.0 / r.len() as f64);
        (0..r.len()).all(|i| {
            let (a, b) = (points[r[i]], points[r[(i + 1) % r.len()]]);
            let n = a.sub(fc).cross(b.sub(fc));
            fc.sub(c).dot(n) > 0.0
        })
    })
}

/// The outward rings of layer `i` of the prism stack on wall ring `w`
/// (wound out of the domain), from the columns: the bottom as `w`, the top
/// reversed, one quadrilateral per edge.
fn prism_rings(w: &[usize], column: &BTreeMap<usize, Vec<usize>>, i: usize) -> Vec<Vec<usize>> {
    let at = |p: usize, j: usize| column[&p][j];
    let k = w.len();
    let mut rings = Vec::with_capacity(k + 2);
    rings.push(w.iter().map(|&p| at(p, i)).collect());
    rings.push(w.iter().rev().map(|&p| at(p, i + 1)).collect());
    for e in 0..k {
        let (p, q) = (w[e], w[(e + 1) % k]);
        rings.push(alloc::vec![at(q, i), at(p, i), at(p, i + 1), at(q, i + 1)]);
    }
    rings
}

/// The point a shell point stands at in the layered point list: its
/// column's top for a wall point, itself otherwise.
fn top_of(shell_points: usize, s: &LayerSetup, column: &BTreeMap<usize, Vec<usize>>) -> Vec<usize> {
    let mut new_of = alloc::vec![usize::MAX; shell_points];
    let mut next = 0usize;
    for (op, slot) in new_of.iter_mut().enumerate() {
        if !s.wall_pts.contains(&op) {
            *slot = next;
            next += 1;
        }
    }
    for &wp in &s.wall_pts {
        new_of[wp] = *column[&wp].last().unwrap();
    }
    new_of
}

/// How many edges from a wall point a non-layer patch's points may slide
/// (step 3b).
pub const SLIDE_RINGS: usize = 3;

/// Jacobi passes of the in-plane smoothing of step 3b.
pub const SLIDE_PASSES: usize = 20;

/// The points of the non-layer patches that slide in their patch's plane to
/// make room for the layers' side faces (step 3b; Gmsh instead re-meshes
/// the non-source surfaces after moving the layer points,
/// `BoundaryLayers.cpp`: "remesh non-source edges", "mesh non-source
/// surfaces"). A point slides when it is not a wall point, every boundary
/// face at it lies in one non-layer patch and in one plane, and it is
/// within [`SLIDE_RINGS`] edges of a wall point; it carries the unit normal
/// of that plane and the wall point the search reached it from (whose column
/// is thinned when a face it moves flips).
pub(crate) struct Sliding {
    /// Shell point -> (plane normal, source wall point).
    pub points: BTreeMap<usize, (Vec3, usize)>,
    /// Neighbours of every shell point along the shell's edges.
    pub nbrs: Vec<BTreeSet<usize>>,
}

fn sliding_points(shell: &VolumeMesh, s: &LayerSetup, engine: &crate::surface_engine::SurfaceEngine, is_wall_face: &[bool]) -> Sliding {
    let np = shell.points.len();
    let mut nbrs: Vec<BTreeSet<usize>> = alloc::vec![BTreeSet::new(); np];
    for r in &shell.faces {
        for i in 0..r.len() {
            let (a, b) = (r[i], r[(i + 1) % r.len()]);
            nbrs[a].insert(b);
            nbrs[b].insert(a);
        }
    }
    let mut patch_of = alloc::vec![usize::MAX; shell.face_count()];
    for (pi, q) in shell.patches.iter().enumerate() {
        for f in q.start_face..q.start_face + q.n_faces {
            patch_of[f] = pi;
        }
    }
    let plane = |p: usize| -> Option<Vec3> {
        let b = engine.bp[p];
        let faces: Vec<usize> = engine.point_faces[b].iter().map(|&bf| engine.boundary_faces[bf]).collect();
        let f0 = *faces.first()?;
        let n0 = crate::surface_engine::ring_area_vector(&shell.points, &shell.faces[f0]).normalize();
        let ok = faces.iter().all(|&f| {
            !is_wall_face[f] && patch_of[f] == patch_of[f0] && crate::surface_engine::ring_area_vector(&shell.points, &shell.faces[f]).normalize().dot(n0) > 1.0 - 1e-12
        });
        ok.then_some(n0)
    };
    let mut points: BTreeMap<usize, (Vec3, usize)> = BTreeMap::new();
    let mut ring: Vec<(usize, usize)> = s.wall_pts.iter().map(|&w| (w, w)).collect();
    let mut seen: BTreeSet<usize> = s.wall_pts.clone();
    for _ in 0..SLIDE_RINGS {
        let mut next = Vec::new();
        for (p, src) in ring {
            for &q in &nbrs[p] {
                if seen.insert(q) {
                    if let Some(n) = plane(q) {
                        points.insert(q, (n, src));
                        next.push((q, src));
                    }
                }
            }
        }
        ring = next;
    }
    Sliding { points, nbrs }
}

/// Step 3b: smooth the sliding points in their planes (Jacobi Laplacian,
/// [`SLIDE_PASSES`] passes), every other point held where the columns put
/// it. `place[p]` is shell point `p`'s index in `pts`.
fn slide(shell: &VolumeMesh, sliding: &Sliding, place: &[usize], pts: &mut [Vec3]) {
    for _ in 0..SLIDE_PASSES {
        let moved: Vec<(usize, Vec3)> = sliding
            .points
            .iter()
            .map(|(&p, &(n, _))| {
                let ns = &sliding.nbrs[p];
                let mut avg = Vec3::ZERO;
                for &q in ns {
                    avg = avg.add(pts[place[q]]);
                }
                let avg = avg.scale(1.0 / ns.len() as f64);
                let x0 = shell.points[p];
                (place[p], avg.sub(n.scale(avg.sub(x0).dot(n))))
            })
            .collect();
        for (i, y) in moved {
            pts[i] = y;
        }
    }
}

/// The wall points whose columns break something (step 3), and how many
/// prisms, inlet triangles and intersecting top triangles that was.
fn find_bad(shell: &VolumeMesh, s: &LayerSetup, points: &[Vec3], column: &BTreeMap<usize, Vec<usize>>, is_wall_face: &[bool], sliding: &Sliding, tol: f64) -> (BTreeSet<usize>, [usize; 3]) {
    let mut bad: BTreeSet<usize> = BTreeSet::new();
    let mut counts = [0usize; 3];
    // (a) prisms.
    for &f in &s.wall_faces {
        let w = &shell.faces[f];
        if (0..s.n_layers).any(|i| !cell_is_valid(points, &prism_rings(w, column, i))) {
            counts[0] += 1;
            bad.extend(w.iter().copied());
        }
    }
    // (b) inlet triangles with a moved (or slid) point must keep their
    //   orientation.
    let top = top_of(shell.points.len(), s, column);
    let moves = |p: &usize| s.wall_pts.contains(p) || sliding.points.contains_key(p);
    let mut tris: Vec<[usize; 3]> = Vec::new();
    let mut owner_pts: Vec<[usize; 3]> = Vec::new();
    for (f, ring) in shell.faces.iter().enumerate() {
        let moved: Vec<usize> = ring.iter().map(|&p| top[p]).collect();
        if !is_wall_face[f] && ring.iter().any(moves) {
            let a0 = crate::surface_engine::ring_area_vector(&shell.points, ring);
            let a1 = crate::surface_engine::ring_area_vector(points, &moved);
            if a1.dot(a0) <= 0.1 * a0.length() * a1.length() {
                counts[1] += 1;
                for p in ring {
                    if s.wall_pts.contains(p) {
                        bad.insert(*p);
                    } else if let Some(&(_, src)) = sliding.points.get(p) {
                        bad.insert(src);
                    }
                }
            }
        }
        if moved.len() == 3 {
            tris.push([moved[0], moved[1], moved[2]]);
            owner_pts.push([ring[0], ring[1], ring[2]]);
        }
    }
    // (c) the inner region's surface must not intersect itself.
    if bad.is_empty() {
        let surf = TriSurf::new(points.to_vec(), tris, None);
        let r = crate::surface::checks::check_self_intersections(&surf, tol);
        counts[2] = r.facets.len();
        for t in r.facets {
            bad.extend(owner_pts[t].iter().copied().filter(|p| s.wall_pts.contains(p)));
        }
    }
    (bad, counts)
}

/// Hold every column to at most its neighbour's thickness plus the
/// distance between them (step 3's slope limit), to a fixed point.
fn limit_slope(totals: &mut BTreeMap<usize, f64>, nbrs: &BTreeMap<usize, BTreeSet<usize>>, x: &[Vec3]) {
    for _ in 0..totals.len().max(1) {
        let mut changed = false;
        for (&p, ns) in nbrs {
            let mut t = totals[&p];
            for &q in ns {
                let lim = totals[&q] + x[p].sub(x[q]).length();
                if lim < t {
                    t = lim;
                }
            }
            if t < totals[&p] {
                totals.insert(p, t);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

/// Grow `opts.n_layers` prism layers from the walls of the closed surface
/// `points` / `tris` (the regions in `opts.layer_patches`, or the whole
/// surface), then mesh the inner region with TetGen (see the module docs).
///
/// # Errors
/// When the tet source is not TetGen (the inner region needs a mesher that
/// keeps its boundary; the carved route cannot), when TetGen refuses a
/// surface, when no layer patch is found, when a border point meets three
/// or more non-layer patches (cfMesh refuses it too), when a column would
/// have to be thinner than [`MIN_FRACTION`] of the request, or when
/// TetGen's tetrahedra do not meet the prisms face to face.
pub fn grow_layers_first(points: &[Vec3], tris: &[[usize; 3]], regions: Option<&SurfaceRegions>, opts: &TetDualOptions) -> Result<LayersFirstMesh, String> {
    if !matches!(opts.tet_source, TetSource::TetGen { .. }) {
        return Err("boundary layers are grown first and need TetGen for the inner region (TetDualOptions::tet_source = TetSource::TetGen); \
                    the carved tet source cannot keep the layers' top surface (gh:#1829)"
            .into());
    }
    let (n, first, ratio) = (opts.n_layers, opts.first_layer_thickness, opts.expansion.max(1.0));
    if n == 0 || !(first > 0.0 && first.is_finite()) {
        return Err(alloc::format!("layers-first needs n_layers > 0 and a positive first thickness (got {n}, {first})"));
    }
    let mut notes: Vec<String> = Vec::new();

    // Step 1: TetGen's wall triangulation.
    let (m0, note0) = tetgen_tets_graded(points, tris, regions, &opts.wall_patch, &opts.layer_patches, opts.tet_source)?;
    notes.push(alloc::format!("layers-first step 1, the wall triangulation: {note0}"));
    let shell = boundary_shell(&m0);

    // Step 2: the columns.
    let layer_names: Vec<String> = match regions {
        Some(_) if !opts.layer_patches.is_empty() => opts.layer_patches.clone(),
        _ => shell.patches.iter().map(|p| p.name.clone()).collect(),
    };
    let names: Vec<&str> = layer_names.iter().map(String::as_str).collect();
    let mut s = layer_setup(&shell, &names, n, first, ratio).ok_or_else(|| {
        alloc::format!("layers-first: no layer can be grown on {layer_names:?} (no such patch, or a border point meets three or more non-layer patches)")
    })?;
    let total_req = s.d[n];
    let mut is_wall_face = alloc::vec![false; shell.face_count()];
    for &f in &s.wall_faces {
        is_wall_face[f] = true;
    }

    // Step 2b: every column must see all its wall faces.
    let engine = crate::surface_engine::SurfaceEngine::new(&shell);
    let n_turned = make_directions_visible(&shell, &mut s, &engine, &is_wall_face);
    if n_turned > 0 {
        notes.push(alloc::format!(
            "layers-first: {n_turned} wall points' smoothed directions turned to see every wall face at them (Gmsh's angle-weighted normal, then pushed off the faces they missed)"
        ));
    }

    // Step 3: the caps.
    let shell_tris: Vec<[usize; 3]> = shell.faces.iter().filter(|r| r.len() == 3).map(|r| [r[0], r[1], r[2]]).collect();
    if shell_tris.len() != shell.faces.len() {
        return Err("layers-first: TetGen's boundary has a face that is not a triangle".into());
    }
    let surf = TriSurf::new(shell.points.clone(), shell_tris, None);
    let octree = crate::surface::checks::search_octree(&surf, 20, 30);
    let mut nbrs: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for &f in &s.wall_faces {
        let w = &shell.faces[f];
        for i in 0..w.len() {
            let (a, b) = (w[i], w[(i + 1) % w.len()]);
            nbrs.entry(a).or_default().insert(b);
            nbrs.entry(b).or_default().insert(a);
        }
    }
    let mut cap0: BTreeMap<usize, f64> = BTreeMap::new();
    let (mut n_prox, mut n_border) = (0usize, 0usize);
    for &wp in &s.wall_pts {
        let x = shell.points[wp];
        let mut cap = total_req;
        if let Some(d) = ray_distance(&surf, &octree, wp, x, s.inward[&wp], 3.0 * total_req / PROXIMITY_FRACTION) {
            if PROXIMITY_FRACTION * d < cap {
                cap = PROXIMITY_FRACTION * d;
                n_prox += 1;
            }
        }
        match s.kind[&wp] {
            LayerVertex::Patch => {}
            // ~~Edge: cfMesh's "limit distances", 0.9 x the distance to the
            // opposite edge of each non-layer face here~~ (2026-11-26: the
            // non-layer patch's points slide out of the way instead, step
            // 3b, and a face that still flips thins the column).
            LayerVertex::Edge => {}
            LayerVertex::Corner => {
                // `layer_setup`'s cap: half the edge to the corner's target.
                if s.cap[&wp] < cap {
                    cap = s.cap[&wp];
                    n_border += 1;
                }
            }
        }
        cap0.insert(wp, cap);
    }
    let mut totals = cap0.clone();
    limit_slope(&mut totals, &nbrs, &shell.points);
    let counts: BTreeMap<usize, usize> = s.wall_pts.iter().map(|&p| (p, n)).collect();
    let tol = 1e-9 * total_req;
    let sliding = sliding_points(&shell, &s, &engine, &is_wall_face);
    let mut passes = 0usize;
    let mut thinned_for_validity: BTreeSet<usize> = BTreeSet::new();
    let (mut pts, mut column) = layer_columns(&shell, &s.wall_pts, &s.inward, &s.d, &totals, &counts);
    let place = top_of(shell.points.len(), &s, &column);
    slide(&shell, &sliding, &place, &mut pts);
    loop {
        let (bad, why) = find_bad(&shell, &s, &pts, &column, &is_wall_face, &sliding, tol);
        if bad.is_empty() {
            break;
        }
        passes += 1;
        if passes > MAX_THINNING_PASSES {
            return Err(alloc::format!(
                "layers-first: the layers still break {} prisms, {} inlet triangles and {} top triangles after {MAX_THINNING_PASSES} thinning passes",
                why[0], why[1], why[2]
            ));
        }
        for p in bad {
            let t = totals[&p] * THINNING_FACTOR;
            if t < MIN_FRACTION * total_req {
                return Err(alloc::format!(
                    "layers-first: a column at {:?} would have to be thinner than {} % of the requested {total_req:.4e} m ({} prisms, {} inlet triangles, {} top triangles wrong)",
                    shell.points[p],
                    100.0 * MIN_FRACTION,
                    why[0],
                    why[1],
                    why[2]
                ));
            }
            totals.insert(p, t);
            thinned_for_validity.insert(p);
        }
        limit_slope(&mut totals, &nbrs, &shell.points);
        (pts, column) = layer_columns(&shell, &s.wall_pts, &s.inward, &s.d, &totals, &counts);
        slide(&shell, &sliding, &place, &mut pts);
    }
    let n_full = s.wall_pts.iter().filter(|p| totals[p] >= total_req * (1.0 - 1e-12)).count();
    let min_frac = s.wall_pts.iter().map(|p| totals[p] / total_req).fold(f64::INFINITY, f64::min);
    notes.push(alloc::format!(
        "layers-first: {n} layers on {} wall points ({} faces), requested total {total_req:.4e} m; {n_full} points at the full thickness, \
         thinnest column {:.1} % of the request; capped by proximity at {n_prox}, by the corner rule at {n_border}, thinned in {passes} passes at {} \
         (wrong prism, flipped inlet triangle or intersecting top surface); {} points of the non-layer patches slid in their planes",
        s.wall_pts.len(),
        s.wall_faces.len(),
        100.0 * min_frac,
        thinned_for_validity.len(),
        sliding.points.len()
    ));

    // Step 4: the inner region, meshed by TetGen with -Y -M -J.
    let layered = assemble_layered(&shell, &s.wall_faces, &s.wall_pts, pts, &column, &counts);
    let mut cf = cells_faces(&layered);
    let inner = core::mem::take(&mut cf[0]);
    let mut inner_of = alloc::vec![usize::MAX; layered.points.len()];
    let mut inner_pts: Vec<Vec3> = Vec::new();
    let mut inner_tris: Vec<[usize; 3]> = Vec::with_capacity(inner.len());
    for ring in &inner {
        if ring.len() != 3 {
            return Err(alloc::format!("layers-first: the inner region has a {}-sided face", ring.len()));
        }
        let mut t = [0usize; 3];
        for (k, &p) in ring.iter().enumerate() {
            if inner_of[p] == usize::MAX {
                inner_of[p] = inner_pts.len();
                inner_pts.push(layered.points[p]);
            }
            t[k] = inner_of[p];
        }
        inner_tris.push(t);
    }
    let mut assembled_of = alloc::vec![usize::MAX; inner_pts.len()];
    for (p, &i) in inner_of.iter().enumerate() {
        if i != usize::MAX {
            assembled_of[i] = p;
        }
    }
    let run = tetgen_plc(&inner_pts, &inner_tris, None, "inner", &[], opts.tet_source)?;
    let behavior = Switches { b: run.behavior }.no_bisect().no_merge_facets().no_jettison().behavior();
    let r = tetrahedralize_plc(&run.plc, &behavior).map_err(|e| alloc::format!("layers-first: TetGen refused the inner region: {e:?}"))?;
    let out = r.output.ok_or("layers-first: TetGen wrote no output for the inner region")?;
    let first_index = out.firstindex;
    let n_in = inner_pts.len();
    for (i, p) in inner_pts.iter().enumerate() {
        let q = out.points.get(i).ok_or("layers-first: TetGen dropped an input point")?;
        if q[0].to_bits() != p.x.to_bits() || q[1].to_bits() != p.y.to_bits() || q[2].to_bits() != p.z.to_bits() {
            return Err("layers-first: TetGen moved or renumbered an input point of the inner region".into());
        }
    }
    // Conformity: the tetrahedra's boundary is exactly the inner region.
    let key = |mut k: [usize; 3]| {
        k.sort_unstable();
        k
    };
    let mut once: BTreeMap<[usize; 3], usize> = BTreeMap::new();
    let idx = |i: i32| (i - first_index) as usize;
    for t in &out.tets {
        let [a, b, c, d] = [idx(t[0]), idx(t[1]), idx(t[2]), idx(t[3])];
        for f in [[a, c, b], [a, b, d], [b, c, d], [c, a, d]] {
            *once.entry(key(f)).or_insert(0) += 1;
        }
    }
    let tet_boundary: BTreeSet<[usize; 3]> = once.into_iter().filter(|x| x.1 == 1).map(|x| x.0).collect();
    let plc_tris: BTreeSet<[usize; 3]> = inner_tris.iter().map(|&t| key(t)).collect();
    if tet_boundary != plc_tris {
        let missing = plc_tris.difference(&tet_boundary).count();
        let extra = tet_boundary.difference(&plc_tris).count();
        return Err(alloc::format!(
            "layers-first: TetGen's tetrahedra do not meet the prisms face to face ({missing} inner-region triangles missing, {extra} other boundary faces)"
        ));
    }
    let n_steiner = out.points.len() - n_in;
    notes.push(alloc::format!(
        "layers-first step 4, the inner region: TetGen -{}YMJ on {} triangles: {} tetrahedra, {} points added inside, none on the boundary",
        opts.tet_source.switches().unwrap_or_default(),
        inner_tris.len(),
        out.tets.len(),
        n_steiner
    ));

    // Step 5: join.
    let mut all_points = layered.points.clone();
    let mut tet_point = alloc::vec![usize::MAX; out.points.len()];
    for (i, slot) in tet_point.iter_mut().enumerate() {
        if i < n_in {
            *slot = assembled_of[i];
        } else {
            *slot = all_points.len();
            let q = out.points[i];
            all_points.push(Vec3::new(q[0], q[1], q[2]));
        }
    }
    let mut cells: Vec<Vec<Vec<usize>>> = Vec::with_capacity(out.tets.len() + cf.len() - 1);
    for t in &out.tets {
        let [a, b, c, d] = [tet_point[idx(t[0])], tet_point[idx(t[1])], tet_point[idx(t[2])], tet_point[idx(t[3])]];
        cells.push(alloc::vec![alloc::vec![a, c, b], alloc::vec![a, b, d], alloc::vec![b, c, d], alloc::vec![c, a, d]]);
    }
    let first_layer_cell = cells.len();
    cells.extend(cf.into_iter().skip(1));
    let mut mesh = from_cell_faces_wound(all_points, &cells);
    mesh = match regions {
        Some(r) => assign_patches_by_region(&mesh, points, tris, r)?,
        None => {
            for p in &mut mesh.patches {
                p.name = opts.wall_patch.clone();
            }
            mesh
        }
    };
    let mut point_layer = alloc::vec![0u32; mesh.points.len()];
    for col in column.values() {
        for (j, &p) in col.iter().enumerate().take(n) {
            point_layer[p] = j as u32 + 1;
        }
    }
    let outcome = LayerOutcome {
        strategy: LayerQualityStrategy::LayersFirst,
        n_layers_requested: n,
        points: s.wall_pts.iter().map(|&p| (shell.points[p], n, totals[&p], cap0[&p])).collect(),
        faces: s.wall_faces.iter().map(|&f| (shell.face_centroid(f), n)).collect(),
        iterations: passes,
        remaining_bad_faces: 0,
        notes: notes.clone(),
    };
    Ok(LayersFirstMesh { mesh, first_layer_cell, layer_patches: layer_names, surface_mesh: m0, point_layer, outcome, notes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::check_quality;
    use crate::shapes::box_surface;

    fn tetgen_opts(n: usize, first: f64, ratio: f64, vol: f64) -> TetDualOptions {
        TetDualOptions {
            n_layers: n,
            first_layer_thickness: first,
            expansion: ratio,
            tet_source: TetSource::tetgen(1.414, Some(vol)),
            ..Default::default()
        }
    }

    /// The properties every layers-first mesh must have: closed, no
    /// inverted cell, the volume of TetGen's mesh of the surface, every
    /// prism face shared with exactly the cells it should be (closure
    /// checks that), `n` layers on every wall face, every column's
    /// thickness its recorded total.
    fn check_properties(lf: &LayersFirstMesh, n: usize) {
        lf.mesh.validate().unwrap();
        let q = check_quality(&lf.mesh);
        assert_eq!(q.n_negative_volume_cells, 0);
        let v0 = lf.surface_mesh.total_volume();
        assert!((lf.mesh.total_volume() - v0).abs() < 1e-9 * v0.abs().max(1.0), "{} vs {v0}", lf.mesh.total_volume());
        let n_faces = lf.outcome.faces.len();
        assert_eq!(lf.mesh.n_cells - lf.first_layer_cell, n * n_faces);
        assert!(lf.outcome.faces.iter().all(|f| f.1 == n));
        // Each prism is a six-point cell: two triangles and three quads.
        let cf = cells_faces(&lf.mesh);
        for c in lf.first_layer_cell..lf.mesh.n_cells {
            let mut k: Vec<usize> = cf[c].iter().map(Vec::len).collect();
            k.sort_unstable();
            assert_eq!(k, [3, 3, 4, 4, 4], "cell {c}");
        }
        for c in 0..lf.first_layer_cell {
            assert!(cf[c].iter().all(|r| r.len() == 3) && cf[c].len() == 4, "cell {c}");
        }
    }

    /// **V&V (gh:#1829), a unit box with three layers.** Methodology: the
    /// box `[0, 1]³` (12 triangles, one `walls` patch), TetGen
    /// `-pq1.414a0.01`, 3 layers from 0.01 m at ratio 1.2 (asked total
    /// 0.0364 m). Prediction (written before the first run): every column
    /// at the full thickness except near the 12 edges and 8 corners, where
    /// the averaged normal runs toward the neighbouring face (proximity cap
    /// 0.4 × the distance to it: at a corner the ray along (1,1,1)/√3
    /// meets nothing before the far faces, so the corners themselves may
    /// keep the full length); no inverted cell; volume exactly 1.
    /// Results are asserted by property (`check_properties`), not pinned.
    #[test]
    fn a_box_grows_three_full_layers_and_meshes_the_inside() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let opts = tetgen_opts(3, 0.01, 1.2, 0.01);
        let lf = grow_layers_first(&p, &t, None, &opts).unwrap();
        check_properties(&lf, 3);
        assert!((lf.mesh.total_volume() - 1.0).abs() < 1e-12);
        assert_eq!(lf.mesh.patches.len(), 1);
        assert_eq!(lf.mesh.patches[0].name, "walls");
        let asked = 0.01 + 0.012 + 0.0144;
        let full = lf.outcome.points.iter().filter(|x| (x.2 - asked).abs() < 1e-12).count();
        assert!(full * 2 > lf.outcome.points.len(), "{full} of {}", lf.outcome.points.len());
        // The layer index: the prism points' layers run 1..=3.
        assert_eq!(*lf.point_layer.iter().max().unwrap(), 3);
    }

    /// The carved tet source cannot carry layers-first: an error, never a
    /// fall-back.
    #[test]
    fn the_carved_source_is_refused() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let opts = TetDualOptions { n_layers: 3, tet_source: TetSource::CarvedCentroid, ..Default::default() };
        let e = grow_layers_first(&p, &t, None, &opts).unwrap_err();
        assert!(e.contains("TetGen"), "{e}");
    }

    /// **Proximity (gh:#1829).** Methodology: a 1 m box with a 0.1 m gap
    /// between two of its walls is not available as a primitive, so the
    /// box is thinned instead: `[0, 1] × [0, 1] × [0, 0.1]`, layers asked
    /// to total 0.0728 m (4 from 0.0128 m at 1.25) from both 0.1 m-apart
    /// faces. Prediction: the columns of the two large faces are capped at
    /// 0.4 × 0.1 = 0.04 m (55 % of the request), so the two stacks leave
    /// 0.02 m between them; no inverted cell.
    #[test]
    fn close_walls_thin_the_layers_to_a_share_of_the_gap() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 0.1));
        let opts = tetgen_opts(4, 0.0128, 1.25, 2e-4);
        let lf = grow_layers_first(&p, &t, None, &opts).unwrap();
        check_properties(&lf, 4);
        let asked = (0..4).map(|k| 0.0128 * 1.25f64.powi(k)).sum::<f64>();
        let mid: Vec<&(Vec3, usize, f64, f64)> =
            lf.outcome.points.iter().filter(|x| x.0.x > 0.2 && x.0.x < 0.8 && x.0.y > 0.2 && x.0.y < 0.8).collect();
        assert!(!mid.is_empty());
        for x in mid {
            // The smoothed direction is not exactly the face normal, so the
            //   ray to the far face is a little longer than 0.1 m (measured
            //   2026-11-26: the most was 0.0402 m, 0.5 % over 0.04).
            assert!(x.2 <= 1.01 * 0.4 * 0.1 && x.2 < asked, "{x:?}");
            assert!(x.2 > 0.5 * 0.4 * 0.1, "{x:?}");
        }
    }

    /// Helpers on their own: the shell keeps the boundary, the slope limit
    /// holds, a prism's rings make a valid cell and a flipped one does not.
    #[test]
    fn the_helpers_do_what_they_say() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let (m, _) = super::super::tet_source::tetgen_tets(&p, &t, None, "walls", TetSource::tetgen(1.414, Some(0.05))).unwrap();
        let shell = boundary_shell(&m);
        assert_eq!(shell.n_cells, 1);
        assert_eq!(shell.face_count(), m.n_boundary_faces());
        assert!((shell.total_volume() - 1.0).abs() < 1e-12);
        let x = alloc::vec![Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0)];
        let mut nb: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
        nb.entry(0).or_default().insert(1);
        nb.entry(1).or_default().insert(0);
        let mut tot: BTreeMap<usize, f64> = [(0, 5.0), (1, 0.5)].into_iter().collect();
        limit_slope(&mut tot, &nb, &x);
        assert_eq!(tot[&0], 1.5);
        let pts = alloc::vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 1.0, 1.0),
            Vec3::new(1.0, 0.0, 1.0),
        ];
        let col: BTreeMap<usize, Vec<usize>> = [(0, alloc::vec![0, 3]), (1, alloc::vec![1, 4]), (2, alloc::vec![2, 5])].into_iter().collect();
        assert!(cell_is_valid(&pts, &prism_rings(&[0, 1, 2], &col, 0)));
        let col_flipped: BTreeMap<usize, Vec<usize>> = [(0, alloc::vec![3, 0]), (1, alloc::vec![4, 1]), (2, alloc::vec![5, 2])].into_iter().collect();
        assert!(!cell_is_valid(&pts, &prism_rings(&[0, 1, 2], &col_flipped, 0)));
    }
}
