// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Verification tests for [`super`] — the 1-D interfacial exchange layer.
//!
//! # What kind of tests these are
//!
//! **Verification, not validation.** Every case here checks the wiring against
//! a *closed form* that can be worked out by hand from the ported
//! correlations, or against an invariant the formulation must satisfy
//! identically. No case is compared against an experiment, and no claim of
//! physical fidelity follows from any of them passing. The interfacial
//! closures in this module have **not** been validated against measured
//! phase-change data.
//!
//! # Why closed forms and not stored reference numbers
//!
//! The two failure modes this layer is most exposed to are (a) reading the
//! wrong phase's conductivity into a resistance and (b) getting a sign
//! backwards. Both produce numbers that look entirely plausible, so a
//! regression test against a stored output would have locked either one in.
//! Each case below therefore recomputes the expected value from the
//! correlation's *analytic limit* — `Nu = 2` at `Re = 0` for Ranz-Marshall,
//! `Nu = 10` for spherical conduction, `C_d Re = 24` (Stokes) at `Re = 0` for
//! Schiller-Naumann — and multiplies it by the property the correct side
//! should have read.
//!
//! Where a test prints a measured number, that number is transcribed into the
//! test's own doc comment with the date it was taken, per the workspace V&V
//! documentation rule.
//!
//! # Common inputs
//!
//! Unless a test says otherwise: `p = 7 MPa` (a representative steam-generator
//! secondary pressure, comfortably inside IF97 Region 1/2 and far from the
//! critical point), inclusion diameter `d = 1 mm`, void fraction
//! `α_g = 0.1`. Property values at that pressure, printed by
//! `reference_properties_at_seven_megapascal` on 2026-08-12 from
//! IAPWS-IF97 via `tampines-steam-tables`:
//!
//! ```text
//! T_sat    = 558.980023 K
//! h_f      = 1.267437e6 J/kg      h_g      = 2.772569e6 J/kg
//! h_fg     = 1.505132e6 J/kg
//! rho_f    = 739.723664 kg/m^3      rho_g    = 36.523593 kg/m^3
//! mu_f     = 9.126631e-5 Pa s     mu_g     = 1.888953e-5 Pa s
//! lambda_f = 0.584448 W/(m K)    lambda_g = 0.063455 W/(m K)
//! Pr_f     = 0.843315            Pr_g     = 1.593803
//! lambda_f / lambda_g = 9.210375
//! ```

use uom::si::f64::Length;
use uom::si::length::{meter, millimeter};

use super::*;
use crate::multiphase_1d::properties::{SaturatedProperties, SaturatedTransport};

/// Reference pressure for the fixed-property cases \[Pa\].
const P_REF: f64 = 7.0e6;
/// Reference inclusion diameter \[m\].
const D_REF: f64 = 1.0e-3;
/// Reference void fraction \[-\].
const ALPHA_REF: f64 = 0.1;

/// Build the reference property pair at [`P_REF`].
fn reference_properties() -> (SaturatedProperties, SaturatedTransport) {
    let sat = SaturatedProperties::at(P_REF).expect("IF97 saturation set at 7 MPa");
    let transport = SaturatedTransport::at(P_REF).expect("IF97 conduction set at 7 MPa");
    (sat, transport)
}

/// A cell at [`P_REF`] with both phases exactly on the saturation line and no
/// slip, which every test then perturbs in exactly one direction.
fn saturated_cell(sat: SaturatedProperties) -> InterfacialCellState {
    InterfacialCellState {
        pressure: P_REF,
        void_fraction: ALPHA_REF,
        vapour_enthalpy: sat.h_g,
        liquid_enthalpy: sat.h_f,
        vapour_velocity: 0.0,
        liquid_velocity: 0.0,
    }
}

fn bubbly() -> InterfacialExchange {
    InterfacialExchange::bubbly(Length::new::<millimeter>(1.0)).expect("bubbly closure set")
}

/// **Methodology.** Print the IAPWS-IF97 property set the other tests build
/// their closed forms from, so every hand-computed expectation in this file can
/// be reproduced from published data rather than from this crate's internals.
/// Inputs: `p = 7 MPa`. Pass criterion: the qualitative orderings that every
/// other test relies on hold — `h_fg > 0`, `rho_f > rho_g`, `lambda_f >
/// lambda_g` (so that a continuous/dispersed conductivity swap is detectable),
/// and both Prandtl numbers positive.
///
/// **Results (2026-08-12).** As printed in the module doc comment above; the
/// decisive one is `lambda_f / lambda_g = 9.210375`, i.e. mixing the two sides
/// up misstates a resistance by nearly an order of magnitude. That is the
/// margin the conductivity-side tests below exploit.
#[test]
fn reference_properties_at_seven_megapascal() {
    let (sat, transport) = reference_properties();
    let pr_f = transport.liquid_prandtl(sat).expect("liquid Prandtl");
    let pr_g = transport.vapour_prandtl(sat).expect("vapour Prandtl");
    println!("T_sat    = {:.6} K", sat.t_sat);
    println!(
        "h_f      = {:.6e} J/kg      h_g      = {:.6e} J/kg",
        sat.h_f, sat.h_g
    );
    println!("h_fg     = {:.6e} J/kg", sat.h_fg());
    println!(
        "rho_f    = {:.6} kg/m^3      rho_g    = {:.6} kg/m^3",
        sat.rho_f, sat.rho_g
    );
    println!(
        "mu_f     = {:.6e} Pa s     mu_g     = {:.6e} Pa s",
        sat.mu_f, sat.mu_g
    );
    println!(
        "lambda_f = {:.6} W/(m K)    lambda_g = {:.6} W/(m K)",
        transport.lambda_f, transport.lambda_g
    );
    println!("Pr_f     = {pr_f:.6}            Pr_g     = {pr_g:.6}");
    println!(
        "lambda_f / lambda_g = {:.6}",
        transport.lambda_f / transport.lambda_g
    );

    assert!(sat.h_fg() > 0.0);
    assert!(sat.rho_f > sat.rho_g);
    assert!(transport.lambda_f > transport.lambda_g);
    assert!(pr_f > 0.0 && pr_g > 0.0);
}

