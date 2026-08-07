// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Implements: O. Sorkine and M. Alexa, "As-Rigid-As-Possible Surface Modeling",
// Eurographics Symposium on Geometry Processing (SGP), 2007, pp. 109-116.
// Cotangent weights: U. Pinkall and K. Polthier, "Computing discrete minimal
// surfaces and their conjugates", Experimental Mathematics 2(1), 1993, 15-36.
// Written from the published formulation; no upstream source was copied.
// Blender analogue (architecture only): the Laplacian-deform / ARAP mesh tools.
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

//! **As-Rigid-As-Possible (ARAP) surface deformation** (Sorkine & Alexa, 2007).
//!
//! Blender analogue: the Laplacian-deform / "As Rigid As Possible" mesh tools.
//! This is the crate's most involved sparse-solve operator: it deforms a mesh to
//! match prescribed **handle** positions while keeping every local
//! neighbourhood **as rigid as possible** (bending and rotating, resisting
//! stretching/shearing). It reuses the cotangent-Laplacian machinery from
//! [`crate::laplacian`] — the global system matrix is exactly that Laplacian.
//!
//! ## Algorithm (local / global alternation)
//!
//! Minimize `E = Σ_i Σ_{j∈N(i)} w_ij ‖(p'_i − p'_j) − R_i (p_i − p_j)‖²` over the
//! deformed positions `p'` and per-vertex rotations `R_i ∈ SO(3)`, where `p` is
//! the rest pose and `w_ij = (cot α + cot β)/2` are the cotangent weights.
//!
//! - **Local step** (fix `p'`, solve each `R_i`): form the `3×3` covariance
//!   `S_i = Σ_j w_ij (p'_i − p'_j)(p_i − p_j)ᵀ` (deformed ⊗ rest) and take the
//!   closest rotation `R_i = U Vᵀ` (the orthogonal Procrustes solution, with a
//!   determinant sign-fix) from the SVD `S_i = U Σ Vᵀ` (`closest_rotation`) —
//!   the rotation that best maps the rest one-ring onto the deformed one.
//! - **Global step** (fix `{R_i}`, solve `p'`): solve `L p' = b` with
//!   `b_i = Σ_j (w_ij/2)(R_i + R_j)(p_i − p_j)`, where `L` is the cotangent
//!   Laplacian. Handle vertices are constrained (pinned to their targets), which
//!   grounds the otherwise-singular `L` — the exact same boundary-pinned SPD
//!   reduced system as [`crate::laplacian::laplacian_smooth`].
//!
//! Because `L` and the free/fixed partition are fixed across iterations, the
//! sparse matrix is **factorized once** (`faer` sparse Cholesky) and only the
//! right-hand side `b` is rebuilt each iteration. The energy `E` is non-increasing
//! (each step is an exact partial minimizer); [`arap_energy`] exposes it for
//! convergence checks.
//!
//! > **Untrusted AI-generated draft** until a human reviews it, per the
//! > workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
//! > reactor control, safety-critical, or licensing decisions.

use crate::laplacian::{edge_weights_from, mesh_triangles, LaplacianWeighting};
use crate::math::Vec3;
use crate::mesh::{Mesh, VertexId};

/// Errors from [`arap_deform`].
#[derive(Debug, thiserror::Error)]
pub enum ArapError {
    /// No handle/anchor constraints were given. Without at least one fixed
    /// vertex per connected component the cotangent Laplacian is singular (the
    /// constant/translation null space), so the solve is undefined.
    #[error("ARAP needs at least one handle/anchor constraint")]
    NoConstraints,
    /// The sparse system could not be assembled (bad index).
    #[error("ARAP matrix assembly failed")]
    Assembly,
    /// The reduced Laplacian is not positive definite — a component with no
    /// constrained vertex, or a very obtuse (non-Delaunay) mesh whose cotangent
    /// weights broke SPD.
    #[error("ARAP system is not positive definite (add a handle per component, or use a better-shaped mesh)")]
    NotPositiveDefinite,
}

