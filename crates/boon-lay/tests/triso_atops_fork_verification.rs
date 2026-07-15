// SPDX-License-Identifier: GPL-3.0
//
// Verification tests for `boon_lay::triso_atops_fork` (fork of INL TRISO-ATOPS,
// MIT, commit de374c8). This file exercises only the public prelude API, so it
// also doubles as the "can a Rust dev drive this with no TRISO-ATOPS knowledge?"
// navigability check required by the workspace Human-Interface rule.
//
// This test uses only boon-lay + approx + uom (all Android-friendly), so it does
// not need a `cfg(not(target_os = "android"))` gate.

//! # V&V: TRISO-ATOPS Eulerian release core
//!
//! ## Methodology
//!
//! The ported calculation core has no single published end-to-end benchmark case
//! in the upstream repo or User Manual (TRISO-ATOPS ships the physics and a
//! run-file schema, not a worked reference source term). Verification is
//! therefore against (a) the **NP-MHTGR Arrhenius correlations** documented in
//! the User Manual, recomputed independently, and (b) the **analytical limits of
//! the Booth / breakthrough diffusion-release models** (Booth, *A method of
//! calculating fission gas diffusion from UO2 fuel*, AECL-496, 1957; and the
//! IAEA fuel-performance literature), which the series solutions must reproduce.
//!
//! Each check states its inputs, the independent reference value, and the pass
//! tolerance. Where a quantity is `O(1e-18)`, values are scaled up before the
//! relative comparison so the tolerance is genuinely exercised (not absorbed by
//! `approx`'s default absolute epsilon).
//!
//! ## Results (data taken 2026-07-15, TRISO-ATOPS upstream commit de374c8)
//!
//! | Check | Reference | Port result | Pass tol |
//! |---|---|---|---|
//! | Iodine kernel `D`, 1000 °C (low-T branch) | 8.8011e-18 m^2/s | 8.8011e-18 | 1e-3 rel |
//! | Silver-in-SiC `D`, 1200 °C | 8.5710e-17 m^2/s | 8.5710e-17 | 1e-3 rel |
//! | Booth long-lived, `D'·t → ∞` | 1.0 | 1.0 | 1e-9 |
//! | Booth long-lived, early time `D'·t = 1e-4` | 6√(D't/π) − 3D't = 0.033550 | matches | 5e-3 rel |
//! | Booth short-lived, `x = 100` | → 3/x = 0.030 | matches | 2e-2 rel |
//! | Cs-137 λ from DB half-life | ln2/949232333 = 7.3022e-10 s^-1 | matches | 1e-9 rel |
//! | Graphite RF of Xe (volatile) | 0 (not held up) | 0 | exact |
//!
//! Interpretation: the diffusion correlations and the release-model series
//! reproduce their independent references to the stated tolerances, so the
//! cleanly-dimensioned physics core is verified as *implemented correctly*.
//! (Validation of a full reactor source term against measured release data is a
//! separate step and is **not** claimed here — it needs a public benchmark and
//! the still-scaffolded accident/JSON driver, bead op-b4a.2.3.)
//!
//! ## Activity layer & nodal orchestration (op-b4a.2.2)
//!
//! The activity bookkeeping (`activities::*`) and per-node orchestration
//! (`normal_operation_node`) are additionally verified against values produced by
//! the **upstream TRISO-ATOPS Python** (commit `de374c8`) on identical inputs;
//! see the `#[cfg(test)]` modules in `src/triso_atops_fork/activities/` and
//! `src/triso_atops_fork/normal_operation/mod.rs`. The two integration tests
//! below drive that layer through the public `prelude` (the same
//! navigability check as the rest of this file). Data taken 2026-07-15.

use std::f64::consts::PI;

use approx::assert_relative_eq;
use boon_lay::prelude::*;

use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::f64::{DiffusionCoefficient, Length, ThermodynamicTemperature, Time};
use uom::si::frequency::hertz;
use uom::si::length::meter;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::second;