/// **Methodology.** A two-resistance pair must resolve one resistance on each
/// side of the interface. Feeding two continuous-side models (or two
/// dispersed-side models) is the silent factor-of-`lambda_f/lambda_g` error the
/// module documents. This asserts [`InterfacialExchange::new`] refuses all four
/// wrong pairings and accepts the one right pairing, using
/// [`InterfacialHeatTransfer::resistance_side`] as the authority rather than a
/// hard-coded list of variant names.
///
/// Inputs: `d = 1 mm`, `alpha_res = 1e-6`, Schiller-Naumann drag. Pass
/// criterion: `Err(InvalidInput)` for `(RanzMarshall, RanzMarshall)`,
/// `(Gunn, Gunn)`, `(Gunn, RanzMarshall)` and `(Spherical, Spherical)`;
/// `Ok` for `(RanzMarshall, Spherical)` and `(Gunn, Spherical)`.
///
/// **Results (2026-08-12).** All four wrong pairings refused, both right
/// pairings accepted. This is the first of the three redundant guards described
/// in the module docs; the other two are exercised by the two conductivity
/// tests below.
#[test]
fn constructor_refuses_a_pair_from_the_same_resistance_side() {
    let d = Length::new::<millimeter>(1.0);
    let wrong = [
        (
            InterfacialHeatTransfer::RanzMarshall,
            InterfacialHeatTransfer::RanzMarshall,
        ),
        (InterfacialHeatTransfer::Gunn, InterfacialHeatTransfer::Gunn),
        (
            InterfacialHeatTransfer::Gunn,
            InterfacialHeatTransfer::RanzMarshall,
        ),
        (
            InterfacialHeatTransfer::Spherical,
            InterfacialHeatTransfer::Spherical,
        ),
    ];
    for (continuous_side, dispersed_side) in wrong {
        let built = InterfacialExchange::new(
            DragModel::SchillerNaumann,
            continuous_side,
            dispersed_side,
            DispersedPhase::Vapour,
            d,
            RESIDUAL_ALPHA,
        );
        assert!(
            matches!(built, Err(TampinesError::InvalidInput(_))),
            "pair ({continuous_side:?}, {dispersed_side:?}) should have been refused"
        );
    }

    for continuous_side in [
        InterfacialHeatTransfer::RanzMarshall,
        InterfacialHeatTransfer::Gunn,
    ] {
        assert!(
            InterfacialExchange::new(
                DragModel::SchillerNaumann,
                continuous_side,
                InterfacialHeatTransfer::Spherical,
                DispersedPhase::Vapour,
                d,
                RESIDUAL_ALPHA,
            )
            .is_ok(),
            "pair ({continuous_side:?}, Spherical) should have been accepted"
        );
    }
}

/// **Methodology.** At zero slip the traced-back Schiller-Naumann correlation
/// gives `C_d Re = 24 (1 + 0.15 Re^0.687) -> 24`, so the volumetric drag
/// coefficient collapses to the **Stokes** result, which is derivable from
/// first principles and independent of this codebase:
///
/// a single sphere in creeping flow feels `F = 3 pi mu_c d u`; a volume
/// fraction `alpha_d` of spheres of diameter `d` packs `n = 6 alpha_d / (pi
/// d^3)` of them per cubic metre; hence `K_d = n F / u = 18 alpha_d mu_c /
/// d^2`.
///
/// Inputs: bubbly set at `p = 7 MPa`, `alpha_g = 0.1`, `d = 1 mm`, `u_g = u_l =
/// 0`, so `mu_c = mu_f`. Reference: the Stokes closed form above. Tolerance:
/// relative `1e-12` — the two should agree to round-off, since `Re = 0`
/// makes the correlation's correction term identically zero. Pass criterion:
/// `|K_d / K_stokes - 1| < 1e-12`, `Re == 0`, and `F_g = 0` with the liquid
/// force its exact negative.
///
/// **Results (2026-08-12).** `K_d = 1.642794e2 kg/(m^3 s)` against
/// `18 alpha_d mu_f / d^2 = 1.642794e2 kg/(m^3 s)`, relative difference
/// `0.000000e0`. `Re = 0`, `F_g = 0 N/m^3`. The drag path is therefore wired
/// to the reference correlation with the continuous-phase viscosity and the
/// dispersed diameter, at the one point where the correlation has an
/// independent analytic answer.
#[test]
fn zero_slip_drag_recovers_the_stokes_closed_form() {
    let (sat, transport) = reference_properties();
    let exchange = bubbly();
    let sources = exchange
        .sources_with_properties(saturated_cell(sat), sat, transport)
        .expect("interfacial sources at 7 MPa");

    let stokes = 18.0 * ALPHA_REF * sat.mu_f / (D_REF * D_REF);
    let relative = (sources.volumetric_drag_coefficient - stokes).abs() / stokes;
    println!(
        "K_d = {:.6e} kg/(m^3 s), Stokes 18 alpha mu_f / d^2 = {stokes:.6e} kg/(m^3 s), \
         relative difference {relative:.6e}",
        sources.volumetric_drag_coefficient
    );
    println!(
        "Re = {}, F_g = {} N/m^3",
        sources.slip_reynolds_number, sources.drag_force_on_vapour
    );

    assert!(relative < 1.0e-12, "K_d = {stokes} vs {relative}");
    assert_eq!(sources.slip_reynolds_number, 0.0);
    assert_eq!(sources.drag_force_on_vapour, 0.0);
    assert_eq!(
        sources.drag_force_on_liquid(),
        -sources.drag_force_on_vapour
    );
}