/// A 3×3 matrix as row-major arrays (local per-element math, per the crate's
/// "fixed-size math stays out of the sparse backend" rule).
type M3 = [[f64; 3]; 3];

const IDENTITY3: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// Deform `mesh` (its positions are the rest pose) so the given `handles`
/// (vertex → target position) are met while every one-ring stays as rigid as
/// possible, via `iterations` local/global ARAP steps.
///
/// Handle vertices are pinned to their targets; all other vertices are solved
/// for. Topology is preserved (only positions change). More `iterations` = tighter
/// convergence; 3–10 is typically plenty. Returns the deformed mesh.
///
/// # Errors
///
/// [`ArapError::NoConstraints`] if `handles` is empty;
/// [`ArapError::NotPositiveDefinite`] / [`ArapError::Assembly`] on a solve/setup
/// failure (see those variants).
pub fn arap_deform(
    mesh: &Mesh,
    handles: &[(VertexId, Vec3)],
    iterations: u32,
) -> Result<Mesh, ArapError> {
    use faer::linalg::solvers::Solve;
    use faer::sparse::{SparseColMat, Triplet};
    use faer::{Mat, Side};

    if handles.is_empty() {
        return Err(ArapError::NoConstraints);
    }

    let n = mesh.vertex_count();
    let rest = mesh.positions();
    let faces: Vec<Vec<usize>> = mesh
        .polygons()
        .iter()
        .map(|f| f.iter().map(|v| v.0).collect())
        .collect();
    let tris = mesh_triangles(mesh);

    // Cotangent weights + one-ring adjacency (rest pose; fixed across iterations).
    let weights = edge_weights_from(&tris, &rest, LaplacianWeighting::Cotangent);
    let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
    let mut diag = vec![0.0f64; n];
    for (&(i, j), &w) in &weights {
        adj[i].push((j, w));
        adj[j].push((i, w));
        diag[i] += w;
        diag[j] += w;
    }

    // Fixed (handle) targets and free/fixed partition.
    let mut target: Vec<Option<Vec3>> = vec![None; n];
    for &(v, p) in handles {
        if v.0 < n {
            target[v.0] = Some(p);
        }
    }
    let mut free_of: Vec<Option<usize>> = vec![None; n];
    let mut free_verts: Vec<usize> = Vec::new();
    for v in 0..n {
        if target[v].is_none() {
            free_of[v] = Some(free_verts.len());
            free_verts.push(v);
        }
    }
    let nfree = free_verts.len();

    // Current deformed positions: rest, with handles snapped to targets.
    let mut cur = rest.clone();
    for v in 0..n {
        if let Some(t) = target[v] {
            cur[v] = t;
        }
    }
    if nfree == 0 {
        return Ok(Mesh::from_polygons(&cur, &faces));
    }

    // Assemble the reduced Laplacian A = L_ff (lower triangle) ONCE and factor it.
    // The constant part of the RHS from eliminating fixed neighbours is also
    // iteration-independent (handles don't move), so precompute it.
    let mut trips: Vec<Triplet<usize, usize, f64>> = Vec::new();
    let mut rhs_const = vec![[0.0f64; 3]; nfree];
    for (&(gi, gj), &w) in &weights {
        match (free_of[gi], free_of[gj]) {
            (Some(fi), Some(fj)) => {
                let (lo, hi) = if fi < fj { (fi, fj) } else { (fj, fi) };
                trips.push(Triplet::new(hi, lo, -w));
            }
            (Some(fi), None) => {
                let c = target[gj].expect("fixed vertex has a target");
                for d in 0..3 {
                    rhs_const[fi][d] += w * component(c, d);
                }
            }
            (None, Some(fj)) => {
                let c = target[gi].expect("fixed vertex has a target");
                for d in 0..3 {
                    rhs_const[fj][d] += w * component(c, d);
                }
            }
            (None, None) => {}
        }
    }
    for (fi, &gi) in free_verts.iter().enumerate() {
        trips.push(Triplet::new(fi, fi, diag[gi]));
    }
    let a = SparseColMat::<usize, f64>::try_new_from_triplets(nfree, nfree, &trips)
        .map_err(|_| ArapError::Assembly)?;
    let llt = a.sp_cholesky(Side::Lower).map_err(|_| ArapError::NotPositiveDefinite)?;

    for _ in 0..iterations {
        // LOCAL: per-vertex optimal rotation from the current positions.
        let rotations: Vec<M3> = (0..n).map(|i| optimal_rotation(i, &adj, &rest, &cur)).collect();

        // GLOBAL: b_i = Σ_j (w_ij/2)(R_i + R_j)(p_i − p_j), plus the constant
        // handle-elimination term; solve L_ff x = rhs.
        let mut rhs = rhs_const.clone();
        for (fi, &gi) in free_verts.iter().enumerate() {
            let mut b = Vec3::ZERO;
            for &(gj, w) in &adj[gi] {
                let rot_sum = mat3_add(rotations[gi], rotations[gj]);
                let e_rest = rest[gi].sub(rest[gj]);
                b = b.add(mat3_vec(rot_sum, e_rest).scale(0.5 * w));
            }
            rhs[fi][0] += b.x;
            rhs[fi][1] += b.y;
            rhs[fi][2] += b.z;
        }

        let bmat = Mat::<f64>::from_fn(nfree, 3, |i, j| rhs[i][j]);
        let x = llt.solve(&bmat);
        for (fi, &gi) in free_verts.iter().enumerate() {
            cur[gi] = Vec3::new(x[(fi, 0)], x[(fi, 1)], x[(fi, 2)]);
        }
    }

    Ok(Mesh::from_polygons(&cur, &faces))
}

