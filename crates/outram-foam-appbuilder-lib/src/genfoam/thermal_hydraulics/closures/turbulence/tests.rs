// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam), commit 652b3da, EPFL,
// GPL-3.0. See the sub-module headers for provenance.
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0.

//! # Verification & validation — two-phase / porous turbulence closures
//!
//! **Scope.** Each closure function is checked against (a) an analytical
//! limit where the physics is unambiguous (a term must vanish or reduce to a
//! known simpler case), (b) an exact algebraic round-trip or independent
//! recomputation of the upstream formula (port correctness), and — where an
//! external reference exists — (c) cross-validation against a published
//! correlation independent of GeN-Foam.
//!
//! ## Methodology
//!
//! 1. **Lahey bubble-induced eddy viscosity / production — zero-void-fraction
//!    limit (analytical).** `nu_t,bi = Cmub*Dh*alpha_g*|Ur|` and `bubbleG =
//!    Cp*(1+Cd^(4/3))*alpha_g*|Ur|^3/Dh` are both linear in `alpha_g`; at
//!    `alpha_g = 0` (no dispersed gas) both must vanish exactly — a
//!    single-phase flow has no bubble-induced turbulence source. Pass
//!    criterion: exactly `0.0`.
//! 2. **`drag_coefficient_from_kd` — exact round-trip (verification).** Given
//!    a target `Cd_0`, compute the corresponding `Kd` from the *forward*
//!    relation `Kd = 0.5*(alpha_c*alpha_d/(alpha_c+alpha_d))*(rho_c/Dh_d)*Ur*Cd_0`
//!    (the definition documented on both upstream classes), then invert with
//!    [`super::drag_coefficient_from_kd`] and check `Cd` is recovered exactly.
//!    Also checks the degenerate `alpha_c = alpha_d = 0` case returns a finite
//!    value (the documented deliberate deviation from upstream's `NaN`).
//!    Reference: the upstream `Kd`/`Cd` relation itself (self-consistency).
//!    Pass criterion: relative error `< 1e-10`.
//! 3. **Mixture dispersion coefficient `Ct2` — two analytical limits.**
//!    (a) At `alpha_g = 0`, `fAlphad = 0` so `Ct2 = Ct0^2` exactly (the
//!    `exp(-0) = 1` collapse). (b) As `beta -> infinity` (strong drag /
//!    tiny bubbles), `Ct0 -> 1` (the `(3+beta)/(1+beta+X)` ratio tends to 1
//!    for any finite `X` as `beta` dominates), so `Ct2 -> 1` — the gas phase's
//!    turbulence fully tracks the liquid's (both evaluated at `alpha_g = 0` so
//!    `Ct2 = Ct0^2` isolates the `Ct0 -> 1` limit cleanly — see the in-test
//!    comment for why nonzero `alpha_g` would instead be dominated by
//!    `exp(-fAlphad)`). Reference: the algebraic limit of the upstream
//!    formula. Pass criteria: (a) exact algebraic match; (b) `Ct2` within
//!    `1e-6` of `1.0` at `Kd = 1e12` kg/(m^3 s).
//! 4. **Mixture density / mixing — degenerate single-phase limit
//!    (analytical).** At `alpha_liquid = 1`, `alpha_gas = 0`, `mixture_density
//!    = rho_liquid_eff` and `mix_k`/`mix_epsilon` reduce exactly to the liquid
//!    value (no gas phase present). Pass criterion: exact match (`< 1e-10`
//!    relative).
//! 5. **Porous `equilibrium_k`/`equilibrium_epsilon` — cross-validation
//!    against OpenFOAM's own standard inlet-condition correlations
//!    (external reference, independent of GeN-Foam).** OpenFOAM's
//!    `turbulentIntensityKineticEnergyInlet` uses the identical `k =
//!    1.5*(I*U)^2` relation, and its companion
//!    `turbulentMixingLengthDissipationRateInlet` uses the identical `epsilon
//!    = Cmu^0.75 * k^1.5 / L` relation — both standard CFD boundary-condition
//!    correlations, not GeN-Foam-specific. With the textbook fully-developed
//!    pipe-flow turbulence-intensity fit `I = 0.16*Re^(-1/8)` (`A = 0.16`,
//!    `B = -1/8`) at `Re = 1e4`, `U = 1` m/s, `Cmu = 0.09`, `L = 0.07*Dh`
//!    (`Dh = 0.02` m): reference computed independently in the test from the
//!    same published formulas. Pass criterion: exact reproduction (`< 1e-10`
//!    relative) — the point is that the port's formula *is* the published
//!    one, so this is simultaneously a verification and a validation check.
//! 6. **Porous 2-phase-corrected `equilibrium_k` — reduces to the 1-phase
//!    correlation at `turbulence_intensity_alpha_coeff = 0`.** Pass
//!    criterion: exact match against [`PorousKEpsilonClosure::equilibrium_k`].
//! 7. **`nut_stabilization` — infinite-`laminarReStruct` limit.** As
//!    `laminarReStruct -> infinity`, the stabilisation addend must vanish (no
//!    structure-scale instability to damp). Pass criterion: `< 1e-6` at
//!    `laminarReStruct = 1e9`.
//!
//! ## Results (measured 2026-07-15, `cargo test --release`, data: source
//! formulae + the two cited OpenFOAM standard correlations)
//!
//! - Lahey zero-void-fraction limit: both `nu_t,bi` and `bubbleG` measured
//!   exactly `0.0` at `alpha_g = 0` — PASS.
//! - `drag_coefficient_from_kd` round-trip: recovered `Cd` to `< 1e-12`
//!   relative across the tested `Cd_0 in {0.1, 0.44, 2.0}`; degenerate
//!   `alpha_c = alpha_d = 0` case returned the finite floor value (no `NaN`)
//!   — PASS.
//! - Mixture `Ct2`: `alpha_g = 0` case matched `Ct0^2` to machine precision;
//!   at `Kd = 1e12` kg/(m^3 s) (`beta = 2.876e8`), measured `Ct0 =
//!   1.0000000069` (`6.9e-9` from the `1.0` limit) and `Ct2 = 1.0000000139`
//!   — PASS.
//! - Mixture single-phase-limit: `mixture_density`, `mix_k`, `mix_epsilon` at
//!   `alpha_liquid = 1` reproduced the liquid-only values to `< 1e-12`
//!   relative — PASS.
//! - Porous `equilibrium_k`/`equilibrium_epsilon` vs. the OpenFOAM standard
//!   correlations: exact reproduction (`< 1e-12` relative) — PASS by
//!   construction (same formula), confirming the port did not transcribe the
//!   published correlation incorrectly.
//! - 2-phase equilibrium_k reduction: exact match at `D = 0` — PASS.
//! - `nut_stabilization` large-Re limit: measured `0.05` m^2/s at
//!   `laminarReStruct = 1.0` vs. `5.0e-11` m^2/s at `laminarReStruct = 1e9`
//!   (`U = 0.5` m/s, `DhStruct = 0.1` m) — PASS.
//!
//! Interpretation: the ported algebra reproduces its upstream source
//! line-for-line (verification) and, for the porous equilibrium correlations,
//! matches long-established OpenFOAM/CFD community correlations independent
//! of GeN-Foam (validation). The Lahey/mixture bubble-induced terms have no
//! external published reference beyond Lahey (2005) itself and are therefore
//! verified against their own analytical limits and round-trips only; full
//! validation against experimental bubble-column data is deferred to the
//! solver-integration bead (op-p6p.7.11+), once field-level assembly exists to
//! run an actual case.

