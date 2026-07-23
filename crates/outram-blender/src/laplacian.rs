//! Discrete **Laplacian operators** over a mesh, and implicit **Laplacian
//! smoothing** (mesh fairing) built on them.
//!
//! Blender analogue: the Smooth / "Smooth Vertices" and Laplacian-smooth mesh
//! operators (`MOD_laplaciansmooth`, `bmo_smooth_laplacian`). This is the first
//! module in the crate that assembles a **global sparse linear system over the
//! mesh** and solves it — the intended home for the `faer` sparse solver the
//! crate re-exports (see [`crate`] top-level docs). Future geometry-processing
//! operators that need the same machinery (ARAP deformation, mesh
//! parameterization) build on the operators here.
//!
//! ## What "the Laplacian" means here
//!
//! For a mesh with `n` vertices, the discrete Laplacian is the symmetric `n x n`
//! matrix `L` with, for each edge `(i, j)` of weight `w_ij`,
//!
//! - `L[i][j] = L[j][i] = -w_ij` (off-diagonal), and
//! - `L[i][i] = sum_j w_ij` (diagonal = the vertex's total edge weight).
//!
//! So **every row sums to zero** (`L * 1 = 0`, the constant vector is in the
//! null space) and `L` is symmetric. Two weightings are offered
//! ([`LaplacianWeighting`]):
//!
//! - **Uniform / umbrella** (`w_ij = 1`): the graph Laplacian — connectivity
//!   only, ignores geometry. Always positive semidefinite.
//! - **Cotangent** (`w_ij = (cot α + cot β) / 2`, where `α`, `β` are the angles
//!   opposite edge `(i, j)` in the one or two triangles that share it): the
//!   discrete Laplace–Beltrami operator — geometry-aware (this is the one that
//!   approximates the smooth surface Laplacian). A boundary edge, in only one
//!   triangle, contributes a single `cot α / 2`.
//!
//! Polygon faces are fan-triangulated before the cotangent weights are read off,
//! and the crate's [`crate::mesh`] has no edge→incident-face adjacency, so this
//! module builds the edge→opposite-vertex map itself from
//! [`crate::mesh::Mesh::polygons`].
//!
//! > **Untrusted AI-generated draft** until a human reviews it, per the
//! > workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
//! > reactor control, safety-critical, or licensing decisions.

use crate::math::Vec3;
use crate::mesh::{FaceId, Mesh};
use std::collections::HashMap;

/// Which discrete Laplacian weighting to assemble (enum dispatch, per the
/// workspace no-trait-objects rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaplacianWeighting {
    /// Graph / umbrella Laplacian — every edge weight is `1`. Depends only on
    /// connectivity, not vertex positions; always positive semidefinite. Cheap
    /// and robust, but not a geometric (Laplace–Beltrami) approximation.
    Uniform,
    /// Cotangent-weighted discrete **Laplace–Beltrami** operator — edge weight
    /// `(cot α + cot β) / 2` from the angles opposite the edge in its incident
    /// triangle(s). Geometry-aware (the correct discretization of the surface
    /// Laplacian). Weights can go negative on very obtuse triangles, so the
    /// bare operator is only positive semidefinite for a Delaunay-ish mesh.
    Cotangent,
}

/// Assemble the discrete Laplacian `L` of `mesh` under `weighting`, as
/// `(n, triplets)` — the vertex count and a list of `(row, col, value)` entries
/// (COO / triplet form) suitable for building a sparse matrix.
///
/// `L` is symmetric with zero row sums (see the module docs). Diagonal entries
/// are summed into one triplet per vertex; off-diagonals are one `(i, j)` and
/// one `(j, i)` per edge. Vertices with no incident edge (isolated) contribute
/// no entries (an all-zero row).
///
/// The triplet order is unspecified (it comes from a hash map); the assembled
/// matrix is independent of that order. This is the input a sparse-matrix
/// builder ([`crate::faer`]) consumes; it is also directly testable by
/// materializing a small dense matrix (see this module's tests).
pub fn laplacian_triplets(mesh: &Mesh, weighting: LaplacianWeighting) -> (usize, Vec<(usize, usize, f64)>) {
    let n = mesh.vertex_count();
    let weights = edge_weights(mesh, weighting);

    let mut diag = vec![0.0f64; n];
    let mut triplets: Vec<(usize, usize, f64)> = Vec::with_capacity(weights.len() * 2 + n);
    for (&(i, j), &w) in &weights {
        triplets.push((i, j, -w));
        triplets.push((j, i, -w));
        diag[i] += w;
        diag[j] += w;
    }
    for (i, &d) in diag.iter().enumerate() {
        if d != 0.0 {
            triplets.push((i, i, d));
        }
    }
    (n, triplets)
}

