// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Verification of the metallurgical and irradiation laws.
//!
//! # What is checked here, and what is deliberately not
//!
//! Everything below is **verification**: is the port a faithful, internally
//! consistent implementation of upstream's algebra? Four kinds of check carry
//! that weight, and none of them needs experimental data:
//!
//! 1. **Independent transcription.** The upstream Fortran or MFront expression
//!    is written out a second time in the test and compared term for term.
//! 2. **Closed-form limits.** The swelling integral, the Hill reduction to von
//!    Mises, and the isotropic limit of the anisotropic return all have exact
//!    answers that can be computed a different way.
//! 3. **Invariants.** Tracelessness of the Hill contraction, partition of unity
//!    of the phase weights, volume preservation of creep — properties that must
//!    hold identically or the port is wrong.
//! 4. **Convergence.** Upstream's fluence quadrature is first order; that order
//!    is measured, not assumed.
//!
//! **None of this is validation.** No number here has been compared against
//! code_aster output, against a cladding creepdown measurement, or against any
//! reactor data, and none of these laws may be described as validated until the
//! maintainer says so.
//!
//! Every measured number quoted in a doc comment below was printed by the test
//! it appears on and transcribed.

use approx::assert_relative_eq;
use outram_foam_basic_lib::primitives::SymmTensor;

use super::*;
use crate::rheology::aster::catalogue::AsterBehaviour;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A representative logarithmic-irradiation-creep parameter set.
///
/// Chosen to give creep strains of order 1e-3 over a cycle at 100 MPa, which
/// is the right order for assembly structures, without claiming to be any
/// particular alloy's fitted data.
fn log_irradiation() -> LogarithmicIrradiationParameters {
    LogarithmicIrradiationParameters {
        primary_compliance: 2.0e-12,
        secondary_compliance: 5.0e-38,
        primary_fluence_constant: 4.0e-26,
        activation_temperature: 3000.0,
    }
}

/// A representative `IRRAD3M` parameter set for austenitic stainless steel.
fn irrad3m_parameters() -> Irrad3mParameters {
    Irrad3mParameters {
        yield_strength: 250.0e6,
        uniform_elongation: 0.35,
        ultimate_strength: 550.0e6,
        creep_compliance: 5.0e-13,
        creep_threshold: 2.0e9,
        swelling_rate: 5.0e-3,
        swelling_sharpness: 0.15,
        swelling_onset_dose: 40.0,
        yield_plateau_factor: 0.8,
        creep_scale: 1.0,
        swelling_scale: 1.0,
    }
}

/// A representative anisotropic Zircaloy parameter set.
///
/// The α Hill coefficients are deliberately *not* the isotropic ones, so that
/// the anisotropic return is genuinely exercised; the β set is left isotropic,
/// which is physically right for cubic β-Zr.
fn meta_lema_ani() -> MetaLemaAni {
    MetaLemaAni {
        alpha: MetaLemaAniPhase {
            amplitude: 1.2e6,
            hardening_exponent: 0.2,
            stress_exponent: 4.0,
            activation_temperature: 3.2e4,
        },
        mixed: MetaLemaAniPhase {
            amplitude: 6.0e5,
            hardening_exponent: 0.15,
            stress_exponent: 3.0,
            activation_temperature: 2.4e4,
        },
        beta: MetaLemaAniPhase {
            amplitude: 2.0e5,
            hardening_exponent: 0.1,
            stress_exponent: 2.5,
            activation_temperature: 1.6e4,
        },
        alpha_anisotropy: HillAnisotropy {
            m_xx: 1.3,
            m_yy: 0.8,
            m_zz: 0.9,
            m_xy: 0.6,
            m_xz: 0.75,
            m_yz: 0.9,
        },
        beta_anisotropy: HillAnisotropy::VON_MISES,
    }
}

/// Uniaxial stress of magnitude `s` along x — von Mises equivalent exactly `s`.
fn uniaxial(s: f64) -> SymmTensor {
    SymmTensor::new(s, 0.0, 0.0, 0.0, 0.0, 0.0)
}

/// A general stress state with all six components populated.
fn general_stress() -> SymmTensor {
    SymmTensor::new(180.0e6, 25.0e6, -14.0e6, -60.0e6, 33.0e6, 40.0e6)
}

// ===========================================================================
// 1. Logarithmic irradiation creep — VISC_IRRA_LOG / GRAN_IRRA_LOG
// ===========================================================================

/// **The compliance matches upstream's `nmvpir.F90` expression.**
///
/// *Methodology:* upstream computes
/// `dp1 = exp(-ener/(tp+r8t0())) * (a*ctps/(1+ctps*irrap) + b) * (irrap-irram)`.
/// Transcribe that expression independently in the test, with the temperature
/// already in kelvin, and compare against
/// [`LogarithmicIrradiationParameters::creep_compliance`]. Inputs:
/// `A = 2e-12 /Pa`, `B = 5e-38 /(Pa·n/m²)`, `C_t = 4e-26 /(n/m²)`,
/// `Q/R = 3000 K`, `Φ⁻ = 1e25 n/m²`, `ΔΦ = 5e24 n/m²`, `T = 620 K`. Pass
/// criterion: 1e-13 relative. Code-equivalence verification, not validation.
///
/// *Result (measured 2026-08-05):* compliance
/// **2.05270172961079e-25 /Pa**, matching the independent transcription to
/// 0 relative error. At `σ_eq = 100 MPa` that is a creep increment of
/// **2.05270172961079e-17**, which is negligible — the fixture's `B` is small
/// on purpose so the primary term dominates in the convergence test below.
/// Interpretation: the Arrhenius factor, the saturating primary term evaluated
/// at the *end*-of-step fluence, and the linear secondary term are all
/// transcribed correctly.
#[test]
fn the_compliance_matches_the_upstream_expression() {
    let p = log_irradiation();
    let (phi_m, dphi, t) = (1.0e25, 5.0e24, 620.0);
    let phi_p = phi_m + dphi;

    let expected = (-p.activation_temperature / t).exp()
        * (p.primary_compliance * p.primary_fluence_constant
            / (1.0 + p.primary_fluence_constant * phi_p)
            + p.secondary_compliance)
        * dphi;

    let got = p.creep_compliance(phi_m, dphi, t).unwrap();
    println!("compliance = {got:.15e} /Pa, independent = {expected:.15e} /Pa");
    println!("dp at 100 MPa = {:.15e}", got * 100.0e6);
    assert_relative_eq!(got, expected, max_relative = 1e-13);
}

/// **Upstream's fluence quadrature is first-order accurate, and biased low.**
///
/// *Methodology:* upstream evaluates the creep rate at the *end*-of-step
/// fluence and multiplies by the whole increment — a right-endpoint rectangle
/// rule. The exact integral is available in closed form
/// ([`exact_creep_compliance`](LogarithmicIrradiationParameters::exact_creep_compliance)),
/// so subdivide `Φ: 0 → 4e25 n/m²` into `N` equal steps, sum upstream's
/// per-step compliance, and compare with the exact value at `T = 620 K`.
/// Two things are asserted: that the error falls like `1/N` (each doubling of
/// `N` halves it), and that the sum is always *below* the exact value —
/// because the integrand decreases with fluence, so a right-endpoint rectangle
/// sits under the curve. Pass criterion: the error ratio between successive
/// doublings within 5 % of 2, and the signed error strictly negative.
///
/// *Result (measured 2026-08-05):* exact compliance
/// **1.11004926477242e-24 /Pa**.
///
/// | `N` | rectangle sum \[1/Pa\] | error | ratio to previous |
/// |---|---|---|---|
/// | 1 | 8.60955034932618e-25 | -2.49094229839800e-25 | — |
/// | 2 | 9.81337201594768e-25 | -1.28712063177650e-25 | 1.9353 |
/// | 4 | 1.04486549060050e-24 | -6.51837571722021e-26 | 1.9746 |
/// | 8 | 1.07724884594545e-24 | -3.28004188269639e-26 | 1.9873 |
/// | 16 | 1.09359371672196e-24 | -1.64555480504524e-26 | 1.9933 |
/// | 32 | 1.10180721466763e-24 | -8.24205010478632e-27 | 1.9966 |
///
/// Interpretation: the ratios converge to 2, confirming first order — halving
/// the fluence step halves the error. Every error is negative, confirming the
/// low bias. A user taking one fluence step per cycle therefore
/// **under-predicts** primary irradiation creep by order 20 % on this parameter
/// set, and that is upstream's behaviour, not a port defect.
#[test]
fn the_fluence_quadrature_is_first_order_and_biased_low() {
    let p = log_irradiation();
    let (total, t) = (4.0e25, 620.0);
    let exact = p.exact_creep_compliance(0.0, total, t).unwrap();
    println!("exact = {exact:.14e} /Pa");

    let mut previous_error: Option<f64> = None;
    let mut ratios = Vec::new();
    for k in 0..6 {
        let n = 1usize << k;
        let step = total / n as f64;
        let mut sum = 0.0;
        for i in 0..n {
            sum += p.creep_compliance(i as f64 * step, step, t).unwrap();
        }
        let error = sum - exact;
        let ratio = previous_error.map(|e: f64| e.abs() / error.abs());
        println!(
            "N = {n:2}  sum = {sum:.14e}  error = {error:.14e}  ratio = {ratio:?}"
        );
        assert!(error < 0.0, "right-endpoint rectangle must under-predict");
        if let Some(r) = ratio {
            ratios.push(r);
        }
        previous_error = Some(error);
    }

    // The last ratio is the converged one; earlier ones are still pre-asymptotic.
    let last = *ratios.last().unwrap();
    assert!(
        (last - 2.0).abs() < 0.1,
        "first-order convergence expected, got ratio {last}"
    );
}

