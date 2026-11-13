// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/anisotropicMeshing/
//     coordinateModification/{coordinateModification,boxScaling,
//       planeScaling,coordinateModifier}.{C,H}
//     polyMeshGenGeometryModification/polyMeshGenGeometryModification.{C,H}
//     surfaceMeshGeometryModification/surfaceMeshGeometryModification.{C,H}
//     edgeMeshGeometryModification/edgeMeshGeometryModification.{C,H}
//   utilities/scaleMesh/scaleMesh.C, utilities/scaleSurfaceMesh/scaleSurfaceMesh.C,
//   utilities/surfaceGenerateBoundingBox/surfaceGenerateBoundingBox.C
// Source: the cfMesh v1.1 line as carried by github.com/DAFoam/cfmesh, commit
// e944f6ba695fcefce73ad306074f6b3c0ea432ae.
// Copyright (C) Creative Fields, Ltd. Licence: GPL-3.0-or-later (cfMesh files
// say "either version 3 of the License, or (at your option) any later
// version"), conveyed here under GPL-3.0-only.
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

//! cfMesh's **anisotropic meshing** (gh:#1279): `meshDict`'s
//! `anisotropicSources` stretch space before meshing and shrink it back
//! afterwards, so that cubic octree cells become boxes in the real
//! geometry.
//!
//! - [`Modification`] is `coordinateModification` (run-time selected
//!   upstream; an enum here): [`BoxScaling`] and [`PlaneScaling`], each
//!   with `displacement` (forward) and `backwardDisplacement`.
//! - [`CoordinateModifier`] is `coordinateModifier`: the forward
//!   modifiers, and their backward copies, each translated by the summed
//!   forward displacement of its origin and with its extent divided by its
//!   scale (`translateAndModifyObject`). `checkForValidInverse` refuses
//!   combinations that cannot be reverted ([`AnisotropicError`]).
//! - `polyMeshGenGeometryModification` / `surfaceMeshGeometryModification`
//!   ([`modify_points`], [`revert_points`]): every point through the
//!   modifier. Upstream's surface version also moves the surface's feature
//!   edges and subsets with it; a [`crate::mesh_octree::TriSurf`] carries
//!   points only through `points`, so the caller rebuilds it
//!   ([`modify_surface`]).
//! - The utilities: `scaleMesh` / `scaleSurfaceMesh` ([`scale_points`]);
//!   `surfaceGenerateBoundingBox` ([`surface_generate_bounding_box`]).
//!
//! **Upstream defect, harmless:** `boxScaling::boundingPlanes` builds the
//! x-max plane with normal `(1, 0, 1)`; `checkForValidInverse` reads only
//! the even planes' normals and the odd planes' points, so it has no
//! effect. Kept (the plane is not used).
//!
//! ~~**No pipeline calls this module**: wiring `anisotropicSources` into a
//! generator (and `projectSurfaceAfterBackScaling`) is not done here.~~
//! **UPDATED 2026-11-05 (gh:#1279 remainder):** the four generators
//! (`pipeline::{cartesian_mesh, tet_mesh, p_mesh, cartesian_2d_mesh}`)
//! read [`crate::mesh_octree_creator::MeshDict::anisotropic_sources`] as
//! upstream: the octree on the modified surface
//! ([`modified_surface_points`]), the revert at the end of the first
//! `optimiseFinalMesh` and `projectSurfaceAfterBackScaling`
//! ([`revert_and_project`], then `optimiseFinalMesh` again) in 3D, the
//! revert inside `generateBoundaryLayers` in 2D. Record:
//! `verification_and_validation/anisotropic_gh1279/`.

use crate::math::Vec3;

/// OpenFOAM's `SMALL` for `double`.
const SMALL: f64 = 1e-15;
/// OpenFOAM's `VSMALL` for `double`.
const VSMALL: f64 = 1e-300;

/// `boxScaling`: inside an axis-aligned box, space is scaled by `scale`
/// per axis; beyond it, shifted by the box's change of length.
#[derive(Debug, Clone, PartialEq)]
pub struct BoxScaling {
    /// `name`.
    pub name: String,
    /// `centre`.
    pub centre: Vec3,
    /// `lengthX/Y/Z`.
    pub length: Vec3,
    /// `scaleX/Y/Z` (default 1).
    pub scale: Vec3,
}

/// `planeScaling`: between a plane and the plane `scaling_distance` along
/// its normal, space is scaled by `scaling_factor` along the normal.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaneScaling {
    /// `name`.
    pub name: String,
    /// `origin`.
    pub origin: Vec3,
    /// `normal` (made unit by [`PlaneScaling::new`]).
    pub normal: Vec3,
    /// `scalingDistance`.
    pub scaling_distance: f64,
    /// `scalingFactor` (below `SMALL` upstream warns and uses 1).
    pub scaling_factor: f64,
}

impl PlaneScaling {
    /// `planeScaling(name, origin, normal, scalingDistance, scalingFactor)`.
    pub fn new(name: &str, origin: Vec3, normal: Vec3, scaling_distance: f64, scaling_factor: f64) -> Self {
        let n = normal.scale(1.0 / normal.length());
        PlaneScaling { name: name.into(), origin, normal: n, scaling_distance, scaling_factor: if scaling_factor < SMALL { 1.0 } else { scaling_factor } }
    }
}

/// A bounding plane (`plane`): a point and a normal.
type BoundPlane = (Vec3, Vec3);

/// `coordinateModification`, by kind.
#[derive(Debug, Clone, PartialEq)]
pub enum Modification {
    /// `boxScaling`.
    Box(BoxScaling),
    /// `planeScaling`.
    Plane(PlaneScaling),
}

fn comp(v: Vec3, i: usize) -> f64 {
    [v.x, v.y, v.z][i]
}

fn from_comps(c: [f64; 3]) -> Vec3 {
    Vec3::new(c[0], c[1], c[2])
}

