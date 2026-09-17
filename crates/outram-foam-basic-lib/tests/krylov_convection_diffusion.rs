// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
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

//! Verification of the asymmetric Krylov solve path against the incumbent
//! Gauss-Seidel path, on a real 2-D finite-volume convection-diffusion system.
//!
//! # Why this test exists
//!
//! `FvMatrix::solve` uses Gauss-Seidel; `solve_cg` and `solve_gamg` require a
//! symmetric matrix and therefore cannot be used on any equation carrying a
//! convection term (upwinding puts the face flux on the donor side only, so
//! `lower[f] != upper[f]`). This test exercises
//! `FvMatrix::solve_bicgstab` / `solve_gmres`, which route the same matrix
//! through the crate's preconditioned BiCGStab / restarted GMRES kernels.
//!
//! # Methodology
//!
//! **Discretised system.** A uniform Cartesian 2-D mesh of `nx x ny` cells on
//! the unit square (`dx = 1/nx`, `dy = 1/ny`, unit depth), built cell-by-cell
//! with real cell volumes, cell centres, face-area vectors and face centres —
//! not a hand-written matrix. Steady scalar transport
//!
//! ```text
//!     div(phi T) - laplacian(Gamma, T) = 0
//! ```
//!
//! is assembled with the crate's own Layer-3 operators,
//! `fvm::div(&phi, &t) - fvm::laplacian(&gamma, &t)`, where
//!
//! - `U = (1, 0.5, 0) m/s` — a uniform, off-axis (so both face families carry
//!   convection) velocity,
//! - `phi_f = U . Sf` — the face volumetric flux `[m^3/s]`,
//! - `Gamma` — a constant diffusivity `[m^2/s]`, swept over **two regimes** so
//!   the comparison is not cherry-picked:
//!   `Gamma = 1e-3` (cell Peclet number `Pe = |U| dx / Gamma = 25`,
//!   convection-dominated) and `Gamma = 1` (`Pe = 0.025`, diffusion-dominated).
//!   Both matrices are asymmetric; they differ in how diagonally dominant they
//!   are, which is exactly what Gauss-Seidel's convergence rate depends on.
//!
//! Boundary conditions: `T = 1` fixed on the inflow faces (left `x = 0` and
//! bottom `y = 0`), zero-gradient on the outflow faces (right and top).
//!
//! **Pass criterion.** All solvers are given the identical matrix, the identical
//! source, and the identical tolerance `1e-8`. Because the incumbent
//! `gauss_seidel` reports an L1-scaled residual and the Krylov path reports the
//! relative 2-norm residual, neither reported number is used for the comparison:
//! the test recomputes one common measure, the **true relative 2-norm residual**
//! `||b - A x||_2 / ||b||_2`, from the returned field for every solver. Every
//! Krylov solve must converge below `1e-7` in that measure in **both** regimes;
//! additionally, in the diffusion-dominated regime BiCGStab + ILU(0) must reach
//! a residual at least 1000x tighter than Gauss-Seidel's in at least 5x fewer
//! iterations.
//!
//! # Results (measured 2026-08-07, release build, `nx = ny = 40`, 1600 cells)
//!
//! Recorded verbatim from the run of
//! `krylov_beats_gauss_seidel_on_convection_diffusion`
//! (`cargo test --release -p outram-foam-basic-lib --test krylov_convection_diffusion
//! -- --nocapture`), tolerance `1e-8`, `max_iter = 1000`.
//!
//! **Convection-dominated (`Gamma = 1e-3`, `Pe = 25`):**
//!
//! | Solver | Preconditioner | Iterations | True rel. 2-norm residual |
//! |---|---|---|---|
//! | Gauss-Seidel (`solve`, incumbent) | — | 21 | 1.638e-09 |
//! | BiCGStab (`solve_bicgstab`) | Jacobi | 52 | 9.203e-09 |
//! | BiCGStab (`solve_bicgstab`) | ILU(0) | **6** | 5.454e-09 |
//! | GMRES(30) (`solve_gmres`) | ILU(0) | 9 | 2.836e-09 |
//!
//! **Diffusion-dominated (`Gamma = 1`, `Pe = 0.025`):**
//!
//! | Solver | Preconditioner | Iterations | True rel. 2-norm residual |
//! |---|---|---|---|
//! | Gauss-Seidel (`solve`, incumbent) | — | 1000 (hit `max_iter`) | **1.417e-01** |
//! | BiCGStab (`solve_bicgstab`) | Jacobi | 173 | 7.930e-09 |
//! | BiCGStab (`solve_bicgstab`) | ILU(0) | **50** | 2.737e-09 |
//! | GMRES(30) (`solve_gmres`) | ILU(0) | 154 | 9.568e-09 |
//!
//! **Interpretation.**
//!
//! - At **high** Peclet number the upwind convection term contributes `|phi_f|`
//!   to the diagonal, so the matrix is strongly diagonally dominant and
//!   Gauss-Seidel is already fast (21 sweeps). ILU(0)-preconditioned BiCGStab is
//!   still ~3.5x cheaper in iterations, but the incumbent path was not broken
//!   here. This is reported rather than omitted: the Krylov path is *not* a
//!   uniform win on every system.
//! - At **low** Peclet number the matrix degenerates towards the 5-point
//!   Laplacian, loses that convection-supplied dominance, and Gauss-Seidel falls
//!   back to the `O(kappa)` elliptic rate: it **fails to converge at all**,
//!   exhausting 1000 sweeps still at a relative residual of `1.417e-01` — seven
//!   orders of magnitude short of the requested `1e-8`. BiCGStab + ILU(0) meets
//!   the tolerance in 50 iterations: **20x fewer iterations and a 5.2e7x
//!   smaller residual**. This is the regime a viscous momentum equation or a
//!   conduction-dominated energy equation actually sits in, and it is the case
//!   the previous asymmetric path could not solve.
//! - ILU(0) beats Jacobi in both regimes (6 vs 52, and 50 vs 173), matching the
//!   pairing OpenFOAM ships as `PBiCGStab` + `DILU`.
//!
//! **Caveat / scope.** These are measured convergence numbers for this crate's
//! linear solvers on this discretised system, on one machine, at one mesh size.
//! They verify the *linear algebra* — that the returned `x` satisfies `A x = b`,
//! cross-checked against a dense LU factorisation of the same matrix in
//! [`krylov_matches_dense_lu_on_convection_diffusion`]. They say **nothing**
//! about the physical accuracy of the first-order-upwind discretisation itself,
//! which is not validated here and is known to be first-order and numerically
//! diffusive. No comparison against an external CFD code has been made.

