// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Verification tests for isotropic hardening and Norton-Hoff limit analysis.
//!
//! # Scope
//!
//! **Verification**, not validation: each check compares against a closed-form
//! limit of the law's own equations, against upstream's algebra, or against an
//! invariant the radial return must satisfy. Nothing here is compared against a
//! tensile test or a measured collapse load, and no such agreement is claimed —
//! that is the maintainer's validation work.
//!
//! # Common parameters
//!
//! A mild-steel-like set, chosen only so the numbers are recognisable:
//! `σ_y = 250` MPa, `E = 210` GPa, `ν = 0.3` hence `μ = E/(2(1+ν)) =
//! 80.769…` GPa. Power-law hardening uses `α = 100`, `n = 10`.

use super::*;
use crate::rheology::aster::hardening::ASTER_POWER_LINEARISATION_STRAIN;
use crate::rheology::aster::kinematics::AsterVoigt;
use outram_foam_basic_lib::primitives::SymmTensor;

const YIELD_STRESS: f64 = 250.0e6;
const YOUNGS_MODULUS: f64 = 210.0e9;
const POISSON_RATIO: f64 = 0.3;

/// Shear modulus `μ = E / (2(1+ν))` \[Pa\].
fn shear_modulus() -> f64 {
    YOUNGS_MODULUS / (2.0 * (1.0 + POISSON_RATIO))
}

fn linear(hardening_modulus: f64) -> IsotropicHardening {
    IsotropicHardening::Linear {
        yield_stress: YIELD_STRESS,
        modulus: hardening_modulus,
    }
}

fn power_law() -> IsotropicHardening {
    IsotropicHardening::AsterPower {
        yield_stress: YIELD_STRESS,
        youngs_modulus: YOUNGS_MODULUS,
        alpha: 100.0,
        exponent: 10.0,
    }
}

// ── Hardening curves ─────────────────────────────────────────────────────────

/// **Methodology.** Linear hardening is `R(p) = σ_y + H p` with constant slope
/// `H`. Check that `R(0)` is the initial yield stress, that `R` is affine (the
/// second difference over an evenly spaced sample vanishes), and that
/// `slope` returns `H` everywhere. Inputs `σ_y = 250` MPa,
/// `H = 2` GPa. Tolerance 1e-9 relative. Reference: upstream `nmisot`'s
/// `_LINE` branch, `rp = sigy + rprim*(pm+dp)`.
///
/// **Results.** Measured `R(0) = 250000000`, `R(0.01) = 270000000`,
/// `R(0.02) = 290000000` Pa; second difference `0` exactly; slope
/// `2000000000` Pa. The curve is affine to the last bit — not merely to
/// tolerance — as a two-parameter linear law should be. Taken 2026-08-05.
#[test]
fn linear_hardening_is_affine_with_the_given_slope() {
    let h = linear(2.0e9);
    let r0 = h.value(0.0);
    let r1 = h.value(0.01);
    let r2 = h.value(0.02);
    let second_difference = r2 - 2.0 * r1 + r0;
    println!("R(0) = {r0}, R(0.01) = {r1}, R(0.02) = {r2}");
    println!(
        "second difference = {second_difference:e}, slope = {}",
        h.slope(0.005)
    );

    assert!((r0 - YIELD_STRESS).abs() < 1e-9 * YIELD_STRESS);
    assert!(second_difference.abs() < 1e-9 * YIELD_STRESS);
    assert!((h.slope(0.005) - 2.0e9).abs() < 1e-9 * 2.0e9);
}

