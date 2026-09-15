// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
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

//! GAMG — Geometric-Agglomerated Multi-Grid solver for symmetric LDU systems.
//!
//! This is a **serial, algebraic** port of OpenFOAM's `Foam::GAMGSolver` with
//! `algebraicPairGAMGAgglomeration`. "Algebraic" means the coarse grids are
//! built purely from the matrix coefficients (the face weights are `|upper|`),
//! with no mesh geometry — so it works on any symmetric [`LduMatrix`], not just
//! one with a backing mesh.
//!
//! ## Why multigrid
//!
//! A DIC-preconditioned CG ([`conjugate_gradient`](crate::ldu_matrix::solvers::conjugate_gradient()))
//! needs O(√κ) ≈ O(Nₓ) iterations on the pressure Poisson equation — the count
//! grows as the mesh is refined. Multigrid eliminates error at every length
//! scale by recursing onto coarser grids, so it converges in a handful of
//! V-cycles almost independently of mesh size. It is OpenFOAM's default
//! pressure solver for this reason.
//!
//! ## The algorithm (recursive correction-scheme V-cycle)
//!
//! Each V-cycle is the textbook correction scheme with pre- and post-smoothing
//! (`GamgCycle::solve_level`):
//!
//! 1. **Pre-smooth** the current level with Gauss-Seidel (`N_PRE_SWEEPS`).
//! 2. Form the residual `r = b − A·x` and **restrict** it to the next coarser
//!    level (additive, `restrict_field`).
//! 3. **Recurse** to compute the coarse correction; the coarsest level is
//!    solved directly by dense LU (`solve_coarsest`).
//! 4. **Prolong** the correction back (injection, `prolong_field`) and add it.
//! 5. **Post-smooth** the current level (`N_POST_SWEEPS`).
//!
//! Pre- *and* post-smoothing makes this a symmetric V-cycle, which converges far
//! faster than a post-smoothing-only sawtooth. OpenFOAM's `GAMGSolver::Vcycle`
//! reaches similar robustness with `nPreSweeps = 0` plus correction *scaling*;
//! the symmetric form is the cleaner equivalent here.
//!
//! The outer loop ([`gamg`]) repeats V-cycles until the relative residual
//! `‖r‖₂ / ‖b‖₂` falls below `settings.tolerance` — the same convergence metric
//! [`conjugate_gradient`](crate::ldu_matrix::solvers::conjugate_gradient()) uses, so the two solvers
//! are interchangeable under one `SolverSettings`.
//!
//! ## Restrictions
//!
//! Symmetric matrices only (`lower == upper`), which is exactly the pressure
//! Poisson case. The coarse matrices inherit symmetry, so the whole hierarchy
//! stays symmetric and the Gauss-Seidel smoother / dense coarsest solve need no
//! special face ordering.

use crate::ldu_matrix::fv_matrix::{SolverPerformance, SolverSettings};
use crate::ldu_matrix::ldu_matrix::LduMatrix;
use crate::ldu_matrix::solvers::gauss_seidel::gauss_seidel;
use crate::matrix::SquareMatrix;
use std::collections::HashMap;

/// Agglomerate to this few cells (or fewer) before stopping the hierarchy and
/// switching to the direct coarsest solve. Matches OpenFOAM's default
/// `nCellsInCoarsestLevel = 10`.
const N_CELLS_COARSEST: usize = 10;
/// Hard cap on the number of coarse levels (safety against pathological meshes).
const MAX_LEVELS: usize = 50;
/// Gauss-Seidel pre-smoothing sweeps per level (before restriction).
const N_PRE_SWEEPS: usize = 2;
/// Gauss-Seidel post-smoothing sweeps per level (after prolongation).
const N_POST_SWEEPS: usize = 2;

/// One coarse level of the multigrid hierarchy.
struct GamgLevel {
    /// Map from each cell of the level **above** to its coarse cell here.
    restrict_map: Vec<usize>,
    /// Number of cells at this level (`= matrix.n_cells`).
    n_coarse: usize,
    /// The agglomerated coarse matrix.
    matrix: LduMatrix,
}

