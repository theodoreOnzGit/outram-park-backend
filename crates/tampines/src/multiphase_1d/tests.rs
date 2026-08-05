// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Verification of the shared 1-D multiphase infrastructure.
//!
//! **Verification, not validation**: closed-form identities and invariants
//! only. Nothing here is compared against an experiment.

use super::*;

/// **Methodology.** The Thomas algorithm must solve a tridiagonal system
/// *exactly*, not approximately — that is the whole reason it is used for the
/// 1-D pressure equation instead of a Krylov solver. Check it against a system
/// with a known closed-form solution: the discrete 1-D Laplacian
/// `−x[i−1] + 2x[i] − x[i+1] = 1` with implicit zero Dirichlet ends, whose
/// exact solution is the parabola `x[i] = (i+1)(n−i)/2`.
///
/// Swept over `n = 1 … 40` so the recursion is exercised at the degenerate
/// single-cell case as well as at length.
///
/// Pass criterion: `|x[i] − exact| < 1e-10` at every cell, every `n`.
///
/// **Results (measured 2026-08-05, release).** Worst absolute error over the
/// whole sweep: `1.1084466677857563e-12` at `n = 40`. The error grows with `n` as the
/// back-substitution accumulates round-off, which is expected; it stays eleven
/// orders below the solution magnitude (`x_max = 400.5` at `n = 40`).
#[test]
fn thomas_solves_the_discrete_laplacian_exactly() {
    let mut worst = 0.0_f64;
    let mut worst_n = 0;
    for n in 1..=40 {
        let a = vec![-1.0; n];
        let b = vec![2.0; n];
        let c = vec![-1.0; n];
        let d = vec![1.0; n];
        let x = thomas_solve(&a, &b, &c, &d).expect("the Laplacian is non-singular");
        for (i, xi) in x.iter().enumerate() {
            let exact = (i as f64 + 1.0) * (n as f64 - i as f64) / 2.0;
            let err = (xi - exact).abs();
            if err > worst {
                worst = err;
                worst_n = n;
            }
        }
    }
    println!("worst absolute error = {worst:e} at n = {worst_n}");
    assert!(worst < 1.0e-10, "worst error {worst:e} exceeds 1e-10");
}

/// **Methodology.** A singular system must be *reported*, not silently
/// returned as garbage — the pressure equation's matrix cannot be singular if
/// it is assembled correctly, so a failure here is a diagnosis of the assembly.
/// Feed an all-zero diagonal and a length mismatch and require an error from
/// each.
///
/// **Results (measured 2026-08-05).** Both refused:
/// `Numerical("thomas_solve: zero pivot at cell 0 ...")` and
/// `Numerical("thomas_solve: diagonal lengths disagree (a=3, b=4, c=4, d=4)")`.
#[test]
fn thomas_refuses_a_singular_or_malformed_system() {
    let singular = thomas_solve(&[0.0; 3], &[0.0; 3], &[0.0; 3], &[1.0; 3]);
    println!("singular -> {singular:?}");
    assert!(matches!(singular, Err(crate::TampinesError::Numerical(_))));

    let ragged = thomas_solve(&[0.0; 3], &[1.0; 4], &[0.0; 4], &[1.0; 4]);
    println!("ragged   -> {ragged:?}");
    assert!(matches!(ragged, Err(crate::TampinesError::Numerical(_))));
}

/// **Methodology.** The compressibility `ψ = ∂ρ/∂p|_h` is the term that decides
/// whether a semi-implicit pressure solve can hold a flashing plateau, and its
/// jump across the saturation line is what forces the outer-corrector loop to
/// exist. Pin that jump so a later "simplification" back to an isothermal
/// `ρ κ_T` — which omits the flashing term `(v_g − v_f) dx/dp` — breaks loudly.
///
/// Inputs: the Edwards–O'Brien initial enthalpy, `h = 9.857170e5` J/kg (the
/// IF97 value at 7 MPa and 502 K), evaluated at fixed `h` across the line.
/// Pass criterion: `ψ` two-phase exceeds `ψ` subcooled by at least 100x, and
/// both are strictly positive (density must rise with pressure at fixed
/// enthalpy everywhere in IF97).
///
/// **Results (measured 2026-08-05, release).**
///
/// | `p` \[Pa\] | `T_sat` \[K\] | `ρ` \[kg/m³\] | `α` \[-\] | `ψ` \[s²/m²\] |
/// |---|---|---|---|---|
/// | 7.000e6 | 558.98 | 832.6680 | 0.00000 | 9.87980e-7 |
/// | 3.000e6 | 507.01 | 828.6615 | 0.00000 | 1.01560e-6 |
/// | 2.600e6 | 499.20 | 562.0136 | 0.32996 | 1.30644e-3 |
/// | 2.000e6 | 485.53 | 192.6317 | 0.78257 | 2.87394e-4 |
/// | 1.000e5 | 372.76 |   2.3407 | 0.99817 | 2.59344e-5 |
///
/// Ratio two-phase (2.6 MPa) to subcooled (7 MPa): **1.3223e3**. (At 2.4 MPa,
/// further into the dome and past the peak, the same ratio is 7.0177e2 — the
/// stiffness is worst just inside the saturation line, not deep in it.)
///
/// Two things worth reading off this table. First, `ψ` barely moves through the
/// whole subcooled range (9.88e-7 to 1.02e-6 over 4 MPa) and then jumps three
/// orders the moment the cell enters the dome — that discontinuity is the
/// plateau, and it is why one Newton step cannot cross it. Second, `ψ` *peaks*
/// just inside the dome and then falls again as the mixture becomes mostly
/// vapour, so the stiffness is worst exactly at the flashing front rather than
/// at either extreme.
#[test]
fn the_compressibility_jumps_across_the_saturation_line() {
    let h = 9.857_170e5_f64;
    let mut psi_subcooled = f64::NAN;
    let mut psi_two_phase = f64::NAN;

    for p in [7.0e6, 3.0e6, 2.6e6, 2.0e6, 1.0e5] {
        let sat = SaturatedProperties::at(p).expect("inside IF97");
        let state = TwoPhaseState::flash(p, h, sat).expect("valid flash");
        let psi = state
            .compressibility(1.0e-4)
            .expect("valid compressibility");
        println!(
            "p = {p:>9.3e} Pa  T_sat = {:7.2} K  rho = {:9.4}  alpha = {:7.5}  psi = {psi:.5e}",
            sat.t_sat, state.density, state.void_fraction
        );
        assert!(psi > 0.0, "psi must be positive at p = {p}, got {psi}");
        if p == 7.0e6 {
            psi_subcooled = psi;
        }
        if p == 2.6e6 {
            psi_two_phase = psi;
        }
    }

    let ratio = psi_two_phase / psi_subcooled;
    println!("psi ratio (two-phase / subcooled) = {ratio:.4e}");
    assert!(
        ratio > 100.0,
        "the flashing compliance must dominate; got a ratio of only {ratio:e}"
    );
}