impl Modification {
    /// `name()`.
    pub fn name(&self) -> &str {
        match self {
            Modification::Box(b) => &b.name,
            Modification::Plane(p) => &p.name,
        }
    }
    /// `origin()`.
    pub fn origin(&self) -> Vec3 {
        match self {
            Modification::Box(b) => b.centre,
            Modification::Plane(p) => p.origin,
        }
    }
    /// `translateAndModifyObject(disp)`.
    pub fn translate_and_modify(&mut self, disp: Vec3) {
        match self {
            Modification::Box(b) => {
                b.centre = b.centre.add(disp);
                b.length = from_comps([0, 1, 2].map(|i| comp(b.length, i) / comp(b.scale, i)));
            }
            Modification::Plane(p) => {
                p.origin = p.origin.add(disp);
                p.scaling_distance /= p.scaling_factor;
            }
        }
    }
    fn forward_or_backward(&self, p: Vec3, forward: bool) -> Vec3 {
        match self {
            Modification::Box(b) => {
                let p_min = b.centre.sub(b.length.scale(0.5));
                from_comps([0, 1, 2].map(|i| {
                    let (l, s) = (comp(b.length, i), comp(b.scale, i));
                    let disp_vec = if forward { l * ((1.0 / s) - 1.0) } else { l * (s - 1.0) };
                    let t = (comp(p, i) - comp(p_min, i)) / l;
                    t.min(1.0).max(0.0) * disp_vec
                }))
            }
            Modification::Plane(pl) => {
                let dist = p.sub(pl.origin).dot(pl.normal);
                let f = if forward { (1.0 / pl.scaling_factor) - 1.0 } else { pl.scaling_factor - 1.0 };
                let translation = pl.normal.scale(pl.scaling_distance * f);
                let t = dist / pl.scaling_distance;
                translation.scale(t.min(1.0).max(0.0))
            }
        }
    }
    /// `displacement(p)` (forward).
    pub fn displacement(&self, p: Vec3) -> Vec3 {
        self.forward_or_backward(p, true)
    }
    /// `backwardDisplacement(p)`.
    pub fn backward_displacement(&self, p: Vec3) -> Vec3 {
        self.forward_or_backward(p, false)
    }
    /// `combiningPossible()` (both kinds: true).
    pub fn combining_possible(&self) -> bool {
        true
    }
    /// `boundingPlanes`: pairs (low, high) per scaled direction.
    pub fn bounding_planes(&self) -> Vec<BoundPlane> {
        match self {
            Modification::Box(b) => {
                let p_min = b.centre.sub(b.length.scale(0.5));
                let p_max = b.centre.add(b.length.scale(0.5));
                let mut pl = Vec::new();
                if (b.scale.x - 1.0).abs() > VSMALL {
                    pl.push((p_min, Vec3::new(1.0, 0.0, 0.0)));
                    pl.push((p_max, Vec3::new(1.0, 0.0, 1.0))); // upstream as written
                }
                if (b.scale.y - 1.0).abs() > VSMALL {
                    pl.push((p_min, Vec3::new(0.0, 1.0, 0.0)));
                    pl.push((p_max, Vec3::new(0.0, 1.0, 0.0)));
                }
                if (b.scale.z - 1.0).abs() > VSMALL {
                    pl.push((p_min, Vec3::new(0.0, 0.0, 1.0)));
                    pl.push((p_max, Vec3::new(0.0, 0.0, 1.0)));
                }
                pl.into_iter().map(|(p, n)| (p, n.scale(1.0 / n.length()))).collect()
            }
            Modification::Plane(p) => {
                if (p.scaling_factor - 1.0).abs() > VSMALL {
                    vec![(p.origin, p.normal), (p.origin.add(p.normal.scale(p.scaling_distance)), p.normal)]
                } else {
                    Vec::new()
                }
            }
        }
    }
}

/// `coordinateModifier::checkForValidInverse`'s `FatalError`s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnisotropicError {
    /// A modifier that cannot be combined.
    CannotBeCombined(String),
    /// "Bounding planes of the objects … are not parallel."
    NotParallel(String, String),
    /// "Scaling regions of objects … are overlapping each other."
    Overlapping(String, String),
}

/// `coordinateModifier`: the forward and backward modifiers.
#[derive(Debug, Clone, PartialEq)]
pub struct CoordinateModifier {
    /// `modifiers_`.
    pub modifiers: Vec<Modification>,
    /// `backwardModifiers_`.
    pub backward_modifiers: Vec<Modification>,
}

impl CoordinateModifier {
    /// `coordinateModifier(geomModDict)`: the backward copies, then
    /// `checkForValidInverse`.
    pub fn new(modifiers: Vec<Modification>) -> Result<Self, AnisotropicError> {
        let mut backward = modifiers.clone();
        for b in backward.iter_mut() {
            let origin = b.origin();
            let disp = modifiers.iter().fold(Vec3::ZERO, |d, m| d.add(m.displacement(origin)));
            b.translate_and_modify(disp);
        }
        let cm = CoordinateModifier { modifiers, backward_modifiers: backward };
        cm.check_for_valid_inverse()?;
        Ok(cm)
    }