/// A precomputed GAMG hierarchy for one finest matrix.
///
/// Built once by [`GamgCycle::build`]; `solve_level` then runs as many V-cycles
/// as the outer solver needs. The finest matrix is **not** stored here — it is
/// passed back into the cycle so the same hierarchy can be reused if only the
/// right-hand side changes.
struct GamgCycle {
    levels: Vec<GamgLevel>,
}

impl GamgCycle {
    /// Build the coarse-grid hierarchy from `finest` by repeated pairwise
    /// algebraic agglomeration until the level has `≤ N_CELLS_COARSEST` cells
    /// (or no further coarsening is possible).
    fn build(finest: &LduMatrix) -> GamgCycle {
        let mut levels: Vec<GamgLevel> = Vec::new();
        // OpenFOAM's `pairGAMGAgglomeration` alternates sweep direction each
        // level to avoid directional bias in the agglomeration.
        let mut forward = true;

        loop {
            let current: &LduMatrix = match levels.last() {
                Some(l) => &l.matrix,
                None => finest,
            };
            if current.n_cells <= N_CELLS_COARSEST || levels.len() >= MAX_LEVELS {
                break;
            }

            // Algebraic face weights: the off-diagonal magnitude.
            let weights: Vec<f64> = current.upper.iter().map(|u| u.abs()).collect();
            let (map, n_coarse) = pair_agglomerate(current, &weights, forward);
            forward = !forward;

            // Stop if a level failed to coarsen (would loop forever).
            if n_coarse >= current.n_cells {
                break;
            }

            let coarse = agglomerate_matrix(current, &map, n_coarse);
            levels.push(GamgLevel {
                restrict_map: map,
                n_coarse,
                matrix: coarse,
            });
        }

        GamgCycle { levels }
    }

    /// One recursive V-cycle level (correction scheme).
    ///
    /// Solves `mat·x = b` approximately, updating `x` in place. `next_level` is
    /// the index into `self.levels` of the next coarser grid; when it runs past
    /// the end, `mat` is the coarsest level and is solved directly.
    fn solve_level(&self, mat: &LduMatrix, x: &mut Vec<f64>, b: &[f64], next_level: usize) {
        // Coarsest level: exact dense solve.
        if next_level >= self.levels.len() {
            *x = solve_coarsest(mat, b);
            return;
        }

        // Pre-smooth, then restrict the residual to the next coarser grid.
        smooth(mat, b, x, N_PRE_SWEEPS);
        let r = mat.residual(x, b); // b − mat·x
        let lvl = &self.levels[next_level];
        let r_coarse = restrict_field(&r, &lvl.restrict_map, lvl.n_coarse);

        // Recurse for the coarse-grid correction (zero initial guess).
        let mut e_coarse = vec![0.0_f64; lvl.n_coarse];
        self.solve_level(&lvl.matrix, &mut e_coarse, &r_coarse, next_level + 1);

        // Prolong the correction, then apply it with an optimal line-search
        // scale factor `sf = (r·c) / (c·mat·c)`. This minimises the residual
        // along the correction direction, so it can never increase ‖r‖ — the
        // robustness fix that unsmoothed (piecewise-constant) aggregation needs:
        // without it the V-cycle can diverge.
        let correction = prolong_field(&e_coarse, &lvl.restrict_map);
        let ac = mat.multiply(&correction);
        let denom = dot(&correction, &ac);
        let sf = if denom.abs() > 1e-300 {
            dot(&r, &correction) / denom
        } else {
            1.0
        };
        for (xi, ci) in x.iter_mut().zip(&correction) {
            *xi += sf * ci;
        }
        smooth(mat, b, x, N_POST_SWEEPS);
    }
}

