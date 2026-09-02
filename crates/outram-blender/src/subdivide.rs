// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Subdivide / Un-Subdivide. Follows the published behaviour of Blender's
// subdivide operator (source/blender/bmesh/operators/bmo_subdivide.cc,
// github.com/blender/blender, GPL-2.0-or-later): cut each face into an
// N x N grid (quads) or a fan (tris / n-gons), with a smoothness pull toward
// the limit surface and an optional fractal displacement along the normal;
// un-subdivide dissolves alternate edge loops. Concepts only — no upstream
// source copied; this is a polygon-soup rebuild.
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

//! **Subdivide** and **Un-Subdivide** (`op-hzs.54.8`, GH issue #37 §B).
//!
//! [`subdivide`] cuts every face into `cuts + 1` pieces per side:
//!
//! - a **quad** becomes a `(cuts+1) × (cuts+1)` grid of quads;
//! - a **triangle** becomes `(cuts+1)²` small triangles;
//! - an **n-gon** is fanned from its centroid, then each fan triangle is cut.
//!
//! Edge points are shared between faces (deduplicated), so the result stays
//! watertight. [`SubdivideOptions`] adds:
//!
//! - `smoothness` — blends new edge / interior points toward a Catmull-Clark-
//!   style smoothed position (0 = linear, 1 = full pull);
//! - `fractal` + `seed` — displaces each new vertex along the local face
//!   normal by `fractal · (rand − ½) · edge_len`, deterministically from
//!   `seed` (a small xorshift PRNG — no external crate, offline-reproducible).
//!
//! [`un_subdivide`] is the partial inverse: it dissolves every other vertex of
//! a clean all-quad grid region, halving the resolution. It only acts where
//! the topology is a regular quad grid; elsewhere it is a no-op (Blender's is
//! similarly limited).

use std::collections::HashMap;

use crate::math::Vec3;
use crate::mesh::{Mesh, VertexId};
use crate::topology::MeshTopology;

/// Tuning for [`subdivide`].
#[derive(Debug, Clone, Copy)]
pub struct SubdivideOptions {
    /// Number of cuts per edge (`>= 1`). `1` = a single midpoint split.
    pub cuts: usize,
    /// Pull toward the smoothed surface, `0.0` (linear) … `1.0` (full).
    pub smoothness: f64,
    /// Fractal displacement amplitude along the face normal (`0.0` = none).
    pub fractal: f64,
    /// PRNG seed for the fractal displacement.
    pub seed: u64,
}

impl Default for SubdivideOptions {
    fn default() -> Self {
        SubdivideOptions { cuts: 1, smoothness: 0.0, fractal: 0.0, seed: 1 }
    }
}

/// Subdivide every face of `mesh` per `opts`. `opts.cuts == 0` returns a clone.
pub fn subdivide(mesh: &Mesh, opts: SubdivideOptions) -> Mesh {
    if opts.cuts == 0 {
        return mesh.clone();
    }
    let n = opts.cuts + 1;

    let src_pos = mesh.positions();
    let polys = mesh.polygons();
    let mut positions = src_pos.clone();
    let mut faces: Vec<Vec<usize>> = Vec::new();

    // Deduplicated points along each edge: key (min,max) → the `cuts` ids from
    // the min endpoint toward the max.
    let mut edge_pts: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    let mut rng = XorShift::new(opts.seed);

    let mut edge_ids = |a: usize, b: usize, positions: &mut Vec<Vec3>| -> Vec<usize> {
        let (lo, hi) = (a.min(b), a.max(b));
        let ids = edge_pts.entry((lo, hi)).or_insert_with(|| {
            let pa = positions[lo];
            let pb = positions[hi];
            (1..n)
                .map(|k| {
                    let t = k as f64 / n as f64;
                    positions.push(pa.add(pb.sub(pa).scale(t)));
                    positions.len() - 1
                })
                .collect()
        });
        if a == lo {
            ids.clone()
        } else {
            ids.iter().rev().copied().collect()
        }
    };

    for poly in &polys {
        let vs: Vec<usize> = poly.iter().map(|v| v.0).collect();
        match vs.len() {
            4 => subdivide_quad(&vs, n, &mut positions, &mut faces, &mut edge_ids),
            3 => subdivide_tri(&vs, n, &mut positions, &mut faces, &mut edge_ids),
            k if k > 4 => {
                // Fan from centroid, then subdivide each fan triangle.
                let c = vs.iter().fold(Vec3::ZERO, |acc, &i| acc.add(src_pos[i])).scale(1.0 / k as f64);
                let ci = positions.len();
                positions.push(c);
                for i in 0..k {
                    let tri = [vs[i], vs[(i + 1) % k], ci];
                    subdivide_tri(&tri, n, &mut positions, &mut faces, &mut edge_ids);
                }
            }
            _ => faces.push(vs),
        }
    }

    // Smoothness: pull each *new* vertex toward the average of its neighbours.
    if opts.smoothness > 0.0 {
        smooth_new_points(&mut positions, &faces, src_pos.len(), opts.smoothness.clamp(0.0, 1.0));
    }

    // Fractal: displace each new vertex along a face normal.
    if opts.fractal.abs() > 0.0 {
        let built = Mesh::from_polygons(&positions, &faces);
        let topo = MeshTopology::new(&built);
        let first_new = src_pos.len();
        for (idx, p) in positions.iter_mut().enumerate().skip(first_new) {
            let nrm = topo
                .vertex_faces(VertexId(idx))
                .first()
                .map(|&f| built.face_normal(f))
                .unwrap_or(Vec3::new(0.0, 0.0, 1.0));
            let amp = opts.fractal * (rng.unit() - 0.5);
            *p = p.add(nrm.scale(amp));
        }
    }

    Mesh::from_polygons(&positions, &faces)
}

