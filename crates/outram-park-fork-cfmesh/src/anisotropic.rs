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
//! **No pipeline calls this module**: wiring `anisotropicSources` into a
//! generator (and `projectSurfaceAfterBackScaling`) is not done here.

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
        assert_eq!(got, [17611812511072804258, 6523714470801621594, 3039883993825308236, 4427172929438988611]);
        assert_eq!((mc.n_cells, mt.n_cells, mp.n_cells, m2.n_cells), (560, 4576, 1529, 120));
    }
}