/// Solve a symmetric SPD LDU system with GAMG (algebraic multigrid).
///
/// Drop-in counterpart of
/// [`conjugate_gradient`](crate::ldu_matrix::solvers::conjugate_gradient()):
/// same signature, same `‖r‖₂ / ‖b‖₂` convergence metric, and the same warm
/// start — pass `Some(previous_solution)` as `x0` to start from the last time
/// step's field. The GAMG hierarchy is rebuilt each call (agglomeration is O(n)
/// and cheap next to the V-cycles).
///
/// Requires `ldu` to be **symmetric** (`lower == upper`); this holds for the
/// pressure Poisson equation assembled by `fvm::laplacian`.
///
/// # Example
///
/// ```
/// use outram_foam_basic_lib::prelude::*;
///
/// // 1-D Poisson −∇²φ = 1 on [0,1], φ(0)=φ(1)=0, 63 interior points.
/// let n = 63;
/// let h = 1.0 / (n + 1) as f64;
/// let owner: Vec<usize> = (0..n - 1).collect();
/// let neighbour: Vec<usize> = (1..n).collect();
/// let mut m = LduMatrix::new(n, owner, neighbour);
/// let c = 1.0 / (h * h);
/// m.diag = vec![2.0 * c; n];
/// m.upper = vec![-c; n - 1];
/// m.lower = vec![-c; n - 1];
/// let b = vec![1.0; n];
///
/// let settings = SolverSettings { tolerance: 1e-8, max_iter: 100 };
/// let (x, perf) = gamg(&m, &b, None, &settings);
/// assert!(perf.converged);
/// // Exact solution is φ = x(1−x)/2; check the midpoint.
/// let mid = (n / 2) as f64 * h;
/// assert!((x[n / 2] - mid * (1.0 - mid) / 2.0).abs() < 1e-3);
/// ```
pub fn gamg(
    ldu: &LduMatrix,
    b: &[f64],
    x0: Option<&[f64]>,
    settings: &SolverSettings,
) -> (Vec<f64>, SolverPerformance) {
    let n = ldu.n_cells;
    debug_assert_eq!(b.len(), n);

    let mut x = match x0 {
        Some(g) => {
            debug_assert_eq!(g.len(), n);
            g.to_vec()
        }
        None => vec![0.0_f64; n],
    };

    let cycle = GamgCycle::build(ldu);

    // Too small to coarsen — solve the whole thing directly.
    if cycle.levels.is_empty() {
        let x = solve_coarsest(ldu, b);
        let r = ldu.residual(&x, b);
        let b_norm = l2(b).max(1e-300);
        let final_residual = l2(&r) / b_norm;
        return (
            x,
            SolverPerformance {
                n_iterations: 1,
                final_residual,
                converged: true,
            },
        );
    }

    let b_norm = l2(b).max(1e-300);
    let mut final_residual = l2(&ldu.residual(&x, b)) / b_norm;

    // Warm start may already satisfy the tolerance.
    if final_residual < settings.tolerance {
        return (
            x,
            SolverPerformance {
                n_iterations: 0,
                final_residual,
                converged: true,
            },
        );
    }

    let mut n_iter = 0;
    while n_iter < settings.max_iter && final_residual >= settings.tolerance {
        cycle.solve_level(ldu, &mut x, b, 0);
        final_residual = l2(&ldu.residual(&x, b)) / b_norm;
        n_iter += 1;
    }

    (
        x,
        SolverPerformance {
            n_iterations: n_iter,
            final_residual,
            converged: final_residual < settings.tolerance,
        },
    )
}

// ── Agglomeration ──────────────────────────────────────────────────────────

