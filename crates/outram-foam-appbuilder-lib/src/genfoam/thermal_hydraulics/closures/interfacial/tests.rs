// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam), commit 652b3da, EPFL,
// GPL-3.0. See each sibling module's header for provenance.
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0.

//! # Verification & validation — two-phase geometry & regime closures
//!
//! **Scope.** These tests verify each `closures::interfacial` sub-module
//! against (a) closed-form analytical/textbook limits, (b) exact algebraic
//! reproduction of each upstream `value()` branch (including geometry-derived
//! constructors), and (c) the documented boundary/continuity/sharp-edge
//! behaviour of the piecewise and gap-interpolated models.
//!
//! ## Methodology
//!
//! 1. **Interfacial area — Zuber dispersed-flow limit.** For a dispersed phase
//!    with no third phase present (`alpha_dispersed + alpha_continuous == 1`),
//!    [`InterfacialArea::Spherical`] below its cutoff reduces to the textbook
//!    spherical-bubble relation `a_i = 6*alpha/D` (Zuber 1964); the annular
//!    variant reduces to `a_i = 4*alpha/D`. Pass criterion: exact match
//!    (`< 1e-10` relative) for input magnitudes chosen so the reference
//!    fraction is a clean decimal (no rounding noise from the reference
//!    computation itself).
//! 2. **Interfacial area — cutoff-boundary continuity.** Both `Spherical` and
//!    `Annular` switch formula branch at `alpha = cutoff_alpha`. `Spherical`'s
//!    branches were constructed (upstream) to meet exactly there, so a hair
//!    below vs. exactly at the cutoff must agree (pass criterion: relative
//!    difference `< 1e-4`). `Annular`'s branches do **not** meet there — this
//!    was discovered while writing this V&V, not assumed going in — so its
//!    test instead pins down the exact jump ratio `1/(1+sqrt(cutoff_alpha))`
//!    upstream's own algebra produces (pass criterion: reproduces that ratio
//!    to `< 1e-6` relative). See `area.rs`'s `Annular` variant doc comment.
//! 3. **Rod-bundle geometry constructors — port correctness.** `NoKazimi`'s and
//!    `Schor`'s `*_from_geometry` constructors are checked by recomputing their
//!    coefficients independently in the test from the documented formula and
//!    comparing to the constructed variant's fields (no independent published
//!    reference data available for these two correlations; this is the same
//!    "port correctness" methodology `fs_drag`'s `tests.rs` uses for its
//!    wire-wrap-bundle constructors). Pass criterion: exact match
//!    (`< 1e-12` relative).
//! 4. **Fluid diameter — ideal-gas analytical scaling.** `IsomolarBubble`
//!    (`pV/T = const`) and `IsothermalBubble` (`pV = const`) are checked
//!    against hand-derivable ideal-gas cube-root scalings at input ratios
//!    chosen to give exact results (`p/p0 = 8 -> cbrt(1/8) = 0.5`,
//!    `T/T0 = 2 -> cbrt(2)`). Pass criterion: `< 1e-10` relative (isomolar's
//!    `cbrt(2)` case) or exact (isothermal's clean `0.5` case).
//! 5. **Film diameter — TRACE / geometric endpoints.** `WallisFilm` at full
//!    void fraction (`alphaN = 1`) reproduces the TRACE theory manual's
//!    `d = 0.25 * Dh` (NRC ML120060218.pdf, eqn. 4-38) exactly; `PipeFilm` at
//!    `alphaN = 1` reproduces the geometric limit `d = Dh` (film occupies the
//!    whole pipe cross-section) exactly.
//! 6. **Virtual mass — Lamb's inviscid-sphere reference.** `Cvm = 0.5` (an
//!    isolated sphere's added-mass coefficient in inviscid potential flow;
//!    Lamb, *Hydrodynamics*, 6th ed., 1932, section 92) is reproduced
//!    unchanged by [`VirtualMassCoefficient::Constant`]'s pass-through, and the
//!    `0.1` value from GeN-Foam's own shipped tutorial dictionary
//!    (`Tutorials/featureCases/1D_boiling/constant/fluidRegion/phaseProperties`)
//!    is reproduced exactly.
//! 7. **Dispersion / contact-partition — fraction-composition invariants.**
//!    `TurbulentDispersion`'s `fluid1`/`fluid2` values sum to `1` by
//!    construction; `ContactPartition::Linear` and `::Complementary` evaluated
//!    on the same input sum to `1` (the two phases partition the wall exactly).
//!    Endpoints `0` and `1` are checked exactly, per the workspace guardrail
//!    requiring interfacial closures to check partition endpoints.
//! 8. **Regime map — boundary continuity and documented sharp edges.** A
//!    single-regime map claims every parameter value; two touching regimes
//!    hand off cleanly at their shared threshold; a genuine gap blends
//!    linearly/quadratically with weights summing to `1` in the gap's
//!    interior. The two known sharp edges documented in `regime_map.rs`
//!    (weight-`0` at a gap's start, weight-`2` for the entering regime at a
//!    gap's end) are asserted as regression tests, not silently accepted. The
//!    `+-1e69` sentinel is checked not to produce `NaN` (a literal
//!    `f64::INFINITY` substitution was tried during development and produces
//!    `inf/inf = NaN` in the outermost window's ratio — see `regime_map.rs`'s
//!    module docs).
//!
//! ## Results (measured 2026-07-15, `cargo test --release`, data: source formulae)
//!
//! - **Spherical/annular Zuber limit:** `a_i(alpha=0.05, D=0.01 m) = 30.0` /
//!   `20.0` (1/m) respectively, both exact to machine precision — PASS.
//! - **Cutoff continuity:** spherical's two branches agree to `< 1e-4`
//!   relative across the `cutoff_alpha = 0.9` boundary — PASS. Annular's two
//!   branches do **not** agree there — measured jump ratio
//!   `1/(1+sqrt(0.9)) = 0.51317`, i.e. an abrupt ~49% drop — reproducing
//!   upstream's own (undocumented, and plausibly unintended) discontinuity
//!   exactly; flagged as `annular_cutoff_branch_has_a_documented_jump_discontinuity`,
//!   a regression test, not a defect in this port — PASS (matches upstream).
//! - **NoKazimi/Schor geometry constructors:** all derived coefficients
//!   (`a_const`, `ia1`, `ia2`) reproduce the documented formula to machine
//!   precision for a `D = 8 mm`, `P = 10 mm` bundle — PASS.
//! - **Isomolar/isothermal bubble scaling:** `cbrt(2) = 1.259921...` and
//!   `cbrt(1/8) = 0.5` both reproduced to `< 1e-10` / exactly — PASS.
//! - **Film diameter endpoints:** `WallisFilm(alphaN=1) = 0.25*Dh`,
//!   `PipeFilm(alphaN=1) = Dh`, both exact — PASS.
//! - **Virtual mass:** `Cvm = 0.5` and `Cvm = 0.1` (tutorial value) both
//!   reproduced exactly (pass-through) — PASS.
//! - **Dispersion/contact-partition composition:** all sum-to-`1` invariants
//!   and endpoint checks hold to machine precision — PASS.
//! - **Regime map:** single-regime and touching-regime handoff exact; gap
//!   linear midpoint gives `0.5/0.5` exact; gap quadratic near-edge weight
//!   `(0.98, 0.02)` at 10% into the gap reproduces `1 - 2*0.1^2` exactly; the
//!   two documented sharp edges (weight-`0` gap start, weight-`2` gap end)
//!   both reproduce as predicted; no `NaN` observed with the `1e69` sentinel
//!   at parameter magnitudes up to `1e10` — PASS.
//!
//! Interpretation: every closed-form branch of this closure family reproduces
//! its documented upstream formula exactly (verification), the interfacial-area
//! and film-diameter closures reproduce independent textbook/technical-manual
//! reference relations at their applicable limits (validation against
//! published theory), and the regime-map's non-obvious boundary behaviour is
//! pinned down by regression test rather than left as an undocumented risk.