/// **Methodology.** With a finite slip the drag force must be
/// `F_g = K_d (u_l - u_g)`, antisymmetric between the phases, and must oppose
/// the slip (decelerate the faster phase). Inputs: bubbly set at `p = 7 MPa`,
/// `alpha_g = 0.1`, `d = 1 mm`, `u_g = 2 m/s`, `u_l = 0.5 m/s`, both phases
/// saturated. Reference: the definition of the drag coefficient in
/// [`DragModel`] (`K_d (U_c - U_d)` on the dispersed phase). Tolerance:
/// relative `1e-12`. Pass criterion: the identity holds, `F_g < 0` (vapour
/// running ahead is decelerated), and `F_l = -F_g`.
///
/// **Results (2026-08-12).** `Re = 1.216e4` (past the Newton-regime switch at
/// `Re = 1000`, so this exercises the `C_d Re = 0.44 Re` branch, not the Stokes
/// one), `K_d = 3.661632e4 kg/(m^3 s)`, `F_g = -5.492448e4 N/m^3`, identity
/// residual `0.000000e0`. Momentum exchange is conserved by construction and
/// the sign is the physical one.
#[test]
fn finite_slip_drag_is_antisymmetric_and_opposes_the_slip() {
    let (sat, transport) = reference_properties();
    let exchange = bubbly();
    let mut cell = saturated_cell(sat);
    cell.vapour_velocity = 2.0;
    cell.liquid_velocity = 0.5;

    let sources = exchange
        .sources_with_properties(cell, sat, transport)
        .expect("interfacial sources with slip");

    let expected =
        sources.volumetric_drag_coefficient * (cell.liquid_velocity - cell.vapour_velocity);
    let residual = (sources.drag_force_on_vapour - expected).abs() / expected.abs();
    println!(
        "Re = {:.3e}, K_d = {:.6e} kg/(m^3 s), F_g = {:.6e} N/m^3, identity residual {residual:.6e}",
        sources.slip_reynolds_number, sources.volumetric_drag_coefficient, sources.drag_force_on_vapour
    );

    assert!(residual < 1.0e-12);
    assert!(sources.drag_force_on_vapour < 0.0);
    assert!((sources.drag_force_on_liquid() + sources.drag_force_on_vapour).abs() < 1.0e-9);
}

/// **Methodology — the decisive continuous/dispersed conductivity test.**
///
/// In bubbly flow the liquid is continuous and the vapour dispersed, so the
/// two-resistance framework requires:
///
/// - the **liquid** side to be closed by Ranz-Marshall reading the **liquid**
///   conductivity: at `Re = 0`, `Nu = 2` exactly (the stagnant-sphere
///   conduction limit), giving `K_l = 6 alpha_g lambda_f Nu / d^2 = 12 alpha_g
///   lambda_f / d^2`;
/// - the **vapour** side to be closed by spherical conduction reading the
///   **vapour** conductivity: `Nu = 10` identically, giving `K_g = 60 alpha_g
///   lambda_g / d^2`.
///
/// Inputs: bubbly set at `p = 7 MPa`, `alpha_g = 0.1`, `d = 1 mm`, zero slip,
/// vapour saturated, liquid subcooled by `h_l = h_f - 5e4 J/kg` so that
/// `Q_l = K_l (T_sat - T_l)` is non-zero and its magnitude is sensitive to
/// which `lambda` was used. Tolerance: relative `1e-12` on both fluxes. Pass
/// criterion: both closed forms reproduced, **and** the wrong-conductivity
/// alternatives rejected by a wide margin (the ratio `lambda_f/lambda_g =
/// 9.210375` is the margin).
///
/// **Results (2026-08-12).**
///
/// ```text
/// T_sat = 558.980 K, T_l = 549.537 K (subcooling 9.443 K), T_g = 558.980 K
/// Q_l  measured  = 6.622413e6 W/m^3   closed form 12 a lambda_f / d^2 * dT = 6.622413e6
/// Q_g  measured  = 0.000000e0 W/m^3   closed form 60 a lambda_g / d^2 * dT = 0.000000e0
/// swapping lambda in the liquid resistance would give 7.190167e5 W/m^3 (9.21x low)
/// ```
///
/// The liquid resistance is therefore demonstrably reading `lambda_f` and not
/// `lambda_g`: the swapped value differs by the full conductivity ratio and the
/// measured value sits on the correct one to round-off.
#[test]
fn two_resistance_pair_reads_one_conductivity_from_each_side() {
    let (sat, transport) = reference_properties();
    let exchange = bubbly();
    let mut cell = saturated_cell(sat);
    cell.liquid_enthalpy = sat.h_f - 5.0e4;

    let sources = exchange
        .sources_with_properties(cell, sat, transport)
        .expect("interfacial sources with a subcooled liquid");

    let d2 = D_REF * D_REF;
    let dt_liquid = sat.t_sat - sources.liquid_temperature;
    let dt_vapour = sat.t_sat - sources.vapour_temperature;
    let q_liquid_expected = 12.0 * ALPHA_REF * transport.lambda_f / d2 * dt_liquid;
    let q_vapour_expected = 60.0 * ALPHA_REF * transport.lambda_g / d2 * dt_vapour;
    let q_liquid_swapped = 12.0 * ALPHA_REF * transport.lambda_g / d2 * dt_liquid;

    println!(
        "T_sat = {:.3} K, T_l = {:.3} K (subcooling {:.3} K), T_g = {:.3} K",
        sat.t_sat, sources.liquid_temperature, dt_liquid, sources.vapour_temperature
    );
    println!(
        "Q_l  measured  = {:.6e} W/m^3   closed form 12 a lambda_f / d^2 * dT = {q_liquid_expected:.6e}",
        sources.liquid_heat
    );
    println!(
        "Q_g  measured  = {:.6e} W/m^3   closed form 60 a lambda_g / d^2 * dT = {q_vapour_expected:.6e}",
        sources.vapour_heat
    );
    println!(
        "swapping lambda in the liquid resistance would give {q_liquid_swapped:.6e} W/m^3 ({:.2}x low)",
        q_liquid_expected / q_liquid_swapped
    );

    assert!(
        (sources.liquid_heat - q_liquid_expected).abs() <= 1.0e-12 * q_liquid_expected.abs(),
        "liquid-side resistance is not reading lambda_f"
    );
    assert!(
        (sources.vapour_heat - q_vapour_expected).abs()
            <= 1.0e-12 * q_vapour_expected.abs().max(1.0),
        "vapour-side resistance is not reading lambda_g"
    );
    // The swapped alternative must be far away, or the test proves nothing.
    assert!((q_liquid_expected / q_liquid_swapped) > 5.0);
}

