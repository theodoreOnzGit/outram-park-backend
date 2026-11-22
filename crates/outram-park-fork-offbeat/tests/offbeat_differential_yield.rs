// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
//! # FRAPTRAN yield stress vs OFFBEAT 80e84450 (gh:#1393, round 2)
//!
//! ## Methodology
//!
//! Reference: `reference_yield.txt` written by `dump_yield.cpp` (verbatim
//! `yieldStressFRAPTRAN::correctYieldStress`, cell part; a transcription, not a
//! run of the compiled library).  Predictions and derived gates:
//! `PREDICTIONS_round2.md` section D.  Three upstream variants per grid point:
//! `intended` (the `>=` chain the port implements), `lit1` (upstream as
//! written, first call) and `lit2` (second call: `phiValue_ *= 1e25` runs on
//! every call).  The port must equal `intended` (G1/G2); where `lit1` takes a
//! different fluence branch from `intended` the port must differ from `lit1`
//! (documented deviation, counted and pinned).  Not compared: `Hp_`.
//!
//! ## Results
//!
//! See `verification_and_validation/yield_vs_offbeat.md`.

mod offbeat_differential_support;

use offbeat_differential_support::{parse_rows, Gates, Row, Stats};
use outram_foam_basic_lib::primitives::SymmTensor;
use outram_park_fork_offbeat::materials::MaterialState;
use outram_park_fork_offbeat::mechanics::LinearElastic;
use outram_park_fork_offbeat::rheology::{RheologyInputs, YieldStressModel};
use std::collections::BTreeMap;

const FIXTURE: &str =
    include_str!("../verification_and_validation/offbeat_differential_gh1393/reference_yield.txt");

/// Columns: T, phi[1e25], CW, epsOld, rate, E, scale.
fn port_sigma_y(c: &[f64]) -> Option<f64> {
    let (t, phi_in, cw, eps_old, rate, e) = (c[0], c[1], c[2], c[3], c[4], c[5]);
    let mut material = MaterialState::fresh(t);
    material.fast_fluence = phi_in * 1e25;
    material.cold_work = cw;
    let elastic = LinearElastic::new(e, 0.3).ok()?;
    let mut inputs = RheologyInputs::quasi_static(elastic, SymmTensor::ZERO, material);
    inputs.equivalent_strain_rate = rate;
    YieldStressModel::Fraptran.yield_stress(eps_old, &inputs).ok()
}

/// Which `Kphi` branch a fluence takes: A (< 0.1e25), B (2.928e-26 phi), C (0.53236 + ...), None.
#[derive(PartialEq, Debug, Clone, Copy)]
enum Branch {
    A,
    B,
    C,
    Top,
    None,
}

fn intended_branch(phi: f64) -> Branch {
    if phi < 0.1e25 {
        Branch::A
    } else if phi < 2e25 {
        Branch::B
    } else if phi < 12e25 {
        Branch::C
    } else {
        Branch::Top
    }
}

fn literal_branch(phi: f64) -> Branch {
    if phi < 0.1e25 {
        Branch::A
    } else if phi <= 0.1e25 && phi < 2e25 {
        Branch::B
    } else if phi <= 2e25 && phi < 12e25 {
        Branch::C
    } else {
        Branch::None
    }
}

#[test]
fn port_equals_the_intended_chain_and_differs_from_upstream_as_written_exactly_where_the_branches_differ() {
    let rows = parse_rows(FIXTURE);
    let mut intended_stats = Stats::default();
    let mut intended_gates = Gates::default();
    let mut skipped = 0;
    let mut by_key: BTreeMap<String, (Option<&Row>, Option<&Row>, Option<&Row>)> = BTreeMap::new();
    for r in &rows {
        let key = format!("{:?}", &r.col[..6]);
        let slot = by_key.entry(key).or_insert((None, None, None));
        match r.name.as_str() {
            "Yield_intended" => slot.0 = Some(r),
            "Yield_lit1" => slot.1 = Some(r),
            "Yield_lit2" => slot.2 = Some(r),
            _ => {}
        }
    }
    let (mut agree_expected, mut agree_unexpected, mut differ_expected, mut differ_unexpected) = (0, 0, 0, 0);
    let mut min_rel_diff_lit1 = f64::INFINITY;
    let (mut lit2_rows, mut lit2_differ) = (0, 0);
    let mut worst_lit1_ratio = 0.0_f64;
    for (inten, lit1, lit2) in by_key.values() {
        let (inten, lit1, lit2) = (inten.unwrap(), lit1.unwrap(), lit2.unwrap());
        let c = &inten.col;
        let scale = c[6];
        let Some(port) = port_sigma_y(&c[..6]) else {
            skipped += 1;
            continue;
        };
        intended_stats.add(port, inten.reference, &format!("{:?}", &c[..6]));
        intended_gates.add(port, inten.reference, scale);

        let phi = c[1] * 1e25;
        let same_branch = intended_branch(phi) == literal_branch(phi);
        let d1 = (port - lit1.reference).abs();
        let agrees1 = d1 <= 8.0 * f64::EPSILON * lit1.col[6];
        match (same_branch, agrees1) {
            (true, true) => agree_expected += 1,
            (true, false) => agree_unexpected += 1,
            (false, false) => differ_expected += 1,
            (false, true) => differ_unexpected += 1,
        }
        if !same_branch {
            min_rel_diff_lit1 = min_rel_diff_lit1.min(d1 / lit1.reference);
            worst_lit1_ratio = worst_lit1_ratio.max(d1 / lit1.reference);
        }
        lit2_rows += 1;
        if (port - lit2.reference).abs() > 8.0 * f64::EPSILON * lit2.col[6] {
            lit2_differ += 1;
        }
    }
    println!(
        "intended: n {} skipped {} bit-id {} worst rel {:.2e}; G1 fail {}, G2 checked {} fail {}, worst near-cancellation {:.2e}, worst G1 ratio {:.3}",
        intended_stats.compared, skipped, intended_stats.bit_identical, intended_stats.worst_rel,
        intended_gates.g1_fail, intended_gates.g2_checked, intended_gates.g2_fail,
        intended_gates.worst_cond_rel, intended_gates.worst_g1_ratio
    );
    println!(
        "port vs upstream-as-written (first call): same branch & agree {agree_expected}, same branch & differ {agree_unexpected}, \
         different branch & differ {differ_expected}, different branch & agree {differ_unexpected}; \
         smallest relative difference where the branches differ {min_rel_diff_lit1:.3e}, largest {worst_lit1_ratio:.3e}"
    );
    println!("port vs second call: {lit2_differ} of {lit2_rows} rows differ beyond G1 (second call is independent of the fluence)");
    assert!(intended_stats.compared > 0);
    assert_eq!(intended_gates.g1_fail, 0);
    assert_eq!(intended_gates.g2_fail, 0);
    assert_eq!(agree_unexpected, 0, "same branch must agree");
    assert_eq!(differ_unexpected, 0, "different branches must differ");
    assert!(differ_expected > 0);
}