use super::lahey_k_epsilon::LaheyBubbleClosure;
use super::mixture_k_epsilon::MixtureKEpsilonClosure;
use super::porous_k_epsilon::{PorousKEpsilon2PhaseClosure, PorousKEpsilonClosure};
use super::units::{
    volumetric_relaxation_coefficient, HydraulicDiameter, RelativeVelocity,
    TurbulentDissipationRate, TurbulentKineticEnergy, VoidFraction,
};
use crate::genfoam::thermal_hydraulics::units::reynolds_number;
use approx::assert_relative_eq;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{MassDensity, Ratio, Velocity};
use uom::si::kinematic_viscosity::square_meter_per_second;
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::ratio::ratio;
use uom::si::specific_power::watt_per_kilogram;
use uom::si::velocity::meter_per_second;

// ---------------------------------------------------------------------------
// 1. Lahey: zero-void-fraction limit
// ---------------------------------------------------------------------------

#[test]
fn lahey_bubble_terms_vanish_at_zero_void_fraction() {
    let model = LaheyBubbleClosure::new_default();
    let dh = HydraulicDiameter::new::<meter>(3.0e-3);
    let ur = RelativeVelocity::new::<meter_per_second>(1.5);
    let zero_alpha = VoidFraction::new::<ratio>(0.0);

    let nut_bi = model.bubble_induced_eddy_viscosity(dh, zero_alpha, ur);
    assert_eq!(nut_bi.get::<square_meter_per_second>(), 0.0);

    let cd = Ratio::new::<ratio>(0.44);
    let bubble_g = model.bubble_induced_production(cd, zero_alpha, ur, dh);
    assert_eq!(bubble_g.get::<watt_per_kilogram>(), 0.0);
}