use std::sync::Arc;

use outram_foam_basic_lib::fields::boundary::bc::PatchField;
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::fields::surface_field::SurfaceScalarField;
use outram_foam_basic_lib::fields::vol_field::VolScalarField;
use outram_foam_basic_lib::fv_operators::fvm;
use outram_foam_basic_lib::ldu_matrix::{
    FvMatrix, KrylovMethod, KrylovOptions, LduMatrix, PreconditionerKind, SolverSettings,
};
use outram_foam_basic_lib::matrix::SquareMatrix;
use outram_foam_basic_lib::mesh::fv_mesh::{BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind};
use outram_foam_basic_lib::primitives::Vector3;

// ── mesh + system construction ───────────────────────────────────────────────

/// Patch index order used by [`cartesian_2d`]: left, right, bottom, top.
const P_LEFT: usize = 0;
const P_RIGHT: usize = 1;
const P_BOTTOM: usize = 2;
const P_TOP: usize = 3;

/// Build a uniform Cartesian 2-D mesh of `nx x ny` cells on the unit square with
/// unit depth in z.
///
/// Cell `(i, j)` has index `j * nx + i`. Internal faces are the `x`-normal faces
/// first (`(i,j) | (i+1,j)`), then the `y`-normal faces (`(i,j) | (i,j+1)`);
/// every face has `owner < neighbour`, as the LDU addressing requires. The four
/// boundary patches follow in the order left, right, bottom, top. There are no
/// `z`-normal faces: this is a genuinely 2-D mesh, not a one-cell-thick 3-D one.
fn cartesian_2d(nx: usize, ny: usize) -> Arc<FvMesh> {
    let dx = 1.0 / nx as f64;
    let dy = 1.0 / ny as f64;
    let dz = 1.0;
    let cell = |i: usize, j: usize| j * nx + i;

    let mut owner = Vec::new();
    let mut neighbour = Vec::new();
    let mut sf = Vec::new();
    let mut cf = Vec::new();

    // Internal x-normal faces: outward normal from owner is +x.
    for j in 0..ny {
        for i in 0..nx - 1 {
            owner.push(cell(i, j));
            neighbour.push(cell(i + 1, j));
            sf.push(Vector3::new(dy * dz, 0.0, 0.0));
            cf.push(Vector3::new(
                (i + 1) as f64 * dx,
                (j as f64 + 0.5) * dy,
                0.5 * dz,
            ));
        }
    }
    // Internal y-normal faces: outward normal from owner is +y.
    for j in 0..ny - 1 {
        for i in 0..nx {
            owner.push(cell(i, j));
            neighbour.push(cell(i, j + 1));
            sf.push(Vector3::new(0.0, dx * dz, 0.0));
            cf.push(Vector3::new(
                (i as f64 + 0.5) * dx,
                (j + 1) as f64 * dy,
                0.5 * dz,
            ));
        }
    }
    let n_internal = owner.len();

    // Boundary patches, in the order left, right, bottom, top.
    let mut patches = Vec::new();
    let mut start = n_internal;

    for j in 0..ny {
        owner.push(cell(0, j));
        sf.push(Vector3::new(-dy * dz, 0.0, 0.0));
        cf.push(Vector3::new(0.0, (j as f64 + 0.5) * dy, 0.5 * dz));
    }
    patches.push(BoundaryPatch::new("left", start, ny, PatchKind::Patch));
    start += ny;

    for j in 0..ny {
        owner.push(cell(nx - 1, j));
        sf.push(Vector3::new(dy * dz, 0.0, 0.0));
        cf.push(Vector3::new(1.0, (j as f64 + 0.5) * dy, 0.5 * dz));
    }
    patches.push(BoundaryPatch::new("right", start, ny, PatchKind::Patch));
    start += ny;

    for i in 0..nx {
        owner.push(cell(i, 0));
        sf.push(Vector3::new(0.0, -dx * dz, 0.0));
        cf.push(Vector3::new((i as f64 + 0.5) * dx, 0.0, 0.5 * dz));
    }
    patches.push(BoundaryPatch::new("bottom", start, nx, PatchKind::Patch));
    start += nx;

    for i in 0..nx {
        owner.push(cell(i, ny - 1));
        sf.push(Vector3::new(0.0, dx * dz, 0.0));
        cf.push(Vector3::new((i as f64 + 0.5) * dx, 1.0, 0.5 * dz));
    }
    patches.push(BoundaryPatch::new("top", start, nx, PatchKind::Patch));

    let n_cells = nx * ny;
    let centres: Vec<Vector3> = (0..n_cells)
        .map(|c| {
            let i = c % nx;
            let j = c / nx;
            Vector3::new((i as f64 + 0.5) * dx, (j as f64 + 0.5) * dy, 0.5 * dz)
        })
        .collect();

    Arc::new(
        FvMeshBuilder::new()
            .n_cells(n_cells)
            .n_internal_faces(n_internal)
            .owner(owner)
            .neighbour(neighbour)
            .patches(patches)
            .cell_volumes(vec![dx * dy * dz; n_cells])
            .cell_centres(centres)
            .face_area_vectors(sf)
            .face_centres(cf)
            .build()
            .expect("2-D Cartesian mesh must validate"),
    )
}