/// Pairwise algebraic agglomeration — a port of
/// `Foam::pairGAMGAgglomeration::agglomerate`.
///
/// Returns `(restrict_map, n_coarse)` where `restrict_map[fine_cell]` is the
/// coarse cell it belongs to. Each cell is paired with its strongest-coupled
/// (largest `weights[f]`) still-unpaired neighbour; cells that cannot pair are
/// merged into an existing neighbouring cluster, and any leftovers become
/// singletons. `forward` selects the sweep direction (alternated per level).
fn pair_agglomerate(ldu: &LduMatrix, weights: &[f64], forward: bool) -> (Vec<usize>, usize) {
    let n = ldu.n_cells;
    let nf = ldu.n_internal_faces;

    // Build a cell → faces CSR adjacency (each internal face lists for both
    // its owner and neighbour).
    let mut n_nbrs = vec![0usize; n];
    for f in 0..nf {
        n_nbrs[ldu.owner[f]] += 1;
        n_nbrs[ldu.neighbour[f]] += 1;
    }
    let mut offsets = vec![0usize; n + 1];
    for c in 0..n {
        offsets[c + 1] = offsets[c] + n_nbrs[c];
    }
    let mut cell_faces = vec![0usize; offsets[n]];
    let mut fill = vec![0usize; n];
    for f in 0..nf {
        let o = ldu.owner[f];
        let nb = ldu.neighbour[f];
        cell_faces[offsets[o] + fill[o]] = f;
        fill[o] += 1;
        cell_faces[offsets[nb] + fill[nb]] = f;
        fill[nb] += 1;
    }

    let mut map = vec![-1isize; n];
    let mut n_coarse: isize = 0;

    for cellfi in 0..n {
        let celli = if forward { cellfi } else { n - cellfi - 1 };
        if map[celli] >= 0 {
            continue;
        }

        // Prefer pairing with the strongest neighbour whose endpoints are both
        // still unassigned.
        let mut match_face: isize = -1;
        let mut max_w = f64::NEG_INFINITY;
        for &f in &cell_faces[offsets[celli]..offsets[celli + 1]] {
            if map[ldu.owner[f]] < 0 && map[ldu.neighbour[f]] < 0 && weights[f] > max_w {
                match_face = f as isize;
                max_w = weights[f];
            }
        }

        if match_face >= 0 {
            let f = match_face as usize;
            map[ldu.owner[f]] = n_coarse;
            map[ldu.neighbour[f]] = n_coarse;
            n_coarse += 1;
        } else {
            // No free pair: join the strongest-coupled existing cluster.
            let mut cl_face: isize = -1;
            let mut cl_w = f64::NEG_INFINITY;
            for &f in &cell_faces[offsets[celli]..offsets[celli + 1]] {
                if weights[f] > cl_w {
                    cl_face = f as isize;
                    cl_w = weights[f];
                }
            }
            if cl_face >= 0 {
                let f = cl_face as usize;
                map[celli] = map[ldu.owner[f]].max(map[ldu.neighbour[f]]);
            }
        }
    }

    // Any cell still unassigned becomes its own coarse cell.
    for cellfi in 0..n {
        let celli = if forward { cellfi } else { n - cellfi - 1 };
        if map[celli] < 0 {
            map[celli] = n_coarse;
            n_coarse += 1;
        }
    }

    // Backward sweeps number cells in reverse — flip so coarse indices increase
    // with the original ordering (matches OpenFOAM's `!forward_` branch).
    if !forward {
        n_coarse -= 1;
        for m in map.iter_mut() {
            *m = n_coarse - *m;
        }
        n_coarse += 1;
    }

    (
        map.into_iter().map(|m| m as usize).collect(),
        n_coarse as usize,
    )
}