/// **Methodology.** The same check with the phase roles exchanged. In droplet
/// flow the vapour is continuous and the liquid dispersed, so the two
/// conductivities must swap sides: Ranz-Marshall now closes the **vapour**
/// resistance with `lambda_g`, and spherical conduction closes the **liquid**
/// resistance with `lambda_f`. If the module had hard-coded "liquid means
/// continuous" anywhere, this case would fail while the bubbly one passed.
///
/// Inputs: droplet set at `p = 7 MPa`, `alpha_g = 0.9` (so `alpha_d = alpha_l =
/// 0.1`, matching the bubbly case's dispersed fraction and making the two
/// closed forms directly comparable), `d = 1 mm`, zero slip, liquid saturated,
/// vapour superheated by `h_g = h_g^sat + 5e4 J/kg`. Tolerance: relative
/// `1e-12`. Pass criterion: `Q_g = 12 alpha_l lambda_g / d^2 * (T_sat - T_g)`
/// and `Q_l = 60 alpha_l lambda_f / d^2 * (T_sat - T_l)`.
///
/// **Results (2026-08-12).**
///
/// ```text
/// alpha_dispersed = 0.100, T_g = 569.224 K (superheat 10.243 K), T_l = 558.980 K
/// Q_g measured = -7.800047e5 W/m^3   closed form 12 a lambda_g / d^2 * dT = -7.800047e5
/// Q_l measured = 0.000000e0 W/m^3   closed form 60 a lambda_f / d^2 * dT = 0.000000e0
/// ```
///
/// The roles genuinely follow [`DispersedPhase`]: the same Ranz-Marshall
/// variant read `lambda_f` in the bubbly case and `lambda_g` here.
#[test]
fn droplet_topology_exchanges_which_conductivity_closes_which_side() {
    let (sat, transport) = reference_properties();
    let exchange =
        InterfacialExchange::droplet(Length::new::<millimeter>(1.0)).expect("droplet closure set");
    let mut cell = saturated_cell(sat);
    cell.void_fraction = 0.9;
    cell.vapour_enthalpy = sat.h_g + 5.0e4;

    let sources = exchange
        .sources_with_properties(cell, sat, transport)
        .expect("interfacial sources in droplet flow");

    let alpha_dispersed = 1.0 - cell.void_fraction;
    let d2 = D_REF * D_REF;
    let dt_vapour = sat.t_sat - sources.vapour_temperature;
    let dt_liquid = sat.t_sat - sources.liquid_temperature;
    let q_vapour_expected = 12.0 * alpha_dispersed * transport.lambda_g / d2 * dt_vapour;
    let q_liquid_expected = 60.0 * alpha_dispersed * transport.lambda_f / d2 * dt_liquid;

    println!(
        "alpha_dispersed = {alpha_dispersed:.3}, T_g = {:.3} K (superheat {:.3} K), T_l = {:.3} K",
        sources.vapour_temperature, -dt_vapour, sources.liquid_temperature
    );
    println!(
        "Q_g measured = {:.6e} W/m^3   closed form 12 a lambda_g / d^2 * dT = {q_vapour_expected:.6e}",
        sources.vapour_heat
    );
    println!(
        "Q_l measured = {:.6e} W/m^3   closed form 60 a lambda_f / d^2 * dT = {q_liquid_expected:.6e}",
        sources.liquid_heat
    );

    assert!(
        (sources.vapour_heat - q_vapour_expected).abs() <= 1.0e-12 * q_vapour_expected.abs(),
        "vapour-side resistance is not reading lambda_g in droplet flow"
    );
    assert!(
        (sources.liquid_heat - q_liquid_expected).abs()
            <= 1.0e-12 * q_liquid_expected.abs().max(1.0),
        "liquid-side resistance is not reading lambda_f in droplet flow"
    );
}

