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

//! Verification of the non-orthogonal Laplacian correction and the
//! least-squares gradient on a deliberately distorted (wavy) mesh.
//!
//! # Why this test exists
//!
//! `fvm::laplacian` discretises the face flux as
//! `Gamma_f |Sf| (phi_N - phi_P) / |d|`, which assumes the face normal `Sf` is
//! parallel to the cell-to-cell vector `d`. On a non-orthogonal mesh that is not
//! merely inaccurate, it is **not exact even for a linear field** — the operator
//! silently loses an order of accuracy, with no diagnostic.
//! `fvm::laplacian_corrected` adds the deferred-correction term that restores
//! it, driven by the mesh-independent `fvc::grad_least_squares` gradient.
//!
//! # Methodology
//!
//! **Mesh.** A `20 x 20` quadrilateral grid on the unit square whose *interior*
//! vertices are displaced sinusoidally by an amplitude `amp * min(dx, dy)`:
//!
//! ```text
//!     x' = x + amp*h*sin(pi x) sin(2 pi y)
//!     y' = y + amp*h*sin(2 pi x) sin(pi y)
//! ```
//!
//! All geometry is then computed **exactly** for the resulting straight-edged
//! z-extruded quads (shoelace area and centroid; edge-normal face-area vectors;
//! edge-midpoint face centres), so no geometric approximation contaminates the
//! measurement. `amp` is swept from `0` (an orthogonal Cartesian control) to
//! `1.5`, giving a measured max non-orthogonality — the same statistic
//! `checkMesh` prints, computed here by `fvm::max_non_orthogonality_deg` — from
//! `0` to `49.6` degrees.
//!
//! A **wavy** mesh, not a uniform shear, is essential. Under a uniform shear
//! every cell is congruent and the uncorrected scheme's per-face errors cancel
//! exactly between a cell's opposing faces, so a linear field is still recovered
//! exactly and nothing is being tested. (That was measured, not assumed: an
//! earlier version of this test used a uniform shear and both schemes were exact
//! to `7e-12` at 45 degrees of non-orthogonality.) A wavy mesh distorts each
//! cell differently and breaks the cancellation, as any real tetrahedral or
//! polyhedral mesh does.
//!
//! **Test problem.** The analytic field `T(x, y) = 2 + 3x - 1.5y` is harmonic
//! (`div(grad T) = 0`), so it is the exact solution of `-div(Gamma grad T) = 0`
//! for constant `Gamma = 1`. Every boundary patch is a per-face Dirichlet
//! (`FixedField`) condition set from the analytic field evaluated at the actual
//! face centre, so `T_exact` is the exact solution of the *discrete* problem if
//! and only if the discretisation is exact for a linear field.
//!
//! **Measure.** The relative L-infinity error of the solved field against the
//! analytic field at each cell centroid,
//! `max_c |T_c - T_exact(C_c)| / max_c |T_exact(C_c)|`. Solved with
//! `fvm::solve_laplacian_non_orthogonal` at a linear-solver tolerance of `1e-12`
//! (so the linear solve is never the limiting error) and **20 non-orthogonal
//! corrector passes**.
//!
//! **Pass criterion.** On the distorted meshes the corrected scheme must reach a
//! relative error below `1e-7` and must beat the uncorrected scheme by at least
//! 1000x. On the orthogonal control both schemes must be exact and the
//! correction must be a no-op.
//!
//! # Results (measured 2026-08-07, release build, 20x20 cells, `Gamma = 1`)
//!
//! Recorded verbatim from the run of
//! `corrected_laplacian_recovers_a_linear_field_on_a_wavy_mesh`
//! (`cargo test --release -p outram-foam-basic-lib --test non_orthogonal_laplacian
//! -- --nocapture --test-threads=1`).
//!
//! | `amp` | Max non-orthogonality | Uncorrected rel. L-inf error | Corrected rel. L-inf error | Ratio |
//! |---|---|---|---|---|
//! | 0.00 | 0.00 deg | 9.377e-12 | 9.377e-12 | 1.0 (control) |
//! | 0.30 | 10.55 deg | 1.024e-02 | 6.981e-12 | 1.47e9 |
//! | 0.60 | 20.92 deg | 2.048e-02 | 6.035e-12 | 3.39e9 |
//! | 0.90 | 30.96 deg | 3.075e-02 | 5.972e-12 | 5.15e9 |
//! | 1.20 | 40.54 deg | 4.104e-02 | 3.268e-10 | 1.26e8 |
//! | 1.50 | 49.56 deg | 5.139e-02 | 1.638e-08 | 3.14e6 |
//!
//! And from `least_squares_gradient_is_exact_for_a_linear_field_on_a_wavy_mesh`
//! (`amp = 0.5`, max non-orthogonality **17.50 deg**), comparing both gradient
//! operators against the exact `grad T = (3, -1.5, 0)`:
//!
//! | Gradient operator | Max absolute error |
//! |---|---|
//! | `fvc::grad` (Gauss) | **1.845e-02** |
//! | `fvc::grad_least_squares` | **1.823e-14** |
//!
//! **Interpretation.**
//!
//! - The uncorrected Laplacian's error grows **linearly with the distortion** —
//!   1.0e-2, 2.0e-2, 3.1e-2, 4.1e-2, 5.1e-2 as the amplitude steps by 0.3 — and
//!   is already a **1% error on an exactly linear field** at only 10 degrees of
//!   non-orthogonality. There is no discretisation error to blame here: a
//!   consistent scheme must be exact on this problem. The 5% error at 50 degrees
//!   is a lower bound on what to expect from a real polyhedral mesh.
//! - The corrected scheme is exact to `6e-12` up to 31 degrees. Beyond that the
//!   error rises (3.3e-10, 1.6e-8) — **not** because the discretisation is less
//!   accurate but because the *deferred-correction fixed-point iteration*
//!   converges more slowly as non-orthogonality grows, and 20 corrector passes
//!   no longer drive it fully to rounding. More passes tighten it further; this
//!   is the practical reason `NonOrthoScheme::Limited` exists.
//! - The Gauss gradient is wrong by 1.8e-2 on an exactly linear field at 17.5
//!   degrees, twelve orders of magnitude worse than the least-squares gradient.
//!   This is why the correction must be driven by the least-squares gradient:
//!   feeding it a Gauss gradient would cap the achievable accuracy at the Gauss
//!   gradient's own error.
//!
//! # Scope and caveats
//!
//! - This is **verification against an analytic solution**, not validation
//!   against an external code or experiment. **No human V&V review has been
//!   done.**
//! - It exercises a *linear* field only. The correction is exact for a linear
//!   field by construction; on a curved field both schemes carry a genuine
//!   truncation error and the correction improves the order, not the exactness.
//!   That is **not** measured here, and no order-of-accuracy study was run.
//! - The distortion tested reaches ~50 degrees of non-orthogonality. Meshes
//!   beyond 70 degrees — where OpenFOAM warns, and where this workspace's
//!   polyhedral dual meshes reportedly sit — are **not** covered by these
//!   measurements. Do not extrapolate the corrected-column numbers there; the
//!   corrector-loop convergence trend above suggests they would be worse.
//! - The wavy map produces non-orthogonality with only mild **skewness**. The
//!   two are distinct mesh defects and this operator corrects the former only.