/// **Methodology.** Upstream's `ecpuis` replaces the power-law curve below
/// `p0 = 1e-10` with the secant line through the origin, because
/// `R'(p) ∝ p^(1/n - 1)` diverges as `p → 0` for `n > 1`. Verify that the curve
/// is continuous **at** the switch point, that `R(0)` is still the initial
/// yield stress, and that both the linearised and the true slope are finite.
/// Pass criterion: continuity to 1e-9 relative; slopes finite; `R(0) = σ_y`.
///
/// **Results.** Measured `R(p0) = 280929146.6146007` Pa from the linearised
/// branch against `280929146.6146038` Pa from the curve — agreeing to 11
/// significant figures, so the two branches meet. `R(0) = 250000000` Pa.
/// Slopes: `3.092914661460072e17` Pa below the cutoff, `3.8937488638228415e15`
/// Pa at `10 p0`.
///
/// **Two corrections to what one would guess.** First, the linearised slope is
/// *larger* than the curve's, not smaller — the secant over `[0, p0]` comes out
/// exactly `n` times the true slope at `p0`, because for `R - σ_y = C p^(1/n)`
/// the chord `(R(p0) - σ_y)/p0` and the derivative `(1/n)(R(p0) - σ_y)/p0`
/// differ by precisely that factor. So the curve is **C0 but not C1**: the
/// derivative jumps by a factor of 10 at the cutoff for this `n = 10`. Second,
/// the guard is not there to *reduce* a large slope but to *bound* an unbounded
/// one — `R' ∝ p^(-0.9)` has no limit at the origin, and a finite 3.09e17 is
/// what replaces it. Taken 2026-08-05.
#[test]
fn the_power_law_curve_is_linearised_below_the_upstream_cutoff() {
    let h = power_law();
    let p0 = ASTER_POWER_LINEARISATION_STRAIN;

    let at_cutoff = h.value(p0);
    let just_above = h.value(p0 * (1.0 + 1e-12));
    let slope_below = h.slope(0.5 * p0);
    let slope_above = h.slope(p0 * 10.0);

    println!("R(p0) = {at_cutoff}, R(p0+) = {just_above}");
    println!("slope below cutoff = {slope_below:e}, slope at 10*p0 = {slope_above:e}");
    println!("R(0) = {}", h.value(0.0));

    assert!(
        (just_above - at_cutoff).abs() < 1e-9 * at_cutoff,
        "curve must be continuous at p0"
    );
    assert!(slope_below.is_finite());
    assert!(slope_above.is_finite());
    assert!((h.value(0.0) - YIELD_STRESS).abs() < 1e-9 * YIELD_STRESS);
}

/// **Methodology.** A hardening curve must be monotone increasing in `p`, or
/// the yield surface would shrink under continued plastic flow and the radial
/// return could admit multiple roots. Sweep `p` over
/// `1e-12 … 1e-1` logarithmically for the power law and require strict
/// increase, with a strictly positive slope at each sample.
///
/// **Results.** Over `p = 1e-12 … 1e-1`, `R` rose from `2.50309291466146e8` to
/// `4.956789443190429e8` Pa, strictly increasing at every sample, with `R' > 0`
/// throughout. The slope fell from `3.092914661460072e17` to
/// `2.4567894431904286e8` Pa — nine orders of magnitude across the sweep. The
/// first three samples (`1e-12`, `1e-11`, `1e-10`) all report the *same* slope
/// `3.092914661460072e17`, which is the signature of the linearised branch: the
/// secant has constant slope by construction, and the cutoff sits at `1e-10`.
/// Taken 2026-08-05.
#[test]
fn the_power_law_curve_is_monotone_increasing() {
    let h = power_law();
    let mut previous = f64::NEG_INFINITY;
    for i in 0..=11 {
        let p = 10.0_f64.powi(-12 + i);
        let r = h.value(p);
        let slope = h.slope(p);
        println!("p = {p:e}  ->  R = {r:e}, R' = {slope:e}");
        assert!(
            r > previous,
            "R must increase: {r} not > {previous} at p = {p:e}"
        );
        assert!(
            slope > 0.0,
            "slope must be positive, got {slope} at p = {p:e}"
        );
        previous = r;
    }
}

// ── Radial return ────────────────────────────────────────────────────────────

/// **Methodology.** A trial stress below the current yield radius is an elastic
/// step: upstream's `seuil = sieleq - rp` is non-positive and it sets `dp = 0`
/// without iterating. Check that a trial equivalent stress of 200 MPa against a
/// 250 MPa yield returns `None`, and that the boundary case
/// `σ_eq = R(p_m)` exactly is also elastic (upstream's `<= 0`, not `< 0`).
///
/// **Results.** Both returned `None`: 200 MPa against a 250 MPa yield, and a
/// trial exactly equal to the yield stress. The boundary case matters — a `<`
/// instead of upstream's `<=` would send an on-surface state through the
/// plastic branch to solve for a multiplier that is identically zero.
/// Taken 2026-08-05.
#[test]
fn a_trial_stress_below_yield_is_an_elastic_step() {
    let h = linear(2.0e9);
    let control = SolverControl::default();

    let below = h
        .radial_return(200.0e6, shear_modulus(), 0.0, &control)
        .unwrap();
    let exactly_at = h
        .radial_return(YIELD_STRESS, shear_modulus(), 0.0, &control)
        .unwrap();
    println!("below yield -> {below:?}");
    println!("exactly at yield -> {exactly_at:?}");

    assert!(
        below.is_none(),
        "200 MPa against a 250 MPa yield must be elastic"
    );
    assert!(
        exactly_at.is_none(),
        "the boundary is elastic in upstream (seuil <= 0)"
    );
}

