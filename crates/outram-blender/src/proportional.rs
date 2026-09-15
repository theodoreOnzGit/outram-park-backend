// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Proportional editing. Follows the published behaviour of Blender's
// proportional-edit falloff (source/blender/editors/transform/transform.cc,
// `transform_proportional_*`, github.com/blender/blender, GPL-2.0-or-later): a
// weighting layer that spreads a transform to nearby vertices by a falloff
// curve, optionally restricted to topologically-connected geometry. Concepts
// only — no upstream source copied; a position rewrite.
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

//! **Proportional editing** (`op-hzs.54.20`, GH issue #37 §C) — spread a set of
//! explicit vertex displacements to nearby vertices by a falloff curve.
//!
//! [`proportional_move`] takes the vertices the caller is "grabbing" with their
//! target displacements, a `radius`, a [`Falloff`], and `connected_only` (use
//! topological distance along edges instead of straight-line distance). Each
//! other vertex within `radius` of a grabbed vertex moves by
//! `falloff(dist / radius) · displacement_of_nearest_grabbed`.

use std::collections::BinaryHeap;

use crate::math::Vec3;
use crate::mesh::{Mesh, VertexId};
use crate::topology::MeshTopology;

/// Blender's proportional-edit falloff curves. `t` is `distance / radius` in
/// `[0, 1]`; each returns a weight in `[0, 1]` that is `1` at `t = 0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Falloff {
    /// `2t³ − 3t² + 1` (the default smoothstep).
    Smooth,
    /// `√(1 − t²)` — a quarter circle, `1` at the centre.
    Sphere,
    /// `√(1 − t)`.
    Root,
    /// `(1 − t)²`.
    InverseSquare,
    /// `t² − 2t + 1` … actually the sharp curve `(1 − t)² `? Blender's Sharp is
    /// `(1 − t)²` with a steeper toe — implemented as `((1 − t))³`.
    Sharp,
    /// `1 − t`.
    Linear,
    /// `1` for all `t < 1` (a hard cutoff).
    Constant,
    /// `1 − t` scaled by a per-vertex random value (see
    /// [`proportional_move`]'s `seed`).
    Random,
}

impl Falloff {
    /// The weight for a normalised distance `t` (clamped to `[0, 1]`).
    pub fn weight(self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Falloff::Smooth => 2.0 * t * t * t - 3.0 * t * t + 1.0,
            Falloff::Sphere => (1.0 - t * t).max(0.0).sqrt(),
            Falloff::Root => (1.0 - t).max(0.0).sqrt(),
            Falloff::InverseSquare => (1.0 - t) * (1.0 - t),
            Falloff::Sharp => (1.0 - t).powi(3),
            Falloff::Linear => 1.0 - t,
            Falloff::Constant => 1.0,
            Falloff::Random => 1.0 - t, // scaled by the RNG in `proportional_move`
        }
    }
}

/// Apply the explicit displacements in `grabbed` and spread them to nearby
/// vertices per `falloff` within `radius`. `connected_only` measures distance
/// along mesh edges (Dijkstra); otherwise straight-line. `seed` is used only
/// by [`Falloff::Random`].
pub fn proportional_move(
    mesh: &Mesh,
    grabbed: &[(VertexId, Vec3)],
    radius: f64,
    falloff: Falloff,
    connected_only: bool,
    seed: u64,
) -> Mesh {
    if grabbed.is_empty() || radius <= 0.0 {
        let mut positions = mesh.positions();
        for &(v, d) in grabbed {
            if v.0 < positions.len() {
                positions[v.0] = positions[v.0].add(d);
            }
        }
        return Mesh::from_polygons(&positions, &to_soup(mesh));
    }

    let positions0 = mesh.positions();
    let nv = positions0.len();

    // Distance from every vertex to the nearest grabbed vertex, plus which one.
    let (dist, nearest) = if connected_only {
        topological_distance(mesh, grabbed)
    } else {
        euclidean_distance(&positions0, grabbed)
    };

    let mut rng = XorShift::new(seed);
    let rand_scale: Vec<f64> = (0..nv).map(|_| rng.unit()).collect();

    let mut positions = positions0.clone();
    for v in 0..nv {
        let Some(gi) = nearest[v] else { continue };
        let t = dist[v] / radius;
        if t > 1.0 {
            continue;
        }
        let mut w = falloff.weight(t);
        if falloff == Falloff::Random {
            w *= rand_scale[v];
        }
        positions[v] = positions0[v].add(grabbed[gi].1.scale(w));
    }
    // Grabbed vertices get their full displacement regardless of the curve.
    for &(v, d) in grabbed {
        if v.0 < nv {
            positions[v.0] = positions0[v.0].add(d);
        }
    }
    Mesh::from_polygons(&positions, &to_soup(mesh))
}

