// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// The LoopTools operations. Blender analogue (architecture only): the bundled
// `mesh_looptools` add-on (Bridge, Circle, Curve, Flatten, GStretch, Loft,
// Relax, Space, Subdivide). No upstream source copied — each operation is
// reimplemented from its documented behaviour, reusing this crate's bridge,
// weld and least-squares plane fit.
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

//! **LoopTools** (`op-hzs.54.42`, GH issue #37 §I) — shape operators on an
//! ordered vertex loop.
//!
//! Every operator takes the mesh and a `loop_verts` slice giving the loop in
//! order (`cyclic` says whether it closes), and returns a new [`Mesh`] with
//! those vertices repositioned (topology unchanged) — except [`bridge`] /
//! [`loft`], which add faces between loops, and [`subdivide`], which splits the
//! loop's edges.
//!
//! - [`circle`] — snap the loop to a best-fit circle (even angular spacing).
//! - [`flatten`] — project the loop onto its best-fit plane.
//! - [`relax`] — Laplacian smoothing along the loop.
//! - [`curve`] — pull the loop toward a Catmull–Rom spline through a subset of
//!   its own vertices.
//! - [`space`] — redistribute the loop to equal arc-length spacing.
//! - [`gstretch`] — redistribute the loop along an external stroke polyline.
//! - [`bridge`] — connect two equal-length loops with a quad strip.
//! - [`loft`] — [`bridge`] a sequence of loops.
//! - [`subdivide`] — split each loop edge at its midpoint.
//!
//! ## Units
//!
//! Positions are dimensionless model-space quantities (see [`crate::math`]).

use crate::math::Vec3;
use crate::mesh::{Mesh, VertexId};

fn soup(mesh: &Mesh) -> (Vec<Vec3>, Vec<Vec<usize>>) {
    (
        mesh.positions(),
        mesh.polygons()
            .iter()
            .map(|f| f.iter().map(|v| v.0).collect())
            .collect(),
    )
}

fn rebuilt(positions: &[Vec3], faces: &[Vec<usize>]) -> Mesh {
    Mesh::from_polygons(positions, faces)
}

/// Best-fit plane of `pts` (centroid + unit normal) by the smallest principal
/// axis of the covariance matrix (power iteration on the inverse is overkill;
/// here: cross-product accumulation à la Newell, robust for near-planar loops).
fn best_fit_plane(pts: &[Vec3]) -> (Vec3, Vec3) {
    let n = pts.len().max(1) as f64;
    let c = pts.iter().fold(Vec3::ZERO, |a, &p| a.add(p)).scale(1.0 / n);
    let mut nrm = Vec3::ZERO;
    for i in 0..pts.len() {
        let cur = pts[i].sub(c);
        let nxt = pts[(i + 1) % pts.len()].sub(c);
        nrm = nrm.add(cur.cross(nxt));
    }
    if nrm.length() < 1e-12 {
        return (c, Vec3::new(0.0, 0.0, 1.0));
    }
    (c, nrm.normalize())
}

/// An orthonormal in-plane basis for a plane with unit `normal`.
fn plane_basis(normal: Vec3) -> (Vec3, Vec3) {
    let a = if normal.x.abs() < 0.9 {
        Vec3::new(1.0, 0.0, 0.0)
    } else {
        Vec3::new(0.0, 1.0, 0.0)
    };
    let u = a.sub(normal.scale(a.dot(normal))).normalize();
    let v = normal.cross(u);
    (u, v)
}

/// Snap the loop to the best-fit circle in its best-fit plane: same centroid,
/// radius = mean vertex distance, vertices placed at even angular spacing
/// starting from the first vertex's current angle.
pub fn circle(mesh: &Mesh, loop_verts: &[VertexId], _cyclic: bool) -> Mesh {
    let (mut positions, faces) = soup(mesh);
    if loop_verts.len() < 3 {
        return rebuilt(&positions, &faces);
    }
    let pts: Vec<Vec3> = loop_verts.iter().map(|v| positions[v.0]).collect();
    let (c, nrm) = best_fit_plane(&pts);
    let (u, v) = plane_basis(nrm);
    let radius = pts.iter().map(|p| p.sub(c).length()).sum::<f64>() / pts.len() as f64;
    let a0 = {
        let d = pts[0].sub(c);
        d.dot(v).atan2(d.dot(u))
    };
    let n = loop_verts.len();
    for (k, vid) in loop_verts.iter().enumerate() {
        let a = a0 + std::f64::consts::TAU * k as f64 / n as f64;
        positions[vid.0] = c
            .add(u.scale(radius * a.cos()))
            .add(v.scale(radius * a.sin()));
    }
    rebuilt(&positions, &faces)
}

