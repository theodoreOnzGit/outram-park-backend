// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! # V&V — the #70 crude workflow runs from the prelude alone
//!
//! ## Methodology
//!
//! This is an *integration* test, so it sees the crate exactly as an external
//! user does. It imports **only** `outram_park_fork_dwsim_libs::prelude::*`
//! and drives the GitHub #70 acceptance workflow —
//!
//! `BlackOilCrude::heavy()` → 12 pseudo-components → `CrudeColumnConfig::atmospheric_default()`
//! (Peng-Robinson 1978) → rigorous MESH column → cut slate
//!
//! — then checks: the prelude really was the only import (the test reads its
//! own source and counts `use outram_park_fork_dwsim_libs` lines), the
//! characterisation is the one the black-oil correlations imply, the column
//! converges, the profile is monotone, the material balance closes on the
//! whole crude, and the cut slate reproduces the figures recorded in #70 from
//! the Python bindings.
//!
//! **Reference figures (issue #70, via the Python bindings, before this test
//! existed):** 22.0 °API → SG 0.9218; 38 iterations to a final error of about
//! `8.5e-7`; naphtha 0.15005, kerosene 0.04001, diesel 0.03335, heavier diesel
//! draw 0.02668, atmospheric residue 0.74991, total 1.00000 mol/s. These are a
//! target to *reproduce*, not to tune to; flows are compared to `1e-4` mol/s
//! (the reference is quoted to five decimals) and the iteration count and
//! error are recorded rather than asserted exactly.
//!
//! ## Results
//!
//! Recorded in the doc comment of [`crude_workflow_needs_only_the_prelude`].
//!
//! ## Honest scope
//!
//! Verification of API reachability and internal consistency, plus agreement
//! with the crate's own earlier output through a different binding. Not
//! validation against a real crude assay or a refinery yield.

use outram_park_fork_dwsim_libs::prelude::*;