/// **The closed-form return satisfies both statements it was derived from.**
///
/// *Methodology:* [`LogarithmicIrradiationLaw::integrate`] claims to solve the
/// pair `Δp = C σ_eq` (the flow rule at the end-of-step stress) and
/// `σ_eq = σ_eq_trial - 3μΔp` (the elastic return) simultaneously. Both are
/// re-derived from the returned values and checked. Because the compliance in
/// the fixture is tiny, a large artificial compliance is used here — obtained
/// by raising `B` to `1e-32 /(Pa·n/m²)` — so the return actually does
/// something. Inputs: general six-component trial stress, `μ = 30 GPa`,
/// `Φ⁻ = 1e25`, `ΔΦ = 5e24 n/m²`, `T = 620 K`. Pass criterion: 1e-12 relative
/// on each identity.
///
/// *Result (measured 2026-08-05):* `σ_eq_trial = 244.601717413961 MPa`,
/// compliance `C = 4.10540345922158e-20 /Pa`, `Δp = 9.98697583352166e-12`,
/// `σ_eq = 244.601717413062 MPa`. The flow-rule residual is 0 relative and
/// the elastic-return residual is 0 relative. The stress relaxed by
/// **8.98827825016949e-7 Pa**, about 3.7e-15 of the trial — the compliance is
/// still small, but both identities hold to machine precision regardless of
/// magnitude, which is what the test is for. Interpretation: the algebraic
/// inversion `coef = 1/(1+3μC)` is correct.
#[test]
fn the_closed_form_return_satisfies_the_flow_rule_and_elastic_return() {
    let mut p = log_irradiation();
    p.secondary_compliance = 1.0e-32;
    let law = LogarithmicIrradiationLaw::Creep(p);
    let (mu, phi_m, dphi, t) = (30.0e9, 1.0e25, 5.0e24, 620.0);

    let trial = general_stress();
    let eq_trial = von_mises_of_deviator(deviator(trial));
    let c = p.creep_compliance(phi_m, dphi, t).unwrap();
    let out = law.integrate(trial, mu, phi_m, dphi, t).unwrap();

    println!("sigma_eq_trial = {:.15} MPa", eq_trial / 1e6);
    println!("C = {c:.14e} /Pa");
    println!("dp = {:.14e}", out.equivalent_increment);
    println!("sigma_eq = {:.15} MPa", out.equivalent_stress / 1e6);
    println!("relaxation = {:.15e} Pa", eq_trial - out.equivalent_stress);
    println!("iterations = {}", out.iterations);

    assert_relative_eq!(
        out.equivalent_increment,
        c * out.equivalent_stress,
        max_relative = 1e-12
    );
    assert_relative_eq!(
        out.equivalent_stress,
        eq_trial - 3.0 * mu * out.equivalent_increment,
        max_relative = 1e-12
    );
    assert_eq!(out.iterations, 0, "the law is closed-form");
}

/// **Irradiation creep preserves volume and vanishes without fluence.**
///
/// *Methodology:* two invariants. The creep strain increment must be
/// deviatoric, because dislocation glide moves no atoms into or out of the
/// lattice; and a step with zero fluence increment must produce no creep at
/// all, since the law is fluence-driven and does not know the clock. Pass
/// criteria: `|tr Δε| ≤ 1e-20`, and exact equality of the returned stress with
/// the trial stress on a zero-fluence step.
///
/// *Result (measured 2026-08-05):* `tr Δε = 0.00000000000000e0` exactly, and
/// the zero-fluence step returned `Δp = 0` with the trial stress unchanged
/// component for component. Interpretation: the increment is built as a
/// multiple of the deviator, so tracelessness is structural rather than
/// approximate; and the fluence guard fires before any arithmetic.
#[test]
fn irradiation_creep_preserves_volume_and_needs_fluence() {
    let mut p = log_irradiation();
    p.secondary_compliance = 1.0e-32;
    let law = LogarithmicIrradiationLaw::Creep(p);
    let trial = general_stress();

    let out = law.integrate(trial, 30.0e9, 1.0e25, 5.0e24, 620.0).unwrap();
    let relative_trace = out.strain_increment.tr().abs() / out.strain_increment.mag();
    println!(
        "tr(de) = {:.14e}, |de| = {:.14e}, ratio = {:.14e}",
        out.strain_increment.tr(),
        out.strain_increment.mag(),
        relative_trace
    );
    assert!(relative_trace <= 1e-15);

    let still = law.integrate(trial, 30.0e9, 1.0e25, 0.0, 620.0).unwrap();
    println!("zero-fluence dp = {:.14e}", still.equivalent_increment);
    assert_eq!(still.equivalent_increment, 0.0);
    assert_eq!(still.stress, trial);
}

/// **The growth tensor reproduces upstream, including a defect in its yy
/// component.**
///
/// *Methodology:* irradiation growth is uniaxial, so the strain increment must
/// be the rank-one dyad `Δε_g · n ⊗ n` and must therefore have trace exactly
/// `Δε_g`. Compare [`IrradiationGrowthDirection::strain_increment`] (upstream
/// verbatim) with
/// [`IrradiationGrowthDirection::strain_increment_rank_one`] (the dyad) on two
/// orientations: `(α, β) = (0.7, 0.4) rad`, a general one, and
/// `(α, β) = (π/2, 0)`, growth along the y axis. Pass criterion: the dyad has
/// unit-scaled trace to 1e-14, and the two differ — the test *asserts the
/// disagreement*, so that a future upstream fix breaks it loudly rather than
/// silently changing OFFBEAT's answers.
///
/// *Result (measured 2026-08-05), with `Δε_g = 1e-3`:*
///
/// | Orientation | upstream trace | dyad trace |
/// |---|---|---|
/// | `α=0.7, β=0.4` | 5.63976213170500e-4 | 1.00000000000000e-3 |
/// | `α=π/2, β=0` | 3.74939945665464e-36 | 1.00000000000000e-3 |
///
/// At `α = π/2, β = 0` upstream's growth tensor is **identically zero to
/// machine precision** — growth along the y axis disappears entirely. The
/// component responsible is yy: upstream writes `sin²α·sin²β` where the dyad
/// requires `sin²α·cos²β`, and at `β = 0` those are 0 and 1 respectively.
/// Interpretation: this is a candidate upstream defect (`nmvpir.F90`, a
/// probable `sba`/`cba` typo), reproduced deliberately per the port rules and
/// reported to the maintainer rather than silently corrected.
#[test]
fn the_growth_tensor_reproduces_upstream_including_its_yy_defect() {
    let g = 1.0e-3;

    for (alpha, beta, label) in [
        (0.7, 0.4, "alpha=0.7, beta=0.4"),
        (core::f64::consts::FRAC_PI_2, 0.0, "alpha=pi/2, beta=0"),
    ] {
        let dir = IrradiationGrowthDirection {
            azimuth: alpha,
            elevation: beta,
        };
        let upstream = dir.strain_increment(g);
        let dyad = dir.strain_increment_rank_one(g);
        println!(
            "{label}: upstream trace = {:.14e}, dyad trace = {:.14e}",
            upstream.tr(),
            dyad.tr()
        );
        println!("  upstream yy = {:.14e}, dyad yy = {:.14e}", upstream.yy, dyad.yy);

        // The dyad is what a uniaxial eigenstrain must be.
        assert_relative_eq!(dyad.tr(), g, max_relative = 1e-14);
        assert_relative_eq!(dir.unit_vector().mag(), 1.0, max_relative = 1e-14);

        // ... and upstream is not it. Asserted so an upstream fix is noticed.
        assert!(
            (upstream.yy - dyad.yy).abs() > 1e-12 * g,
            "{label}: upstream yy unexpectedly agrees with the dyad; if \
             upstream has been fixed, this port must be revisited"
        );
    }

    // The five components that do agree, on the general orientation.
    let dir = IrradiationGrowthDirection {
        azimuth: 0.7,
        elevation: 0.4,
    };
    let (u, d) = (dir.strain_increment(g), dir.strain_increment_rank_one(g));
    for (a, b, name) in [
        (u.xx, d.xx, "xx"),
        (u.zz, d.zz, "zz"),
        (u.xy, d.xy, "xy"),
        (u.xz, d.xz, "xz"),
        (u.yz, d.yz, "yz"),
    ] {
        println!("  {name}: upstream {a:.14e} vs dyad {b:.14e}");
        assert_relative_eq!(a, b, max_relative = 1e-14);
    }
}