/// Build the coarse symmetric matrix by restricting `fine` through
/// `restrict_map` — a port of the symmetric branch of
/// `GAMGSolver::agglomerateMatrix` plus the coarse-face construction from
/// `GAMGAgglomeration::agglomerateLduAddressing`.
///
/// Coarse diagonal = sum of fine diagonals in each cluster, plus both
/// off-diagonal coefficients of every fine face that becomes intra-cluster.
/// Coarse off-diagonals = sum of fine `upper` over the fine faces collapsing
/// onto each coarse face (deduplicated by coarse-cell pair).
fn agglomerate_matrix(fine: &LduMatrix, map: &[usize], n_coarse: usize) -> LduMatrix {
    let nf = fine.n_internal_faces;

    let mut coarse_diag = vec![0.0_f64; n_coarse];
    for c in 0..fine.n_cells {
        coarse_diag[map[c]] += fine.diag[c];
    }

    // Build coarse faces, deduplicated by (lo, hi) coarse-cell pair.
    let mut coarse_owner: Vec<usize> = Vec::new();
    let mut coarse_neighbour: Vec<usize> = Vec::new();
    let mut face_map: HashMap<(usize, usize), usize> = HashMap::new();
    // ≥0: coarse face index; <0: encodes intra-cluster cell as −(cell+1).
    let mut face_restrict = vec![0isize; nf];

    for f in 0..nf {
        let co = map[fine.owner[f]];
        let cn = map[fine.neighbour[f]];
        if co == cn {
            face_restrict[f] = -((co as isize) + 1);
        } else {
            let key = if co < cn { (co, cn) } else { (cn, co) };
            let cf = *face_map.entry(key).or_insert_with(|| {
                coarse_owner.push(key.0);
                coarse_neighbour.push(key.1);
                coarse_owner.len() - 1
            });
            face_restrict[f] = cf as isize;
        }
    }

    let n_cf = coarse_owner.len();
    let mut coarse_upper = vec![0.0_f64; n_cf];
    for f in 0..nf {
        let cf = face_restrict[f];
        if cf >= 0 {
            coarse_upper[cf as usize] += fine.upper[f];
        } else {
            // Intra-cluster face folds both its coefficients into the diagonal
            // (symmetric: upper + lower = 2·upper).
            coarse_diag[(-1 - cf) as usize] += 2.0 * fine.upper[f];
        }
    }

    let mut m = LduMatrix::new(n_coarse, coarse_owner, coarse_neighbour);
    m.diag = coarse_diag;
    m.lower = coarse_upper.clone();
    m.upper = coarse_upper;
    m
}

// ── Inter-grid transfer ──────────────────────────────────────────────────────

/// Additive restriction (fine → coarse): `cf[map[i]] += ff[i]`.
/// Port of `GAMGAgglomeration::restrictField`.
fn restrict_field(fine: &[f64], map: &[usize], n_coarse: usize) -> Vec<f64> {
    let mut cf = vec![0.0_f64; n_coarse];
    for (i, &c) in map.iter().enumerate() {
        cf[c] += fine[i];
    }
    cf
}

/// Injection prolongation (coarse → fine): `ff[i] = cf[map[i]]`.
/// Port of `GAMGAgglomeration::prolongField` (interpolateCorrection = false).
fn prolong_field(coarse: &[f64], map: &[usize]) -> Vec<f64> {
    map.iter().map(|&c| coarse[c]).collect()
}

// ── Smoothing, scaling, coarsest solve ───────────────────────────────────────

/// Gauss-Seidel smoother: `n_sweeps` forward sweeps (no convergence test).
fn smooth(a: &LduMatrix, source: &[f64], x: &mut Vec<f64>, n_sweeps: usize) {
    if n_sweeps == 0 {
        return;
    }
    // tol = 0 makes `gauss_seidel` run the full sweep count (residual ≥ 0).
    gauss_seidel(a, source, x, 0.0, n_sweeps);
}

/// Direct dense LU solve of the coarsest level — `directSolveCoarsest` via
/// [`SquareMatrix`]. The coarsest matrix has only a handful of cells, so the
/// O(n³) factorisation is negligible. Falls back to Gauss-Seidel if the dense
/// matrix is singular.
fn solve_coarsest(a: &LduMatrix, source: &[f64]) -> Vec<f64> {
    let n = a.n_cells;
    let mut dense = SquareMatrix::new(n);
    for c in 0..n {
        dense.add(c, c, a.diag[c]);
    }
    for f in 0..a.n_internal_faces {
        let o = a.owner[f];
        let nb = a.neighbour[f];
        dense.add(o, nb, a.upper[f]);
        dense.add(nb, o, a.lower[f]);
    }
    match dense.solve(source) {
        Ok(x) => x,
        Err(_) => {
            let mut x = vec![0.0_f64; n];
            gauss_seidel(a, source, &mut x, 1e-12, 1000);
            x
        }
    }
}

