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
//! `BlackOilCrude::light_sweet()` → 12 pseudo-components → `CrudeColumnConfig::atmospheric_default()`
//! (Peng-Robinson 1978) → rigorous MESH column → cut slate
//!
//! — then checks: the prelude really was the only import (the test reads its
//! own source and counts `use outram_park_fork_dwsim_libs` lines), the
//! characterisation is the one the black-oil correlations imply, the column
//! converges, the profile is monotone, the material balance closes on the
//! whole crude, and the cut slate reproduces the figures this crate produced
//! when the test was (re)baselined.
//!
//! **Why the crude changed (2026-09-10, GitHub #170).** #70 ran this workflow
//! on the 22 °API `BlackOilCrude::heavy()` and recorded SG 0.9218, 38
//! iterations to `~8.5e-7`, naphtha 0.15005, kerosene 0.04001, diesel 0.03335
//! and 0.02668, residue 0.74991 mol/s. That slate's twelfth cut carried a
//! **negative critical volume** (`Vc = −7.33e-2 m³/mol`, `ω = 25.3`) as an
//! ordinary finite `f64`; the column converged and the balance closed on it
//! anyway. The characterisation now refuses that cut, so those figures are
//! unreachable by design and this test asserts the refusal instead. The
//! column half of the workflow runs on the 38 °API crude, whose 12-cut slate
//! stays inside the correlations' range.
//!
//! **Pinned figures (this crate, 2026-09-10, release build):** 38.0 °API →
//! SG 0.834808; 34 iterations to a final error of `7.7244e-7`; naphtha
//! 0.33764, kerosene 0.09004, 0.07503, 0.06003, atmospheric residue 0.43726,
//! total 1.000000 mol/s. Flows are compared to `1e-4` mol/s; the iteration
//! count and error are recorded rather than asserted exactly. These are the
//! crate's own output, pinned for regression — not a reference from anywhere.
//!
//! ## Results
//!
//! Recorded in the doc comment of [`crude_workflow_needs_only_the_prelude`].
//!
//! ## Honest scope
//!
//! Verification of API reachability and internal consistency. Not validation
//! against a real crude assay or a refinery yield, and — since the #170
//! rebaseline — no longer a cross-check against an independent earlier run.

use outram_park_fork_dwsim_libs::prelude::*;