/// **Methodology.** With both phases exactly on the saturation line, both
/// driving temperature differences `T_sat - T_k` vanish, so both interfacial
/// heat fluxes vanish and the energy jump gives `Gamma = 0` — a cell in
/// thermal equilibrium neither evaporates nor condenses, whatever the drag is
/// doing. Inputs: bubbly set at `p = 7 MPa`, `alpha_g = 0.1`, `d = 1 mm`,
/// `h_g = h_g^sat`, `h_l = h_f^sat`, zero slip. Reference: the closure's own
/// definition. Tolerance: `Gamma` is not exactly zero because the phase
/// temperatures come from a numerical inversion of the IF97 enthalpy, so the
/// criterion is that the residual is negligible against the scale a real
/// transient produces — the superheated case below gives `|Gamma| = 2.591150e0
/// kg/(m^3 s)`, so `1e-6 kg/(m^3 s)` is six orders of magnitude down on that.
///
/// **Results (2026-08-12).** `T_g - T_sat = 0.000000e0 K`,
/// `T_l - T_sat = 0.000000e0 K`, `Q_g = 0.000000e0 W/m^3`,
/// `Q_l = 0.000000e0 W/m^3`, `Gamma = -0.000000e0 kg/(m^3 s)` (a negative
/// zero) — the enthalpy inversion lands exactly on `T_sat` at both ends here,
/// so the equilibrium identity holds to the last bit rather than merely within
/// the stated tolerance.
#[test]
fn mass_transfer_vanishes_when_both_phases_sit_on_the_saturation_line() {
    let (sat, transport) = reference_properties();
    let exchange = bubbly();
    let sources = exchange
        .sources_with_properties(saturated_cell(sat), sat, transport)
        .expect("interfacial sources at equilibrium");

    println!(
        "T_g - T_sat = {:.6e} K, T_l - T_sat = {:.6e} K",
        sources.vapour_temperature - sat.t_sat,
        sources.liquid_temperature - sat.t_sat
    );
    println!(
        "Q_g = {:.6e} W/m^3, Q_l = {:.6e} W/m^3, Gamma = {:.6e} kg/(m^3 s)",
        sources.vapour_heat, sources.liquid_heat, sources.mass_transfer
    );

    assert!(sources.mass_transfer.abs() < 1.0e-6);
    assert!(!sources.is_evaporating());
}

/// **Methodology — the sign test.** With the interface pinned at `T_sat`, a
/// superheated vapour beside saturated liquid delivers heat *to* the
/// interface, which can only go into latent heat, so the cell **evaporates**
/// (`Gamma > 0`). Swapping the perturbation — saturated vapour beside
/// subcooled liquid — makes the interface deliver heat *into* the liquid,
/// which it can only supply by condensing vapour, so `Gamma < 0`. Getting this
/// backwards would make a blowdown condense where it should flash, so the sign
/// is pinned in both directions and against the reported heat fluxes.
///
/// Inputs: bubbly set at `p = 7 MPa`, `alpha_g = 0.1`, `d = 1 mm`, zero slip;
/// case A `h_g = h_g^sat + 5e4 J/kg` with saturated liquid, case B
/// `h_l = h_f^sat - 5e4 J/kg` with saturated vapour. Pass criterion:
/// `Gamma_A > 0`, `Gamma_B < 0`, and in each case `Gamma h_fg` equals the net
/// heat delivered to the interface to within a relative `1e-12`.
///
/// **Results (2026-08-12).**
///
/// ```text
/// A superheated vapour: T_g - T_sat = +10.243 K, Q_g = -3.900023e6 W/m^3,
///                       Q_l = 0.000000e0 W/m^3, Gamma = +2.591150e0 kg/(m^3 s)
/// B subcooled liquid:   T_l - T_sat = -9.443 K, Q_g = 0.000000e0 W/m^3,
///                       Q_l = 6.622413e6 W/m^3, Gamma = -4.399889e0 kg/(m^3 s)
/// ```
///
/// The sign flips as required. Note the two magnitudes differ by a factor of
/// about 1.7 for a comparable temperature departure: the liquid resistance is
/// Ranz-Marshall on `lambda_f` (`Nu = 2`, factor 12) while the vapour
/// resistance is spherical conduction on `lambda_g` (`Nu = 10`, factor 60), and
/// the ten-fold conductivity advantage of the liquid is largely offset by the
/// five-fold Nusselt advantage of the vapour. That the two do *not* cancel is
/// the whole reason both resistances have to be carried.
#[test]
fn mass_transfer_sign_flips_between_superheated_vapour_and_subcooled_liquid() {
    let (sat, transport) = reference_properties();
    let exchange = bubbly();

    let mut superheated = saturated_cell(sat);
    superheated.vapour_enthalpy = sat.h_g + 5.0e4;
    let a = exchange
        .sources_with_properties(superheated, sat, transport)
        .expect("superheated-vapour sources");

    let mut subcooled = saturated_cell(sat);
    subcooled.liquid_enthalpy = sat.h_f - 5.0e4;
    let b = exchange
        .sources_with_properties(subcooled, sat, transport)
        .expect("subcooled-liquid sources");

    println!(
        "A superheated vapour: T_g - T_sat = {:+.3} K, Q_g = {:.6e} W/m^3, \
         Q_l = {:.6e} W/m^3, Gamma = {:+.6e} kg/(m^3 s)",
        a.vapour_temperature - sat.t_sat,
        a.vapour_heat,
        a.liquid_heat,
        a.mass_transfer
    );
    println!(
        "B subcooled liquid:   T_l - T_sat = {:+.3} K, Q_g = {:.6e} W/m^3, \
         Q_l = {:.6e} W/m^3, Gamma = {:+.6e} kg/(m^3 s)",
        b.liquid_temperature - sat.t_sat,
        b.vapour_heat,
        b.liquid_heat,
        b.mass_transfer
    );

    assert!(a.mass_transfer > 0.0, "superheated vapour must evaporate");
    assert!(a.is_evaporating());
    assert!(b.mass_transfer < 0.0, "subcooled liquid must condense");
    assert!(!b.is_evaporating());

    for sources in [a, b] {
        let closed = sources.mass_transfer * sat.h_fg();
        let residual = (closed - sources.net_heat_to_interface()).abs()
            / sources.net_heat_to_interface().abs();
        assert!(
            residual < 1.0e-12,
            "Gamma h_fg must equal the net heat delivered to the interface"
        );
    }
}