/// **Methodology.** For linear hardening the return has the closed form
/// `Δp = (σ_eq - σ_y - H p_m) / (H + 3μ)`, which upstream uses directly rather
/// than iterating. Drive it with `σ_eq = 400` MPa, `H = 2` GPa, `p_m = 0`, and
/// compare against that expression evaluated independently. Pass criterion:
/// relative agreement to 1e-14, zero iterations reported, and the residual on
/// the yield surface below 1e-6 Pa — i.e. the returned point genuinely lies on
/// the surface, not merely near it.
///
/// **Results.** Measured `Δp = 0.0006139798488664988` against the closed form
/// `0.0006139798488664988`, relative error `0`; residual `0` Pa exactly;
/// `0` iterations. Bit-identical to the closed form, and the returned point is
/// exactly on the yield surface rather than near it. Taken 2026-08-05.
#[test]
fn linear_hardening_return_matches_its_closed_form() {
    let hardening_modulus = 2.0e9;
    let h = linear(hardening_modulus);
    let mu = shear_modulus();
    let trial = 400.0e6;

    let solution = h
        .radial_return(trial, mu, 0.0, &SolverControl::default())
        .unwrap()
        .expect("400 MPa against a 250 MPa yield must be plastic");

    let closed_form = (trial - YIELD_STRESS) / (hardening_modulus + 3.0 * mu);
    let rel = ((solution.root - closed_form) / closed_form).abs();
    println!(
        "delta_p = {}, closed form = {closed_form}, rel err = {rel:e}",
        solution.root
    );
    println!(
        "residual = {:e}, iterations = {}",
        solution.residual, solution.iterations
    );

    assert!(rel < 1e-14);
    assert_eq!(
        solution.iterations, 0,
        "the linear case is closed-form, not iterated"
    );
    assert!(solution.residual.abs() < 1e-6);
}

/// **Methodology — the invariant that matters most.** Whatever the hardening
/// curve, the returned stress must lie *on* the yield surface: upstream's
/// residual `R(p_m + Δp) + 3μΔp - σ_eq^trial` must vanish. This is the
/// definition of the return, so a violation means the solve is wrong regardless
/// of how plausible `Δp` looks. Check both curves across trial stresses of
/// 300, 400, 600 and 1000 MPa, at `p_m = 0` and `p_m = 0.02`.
///
/// Two specification points, both settled by what the code actually does:
///
/// - Not every combination is plastic. Power-law hardening at `p_m = 0.02` has
///   already raised the yield radius well above 300 MPa, so that case is
///   *correctly* elastic and returns `None`. The test asserts the elastic
///   branch is justified — `σ_eq ≤ R(p_m)` — rather than assuming plasticity.
/// - The pass criterion is **relative**, not an absolute number of pascals.
///   An absolute tolerance is the wrong specification here: the attainable
///   residual is roughly `R'(p) × step_tol`, and the power-law slope near
///   `p → 0` is enormous, so no solver can drive the residual to a small
///   number of pascals at a root of order `1e-8`. Criterion: closure relative
///   to the trial stress below `1e-8`.
///
/// **Results.** 14 plastic and 2 elastic cases; worst relative residual
/// `6.478915611902873e-10`, well inside the `1e-8` criterion.
///
/// All eight `_LINE` cases closed to residual `0` Pa exactly in `0` iterations,
/// as the closed form must. The `_PUIS` cases took 4 to 19 Brent iterations,
/// with residuals from `0` to `1.9436746835708618e-1` Pa. That worst case is
/// the shallowest step (`trial = 3e8`, `p_m = 0`, `Δp = 1.2183281068525094e-8`)
/// and it is the conditioning effect the criterion anticipates, not a solver
/// defect: at a root of order `1e-8` the step tolerance times the local
/// hardening slope — which is of order `1e15` Pa there — is around a tenth of a
/// pascal. Relative to the 300 MPa trial stress that is a closure of `6.5e-10`.
///
/// The two elastic verdicts are both `_PUIS` at `p_m = 0.02`, where hardening
/// has already lifted the yield radius to `4.591562934215621e8` Pa — above the
/// 300 and 400 MPa trials. Correct behaviour, and a reminder that "high trial
/// stress" does not imply "plastic" once the material has hardened.
/// Taken 2026-08-05.
#[test]
fn the_returned_stress_lands_on_the_yield_surface() {
    let mu = shear_modulus();
    let control = SolverControl::default();
    let mut worst_relative: f64 = 0.0;
    let mut plastic_cases = 0;
    let mut elastic_cases = 0;

    for h in [linear(2.0e9), power_law()] {
        for &trial in &[300.0e6, 400.0e6, 600.0e6, 1000.0e6] {
            for &pm in &[0.0, 0.02] {
                match h.radial_return(trial, mu, pm, &control).unwrap() {
                    None => {
                        elastic_cases += 1;
                        let radius = h.value(pm);
                        println!(
                            "{} trial = {trial:e} pm = {pm}  ->  ELASTIC (R(pm) = {radius:e})",
                            h.aster_name_suffix().unwrap_or("?")
                        );
                        assert!(
                            trial <= radius,
                            "an elastic verdict requires the trial stress below the \
                             yield radius: {trial:e} > {radius:e}"
                        );
                    }
                    Some(solution) => {
                        plastic_cases += 1;
                        let residual = h.return_residual(solution.root, trial, 3.0 * mu, pm);
                        let relative = residual.abs() / trial;
                        println!(
                            "{} trial = {trial:e} pm = {pm}  ->  delta_p = {:e}, residual = {residual:e} Pa \
                             ({relative:e} relative), iters = {}",
                            h.aster_name_suffix().unwrap_or("?"),
                            solution.root,
                            solution.iterations
                        );
                        assert!(
                            solution.root > 0.0,
                            "a plastic step needs a positive multiplier"
                        );
                        worst_relative = worst_relative.max(relative);
                    }
                }
            }
        }
    }

    println!(
        "{plastic_cases} plastic, {elastic_cases} elastic; worst relative residual = {worst_relative:e}"
    );
    assert!(
        worst_relative < 1e-8,
        "worst relative residual {worst_relative:e} exceeds tolerance"
    );
}