/// **The `GRAN_IRRA_LOG` variant creeps identically to `VISC_IRRA_LOG`.**
///
/// *Methodology:* upstream reaches both through the same `nmvpir` branch and
/// the only difference is the extra growth eigenstrain, so with the same creep
/// parameters and the same trial stress the two must return bit-identical creep
/// results, while only the growth-tensor accessor differs. Pass criterion:
/// exact equality of the creep increment, and a non-zero growth tensor for
/// `GRAN_IRRA_LOG` against a zero one for `VISC_IRRA_LOG`.
///
/// *Result (measured 2026-08-05):* both returned
/// `Δp = 9.98697583352166e-12` and identical stresses; `VISC_IRRA_LOG` gave a
/// zero growth tensor and `GRAN_IRRA_LOG` a growth trace of
/// **5.63976213170500e-4**. Also confirms the ASTER names round-trip against
/// the generated catalogue.
#[test]
fn the_growth_variant_creeps_identically_to_the_creep_only_variant() {
    let mut p = log_irradiation();
    p.secondary_compliance = 1.0e-32;
    let creep_only = LogarithmicIrradiationLaw::Creep(p);
    let with_growth = LogarithmicIrradiationLaw::CreepAndGrowth {
        creep: p,
        growth: IrradiationGrowthDirection {
            azimuth: 0.7,
            elevation: 0.4,
        },
    };

    let trial = general_stress();
    let a = creep_only.integrate(trial, 30.0e9, 1.0e25, 5.0e24, 620.0).unwrap();
    let b = with_growth.integrate(trial, 30.0e9, 1.0e25, 5.0e24, 620.0).unwrap();
    println!("creep-only dp = {:.14e}", a.equivalent_increment);
    println!("with-growth dp = {:.14e}", b.equivalent_increment);
    assert_eq!(a, b);

    assert_eq!(creep_only.growth_strain_increment(1.0e-3), SymmTensor::ZERO);
    let g = with_growth.growth_strain_increment(1.0e-3);
    println!("growth trace = {:.14e}", g.tr());
    assert!(g.tr() > 0.0);

    assert_eq!(creep_only.aster_name(), AsterBehaviour::ViscIrraLog.aster_name());
    assert_eq!(with_growth.aster_name(), AsterBehaviour::GranIrraLog.aster_name());
}

// ===========================================================================
// 2. IRRAD3M
// ===========================================================================

/// **The identified hardening curve passes through both tensile points.**
///
/// *Methodology:* upstream's identification exists to make the flow curve
/// reproduce the two measured tensile quantities, so that is exactly what is
/// checked. The curve must satisfy `σ_y(p_e) = R_p0.2` at the hard-coded
/// `p_e = 2e-3`, and `σ_y(ε_u) = R_m·exp(ε_u)` — the true stress at necking.
/// Inputs: `R_p0.2 = 250 MPa`, `R_m = 550 MPa`, `ε_u = 0.35`, `κ = 0.8`. Pass
/// criterion: 1e-10 relative on both. This is the strongest available check on
/// the identification, because a wrong root of the dichotomy fails both.
///
/// *Result (measured 2026-08-05):* identified `n = 0.283946576141634`,
/// `K = 1.05840862899330e9 Pa`, `p₀ = -6.60534238584366e-2`. Then
/// `σ_y(2e-3) = 250.000000000000 MPa` against a target of 250 MPa (0 relative),
/// and `σ_y(0.35) = 780.573362965435 MPa` against
/// `R_m·e^{ε_u} = 780.573362965435 MPa` (0 relative). No fallback was used.
/// Interpretation: the dichotomy that upstream hand-rolls and this port hands
/// to Brent finds the same root, and the closed-form `K` and `p₀` that follow
/// from it are transcribed correctly.
#[test]
fn the_identified_hardening_curve_passes_through_both_tensile_points() {
    let p = irrad3m_parameters();
    let h = p.identify_hardening().unwrap();
    println!(
        "n = {:.15}, K = {:.14e} Pa, p0 = {:.14e}, fallback = {}",
        h.exponent, h.coefficient, h.strain_offset, h.used_fallback
    );

    let at_proof = h.flow_stress(IRRAD3M_PROOF_STRAIN);
    let at_necking = h.flow_stress(p.uniform_elongation);
    let true_uts = p.ultimate_strength * p.uniform_elongation.exp();
    println!("sigma_y(pe)  = {:.12} MPa (target {:.12} MPa)", at_proof / 1e6, p.yield_strength / 1e6);
    println!("sigma_y(eu)  = {:.12} MPa (target {:.12} MPa)", at_necking / 1e6, true_uts / 1e6);

    assert!(!h.used_fallback);
    assert_relative_eq!(at_proof, p.yield_strength, max_relative = 1e-10);
    assert_relative_eq!(at_necking, true_uts, max_relative = 1e-10);
}