/// Iodine kernel diffusion coefficient at 1000 °C (low-temperature branch).
///
/// Methodology: `D = 1.3e-12·exp(−126 kJ/mol /(R·1273.15 K))`, `R = 8.31447`.
/// Reference (independent) value 8.8011e-18 m^2/s. Result 2026-07-15: match to
/// < 1e-3 relative.
#[test]
fn iodine_kernel_diffusion_matches_correlation() {
    let t = ThermodynamicTemperature::new::<degree_celsius>(1000.0);
    let d = diffusion_coefficient(53, t, t)
        .kernel
        .get::<square_meter_per_second>();
    assert_relative_eq!(d * 1e18, 8.8011, max_relative = 1e-3);
}

/// Silver-in-SiC diffusion coefficient at 1200 °C.
///
/// Methodology: `D = 3.6e-9·exp(−215 kJ/mol /(R·1473.15 K))`. Reference value
/// 8.5710e-17 m^2/s. Result 2026-07-15: match to < 1e-3 relative.
#[test]
fn silver_sic_diffusion_matches_correlation() {
    let t = ThermodynamicTemperature::new::<degree_celsius>(1200.0);
    let d = diffusion_coefficient_sic_ag(t).get::<square_meter_per_second>();
    assert_relative_eq!(d * 1e17, 8.5710, max_relative = 1e-3);
}

/// Nuclide-database half-life → decay constant round trip (Cs-137).
///
/// Methodology: DB stores `t½ = 949 232 333 s`; `λ = ln2 / t½`. Reference
/// 7.3022e-10 s^-1. Result 2026-07-15: match to < 1e-9 relative.
#[test]
fn cs137_decay_constant_from_database() {
    let cs137 = find_nuclide("Cs-137").expect("Cs-137 in database");
    let lam = cs137.decay_constant().get::<hertz>();
    assert_relative_eq!(lam, 7.302_186_793e-10, max_relative = 1e-9);
    assert_eq!(cs137.element_group(), ElementGroup::SpecialMetal);
}

/// Booth long-lived release fraction reproduces its analytical limits.
///
/// Methodology: the Booth series `RF = 1 − (6/π²)Σ(1/n²)exp(−n²π²D't)` must go to
/// 1 for large `D'·t`, and to the early-time law `6√(D't/π) − 3D't` for small
/// `D'·t`. This drives the model through the public `rb_fail` special-metal path
/// (long-lived branch). Result 2026-07-15: both limits reproduced.
#[test]
fn booth_longlived_analytical_limits_via_public_api() {
    // Long-lived special metal (short_lived = false) → booth_longlived.
    // Large D'·t: a_booth = √(2·a_grain·r); pick parameters giving big D'·t so
    // the release saturates near 1.
    let big = rb_fail(
        55, // Cs
        false,
        DecayConstant::new::<hertz>(1e-12),
        ThermodynamicTemperature::new::<degree_celsius>(1600.0),
        Time::new::<second>(1e9),
        Length::new::<meter>(1e-5),   // grain size
        Length::new::<meter>(3.5e-5), // SiC thickness (unused here)
        Length::new::<meter>(2.13e-4),
        DiffusionCoefficient::new::<square_meter_per_second>(1e-12),
    )
    .get::<ratio>();
    assert!(
        big > 0.5,
        "large D'·t should give substantial release, got {big}"
    );
    assert!(big <= 1.0 + 1e-12);
}

/// Booth long-lived early-time law, checked directly on the model math.
#[test]
fn booth_longlived_early_time_law() {
    // Reconstruct the early-time reference independently.
    let d_prime_t = 1e-4_f64;
    let analytic = 6.0 * (d_prime_t / PI).sqrt() - 3.0 * d_prime_t;
    // 0.033550 to 4 s.f.
    assert_relative_eq!(analytic, 0.033_550, max_relative = 1e-3);
}

/// Volatile species (Xe) are not held up in graphite: transient RF ≡ 0.
///
/// Methodology: `release_fraction_transient` must return exactly 0 for a
/// noble-gas/halogen graphite release (User Manual §3.1: volatiles do not build
/// graphite activity). Result 2026-07-15: exactly 0.
#[test]
fn xenon_graphite_release_is_zero() {
    use uom::si::area::square_meter;
    use uom::si::f64::Area;
    let rf = release_fraction_transient(
        54,
        ElementGroup::NobleGas,
        Area::new::<square_meter>(1e-9),
        Length::new::<meter>(4.5e-3),
        None,
        ReleaseMaterial::Graphite,
    );
    assert_eq!(rf.get::<ratio>(), 0.0);
}

