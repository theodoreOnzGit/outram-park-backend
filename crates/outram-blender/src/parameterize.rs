//! Planar **mesh parameterization** — flatten a disk-topology surface patch to
//! 2D `(u, v)` coordinates by a harmonic / Tutte embedding.
//!
//! Blender analogue: UV unwrapping (`uvedit` / `bmo_...unwrap`, the "Unwrap"
//! operator's underlying harmonic map). This is the crate's third sparse-solve
//! operator; it **reuses the same boundary-pinned SPD reduced system** as
//! [`crate::laplacian::laplacian_smooth`] — only the right-hand side and the
//! solution width (2, for `u`/`v`) differ.
//!
//! ## The map
//!
//! For a mesh that is a **topological disk** (connected, orientable, genus 0,
//! with a single boundary loop), the parameterization:
//!
//! 1. **pins the boundary loop** to a convex target ([`BoundaryShape`] — a unit
//!    circle or square) by *arc length* along the loop, then
//! 2. **solves the Laplace equation** `L x = 0` on the interior vertices with
//!    those boundary values fixed — a harmonic map. This is exactly the
//!    Dirichlet (grounded) Laplacian solve, `A_ff x_f = -L_fb x_b`, that the
//!    smoothing operator already assembles.
//!
//! The [`crate::laplacian::LaplacianWeighting`] chooses the flavour:
//!
//! - **Uniform** = *Tutte's barycentric embedding*. By Tutte's theorem, pinning
//!   the boundary of a 3-connected planar graph to a convex polygon yields a
//!   valid straight-line embedding — **no triangle flips or degeneracies**,
//!   guaranteed. Always solvable (SPD).
//! - **Cotangent** = *harmonic map*. Lower angle distortion (nearer conformal),
//!   but the guarantee is lost on obtuse (non-Delaunay) meshes, where a triangle
//!   can flip or the system can fail to be positive definite.
//!
//! ## Requirements
//!
//! The mesh **must be an open disk**: a closed mesh (sphere) has no boundary to
//! pin and cannot be flattened without a cut; an annulus / disk-with-holes has
//! more than one boundary loop; a handle body is not genus 0. Each of these is
//! rejected with a specific [`ParamError`] rather than producing garbage.
//!
//! > **Untrusted AI-generated draft** until a human reviews it, per the
//! > workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
//! > reactor control, safety-critical, or licensing decisions.

use crate::laplacian::{edge_weights_from, mesh_triangles, LaplacianWeighting};
use crate::math::Vec3;
use crate::mesh::Mesh;
use std::collections::HashMap;
use std::f64::consts::PI;

/// The convex boundary shape the mesh's border is pinned to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryShape {
    /// The unit circle (radius 1, centred at the origin) — the natural default;
    /// always convex, so Tutte's guarantee holds.
    Circle,
    /// The unit square `[0, 1]^2`, one quarter of the boundary length per side.
    Square,
}

/// Errors from [`parameterize`].
#[derive(Debug, thiserror::Error)]
pub enum ParamError {
    /// The mesh has no boundary (it is closed). A closed surface cannot be
    /// flattened to a disk without first introducing a cut/seam.
    #[error("mesh is closed (no boundary loop) — cannot parameterize to a disk without a cut")]
    NoBoundary,
    /// The mesh has more than one boundary loop (an annulus, or a disk with
    /// holes). Only a single-boundary disk is supported.
    #[error("mesh has multiple boundary loops — only a single-boundary disk is supported")]
    MultipleBoundaries,
    /// A boundary vertex has two outgoing boundary edges (a non-manifold
    /// "bowtie" pinch); the boundary is not a simple loop.
    #[error("non-manifold boundary (a vertex with two boundary edges)")]
    NonManifoldBoundary,
    /// The mesh is not a genus-0 disk (`Euler characteristic != 1`, e.g. it has
    /// a handle).
    #[error("mesh is not a topological disk (Euler characteristic != 1)")]
    NotADisk,
    /// The sparse harmonic system could not be assembled.
    #[error("parameterization matrix assembly failed")]
    Assembly,
    /// The sparse Cholesky factorization failed — the interior Laplacian system
    /// is not positive definite (can happen with the cotangent weighting on an
    /// obtuse mesh, or a disconnected interior). Try the uniform weighting.
    #[error("parameterization system is not positive definite (try Uniform weighting)")]
    NotPositiveDefinite,
}