/// **Methodology.** As the module header. Additionally exercises, through the
/// prelude only, the thermo and column entry points a user would reach next:
/// a Peng-Robinson 1978 PT flash of the light end of the slate, and a
/// `RigorousColumn::distillation` built by hand from the prelude's `Stage`,
/// `ColumnSpec` and `units` markers — so the prelude is proven sufficient for
/// the *manual* path as well as the packaged one.
///
/// **Results (2026-09-10, `cargo test --release --test prelude_workflow`,
/// after the #170 rebaseline):** the 22 °API `heavy()` at 12 cuts is refused
/// with `CharacterizationError::PseudoComponent` whose message names
/// `Crude22API_NBP_1231`, cut 12 and `critical_volume` (value
/// `−7.329e-2 m³/mol`), and `solve_crude_column` on it fails with
/// `CrudeColumnError::Characterisation`. For the 38 °API `light_sweet()`:
/// `SG = 0.834808` (141.5/169.5); black-oil mean molar mass 206.5 g/mol, mean
/// NBP 538.8 K; 12 pseudo-components with `Tb` from 381.7 K to 866.3 K, of
/// which the nine below the 650 K cut point enter the column. The column
/// **converged in 34 iterations to a final error of 7.7244e-7**. Cuts
/// (mol/s): naphtha 0.33764 (stage 0, 423.69 K), kerosene 0.09004 (stage 4,
/// 494.32 K), kerosene 0.07503 (stage 6, 511.17 K), kerosene 0.06003 (stage
/// 8, 528.98 K), residue 0.43726 (stage 11, 587.32 K); total 1.000000 mol/s,
/// residual 0. Stage profile monotone, 423.69 K → 587.32 K. Labels are pinned
/// except at stage 8, which sits 1 K below the 530 K kerosene/diesel boundary
/// and is allowed either label; the bottoms is labelled `Residue` by
/// construction, not from its 587 K temperature. The PR78 flash
/// of the four lightest pseudo-components at their mid boiling point
/// (423.9 K) and 1.2 bar is two-phase (`β = 0.100`), and the hand-built
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

    // ── 0. The #170 guard, reached through the prelude. ─────────────────────
    // The 22 API crude's heaviest cut has a negative critical volume at 12
    // cuts; the characterisation must refuse it and say which cut and why,
    // and the packaged column must surface that refusal rather than solve.
    let cut_count = 12;
    let heavy = BlackOilCrude::heavy();
    assert_eq!(heavy.api_gravity, 22.0);
    match heavy.pseudo_components(cut_count) {
        Err(CharacterizationError::PseudoComponent(inner)) => {
            let msg = inner.to_string();
            assert!(
                msg.contains("critical_volume") && msg.contains("(cut 12)"),
                "the refusal must name the property and the cut: {msg}"
            );
            assert!(msg.contains("Crude22API_NBP_1231"), "{msg}");
        }
        other => panic!("22 API at 12 cuts must be refused with a NonPhysical Vc, got {other:?}"),
    }
    assert!(
        matches!(
            solve_crude_column(&heavy, &CrudeColumnConfig::atmospheric_default(), cut_count),
            Err(CrudeColumnError::Characterisation(_))
        ),
        "the column must not solve on a slate the characterisation refused"
    );

    // ── 1. Black-oil characterisation. ───────────────────────────────────────
    let crude = BlackOilCrude::light_sweet();
    assert_eq!(crude.api_gravity, 38.0);
    let sg = crude.oil_specific_gravity();
    let sg_expected = 141.5 / (38.0 + 131.5);
    assert!(
        (sg - sg_expected).abs() < 1e-12,
        "SG = {sg}, expected {sg_expected}"
    );
    assert!(
        (sg - 0.834808).abs() < 5e-7,
        "SG = {sg} vs the pinned figure 0.834808"
    );

    let slate: Vec<PseudoComponent> = crude
        .pseudo_components(cut_count)
        .expect("38 API crude characterises");
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
        solve_crude_column(&crude, &config, cut_count).expect("the 38 API crude column converges");

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

    // ── 5. Material balance and the pinned cut slate. ────────────────────────
    let total = result.total_product_mol_s();
    assert!(
        (total - config.feed_flow_mol_s).abs() < 1e-9,
        "products total {total} mol/s"
    );

    // Flows and labels pinned from this crate's own 2026-09-10 run (see the
    // module doc). The distillate and side draws are labelled from their draw
    // temperatures; the bottoms is always `Residue`. The stage-8 draw sits
    // within 1 K of the 530 K kerosene/diesel boundary, so either label is
    // accepted there rather than pinning a coin-flip.
    let reference = [
        (0_usize, 0.33764_f64, CrudeCut::Naphtha),
        (4, 0.09004, CrudeCut::Kerosene),
        (6, 0.07503, CrudeCut::Kerosene),
        (8, 0.06003, CrudeCut::Kerosene),
        (11, 0.43726, CrudeCut::Residue),
    ];
    assert_eq!(result.cuts.len(), reference.len());
    for (cut, (stage, flow, label)) in result.cuts.iter().zip(reference) {
        assert_eq!(cut.stage, stage);
        assert!(
            (cut.flow_mol_s - flow).abs() < 1e-4,
            "stage {stage}: {} mol/s vs the pinned {flow}",
            cut.flow_mol_s
        );
        if stage == 8 {
            assert!(
                matches!(cut.cut, CrudeCut::Kerosene | CrudeCut::Diesel),
                "stage 8 ({:.2} K) labelled {:?}, expected kerosene or diesel",
                cut.temperature_k,
                cut.cut
            );
        } else {
            assert_eq!(
                cut.cut, label,
                "stage {stage} labelled {:?}, pinned {label:?}",
                cut.cut
            );
        }
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