    fn check_for_valid_inverse(&self) -> Result<(), AnisotropicError> {
        if self.modifiers.len() <= 1 {
            return Ok(());
        }
        for m in &self.modifiers {
            if !m.combining_possible() {
                return Err(AnisotropicError::CannotBeCombined(m.name().into()));
            }
        }
        for (i, mi) in self.modifiers.iter().enumerate() {
            let bnd = mi.bounding_planes();
            for mj in &self.modifiers[i + 1..] {
                let other = mj.bounding_planes();
                for a in (0..bnd.len()).step_by(2) {
                    let pl = bnd[a];
                    for b in (0..other.len()).step_by(2) {
                        let opl = other[b];
                        let dn = pl.1.dot(opl.1).abs();
                        if dn > SMALL {
                            if dn < 1.0 - SMALL {
                                return Err(AnisotropicError::NotParallel(mi.name().into(), mj.name().into()));
                            }
                            let t_max = bnd[a + 1].0.sub(pl.0).dot(pl.1);
                            let t0 = other[b].0.sub(pl.0).dot(pl.1);
                            let t1 = other[b + 1].0.sub(pl.0).dot(pl.1);
                            if t1 >= 0.0 && t0 < t_max {
                                return Err(AnisotropicError::Overlapping(mi.name().into(), mj.name().into()));
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// `modifiedPoint(p)`: p plus every forward displacement at p.
    pub fn modified_point(&self, p: Vec3) -> Vec3 {
        self.modifiers.iter().fold(p, |q, m| q.add(m.displacement(p)))
    }

    /// `backwardModifiedPoint(p)`: p plus every backward displacement at p.
    pub fn backward_modified_point(&self, p: Vec3) -> Vec3 {
        self.backward_modifiers.iter().fold(p, |q, m| q.add(m.backward_displacement(p)))
    }
}

/// `polyMeshGenGeometryModification::modifyGeometry` (and the surface's):
/// every point through [`CoordinateModifier::modified_point`].
pub fn modify_points(points: &mut [Vec3], cm: &CoordinateModifier) {
    for p in points.iter_mut() {
        *p = cm.modified_point(*p);
    }
}

/// `revertGeometryModification`: every point through
/// [`CoordinateModifier::backward_modified_point`].
pub fn revert_points(points: &mut [Vec3], cm: &CoordinateModifier) {
    for p in points.iter_mut() {
        *p = cm.backward_modified_point(*p);
    }
}

/// `surfaceMeshGeometryModification::modifyGeometry` on a triangulated
/// surface: the points moved and the surface rebuilt (patches, regions and
/// edges kept).
pub fn modify_surface(surf: &crate::mesh_octree::TriSurf, cm: &CoordinateModifier, forward: bool) -> crate::mesh_octree::TriSurf {
    let mut s = surf.clone();
    if forward {
        modify_points(&mut s.points, cm);
    } else {
        revert_points(&mut s.points, cm);
    }
    s
}

/// The generators' constructors (gh:#1279 remainder): with
/// `anisotropicSources`, `surfaceMeshGeometryModification(surf,
/// meshDict).modifyGeometry()`, the surface's points moved forward, on
/// which the octree is then built; `None` without sources (upstream keeps
/// the original surface, `modSurfacePtr_` null).
pub fn modified_surface_points(points: &[Vec3], sources: Option<&CoordinateModifier>) -> Option<Vec<Vec3>> {
    sources.map(|cm| {
        let mut q = points.to_vec();
        modify_points(&mut q, cm);
        q
    })
}

/// What [`revert_and_project`] did.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BackScalingReport {
    /// Mesh points reverted (`revertGeometryModification`: all of them).
    pub n_points_reverted: usize,
    /// Largest move `mapVerticesOntoSurface` made: how far the reverted
    /// boundary was from the original surface.
    pub max_projection_move: f64,
}

/// The 3D generators' back-scaling (`cartesianMeshGenerator`,
/// `tetMeshGenerator`, `voronoiMeshGenerator`): the end of the first
/// `optimiseFinalMesh`, `polyMeshGenGeometryModification::
/// revertGeometryModification` (every mesh point backward), then the
/// mapping part of `projectSurfaceAfterBackScaling`:
/// `meshSurfaceMapper::mapVerticesOntoSurface`, every boundary point to the
/// nearest point of the **original** surface `(points, tris)`. Upstream
/// searches a new octree on the original surface
/// (`createOctreeWithRefinedBoundary(20, 30)`); here
/// [`crate::snap::snap_to_surface_with`] with [`SurfaceSearch::Octree`]
/// (bit-identical to the brute-force nearest point, gh:#1263). The caller
/// then runs its `optimiseFinalMesh` again on the original surface.
///
/// [`SurfaceSearch::Octree`]: crate::mesh_octree_queries::SurfaceSearch::Octree
pub fn revert_and_project(m: &mut crate::volume_mesh::VolumeMesh, cm: &CoordinateModifier, points: &[Vec3], tris: &[[usize; 3]]) -> BackScalingReport {
    revert_points(&mut m.points, cm);
    crate::pipeline::cartesian_mesh::dump_points("B_after_revert", m);
    let mapped = crate::snap::snap_to_surface_with(m, points, tris, crate::mesh_octree_queries::SurfaceSearch::Octree);
    let max_projection_move = mapped.points.iter().zip(&m.points).map(|(a, b)| a.sub(*b).length()).fold(0.0, f64::max);
    let n_points_reverted = m.points.len();
    *m = mapped;
    crate::pipeline::cartesian_mesh::dump_points("C_after_map", m);
    BackScalingReport { n_points_reverted, max_projection_move }
}

/// [`revert_and_project`] as `cartesianMeshGenerator::projectSurfaceAfterBackScaling`
/// does it (gh:#1385 round 5): `revertGeometryModification`, then
/// `meshSurfaceMapper(mse, octree).mapVerticesOntoSurface()` with a **new
/// octree on the original surface** (`meshOctree(*surfacePtr_)` +
/// `createOctreeWithRefinedBoundary(20, 30)`, built by the caller and passed
/// as `octree`): every boundary point to the octree's `findNearestSurfacePoint`
/// (not an exact nearest point, so not [`crate::snap`]'s), with no
/// untangling. The caller then runs its `optimiseFinalMesh` again with the
/// same octree, as upstream does. [`revert_and_project`] is the earlier
/// route (exact nearest point) that `tetMesh` and `pMesh` still call.
pub fn revert_and_project_octree(m: &mut crate::volume_mesh::VolumeMesh, cm: &CoordinateModifier, octree: &crate::mesh_octree::MeshOctree, points: &[Vec3], tris: &[[usize; 3]], regions: &crate::patches::SurfaceRegions) -> BackScalingReport {
    revert_points(&mut m.points, cm);
    crate::pipeline::cartesian_mesh::dump_points("B_after_revert", m);
    let q = crate::surface_mapper::SurfaceQueries::new(points, tris, regions).with_octree(std::sync::Arc::new(octree.clone()));
    let engine = crate::surface_engine::SurfaceEngine::new(m);
    let before = m.points.clone();
    for &v in &engine.boundary_points {
        m.points[v] = q.nearest(before[v]);
    }
    let max_projection_move = m.points.iter().zip(&before).map(|(a, b)| a.sub(*b).length()).fold(0.0, f64::max);
    crate::pipeline::cartesian_mesh::dump_points("C_after_map", m);
    BackScalingReport { n_points_reverted: m.points.len(), max_projection_move }
}

/// `scaleMesh` / `scaleSurfaceMesh`: every point times `factor`.
pub fn scale_points(points: &mut [Vec3], factor: f64) {
    for p in points.iter_mut() {
        *p = p.scale(factor);
    }
}

/// `surfaceGenerateBoundingBox`' box: the surface's bounding box widened
/// by the given margins on each side (xMin, xMax, yMin, yMax, zMin, zMax),
/// as (min, max); [`surface_generate_bounding_box`] appends its 12
/// triangles to the surface, as upstream.
pub fn surface_bounding_box(points: &[Vec3], margins: [f64; 6]) -> (Vec3, Vec3) {
    let mut lo = Vec3::new(f64::MAX, f64::MAX, f64::MAX);
    let mut hi = Vec3::new(f64::MIN, f64::MIN, f64::MIN);
    for p in points {
        lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    (Vec3::new(lo.x - margins[0], lo.y - margins[2], lo.z - margins[4]), Vec3::new(hi.x + margins[1], hi.y + margins[3], hi.z + margins[5]))
}

/// `surfaceGenerateBoundingBox`: `surf` with the box of
/// [`surface_bounding_box`] appended: 8 points, 12 triangles in upstream's
/// order and winding, in six new patches `xMin`, `xMax`, `yMin`, `yMax`,
/// `zMin`, `zMax`. Existing points, facets, feature edges and subsets keep
/// their labels.
pub fn surface_generate_bounding_box(surf: &crate::mesh_octree::TriSurf, margins: [f64; 6]) -> crate::mesh_octree::TriSurf {
    use crate::mesh_octree::{SurfacePatch, TriSurf};
    let (lo, hi) = surface_bounding_box(&surf.points, margins);
    let n = surf.points.len();
    let mut points = surf.points.clone();
    points.extend([
        lo,
        Vec3::new(hi.x, lo.y, lo.z),
        Vec3::new(lo.x, hi.y, lo.z),
        Vec3::new(hi.x, hi.y, lo.z),
        Vec3::new(lo.x, lo.y, hi.z),
        Vec3::new(hi.x, lo.y, hi.z),
        Vec3::new(lo.x, hi.y, hi.z),
        hi,
    ]);
    let np = surf.patches.len().max(surf.regions.iter().map(|&r| r + 1).max().unwrap_or(0));
    let mut patches = surf.patches.clone();
    patches.resize(np, SurfacePatch::new("patch"));
    for name in ["xMin", "xMax", "yMin", "yMax", "zMin", "zMax"] {
        patches.push(SurfacePatch::new(name));
    }
    let t: [([usize; 3], usize); 12] = [
        ([0, 6, 2], 0),
        ([0, 4, 6], 0),
        ([1, 3, 7], 1),
        ([1, 7, 5], 1),
        ([0, 1, 5], 2),
        ([0, 5, 4], 2),
        ([2, 7, 3], 3),
        ([2, 6, 7], 3),
        ([0, 2, 3], 4),
        ([0, 3, 1], 4),
        ([4, 7, 6], 5),
        ([4, 5, 7], 5),
    ];
    let mut triangles = surf.triangles.clone();
    let mut regions = surf.regions.clone();
    for (tri, p) in t {
        triangles.push([n + tri[0], n + tri[1], n + tri[2]]);
        regions.push(np + p);
    }
    let mut out = TriSurf::from_parts(points, triangles, regions, patches, surf.feature_edges.clone());
    out.point_subsets = surf.point_subsets.clone();
    out.facet_subsets = surf.facet_subsets.clone();
    out.edge_subsets = surf.edge_subsets.clone();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartesian::cartesian_box;

    fn grid(n: usize, lo: f64, hi: f64) -> Vec<Vec3> {
        let mut v = Vec::new();
        for i in 0..=n {
            for j in 0..=n {
                for k in 0..=n {
                    let f = |a: usize| lo + (hi - lo) * a as f64 / n as f64;
                    v.push(Vec3::new(f(i), f(j) * 0.9 + 0.013, f(k) * 1.1 - 0.021));
                }
            }
        }
        v
    }

    /// V&V (gh:#1279), the plan's tests. **Predictions (written 2026-11-05,
    /// before the first run):**
    /// 1. `boxScaling` (centre (0.5, 0.5, 0.5), lengths (0.4, 0.6, 1),
    ///    scales (2, 0.5, 1)) and `planeScaling` (origin (0, 0, 0.2),
    ///    normal z, distance 0.3, factor 3), each alone, and the two
    ///    together (their scaled directions are x/y and z: the planes are
    ///    perpendicular, so `checkForValidInverse` accepts them):
    ///    modify-then-revert returns every point of a 11³ grid over
    ///    [−0.5, 1.5]³ (inside, across and beyond both regions) to within
    ///    1e-12.
    /// 2. A unit box mesh scaled ×2 in x by a `boxScaling` that covers it
    ///    (scale 0.5 in x: the forward map divides by the scale) has twice
    ///    the volume to 1e-12.
    /// 3. Two plane scalings with the same normal and overlapping ranges are
    ///    refused (`Overlapping`); with non-parallel, non-perpendicular
    ///    normals (`NotParallel`).
    ///
    /// **Results (2026-11-05): all held.** Largest revert error 2.2e-16
    /// (box, 1 111 of 1 331 points moved), 1.1e-16 (plane, 847), 2.5e-16
    /// (both, 1 251); the ×2 box's volume 2.000000000000001; both refusals
    /// as predicted.
    #[test]
    fn modify_then_revert_and_scaling() {
        let bx = Modification::Box(BoxScaling { name: "box".into(), centre: Vec3::new(0.5, 0.5, 0.5), length: Vec3::new(0.4, 0.6, 1.0), scale: Vec3::new(2.0, 0.5, 1.0) });
        let pl = Modification::Plane(PlaneScaling::new("plane", Vec3::new(0.0, 0.0, 0.2), Vec3::new(0.0, 0.0, 1.0), 0.3, 3.0));
        let pts0 = grid(10, -0.5, 1.5);
        for mods in [vec![bx.clone()], vec![pl.clone()], vec![bx.clone(), pl.clone()]] {
            let cm = CoordinateModifier::new(mods.clone()).expect("valid inverse");
            let mut pts = pts0.clone();
            modify_points(&mut pts, &cm);
            let moved = pts.iter().zip(&pts0).filter(|(a, b)| a != b).count();
            revert_points(&mut pts, &cm);
            let err = pts.iter().zip(&pts0).map(|(a, b)| a.sub(*b).length()).fold(0.0, f64::max);
            println!("{:?}: {moved} of {} points moved, max revert error {err:e}", mods.iter().map(|m| m.name()).collect::<Vec<_>>(), pts0.len());
            assert!(moved > 0);
            assert!(err < 1e-12, "{err}");
        }
        // x2 in x.
        let mut m = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [3, 3, 3]);
        let cover = Modification::Box(BoxScaling { name: "x2".into(), centre: Vec3::new(0.5, 0.5, 0.5), length: Vec3::new(1.0, 1.0, 1.0), scale: Vec3::new(0.5, 1.0, 1.0) });
        let cm = CoordinateModifier::new(vec![cover]).unwrap();
        modify_points(&mut m.points, &cm);
        println!("x2 box volume {}", m.total_volume());
        assert!((m.total_volume() - 2.0).abs() < 1e-12);
        // Refusals.
        let p2 = Modification::Plane(PlaneScaling::new("plane2", Vec3::new(0.0, 0.0, 0.4), Vec3::new(0.0, 0.0, 1.0), 0.3, 2.0));
        assert!(matches!(CoordinateModifier::new(vec![pl.clone(), p2]), Err(AnisotropicError::Overlapping(_, _))));
        let p3 = Modification::Plane(PlaneScaling::new("plane3", Vec3::new(0.0, 0.0, 5.0), Vec3::new(0.0, 1.0, 1.0), 0.3, 2.0));
        assert!(matches!(CoordinateModifier::new(vec![pl, p3]), Err(AnisotropicError::NotParallel(_, _))));
    }

    #[test]
    fn scale_and_bounding_box() {
        let mut p = vec![Vec3::new(1.0, -2.0, 3.0), Vec3::new(-1.0, 4.0, 0.5)];
        scale_points(&mut p, 1e-3);
        assert_eq!(p[0], Vec3::new(1e-3, -2e-3, 3e-3));
        let (lo, hi) = surface_bounding_box(&p, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert_eq!(lo, Vec3::new(-1e-3 - 1.0, -2e-3 - 3.0, 0.5e-3 - 5.0));
        assert_eq!(hi, Vec3::new(1e-3 + 2.0, 4e-3 + 4.0, 3e-3 + 6.0));
    }

    /// `surfaceGenerateBoundingBox` around a unit sphere surface, margins 1
    /// on every side. **Prediction (written 2026-11-05, before the first
    /// run):** 12 more triangles in 6 new patches; upstream's winding makes
    /// the box's triangles point **outward**, so the box alone (its 12
    /// triangles) has signed volume +3³ = 27 by `shapes::surface_volume`.
    #[test]
    fn surface_generate_bounding_box_appends_an_outward_box() {
        use crate::mesh_octree::TriSurf;
        let (p, t) = crate::shapes::sphere_surface(Vec3::ZERO, 0.5, 6, 12);
        let s = TriSurf::new(p, t, None);
        let out = surface_generate_bounding_box(&s, [1.0; 6]);
        assert_eq!(out.triangles.len(), s.triangles.len() + 12);
        assert_eq!(out.patches.len(), 1 + 6);
        let boxv = crate::shapes::surface_volume(&out.points, &out.triangles[s.triangles.len()..]);
        println!("bounding box signed volume {boxv}");
        assert!((boxv - 27.0).abs() < 1e-9, "{boxv}");
    }

    /// The 2D channel ribbon `[0, 1] x [0, 0.25]` (regions walls, inlet,
    /// outlet), as `cartesian_2d_mesh`'s tests build it.
    pub(crate) fn channel_ribbon() -> (Vec<Vec3>, Vec<[usize; 3]>, crate::patches::SurfaceRegions) {
        let p = vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(1.0, 0.25, 0.0), Vec3::new(0.0, 0.25, 0.0)];
        let e = [[0, 1], [1, 2], [2, 3], [3, 0]];
        let (pts, tris, r) = crate::tri_surface_2d::extrude_2d_edges(&p, &e, Some(&[0, 2, 0, 1])).unwrap();
        let names = ["walls", "inlet", "outlet"];
        let labels: Vec<&str> = r.iter().map(|&i| names[i]).collect();
        (pts, tris, crate::patches::SurfaceRegions::from_labels(&labels))
    }

    /// gh:#1279 remainder, prediction W0 of
    /// `verification_and_validation/anisotropic_gh1279/README.md`: with no
    /// `anisotropicSources` every generator's output is the one recorded
    /// **before** the anisotropic workflow was wired (fingerprints taken on
    /// the unwired code, 2026-11-05): `cartesian_mesh`, `tet_mesh` and
    /// `p_mesh` on the 24-sided pipe (R 10 mm, L 60 mm) at 4 mm,
    /// `cartesian_2d_mesh` on the channel ribbon at 0.05.
    #[test]
    fn generators_without_anisotropic_sources_are_pinned_bit_for_bit() {
        use crate::mesh_octree_creator::MeshDict;
        use crate::pipeline::tests::mesh_fingerprint;
        let (p, t, r) = crate::pipeline::tet_mesh::tests::pipe_surface(0.01, 0.06, 24);
        let dict = MeshDict::with_max_cell_size(0.004);
        let c = crate::pipeline::cartesian_mesh::CartesianMeshGeneratorOptions { dict: dict.clone(), ..Default::default() };
        let (mc, _) = crate::pipeline::cartesian_mesh::cartesian_mesh(&p, &t, Some(&r), &c).unwrap();
        let o = crate::pipeline::tet_mesh::TetMeshOptions { dict: dict.clone(), ..Default::default() };
        let (mt, _) = crate::pipeline::tet_mesh::tet_mesh(&p, &t, Some(&r), &o).unwrap();
        let (mp, _) = crate::pipeline::p_mesh::p_mesh(&p, &t, Some(&r), &o).unwrap();
        let (cp, ct, cr) = channel_ribbon();
        let o2 = crate::pipeline::cartesian_2d_mesh::Cartesian2DMeshOptions { dict: MeshDict::with_max_cell_size(0.05), ..Default::default() };
        let (m2, _) = crate::pipeline::cartesian_2d_mesh::cartesian_2d_mesh(&cp, &ct, Some(&cr), &o2).unwrap();
        let got = [mesh_fingerprint(&mc), mesh_fingerprint(&mt), mesh_fingerprint(&mp), mesh_fingerprint(&m2)];
        println!("fingerprints cartesian {} tet {} p {} 2D {}; cells {} {} {} {}", got[0], got[1], got[2], got[3], mc.n_cells, mt.n_cells, mp.n_cells, m2.n_cells);
        // Re-pinned 2026-11-11 (gh:#1385), by decision of the task that made
        // the generators follow upstream bit for bit (the cell counts did not
        // move): cartesian ~~17611812511072804258~~ ~~7180829290865451833~~ 13254281021629600304
        // (moved again 2026-11-11, gh:#1385: the mesh carries cfMesh's cell face
        // lists, so `meshOptimisation`'s sums run in upstream's order and the
        // sphere is bit-identical to upstream; the pipe moves with it),
        // tet ~~6523714470801621594~~ ~~17254294393733896570~~ 13218850471215003652
        // (moved again 2026-11-11, gh:#1385: `TopoMesh::remove_cells` follows
        // `reorderBoundaryFaces`' pairwise swap and `face::reverseFace`, and
        // `tetMesh`'s `optimizeSurface` goes through the octree), p
        // ~~3039883993825308236~~ ~~3134079275981119636~~ 11509605158896565506, 2D
        // ~~4427172929438988611~~ ~~11695678344881703815~~ 871841484633274847.
        // Third move, 2026-11-11 (gh:#1385), all four again (cells unchanged):
        // cartesian ~~13254281021629600304~~ 13144889121674496086, tet
        // ~~13218850471215003652~~ 5974506692092754484. Causes: `decomposeCells`
        // in upstream's sequence, `createNewVertex` as upstream
        // (infinite-line edge distance, divisions), the layer's cell lists as
        // `replaceBoundary` appends them, the octree-backed `pMesh`
        // `optimiseFinalMesh`, the corner / edge mapper as upstream writes it.
        // See the gh:#1385 section of the cfMesh crate's V&V README.
        // **Fourth move, 2026-11-11 (gh:#1385 round 4, FLAGGED for the maintainer):
        // only the 2D fingerprint, ~~871841484633274847~~ 4370305401663068383
        // (cells 120 unchanged; cartesian, tet and p did not move).** Causes:
        // `meshSurfaceMapper2D::preMapVertices` runs 2 iterations as upstream
        // (the port ran the 3D routine's 3), and the 2D template's
        // `createEmptyPatches` / `distributeBoundaryFaces` carry the cell lists
        // through `replaceBoundary` (the lists decide the final `renumberMesh`).
        // **Fifth move, 2026-11-11 (gh:#1385 round 4, FLAGGED for the maintainer):
        // only the `pMesh` fingerprint, ~~11509605158896565506~~ 9105338267624933032
        // (cells 1529 unchanged).** Causes: `createFundamentalSheetsJFS` /
        // `extrudeLayer` follow upstream's face and cell-list order, `surfaceMorpherCells`
        // keeps the cell lists (`removeFaces`), `optimiseMeshSurface` untangles on the
        // same optimiser as `optimizeSurface`, and the volume optimiser's zero-volume
        // branch rounds as upstream.
        assert_eq!(got, [13144889121674496086, 5974506692092754484, 9105338267624933032, 4370305401663068383]);
        assert_eq!((mc.n_cells, mt.n_cells, mp.n_cells, m2.n_cells), (560, 4576, 1529, 120));
    }

    /// The slab of the gh:#1279 remainder's setup S: `|z| <= 1` compressed
    /// by 0.5 in z once reverted (the forward map stretches it by 2).
    pub(crate) fn slab() -> CoordinateModifier {
        CoordinateModifier::new(vec![Modification::Box(BoxScaling { name: "slab".into(), centre: Vec3::ZERO, length: Vec3::new(8.0, 8.0, 2.0), scale: Vec3::new(1.0, 1.0, 0.5) })]).unwrap()
    }

    /// Per cell `rings` cells or more from the wall (`rings` 1: touching
    /// none of the wall points; 2: also sharing no point with a cell that
    /// does): (is a hexahedron, bounding-box min, max).
    fn interior_cells(m: &crate::volume_mesh::VolumeMesh, skip_patch: &dyn Fn(&str) -> bool, rings: usize) -> Vec<(bool, Vec3, Vec3)> {
        let mut wall = vec![false; m.points.len()];
        for p in m.patches.iter().filter(|p| !skip_patch(&p.name)) {
            for f in p.start_face..p.start_face + p.n_faces {
                for &v in &m.faces[f] {
                    wall[v] = true;
                }
            }
        }
        let cf = crate::volume_mesh::cells_faces(m);
        for _ in 1..rings {
            let near: Vec<usize> = cf.iter().filter(|c| c.iter().flatten().any(|&v| wall[v])).flat_map(|c| c.iter().flatten().copied().collect::<Vec<_>>()).collect();
            for v in near {
                wall[v] = true;
            }
        }
        let mut out = Vec::new();
        for c in cf {
            if c.iter().flatten().any(|&v| wall[v]) {
                continue;
            }
            let hex = c.len() == 6 && c.iter().all(|f| f.len() == 4);
            let mut lo = Vec3::new(f64::MAX, f64::MAX, f64::MAX);
            let mut hi = Vec3::new(f64::MIN, f64::MIN, f64::MIN);
            for &v in c.iter().flatten() {
                let p = m.points[v];
                lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
                hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
            }
            out.push((hex, lo, hi));
        }
        out
    }

    /// (count, mean z-extent / mean x-extent, smallest and largest per-cell
    /// dz/dx) of the cells `pick` keeps.
    fn extent_ratio(cells: &[(bool, Vec3, Vec3)], pick: &dyn Fn(&(bool, Vec3, Vec3)) -> bool) -> (usize, f64, f64, f64) {
        let sel: Vec<_> = cells.iter().filter(|c| pick(c)).collect();
        let (mut sx, mut sz, mut lo, mut hi) = (0.0, 0.0, f64::MAX, f64::MIN);
        for (_, a, b) in &sel {
            let (dx, dz) = (b.x - a.x, b.z - a.z);
            sx += dx;
            sz += dz;
            lo = lo.min(dz / dx);
            hi = hi.max(dz / dx);
        }
        (sel.len(), sz / sx, lo, hi)
    }

    /// Largest distance of a boundary point of `m` from the surface.
    fn max_boundary_distance(m: &crate::volume_mesh::VolumeMesh, p: &[Vec3], t: &[[usize; 3]]) -> f64 {
        let mut d: f64 = 0.0;
        for f in m.n_internal_faces()..m.face_count() {
            for &v in &m.faces[f] {
                d = d.max(crate::snap::closest_point_on_surface(m.points[v], p, t).sub(m.points[v]).length());
            }
        }
        d
    }

    /// The meshes of the gh:#1279 remainder's V&V on setup S (sphere R 3 m,
    /// `maxCellSize` 0.5): (name, isotropic control, with the slab).
    pub(crate) fn sphere_runs() -> Vec<(&'static str, crate::volume_mesh::VolumeMesh, crate::volume_mesh::VolumeMesh, Vec<String>)> {
        use crate::mesh_octree_creator::MeshDict;
        use crate::pipeline::cartesian_mesh::{cartesian_mesh, CartesianMeshGeneratorOptions};
        use crate::pipeline::tet_mesh::{tet_mesh, TetMeshOptions};
        let (p, t) = crate::shapes::sphere_surface(Vec3::ZERO, 3.0, 24, 48);
        let iso = MeshDict::with_max_cell_size(0.5);
        let ani = MeshDict { anisotropic_sources: Some(slab()), ..iso.clone() };
        let mut out = Vec::new();
        let c = |d: &MeshDict| cartesian_mesh(&p, &t, None, &CartesianMeshGeneratorOptions { dict: d.clone(), ..Default::default() }).unwrap();
        let (a, b) = (c(&iso), c(&ani));
        out.push(("cartesian_mesh", a.0, b.0, b.1.notes));
        let tm = |d: &MeshDict| tet_mesh(&p, &t, None, &TetMeshOptions { dict: d.clone(), ..Default::default() }).unwrap();
        let (a, b) = (tm(&iso), tm(&ani));
        out.push(("tet_mesh", a.0, b.0, b.1.notes));
        let pm = |d: &MeshDict| crate::pipeline::p_mesh::p_mesh(&p, &t, None, &TetMeshOptions { dict: d.clone(), ..Default::default() }).unwrap();
        let (a, b) = (pm(&iso), pm(&ani));
        out.push(("p_mesh", a.0, b.0, b.1.notes));
        out
    }

    /// The 2D run of W7: the channel ribbon at 0.05, `x <= 0.5` stretched
    /// by 2 (isotropic control, with the box).
    pub(crate) fn channel_runs() -> (crate::volume_mesh::VolumeMesh, crate::volume_mesh::VolumeMesh, Vec<String>) {
        use crate::mesh_octree_creator::MeshDict;
        use crate::pipeline::cartesian_2d_mesh::{cartesian_2d_mesh, Cartesian2DMeshOptions};
        let (p, t, r) = channel_ribbon();
        let half = CoordinateModifier::new(vec![Modification::Box(BoxScaling { name: "half".into(), centre: Vec3::new(0.25, 0.125, 0.0), length: Vec3::new(0.5, 1.0, 1.0), scale: Vec3::new(0.5, 1.0, 1.0) })]).unwrap();
        let iso = MeshDict::with_max_cell_size(0.05);
        let ani = MeshDict { anisotropic_sources: Some(half), ..iso.clone() };
        let (a, _) = cartesian_2d_mesh(&p, &t, Some(&r), &Cartesian2DMeshOptions { dict: iso, ..Default::default() }).unwrap();
        let (b, rb) = cartesian_2d_mesh(&p, &t, Some(&r), &Cartesian2DMeshOptions { dict: ani, ..Default::default() }).unwrap();
        (a, b, rb.notes)
    }

    /// V&V (gh:#1279 remainder), predictions W1–W8 of
    /// `verification_and_validation/anisotropic_gh1279/README.md`: the
    /// slab `boxScaling` on the sphere through `cartesian_mesh`, `tet_mesh`
    /// and `p_mesh` against isotropic control runs, the half-channel box
    /// through `cartesian_2d_mesh`, and an anisotropic restart. Asserts the
    /// closure, the positive cells, the restart, and the predictions that
    /// held; the rest is recorded in the README.
    #[test]
    fn anisotropic_sources_through_the_generators() {
        let (sp, st) = crate::shapes::sphere_surface(Vec3::ZERO, 3.0, 24, 48);
        let exact = 4.0 / 3.0 * std::f64::consts::PI * 27.0;
        let nonpos = |m: &crate::volume_mesh::VolumeMesh| crate::checks::cell_geometry(m).0.iter().filter(|&&v| v <= 0.0).count();
        let e = 1e-9;
        let in_slab = |c: &(bool, Vec3, Vec3)| c.1.z >= -1.0 - e && c.2.z <= 1.0 + e;
        let outside = |c: &(bool, Vec3, Vec3)| c.1.z >= 1.0 - e || c.2.z <= -1.0 + e;
        let no_skip = |_: &str| false;
        println!("| generator | run | rings | cells | volume error | non-positive | closed | interior cells in slab: n, mean dz/mean dx, min..max | outside: n, ratio, min..max | max boundary distance |");
        println!("|---|---|---|---|---|---|---|---|---|---|");
        for (name, iso, ani, notes) in sphere_runs() {
            for n in &notes {
                println!("  {name}: {n}");
            }
            for (run, m, rings) in [("isotropic", &iso, 1), ("slab", &ani, 1), ("isotropic", &iso, 2), ("slab", &ani, 2)] {
                let cells = interior_cells(m, &no_skip, rings);
                let hex_only = name == "cartesian_mesh";
                let pick_in = |c: &(bool, Vec3, Vec3)| in_slab(c) && (!hex_only || c.0);
                let pick_out = |c: &(bool, Vec3, Vec3)| outside(c) && (!hex_only || c.0);
                let a = extent_ratio(&cells, &pick_in);
                let b = extent_ratio(&cells, &pick_out);
                println!(
                    "| {name} | {run} | {rings} | {} | {:+.3} % | {} | {} | {}, {:.6}, {:.6}..{:.6} | {}, {:.6}, {:.6}..{:.6} | {:.3e} |",
                    m.n_cells,
                    (m.total_volume() - exact) / exact * 100.0,
                    nonpos(m),
                    if m.validate().is_ok() { "yes" } else { "NO" },
                    a.0,
                    a.1,
                    a.2,
                    a.3,
                    b.0,
                    b.1,
                    b.2,
                    b.3,
                    max_boundary_distance(m, &sp, &st)
                );
            }
            ani.validate().unwrap();
            assert_eq!(nonpos(&ani), 0, "{name}");
        }
        // 2D
        let (iso2, ani2, notes2) = channel_runs();
        for n in &notes2 {
            println!("  cartesian_2d_mesh: {n}");
        }
        let empty = |s: &str| s.ends_with("EmptyFaces");
        let zt = ani2.points.iter().map(|q| q.z).fold(f64::MIN, f64::max);
        for (run, m) in [("isotropic", &iso2), ("half box", &ani2)] {
            let cells = interior_cells(m, &empty, 2);
            // dx/dy here: swap the roles of z and y in extent_ratio
            let yz: Vec<(bool, Vec3, Vec3)> = cells.iter().map(|(h, a, b)| (*h, Vec3::new(a.y, a.x, a.x), Vec3::new(b.y, b.x, b.x))).collect();
            let left = extent_ratio(&yz, &|c| c.2.z <= 0.5 + e);
            let right = extent_ratio(&yz, &|c| c.1.z >= 0.5 - e);
            let area = m.total_volume() / zt;
            println!(
                "2D {run}: cells {} area {area:.15} ({:+.3e}) non-positive {} closed {} | interior x <= 0.5: {} cells, dx/dy {:.6} ({:.6}..{:.6}) | x >= 0.5: {} cells, dx/dy {:.6} ({:.6}..{:.6}) | z in [{}, {zt}]",
                m.n_cells,
                (area - 0.25) / 0.25,
                nonpos(m),
                m.validate().is_ok(),
                left.0,
                left.1,
                left.2,
                left.3,
                right.0,
                right.1,
                right.2,
                right.3,
                m.points.iter().map(|q| q.z).fold(f64::MAX, f64::min)
            );
        }
        ani2.validate().unwrap();
        assert_eq!(nonpos(&ani2), 0);
        // W8: restart of an anisotropic cartesian_mesh run
        use crate::pipeline::cartesian_mesh::{cartesian_mesh, cartesian_mesh_restart, CartesianMeshGeneratorOptions};
        use crate::workflow_controls::{WorkflowSettings, WorkflowStep};
        let base = CartesianMeshGeneratorOptions { dict: crate::mesh_octree_creator::MeshDict { anisotropic_sources: Some(slab()), ..crate::mesh_octree_creator::MeshDict::with_max_cell_size(0.5) }, ..Default::default() };
        let (full, _) = cartesian_mesh(&sp, &st, None, &base).unwrap();
        let stop = CartesianMeshGeneratorOptions { workflow: WorkflowSettings { stop_after: Some(WorkflowStep::EdgeExtraction), restart_from_latest_step: false }, ..base.clone() };
        let (stopped, srep) = cartesian_mesh(&sp, &st, None, &stop).unwrap();
        let zmax = stopped.points.iter().map(|q| q.z).fold(f64::MIN, f64::max);
        println!("W8: the mesh stopped after edgeExtraction reaches z {zmax:.4} (modified space: the sphere's top is at 5)");
        let saved = crate::pipeline::tet_mesh::SavedMesh { mesh: stopped, patch_types: srep.patch_types.clone(), meta: srep.meta.clone() };
        let restart = CartesianMeshGeneratorOptions { workflow: WorkflowSettings { stop_after: None, restart_from_latest_step: true }, ..base.clone() };
        let (resumed, _) = cartesian_mesh_restart(&sp, &st, None, &restart, &saved).unwrap();
        let same = crate::tet_mesh_extractor_octree::same_mesh(&resumed, &full);
        println!("W8: restarted run equals the full run bit for bit: {same}");
        assert!(same);
        assert!(zmax > 4.0);
    }

    /// Images (gh:#1279 remainder), `--ignored`: x-z slices at y = 0.05 of
    /// the sphere meshes (isotropic control and slab) of the three 3D
    /// generators, and x-y slices of the 2D channel, into
    /// `verification_and_validation/anisotropic_gh1279/`. Drawn from the
    /// assembled meshes; the dashed circle is the analytic sphere.
    #[test]
    #[ignore]
    fn draw_anisotropic_images() {
        use crate::topology::tests::write_slice_png;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/anisotropic_gh1279");
        let none = |_: usize| None;
        for (name, iso, ani, _) in sphere_runs() {
            for (run, m) in [("isotropic", &iso), ("slab", &ani)] {
                let t = format!("{name} sphere R 3 m, maxCellSize 0.5, {run}{}: x-z at y 0.05, {} cells", if run == "slab" { " (boxScaling |z| <= 1, scale z 0.5)" } else { "" }, m.n_cells);
                write_slice_png(m, 1, 0.05, [-3.4, 3.4, -3.4, 3.4], &[(0.0, 0.0, 3.0)], &none, &t, &dir.join(format!("sphere_xz_{name}_{run}.png")));
            }
        }
        let (iso2, ani2, _) = channel_runs();
        for (run, m) in [("isotropic", &iso2), ("half_box", &ani2)] {
            let zt = m.points.iter().map(|q| q.z).fold(f64::MIN, f64::max);
            let t = format!("cartesian_2d_mesh channel 1 x 0.25, maxCellSize 0.05, {run}: x-y mid-plane, {} cells", m.n_cells);
            write_slice_png(m, 2, 0.5 * zt, [-0.05, 1.05, -0.05, 0.30], &[], &none, &t, &dir.join(format!("channel_xy_{run}.png")));
        }
    }

    /// gh:#1279 remainder, the check behind W1's miss (README): on the
    /// isotropic control (sphere, 0.5), the hexahedra two rings from the
    /// wall are exact cubes after every step up to `boundaryLayerGeneration`
    /// and stop being so only in `meshOptimisation` (the optimiser's
    /// smoothing, upstream's design). Recorded 2026-11-05: ratio 1.000000
    /// through `boundaryLayerGeneration` (32, then 184 cells), 0.930..1.074
    /// after `meshOptimisation`.
    #[test]
    fn interior_hexes_stay_cubes_until_mesh_optimisation() {
        use crate::pipeline::cartesian_mesh::{cartesian_mesh, CartesianMeshGeneratorOptions};
        use crate::workflow_controls::{WorkflowSettings, WorkflowStep};
        let (p, t) = crate::shapes::sphere_surface(Vec3::ZERO, 3.0, 24, 48);
        for st in [WorkflowStep::TemplateGeneration, WorkflowStep::SurfaceTopology, WorkflowStep::SurfaceProjection, WorkflowStep::PatchAssignment, WorkflowStep::EdgeExtraction, WorkflowStep::BoundaryLayerGeneration, WorkflowStep::MeshOptimisation] {
            let o = CartesianMeshGeneratorOptions { dict: crate::mesh_octree_creator::MeshDict::with_max_cell_size(0.5), workflow: WorkflowSettings { stop_after: Some(st), restart_from_latest_step: false }, ..Default::default() };
            let (m, _) = cartesian_mesh(&p, &t, None, &o).unwrap();
            let c = interior_cells(&m, &|_: &str| false, 2);
            let r = extent_ratio(&c, &|c| c.0);
            println!("after {st:?}: {} cells, ring-2 hexes {} ratio {:.6} {:.6}..{:.6}", m.n_cells, r.0, r.1, r.2, r.3);
            if st != WorkflowStep::MeshOptimisation {
                assert!((r.2 - 1.0).abs() < 1e-12 && (r.3 - 1.0).abs() < 1e-12, "{st:?}");
            }
        }
    }
}
