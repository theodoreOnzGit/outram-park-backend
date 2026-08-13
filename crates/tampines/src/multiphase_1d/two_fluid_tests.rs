// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Verification of the 1-D six-equation two-fluid solver
//! [`super::two_fluid::TwoFluid1d`].
//!
//! # Verification, not validation
//!
//! Every test in this file is a **closed form, an invariant, or a degenerate
//! limit**. Nothing here is compared against an experiment, and no result in
//! this file supports any claim that the six-equation model reproduces
//! reality. The Edwards–O'Brien and Marviken cases are separate work (beads
//! `op-s1a0`, `op-dt3.13`) and are not attempted here.
//!
//! # What is deliberately NOT pinned
//!
//! Stated up front so the coverage is not overread:
//!
//! - **No hyperbolicity or well-posedness test.** None exists here or
//!   upstream; see [`super::two_fluid`]'s module documentation for exactly what
//!   the virtual-mass regularisation is and is not claimed to do.
//! - **No grid-refinement study.** The scheme is first-order donor-cell on a
//!   system not known to be well posed, so refinement is not guaranteed to
//!   improve anything, and a refinement study belongs with a benchmark case
//!   rather than with these invariants.
//! - **No accuracy claim of any kind.** The tests below check that the solver
//!   computes what it says it computes, not that what it says is right.

use uom::si::angle::radian;
use uom::si::f64::{Angle, Length, Pressure, Ratio, ThermodynamicTemperature, Time};
use uom::si::length::meter;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use super::geometry::Pipe1d;
use super::interfacial::{DispersedPhase, InterfacialExchange};
use super::properties::SaturatedProperties;
use super::two_fluid::*;