/// **Methodology.** Perfect plasticity (`H = 0`) is the limit in which the
/// yield surface never grows, so the return must bring the equivalent stress
/// back to exactly `σ_y` no matter how far the trial overshot, giving
/// `Δp = (σ_eq - σ_y) / 3μ`. Drive with 400 MPa and 1000 MPa. Pass criterion:
/// the returned radius equals `σ_y` to 1e-9 relative, and `Δp` matches the
/// closed form to 1e-14.
///
/// **Results.** At `trial = 4e8` Pa, `Δp = 6.19047619047619e-4` against the
/// closed form `6.19047619047619e-4`; at `trial = 1e9` Pa,
/// `Δp = 3.095238095238095e-3` against `3.095238095238095e-3`. Both
/// bit-identical. The returned radius was `2.5e8` Pa in both cases — exactly
/// the initial yield stress, unchanged by a multiplier five times larger in the
/// second case, which is what "perfect" plasticity means. Taken 2026-08-05.
#[test]
fn perfect_plasticity_returns_exactly_to_the_initial_yield_stress() {
    let h = linear(0.0);
    let mu = shear_modulus();

    for &trial in &[400.0e6, 1000.0e6] {
        let solution = h
            .radial_return(trial, mu, 0.0, &SolverControl::default())
            .unwrap()
            .expect("both trials exceed yield");
        let closed_form = (trial - YIELD_STRESS) / (3.0 * mu);
        let returned_radius = h.value(solution.root);
        println!(
            "trial = {trial:e}  ->  delta_p = {:e} (closed form {closed_form:e}), returned radius = {returned_radius:e}",
            solution.root
        );
        assert!(((solution.root - closed_form) / closed_form).abs() < 1e-14);
        assert!((returned_radius - YIELD_STRESS).abs() < 1e-9 * YIELD_STRESS);
    }
}

/// **Methodology.** Softening at or beyond `H = -3μ` makes the return residual
/// non-increasing in `Δp`, so no unique root exists — the material sheds stress
/// faster than elastic unloading can follow. The closed form still evaluates to
/// a finite number there, so a port that does not guard it returns a plausible
/// wrong answer silently. Upstream does not guard it; this port does. Check
/// that `H = -3μ` exactly and `H = -4μ` both error, while a mild `H = -0.1μ`
/// still solves. Pass criterion: `Unphysical` for the first two, a finite root
/// for the third.
///
/// **Results.** `H = -3μ = -242307692307.69232` Pa and
/// `H = -4μ = -323076923076.9231` Pa both returned `Unphysical` with the
/// non-uniqueness reason. Mild softening `H = -0.1μ` still solved, giving
/// `Δp = 6.403940886699507e-4` — larger than the `6.139798488664988e-4` the
/// same trial produces under `H = +2` GPa hardening, as softening should.
/// Taken 2026-08-05.
#[test]
fn softening_beyond_the_uniqueness_limit_is_rejected() {
    let mu = shear_modulus();
    let control = SolverControl::default();

    for &factor in &[-3.0, -4.0] {
        let outcome = linear(factor * mu).radial_return(400.0e6, mu, 0.0, &control);
        println!("H = {factor}*mu  ->  {outcome:?}");
        assert!(
            matches!(outcome, Err(OffbeatError::Unphysical { .. })),
            "H = {factor}*mu must be rejected as non-unique"
        );
    }

    let mild = linear(-0.1 * mu)
        .radial_return(400.0e6, mu, 0.0, &control)
        .unwrap()
        .expect("mild softening is still a plastic step");
    println!("H = -0.1*mu  ->  delta_p = {:e}", mild.root);
    assert!(mild.root.is_finite() && mild.root > 0.0);
}

