// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Non-destructive modifier stack. Follows the published architecture of Blender's
// modifier system (modifiers/intern/MOD_*, GPL-2.0-or-later) — concepts only, no
// upstream source was copied. The individual modifiers delegate to this crate's
// own operators; see subdivision.rs for the Catmull-Clark reference.
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

//! Non-destructive **modifier stack** (Blender's `modifiers/intern/MOD_*`).
//!
//! The [`Modifier`] enum and the [`ModifierStack`] that evaluates an ordered
//! list of them are real, compile, and produce geometry. Each modifier is a
//! pure function over the **polygon-soup** view of a mesh
//! ([`Mesh::positions`] + [`Mesh::polygons`]): it reads the base mesh, computes
//! new positions and faces, and rebuilds a fresh [`Mesh`] through
//! [`Mesh::from_polygons`] (which recomputes edge dedup and loop wiring for
//! free). The base mesh is never mutated — that is the "non-destructive"
//! property that distinguishes a modifier from a [`crate::ops`] operator.
//!
//! ## Modifier stack vs. operators
//!
//! A Blender **modifier** is *non-destructive*: it sits in an ordered stack on
//! an object and recomputes derived geometry from the original mesh every time,
//! leaving the base mesh untouched. That is the distinction from
//! [`crate::ops`], whose operators destructively edit a mesh in place. The
//! stack is evaluated top-to-bottom by [`ModifierStack::evaluate`].
//!
//! ## The modifiers (Blender analogue in parentheses)
//!
//! - [`Modifier::Subsurf`] — Catmull-Clark subdivision surface at a view level
//!   (`MOD_subsurf`, backed by OpenSubdiv upstream). Forwards to the locked
//!   [`crate::subdivision::catmull_clark`] contract.
//! - [`Modifier::Mirror`] — mirror across one or more axis planes with a seam
//!   weld (`MOD_mirror`).
//! - [`Modifier::Array`] — repeat the mesh in a regular relative-offset pattern
//!   (`MOD_array`).
//!
//! ## Units
//!
//! All coordinates are dimensionless model-space lengths (see [`crate::math`]);
//! the [`Modifier::Array`] `offset` is a *relative* offset expressed in
//! multiples of the input mesh's bounding-box extent along each axis, exactly
//! like Blender's Array modifier "Relative Offset" factor.

use crate::math::Vec3;
use crate::mesh::Mesh;

/// Errors returned while evaluating a [`Modifier`] or a [`ModifierStack`].
#[derive(Debug, thiserror::Error)]
pub enum ModifierError {
    /// A modifier is scaffolded but its algorithm is not implemented yet.
    ///
    /// Retained for forward compatibility (new modifier variants may land as
    /// stubs).
    #[error("modifier not yet implemented: {0}")]
    NotImplemented(&'static str),
    /// A modifier's underlying operator failed (e.g. a boolean on
    /// non-manifold input).
    #[error("modifier evaluation failed: {0}")]
    Failed(String),
}

/// Which axis planes a [`Modifier::Mirror`] reflects across.
///
/// Each `bool` enables reflecting across the plane orthogonal to that axis; set
/// more than one to mirror sequentially (X, then Y, then Z).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MirrorAxes {
    /// Mirror across the YZ plane (reflect the X coordinate).
    pub x: bool,
    /// Mirror across the XZ plane (reflect the Y coordinate).
    pub y: bool,
    /// Mirror across the XY plane (reflect the Z coordinate).
    pub z: bool,
}