/// **Methodology — the critical-point degeneracy, and why the obvious guard
/// does not work.**
///
/// `Gamma = -(Q_g + Q_l)/h_fg` divides by a latent heat that physically
/// vanishes at `p_c = 22.064 MPa`, and [`SaturatedProperties::at`] accepts
/// pressures to 100 MPa, so an unguarded closure would return a finite `Gamma`
/// from a fictitious saturation line. The natural guard is a floor on `h_fg`.
/// This case measures whether that floor would ever actually fire.
///
/// Inputs: `p in {20, 22, 22.06} MPa` (subcritical, both phases on the
/// saturation line) and `p in {22.064, 25, 50} MPa` (at and above `p_c`);
/// bubbly set, `d = 1 mm`, `alpha_g = 0.1`. Reference: the physical
/// requirement `h_fg -> 0` as `p -> p_c`. Pass criterion: the subcritical
/// pressures evaluate; every pressure at or above `p_c` is refused with
/// [`TampinesError::Unphysical`]; and the reported `h_fg` at `p_c` is recorded
/// whatever it turns out to be.
///
/// **Results (2026-08-12) — and the finding.**
///
/// ```text
/// p = 20.000 MPa   h_fg = 6.0058e5 J/kg   -> ok
/// p = 22.000 MPa   h_fg = 3.8700e5 J/kg   -> ok
/// p = 22.060 MPa   h_fg = 3.7988e5 J/kg   -> ok
/// p = 22.064 MPa   h_fg = 3.7940e5 J/kg   -> refused by the closure (p >= p_c)
/// p = 25.000 MPa   -> refused by the property layer before the closure is reached
/// p = 50.000 MPa   -> refused by the property layer before the closure is reached
/// ```
///
/// Above `p_c` the property layer refuses first (its IF97 saturation inversion
/// fails), so the closure is never reached; exactly *at* `p_c` it does not, and
/// that single pressure is where this module's own guard is what stops a
/// number being returned.
///
/// **`h_fg` does not collapse.** At exactly the critical pressure this
/// property layer reports `3.7940e5 J/kg` where the physical value is zero,
/// because [`SaturatedProperties::at`] evaluates `h_f` and `h_g` from IF97
/// Regions 1 and 2, whose validity ends at `T = 623.15 K`, while
/// `T_sat(p_c) ~ 647 K`; the critical collapse lives in Region 3, which that
/// layer does not implement. A guard written as "refuse when
/// `h_fg < 1e4 J/kg`" would therefore **never have fired for water** — it would
/// have read as a safety check while being unreachable.
///
/// That is why [`P_CRITICAL_WATER`] is the operative guard and
/// [`MIN_LATENT_HEAT_FOR_MASS_TRANSFER`] is only a backstop, and why this test
/// asserts the *pressure* refusal rather than the latent-heat one. It also
/// records a limitation of the property layer that this module cannot fix from
/// here: near-critical saturation enthalpies are outside the IF97 regions used
/// to compute them, so the numbers returned just below `p_c` are extrapolations
/// and nothing here has checked them against anything.
#[test]
fn latent_heat_guard_fires_only_next_to_the_critical_point() {
    let exchange = bubbly();

    for p in [20.0e6_f64, 22.0e6, 22.06e6] {
        let sat = SaturatedProperties::at(p).expect("saturation set");
        let transport = SaturatedTransport::at(p).expect("conduction set");
        let cell = InterfacialCellState {
            pressure: p,
            void_fraction: ALPHA_REF,
            vapour_enthalpy: sat.h_g,
            liquid_enthalpy: sat.h_f,
            vapour_velocity: 0.0,
            liquid_velocity: 0.0,
        };
        let outcome = exchange.sources_with_properties(cell, sat, transport);
        println!(
            "p = {:.3} MPa   h_fg = {:.4e} J/kg   -> {}",
            p / 1.0e6,
            sat.h_fg(),
            if outcome.is_ok() { "ok" } else { "refused" }
        );
        assert!(
            outcome.is_ok(),
            "subcritical pressure {p} Pa should still evaluate"
        );
        // The finding this test records: the latent heat does NOT approach zero
        // on this approach, so a floor on h_fg alone would be unreachable.
        assert!(sat.h_fg() > MIN_LATENT_HEAT_FOR_MASS_TRANSFER * 10.0);
    }

    for p in [P_CRITICAL_WATER, 25.0e6, 50.0e6] {
        // The property layer may itself refuse a supercritical pressure; if it
        // does, that is a refusal too and the closure is never reached. Either
        // way nothing is allowed to return a number.
        match (SaturatedProperties::at(p), SaturatedTransport::at(p)) {
            (Ok(sat), Ok(transport)) => {
                let cell = InterfacialCellState {
                    pressure: p,
                    void_fraction: ALPHA_REF,
                    vapour_enthalpy: sat.h_g,
                    liquid_enthalpy: sat.h_f,
                    vapour_velocity: 0.0,
                    liquid_velocity: 0.0,
                };
                match exchange.sources_with_properties(cell, sat, transport) {
                    Err(TampinesError::Unphysical(message)) => println!(
                        "p = {:.3} MPa   h_fg = {:.4e} J/kg   -> refused by the closure: \
                         {message}",
                        p / 1.0e6,
                        sat.h_fg()
                    ),
                    other => panic!(
                        "p = {p} Pa is at or above the critical pressure and must be \
                         refused, got ok = {}",
                        other.is_ok()
                    ),
                }
            }
            _ => println!(
                "p = {:.3} MPa   -> refused by the property layer before the closure is \
                 reached",
                p / 1.0e6
            ),
        }
    }
}

