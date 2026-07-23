//! **Loop subdivision** — a smooth subdivision surface for **triangle** meshes.
//!
//! Blender analogue: the Subdivision-Surface modifier in its triangle path
//! (Loop is the triangle analogue of Catmull–Clark, which this crate already
//! provides for quads in [`crate::subdivision`]). Each refinement step splits
//! every triangle into four and moves vertices toward a limit surface that is
//! `C²` almost everywhere (`C¹` at extraordinary vertices), producing a
//! progressively smoother mesh while preserving topology.
//!
//! ## The stencils (Loop, 1987; with Warren's β)
//!
//! Given a triangle mesh, one step produces:
//!
//! - **Edge points** — one new vertex per edge. An *interior* edge `(a, b)`
//!   shared by two triangles with opposite vertices `c, d` gets
//!   `3/8 (a + b) + 1/8 (c + d)`; a *boundary* edge gets the midpoint
//!   `1/2 (a + b)`.
//! - **Repositioned original vertices** — an *interior* vertex `v` of valence
//!   `n` with neighbours `v_k` moves to `(1 − nβ) v + β Σ_k v_k`, where
//!   `β = 3/16` for `n = 3` and `β = 3/(8n)` otherwise (Warren's simplified
//!   weights). A *boundary* vertex moves to `3/4 v + 1/8 (b₁ + b₂)`, using only
//!   its two boundary neighbours — so the boundary curve refines independently
//!   of the interior and is preserved.
//! - **New faces** — each old triangle `(a, b, c)` with edge points
//!   `e_ab, e_bc, e_ca` becomes four triangles: `(a, e_ab, e_ca)`,
//!   `(e_ab, b, e_bc)`, `(e_ca, e_bc, c)`, `(e_ab, e_bc, e_ca)`.
//!
//! Non-triangular input faces are fan-triangulated first. Boundary edges (used
//! by a single triangle) are detected from the face soup, as elsewhere in the
//! crate.
//!
//! > **Untrusted AI-generated draft** until a human reviews it, per the
//! > workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
//! > reactor control, safety-critical, or licensing decisions.

use crate::math::Vec3;
use crate::mesh::{FaceId, Mesh};
use std::collections::{HashMap, HashSet};

/// Apply `iterations` steps of **Loop subdivision** to `mesh`, returning the
/// refined triangle mesh.
///
/// Each step quadruples the triangle count and smooths toward the Loop limit
/// surface; `iterations = 0` fan-triangulates and returns (a no-op-shaped
/// clone). Topology (Euler characteristic, boundary loops) is preserved. Input
/// faces are triangulated first, so the output is always a triangle mesh.
pub fn loop_subdivide(mesh: &Mesh, iterations: u32) -> Mesh {
    let mut current = triangulated(mesh);
    for _ in 0..iterations {
        current = subdivide_once(&current);
    }
    current.to_mesh()
}