/// A closed set of non-destructive modifiers.
#[derive(Debug, Clone)]
pub enum Modifier {
    /// Catmull-Clark subdivision to `levels` refinement passes.
    Subsurf {
        /// Number of subdivision levels (viewport render level). `0` returns
        /// the input unchanged; each level quadruples the face count of a
        /// quad mesh.
        levels: u32,
    },
    /// Mirror the mesh across the selected axis planes and weld the seam.
    Mirror {
        /// Which axis planes to reflect across.
        axes: MirrorAxes,
    },
    /// Repeat the mesh `count` times, each copy offset by a multiple of the
    /// mesh's bounding-box extent along each axis (relative-offset array).
    Array {
        /// Number of copies including the original (`>= 1`; `0` is treated as
        /// `1`).
        count: u32,
        /// Per-axis relative offset. Copy `k` is translated by
        /// `k * offset[axis] * bbox_size[axis]`, where `bbox_size` is the
        /// input mesh's bounding-box extent along that axis.
        offset: [f64; 3],
    },
    /// Bevel every edge — forwards to [`crate::bevel::bevel`] (GH issue #37 §F,
    /// `op-hzs.54.29`).
    Bevel {
        /// [`crate::bevel::BevelOptions`] for the bevel.
        options: crate::bevel::BevelOptions,
    },
    /// CSG boolean against `operand` — forwards to [`crate::boolean::boolean`].
    Boolean {
        /// The cutter / combiner mesh (shared, never mutated).
        operand: std::sync::Arc<Mesh>,
        /// Union / difference / intersect.
        op: crate::ops::BooleanMode,
    },
    /// Thicken the surface into a shell — forwards to
    /// [`crate::solidify::solidify`].
    Solidify {
        /// Shell thickness (model units).
        thickness: f64,
    },
    /// Merge vertices within `distance` — forwards to [`crate::weld::weld`].
    Weld {
        /// Merge distance.
        distance: f64,
    },
    /// Replace the surface with a wireframe of beams along its edges.
    Wireframe {
        /// Beam thickness as a fraction of the local face size.
        thickness: f64,
    },
    /// Split the mesh along edges sharper than `angle` radians — forwards to
    /// [`crate::edge_tools::edge_split`].
    EdgeSplit {
        /// Dihedral angle above which an edge is split.
        angle: f64,
    },
    /// Fan-triangulate every face — forwards to
    /// [`crate::triangulate::triangulate`].
    Triangulate,
    /// Keep only faces all of whose vertices are in `keep`; `invert` keeps the
    /// complement instead.
    Mask {
        /// Vertices whose fully-covered faces survive.
        keep: std::sync::Arc<Vec<crate::mesh::VertexId>>,
        /// Keep the complement.
        invert: bool,
    },
    /// Voxel remesh — rasterise and re-surface at `voxel_size`, then
    /// `smooth_iterations` Laplacian passes (GH issue #37 §F, `op-hzs.54.30`).
    Remesh {
        /// Voxel edge length.
        voxel_size: f64,
        /// Smoothing passes (`0` = blocky).
        smooth_iterations: u32,
    },
    /// Screw: revolve the whole mesh's profile around `axis`, `turns` turns,
    /// `steps` steps, advancing `screw_offset` along the axis (a helix).
    Screw {
        /// Rotation axis.
        axis: crate::selection::Axis,
        /// Number of full revolutions.
        turns: f64,
        /// Steps per revolution.
        steps: usize,
        /// Total axial advance.
        screw_offset: f64,
    },
    /// Skin: a rectangular tube of half-width `radius` along every edge.
    Skin {
        /// Tube half-width.
        radius: f64,
    },
    /// Build: reveal only the first `factor` fraction of the faces
    /// (`0.0` … `1.0`).
    Build {
        /// Fraction of faces to keep.
        factor: f64,
    },
    /// Multiresolution: `levels` of Catmull-Clark subdivision (the "simple"
    /// flavour; forwards to [`crate::subdivision::catmull_clark`]).
    Multires {
        /// Subdivision levels.
        levels: u32,
    },
    /// Decimate: reduce to `ratio` of the current face count (QEM collapse).
    Decimate {
        /// Target face fraction (`0.0` … `1.0`).
        ratio: f64,
    },
    /// Simple Deform (twist / bend / taper / stretch) along an axis (GH issue
    /// #37 §F, `op-hzs.54.31`).
    SimpleDeform {
        /// The deform mode + amount.
        mode: crate::deform::SimpleDeform,
        /// Axis the deform is parameterised along.
        axis: crate::selection::Axis,
    },
    /// Cast toward a sphere / cylinder / cuboid.
    Cast {
        /// The shape to cast toward.
        target: crate::deform::CastTarget,
        /// `0` = unchanged, `1` = fully on the target.
        factor: f64,
    },
    /// Displace along `direction` by value noise (a texture-free stand-in).
    Displace {
        /// Displacement direction.
        direction: Vec3,
        /// Displacement amplitude.
        strength: f64,
        /// Noise frequency.
        noise_scale: f64,
        /// Noise seed.
        seed: u64,
    },
    /// Wave: a travelling sine ripple along `axis`.
    Wave {
        /// Ripple axis.
        axis: crate::selection::Axis,
        /// Peak displacement.
        amplitude: f64,
        /// Wave period.
        wavelength: f64,
        /// Travel speed.
        speed: f64,
        /// Evaluation time.
        time: f64,
    },
}

/// Epsilon (dimensionless model-space length) below which two vertex positions
/// are treated as coincident and welded by [`Modifier::Mirror`].
const WELD_EPS: f64 = 1e-9;