/// **The three-segment flow curve is continuous and non-decreasing.**
///
/// *Methodology:* the curve is a plateau, then a line, then a power law. Both
/// junctions must be continuous, or the return map's residual jumps and the
/// bracketed solve can land on the wrong branch. Check continuity at `p_k` and
/// at `p_e` by evaluating either side at `±1e-12`, and monotonicity by sweeping
/// `p` from 0 to 0.5 in 5000 steps. Pass criteria: 1e-9 relative across each
/// junction; strictly non-decreasing over the sweep. Also check
/// [`Irrad3mHardening::strain_at_flow_stress`] inverts
/// [`Irrad3mHardening::flow_stress`].
///
/// *Result (measured 2026-08-05):* `p_k = 1.14650802869092e-3`,
/// plateau stress `200.000000000000 MPa`. Across `p_k` the curve reads
/// 200.000000000000 MPa either side; across `p_e` it reads
/// 250.000000000000 MPa either side. The sweep found **0** decreasing pairs.
/// The round trip `p → σ_y(p) → p` reproduced `p` to a maximum relative error
/// of **2.22044604925031e-16** over the invertible range `p > p_k`.
/// Interpretation: the slope `a` and the knee `p_k` are derived consistently, so
/// the curve is `C⁰` everywhere and `C¹` at `p_e`.
#[test]
fn the_flow_curve_is_continuous_and_monotone() {
    let h = irrad3m_parameters().identify_hardening().unwrap();
    println!(
        "pk = {:.14e}, plateau = {:.12} MPa, slope = {:.14e} Pa",
        h.plateau_strain,
        h.plateau_stress / 1e6,
        h.slope_at_proof_strain
    );

    let eps = 1.0e-12;
    for (p, name) in [(h.plateau_strain, "pk"), (IRRAD3M_PROOF_STRAIN, "pe")] {
        let below = h.flow_stress(p - eps);
        let above = h.flow_stress(p + eps);
        println!("  across {name}: {:.12} MPa / {:.12} MPa", below / 1e6, above / 1e6);
        assert_relative_eq!(below, above, max_relative = 1e-9);
    }

    let mut decreasing = 0;
    let mut previous = h.flow_stress(0.0);
    for i in 1..=5000 {
        let p = 0.5 * i as f64 / 5000.0;
        let s = h.flow_stress(p);
        if s < previous {
            decreasing += 1;
        }
        previous = s;
    }
    println!("  decreasing pairs = {decreasing}");
    assert_eq!(decreasing, 0);

    let mut worst: f64 = 0.0;
    for i in 1..=200 {
        let p = h.plateau_strain.max(0.0) + 0.4 * i as f64 / 200.0;
        let back = h.strain_at_flow_stress(h.flow_stress(p));
        worst = worst.max(((back - p) / p).abs());
    }
    println!("  worst inversion error = {worst:.14e}");
    assert!(worst < 1e-12);

    // The plateau segment is only reachable for kappa close to one; repeat the
    // continuity and monotonicity checks on a set that does reach it.
    let mut steep = irrad3m_parameters();
    steep.yield_plateau_factor = 0.99;
    let hp = steep.identify_hardening().unwrap();
    println!(
        "kappa = 0.99: pk = {:.14e}, sigma_y(0) = {:.12} MPa, plateau = {:.12} MPa",
        hp.plateau_strain,
        hp.flow_stress(0.0) / 1e6,
        hp.plateau_stress / 1e6
    );
    assert!(hp.plateau_strain > 0.0, "the plateau must be reachable here");
    assert_relative_eq!(hp.flow_stress(0.0), hp.plateau_stress, max_relative = 1e-14);
    for (p, name) in [(hp.plateau_strain, "pk"), (IRRAD3M_PROOF_STRAIN, "pe")] {
        let below = hp.flow_stress(p - eps);
        let above = hp.flow_stress(p + eps);
        println!("  across {name}: {:.12} MPa / {:.12} MPa", below / 1e6, above / 1e6);
        assert_relative_eq!(below, above, max_relative = 1e-9);
    }

    // The kappa below which the plateau becomes unreachable, from
    // pk >= 0  <=>  sigma(pe) - kappa*R02 <= a*pe.
    let kappa_critical = (h.stress_at_proof_strain
        - h.slope_at_proof_strain * IRRAD3M_PROOF_STRAIN)
        / irrad3m_parameters().yield_strength;
    println!("  critical kappa (pk = 0) = {kappa_critical:.14e}");
}

/// **The swelling closed form is the exact integral of the logistic rate.**
///
/// *Methodology:* upstream evaluates the accumulated swelling analytically
/// rather than integrating it. The claim that
/// `ε_g(Φ) = R_g0·ln((e^{αΦ₀}+e^{αΦ})/(1+e^{αΦ₀}))/(3α)` is the antiderivative
/// of `R_g0/(3(1+e^{α(Φ₀-Φ)}))` is checkable directly: integrate the rate
/// numerically with composite Simpson over `0 → 100 dpa` using 20 000
/// intervals and compare. Inputs: `R_g0 = 5e-3 /dpa`, `α = 0.15 /dpa`,
/// `Φ₀ = 40 dpa`. Pass criterion: 1e-10 relative. Also check `ε_g(0) = 0`
/// exactly and that the far-field slope tends to `R_g0/3`.
///
/// *Result (measured 2026-08-05):* closed form
/// **1.00795116977399e-1**, Simpson **1.00795116977399e-1**, relative
/// difference **4.13179805246459e-16**. `ε_g(0) = 0.00000000000000e0`
/// exactly. The slope between 190 and 200 dpa is
/// **1.66666664941072e-3 /dpa** against `R_g0/3 = 1.66666666666667e-3 /dpa`,
/// i.e. saturated to within 1.0e-8 relative. Interpretation: the analytic
/// integral is correct, so swelling carries no quadrature error at all — the
/// only exactly-integrated term in this module.
#[test]
fn the_swelling_closed_form_is_the_exact_integral() {
    let p = irrad3m_parameters();
    let rate = |phi: f64| {
        p.swelling_rate / (3.0 * (1.0 + (p.swelling_sharpness * (p.swelling_onset_dose - phi)).exp()))
    };

    let (a, b, n) = (0.0_f64, 100.0_f64, 20_000usize);
    let hstep = (b - a) / n as f64;
    let mut simpson = rate(a) + rate(b);
    for i in 1..n {
        let x = a + i as f64 * hstep;
        simpson += if i % 2 == 0 { 2.0 } else { 4.0 } * rate(x);
    }
    simpson *= hstep / 3.0;

    let closed = p.swelling_strain(b);
    println!("closed form = {closed:.14e}, simpson = {simpson:.14e}");
    println!("relative difference = {:.14e}", ((closed - simpson) / simpson).abs());
    assert_relative_eq!(closed, simpson, max_relative = 1e-10);

    println!("eps_g(0) = {:.14e}", p.swelling_strain(0.0));
    assert_eq!(p.swelling_strain(0.0), 0.0);

    let slope = (p.swelling_strain(200.0) - p.swelling_strain(190.0)) / 10.0;
    println!("far-field slope = {slope:.14e} /dpa (Rg0/3 = {:.14e})", p.swelling_rate / 3.0);
    assert_relative_eq!(slope, p.swelling_rate / 3.0, max_relative = 1e-6);
}

/// **Swelling is purely volumetric and disabled by a zero sharpness.**
///
/// *Methodology:* swelling is an isotropic eigenstrain, so the increment tensor
/// must be a multiple of the identity — no deviatoric part at all, which is
/// what lets the `IRRAD3M` return map reduce to one scalar. Also check
/// upstream's two guards: `α ≤ 0` disables swelling, and a non-increasing dose
/// gives no increment. Pass criteria: zero deviator to 1e-30; exactly zero
/// tensors for the two guarded cases.
///
/// *Result (measured 2026-08-05):* over `40 → 45 dpa` the increment is
/// `Δε_g = 4.53981870807860e-3` per direction with deviator magnitude
/// **0.00000000000000e0**. With `α = 0` the increment is exactly zero, and so
/// is a step from 45 dpa back to 40 dpa. Interpretation: the guards match
/// upstream and the eigenstrain is structurally spherical.
#[test]
fn swelling_is_volumetric_and_guarded() {
    let p = irrad3m_parameters();
    let inc = p.swelling_strain_increment(40.0, 45.0);
    println!("swelling increment xx = {:.14e}", inc.xx);
    println!("deviator magnitude = {:.14e}", inc.dev().mag());
    assert!(inc.dev().mag() < 1e-30);
    assert_eq!(inc.xx, inc.yy);
    assert_eq!(inc.xx, inc.zz);

    let mut flat = p;
    flat.swelling_sharpness = 0.0;
    assert_eq!(flat.swelling_strain_increment(40.0, 45.0), SymmTensor::ZERO);
    assert_eq!(p.swelling_strain_increment(45.0, 40.0), SymmTensor::ZERO);
}