/// Parameterize `mesh` (a topological disk) to 2D, returning one `(u, v)` per
/// vertex in [`crate::mesh::VertexId`] order (`result[i]` is the UV of vertex
/// `i`).
///
/// Boundary vertices land exactly on the [`BoundaryShape`] (by arc length);
/// interior vertices are the harmonic solution under `weighting` (see the module
/// docs). Use [`LaplacianWeighting::Uniform`] for a guaranteed flip-free (Tutte)
/// embedding, or [`LaplacianWeighting::Cotangent`] for lower angle distortion on
/// a well-shaped mesh.
///
/// # Errors
///
/// See [`ParamError`] — the mesh must be a single-boundary genus-0 disk.
pub fn parameterize(
    mesh: &Mesh,
    weighting: LaplacianWeighting,
    boundary: BoundaryShape,
) -> Result<Vec<(f64, f64)>, ParamError> {
    use faer::linalg::solvers::Solve;
    use faer::sparse::{SparseColMat, Triplet};
    use faer::{Mat, Side};

    let tris = mesh_triangles(mesh);
    // Reject a closed mesh (NoBoundary) before the genus check, so the common
    // "you gave me a sphere/cube" case gets the most informative error.
    let loop_verts = boundary_loop(&tris)?;
    // A single-boundary mesh that is not genus 0 (a handle body) is not a disk.
    if mesh.euler_characteristic() != 1 {
        return Err(ParamError::NotADisk);
    }
    let ps = mesh.positions();
    let n = mesh.vertex_count();

    // 1. Pin the boundary loop to the target shape by arc length.
    let mut uv = vec![(0.0f64, 0.0f64); n];
    let mut on_boundary = vec![false; n];
    assign_boundary_uv(&loop_verts, &ps, boundary, &mut uv, &mut on_boundary);

    // 2. Free (interior) vertex mapping.
    let mut free_of: Vec<Option<usize>> = vec![None; n];
    let mut free_verts: Vec<usize> = Vec::new();
    for v in 0..n {
        if !on_boundary[v] {
            free_of[v] = Some(free_verts.len());
            free_verts.push(v);
        }
    }
    let nfree = free_verts.len();
    if nfree == 0 {
        // Degenerate: every vertex is on the boundary (e.g. a single triangle).
        // The boundary map alone is the answer.
        return Ok(uv);
    }

    // 3. Assemble and solve the grounded Laplacian: A_ff x_f = -L_fb x_b, with a
    // zero interior source. Diagonal is the full row sum L_ii = Σ_j w_ij.
    let weights = edge_weights_from(&tris, &ps, weighting);
    let mut diag = vec![0.0f64; n];
    let mut rhs = vec![[0.0f64; 2]; nfree];
    let mut trips: Vec<Triplet<usize, usize, f64>> = Vec::new();

    for (&(gi, gj), &w) in &weights {
        diag[gi] += w;
        diag[gj] += w;
        match (free_of[gi], free_of[gj]) {
            (Some(fi), Some(fj)) => {
                let (lo, hi) = if fi < fj { (fi, fj) } else { (fj, fi) };
                trips.push(Triplet::new(hi, lo, -w));
            }
            (Some(fi), None) => {
                rhs[fi][0] += w * uv[gj].0;
                rhs[fi][1] += w * uv[gj].1;
            }
            (None, Some(fj)) => {
                rhs[fj][0] += w * uv[gi].0;
                rhs[fj][1] += w * uv[gi].1;
            }
            (None, None) => {}
        }
    }
    for (fi, &gi) in free_verts.iter().enumerate() {
        trips.push(Triplet::new(fi, fi, diag[gi]));
    }

    let a = SparseColMat::<usize, f64>::try_new_from_triplets(nfree, nfree, &trips)
        .map_err(|_| ParamError::Assembly)?;
    let llt = a.sp_cholesky(Side::Lower).map_err(|_| ParamError::NotPositiveDefinite)?;
    let b = Mat::<f64>::from_fn(nfree, 2, |i, j| rhs[i][j]);
    let x = llt.solve(&b);

    for (fi, &gi) in free_verts.iter().enumerate() {
        uv[gi] = (x[(fi, 0)], x[(fi, 1)]);
    }
    Ok(uv)
}

/// Flatten `mesh` into the `z = 0` plane using its [`parameterize`] UVs: a new
/// mesh with the same faces but positions `(u, v, 0)`. Convenience wrapper when
/// a 2D *mesh* (rather than a UV list) is wanted.
pub fn flatten_to_plane(
    mesh: &Mesh,
    weighting: LaplacianWeighting,
    boundary: BoundaryShape,
) -> Result<Mesh, ParamError> {
    let uv = parameterize(mesh, weighting, boundary)?;
    let positions: Vec<Vec3> = uv.iter().map(|&(u, v)| Vec3::new(u, v, 0.0)).collect();
    let faces: Vec<Vec<usize>> = mesh
        .polygons()
        .iter()
        .map(|f| f.iter().map(|v| v.0).collect())
        .collect();
    Ok(Mesh::from_polygons(&positions, &faces))
}