impl Modifier {
    /// Evaluate this modifier against `input`, returning the derived mesh.
    ///
    /// The input is borrowed and never mutated (non-destructive: the base mesh
    /// is preserved); a freshly rebuilt [`Mesh`] is returned. All three
    /// variants are implemented:
    ///
    /// - [`Modifier::Subsurf`] forwards to
    ///   [`crate::subdivision::catmull_clark`].
    /// - [`Modifier::Mirror`] reflects + welds across each enabled axis plane.
    /// - [`Modifier::Array`] tiles the mesh with a relative per-copy offset.
    ///
    /// Returns [`Ok`] in all current cases; the [`Result`] is kept so future
    /// stub variants can surface [`ModifierError::NotImplemented`].
    pub fn evaluate(&self, input: &Mesh) -> Result<Mesh, ModifierError> {
        match self {
            Modifier::Subsurf { levels } => Ok(crate::subdivision::catmull_clark(input, *levels)),
            Modifier::Mirror { axes } => Ok(mirror(input, *axes)),
            Modifier::Array { count, offset } => Ok(array(input, *count, *offset)),
            Modifier::Bevel { options } => Ok(crate::bevel::bevel(input, *options)),
            Modifier::Boolean { operand, op } => {
                crate::boolean::boolean(input, operand, *op).map_err(|e| ModifierError::Failed(e.to_string()))
            }
            Modifier::Solidify { thickness } => Ok(crate::solidify::solidify(input, *thickness)),
            Modifier::Weld { distance } => Ok(crate::weld::weld(input, *distance)),
            Modifier::Wireframe { thickness } => Ok(wireframe(input, *thickness)),
            Modifier::EdgeSplit { angle } => {
                let sharp = crate::measure::sharp_edges(input, *angle);
                Ok(crate::edge_tools::edge_split(input, &sharp))
            }
            Modifier::Triangulate => Ok(crate::triangulate::triangulate(input)),
            Modifier::Mask { keep, invert } => Ok(mask(input, keep, *invert)),
            Modifier::Remesh { voxel_size, smooth_iterations } => {
                Ok(crate::remesh::voxel_remesh(input, *voxel_size, *smooth_iterations))
            }
            Modifier::Screw { axis, turns, steps, screw_offset } => {
                let profile = screw_profile(input);
                Ok(crate::spin_screw::screw(input, &profile, Vec3::ZERO, *axis, *screw_offset, *turns, *steps))
            }
            Modifier::Skin { radius } => Ok(skin(input, *radius)),
            Modifier::Build { factor } => Ok(build(input, *factor)),
            Modifier::Multires { levels } => Ok(crate::subdivision::catmull_clark(input, *levels)),
            Modifier::Decimate { ratio } => {
                let target = ((input.face_count() as f64) * ratio.clamp(0.0, 1.0)).round() as usize;
                Ok(crate::decimate::decimate(input, target.max(1)))
            }
            Modifier::SimpleDeform { mode, axis } => {
                Ok(crate::deform::simple_deform(input, &[], *mode, *axis))
            }
            Modifier::Cast { target, factor } => Ok(crate::deform::cast(input, &[], *target, *factor)),
            Modifier::Displace { direction, strength, noise_scale, seed } => {
                Ok(crate::deform::displace(input, &[], *direction, *strength, *noise_scale, *seed))
            }
            Modifier::Wave { axis, amplitude, wavelength, speed, time } => {
                Ok(crate::deform::wave(input, &[], *axis, *amplitude, *wavelength, *speed, *time))
            }
        }
    }
}

/// The profile the Screw modifier revolves: the ordered boundary vertex ring
/// if the mesh is open, otherwise every vertex in id order.
fn screw_profile(mesh: &Mesh) -> Vec<crate::mesh::VertexId> {
    let topo = crate::topology::MeshTopology::new(mesh);
    let boundary: Vec<crate::mesh::EdgeId> = (0..mesh.edge_count())
        .map(crate::mesh::EdgeId)
        .filter(|&e| topo.is_boundary_edge(e))
        .collect();
    if let Some((ring, _)) = crate::bridge::ordered_ring(mesh, &boundary) {
        if ring.len() >= 2 {
            return ring;
        }
    }
    (0..mesh.vertex_count()).map(crate::mesh::VertexId).collect()
}

/// Skin: a square tube of half-width `radius` along every edge.
fn skin(input: &Mesh, radius: f64) -> Mesh {
    let r = radius.max(1e-4);
    let src = input.positions();
    let mut positions: Vec<Vec3> = Vec::new();
    let mut faces: Vec<Vec<usize>> = Vec::new();
    for e in 0..input.edge_count() {
        let Some(ed) = input.edge(crate::mesh::EdgeId(e)) else { continue };
        let a = src[ed.verts[0].0];
        let b = src[ed.verts[1].0];
        let axis = b.sub(a);
        if axis.length() < 1e-9 {
            continue;
        }
        let ax = axis.normalize();
        let up = if ax.z.abs() < 0.9 { Vec3::new(0.0, 0.0, 1.0) } else { Vec3::new(1.0, 0.0, 0.0) };
        let u = up.cross(ax).normalize().scale(r);
        let v = ax.cross(u.normalize()).scale(r);
        let ring = |c: Vec3, positions: &mut Vec<Vec3>| -> [usize; 4] {
            let base = positions.len();
            positions.push(c.add(u).add(v));
            positions.push(c.sub(u).add(v));
            positions.push(c.sub(u).sub(v));
            positions.push(c.add(u).sub(v));
            [base, base + 1, base + 2, base + 3]
        };
        let ra = ring(a, &mut positions);
        let rb = ring(b, &mut positions);
        for i in 0..4 {
            let j = (i + 1) % 4;
            faces.push(vec![ra[i], ra[j], rb[j], rb[i]]);
        }
        faces.push(vec![ra[3], ra[2], ra[1], ra[0]]); // cap a
        faces.push(vec![rb[0], rb[1], rb[2], rb[3]]); // cap b
    }
    if faces.is_empty() {
        return input.clone();
    }
    Mesh::from_polygons(&positions, &faces)
}

