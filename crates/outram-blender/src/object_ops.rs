// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Object-mode CAD operators. Blender analogue (architecture only): the
// Object menu operators — Duplicate / Linked Duplicate, Join, Separate,
// Convert, Apply Transform, Set Origin, Align Objects, Snap. No upstream
// source copied. Built on this crate's `Affine3` and `measure`.
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

//! **Object-mode CAD operators** (`op-hzs.54.47`, GH issue #37 §J).
//!
//! A [`SceneObject`] is a shared mesh in *local* space plus an [`Affine3`]
//! placing it in the world. The operators here work on objects and lists of
//! them:
//!
//! - [`SceneObject::duplicate`] / [`SceneObject::linked_duplicate`] — a full
//!   copy vs. a copy that shares the same `Arc<Mesh>`.
//! - [`join`] — bake several objects' world geometry into one mesh.
//! - [`separate_loose_parts`] — split an object's mesh into its connected
//!   components, each a new object.
//! - [`SceneObject::apply_transform`] — bake the transform into the mesh and
//!   reset it to the identity.
//! - [`SceneObject::set_origin`] — move the object's local origin
//!   ([`OriginMode`]) without moving the geometry in the world.
//! - [`align`] — line objects' bounding boxes up along an axis.
//! - [`snap_objects`] / [`cursor_to_objects`] — the Snap menu.
//!
//! ## Units
//!
//! Positions/lengths are dimensionless model-space quantities (see
//! [`crate::math`]).

use std::sync::Arc;

use crate::math::Vec3;
use crate::mesh::Mesh;
use crate::selection::Axis;
use crate::transform::Affine3;

/// A placed mesh: geometry in local space, [`Affine3`] to world space.
#[derive(Debug, Clone)]
pub struct SceneObject {
    /// Geometry in the object's local frame (shared, never mutated in place).
    pub mesh: Arc<Mesh>,
    /// Local → world transform.
    pub transform: Affine3,
}

impl SceneObject {
    /// A new object at the identity transform.
    pub fn new(mesh: Mesh) -> Self {
        SceneObject { mesh: Arc::new(mesh), transform: Affine3::IDENTITY }
    }

    /// This object's geometry in world space.
    pub fn world_mesh(&self) -> Mesh {
        let positions = self.transform.transform_points(&self.mesh.positions());
        let faces: Vec<Vec<usize>> =
            self.mesh.polygons().iter().map(|f| f.iter().map(|v| v.0).collect()).collect();
        Mesh::from_polygons(&positions, &faces)
    }

    /// The object's origin in world space (its transform's translation).
    pub fn world_origin(&self) -> Vec3 {
        self.transform.translation
    }

    /// A full, independent copy (new mesh storage).
    pub fn duplicate(&self) -> SceneObject {
        SceneObject { mesh: Arc::new((*self.mesh).clone()), transform: self.transform }
    }

    /// A copy that **shares** the same mesh data (Blender's Linked Duplicate);
    /// editing one instance's mesh would affect both.
    pub fn linked_duplicate(&self) -> SceneObject {
        SceneObject { mesh: Arc::clone(&self.mesh), transform: self.transform }
    }

    /// Whether this object shares its mesh with `other`.
    pub fn shares_mesh_with(&self, other: &SceneObject) -> bool {
        Arc::ptr_eq(&self.mesh, &other.mesh)
    }

    /// Bake the transform into the geometry and reset the transform to the
    /// identity (Apply Transform → All).
    pub fn apply_transform(&self) -> SceneObject {
        SceneObject { mesh: Arc::new(self.world_mesh()), transform: Affine3::IDENTITY }
    }

