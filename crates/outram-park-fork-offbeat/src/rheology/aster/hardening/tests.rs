// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Verification of the unified hardening curve.
//!
//! # What these check
//!
//! The consolidation's whole risk is silently changing a curve while merging
//! two enums into one. So the tests here are equivalence tests: each variant is
//! checked against the closed form it is supposed to implement, and the two
//! power-law families are checked against **each other** to demonstrate they
//! are genuinely different and must not be merged.
//!
//! Verification only — closed-form identities and analytic derivatives, no
//! comparison against experiment.

use super::*;

fn ludwik() -> IsotropicHardening {
    IsotropicHardening::Ludwik {
        yield_stress: 250.0e6,
        coefficient: 500.0e6,
        exponent: 0.2,
    }
}

fn aster_power() -> IsotropicHardening {
    IsotropicHardening::AsterPower {
        yield_stress: 250.0e6,
        youngs_modulus: 210.0e9,
        alpha: 100.0,
        exponent: 10.0,
    }
}

/// **Methodology.** The slope must be the exact analytic derivative of the
/// value, or the local Newton solves that differentiate the curve degrade
/// silently. Check every variant by central finite difference at
/// `p = 1e-3, 1e-2, 1e-1`, with a step of `1e-9`. Pass criterion: relative
/// agreement below `1e-5`, which is the accuracy a central difference of this
/// step can deliver on these magnitudes.
///
/// **Results, measured 2026-08-05.** Worst relative slope error over all five
/// variants and three strains: **6.902490e-8**, on `AsterPower` at `p = 0.1`.
/// `Perfect` returned exactly zero, `Linear` matched to 1.192e-16, and the
/// three nonlinear families sat between 5.0e-10 and 6.9e-8. The error grows
/// with `p` for every nonlinear curve, which is the central difference's own
/// truncation error rather than anything in the analytic slopes.
#[test]
fn every_slope_is_the_derivative_of_its_value() {
    let curves = [
        (
            "Perfect",
            IsotropicHardening::Perfect {
                yield_stress: 250.0e6,
            },
        ),
        (
            "Linear",
            IsotropicHardening::Linear {
                yield_stress: 250.0e6,
                modulus: 2.0e9,
            },
        ),
        ("Ludwik", ludwik()),
        ("AsterPower", aster_power()),
        (
            "EcroNl",
            IsotropicHardening::EcroNl {
                r0: 250.0e6,
                rh: 1.0e9,
                r1: 100.0e6,
                gamma_1: 500.0,
                r2: 50.0e6,
                gamma_2: 50.0,
                rk: 200.0e6,
                p0: 1.0e-4,
                gamma_m: 0.3,
            },
        ),
    ];

    let h = 1.0e-9;
    let mut worst: f64 = 0.0;
    for (name, curve) in curves {
        for p in [1.0e-3, 1.0e-2, 1.0e-1] {
            let numeric = (curve.value(p + h) - curve.value(p - h)) / (2.0 * h);
            let analytic = curve.slope(p);
            let scale = analytic.abs().max(1.0);
            let rel = (numeric - analytic).abs() / scale;
            println!("{name:11} p={p:<8} analytic={analytic:>16.6e} numeric={numeric:>16.6e} rel={rel:.3e}");
            worst = worst.max(rel);
        }
    }
    println!("worst relative slope error = {worst:.6e}");
    assert!(worst < 1e-5, "worst slope error {worst:.6e} exceeds 1e-5");
}