use super::area::InterfacialArea;
use super::contact_partition::ContactPartition;
use super::diameter::{BubbleDiameter, FilmDiameter};
use super::dispersion::TurbulentDispersion;
use super::regime_map::{RegimeBound, RegimeInterpolationMode, RegimeMap1D};
use super::virtual_mass::VirtualMassCoefficient;
use approx::assert_relative_eq;
use uom::si::f64::{Length, Pressure, Ratio, ThermodynamicTemperature};
use uom::si::length::meter;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::reciprocal_length::reciprocal_meter;
use uom::si::thermodynamic_temperature::kelvin;

fn r(x: f64) -> Ratio {
    Ratio::new::<ratio>(x)
}
fn len(x: f64) -> Length {
    Length::new::<meter>(x)
}
fn pa(x: f64) -> Pressure {
    Pressure::new::<pascal>(x)
}
fn kelvin_of(x: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(x)
}

// ---------------------------------------------------------------------------
// area.rs
// ---------------------------------------------------------------------------

/// V&V 1 — Spherical/annular reproduce the Zuber dispersed-flow limits
/// `a_i = 6*alpha/D` and `a_i = 4*alpha/D` exactly (no third phase, so
/// `alpha_dispersed + alpha_continuous == 1` and the pair-normalized fraction
/// equals `alpha_dispersed`).
#[test]
fn spherical_and_annular_reproduce_zuber_limit() {
    let d = 0.01; // 1 cm bubble diameter
    let alpha_d = r(0.05);
    let alpha_c = r(0.95);
    let dispersed_diameter = len(d);

    let spherical = InterfacialArea::Spherical { cutoff_alpha: 0.9 };
    let a_i = spherical
        .area_concentration(alpha_d, alpha_c, dispersed_diameter)
        .get::<reciprocal_meter>();
    assert_relative_eq!(a_i, 6.0 * 0.05 / d, max_relative = 1e-10);

    let annular = InterfacialArea::Annular { cutoff_alpha: 0.9 };
    let a_i = annular
        .area_concentration(alpha_d, alpha_c, dispersed_diameter)
        .get::<reciprocal_meter>();
    assert_relative_eq!(a_i, 4.0 * 0.05 / d, max_relative = 1e-10);
}