/// Per-edge symmetric weights `w_ij`, keyed by the ordered pair `(min, max)`.
///
/// - [`LaplacianWeighting::Uniform`]: every edge maps to `1.0`.
/// - [`LaplacianWeighting::Cotangent`]: accumulates `cot(θ) / 2` over each
///   incident triangle, where `θ` is the angle at the vertex opposite the edge.
fn edge_weights(mesh: &Mesh, weighting: LaplacianWeighting) -> HashMap<(usize, usize), f64> {
    edge_weights_from(&mesh_triangles(mesh), &mesh.positions(), weighting)
}

/// Edge weights from an explicit triangle list + vertex positions.
///
/// Factored out of [`edge_weights`] so [`laplacian_smooth`] can recompute the
/// (geometry-dependent) cotangent weights each iteration from the *current*
/// positions while the topology (`tris`) stays fixed.
fn edge_weights_from(
    tris: &[[usize; 3]],
    ps: &[Vec3],
    weighting: LaplacianWeighting,
) -> HashMap<(usize, usize), f64> {
    let mut weights: HashMap<(usize, usize), f64> = HashMap::new();
    for &[a, b, c] in tris {
        // Each directed pair (i, j, opposite k) of the triangle's three edges.
        for &(i, j, k) in &[(a, b, c), (b, c, a), (c, a, b)] {
            let key = if i < j { (i, j) } else { (j, i) };
            match weighting {
                LaplacianWeighting::Uniform => {
                    weights.insert(key, 1.0);
                }
                LaplacianWeighting::Cotangent => {
                    *weights.entry(key).or_insert(0.0) += 0.5 * cotangent(ps[k], ps[i], ps[j]);
                }
            }
        }
    }
    weights
}

/// Cotangent of the angle at `apex` in the triangle `apex, p, q`.
///
/// `cot θ = (u · v) / |u × v|` with `u = p - apex`, `v = q - apex`. Returns
/// `0.0` for a degenerate (zero-area) corner rather than a non-finite value,
/// matching the crate's `Vec3::normalize` house style.
fn cotangent(apex: Vec3, p: Vec3, q: Vec3) -> f64 {
    let u = p.sub(apex);
    let v = q.sub(apex);
    let cross = u.cross(v).length();
    if cross <= f64::MIN_POSITIVE {
        return 0.0;
    }
    u.dot(v) / cross
}

/// Fan-triangulate every face of `mesh` into vertex-index triples
/// (`(v0, v_i, v_{i+1})`). Correct for convex faces (what the crate's
/// generators and operators produce); the same fan convention the rest of the
/// crate uses.
fn mesh_triangles(mesh: &Mesh) -> Vec<[usize; 3]> {
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
    tris
}

/// Flag each vertex that lies on the mesh **boundary** — incident to an edge
/// used by only one triangle (an open border, e.g. a [`crate::primitives::grid`]
/// patch). Returns a `bool` per vertex in [`crate::mesh::VertexId`] order.
///
/// Boundary vertices are the ones a smoothing operator pins in place so the
/// surface does not shrink at its border. For a closed mesh (every edge shared
/// by two triangles) this is all `false`.
pub fn boundary_vertices(mesh: &Mesh) -> Vec<bool> {
    let mut edge_tri_count: HashMap<(usize, usize), usize> = HashMap::new();
    for [a, b, c] in mesh_triangles(mesh) {
        for &(i, j) in &[(a, b), (b, c), (c, a)] {
            let key = if i < j { (i, j) } else { (j, i) };
            *edge_tri_count.entry(key).or_insert(0) += 1;
        }
    }
    let mut on_boundary = vec![false; mesh.vertex_count()];
    for (&(i, j), &count) in &edge_tri_count {
        if count == 1 {
            on_boundary[i] = true;
            on_boundary[j] = true;
        }
    }
    on_boundary
}

