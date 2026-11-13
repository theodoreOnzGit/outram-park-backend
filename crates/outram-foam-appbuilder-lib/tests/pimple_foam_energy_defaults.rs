//! Pins for the energy equation in `PimpleFoam` (gh:#1156).
//!
//! 1. **The temperature is passive and optional.** With no energy equation
//!    attached, `step` is `step_flow`; with one attached, U, p and phi after
//!    each step are bit-identical to the run without it (constant
//!    properties, no buoyancy), so attaching T cannot move any pinned flow
//!    number.
//! 2. **Correct physics is the default** (root `CLAUDE.md`): built through
//!    the ordinary path, the energy equation applies the non-orthogonal
//!    correction (`Gauss linear corrected`, the flow's default Laplacian);
//!    the ablation `without_non_orthogonal_correction` turns it off, and on
//!    the O-grid the two give different temperatures.
use std::sync::Arc;

use outram_foam_appbuilder_lib::cases::laminar_pipe::PipeFlow;
use outram_foam_appbuilder_lib::solvers::energy::{EnergyEquation, IncompressibleThermo};
use outram_foam_appbuilder_lib::solvers::pimple_foam::PimpleFoam;
use outram_foam_basic_lib::prelude::*;
use outram_foam_mesh::block_primitives::PipeOGrid;
use uom::si::f64::{Length, Time};
use uom::si::length::meter;
use uom::si::time::second;

fn pipe() -> (Arc<FvMesh>, PipeFlow) {
    let mut f = PipeFlow::stage0();
    f.length = Length::new::<meter>(0.04);
    let g = PipeOGrid::new(f.radius, f.length, 3, 2, 8);
    (Arc::new(g.mesh().unwrap().to_fv_mesh().unwrap()), f)
}

fn with_energy(s: &mut PimpleFoam) {
    let mut e = EnergyEquation::uniform(s.mesh.clone(), IncompressibleThermo::si(1000.0, 4000.0, 0.6), 300.0);
    e.set_patch("inlet", BoundaryCondition::FixedValue(320.0));
    e.set_patch("walls", BoundaryCondition::FixedValue(350.0));
    s.energy = Some(e);
}

#[test]
fn temperature_is_passive_and_flow_is_bit_identical() {
    let (mesh, flow) = pipe();
    let dt = Time::new::<second>(0.5);
    let mut a = flow.build_solver(mesh.clone(), dt);
    let mut b = flow.build_solver(mesh, dt);
    with_energy(&mut b);
    for _ in 0..4 {
        a.step().unwrap();
        b.step().unwrap();
    }
    assert_eq!(a.u.internal.as_slice(), b.u.internal.as_slice());
    assert_eq!(a.p.internal.as_slice(), b.p.internal.as_slice());
    assert_eq!(a.phi.internal.as_slice(), b.phi.internal.as_slice());
    let t = &b.energy.as_ref().unwrap().t;
    assert!(t.internal.as_slice().iter().any(|v| *v > 300.5), "the wall heated nothing");
    assert!(t.internal.as_slice().iter().all(|v| (299.999..=350.001).contains(v)), "T left its bounds");
    // No energy equation: solve_energy is a no-op.
    let t_old = VolScalarField::uniform("T", a.mesh.clone(), 1.0);
    assert_eq!(a.solve_energy(&t_old).unwrap().max_change, 0.0);
}

#[test]
fn energy_non_orthogonal_correction_is_on_by_default() {
    let (mesh, flow) = pipe();
    let dt = Time::new::<second>(0.5);
    let mut on = flow.build_solver(mesh.clone(), dt);
    let mut off = flow.build_solver(mesh, dt).without_non_orthogonal_correction();
    assert_eq!(on.schemes.default_laplacian, outram_foam_appbuilder_lib::io::fv_schemes::LaplacianScheme::GaussLinearCorrected);
    for s in [&mut on, &mut off] {
        with_energy(s);
        s.solution.pimple.frozen_flow = true;
        for _ in 0..3 {
            s.step().unwrap();
        }
    }
    // Frozen at rest: only conduction, and the O-grid is non-orthogonal, so
    // the corrected and orthogonal Laplacians must differ.
    let (ta, tb) = (&on.energy.as_ref().unwrap().t, &off.energy.as_ref().unwrap().t);
    let d = ta.internal.as_slice().iter().zip(tb.internal.as_slice()).fold(0.0_f64, |m, (x, y)| m.max((x - y).abs()));
    assert!(d > 1e-6, "correction had no effect: {d}");
    // Frozen flow: U stays at rest.
    assert!(on.u.internal.as_slice().iter().all(|v| v.mag() == 0.0));
}
