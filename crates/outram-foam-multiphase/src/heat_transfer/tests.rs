// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Verification of the interfacial heat-transfer closures.
//!
//! **Verification, not validation**: each check compares against the
//! correlation's own closed-form limit, or against upstream's algebra
//! transcribed independently. Nothing is compared against a measured
//! heat-transfer coefficient and no such agreement is claimed.

use super::*;

/// **Methodology.** Ranz-Marshall's defining property is its stagnant limit:
/// as `Re → 0` it must return `Nu = 2` exactly — the analytical conduction
/// solution for an isolated sphere in an infinite stagnant medium. A
/// correlation that missed this would under-predict heat transfer to a
/// nearly-stationary bubble, silently.
///
/// Checks `Nu(Re=0) = 2` exactly, that `Nu` rises monotonically with `Re`, and
/// that `K` matches `6 α_d κ_c Nu / d²` transcribed independently from
/// upstream. Inputs: `κ_c = 0.6` W/(m·K), `d = 1` mm, `α_d = 0.1`, `Pr = 2`,
/// `α_res = 1e-6`. Pass criterion: stagnant limit exact; closed form to 1e-12
/// relative.
///
/// **Results (measured 2026-08-05, release).** Printed by the test.
#[test]
fn ranz_marshall_recovers_the_stagnant_sphere_limit() {
    let model = InterfacialHeatTransfer::RanzMarshall;
    let (kappa_c, d, alpha_d, pr, res) = (0.6, 1.0e-3, 0.1, 2.0, 1.0e-6);

    let nu0 = model.nusselt(0.9, 0.0, pr, res).expect("valid inputs");
    println!("Nu(Re=0, Pr={pr}) = {nu0}");
    assert_eq!(nu0, 2.0, "the stagnant limit must be exactly 2");

    let mut previous = nu0;
    for re in [1.0, 10.0, 100.0, 1000.0, 10_000.0] {
        let nu = model.nusselt(0.9, re, pr, res).expect("valid inputs");
        let closed = 2.0 + 0.6 * re.sqrt() * pr.cbrt();
        let k = model
            .volumetric_coefficient_si(alpha_d, 0.9, kappa_c, 0.02, d, re, pr, res)
            .expect("valid inputs");
        let k_closed = 6.0 * alpha_d * kappa_c * closed / (d * d);
        println!("Re = {re:>8}  Nu = {nu:.6}  K = {k:.6e} W/(m^3 K)");
        assert!((nu - closed).abs() < 1e-12 * closed);
        assert!((k - k_closed).abs() < 1e-12 * k_closed);
        assert!(nu > previous, "Nu must increase with Re");
        previous = nu;
    }
}

/// **Methodology.** The spherical closure is the *dispersed*-side resistance,
/// so it must read `κ_d` and be completely insensitive to `κ_c`, `Re` and
/// `Pr`. This is the one that is easy to wire up backwards — swapping the two
/// conductivities is a silent error worth roughly a factor of 10 for
/// steam/water — so the insensitivity is asserted directly rather than assumed.
///
/// Inputs: `κ_d = 0.02` W/(m·K) (steam), `κ_c = 0.6` W/(m·K) (water),
/// `d = 1` mm, `α_d = 0.1`. Pass criterion: `K` **bit-identical** across a 10⁴
/// sweep in `Re` and a 1000x change in `κ_c`; closed form to 1e-12 relative.
///
/// **Results (measured 2026-08-05, release).** Printed by the test.
#[test]
fn spherical_conduction_reads_only_the_dispersed_side() {
    let model = InterfacialHeatTransfer::Spherical;
    let (kappa_c, kappa_d, d, alpha_d, res) = (0.6, 0.02, 1.0e-3, 0.1, 1.0e-6);

    let baseline = model
        .volumetric_coefficient_si(alpha_d, 0.9, kappa_c, kappa_d, d, 0.0, 1.0, res)
        .expect("valid inputs");
    let closed = 60.0 * alpha_d * kappa_d / (d * d);
    println!("K = {baseline:.6e} W/(m^3 K), closed form {closed:.6e}");
    assert!((baseline - closed).abs() < 1e-12 * closed);
    assert_eq!(model.nusselt(0.9, 0.0, 1.0, res).unwrap(), 10.0);

    for (re, pr, kc) in [
        (1.0, 1.0, kappa_c),
        (1.0e4, 7.0, kappa_c),
        (0.0, 1.0, 1000.0 * kappa_c),
    ] {
        let k = model
            .volumetric_coefficient_si(alpha_d, 0.9, kc, kappa_d, d, re, pr, res)
            .expect("valid inputs");
        println!("Re={re:>8} Pr={pr} kappa_c={kc:>8} -> K = {k:.6e}");
        assert_eq!(
            k, baseline,
            "the dispersed-side closure must ignore Re, Pr and kappa_c"
        );
    }

    assert_eq!(model.resistance_side(), ResistanceSide::Dispersed);
    assert_eq!(
        InterfacialHeatTransfer::RanzMarshall.resistance_side(),
        ResistanceSide::Continuous
    );
}