/// V&V 2a — the spherical cutoff branch switch is continuous (the
/// `(1-a)/(1-cutoff)` taper is exactly `1` at `a = cutoff_alpha`).
#[test]
fn spherical_cutoff_branch_is_continuous() {
    let d = 0.01;
    let cutoff = 0.9;
    let dispersed_diameter = len(d);
    let model = InterfacialArea::Spherical {
        cutoff_alpha: cutoff,
    };

    let below = model
        .area_concentration(
            r(cutoff - 1e-6),
            r(1.0 - (cutoff - 1e-6)),
            dispersed_diameter,
        )
        .get::<reciprocal_meter>();
    let at = model
        .area_concentration(r(cutoff), r(1.0 - cutoff), dispersed_diameter)
        .get::<reciprocal_meter>();
    assert_relative_eq!(below, at, max_relative = 1e-4);
}

/// V&V 2b — **discovered during this port's V&V, not assumed**: unlike
/// `Spherical`, the `Annular` model's `(1-sqrt(a))/(1-cutoff)` taper does
/// *not* reduce to `1` at `a = cutoff_alpha`, so its two branches meet with a
/// genuine jump discontinuity of exactly `1/(1+sqrt(cutoff_alpha))`. Verified
/// against the upstream `.C` source directly (not a porting slip — see
/// `area.rs`'s `Annular` variant doc comment for the derivation and the human
/// follow-up flag).
#[test]
fn annular_cutoff_branch_has_a_documented_jump_discontinuity() {
    let d = 0.01;
    let cutoff = 0.9;
    let dispersed_diameter = len(d);
    let model = InterfacialArea::Annular {
        cutoff_alpha: cutoff,
    };

    let below = model
        .area_concentration(
            r(cutoff - 1e-9),
            r(1.0 - (cutoff - 1e-9)),
            dispersed_diameter,
        )
        .get::<reciprocal_meter>();
    let at = model
        .area_concentration(r(cutoff), r(1.0 - cutoff), dispersed_diameter)
        .get::<reciprocal_meter>();

    let expected_ratio = 1.0 / (1.0 + cutoff.sqrt());
    assert_relative_eq!(at / below, expected_ratio, max_relative = 1e-6);
    // Sanity: this really is a large (~49%), not a rounding-noise-sized, jump.
    assert!((1.0 - at / below).abs() > 0.4);
}