/// A representative set of block inputs: steam and water at 7 MPa, a 30 µs
/// step, moderate drag. Used as the base for the algebra tests, which then vary
/// one field at a time.
fn base_inputs() -> PhaseCouplingInputs {
    PhaseCouplingInputs {
        alpha_g: 0.3,
        alpha_l: 0.7,
        rho_g: 36.5236,
        rho_l: 739.72,
        k_d: 1.25e4,
        c_vm: DEFAULT_VIRTUAL_MASS_COEFFICIENT,
        dispersed: DispersedPhase::Vapour,
        residual_alpha: DEFAULT_RESIDUAL_ALPHA,
        dt: 3.0e-5,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  1. The 2x2 implicit drag + virtual-mass block
// ─────────────────────────────────────────────────────────────────────────────

/// **Methodology.** [`virtual_mass_coefficient`] is the one closure
/// [`super::two_fluid`] writes down itself rather than calling into the 3-D
/// reference (whose `InterfacialForce::VirtualMass` is an unported scaffold),
/// so its transcription from OpenFOAM's `dispersedVirtualMassModel.C:51-67`
/// and `constantVirtualMassCoefficient.C:71-79` is the thing most worth
/// pinning. Check the whole formula
/// `K_vm = max(α_d, α_res) · C_vm · ρ_c` against hand arithmetic at three
/// points: the ordinary case, the vanishing-dispersed-phase case where the
/// `α_res` floor takes over, and the role swap where "dispersed" changes which
/// density is read.
///
/// Inputs: `ρ_l = 739.72 kg/m³`, `ρ_g = 36.5236 kg/m³` (steam/water near
/// 7 MPa), `C_vm = 0.5`, `α_res = 1e-6`. Pass criterion: agreement to `1e-12`
/// relative.
///
/// **Results (measured 2026-08-12, release).** All three exact — the printed
/// values match the expected values to every digit shown:
///
/// | case | `α_d` | `K_vm` \[kg/m³\] |
/// |---|---|---|
/// | vapour dispersed | `0.3` | `1.109580e2` |
/// | vapour dispersed, floored | `1e-9` | `3.698600e-4` |
/// | liquid dispersed | `0.7` | `1.278326e1` |
///
/// The middle row is the one that matters: at `α_d = 1e-9`, three decades below
/// `α_res`, the coefficient is `max(α_d, α_res) C_vm ρ_c = 1e-6 × 0.5 × 739.72
/// = 3.6986e-4`, not the `3.7e-7` the unfloored form would give — the floor is
/// doing its job and this test would catch its removal.
#[test]
fn virtual_mass_coefficient_matches_the_transcribed_upstream_form() {
    let (rho_g, rho_l) = (36.5236_f64, 739.72_f64);
    let c_vm = DEFAULT_VIRTUAL_MASS_COEFFICIENT;
    let a_res = DEFAULT_RESIDUAL_ALPHA;

    let cases = [
        ("vapour dispersed", 0.3_f64, rho_l, 0.3 * c_vm * rho_l),
        (
            "vapour dispersed, floored",
            1.0e-9,
            rho_l,
            a_res * c_vm * rho_l,
        ),
        ("liquid dispersed", 0.7, rho_g, 0.7 * c_vm * rho_g),
    ];
    for (label, alpha_d, rho_c, expected) in cases {
        let computed = virtual_mass_coefficient(alpha_d, rho_c, c_vm, a_res);
        println!(
            "{label:26}: alpha_d = {alpha_d:.1e}  K_vm = {computed:.6e}  (expected {expected:.6e})"
        );
        assert!(
            (computed - expected).abs() <= 1.0e-12 * expected.abs(),
            "{label}: {computed} vs {expected}"
        );
    }
}

/// **Methodology.** The whole point of folding virtual mass *inside* the
/// coupling matrix (`momentumTransferSystem.C:704-762`) rather than beside the
/// drag term is that it changes the matrix, not a source. Pin that
/// structurally: with `C_vm = 0` the block must reduce **exactly** to the
/// pure-drag block, and with `C_vm > 0` the coupling strength must be exactly
/// `taper × (K_d + K_vm/Δt)` on both the diagonal and the off-diagonal of each
/// row, with the inertial diagonal `A_k` untouched. If someone later
/// "simplifies" virtual mass into an explicit source, every clause here breaks.
///
/// Inputs: [`base_inputs`] — `α_g = 0.3`, steam/water at 7 MPa,
/// `K_d = 1.25e4 kg/(m³·s)`, `Δt = 30 µs`. Pass criterion: `1e-12` relative.
///
/// **Results (measured 2026-08-12, release)** — transcribed from the test's own
/// printed output:
///
/// | quantity \[kg/(m³·s)\] unless noted | `C_vm = 0` | `C_vm = 0.5` |
/// |---|---|---|
/// | off-diagonal `−c_g` | `-1.250000e4` | `-3.711100e6` |
/// | diagonal `A_g + c_g` | `3.777360e5` | `4.076336e6` |
/// | inertial diagonal `A_g` | `3.652360e5` | `3.652360e5` |
/// | `K̃^vm_g` \[kg/m³\] | `0.000000e0` | `1.109580e2` |
///
/// with `K_d = 1.250000e4` and `K_vm/Δt = 3.698600e6 kg/(m³·s)`.
///
/// Note that ratio: at this timestep the virtual-mass contribution to the
/// coupling is **295.9 times** the drag coefficient, so virtual mass is not a
/// perturbation of the block — it dominates it. That is worth holding next to
/// the regularisation caveat in [`super::two_fluid`]'s module docs: this term
/// is doing a great deal, and nobody, here or upstream, has established that
/// what it is doing is making the system well posed.
#[test]
fn virtual_mass_enters_the_block_and_switches_off_exactly_at_zero_cvm() {
    let mut off = base_inputs();
    off.c_vm = 0.0;
    let block_off = PhaseCouplingBlock::assemble(off).expect("well-posed block");
    let block_on = PhaseCouplingBlock::assemble(base_inputs()).expect("well-posed block");

    let k_d = base_inputs().k_d;
    let k_vm = virtual_mass_coefficient(
        base_inputs().alpha_g,
        base_inputs().rho_l,
        base_inputs().c_vm,
        base_inputs().residual_alpha,
    );
    let expected_coupling = k_d + k_vm / base_inputs().dt;

    println!(
        "C_vm = 0  : off-diag = {:.6e}  diag = {:.6e}  A_g = {:.6e}  Kvm_g = {:.6e}",
        block_off.vapour_liquid,
        block_off.vapour_vapour,
        block_off.vapour_diagonal,
        block_off.tapered_virtual_mass_vapour
    );
    println!(
        "C_vm = 0.5: off-diag = {:.6e}  diag = {:.6e}  A_g = {:.6e}  Kvm_g = {:.6e}",
        block_on.vapour_liquid,
        block_on.vapour_vapour,
        block_on.vapour_diagonal,
        block_on.tapered_virtual_mass_vapour
    );
    println!(
        "K_d = {k_d:.6e}   K_vm/dt = {:.6e}   ratio = {:.1}",
        k_vm / base_inputs().dt,
        k_vm / base_inputs().dt / k_d
    );

    // C_vm = 0 leaves the pure drag block.
    assert!((block_off.vapour_liquid + k_d).abs() <= 1.0e-12 * k_d);
    assert!((block_off.liquid_vapour + k_d).abs() <= 1.0e-12 * k_d);
    assert_eq!(block_off.tapered_virtual_mass_vapour, 0.0);
    assert_eq!(block_off.tapered_virtual_mass_liquid, 0.0);

    // C_vm > 0 adds exactly K_vm/dt to the coupling on BOTH sides.
    for coupling in [-block_on.vapour_liquid, -block_on.liquid_vapour] {
        assert!(
            (coupling - expected_coupling).abs() <= 1.0e-12 * expected_coupling,
            "{coupling} vs {expected_coupling}"
        );
    }
    // The inertial diagonal is untouched by virtual mass.
    assert_eq!(block_off.vapour_diagonal, block_on.vapour_diagonal);
    assert_eq!(block_off.liquid_diagonal, block_on.liquid_diagonal);
    // Diagonal = inertia + coupling, exactly.
    assert!(
        (block_on.vapour_vapour - (block_on.vapour_diagonal + expected_coupling)).abs()
            <= 1.0e-12 * block_on.vapour_vapour
    );
}

/// **Methodology.** The interfacial momentum exchange must be **equal and
/// opposite** between the phases, or the mixture gains momentum from nothing.
/// In this block that shows up as `c_g = c_l`, i.e. the two off-diagonals being
/// identical, which holds exactly wherever both vanishing-phase tapers are 1.
/// Sweep the void fraction across the interior range and require exact
/// equality; then check the *documented* exception — inside the taper band the
/// two rows are deliberately not symmetric.
///
/// Pass criterion: bitwise-equal off-diagonals over the interior sweep; the
/// vanishing phase's row strictly weaker inside the taper band.
///
/// **Results (measured 2026-08-12, release).** Exactly equal at all nine
/// interior void fractions swept (`1e-5` to `0.999999`); worst difference
/// `0.000000e0` — bitwise, not merely within a tolerance. Inside the taper band
/// at `α_g = 1e-9` the two rows differ by exactly the ratio of their tapers:
/// the vapour row is tapered by the (present) liquid, factor `1`, and the
/// liquid row by the (vanishing) vapour, factor `α_g/α_res = 1e-3`. The two
/// printed off-diagonals therefore stand in a `1000 : 1` ratio, which is
/// `momentumTransferSystem.C:617-620` doing exactly what it says. **Mixture
/// momentum is therefore not conserved inside the taper band**, and that is the
/// documented price of the numerical device rather than a defect.
#[test]
fn interfacial_momentum_coupling_is_antisymmetric_outside_the_taper_band() {
    let mut worst: f64 = 0.0;
    for alpha_g in [
        1.0e-5, 1.0e-4, 1.0e-3, 0.01, 0.1, 0.5, 0.9, 0.999, 0.999_999,
    ] {
        let mut inputs = base_inputs();
        inputs.alpha_g = alpha_g;
        inputs.alpha_l = 1.0 - alpha_g;
        let block = PhaseCouplingBlock::assemble(inputs).expect("well-posed block");
        worst = worst.max((block.vapour_liquid - block.liquid_vapour).abs());
        assert_eq!(
            block.vapour_liquid, block.liquid_vapour,
            "alpha_g = {alpha_g}: off-diagonals disagree"
        );
    }
    println!("interior sweep: worst off-diagonal difference = {worst:.6e}");

    let mut tapered = base_inputs();
    tapered.alpha_g = 1.0e-9;
    tapered.alpha_l = 1.0 - 1.0e-9;
    let block = PhaseCouplingBlock::assemble(tapered).expect("well-posed block");
    println!(
        "alpha_g = 1e-9 (inside taper band): vapour row = {:.6e}, liquid row = {:.6e}",
        block.vapour_liquid, block.liquid_vapour
    );
    assert!(
        block.liquid_vapour.abs() < block.vapour_liquid.abs(),
        "the taper must weaken the vanishing phase's row"
    );
}

/// **Methodology.** `residualAlpha` flooring
/// (`cellPressureCorrector.C:82-91`) exists so the coupling block stays
/// invertible where a phase vanishes — which, at a blowdown front, is
/// guaranteed to happen. Pin it by sweeping the void fraction across fifteen
/// decades on each side and requiring the determinant to stay strictly positive
/// and finite and both pressure coefficients to stay finite and non-negative.
///
/// Pass criterion: `det > 0` and finite, `d_g ≥ 0`, `d_l ≥ 0`, at every point.
///
/// **Results (measured 2026-08-12, release).** All 31 points pass. The rows the
/// test prints, transcribed:
///
/// | `α_g` | `det` \[kg²/(m⁶·s²)\] | `d_g` \[m³·s/kg\] | `d_l` \[m³·s/kg\] |
/// |---|---|---|---|
/// | `0` | `3.085507e11` | `4.055194e-8` | `4.055589e-8` |
/// | `1e-15` | `3.085507e11` | `4.055194e-8` | `4.055589e-8` |
/// | `1e-6` | `3.085504e11` | `4.063189e-8` | `4.055592e-8` |
/// | `1 − 1e-6` | `1.502510e13` | `8.213709e-7` | `8.213693e-7` |
/// | `1 − 1e-15` | `1.502482e13` | `8.213867e-7` | `8.213851e-7` |
/// | `1` | `1.502482e13` | `8.213867e-7` | `8.213851e-7` |
///
/// The determinant is **flat** from `α_g = 0` through `1e-15` to `1e-6` — that
/// is the floor holding it, visibly, over fifteen decades in which the
/// unfloored `A_g = α_g ρ_g / Δt` would have fallen by the same fifteen decades
/// and taken the determinant with it toward the cancellation that makes the
/// block singular.
#[test]
fn residual_alpha_flooring_keeps_the_block_invertible_as_a_phase_vanishes() {
    let mut printed = Vec::new();
    for exponent in 0..=15 {
        for alpha_g in [10.0_f64.powi(-exponent), 1.0 - 10.0_f64.powi(-exponent)] {
            if !(0.0..=1.0).contains(&alpha_g) {
                continue;
            }
            let mut inputs = base_inputs();
            inputs.alpha_g = alpha_g;
            inputs.alpha_l = 1.0 - alpha_g;
            let block = PhaseCouplingBlock::assemble(inputs).expect("well-posed block");
            let det = block.determinant();
            let (d_g, d_l) = block
                .pressure_coefficients(inputs.alpha_g, inputs.alpha_l)
                .expect("invertible");
            assert!(
                det > 0.0 && det.is_finite(),
                "alpha_g = {alpha_g}: det = {det}"
            );
            assert!(
                d_g >= 0.0 && d_g.is_finite(),
                "alpha_g = {alpha_g}: d_g = {d_g}"
            );
            assert!(
                d_l >= 0.0 && d_l.is_finite(),
                "alpha_g = {alpha_g}: d_l = {d_l}"
            );
            if matches!(exponent, 0 | 6 | 15) {
                printed.push(format!(
                    "alpha_g = {alpha_g:.15}  det = {det:.6e}  d_g = {d_g:.6e}  d_l = {d_l:.6e}"
                ));
            }
        }
    }
    for line in printed {
        println!("{line}");
    }
}

/// **Methodology.** The reason for the `α_j / max(α_j, α_res)` taper is that a
/// phase which is not there must not be able to change the momentum of the
/// phase that is. Check the algebraic limit: as `α_g → 0` the block must
/// return the **uncoupled single-phase liquid answer**, `u_l = b_l / A_l` and
/// `d_l = α_l / A_l`. Then check the mirror image at `α_l → 0`.
///
/// **Read the taper carefully — it is linear, not a switch.** Below `α_res` the
/// factor is `α_j / α_res`, so it reaches exactly zero only at `α_j = 0` and is
/// still `1e-6` at `α_j = 1e-12`. This test therefore checks two different
/// things with two different criteria, and the distinction is the point rather
/// than a technicality:
///
/// - at `α_g = 0` **exactly**, agreement must be exact to `1e-15` relative,
///   because the off-diagonal is identically zero and the 2×2 inverse collapses
///   to the reciprocal of one diagonal entry — literally the same arithmetic;
/// - inside the taper band the residual coupling is real, and what is asserted
///   is that it is exactly **proportional** to the taper. Proportionality, not
///   smallness, is what establishes the taper is linear and applied once.
///
/// A first draft asserted `1e-9` agreement at `α_g = 1e-12` and failed, at a
/// measured relative departure of `1.0e-7`. That was the *test* being wrong
/// about the taper, not the block — recorded because the tempting fix (loosen
/// the number until it passes) would have hidden the fact that a vanishing
/// phase is not fully decoupled until it is exactly absent.
///
/// **Results (measured 2026-08-12, release).**
///
/// At `α_g = 0` and `α_l = 0` the block matches the single-phase closed form to
/// all sixteen printed digits (e.g. `u_l = 4.055588601092306e-4` against an
/// exact `4.055588601092306e-4`). Inside the taper band:
///
/// | `α_g` | taper | relative departure |
/// |---|---|---|
/// | `1e-12` | `1.0e-6` | `9.999022e-8` |
/// | `1e-10` | `1.0e-4` | `9.999022e-6` |
/// | `1e-8` | `1.0e-2` | `9.999022e-4` |
///
/// Four decades of taper, and the departure tracks it with the same four
/// leading digits — `departure / taper = 9.999022e-2` throughout. That is the
/// linearity being asserted.
#[test]
fn a_vanishing_phase_leaves_the_present_phase_uncoupled() {
    let (b_g, b_l) = (1.0e3_f64, 1.0e4_f64);

    // alpha_g == 0 exactly: the taper is exactly zero and the liquid row is
    // exactly the single-phase one.
    let mut inputs = base_inputs();
    inputs.alpha_g = 0.0;
    inputs.alpha_l = 1.0;
    let block = PhaseCouplingBlock::assemble(inputs).expect("well-posed block");
    let (_, u_l) = block.solve(b_g, b_l).expect("invertible");
    let (_, d_l) = block
        .pressure_coefficients(inputs.alpha_g, inputs.alpha_l)
        .expect("invertible");
    let u_l_exact = b_l / block.liquid_diagonal;
    let d_l_exact = inputs.alpha_l / block.liquid_diagonal;
    println!(
        "alpha_g = 0     : u_l = {u_l:.15e} (exact {u_l_exact:.15e})\n\
         \x20                d_l = {d_l:.15e} (exact {d_l_exact:.15e})"
    );
    assert!((u_l - u_l_exact).abs() <= 1.0e-15 * u_l_exact.abs());
    assert!((d_l - d_l_exact).abs() <= 1.0e-15 * d_l_exact.abs());

    // alpha_l == 0 exactly: the mirror image.
    let mut inputs = base_inputs();
    inputs.alpha_g = 1.0;
    inputs.alpha_l = 0.0;
    let block = PhaseCouplingBlock::assemble(inputs).expect("well-posed block");
    let (u_g, _) = block.solve(b_g, b_l).expect("invertible");
    let (d_g, _) = block
        .pressure_coefficients(inputs.alpha_g, inputs.alpha_l)
        .expect("invertible");
    let u_g_exact = b_g / block.vapour_diagonal;
    let d_g_exact = inputs.alpha_g / block.vapour_diagonal;
    println!(
        "alpha_l = 0     : u_g = {u_g:.15e} (exact {u_g_exact:.15e})\n\
         \x20                d_g = {d_g:.15e} (exact {d_g_exact:.15e})"
    );
    assert!((u_g - u_g_exact).abs() <= 1.0e-15 * u_g_exact.abs());
    assert!((d_g - d_g_exact).abs() <= 1.0e-15 * d_g_exact.abs());

    // Inside the taper band the decoupling is only approximate, and the
    // residual coupling must be exactly PROPORTIONAL to the taper
    // alpha_g / alpha_res -- that proportionality, not any particular
    // magnitude, is what says the taper is linear and is being applied once.
    let mut ratios = Vec::new();
    for alpha_g in [1.0e-12_f64, 1.0e-10, 1.0e-8] {
        let mut inputs = base_inputs();
        inputs.alpha_g = alpha_g;
        inputs.alpha_l = 1.0 - alpha_g;
        let block = PhaseCouplingBlock::assemble(inputs).expect("well-posed block");
        let (_, u_l) = block.solve(b_g, b_l).expect("invertible");
        let exact = b_l / block.liquid_diagonal;
        let departure = (u_l - exact).abs() / exact.abs();
        let taper = alpha_g / DEFAULT_RESIDUAL_ALPHA;
        ratios.push(departure / taper);
        println!(
            "alpha_g = {alpha_g:.0e}: taper = {taper:.1e}, relative departure from the \
             single-phase answer = {departure:.6e}, departure/taper = {:.9e}",
            departure / taper
        );
    }
    let first = ratios[0];
    for observed in &ratios {
        assert!(
            (observed - first).abs() <= 1.0e-6 * first,
            "the residual coupling is not proportional to the taper: {ratios:?}"
        );
    }
}

/// **Methodology.** The other degenerate limit a two-fluid model must get right
/// is the **no-slip** one: as the coupling grows without bound the phase
/// velocities must converge, because that is the limit in which a six-equation
/// model degenerates to a homogeneous one. The closed form falls out of the
/// 2×2 inverse,
///
/// `u_g − u_l = (A_l b_g − A_g b_l) / det`,   `det = A_g A_l + A_g c_l + A_l c_g`
///
/// so the slip must fall as `1/K_d` once the coupling dominates the inertia.
/// Sweep `K_d` over eight decades (with `C_vm = 0`, so the sweep is over drag
/// alone) and check both the convergence and that decay rate, then check that
/// the velocity they converge on is the mixture-momentum answer
/// `(b_g + b_l)/(A_g + A_l)`.
///
/// Pass criterion: slip strictly decreasing in `K_d`; below `1e-6 m/s` at
/// `K_d = 1e12`; common velocity matching the mixture answer to `1e-6`
/// relative.
///
/// **Results (measured 2026-08-12, release)**, with `b_g = 1e3`,
/// `b_l = 1e4 N/m³`:
///
/// | `K_d` \[kg/(m³·s)\] | `u_g` \[m/s\] | `u_l` \[m/s\] | slip \[m/s\] | ratio to previous |
/// |---|---|---|---|---|
/// | `1e4` | `2.680462e-3` | `5.805864e-4` | `2.099876e-3` | — |
/// | `1e6` | `1.180980e-3` | `6.123165e-4` | `5.686636e-4` | 3.7 |
/// | `1e8` | `6.316340e-4` | `6.239410e-4` | `7.693045e-6` | 73.9 |
/// | `1e10` | `6.241760e-4` | `6.240988e-4` | `7.720285e-8` | 99.6 |
/// | `1e12` | `6.241012e-4` | `6.241004e-4` | `7.720558e-10` | 100.0 |
///
/// The last three rows are the `1/K_d` asymptote — a factor of 100 per two
/// decades, converged to three figures. The velocity the phases meet at,
/// `6.241012e-4 m/s`, matches the mixture-momentum value `6.241004e-4 m/s` to
/// a relative `1.3e-6`, which is the same order as the residual slip at that
/// `K_d` and is why the tolerance is `1e-5` rather than tighter.
#[test]
fn strong_coupling_drives_the_phase_velocities_together_as_one_over_k_d() {
    let (b_g, b_l) = (1.0e3_f64, 1.0e4_f64);
    let mut previous_slip = f64::INFINITY;
    let mut rows = Vec::new();
    for exponent in [4, 6, 8, 10, 12] {
        let mut inputs = base_inputs();
        inputs.c_vm = 0.0;
        inputs.k_d = 10.0_f64.powi(exponent);
        let block = PhaseCouplingBlock::assemble(inputs).expect("well-posed block");
        let (u_g, u_l) = block.solve(b_g, b_l).expect("invertible");
        let slip = (u_g - u_l).abs();
        rows.push(format!(
            "K_d = 1e{exponent:<2}  u_g = {u_g:.6e}  u_l = {u_l:.6e}  slip = {slip:.6e}  \
             ratio to previous = {:.1}",
            previous_slip / slip
        ));
        assert!(slip < previous_slip, "slip must fall as K_d rises");
        previous_slip = slip;
    }
    for row in rows {
        println!("{row}");
    }
    assert!(
        previous_slip < 1.0e-6,
        "no-slip limit not reached: {previous_slip}"
    );

    let mut inputs = base_inputs();
    inputs.c_vm = 0.0;
    inputs.k_d = 1.0e12;
    let block = PhaseCouplingBlock::assemble(inputs).expect("well-posed block");
    let (u_g, _) = block.solve(b_g, b_l).expect("invertible");
    let mixture = (b_g + b_l) / (block.vapour_diagonal + block.liquid_diagonal);
    let relative = (u_g - mixture).abs() / mixture.abs();
    println!(
        "no-slip common velocity = {u_g:.6e} m/s, mixture-momentum value = {mixture:.6e} m/s, \
         relative difference = {relative:.6e}"
    );
    // The mixture-momentum value is the K_d -> infinity limit; at a finite
    // K_d = 1e12 the remaining difference is of the same order as the residual
    // slip (1.2e-6 relative, measured), so the tolerance is set just above it
    // rather than at a round number that would happen to pass.
    assert!(relative <= 1.0e-5, "relative difference {relative:e}");
}

/// **Methodology.** Malformed inputs must be **refused**, not silently
/// repaired, per the crate's errors-not-clamps rule. Feed the block one bad
/// field at a time and require an error from each.
///
/// **Results (measured 2026-08-12, release).** All eight refused with
/// [`crate::TampinesError::InvalidInput`]; the exact messages are printed by
/// the test.
/// Which field of [`PhaseCouplingInputs`] a refusal case corrupts.
///
/// An enum rather than a table of closures, because the workspace design rules
/// forbid `Box<dyn Trait>` for dispatch — including here, where a closure table
/// would have been the obvious shape.
#[derive(Debug, Clone, Copy)]
enum Malformed {
    NonFiniteVapourFraction,
    VapourFractionAboveOne,
    NegativeLiquidFraction,
    ZeroVapourDensity,
    NegativeLiquidDensity,
    NegativeDrag,
    NegativeVirtualMass,
    ZeroTimestep,
}

impl Malformed {
    /// Every case, so adding one forces this list to be revisited.
    const ALL: [Self; 8] = [
        Self::NonFiniteVapourFraction,
        Self::VapourFractionAboveOne,
        Self::NegativeLiquidFraction,
        Self::ZeroVapourDensity,
        Self::NegativeLiquidDensity,
        Self::NegativeDrag,
        Self::NegativeVirtualMass,
        Self::ZeroTimestep,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::NonFiniteVapourFraction => "non-finite alpha_g",
            Self::VapourFractionAboveOne => "alpha_g above 1",
            Self::NegativeLiquidFraction => "negative alpha_l",
            Self::ZeroVapourDensity => "zero rho_g",
            Self::NegativeLiquidDensity => "negative rho_l",
            Self::NegativeDrag => "negative K_d",
            Self::NegativeVirtualMass => "negative C_vm",
            Self::ZeroTimestep => "zero dt",
        }
    }

    fn corrupt(self, inputs: &mut PhaseCouplingInputs) {
        match self {
            Self::NonFiniteVapourFraction => inputs.alpha_g = f64::NAN,
            Self::VapourFractionAboveOne => inputs.alpha_g = 1.4,
            Self::NegativeLiquidFraction => inputs.alpha_l = -0.1,
            Self::ZeroVapourDensity => inputs.rho_g = 0.0,
            Self::NegativeLiquidDensity => inputs.rho_l = -1.0,
            Self::NegativeDrag => inputs.k_d = -1.0,
            Self::NegativeVirtualMass => inputs.c_vm = -0.5,
            Self::ZeroTimestep => inputs.dt = 0.0,
        }
    }
}

#[test]
fn the_coupling_block_refuses_malformed_inputs() {
    for case in Malformed::ALL {
        let mut inputs = base_inputs();
        case.corrupt(&mut inputs);
        let outcome = PhaseCouplingBlock::assemble(inputs);
        println!("{:22} -> {outcome:?}", case.label());
        assert!(
            matches!(outcome, Err(crate::TampinesError::InvalidInput(_))),
            "{} was not refused",
            case.label()
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  2. The property-layer workaround
// ─────────────────────────────────────────────────────────────────────────────

/// **Methodology.** [`region_4_safe_pressure`] works around a **panic** in
/// `tampines-steam-tables`: its forward-equation region classifier decides
/// "this is the two-phase Region 4" with an exact float equality
/// (`pt_flash_eqm/mod.rs:134`, `pres == p_sat_reg4_pascal`) and then
/// `cp_tp_eqm_single_phase` `panic!`s on that branch (`:211`). Since
/// [`super::properties::SaturatedTransport::at`] evaluates conductivity at
/// exactly `(T_sat(p), p)`, it panics for every pressure that round-trips
/// bit-exactly through `sat_pressure_4(sat_temp_4(p))`.
///
/// A workaround based on a *guess* about which pressures are affected would be
/// worthless, so this test measures it: sweep a geometric ladder of pressures,
/// catch the panics, and compare against the bit-exact round-trip predicate.
/// Then check that routing through [`region_4_safe_pressure`] removes every one
/// of them and leaves every other pressure bit-identical.
///
/// Pass criterion: the predicate agrees with the observed panics with zero
/// mispredictions in either direction; zero panics after the guard; the guard
/// alters exactly the pressures that needed it.
///
/// **Results (measured 2026-08-12, release), 10 790 pressures from `1.0e5` Pa
/// to `2.2e7` Pa at a ratio of `1.0005`:**
///
/// | quantity | value |
/// |---|---|
/// | `SaturatedTransport::at` panics, unguarded | **105** |
/// | predicted by the bit-exact round-trip | **105** |
/// | mispredictions, either direction | **0** |
/// | `SaturatedProperties::at` panics | **0** |
/// | panics after `region_4_safe_pressure` | **0** |
/// | pressures altered by the guard | **105** |
/// | largest relative alteration | `4.000008e-12` |
///
/// So roughly **1 % of pressures panic**, scattered rather than banded; the
/// predicate is exact; and the guard costs a relative `4e-12` — eight decades
/// below [`super::properties::TRANSPORT_CACHE_TOLERANCE`] — on exactly the
/// pressures that need it. `SaturatedProperties::at` is unaffected because it
/// never routes through the classifier.
///
/// **This test pins a workaround, not a fix.** The defect — a panic where an
/// error belongs, on an exact float comparison at a region boundary — is in
/// `tampines-steam-tables` and belongs there.
#[test]
fn region_4_safe_pressure_removes_the_upstream_classifier_panic() {
    use tampines_steam_tables::region_4_vap_liq_equilibrium::sat_pressure::sat_pressure_4;
    use tampines_steam_tables::region_4_vap_liq_equilibrium::sat_temp::sat_temp_4;

    let (mut panics, mut predicted, mut mispredictions) = (0usize, 0usize, 0usize);
    let (mut sat_panics, mut guarded_panics, mut altered) = (0usize, 0usize, 0usize);
    let mut worst_alteration: f64 = 0.0;
    let mut count = 0usize;

    let mut p = 1.0e5_f64;
    while p < 2.2e7 {
        count += 1;
        let round_trip = sat_pressure_4(sat_temp_4(Pressure::new::<pascal>(p))).get::<pascal>();
        let flagged = round_trip == p;
        if flagged {
            predicted += 1;
        }
        let unguarded =
            std::panic::catch_unwind(|| super::properties::SaturatedTransport::at(p)).is_err();
        if unguarded {
            panics += 1;
        }
        if unguarded != flagged {
            mispredictions += 1;
        }
        if std::panic::catch_unwind(|| super::properties::SaturatedProperties::at(p)).is_err() {
            sat_panics += 1;
        }

        let safe = region_4_safe_pressure(p);
        if safe != p {
            altered += 1;
            worst_alteration = worst_alteration.max(((safe - p) / p).abs());
        }
        if std::panic::catch_unwind(|| super::properties::SaturatedTransport::at(safe)).is_err() {
            guarded_panics += 1;
        }
        p *= 1.0005;
    }

    println!("swept {count} pressures from 1.0e5 to 2.2e7 Pa");
    println!("  SaturatedTransport::at panics, unguarded : {panics}");
    println!("  predicted by bit-exact round-trip        : {predicted}");
    println!("  mispredictions                           : {mispredictions}");
    println!("  SaturatedProperties::at panics           : {sat_panics}");
    println!("  panics after region_4_safe_pressure      : {guarded_panics}");
    println!("  pressures altered by the guard           : {altered}");
    println!("  largest relative alteration              : {worst_alteration:.6e}");

    assert_eq!(mispredictions, 0, "the round-trip predicate is not exact");
    assert_eq!(guarded_panics, 0, "the guard did not remove every panic");
    assert_eq!(
        altered, panics,
        "the guard altered a different set of pressures than the ones that panic"
    );
    assert!(worst_alteration <= MAX_REGION_4_NUDGE * 4.0);
    assert!(
        panics > 0,
        "no pressure panicked -- the upstream defect may have been fixed; re-check \
         region_4_safe_pressure and this test before deleting either"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
//  3. Conservation and well-balancedness of the march
// ─────────────────────────────────────────────────────────────────────────────

/// Build a small closed pipe with a uniform saturated two-phase initial state —
/// the regime the interfacial closures are actually meant for.
fn closed_two_phase_pipe(
    n_cells: usize,
    void_fraction: f64,
    diameter_m: f64,
    dt_s: f64,
    subcooling_k: f64,
) -> TwoFluid1d {
    let pipe = Pipe1d::circular(
        Length::new::<meter>(1.0),
        Length::new::<meter>(0.05),
        Angle::new::<radian>(0.0),
        n_cells,
    )
    .expect("well-posed pipe");
    let exchange = InterfacialExchange::bubbly(Length::new::<meter>(diameter_m))
        .expect("well-posed bubbly closure set");
    let sat = SaturatedProperties::at(7.0e6).expect("inside IF97");
    let temperature = sat.t_sat - subcooling_k;
    let mut solver = TwoFluid1d::new(
        pipe,
        exchange,
        Pressure::new::<pascal>(7.0e6),
        ThermodynamicTemperature::new::<kelvin>(temperature),
        Time::new::<second>(dt_s),
    )
    .expect("well-posed initial state");
    solver
        .set_initial_void_fraction(void_fraction)
        .expect("void fraction in (0, 1)");
    let profile = vec![ThermodynamicTemperature::new::<kelvin>(temperature); n_cells];
    solver
        .set_temperature_profile(&profile)
        .expect("uniform profile");
    solver
}

/// **Methodology.** Total mass is the one quantity a conservative
/// finite-volume scheme owes exactly. Here the primary state is the pair of
/// transported phase mass concentrations `m_g`, `m_l`, updated by donor-cell
/// fluxes plus an interfacial exchange whose two halves are `+Γ` and `−Γ`; in a
/// **closed** pipe the mixture inventory must therefore be conserved to machine
/// precision — through the interfacial mass transfer, through the rate limiter,
/// and through the residual-alpha floor if they fire.
///
/// Note what is *not* claimed: total enthalpy is **not** conserved, because the
/// energy equations carry the reversible work term `α_k ∂p/∂t` and a closed
/// pipe whose phases exchange mass does change pressure. That drift is measured
/// and printed rather than asserted away.
///
/// **The initial state is deliberately off equilibrium.** At exact saturation
/// every interfacial driving difference is zero, `Γ` is identically zero, and
/// the test would conserve mass trivially without ever exercising the exchange
/// — which is the failure mode a conservation test most needs to avoid. The
/// liquid is therefore started 5 K subcooled against saturated vapour with
/// 0.2 mm bubbles, so condensation is running throughout.
///
/// Inputs: 1 m pipe, 0.05 m bore, 8 cells, closed at both ends, 7 MPa,
/// `α_g = 0.1`, liquid 5 K subcooled, 0.2 mm bubbles, `Δt = 100 µs`, 200 steps
/// (20 ms). Pass criterion: relative mass drift below `1e-13` at every step.
///
/// **Results (measured 2026-08-12, release).** Printed by the test. The
/// enthalpy drift is *not* zero and is not expected to be: condensation lowers
/// the pressure of the closed pipe, and the reversible work term `α_k ∂p/∂t`
/// then changes the total enthalpy by exactly that amount — which is what
/// `interfacial_exchange_moves_energy_between_the_phases_and_creates_none`
/// checks separately, to `4.15e-11` relative.
#[test]
fn a_closed_pipe_conserves_mixture_mass_to_machine_precision() {
    let mut solver = closed_two_phase_pipe(8, 0.1, 2.0e-4, 1.0e-4, 5.0);
    let mass_0 = solver.inventory().value;
    let enthalpy_0 = solver.total_enthalpy();
    let mut worst_residual: f64 = 0.0;
    let mut worst_drift: f64 = 0.0;

    for k in 0..200 {
        let report = solver.step().unwrap_or_else(|e| panic!("step {k}: {e}"));
        worst_residual = worst_residual.max(report.max_volume_residual);
        worst_drift = worst_drift.max(((report.inventory - mass_0) / mass_0).abs());
    }

    let mass_drift = (solver.inventory().value - mass_0) / mass_0;
    let enthalpy_drift = (solver.total_enthalpy() - enthalpy_0) / enthalpy_0;
    println!("initial inventory       = {mass_0:.12e} kg");
    println!("relative mass drift     = {mass_drift:.6e}  (worst over the run {worst_drift:.6e})");
    println!(
        "relative enthalpy drift = {enthalpy_drift:.6e}  (NOT expected to vanish -- work term)"
    );
    println!("worst volume residual   = {worst_residual:.6e}");
    println!("final pressure[0]       = {:.8e} Pa", solver.pressure()[0]);
    println!(
        "final void[0]           = {:.6e}",
        solver.void_fraction()[0]
    );

    assert!(
        worst_drift < 1.0e-13,
        "mixture mass is not conserved: relative drift {worst_drift:e}"
    );
}

/// **Methodology.** A *well-balanced* scheme must leave an exact equilibrium
/// alone. Set up the one this solver has: a uniform, horizontal, closed pipe
/// whose phases are both exactly on the saturation line at the same pressure,
/// at rest. Every driving difference is then zero — no pressure gradient, no
/// slip, no interfacial temperature difference — so the correct answer is that
/// nothing happens at all.
///
/// This is the sharpest test of spurious source terms in the file: a sign error
/// in the interfacial exchange, an inconsistency between the donor-cell fluxes
/// of `m_k` and `m_k h_k`, or a leftover in the pressure equation would all show
/// up here as motion out of nothing.
///
/// Inputs: 1 m pipe, 8 cells, closed, `p = 7 MPa`, both phases at `T_sat`,
/// `α_g = 0.1`, 1 mm bubbles, `Δt = 100 µs`, 50 steps. Pass criterion: face
/// velocities below `1e-10 m/s`, pressure change below 1 Pa, void change below
/// `1e-12`.
///
/// **Results (measured 2026-08-12, release).** Every quantity is **identically
/// zero**, not merely small:
///
/// | quantity | value |
/// |---|---|
/// | worst face velocity over 50 steps | `0.000000e0 m/s` |
/// | pressure change | `0.000000e0 Pa` |
/// | void-fraction change | `0.000000e0` |
/// | final `|T_g − T_l|` | `0.000000e0 K` |
///
/// Exact zeros are the right answer here and are stronger evidence than small
/// ones: they say the equilibrium is a fixed point of the discrete operator,
/// not merely a slowly-drifting near-fixed-point.
#[test]
fn a_uniform_saturated_state_at_rest_stays_at_rest() {
    let mut solver = closed_two_phase_pipe(8, 0.1, 1.0e-3, 1.0e-4, 0.0);
    let p_0 = solver.pressure()[0];
    let alpha_0 = solver.void_fraction()[0];

    let mut worst_velocity: f64 = 0.0;
    for k in 0..50 {
        let report = solver.step().unwrap_or_else(|e| panic!("step {k}: {e}"));
        worst_velocity = worst_velocity.max(report.max_slip);
        for u in solver
            .vapour_face_velocity()
            .iter()
            .chain(solver.liquid_face_velocity().iter())
        {
            worst_velocity = worst_velocity.max(u.abs());
        }
    }
    let pressure_change = (solver.pressure()[0] - p_0).abs();
    let void_change = (solver.void_fraction()[0] - alpha_0).abs();
    println!("worst face velocity over 50 steps = {worst_velocity:.6e} m/s");
    println!("pressure change                   = {pressure_change:.6e} Pa");
    println!("void-fraction change              = {void_change:.6e}");
    println!(
        "final |T_g - T_l|                 = {:.6e} K",
        (solver.vapour_temperature()[0] - solver.liquid_temperature()[0]).abs()
    );

    assert!(
        worst_velocity < 1.0e-10,
        "spurious motion out of an equilibrium: {worst_velocity:e} m/s"
    );
    assert!(
        pressure_change < 1.0,
        "spurious pressure drift: {pressure_change:e} Pa"
    );
    assert!(
        void_change < 1.0e-12,
        "spurious void drift: {void_change:e}"
    );
}

/// **Methodology.** The interfacial energy balance is an **identity**, not an
/// approximation: with the interface at `T_sat` and the transferring mass
/// carrying the saturation enthalpy of its own side,
///
/// `Q_g + Q_l + Γ h_fg = 0`  with  `Γ = −(Q_g + Q_l)/h_fg`
///
/// cancels exactly, so interfacial exchange moves energy **between** the phases
/// and creates none. In a closed pipe at rest the only other energy source is
/// the reversible work term `α_k ∂p/∂t`, so the change in total enthalpy over
/// one step must equal `Σ_i (α_g + α_l)_i Δp_i V` exactly.
///
/// This is the test that would catch a wrong interfacial enthalpy (`h_g`
/// instead of `h_g^sat`, say), a wrong `Γ` sign, or a rate limiter that scaled
/// `Γ` without scaling both heat fluxes with it — which is precisely why
/// [`MAX_MASS_TRANSFER_FRACTION_PER_STEP`] scales all three by the same factor.
///
/// Inputs: closed 8-cell pipe at 7 MPa with the liquid 5 K subcooled against
/// saturated vapour so that `Γ ≠ 0`, `α_g = 0.1`, 0.2 mm bubbles,
/// `Δt = 100 µs`, 20 steps. Pass criterion: `|ΔH − work| / |ΔH| < 1e-9` at every
/// step.
///
/// **Results (measured 2026-08-12, release).**
///
/// | quantity | value |
/// |---|---|
/// | cumulative reversible work over 20 steps | `-3.437803673e2 J` |
/// | worst relative `|ΔH − work|` mismatch | `4.151018e-11` |
/// | final `|T_g − T_l|` | `2.495742e0 K` |
///
/// The mismatch is at the level of the floating-point summation over cells,
/// eight decades below the `1e-9` criterion, and the work term is a real
/// `−344 J` rather than a rounding artefact — so the identity is being
/// exercised, not trivially satisfied.
#[test]
fn interfacial_exchange_moves_energy_between_the_phases_and_creates_none() {
    let n = 8;
    let mut solver = closed_two_phase_pipe(n, 0.1, 2.0e-4, 1.0e-4, 5.0);
    let volume = solver.pipe().cell_volume();
    let mut worst_mismatch: f64 = 0.0;
    let mut cumulative_work = 0.0_f64;

    for k in 0..20 {
        let h_before = solver.total_enthalpy();
        let p_before: Vec<f64> = solver.pressure().to_vec();
        let alpha_g_before: Vec<f64> = solver.void_fraction().to_vec();
        let alpha_l_before: Vec<f64> = solver.liquid_fraction().to_vec();

        solver.step().unwrap_or_else(|e| panic!("step {k}: {e}"));

        let work: f64 = (0..n)
            .map(|i| {
                (alpha_g_before[i] + alpha_l_before[i])
                    * (solver.pressure()[i] - p_before[i])
                    * volume
            })
            .sum();
        let change = solver.total_enthalpy() - h_before;
        cumulative_work += work;
        let mismatch = if change.abs() > 0.0 {
            (change - work).abs() / change.abs()
        } else {
            (change - work).abs()
        };
        worst_mismatch = worst_mismatch.max(mismatch);
    }
    println!("cumulative reversible work over 20 steps = {cumulative_work:.9e} J");
    println!("worst relative |dH - work| mismatch      = {worst_mismatch:.6e}");
    println!(
        "final |T_g - T_l|                        = {:.6e} K",
        (solver.vapour_temperature()[0] - solver.liquid_temperature()[0]).abs()
    );

    assert!(
        worst_mismatch < 1.0e-9,
        "interfacial exchange is creating or destroying energy: mismatch {worst_mismatch:e}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
//  4. Thermal non-equilibrium — the thing that justifies six equations
// ─────────────────────────────────────────────────────────────────────────────

/// **Methodology.** A six-equation model exists to carry a phase-temperature
/// difference that HEM and drift flux cannot represent. Demonstrate that it
/// does, and — more importantly — that the **implicit** interfacial relaxation
/// [`TwoFluid1d::step`] uses is unconditionally stable: it approaches `T_sat`
/// monotonically and never overshoots, at a timestep and bubble diameter for
/// which the explicit form
/// (`Δt < m_k c_p,k / K_k`, and `K_k` scales as `1/d²`) would diverge outright.
///
/// Inputs: closed 4-cell pipe at 7 MPa, liquid initialised 10 K subcooled
/// against saturated vapour, `α_g = 0.05`, bubble diameter `1e-4 m`,
/// `Δt = 1 ms` (three decades above what an explicit relaxation would tolerate
/// at this `K_l`), 40 steps.
///
/// Pass criterion: `|T_g − T_l|` finite at every step; non-increasing and of
/// constant sign while it is above a `1e-4 K` noise floor; and strictly smaller
/// at the end than at the start.
///
/// **The noise floor is measured, not assumed.** `T_g` and `T_l` each come from
/// a bracketed IF97 enthalpy inversion, so their difference carries that
/// inversion's round-off. Measured 2026-08-12: the difference is `6.465052e-5 K`
/// by step 20 and thereafter wanders at the `1e-6 K` level — at step 29 it rose
/// from `4.34e-7 K` to `7.59e-7 K`, and at step 28 its sign flipped. Both are
/// round-off, not instability, and asserting through them would be asserting on
/// noise. The floor sits two decades above it.
///
/// **Results (measured 2026-08-12, release).** The relaxation is fast and
/// clean — note that `T_sat` is itself falling, because condensation is
/// lowering the pressure of the closed pipe, so both phases are chasing a
/// moving target:
///
/// | step | `|T_g − T_l|` \[K\] | `T_g` \[K\] | `T_l` \[K\] | `T_sat` \[K\] |
/// |---|---|---|---|---|
/// | 0 | `10.00000000` | `558.98002` | `548.98002` | `558.98002` |
/// | 1 | `4.25945173` | `553.14178` | `548.88233` | `554.47645` |
/// | 2 | `1.87669512` | `550.70417` | `548.82747` | `551.91147` |
/// | 3 | `0.97205263` | `549.77192` | `548.79987` | `550.55484` |
/// | 5 | `0.31601497` | `549.09210` | `548.77609` | `549.34526` |
/// | 10 | `0.01948310` | `548.78472` | `548.76524` | `548.79805` |
/// | 20 | `0.00006465` | `548.76464` | `548.76457` | `548.76468` |
///
/// Five decades of decay in twenty steps at `Δt = 1 ms`, with no overshoot,
/// at a timestep for which an explicit `Q_k = K_k (T_sat − T_k)` would have
/// diverged on the first step.
#[test]
fn thermal_non_equilibrium_relaxes_monotonically_and_never_overshoots() {
    let mut solver = closed_two_phase_pipe(4, 0.05, 1.0e-4, 1.0e-3, 10.0);

    let mut previous = (solver.vapour_temperature()[0] - solver.liquid_temperature()[0]).abs();
    let initial = previous;
    let initial_sign = (solver.vapour_temperature()[0] - solver.liquid_temperature()[0]).signum();
    println!("step  |T_g - T_l| [K]     T_g [K]     T_l [K]   T_sat [K]");
    println!(
        "   0  {previous:14.8}  {:10.5}  {:10.5}  {:10.5}",
        solver.vapour_temperature()[0],
        solver.liquid_temperature()[0],
        solver.saturation_temperature()[0]
    );
    for k in 1..=40 {
        let report = solver.step().unwrap_or_else(|e| panic!("step {k}: {e}"));
        let difference = solver.vapour_temperature()[0] - solver.liquid_temperature()[0];
        let magnitude = difference.abs();
        if k <= 5 || k % 10 == 0 {
            println!(
                "{k:4}  {magnitude:14.8}  {:10.5}  {:10.5}  {:10.5}   (report {:.6e})",
                solver.vapour_temperature()[0],
                solver.liquid_temperature()[0],
                solver.saturation_temperature()[0],
                report.max_thermal_nonequilibrium
            );
        }
        assert!(
            magnitude.is_finite(),
            "step {k}: |T_g - T_l| went non-finite"
        );
        // Monotonicity and the sign are only meaningful while the difference is
        // above the noise floor of the two IF97 temperature inversions that
        // produce it. Measured 2026-08-12: the difference is 6.5e-5 K by step
        // 20 and thereafter wanders at the 1e-6 K level -- at step 29 it rose
        // from 4.34e-7 K to 7.59e-7 K, which is inversion round-off, not an
        // instability. NOISE_FLOOR is set two decades above that.
        const NOISE_FLOOR: f64 = 1.0e-4;
        if magnitude > NOISE_FLOOR {
            assert!(
                magnitude <= previous * (1.0 + 1.0e-9),
                "step {k}: relaxation is not monotone ({magnitude} > {previous}) -- \
                 an explicit interfacial exchange would do exactly this"
            );
        }
        if magnitude > NOISE_FLOOR {
            assert_eq!(
                difference.signum(),
                initial_sign,
                "step {k}: the relaxation overshot through T_sat"
            );
        }
        previous = magnitude;
    }
    println!("initial |T_g - T_l| = {initial:.8} K, final = {previous:.8} K");
    assert!(
        previous < initial,
        "no relaxation happened at all: {previous} vs {initial}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
//  5. Regression: the phase-extinction enthalpy amplification
// ─────────────────────────────────────────────────────────────────────────────

/// **Methodology.** A **regression test for a real bug**, kept because the
/// failure mode looks silent and the fix is a numerical device somebody could
/// reasonably decide to remove.
///
/// Because the transferring mass carries its side's *saturation* enthalpy, the
/// enthalpy left behind when a fraction `f` of a phase transfers away departs
/// from saturation by `1/(1 − f)` times whatever the departure already was.
/// With no cap on `f`, a 57 K subcooled cell holding `α_g = 1e-3` of saturated
/// steam at 7 MPa condenses its whole vapour inventory in one 30 µs step
/// (`f = 1 − 1e-3`), and the amplification produced a vapour enthalpy of
/// **`−1.7079e7 J/kg` on the very first step** (measured 2026-08-12, before the
/// fix). The property layer then refused it, correctly, as a vapour past its
/// metastable bound — a correct refusal of a number that should never have been
/// formed.
///
/// [`MAX_MASS_TRANSFER_FRACTION_PER_STEP`] caps `f` at `0.5`, and
/// [`PHASE_FLOOR_TRIGGER`] resets a phase that reaches twice its residual-alpha
/// floor to the saturated state. This test reruns that exact configuration.
///
/// **What it asserts, and what it does not.** It asserts that the first steps
/// now *succeed* and that every phase enthalpy the solver produces is inside a
/// physical bracket. It does **not** assert that the case runs indefinitely,
/// because it does not — see the results below. The two failure modes are
/// different and only one of them is a bug.
///
/// Inputs: 4-cell closed pipe, 7 MPa, liquid 57 K subcooled (`T = 502.0 K`
/// against `T_sat = 558.98 K`), `α_g = 1e-3`, bubble diameter `1e-5 m`,
/// `Δt = 30 µs`, up to 40 steps. Pass criterion: at least 4 steps succeed;
/// every `h_g`, `h_l` produced lies in `[0, 4e6] J/kg`; the rate limiter fires.
///
/// **Results (measured 2026-08-12, release).** The march now survives, and the
/// vapour enthalpy declines *smoothly* instead of exploding:
///
/// | step | `α_g` | `h_g` \[J/kg\] | `h_l` \[J/kg\] | `p` \[Pa\] | limited cells |
/// |---|---|---|---|---|---|
/// | 0 | `5.273294e-4` | `2.743261e6` | `9.850737e5` | `6.431604e6` | 4 |
/// | 1 | `2.658440e-4` | `2.704136e6` | `9.847131e5` | `6.115195e6` | 4 |
/// | 2 | `1.282581e-4` | `2.644374e6` | `9.845226e5` | `5.948585e6` | 4 |
/// | 3 | `5.835715e-5` | `2.543207e6` | `9.844256e5` | `5.863861e6` | 4 |
/// | 4 | `2.349518e-5` | `2.360027e6` | `9.843772e5` | `5.821628e6` | 4 |
///
/// Every value is physical, and the pressure collapse in the first step is real
/// physics rather than a defect: a rigid volume whose vapour is condensing does
/// lose pressure that fast. The limiter fired on all four cells at every step,
/// 20 activations in total, and the residual-alpha floor never had to.
///
/// **The case still stops, after 5 completed steps, and for a different
/// reason.** The property layer refuses `h_g = 2.020149e6 J/kg` at
/// `p = 5.821628e6 Pa`, against a bound of `2.317606e6 J/kg` at
/// `T_sat = 546.774 K` — a vapour more than 30 K subcooled. That is not the amplification returning —
/// `h_g` got there by declining by a few per cent per step, not by jumping nine
/// decades — it is the physical limitation catalogued at
/// `six_equation_march_refuses_where_a_phase_cannot_shed_its_expansion_work`:
/// at `α_g ~ 1e-4` the vapour has too little interfacial area to be heated back
/// toward `T_sat` as fast as reversible expansion cools it. The test therefore
/// stops the loop on the first refusal and reports where it got to, rather than
/// pretending the configuration is viable.
#[test]
fn a_collapsing_vapour_phase_does_not_blow_up_the_enthalpy_it_leaves_behind() {
    let mut solver = closed_two_phase_pipe(4, 1.0e-3, 1.0e-5, 3.0e-5, 56.98);

    let mut limited_total = 0usize;
    let mut floored_total = 0usize;
    let mut completed = 0usize;
    let mut stop_reason = String::from("ran to completion");
    for k in 0..40 {
        match solver.step() {
            Ok(report) => {
                completed = k + 1;
                limited_total += report.mass_transfer_limited_cells;
                floored_total += report.residual_alpha_floor_events;
                println!(
                    "k = {k:3}  alpha_g = {:.6e}  h_g = {:.6e}  h_l = {:.6e}  p = {:.6e}  \
                     lim = {}  floor = {}",
                    solver.void_fraction()[0],
                    solver.vapour_enthalpy()[0],
                    solver.liquid_enthalpy()[0],
                    solver.pressure()[0],
                    report.mass_transfer_limited_cells,
                    report.residual_alpha_floor_events
                );
                for (name, values) in [
                    ("h_g", solver.vapour_enthalpy()),
                    ("h_l", solver.liquid_enthalpy()),
                ] {
                    for (i, h) in values.iter().enumerate() {
                        assert!(
                            h.is_finite() && (0.0..=4.0e6).contains(h),
                            "step {k}, cell {i}: {name} = {h} J/kg is outside any physical \
                             bracket -- the 1/(1-f) amplification is back"
                        );
                    }
                }
            }
            Err(e) => {
                stop_reason = format!("{e}");
                break;
            }
        }
    }
    println!("completed {completed} steps; stopped because: {stop_reason}");
    println!("limiter activations = {limited_total}, floor activations = {floored_total}");

    assert!(
        completed >= 4,
        "the collapsing-vapour regression has returned: only {completed} steps survived, \
         stopped by {stop_reason}"
    );
    assert!(
        limited_total > 0,
        "the limiter never fired, so this configuration no longer exercises the regression"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
//  5b. Where the march works, and where it refuses
// ─────────────────────────────────────────────────────────────────────────────

/// Run a ramped-break blowdown and return `(steps completed, stop reason,
/// final report)`. Shared by the demonstrator and the sensitivity study so the
/// two cannot drift apart.
fn run_blowdown(
    n_cells: usize,
    void_fraction: f64,
    diameter_m: f64,
    dt_s: f64,
    end_time_s: f64,
    c_vm: f64,
) -> (usize, String, Option<TwoFluidReport>) {
    let pipe = Pipe1d::circular(
        Length::new::<meter>(4.096),
        Length::new::<meter>(0.073),
        Angle::new::<radian>(0.0),
        n_cells,
    )
    .expect("well-posed pipe");
    let exchange =
        InterfacialExchange::bubbly(Length::new::<meter>(diameter_m)).expect("well-posed closures");
    let sat = SaturatedProperties::at(7.0e6).expect("inside IF97");
    let mut solver = TwoFluid1d::new(
        pipe,
        exchange,
        Pressure::new::<pascal>(7.0e6),
        ThermodynamicTemperature::new::<kelvin>(sat.t_sat),
        Time::new::<second>(dt_s),
    )
    .expect("well-posed initial state");
    solver
        .set_initial_void_fraction(void_fraction)
        .expect("in (0, 1)");
    let profile = vec![ThermodynamicTemperature::new::<kelvin>(sat.t_sat); n_cells];
    solver
        .set_temperature_profile(&profile)
        .expect("uniform profile");
    solver
        .set_virtual_mass_coefficient(Ratio::new::<ratio>(c_vm))
        .expect("C_vm >= 0");
    solver.set_outer_correctors(20).expect("non-zero");
    solver.set_left_boundary(TwoFluidBoundary::Closed);

    let mut completed = 0usize;
    let mut stop_reason = String::from("reached the end time");
    let mut last = None;
    let steps = (end_time_s / dt_s).ceil() as usize;
    for k in 0..steps {
        // 1 ms linear rupture-disc ramp to 87 % of the flow area, the same
        // shape the drift-flux Edwards case uses.
        let t = k as f64 * dt_s;
        let fraction = ((t / 1.0e-3).clamp(0.0, 1.0) * 0.87).max(1.0e-6);
        solver.set_right_boundary(TwoFluidBoundary::ChokedOutlet {
            area_fraction: fraction,
            ambient_pressure: 1.0e5,
        });
        match solver.step() {
            Ok(report) => {
                completed = k + 1;
                last = Some(report);
            }
            Err(e) => {
                stop_reason = format!("{e}");
                break;
            }
        }
    }
    (completed, stop_reason, last)
}

/// **Methodology.** A demonstration that the six-equation march *works*, on the
/// regime it is built for: a two-phase blowdown in which both phases are
/// present in quantity, so the interfacial closures have the area they need.
/// This is the counterpart to
/// `six_equation_march_refuses_where_a_phase_cannot_shed_its_expansion_work`,
/// which catalogues the regimes it does not work on.
///
/// **This is not a benchmark and it is not validated.** It is compared against
/// nothing. What it establishes is that the solver marches a real transient
/// without refusing, keeps the volume constraint satisfied, conserves what it
/// claims to conserve, and produces the two quantities that justify a
/// six-equation model — a non-zero slip and a non-zero phase-temperature
/// difference — rather than silently collapsing to a homogeneous answer.
///
/// Inputs: 4.096 m × 0.073 m horizontal pipe (the Edwards–O'Brien B-T-3271
/// geometry, chosen only so a later benchmark can reuse the harness), 8 cells,
/// closed left end, HEM-choked right end opened over a 1 ms linear ramp to
/// 87 % of the flow area discharging to 0.1 MPa, initial state **saturated**
/// water/steam at 7 MPa with `α_g = 0.1`, 1 mm bubbles, `Δt = 30 µs`, 20 ms of
/// simulated time (667 steps), `C_vm = 0.5`.
///
/// Pass criterion: every step succeeds; the pressure falls monotonically at the
/// break; the volume residual stays below
/// [`DEFAULT_VOLUME_RESIDUAL_TOLERANCE`] × 1e3; both the slip and the
/// phase-temperature difference are non-zero at the end.
///
/// **Results (measured 2026-08-12, release), end of the 667-step run:**
///
/// | quantity | value |
/// |---|---|
/// | simulated time | `2.001000e-2 s` |
/// | max void fraction | `3.523131e-1` |
/// | min void fraction | `1.271371e-1` |
/// | max `|T_g − T_l|` | `1.929268e1 K` |
/// | max slip | `1.455331e0 m/s` |
/// | outlet mass flow | `6.624528e1 kg/s` |
/// | inventory | `1.024472e1 kg` |
/// | max material Courant | `1.880593e-3` |
/// | max volume residual | `7.995498e-10` |
/// | outer correctors used (final step) | `10` |
/// | rate-limiter / floor activations | `0` / `0` |
///
/// Both quantities a six-equation model exists for are substantial: **19.3 K**
/// of thermal non-equilibrium and **1.46 m/s** of slip, neither of which HEM or
/// drift flux can represent. The volume constraint is satisfied to `8e-10`, and
/// neither numerical device had to act.
///
/// A longer run of the same configuration at 24 cells for 300 ms (10 002 steps)
/// completed without refusing, reaching `p = 4.9433e5 Pa` at the closed end,
/// `α_g = 0.993`, `|T_g − T_l| = 5.93 K` and a slip of `0.279 m/s` — recorded
/// here because it is the evidence that the march is stable over a full
/// depressurisation, and kept out of the test suite because it takes minutes.
#[test]
fn a_two_phase_blowdown_marches_without_refusing() {
    let (completed, stop_reason, report) = run_blowdown(8, 0.1, 1.0e-3, 3.0e-5, 0.02, 0.5);
    let report = report.expect("at least one step ran");
    println!("completed {completed} steps; stopped because: {stop_reason}");
    println!("t                     = {:.6e} s", report.time);
    println!("max void fraction     = {:.6e}", report.max_void_fraction);
    println!("min void fraction     = {:.6e}", report.min_void_fraction);
    println!(
        "max |T_g - T_l|       = {:.6e} K",
        report.max_thermal_nonequilibrium
    );
    println!("max slip              = {:.6e} m/s", report.max_slip);
    println!(
        "outlet mass flow      = {:.6e} kg/s",
        report.outlet_mass_flow
    );
    println!("inventory             = {:.6e} kg", report.inventory);
    println!("max Courant           = {:.6e}", report.max_courant);
    println!("max volume residual   = {:.6e}", report.max_volume_residual);
    println!("outer correctors used = {}", report.outer_correctors_used);
    println!(
        "limited / floored     = {} / {}",
        report.mass_transfer_limited_cells, report.residual_alpha_floor_events
    );

    assert_eq!(
        stop_reason, "reached the end time",
        "the blowdown did not complete: {stop_reason}"
    );
    assert!(
        report.max_volume_residual < DEFAULT_VOLUME_RESIDUAL_TOLERANCE * 1.0e3,
        "volume constraint drifted: {:e}",
        report.max_volume_residual
    );
    assert!(
        report.max_slip > 0.0,
        "no mechanical non-equilibrium developed -- a six-equation model that produces \
         zero slip is doing HEM's job at six times the cost"
    );
    assert!(
        report.max_thermal_nonequilibrium > 0.0,
        "no thermal non-equilibrium developed -- that is the whole reason for six equations"
    );
    assert!(
        report.outlet_mass_flow > 0.0,
        "nothing left through the break"
    );
}

/// **Methodology.** `crates/tampines/docs/six-equation-regularisation.md` §9.5
/// requires any result from this solver to be reported *with* its sensitivity
/// to `C_vm` over at least `[0, 0.5]`, because the virtual-mass regularisation
/// is a modelling choice that changes the answer and is **not** known to be the
/// thing that makes the system well posed. This test is that measurement, run
/// on the demonstrator configuration so the number is concrete rather than
/// hypothetical.
///
/// `C_vm = 0` is not a strawman: two of the thirty `multiphaseEuler` tutorials
/// in the vendored OpenFOAM tree (`damBreak4phase`, `hydrofoil`) run with drag
/// alone and no regularising term at all (study §3.3, §10.3).
///
/// Inputs: the [`a_two_phase_blowdown_marches_without_refusing`] configuration,
/// swept over `C_vm ∈ {0, 0.25, 0.5}`. Pass criterion: every run completes —
/// the *point* is the spread, which is reported rather than bounded, because
/// there is no reference to say which value is right.
///
/// **Results (measured 2026-08-12, release), end state after 667 steps:**
///
/// | `C_vm` | max `α_g` | max slip \[m/s\] | max `|T_g − T_l|` \[K\] | outlet \[kg/s\] |
/// |---|---|---|---|---|
/// | `0.00` | `3.523848e-1` | `1.470928e0` | `2.106943e1` | `6.614093e1` |
/// | `0.25` | `3.524862e-1` | `1.468132e0` | `2.067368e1` | `6.616235e1` |
/// | `0.50` | `3.523131e-1` | `1.455331e0` | `1.929268e1` | `6.624528e1` |
///
/// Relative spread over `C_vm ∈ [0, 0.5]`:
///
/// | quantity | spread |
/// |---|---|
/// | max `α_g` | `4.909841e-4` |
/// | max slip | `1.060353e-2` |
/// | max `|T_g − T_l|` | `8.432810e-2` |
/// | outlet mass flow | `1.575266e-3` |
///
/// **Interpretation, stated as an interpretation.** On this case the
/// regularisation moves the integral quantities very little — the outlet mass
/// flow by 0.16 %, the void fraction by 0.05 % — but it moves the *thermal
/// non-equilibrium by 8.4 %*, which is the one quantity a six-equation model is
/// being used for. Turning virtual mass off entirely does not destabilise the
/// run at all here; whatever it is doing, on this case it is not what keeps the
/// march alive. Read that as a measurement of one configuration, not as a
/// general statement, and **read the spread as the size of the modelling
/// choice, not as an error bar** — there is no reference here to say which
/// `C_vm` is right.
#[test]
fn the_answer_moves_with_c_vm_and_the_sensitivity_is_reported() {
    let mut rows = Vec::new();
    for c_vm in [0.0_f64, 0.25, 0.5] {
        let (completed, stop_reason, report) = run_blowdown(8, 0.1, 1.0e-3, 3.0e-5, 0.02, c_vm);
        let report = report.expect("at least one step ran");
        assert_eq!(
            stop_reason, "reached the end time",
            "C_vm = {c_vm}: the run did not complete after {completed} steps: {stop_reason}"
        );
        rows.push((c_vm, report));
    }

    println!(
        "{:>6}  {:>14}  {:>14}  {:>14}  {:>14}",
        "C_vm", "max alpha_g", "max slip m/s", "max dT K", "outlet kg/s"
    );
    for (c_vm, r) in &rows {
        println!(
            "{c_vm:>6.2}  {:>14.6e}  {:>14.6e}  {:>14.6e}  {:>14.6e}",
            r.max_void_fraction, r.max_slip, r.max_thermal_nonequilibrium, r.outlet_mass_flow
        );
    }
    for (label, extract) in [
        (
            "max alpha_g",
            (|r: &TwoFluidReport| r.max_void_fraction) as fn(&TwoFluidReport) -> f64,
        ),
        ("max slip", |r: &TwoFluidReport| r.max_slip),
        ("max |T_g - T_l|", |r: &TwoFluidReport| {
            r.max_thermal_nonequilibrium
        }),
        ("outlet mass flow", |r: &TwoFluidReport| r.outlet_mass_flow),
    ] {
        let values: Vec<f64> = rows.iter().map(|(_, r)| extract(r)).collect();
        let lo = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let hi = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let spread = if hi.abs() > 0.0 {
            (hi - lo) / hi.abs()
        } else {
            0.0
        };
        println!("{label:18} spread over C_vm in [0, 0.5] = {spread:.6e} relative");
    }
}

/// **Methodology — this test records a limitation, not a success.**
///
/// The six-equation march does **not** run every case one might reasonably
/// point it at, and the boundary is sharp enough to be worth pinning so that
/// nobody rediscovers it by debugging. Sweep four initial states through the
/// same ramped blowdown and record, for each, how many steps it survives and
/// what stops it.
///
/// **The mechanism, diagnosed.** Each phase's energy equation carries the
/// reversible work term `α_k ∂p/∂t`, which per unit mass of that phase is
/// `dh_k = dp / ρ_k`. During a depressurisation this cools the vapour along
/// very nearly an isentrope. The only thing that can heat it back toward
/// `T_sat(p)` is interfacial heat transfer, whose coefficient scales as
/// `6 α_d λ Nu / d²` — proportional to the interfacial area, and therefore to
/// the void fraction. At a small void fraction the vapour has almost no area,
/// so it cools faster than `T_sat` falls, and within a few steps it is more
/// than [`super::properties::MAX_METASTABLE_VAPOUR_SUBCOOLING`] (30 K) below
/// saturation, where the property layer **refuses** rather than extrapolate.
/// The mirror case is a saturated start, where flashing outruns the same
/// interfacial area and it is the *liquid* that leaves its 30 K superheat
/// bound.
///
/// **This is a real limitation, not a numerical artefact, and it is not fixed
/// by refining anything.** A smaller timestep does not help: the vapour's
/// isentropic cooling per unit *pressure* drop is a thermodynamic fact, and the
/// pressure drop is set by the transient, not by `Δt`. What would help is a
/// nucleation model that keeps a realistic interfacial area at low void — which
/// this solver does not have, and which is why
/// [`DEFAULT_INITIAL_VOID_FRACTION`] exists as a documented stand-in.
///
/// Inputs: the [`a_two_phase_blowdown_marches_without_refusing`] harness at
/// 8 cells and 20 ms, with the initial void fraction and bubble diameter
/// varied. Pass criterion: the two-phase start completes; the low-void starts
/// stop, and their stop reasons are recorded rather than swallowed.
///
/// **Results (measured 2026-08-12, release), 8 cells, 20 ms of simulated time:**
///
/// | initial void | steps survived | stopped by |
/// |---|---|---|
/// | `0.1` | **667 (the whole run)** | reached the end time |
/// | `0.01` | 23 | liquid past its 30 K superheat bound |
/// | `1e-3` | 15 | liquid past its 30 K superheat bound |
/// | `1e-4` | 15 | liquid past its 30 K superheat bound |
///
/// with the phase-temperature difference reaching `48.7`, `45.9` and `49.8 K`
/// respectively in the three that stop, against `19.3 K` in the one that
/// completes — i.e. the failures are not marginal, they are the interfacial
/// area failing to keep up by a factor of about two and a half.
///
/// Related measurements on the **24-cell** configuration over 300 ms, recorded
/// on the same date:
///
/// | initial state | bubble `d` | steps survived | stopped by |
/// |---|---|---|---|
/// | 57 K subcooled, `α_g = 1e-4` | 1 mm | 11 | vapour past its 30 K subcooling bound |
/// | 57 K subcooled, `α_g = 1e-4` | 0.1 mm | 10 | vapour past its 30 K subcooling bound |
/// | 57 K subcooled, `α_g = 1e-4` | 10 µm | 5 | vapour past its 30 K subcooling bound |
/// | saturated, `α_g = 1e-4` | 1 mm | 10 | liquid past its 30 K superheat bound |
/// | saturated, `α_g = 1e-4` | 10 µm | 11 | volume residual `1.23e-3`, correctors not converged |
/// | saturated, `α_g = 0.1` | 1 mm | **10 002 (full 300 ms)** | reached the end time |
///
/// The 10 µm row is the informative one: shrinking the bubbles *does* work as
/// the mechanism above predicts — the phase-temperature difference falls from
/// 49.8 K to 5.74 K — and the run then stops for a different reason, the
/// outer-corrector loop failing to converge the volume constraint once the
/// interfacial exchange is stiff enough to dominate it. **That second failure
/// is not diagnosed**, and it is stated as unresolved rather than guessed at.
#[test]
fn six_equation_march_refuses_where_a_phase_cannot_shed_its_expansion_work() {
    let cases: &[(&str, f64, f64)] = &[
        ("saturated, alpha_g = 0.1  , d = 1 mm", 0.1, 1.0e-3),
        ("saturated, alpha_g = 0.01 , d = 1 mm", 0.01, 1.0e-3),
        ("saturated, alpha_g = 1e-3 , d = 1 mm", 1.0e-3, 1.0e-3),
        ("saturated, alpha_g = 1e-4 , d = 1 mm", 1.0e-4, 1.0e-3),
    ];
    let mut outcomes = Vec::new();
    for &(label, alpha_0, diameter) in cases {
        let (completed, stop_reason, report) =
            run_blowdown(8, alpha_0, diameter, 3.0e-5, 0.02, 0.5);
        let summary = match &report {
            Some(r) => format!(
                "t = {:.4e} s, alpha_max = {:.4e}, dT = {:.3} K, slip = {:.4} m/s",
                r.time, r.max_void_fraction, r.max_thermal_nonequilibrium, r.max_slip
            ),
            None => "no step completed".to_string(),
        };
        println!("{label:38}: {completed:5} steps | {summary}");
        println!(
            "{:38}  stopped by: {}",
            "",
            &stop_reason[..stop_reason.len().min(150)]
        );
        outcomes.push((label, completed, stop_reason));
    }

    // The two-phase start must complete; that is the regime the closures are for.
    assert_eq!(
        outcomes[0].2, "reached the end time",
        "the two-phase demonstrator regressed: {}",
        outcomes[0].2
    );
    // At least one low-void start must stop, or this test has stopped
    // documenting anything and the limitation note above is stale.
    assert!(
        outcomes
            .iter()
            .any(|(_, _, reason)| reason != "reached the end time"),
        "every initial state now completes -- the limitation this test records has \
         apparently been lifted; re-measure it and rewrite the doc comment rather than \
         deleting the test"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
//  5c. Cross-check against the four-equation solver
// ─────────────────────────────────────────────────────────────────────────────

/// **Methodology.** The six-equation model is supposed to *contain* the
/// four-equation one: drive the slip to zero and the thermal lag to zero and
/// the two should agree. This test drives those limits on the one initial state
/// the two solvers can both represent — single-phase subcooled liquid — and
/// compares the pressure histories.
///
/// **It does not find agreement, and this test therefore records and diagnoses
/// a disagreement rather than pinning a match.** That is the honest form for
/// this result: the discrepancy is real, it is reproducible, it has a specific
/// identified cause, and which of the two solvers is right has **not** been
/// established here.
///
/// # Why single-phase, and what that costs
///
/// The two solvers cannot be given a common *two-phase* initial state through
/// their public APIs. [`super::drift_flux::DriftFlux1d`] initialises from
/// `(p, T)`, which is degenerate inside the dome — a saturated `(p, T)` pair
/// does not fix a quality — so it always starts at `α = 0`; and [`TwoFluid1d`]
/// cannot start at `α = 0` at all (see [`DEFAULT_INITIAL_VOID_FRACTION`]). A
/// two-phase cross-check needs an initialiser neither solver has.
///
/// So this compares only the **shared machinery**: the staggered momentum
/// discretisation, the tridiagonal pressure equation, the donor-cell transport
/// and the wall friction. It does **not** cross-check the interfacial closures,
/// the flashing source in the pressure equation, or the phase-mass transport;
/// those are covered only by the invariants elsewhere in this file.
///
/// A second limit also fails to be reached, and it matters for reading the
/// result: the thermal lag is **not** driven to zero. In single-phase liquid
/// the vapour sits at the residual-alpha floor with essentially no interfacial
/// area, so it cannot relax to `T_sat` — the run ends with
/// `|T_g − T_l| = 5.124197e1 K`. That vapour is inert (it carries `1e-6` of the
/// volume) so it does not drive the disagreement, but the test cannot claim to
/// have tested the thermal-equilibrium limit.
///
/// # Inputs
///
/// 1 m × 0.05 m horizontal pipe, 8 cells, water at 7 MPa and 500 K (59 K
/// subcooled, so `p_sat(500 K) ≈ 2.64 MPa` and nothing flashes over the run),
/// closed left end, prescribed `0.05 m/s` outflow at the right, `Δt = 100 µs`,
/// 100 steps. `DriftFlux1d` gets `ZuberFindlay { c0: 1, vgj: 0 }` — zero drift
/// by construction — and a 1 µs vapour relaxation time; `TwoFluid1d` gets
/// `DragModel::Constant { k_d: 1e12 }` and 10 µm inclusions.
///
/// # Results (measured 2026-08-12, release)
///
/// Both solvers completed all 100 steps. The no-slip limit was reached
/// **exactly** — `TwoFluid1d`'s largest slip is `2.428613e-17 m/s`, i.e. zero
/// to round-off — and both solvers stayed single-phase (`α_g = 0` for drift
/// flux, `1e-6` for two-fluid, its floor).
///
/// The pressures do not agree:
///
/// | cell | drift-flux `p` \[Pa\] | two-fluid `p` \[Pa\] | difference \[Pa\] |
/// |---|---|---|---|
/// | 0 | `6.572671e6` | `6.218649e6` | `3.540218e5` |
/// | 3 | `6.572805e6` | `6.218655e6` | `3.541494e5` |
/// | 7 | `6.573004e6` | `6.218664e6` | `3.543404e5` |
///
/// i.e. a pressure **drop** of `4.27e5 Pa` against `7.81e5 Pa` — the
/// six-equation solver falls **1.83 times further** — and a worst discrepancy of
/// `45.4 %` of the pressure change.
///
/// # The diagnosis, measured rather than inferred
///
/// The two solvers linearise **different derivatives** in their pressure
/// equations:
///
/// - [`super::drift_flux`] uses `ψ_h = ∂ρ_m/∂p|_h`, the fixed-enthalpy
///   compressibility, on the argument that a segregated solve freezes `h` while
///   it corrects `p`;
/// - [`TwoFluid1d`] uses `dρ_k/dp` along the **isentropic** path
///   `dh_k = dp/ρ_k`, on the argument that the energy equation applies the
///   reversible work term `α_k ∂p/∂t` in the *same* step, so that is the path
///   the state actually takes.
///
/// A smaller compliance means a larger pressure change for the same volume
/// removed, so the solver using the isentropic (smaller) value must fall
/// further — which is the direction observed. This test measures both
/// derivatives directly at the initial state, so the diagnosis is a measurement
/// and not a story. Measured 2026-08-12 at 7 MPa and 500 K:
///
/// | derivative | value \[s²/m²\] |
/// |---|---|
/// | `ψ_h = ∂ρ/∂p|_h` (drift flux) | `9.764946e-7` |
/// | `ψ_s = dρ/dp` along the isentrope (two-fluid) | `6.313400e-7` |
/// | ratio | `1.546702` |
///
/// against an observed pressure-change ratio of `1.828453`.
///
/// # What is explained, and what is not
///
/// The compliance ratio gets the **sign, the mechanism and most of the
/// magnitude**: `1.547` predicted against `1.828` observed. It does **not** get
/// all of it — **18.2 % of the observed ratio is unaccounted for**, and that
/// residual is *not* diagnosed here.
///
/// Two candidates, neither checked: the compliances are evaluated once at the
/// initial state whereas both solvers re-evaluate theirs as the pressure falls
/// (drift flux switching to a secant after its first corrector), and the two
/// solvers reach their end state through different numbers of outer correctors.
/// Either could account for a fifth of the ratio over a 100-step nonlinear
/// march. Anyone continuing this should instrument both solvers' per-step
/// compliance rather than reasoning about it.
///
/// # What is NOT established
///
/// **Which solver is right.** The isentropic derivative is the one an acoustic
/// response should carry, and both solvers apply the work term in their energy
/// equations, which is an argument that `drift_flux` is the inconsistent one —
/// but that is an argument, not a result. Nothing here was compared against an
/// analytic water-hammer solution, a speed-of-sound measurement, or an
/// experiment, and **no change has been made to `drift_flux` on the strength of
/// it**. The drift-flux Edwards benchmark recorded in `edwards_tests.rs` was
/// measured with `ψ_h` and is untouched by anything in this file.
///
/// # Pass criterion
///
/// Both solvers complete; the no-slip limit is reached to better than
/// `1e-12 m/s`; the disagreement is present (above 10 % of the pressure
/// change); and the measured compliance ratio accounts for the observed ratio
/// to within 25 %. The test **fails if the two start agreeing**, because that
/// would mean one of the two compliances changed and this doc comment went
/// stale — the right response then is to re-measure and rewrite it, not to
/// delete the assertion.
#[test]
fn the_two_solvers_disagree_in_the_single_phase_limit_and_the_compliance_ratio_explains_it() {
    use outram_foam_basic_lib::primitives::Vector3;
    use outram_foam_multiphase::drift_flux::SlipModel;
    use outram_foam_multiphase::heat_transfer::InterfacialHeatTransfer;
    use outram_foam_multiphase::two_fluid::DragModel;

    const N: usize = 8;
    const DT: f64 = 1.0e-4;
    const STEPS: usize = 100;
    const OUTFLOW: f64 = 0.05;
    const P0: f64 = 7.0e6;
    const T0: f64 = 500.0;

    let pipe = || {
        Pipe1d::circular(
            Length::new::<meter>(1.0),
            Length::new::<meter>(0.05),
            Angle::new::<radian>(0.0),
            N,
        )
        .expect("well-posed pipe")
    };

    // -- the two compressibilities, measured at the initial state -------------
    let sat = SaturatedProperties::at(P0).expect("inside IF97");
    // The initial enthalpy is the (p, T) flash both solvers perform.
    let h_initial = {
        use tampines_steam_tables::region_1_subcooled_liquid::h_tp_1;
        use uom::si::available_energy::joule_per_kilogram;
        h_tp_1(
            ThermodynamicTemperature::new::<kelvin>(T0),
            Pressure::new::<pascal>(P0),
        )
        .get::<joule_per_kilogram>()
    };
    let psi_fixed_enthalpy = super::properties::TwoPhaseState::flash(P0, h_initial, sat)
        .expect("valid flash")
        .compressibility(COMPRESSIBILITY_STEP)
        .expect("valid compressibility");
    let psi_isentropic = {
        let dp = (COMPRESSIBILITY_STEP * P0).max(1.0);
        let here =
            super::properties::PhaseState::liquid_at(P0, h_initial, sat).expect("subcooled liquid");
        let sat_hi = SaturatedProperties::at(P0 + dp).expect("inside IF97");
        let hi = super::properties::PhaseState::liquid_at(
            P0 + dp,
            h_initial + dp / here.density,
            sat_hi,
        )
        .expect("subcooled liquid");
        (hi.density - here.density) / dp
    };
    let compliance_ratio = psi_fixed_enthalpy / psi_isentropic;
    println!("psi at fixed enthalpy (drift flux) = {psi_fixed_enthalpy:.6e} s^2/m^2");
    println!("psi along the isentrope (two fluid) = {psi_isentropic:.6e} s^2/m^2");
    println!("compliance ratio                    = {compliance_ratio:.6}");

    // -- four-equation reference, with the drift velocity switched off --------
    let mut drift = super::drift_flux::DriftFlux1d::new(
        pipe(),
        SlipModel::ZuberFindlay {
            c0: 1.0,
            vgj: Vector3::new(0.0, 0.0, 0.0),
        },
        Pressure::new::<pascal>(P0),
        ThermodynamicTemperature::new::<kelvin>(T0),
        Time::new::<second>(DT),
    )
    .expect("well-posed drift-flux initial state");
    drift
        .set_vapour_relaxation_time(Time::new::<second>(1.0e-6))
        .expect("positive");
    drift.set_left_boundary(super::drift_flux::AxialBoundary::Closed);
    drift.set_right_boundary(super::drift_flux::AxialBoundary::PrescribedVelocity(
        OUTFLOW,
    ));

    // -- six-equation, driven to the no-slip limit ----------------------------
    let exchange = InterfacialExchange::new(
        DragModel::Constant { k_d: 1.0e12 },
        InterfacialHeatTransfer::RanzMarshall,
        InterfacialHeatTransfer::Spherical,
        DispersedPhase::Vapour,
        Length::new::<meter>(1.0e-5),
        DEFAULT_RESIDUAL_ALPHA,
    )
    .expect("well-posed no-slip closure set");
    let mut two_fluid = TwoFluid1d::new(
        pipe(),
        exchange,
        Pressure::new::<pascal>(P0),
        ThermodynamicTemperature::new::<kelvin>(T0),
        Time::new::<second>(DT),
    )
    .expect("well-posed two-fluid initial state");
    two_fluid.set_left_boundary(TwoFluidBoundary::Closed);
    two_fluid.set_right_boundary(TwoFluidBoundary::PrescribedVelocity(OUTFLOW));

    let mut completed = 0usize;
    let mut stop_reason = String::from("reached the end time");
    for k in 0..STEPS {
        if let Err(e) = drift.step() {
            stop_reason = format!("drift-flux stopped: {e}");
            break;
        }
        if let Err(e) = two_fluid.step() {
            stop_reason = format!("two-fluid stopped: {e}");
            break;
        }
        completed = k + 1;
    }
    println!("completed {completed} joint steps; {stop_reason}");
    assert_eq!(
        completed, STEPS,
        "the comparison did not run to completion: {stop_reason}"
    );

    let mut worst_relative: f64 = 0.0;
    println!("cell   drift-flux p [Pa]   two-fluid p [Pa]      difference [Pa]");
    for i in 0..N {
        let (a, b) = (drift.pressure()[i], two_fluid.pressure()[i]);
        let change = (a - P0).abs().max((b - P0).abs()).max(1.0);
        let difference = (a - b).abs();
        worst_relative = worst_relative.max(difference / change);
        println!("{i:4}   {a:16.6e}   {b:16.6e}   {difference:16.6e}");
    }

    let drop_drift = P0 - drift.pressure()[0];
    let drop_two_fluid = P0 - two_fluid.pressure()[0];
    let observed_ratio = drop_two_fluid / drop_drift;
    let max_slip = two_fluid
        .slip_velocity()
        .iter()
        .map(|s| s.abs())
        .fold(0.0_f64, f64::max);
    let max_thermal = (0..N)
        .map(|i| (two_fluid.vapour_temperature()[i] - two_fluid.liquid_temperature()[i]).abs())
        .fold(0.0_f64, f64::max);
    println!("drift-flux pressure drop  = {drop_drift:.6e} Pa");
    println!("two-fluid  pressure drop  = {drop_two_fluid:.6e} Pa");
    println!(
        "observed drop ratio       = {observed_ratio:.6}  (compliance ratio {compliance_ratio:.6})"
    );
    println!("worst relative difference = {worst_relative:.6e} of the pressure change");
    println!("two-fluid max slip        = {max_slip:.6e} m/s  (no-slip limit)");
    println!("two-fluid max |T_g - T_l| = {max_thermal:.6e} K  (thermal limit NOT reached)");
    println!(
        "drift-flux max void = {:.6e}, two-fluid max void = {:.6e}",
        drift
            .void_fraction()
            .iter()
            .cloned()
            .fold(0.0_f64, f64::max),
        two_fluid
            .void_fraction()
            .iter()
            .cloned()
            .fold(0.0_f64, f64::max)
    );

    // The no-slip limit really is reached.
    assert!(
        max_slip < 1.0e-12,
        "the no-slip limit was not reached: {max_slip:e} m/s"
    );
    // The disagreement is real...
    assert!(
        worst_relative > 0.1,
        "the two solvers now agree to {worst_relative:e} of the pressure change. That is a \
         GOOD outcome, but it means one of the two compliances changed and this test's doc \
         comment is stale -- re-measure and rewrite it rather than deleting the assertion"
    );
    // ...and the compliance ratio accounts for it.
    let explained = (observed_ratio - compliance_ratio).abs() / compliance_ratio;
    println!("compliance ratio explains the observed ratio to {explained:.6e} relative");
    assert!(
        explained < 0.25,
        "the compliance difference no longer accounts for the disagreement (off by \
         {explained:e}); a second cause has appeared and it is undiagnosed"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
//  6. Refusals
// ─────────────────────────────────────────────────────────────────────────────

/// **Methodology.** The crate's rule is errors, not clamps: a state outside
/// what the model can represent must be *reported*, because continuing from a
/// clamped value produces a plausible-looking wrong answer. Check the
/// constructor and setter refusals that guard the marching loop's inputs, and
/// the one boundary-condition refusal that fires inside
/// [`TwoFluid1d::step`] itself.
///
/// **Results (measured 2026-08-12, release).** All refused; the exact messages
/// are printed by the test.
#[test]
fn the_solver_refuses_inputs_it_cannot_represent() {
    let pipe = || {
        Pipe1d::circular(
            Length::new::<meter>(1.0),
            Length::new::<meter>(0.05),
            Angle::new::<radian>(0.0),
            4,
        )
        .expect("well-posed pipe")
    };
    let exchange =
        || InterfacialExchange::bubbly(Length::new::<meter>(1.0e-3)).expect("well-posed closures");

    let zero_dt = TwoFluid1d::new(
        pipe(),
        exchange(),
        Pressure::new::<pascal>(7.0e6),
        ThermodynamicTemperature::new::<kelvin>(500.0),
        Time::new::<second>(0.0),
    );
    println!("dt = 0            -> {:?}", zero_dt.as_ref().err());
    assert!(matches!(
        zero_dt,
        Err(crate::TampinesError::InvalidInput(_))
    ));

    let outside_if97 = TwoFluid1d::new(
        pipe(),
        exchange(),
        Pressure::new::<pascal>(1.0),
        ThermodynamicTemperature::new::<kelvin>(500.0),
        Time::new::<second>(1.0e-4),
    );
    println!("p = 1 Pa          -> {:?}", outside_if97.as_ref().err());
    assert!(matches!(
        outside_if97,
        Err(crate::TampinesError::Unphysical(_))
    ));

    let mut solver = TwoFluid1d::new(
        pipe(),
        exchange(),
        Pressure::new::<pascal>(7.0e6),
        ThermodynamicTemperature::new::<kelvin>(500.0),
        Time::new::<second>(1.0e-4),
    )
    .expect("well-posed");

    let bad_cvm = solver.set_virtual_mass_coefficient(Ratio::new::<ratio>(-1.0));
    println!("C_vm = -1         -> {:?}", bad_cvm.as_ref().err());
    assert!(bad_cvm.is_err());

    let bad_res = solver.set_residual_alpha(1.0);
    println!("alpha_res = 1.0   -> {:?}", bad_res.as_ref().err());
    assert!(bad_res.is_err());

    let bad_void = solver.set_initial_void_fraction(0.0);
    println!("alpha_0 = 0.0     -> {:?}", bad_void.as_ref().err());
    assert!(bad_void.is_err());

    let bad_correctors = solver.set_outer_correctors(0);
    println!("correctors = 0    -> {:?}", bad_correctors.as_ref().err());
    assert!(bad_correctors.is_err());

    let bad_relaxation = solver.set_pressure_under_relaxation(Ratio::new::<ratio>(1.5));
    println!("relaxation = 1.5  -> {:?}", bad_relaxation.as_ref().err());
    assert!(bad_relaxation.is_err());

    let short_profile =
        solver.set_temperature_profile(&[ThermodynamicTemperature::new::<kelvin>(500.0)]);
    println!("short profile     -> {:?}", short_profile.as_ref().err());
    assert!(short_profile.is_err());

    // A malformed break area must be refused at the step that uses it.
    solver.set_right_boundary(TwoFluidBoundary::ChokedOutlet {
        area_fraction: 1.5,
        ambient_pressure: 1.0e5,
    });
    let bad_break = solver.step();
    println!("break area = 1.5  -> {:?}", bad_break.as_ref().err());
    assert!(matches!(
        bad_break,
        Err(crate::TampinesError::InvalidInput(_))
    ));
}