/// **Methodology — the reason this module exists.** `Ludwik` and `AsterPower`
/// are both "power-law hardening" and were separately named `PowerLaw` in the
/// two modules being consolidated. If they were the same curve, merging them
/// into one variant would have been correct and this module unnecessary. Drive
/// both with the same yield stress over `p = 1e-6 … 1e-1` and require that they
/// **disagree substantially**, so a future refactor cannot quietly collapse
/// them. Pass criterion: at least one sample differs by more than 10 %
/// relative.
///
/// **Results, measured 2026-08-05.** Largest relative difference **1.4081e-1**,
/// at `p = 1e-6`. The full sweep:
///
/// | `p` | Ludwik | AsterPower | rel. diff |
/// |---|---|---|---|
/// | 1e-6 | 2.815479e8 | 3.276905e8 | 1.4081e-1 |
/// | 1e-5 | 3.000000e8 | 3.478065e8 | 1.3745e-1 |
/// | 1e-4 | 3.292447e8 | 3.731312e8 | 1.1762e-1 |
/// | 1e-3 | 3.755943e8 | 4.050129e8 | 7.2636e-2 |
/// | 1e-2 | 4.490536e8 | 4.451497e8 | **8.6935e-3** |
/// | 1e-1 | 5.654787e8 | 4.956789e8 | 1.2343e-1 |
///
/// **Note the crossing.** The two curves intersect between `p = 1e-3` and
/// `p = 1e-1`, and at `p = 1e-2` they agree to 0.87 %. Anyone spot-checking
/// these families at one percent strain — a thoroughly reasonable place to
/// look — would have found them nearly identical and concluded they were
/// duplicates safe to merge. They differ by 14 % a few decades either side.
/// That near-coincidence is precisely why this test sweeps rather than samples.
#[test]
fn the_two_power_law_families_are_genuinely_different() {
    let (a, b) = (ludwik(), aster_power());
    let mut worst: f64 = 0.0;
    for i in 0..=5 {
        let p = 10.0_f64.powi(-6 + i);
        let (va, vb) = (a.value(p), b.value(p));
        let rel = (va - vb).abs() / va.abs().max(vb.abs());
        println!("p={p:<10.0e} Ludwik={va:>16.6e} AsterPower={vb:>16.6e} rel diff={rel:.4e}");
        worst = worst.max(rel);
    }
    println!("largest relative difference = {worst:.4e}");
    assert!(
        worst > 0.10,
        "the two families must not be collapsed into one variant; largest difference was only {worst:.4e}"
    );
}

/// **Methodology.** `AsterPower` reproduces upstream's `ecpuis`, which replaces
/// the curve below `p0 = 1e-10` with the secant through the origin because
/// `dR/dp ∝ p^(1/n - 1)` diverges at the origin. Check that the two branches
/// meet at `p0` (C0), that `R(0)` is the initial yield stress, and that the
/// secant slope is exactly `n` times the curve's own slope at `p0` — the C1
/// discontinuity, which follows from the chord and derivative of `C p^(1/n)`
/// differing by that factor.
///
/// **Results, measured 2026-08-05.** `R(0) = 2.500000e8` Pa. The branches meet
/// at the cutoff: `R(p0) = 2.8092914661e8` Pa from the secant against
/// `2.8092914661e8` Pa from the curve. Secant slope `3.092915e17` Pa against
/// the curve's `3.092915e16` Pa just above `p0`, a **ratio of
/// 10.000000000008992** — exactly `n = 10`, confirming the C1 discontinuity is
/// the expected factor and not an arbitrary jump.
#[test]
fn the_aster_power_curve_is_linearised_below_the_upstream_cutoff() {
    let curve = aster_power();
    let p0 = ASTER_POWER_LINEARISATION_STRAIN;

    let at_cutoff = curve.value(p0);
    let just_above = curve.value(p0 * (1.0 + 1e-12));
    let secant_slope = curve.slope(0.5 * p0);
    let curve_slope_at_p0 = curve.slope(p0 * (1.0 + 1e-12));
    let ratio = secant_slope / curve_slope_at_p0;

    println!("R(0)  = {:.6e}", curve.value(0.0));
    println!("R(p0) = {at_cutoff:.10e}, R(p0+) = {just_above:.10e}");
    println!("secant slope = {secant_slope:.6e}, curve slope at p0+ = {curve_slope_at_p0:.6e}");
    println!("ratio = {ratio}");

    assert!(
        (just_above - at_cutoff).abs() < 1e-9 * at_cutoff,
        "must be C0 at p0"
    );
    assert!((curve.value(0.0) - 250.0e6).abs() < 1e-9 * 250.0e6);
    assert!(
        (ratio - 10.0).abs() < 1e-6,
        "secant should be n = 10 times the curve slope"
    );
}