use std::sync::Arc;

use outram_foam_basic_lib::fields::boundary::bc::PatchField;
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::fields::surface_field::SurfaceScalarField;
use outram_foam_basic_lib::fields::vol_field::VolScalarField;
use outram_foam_basic_lib::fv_operators::fvc;
use outram_foam_basic_lib::fv_operators::fvm;
use outram_foam_basic_lib::fv_operators::fvm::NonOrthoScheme;
use outram_foam_basic_lib::ldu_matrix::SolverSettings;
use outram_foam_basic_lib::mesh::fv_mesh::{BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind};
use outram_foam_basic_lib::primitives::Vector3;

// ── the analytic field ───────────────────────────────────────────────────────

/// The exact solution used throughout: `T(x, y) = 2 + 3x - 1.5y`.
///
/// Linear, hence harmonic (`div(grad T) = 0`), so it solves
/// `-div(Gamma grad T) = 0` for any constant `Gamma`, and any consistent
/// discretisation must reproduce it exactly given exact Dirichlet data.
fn t_exact(p: Vector3) -> f64 {
    2.0 + 3.0 * p.x - 1.5 * p.y
}

// ── wavy (non-uniform, non-orthogonal) mesh ──────────────────────────────────

/// Exact area and centroid of a planar convex polygon given in counter-clockwise
/// order — the standard shoelace formulas.
///
/// Returns `(area, centroid)` with the centroid's `z` left at zero; the caller
/// sets it to the extrusion mid-plane.
fn polygon_area_centroid(v: &[Vector3]) -> (f64, Vector3) {
    let n = v.len();
    let mut a2 = 0.0;
    let mut cx = 0.0;
    let mut cy = 0.0;
    for k in 0..n {
        let p = v[k];
        let q = v[(k + 1) % n];
        let cross = p.x * q.y - q.x * p.y;
        a2 += cross;
        cx += (p.x + q.x) * cross;
        cy += (p.y + q.y) * cross;
    }
    let area = 0.5 * a2;
    (
        area.abs(),
        Vector3::new(cx / (3.0 * a2), cy / (3.0 * a2), 0.0),
    )
}