/// Port correctness: `bubble_induced_eddy_viscosity` reproduces `Cmub*Dh*alpha*|Ur|`
/// exactly at a nonzero reference state.
#[test]
fn lahey_bubble_induced_eddy_viscosity_reference_value() {
    let model = LaheyBubbleClosure::new_default();
    let dh = HydraulicDiameter::new::<meter>(3.0e-3);
    let alpha_gas = VoidFraction::new::<ratio>(0.3);
    let ur = RelativeVelocity::new::<meter_per_second>(1.0);

    let nut_bi = model.bubble_induced_eddy_viscosity(dh, alpha_gas, ur);
    let expected = 0.6 * 3.0e-3 * 0.3 * 1.0;
    assert_relative_eq!(
        nut_bi.get::<square_meter_per_second>(),
        expected,
        max_relative = 1e-12
    );
}

// ---------------------------------------------------------------------------
// 2. drag_coefficient_from_kd: exact round-trip
// ---------------------------------------------------------------------------

#[test]
fn drag_coefficient_round_trips_through_kd() {
    let dh = HydraulicDiameter::new::<meter>(2.0e-3);
    let rho_c = MassDensity::new::<kilogram_per_cubic_meter>(958.0); // liquid water @ 100 C
    let alpha_c = VoidFraction::new::<ratio>(0.85);
    let alpha_d = VoidFraction::new::<ratio>(0.15);
    let ur = RelativeVelocity::new::<meter_per_second>(0.8);

    for &cd0 in &[0.1_f64, 0.44, 2.0] {
        // Forward relation (upstream doc comment on both LaheyKEpsilon and
        // mixtureKEpsilon): Kd = 0.5*(ac*ad/(ac+ad))*(rho_c/Dh_d)*Ur*Cd.
        let ac = alpha_c.get::<ratio>();
        let ad = alpha_d.get::<ratio>();
        let kd_value = 0.5
            * (ac * ad / (ac + ad))
            * (rho_c.get::<kilogram_per_cubic_meter>() / dh.get::<meter>())
            * ur.get::<meter_per_second>()
            * cd0;
        let kd = volumetric_relaxation_coefficient(kd_value);

        let cd = super::drag_coefficient_from_kd(kd, dh, rho_c, alpha_c, alpha_d, ur);
        assert_relative_eq!(cd.get::<ratio>(), cd0, max_relative = 1e-10);
    }
}

#[test]
fn drag_coefficient_degenerate_case_is_finite_not_nan() {
    let dh = HydraulicDiameter::new::<meter>(2.0e-3);
    let rho_c = MassDensity::new::<kilogram_per_cubic_meter>(958.0);
    let zero = VoidFraction::new::<ratio>(0.0);
    let ur = RelativeVelocity::new::<meter_per_second>(0.8);
    let kd = volumetric_relaxation_coefficient(1.0);

    let cd = super::drag_coefficient_from_kd(kd, dh, rho_c, zero, zero, ur);
    assert!(cd.get::<ratio>().is_finite());
    // At the regularisation floor (denom = 1e-3 m/s): Cd = 2*Kd*Dh/rho_c/1e-3.
    let expected = 2.0 * 1.0 * 2.0e-3 / 958.0 / 1e-3;
    assert_relative_eq!(cd.get::<ratio>(), expected, max_relative = 1e-10);
}

// ---------------------------------------------------------------------------
// 3. Mixture Ct2: analytical limits
// ---------------------------------------------------------------------------

#[test]
fn mixture_ct2_collapses_to_ct0_squared_at_zero_void_fraction() {
    let model = MixtureKEpsilonClosure::new_default();
    let kd = volumetric_relaxation_coefficient(50.0);
    let rho_l = MassDensity::new::<kilogram_per_cubic_meter>(958.0);
    let rho_g = MassDensity::new::<kilogram_per_cubic_meter>(0.6);
    let k_l = TurbulentKineticEnergy::new::<joule_per_kilogram>(0.05);
    let eps_l = TurbulentDissipationRate::new::<watt_per_kilogram>(0.02);
    let zero_alpha = VoidFraction::new::<ratio>(0.0);

    let ct2 = model.dispersion_coefficient(kd, rho_l, rho_g, k_l, eps_l, zero_alpha);

    // Independently recompute Ct0 to cross-check (fAlphad = 0 at alpha_g = 0).
    let beta = (6.0 * model.cmu / (4.0 * (1.5_f64).sqrt())) * (kd.value / 958.0) * (0.05 / 0.02);
    let ct0 = (3.0 + beta) / (1.0 + beta + 2.0 * 0.6 / 958.0);
    assert_relative_eq!(ct2.get::<ratio>(), ct0 * ct0, max_relative = 1e-12);
}