/// Assemble the steady convection-diffusion matrix
/// `div(phi T) - laplacian(Gamma, T)` on a `cartesian_2d` mesh.
///
/// `u` is the uniform velocity `[m/s]`, `gamma_value` the diffusivity
/// `[m^2/s]`, and `t_inlet` the fixed inflow value of the transported scalar.
/// Left and bottom are Dirichlet (`FixedValue(t_inlet)`), right and top are
/// zero-gradient, so the problem is well posed and the matrix nonsingular.
fn convection_diffusion(
    mesh: Arc<FvMesh>,
    u: Vector3,
    gamma_value: f64,
    t_inlet: f64,
) -> (FvMatrix, Vec<f64>) {
    // Transported scalar with the inflow/outflow BCs applied.
    let n_cells = mesh.n_cells;
    let boundary = vec![
        PatchField::fixed_value(mesh.patches[P_LEFT].size, t_inlet),
        PatchField::zero_gradient(mesh.patches[P_RIGHT].size),
        PatchField::fixed_value(mesh.patches[P_BOTTOM].size, t_inlet),
        PatchField::zero_gradient(mesh.patches[P_TOP].size),
    ];
    let t = VolScalarField::new("T", mesh.clone(), Field::zeros(n_cells), boundary);

    // Face volumetric flux phi_f = U . Sf  [m^3/s].
    let phi_int: Vec<f64> = (0..mesh.n_internal_faces)
        .map(|f| u.dot(mesh.face_area_vectors[f]))
        .collect();
    let phi_bnd: Vec<PatchField<f64>> = mesh
        .patches
        .iter()
        .map(|p| {
            let vals: Vec<f64> = (0..p.size)
                .map(|k| u.dot(mesh.face_area_vectors[p.start + k]))
                .collect();
            // Only `values` is read by `fvm::div` / `fvm::laplacian` for the
            // flux field; the BC kind on `phi` itself is immaterial.
            let mut pf = PatchField::zero_gradient(p.size);
            pf.values = Field::new(vals);
            pf
        })
        .collect();
    let phi = SurfaceScalarField::new("phi", mesh.clone(), Field::new(phi_int), phi_bnd);

    let gamma = SurfaceScalarField::uniform("Gamma", mesh.clone(), gamma_value);

    // Sign convention: this crate's `fvm::laplacian` returns the matrix of the
    // *positive-definite* operator `-div(Gamma grad T)` (positive diagonal,
    // negative off-diagonals) — the opposite sign to OpenFOAM's `fvm::laplacian`,
    // which is `+div(Gamma grad T)`. Verified against the pure-conduction test
    // in `src/fluid_thermo/solid_thermo.rs`, which solves `laplacian(kappa, T)
    // == 0` and recovers the correct linear profile. So the steady transport
    // equation `div(phi T) - div(Gamma grad T) = 0` is assembled here as a
    // **sum**, not a difference.
    let mat = fvm::div(&phi, &t) + fvm::laplacian(&gamma, &t);
    let b: Vec<f64> = mat.source.iter().copied().collect();
    (mat, b)
}