/// Euclidean (L2) norm of a slice.
#[inline]
fn l2(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

/// Dot product of two equal-length slices.
#[inline]
fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// −∇²φ = 1 on [0,1] with φ(0)=φ(1)=0; exact φ = x(1−x)/2.
    fn poisson_1d(n: usize) -> (LduMatrix, Vec<f64>) {
        let h = 1.0 / (n + 1) as f64;
        let owner: Vec<usize> = (0..n - 1).collect();
        let neighbour: Vec<usize> = (1..n).collect();
        let mut m = LduMatrix::new(n, owner, neighbour);
        let c = 1.0 / (h * h);
        m.diag = vec![2.0 * c; n];
        m.upper = vec![-c; n - 1];
        m.lower = vec![-c; n - 1];
        (m, vec![1.0; n])
    }

    #[test]
    fn gamg_solves_1d_poisson() {
        let n = 200;
        let h = 1.0 / (n + 1) as f64;
        let (m, b) = poisson_1d(n);
        let settings = SolverSettings {
            tolerance: 1e-9,
            max_iter: 100,
        };
        let (x, perf) = gamg(&m, &b, None, &settings);
        assert!(
            perf.converged,
            "GAMG did not converge: res = {}",
            perf.final_residual
        );
        for i in 0..n {
            let xi = (i + 1) as f64 * h;
            let exact = xi * (1.0 - xi) / 2.0;
            assert!((x[i] - exact).abs() < 1e-3, "cell {i}: {} vs {exact}", x[i]);
        }
    }

    #[test]
    fn gamg_cycle_count_is_mesh_independent() {
        // The point of multigrid: the V-cycle count to a fixed tolerance does
        // not blow up as the mesh is refined. (Unsmoothed aggregation is not
        // textbook-optimal standalone — hence the loose absolute bound — but it
        // must stay bounded across an 8× refinement, unlike PCG whose count
        // grows ∝ Nₓ.)
        let settings = SolverSettings {
            tolerance: 1e-8,
            max_iter: 200,
        };
        let (m1, b1) = poisson_1d(250);
        let (m2, b2) = poisson_1d(2000);
        let (_, p1) = gamg(&m1, &b1, None, &settings);
        let (_, p2) = gamg(&m2, &b2, None, &settings);
        assert!(p1.converged && p2.converged);
        // 8× more cells must not need more than ~1.5× the cycles.
        assert!(
            p2.n_iterations <= (p1.n_iterations * 3) / 2 + 2,
            "cycle count grew with mesh: {} → {}",
            p1.n_iterations,
            p2.n_iterations
        );
    }

    #[test]
    fn gamg_warm_start_returns_immediately() {
        let (m, b) = poisson_1d(200);
        let settings = SolverSettings {
            tolerance: 1e-8,
            max_iter: 100,
        };
        let (x, _) = gamg(&m, &b, None, &settings);
        // Re-solving from the converged field needs zero cycles.
        let (_, perf) = gamg(&m, &b, Some(&x), &settings);
        assert_eq!(perf.n_iterations, 0);
        assert!(perf.converged);
    }

    #[test]
    fn gamg_handles_tiny_matrix() {
        // Below the coarsening threshold → direct solve, no hierarchy.
        let mut m = LduMatrix::new(3, vec![0, 1], vec![1, 2]);
        m.diag = vec![2.0, 2.0, 2.0];
        m.upper = vec![-1.0, -1.0];
        m.lower = vec![-1.0, -1.0];
        let b = vec![1.0, 1.0, 1.0];
        let settings = SolverSettings {
            tolerance: 1e-10,
            max_iter: 50,
        };
        let (x, perf) = gamg(&m, &b, None, &settings);
        assert!(perf.converged);
        // A·x = b solution: [1.5, 2.0, 1.5].
        assert!((x[0] - 1.5).abs() < 1e-9);
        assert!((x[1] - 2.0).abs() < 1e-9);
        assert!((x[2] - 1.5).abs() < 1e-9);
    }
}