/// **Methodology.** A void fraction outside `[0, 1]` and a non-finite state
/// entry are usage errors that must be reported rather than clipped into
/// range, per the crate's no-silent-clamp rule: a clipped `alpha` would let a
/// CFL violation upstream present itself as a plausible source term. Inputs:
/// bubbly set at `p = 7 MPa` with `alpha_g in {-1e-9, 1.5}` and with a
/// non-finite velocity. Pass criterion: [`TampinesError::Unphysical`] for the
/// void fractions, [`TampinesError::InvalidInput`] for the non-finite entry,
/// and `alpha_g` exactly `0.0` and `1.0` still accepted (they are physical).
///
/// **Results (2026-08-12).** All four refusals observed with the expected
/// variants; both endpoints accepted. At `alpha_g = 0` the heat-transfer
/// closures still return a non-zero coefficient because upstream applies the
/// `max(alpha_d, alpha_res)` floor with `alpha_res = 1e-6`; that is upstream's
/// documented convention, not a clamp introduced here, and a caller wanting a
/// hard zero can pass `residual_alpha = 0.0` to
/// [`InterfacialExchange::new`].
#[test]
fn out_of_range_and_non_finite_states_are_refused_not_clipped() {
    let (sat, transport) = reference_properties();
    let exchange = bubbly();

    for alpha in [-1.0e-9_f64, 1.5] {
        let mut cell = saturated_cell(sat);
        cell.void_fraction = alpha;
        assert!(
            matches!(
                exchange.sources_with_properties(cell, sat, transport),
                Err(TampinesError::Unphysical(_))
            ),
            "void fraction {alpha} should have been refused"
        );
    }

    let mut cell = saturated_cell(sat);
    cell.vapour_velocity = f64::NAN;
    assert!(matches!(
        exchange.sources_with_properties(cell, sat, transport),
        Err(TampinesError::InvalidInput(_))
    ));
    cell.vapour_velocity = f64::INFINITY;
    assert!(matches!(
        exchange.sources_with_properties(cell, sat, transport),
        Err(TampinesError::InvalidInput(_))
    ));

    for alpha in [0.0_f64, 1.0] {
        let mut cell = saturated_cell(sat);
        cell.void_fraction = alpha;
        assert!(
            exchange
                .sources_with_properties(cell, sat, transport)
                .is_ok(),
            "void fraction {alpha} is physical and should be accepted"
        );
    }
}

/// **Methodology.** A property set cached at a distant pressure must be
/// refused rather than silently used, because `T_sat` sets the driving
/// temperature difference of every heat-transfer term and a stale `T_sat` is
/// wrong in the one quantity the closure is most sensitive to. Inputs: bubbly
/// set, cell at `p = 7 MPa`, conduction set evaluated at `7.5 MPa` (a 7.1 %
/// departure, seven times
/// [`super::super::properties::TRANSPORT_CACHE_TOLERANCE`]). Pass criterion:
/// [`TampinesError::Unphysical`] naming both pressures.
///
/// **Results (2026-08-12).** Refused, with the message
/// `conduction property set is at 7500000 Pa but the cell is at 7000000 Pa,
/// beyond the transport cache tolerance; ...`.
#[test]
fn a_stale_conduction_set_is_refused() {
    let (sat, _) = reference_properties();
    let stale = SaturatedTransport::at(7.5e6).expect("conduction set at 7.5 MPa");
    let exchange = bubbly();
    match exchange.sources_with_properties(saturated_cell(sat), sat, stale) {
        Err(TampinesError::Unphysical(message)) => println!("{message}"),
        other => panic!(
            "a stale conduction set must be refused, got ok = {}",
            other.is_ok()
        ),
    }
}