// ── measurement helpers ──────────────────────────────────────────────────────

/// True relative 2-norm residual `||b - A x||_2 / ||b||_2` — the single common
/// convergence measure this test uses for *every* solver, so that solvers
/// reporting different internal norms can be compared honestly.
fn true_rel_residual(a: &LduMatrix, b: &[f64], x: &[f64]) -> f64 {
    let r = a.residual(x, b);
    let rn: f64 = r.iter().map(|v| v * v).sum::<f64>().sqrt();
    let bn: f64 = b.iter().map(|v| v * v).sum::<f64>().sqrt();
    rn / bn
}

/// Materialise an `LduMatrix` as a dense `SquareMatrix` for a direct LU
/// cross-check. Only used on small systems.
fn dense_of(a: &LduMatrix) -> SquareMatrix {
    let mut m = SquareMatrix::new(a.n_cells);
    for i in 0..a.n_cells {
        m.set(i, i, a.diag[i]);
    }
    for f in 0..a.n_internal_faces {
        m.set(a.owner[f], a.neighbour[f], a.upper[f]);
        m.set(a.neighbour[f], a.owner[f], a.lower[f]);
    }
    m
}

/// Max absolute difference between two vectors.
fn max_abs_diff(x: &[f64], y: &[f64]) -> f64 {
    x.iter()
        .zip(y)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0_f64, f64::max)
}

// ── tests ────────────────────────────────────────────────────────────────────