/// **Methodology.** As the module header. Additionally exercises, through the
/// prelude only, the thermo and column entry points a user would reach next:
/// a Peng-Robinson 1978 PT flash of the light end of the slate, and a
/// `RigorousColumn::distillation` built by hand from the prelude's `Stage`,
/// `ColumnSpec` and `units` markers — so the prelude is proven sufficient for
/// the *manual* path as well as the packaged one.
///
/// **Results (2026-09-10, `cargo test --release --test prelude_workflow`):**
/// `SG = 0.921824` (141.5/153.5); black-oil mean molar mass 499.1 g/mol, mean
/// NBP 767.6 K; 12 pseudo-components with `Tb` from 414.4 K to 1503.7 K, of
/// which the five below the 650 K cut point enter the column. The column
/// **converged in 38 iterations to a final error of 8.5103e-7**. Cuts
/// (mol/s): naphtha 0.15005 (stage 0, 442.42 K), kerosene 0.04001 (stage 4,
/// 524.19 K), diesel 0.03335 (stage 6, 537.85 K), diesel 0.02668 (stage 8,
/// 550.73 K), residue 0.74991 (stage 11, 601.51 K); total 1.000000 mol/s,
/// residual 0. Stage profile monotone, 442.42 K → 601.51 K. Every flow
/// matches the #70 reference to the five decimals quoted there, the iteration
/// count matches exactly (38), and the final error agrees with the quoted
/// "~8.5e-7". The PR78 flash of the four lightest pseudo-components at their
/// mid boiling point and 1.2 bar is two-phase as expected, and the hand-built
/// benzene/toluene `RigorousColumn::distillation` solves to `D = 0.5 mol/s`.
#[test]
fn crude_workflow_needs_only_the_prelude() {
    // ── The import check: this file must use the prelude and nothing else. ──
    let source = include_str!("prelude_workflow.rs");
    let crate_uses: Vec<&str> = source
        .lines()
        .filter(|l| {
            l.trim_start()
                .starts_with("use outram_park_fork_dwsim_libs")
        })
        .collect();
    assert_eq!(
        crate_uses,
        vec!["use outram_park_fork_dwsim_libs::prelude::*;"],
        "this test must import the crate through the prelude only"
    );
    // The needle is assembled at run time so this check does not match itself.
    let needle = format!("{}::", "outram_park_fork_dwsim_libs");
    let inline_paths = source
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .filter(|l| l.contains(&needle) && !l.contains("prelude::*"))
        .count();
    assert_eq!(
        inline_paths, 0,
        "no inline crate paths allowed outside the prelude import"
    );

    // ── 1. Black-oil characterisation. ───────────────────────────────────────
    let crude = BlackOilCrude::heavy();
    assert_eq!(crude.api_gravity, 22.0);
    let sg = crude.oil_specific_gravity();
    let sg_expected = 141.5 / (22.0 + 131.5);
    assert!(
        (sg - sg_expected).abs() < 1e-12,
        "SG = {sg}, expected {sg_expected}"
    );
    assert!(
        (sg - 0.9218).abs() < 5e-5,
        "SG = {sg} vs the #70 figure 0.9218"
    );

    let cut_count = 12;
    let slate: Vec<PseudoComponent> = crude
        .pseudo_components(cut_count)
        .expect("22 API crude characterises");
    assert_eq!(slate.len(), cut_count);
    let z_sum: f64 = slate
        .iter()
        .map(|pc| pc.mole_fraction.get::<units::ratio>())
        .sum();
    assert!(
        (z_sum - 1.0).abs() < 1e-9,
        "slate mole fractions sum to {z_sum}"
    );
    for w in slate.windows(2) {
        assert!(
            w[1].component.normal_boiling_point > w[0].component.normal_boiling_point,
            "slate must be in ascending Tb"
        );
    }

    // ── 2. A PT flash on the light end, Peng-Robinson 1978, via the prelude. ─
    let light: Vec<Component> = slate[..4].iter().map(|pc| pc.component.clone()).collect();
    let z = vec![0.25_f64; 4];
    let t_mid = 0.5 * (light[0].normal_boiling_point + light[3].normal_boiling_point);
    let flash: FlashResult = PropertyPackageModel::PengRobinson1978
        .flash_pt(&light, &z, t_mid, 120_000.0)
        .expect("PR78 flash of the light end at its mid boiling point");
    assert!(
        flash.beta > 0.0 && flash.beta < 1.0,
        "two-phase expected, beta = {}",
        flash.beta
    );
    let sum_x: f64 = flash.x.iter().sum();
    assert!((sum_x - 1.0).abs() < 1e-9);

    // ── 3-4. The packaged column. ────────────────────────────────────────────
    let config = CrudeColumnConfig::atmospheric_default();
    assert_eq!(config.package, PropertyPackageModel::PengRobinson1978);
    let result: CrudeColumnResult =
        solve_crude_column(&crude, &config, cut_count).expect("the 22 API crude column converges");

    println!(
        "[prelude] converged in {} iterations, final error {:.4e}",
        result.iterations, result.final_error
    );
    for c in &result.cuts {
        println!(
            "[prelude] stage {:>2}  {:>8.5} mol/s  {:>7.2} K  {}",
            c.stage,
            c.flow_mol_s,
            c.temperature_k,
            c.cut.label()
        );
    }
    assert!(result.iterations < 100);
    assert!(
        result.final_error < 1e-5,
        "final error {}",
        result.final_error
    );
    for w in result.stage_temperatures_k.windows(2) {
        assert!(
            w[1] >= w[0] - 1e-6,
            "profile not monotone: {:?}",
            result.stage_temperatures_k
        );
    }

    // ── 5. Material balance and the #70 reference slate. ─────────────────────
    let total = result.total_product_mol_s();
    assert!(
        (total - config.feed_flow_mol_s).abs() < 1e-9,
        "products total {total} mol/s"
    );

    let reference = [
        (0_usize, 0.15005_f64, CrudeCut::Naphtha),
        (4, 0.04001, CrudeCut::Kerosene),
        (6, 0.03335, CrudeCut::Diesel),
        (8, 0.02668, CrudeCut::Diesel),
        (11, 0.74991, CrudeCut::Residue),
    ];
    assert_eq!(result.cuts.len(), reference.len());
    for (cut, (stage, flow, label)) in result.cuts.iter().zip(reference) {
        assert_eq!(cut.stage, stage);
        assert!(
            (cut.flow_mol_s - flow).abs() < 1e-4,
            "stage {stage}: {} mol/s vs #70 reference {flow}",
            cut.flow_mol_s
        );
        assert_eq!(
            cut.cut, label,
            "stage {stage} labelled {:?}, reference {label:?}",
            cut.cut
        );
    }

    // ── 6. The manual column path, also prelude-only. ────────────────────────
    let comps = vec![reference::benzene(), reference::toluene()];
    let thermo = ColumnThermo::new(comps.clone(), PropertyPackageModel::Ideal);
    let feed_z = [0.5, 0.5];
    let t_feed = thermo
        .bubble_temperature(&feed_z, 101_325.0, 365.0, 4)
        .map(|(t, _)| t)
        .expect("bubble point");
    let h_feed = thermo.feed_molar_enthalpy(&feed_z, t_feed, 101_325.0, 0.0);
    let p = StagePressure::new::<units::pascal>(101_325.0);
    let mut stages: Vec<Stage> = (0..8)
        .map(|i| {
            let t = StageTemperature::new::<units::kelvin>(355.0 + 4.0 * i as f64);
            Stage::new(format!("stage {i}"), p, t, 2)
        })
        .collect();
    stages[4] = stages[4].clone().with_feed(
        MolarFlowRate::new::<units::katal>(1.0),
        feed_z.to_vec(),
        MolarEnthalpy::new::<units::joule_per_mole>(h_feed),
    );
    let input: ColumnSolverInput = RigorousColumn::distillation(
        comps,
        PropertyPackageModel::Ideal,
        stages,
        ColumnSpec::reflux_ratio(2.0),
        ColumnSpec::product_molar_flow(MolarFlowRate::new::<units::katal>(0.5)),
    )
    .with_distillate_estimate(MolarFlowRate::new::<units::katal>(0.5))
    .with_reflux_ratio_estimate(2.0)
    .solver_input()
    .expect("estimates");
    let out: ColumnSolverOutput = ColumnSolverMethod::default().solve(&input).expect("solves");
    let d = out
        .distillate_molar_flow(input.condenser_type)
        .get::<units::katal>();
    assert!((d - 0.5).abs() < 1e-6, "D = {d}");
    assert!(out.liquid_compositions[0][0] > 0.8, "benzene overhead");
    assert_eq!(input.column_type, ColumnType::DistillationColumn);
    assert_eq!(input.condenser_type, CondenserType::TotalCondenser);
}
