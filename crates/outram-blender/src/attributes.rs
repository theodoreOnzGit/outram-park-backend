// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Per-element mesh attributes. Follows the published behaviour of Blender's
// edit-mode marks and the CustomData layers behind them (source/blender/
// editors/mesh/editmesh_tools.cc `MESH_OT_mark_*`, `MESH_OT_faces_shade_*`,
// `MESH_OT_edge_crease`, `MESH_OT_edge_bevelweight`, and the auto-smooth
// angle, github.com/blender/blender, GPL-2.0-or-later): sharp / seam marks,
// edge crease and bevel weight, per-face smooth shading and material index.
// Concepts only — no upstream source copied.
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

//! **Marks & surface attributes** (`op-hzs.54.28`, GH issue #37 §E) — the
//! per-element attribute layer that `select_linked` delimiters (`op-hzs.54.2`),
//! `tris_to_quads` comparisons (`op-hzs.54.17`), `edge_split` (`op-hzs.54.18`),
//! `bevel`'s mark-seam/sharp (`op-hzs.54.9`) and `separate_by_material`
//! (`op-hzs.54.12`) were all deferred to.
//!
//! [`MeshAttributes`] holds:
//!
//! - `sharp` / `seam` — `BTreeSet<EdgeId>` marks.
//! - `crease` / `edge_bevel_weight` / `vertex_bevel_weight` — `f64` in `[0, 1]`.
//! - `smooth` — `BTreeSet<FaceId>` (shade-smooth; default is flat).
//! - `material` — per-face `usize` index (default `0`).
//! - `freestyle_edge` / `freestyle_face` — marks.
//!
//! The keys are mesh indices, so a `MeshAttributes` goes stale when topology
//! changes exactly like any [`crate::mesh::VertexId`] — rebuild it alongside
//! the mesh.
//!
//! [`MeshAttributes::auto_smooth`] derives `smooth` + `sharp` from the dihedral
//! angle (Blender's auto-smooth). [`MeshAttributes::linked_delimiters`] gives
//! the edge set `select_linked` should not cross.

use std::collections::{BTreeSet, HashMap};

use crate::mesh::{EdgeId, FaceId, Mesh, VertexId};

/// The full per-element attribute layer for one mesh.
#[derive(Debug, Clone, Default)]
pub struct MeshAttributes {
    /// Edges marked sharp (a hard shading edge; also a bevel/split seed).
    pub sharp: BTreeSet<EdgeId>,
    /// Edges marked as a UV seam.
    pub seam: BTreeSet<EdgeId>,
    /// Subdivision-surface crease per edge, `[0, 1]`.
    pub crease: HashMap<EdgeId, f64>,
    /// Bevel-modifier weight per edge, `[0, 1]`.
    pub edge_bevel_weight: HashMap<EdgeId, f64>,
    /// Bevel-modifier weight per vertex, `[0, 1]`.
    pub vertex_bevel_weight: HashMap<VertexId, f64>,
    /// Faces shaded smooth (default: flat).
    pub smooth: BTreeSet<FaceId>,
    /// Per-face material index (default: `0`).
    pub material: HashMap<FaceId, usize>,
    /// Edges marked as a Freestyle edge.
    pub freestyle_edge: BTreeSet<EdgeId>,
    /// Faces marked as a Freestyle face.
    pub freestyle_face: BTreeSet<FaceId>,
}

impl MeshAttributes {
    /// A fresh, empty attribute layer for `mesh` (everything at its default).
    pub fn new(_mesh: &Mesh) -> Self {
        MeshAttributes::default()
    }

    // -- sharp / seam / freestyle --------------------------------------------

    /// Mark `edges` sharp.
    pub fn mark_sharp(&mut self, edges: &[EdgeId]) {
        self.sharp.extend(edges.iter().copied());
    }
    /// Clear the sharp mark from `edges`.
    pub fn clear_sharp(&mut self, edges: &[EdgeId]) {
        for e in edges {
            self.sharp.remove(e);
        }
    }
    /// Whether `e` is sharp.
    pub fn is_sharp(&self, e: EdgeId) -> bool {
        self.sharp.contains(&e)
    }

    /// Mark `edges` as a seam.
    pub fn mark_seam(&mut self, edges: &[EdgeId]) {
        self.seam.extend(edges.iter().copied());
    }
    /// Clear the seam mark from `edges`.
    pub fn clear_seam(&mut self, edges: &[EdgeId]) {
        for e in edges {
            self.seam.remove(e);
        }
    }
    /// Whether `e` is a seam.
    pub fn is_seam(&self, e: EdgeId) -> bool {
        self.seam.contains(&e)
    }