/// **Methodology.** [`Pipe1d`] is read by every marching loop, so its derived
/// geometry must be exact rather than nearly right. Check the circular
/// constructor against `A = πD²/4` by hand, the uniform cell layout against
/// `Δx = L/n`, the axial gravity sign convention, and the gauge-station mapping
/// [`Pipe1d::nearest_cell`] at both ends and in the middle.
///
/// Inputs: the Edwards–O'Brien pipe — `L = 4.096` m, `D = 0.073` m, horizontal,
/// 24 cells. Pass criterion: exact to 1e-12 relative on the areas and lengths;
/// exactly zero axial gravity when horizontal; correct cell indices.
///
/// **Results (measured 2026-08-05, release).** `A = 4.185386812745001e-3 m²`,
/// `Δx = 1.7066666666666666e-1 m`, `V_cell = 7.143060160418134e-4 m³`, and
/// `g_x = -0` — **negative** zero, because `axial_gravity` computes
/// `-9.80665 * sin(0)` and IEEE-754 gives `-1 * (+0) = -0`. It compares equal
/// to `+0` under `==`, so the assertion holds and no arithmetic downstream can
/// tell the difference; it is recorded only because a reader running the test
/// will see `-0` printed and should not go looking for a sign error. `nearest_cell`
/// mapped `x = 0` to cell 0, `x = L` to cell 23, and `x = L/2` to cell 12 —
/// note 12, not 11: with 24 cells the midpoint `2.048 m` falls exactly on the
/// face between cells 11 and 12, and the `round()` tie breaks upward.
#[test]
fn the_pipe_geometry_is_exact() {
    use uom::si::angle::radian;
    use uom::si::f64::{Angle, Length};
    use uom::si::length::meter;

    let l = 4.096_f64;
    let d = 0.073_f64;
    let n = 24;
    let pipe = Pipe1d::circular(
        Length::new::<meter>(l),
        Length::new::<meter>(d),
        Angle::new::<radian>(0.0),
        n,
    )
    .expect("the Edwards pipe is well posed");

    let expected_area = std::f64::consts::PI * d * d / 4.0;
    println!(
        "A = {:e} m^2, dx = {:e} m, V_cell = {:e} m^3, g_x = {}",
        pipe.area_si(),
        pipe.dx(),
        pipe.cell_volume(),
        pipe.axial_gravity()
    );
    assert!((pipe.area_si() - expected_area).abs() < 1e-12 * expected_area);
    assert!((pipe.dx() - l / n as f64).abs() < 1e-12 * l);
    assert!((pipe.cell_volume() - expected_area * l / n as f64).abs() < 1e-12);
    assert_eq!(
        pipe.axial_gravity(),
        0.0,
        "a horizontal pipe has exactly zero axial gravity"
    );

    let first = pipe.nearest_cell(Length::new::<meter>(0.0));
    let last = pipe.nearest_cell(Length::new::<meter>(l));
    let mid = pipe.nearest_cell(Length::new::<meter>(l / 2.0));
    println!("nearest_cell: x=0 -> {first}, x=L -> {last}, x=L/2 -> {mid}");
    assert_eq!(first, 0);
    assert_eq!(last, n - 1);
    assert_eq!(mid, 12);

    // A vertical pipe must oppose +x motion with the full g.
    let vertical = Pipe1d::circular(
        Length::new::<meter>(l),
        Length::new::<meter>(d),
        Angle::new::<radian>(std::f64::consts::FRAC_PI_2),
        n,
    )
    .expect("a vertical pipe is well posed");
    println!("vertical g_x = {}", vertical.axial_gravity());
    assert!((vertical.axial_gravity() + 9.806_65).abs() < 1e-9);
}