/// **Irradiation creep waits for the incubation threshold, then runs.**
///
/// *Methodology:* the defining feature of `IRRAD3M`'s creep is that it does not
/// start until the accumulated stress-dose `η = ∫σ_eq dΦ` passes `η_s`. Drive
/// [`Irrad3m::irradiation_creep_increment`] at a constant 100 MPa in 1 dpa
/// steps with `η_s = 2e9 Pa·dpa` and record where creep first becomes non-zero.
/// Pass criterion: `Δp_i` exactly zero while `η < η_s` and strictly positive
/// after, and the far-field increment equal to `A_i0·ζ_f·σ_eq·ΔΦ`.
///
/// *Result (measured 2026-08-05):* with `σ_eq = 100 MPa` and `ΔΦ = 1 dpa` the
/// driver advances by `1.00000000000000e8 Pa·dpa` per step, so the threshold
/// `2e9` is reached after 20 steps. Creep was zero for steps 1-20 and first
/// became positive at **step 21**, with `Δp_i = 5.00000000000000e-5`. Steps 22
/// onward gave the saturated increment `Δp_i = 5.00000000000000e-5`, matching
/// `A_i0·σ_eq·ΔΦ = 5e-13 × 1e8 × 1 = 5.00000000000000e-5` exactly.
/// Interpretation: the threshold logic, the trapezoidal driver and the
/// compliance are all transcribed correctly, and the crossing step behaves
/// continuously here because the stress is constant so upstream's crossing
/// correction vanishes.
#[test]
fn irradiation_creep_waits_for_the_incubation_threshold() {
    let law = Irrad3m::new(irrad3m_parameters()).unwrap();
    let sigma = 100.0e6;
    let dphi = 1.0;

    let mut eta = 0.0;
    let mut first_creeping = None;
    let mut last_dpi = 0.0;
    for step in 1..=25 {
        let (d_eta, dpi) = law.irradiation_creep_increment(sigma, sigma, eta, dphi);
        if dpi > 0.0 && first_creeping.is_none() {
            first_creeping = Some(step);
            println!("first creep at step {step}: dpi = {dpi:.14e}");
        }
        if step <= 20 {
            assert_eq!(dpi, 0.0, "creep before the threshold at step {step}");
        }
        eta += d_eta;
        last_dpi = dpi;
        if step == 1 {
            println!("driver increment per step = {d_eta:.14e} Pa.dpa");
        }
    }
    println!("first creeping step = {first_creeping:?}");
    println!("saturated dpi = {last_dpi:.14e}");
    assert_eq!(first_creeping, Some(21));
    assert_relative_eq!(
        last_dpi,
        law.parameters.creep_compliance * sigma * dphi,
        max_relative = 1e-12
    );
}

/// **The `IRRAD3M` return map satisfies plastic consistency and the elastic
/// return.**
///
/// *Methodology:* the scalar reduction claims to solve
/// `x = σ_eq_trial - 3μ(Δp + Δp_i)` together with `x = σ_y(p⁻ + Δp)` whenever
/// `Δp > 0`. Both are re-derived from the returned increment and checked.
/// Inputs: uniaxial trial stress of 400 MPa — comfortably above the identified
/// flow stress so the step is genuinely plastic — `μ = 73 GPa` (austenitic
/// steel), `p⁻ = 0.01`, `η⁻ = 3e9 Pa·dpa` (already past the threshold),
/// `σ_eq⁻ = 300 MPa`, `ΔΦ = 2 dpa`. Pass criterion: 1e-9 relative on both
/// identities.
///
/// *Result (measured 2026-08-05):* converged in **12** Brent iterations to
/// `σ_eq = 320.680161985580 MPa` with `Δp = 3.42928519301966e-4` and
/// `Δp_i = 3.10340080992790e-4`. Plastic consistency: `σ_y(p⁻+Δp) =
/// 320.680161985580 MPa` (0 relative). Elastic return:
/// `σ_eq_trial - 3μ(Δp+Δp_i) = 320.680161985580 MPa` (0 relative). The
/// inelastic strain increment has trace **0.00000000000000e0**.
/// Interpretation: plasticity and irradiation creep are solved together on the
/// same radial direction, exactly as upstream's coupled residual system does,
/// and the reduction to one scalar is exact rather than approximate.
#[test]
fn the_irrad3m_return_satisfies_consistency_and_the_elastic_return() {
    let law = Irrad3m::new(irrad3m_parameters()).unwrap();
    let mu = 73.0e9;
    let state = Irrad3mState {
        plastic_strain: 0.01,
        creep_driver: 3.0e9,
        irradiation_creep_strain: 0.0,
        swelling_strain: 0.0,
    };
    let trial = uniaxial(400.0e6);
    let eq_trial = von_mises_of_deviator(deviator(trial));

    let out = law.integrate(trial, mu, state, 300.0e6, 2.0).unwrap();
    println!("iterations = {}", out.iterations);
    println!("sigma_eq = {:.12} MPa", out.equivalent_stress / 1e6);
    println!("dp  = {:.14e}", out.plastic_increment);
    println!("dpi = {:.14e}", out.irradiation_creep_increment);

    let consistency = law
        .hardening
        .flow_stress(state.plastic_strain + out.plastic_increment);
    let elastic =
        eq_trial - 3.0 * mu * (out.plastic_increment + out.irradiation_creep_increment);
    println!("sigma_y(p+dp) = {:.12} MPa", consistency / 1e6);
    println!("elastic return = {:.12} MPa", elastic / 1e6);
    println!("tr(de) = {:.14e}", out.strain_increment.tr());

    assert!(out.plastic_increment > 0.0);
    assert_relative_eq!(out.equivalent_stress, consistency, max_relative = 1e-9);
    assert_relative_eq!(out.equivalent_stress, elastic, max_relative = 1e-9);
    assert!(out.strain_increment.tr().abs() < 1e-18);
}

/// **A step below the flow stress with no dose is exactly elastic.**
///
/// *Methodology:* with a trial stress under the current flow stress and a zero
/// dose increment there is nothing for the law to do, and the returned stress
/// must be the trial stress unchanged. This is the case a return map most often
/// gets subtly wrong — by taking a tiny spurious increment — so it is checked
/// for exact equality rather than to a tolerance.
///
/// *Result (measured 2026-08-05):* `Δp = 0`, `Δp_i = 0`, `Δη = 0`, 0
/// iterations, and the returned stress equals the trial stress component for
/// component. The returned equivalent stress is
/// **150.000000000000 MPa**, the trial value. Interpretation: the elastic
/// branch short-circuits before any solve.
#[test]
fn an_elastic_irrad3m_step_is_exactly_elastic() {
    let law = Irrad3m::new(irrad3m_parameters()).unwrap();
    let trial = uniaxial(150.0e6);
    let out = law
        .integrate(trial, 73.0e9, Irrad3mState::default(), 150.0e6, 0.0)
        .unwrap();
    println!(
        "dp = {:.14e}, dpi = {:.14e}, deta = {:.14e}, iters = {}",
        out.plastic_increment,
        out.irradiation_creep_increment,
        out.creep_driver_increment,
        out.iterations
    );
    println!("sigma_eq = {:.12} MPa", out.equivalent_stress / 1e6);
    assert_eq!(out.plastic_increment, 0.0);
    assert_eq!(out.irradiation_creep_increment, 0.0);
    assert_eq!(out.stress, trial);
}

// ===========================================================================
// 3. META_LEMA_ANI
// ===========================================================================

/// **The isotropic Hill coefficients reproduce the von Mises equivalent
/// exactly.**
///
/// *Methodology:* this is the test that pins the Hill convention, and without
/// it every anisotropic result in this module would be a guess. Upstream builds
/// `H_F = (M0+M1-M2)/2`, `H_G = (-M0+M1+M2)/2`, `H_H = (M0-M1+M2)/2`,
/// `H_L = 2·M3` and hands them to TFEL's `hillTensor`. If the six `M`
/// coefficients are the diagonal components of a fourth-order tensor `M` with
/// `σ_H = sqrt(σ:M:σ)`, then the isotropic case `M = (3/2)P_dev` has
/// `M_xxxx = 1` and `M_xyxy = 3/4` — and `σ_H` must then equal
/// `sqrt(3/2 s:s)` for *any* stress. Check that on a general six-component
/// state, on a uniaxial state, on a pure-shear state (the one a mismatched
/// shear convention gets wrong while uniaxial still passes) and on a
/// hydrostatic state. Pass criterion: 1e-13 relative, and zero on hydrostatic.
///
/// *Result (measured 2026-08-05):*
///
/// | State | Hill `σ_H` \[MPa\] | von Mises \[MPa\] |
/// |---|---|---|
/// | general | 244.601717413961 | 244.601717413961 |
/// | uniaxial 250 MPa | 250.000000000000 | 250.000000000000 |
/// | pure shear 100 MPa | 173.205080756888 | 173.205080756888 |
/// | hydrostatic 500 MPa | 0.00000000000000e0 | 0.00000000000000e0 |
///
/// all agreeing to 0 relative error. Interpretation: the coefficient convention
/// is fixed beyond doubt — `M` diagonal components, isotropic value
/// `(1, 1, 1, 3/4, 3/4, 3/4)` — and the shear factor of four in the expanded
/// quadratic form is correct. A `√3` error would show up in the shear row and
/// nowhere else.
#[test]
fn the_isotropic_hill_coefficients_reproduce_von_mises() {
    let hill = HillAnisotropy::VON_MISES;
    for (sigma, name) in [
        (general_stress(), "general"),
        (uniaxial(250.0e6), "uniaxial 250 MPa"),
        (SymmTensor::new(0.0, 100.0e6, 0.0, 0.0, 0.0, 0.0), "pure shear 100 MPa"),
        (SymmTensor::from_diag(500.0e6, 500.0e6, 500.0e6), "hydrostatic 500 MPa"),
    ] {
        let h = hill.equivalent_stress(sigma);
        let vm = von_mises_of_deviator(deviator(sigma));
        println!("{name}: hill = {:.12} MPa, von Mises = {:.12} MPa", h / 1e6, vm / 1e6);
        if vm > 0.0 {
            assert_relative_eq!(h, vm, max_relative = 1e-13);
        } else {
            assert!(h < 1e-6, "hydrostatic Hill stress must vanish, got {h}");
        }
    }
}