/// V&V 3 — the `NoKazimi`/`Schor` rod-bundle geometry constructors reproduce
/// their documented coefficient formulae exactly for a concrete bundle
/// geometry (`D = 8 mm`, `P = 10 mm`).
#[test]
fn rod_bundle_geometry_constructors_reproduce_formulae() {
    let d = 8.0e-3;
    let p = 10.0e-3;
    let pd = p / d;

    let no_kazimi = InterfacialArea::no_kazimi_from_geometry(d, p);
    let expected_a = 4.0 * core::f64::consts::PI
        / (d * (2.0 * 3.0_f64.sqrt() * pd * pd - core::f64::consts::PI));
    match no_kazimi {
        InterfacialArea::NoKazimi {
            a_const,
            pin_diameter_m,
        } => {
            assert_relative_eq!(a_const, expected_a, max_relative = 1e-12);
            assert_relative_eq!(pin_diameter_m, d, max_relative = 1e-12);
        }
        _ => panic!("wrong variant"),
    }

    // Deliberately upstream's literal truncation, not a typo of PI — see
    // `area.rs`'s `SCHOR_PI` doc comment.
    #[allow(clippy::approx_constant)]
    const SCHOR_PI: f64 = 3.141_592_7;
    let schor = InterfacialArea::schor_from_geometry(d, p, 0.957, 0.0);
    let a_expected = 2.0 * 3.0_f64.sqrt() * pd * pd;
    let ia1_expected = (4.0 / d) * (SCHOR_PI * 0.55 / (3.0 * (a_expected - SCHOR_PI))).sqrt();
    let ia2_expected = (4.0 / d) * SCHOR_PI.sqrt() / (3.0 * (a_expected - SCHOR_PI))
        * ((1.0 - 0.65) * a_expected + SCHOR_PI * 0.65).sqrt();
    match schor {
        InterfacialArea::Schor {
            a_const, ia1, ia2, ..
        } => {
            assert_relative_eq!(a_const, a_expected, max_relative = 1e-12);
            assert_relative_eq!(ia1, ia1_expected, max_relative = 1e-12);
            assert_relative_eq!(ia2, ia2_expected, max_relative = 1e-12);
        }
        _ => panic!("wrong variant"),
    }
}

// ---------------------------------------------------------------------------
// diameter.rs
// ---------------------------------------------------------------------------

/// V&V 4 — isomolar/isothermal bubble diameter reproduce ideal-gas cube-root
/// scaling exactly at input ratios chosen to give clean reference values.
#[test]
fn bubble_diameter_reproduces_ideal_gas_scaling() {
    let d0 = 1.0e-3; // 1 mm reference bubble
    let p0 = 1.0e5; // 1 bar reference pressure
    let t0 = 300.0; // K reference temperature

    // Isomolar: halving T at fixed p halves p0*T/(p*T0), so d = d0*cbrt(1/2)... use
    // instead T = 2*T0 at fixed p = p0, giving d = d0*cbrt(2) (a clean cube root).
    let isomolar = BubbleDiameter::IsomolarBubble {
        d0_m: d0,
        p0_pa: p0,
        t0_k: t0,
    };
    let d = isomolar
        .diameter(pa(p0), kelvin_of(2.0 * t0))
        .get::<meter>();
    assert_relative_eq!(d, d0 * 2.0_f64.cbrt(), max_relative = 1e-10);

    // At the reference state exactly, d == d0.
    let d_ref = isomolar.diameter(pa(p0), kelvin_of(t0)).get::<meter>();
    assert_relative_eq!(d_ref, d0, max_relative = 1e-12);

    // Isothermal: p = 8*p0 gives d = d0*cbrt(1/8) = d0*0.5 exactly.
    let isothermal = BubbleDiameter::IsothermalBubble {
        d0_m: d0,
        p0_pa: p0,
    };
    let d = isothermal
        .diameter(pa(8.0 * p0), kelvin_of(t0))
        .get::<meter>();
    assert_relative_eq!(d, 0.5 * d0, max_relative = 1e-12);

    // Isothermal is temperature-independent: a different T at the same p gives
    // the same diameter (the `temperature` argument is accepted but unused).
    let d_hot = isothermal
        .diameter(pa(8.0 * p0), kelvin_of(900.0))
        .get::<meter>();
    assert_relative_eq!(d_hot, d, max_relative = 1e-12);
}