/// Project the loop's vertices onto their best-fit plane.
pub fn flatten(mesh: &Mesh, loop_verts: &[VertexId]) -> Mesh {
    let (mut positions, faces) = soup(mesh);
    if loop_verts.len() < 3 {
        return rebuilt(&positions, &faces);
    }
    let pts: Vec<Vec3> = loop_verts.iter().map(|v| positions[v.0]).collect();
    let (c, nrm) = best_fit_plane(&pts);
    for vid in loop_verts {
        let p = positions[vid.0];
        let dist = p.sub(c).dot(nrm);
        positions[vid.0] = p.sub(nrm.scale(dist));
    }
    rebuilt(&positions, &faces)
}

/// Laplacian smoothing along the loop: each vertex moves a `factor` fraction
/// toward the midpoint of its two loop neighbours, `iterations` times.
pub fn relax(
    mesh: &Mesh,
    loop_verts: &[VertexId],
    cyclic: bool,
    iterations: usize,
    factor: f64,
) -> Mesh {
    let (mut positions, faces) = soup(mesh);
    let n = loop_verts.len();
    if n < 3 {
        return rebuilt(&positions, &faces);
    }
    let f = factor.clamp(0.0, 1.0);
    for _ in 0..iterations.max(1) {
        let cur: Vec<Vec3> = loop_verts.iter().map(|v| positions[v.0]).collect();
        for i in 0..n {
            if !cyclic && (i == 0 || i == n - 1) {
                continue;
            }
            let prev = cur[(i + n - 1) % n];
            let next = cur[(i + 1) % n];
            let target = prev.add(next).scale(0.5);
            positions[loop_verts[i].0] = cur[i].add(target.sub(cur[i]).scale(f));
        }
    }
    rebuilt(&positions, &faces)
}

/// Pull the loop toward a smooth Catmull–Rom spline through every `keep`-th
/// vertex (the "anchors"), by `factor`. `keep >= 2`.
pub fn curve(mesh: &Mesh, loop_verts: &[VertexId], cyclic: bool, keep: usize, factor: f64) -> Mesh {
    let (mut positions, faces) = soup(mesh);
    let n = loop_verts.len();
    if n < 4 {
        return rebuilt(&positions, &faces);
    }
    let step = keep.max(2);
    let f = factor.clamp(0.0, 1.0);
    let cur: Vec<Vec3> = loop_verts.iter().map(|v| positions[v.0]).collect();

    // Anchor indices.
    let anchors: Vec<usize> = (0..n).step_by(step).collect();
    let m = anchors.len();
    if m < 2 {
        return rebuilt(&positions, &faces);
    }
    let anchor_pt = |ai: i64| -> Vec3 {
        let idx = if cyclic {
            anchors[ai.rem_euclid(m as i64) as usize]
        } else {
            anchors[ai.clamp(0, m as i64 - 1) as usize]
        };
        cur[idx]
    };

    for (seg, win) in anchors.windows(2).enumerate() {
        let (i0, i1) = (win[0], win[1]);
        let p0 = anchor_pt(seg as i64 - 1);
        let p1 = anchor_pt(seg as i64);
        let p2 = anchor_pt(seg as i64 + 1);
        let p3 = anchor_pt(seg as i64 + 2);
        let span = i1 - i0;
        for j in 1..span {
            let t = j as f64 / span as f64;
            let target = catmull_rom(p0, p1, p2, p3, t);
            let gi = loop_verts[i0 + j].0;
            positions[gi] = cur[i0 + j].add(target.sub(cur[i0 + j]).scale(f));
        }
    }
    rebuilt(&positions, &faces)
}

fn catmull_rom(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f64) -> Vec3 {
    let t2 = t * t;
    let t3 = t2 * t;
    let a = p1.scale(2.0);
    let b = p2.sub(p0).scale(t);
    let c = p0
        .scale(2.0)
        .sub(p1.scale(5.0))
        .add(p2.scale(4.0))
        .sub(p3)
        .scale(t2);
    let d = p1.scale(3.0).sub(p0).sub(p2.scale(3.0)).add(p3).scale(t3);
    a.add(b).add(c).add(d).scale(0.5)
}