/// Build a **wavy** `nx x ny` quadrilateral mesh on the unit square: a uniform
/// Cartesian vertex grid with every *interior* vertex displaced sinusoidally,
///
/// ```text
///     x' = x + amp*h*sin(pi x) sin(2 pi y)
///     y' = y + amp*h*sin(2 pi x) sin(pi y) ,    h = min(dx, dy)
/// ```
///
/// The `sin(pi x) sin(pi y)`-type factors vanish on the domain boundary, so the
/// unit square keeps its shape and only the interior is distorted.
///
/// This is deliberately **not** a uniform shear. A uniform shear makes every
/// cell congruent, and then the uncorrected Laplacian's per-face errors cancel
/// exactly between a cell's opposing faces, so a linear field is still recovered
/// exactly and nothing is being tested. A wavy mesh makes each cell differently
/// distorted, breaking that cancellation — which is the realistic situation on
/// any tetrahedral or polyhedral mesh.
///
/// All geometry is computed exactly for the resulting straight-edged,
/// z-extruded quads: cell volume and centroid from the shoelace formulas, face
/// area vectors as the outward edge normals times the extrusion depth, and face
/// centres as edge midpoints. `amp = 0` returns the orthogonal Cartesian mesh.
fn wavy_mesh(nx: usize, ny: usize, amp: f64) -> Arc<FvMesh> {
    let dx = 1.0 / nx as f64;
    let dy = 1.0 / ny as f64;
    let dz = 1.0;
    let h = dx.min(dy);
    let cell = |i: usize, j: usize| j * nx + i;

    // Vertex grid, (nx+1) x (ny+1).
    let vert = |i: usize, j: usize| -> Vector3 {
        let x = i as f64 * dx;
        let y = j as f64 * dy;
        Vector3::new(
            x + amp * h * (std::f64::consts::PI * x).sin() * (2.0 * std::f64::consts::PI * y).sin(),
            y + amp * h * (2.0 * std::f64::consts::PI * x).sin() * (std::f64::consts::PI * y).sin(),
            0.0,
        )
    };

    // Outward area vector of the edge A -> B taken in counter-clockwise order
    // around the cell, for a prism of depth `dz`: rotate (B-A) by -90 degrees.
    let edge_sf = |a: Vector3, b: Vector3| {
        let e = b - a;
        Vector3::new(e.y * dz, -e.x * dz, 0.0)
    };
    let edge_cf = |a: Vector3, b: Vector3| (a + b) * 0.5 + Vector3::new(0.0, 0.0, 0.5 * dz);

    let mut owner = Vec::new();
    let mut neighbour = Vec::new();
    let mut sf = Vec::new();
    let mut cf = Vec::new();

    // Internal x-normal faces: the owner cell's "east" edge, P(i+1,j) -> P(i+1,j+1).
    for j in 0..ny {
        for i in 0..nx - 1 {
            let a = vert(i + 1, j);
            let b = vert(i + 1, j + 1);
            owner.push(cell(i, j));
            neighbour.push(cell(i + 1, j));
            sf.push(edge_sf(a, b));
            cf.push(edge_cf(a, b));
        }
    }
    // Internal y-normal faces: the owner cell's "north" edge, P(i+1,j+1) -> P(i,j+1).
    for j in 0..ny - 1 {
        for i in 0..nx {
            let a = vert(i + 1, j + 1);
            let b = vert(i, j + 1);
            owner.push(cell(i, j));
            neighbour.push(cell(i, j + 1));
            sf.push(edge_sf(a, b));
            cf.push(edge_cf(a, b));
        }
    }
    let n_internal = owner.len();

    let mut patches = Vec::new();
    let mut start = n_internal;

    // left: the "west" edge P(0,j+1) -> P(0,j)
    for j in 0..ny {
        let (a, b) = (vert(0, j + 1), vert(0, j));
        owner.push(cell(0, j));
        sf.push(edge_sf(a, b));
        cf.push(edge_cf(a, b));
    }
    patches.push(BoundaryPatch::new("left", start, ny, PatchKind::Patch));
    start += ny;

    // right: the "east" edge P(nx,j) -> P(nx,j+1)
    for j in 0..ny {
        let (a, b) = (vert(nx, j), vert(nx, j + 1));
        owner.push(cell(nx - 1, j));
        sf.push(edge_sf(a, b));
        cf.push(edge_cf(a, b));
    }
    patches.push(BoundaryPatch::new("right", start, ny, PatchKind::Patch));
    start += ny;

    // bottom: the "south" edge P(i,0) -> P(i+1,0)
    for i in 0..nx {
        let (a, b) = (vert(i, 0), vert(i + 1, 0));
        owner.push(cell(i, 0));
        sf.push(edge_sf(a, b));
        cf.push(edge_cf(a, b));
    }
    patches.push(BoundaryPatch::new("bottom", start, nx, PatchKind::Patch));
    start += nx;

    // top: the "north" edge P(i+1,ny) -> P(i,ny)
    for i in 0..nx {
        let (a, b) = (vert(i + 1, ny), vert(i, ny));
        owner.push(cell(i, ny - 1));
        sf.push(edge_sf(a, b));
        cf.push(edge_cf(a, b));
    }
    patches.push(BoundaryPatch::new("top", start, nx, PatchKind::Patch));

    let n_cells = nx * ny;
    let mut volumes = Vec::with_capacity(n_cells);
    let mut centres = Vec::with_capacity(n_cells);
    for c in 0..n_cells {
        let i = c % nx;
        let j = c / nx;
        let quad = [
            vert(i, j),
            vert(i + 1, j),
            vert(i + 1, j + 1),
            vert(i, j + 1),
        ];
        let (area, mut centroid) = polygon_area_centroid(&quad);
        centroid.z = 0.5 * dz;
        volumes.push(area * dz);
        centres.push(centroid);
    }

    Arc::new(
        FvMeshBuilder::new()
            .n_cells(n_cells)
            .n_internal_faces(n_internal)
            .owner(owner)
            .neighbour(neighbour)
            .patches(patches)
            .cell_volumes(volumes)
            .cell_centres(centres)
            .face_area_vectors(sf)
            .face_centres(cf)
            .build()
            .expect("wavy mesh must validate"),
    )
}