#[test]
fn mixture_ct0_tends_to_one_as_beta_grows() {
    let model = MixtureKEpsilonClosure::new_default();
    // Drive beta very large via a huge Kd. Use alpha_g = 0 so fAlphad = 0
    // exactly (Ct2 = Ct0^2 identically) and the Ct0 -> 1 limit is isolated
    // cleanly — at nonzero alpha_g, exp(-fAlphad) can itself be enormous
    // (fAlphad is a cubic in alpha_g that goes negative for small alpha_g),
    // which would swamp the tiny (Ct0-1) and defeat the point of this test.
    let kd = volumetric_relaxation_coefficient(1.0e12);
    let rho_l = MassDensity::new::<kilogram_per_cubic_meter>(958.0);
    let rho_g = MassDensity::new::<kilogram_per_cubic_meter>(0.6);
    let k_l = TurbulentKineticEnergy::new::<joule_per_kilogram>(0.05);
    let eps_l = TurbulentDissipationRate::new::<watt_per_kilogram>(0.02);
    let zero_alpha = VoidFraction::new::<ratio>(0.0);

    let ct2 = model.dispersion_coefficient(kd, rho_l, rho_g, k_l, eps_l, zero_alpha);
    assert_relative_eq!(ct2.get::<ratio>(), 1.0, max_relative = 1e-6);
}

// ---------------------------------------------------------------------------
// 4. Mixture density / mixing: single-phase limit
// ---------------------------------------------------------------------------

#[test]
fn mixture_quantities_reduce_to_liquid_only_at_alpha_liquid_one() {
    let alpha_liquid = VoidFraction::new::<ratio>(1.0);
    let alpha_gas = VoidFraction::new::<ratio>(0.0);
    let rho_l = MassDensity::new::<kilogram_per_cubic_meter>(958.0);
    let rho_g = MassDensity::new::<kilogram_per_cubic_meter>(0.6);
    let k_l = TurbulentKineticEnergy::new::<joule_per_kilogram>(0.05);
    let k_g = TurbulentKineticEnergy::new::<joule_per_kilogram>(0.9);
    let eps_l = TurbulentDissipationRate::new::<watt_per_kilogram>(0.02);
    let eps_g = TurbulentDissipationRate::new::<watt_per_kilogram>(0.5);

    let rhom = MixtureKEpsilonClosure::mixture_density(alpha_liquid, rho_l, alpha_gas, rho_g);
    assert_relative_eq!(
        rhom.get::<kilogram_per_cubic_meter>(),
        rho_l.get::<kilogram_per_cubic_meter>(),
        max_relative = 1e-12
    );

    let km = MixtureKEpsilonClosure::mix_k(alpha_liquid, rho_l, k_l, alpha_gas, rho_g, k_g);
    assert_relative_eq!(
        km.get::<joule_per_kilogram>(),
        k_l.get::<joule_per_kilogram>(),
        max_relative = 1e-12
    );

    let epsm =
        MixtureKEpsilonClosure::mix_epsilon(alpha_liquid, rho_l, eps_l, alpha_gas, rho_g, eps_g);
    assert_relative_eq!(
        epsm.get::<watt_per_kilogram>(),
        eps_l.get::<watt_per_kilogram>(),
        max_relative = 1e-12
    );
}

#[test]
fn mixture_effective_gas_density_reference_value() {
    let rho_g = MassDensity::new::<kilogram_per_cubic_meter>(0.6);
    let rho_l = MassDensity::new::<kilogram_per_cubic_meter>(958.0);
    let vm = Ratio::new::<ratio>(0.5);
    let rho_g_eff = MixtureKEpsilonClosure::effective_gas_density(rho_g, vm, rho_l);
    assert_relative_eq!(
        rho_g_eff.get::<kilogram_per_cubic_meter>(),
        0.6 + 0.5 * 958.0,
        max_relative = 1e-12
    );
}

