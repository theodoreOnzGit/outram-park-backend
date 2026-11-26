//! Unit tests of the finite-volume `legacyThermoMechanics` translation. The comparison with the real GeN-Foam
//! class is `tests/genfoam_legacy_thermo_mechanics_vs_upstream.rs`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use super::*;
use crate::io::poly_mesh::read_poly_mesh;
use std::path::PathBuf;

/// The 6x4x3 graded block of the `solidDisplacementFoam` comparison (gh:#1383): `left`/`right` face `x`,
/// `bottom`/`top` face `y` (18 faces: 6 x 3), `back`/`front` face `z`.
fn block() -> Arc<FvMesh> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/upstream_solid_displacement_gh1383/case/constant/polyMesh");
    read_poly_mesh(&p).expect("block polyMesh")
}

fn settings(grad: GradScheme, laplacian: LaplacianCorrection) -> LegacyFvmSettings {
    LegacyFvmSettings {
        plane_stress: false,
        solve_displacement: true,
        solve_thermo: true,
        pin_direction: Vector3::new(0.0, 1.0, 0.0),
        n_correctors: 3,
        convergence_tolerance: 1e-6,
        compact_normal_stress: true,
        grad,
        laplacian,
        d_solver: LinearSolverSettings::tight(1e-14),
        fuel_solver: LinearSolverSettings::tight(1e-14),
        cr_solver: LinearSolverSettings::tight(1e-14),
        t_solver: LinearSolverSettings::tight(1e-14),
        t_solver_kind: TStructSolver::PbicgDilu,
    }
}

fn card() -> ZoneCard {
    ZoneCard {
        name: "all".to_string(),
        rho: 7854.0,
        cp: 434.0,
        k: 60.5,
        e: 2e11,
        nu: 0.3,
        alpha: 1.1e-5,
        t_ref: 700.0,
        t_fuel_ref: 700.0,
        alpha_fuel: 1.0e-5,
        t_cr_ref: 700.0,
        alpha_cr: 2.0e-5,
    }
}

/// `left` held at 0 and `right` stretched by 1e-4 m, the other patches free; `bottom` holds the fuel and
/// control-rod expansions at 0, `top` is free.
fn build(mesh: &Arc<FvMesh>, s: LegacyFvmSettings) -> LegacyFvmThermoMechanics {
    let zones = vec![("all".to_string(), (0..mesh.n_cells).collect::<Vec<usize>>())];
    let disp: Vec<DispPatch> = mesh
        .patches
        .iter()
        .map(|p| match p.name.as_str() {
            "left" => DispPatch::FixedValue(Vector3::ZERO),
            "right" => DispPatch::FixedValue(Vector3::new(1e-4, 0.0, 0.0)),
            _ => DispPatch::ZeroGradient,
        })
        .collect();
    let scalar: Vec<ScalarPatch> = mesh
        .patches
        .iter()
        .map(|p| if p.name == "bottom" { ScalarPatch::FixedValue(0.0) } else { ScalarPatch::ZeroGradient })
        .collect();
    LegacyFvmThermoMechanics::new(mesh.clone(), s, &zones, &[card()], disp, scalar.clone(), scalar, vec![700.0; mesh.n_cells], 1.0)
        .expect("build")
}

/// A chain of three cells with an upwind-style asymmetric matrix is solved to the tolerance by `PBiCG`/`DILU`; a
/// system whose guess already satisfies the tolerance reports 0 iterations (the upstream early exit).
#[test]
fn pbicg_dilu_solves_an_asymmetric_chain_and_exits_at_once_when_converged() {
    let mut a = LduMatrix::new(3, vec![0, 1], vec![1, 2]);
    a.diag = vec![2.0, 3.0, 4.0];
    a.upper = vec![-0.5, -1.0];
    a.lower = vec![-1.5, -0.25];
    let x_exact = [1.0, 2.0, 3.0];
    let b = a.multiply(&x_exact);
    let mut x = vec![0.0; 3];
    let perf = pbicg::pbicg_dilu(&a, &b, &mut x, 1e-14, 0.0, 1000, 0);
    assert!(perf.iterations >= 1 && perf.iterations < 10, "{perf:?}");
    for i in 0..3 {
        assert!((x[i] - x_exact[i]).abs() < 1e-12, "x[{i}] = {}", x[i]);
    }
    let again = pbicg::pbicg_dilu(&a, &b, &mut x, 1e-6, 0.0, 1000, 0);
    assert_eq!(again.iterations, 0);
}