    /// Move the object's local origin per `mode`, keeping every vertex in the
    /// same world position. Only meaningful for a translation-only transform
    /// (the common object-mode case); the linear part is preserved as-is.
    pub fn set_origin(&self, mode: OriginMode) -> SceneObject {
        let new_local_origin = match mode {
            OriginMode::GeometryMedian => vertex_median(&self.mesh),
            OriginMode::CenterOfMassSurface => surface_com(&self.mesh),
            OriginMode::CenterOfMassVolume => volume_com(&self.mesh),
            OriginMode::Cursor { world } => {
                // World point → local via the inverse translation (assumes a
                // translation-only transform).
                world.sub(self.transform.translation)
            }
        };
        let shifted: Vec<Vec3> =
            self.mesh.positions().iter().map(|p| p.sub(new_local_origin)).collect();
        let faces: Vec<Vec<usize>> =
            self.mesh.polygons().iter().map(|f| f.iter().map(|v| v.0).collect()).collect();
        let m = self.transform.linear;
        let mapped = Vec3::new(
            m[0][0] * new_local_origin.x + m[0][1] * new_local_origin.y + m[0][2] * new_local_origin.z,
            m[1][0] * new_local_origin.x + m[1][1] * new_local_origin.y + m[1][2] * new_local_origin.z,
            m[2][0] * new_local_origin.x + m[2][1] * new_local_origin.y + m[2][2] * new_local_origin.z,
        );
        SceneObject {
            mesh: Arc::new(Mesh::from_polygons(&shifted, &faces)),
            transform: Affine3::from_rows(
                self.transform.linear,
                self.transform.translation.add(mapped),
            ),
        }
    }
}

/// Where [`SceneObject::set_origin`] puts the origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OriginMode {
    /// The mean of the mesh vertices (Origin to Geometry).
    GeometryMedian,
    /// Area-weighted mean face centroid (Center of Mass — Surface).
    CenterOfMassSurface,
    /// Volume centroid via signed tetrahedra (Center of Mass — Volume).
    CenterOfMassVolume,
    /// A given world point (Origin to 3D Cursor).
    Cursor {
        /// The cursor position in world space.
        world: Vec3,
    },
}

/// Bake `objects`' world geometry into a single identity-transform object
/// (Join). The first object's transform is discarded along with the rest —
/// use [`SceneObject::apply_transform`] on the result if you want it re-based.
pub fn join(objects: &[SceneObject]) -> SceneObject {
    let mut positions: Vec<Vec3> = Vec::new();
    let mut faces: Vec<Vec<usize>> = Vec::new();
    for obj in objects {
        let w = obj.world_mesh();
        let base = positions.len();
        positions.extend(w.positions());
        for f in w.polygons() {
            faces.push(f.iter().map(|v| v.0 + base).collect());
        }
    }
    SceneObject { mesh: Arc::new(Mesh::from_polygons(&positions, &faces)), transform: Affine3::IDENTITY }
}

/// Split `object`'s mesh into connected components (by shared vertices), each
/// returned as a new object with the **same** transform as the original
/// (Separate → By Loose Parts).
pub fn separate_loose_parts(object: &SceneObject) -> Vec<SceneObject> {
    let mesh = &*object.mesh;
    let n = mesh.vertex_count();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut [usize], x: usize) -> usize {
        let mut r = x;
        while p[r] != r {
            r = p[r];
        }
        let mut c = x;
        while p[c] != r {
            let nx = p[c];
            p[c] = r;
            c = nx;
        }
        r
    }
    let polys = mesh.polygons();
    for f in &polys {
        for w in f.windows(2) {
            let (a, b) = (find(&mut parent, w[0].0), find(&mut parent, w[1].0));
            if a != b {
                parent[a] = b;
            }
        }
        if f.len() > 2 {
            let (a, b) = (find(&mut parent, f[0].0), find(&mut parent, f[f.len() - 1].0));
            if a != b {
                parent[a] = b;
            }
        }
    }
    use std::collections::BTreeMap;
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for v in 0..n {
        let r = find(&mut parent, v);
        groups.entry(r).or_default().push(v);
    }
    let positions = mesh.positions();
    groups
        .values()
        .filter_map(|verts| {
            let remap: std::collections::HashMap<usize, usize> =
                verts.iter().enumerate().map(|(i, &v)| (v, i)).collect();
            let sub_pos: Vec<Vec3> = verts.iter().map(|&v| positions[v]).collect();
            let sub_faces: Vec<Vec<usize>> = polys
                .iter()
                .filter(|f| f.iter().all(|v| remap.contains_key(&v.0)))
                .map(|f| f.iter().map(|v| remap[&v.0]).collect())
                .collect();
            if sub_faces.is_empty() {
                return None;
            }
            Some(SceneObject {
                mesh: Arc::new(Mesh::from_polygons(&sub_pos, &sub_faces)),
                transform: object.transform,
            })
        })
        .collect()
}