/// Redistribute the loop's vertices to equal arc-length spacing along the
/// polyline through their current positions (endpoints of an open loop stay
/// put).
pub fn space(mesh: &Mesh, loop_verts: &[VertexId], cyclic: bool) -> Mesh {
    let cur: Vec<Vec3> = loop_verts
        .iter()
        .map(|v| mesh.vertex(*v).unwrap().position)
        .collect();
    resample_onto(mesh, loop_verts, &cur, cyclic)
}

/// Redistribute the loop's vertices to equal arc-length spacing along an
/// external `stroke` polyline (the "GStretch" grease-pencil behaviour).
pub fn gstretch(mesh: &Mesh, loop_verts: &[VertexId], stroke: &[Vec3], cyclic: bool) -> Mesh {
    resample_onto(mesh, loop_verts, stroke, cyclic)
}

fn resample_onto(mesh: &Mesh, loop_verts: &[VertexId], path: &[Vec3], cyclic: bool) -> Mesh {
    let (mut positions, faces) = soup(mesh);
    let n = loop_verts.len();
    if n < 2 || path.len() < 2 {
        return rebuilt(&positions, &faces);
    }
    // Cumulative arc length of `path`.
    let mut acc = vec![0.0_f64];
    let last = if cyclic { path.len() } else { path.len() - 1 };
    for i in 0..last {
        let a = path[i];
        let b = path[(i + 1) % path.len()];
        acc.push(acc[acc.len() - 1] + b.sub(a).length());
    }
    let total = *acc.last().unwrap();
    if total < 1e-12 {
        return rebuilt(&positions, &faces);
    }
    let sample = |s: f64| -> Vec3 {
        let target = s.clamp(0.0, total);
        let mut seg = 0;
        while seg + 1 < acc.len() && acc[seg + 1] < target {
            seg += 1;
        }
        let seg_len = (acc[seg + 1] - acc[seg]).max(1e-12);
        let t = (target - acc[seg]) / seg_len;
        let a = path[seg % path.len()];
        let b = path[(seg + 1) % path.len()];
        a.add(b.sub(a).scale(t))
    };
    let denom = if cyclic { n } else { n - 1 };
    for (k, vid) in loop_verts.iter().enumerate() {
        let s = total * k as f64 / denom as f64;
        positions[vid.0] = sample(s);
    }
    rebuilt(&positions, &faces)
}

/// Bridge two equal-length ordered loops with a quad strip.
pub fn bridge(mesh: &Mesh, loop_a: &[VertexId], loop_b: &[VertexId], cyclic: bool) -> Mesh {
    crate::bridge::bridge_edge_loops(
        mesh,
        loop_a,
        loop_b,
        cyclic,
        crate::bridge::BridgeOptions::default(),
    )
}

/// Bridge a sequence of loops in order (`loft`). All loops must be the same
/// length.
pub fn loft(mesh: &Mesh, loops: &[&[VertexId]], cyclic: bool) -> Mesh {
    if loops.len() < 2 {
        return mesh.clone();
    }
    let mut out = mesh.clone();
    for w in loops.windows(2) {
        out = crate::bridge::bridge_edge_loops(
            &out,
            w[0],
            w[1],
            cyclic,
            crate::bridge::BridgeOptions::default(),
        );
    }
    out
}

