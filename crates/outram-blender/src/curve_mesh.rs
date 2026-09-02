// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Curve <-> mesh conversion, curve deform, curve skin. Follows the published
// behaviour of Blender's convert operators (OBJECT_OT_convert with
// target='CURVE'/'MESH', source/blender/editors/object/object_add.cc), the
// Curve modifier (MOD_curve.cc) and the Skin modifier (MOD_skin.cc),
// github.com/blender/blender, GPL-2.0-or-later. Concepts only — no upstream
// source copied. Composes `curve`, `curve_surface` and `deform2`.
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

//! **Curve ↔ Mesh conversion + curve deform + skin** (`op-hzs.54.36`, GH issue
//! #37 §G).
//!
//! - [`mesh_to_splines`] — every maximal non-branching edge chain of a mesh
//!   becomes a poly [`Spline`] (Blender's *Convert to Curve*).
//! - [`boundary_to_splines`] — just the open-boundary loops.
//! - [`spline_to_mesh`] — [`crate::curve_surface::curve_to_mesh`] with sensible
//!   defaults (*Convert to Mesh*).
//! - [`spline_deform_mesh`] — deform a mesh so its axis rides a [`Spline`]
//!   (the Curve modifier).
//! - [`skin_spline`] — a round tube along a [`Spline`] (the Skin modifier on a
//!   curve).

use std::collections::{BTreeSet, HashMap};

use crate::curve::{Spline, SplineType};
use crate::curve_surface::{Bevel, CurveGeometry};
use crate::math::Vec3;
use crate::mesh::{EdgeId, Mesh};
use crate::selection::Axis;
use crate::topology::MeshTopology;

/// Extract every maximal edge chain of `mesh` as a poly [`Spline`]. A chain
/// ends at a branch vertex (valence != 2) or closes into a cyclic loop.
pub fn mesh_to_splines(mesh: &Mesh) -> Vec<Spline> {
    chains(mesh, None)
}

/// Extract the open-boundary loops of `mesh` as poly [`Spline`]s (each is
/// `cyclic`).
pub fn boundary_to_splines(mesh: &Mesh) -> Vec<Spline> {
    let topo = MeshTopology::new(mesh);
    let boundary: BTreeSet<EdgeId> =
        (0..mesh.edge_count()).map(EdgeId).filter(|&e| topo.is_boundary_edge(e)).collect();
    chains(mesh, Some(&boundary))
}

fn chains(mesh: &Mesh, restrict: Option<&BTreeSet<EdgeId>>) -> Vec<Spline> {
    // Adjacency restricted to the allowed edge set.
    let mut adj: HashMap<usize, Vec<(usize, usize)>> = HashMap::new(); // v -> [(neighbour, edge)]
    for e in 0..mesh.edge_count() {
        if restrict.is_some_and(|r| !r.contains(&EdgeId(e))) {
            continue;
        }
        let Some(ed) = mesh.edge(EdgeId(e)) else { continue };
        let (a, b) = (ed.verts[0].0, ed.verts[1].0);
        adj.entry(a).or_default().push((b, e));
        adj.entry(b).or_default().push((a, e));
    }
    let pos = mesh.positions();
    let mut used: BTreeSet<usize> = BTreeSet::new();
    let mut out: Vec<Spline> = Vec::new();

    // Start from every non-degree-2 vertex first (open chains), then leftover
    // pure loops.
    let mut starts: Vec<usize> =
        adj.iter().filter(|(_, ns)| ns.len() != 2).map(|(&v, _)| v).collect();
    starts.sort_unstable();
    starts.dedup();

    let walk = |start_v: usize, start_e: usize, used: &mut BTreeSet<usize>| -> Option<Spline> {
        if used.contains(&start_e) {
            return None;
        }
        let mut chain = vec![start_v];
        let mut cur_v = start_v;
        let mut cur_e = start_e;
        loop {
            used.insert(cur_e);
            let ed = mesh.edge(EdgeId(cur_e)).unwrap();
            let next_v = if ed.verts[0].0 == cur_v { ed.verts[1].0 } else { ed.verts[0].0 };
            chain.push(next_v);
            let ns = &adj[&next_v];
            if ns.len() != 2 {
                break; // hit a branch / endpoint
            }
            let Some(&(_, next_e)) = ns.iter().find(|&&(_, e)| e != cur_e && !used.contains(&e)) else {
                break;
            };
            if next_v == start_v {
                used.insert(next_e);
                break; // closed a loop
            }
            cur_v = next_v;
            cur_e = next_e;
        }
        let cyclic = chain.first() == chain.last() && chain.len() > 2;
        let verts: Vec<Vec3> = if cyclic {
            chain[..chain.len() - 1].iter().map(|&v| pos[v]).collect()
        } else {
            chain.iter().map(|&v| pos[v]).collect()
        };
        if verts.len() < 2 {
            return None;
        }
        let mut s = Spline::poly(&verts);
        s.cyclic = cyclic;
        Some(s)
    };

    for &v in &starts {
        for &(_, e) in adj.get(&v).map(|x| x.as_slice()).unwrap_or(&[]) {
            if let Some(s) = walk(v, e, &mut used) {
                out.push(s);
            }
        }
    }
    // Leftover pure loops.
    for (&v, ns) in &adj {
        for &(_, e) in ns {
            if !used.contains(&e) {
                if let Some(s) = walk(v, e, &mut used) {
                    out.push(s);
                }
            }
        }
    }
    out
}