/// How [`align`] lines objects up on the chosen axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AlignMode {
    /// Match the minimum (negative) side of each bounding box.
    Min,
    /// Match the bounding-box centres.
    Center,
    /// Match the maximum (positive) side.
    Max,
}

/// Translate each object along `axis` so its world bounding box lines up per
/// `mode` with the group's average reference coordinate. Returns new objects
/// (translation-only change).
pub fn align(objects: &[SceneObject], axis: Axis, mode: AlignMode) -> Vec<SceneObject> {
    let comp = |v: Vec3| match axis {
        Axis::X => v.x,
        Axis::Y => v.y,
        Axis::Z => v.z,
    };
    let refs: Vec<f64> = objects
        .iter()
        .map(|o| {
            let (lo, hi) = crate::measure::bounding_box(&o.world_mesh());
            match mode {
                AlignMode::Min => comp(lo),
                AlignMode::Center => (comp(lo) + comp(hi)) * 0.5,
                AlignMode::Max => comp(hi),
            }
        })
        .collect();
    let target = refs.iter().sum::<f64>() / refs.len().max(1) as f64;
    objects
        .iter()
        .zip(refs)
        .map(|(o, r)| {
            let d = target - r;
            let delta = match axis {
                Axis::X => Vec3::new(d, 0.0, 0.0),
                Axis::Y => Vec3::new(0.0, d, 0.0),
                Axis::Z => Vec3::new(0.0, 0.0, d),
            };
            SceneObject {
                mesh: Arc::clone(&o.mesh),
                transform: Affine3::from_rows(o.transform.linear, o.transform.translation.add(delta)),
            }
        })
        .collect()
}

/// Target for [`snap_objects`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SnapObjectsTo {
    /// Move each object's origin to `world` (Selection to Cursor).
    Cursor {
        /// The cursor position.
        world: Vec3,
    },
    /// Round each object's origin to a multiple of `step` (Selection to Grid).
    Grid {
        /// The grid step.
        step: f64,
    },
    /// Move every object's origin onto object `index`'s origin (Selection to
    /// Active).
    Active {
        /// Index of the active object in the slice.
        index: usize,
    },
}

/// Apply a Snap-menu target to every object (translation only).
pub fn snap_objects(objects: &[SceneObject], to: SnapObjectsTo) -> Vec<SceneObject> {
    let dest = |o: &SceneObject| -> Vec3 {
        match to {
            SnapObjectsTo::Cursor { world } => world,
            SnapObjectsTo::Grid { step } => {
                let s = step.max(1e-9);
                let p = o.transform.translation;
                Vec3::new((p.x / s).round() * s, (p.y / s).round() * s, (p.z / s).round() * s)
            }
            SnapObjectsTo::Active { index } => objects
                .get(index)
                .map(|a| a.transform.translation)
                .unwrap_or(o.transform.translation),
        }
    };
    objects
        .iter()
        .map(|o| SceneObject {
            mesh: Arc::clone(&o.mesh),
            transform: Affine3::from_rows(o.transform.linear, dest(o)),
        })
        .collect()
}