/// **Methodology.** Gunn's voidage polynomial must collapse to the same
/// stagnant-sphere limit as Ranz-Marshall when the continuous phase fills the
/// cell: at `α_c = 1` the leading bracket is `7 − 10 + 5 = 2`, so `Nu → 2` as
/// `Re → 0`. Checking it pins those coefficients, the easiest thing in the
/// module to mistype.
///
/// The test also records how far Gunn and Ranz-Marshall diverge at finite `Re`
/// *even in the dilute limit*, because a reader might reasonably expect them to
/// agree there and they do not.
///
/// Inputs: `Pr = 2`, `α_res = 1e-6`, swept over `Re` and `α_c`. Pass criterion:
/// `Nu(α_c=1, Re=0) = 2` to 1e-12; a denser bed strictly larger at fixed `Re`.
///
/// **Results (measured 2026-08-05, release).** Printed by the test.
#[test]
fn gunn_collapses_to_the_stagnant_limit_in_the_dilute_limit() {
    let gunn = InterfacialHeatTransfer::Gunn;
    let rm = InterfacialHeatTransfer::RanzMarshall;
    let (pr, res) = (2.0, 1.0e-6);

    let nu0 = gunn.nusselt(1.0, 0.0, pr, res).expect("valid inputs");
    println!("Gunn Nu(alpha_c=1, Re=0) = {nu0}");
    assert!(
        (nu0 - 2.0).abs() < 1e-12,
        "dilute stagnant limit must be 2, got {nu0}"
    );

    println!(
        "{:>8} {:>12} {:>14} {:>10}",
        "Re", "Gunn", "RanzMarshall", "ratio"
    );
    for re in [0.0, 1.0, 100.0, 10_000.0] {
        let g = gunn.nusselt(1.0, re, pr, res).expect("valid inputs");
        let r = rm.nusselt(1.0, re, pr, res).expect("valid inputs");
        println!("{re:>8} {g:>12.5} {r:>14.5} {:>10.5}", g / r);
    }

    let dilute = gunn.nusselt(1.0, 100.0, pr, res).expect("valid inputs");
    let dense = gunn.nusselt(0.4, 100.0, pr, res).expect("valid inputs");
    println!("Nu at Re=100: alpha_c=1.0 -> {dilute:.5}, alpha_c=0.4 -> {dense:.5}");
    assert!(
        dense > dilute,
        "packing must raise Nu: {dense} not > {dilute}"
    );
}

/// **Methodology.** Unphysical inputs must be refused, not turned into an
/// infinity or a NaN that surfaces several timesteps later far from its cause.
///
/// **Results (measured 2026-08-05).** All four refused with `InvalidInput`;
/// each message names the offending quantity and its value.
#[test]
fn unphysical_inputs_are_refused() {
    let m = InterfacialHeatTransfer::RanzMarshall;
    let cases = [
        (
            "zero diameter",
            m.volumetric_coefficient_si(0.1, 0.9, 0.6, 0.02, 0.0, 1.0, 1.0, 1e-6),
        ),
        (
            "negative kappa_c",
            m.volumetric_coefficient_si(0.1, 0.9, -0.6, 0.02, 1e-3, 1.0, 1.0, 1e-6),
        ),
        (
            "negative Re",
            m.volumetric_coefficient_si(0.1, 0.9, 0.6, 0.02, 1e-3, -1.0, 1.0, 1e-6),
        ),
        (
            "negative Pr",
            m.volumetric_coefficient_si(0.1, 0.9, 0.6, 0.02, 1e-3, 1.0, -1.0, 1e-6),
        ),
    ];
    for (name, outcome) in cases {
        println!("{name} -> {outcome:?}");
        assert!(
            matches!(outcome, Err(MultiphaseError::InvalidInput(_))),
            "{name} must be refused"
        );
    }
}