/// The transported scalar with exact Dirichlet data on every patch, taken from
/// [`t_exact`] at each face centre. Internal values start at zero.
fn field_with_exact_bcs(mesh: Arc<FvMesh>) -> VolScalarField {
    let boundary: Vec<PatchField<f64>> = mesh
        .patches
        .iter()
        .map(|p| {
            let vals: Vec<f64> = (0..p.size)
                .map(|k| t_exact(mesh.face_centres[p.start + k]))
                .collect();
            // `FixedField` is the per-face Dirichlet variant, which is what an
            // exact analytic boundary needs (the value differs face to face).
            let mut pf = PatchField::fixed_value(p.size, 0.0);
            pf.bc = outram_foam_basic_lib::fields::boundary::bc::BoundaryCondition::FixedField(
                Field::new(vals.clone()),
            );
            pf.values = Field::new(vals);
            pf
        })
        .collect();
    let n = mesh.n_cells;
    VolScalarField::new("T", mesh, Field::zeros(n), boundary)
}

/// A surface scalar field holding `value` on **both** internal and boundary
/// faces.
///
/// `SurfaceScalarField::uniform` sets the internal faces only — its boundary
/// patches are zero-gradient placeholders whose `values` are all `0.0`, which
/// would silently zero out every boundary diffusive flux in `fvm::laplacian`.
/// This helper avoids that trap.
fn uniform_surface_everywhere(name: &str, mesh: Arc<FvMesh>, value: f64) -> SurfaceScalarField {
    let n_int = mesh.n_internal_faces;
    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::fixed_value(p.size, value))
        .collect();
    SurfaceScalarField::new(name, mesh, Field::uniform(n_int, value), boundary)
}

/// Relative L-infinity error of `t` against [`t_exact`] at the cell centroids.
fn rel_linf_error(t: &VolScalarField) -> f64 {
    let mesh = &t.mesh;
    let mut num = 0.0_f64;
    let mut den = 0.0_f64;
    for c in 0..mesh.n_cells {
        let e = t_exact(mesh.cell_centres[c]);
        num = num.max((t.internal[c] - e).abs());
        den = den.max(e.abs());
    }
    num / den
}

