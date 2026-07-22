//! Verification tests — "is the RICHARDS discretisation implemented correctly?"
//!
//! These are **verification**, not validation: they check the numerics against
//! closed-form / manufactured references, not against physical experiment or
//! published PFLOTRAN gold-files (that is bead op-v6s.9, still open). Per the
//! workspace V&V-documentation rule, each test records both its **methodology**
//! and its **measured results** in the doc comment.
//!
//! All tests use only the crate's public API, so they double as executable,
//! read-top-to-bottom usage examples (the human-interface rule).

use outram_park_fork_pflotran::flow::richards::{
    BoundaryCondition, BoundaryConditionKind, RichardsProblem,
};
use outram_park_fork_pflotran::grid::{BoundaryLocation, CartesianGrid};
use outram_park_fork_pflotran::properties::{CharacteristicCurves, LiquidWaterEos, VanGenuchten};
use outram_park_fork_pflotran::solver::{
    LinearSolverKind, NewtonConfig, NewtonSolver, NonlinearSystem, PreconditionerKind,
};
use outram_foam_basic_lib::krylov::KrylovSettings;
use uom::si::f64::Length;
use uom::si::length::meter;

/// Tight Newton/Krylov config for steady verification solves.
fn steady_config() -> NewtonConfig {
    NewtonConfig {
        max_iterations: 100,
        abs_tol: 1.0e-8,
        rel_tol: 1.0e-12,
        linear: LinearSolverKind::BiCGStab,
        preconditioner: PreconditionerKind::Ilu0,
        linear_settings: KrylovSettings {
            tolerance: 1.0e-13,
            max_iter: 5000,
            restart: 60,
        },
        max_backtracks: 12,
    }
}

/// Build a 1D saturated, incompressible, gravity-free column of `nx` cells over
/// physical length `L`, with a manufactured source so that
///
///   p*(x) = p_L + (p_R - p_L) x/L + amp * sin(pi x / L)
///
/// is the exact continuous steady solution of  -rho*lambda * p'' = Q_vol .
/// Returns the solved cell pressures and the exact pressures at cell centres.
fn solve_mms_column(nx: usize, length: f64, amp: f64) -> (Vec<f64>, Vec<f64>) {
    let dx = length / nx as f64;
    let grid = CartesianGrid::uniform(
        nx,
        1,
        1,
        Length::new::<meter>(dx),
        Length::new::<meter>(1.0),
        Length::new::<meter>(1.0),
    )
    .unwrap();

    // Incompressible water: compressibility = 0 => rho = rho_ref (constant),
    // so the saturated operator is the constant-coefficient Laplacian.
    let rho = 1000.0;
    let mu = 1.0e-3;
    let eos = LiquidWaterEos::new(rho, 0.0, 0.0, mu).unwrap();
    let k = 1.0e-12;
    // Saturated everywhere: reference gas pressure 0 (gauge) and pressures > 0
    // => Se = 1, k_r = 1. Residual saturation 0 so S_l = 1.
    let curves = CharacteristicCurves::VanGenuchten(VanGenuchten::new(1.0e-4, 2.0, 0.0).unwrap());

    let p_l = 2.0e5;
    let p_r = 1.0e5;
    let pi = std::f64::consts::PI;
    let exact = |x: f64| p_l + (p_r - p_l) * (x / length) + amp * (pi * x / length).sin();

    let boundary = vec![
        BoundaryCondition {
            location: BoundaryLocation::XMin,
            kind: BoundaryConditionKind::DirichletPressure(exact(0.0)),
        },
        BoundaryCondition {
            location: BoundaryLocation::XMax,
            kind: BoundaryConditionKind::DirichletPressure(exact(length)),
        },
    ];

    let mut problem = RichardsProblem::new(
        grid, eos, curves, 0.35, k, 0.0, /*gravity*/ 0.0, /*p_gas*/ boundary,
    )
    .unwrap();

    // Manufactured volumetric mass source Q_vol = -rho*lambda*p''(x), with
    // lambda = k/mu (k_r = 1). p''(x) = -amp*(pi/L)^2 sin(pi x/L).
    let lambda = k / mu;
    let n = problem.n_dof();
    let mut source = vec![0.0; n];
    let cell_volume = dx * 1.0 * 1.0;
    for i in 0..n {
        let x = (i as f64 + 0.5) * dx;
        let p_second = -amp * (pi / length).powi(2) * (pi * x / length).sin();
        let q_vol = -rho * lambda * p_second; // kg / m^3 / s
        source[i] = q_vol * cell_volume; // kg / s
    }
    problem.set_source(&source);

    // Steady state: dt -> infinity kills accumulation.
    problem.set_timestep(1.0e30);
    problem.set_previous(&vec![1.5e5; n]);

    let solver = NewtonSolver::new(steady_config());
    let mut p = vec![1.5e5; n];
    solver
        .solve(&mut problem, &mut p)
        .expect("MMS steady solve converges");

    let exact_cells: Vec<f64> = (0..n).map(|i| exact((i as f64 + 0.5) * dx)).collect();
    (p, exact_cells)
}