/// The ARAP energy `E = Σ_i Σ_{j∈N(i)} w_ij ‖(d_i − d_j) − R_i (p_i − p_j)‖²` of a
/// deformed configuration `deformed` relative to `rest` (`R_i` the per-vertex
/// optimal rotation). Non-negative; non-increasing across [`arap_deform`]
/// iterations — a convergence diagnostic (see the module tests).
///
/// `deformed` must have one position per vertex, in [`crate::mesh::VertexId`]
/// order (e.g. `arap_deform(...)?.positions()`).
pub fn arap_energy(rest: &Mesh, deformed: &[Vec3]) -> f64 {
    let n = rest.vertex_count();
    let rp = rest.positions();
    let tris = mesh_triangles(rest);
    let weights = edge_weights_from(&tris, &rp, LaplacianWeighting::Cotangent);
    let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
    for (&(i, j), &w) in &weights {
        adj[i].push((j, w));
        adj[j].push((i, w));
    }

    let mut energy = 0.0;
    for i in 0..n {
        let r = optimal_rotation(i, &adj, &rp, deformed);
        for &(j, w) in &adj[i] {
            let e_rest = rp[i].sub(rp[j]);
            let e_def = deformed[i].sub(deformed[j]);
            let resid = e_def.sub(mat3_vec(r, e_rest));
            energy += w * resid.dot(resid);
        }
    }
    energy
}

// ---------------------------------------------------------------------------
// Local step: per-vertex optimal rotation + 3×3 helpers.
// ---------------------------------------------------------------------------

/// The optimal rotation for vertex `i`: build the covariance
/// `S_i = Σ_j w_ij (p_i − p_j)(d_i − d_j)ᵀ` over its one-ring (rest positions
/// `p`, deformed `d`) and return its closest rotation.
fn optimal_rotation(i: usize, adj: &[Vec<(usize, f64)>], rest: &[Vec3], def: &[Vec3]) -> M3 {
    // Covariance S_i = Σ w (d_i − d_j)(p_i − p_j)ᵀ (deformed ⊗ rest). Its
    // Procrustes rotation (`closest_rotation`, R = U Vᵀ) is the rotation that
    // best maps the rest one-ring onto the deformed one.
    let mut s = [[0.0f64; 3]; 3];
    for &(j, w) in &adj[i] {
        let e_rest = rest[i].sub(rest[j]);
        let e_def = def[i].sub(def[j]);
        outer_add(&mut s, w, e_def, e_rest);
    }
    closest_rotation(s)
}