// ── tests ────────────────────────────────────────────────────────────────────

/// **Acceptance test.** On a wavy (non-orthogonal) mesh the corrected Laplacian
/// recovers the exact linear field; the uncorrected one does not.
///
/// Methodology, the full measured results table, and the caveats are in this
/// file's module-level `//!` documentation. Headline: at 30.96 degrees of max
/// non-orthogonality the uncorrected scheme is wrong by **3.075e-02** relative
/// on an exactly linear field, the corrected scheme by **5.972e-12** — a factor
/// of 5.1e9.
#[test]
fn corrected_laplacian_recovers_a_linear_field_on_a_wavy_mesh() {
    let (nx, ny) = (20, 20);
    let settings = SolverSettings {
        tolerance: 1e-12,
        max_iter: 5000,
    };
    let n_correctors = 20;

    eprintln!(
        "{:>7} {:>12} {:>14} {:>14} {:>10}",
        "amp", "max nonOrth", "uncorrected", "corrected", "ratio"
    );

    for amp in [0.0, 0.3, 0.6, 0.9, 1.2, 1.5] {
        let mesh = wavy_mesh(nx, ny, amp);
        let non_orth = fvm::max_non_orthogonality_deg(&mesh);
        let t = field_with_exact_bcs(mesh.clone());
        let gamma = uniform_surface_everywhere("Gamma", mesh.clone(), 1.0);

        // Uncorrected: today's behaviour, reached via NonOrthoScheme::Orthogonal
        // (the default), which is bit-for-bit `fvm::laplacian`.
        let (t_orth, p_orth) = fvm::solve_laplacian_non_orthogonal(
            &gamma,
            &t,
            NonOrthoScheme::Orthogonal,
            n_correctors,
            settings,
        );
        assert!(
            p_orth.converged,
            "amp {amp}: uncorrected linear solve failed"
        );
        let e_orth = rel_linf_error(&t_orth);

        // Corrected: deferred correction driven by the least-squares gradient.
        let (t_corr, p_corr) = fvm::solve_laplacian_non_orthogonal(
            &gamma,
            &t,
            NonOrthoScheme::Corrected,
            n_correctors,
            settings,
        );
        assert!(p_corr.converged, "amp {amp}: corrected linear solve failed");
        let e_corr = rel_linf_error(&t_corr);

        eprintln!(
            "{amp:>7.2} {non_orth:>9.2} deg {e_orth:>14.3e} {e_corr:>14.3e} {:>10.3e}",
            e_orth / e_corr.max(f64::MIN_POSITIVE)
        );

        if amp == 0.0 {
            // Orthogonal control: both schemes must be exact, and the
            // correction must change nothing.
            assert!(
                non_orth < 1e-9,
                "control mesh is not orthogonal: {non_orth}"
            );
            assert!(e_orth < 1e-10, "control: uncorrected error {e_orth:.3e}");
            assert!(e_corr < 1e-7, "control: corrected error {e_corr:.3e}");
        } else {
            // Distorted (wavy): the correction must recover the linear field to
            // rounding, and must beat the uncorrected scheme by at least 1000x.
            assert!(
                e_corr < 1e-7,
                "amp {amp} (nonOrth {non_orth:.1} deg): corrected error {e_corr:.3e} \
                 is not far below the uncorrected error"
            );
            assert!(
                e_corr * 1e3 < e_orth,
                "amp {amp} (nonOrth {non_orth:.1} deg): corrected {e_corr:.3e} is not \
                 1000x better than uncorrected {e_orth:.3e}"
            );
        }
    }
}