/// V&V 5 — `WallisFilm`/`PipeFilm` reproduce their geometric/technical-manual
/// endpoint values exactly at full void fraction (`alphaN = 1`).
#[test]
fn film_diameter_reproduces_endpoint_values() {
    let dhs = len(0.02); // 2 cm structure hydraulic diameter

    // WallisFilm: TRACE theory manual eqn. 4-38, d = 0.25*alphaN*Dh.
    let wallis = FilmDiameter::WallisFilm {
        residual_alpha: 1e-2,
    };
    let d = wallis.diameter(r(1.0), dhs).get::<meter>();
    assert_relative_eq!(d, 0.25 * dhs.get::<meter>(), max_relative = 1e-12);

    // PipeFilm at alphaN = 1: film occupies the whole pipe cross-section, d = Dh.
    let pipe = FilmDiameter::PipeFilm {
        residual_alpha: 1e-2,
    };
    let d = pipe.diameter(r(1.0), dhs).get::<meter>();
    assert_relative_eq!(d, dhs.get::<meter>(), max_relative = 1e-12);

    // Both clamp to residual_alpha below it (no null film thickness at alphaN=0).
    let d_wallis_floor = wallis.diameter(r(0.0), dhs).get::<meter>();
    assert_relative_eq!(
        d_wallis_floor,
        0.25 * 1e-2 * dhs.get::<meter>(),
        max_relative = 1e-12
    );
}

// ---------------------------------------------------------------------------
// virtual_mass.rs
// ---------------------------------------------------------------------------

/// V&V 6 — `VirtualMassCoefficient::Constant` reproduces Lamb's inviscid-sphere
/// reference `Cvm = 0.5` and GeN-Foam's own shipped tutorial value `0.1`
/// exactly (pure pass-through, see module docs for why).
#[test]
fn virtual_mass_coefficient_pass_through() {
    assert_relative_eq!(
        VirtualMassCoefficient::constant(r(0.5))
            .coefficient()
            .get::<ratio>(),
        0.5,
        max_relative = 1e-12
    );
    assert_relative_eq!(
        VirtualMassCoefficient::constant(r(0.1))
            .coefficient()
            .get::<ratio>(),
        0.1,
        max_relative = 1e-12
    );
}

// ---------------------------------------------------------------------------
// dispersion.rs
// ---------------------------------------------------------------------------

/// V&V 7a — the `fluid1`/`fluid2` resolution matches GeN-Foam's constructor
/// branch exactly, and the two values always sum to `1`.
#[test]
fn dispersion_fluid1_fluid2_resolution_and_composition() {
    let a = TurbulentDispersion::constant_for_fluid1(r(0.3), true);
    assert_relative_eq!(
        a.value_for_fluid1().get::<ratio>(),
        0.3,
        max_relative = 1e-12
    );
    assert_relative_eq!(
        a.value_for_fluid2().get::<ratio>(),
        0.7,
        max_relative = 1e-12
    );

    let b = TurbulentDispersion::constant_for_fluid1(r(0.3), false);
    assert_relative_eq!(
        b.value_for_fluid1().get::<ratio>(),
        0.7,
        max_relative = 1e-12
    );
    assert_relative_eq!(
        b.value_for_fluid2().get::<ratio>(),
        0.3,
        max_relative = 1e-12
    );

    for model in [a, b] {
        let sum = model.value_for_fluid1().get::<ratio>() + model.value_for_fluid2().get::<ratio>();
        assert_relative_eq!(sum, 1.0, max_relative = 1e-12);
    }
}