/// Build: keep only the first `factor` fraction of the faces (id order).
fn build(input: &Mesh, factor: f64) -> Mesh {
    let n = input.face_count();
    let keep = ((n as f64) * factor.clamp(0.0, 1.0)).round() as usize;
    let faces: Vec<Vec<usize>> = input
        .polygons()
        .iter()
        .take(keep)
        .map(|p| p.iter().map(|v| v.0).collect())
        .collect();
    let positions = input.positions();
    let mut used = vec![false; positions.len()];
    for f in &faces {
        for &v in f {
            used[v] = true;
        }
    }
    let mut idx = vec![usize::MAX; positions.len()];
    let mut pos = Vec::new();
    for (i, u) in used.iter().enumerate() {
        if *u {
            idx[i] = pos.len();
            pos.push(positions[i]);
        }
    }
    let f: Vec<Vec<usize>> = faces.iter().map(|face| face.iter().map(|&v| idx[v]).collect()).collect();
    Mesh::from_polygons(&pos, &f)
}

/// Wireframe: inset every face and drop the shrunk interior, leaving a lattice
/// of border strips along the edges.
fn wireframe(input: &Mesh, thickness: f64) -> Mesh {
    let t = thickness.clamp(0.01, 0.9);
    let bbox = crate::measure::dimensions(input);
    let amount = bbox.length() * 0.02 * (t / 0.1).max(0.1);
    let inset = crate::inset::inset_faces(input, amount);
    // `inset_faces` emits, per source face, its shrunk copy plus a ring of
    // bridging quads. The shrunk copies keep the source face's side count and
    // sit first; keep only the ring quads (n-gons of the same side count are
    // the shrunk faces, quads are the ring — but a quad mesh makes both
    // 4-gons, so instead keep everything with a *new* vertex).
    let n_src_verts = input.positions().len();
    let faces: Vec<Vec<usize>> = inset
        .polygons()
        .iter()
        .filter(|p| {
            p.iter().any(|v| v.0 < n_src_verts) && p.iter().any(|v| v.0 >= n_src_verts)
        })
        .map(|p| p.iter().map(|v| v.0).collect())
        .collect();
    if faces.is_empty() {
        return input.clone();
    }
    Mesh::from_polygons(&inset.positions(), &faces)
}

/// Mask: keep faces whose vertices are all in `keep` (or the complement).
fn mask(input: &Mesh, keep: &[crate::mesh::VertexId], invert: bool) -> Mesh {
    let set: std::collections::BTreeSet<usize> = keep.iter().map(|v| v.0).collect();
    let faces: Vec<Vec<usize>> = input
        .polygons()
        .iter()
        .filter(|p| {
            let covered = p.iter().all(|v| set.contains(&v.0));
            covered != invert
        })
        .map(|p| p.iter().map(|v| v.0).collect())
        .collect();
    // Compact unreferenced verts.
    let positions = input.positions();
    let mut used = vec![false; positions.len()];
    for f in &faces {
        for &v in f {
            used[v] = true;
        }
    }
    let mut idx = vec![usize::MAX; positions.len()];
    let mut pos = Vec::new();
    for (i, u) in used.iter().enumerate() {
        if *u {
            idx[i] = pos.len();
            pos.push(positions[i]);
        }
    }
    let f: Vec<Vec<usize>> = faces.iter().map(|face| face.iter().map(|&v| idx[v]).collect()).collect();
    Mesh::from_polygons(&pos, &f)
}

/// Reflect one component of `p` across the origin plane for `axis`
/// (`0 = X`, `1 = Y`, `2 = Z`).
fn reflect_component(p: Vec3, axis: usize) -> Vec3 {
    match axis {
        0 => Vec3::new(-p.x, p.y, p.z),
        1 => Vec3::new(p.x, -p.y, p.z),
        _ => Vec3::new(p.x, p.y, -p.z),
    }
}

/// The polygon-soup view of `mesh` with faces as plain `usize` index rings.
fn soup(mesh: &Mesh) -> (Vec<Vec3>, Vec<Vec<usize>>) {
    let positions = mesh.positions();
    let polygons: Vec<Vec<usize>> = mesh
        .polygons()
        .iter()
        .map(|f| f.iter().map(|v| v.0).collect())
        .collect();
    (positions, polygons)
}