/// Uniform stretch of a block (plane strain, free sides, uniform `T = TRef`): the displacement is linear in `x`
/// and the explicit `divSigmaExp` of the compact form vanishes, so the implicit solve is exact; `meshDisp` is
/// `disp` with the pin-direction (`y`) part removed.
#[test]
fn uniform_stretch_is_linear_and_mesh_displacement_drops_the_axial_part() {
    let mesh = block();
    let mut tm = build(&mesh, settings(GradScheme::GaussLinear, LaplacianCorrection::Uncorrected));
    let n = mesh.n_cells;
    tm.set_coupling_fields(&vec![700.0; n], &vec![700.0; n], &vec![0.0; n]);
    let log = tm.correct_physics();
    assert!(log.n_correctors_run >= 1);
    assert!(log.max_sigma_eq.is_some_and(|m| m.is_finite() && m > 0.0));
    assert!(tm.t_struct_gamg_substituted() == false);
    for c in 0..n {
        let x = mesh.cell_centres[c].x;
        let d = tm.displacement()[c];
        assert!((d.x - 1e-4 * x / 0.6).abs() < 1e-11, "cell {c}: {} vs {}", d.x, 1e-4 * x / 0.6);
        assert!(d.y.abs() < 1e-9 && d.z.abs() < 1e-9);
        let m = tm.mesh_displacement()[c];
        assert!(m.y.abs() < 1e-15, "the pin-direction part of disp is removed and the expansions are zero");
        assert_eq!(m.x, d.x);
    }
    assert!(tm.residual().is_finite());
    assert!(tm.sigma_d().iter().all(|s| s.xx.is_finite()));
}

/// The fuel-expansion equation `div(n & Sf, fuelDisp) == alphaFuel (TFuel - TFuelRef)` with `bottom` fixed at 0 and
/// upwind transport along `y` integrates the source up the column: `fuelDisp = alphaFuel (TFuel - TFuelRef) y_top`
/// (cell value, `y_top` the upper face of the cell). The control-rod equation is the same with the reversed
/// orientation and a free end, so it carries the source toward the `top` boundary only through its own cells.
#[test]
fn axial_fuel_expansion_integrates_the_source_up_the_column() {
    let mesh = block();
    let mut tm = build(&mesh, settings(GradScheme::GaussLinear, LaplacianCorrection::Uncorrected));
    let n = mesh.n_cells;
    tm.set_coupling_fields(&vec![700.0; n], &vec![800.0; n], &vec![0.0; n]);
    let log = tm.correct_physics();
    assert!(log.solves.iter().any(|s| s.field == "fuelDisp") && log.solves.iter().any(|s| s.field == "CRDisp"));
    let rhs = 1.0e-5 * 100.0;
    for c in 0..n {
        let y_top = mesh.cell_centres[c].y + 0.05;
        assert!((tm.fuel_displacement()[c] - rhs * y_top).abs() < 1e-12, "cell {c}: {}", tm.fuel_displacement()[c]);
        // mesh displacement carries the fuel expansion along the pin direction
        assert!((tm.mesh_displacement()[c].y - (tm.fuel_displacement()[c] + tm.control_rod_displacement()[c])).abs() < 1e-12);
    }
}

/// `leastSquares` gradients with the `corrected` Laplacian (the shipped ESFR selection) reproduce the linear
/// stretch on this orthogonal block to far below the displacement amplitude, and the explicit correction path
/// (`vector_correction`, `scalar_correction`) runs.
#[test]
fn least_squares_corrected_scheme_keeps_the_linear_stretch() {
    let mesh = block();
    let mut tm = build(&mesh, settings(GradScheme::LeastSquares, LaplacianCorrection::Corrected));
    let n = mesh.n_cells;
    tm.set_coupling_fields(&vec![700.0; n], &vec![700.0; n], &vec![0.0; n]);
    tm.correct_physics();
    for c in 0..n {
        let x = mesh.cell_centres[c].x;
        assert!((tm.displacement()[c].x - 1e-4 * x / 0.6).abs() < 1e-8, "cell {c}");
    }
}