/// **Acceptance test.** Preconditioned Krylov beats Gauss-Seidel on a real
/// asymmetric 2-D convection-diffusion system.
///
/// Methodology and both full measured results tables are in this file's
/// module-level `//!` documentation. In one line: on a 40x40 (1600-cell)
/// **diffusion-dominated** (`Pe = 0.025`) system at tolerance `1e-8`,
/// Gauss-Seidel exhausts its 1000-sweep budget still at a true relative 2-norm
/// residual of **1.417e-01**, while ILU(0)-preconditioned BiCGStab converges in
/// **50** iterations to **2.737e-09** — 20x fewer iterations and a 5.2e7x
/// smaller residual. In the **convection-dominated** (`Pe = 25`) regime
/// Gauss-Seidel is already competitive (21 sweeps to 1.638e-09) and BiCGStab +
/// ILU(0) merely wins on iterations (6); that case is measured and reported too
/// rather than omitted, and is deliberately *not* asserted as a win.
///
/// The assertions below are loose relative to those measurements so the test
/// pins the *qualitative* claim (every Krylov solve converges; Gauss-Seidel does
/// not, in the low-Peclet regime) rather than exact iteration counts, which are
/// legitimately sensitive to floating-point summation order.
#[test]
fn krylov_beats_gauss_seidel_on_convection_diffusion() {
    let (nx, ny) = (40, 40);
    let settings = SolverSettings {
        tolerance: 1e-8,
        max_iter: 1000,
    };
    let u = Vector3::new(1.0, 0.5, 0.0);

    // Two regimes of the same equation, distinguished by the cell Peclet number
    // Pe = |U| dx / Gamma. Both matrices are asymmetric; they differ in how
    // diagonally dominant they are, which is exactly what Gauss-Seidel's
    // convergence rate depends on.
    for (label, gamma) in [
        ("convection-dominated", 1.0e-3),
        ("diffusion-dominated", 1.0e0),
    ] {
        let mesh = cartesian_2d(nx, ny);
        let (mat, b) = convection_diffusion(mesh, u, gamma, 1.0);
        let pe = 1.0 * (1.0 / nx as f64) / gamma;
        eprintln!(
            "\n=== {label}: {nx}x{ny} = {} cells, Gamma = {gamma:.0e}, cell Pe = {pe:.3} ===",
            nx * ny
        );
        assert!(
            mat.ldu
                .lower
                .iter()
                .zip(&mat.ldu.upper)
                .any(|(l, u)| (l - u).abs() > 1e-12),
            "{label}: system must actually be asymmetric for this test to mean anything"
        );

        // Incumbent path: Gauss-Seidel.
        let (t_gs, p_gs) = mat.solve("T_gs", settings);
        let r_gs = true_rel_residual(&mat.ldu, &b, t_gs.internal.as_slice());
        eprintln!(
            "Gauss-Seidel (solve)        : {:4} iters, true rel. 2-norm residual {:.3e}",
            p_gs.n_iterations, r_gs
        );

        // New path: BiCGStab (Jacobi and ILU(0)) and GMRES(30) with ILU(0).
        let (t_jac, p_jac) = mat.solve_bicgstab(
            "T_bicg_jacobi",
            KrylovOptions::with_preconditioner(PreconditionerKind::Jacobi),
            settings,
        );
        let r_jac = true_rel_residual(&mat.ldu, &b, t_jac.internal.as_slice());
        eprintln!(
            "BiCGStab + Jacobi           : {:4} iters, true rel. 2-norm residual {:.3e}",
            p_jac.n_iterations, r_jac
        );

        let (t_ilu, p_ilu) = mat.solve_bicgstab("T_bicg_ilu0", KrylovOptions::default(), settings);
        let r_ilu = true_rel_residual(&mat.ldu, &b, t_ilu.internal.as_slice());
        eprintln!(
            "BiCGStab + ILU(0)           : {:4} iters, true rel. 2-norm residual {:.3e}",
            p_ilu.n_iterations, r_ilu
        );

        let (t_gm, p_gm) = mat.solve_gmres("T_gmres_ilu0", KrylovOptions::default(), settings);
        let r_gm = true_rel_residual(&mat.ldu, &b, t_gm.internal.as_slice());
        eprintln!(
            "GMRES(30) + ILU(0)          : {:4} iters, true rel. 2-norm residual {:.3e}",
            p_gm.n_iterations, r_gm
        );

        // (a) In BOTH regimes, every Krylov solve must converge to the requested
        //     tolerance in the true relative 2-norm.
        assert!(
            p_jac.converged,
            "{label}: BiCGStab+Jacobi did not converge: {p_jac:?}"
        );
        assert!(
            p_ilu.converged,
            "{label}: BiCGStab+ILU(0) did not converge: {p_ilu:?}"
        );
        assert!(
            p_gm.converged,
            "{label}: GMRES+ILU(0) did not converge: {p_gm:?}"
        );
        for (pc, r) in [("jacobi", r_jac), ("ilu0", r_ilu), ("gmres", r_gm)] {
            assert!(
                r < 1e-7,
                "{label}/{pc}: true relative residual {r:.3e} not below 1e-7"
            );
        }

        // (b) BiCGStab and GMRES must agree on the same solution.
        let d = max_abs_diff(t_ilu.internal.as_slice(), t_gm.internal.as_slice());
        eprintln!("max |T_bicgstab - T_gmres|  : {d:.3e}");
        assert!(d < 1e-6, "{label}: BiCGStab and GMRES disagree by {d:.3e}");

        // (c) The headline claim, asserted only in the regime where the
        //     incumbent path is actually the bottleneck: at low Peclet number
        //     the matrix loses its convection-supplied diagonal dominance,
        //     Gauss-Seidel degrades to the O(kappa) elliptic rate, and the
        //     preconditioned Krylov solve must beat it by a wide margin.
        if gamma > 1.0e-2 {
            assert!(
                !p_gs.converged,
                "{label}: Gauss-Seidel unexpectedly converged in {} iters — \
                 the comparison below assumes it does not",
                p_gs.n_iterations
            );
            assert!(
                r_ilu < r_gs / 1e3,
                "{label}: BiCGStab+ILU(0) residual {r_ilu:.3e} is not 1000x tighter \
                 than Gauss-Seidel's {r_gs:.3e}"
            );
            assert!(
                p_ilu.n_iterations * 5 < p_gs.n_iterations,
                "{label}: BiCGStab+ILU(0) took {} iters vs Gauss-Seidel's {}",
                p_ilu.n_iterations,
                p_gs.n_iterations
            );
        }

        // (d) ILU(0) must never need more iterations than Jacobi on this family.
        assert!(
            p_ilu.n_iterations <= p_jac.n_iterations,
            "{label}: ILU(0) {} iters vs Jacobi {} iters",
            p_ilu.n_iterations,
            p_jac.n_iterations
        );
    }
}