/// Weld coincident vertices (within `eps`) in a polygon soup.
///
/// Returns a deduplicated position list and the faces re-indexed onto it. Two
/// positions weld when their Euclidean separation is `<= eps`. This is the
/// O(n^2) linear-scan dedup matching [`Mesh::add_face`]'s own edge dedup style;
/// it is fine at the mesh sizes this crate authors.
fn weld(positions: &[Vec3], faces: &[Vec<usize>], eps: f64) -> (Vec<Vec3>, Vec<Vec<usize>>) {
    let mut unique: Vec<Vec3> = Vec::new();
    let mut remap: Vec<usize> = vec![0; positions.len()];
    for (i, &p) in positions.iter().enumerate() {
        let mut found = None;
        for (j, &q) in unique.iter().enumerate() {
            if p.sub(q).length() <= eps {
                found = Some(j);
                break;
            }
        }
        remap[i] = match found {
            Some(j) => j,
            None => {
                unique.push(p);
                unique.len() - 1
            }
        };
    }
    let new_faces: Vec<Vec<usize>> = faces
        .iter()
        .map(|f| f.iter().map(|&idx| remap[idx]).collect())
        .collect();
    (unique, new_faces)
}

/// Mirror a polygon soup once across the plane orthogonal to `axis`.
///
/// Builds a reflected copy (component `axis` negated), reverses each reflected
/// face's winding — reflection flips orientation, so reversing keeps the
/// outward normal pointing outward — concatenates original + reflected, then
/// welds coincident vertices so the seam on the mirror plane is a single shared
/// vertex ring rather than a doubled one.
fn mirror_once(
    positions: &[Vec3],
    faces: &[Vec<usize>],
    axis: usize,
    eps: f64,
) -> (Vec<Vec3>, Vec<Vec<usize>>) {
    let n = positions.len();
    let mut out_pos: Vec<Vec3> = positions.to_vec();
    out_pos.extend(positions.iter().map(|&p| reflect_component(p, axis)));

    let mut out_faces: Vec<Vec<usize>> = faces.to_vec();
    for f in faces {
        // Reverse winding and offset onto the reflected vertex block.
        let rf: Vec<usize> = f.iter().rev().map(|&i| i + n).collect();
        out_faces.push(rf);
    }

    weld(&out_pos, &out_faces, eps)
}

/// Evaluate [`Modifier::Mirror`]: mirror `input` across each enabled axis in
/// turn (X, then Y, then Z), welding each seam. With no axis enabled the mesh
/// is rebuilt unchanged.
fn mirror(input: &Mesh, axes: MirrorAxes) -> Mesh {
    let (mut positions, mut polygons) = soup(input);
    if axes.x {
        let (p, f) = mirror_once(&positions, &polygons, 0, WELD_EPS);
        positions = p;
        polygons = f;
    }
    if axes.y {
        let (p, f) = mirror_once(&positions, &polygons, 1, WELD_EPS);
        positions = p;
        polygons = f;
    }
    if axes.z {
        let (p, f) = mirror_once(&positions, &polygons, 2, WELD_EPS);
        positions = p;
        polygons = f;
    }
    Mesh::from_polygons(&positions, &polygons)
}

/// Evaluate [`Modifier::Array`]: tile `input` `count` times with a relative
/// per-copy offset.
///
/// `count == 0` or `count == 1` returns a single copy of the input. For
/// `count >= 2`, copy `k` (0-based) is translated by
/// `k * offset[axis] * bbox_size[axis]` along each axis, where `bbox_size` is
/// the input's bounding-box extent. Copies are concatenated **without** a weld
/// — abutting copies keep their own coincident vertices (Blender's "Merge" is
/// off by default), so the vertex/face counts are exactly `count` times the
/// input's.
fn array(input: &Mesh, count: u32, offset: [f64; 3]) -> Mesh {
    let (positions, polygons) = soup(input);
    let n = positions.len();
    if n == 0 {
        return input.clone();
    }
    let copies = count.max(1) as usize;

    // Axis-aligned bounding-box extent of the input.
    let mut min = positions[0];
    let mut max = positions[0];
    for &p in &positions {
        min = Vec3::new(min.x.min(p.x), min.y.min(p.y), min.z.min(p.z));
        max = Vec3::new(max.x.max(p.x), max.y.max(p.y), max.z.max(p.z));
    }
    let size = [max.x - min.x, max.y - min.y, max.z - min.z];

    let mut out_pos: Vec<Vec3> = Vec::with_capacity(n * copies);
    let mut out_faces: Vec<Vec<usize>> = Vec::with_capacity(polygons.len() * copies);
    for k in 0..copies {
        let kf = k as f64;
        let t = Vec3::new(
            kf * offset[0] * size[0],
            kf * offset[1] * size[1],
            kf * offset[2] * size[2],
        );
        for &p in &positions {
            out_pos.push(p.add(t));
        }
        for f in &polygons {
            out_faces.push(f.iter().map(|&i| i + k * n).collect());
        }
    }
    Mesh::from_polygons(&out_pos, &out_faces)
}