// ---------------------------------------------------------------------------
// contact_partition.rs
// ---------------------------------------------------------------------------

/// V&V 7b — `Linear`/`Complementary` endpoints (`0`, `1`) and their
/// sum-to-`1` composition invariant.
#[test]
fn contact_partition_endpoints_and_composition() {
    let linear = ContactPartition::Linear;
    let complementary = ContactPartition::Complementary;

    assert_relative_eq!(
        linear.value(r(0.0)).get::<ratio>(),
        0.0,
        max_relative = 1e-12
    );
    assert_relative_eq!(
        linear.value(r(1.0)).get::<ratio>(),
        1.0,
        max_relative = 1e-12
    );
    assert_relative_eq!(
        complementary.value(r(0.0)).get::<ratio>(),
        1.0,
        max_relative = 1e-12
    );
    assert_relative_eq!(
        complementary.value(r(1.0)).get::<ratio>(),
        0.0,
        max_relative = 1e-12
    );

    for x in [0.0, 0.4, 1.0] {
        let sum = linear.value(r(x)).get::<ratio>() + complementary.value(r(x)).get::<ratio>();
        assert_relative_eq!(sum, 1.0, max_relative = 1e-12);
    }
}

// ---------------------------------------------------------------------------
// regime_map.rs
// ---------------------------------------------------------------------------

/// V&V 8a — a single-regime map claims every parameter value, including far
/// outside its declared window (the map's edges always extend to the
/// sentinel — see `regime_map.rs`'s module docs).
#[test]
fn single_regime_map_claims_every_parameter() {
    let map = RegimeMap1D::new(
        vec![RegimeBound {
            name: "single".to_string(),
            lower: 0.0,
            upper: 1.0,
        }],
        RegimeInterpolationMode::Linear,
    );
    for p in [-500.0, 0.5, 2.0, 1e10] {
        let weights = map.regime_weights(p);
        assert_eq!(weights, vec![("single".to_string(), 1.0)], "p = {p}");
    }
}

/// V&V 8b — two regimes with touching bounds (no gap) hand off cleanly at the
/// shared threshold, and each extends to the sentinel beyond its own declared
/// edge (documented, not a bug — see `regime_map.rs`'s module docs).
#[test]
fn outermost_regimes_extend_to_the_sentinel_not_their_declared_edge() {
    let map = RegimeMap1D::new(
        vec![
            RegimeBound {
                name: "bubbly".to_string(),
                lower: 0.0,
                upper: 0.3,
            },
            RegimeBound {
                name: "slug".to_string(),
                lower: 0.3,
                upper: 0.6,
            },
        ],
        RegimeInterpolationMode::Linear,
    );
    assert_eq!(map.regime_weights(0.1), vec![("bubbly".to_string(), 1.0)]);
    assert_eq!(map.regime_weights(0.3), vec![("slug".to_string(), 1.0)]);
    assert_eq!(map.regime_weights(0.5), vec![("slug".to_string(), 1.0)]);
    // Far outside slug's declared upper edge (0.6) — still claimed, because the
    // highest regime's outer edge is the sentinel, not its declared bound.
    assert_eq!(map.regime_weights(1000.0), vec![("slug".to_string(), 1.0)]);
    // Symmetric on the low side.
    assert_eq!(
        map.regime_weights(-1000.0),
        vec![("bubbly".to_string(), 1.0)]
    );
}

/// V&V 8c — a genuine gap between two regimes blends linearly, with weights
/// summing to `1` in its interior.
#[test]
fn gap_interior_blends_linearly_summing_to_one() {
    let map = RegimeMap1D::new(
        vec![
            RegimeBound {
                name: "bubbly".to_string(),
                lower: 0.0,
                upper: 0.25,
            },
            RegimeBound {
                name: "slug".to_string(),
                lower: 0.35,
                upper: 0.6,
            },
        ],
        RegimeInterpolationMode::Linear,
    );
    let weights = map.regime_weights(0.30); // gap midpoint
    assert_eq!(weights.len(), 2);
    let sum: f64 = weights.iter().map(|(_, w)| w).sum();
    assert_relative_eq!(sum, 1.0, max_relative = 1e-12);
    for (_name, w) in &weights {
        assert_relative_eq!(*w, 0.5, max_relative = 1e-12);
    }
}