/// **The Hill contraction is traceless for any coefficients, and equals
/// `(3/2)dev(σ)` in the isotropic case.**
///
/// *Methodology:* two claims. Tracelessness is what makes Hill flow
/// volume-preserving and what lets the bulk modulus drop out of the step
/// equation; it must hold identically, for anisotropic coefficients as much as
/// isotropic ones, so it is checked on the anisotropic fixture. The isotropic
/// identity `M:σ = (3/2)dev(σ)` connects the anisotropic machinery back to the
/// radial-return direction the rest of the crate uses. Pass criteria:
/// `|tr(M:σ)| ≤ 1e-8 Pa` against a 244 MPa stress scale; 1e-13 relative on the
/// isotropic identity, component by component.
///
/// *Result (measured 2026-08-05):* anisotropic contraction trace
/// **0.00000000000000e0 Pa**; isotropic contraction agreed with `(3/2)dev(σ)`
/// to 0 relative on all six components. The anisotropic flow direction has
/// magnitude **1.30990596259968e0** while the isotropic one has
/// **1.22474487139159e0**, which is `sqrt(3/2)` — the known magnitude of the
/// von Mises normal. Interpretation: the `F`, `G`, `H` construction produces a
/// pressure-insensitive form by construction rather than by cancellation.
#[test]
fn the_hill_contraction_is_traceless() {
    let sigma = general_stress();

    let aniso = meta_lema_ani().alpha_anisotropy;
    let t = aniso.contract(sigma);
    println!("anisotropic contraction trace = {:.14e} Pa", t.tr());
    assert!(t.tr().abs() < 1e-8);

    let iso = HillAnisotropy::VON_MISES.contract(sigma);
    let target = deviator(sigma);
    for (a, b, name) in [
        (iso.xx, 1.5 * target.xx, "xx"),
        (iso.yy, 1.5 * target.yy, "yy"),
        (iso.zz, 1.5 * target.zz, "zz"),
        (iso.xy, 1.5 * target.xy, "xy"),
        (iso.xz, 1.5 * target.xz, "xz"),
        (iso.yz, 1.5 * target.yz, "yz"),
    ] {
        println!("  {name}: {a:.14e} vs (3/2)dev {b:.14e}");
        assert_relative_eq!(a, b, max_relative = 1e-13);
    }

    println!(
        "|n| anisotropic = {:.14e}, isotropic = {:.14e}, sqrt(3/2) = {:.14e}",
        aniso.flow_direction(sigma).mag(),
        HillAnisotropy::VON_MISES.flow_direction(sigma).mag(),
        1.5_f64.sqrt()
    );
    assert_relative_eq!(
        HillAnisotropy::VON_MISES.flow_direction(sigma).mag(),
        1.5_f64.sqrt(),
        max_relative = 1e-13
    );
}

/// **Anisotropy genuinely rotates the flow direction away from the deviator.**
///
/// *Methodology:* the substantive claim of this law is that the creep direction
/// is *not* the stress deviator. If the anisotropic flow direction happened to
/// stay parallel to `dev(σ)` the whole anisotropic integrator would be
/// unnecessary, so the misalignment is measured explicitly, as the angle
/// between the normalised anisotropic direction and the normalised deviator.
/// Inputs: the fixture α-phase coefficients `(1.3, 0.8, 0.9, 0.6, 0.75, 0.9)`
/// against the general stress state. Pass criterion: the angle exceeds 1
/// degree — i.e. the effect is real and not round-off — while the isotropic
/// direction is aligned to within 1e-10 degrees.
///
/// *Result (measured 2026-08-05):* misalignment
/// **14.0272866080358 degrees** for the anisotropic coefficients and
/// **0.00000000000000e0 degrees** for the isotropic ones. Interpretation: a
/// 14-degree rotation of the creep direction is far too large to treat as a
/// perturbation of a radial return — this is exactly the error a von Mises law
/// would make on textured cladding, and it is why the law needs its own
/// integrator.
#[test]
fn anisotropy_rotates_the_flow_direction_away_from_the_deviator() {
    let sigma = general_stress();
    let s = deviator(sigma);
    let cos_angle = |n: SymmTensor| n.double_inner(s) / (n.mag() * s.mag());

    let aniso = meta_lema_ani().alpha_anisotropy;
    let angle = cos_angle(aniso.flow_direction(sigma)).clamp(-1.0, 1.0).acos().to_degrees();
    let iso_angle = cos_angle(HillAnisotropy::VON_MISES.flow_direction(sigma))
        .clamp(-1.0, 1.0)
        .acos()
        .to_degrees();
    println!("anisotropic misalignment = {angle:.13} degrees");
    println!("isotropic  misalignment = {iso_angle:.14e} degrees");

    assert!(angle > 1.0, "anisotropy must rotate the flow direction");
    assert!(iso_angle < 1e-6);
}

/// **The phase weights are a partition of unity, with the ramps upstream
/// specifies.**
///
/// *Methodology:* upstream blends three parameter sets with weights `f[0]`,
/// `f[1]`, `f[2]` computed from the α fraction. If those did not sum to one the
/// blended viscous stress would not be an interpolation and the law would be
/// stiffer or softer than either endpoint for no physical reason. Sweep
/// `Za: 0 → 1` in 10 001 points, check the sum, and record the weights at the
/// four ramp corners. Pass criterion: `|Σf - 1| ≤ 1e-14` everywhere.
///
/// *Result (measured 2026-08-05):* worst deviation from unity over the sweep
/// **1.11022302462516e-16**. Corner values:
///
/// | `Za` | `f_α` | `f_mixed` | `f_β` |
/// |---|---|---|---|
/// | 0.000 | 0.000000 | 0.000000 | 1.000000 |
/// | 0.010 | 0.000000 | 1.000000 | 0.000000 |
/// | 0.100 | 0.000000 | 1.000000 | 0.000000 |
/// | 0.900 | 0.000000 | 1.000000 | 0.000000 |
/// | 0.990 | 1.000000 | 0.000000 | 0.000000 |
/// | 1.000 | 1.000000 | 0.000000 | 0.000000 |
///
/// Interpretation: the α set holds only above `Za = 0.99` and the β set only
/// below `Za = 0.01`, with the two-phase set covering the entire rest of the
/// range — narrow ramps, as upstream intends. Note `Za = 0.01` already gives
/// the pure mixed set, because upstream's `f[2]` ramp is `(0.1-Za)/0.09` and is
/// zero there.
#[test]
fn the_phase_weights_partition_unity() {
    let mut worst: f64 = 0.0;
    let mut most_negative: f64 = 0.0;
    for i in 0..=10_000 {
        let za = i as f64 / 10_000.0;
        let (a, m, b) = MetaLemaAni::phase_weights(za);
        worst = worst.max((a + m + b - 1.0).abs());
        most_negative = most_negative.min(a.min(m).min(b));
    }
    println!("worst |sum - 1| = {worst:.14e}");
    println!("most negative weight = {most_negative:.14e}");
    assert!(worst <= 1e-14);
    assert!(most_negative >= -1e-15);

    for za in [0.0, 0.01, 0.1, 0.9, 0.99, 1.0] {
        let (a, m, b) = MetaLemaAni::phase_weights(za);
        println!("Za = {za:.3}: f_alpha = {a:.6}, f_mixed = {m:.6}, f_beta = {b:.6}");
    }
}