// ---------------------------------------------------------------------------
// Boundary-loop extraction.
// ---------------------------------------------------------------------------

/// Extract the single ordered boundary loop from the triangulation `tris`.
///
/// A boundary edge is used by exactly one triangle; taken in that triangle's
/// (consistent CCW) winding it is a directed half-edge `i -> j`, and chaining
/// `next_of[i] = j` walks the loop in order. Returns the ordered vertex indices,
/// or a [`ParamError`] if there is no boundary, more than one loop, or a
/// non-manifold boundary vertex.
fn boundary_loop(tris: &[[usize; 3]]) -> Result<Vec<usize>, ParamError> {
    // Count undirected-edge incidence.
    let mut count: HashMap<(usize, usize), usize> = HashMap::new();
    for &[a, b, c] in tris {
        for &(i, j) in &[(a, b), (b, c), (c, a)] {
            let key = if i < j { (i, j) } else { (j, i) };
            *count.entry(key).or_insert(0) += 1;
        }
    }
    // Directed boundary half-edges (count == 1), as a successor map.
    let mut next_of: HashMap<usize, usize> = HashMap::new();
    let mut n_boundary = 0usize;
    for &[a, b, c] in tris {
        for &(i, j) in &[(a, b), (b, c), (c, a)] {
            let key = if i < j { (i, j) } else { (j, i) };
            if count[&key] == 1 {
                n_boundary += 1;
                if next_of.insert(i, j).is_some() {
                    return Err(ParamError::NonManifoldBoundary);
                }
            }
        }
    }
    if next_of.is_empty() {
        return Err(ParamError::NoBoundary);
    }

    // Walk one loop, starting from the smallest boundary vertex index so the
    // result is deterministic (independent of hash-map iteration order).
    let start = *next_of.keys().min().expect("non-empty boundary");
    let mut loop_verts = vec![start];
    let mut v = start;
    loop {
        v = *next_of.get(&v).ok_or(ParamError::NonManifoldBoundary)?;
        if v == start {
            break;
        }
        loop_verts.push(v);
        if loop_verts.len() > n_boundary {
            break; // safety — should not happen for a manifold boundary
        }
    }
    if loop_verts.len() != n_boundary {
        // The single walked loop did not cover every boundary edge.
        return Err(ParamError::MultipleBoundaries);
    }
    Ok(loop_verts)
}

/// Assign each boundary vertex a fixed `(u, v)` on `shape` by its cumulative
/// arc-length fraction around the loop (using 3D chord lengths).
fn assign_boundary_uv(
    loop_verts: &[usize],
    ps: &[Vec3],
    shape: BoundaryShape,
    uv: &mut [(f64, f64)],
    on_boundary: &mut [bool],
) {
    let m = loop_verts.len();
    let mut cum = vec![0.0f64; m + 1];
    for k in 0..m {
        let a = ps[loop_verts[k]];
        let b = ps[loop_verts[(k + 1) % m]];
        cum[k + 1] = cum[k] + b.sub(a).length();
    }
    let total = cum[m].max(f64::MIN_POSITIVE);

    for k in 0..m {
        let t = cum[k] / total; // in [0, 1)
        uv[loop_verts[k]] = match shape {
            BoundaryShape::Circle => {
                let theta = 2.0 * PI * t;
                (theta.cos(), theta.sin())
            }
            BoundaryShape::Square => square_point(t),
        };
        on_boundary[loop_verts[k]] = true;
    }
}