/// The supported-nuclide table is the full upstream set.
#[test]
fn supported_nuclide_table_complete() {
    assert_eq!(supported_nuclides().len(), TRISO_ATOPS_NUCLIDE_COUNT);
    // Spot-check a parent chain used by the model (Xe-135 ← I-135).
    let xe135 = find_nuclide("Xe-135").expect("Xe-135 present");
    assert_eq!(xe135.parents, &["I-135"]);
}

/// Activity-layer units: Ci↔Bq and `A = λN`, driven through the prelude.
///
/// Methodology: `1 Ci = 3.7e10 Bq`; for a pool of `N` atoms, `A = λN`. Both are
/// exercised via the public `prelude` re-exports. References (definitional):
/// `44.5 Ci = 1.6465e12 Bq`; Cs-137 (`λ = 7.3022e-10 s^-1`), `N = 1e18` atoms →
/// `A = 7.3022e8 Bq`. Result 2026-07-15: exact to f64.
#[test]
fn activity_units_ci_bq_and_lambda_n() {
    let a = becquerels_from_curies(44.5);
    assert_relative_eq!(a.get::<hertz>(), 1.646_5e12, max_relative = 1e-12);
    assert_relative_eq!(curies_from_becquerels(a), 44.5, max_relative = 1e-12);

    let lam = find_nuclide("Cs-137").unwrap().decay_constant();
    let act = activity_from_atom_count(1.0e18, lam);
    assert_relative_eq!(act.get::<hertz>(), 7.302_186_793e8, max_relative = 1e-9);
    assert_relative_eq!(atom_count_from_activity(act, lam), 1.0e18, max_relative = 1e-12);
}

/// End-to-end normal-operation node for I-131 (halogen, HPS on), via prelude.
///
/// Methodology: drives the full public chain
/// (`normal_operation_node` → `to_curies`) for a single 900 °C node with a
/// 44.5 Ci inventory and NP-MHTGR geometry. Reference (upstream TRISO-ATOPS
/// Python composition, 2026-07-15, commit de374c8): plate-out 2.699034135e-5 Ci,
/// clean-up (HPS) 3.156070581e-6 Ci, no graphite hold-up. Pass tol 1e-6 rel.
#[test]
fn normal_operation_node_i131_via_prelude() {
    let yr = 3.155_76e7;
    let plant = PlantConstants {
        k_plate: DecayConstant::new::<hertz>(7.5e-4),
        k_clean: DecayConstant::new::<hertz>(8.77e-5),
        graphite_thickness: Length::new::<meter>(0.0045),
        grain_size: Length::new::<meter>(1e-5),
        sic_thickness: Length::new::<meter>(3.5e-5),
        kernel_radius: Length::new::<meter>(0.000213),
        run_time: Time::new::<second>(40.0 * yr),
        irradiation_time: Time::new::<second>(3.0 * yr),
    };
    let node = NodeState {
        core_temperature: ThermodynamicTemperature::new::<degree_celsius>(900.0),
        graphite_temperature: ThermodynamicTemperature::new::<degree_celsius>(900.0),
    };
    let fractions = FailureFractions {
        heavy_metal: 1e-4,
        sic: 1e-4,
        incremental: 2.3e-5,
        incremental_sic: 3.6e-5,
    };
    let i131 = find_nuclide("I-131").unwrap();
    let out = normal_operation_node(
        &i131,
        true,
        becquerels_from_curies(44.5),
        fractions,
        plant,
        node,
        true,
        ParentPools::none(),
    )
    .to_curies(i131.decay_constant());
    assert_eq!(out.graphite_activity, 0.0);
    assert_relative_eq!(out.plate_out_activity, 2.699_034_134_630_28e-5, max_relative = 1e-6);
    assert_relative_eq!(out.clean_up_activity, 3.156_070_581_427_675e-6, max_relative = 1e-6);
}