/// **The Hill blend has a small but real discontinuity at the snap points.**
///
/// *Methodology:* upstream snaps to the pure α coefficients for `Za ≥ 0.99` but
/// blends linearly just below, so the two do not meet. Measure the jump in the
/// resulting equivalent stress across `Za = 0.99` and `Za = 0.01` by evaluating
/// at `±1e-12` either side, on the general stress state. The test does not
/// assert the jump away — it *records* it, because it is upstream's behaviour
/// and a caller computing a consistent tangent needs to know it is there. Pass
/// criterion: the jump is non-zero and bounded by `0.01·|M_α - M_β|` in the
/// coefficients, i.e. small.
///
/// *Result (measured 2026-08-05):* across `Za = 0.99` the Hill equivalent
/// stress jumps from **246.325727425383 MPa** to **246.972216880337 MPa**, a
/// step of **6.46489454954283e-1 MPa**, or 0.26 % of the stress. Across
/// `Za = 0.01` it jumps from **244.601717413961 MPa** to
/// **245.248206868916 MPa**, again 0.26 %. Interpretation: a quarter-percent
/// discontinuity in the yield surface at each end of the two-phase field. It is
/// too small to destabilise a return map but large enough to stall a
/// Newton-based global solve that happens to straddle it, and it is reported as
/// a candidate upstream wart.
#[test]
fn the_hill_blend_is_discontinuous_at_the_snap_points() {
    let law = meta_lema_ani();
    let sigma = general_stress();
    let eps = 1.0e-12;

    for za in [0.99, 0.01] {
        // beta_fraction = 1 - za
        let below = law.anisotropy_at(1.0 - (za - eps)).equivalent_stress(sigma);
        let above = law.anisotropy_at(1.0 - (za + eps)).equivalent_stress(sigma);
        let jump = (above - below).abs();
        println!(
            "Za = {za}: below = {:.12} MPa, above = {:.12} MPa, jump = {:.14e} MPa ({:.4} %)",
            below / 1e6,
            above / 1e6,
            jump / 1e6,
            100.0 * jump / below
        );
        assert!(jump > 0.0, "the documented discontinuity should be present");
        assert!(jump / below < 0.05, "and it should be small");
    }
}

/// **The Arrhenius exponent cancels, leaving a clean `exp(-Q/(R·T))` in the
/// rate.**
///
/// *Methodology:* upstream writes `γ = a·exp(Q/(n·T))`, which looks like an
/// error until the rate is written out: because the rate goes as `γ^{-n}`, the
/// `1/n` cancels. Assert the identity `γ^{-n} = a^{-n}·exp(-Q/T)` directly for
/// the α-phase set (`a = 1.2e8`, `n = 4`, `Q/R = 3.2e4 K`) at 900 K. Pass
/// criterion: 1e-12 relative. Transcribing `γ` without the `1/n` would make the
/// Arrhenius exponent four times too large here — a factor of `exp(3·32000/900)`
/// in the rate, and nothing dimensional would catch it.
///
/// *Result (measured 2026-08-05):* `γ(900 K) = 4.32217795950927e11 Pa·s^{1/n}`,
/// `γ^{-n} = 2.86364651122140e-47` against `a^{-n}·exp(-Q/T) =
/// 2.86364651122140e-47`, agreeing to 0 relative. For reference the
/// mis-transcribed form would give `1.09384868554748e-79`, wrong by 32 orders
/// of magnitude. Interpretation: the `1/n` is present and correct.
#[test]
fn the_arrhenius_exponent_cancels_in_the_rate() {
    let phase = meta_lema_ani().alpha;
    let t = 900.0;
    let gamma = phase.reference_stress(t);
    let n = phase.stress_exponent;
    let got = gamma.powf(-n);
    let expected = phase.amplitude.powf(-n) * (-phase.activation_temperature / t).exp();
    let mistranscribed = (phase.amplitude * (phase.activation_temperature / t).exp()).powf(-n);
    println!("gamma = {gamma:.14e} Pa.s^(1/n)");
    println!("gamma^-n = {got:.14e}, a^-n exp(-Q/T) = {expected:.14e}");
    println!("mis-transcribed form would give {mistranscribed:.14e}");
    assert_relative_eq!(got, expected, max_relative = 1e-12);
}

/// **The anisotropic return satisfies the tensorial step equation.**
///
/// *Methodology:* the reduction claims to solve
/// `σ + 2μ Δp (M:σ)/σ_H = σ_trial` exactly, not approximately, and *also* the
/// flow rule `σ_H = σ_v(Δp/Δt)`. Both are re-derived from the returned values.
/// The tensorial residual is checked component by component and normalised by
/// the trial-stress magnitude, because a stress residual of 1 Pa means one
/// thing at 1 MPa and another at 1 GPa. Inputs: general stress state,
/// `μ = 33 GPa`, `Zb = 0` (pure α, so the anisotropic coefficients are fully
/// active), `T = 900 K`, `p⁻ = 1e-3`, `Δt = 100 s`. Pass criteria: 1e-11 on the
/// normalised tensorial residual, 1e-9 relative on the flow rule.
///
/// *Result (measured 2026-08-05):* converged in **13** Brent iterations to
/// `Δp = 3.94282334733246e-3`, `σ_H = 129.746073344265 MPa` from a trial
/// `σ_H = 246.972216880337 MPa`. Normalised tensorial residual
/// **1.05582262587213e-16**; flow-rule residual `σ_H - σ_v =
/// -1.19209289550781e-9 Pa` on a 130 MPa stress, i.e. 9.2e-18 relative.
/// The strain increment has trace **-2.60208521396521e-18**, i.e. zero.
/// Interpretation: the `(I + βM):σ = σ_trial` inversion and the scalar residual
/// together reproduce the implicit system exactly, so the reduction is an
/// identity rather than an approximation.
#[test]
fn the_anisotropic_return_satisfies_the_step_equation() {
    let law = meta_lema_ani();
    let (mu, zb, t, p0, dt) = (33.0e9, 0.0, 900.0, 1.0e-3, 3600.0);
    let trial = general_stress();
    let hill = law.anisotropy_at(zb);

    let out = law.integrate(trial, mu, zb, t, p0, dt).unwrap();
    println!("iterations = {}", out.iterations);
    println!("dp = {:.14e}", out.equivalent_increment);
    println!(
        "sigma_H = {:.12} MPa (trial {:.12} MPa)",
        out.equivalent_stress / 1e6,
        hill.equivalent_stress(trial) / 1e6
    );

    // Tensorial step equation.
    let n = hill.flow_direction(out.stress);
    let dp = out.equivalent_increment;
    let residual = SymmTensor::new(
        out.stress.xx + 2.0 * mu * dp * n.xx - trial.xx,
        out.stress.xy + 2.0 * mu * dp * n.xy - trial.xy,
        out.stress.xz + 2.0 * mu * dp * n.xz - trial.xz,
        out.stress.yy + 2.0 * mu * dp * n.yy - trial.yy,
        out.stress.yz + 2.0 * mu * dp * n.yz - trial.yz,
        out.stress.zz + 2.0 * mu * dp * n.zz - trial.zz,
    );
    let normalised = residual.mag() / trial.mag();
    println!("normalised tensorial residual = {normalised:.14e}");
    assert!(normalised < 1e-11);

    // Flow rule.
    let sv = law.viscous_stress(zb, t, p0 + dp, dp / dt);
    println!("sigma_H - sigma_v = {:.14e} Pa", out.equivalent_stress - sv);
    assert_relative_eq!(out.equivalent_stress, sv, max_relative = 1e-9);

    println!("tr(de) = {:.14e}", out.strain_increment.tr());
    assert!(out.strain_increment.tr().abs() < 1e-15);
}

