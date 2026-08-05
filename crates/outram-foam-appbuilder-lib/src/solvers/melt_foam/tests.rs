// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See the crate root for the full licence.

//! Unit tests for [`MeltFoam`](super::MeltFoam).
//!
//! These cover the solver's *wiring* — coefficient conversion, field naming,
//! per-step state advancement — not the melting physics, which is verified
//! against the analytical Stefan solution in the crate's `melting_vv_cases`
//! integration test.

use super::*;

/// A 1-D mesh of `n` cells over 1 m with unit cross-section.
fn line_mesh(n: usize) -> Arc<FvMesh> {
    let h = 1.0 / n as f64;
    let ax = |x: f64| Vector3::new(x, 0.0, 0.0);
    let mut owner: Vec<usize> = (0..n - 1).collect();
    owner.push(0);
    owner.push(n - 1);
    let neighbour: Vec<usize> = (1..n).collect();
    let mut fav: Vec<Vector3> = (0..n - 1).map(|_| ax(1.0)).collect();
    fav.push(ax(-1.0));
    fav.push(ax(1.0));
    let mut fc: Vec<Vector3> = (0..n - 1).map(|i| ax((i + 1) as f64 * h)).collect();
    fc.push(ax(0.0));
    fc.push(ax(1.0));
    Arc::new(
        FvMeshBuilder::new()
            .n_cells(n)
            .n_internal_faces(n - 1)
            .owner(owner)
            .neighbour(neighbour)
            .patches(vec![
                BoundaryPatch::new("hot", n - 1, 1, PatchKind::Wall),
                BoundaryPatch::new("far", n, 1, PatchKind::Wall),
            ])
            .cell_volumes(vec![h; n])
            .cell_centres((0..n).map(|i| ax((i as f64 + 0.5) * h)).collect())
            .face_area_vectors(fav)
            .face_centres(fc)
            .build()
            .expect("mesh must build"),
    )
}

/// Build a solver with a melting model attached, hot wall at patch 0.
fn solver_with_model(n: usize) -> MeltFoam {
    let mesh = line_mesh(n);
    let mut control = ControlDict::default();
    control.delta_t = 0.01;
    let mut s = MeltFoam::new(
        mesh.clone(),
        control,
        FvSchemes::default(),
        FvSolution::default(),
    );
    s.t = VolScalarField::uniform("T", mesh.clone(), 300.0);
    s.alpha_thermal = VolScalarField::uniform("alphat", mesh.clone(), 1e-5);
    s.t.boundary[0].bc = BoundaryCondition::FixedValue(320.0);
    for v in s.t.boundary[0].values.iter_mut() {
        *v = 320.0;
    }
    s.t.boundary[1].bc = BoundaryCondition::ZeroGradient;
    let coeffs =
        MeltFoam::boussinesq_coefficients(300.0, 300.2, 100_000.0, 1000.0, 1.0, 0.0, 1.0e8);
    s.fv_models
        .push(FvModel::SolidificationMelting(SolidificationMelting::new(
            "melt",
            "U",
            "T",
            true,
            CellSelection::All,
            coeffs,
            Vector3::new(0.0, 0.0, 0.0),
            n,
        )));
    s
}

/// `boussinesq_coefficients` must set the reference density to 1 and divide the
/// Darcy coefficient by the material density — the kinematic convention the
/// solver's momentum equation needs (see the module docs).
#[test]
fn boussinesq_coefficients_convert_to_kinematic_units() {
    let c = MeltFoam::boussinesq_coefficients(
        302.8,    // solidus [K]
        303.0,    // liquidus [K]
        80_160.0, // latent heat [J/kg]
        381.5,    // Cp [J/(kg K)]
        6093.0,   // density [kg/m^3]
        1.2e-4,   // beta [1/K]
        1.6e6,    // Cu in force-form units [kg/(m^3 s)]
    );
    assert_eq!(
        c.reference_density, 1.0,
        "reference density must be 1 so the kinematic momentum equation is not \
         scaled by rho a second time"
    );
    let expected = 1.6e6 / 6093.0;
    assert!(
        (c.darcy_coefficient - expected).abs() < 1e-9,
        "Darcy coefficient must be divided by density: got {}, expected {expected}",
        c.darcy_coefficient
    );
    // Everything else passes through untouched.
    assert_eq!(c.solidus, 302.8);
    assert_eq!(c.liquidus, 303.0);
    assert_eq!(c.latent_heat, 80_160.0);
    assert_eq!(c.specific_heat, 381.5);
    assert_eq!(c.thermal_expansion, 1.2e-4);
    // Upstream defaults preserved.
    assert_eq!(c.relaxation, 0.9);
    assert_eq!(c.darcy_regularisation, 1.0e-3);
    assert_eq!(c.eutectic_fraction, 0.0);
}