/// **Methodology.** Unphysical inputs must be refused rather than propagated: a
/// negative trial equivalent stress (a norm cannot be negative), a negative
/// accumulated plastic strain (it is monotone), a non-positive shear modulus,
/// and a non-positive yield stress. Pass criterion: every one returns
/// `Unphysical`.
///
/// **Results.** All four returned `Unphysical`, each naming the offending
/// quantity: `"trial equivalent stress"` at `-1.0` Pa, `"accumulated equivalent
/// plastic strain"` at `-0.001`, `"shear modulus"` at `0.0` Pa, and
/// `"yield stress"` at `0.0` Pa. The error carries the quantity, value, unit
/// and reason, so a caller can report which cell and which input failed rather
/// than only that something did. Taken 2026-08-05.
#[test]
fn unphysical_inputs_are_refused() {
    let h = linear(2.0e9);
    let mu = shear_modulus();
    let control = SolverControl::default();

    let cases: [(&str, Result<Option<LocalSolution>>); 4] = [
        (
            "negative trial stress",
            h.radial_return(-1.0, mu, 0.0, &control),
        ),
        (
            "negative accumulated strain",
            h.radial_return(400.0e6, mu, -1e-3, &control),
        ),
        (
            "zero shear modulus",
            h.radial_return(400.0e6, 0.0, 0.0, &control),
        ),
        (
            "zero yield stress",
            linear(2.0e9).radial_return(400.0e6, mu, 0.0, &control).and(
                IsotropicHardening::Linear {
                    yield_stress: 0.0,
                    modulus: 2.0e9,
                }
                .radial_return(400.0e6, mu, 0.0, &control),
            ),
        ),
    ];

    for (name, outcome) in cases {
        println!("{name}  ->  {outcome:?}");
        assert!(
            matches!(outcome, Err(OffbeatError::Unphysical { .. })),
            "{name} must be refused"
        );
    }
}

// ── Norton-Hoff ──────────────────────────────────────────────────────────────

/// **Methodology.** The continuation parameter drives `m = 1 + 10^(1-t)`. At
/// `t = 1` this is exactly 2 — the linear (Newtonian) starting point — and as
/// `t` grows it must decrease monotonically toward 1, the rigid-perfectly-
/// plastic limit. Sample `t = 1, 2, 3, 4, 5`. Pass criterion: `m(1) = 2`
/// exactly; strictly decreasing; every value above 1.
///
/// **Results.** Measured `m = 2, 1.1, 1.01, 1.001, 1.0001` at
/// `t = 1, 2, 3, 4, 5`, and `m(1) = 2` exactly. Each unit step of `t` closes
/// the remaining gap to the rigid-plastic limit by a factor of ten, so the
/// continuation is geometric in `t` — which is why limit-analysis runs
/// advance `t` in unit steps rather than by a fixed increment of `m`.
/// Taken 2026-08-05.
#[test]
fn the_norton_hoff_exponent_walks_from_linear_toward_rigid_plastic() {
    let mut previous = f64::INFINITY;
    for t in 1..=5 {
        let m = NortonHoffLimitAnalysis::exponent(f64::from(t));
        println!("t = {t}  ->  m = {m}");
        assert!(m < previous, "m must decrease with t: {m} not < {previous}");
        assert!(m > 1.0, "m must stay above the rigid-plastic limit of 1");
        previous = m;
    }
    let at_one = NortonHoffLimitAnalysis::exponent(1.0);
    println!("m(1) = {at_one}");
    assert_eq!(at_one, 2.0, "t = 1 must give exactly the linear exponent");
}