    // -- crease / bevel weight ---------------------------------------------

    /// Set the crease of `edges` to `value` (clamped to `[0, 1]`; `0` removes
    /// the entry).
    pub fn set_crease(&mut self, edges: &[EdgeId], value: f64) {
        let v = value.clamp(0.0, 1.0);
        for &e in edges {
            if v == 0.0 {
                self.crease.remove(&e);
            } else {
                self.crease.insert(e, v);
            }
        }
    }
    /// The crease of `e` (`0.0` if unset).
    pub fn crease(&self, e: EdgeId) -> f64 {
        self.crease.get(&e).copied().unwrap_or(0.0)
    }

    /// Set the bevel weight of `edges` to `value` (clamped, `0` removes).
    pub fn set_edge_bevel_weight(&mut self, edges: &[EdgeId], value: f64) {
        let v = value.clamp(0.0, 1.0);
        for &e in edges {
            if v == 0.0 {
                self.edge_bevel_weight.remove(&e);
            } else {
                self.edge_bevel_weight.insert(e, v);
            }
        }
    }
    /// The bevel weight of `e` (`0.0` if unset).
    pub fn edge_bevel_weight(&self, e: EdgeId) -> f64 {
        self.edge_bevel_weight.get(&e).copied().unwrap_or(0.0)
    }

    // -- shading / material -----------------------------------------------

    /// Shade `faces` smooth (empty = whole mesh).
    pub fn shade_smooth(&mut self, mesh: &Mesh, faces: &[FaceId]) {
        let set = self.face_set(mesh, faces);
        self.smooth.extend(set);
    }
    /// Shade `faces` flat (empty = whole mesh).
    pub fn shade_flat(&mut self, mesh: &Mesh, faces: &[FaceId]) {
        for f in self.face_set(mesh, faces) {
            self.smooth.remove(&f);
        }
    }
    /// Whether `f` is shaded smooth.
    pub fn is_smooth(&self, f: FaceId) -> bool {
        self.smooth.contains(&f)
    }

    /// Set the material index of `faces`.
    pub fn set_material(&mut self, faces: &[FaceId], index: usize) {
        for &f in faces {
            self.material.insert(f, index);
        }
    }
    /// The material index of `f` (`0` if unset).
    pub fn material(&self, f: FaceId) -> usize {
        self.material.get(&f).copied().unwrap_or(0)
    }

    // -- derived ---------------------------------------------------------

    /// Auto-smooth: every face becomes smooth, and every interior edge whose
    /// dihedral angle exceeds `angle` radians is marked sharp (so a shading
    /// pass splits the normal there). Blender's *Shade Auto Smooth*.
    pub fn auto_smooth(&mut self, mesh: &Mesh, angle: f64) {
        self.smooth = (0..mesh.face_count()).map(FaceId).collect();
        for e in crate::measure::sharp_edges(mesh, angle) {
            self.sharp.insert(e);
        }
    }

    /// The edges `select_linked` must not cross, per the given delimiters
    /// (`op-hzs.54.2`): any combination of seam / sharp / material boundary.
    pub fn linked_delimiters(
        &self,
        mesh: &Mesh,
        by_seam: bool,
        by_sharp: bool,
        by_material: bool,
    ) -> BTreeSet<EdgeId> {
        let mut out = BTreeSet::new();
        if by_seam {
            out.extend(self.seam.iter().copied());
        }
        if by_sharp {
            out.extend(self.sharp.iter().copied());
        }
        if by_material {
            let topo = crate::topology::MeshTopology::new(mesh);
            for e in 0..mesh.edge_count() {
                let f = topo.edge_faces(EdgeId(e));
                if f.len() == 2 && self.material(f[0]) != self.material(f[1]) {
                    out.insert(EdgeId(e));
                }
            }
        }
        out
    }