/// Map an arc-length fraction `t in [0, 1)` to a point on the unit square
/// `[0,1]^2` boundary (one side per quarter, counter-clockwise from the bottom).
fn square_point(t: f64) -> (f64, f64) {
    let s = t * 4.0;
    if s < 1.0 {
        (s, 0.0) // bottom, left→right
    } else if s < 2.0 {
        (1.0, s - 1.0) // right, bottom→top
    } else if s < 3.0 {
        (1.0 - (s - 2.0), 1.0) // top, right→left
    } else {
        (0.0, 1.0 - (s - 3.0)) // left, top→bottom
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    /// Signed area (×2) of a 2D triangle `a, b, c` — the z-component of
    /// `(b-a) × (c-a)`. Positive = counter-clockwise.
    fn signed_area2(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
        (b.0 - a.0) * (c.1 - a.1) - (c.0 - a.0) * (b.1 - a.1)
    }

    /// Assert the parameterization has **no flipped triangles**: every
    /// triangle's 2D signed area shares one sign and is non-degenerate.
    fn assert_no_flips(mesh: &Mesh, uv: &[(f64, f64)]) {
        let tris = mesh_triangles(mesh);
        let mut sign = 0.0;
        for &[a, b, c] in &tris {
            let area = signed_area2(uv[a], uv[b], uv[c]);
            assert!(area.abs() > 1e-12, "degenerate (zero-area) triangle in UV");
            if sign == 0.0 {
                sign = area.signum();
            }
            assert!(area * sign > 0.0, "flipped triangle in UV (sign flip)");
        }
    }

    /// Methodology: **Tutte guarantee.** A flat `primitives::grid(4, 4)` patch is
    /// a topological disk (`chi = 1`). Under the uniform (Tutte) weighting with a
    /// convex circular boundary, the embedding must be valid — every triangle
    /// non-degenerate and consistently oriented (Tutte's theorem), every
    /// boundary vertex exactly on the unit circle, and every interior vertex
    /// strictly inside it (maximum principle).
    ///
    /// Results (asserted): 16 boundary vertices land at radius `1` (tol `1e-12`),
    /// 9 interior vertices have radius `< 1`, and all `32` triangles are
    /// flip-free.
    #[test]
    fn tutte_grid_is_a_valid_flipfree_embedding() {
        let grid = primitives::grid(4, 4, 1.0);
        let uv = parameterize(&grid, LaplacianWeighting::Uniform, BoundaryShape::Circle)
            .expect("grid is a disk");

        // Boundary on the circle, interior strictly inside.
        let on_bound = crate::laplacian::boundary_vertices(&grid);
        let mut n_bound = 0;
        let mut n_interior = 0;
        for (i, &(u, v)) in uv.iter().enumerate() {
            let r = (u * u + v * v).sqrt();
            if on_bound[i] {
                assert!((r - 1.0).abs() < 1e-12, "boundary vertex {i} off circle: r={r}");
                n_bound += 1;
            } else {
                assert!(r < 1.0 - 1e-9, "interior vertex {i} not strictly inside: r={r}");
                n_interior += 1;
            }
        }
        assert_eq!(n_bound, 16, "grid(4,4) boundary count");
        assert_eq!(n_interior, 9, "grid(4,4) interior count");
        assert_no_flips(&grid, &uv);
    }

    /// The cotangent (harmonic) weighting yields a valid, flip-free embedding on
    /// the well-shaped grid with the (corner-free) **circle** boundary; the
    /// **square** boundary is exercised with the **uniform (Tutte)** weighting,
    /// whose convex-boundary guarantee holds regardless of corner alignment.
    /// `flatten_to_plane` produces a valid planar mesh (chi preserved).
    ///
    /// (Note: cotangent + square on a *rectangular* patch is deliberately not
    /// asserted flip-free — arc-length square mapping does not align the
    /// rectangle's corners with the square's, and harmonic weights carry no
    /// Tutte guarantee there. Uniform weighting is the robust choice for a
    /// square target; circle is robust for either weighting.)
    #[test]
    fn harmonic_grid_and_flatten() {
        let grid = primitives::grid(5, 3, 2.0);

        // Cotangent + circle: flip-free on a well-shaped mesh.
        let uv_c = parameterize(&grid, LaplacianWeighting::Cotangent, BoundaryShape::Circle)
            .expect("grid is a disk");
        assert_no_flips(&grid, &uv_c);

        // Uniform (Tutte) + square: flip-free by Tutte's theorem (convex target),
        // and every boundary vertex lands exactly on a side of [0,1]^2.
        let uv_s = parameterize(&grid, LaplacianWeighting::Uniform, BoundaryShape::Square)
            .expect("grid is a disk");
        assert_no_flips(&grid, &uv_s);
        let on_bound = crate::laplacian::boundary_vertices(&grid);
        for (i, &(u, v)) in uv_s.iter().enumerate() {
            if on_bound[i] {
                let on_side = u.abs() < 1e-9 || (u - 1.0).abs() < 1e-9 || v.abs() < 1e-9 || (v - 1.0).abs() < 1e-9;
                assert!(on_side, "boundary vertex {i} not on square: ({u},{v})");
            }
        }

        let flat = flatten_to_plane(&grid, LaplacianWeighting::Uniform, BoundaryShape::Circle)
            .expect("flatten");
        assert_eq!(flat.euler_characteristic(), grid.euler_characteristic(), "flatten preserves topology");
        assert!(flat.positions().iter().all(|p| p.z == 0.0), "flattened mesh is planar");
    }

    /// A **closed** mesh (a cube) has no boundary and must be rejected, not
    /// silently mis-parameterized.
    #[test]
    fn closed_mesh_is_rejected() {
        let cube = primitives::cube(2.0);
        assert!(matches!(
            parameterize(&cube, LaplacianWeighting::Uniform, BoundaryShape::Circle),
            Err(ParamError::NoBoundary)
        ));
    }
}