/// **Methodology.** At `t = 1` the exponent is 2 and the law collapses to
/// `σ = A ε` with `A = σ_y (2/3)` — linear in the strain. Verify linearity
/// directly: doubling the strain must exactly double the stress. Reference:
/// upstream's `line` branch, `coef = am`. Inputs: a uniaxial-like Mandel strain
/// `(1e-3, -3e-4, -3e-4, 0, 0, 0)`. Pass criterion: exact doubling to 1e-14
/// relative.
///
/// **Results.** Measured `σ(ε) = [166666.66666666666, -49999.99999999999,
/// -49999.99999999999, 0, 0, 0]` Pa and `σ(2ε) = [333333.3333333333,
/// -99999.99999999999, -99999.99999999999, 0, 0, 0]` Pa; ratio on the `xx`
/// component `2` exactly. At `t = 1` the law is a plain linear map, as the
/// `line` branch requires. Taken 2026-08-05.
#[test]
fn norton_hoff_is_linear_at_the_starting_pseudo_time() {
    let law = NortonHoffLimitAnalysis::new(YIELD_STRESS);
    let strain = AsterVoigt::from_tensor(SymmTensor::new(1.0e-3, 0.0, 0.0, -3.0e-4, 0.0, -3.0e-4));
    let doubled = AsterVoigt::from_tensor(SymmTensor::new(2.0e-3, 0.0, 0.0, -6.0e-4, 0.0, -6.0e-4));

    let s1 = law.stress(strain, 1.0).unwrap();
    let s2 = law.stress(doubled, 1.0).unwrap();
    let ratio = s2.components()[0] / s1.components()[0];

    println!("sigma(eps)   = {:?}", s1.components());
    println!("sigma(2 eps) = {:?}", s2.components());
    println!("ratio on the xx component = {ratio}");

    assert!(
        (ratio - 2.0).abs() < 1e-14,
        "t = 1 must be exactly linear, ratio = {ratio}"
    );
}

/// **Methodology — the whole point of the law.** As `t` grows the stress
/// magnitude must become independent of the strain magnitude, approaching
/// `A = σ_y (2/3)^(m/2)` — that plateau *is* the collapse state a limit-load
/// analysis is looking for. Take two strains differing by a factor of 100 and
/// track the ratio of their stress norms as `t` advances. Pass criterion: the
/// ratio falls monotonically toward 1 and is below 1.2 by `t = 5`.
///
/// **Results.** For two strains a factor of 100 apart, the ratio of stress
/// norms fell monotonically:
///
/// ```text
/// t = 1  m = 2.000000  ratio = 1e2
/// t = 2  m = 1.100000  ratio = 1.584893192461114
/// t = 3  m = 1.010000  ratio = 1.0471285480508996
/// t = 4  m = 1.001000  ratio = 1.0046157902783945
/// t = 5  m = 1.000100  ratio = 1.0004606230728401
/// ```
///
/// A hundredfold spread in strain produces a 0.05 % spread in stress by
/// `t = 5`. The stress norms themselves converge on `2.04e8` Pa — compare
/// `σ_y (2/3)^(m/2) → 250e6 × √(2/3) = 2.0412e8` Pa as `m → 1`, which the
/// `t = 1` small-strain norm `2.041241452319315e4` also carries as its
/// amplitude. That plateau is the collapse state a limit-load analysis reports.
/// Taken 2026-08-05.
#[test]
fn norton_hoff_stress_becomes_strain_magnitude_independent_as_t_advances() {
    let law = NortonHoffLimitAnalysis::new(YIELD_STRESS);
    let small = AsterVoigt::from_tensor(SymmTensor::new(1.0e-4, 0.0, 0.0, -5.0e-5, 0.0, -5.0e-5));
    let large = AsterVoigt::from_tensor(SymmTensor::new(1.0e-2, 0.0, 0.0, -5.0e-3, 0.0, -5.0e-3));

    let mut previous = f64::INFINITY;
    let mut last = f64::NAN;
    for t in 1..=5 {
        let t = f64::from(t);
        let rs = law.stress(small, t).unwrap().norm();
        let rl = law.stress(large, t).unwrap().norm();
        let ratio = rl / rs;
        println!(
            "t = {t}  ->  m = {:.6}, |sigma(small)| = {rs:e}, |sigma(large)| = {rl:e}, ratio = {ratio:e}",
            NortonHoffLimitAnalysis::exponent(t)
        );
        assert!(ratio < previous, "the ratio must fall toward 1");
        previous = ratio;
        last = ratio;
    }
    assert!(
        last < 1.2,
        "by t = 5 the stress should be nearly magnitude-independent, ratio = {last}"
    );
}