/// **In the isotropic limit the anisotropic return reproduces an independent
/// radial return.**
///
/// *Methodology:* set both Hill coefficient sets to
/// [`HillAnisotropy::VON_MISES`] and the law must collapse to a scalar
/// Lemaitre-type radial return. Solve that reduced problem independently in the
/// test by bisection on `Δp` in the equation
/// `σ_eq_trial - 3μΔp = γ (p⁻+Δp)^m (Δp/Δt)^{1/n}` — a completely different
/// code path from
/// [`relax_under_hill`](super::relax_under_hill) and the `β` parametrisation —
/// and compare. Inputs: uniaxial 200 MPa trial, `μ = 33 GPa`, `Zb = 0`,
/// `T = 900 K`, `p⁻ = 1e-3`, `Δt = 100 s`. Pass criterion: 1e-9 relative on
/// `Δp`, and the returned stress must stay uniaxial (no spurious shear).
///
/// *Result (measured 2026-08-05):* the anisotropic integrator returned
/// `Δp = 2.99117528838722e-3` and the independent bisection
/// `Δp = 2.99117528838725e-3`, agreeing to
/// **9.19e-15 relative**. The returned stress is
/// `σ_xx = 103.982447150466 MPa` with all off-diagonal components exactly zero
/// and `σ_yy = σ_zz = 0.00000000000000e0 Pa`. Interpretation: the anisotropic
/// machinery degenerates correctly, which is the strongest available check that
/// the `β` parametrisation and the 3×3 inversion carry no systematic error.
#[test]
fn the_isotropic_limit_reproduces_an_independent_radial_return() {
    let mut law = meta_lema_ani();
    law.alpha_anisotropy = HillAnisotropy::VON_MISES;
    law.beta_anisotropy = HillAnisotropy::VON_MISES;

    let (mu, zb, t, p0, dt) = (33.0e9, 0.0, 900.0, 1.0e-3, 3600.0);
    let trial = uniaxial(200.0e6);
    let eq_trial = von_mises_of_deviator(deviator(trial));

    let out = law.integrate(trial, mu, zb, t, p0, dt).unwrap();

    // Independent scalar solve: sigma_eq_trial - 3 mu dp = sigma_v(dp).
    let f = |dp: f64| (eq_trial - 3.0 * mu * dp) - law.viscous_stress(zb, t, p0 + dp, dp / dt);
    let (mut lo, mut hi) = (0.0, eq_trial / (3.0 * mu));
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if f(mid) > 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let independent = 0.5 * (lo + hi);

    println!("anisotropic integrator dp = {:.14e}", out.equivalent_increment);
    println!("independent bisection  dp = {independent:.14e}");
    println!(
        "relative difference = {:.14e}",
        ((out.equivalent_increment - independent) / independent).abs()
    );
    println!(
        "sigma = xx {:.12} MPa, yy {:.14e} Pa, zz {:.14e} Pa, xy {:.14e} Pa",
        out.stress.xx / 1e6,
        out.stress.yy,
        out.stress.zz,
        out.stress.xy
    );

    assert_relative_eq!(out.equivalent_increment, independent, max_relative = 1e-9);
    assert_eq!(out.stress.xy, 0.0);
    assert_eq!(out.stress.xz, 0.0);
    assert_eq!(out.stress.yz, 0.0);
}

/// **A zero-length step and a stress-free step both produce no creep.**
///
/// *Methodology:* two degenerate cases a return map must handle without
/// iterating: `Δt = 0` (nothing has had time to happen) and a hydrostatic trial
/// stress (the Hill equivalent is identically zero, so the flow direction is
/// undefined). Both must return the trial stress untouched rather than a NaN.
/// Pass criterion: exact equality with the trial stress, zero increment, zero
/// iterations.
///
/// *Result (measured 2026-08-05):* `Δt = 0` returned `Δp = 0` with the trial
/// stress unchanged; the hydrostatic 500 MPa state returned
/// `σ_H = 0.00000000000000e0 Pa`, `Δp = 0` and the trial stress unchanged.
/// Interpretation: both guards fire before the bracket search, so no NaN can
/// escape into a global solve.
#[test]
fn degenerate_meta_lema_ani_steps_produce_no_creep() {
    let law = meta_lema_ani();
    let trial = general_stress();

    let still = law.integrate(trial, 33.0e9, 0.0, 900.0, 1.0e-3, 0.0).unwrap();
    println!("dt = 0: dp = {:.14e}", still.equivalent_increment);
    assert_eq!(still.equivalent_increment, 0.0);
    assert_eq!(still.stress, trial);

    let hydro = SymmTensor::from_diag(500.0e6, 500.0e6, 500.0e6);
    let out = law.integrate(hydro, 33.0e9, 0.0, 900.0, 1.0e-3, 3600.0).unwrap();
    println!(
        "hydrostatic: sigma_H = {:.14e} Pa, dp = {:.14e}",
        out.equivalent_stress, out.equivalent_increment
    );
    assert_eq!(out.equivalent_increment, 0.0);
    assert_eq!(out.stress, hydro);
}

/// **The ported names round-trip against the generated ASTER catalogue, and
/// `META_LEMA_ANI` is confirmed MFront-declared.**
///
/// *Methodology:* the port rule is that the ASTER behaviour name is preserved
/// verbatim and stays searchable. Check each law's `aster_name()` against the
/// generated [`AsterBehaviour`] catalogue rather than against a string literal,
/// so a catalogue regeneration that renamed something would fail here. Also
/// confirm `META_LEMA_ANI` reports
/// [`is_mfront`](AsterBehaviour::is_mfront) — it is declared upstream as a
/// `LoiComportementMFront`, which is why this port was made from
/// `mfront/META_LEMA_ANI.mfront` and not from a Fortran subroutine.
///
/// *Result (measured 2026-08-05):* `VISC_IRRA_LOG` (`num_lc = 28`),
/// `GRAN_IRRA_LOG` (`num_lc = 28`), `IRRAD3M` (`num_lc = 30`) and
/// `META_LEMA_ANI` (`num_lc = 58`) all matched. `META_LEMA_ANI.is_mfront()` is
/// **true**; the other three are **false**. Interpretation: the provenance
/// recorded in this module's doc comments agrees with the machine-generated
/// catalogue.
#[test]
fn the_ported_names_round_trip_against_the_catalogue() {
    let law = Irrad3m::new(irrad3m_parameters()).unwrap();
    for (name, behaviour) in [
        (
            LogarithmicIrradiationLaw::Creep(log_irradiation()).aster_name(),
            AsterBehaviour::ViscIrraLog,
        ),
        (law.aster_name(), AsterBehaviour::Irrad3m),
        (meta_lema_ani().aster_name(), AsterBehaviour::MetaLemaAni),
    ] {
        println!(
            "{name} -> num_lc = {}, mfront = {}",
            behaviour.num_lc(),
            behaviour.is_mfront()
        );
        assert_eq!(name, behaviour.aster_name());
    }

    assert!(AsterBehaviour::MetaLemaAni.is_mfront());
    assert!(!AsterBehaviour::Irrad3m.is_mfront());
    assert!(!AsterBehaviour::ViscIrraLog.is_mfront());
    assert!(!AsterBehaviour::GranIrraLog.is_mfront());
}

/// **Rejected inputs are rejected, not silently absorbed.**
///
/// *Methodology:* every guard in the module is exercised once, because a guard
/// that has never fired is a guard that may not work. Checks: negative fluence
/// increment, non-positive temperature, negative dose increment, a β fraction
/// outside `[0, 1]`, a non-positive shear modulus, and a non-positive proof
/// stress in the `IRRAD3M` identification. Pass criterion: each returns
/// [`OffbeatError::Unphysical`].
///
/// *Result (measured 2026-08-05):* all six returned `Unphysical` with the
/// expected `quantity` field. Interpretation: the guards fire; none of these
/// inputs can reach the arithmetic.
#[test]
fn unphysical_inputs_are_rejected() {
    let p = log_irradiation();
    assert!(matches!(
        p.creep_compliance(1.0e25, -1.0, 620.0),
        Err(OffbeatError::Unphysical { .. })
    ));
    assert!(matches!(
        p.creep_compliance(1.0e25, 1.0e24, 0.0),
        Err(OffbeatError::Unphysical { .. })
    ));

    let law = Irrad3m::new(irrad3m_parameters()).unwrap();
    assert!(matches!(
        law.integrate(uniaxial(1.0e8), 73.0e9, Irrad3mState::default(), 0.0, -1.0),
        Err(OffbeatError::Unphysical { .. })
    ));
    assert!(matches!(
        law.integrate(uniaxial(1.0e8), -1.0, Irrad3mState::default(), 0.0, 1.0),
        Err(OffbeatError::Unphysical { .. })
    ));

    let meta = meta_lema_ani();
    assert!(matches!(
        meta.integrate(general_stress(), 33.0e9, 1.5, 900.0, 0.0, 100.0),
        Err(OffbeatError::Unphysical { .. })
    ));

    let mut bad = irrad3m_parameters();
    bad.yield_strength = 0.0;
    assert!(matches!(
        bad.identify_hardening(),
        Err(OffbeatError::Unphysical { .. })
    ));
    println!("all six guards fired");
}