/// Accumulate `w · u vᵀ` (outer product) into `s`.
fn outer_add(s: &mut M3, w: f64, u: Vec3, v: Vec3) {
    let ua = [u.x, u.y, u.z];
    let va = [v.x, v.y, v.z];
    for a in 0..3 {
        for b in 0..3 {
            s[a][b] += w * ua[a] * va[b];
        }
    }
}

/// The closest proper rotation (`det = +1`) to a `3×3` matrix `m` — the
/// orthogonal Procrustes / Kabsch solution. For the SVD `m = U Σ Vᵀ` this is
/// `R = U Vᵀ`, with the determinant sign-fix (negate the last column of `U` when
/// `det(U Vᵀ) < 0`, flipping the smallest singular direction) so `R` is a proper
/// rotation, not a reflection. For a rotation input it returns that rotation.
///
/// Uses `faer`'s dense SVD for the `3×3` decomposition, then does the small
/// matrix products with plain arrays. Returns the identity for a degenerate
/// covariance (empty one-ring) whose SVD fails.
fn closest_rotation(m: M3) -> M3 {
    let a = faer::mat![
        [m[0][0], m[0][1], m[0][2]],
        [m[1][0], m[1][1], m[1][2]],
        [m[2][0], m[2][1], m[2][2]],
    ];
    let svd = match a.svd() {
        Ok(s) => s,
        Err(_) => return IDENTITY3,
    };
    let u = svd.U();
    let v = svd.V();
    let mut uu: M3 = [
        [u[(0, 0)], u[(0, 1)], u[(0, 2)]],
        [u[(1, 0)], u[(1, 1)], u[(1, 2)]],
        [u[(2, 0)], u[(2, 1)], u[(2, 2)]],
    ];
    let vv: M3 = [
        [v[(0, 0)], v[(0, 1)], v[(0, 2)]],
        [v[(1, 0)], v[(1, 1)], v[(1, 2)]],
        [v[(2, 0)], v[(2, 1)], v[(2, 2)]],
    ];
    let vt = transpose3(vv);
    // R = U Vᵀ; if it is a reflection, negate U's last column (= U·diag(1,1,−1)).
    if det3(matmul3(uu, vt)) < 0.0 {
        for row in uu.iter_mut() {
            row[2] = -row[2];
        }
    }
    matmul3(uu, vt)
}

/// `a · b` for `3×3` matrices.
fn matmul3(a: M3, b: M3) -> M3 {
    let mut c = [[0.0f64; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            c[i][j] = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
        }
    }
    c
}

/// Transpose of a `3×3` matrix.
fn transpose3(a: M3) -> M3 {
    let mut t = [[0.0f64; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            t[i][j] = a[j][i];
        }
    }
    t
}

/// Determinant of a `3×3` matrix.
fn det3(a: M3) -> f64 {
    a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1])
        - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
        + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0])
}

/// Component-wise sum of two `3×3` matrices.
fn mat3_add(a: M3, b: M3) -> M3 {
    let mut c = [[0.0f64; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            c[i][j] = a[i][j] + b[i][j];
        }
    }
    c
}

/// Apply a `3×3` matrix to a [`Vec3`].
fn mat3_vec(m: M3, v: Vec3) -> Vec3 {
    Vec3::new(
        m[0][0] * v.x + m[0][1] * v.y + m[0][2] * v.z,
        m[1][0] * v.x + m[1][1] * v.y + m[1][2] * v.z,
        m[2][0] * v.x + m[2][1] * v.y + m[2][2] * v.z,
    )
}