/// **Methodology — the branch that stops a NaN.** For `m < 2` the exponent
/// `m - 2` is negative, so `‖ε‖^(m-2)` at zero strain is an infinity and the
/// stress would be `inf × 0 = NaN`. Upstream guards this with the second half
/// of `line = inst .eq. 1 .or. epsno .eq. 0.d0`, and additionally disables
/// floating-point trapping around the routine. Feed a zero strain at `t = 4`
/// (where `m ≈ 1.001`) and require a finite, exactly-zero stress.
///
/// **Results.** At `t = 4`, `m = 1.001`, so the exponent `m - 2 = -0.999` is
/// negative and the unguarded expression would evaluate `0^(-0.999) = inf` and
/// then `inf × 0 = NaN`. Measured stress `[0, 0, 0, 0, 0, 0]` Pa — every
/// component finite and exactly zero. Taken 2026-08-05.
#[test]
fn norton_hoff_at_zero_strain_is_zero_and_not_a_nan() {
    let law = NortonHoffLimitAnalysis::new(YIELD_STRESS);
    let zero = AsterVoigt::from_components([0.0; 6]);

    let m = NortonHoffLimitAnalysis::exponent(4.0);
    let stress = law.stress(zero, 4.0).unwrap();
    println!(
        "m(4) = {m}, sigma at zero strain = {:?}",
        stress.components()
    );

    for (i, c) in stress.components().iter().enumerate() {
        assert!(c.is_finite(), "component {i} is not finite: {c}");
        assert_eq!(*c, 0.0, "component {i} must be exactly zero, got {c}");
    }
}

/// **Methodology.** `Norton-Hoff` has no internal state and no history, so the
/// stress must be a pure function of the current strain — calling it twice with
/// the same arguments, and in either order with different arguments, must give
/// identical results. This is worth pinning because every other law in this
/// module carries accumulated plastic strain, and a future refactor that adds
/// state here would silently break limit analysis. Pass criterion: bit-exact
/// equality.
///
/// **Results.** Both evaluations of the same strain gave
/// `[175158948.42978224, -52547684.528934665, -52547684.528934665, 0, 0, 0]`
/// Pa, bit-identical across an intervening call with a different strain.
/// Taken 2026-08-05.
#[test]
fn norton_hoff_is_stateless() {
    let law = NortonHoffLimitAnalysis::new(YIELD_STRESS);
    let a = AsterVoigt::from_tensor(SymmTensor::new(1.0e-3, 0.0, 0.0, -3.0e-4, 0.0, -3.0e-4));
    let b = AsterVoigt::from_tensor(SymmTensor::new(5.0e-3, 0.0, 0.0, -1.0e-3, 0.0, -2.0e-3));

    let first = law.stress(a, 3.0).unwrap();
    let _ = law.stress(b, 3.0).unwrap();
    let again = law.stress(a, 3.0).unwrap();

    println!("first = {:?}", first.components());
    println!("again = {:?}", again.components());
    assert_eq!(first.components(), again.components());
}

/// **Methodology.** The law's amplitude is `A = σ_y (2/3)^(m/2)`, so at fixed
/// `t` the stress must scale exactly linearly in the yield stress. Compare
/// `σ_y = 250` MPa against `σ_y = 500` MPa at `t = 3`. Pass criterion: the
/// stress ratio is exactly 2 to 1e-14.
///
/// **Results.** Measured `|σ| = 1.9027132078621832e8` Pa at `σ_y = 250` MPa
/// and `3.8054264157243663e8` Pa at `σ_y = 500` MPa; ratio `2` exactly.
/// Taken 2026-08-05.
#[test]
fn norton_hoff_stress_scales_linearly_with_the_yield_stress() {
    let strain = AsterVoigt::from_tensor(SymmTensor::new(1.0e-3, 0.0, 0.0, -3.0e-4, 0.0, -3.0e-4));
    let single = NortonHoffLimitAnalysis::new(YIELD_STRESS)
        .stress(strain, 3.0)
        .unwrap();
    let double = NortonHoffLimitAnalysis::new(2.0 * YIELD_STRESS)
        .stress(strain, 3.0)
        .unwrap();

    let ratio = double.norm() / single.norm();
    println!(
        "|sigma| at sy = {:e}, at 2 sy = {:e}, ratio = {ratio}",
        single.norm(),
        double.norm()
    );
    assert!((ratio - 2.0).abs() < 1e-14, "ratio = {ratio}");
}

/// **Methodology.** A non-positive yield stress has no meaning and must be
/// refused rather than producing a zero or sign-flipped stress field.
///
/// **Results.** `σ_y = 0` and `σ_y = -1e6` Pa both returned `Unphysical`
/// naming `"yield stress"`. Taken 2026-08-05.
#[test]
fn norton_hoff_refuses_a_non_positive_yield_stress() {
    let strain = AsterVoigt::from_tensor(SymmTensor::new(1.0e-3, 0.0, 0.0, 0.0, 0.0, 0.0));
    for sy in [0.0, -1.0e6] {
        let outcome = NortonHoffLimitAnalysis::new(sy).stress(strain, 2.0);
        println!("sy = {sy:e}  ->  {outcome:?}");
        assert!(matches!(outcome, Err(OffbeatError::Unphysical { .. })));
    }
}