/// `mechanicsSolver off` zeroes `disp`, `fuelDisp`, `CRDisp` and `meshDisp` (the `else` branch of upstream), and
/// the porous `TStruct` equation then still follows `TStructFromTH`.
#[test]
fn mechanics_off_zeroes_the_displacements_and_thermal_solve_follows_the_forcing() {
    let mesh = block();
    let mut s = settings(GradScheme::GaussLinear, LaplacianCorrection::Uncorrected);
    s.solve_displacement = false;
    let mut tm = build(&mesh, s);
    let n = mesh.n_cells;
    tm.set_coupling_fields(&vec![750.0; n], &vec![800.0; n], &vec![0.0; n]);
    let log = tm.correct_physics();
    assert!(log.solves.iter().all(|s| s.field == "TStruct"));
    assert!(tm.displacement().iter().all(|d| d.x == 0.0 && d.y == 0.0 && d.z == 0.0));
    assert!(tm.mesh_displacement().iter().all(|d| d.x == 0.0));
    for c in 0..n {
        assert!((tm.t_struct()[c] - 750.0).abs() < 1e-9, "isPorous = 1 forces T = TStructFromTH");
    }
}

/// Selections the port does not implement are refused by name, never replaced.
#[test]
fn unported_selections_are_refused() {
    let mesh = block();
    let zones = vec![("all".to_string(), (0..mesh.n_cells).collect::<Vec<usize>>())];
    let np = mesh.patches.len();
    let mk = |s: LegacyFvmSettings| {
        LegacyFvmThermoMechanics::new(
            mesh.clone(),
            s,
            &zones,
            &[card()],
            vec![DispPatch::ZeroGradient; np],
            vec![ScalarPatch::ZeroGradient; np],
            vec![ScalarPatch::ZeroGradient; np],
            vec![0.0; mesh.n_cells],
            1.0,
        )
    };
    let mut s = settings(GradScheme::GaussLinear, LaplacianCorrection::Corrected);
    s.compact_normal_stress = false;
    assert!(matches!(mk(s), Err(AppBuilderError::UnsupportedScheme { .. })));
    let s = settings(GradScheme::CellLimitedGaussLinear(0.5), LaplacianCorrection::Corrected);
    assert!(matches!(mk(s), Err(AppBuilderError::UnsupportedScheme { .. })));
    let s = settings(GradScheme::GaussLinear, LaplacianCorrection::Limited(0.5));
    assert!(matches!(mk(s), Err(AppBuilderError::UnsupportedScheme { .. })));
    let ok = settings(GradScheme::GaussLinear, LaplacianCorrection::Corrected);
    assert!(mk(ok).is_ok());
    // a card whose zone does not exist
    let mut bad = card();
    bad.name = "nowhere".to_string();
    let r = LegacyFvmThermoMechanics::new(
        mesh.clone(),
        settings(GradScheme::GaussLinear, LaplacianCorrection::Corrected),
        &zones,
        &[bad],
        vec![DispPatch::ZeroGradient; np],
        vec![ScalarPatch::ZeroGradient; np],
        vec![ScalarPatch::ZeroGradient; np],
        vec![0.0; mesh.n_cells],
        1.0,
    );
    assert!(matches!(r, Err(AppBuilderError::Case { .. })));
}

/// `ZoneCard::new` carries upstream's `getOrDefault` defaults.
#[test]
fn zone_card_defaults_are_upstreams() {
    let c = ZoneCard::new("z");
    assert_eq!((c.rho, c.e, c.nu, c.cp, c.k, c.alpha), (1.0, 1.0, 1.0, 1.0, 1.0, 1.0));
    assert_eq!((c.t_ref, c.t_fuel_ref, c.alpha_fuel, c.t_cr_ref, c.alpha_cr), (0.0, 0.0, 0.0, 0.0, 0.0));
}