/// One Loop step over a triangle mesh given as `(positions, triangles)`.
fn subdivide_once(mesh: &TriMesh) -> TriMesh {
    let positions = &mesh.positions;
    let tris = &mesh.tris;
    let n = positions.len();

    // Edge → the (1 or 2) opposite vertices in its incident triangle(s).
    let mut edge_opp: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    // Vertex → neighbours (for the interior vertex rule).
    let mut adj: Vec<HashSet<usize>> = vec![HashSet::new(); n];
    for &[a, b, c] in tris {
        for &(u, w, o) in &[(a, b, c), (b, c, a), (c, a, b)] {
            edge_opp.entry(key(u, w)).or_default().push(o);
            adj[u].insert(w);
            adj[w].insert(u);
        }
    }

    // Boundary vertices + their two boundary neighbours.
    let mut boundary_nbrs: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut is_boundary = vec![false; n];
    for (&(u, w), opp) in &edge_opp {
        if opp.len() == 1 {
            is_boundary[u] = true;
            is_boundary[w] = true;
            boundary_nbrs[u].push(w);
            boundary_nbrs[w].push(u);
        }
    }

    // 1. Edge points: one new vertex per unique edge (indexed after the n
    //    repositioned originals).
    let mut new_positions: Vec<Vec3> = vec![Vec3::ZERO; n];
    let mut edge_index: HashMap<(usize, usize), usize> = HashMap::new();
    for (&(u, w), opp) in &edge_opp {
        let p = if opp.len() >= 2 {
            // Interior: 3/8 (a+b) + 1/8 (c+d).
            positions[u].add(positions[w]).scale(3.0 / 8.0)
                .add(positions[opp[0]].add(positions[opp[1]]).scale(1.0 / 8.0))
        } else {
            // Boundary: midpoint.
            positions[u].add(positions[w]).scale(0.5)
        };
        edge_index.insert(key(u, w), new_positions.len());
        new_positions.push(p);
    }

    // 2. Reposition original vertices.
    for v in 0..n {
        new_positions[v] = if is_boundary[v] {
            // 3/4 v + 1/8 (b1 + b2). Fall back to v if the boundary is malformed.
            if boundary_nbrs[v].len() == 2 {
                let (b1, b2) = (boundary_nbrs[v][0], boundary_nbrs[v][1]);
                positions[v].scale(0.75)
                    .add(positions[b1].add(positions[b2]).scale(1.0 / 8.0))
            } else {
                positions[v]
            }
        } else {
            let nbrs = &adj[v];
            let deg = nbrs.len();
            if deg == 0 {
                positions[v]
            } else {
                let beta = if deg == 3 { 3.0 / 16.0 } else { 3.0 / (8.0 * deg as f64) };
                let mut sum = Vec3::ZERO;
                for &k in nbrs {
                    sum = sum.add(positions[k]);
                }
                positions[v].scale(1.0 - deg as f64 * beta).add(sum.scale(beta))
            }
        };
    }

    // 3. Four triangles per original triangle.
    let mut new_tris: Vec<[usize; 3]> = Vec::with_capacity(tris.len() * 4);
    for &[a, b, c] in tris {
        let e_ab = edge_index[&key(a, b)];
        let e_bc = edge_index[&key(b, c)];
        let e_ca = edge_index[&key(c, a)];
        new_tris.push([a, e_ab, e_ca]);
        new_tris.push([e_ab, b, e_bc]);
        new_tris.push([e_ca, e_bc, c]);
        new_tris.push([e_ab, e_bc, e_ca]);
    }

    TriMesh { positions: new_positions, tris: new_tris }
}

/// A plain triangle mesh (positions + index triples), the working form here.
struct TriMesh {
    positions: Vec<Vec3>,
    tris: Vec<[usize; 3]>,
}

impl TriMesh {
    /// Rebuild a [`Mesh`] from the triangle soup.
    fn to_mesh(&self) -> Mesh {
        let faces: Vec<Vec<usize>> = self.tris.iter().map(|t| t.to_vec()).collect();
        Mesh::from_polygons(&self.positions, &faces)
    }
}

/// Fan-triangulate `mesh` into a [`TriMesh`].
fn triangulated(mesh: &Mesh) -> TriMesh {
    let positions = mesh.positions();
    let mut tris = Vec::new();
    for f in 0..mesh.face_count() {
        let vs = mesh.face_vertices(FaceId(f));
        if vs.len() < 3 {
            continue;
        }
        for k in 1..vs.len() - 1 {
            tris.push([vs[0].0, vs[k].0, vs[k + 1].0]);
        }
    }
    TriMesh { positions, tris }
}