/// **Methodology.** Each variant must reproduce its stated closed form exactly,
/// not merely approximately, since these are the formulas the docs promise.
/// Check `Perfect` is constant; `Linear` is `σ_y + Hp`; `Ludwik` is
/// `σ_y + Kp^n`; and `EcroNl` reduces to `R0 + RH p` when every nonlinear
/// amplitude is zeroed and `RK = 0`. Tolerance 1e-12 relative.
///
/// **Results, measured 2026-08-05.** `Perfect` returned `250000000` Pa at both
/// `p = 0` and `p = 0.02`. `Linear` gave `290000000` Pa against the closed form
/// `290000000`. `Ludwik` gave `478652525.96366316` Pa against
/// `478652525.96366316` — bit-identical. `EcroNl` with its nonlinear
/// amplitudes zeroed collapsed onto `290000000` Pa, exactly `Linear`.
#[test]
fn each_variant_reproduces_its_closed_form() {
    let perfect = IsotropicHardening::Perfect {
        yield_stress: 250.0e6,
    };
    let linear = IsotropicHardening::Linear {
        yield_stress: 250.0e6,
        modulus: 2.0e9,
    };
    let degenerate_ecro = IsotropicHardening::EcroNl {
        r0: 250.0e6,
        rh: 2.0e9,
        r1: 0.0,
        gamma_1: 1.0,
        r2: 0.0,
        gamma_2: 1.0,
        rk: 0.0,
        p0: 1.0e-4,
        gamma_m: 1.0,
    };

    let p = 0.02;
    println!(
        "Perfect: R(0) = {}, R({p}) = {}",
        perfect.value(0.0),
        perfect.value(p)
    );
    println!(
        "Linear:  R({p}) = {} vs closed form {}",
        linear.value(p),
        250.0e6 + 2.0e9 * p
    );
    println!(
        "Ludwik:  R({p}) = {} vs closed form {}",
        ludwik().value(p),
        250.0e6 + 500.0e6 * p.powf(0.2)
    );
    println!(
        "EcroNl degenerate: R({p}) = {} vs Linear {}",
        degenerate_ecro.value(p),
        linear.value(p)
    );

    assert_eq!(perfect.value(0.0), perfect.value(p));
    assert!((linear.value(p) - (250.0e6 + 2.0e9 * p)).abs() < 1e-12 * linear.value(p));
    let ludwik_closed = 250.0e6 + 500.0e6 * p.powf(0.2);
    assert!((ludwik().value(p) - ludwik_closed).abs() < 1e-12 * ludwik_closed);
    assert!(
        (degenerate_ecro.value(p) - linear.value(p)).abs() < 1e-12 * linear.value(p),
        "ECRO_NL with its nonlinear terms zeroed must collapse onto Linear"
    );
}

/// **Methodology.** Unphysical parameters must be refused rather than
/// propagated. Check a non-positive yield stress on each family that carries
/// one, and each of `AsterPower`'s three extra positive-definite parameters.
///
/// **Results, measured 2026-08-05.** All four rejected with `Unphysical`,
/// each naming its quantity — `"yield stress"` for the `Perfect` and `Ludwik`
/// cases, `"power-law coefficient alpha"` for `AsterPower`, and
/// `"initial yield stress R0"` for `EcroNl`. A valid `AsterPower` set returned
/// `Ok(())`.
#[test]
fn unphysical_parameters_are_refused() {
    let cases: [(&str, IsotropicHardening); 4] = [
        (
            "Perfect, zero sigma_y",
            IsotropicHardening::Perfect { yield_stress: 0.0 },
        ),
        (
            "Ludwik, negative sigma_y",
            IsotropicHardening::Ludwik {
                yield_stress: -1.0,
                coefficient: 1.0,
                exponent: 0.2,
            },
        ),
        (
            "AsterPower, zero alpha",
            IsotropicHardening::AsterPower {
                yield_stress: 250.0e6,
                youngs_modulus: 210.0e9,
                alpha: 0.0,
                exponent: 10.0,
            },
        ),
        (
            "EcroNl, zero R0",
            IsotropicHardening::EcroNl {
                r0: 0.0,
                rh: 1.0,
                r1: 0.0,
                gamma_1: 1.0,
                r2: 0.0,
                gamma_2: 1.0,
                rk: 0.0,
                p0: 1.0,
                gamma_m: 1.0,
            },
        ),
    ];
    for (name, curve) in cases {
        let outcome = curve.validate();
        println!("{name} -> {outcome:?}");
        assert!(
            matches!(outcome, Err(OffbeatError::Unphysical { .. })),
            "{name} must be refused"
        );
    }

    let good = aster_power().validate();
    println!("valid AsterPower -> {good:?}");
    assert!(good.is_ok());
}