/// Component `d` (0=x, 1=y, 2=z) of a [`Vec3`].
fn component(p: Vec3, d: usize) -> f64 {
    match d {
        0 => p.x,
        1 => p.y,
        _ => p.z,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    /// Apply a rigid transform (rotation `θ` about z + translation `t`).
    fn rigid(p: Vec3, theta: f64, t: Vec3) -> Vec3 {
        let (s, c) = theta.sin_cos();
        Vec3::new(c * p.x - s * p.y + t.x, s * p.x + c * p.y + t.y, p.z + t.z)
    }

    /// Methodology: **rigid-motion reproduction** (the cleanest analytic ARAP
    /// property). Constrain *every boundary vertex* of a `grid(5,5)` to its image
    /// under a rigid transform `T` (30° about z + a translation). ARAP's energy
    /// is zero for a global rigid motion (all `R_i = Q`, `p' = T(p)`), so the
    /// deformation converges to exactly `T(p)`. The local/global alternation
    /// converges **linearly**, so this checks the error *shrinks toward zero*
    /// with iterations and reaches a tight tolerance at high iteration count,
    /// with the energy driven to (near) zero.
    ///
    /// Results (measured 2026-07-23): max vertex error `0.190` (5 iters) →
    /// `0.036` (20) → `6.5e-4` (60); ARAP energy `4.0e-2` → `4.7e-7`. Asserted
    /// below: error strictly shrinks, is `< 2e-3` at 60 iterations, and the
    /// energy is `< 1e-5` there.
    #[test]
    fn arap_reproduces_a_rigid_motion() {
        let grid = primitives::grid(5, 5, 2.0);
        let boundary = crate::laplacian::boundary_vertices(&grid);
        let theta = std::f64::consts::FRAC_PI_6; // 30°
        let t = Vec3::new(0.5, -0.3, 0.2);

        let handles: Vec<(VertexId, Vec3)> = (0..grid.vertex_count())
            .filter(|&i| boundary[i])
            .map(|i| (VertexId(i), rigid(grid.positions()[i], theta, t)))
            .collect();

        let max_err = |it: u32| {
            let op = arap_deform(&grid, &handles, it).expect("arap solves").positions();
            (0..grid.vertex_count())
                .map(|i| op[i].sub(rigid(grid.positions()[i], theta, t)).length())
                .fold(0.0f64, f64::max)
        };
        let (e5, e60) = (max_err(5), max_err(60));
        assert!(e60 < e5, "error must shrink with iterations: {e5} -> {e60}");
        assert!(e60 < 2e-3, "rigid reproduction converges (maxerr at 60 iters = {e60})");

        let op60 = arap_deform(&grid, &handles, 60).expect("arap").positions();
        assert!(arap_energy(&grid, &op60) < 1e-5, "rigid motion drives ARAP energy to ~0");
    }

    /// Methodology: **bends without stretching + monotone energy.** A thin bar
    /// `grid(16, 2)` in the z=0 plane: pin the `x = −2` end column in place, lift
    /// the `x = +2` end column by `+1.5 z`. ARAP should *bend* the bar,
    /// preserving edge lengths far better than the 1-iteration (near-Laplacian)
    /// warm start, and the ARAP energy must not increase across iterations.
    ///
    /// Results (measured 2026-07-23): max relative edge-length change `0.290`
    /// (1 iter) → `0.068` (30 iters); ARAP energy decreases monotonically
    /// `0.806 → 0.459 → 0.345 → 0.281 → 0.242 → 0.218` over iterations 1..6.
    /// Asserted: `stretch_30 < stretch_1`, `stretch_30 < 0.08`, energy
    /// non-increasing, interior actually moved.
    #[test]
    fn arap_bar_bends_without_stretching_and_energy_decreases() {
        let bar = primitives::grid(16, 2, 4.0);
        let ps = bar.positions();
        let half = 2.0; // grid spans [-2, 2] on x (size 4.0)
        let mut handles: Vec<(VertexId, Vec3)> = Vec::new();
        for i in 0..bar.vertex_count() {
            if (ps[i].x + half).abs() < 1e-9 {
                handles.push((VertexId(i), ps[i])); // fixed end
            } else if (ps[i].x - half).abs() < 1e-9 {
                handles.push((VertexId(i), Vec3::new(ps[i].x, ps[i].y, ps[i].z + 1.5))); // lifted end
            }
        }

        // Rest edge lengths.
        let edges: Vec<(usize, usize, f64)> = {
            let tris = mesh_triangles(&bar);
            let mut set = std::collections::HashSet::new();
            let mut v = Vec::new();
            for [a, b, c] in tris {
                for &(i, j) in &[(a, b), (b, c), (c, a)] {
                    let k = if i < j { (i, j) } else { (j, i) };
                    if set.insert(k) {
                        v.push((k.0, k.1, ps[k.0].sub(ps[k.1]).length()));
                    }
                }
            }
            v
        };
        let max_rel_stretch = |m: &Mesh| {
            let q = m.positions();
            edges
                .iter()
                .map(|&(i, j, l0)| ((q[i].sub(q[j]).length() - l0) / l0).abs())
                .fold(0.0f64, f64::max)
        };

        let one = arap_deform(&bar, &handles, 1).expect("arap");
        let many = arap_deform(&bar, &handles, 30).expect("arap");
        let stretch_1 = max_rel_stretch(&one);
        let stretch_30 = max_rel_stretch(&many);
        assert!(stretch_30 < stretch_1, "iterations must reduce stretch: {stretch_1} -> {stretch_30}");
        assert!(stretch_30 < 0.08, "bent bar should barely stretch, got {stretch_30}");

        // Interior actually moved.
        let moved = many.positions().iter().zip(ps.iter()).any(|(a, b)| a.sub(*b).length() > 0.1);
        assert!(moved, "the bar must actually deform");

        // Energy is non-increasing across iterations.
        let mut prev = f64::INFINITY;
        for k in 1..=6 {
            let e = arap_energy(&bar, &arap_deform(&bar, &handles, k).unwrap().positions());
            assert!(e <= prev + 1e-9, "energy rose at k={k}: {prev} -> {e}");
            assert!(e >= 0.0, "energy is a sum of squares");
            prev = e;
        }
    }

    /// `closest_rotation` recovers a known rotation and always returns a proper
    /// rotation (orthonormal, `det = +1`), including the reflection case.
    #[test]
    fn closest_rotation_is_proper() {
        // A pure 40° rotation about z: its closest rotation is itself.
        let (s, c) = 0.7f64.sin_cos();
        let rot: M3 = [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]];
        let r = closest_rotation(rot);
        for i in 0..3 {
            for j in 0..3 {
                assert!((r[i][j] - rot[i][j]).abs() < 1e-9, "did not recover the rotation");
            }
        }
        // A reflection (det < 0) must map to a proper rotation.
        let refl: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]];
        let rr = closest_rotation(refl);
        assert!((det3(rr) - 1.0).abs() < 1e-9, "must be a proper rotation, det={}", det3(rr));
        // Orthonormal: Rᵀ R = I.
        let rtr = matmul3(transpose3(rr), rr);
        for i in 0..3 {
            for j in 0..3 {
                let expect = if i == j { 1.0 } else { 0.0 };
                assert!((rtr[i][j] - expect).abs() < 1e-9, "not orthonormal");
            }
        }
    }

    /// ARAP with no handles is an error (singular system), not a silent garbage
    /// result.
    #[test]
    fn arap_without_handles_errors() {
        let grid = primitives::grid(4, 4, 1.0);
        assert!(matches!(arap_deform(&grid, &[], 3), Err(ArapError::NoConstraints)));
    }
}