/// Split each edge of the loop at its midpoint, subdividing the faces those
/// edges bound. A face that gains exactly two midpoints is cut in two along
/// the chord between them; a face gaining one keeps it as an extra boundary
/// vertex.
pub fn subdivide(mesh: &Mesh, loop_verts: &[VertexId], cyclic: bool) -> Mesh {
    let (mut positions, faces) = soup(mesh);
    let n = loop_verts.len();
    if n < 2 {
        return rebuilt(&positions, &faces);
    }
    // Loop edges as an unordered-pair set → midpoint vertex id.
    use std::collections::HashMap;
    let mut mid: HashMap<(usize, usize), usize> = HashMap::new();
    let last = if cyclic { n } else { n - 1 };
    for i in 0..last {
        let (a, b) = (loop_verts[i].0, loop_verts[(i + 1) % n].0);
        let key = if a < b { (a, b) } else { (b, a) };
        let m = positions[a].add(positions[b]).scale(0.5);
        positions.push(m);
        mid.insert(key, positions.len() - 1);
    }

    let mut out_faces: Vec<Vec<usize>> = Vec::with_capacity(faces.len());
    for face in &faces {
        // Rebuild the boundary, inserting a midpoint after any loop edge.
        let mut ring: Vec<usize> = Vec::with_capacity(face.len() * 2);
        let mut inserted: Vec<usize> = Vec::new(); // positions in `ring` of midpoints
        for i in 0..face.len() {
            let a = face[i];
            let b = face[(i + 1) % face.len()];
            ring.push(a);
            let key = if a < b { (a, b) } else { (b, a) };
            if let Some(&mv) = mid.get(&key) {
                inserted.push(ring.len());
                ring.push(mv);
            }
        }
        match inserted.len() {
            0 => out_faces.push(face.clone()),
            2 => {
                let (i, j) = (inserted[0], inserted[1]);
                out_faces.push(ring[i..=j].to_vec());
                let mut other: Vec<usize> = ring[j..].to_vec();
                other.extend_from_slice(&ring[..=i]);
                out_faces.push(other);
            }
            _ => out_faces.push(ring),
        }
    }
    rebuilt(&positions, &out_faces)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    /// The 12-vertex border loop of a 3x3 grid, in ring order.
    fn grid_border(m: &Mesh) -> Vec<VertexId> {
        let (ring, _) = crate::bridge::ordered_ring(
            m,
            &(0..m.edge_count())
                .map(crate::mesh::EdgeId)
                .filter(|&e| {
                    let topo = crate::topology::MeshTopology::new(m);
                    topo.is_boundary_edge(e)
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
        ring
    }

    #[test]
    fn circle_puts_every_vertex_at_one_radius() {
        let m = primitives::grid(3, 3, 4.0);
        let loop_v = grid_border(&m);
        let out = circle(&m, &loop_v, true);
        let pts: Vec<Vec3> = loop_v
            .iter()
            .map(|v| out.vertex(*v).unwrap().position)
            .collect();
        let c = pts
            .iter()
            .fold(Vec3::ZERO, |a, &p| a.add(p))
            .scale(1.0 / pts.len() as f64);
        let radii: Vec<f64> = pts.iter().map(|p| p.sub(c).length()).collect();
        let r0 = radii[0];
        assert!(
            radii.iter().all(|r| (r - r0).abs() < 1e-9),
            "all on one circle"
        );
    }

    #[test]
    fn flatten_removes_out_of_plane_displacement() {
        let mut m = primitives::grid(3, 3, 4.0);
        // Nudge one border vertex off the z=0 plane.
        let loop_v = grid_border(&m);
        let mut positions = m.positions();
        positions[loop_v[2].0] = positions[loop_v[2].0].add(Vec3::new(0.0, 0.0, 1.7));
        let faces: Vec<Vec<usize>> = m
            .polygons()
            .iter()
            .map(|f| f.iter().map(|v| v.0).collect())
            .collect();
        m = Mesh::from_polygons(&positions, &faces);

        let out = flatten(&m, &loop_v);
        // Every loop vertex now lies in one common plane.
        let pts: Vec<Vec3> = loop_v
            .iter()
            .map(|v| out.vertex(*v).unwrap().position)
            .collect();
        let (c, nrm) = super::best_fit_plane(&pts);
        for p in &pts {
            assert!(p.sub(c).dot(nrm).abs() < 1e-9, "coplanar after flatten");
        }
        // And the z spread shrank versus the 1.7-unit spike.
        let zspread = pts.iter().map(|p| p.z).fold(f64::MIN, f64::max)
            - pts.iter().map(|p| p.z).fold(f64::MAX, f64::min);
        assert!(
            zspread < 1.7,
            "spike pulled into plane (was 1.7, now {zspread:.3})"
        );
    }

    #[test]
    fn relax_reduces_loop_roughness() {
        let m = primitives::grid(5, 5, 6.0);
        let loop_v = grid_border(&m);
        // Perturb.
        let mut positions = m.positions();
        for (k, v) in loop_v.iter().enumerate() {
            if k % 2 == 0 {
                positions[v.0] = positions[v.0].add(Vec3::new(0.0, 0.0, 0.6));
            }
        }
        let faces: Vec<Vec<usize>> = m
            .polygons()
            .iter()
            .map(|f| f.iter().map(|v| v.0).collect())
            .collect();
        let rough = Mesh::from_polygons(&positions, &faces);

        let rough_var = loop_variance(&rough, &loop_v);
        let smooth = relax(&rough, &loop_v, true, 8, 0.5);
        assert!(loop_variance(&smooth, &loop_v) < rough_var);
    }

    fn loop_variance(m: &Mesh, loop_v: &[VertexId]) -> f64 {
        let zs: Vec<f64> = loop_v
            .iter()
            .map(|v| m.vertex(*v).unwrap().position.z)
            .collect();
        let mean = zs.iter().sum::<f64>() / zs.len() as f64;
        zs.iter().map(|z| (z - mean).powi(2)).sum::<f64>() / zs.len() as f64
    }

    #[test]
    fn space_evens_out_spacing() {
        let m = primitives::grid(4, 4, 6.0);
        let loop_v = grid_border(&m);
        let out = space(&m, &loop_v, true);
        let pts: Vec<Vec3> = loop_v
            .iter()
            .map(|v| out.vertex(*v).unwrap().position)
            .collect();
        let n = pts.len();
        let gaps: Vec<f64> = (0..n)
            .map(|i| pts[(i + 1) % n].sub(pts[i]).length())
            .collect();
        let g0 = gaps[0];
        assert!(
            gaps.iter().all(|g| (g - g0).abs() < 1e-6),
            "equal arc spacing"
        );
    }

    #[test]
    fn gstretch_lays_the_loop_on_the_stroke() {
        let m = primitives::grid(4, 4, 6.0);
        let loop_v = grid_border(&m);
        let stroke = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(3.0, 0.0, 0.0),
            Vec3::new(3.0, 3.0, 0.0),
        ];
        let out = gstretch(&m, &loop_v, &stroke, false);
        // First and last loop vertices land on the stroke endpoints.
        assert!(
            out.vertex(loop_v[0])
                .unwrap()
                .position
                .sub(stroke[0])
                .length()
                < 1e-9
        );
        assert!(
            out.vertex(*loop_v.last().unwrap())
                .unwrap()
                .position
                .sub(stroke[2])
                .length()
                < 1e-9
        );
    }

    #[test]
    fn bridge_two_loops_adds_a_quad_strip() {
        // Two triangles (as thin quads) offset in z.
        let positions = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(1.0, 2.0, 0.0),
            Vec3::new(0.0, 0.0, 3.0),
            Vec3::new(2.0, 0.0, 3.0),
            Vec3::new(1.0, 2.0, 3.0),
        ];
        let m = Mesh::from_polygons(&positions, &[vec![0, 1, 2], vec![3, 5, 4]]);
        let a = [VertexId(0), VertexId(1), VertexId(2)];
        let b = [VertexId(3), VertexId(4), VertexId(5)];
        let out = bridge(&m, &a, &b, true);
        assert_eq!(out.face_count(), 5, "2 caps + 3 side quads");
    }

    #[test]
    fn subdivide_loop_splits_border_edges() {
        let m = primitives::grid(3, 3, 4.0);
        let loop_v = grid_border(&m);
        let before_v = m.vertex_count();
        let out = subdivide(&m, &loop_v, true);
        assert_eq!(
            out.vertex_count(),
            before_v + loop_v.len(),
            "one midpoint per loop edge"
        );
        assert!(out.face_count() >= m.face_count());
    }

    #[test]
    fn curve_smooths_toward_anchor_spline() {
        let m = primitives::grid(6, 6, 8.0);
        let loop_v = grid_border(&m);
        let mut positions = m.positions();
        for (k, v) in loop_v.iter().enumerate() {
            if k % 3 == 1 {
                positions[v.0] = positions[v.0].add(Vec3::new(0.0, 0.0, 0.9));
            }
        }
        let faces: Vec<Vec<usize>> = m
            .polygons()
            .iter()
            .map(|f| f.iter().map(|v| v.0).collect())
            .collect();
        let rough = Mesh::from_polygons(&positions, &faces);
        let out = curve(&rough, &loop_v, true, 3, 1.0);
        // The between-anchor vertices moved toward the spline (z reduced).
        let moved = loop_v
            .iter()
            .enumerate()
            .filter(|(k, _)| k % 3 == 1)
            .any(|(_, v)| {
                out.vertex(*v).unwrap().position.z < rough.vertex(*v).unwrap().position.z - 1e-6
            });
        assert!(moved);
    }
}