/// V&V 8d — the quadratic blend reproduces its documented piecewise formula
/// exactly (not a plain linear ease) at a point 10% into the gap.
#[test]
fn gap_quadratic_blend_matches_documented_formula() {
    let map = RegimeMap1D::new(
        vec![
            RegimeBound {
                name: "bubbly".to_string(),
                lower: 0.0,
                upper: 0.25,
            },
            RegimeBound {
                name: "slug".to_string(),
                lower: 0.35,
                upper: 0.6,
            },
        ],
        RegimeInterpolationMode::Quadratic,
    );
    // p = 0.26 is 10% of the way across the [0.25, 0.35] gap.
    let weights = map.regime_weights(0.26);
    let bubbly_weight = weights
        .iter()
        .find(|(n, _)| n == "bubbly")
        .map(|(_, w)| *w)
        .unwrap();
    let slug_weight = weights
        .iter()
        .find(|(n, _)| n == "slug")
        .map(|(_, w)| *w)
        .unwrap();
    // p <= mid (0.30), so c0 = 1 - 2*(0.1)^2 = 0.98 for the exiting regime.
    assert_relative_eq!(bubbly_weight, 1.0 - 2.0 * 0.1 * 0.1, max_relative = 1e-10);
    assert_relative_eq!(slug_weight, 1.0 - bubbly_weight, max_relative = 1e-10);

    // Exact gap midpoint reduces to the same 0.5/0.5 split as linear mode.
    let mid_weights = map.regime_weights(0.30);
    for (_, w) in &mid_weights {
        assert_relative_eq!(*w, 0.5, max_relative = 1e-10);
    }
}

/// V&V 8e — the two documented sharp edges at a gap's boundaries: weight `0`
/// (dropped) at the gap's start, and the entering regime double-counted
/// (total weight `2`) at the gap's end. Both are genuine upstream properties
/// of the half-open interval scheme, reproduced faithfully, not introduced by
/// this port — see `regime_map.rs`'s module docs for the derivation.
#[test]
fn gap_junction_sharp_edges_are_reproduced_faithfully() {
    let map = RegimeMap1D::new(
        vec![
            RegimeBound {
                name: "bubbly".to_string(),
                lower: 0.0,
                upper: 0.25,
            },
            RegimeBound {
                name: "slug".to_string(),
                lower: 0.35,
                upper: 0.6,
            },
        ],
        RegimeInterpolationMode::Linear,
    );
    // Gap start (== bubbly's declared upper edge): nothing claims it.
    assert!(map.regime_weights(0.25).is_empty());

    // Gap end (== slug's declared lower edge): both the gap and the slug
    // window claim it, so slug's total weight is 2, not 1.
    let weights = map.regime_weights(0.35);
    assert_eq!(weights.len(), 3);
    let slug_total: f64 = weights
        .iter()
        .filter(|(n, _)| n == "slug")
        .map(|(_, w)| w)
        .sum();
    assert_relative_eq!(slug_total, 2.0, max_relative = 1e-12);
    let bubbly_total: f64 = weights
        .iter()
        .filter(|(n, _)| n == "bubbly")
        .map(|(_, w)| w)
        .sum();
    assert_eq!(bubbly_total, 0.0);
}

/// V&V 8f — the `1e69` sentinel (not a literal `f64::INFINITY`) keeps the
/// outermost window's ratio finite; no `NaN` for any parameter magnitude a
/// physical simulation would plausibly reach.
#[test]
fn outermost_window_sentinel_does_not_yield_nan() {
    let map = RegimeMap1D::new(
        vec![RegimeBound {
            name: "only".to_string(),
            lower: 0.0,
            upper: 1.0,
        }],
        RegimeInterpolationMode::Linear,
    );
    for p in [0.0, 1.0, 1e6, 1e10, -1e10] {
        let weights = map.regime_weights(p);
        assert!(
            !weights.iter().any(|(_, w)| w.is_nan()),
            "p = {p} produced NaN"
        );
    }
}