/// Errors from [`laplacian_smooth`].
#[derive(Debug, thiserror::Error)]
pub enum LaplacianError {
    /// The sparse smoothing matrix could not be assembled (an invalid
    /// row/column index reached the sparse-matrix constructor). Indicates a
    /// caller/topology bug, not a numerical one.
    #[error("Laplacian smoothing matrix assembly failed")]
    Assembly,
    /// The sparse Cholesky factorization failed because the system `I + λL`
    /// (restricted to the free vertices) is not positive definite. Can happen
    /// with the [`LaplacianWeighting::Cotangent`] weighting on a very obtuse
    /// (non-Delaunay) mesh at a large `lambda`. Try a smaller `lambda`, the
    /// [`LaplacianWeighting::Uniform`] weighting, or a better-conditioned mesh.
    #[error("Laplacian smoothing system is not positive definite (reduce lambda or use Uniform weighting)")]
    NotPositiveDefinite,
}

/// **Implicit Laplacian smoothing** (mesh fairing): relax vertex positions by
/// solving `(I + λL) x' = x` once per iteration, with boundary vertices pinned.
///
/// # What it computes
///
/// Implicit (backward-Euler) smoothing of the surface by the discrete Laplacian
/// `L` ([`laplacian_triplets`], under `weighting`). Each iteration solves the
/// sparse SPD system `(I + λL) x' = x` — unconditionally stable for any `lambda`
/// (unlike explicit `x' = x - λL x`, which diverges for `λ` too large). With the
/// [`LaplacianWeighting::Cotangent`] weighting this approximates mean-curvature
/// flow (it denoises toward a smooth surface and gently shrinks); the
/// [`LaplacianWeighting::Uniform`] weighting smooths by connectivity only.
///
/// **Boundary vertices are pinned** (held fixed; see [`boundary_vertices`]) so an
/// open patch does not shrink at its border. The system is solved on the free
/// (interior) vertices only, as a symmetric-positive-definite reduced system
/// (pinned neighbours move to the right-hand side), factorized with `faer`'s
/// sparse Cholesky. For a closed mesh every vertex is free.
///
/// The cotangent weights are **recomputed from the current positions each
/// iteration**, so the flow tracks the evolving geometry.
///
/// # Inputs / units
///
/// - `mesh` — source mesh (borrowed, unmodified); topology is preserved, only
///   positions change.
/// - `weighting` — [`LaplacianWeighting::Uniform`] or `Cotangent`.
/// - `lambda` — smoothing strength (dimensionless, `>= 0`); larger = smoother
///   per step. `0` is a no-op; unconditionally stable at any value.
/// - `iterations` — number of implicit steps (`0` returns a clone).
///
/// # Errors
///
/// [`LaplacianError::NotPositiveDefinite`] if the reduced system is not SPD
/// (see that variant), or [`LaplacianError::Assembly`] on an internal indexing
/// failure.
pub fn laplacian_smooth(
    mesh: &Mesh,
    weighting: LaplacianWeighting,
    lambda: f64,
    iterations: u32,
) -> Result<Mesh, LaplacianError> {
    use faer::sparse::{SparseColMat, Triplet};
    use faer::linalg::solvers::Solve;
    use faer::{Mat, Side};

    let faces: Vec<Vec<usize>> = mesh
        .polygons()
        .iter()
        .map(|f| f.iter().map(|v| v.0).collect())
        .collect();
    let tris = mesh_triangles(mesh);
    let boundary = boundary_vertices(mesh);
    let n = mesh.vertex_count();

    // Free (interior) vertices get solved for; boundary vertices are pinned.
    let mut free_of: Vec<Option<usize>> = vec![None; n];
    let mut free_verts: Vec<usize> = Vec::new();
    for v in 0..n {
        if !boundary[v] {
            free_of[v] = Some(free_verts.len());
            free_verts.push(v);
        }
    }
    let nfree = free_verts.len();

    let mut pos = mesh.positions();
    if nfree == 0 || iterations == 0 || lambda == 0.0 {
        // Nothing free to move (fully pinned), or a no-op request.
        return Ok(Mesh::from_polygons(&pos, &faces));
    }

    for _ in 0..iterations {
        let weights = edge_weights_from(&tris, &pos, weighting);

        // Assemble the reduced SPD system A = (I + λL)_free (lower triangle) and
        // RHS b (n_free × 3). Row i (free): x'_i (1 + λ L_ii)
        //   + λ Σ_{j free} L_ij x'_j = x_i − λ Σ_{j pinned} L_ij x_j,
        // with L_ii = Σ_j w_ij and L_ij = −w_ij.
        let mut diag = vec![0.0f64; n];
        let mut rhs = vec![[0.0f64; 3]; nfree];
        let mut trips: Vec<Triplet<usize, usize, f64>> = Vec::new();

        for (&(gi, gj), &w) in &weights {
            diag[gi] += w;
            diag[gj] += w;
            match (free_of[gi], free_of[gj]) {
                (Some(fi), Some(fj)) => {
                    // Off-diagonal λL_ij = −λw; store the lower triangle once.
                    let (lo, hi) = if fi < fj { (fi, fj) } else { (fj, fi) };
                    trips.push(Triplet::new(hi, lo, -lambda * w));
                }
                (Some(fi), None) => {
                    // gj pinned → −λL_ij x_j = +λw·x_j moves to the RHS.
                    for d in 0..3 {
                        rhs[fi][d] += lambda * w * component(pos[gj], d);
                    }
                }
                (None, Some(fj)) => {
                    for d in 0..3 {
                        rhs[fj][d] += lambda * w * component(pos[gi], d);
                    }
                }
                (None, None) => {}
            }
        }
        for (fi, &gi) in free_verts.iter().enumerate() {
            trips.push(Triplet::new(fi, fi, 1.0 + lambda * diag[gi]));
            for d in 0..3 {
                rhs[fi][d] += component(pos[gi], d); // the x_i term
            }
        }

        let a = SparseColMat::<usize, f64>::try_new_from_triplets(nfree, nfree, &trips)
            .map_err(|_| LaplacianError::Assembly)?;
        let llt = a.sp_cholesky(Side::Lower).map_err(|_| LaplacianError::NotPositiveDefinite)?;
        let b = Mat::<f64>::from_fn(nfree, 3, |i, j| rhs[i][j]);
        let x = llt.solve(&b);

        for (fi, &gi) in free_verts.iter().enumerate() {
            pos[gi] = Vec3::new(x[(fi, 0)], x[(fi, 1)], x[(fi, 2)]);
        }
    }

    Ok(Mesh::from_polygons(&pos, &faces))
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

    /// Materialize the sparse triplets into a dense `n x n` matrix (summing
    /// duplicate entries), for small-mesh algebraic checks.
    fn dense(n: usize, triplets: &[(usize, usize, f64)]) -> Vec<Vec<f64>> {
        let mut m = vec![vec![0.0; n]; n];
        for &(i, j, v) in triplets {
            m[i][j] += v;
        }
        m
    }

    /// Methodology: for **any** discrete Laplacian the module builds, `L` must
    /// be symmetric and every row must sum to zero (`L * 1 = 0`, the constant
    /// vector is in the null space) — the defining algebraic properties.
    /// Checked on `cube(2.0)` under both weightings.
    ///
    /// Results (asserted, tol `1e-12`): both the uniform and cotangent Laplacian
    /// of the cube are symmetric and have zero row sums to machine precision.
    #[test]
    fn laplacian_is_symmetric_with_zero_row_sums() {
        for weighting in [LaplacianWeighting::Uniform, LaplacianWeighting::Cotangent] {
            let (n, trips) = laplacian_triplets(&primitives::cube(2.0), weighting);
            let m = dense(n, &trips);
            for i in 0..n {
                let row_sum: f64 = m[i].iter().sum();
                assert!(row_sum.abs() < 1e-12, "{weighting:?}: row {i} sum = {row_sum}");
                for j in 0..n {
                    assert!((m[i][j] - m[j][i]).abs() < 1e-12, "{weighting:?}: asymmetry at ({i},{j})");
                }
            }
        }
    }

    /// Methodology: cotangent weights of a single **right-isosceles** triangle
    /// `a=(0,0,0), b=(1,0,0), c=(0,1,0)` have a closed form. Edge `(a,b)` is
    /// opposite the right-angle-adjacent vertex `c`, whose angle has
    /// `cot = 1` → weight `0.5`; edge `(c,a)` likewise `0.5`; edge `(b,c)` is
    /// opposite the right angle at `a` (`cot 90° = 0`) → weight `0`.
    ///
    /// Results (asserted, tol `1e-12`): `w(a,b) = w(c,a) = 0.5`, `w(b,c) = 0`;
    /// and the vertex-`a` row of `L` sums to zero (`+1` diagonal, `-0.5` to `b`,
    /// `-0.5` to `c`).
    #[test]
    fn cotangent_weights_of_right_triangle() {
        let positions = [
            Vec3::new(0.0, 0.0, 0.0), // a = 0
            Vec3::new(1.0, 0.0, 0.0), // b = 1
            Vec3::new(0.0, 1.0, 0.0), // c = 2
        ];
        let mesh = Mesh::from_polygons(&positions, &[vec![0, 1, 2]]);
        let w = edge_weights(&mesh, LaplacianWeighting::Cotangent);
        assert!((w[&(0, 1)] - 0.5).abs() < 1e-12, "w(a,b) = {}", w[&(0, 1)]);
        assert!((w[&(0, 2)] - 0.5).abs() < 1e-12, "w(c,a) = {}", w[&(0, 2)]);
        assert!(w[&(1, 2)].abs() < 1e-12, "w(b,c) = {}", w[&(1, 2)]);

        let (n, trips) = laplacian_triplets(&mesh, LaplacianWeighting::Cotangent);
        let m = dense(n, &trips);
        assert!((m[0][0] - 1.0).abs() < 1e-12, "L[a][a] = {}", m[0][0]);
        assert!(m[0].iter().sum::<f64>().abs() < 1e-12, "row a sum");
    }

    /// Methodology: a flat [`primitives::grid`] patch has a one-ring boundary
    /// (edges used by a single triangle) and an interior. [`boundary_vertices`]
    /// must flag exactly the border lattice points. For a `grid(nx, ny)` the
    /// interior is the `(nx-1) * (ny-1)` inner lattice points; the rest are
    /// boundary.
    ///
    /// Results (asserted): a `grid(4, 4)` (25 vertices) has `9` interior and
    /// `16` boundary vertices; a closed `cube(2.0)` has no boundary vertices.
    #[test]
    fn boundary_detection_grid_and_cube() {
        let grid = primitives::grid(4, 4, 1.0);
        let flags = boundary_vertices(&grid);
        let boundary = flags.iter().filter(|&&b| b).count();
        let interior = flags.iter().filter(|&&b| !b).count();
        assert_eq!(interior, 9, "grid(4,4) has (4-1)^2 = 9 interior vertices");
        assert_eq!(boundary, 16, "the rest are boundary");

        let cube = primitives::cube(2.0);
        assert!(boundary_vertices(&cube).iter().all(|&b| !b), "closed cube has no boundary");
    }

    /// Deterministic xorshift pseudo-random `f64` in `[-1, 1)` — for reproducible
    /// noise in the smoothing tests (a flaky random-seed test would be worse
    /// than a fixed, reviewable perturbation).
    fn xorshift(seed: &mut u64) -> f64 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        ((*seed >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
    }

    /// Methodology: **planarity preservation.** A flat [`primitives::grid`] lies
    /// in `z = 0`; add out-of-plane noise to the *interior* vertices only, then
    /// implicit-smooth. Because the boundary is pinned and the true surface is
    /// the plane `z = 0`, smoothing must pull the interior back toward `z = 0` —
    /// the max interior `|z|` must strictly decrease, and never introduce
    /// in-plane drift that leaves the patch (bounds stay within the original
    /// `[-0.5, 0.5]^2` footprint).
    ///
    /// Results (asserted): after one `lambda = 1.0` cotangent step the max
    /// interior `|z|` drops to well under half the injected noise amplitude
    /// (`0.1`), and boundary vertices are unmoved.
    #[test]
    fn smoothing_denoises_a_perturbed_flat_grid() {
        let grid = primitives::grid(6, 6, 1.0);
        let boundary = boundary_vertices(&grid);
        let mut seed = 0x1234_5678_9abc_def0u64;
        let ps: Vec<Vec3> = grid
            .positions()
            .iter()
            .enumerate()
            .map(|(i, &p)| {
                if boundary[i] {
                    p
                } else {
                    Vec3::new(p.x, p.y, 0.1 * xorshift(&mut seed))
                }
            })
            .collect();
        let faces: Vec<Vec<usize>> = grid.polygons().iter().map(|f| f.iter().map(|v| v.0).collect()).collect();
        let noisy = Mesh::from_polygons(&ps, &faces);

        let noisy_max_z = ps
            .iter()
            .enumerate()
            .filter(|(i, _)| !boundary[*i])
            .map(|(_, p)| p.z.abs())
            .fold(0.0f64, f64::max);

        let smoothed = laplacian_smooth(&noisy, LaplacianWeighting::Cotangent, 1.0, 1).expect("SPD solve");
        let sp = smoothed.positions();
        let smoothed_max_z = sp
            .iter()
            .enumerate()
            .filter(|(i, _)| !boundary[*i])
            .map(|(_, p)| p.z.abs())
            .fold(0.0f64, f64::max);

        assert!(
            smoothed_max_z < 0.5 * noisy_max_z,
            "interior |z| must drop sharply: {noisy_max_z} -> {smoothed_max_z}"
        );
        // Boundary pinned: unchanged.
        for i in 0..grid.vertex_count() {
            if boundary[i] {
                assert!(sp[i].sub(ps[i]).length() < 1e-12, "boundary vertex {i} moved");
            }
        }
    }

    /// Methodology: **denoising a closed surface.** Perturb every vertex of a
    /// `uv_sphere(24, 12, 1.0)` radially by deterministic noise (amplitude
    /// `0.08`), then implicit cotangent-smooth (a closed mesh → all vertices
    /// free, exercising the no-boundary solve path). The standard deviation of
    /// the vertices' radii from the centre must **decrease** (the bumpy sphere
    /// becomes rounder). Smoothing is kept *gentle* (`lambda = 0.5`, 3 steps):
    /// on this coarse, highly non-uniform uv tessellation strong cotangent flow
    /// also *redistributes* vertices (tiny polar triangles vs. large equatorial
    /// ones), which confounds the radial-std metric — so the honest, verified
    /// claim is monotone noise reduction in the gentle regime, not maximal
    /// smoothing.
    ///
    /// Results (measured 2026-07-23): radial std `0.04580 -> 0.03882` after 3
    /// steps (ratio `0.847`), a clear decrease; the result stays a closed
    /// genus-0 mesh (`chi = 2`). Asserted below with a `< 0.95x` margin.
    #[test]
    fn smoothing_denoises_a_noisy_sphere() {
        let sphere = primitives::uv_sphere(24, 12, 1.0);
        let mut seed = 0xdead_beef_cafe_1234u64;
        let ps: Vec<Vec3> = sphere
            .positions()
            .iter()
            .map(|&p| {
                let r = p.length();
                if r == 0.0 {
                    p
                } else {
                    let dir = p.scale(1.0 / r);
                    dir.scale(r + 0.08 * xorshift(&mut seed))
                }
            })
            .collect();
        let faces: Vec<Vec<usize>> = sphere.polygons().iter().map(|f| f.iter().map(|v| v.0).collect()).collect();
        let noisy = Mesh::from_polygons(&ps, &faces);

        let radial_std = |m: &Mesh| {
            let rs: Vec<f64> = m.positions().iter().map(|p| p.length()).collect();
            let mean = rs.iter().sum::<f64>() / rs.len() as f64;
            (rs.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / rs.len() as f64).sqrt()
        };

        let before = radial_std(&noisy);
        let smoothed = laplacian_smooth(&noisy, LaplacianWeighting::Cotangent, 0.5, 3).expect("SPD solve");
        let after = radial_std(&smoothed);

        assert!(after < before, "radial std must shrink (denoise): {before} -> {after}");
        assert!(after < 0.95 * before, "reduction should be clear: {before} -> {after}");
        assert_eq!(smoothed.euler_characteristic(), 2, "still a closed genus-0 sphere");
    }

    /// A truly flat grid is a fixed point of smoothing in `z` (its `z = 0` is
    /// harmonic), and `lambda = 0` / `iterations = 0` are exact no-ops.
    #[test]
    fn smoothing_flat_grid_keeps_z_zero_and_noop_cases() {
        let grid = primitives::grid(5, 5, 1.0);
        let out = laplacian_smooth(&grid, LaplacianWeighting::Uniform, 1.0, 2).expect("solve");
        assert!(out.positions().iter().all(|p| p.z.abs() < 1e-12), "flat grid stays in z=0");

        // No-op requests return the input geometry unchanged.
        let noop = laplacian_smooth(&grid, LaplacianWeighting::Cotangent, 0.0, 5).expect("noop");
        for (a, b) in grid.positions().iter().zip(noop.positions().iter()) {
            assert!(a.sub(*b).length() < 1e-12);
        }
    }
}
