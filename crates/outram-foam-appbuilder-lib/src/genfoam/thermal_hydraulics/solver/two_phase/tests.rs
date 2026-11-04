// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! Unit tests of the two-phase driver's FV layer and case reader. The
//! end-to-end runs on upstream's tutorial cases are integration tests in
//! `tests/genfoam_two_phase_1d_boiling.rs` (they need the upstream clone).

use super::case::TwoPhaseCase;
use super::fv::{self, Bc, Eqn, Geometry, Scalar};
use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;
use outram_foam_basic_lib::prelude::*;
use uom::si::area::square_meter;
use uom::si::f64::{Area, Length};
use uom::si::length::meter;

fn line(n: i64) -> Geometry {
    let mesh = create_one_d_mesh(
        Length::new::<meter>(1.0),
        Area::new::<square_meter>(1e-2),
        n,
    )
    .expect("1-D mesh");
    Geometry::new(std::sync::Arc::new(mesh))
}

/// `fvc::reconstruct` of the flux of a uniform field returns that field in
/// the resolved direction and zero in the empty ones.
#[test]
fn reconstruct_recovers_a_uniform_velocity() {
    let g = line(8);
    let u = fv::Vector::uniform(&g, Vector3::new(0.7, 0.0, 0.0));
    let phi = fv::flux(&g, &u);
    for v in fv::reconstruct(&g, &phi) {
        assert!((v.x - 0.7).abs() < 1e-12 && v.y.abs() < 1e-12 && v.z.abs() < 1e-12, "{v:?}");
    }
}

/// `-laplacian(1, T)` with fixed ends reproduces the linear conduction
/// profile exactly (second-order operator on a linear solution).
#[test]
fn neg_laplacian_with_fixed_ends_gives_the_linear_profile() {
    let g = line(10);
    let mut bc = g.default_bcs();
    for (pi, p) in g.mesh.patches.iter().enumerate() {
        if p.kind != PatchKind::Empty {
            let v = if g.mesh.face_centres[p.start].x < 0.5 { 300.0 } else { 400.0 };
            bc[pi] = Bc::Fixed(vec![v; p.size]);
        }
    }
    let t = Scalar::uniform(&g, 350.0, bc);
    let mut e = Eqn::new(&g);
    let gamma = vec![1.0; g.n_faces()];
    e.neg_laplacian(&g, &gamma, &t);
    let mut x = vec![350.0; g.n_cells()];
    let _ = super::solve_krylov(&e, &mut x, 0.0);
    for c in 0..g.n_cells() {
        let exact = 300.0 + 100.0 * g.mesh.cell_centres[c].x;
        assert!((x[c] - exact).abs() < 1e-8, "cell {c}: {} vs {exact}", x[c]);
    }
}

/// The case reader rejects a directory that is not a case.
#[test]
fn reading_a_missing_case_is_an_error() {
    let r = TwoPhaseCase::read(std::path::Path::new("/nonexistent/genfoam/case"));
    assert!(r.is_err());
}

/// The vendored upstream `1D_boiling` case reads, builds, and steps through
/// the `ThermalHydraulicsSolver` enum; the fluid fractions fill the moving
/// volume exactly (`twoPhase.C` normalisation).
#[test]
fn one_d_boiling_case_steps_through_the_solver_enum() {
    use super::super::ThermalHydraulicsSolver;
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/genfoam_1d_boiling");
    let case = TwoPhaseCase::read(&dir).expect("read vendored 1D_boiling");
    assert_eq!(case.deviations.len(), 1, "MULES -> implicitUpwind is the only deviation");
    let mut th = ThermalHydraulicsSolver::TwoPhase(super::TwoPhaseSolver::new(case));
    for _ in 0..5 {
        th.step(1e-3).expect("step");
    }
    let ThermalHydraulicsSolver::TwoPhase(s) = th else {
        unreachable!("constructed as TwoPhase")
    };
    for c in 0..s.geom.n_cells() {
        let sum = s.fluids[0].alpha.c[c] + s.fluids[1].alpha.c[c] + s.structure.alpha.c[c];
        assert!((sum - 1.0).abs() < 1e-12, "cell {c}: {sum}");
        assert!(s.fluids[0].t.c[c].is_finite() && s.p.c[c].is_finite());
    }
    assert!((s.time - 5e-3).abs() < 1e-12);
}