/// An ordered, non-destructive stack of [`Modifier`]s applied to a base mesh.
///
/// Mirrors Blender's per-object modifier stack: the base mesh is kept, and
/// [`ModifierStack::evaluate`] folds each modifier in order to produce the
/// final derived mesh.
#[derive(Debug, Clone, Default)]
pub struct ModifierStack {
    /// The modifiers, evaluated first-to-last (top-to-bottom in Blender's UI).
    pub modifiers: Vec<Modifier>,
}

impl ModifierStack {
    /// Create an empty stack (evaluates to the input mesh unchanged).
    pub fn new() -> Self {
        ModifierStack::default()
    }

    /// Append a modifier to the end of the stack (builder style).
    pub fn push(mut self, m: Modifier) -> Self {
        self.modifiers.push(m);
        self
    }

    /// Evaluate the whole stack against `base`.
    ///
    /// An empty stack clones `base` unchanged. Otherwise each [`Modifier`] is
    /// folded in order — the output mesh of one becomes the input of the next —
    /// so the composed geometry (e.g. mirror-then-array) is produced in a
    /// single pass. Propagates any [`ModifierError`] from a modifier.
    pub fn evaluate(&self, base: &Mesh) -> Result<Mesh, ModifierError> {
        let mut current = base.clone();
        for m in &self.modifiers {
            current = m.evaluate(&current)?;
        }
        Ok(current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Vec3;
    use crate::mesh::Mesh;
    use crate::primitives;

    /// Empty stack is the identity: counts match the base mesh exactly.
    #[test]
    fn empty_stack_is_identity() {
        let base = primitives::cube(1.0);
        let out = ModifierStack::new().evaluate(&base).unwrap();
        assert_eq!(out.vertex_count(), base.vertex_count());
        assert_eq!(out.face_count(), base.face_count());
    }

    /// A non-empty stack now produces real geometry (replaces the former
    /// "reports NotImplemented" scaffold test).
    ///
    /// Methodology: push a single `Subsurf { levels: 1 }` onto a stack and
    /// evaluate it against a unit cube; the stack must forward to
    /// [`crate::subdivision::catmull_clark`] and yield its result.
    /// Result: one Catmull-Clark level of a cube gives V = 8 + 6 face-points +
    /// 12 edge-points = 26 vertices and 6 * 4 = 24 quad faces.
    #[test]
    fn nonempty_stack_produces_geometry() {
        let base = primitives::cube(1.0);
        let stack = ModifierStack::new().push(Modifier::Subsurf { levels: 1 });
        let out = stack.evaluate(&base).unwrap();
        assert_eq!(out.vertex_count(), 26);
        assert_eq!(out.face_count(), 24);
    }

    /// Mirror welds the seam: a quad spanning x in [0, 2], mirrored across X,
    /// welds its two x = 0 vertices with their reflections.
    ///
    /// Methodology: take `grid(1, 1, 2.0)` (a single quad spanning [-1, 1] on
    /// X and Y), shift every vertex +1 in X so it spans [0, 2] on X, then apply
    /// `Mirror { x }`. The two vertices on the mirror plane (x = 0) coincide
    /// with their reflections and must weld; the two vertices at x = 2 reflect
    /// to two new vertices at x = -2.
    ///
    /// Result (exact integer topology): 4 input vertices produce a welded union
    /// of V = 6 (not the un-welded 8): 2 shared on the seam + 2 at x = 2 + 2 at
    /// x = -2. F = 2 (original quad + reflected quad). The union spans
    /// [-2, 2] on X.
    #[test]
    fn mirror_grid_welds_seam() {
        let g = primitives::grid(1, 1, 2.0);
        // Shift +1 in X: original spans [-1, 1] -> now [0, 2].
        let shifted: Vec<Vec3> = g
            .positions()
            .iter()
            .map(|p| Vec3::new(p.x + 1.0, p.y, p.z))
            .collect();
        let faces: Vec<Vec<usize>> = g
            .polygons()
            .iter()
            .map(|f| f.iter().map(|v| v.0).collect())
            .collect();
        let quad = Mesh::from_polygons(&shifted, &faces);
        assert_eq!(quad.vertex_count(), 4);

        let out = Modifier::Mirror {
            axes: MirrorAxes {
                x: true,
                y: false,
                z: false,
            },
        }
        .evaluate(&quad)
        .unwrap();

        assert_eq!(out.vertex_count(), 6, "seam x=0 pair must weld (8 -> 6)");
        assert_eq!(out.face_count(), 2);

        let xs: Vec<f64> = out.positions().iter().map(|p| p.x).collect();
        let minx = xs.iter().cloned().fold(f64::INFINITY, f64::min);
        let maxx = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        assert!((minx - (-2.0)).abs() < 1e-12, "min x = {minx}, want -2");
        assert!((maxx - 2.0).abs() < 1e-12, "max x = {maxx}, want 2");
    }

    /// Array tiles the mesh with a relative offset; copies are not welded.
    ///
    /// Methodology: apply `Array { count: 3, offset: [1, 0, 0] }` to `cube(1.0)`
    /// (8 vertices, 6 faces, X-extent 1.0 spanning [-0.5, 0.5]). Relative
    /// offset 1.0 in X means copy k is translated by k * 1.0 * 1.0 = k in X.
    ///
    /// Result (exact integer topology, no weld): V = 3 * 8 = 24, F = 3 * 6 = 18.
    /// Copy k occupies vertex indices [8k, 8k+8); vertex 0 of copies 0, 1, 2 is
    /// at x = -0.5, 0.5, 1.5 respectively — spacing 1.0, equal to the cube's
    /// X-extent.
    #[test]
    fn array_cube_relative_offset() {
        let c = primitives::cube(1.0);
        let out = Modifier::Array {
            count: 3,
            offset: [1.0, 0.0, 0.0],
        }
        .evaluate(&c)
        .unwrap();
        assert_eq!(out.vertex_count(), 24, "3 x 8, copies not welded");
        assert_eq!(out.face_count(), 18, "3 x 6");

        let pos = out.positions();
        // Vertex 0 of the cube is (-0.5, -0.5, -0.5); copies are 8 verts apart.
        assert!((pos[0].x - (-0.5)).abs() < 1e-12, "copy 0 x = {}", pos[0].x);
        assert!((pos[8].x - 0.5).abs() < 1e-12, "copy 1 x = {}", pos[8].x);
        assert!((pos[16].x - 1.5).abs() < 1e-12, "copy 2 x = {}", pos[16].x);
    }

    /// Subsurf forwards to Catmull-Clark.
    ///
    /// Methodology: apply `Subsurf { levels: 1 }` to `cube(1.0)`; it must return
    /// exactly `catmull_clark(cube, 1)`.
    /// Result (analytic Catmull-Clark of a cube, one level): V = 26 (8 corner +
    /// 6 face + 12 edge points), F = 24 (each of 6 quads -> 4), and chi =
    /// V - E + F = 26 - 48 + 24 = 2 (a closed genus-0 surface).
    #[test]
    fn subsurf_cube_level1_forwards_to_catmull_clark() {
        let c = primitives::cube(1.0);
        let out = Modifier::Subsurf { levels: 1 }.evaluate(&c).unwrap();
        assert_eq!(out.vertex_count(), 26);
        assert_eq!(out.face_count(), 24);
        assert_eq!(out.euler_characteristic(), 2);
    }

    #[test]
    fn generate_modifiers_pt1_forward_to_their_operators() {
        let c = primitives::cube(2.0);

        let bevel = Modifier::Bevel { options: crate::bevel::BevelOptions { amount: 0.2, ..Default::default() } }
            .evaluate(&c)
            .unwrap();
        assert!(bevel.face_count() > 6);

        let solid = Modifier::Solidify { thickness: 0.3 }.evaluate(&c).unwrap();
        assert!(solid.face_count() > 6);

        let weld = Modifier::Weld { distance: 0.01 }.evaluate(&c).unwrap();
        assert_eq!(weld.vertex_count(), 8);

        let tri = Modifier::Triangulate.evaluate(&c).unwrap();
        assert_eq!(tri.face_count(), 12);

        let split = Modifier::EdgeSplit { angle: std::f64::consts::FRAC_PI_4 }.evaluate(&c).unwrap();
        assert!(split.vertex_count() >= 8);

        let wire = Modifier::Wireframe { thickness: 0.2 }.evaluate(&c).unwrap();
        assert!(wire.face_count() > 0 && wire.face_count() != 6, "not the solid faces");
    }

    #[test]
    fn boolean_modifier_unions_two_cubes() {
        let a = primitives::cube(2.0);
        let mut b = primitives::cube(2.0);
        // Shift b so it overlaps a with no coplanar faces (the boolean impl
        // rejects coplanar operand faces).
        let mut p = b.positions();
        for q in &mut p {
            q.x += 1.3;
            q.y += 0.4;
            q.z += 0.4;
        }
        b = Mesh::from_polygons(&p, &b.polygons().iter().map(|f| f.iter().map(|v| v.0).collect()).collect::<Vec<_>>());
        let out = Modifier::Boolean {
            operand: std::sync::Arc::new(b),
            op: crate::ops::BooleanMode::Union,
        }
        .evaluate(&a)
        .unwrap();
        assert!(out.face_count() > 0);
    }

    #[test]
    fn generate_modifiers_pt2() {
        let c = primitives::cube(2.0);

        let re = Modifier::Remesh { voxel_size: 0.7, smooth_iterations: 0 }.evaluate(&c).unwrap();
        assert_eq!(re.euler_characteristic(), 2);

        let dec = Modifier::Decimate { ratio: 0.5 }.evaluate(&primitives::uv_sphere(20, 12, 1.0)).unwrap();
        assert!(dec.face_count() < primitives::uv_sphere(20, 12, 1.0).face_count());

        let mr = Modifier::Multires { levels: 1 }.evaluate(&c).unwrap();
        assert_eq!(mr.face_count(), 24);

        let bd = Modifier::Build { factor: 0.5 }.evaluate(&c).unwrap();
        assert_eq!(bd.face_count(), 3);

        let sk = Modifier::Skin { radius: 0.1 }.evaluate(&c).unwrap();
        assert!(sk.face_count() > 12, "a tube per cube edge");

        let sc = Modifier::Screw {
            axis: crate::selection::Axis::Z,
            turns: 1.0,
            steps: 8,
            screw_offset: 2.0,
        }
        .evaluate(&primitives::grid(1, 3, 2.0))
        .unwrap();
        assert!(sc.face_count() > 3);
    }

    #[test]
    fn deform_modifiers_pt1() {
        let c = primitives::cube(2.0);
        assert!(Modifier::SimpleDeform {
            mode: crate::deform::SimpleDeform::Twist(std::f64::consts::PI),
            axis: crate::selection::Axis::Z,
        }
        .evaluate(&c)
        .unwrap()
        .face_count()
            == 6);

        let cast = Modifier::Cast { target: crate::deform::CastTarget::Sphere(2.0), factor: 1.0 }
            .evaluate(&primitives::grid(4, 4, 4.0))
            .unwrap();
        assert!(cast.face_count() > 0);

        let disp = Modifier::Displace {
            direction: Vec3::new(0.0, 0.0, 1.0),
            strength: 0.3,
            noise_scale: 0.5,
            seed: 1,
        }
        .evaluate(&primitives::grid(5, 5, 5.0))
        .unwrap();
        assert!((0..disp.vertex_count()).any(|i| disp.vertex(crate::mesh::VertexId(i)).unwrap().position.z.abs() > 1e-6));

        let wv = Modifier::Wave {
            axis: crate::selection::Axis::Z,
            amplitude: 0.4,
            wavelength: 3.0,
            speed: 0.0,
            time: 0.0,
        }
        .evaluate(&primitives::grid(8, 8, 8.0))
        .unwrap();
        assert!(wv.face_count() > 0);
    }

    #[test]
    fn mask_modifier_keeps_only_covered_faces() {
        let m = primitives::cube(2.0);
        // Keep the 4 verts of face 0.
        let keep: Vec<crate::mesh::VertexId> = m.face_vertices(crate::mesh::FaceId(0));
        let out = Modifier::Mask { keep: std::sync::Arc::new(keep), invert: false }.evaluate(&m).unwrap();
        assert_eq!(out.face_count(), 1);
        let inv = Modifier::Mask {
            keep: std::sync::Arc::new(m.face_vertices(crate::mesh::FaceId(0))),
            invert: true,
        }
        .evaluate(&m)
        .unwrap();
        assert_eq!(inv.face_count(), 5);
    }

    /// A two-modifier stack composes: Mirror then Array.
    ///
    /// Methodology: start from the shifted quad spanning [0, 2] on X (4 verts,
    /// 1 face), stack `Mirror { x }` then `Array { count: 2, offset: [1, 0, 0] }`.
    /// Mirror yields the welded union spanning [-2, 2] on X (6 verts, 2 faces,
    /// X-extent 4). Array then makes 2 un-welded copies offset by
    /// 1 * 4 = 4 in X.
    ///
    /// Result (exact integer topology): V = 2 * 6 = 12, F = 2 * 2 = 4. The
    /// second copy is shifted +4 in X relative to the first.
    #[test]
    fn stack_mirror_then_array_composes() {
        let g = primitives::grid(1, 1, 2.0);
        let shifted: Vec<Vec3> = g
            .positions()
            .iter()
            .map(|p| Vec3::new(p.x + 1.0, p.y, p.z))
            .collect();
        let faces: Vec<Vec<usize>> = g
            .polygons()
            .iter()
            .map(|f| f.iter().map(|v| v.0).collect())
            .collect();
        let quad = Mesh::from_polygons(&shifted, &faces);

        let stack = ModifierStack::new()
            .push(Modifier::Mirror {
                axes: MirrorAxes {
                    x: true,
                    y: false,
                    z: false,
                },
            })
            .push(Modifier::Array {
                count: 2,
                offset: [1.0, 0.0, 0.0],
            });
        let out = stack.evaluate(&quad).unwrap();

        assert_eq!(out.vertex_count(), 12, "mirror -> 6 verts, x2 array -> 12");
        assert_eq!(out.face_count(), 4, "mirror -> 2 faces, x2 array -> 4");

        let xs: Vec<f64> = out.positions().iter().map(|p| p.x).collect();
        let minx = xs.iter().cloned().fold(f64::INFINITY, f64::min);
        let maxx = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        // Copy 0 spans [-2, 2], copy 1 spans [2, 6] -> union [-2, 6].
        assert!((minx - (-2.0)).abs() < 1e-12, "min x = {minx}");
        assert!((maxx - 6.0).abs() < 1e-12, "max x = {maxx}");
    }
}