/// Dissolve alternate rows/columns of a clean all-quad grid region, halving its
/// resolution. A no-op where the topology is not a regular quad grid.
///
/// `iterations` repeats the halving. Returns a new mesh.
pub fn un_subdivide(mesh: &Mesh, iterations: u32) -> Mesh {
    let mut m = mesh.clone();
    for _ in 0..iterations {
        let Some(next) = un_subdivide_once(&m) else { break };
        m = next;
    }
    m
}

fn un_subdivide_once(mesh: &Mesh) -> Option<Mesh> {
    // Only handle the case every face is a quad and every vertex has valence
    // 2, 3 or 4 — i.e. a grid patch. Keep vertices whose *both* grid indices
    // are even. We recover grid indices by a BFS that assigns (i,j) from an
    // arbitrary seed quad.
    let topo = MeshTopology::new(mesh);
    if (0..mesh.face_count()).any(|f| mesh.face_vertices(crate::mesh::FaceId(f)).len() != 4) {
        return None;
    }
    let nv = mesh.vertex_count();
    let mut coord: Vec<Option<(i64, i64)>> = vec![None; nv];
    let seed_face = crate::mesh::FaceId(0);
    let fv = mesh.face_vertices(seed_face);
    coord[fv[0].0] = Some((0, 0));
    coord[fv[1].0] = Some((1, 0));
    coord[fv[2].0] = Some((1, 1));
    coord[fv[3].0] = Some((0, 1));

    // Propagate across shared edges: a neighbour quad sharing edge (a,b) has
    // its other two corners at the reflection of the near corners.
    let mut changed = true;
    let mut guard = 0;
    while changed && guard < nv * 8 {
        changed = false;
        guard += 1;
        for f in 0..mesh.face_count() {
            let q = mesh.face_vertices(crate::mesh::FaceId(f));
            let known: Vec<usize> = (0..4).filter(|&i| coord[q[i].0].is_some()).collect();
            if known.len() < 2 || known.len() == 4 {
                continue;
            }
            // With two adjacent known corners we can fill the quad.
            for i in 0..4 {
                let (a, b, cc, d) = (q[i].0, q[(i + 1) % 4].0, q[(i + 2) % 4].0, q[(i + 3) % 4].0);
                if let (Some(ca), Some(cb)) = (coord[a], coord[b]) {
                    let du = (cb.0 - ca.0, cb.1 - ca.1);
                    let dv = (-du.1, du.0); // 90° turn
                    let want_c = (cb.0 + dv.0, cb.1 + dv.1);
                    let want_d = (ca.0 + dv.0, ca.1 + dv.1);
                    for (idx, want) in [(cc, want_c), (d, want_d)] {
                        if coord[idx].is_none() {
                            coord[idx] = Some(want);
                            changed = true;
                        }
                    }
                }
            }
        }
    }
    if coord.iter().any(|c| c.is_none()) {
        return None; // not a single grid patch
    }

    // Keep vertices with both coords even; rebuild quads over the coarse grid.
    let keep: Vec<bool> = coord.iter().map(|c| {
        let (i, j) = c.unwrap();
        i.rem_euclid(2) == 0 && j.rem_euclid(2) == 0
    }).collect();
    if keep.iter().filter(|&&k| k).count() < 4 {
        return None;
    }
    let by_coord: HashMap<(i64, i64), usize> = coord
        .iter()
        .enumerate()
        .filter(|(i, _)| keep[*i])
        .map(|(i, c)| (c.unwrap(), i))
        .collect();

    let positions: Vec<Vec3> = (0..nv).filter(|&i| keep[i]).map(|i| mesh.vertex(VertexId(i)).unwrap().position).collect();
    let remap: HashMap<usize, usize> = (0..nv).filter(|&i| keep[i]).enumerate().map(|(new, old)| (old, new)).collect();

    let (mut imin, mut imax, mut jmin, mut jmax) = (i64::MAX, i64::MIN, i64::MAX, i64::MIN);
    for c in coord.iter().flatten() {
        imin = imin.min(c.0);
        imax = imax.max(c.0);
        jmin = jmin.min(c.1);
        jmax = jmax.max(c.1);
    }
    let mut faces: Vec<Vec<usize>> = Vec::new();
    let mut ii = imin;
    while ii + 2 <= imax {
        let mut jj = jmin;
        while jj + 2 <= jmax {
            let corners = [(ii, jj), (ii + 2, jj), (ii + 2, jj + 2), (ii, jj + 2)];
            if let Some(q) = corners.iter().map(|c| by_coord.get(c).map(|&o| remap[&o])).collect::<Option<Vec<_>>>() {
                faces.push(q);
            }
            jj += 2;
        }
        ii += 2;
    }
    let _ = &topo;
    if faces.is_empty() {
        return None;
    }
    Some(Mesh::from_polygons(&positions, &faces))
}