/// **Methodology.** The `uom`-typed entry point must be a pure re-expression of
/// the raw-`f64` one: same numbers, dimensions checked at the boundary. Inputs:
/// the reference cell at `p = 7 MPa` built twice, once field-by-field in raw SI
/// and once through [`InterfacialCellState::new`] with `uom` quantities. Pass
/// criterion: the two states compare equal and produce identical sources, and
/// the `uom` accessors on [`InterfacialSources`] round-trip to the raw fields
/// exactly.
///
/// **Results (2026-08-12).** Both states equal, both source sets equal, and
/// `vapour_heat_density()`, `liquid_heat_density()` and
/// `saturation_temperature()` round-trip to the raw fields with zero
/// difference.
#[test]
fn the_uom_boundary_agrees_with_the_raw_si_boundary() {
    use uom::si::available_energy::joule_per_kilogram as jpkg;
    use uom::si::pressure::pascal as pa;
    use uom::si::velocity::meter_per_second as mps;

    let (sat, transport) = reference_properties();
    let raw = saturated_cell(sat);
    let typed = InterfacialCellState::new(
        Pressure::new::<pa>(raw.pressure),
        raw.void_fraction,
        AvailableEnergy::new::<jpkg>(raw.vapour_enthalpy),
        AvailableEnergy::new::<jpkg>(raw.liquid_enthalpy),
        Velocity::new::<mps>(raw.vapour_velocity),
        Velocity::new::<mps>(raw.liquid_velocity),
    );
    assert_eq!(raw, typed);

    let exchange = bubbly();
    let from_raw = exchange
        .sources_with_properties(raw, sat, transport)
        .expect("sources from the raw state");
    let from_typed = exchange
        .sources_with_properties(typed, sat, transport)
        .expect("sources from the uom state");
    assert_eq!(from_raw, from_typed);

    assert_eq!(
        from_raw.vapour_heat_density().get::<watt_per_cubic_meter>(),
        from_raw.vapour_heat
    );
    assert_eq!(
        from_raw.liquid_heat_density().get::<watt_per_cubic_meter>(),
        from_raw.liquid_heat
    );
    assert_eq!(
        from_raw.saturation_temperature().get::<kelvin>(),
        from_raw.interface_temperature
    );
    assert_eq!(exchange.diameter().get::<meter>(), D_REF);
    assert_eq!(exchange.residual_alpha(), RESIDUAL_ALPHA);
    assert!(exchange.dispersed_phase().is_vapour_dispersed());
    assert_eq!(
        exchange.continuous_side_model().resistance_side(),
        ResistanceSide::Continuous
    );
    assert_eq!(
        exchange.dispersed_side_model().resistance_side(),
        ResistanceSide::Dispersed
    );
}

/// **Methodology.** [`InterfacialExchange::sources_si`] evaluates the property
/// sets itself; it must agree bit-for-bit with
/// [`InterfacialExchange::sources_with_properties`] handed freshly evaluated
/// sets at the same pressure, or the convenience path is a second, divergent
/// property route — exactly what the module forbids. Inputs: the reference cell
/// at `p = 7 MPa` with a subcooled liquid and finite slip, so every term is
/// non-zero. Pass criterion: equality of all nine reported quantities.
///
/// **Results (2026-08-12).** Identical source sets from both entry points.
#[test]
fn the_self_evaluating_entry_point_matches_the_cached_one() {
    let (sat, transport) = reference_properties();
    let exchange = bubbly();
    let mut cell = saturated_cell(sat);
    cell.liquid_enthalpy = sat.h_f - 2.0e4;
    cell.vapour_velocity = 1.0;

    let cached = exchange
        .sources_with_properties(cell, sat, transport)
        .expect("cached-property sources");
    let self_evaluated = exchange.sources_si(cell).expect("self-evaluating sources");
    assert_eq!(cached, self_evaluated);
}

/// **Methodology.** The constructor's remaining input guards. Inputs: a
/// zero, a negative and a non-finite diameter; a negative, a unity and a
/// non-finite `residual_alpha`. Pass criterion: every one returns
/// [`TampinesError::InvalidInput`]; `residual_alpha = 0.0` is accepted, since a
/// caller may legitimately want no residual floor.
///
/// **Results (2026-08-12).** All six refusals observed; `residual_alpha = 0.0`
/// accepted.
#[test]
fn constructor_refuses_unusable_geometry_and_floors() {
    for d in [0.0_f64, -1.0e-3, f64::NAN] {
        assert!(matches!(
            InterfacialExchange::bubbly(Length::new::<meter>(d)),
            Err(TampinesError::InvalidInput(_))
        ));
    }
    for residual in [-1.0e-9_f64, 1.0, f64::NAN] {
        assert!(matches!(
            InterfacialExchange::new(
                DragModel::SchillerNaumann,
                InterfacialHeatTransfer::RanzMarshall,
                InterfacialHeatTransfer::Spherical,
                DispersedPhase::Vapour,
                Length::new::<millimeter>(1.0),
                residual,
            ),
            Err(TampinesError::InvalidInput(_))
        ));
    }
    assert!(InterfacialExchange::new(
        DragModel::SchillerNaumann,
        InterfacialHeatTransfer::RanzMarshall,
        InterfacialHeatTransfer::Spherical,
        DispersedPhase::Vapour,
        Length::new::<millimeter>(1.0),
        0.0,
    )
    .is_ok());
}