/// **Methodology.** Consolidating the two `IsotropicHardening` enums widened
/// this radial return from two curve families to five: `Perfect` and `Linear`
/// take the closed form, and `Ludwik`, `AsterPower` and `EcroNl` are bracketed
/// with [`brent`]. Upstream's `nmisot` only ever reaches `_LINE` and `_PUIS`,
/// so the three new paths have no upstream branch to transcribe and must be
/// justified on the return's own defining property instead.
///
/// That property is the whole of the check: whatever solver produced `Δp`, the
/// returned stress must sit **on** the yield surface, i.e. the `nmcri2`
/// residual `R(p_m + Δp) + 3μ Δp - σ_eq^trial` must vanish. Each family is
/// driven with the same clearly-plastic trial state and the residual is
/// measured relative to `σ_eq^trial`.
///
/// Inputs: `σ_eq^trial = 400` MPa, `μ = 80.769…` GPa (from `E = 210` GPa,
/// `ν = 0.3`), `p_m = 0`. Every curve starts at `σ_y = 250` MPa so all five are
/// plastic at that trial. Pass criterion: `|residual| / σ_eq^trial < 1e-12`,
/// and `Δp > 0` — a return that yielded must actually advance the plastic
/// strain.
///
/// **Results (measured 2026-08-05, release).**
///
/// | Curve | `Δp` | residual \[Pa\] | relative | iters |
/// |---|---|---|---|---|
/// | `Perfect` | 6.190476190476190e-4 | 0 | 0 | 0 |
/// | `Linear` | 6.139798488664988e-4 | 0 | 0 | 0 |
/// | `Ludwik` | 2.320427670143995e-4 | 1.788139e-7 | 4.470e-16 | 9 |
/// | `AsterPower` | 1.072961087918394e-4 | 1.035929e-4 | 2.590e-13 | 9 |
/// | `EcroNl` | 5.935233776854477e-4 | 5.960464e-8 | 1.490e-16 | 5 |
///
/// The two closed-form families close **exactly** — a residual of literally
/// zero, not merely small — which is the reason they are not iterated.
///
/// `AsterPower` is three orders worse than the other two bracketed families and
/// sits only a factor 4 inside the pass criterion. That is not a defect in the
/// return: with `α = 100, n = 10` the curve's slope near the root is
/// `≈ 1.5e10 Pa`, so Brent converging `Δp` to its `f64` x-tolerance still
/// leaves `R` uncertain at the `1e-4` Pa level. The residual is the slope times
/// the bracket width, and the bracket is already at machine precision. Tighten
/// the criterion past `1e-13` and this family fails for arithmetic reasons
/// rather than physical ones.
#[test]
fn the_radial_return_lands_on_the_surface_for_every_curve_family() {
    let mu = shear_modulus();
    let control = SolverControl::default();
    let trial = 400.0e6;

    let curves = [
        (
            "Perfect",
            IsotropicHardening::Perfect {
                yield_stress: YIELD_STRESS,
            },
        ),
        ("Linear", linear(2.0e9)),
        (
            "Ludwik",
            IsotropicHardening::Ludwik {
                yield_stress: YIELD_STRESS,
                coefficient: 500.0e6,
                exponent: 0.2,
            },
        ),
        ("AsterPower", power_law()),
        (
            "EcroNl",
            IsotropicHardening::EcroNl {
                r0: YIELD_STRESS,
                rh: 1.5e9,
                r1: 300.0e6,
                gamma_1: 30.0,
                r2: 0.0,
                gamma_2: 1.0,
                rk: 0.0,
                p0: 1.0,
                gamma_m: 1.0,
            },
        ),
    ];

    for (name, curve) in curves {
        let solution = curve
            .radial_return(trial, mu, 0.0, &control)
            .unwrap_or_else(|e| panic!("{name}: {e:?}"))
            .unwrap_or_else(|| {
                panic!("{name}: sigma_eq = 400 MPa is above every sigma_y = 250 MPa")
            });
        let relative = solution.residual.abs() / trial;
        println!(
            "{name:<11} delta_p = {:e}  residual = {:e} Pa ({relative:e} relative)  iters = {}",
            solution.root, solution.residual, solution.iterations
        );

        assert!(solution.root > 0.0, "{name}: a plastic step must advance p");
        assert!(
            relative < 1.0e-12,
            "{name}: the return must land on the yield surface, got {relative:e} relative"
        );
    }
}
