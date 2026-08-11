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

/// **Methodology — direct verification of
/// [`super::drift_flux::AxialBoundary::ReservoirInlet`] against Bernoulli.**
///
/// # What is computed
///
/// A horizontal pipe, initially at rest and uniformly filled with **deeply
/// subcooled** water, is opened at `t = 0` between a stagnation plenum
/// ([`ReservoirInlet`](super::drift_flux::AxialBoundary::ReservoirInlet) at
/// `p_0`, `h_0`) and a receiver
/// ([`PressureOutlet`](super::drift_flux::AxialBoundary::PressureOutlet) at
/// `p_out`) and marched to steady state. The steady face velocity is compared
/// against a **closed form derived from the discrete scheme itself**, which for
/// this configuration coincides with the textbook incompressible
/// pipe-discharge result. Nothing here is compared against an experiment: this
/// is verification, not validation.
///
/// # The closed form, and why it is exact for this scheme
///
/// Single-phase (`α = 0`) kills the drift momentum flux `Φ` (`super::drift_flux`'s
/// `drift_momentum_flux` returns `0` at `α ≤ 0`), and a
/// horizontal pipe kills `g_x`. Setting `u_new = u_old` in each of the three
/// momentum balances the solver actually writes, at a spatially uniform `u` and
/// `ρ` (justified below), leaves:
///
/// - **Interior face `j ∈ [1, n−1]`** — donor convection `u ∂u/∂x` vanishes at
///   uniform `u`, and the correction `u ← u* − d (p_j − p_{j−1})` with
///   `d = Δt/(ρ Δx)` gives `p[j] − p[j−1] = −ρ F Δx` between the two cell
///   centres the face separates.
/// - **Left face (the boundary under test)** — the *conservative half-cell*
///   convection `d(u²/2)/dx ≈ (u²/2 − 0)/(Δx/2) = u²/Δx` from the stagnant
///   plenum, with the half-cell coefficient `d = Δt/(ρ Δx/2)`, gives
///   `u²/2 + F Δx/2 + (p[0] − p_0)/ρ = 0`. **This is the mechanism being
///   tested**: it is exactly what the variant's doc comment claims ("the steady
///   state of the conservative form is exactly `u = sqrt(2 (p_0 − p_1)/ρ)` …
///   whereas the non-conservative donor form … returns `sqrt((p_0 − p_1)/ρ)`,
///   low by `sqrt(2)`"). A regression to the non-conservative form would show
///   up here as a `1/sqrt(2) = −29.3 %` velocity error, thirty times the
///   tolerance below.
/// - **Right face** — outflow uses the interior donor form, which again
///   vanishes at uniform `u`, so `p[n−1] − p_out = ρ F Δx/2`.
///
/// Summing the three telescopes the interior pressures away and charges wall
/// friction over `Δx/2 + (n−1) Δx + Δx/2 = L` exactly:
///
/// `p_0 − p_out = ρ u²/2 + ρ F L`,  `F = f |u| u / (2 D_h)`
///
/// so
///
/// `u_steady = sqrt( 2 (p_0 − p_out) / (ρ (1 + f L / D_h)) )`.
///
/// **Wall friction is incorporated into the expected value, not made
/// negligible** — it is a ~3.5 % share of the driving head here, deliberately
/// large enough that the test also pins the friction bookkeeping (`L`, not
/// `L ± Δx`). `f` is the solver's own documented Darcy closure — laminar
/// `64/Re` below `Re = 2300`, Blasius `0.316 Re^{−1/4}` above — evaluated by
/// fixed-point iteration on `Re = ρ u D_h / μ_f`. Blasius is used well past its
/// nominal `Re ≈ 10⁵` range; that is a property of the *code under test*, which
/// this verification reproduces rather than corrects.
///
/// # Inputs
///
/// | Quantity | Value |
/// |---|---|
/// | Plenum stagnation pressure `p_0` | `1.000` MPa |
/// | Receiver pressure `p_out` | `0.980` MPa (`Δp = 20` kPa) |
/// | Plenum / initial temperature | `300.0` K (`T_sat(1 MPa) = 453.0` K, so ~153 K subcooled) |
/// | Plenum stagnation enthalpy `h_0` | IF97 Region-1 `h(p_0, 300 K)` |
/// | Initial field | uniform `p = p_out`, `T = 300 K`, `u = 0` |
/// | Pipe | `L = 0.5` m, `D = 0.1` m, horizontal, 20 uniform cells (`Δx = 25` mm) |
/// | Timestep / end | `Δt = 5×10⁻⁴` s, `t_end = 1.5` s (3000 steps) |
/// | Slip closure | Zuber-Findlay `C₀ = 1.13`, `V_gj = 0.1` m/s `+x` (inert at `α = 0`) |
/// | `τ`, outer correctors, `α_p` | solver defaults, untouched |
///
/// `Δp = 20` kPa is chosen so the water stays single-phase and effectively
/// incompressible: at `ψ = ∂ρ/∂p|_h ≈ 4.5×10⁻⁷ s²/m²` the density varies by
/// `ψ Δp / ρ ≈ 9×10⁻⁶` over the whole drop, and `u²/2 ≈ 20` J/kg against
/// `h_0 ≈ 1.13×10⁵` J/kg is a `2×10⁻⁴` perturbation on the enthalpy — both far
/// below the tolerance. That is what justifies "uniform `ρ`" in the derivation.
///
/// # Pass criterion and its justification — **pre-committed before the first
/// run**
///
/// `|u_measured − u_closed_form| / u_closed_form <= 1.0 %`.
///
/// Error budget, summed *a priori*: property variation along the pipe (`μ_f` is
/// the saturated-liquid value at the local cell pressure, and the cells span
/// only ~0.7 kPa, so `f` varies by `<0.15 %`, i.e. `<0.08 %` on `u`) +
/// compressibility (`<0.01 %`) + the `u²/2` enthalpy shift on `ρ`
/// (`<0.001 %`) + the outer-corrector stopping tolerance (`1` Pa on a 20 kPa
/// head, parked at worst `~3` Pa by the `α_p = 0.7` under-relaxation, i.e.
/// `<0.01 %` on `u`) ≈ **0.1 %**. The gate is set an order of magnitude
/// looser at **1 %** because the reference `f` is evaluated once at
/// `(p_out, μ_f(p_out))` rather than face-by-face, and a knife-edge gate on a
/// friction model is not what this test exists to check. The scheme has **no
/// truncation error to budget for** — the derivation above is the discrete
/// steady state, not its continuum limit.
///
/// Three further gates, all closed-form or invariant, none reverse-fitted:
///
/// - `u_measured < u_frictionless_Bernoulli` strictly, and within `5 %` of it —
///   friction can only remove head, and `f L / D <= 0.05` bounds how much.
/// - **Global continuity**: `|ṁ_in − ṁ_out| / ṁ_out <= 1×10⁻³`.
/// - **Steadiness**: peak-to-peak variation of the inlet face velocity over the
///   last 20 % of the march, relative to its mean, `<= 1×10⁻³`. Asserted, not
///   assumed — a run still accelerating would otherwise pass low by an
///   arbitrary amount.
/// - Single-phase throughout (`max α = 0`) and `max Courant < 0.5`.
///
/// # Results (measured 2026-08-11, release, `cargo test --release -p tampines
/// --lib multiphase_1d`)
///
/// The 3000-step march completed in **6.76 s wall clock**, no step refused, no
/// NaN. (Two runs on 2026-08-11: 6.76 s and 6.05 s — the spread is machine
/// load; both produced bit-identical numbers.)
///
/// | Quantity | Measured |
/// |---|---|
/// | Reference state `h_0` | `1.134923e5` J/kg (`T = 300.004` K back out of the flash, `α = 0`) |
/// | Reference `ρ`, `μ_f` at `p_out` | `996.9502` kg/m³, `1.512639e-4` Pa·s |
/// | `Re`, `f`, `f L/D` | `4.1034e6`, `0.007021`, `0.035105` |
/// | **Closed form `u_steady`** | **`6.225884` m/s** |
/// | **Measured steady inlet `u`** | **`6.225891` m/s** |
/// | **Relative error** | **`+1.12e-6` (`+0.000112 %`)** |
/// | Frictionless Bernoulli `sqrt(2Δp/ρ)` | `6.334222` m/s; measured is `−1.7102 %` below it |
/// | Outlet face `u` | `6.225885` m/s |
/// | Global continuity `\|ṁ_in − ṁ_out\|/ṁ_out` | `1.277e-6` (`48.748981` vs `48.748919` kg/s) |
/// | Steadiness (p-p / mean, last 20 %) | `3.344e-7` |
/// | Max material Courant | `0.12452` |
/// | Max `α` over the run | `0` exactly |
///
/// Approach to steady state (inlet face velocity): `0.4007` m/s at `t = 0.01` s,
/// `4.8199` at `0.16` s, `6.0003` at `0.31` s, `6.1927` at `0.46` s, `6.2251`
/// at `0.76` s, `6.225891` at `1.36` s — an e-folding time of roughly
/// `L/u ≈ 0.08` s, so `t_end = 1.5` s is ~19 time constants and the residual
/// drift is at the `1e-7` level the steadiness metric reports.
///
/// **The pressure field decomposes exactly as the derivation predicts**, which
/// is the strongest single piece of evidence here because it checks each of the
/// three momentum balances separately rather than only their sum:
///
/// | Term | Predicted \[Pa\] | Measured \[Pa\] |
/// |---|---|---|
/// | `p_0 − p[0]` = `ρu²/2 + ρFΔx/2` | `19321.75 + 16.96` | `p[0] = 980661.29` vs predicted `980661.291` |
/// | `p[0] − p[n−1]` = `(n−1) ρ F Δx` | `644.376` | `644.33` |
/// | `p[n−1] − p_out` = `ρ F Δx/2` | `16.957` | `16.96` |
///
/// with `F = f u²/(2D) = 1.36073` m/s². Every term agrees to `≈0.01` Pa out of
/// a 20 kPa head.
///
/// # Interpretation
///
/// The `ReservoirInlet` boundary reaches its documented Bernoulli steady state
/// to **1.1 parts per million** — five orders inside the 1 % gate and three
/// orders inside the ~0.1 % *a priori* error budget, i.e. the discrete steady
/// state really is the closed form and the residual is the outer-corrector
/// stopping tolerance rather than any modelling error. In particular:
///
/// - The **conservative half-cell convection** is doing exactly what its doc
///   comment claims. The non-conservative alternative would land at
///   `sqrt(Δp/ρ) ≈ 4.48` m/s, `−28 %`; nothing near that is seen.
/// - The **half-cell pressure coefficient** `d = Δt/(ρ Δx/2)` is right: a
///   whole-cell `d` at the two Dirichlet faces would mis-charge the end
///   half-cells and show up in the `16.96` Pa end terms above (they would
///   double), which it does not.
/// - Wall friction is charged over exactly `L`, not `L ± Δx`.
///
/// **What this does NOT establish.** It is a single-phase, subcooled, steady,
/// horizontal, low-Mach check. It says nothing about the inflow *flash* (`α`
/// was identically zero throughout, so the `void_fraction`/`rho_g` members of
/// the inflow state were never exercised on a two-phase inlet), nothing about
/// the `h_0 − u²/2` static-enthalpy split beyond its `2e-4` relative size here,
/// nothing about backflow through either boundary, and nothing about
/// transients. Those remain unverified.
#[test]
fn reservoir_inlet_reaches_the_bernoulli_limit_in_subcooled_water() {
    use std::time::Instant;

    use outram_foam_basic_lib::primitives::Vector3;
    use outram_foam_multiphase::drift_flux::SlipModel;
    use uom::si::angle::radian;
    use uom::si::available_energy::joule_per_kilogram;
    use uom::si::f64::{Angle, Length, Pressure, ThermodynamicTemperature, Time};
    use uom::si::length::meter;
    use uom::si::pressure::pascal;
    use uom::si::thermodynamic_temperature::kelvin;
    use uom::si::time::second;

    use super::drift_flux::{AxialBoundary, DriftFlux1d};

    // ── The case, as tabulated in the doc comment above ──────────────────────
    const P0_PA: f64 = 1.000e6;
    const P_OUT_PA: f64 = 0.980e6;
    const T0_K: f64 = 300.0;
    const L_M: f64 = 0.5;
    const D_M: f64 = 0.1;
    const N_CELLS: usize = 20;
    const DT_S: f64 = 5.0e-4;
    const T_END_S: f64 = 1.5;
    const SAMPLE_EVERY: usize = 20;
    // Pre-committed; see the "Pass criterion" section above. Never widen this
    // to make a run pass -- a miss is a finding about the boundary condition.
    const REL_TOL: f64 = 1.0e-2;

    // ── The closed-form reference, computed BEFORE the march ─────────────────
    let h0 = tampines_steam_tables::region_1_subcooled_liquid::h_tp_1(
        ThermodynamicTemperature::new::<kelvin>(T0_K),
        Pressure::new::<pascal>(P0_PA),
    )
    .get::<joule_per_kilogram>();
    let sat_out = SaturatedProperties::at(P_OUT_PA).expect("0.98 MPa is inside IF97");
    let state_ref = TwoPhaseState::flash(P_OUT_PA, h0, sat_out).expect("subcooled flash");
    let rho_ref = state_ref.density;
    let mu_ref = sat_out.mu_f;
    let dp = P0_PA - P_OUT_PA;

    // The solver's own Darcy closure, reproduced (it is a private fn of
    // `drift_flux`, so it cannot be called from a sibling test module).
    let darcy = |re: f64| {
        if re < 2300.0 {
            64.0 / re
        } else {
            0.316 * re.powf(-0.25)
        }
    };
    let u_bernoulli = (2.0 * dp / rho_ref).sqrt();
    let mut u_expected = u_bernoulli;
    let mut f_expected = 0.0;
    let mut re_expected = 0.0;
    for _ in 0..200 {
        re_expected = rho_ref * u_expected * D_M / mu_ref;
        f_expected = darcy(re_expected);
        u_expected = (2.0 * dp / (rho_ref * (1.0 + f_expected * L_M / D_M))).sqrt();
    }
    println!(
        "reference state: h_0 = {h0:.6e} J/kg, T = {:.3} K, rho = {rho_ref:.4} kg/m^3, \
         mu_f = {mu_ref:.6e} Pa.s, alpha = {:.3e}",
        state_ref.temperature, state_ref.void_fraction
    );
    println!(
        "closed form: u_frictionless = {u_bernoulli:.6} m/s, Re = {re_expected:.4e}, \
         f = {f_expected:.6}, f L/D = {:.6}, u_expected = {u_expected:.6} m/s \
         (friction costs {:.3} %)",
        f_expected * L_M / D_M,
        100.0 * (1.0 - u_expected / u_bernoulli)
    );
    assert!(
        state_ref.void_fraction == 0.0,
        "the reference state must be single-phase subcooled liquid, got alpha = {}",
        state_ref.void_fraction
    );

    // ── Build and march ──────────────────────────────────────────────────────
    let pipe = Pipe1d::circular(
        Length::new::<meter>(L_M),
        Length::new::<meter>(D_M),
        Angle::new::<radian>(0.0),
        N_CELLS,
    )
    .expect("the discharge pipe is well posed");
    let area = pipe.area_si();

    let slip = SlipModel::ZuberFindlay {
        c0: 1.13,
        vgj: Vector3::new(0.1, 0.0, 0.0),
    };
    let mut solver = DriftFlux1d::new(
        pipe,
        slip,
        Pressure::new::<pascal>(P_OUT_PA),
        ThermodynamicTemperature::new::<kelvin>(T0_K),
        Time::new::<second>(DT_S),
    )
    .expect("the initial state is inside IF97");
    solver.set_left_boundary(AxialBoundary::ReservoirInlet {
        stagnation_pressure: P0_PA,
        stagnation_enthalpy: h0,
    });
    solver.set_right_boundary(AxialBoundary::PressureOutlet {
        pressure: P_OUT_PA,
    });

    let n_steps = (T_END_S / DT_S).round() as usize;
    let mut u_inlet_history: Vec<(f64, f64)> = Vec::new();
    let mut max_courant = 0.0_f64;
    let mut max_void = 0.0_f64;
    let started = Instant::now();
    for step in 1..=n_steps {
        let report = solver
            .step()
            .unwrap_or_else(|e| panic!("step {step} failed at t = {} s: {e}", solver.time().get::<second>()));
        max_courant = max_courant.max(report.max_courant);
        max_void = max_void.max(report.max_void_fraction);
        if step % SAMPLE_EVERY == 0 {
            u_inlet_history.push((report.time, solver.face_velocity()[0]));
        }
    }
    let wall = started.elapsed();

    // ── Steadiness, measured rather than assumed ─────────────────────────────
    let tail_start = u_inlet_history.len() * 4 / 5;
    let tail = &u_inlet_history[tail_start..];
    let tail_mean = tail.iter().map(|&(_, u)| u).sum::<f64>() / tail.len() as f64;
    let tail_min = tail.iter().map(|&(_, u)| u).fold(f64::INFINITY, f64::min);
    let tail_max = tail
        .iter()
        .map(|&(_, u)| u)
        .fold(f64::NEG_INFINITY, f64::max);
    let steadiness = (tail_max - tail_min) / tail_mean;

    // ── Measured quantities ──────────────────────────────────────────────────
    let u_in = solver.face_velocity()[0];
    let u_out = solver.face_velocity()[N_CELLS];
    let rho = solver.density();
    let mdot_in = rho[0] * u_in * area;
    let mdot_out = rho[N_CELLS - 1] * u_out * area;
    let continuity = (mdot_in - mdot_out).abs() / mdot_out.abs();
    let rel_err = (u_in - u_expected) / u_expected;

    println!(
        "march: {n_steps} steps of {DT_S} s to t = {:.4} s in {:.2} s wall clock",
        solver.time().get::<second>(),
        wall.as_secs_f64()
    );
    for &(t, u) in u_inlet_history.iter().step_by(u_inlet_history.len() / 10) {
        println!("  t = {t:.4} s  u_inlet = {u:.6} m/s");
    }
    println!(
        "steady: u_in = {u_in:.6} m/s, u_out = {u_out:.6} m/s, \
         u_expected = {u_expected:.6} m/s, rel err = {:.4} %",
        100.0 * rel_err
    );
    println!(
        "        vs frictionless Bernoulli {u_bernoulli:.6} m/s: {:.4} %",
        100.0 * (u_in - u_bernoulli) / u_bernoulli
    );
    println!(
        "        mdot_in = {mdot_in:.6} kg/s, mdot_out = {mdot_out:.6} kg/s, \
         continuity = {continuity:.3e}"
    );
    println!(
        "        steadiness (peak-to-peak / mean over last 20 % of the march) = {steadiness:.3e}"
    );
    println!("        max Courant = {max_courant:.5}, max alpha = {max_void:.3e}");
    println!(
        "        p[0] = {:.2} Pa, p[n-1] = {:.2} Pa, rho[0] = {:.4}, rho[n-1] = {:.4} kg/m^3",
        solver.pressure()[0],
        solver.pressure()[N_CELLS - 1],
        rho[0],
        rho[N_CELLS - 1]
    );

    // ── Gates ────────────────────────────────────────────────────────────────
    assert!(
        max_void == 0.0,
        "the case must stay single-phase for the Bernoulli comparison to mean \
         anything; got max alpha = {max_void}"
    );
    assert!(
        max_courant < 0.5,
        "material Courant {max_courant} is too large for donor-cell transport"
    );
    assert!(
        steadiness <= 1.0e-3,
        "the march has not reached steady state: peak-to-peak / mean over the \
         last 20 % is {steadiness:e}"
    );
    assert!(
        continuity <= 1.0e-3,
        "global continuity is violated: mdot_in = {mdot_in} kg/s vs mdot_out = \
         {mdot_out} kg/s ({continuity:e} relative)"
    );
    assert!(
        u_in < u_bernoulli,
        "wall friction can only remove head, so the steady velocity {u_in} m/s \
         must be below the frictionless Bernoulli value {u_bernoulli} m/s"
    );
    assert!(
        u_in > 0.95 * u_bernoulli,
        "f L/D <= 0.05 bounds the friction correction at 2.5 % on u; {u_in} m/s \
         is more than 5 % below the frictionless {u_bernoulli} m/s, so something \
         other than the documented Darcy closure is removing momentum"
    );
    assert!(
        rel_err.abs() <= REL_TOL,
        "steady inlet velocity {u_in} m/s differs from the closed-form \
         {u_expected} m/s by {:.4} %, outside the pre-committed {:.2} % gate",
        100.0 * rel_err,
        100.0 * REL_TOL
    );
}