fn subdivide_quad(
    vs: &[usize],
    n: usize,
    positions: &mut Vec<Vec3>,
    faces: &mut Vec<Vec<usize>>,
    edge_ids: &mut impl FnMut(usize, usize, &mut Vec<Vec3>) -> Vec<usize>,
) {
    let (a, b, c, d) = (vs[0], vs[1], vs[2], vs[3]);
    let ab = edge_ids(a, b, positions);
    let bc = edge_ids(b, c, positions);
    let dc = edge_ids(d, c, positions);
    let ad = edge_ids(a, d, positions);

    // grid[i][j], i along a→b, j along a→d.
    let mut grid = vec![vec![0usize; n + 1]; n + 1];
    for i in 0..=n {
        for j in 0..=n {
            grid[i][j] = if i == 0 && j == 0 {
                a
            } else if i == n && j == 0 {
                b
            } else if i == n && j == n {
                c
            } else if i == 0 && j == n {
                d
            } else if j == 0 {
                ab[i - 1]
            } else if j == n {
                dc[i - 1]
            } else if i == 0 {
                ad[j - 1]
            } else if i == n {
                bc[j - 1]
            } else {
                // interior: bilinear
                let pa = positions[a];
                let pb = positions[b];
                let pc = positions[c];
                let pd = positions[d];
                let (u, v) = (i as f64 / n as f64, j as f64 / n as f64);
                let p = pa.scale((1.0 - u) * (1.0 - v))
                    .add(pb.scale(u * (1.0 - v)))
                    .add(pc.scale(u * v))
                    .add(pd.scale((1.0 - u) * v));
                positions.push(p);
                positions.len() - 1
            };
        }
    }
    for i in 0..n {
        for j in 0..n {
            faces.push(vec![grid[i][j], grid[i + 1][j], grid[i + 1][j + 1], grid[i][j + 1]]);
        }
    }
}

fn subdivide_tri(
    vs: &[usize],
    n: usize,
    positions: &mut Vec<Vec3>,
    faces: &mut Vec<Vec<usize>>,
    edge_ids: &mut impl FnMut(usize, usize, &mut Vec<Vec3>) -> Vec<usize>,
) {
    let (a, b, c) = (vs[0], vs[1], vs[2]);
    let ab = edge_ids(a, b, positions);
    let bc = edge_ids(b, c, positions);
    let ac = edge_ids(a, c, positions);
    let pa = positions[a];
    let pb = positions[b];
    let pc = positions[c];

    // Barycentric lattice: point(i,j) with i+j <= n, at (a + i/n(b-a) + j/n(c-a)).
    let mut pt: HashMap<(usize, usize), usize> = HashMap::new();
    let mut id = |i: usize, j: usize, positions: &mut Vec<Vec3>| -> usize {
        if let Some(&x) = pt.get(&(i, j)) {
            return x;
        }
        let k = n - i - j;
        let x = if k == n {
            a
        } else if i == n {
            b
        } else if j == n {
            c
        } else if j == 0 {
            ab[i - 1]
        } else if k == 0 {
            bc[j - 1]
        } else if i == 0 {
            ac[j - 1]
        } else {
            let (u, w) = (i as f64 / n as f64, j as f64 / n as f64);
            positions.push(pa.add(pb.sub(pa).scale(u)).add(pc.sub(pa).scale(w)));
            positions.len() - 1
        };
        pt.insert((i, j), x);
        x
    };
    for i in 0..n {
        for j in 0..(n - i) {
            let p00 = id(i, j, positions);
            let p10 = id(i + 1, j, positions);
            let p01 = id(i, j + 1, positions);
            faces.push(vec![p00, p10, p01]);
            if i + j + 1 < n {
                let p11 = id(i + 1, j + 1, positions);
                faces.push(vec![p10, p11, p01]);
            }
        }
    }
}