    fn face_set(&self, mesh: &Mesh, faces: &[FaceId]) -> Vec<FaceId> {
        if faces.is_empty() {
            (0..mesh.face_count()).map(FaceId).collect()
        } else {
            faces.to_vec()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;
    use crate::selection::{SelectMode, Selection};

    #[test]
    fn mark_and_clear_sharp_seam() {
        let m = primitives::cube(2.0);
        let mut a = MeshAttributes::new(&m);
        a.mark_sharp(&[EdgeId(0), EdgeId(1)]);
        a.mark_seam(&[EdgeId(0)]);
        assert!(a.is_sharp(EdgeId(0)) && a.is_sharp(EdgeId(1)));
        assert!(a.is_seam(EdgeId(0)) && !a.is_seam(EdgeId(1)));
        a.clear_sharp(&[EdgeId(0)]);
        assert!(!a.is_sharp(EdgeId(0)) && a.is_sharp(EdgeId(1)));
    }

    #[test]
    fn crease_and_bevel_weight_clamp_and_default() {
        let m = primitives::cube(2.0);
        let mut a = MeshAttributes::new(&m);
        a.set_crease(&[EdgeId(3)], 1.5);
        assert_eq!(a.crease(EdgeId(3)), 1.0);
        assert_eq!(a.crease(EdgeId(4)), 0.0);
        a.set_crease(&[EdgeId(3)], 0.0);
        assert_eq!(a.crease(EdgeId(3)), 0.0, "zero removes the entry");
        a.set_edge_bevel_weight(&[EdgeId(2)], 0.6);
        assert_eq!(a.edge_bevel_weight(EdgeId(2)), 0.6);
    }

    #[test]
    fn shade_smooth_flat_per_selection() {
        let m = primitives::cube(2.0);
        let mut a = MeshAttributes::new(&m);
        a.shade_smooth(&m, &[]); // all
        assert!((0..m.face_count()).all(|f| a.is_smooth(FaceId(f))));
        a.shade_flat(&m, &[FaceId(0)]);
        assert!(!a.is_smooth(FaceId(0)) && a.is_smooth(FaceId(1)));
    }

    #[test]
    fn auto_smooth_marks_sharp_on_a_cube_but_not_a_sphere_band() {
        let cube = primitives::cube(2.0);
        let mut a = MeshAttributes::new(&cube);
        a.auto_smooth(&cube, std::f64::consts::FRAC_PI_4); // 45°
        assert_eq!(a.sharp.len(), 12, "all cube edges are 90° → sharp");
        assert_eq!(a.smooth.len(), 6);

        let sph = primitives::uv_sphere(24, 12, 1.0);
        let mut b = MeshAttributes::new(&sph);
        b.auto_smooth(&sph, std::f64::consts::FRAC_PI_4);
        assert!(b.sharp.len() < sph.edge_count(), "most sphere edges stay smooth");
    }

    #[test]
    fn material_boundary_is_a_linked_delimiter() {
        // Grid split into two material groups; the seam between them delimits.
        let m = primitives::grid(4, 1, 4.0); // 4 faces in a row
        let mut a = MeshAttributes::new(&m);
        a.set_material(&[FaceId(0), FaceId(1)], 0);
        a.set_material(&[FaceId(2), FaceId(3)], 1);
        let delim = a.linked_delimiters(&m, false, false, true);
        assert_eq!(delim.len(), 1, "one edge between material 0 and 1");
    }

    #[test]
    fn seam_delimiter_stops_select_linked() {
        // A 4x1 grid, seam between faces 1 and 2; grabbing face 0 with the seam
        // as a delimiter should not reach faces 2-3.
        let m = primitives::grid(4, 1, 4.0);
        let topo = crate::topology::MeshTopology::new(&m);
        // The shared edge between face 1 and 2.
        let seam_edge = (0..m.edge_count()).map(EdgeId).find(|&e| {
            let f = topo.edge_faces(e);
            f.len() == 2 && ((f[0].0 == 1 && f[1].0 == 2) || (f[0].0 == 2 && f[1].0 == 1))
        }).unwrap();
        let mut a = MeshAttributes::new(&m);
        a.mark_seam(&[seam_edge]);
        let delim = a.linked_delimiters(&m, true, false, false);
        assert!(delim.contains(&seam_edge));

        // Without the delimiter, linked grows to all 4 faces.
        let mut plain = Selection::new(SelectMode::Face);
        plain.select(&m, crate::selection::Element::Face(FaceId(0)));
        plain.select_linked(&m);
        assert_eq!(plain.face_count(), 4);

        // With the seam delimiter, it stops at faces 0-1.
        let mut stopped = Selection::new(SelectMode::Face);
        stopped.select(&m, crate::selection::Element::Face(FaceId(0)));
        stopped.select_linked_delimited(&m, &delim);
        assert_eq!(stopped.face_count(), 2);
    }
}