/// Correctness cross-check: the Krylov solution of the convection-diffusion
/// system equals a **dense LU** solution of the identical matrix.
///
/// # Methodology
///
/// A 24x24 (576-cell) instance of the convection-dominated case
/// (`Gamma = 1e-3`) is materialised as a dense `SquareMatrix` and solved with
/// `SquareMatrix::solve` (Crout LU, scaled partial pivoting). That direct
/// solution is the reference. Solver tolerance `1e-10`. Pass criterion:
/// `max_c |T_krylov[c] - T_LU[c]| < 1e-8`, for both BiCGStab and GMRES.
///
/// # Results (measured 2026-08-07, release build)
///
/// - BiCGStab + ILU(0): converged in **5** iterations;
///   `max |T_bicgstab - T_LU| = 1.881e-10`.
/// - GMRES(30) + ILU(0): converged in **8** iterations;
///   `max |T_gmres - T_LU| = 3.704e-11`.
///
/// Both agree with the direct solve to ~1e-10 in absolute value on a field of
/// order 1 — comfortably inside the `1e-8` bound. This verifies that the Krylov
/// path solves the *same* system the FV operators assembled, not merely that it
/// converges to something.
#[test]
fn krylov_matches_dense_lu_on_convection_diffusion() {
    let (nx, ny) = (24, 24);
    let mesh = cartesian_2d(nx, ny);
    let (mat, b) = convection_diffusion(mesh, Vector3::new(1.0, 0.5, 0.0), 1.0e-3, 1.0);

    let dense = dense_of(&mat.ldu);
    let x_ref = dense
        .solve(&b)
        .expect("convection-diffusion matrix is nonsingular");

    let settings = SolverSettings {
        tolerance: 1e-10,
        max_iter: 2000,
    };

    let (t_b, p_b) = mat.solve_bicgstab("T_bicg", KrylovOptions::default(), settings);
    let d_b = max_abs_diff(t_b.internal.as_slice(), &x_ref);
    eprintln!(
        "BiCGStab + ILU(0) vs dense LU: {} iters, max |dT| = {d_b:.3e}",
        p_b.n_iterations
    );
    assert!(p_b.converged, "BiCGStab did not converge: {p_b:?}");
    assert!(d_b < 1e-8, "BiCGStab differs from dense LU by {d_b:.3e}");

    let (t_g, p_g) = mat.solve_gmres("T_gmres", KrylovOptions::default(), settings);
    let d_g = max_abs_diff(t_g.internal.as_slice(), &x_ref);
    eprintln!(
        "GMRES(30) + ILU(0) vs dense LU: {} iters, max |dT| = {d_g:.3e}",
        p_g.n_iterations
    );
    assert!(p_g.converged, "GMRES did not converge: {p_g:?}");
    assert!(d_g < 1e-8, "GMRES differs from dense LU by {d_g:.3e}");
}