/// Pull every vertex with index `>= first_new` toward the mean of its polygon
/// neighbours, by `weight`.
fn smooth_new_points(positions: &mut [Vec3], faces: &[Vec<usize>], first_new: usize, weight: f64) {
    let mut sum = vec![Vec3::ZERO; positions.len()];
    let mut cnt = vec![0usize; positions.len()];
    for f in faces {
        let k = f.len();
        for i in 0..k {
            let (a, b) = (f[i], f[(i + 1) % k]);
            sum[a] = sum[a].add(positions[b]);
            cnt[a] += 1;
            sum[b] = sum[b].add(positions[a]);
            cnt[b] += 1;
        }
    }
    for v in first_new..positions.len() {
        if cnt[v] > 0 {
            let avg = sum[v].scale(1.0 / cnt[v] as f64);
            positions[v] = positions[v].add(avg.sub(positions[v]).scale(weight));
        }
    }
}

/// Tiny deterministic xorshift64* PRNG — no external crate, so the fractal
/// displacement is reproducible on any platform (workspace offline rule).
struct XorShift(u64);
impl XorShift {
    fn new(seed: u64) -> Self {
        XorShift(seed.max(1))
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
    /// A value in `[0, 1)`.
    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn one_cut_matches_the_simple_subdivide_counts() {
        // A cube (V=8,E=12,F=6): one cut → V=26, F=24, chi=2.
        let m = primitives::cube(2.0);
        let s = subdivide(&m, SubdivideOptions { cuts: 1, ..Default::default() });
        assert_eq!(s.face_count(), 24);
        assert_eq!(s.vertex_count(), 26);
        assert_eq!(s.euler_characteristic(), 2);
    }

    #[test]
    fn two_cuts_on_a_quad_make_nine() {
        let m = primitives::grid(1, 1, 2.0);
        let s = subdivide(&m, SubdivideOptions { cuts: 2, ..Default::default() });
        assert_eq!(s.face_count(), 9);
        assert_eq!(s.euler_characteristic(), 1);
    }

    #[test]
    fn cuts_on_a_triangle_scale_quadratically() {
        let mut m = Mesh::new();
        let a = m.add_vertex(Vec3::new(0.0, 0.0, 0.0));
        let b = m.add_vertex(Vec3::new(1.0, 0.0, 0.0));
        let c = m.add_vertex(Vec3::new(0.0, 1.0, 0.0));
        m.add_face(&[a, b, c]);
        let s = subdivide(&m, SubdivideOptions { cuts: 2, ..Default::default() });
        assert_eq!(s.face_count(), 9, "(cuts+1)^2 triangles");
        assert_eq!(s.euler_characteristic(), 1);
    }

    #[test]
    fn fractal_is_deterministic_and_moves_points() {
        let m = primitives::grid(2, 2, 2.0);
        let a = subdivide(&m, SubdivideOptions { cuts: 2, fractal: 0.3, seed: 7, ..Default::default() });
        let b = subdivide(&m, SubdivideOptions { cuts: 2, fractal: 0.3, seed: 7, ..Default::default() });
        let flat = subdivide(&m, SubdivideOptions { cuts: 2, ..Default::default() });
        // Same seed → identical.
        for i in 0..a.vertex_count() {
            assert!(a.vertex(VertexId(i)).unwrap().position
                .sub(b.vertex(VertexId(i)).unwrap().position).length() < 1e-12);
        }
        // Some vertex moved off the z = 0 plane.
        assert!((0..a.vertex_count()).any(|i| a.vertex(VertexId(i)).unwrap().position.z.abs() > 1e-6));
        assert!((0..flat.vertex_count()).all(|i| flat.vertex(VertexId(i)).unwrap().position.z.abs() < 1e-9));
    }

    #[test]
    fn un_subdivide_halves_a_grid() {
        let m = primitives::grid(4, 4, 4.0); // 16 quads, 25 verts
        let u = un_subdivide(&m, 1);
        assert_eq!(u.face_count(), 4, "16 → 4");
        assert_eq!(u.vertex_count(), 9);
    }

    #[test]
    fn un_subdivide_is_a_noop_on_a_triangle_mesh() {
        let mut m = Mesh::new();
        let a = m.add_vertex(Vec3::new(0.0, 0.0, 0.0));
        let b = m.add_vertex(Vec3::new(1.0, 0.0, 0.0));
        let c = m.add_vertex(Vec3::new(0.0, 1.0, 0.0));
        m.add_face(&[a, b, c]);
        let u = un_subdivide(&m, 1);
        assert_eq!(u.face_count(), 1);
    }
}