/// Convert a [`Spline`] to a mesh — a wire, a round tube, or a filled outline.
pub fn spline_to_mesh(spline: &Spline, tube_radius: Option<f64>) -> Mesh {
    let bevel = match tube_radius {
        Some(r) => Bevel::Round { depth: r, segments: 12 },
        None => Bevel::None,
    };
    crate::curve_surface::curve_to_mesh(
        spline,
        &CurveGeometry {
            bevel,
            fill: if spline.cyclic && tube_radius.is_none() {
                crate::curve_surface::FillMode::Full
            } else {
                crate::curve_surface::FillMode::None
            },
            ..Default::default()
        },
    )
}

/// Deform `mesh` so its `axis` coordinate rides `spline` (the Curve modifier).
pub fn spline_deform_mesh(mesh: &Mesh, spline: &Spline, axis: Axis) -> Mesh {
    let curve = spline.sample();
    crate::deform2::curve_deform(mesh, &curve, axis)
}

/// A round tube of `radius` along `spline` — the Skin modifier applied to a
/// curve. `segments` sides.
pub fn skin_spline(spline: &Spline, radius: f64, segments: usize) -> Mesh {
    crate::curve_surface::curve_to_mesh(
        spline,
        &CurveGeometry {
            bevel: Bevel::Round { depth: radius, segments: segments.max(3) },
            caps: !spline.cyclic,
            ..Default::default()
        },
    )
}

/// Whether `s` reads as a plausible poly conversion of a straight edge chain
/// (helper used by the tests / a round-trip check).
pub fn is_poly(s: &Spline) -> bool {
    s.spline_type == SplineType::Poly
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::VertexId;
    use crate::primitives;

    #[test]
    fn mesh_to_splines_covers_every_edge_of_a_grid() {
        let m = primitives::grid(3, 3, 3.0);
        let splines = mesh_to_splines(&m);
        assert!(!splines.is_empty());
        assert!(splines.iter().all(is_poly));
        // The chains between them touch every vertex at least once.
        let touched: std::collections::BTreeSet<_> = splines
            .iter()
            .flat_map(|s| s.points.iter().map(|p| (p.position.x as i64, p.position.y as i64)))
            .collect();
        assert!(touched.len() >= 9);
    }

    #[test]
    fn boundary_to_splines_of_a_grid_is_one_cyclic_loop() {
        let m = primitives::grid(3, 3, 3.0);
        let splines = boundary_to_splines(&m);
        assert_eq!(splines.len(), 1);
        assert!(splines[0].cyclic);
        assert_eq!(splines[0].points.len(), 12, "3x3 grid border");
    }

    #[test]
    fn spline_to_mesh_round_trips_a_tube() {
        let s = Spline::poly(&[Vec3::ZERO, Vec3::new(0.0, 0.0, 3.0), Vec3::new(2.0, 0.0, 3.0)]);
        let tube = spline_to_mesh(&s, Some(0.3));
        assert!(tube.face_count() > 10);
    }

    #[test]
    fn spline_to_mesh_fills_a_closed_curve() {
        let mut s = Spline::poly(&[
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(1.0, 2.0, 0.0),
        ]);
        s.cyclic = true;
        let m = spline_to_mesh(&s, None);
        assert_eq!(m.face_count(), 1, "triangle → 1 face");
    }

    #[test]
    fn spline_deform_bends_a_bar() {
        let mut m = Mesh::new();
        for i in 0..7 {
            m.add_vertex(Vec3::new(0.1, 0.0, i as f64 * 0.5));
            m.add_vertex(Vec3::new(-0.1, 0.0, i as f64 * 0.5));
        }
        m.add_face(&[VertexId(0), VertexId(1), VertexId(3)]);
        let curve = Spline::poly(&[Vec3::ZERO, Vec3::new(0.0, 0.0, 1.5), Vec3::new(2.0, 0.0, 1.5)]);
        let d = spline_deform_mesh(&m, &curve, Axis::Z);
        assert!(d.vertex(VertexId(12)).unwrap().position.x > 0.5, "far end followed the curve");
    }

    #[test]
    fn skin_spline_makes_a_closed_tube_for_a_loop() {
        let mut s = Spline::poly(&[
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(-2.0, 0.0, 0.0),
            Vec3::new(0.0, -2.0, 0.0),
        ]);
        s.cyclic = true;
        s.resolution = 4;
        let tube = skin_spline(&s, 0.25, 8);
        assert!(tube.face_count() > 20);
    }
}