/// **Regression test for a silent, physics-destroying bug.**
///
/// `self.u = hbya - rau*grad(p)` inherits the LEFT operand's name, so without an
/// explicit restore the velocity field ends the step called `"HbyA"`. Because
/// `FvModels` selects models by field name, that makes `contributes_to("U")`
/// false from step 2 onward and the Darcy drag and Boussinesq buoyancy are
/// dropped from every subsequent step — with no error, no warning, and a
/// perfectly plausible-looking conduction-only answer.
///
/// The measured symptom in the gallium cavity was a peak speed of 1.18e-13 m/s
/// (machine noise) and a perfectly vertical melt front.
#[test]
fn velocity_field_keeps_its_name_after_correction() {
    let mut s = solver_with_model(20);
    assert_eq!(s.u.name, "U", "precondition: field starts named U");
    for _ in 0..3 {
        s.step().expect("step must converge");
    }
    assert_eq!(
        s.u.name, "U",
        "velocity must keep its registered name across a step, or FvModels \
         silently stops applying the momentum source"
    );
    assert_eq!(
        s.t.name, "T",
        "temperature must keep its registered name across a step"
    );
}

/// The attached model must still be selected for both equations after several
/// steps — the behavioural counterpart of the naming test above.
#[test]
fn attached_model_contributes_to_both_equations_after_stepping() {
    let mut s = solver_with_model(20);
    for _ in 0..3 {
        s.step().expect("step must converge");
    }
    assert!(
        s.fv_models.contributes_to(&s.u.name),
        "the melting model must still be selected for the momentum equation"
    );
    assert!(
        s.fv_models.contributes_to(&s.t.name),
        "the melting model must still be selected for the energy equation"
    );
}

/// Melting must actually progress when a wall is held above the liquidus, and
/// the liquid fraction must stay physical.
#[test]
fn hot_wall_drives_melting_with_bounded_liquid_fraction() {
    let mut s = solver_with_model(50);
    for _ in 0..200 {
        s.step().expect("step must converge");
    }
    let a = s.liquid_fraction().expect("model attached");
    assert!(
        a.iter().all(|v| (0.0..=1.0).contains(v)),
        "liquid fraction must stay within [0, 1]"
    );
    assert!(a[0] > 0.0, "the cell against the hot wall must melt");
    assert!(
        a.iter().any(|&v| v < 1.0),
        "the far end must remain unmelted at this time"
    );
    assert!(
        a[0] >= a[a.len() - 1],
        "melt must lead at the hot wall, got {} vs {}",
        a[0],
        a[a.len() - 1]
    );
}

/// The temperature solver must default to a tolerance tight enough for a long
/// melting run. See [`MeltFoam::temperature_solver`] for the measured energy
/// drift that motivates this.
#[test]
fn temperature_solver_defaults_to_a_tight_tolerance() {
    let s = MeltFoam::new(
        line_mesh(10),
        ControlDict::default(),
        FvSchemes::default(),
        FvSolution::default(),
    );
    assert!(
        s.temperature_solver.tolerance <= 1e-12,
        "temperature solve must default tight (<= 1e-12); a generic 1e-7 \
         accumulated a 0.92 % energy loss over 10 000 steps"
    );
}

/// With no models attached the solver must still run — it degenerates to
/// pimpleFoam plus a passive temperature equation.
#[test]
fn runs_without_any_fv_model() {
    let mesh = line_mesh(10);
    let mut control = ControlDict::default();
    control.delta_t = 0.01;
    let mut s = MeltFoam::new(
        mesh.clone(),
        control,
        FvSchemes::default(),
        FvSolution::default(),
    );
    s.t = VolScalarField::uniform("T", mesh.clone(), 300.0);
    for _ in 0..5 {
        s.step()
            .expect("step must converge with no models attached");
    }
    assert!(
        s.liquid_fraction().is_none(),
        "no melting model is attached"
    );
    assert!(
        s.t.internal.as_slice().iter().all(|v| v.is_finite()),
        "temperature must stay finite"
    );
}

/// Wall face fluxes must equal the prescribed velocity BC after a step.
///
/// The PISO corrector rebuilds internal faces only; the boundary fluxes are
/// constrained separately. If that constraint is dropped, `fvm::div` advects
/// through solid walls — see the note in `MeltFoam::step`.
///
/// # Coverage honesty
///
/// This test asserts the invariant but is **not** the test that would catch the
/// constraint being removed. Verified 2026-08-05 by deleting the constraint and
/// re-running: this test still passed, because the 1-D configuration here has no
/// buoyancy and hence no flow, so the unconstrained `flux(HbyA)` extrapolation
/// is itself ~0 at the walls and the two agree by accident.
///
/// The discriminating coverage is the 2-D gallium cavity in the crate's
/// `melting_vv_cases` integration test, whose temperature-range criterion fails
/// loudly without the constraint — measured 362.93 K against a 311 K hot wall.
/// Keep both: this one documents the invariant cheaply, that one enforces it.
#[test]
fn wall_face_fluxes_are_constrained_to_the_velocity_bc() {
    let mut s = solver_with_model(20);
    for p in 0..s.u.boundary.len() {
        s.u.boundary[p].bc = BoundaryCondition::NoSlip;
        for v in s.u.boundary[p].values.iter_mut() {
            *v = Vector3::new(0.0, 0.0, 0.0);
        }
    }
    s.step().expect("step must converge");
    for (pi, patch) in s.mesh.patches.iter().enumerate() {
        for fi in 0..patch.size {
            let f = s.phi.boundary[pi].values[fi];
            assert!(
                f.abs() < 1e-30,
                "no-slip wall must carry zero volumetric flux, patch {pi} face \
                 {fi} carries {f:e}"
            );
        }
    }
}