/// The least-squares gradient is exact for a linear field on a wavy mesh, where
/// the Gauss gradient is not.
///
/// # Methodology
///
/// On the `amp = 0.5` 20x20 wavy mesh (measured max non-orthogonality **17.50
/// deg**), set every cell value and every boundary face value to the analytic
/// [`t_exact`], then compare `fvc::grad` and `fvc::grad_least_squares` against
/// the exact gradient `(3, -1.5, 0)`. Measure the max absolute error over all
/// cells. Pass criterion: the least-squares gradient is exact to `1e-10`.
///
/// # Results (measured 2026-08-07, release build)
///
/// - `fvc::grad` (Gauss): max error **1.845e-02**.
/// - `fvc::grad_least_squares`: max error **1.823e-14**.
///
/// Twelve orders of magnitude apart on a field the Gauss gradient is supposed to
/// handle exactly. Only the least-squares error is asserted; the Gauss figure is
/// reported for contrast and deliberately **not** asserted, since it is a
/// property of the mesh distortion rather than of a contract this crate makes.
#[test]
fn least_squares_gradient_is_exact_for_a_linear_field_on_a_wavy_mesh() {
    let mesh = wavy_mesh(20, 20, 0.5);
    let non_orth = fvm::max_non_orthogonality_deg(&mesh);
    let mut t = field_with_exact_bcs(mesh.clone());
    // Seed the internal values with the exact field: this test is about the
    // gradient operator, not about a solve.
    for c in 0..mesh.n_cells {
        t.internal[c] = t_exact(mesh.cell_centres[c]);
    }
    let exact = Vector3::new(3.0, -1.5, 0.0);

    let g_gauss = fvc::grad(&t);
    let g_ls = fvc::grad_least_squares(&t);

    let err = |g: &outram_foam_basic_lib::fields::vol_field::VolVectorField| {
        (0..mesh.n_cells)
            .map(|c| (g.internal[c] - exact).mag())
            .fold(0.0_f64, f64::max)
    };
    let e_gauss = err(&g_gauss);
    let e_ls = err(&g_ls);
    eprintln!("wavy mesh, max non-orthogonality {non_orth:.2} deg");
    eprintln!("max |grad_gauss - exact|         : {e_gauss:.3e}");
    eprintln!("max |grad_least_squares - exact| : {e_ls:.3e}");

    assert!(
        e_ls < 1e-10,
        "least-squares gradient error {e_ls:.3e} is not at rounding level"
    );
}

/// `NonOrthoScheme::Limited` sits between the two extremes, monotonically.
///
/// # Methodology
///
/// Same problem on a 16x16 `amp = 0.5` wavy mesh, with **5** corrector passes.
/// Solve with limiter `0.0`, `0.5` and `1.0` and check the error decreases
/// monotonically as more correction is applied, with the endpoints matching
/// `Orthogonal` and `Corrected` exactly.
///
/// # Results (measured 2026-08-07, release build)
///
/// Relative L-infinity error: limiter `0.0` -> **2.114e-02**, `0.5` ->
/// **1.183e-02**, `1.0` -> **2.955e-06**; and `Orthogonal` -> 2.114e-02,
/// `Corrected` -> 2.955e-06, matching the `0.0`/`1.0` endpoints. The `1.0` value
/// is far above the acceptance test's `~6e-12` because this test uses only 5
/// corrector passes rather than 20 — the same deferred-correction convergence
/// effect discussed in the module documentation, seen from the other side.
#[test]
fn limited_scheme_is_monotone_between_orthogonal_and_corrected() {
    let mesh = wavy_mesh(16, 16, 0.5);
    let t = field_with_exact_bcs(mesh.clone());
    let gamma = uniform_surface_everywhere("Gamma", mesh.clone(), 1.0);
    let settings = SolverSettings {
        tolerance: 1e-12,
        max_iter: 5000,
    };

    let solve = |scheme| {
        let (f, p) = fvm::solve_laplacian_non_orthogonal(&gamma, &t, scheme, 5, settings);
        assert!(p.converged, "{scheme:?} linear solve failed");
        rel_linf_error(&f)
    };

    let e0 = solve(NonOrthoScheme::Limited(0.0));
    let e5 = solve(NonOrthoScheme::Limited(0.5));
    let e1 = solve(NonOrthoScheme::Limited(1.0));
    let e_orth = solve(NonOrthoScheme::Orthogonal);
    let e_corr = solve(NonOrthoScheme::Corrected);
    eprintln!("limiter 0.0 -> {e0:.3e}, 0.5 -> {e5:.3e}, 1.0 -> {e1:.3e}");
    eprintln!("Orthogonal  -> {e_orth:.3e}, Corrected -> {e_corr:.3e}");

    assert!(
        (e0 - e_orth).abs() <= 1e-14 + 1e-9 * e_orth,
        "limiter 0 != Orthogonal"
    );
    assert!(
        (e1 - e_corr).abs() <= 1e-14 + 1e-9 * e_corr.max(1e-14),
        "limiter 1 != Corrected"
    );
    assert!(
        e5 < e0,
        "half correction {e5:.3e} not better than none {e0:.3e}"
    );
    assert!(
        e1 < e5,
        "full correction {e1:.3e} not better than half {e5:.3e}"
    );
}