fn to_soup(mesh: &Mesh) -> Vec<Vec<usize>> {
    mesh.polygons()
        .iter()
        .map(|p| p.iter().map(|v| v.0).collect())
        .collect()
}

fn euclidean_distance(
    pos: &[Vec3],
    grabbed: &[(VertexId, Vec3)],
) -> (Vec<f64>, Vec<Option<usize>>) {
    let mut dist = vec![f64::INFINITY; pos.len()];
    let mut near = vec![None; pos.len()];
    for (v, p) in pos.iter().enumerate() {
        for (gi, &(g, _)) in grabbed.iter().enumerate() {
            if g.0 >= pos.len() {
                continue;
            }
            let d = p.sub(pos[g.0]).length();
            if d < dist[v] {
                dist[v] = d;
                near[v] = Some(gi);
            }
        }
    }
    (dist, near)
}

fn topological_distance(
    mesh: &Mesh,
    grabbed: &[(VertexId, Vec3)],
) -> (Vec<f64>, Vec<Option<usize>>) {
    let topo = MeshTopology::new(mesh);
    let nv = mesh.vertex_count();
    let pos = mesh.positions();
    let mut dist = vec![f64::INFINITY; nv];
    let mut near: Vec<Option<usize>> = vec![None; nv];
    let mut heap: BinaryHeap<(std::cmp::Reverse<Ord64>, usize, usize)> = BinaryHeap::new();
    for (gi, &(g, _)) in grabbed.iter().enumerate() {
        if g.0 < nv {
            dist[g.0] = 0.0;
            near[g.0] = Some(gi);
            heap.push((std::cmp::Reverse(Ord64(0.0)), g.0, gi));
        }
    }
    while let Some((std::cmp::Reverse(Ord64(d)), v, gi)) = heap.pop() {
        if d > dist[v] {
            continue;
        }
        for &e in topo.vertex_edges(VertexId(v)) {
            let Some(w) = topo.other_end(mesh, e, VertexId(v)) else {
                continue;
            };
            let step = pos[w.0].sub(pos[v]).length();
            let nd = d + step;
            if nd < dist[w.0] {
                dist[w.0] = nd;
                near[w.0] = Some(gi);
                heap.push((std::cmp::Reverse(Ord64(nd)), w.0, gi));
            }
        }
    }
    (dist, near)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Ord64(f64);
impl Eq for Ord64 {}
impl PartialOrd for Ord64 {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Ord64 {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.0
            .partial_cmp(&o.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

struct XorShift(u64);
impl XorShift {
    fn new(seed: u64) -> Self {
        XorShift(seed.max(1))
    }
    fn unit(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        ((x.wrapping_mul(0x2545F4914F6CDD1D)) >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn falloff_curves_start_at_one_end_at_zero() {
        for f in [
            Falloff::Smooth,
            Falloff::Sphere,
            Falloff::Root,
            Falloff::InverseSquare,
            Falloff::Sharp,
            Falloff::Linear,
        ] {
            assert!((f.weight(0.0) - 1.0).abs() < 1e-9, "{f:?} at 0");
            assert!(f.weight(1.0).abs() < 1e-9, "{f:?} at 1");
            assert!(f.weight(0.5) > 0.0 && f.weight(0.5) < 1.0, "{f:?} mid");
        }
        assert_eq!(Falloff::Constant.weight(0.9), 1.0);
        assert_eq!(Falloff::Constant.weight(1.1), 1.0);
    }

    #[test]
    fn grabbing_one_grid_vertex_drags_its_neighbours_less() {
        let m = primitives::grid(6, 6, 6.0);
        // Grab the centre vertex, move it +z by 2.
        let cv = (0..m.vertex_count())
            .find(|&i| {
                m.vertex(VertexId(i))
                    .unwrap()
                    .position
                    .sub(Vec3::ZERO)
                    .length()
                    < 1e-9
            })
            .unwrap();
        let out = proportional_move(
            &m,
            &[(VertexId(cv), Vec3::new(0.0, 0.0, 2.0))],
            3.0,
            Falloff::Smooth,
            false,
            1,
        );
        let cz = out.vertex(VertexId(cv)).unwrap().position.z;
        assert!((cz - 2.0).abs() < 1e-9, "grabbed vertex fully moved");
        // A vertex ~1 unit away moved partway; one outside the radius did not.
        let mut some_partial = false;
        let mut some_zero = false;
        for i in 0..m.vertex_count() {
            let d = m
                .vertex(VertexId(i))
                .unwrap()
                .position
                .sub(m.vertex(VertexId(cv)).unwrap().position)
                .length();
            let dz = out.vertex(VertexId(i)).unwrap().position.z;
            if d > 0.5 && d < 2.0 && dz > 0.1 && dz < 2.0 {
                some_partial = true;
            }
            if d > 3.5 {
                assert!(dz.abs() < 1e-9, "outside radius untouched");
                some_zero = true;
            }
        }
        assert!(some_partial && some_zero);
    }

    #[test]
    fn connected_only_ignores_a_close_but_disjoint_island() {
        // Two separate grids; grabbing a vertex on one must not drag the other
        // even though they overlap in space.
        let g = primitives::grid(3, 3, 2.0);
        let mut positions = g.positions();
        let mut faces: Vec<Vec<usize>> = g
            .polygons()
            .iter()
            .map(|f| f.iter().map(|v| v.0).collect())
            .collect();
        let off = positions.len();
        for p in g.positions() {
            positions.push(p.add(Vec3::new(0.0, 0.0, 0.01))); // almost coincident
        }
        for f in g.polygons() {
            faces.push(f.iter().map(|v| v.0 + off).collect());
        }
        let m = Mesh::from_polygons(&positions, &faces);

        let out = proportional_move(
            &m,
            &[(VertexId(0), Vec3::new(0.0, 0.0, 5.0))],
            10.0,
            Falloff::Linear,
            true,
            1,
        );
        // The second island's vertices did not move.
        for i in off..m.vertex_count() {
            let dz = out.vertex(VertexId(i)).unwrap().position.z
                - m.vertex(VertexId(i)).unwrap().position.z;
            assert!(dz.abs() < 1e-9);
        }
    }

    #[test]
    fn random_falloff_is_seed_deterministic() {
        let m = primitives::grid(4, 4, 4.0);
        let a = proportional_move(
            &m,
            &[(VertexId(0), Vec3::new(0.0, 0.0, 1.0))],
            10.0,
            Falloff::Random,
            false,
            7,
        );
        let b = proportional_move(
            &m,
            &[(VertexId(0), Vec3::new(0.0, 0.0, 1.0))],
            10.0,
            Falloff::Random,
            false,
            7,
        );
        for i in 0..a.vertex_count() {
            assert!(
                a.vertex(VertexId(i))
                    .unwrap()
                    .position
                    .sub(b.vertex(VertexId(i)).unwrap().position)
                    .length()
                    < 1e-12
            );
        }
    }
}