fn linf_error(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f64::max)
}

/// **Method of Manufactured Solutions — spatial convergence order.**
///
/// Methodology: solve the saturated 1D steady operator on grids of 10/20/40/80/
/// 160 cells with the manufactured sinusoidal source above; measure the Linf
/// pressure error against the exact solution at cell centres; compute the
/// observed order of accuracy `log2(e_coarse / e_fine)` between successive
/// refinements. A cell-centred two-point-flux Laplacian is 2nd-order, so the
/// observed order must approach 2.
///
/// Results (measured 2026-07-22, release build):
///
/// | cells n | h (m) | Linf error (Pa) | ratio |
/// |--------:|------:|----------------:|------:|
/// |      10 | 0.100 |      4.0818e+1  |   —   |
/// |      20 | 0.050 |      1.0262e+1  | 3.98  |
/// |      40 | 0.025 |      2.5690e+0  | 3.99  |
/// |      80 | 0.0125|      6.4248e-1  | 4.00  |
/// |     160 | 0.00625|     1.6063e-1  | 4.00  |
///
/// Observed order between the two finest grids = **2.000** — the two-point-flux
/// Laplacian converges at its 2nd-order design rate. Interpretation: the spatial
/// discretisation of the saturated operator is implemented correctly. (This is
/// verification only; it says nothing about validity for a real soil.)
#[test]
fn mms_saturated_operator_is_second_order() {
    let length = 1.0;
    let amp = 5.0e3;
    let sizes = [10usize, 20, 40, 80, 160];
    let mut errors = Vec::new();
    for &n in &sizes {
        let (p, exact) = solve_mms_column(n, length, amp);
        let e = linf_error(&p, &exact);
        errors.push(e);
        println!("n={n:4}  Linf_error={e:.6e}");
    }
    // Every refinement must reduce the error.
    for w in errors.windows(2) {
        assert!(w[1] < w[0], "error did not decrease under refinement: {w:?}");
    }
    // Observed order between the two finest grids must approach 2.
    let order = (errors[errors.len() - 2] / errors[errors.len() - 1]).log2();
    println!("observed order (finest pair) = {order:.3}");
    assert!(
        order > 1.8,
        "observed convergence order {order:.3} is well below the 2nd-order design"
    );
}

/// **Closed-form saturated steady state — exact linear profile.**
///
/// Methodology: gravity-free saturated column, Dirichlet ends, zero source. The
/// steady solution of a constant-coefficient Laplacian is exactly linear, which
/// a 2nd-order scheme must reproduce to round-off (independent of grid size).
///
/// Result (2026-07-22): max deviation from the analytical line is < 1e-3 Pa on a
/// 25-cell grid (asserted below).
#[test]
fn saturated_linear_profile_is_exact() {
    let (p, exact) = solve_mms_column(25, 1.0, 0.0 /* amp = 0 => pure linear */);
    let e = linf_error(&p, &exact);
    println!("linear-profile Linf error = {e:.3e} Pa");
    assert!(e < 1.0e-3, "linear steady profile not reproduced: {e:e} Pa");
}