/// The median of the objects' world origins (Cursor to Selected).
pub fn cursor_to_objects(objects: &[SceneObject]) -> Vec3 {
    if objects.is_empty() {
        return Vec3::ZERO;
    }
    objects
        .iter()
        .fold(Vec3::ZERO, |a, o| a.add(o.transform.translation))
        .scale(1.0 / objects.len() as f64)
}

// --- centre-of-mass helpers ---

fn vertex_median(mesh: &Mesh) -> Vec3 {
    let p = mesh.positions();
    if p.is_empty() {
        return Vec3::ZERO;
    }
    p.iter().fold(Vec3::ZERO, |a, &q| a.add(q)).scale(1.0 / p.len() as f64)
}

fn surface_com(mesh: &Mesh) -> Vec3 {
    let mut wsum = 0.0;
    let mut acc = Vec3::ZERO;
    for f in 0..mesh.face_count() {
        let a = crate::measure::face_area(mesh, crate::mesh::FaceId(f));
        let c = mesh.face_centroid(crate::mesh::FaceId(f));
        acc = acc.add(c.scale(a));
        wsum += a;
    }
    if wsum < 1e-12 {
        vertex_median(mesh)
    } else {
        acc.scale(1.0 / wsum)
    }
}

fn volume_com(mesh: &Mesh) -> Vec3 {
    // Σ over face fan-triangles of (signed tet volume) * (tet centroid),
    // origin at the mesh's vertex median for conditioning.
    let o = vertex_median(mesh);
    let mut vsum = 0.0;
    let mut acc = Vec3::ZERO;
    for f in 0..mesh.face_count() {
        let vs = mesh.face_vertices(crate::mesh::FaceId(f));
        if vs.len() < 3 {
            continue;
        }
        let a = mesh.vertex(vs[0]).unwrap().position.sub(o);
        for i in 1..vs.len() - 1 {
            let b = mesh.vertex(vs[i]).unwrap().position.sub(o);
            let c = mesh.vertex(vs[i + 1]).unwrap().position.sub(o);
            let vol = a.dot(b.cross(c)) / 6.0;
            let centroid = a.add(b).add(c).scale(0.25); // (0 + a + b + c)/4
            acc = acc.add(centroid.scale(vol));
            vsum += vol;
        }
    }
    if vsum.abs() < 1e-12 {
        surface_com(mesh)
    } else {
        o.add(acc.scale(1.0 / vsum))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    fn translated(mesh: Mesh, t: Vec3) -> SceneObject {
        SceneObject { mesh: Arc::new(mesh), transform: Affine3::translation(t) }
    }

    #[test]
    fn linked_vs_full_duplicate() {
        let o = SceneObject::new(primitives::cube(2.0));
        assert!(o.linked_duplicate().shares_mesh_with(&o));
        assert!(!o.duplicate().shares_mesh_with(&o));
    }

    #[test]
    fn join_bakes_world_geometry() {
        let a = translated(primitives::cube(2.0), Vec3::new(-3.0, 0.0, 0.0));
        let b = translated(primitives::cube(2.0), Vec3::new(3.0, 0.0, 0.0));
        let j = join(&[a, b]);
        assert_eq!(j.mesh.vertex_count(), 16);
        let (lo, hi) = crate::measure::bounding_box(&j.mesh);
        assert!((lo.x + 4.0).abs() < 1e-9 && (hi.x - 4.0).abs() < 1e-9);
    }

    #[test]
    fn separate_loose_parts_splits_two_cubes() {
        let joined = join(&[
            translated(primitives::cube(2.0), Vec3::new(-3.0, 0.0, 0.0)),
            translated(primitives::cube(2.0), Vec3::new(3.0, 0.0, 0.0)),
        ]);
        let parts = separate_loose_parts(&joined);
        assert_eq!(parts.len(), 2);
        for p in &parts {
            assert_eq!(p.mesh.vertex_count(), 8);
            assert_eq!(p.mesh.euler_characteristic(), 2);
        }
    }

    #[test]
    fn apply_transform_bakes_and_resets() {
        let o = translated(primitives::cube(2.0), Vec3::new(5.0, 0.0, 0.0));
        let applied = o.apply_transform();
        assert_eq!(applied.transform, Affine3::IDENTITY);
        let (lo, hi) = crate::measure::bounding_box(&applied.mesh);
        assert!((lo.x - 4.0).abs() < 1e-9 && (hi.x - 6.0).abs() < 1e-9);
    }

    #[test]
    fn set_origin_to_geometry_keeps_world_position() {
        // Cube local-space corners at ±1, transform +10 x. Local geometry
        // median is the origin already, so move origin to a corner instead.
        let o = translated(primitives::cube(2.0), Vec3::new(10.0, 0.0, 0.0));
        let re = o.set_origin(OriginMode::Cursor { world: Vec3::new(11.0, 1.0, 1.0) });
        // World geometry unchanged.
        let (lo0, hi0) = crate::measure::bounding_box(&o.world_mesh());
        let (lo1, hi1) = crate::measure::bounding_box(&re.world_mesh());
        assert!(lo0.sub(lo1).length() < 1e-9 && hi0.sub(hi1).length() < 1e-9);
        // Origin moved to the requested world point.
        assert!(re.world_origin().sub(Vec3::new(11.0, 1.0, 1.0)).length() < 1e-9);
    }

    #[test]
    fn volume_com_of_a_cube_is_its_centre() {
        let mut m = primitives::cube(2.0);
        let shifted: Vec<Vec3> = m.positions().iter().map(|p| p.add(Vec3::new(1.0, 2.0, 3.0))).collect();
        let faces: Vec<Vec<usize>> =
            m.polygons().iter().map(|f| f.iter().map(|v| v.0).collect()).collect();
        m = Mesh::from_polygons(&shifted, &faces);
        let com = volume_com(&m);
        assert!(com.sub(Vec3::new(1.0, 2.0, 3.0)).length() < 1e-9);
    }

    #[test]
    fn align_centers_objects_on_x() {
        let objs = [
            translated(primitives::cube(2.0), Vec3::new(-5.0, 0.0, 0.0)),
            translated(primitives::cube(2.0), Vec3::new(1.0, 0.0, 0.0)),
            translated(primitives::cube(2.0), Vec3::new(10.0, 0.0, 0.0)),
        ];
        let aligned = align(&objs, Axis::X, AlignMode::Center);
        let centres: Vec<f64> = aligned
            .iter()
            .map(|o| {
                let (lo, hi) = crate::measure::bounding_box(&o.world_mesh());
                (lo.x + hi.x) * 0.5
            })
            .collect();
        let c0 = centres[0];
        assert!(centres.iter().all(|c| (c - c0).abs() < 1e-9));
    }

    #[test]
    fn snap_objects_to_grid_and_cursor() {
        let objs = [translated(primitives::cube(1.0), Vec3::new(0.34, 1.71, -0.4))];
        let g = snap_objects(&objs, SnapObjectsTo::Grid { step: 0.25 });
        assert!(g[0].world_origin().sub(Vec3::new(0.25, 1.75, -0.5)).length() < 1e-9);
        let c = snap_objects(&objs, SnapObjectsTo::Cursor { world: Vec3::new(7.0, 7.0, 7.0) });
        assert_eq!(c[0].world_origin(), Vec3::new(7.0, 7.0, 7.0));
    }

    #[test]
    fn cursor_to_objects_is_the_origin_median() {
        let objs = [
            translated(primitives::cube(1.0), Vec3::new(0.0, 0.0, 0.0)),
            translated(primitives::cube(1.0), Vec3::new(6.0, 0.0, 0.0)),
        ];
        assert_eq!(cursor_to_objects(&objs), Vec3::new(3.0, 0.0, 0.0));
    }
}