/// Warm-starting a Krylov solve from the previous solution costs almost nothing
/// — the transient-loop use case.
///
/// # Methodology
///
/// Solve the 24x24 convection-diffusion system (`Gamma = 1e-3`) cold (`x = 0`)
/// at tolerance `1e-8`, then re-solve the *same* system warm-started from that
/// answer via `FvMatrix::solve_bicgstab_with_guess`. Pass criterion: the warm
/// solve uses strictly fewer iterations than the cold one and still converges.
///
/// # Results (measured 2026-08-07, release build)
///
/// Cold start: **4** iterations. Warm start from the converged field: **0**
/// iterations (the initial residual is already inside the tolerance). This is
/// the behaviour a PIMPLE/PISO outer loop relies on near steady state — note it
/// is a best case, since the guess here is the exact answer rather than a
/// previous time step's field.
#[test]
fn warm_started_krylov_costs_fewer_iterations() {
    let mesh = cartesian_2d(24, 24);
    let (mat, _b) = convection_diffusion(mesh, Vector3::new(1.0, 0.5, 0.0), 1.0e-3, 1.0);
    let settings = SolverSettings {
        tolerance: 1e-8,
        max_iter: 2000,
    };

    let (t_cold, p_cold) = mat.solve_bicgstab("T", KrylovOptions::default(), settings);
    let (_t_warm, p_warm) =
        mat.solve_bicgstab_with_guess("T", &t_cold, KrylovOptions::default(), settings);
    eprintln!(
        "cold start: {} iters; warm start: {} iters",
        p_cold.n_iterations, p_warm.n_iterations
    );
    assert!(p_cold.converged && p_warm.converged);
    assert!(
        p_warm.n_iterations < p_cold.n_iterations,
        "warm start {} iters was not cheaper than cold start {} iters",
        p_warm.n_iterations,
        p_cold.n_iterations
    );
}

/// `solve_krylov` with an explicit method selector reproduces the dedicated
/// `solve_bicgstab` / `solve_gmres` entry points exactly.
///
/// Guards the run-time-method-selection path (e.g. falling back from BiCGStab to
/// GMRES after a breakdown) against drifting away from the fixed entry points.
#[test]
fn solve_krylov_dispatch_matches_the_named_entry_points() {
    let mesh = cartesian_2d(16, 16);
    let (mat, _b) = convection_diffusion(mesh, Vector3::new(1.0, 0.5, 0.0), 1.0e-3, 1.0);
    let settings = SolverSettings {
        tolerance: 1e-9,
        max_iter: 1000,
    };
    let opts = KrylovOptions::default();

    let (a1, _) = mat.solve_bicgstab("T", opts, settings);
    let (a2, _) = mat.solve_krylov("T", None, KrylovMethod::BiCGStab, opts, settings);
    assert_eq!(a1.internal.as_slice(), a2.internal.as_slice());

    let (g1, _) = mat.solve_gmres("T", opts, settings);
    let (g2, _) = mat.solve_krylov("T", None, KrylovMethod::Gmres, opts, settings);
    assert_eq!(g1.internal.as_slice(), g2.internal.as_slice());
}