// ---------------------------------------------------------------------------
// 5. Porous equilibrium correlations vs. OpenFOAM standard inlet conditions
// ---------------------------------------------------------------------------

/// Cross-validates against OpenFOAM's own `turbulentIntensityKineticEnergyInlet`
/// (`k = 1.5*(I*U)^2`) with the textbook fully-developed pipe-flow intensity
/// fit `I = 0.16*Re^(-1/8)` — both standard CFD correlations independent of
/// GeN-Foam.
#[test]
fn porous_equilibrium_k_matches_openfoam_intensity_correlation() {
    let model = PorousKEpsilonClosure {
        cmu: 0.09,
        turbulence_intensity_coeff: 0.16,
        turbulence_intensity_exp: -1.0 / 8.0,
        turbulence_length_scale_coeff: 0.07,
    };
    let speed = Velocity::new::<meter_per_second>(1.0);
    let re = reynolds_number(1.0e4);

    let k_eq = model.equilibrium_k(speed, re);

    let intensity = 0.16 * (1.0e4_f64).powf(-1.0 / 8.0);
    let expected_k = 1.5 * (1.0 * intensity).powi(2);
    assert_relative_eq!(
        k_eq.get::<joule_per_kilogram>(),
        expected_k,
        max_relative = 1e-12
    );
}

/// Cross-validates against OpenFOAM's own
/// `turbulentMixingLengthDissipationRateInlet` (`epsilon = Cmu^0.75 * k^1.5 /
/// L`).
#[test]
fn porous_equilibrium_epsilon_matches_openfoam_mixing_length_correlation() {
    let model = PorousKEpsilonClosure {
        cmu: 0.09,
        turbulence_intensity_coeff: 0.16,
        turbulence_intensity_exp: -1.0 / 8.0,
        turbulence_length_scale_coeff: 0.07,
    };
    let speed = Velocity::new::<meter_per_second>(1.0);
    let re = reynolds_number(1.0e4);
    let dh = HydraulicDiameter::new::<meter>(0.02);

    let k_eq = model.equilibrium_k(speed, re);
    let eps_eq = model.equilibrium_epsilon(k_eq, dh);

    let length_scale = 0.07 * 0.02;
    let expected_eps =
        0.09_f64.powf(0.75) * k_eq.get::<joule_per_kilogram>().powf(1.5) / length_scale;
    assert_relative_eq!(
        eps_eq.get::<watt_per_kilogram>(),
        expected_eps,
        max_relative = 1e-12
    );
}

// ---------------------------------------------------------------------------
// 6. Porous 2-phase-corrected equilibrium_k: reduces to 1-phase at D = 0
// ---------------------------------------------------------------------------

#[test]
fn porous_2phase_equilibrium_k_reduces_to_1phase_at_zero_alpha_coeff() {
    let base = PorousKEpsilonClosure {
        cmu: 0.09,
        turbulence_intensity_coeff: 0.16,
        turbulence_intensity_exp: -1.0 / 8.0,
        turbulence_length_scale_coeff: 0.07,
    };
    let two_phase = PorousKEpsilon2PhaseClosure {
        base,
        turbulence_intensity_alpha_coeff: 0.0,
    };
    let speed = Velocity::new::<meter_per_second>(1.0);
    let re = reynolds_number(1.0e4);
    let relative_alpha = VoidFraction::new::<ratio>(0.4); // multiplied by D = 0, must not matter

    let k_1phase = base.equilibrium_k(speed, re);
    let k_2phase = two_phase.equilibrium_k(speed, re, relative_alpha);

    assert_relative_eq!(
        k_2phase.get::<joule_per_kilogram>(),
        k_1phase.get::<joule_per_kilogram>(),
        max_relative = 1e-12
    );
}

// ---------------------------------------------------------------------------
// 7. nut_stabilization: large-Re limit
// ---------------------------------------------------------------------------

#[test]
fn nut_stabilization_vanishes_as_laminar_re_struct_grows() {
    let speed = Velocity::new::<meter_per_second>(0.5);
    let dh_struct = HydraulicDiameter::new::<meter>(0.1);

    let small_re = PorousKEpsilonClosure::nut_stabilization(speed, dh_struct, 1.0);
    let large_re = PorousKEpsilonClosure::nut_stabilization(speed, dh_struct, 1.0e9);

    assert_relative_eq!(
        small_re.get::<square_meter_per_second>(),
        0.5 * 0.1 / 1.0,
        max_relative = 1e-12
    );
    assert!(large_re.get::<square_meter_per_second>() < 1e-6);
}
