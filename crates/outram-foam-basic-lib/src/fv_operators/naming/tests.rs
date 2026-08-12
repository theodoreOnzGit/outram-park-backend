// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Verification that operator-derived field names cannot grow without bound.

use super::*;

/// A first application keeps full diagnostic value.
///
/// **Methodology.** Apply [`derived_name`] and [`derived_name2`] to plain field
/// names and compare against the OpenFOAM-style spelling this crate used
/// before the guard existed. Pass criterion: exact string match, so the guard
/// costs nothing in the common case.
///
/// **Results (2026-08-12).** `grad(p)`, `interpolate(T)`, `div(phi,rho)` — all
/// unchanged from the pre-guard names. This matters: the guard would not be
/// worth having if it degraded the names a reader actually sees in a solver's
/// field list.
#[test]
fn first_application_is_unchanged_from_the_openfoam_spelling() {
    assert_eq!(derived_name("grad", "p"), "grad(p)");
    assert_eq!(derived_name("interpolate", "T"), "interpolate(T)");
    assert_eq!(derived_name("snGrad", "alpha.water"), "snGrad(alpha.water)");
    assert_eq!(derived_name2("div", "phi", "rho"), "div(phi,rho)");
    assert_eq!(derived_name2("div", "phi", "U"), "div(phi,U)");
}

/// Repeated application reaches a fixed point — the actual guard.
///
/// **Methodology.** Simulate the self-feedback pattern that `fvc::div` permits
/// because it returns the same type it consumes (`psi = fvc::div(&phi, &psi)`),
/// iterating 1000 times and recording the name length at each step. Reference:
/// the requirement that a solver's field name be bounded independently of step
/// count. Pass criterion: the name is identical from step 2 onward and its
/// length never exceeds a small constant.
///
/// **Results (2026-08-12).** The sequence is
/// `rho` (3 chars) -> `div(phi,rho)` (12) -> `div(phi,..)` (11) -> `div(phi,..)`
/// (11) -> ... The length is **11 characters at step 1000**, identical to
/// step 2.
///
/// Without the guard the same 1000 steps would produce a name of roughly
/// 9000 characters and growing linearly forever — the same class of defect as
/// the exponential one that cost this project a 24 GB SIGTERM, differing only
/// in how long it takes to become fatal.
#[test]
fn repeated_self_feedback_reaches_a_fixed_point() {
    let mut name = String::from("rho");
    let step1 = derived_name2("div", "phi", &name);
    assert_eq!(step1, "div(phi,rho)");

    name = step1;
    let step2 = derived_name2("div", "phi", &name);
    assert_eq!(step2, "div(phi,..)");

    name = step2.clone();
    for step in 3..=1000 {
        name = derived_name2("div", "phi", &name);
        assert_eq!(
            name, step2,
            "step {step} diverged from the fixed point: {name}"
        );
    }
    assert_eq!(name.len(), 11, "fixed-point name length changed");
}

/// The single-operand helper reaches a fixed point too.
///
/// **Methodology.** Iterate [`derived_name`] 1000 times, as a solver would if
/// it reassigned a field from a same-type unary operator. Pass criterion:
/// identical from step 2 onward.
///
/// **Results (2026-08-12).** `p` -> `grad(p)` -> `grad(..)` -> `grad(..)`,
/// stable at **8 characters** through step 1000.
#[test]
fn repeated_unary_application_reaches_a_fixed_point() {
    let mut name = derived_name("grad", "p");
    assert_eq!(name, "grad(p)");
    name = derived_name("grad", &name);
    let fixed = name.clone();
    assert_eq!(fixed, "grad(..)");

    for step in 3..=1000 {
        name = derived_name("grad", &name);
        assert_eq!(name, fixed, "step {step} diverged: {name}");
    }
    assert_eq!(name.len(), 8);
}

/// Only the derived operand is elided; a plain one survives.
///
/// **Methodology.** Feed a derived name as the FIRST operand and a plain name
/// as the second, and vice versa. Pass criterion: each operand is judged
/// independently, so a plain name is never lost just because its partner was
/// derived.
///
/// **Results (2026-08-12).** `div(..,rho)` and `div(phi,..)` — each side
/// elided independently, as intended. Collapsing both would have thrown away
/// diagnostic information the guard does not need to take.
#[test]
fn operands_are_elided_independently() {
    assert_eq!(derived_name2("div", "grad(p)", "rho"), "div(..,rho)");
    assert_eq!(derived_name2("div", "phi", "grad(p)"), "div(phi,..)");
    assert_eq!(derived_name2("div", "grad(p)", "grad(T)"), "div(..,..)");
}

/// Plain field names are never mistaken for derived ones.
///
/// **Methodology.** Run [`is_derived`] over names taken from real OpenFOAM
/// cases, including the dotted multiphase convention. Pass criterion: only
/// names containing a parenthesis are treated as derived.
///
/// **Results (2026-08-12).** `rho`, `U`, `p`, `T`, `alpha.water`, `p_rgh`,
/// `nut` all report plain; `div(phi,rho)`, `grad(p)`, `interpolate(T)` report
/// derived. A false positive here would silently degrade a legitimate field
/// name, which is why the plain cases are asserted explicitly rather than
/// assumed.
#[test]
fn plain_field_names_are_not_treated_as_derived() {
    for plain in ["rho", "U", "p", "T", "alpha.water", "p_rgh", "nut", "k"] {
        assert!(!is_derived(plain), "{plain} misjudged as derived");
    }
    for derived in ["div(phi,rho)", "grad(p)", "interpolate(T)", "div(phi,..)"] {
        assert!(is_derived(derived), "{derived} misjudged as plain");
    }
}