/// Undirected edge key `(min, max)`.
fn key(a: usize, b: usize) -> (usize, usize) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    /// A closed octahedron (6 vertices, 8 triangular faces) as a pure-triangle
    /// test solid.
    fn octahedron() -> Mesh {
        let positions = [
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, -1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, -1.0),
        ];
        let faces = vec![
            vec![0, 2, 4], vec![2, 1, 4], vec![1, 3, 4], vec![3, 0, 4],
            vec![2, 0, 5], vec![1, 2, 5], vec![3, 1, 5], vec![0, 3, 5],
        ];
        Mesh::from_polygons(&positions, &faces)
    }

    /// `true` if every undirected edge borders exactly two faces.
    fn is_watertight(m: &Mesh) -> bool {
        let mut use_count: HashMap<(usize, usize), usize> = HashMap::new();
        for face in m.polygons() {
            let n = face.len();
            for i in 0..n {
                *use_count.entry(key(face[i].0, face[(i + 1) % n].0)).or_insert(0) += 1;
            }
        }
        use_count.values().all(|&c| c == 2)
    }

    /// Methodology: **closed triangle mesh refines correctly.** One Loop step on
    /// the octahedron (`V=6, E=12, F=8`) must quadruple the faces (`F' = 4F =
    /// 32`), add one vertex per edge (`V' = V + E = 18`), stay closed and
    /// watertight (`chi = 2`), and — being a subdivision toward a smooth
    /// surface — pull vertices *inward* toward the sphere the octahedron
    /// approximates (max radius decreases from 1).
    ///
    /// Results (asserted): `F = 32`, `V = 18`, `chi = 2`, watertight; and after
    /// two steps every vertex radius is `< 1.0` (the sharp corners are smoothed
    /// in).
    #[test]
    fn loop_refines_octahedron() {
        let octa = octahedron();
        let one = loop_subdivide(&octa, 1);
        assert_eq!(one.face_count(), 32, "F' = 4F");
        assert_eq!(one.vertex_count(), 18, "V' = V + E");
        assert_eq!(one.euler_characteristic(), 2, "stays closed (chi=2)");
        assert!(is_watertight(&one), "stays watertight");

        let two = loop_subdivide(&octa, 2);
        assert_eq!(two.euler_characteristic(), 2);
        // Smoothing pulls the corners in: no vertex sticks out past the original
        // unit corners.
        assert!(
            two.positions().iter().all(|p| p.length() < 1.0 + 1e-9),
            "subdivision must not inflate past the original corners"
        );
        // And the mesh genuinely rounded (mean radius grew toward the inscribed
        // region, i.e. strictly less than the corner radius 1 on average).
        let mean_r = two.positions().iter().map(|p| p.length()).sum::<f64>() / two.vertex_count() as f64;
        assert!(mean_r < 1.0, "corners smoothed inward, mean radius {mean_r} < 1");
    }

    /// Methodology: **boundary preserved, patch stays planar.** Subdividing a
    /// flat `grid(3, 3)` (a disc, `chi = 1`, in `z = 0`) must keep it planar and
    /// a disc, with the boundary curve refined by the boundary rule (staying
    /// within the original `[-0.5, 0.5]²` footprint — the boundary stencil is a
    /// convex combination of boundary points).
    ///
    /// Results (asserted): `chi = 1`; all vertices have `|z| < 1e-12` and lie in
    /// the original footprint; face count quadruples from the triangulated grid.
    #[test]
    fn loop_grid_stays_planar_and_bounded() {
        let grid = primitives::grid(3, 3, 1.0);
        let tri_faces = 3 * 3 * 2; // fan-triangulated quads
        let out = loop_subdivide(&grid, 1);
        assert_eq!(out.face_count(), tri_faces * 4, "F' = 4F of the triangulated grid");
        assert_eq!(out.euler_characteristic(), 1, "stays a disc (chi=1)");
        for p in out.positions() {
            assert!(p.z.abs() < 1e-12, "left the z=0 plane: {}", p.z);
            assert!(p.x >= -0.5 - 1e-12 && p.x <= 0.5 + 1e-12, "x out of footprint: {}", p.x);
            assert!(p.y >= -0.5 - 1e-12 && p.y <= 0.5 + 1e-12, "y out of footprint: {}", p.y);
        }
    }

    /// `iterations = 0` triangulates and returns without smoothing (positions
    /// unchanged for an already-triangular mesh).
    #[test]
    fn loop_zero_iterations_is_triangulate_only() {
        let octa = octahedron();
        let out = loop_subdivide(&octa, 0);
        assert_eq!(out.face_count(), 8, "already triangular");
        assert_eq!(out.vertex_count(), 6);
        for (a, b) in octa.positions().iter().zip(out.positions().iter()) {
            assert!(a.sub(*b).length() < 1e-15, "positions unchanged");
        }
    }
}
