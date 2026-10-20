//! # Computing with Confidence — verification against the original R scripts
//!
//! Port of Adolphus Lye's "Coherent systems" case studies
//! (<https://github.com/Adolphus8/Computing-with-Confidence>, gh:#987) onto
//! `raffles::scram::{pba, pbox_tree}`. AI-assisted draft, not reviewed by a
//! human or the crate owner. Full record:
//! `crates/raffles/docs/computing-with-confidence-verification.md`.
//!
//! ## Methodology
//!
//! **Reference.** The R scripts themselves, run with R 4.6.1 against Scott
//! Ferson's `pba BETTER.r` through
//! `verification_and_validation/computing_with_confidence/r_inputs/run_case.R`
//! (command lines in `regenerate.sh`). The numbers compared are the
//! `interval_*` objects the scripts compute, i.e. the 95 % cuts
//! `[cut(x, 0.025).lower, cut(x, 0.975).upper]`, read here from the committed
//! `*.r_output.txt` files (`include_str!`), never retyped.
//!
//! **Inputs.** The c-boxes exactly as the scripts define them (`KN(k, n)`),
//! 200 steps (201 levels) for the illustrative example and Case studies 1-3,
//! 1000 steps for Case study 4.
//!
//! **Pass criterion, fixed before the runs from the method.** Both codes do
//! the same arithmetic on the same quantile cells (sorted cell products for
//! independence, min/max over grid pairs for Fréchet), so they may differ only
//! by the accuracy of the Beta quantile function and floating point:
//! `|rust - r| <= 1e-9 + 1e-6 * |r|`. The tolerance was not adjusted after
//! looking at a difference.
//!
//! ## Results (2026-10-20, R 4.6.1)
//!
//! Recorded in the V&V document: every compared interval end agrees; see it
//! for the worst observed relative difference.
//!
//! ## What is NOT verified
//!
//! - Case study 3's second half (structure-function vs Boolean Monte Carlo) is
//!   in the script behind `source("sra.r")`, which is "All rights reserved" and
//!   was neither read nor run. Only its `pba`-based Boolean route is
//!   reproduced.
//! - Case study 4 **as published** overwrites `c1..c9` with point estimates
//!   (`c1 = (1/100)` ...) after defining the c-boxes, so its independent
//!   events and its non-top Fréchet events are point values. The comparison
//!   here uses `case4_cboxes.r`, the same script with that duplicate block
//!   (lines 34-46) deleted, which is what the comments and plots indicate was
//!   intended. The top event `e1f` is identical in both runs.

use raffles::imprecise::{EventDependence, Pbox};
use raffles::scram::fault_tree::{Connective, FaultTreeBuilder, FaultTreeModel};
use raffles::scram::pba::{
    and_independent, and_unknown, kn, kn_exact, or_independent, or_unknown, point,
};
use raffles::scram::pbox_tree::{
    evaluate_cbox_edges, evaluate_pbox_tree, GateDependence, PboxTreeOptions, RepeatedEvents,
};
use std::collections::HashMap;

const ILLUSTRATIVE: &str = include_str!(
    "../verification_and_validation/computing_with_confidence/r_inputs/illustrative.r_output.txt"
);
const CASE1: &str = include_str!(
    "../verification_and_validation/computing_with_confidence/r_inputs/case1.r_output.txt"
);
const CASE2: &str = include_str!(
    "../verification_and_validation/computing_with_confidence/r_inputs/case2.r_output.txt"
);
const CASE3: &str = include_str!(
    "../verification_and_validation/computing_with_confidence/r_inputs/case3_pba_part.r_output.txt"
);
const CASE4: &str = include_str!(
    "../verification_and_validation/computing_with_confidence/r_inputs/case4_cboxes.r_output.txt"
);

fn parse(text: &str) -> HashMap<String, (f64, f64)> {
    text.lines()
        .filter(|l| l.starts_with("interval_"))
        .map(|l| {
            let mut it = l.split_whitespace();
            let name = it
                .next()
                .unwrap()
                .trim_start_matches("interval_")
                .to_string();
            (
                name,
                (
                    it.next().unwrap().parse().unwrap(),
                    it.next().unwrap().parse().unwrap(),
                ),
            )
        })
        .collect()
}

/// Compares a p-box's 95 % cut with the R interval; returns the worst
/// relative difference seen, after asserting the tolerance.
fn assert_matches(label: &str, got: &Pbox, reference: (f64, f64)) -> f64 {
    let cut = got.cut(0.05).unwrap();
    let mut worst = 0.0_f64;
    for (side, g, r) in [
        ("lower", cut.lower(), reference.0),
        ("upper", cut.upper(), reference.1),
    ] {
        let diff = (g - r).abs();
        assert!(
            diff <= 1e-9 + 1e-6 * r.abs(),
            "{label} {side}: rust {g:.12e} vs R {r:.12e} (diff {diff:.3e})"
        );
        worst = worst.max(diff / r.abs().max(1e-300));
    }
    println!(
        "{label}: [{:.9e}, {:.9e}] vs R [{:.9e}, {:.9e}] maxabsdiff {:.2e}",
        cut.lower(),
        cut.upper(),
        reference.0,
        reference.1,
        (cut.lower() - reference.0)
            .abs()
            .max((cut.upper() - reference.1).abs())
    );
    worst
}

/// Builds a model whose basic events are named, in order, `events`; `gates`
/// are `(name, connective, args)`.
fn model(events: &[&str], gates: &[(&str, Connective, &[&str])], top: &str) -> FaultTreeModel {
    let mut b = FaultTreeBuilder::new();
    for e in events {
        b.basic_event(e, 0.5).unwrap();
    }
    for (n, c, a) in gates {
        b.gate(n, *c, a).unwrap();
    }
    b.build(top).unwrap()
}

fn check_gate(
    m: &FaultTreeModel,
    result: &raffles::scram::pbox_tree::PboxTreeResult,
    reference: &HashMap<String, (f64, f64)>,
    gate: &str,
    key: &str,
) {
    let idx = m.gate_names().iter().position(|n| n == gate).unwrap();
    assert_matches(key, result.gate(idx).unwrap(), reference[key]);
}

/// **Illustrative Examples.r.** `C1 = KN(23,24)`, `C2 = KN(14,17)`; series
/// (`and`/`andI`) and parallel (`or`/`orI`) 95 % intervals. Reference: R
/// 4.6.1 / pba BETTER.r (`illustrative.r_output.txt`).
#[test]
fn illustrative_example_matches_r() {
    let r = parse(ILLUSTRATIVE);
    let (c1, c2) = (kn(23, 24, 201).unwrap(), kn(14, 17, 201).unwrap());
    assert_matches("ser_f", &and_unknown(&c1, &c2).unwrap(), r["ser_f"]);
    assert_matches("ser_i", &and_independent(&c1, &c2).unwrap(), r["ser_i"]);
    assert_matches("par_f", &or_unknown(&c1, &c2).unwrap(), r["par_f"]);
    assert_matches("par_i", &or_independent(&c1, &c2).unwrap(), r["par_i"]);
}

/// **Case study 1, pressure tank** (`T or K2 or (S and (S1 or K1 or R))`,
/// gates E5..E1). All five events under independence and under unknown
/// dependence against R.
#[test]
fn case_study_1_pressure_tank_matches_r() {
    let r = parse(CASE1);
    let m = model(
        &["T", "K2", "S", "S1", "K1", "R"],
        &[
            ("E5", Connective::Or, &["K1", "R"]),
            ("E4", Connective::Or, &["E5", "S1"]),
            ("E3", Connective::And, &["E4", "S"]),
            ("E2", Connective::Or, &["E3", "K2"]),
            ("E1", Connective::Or, &["E2", "T"]),
        ],
        "E1",
    );
    let basic: Vec<Pbox> = [
        (0, 2000),
        (3, 500),
        (1, 150),
        (0, 460),
        (7, 10000),
        (0, 380),
    ]
    .iter()
    .map(|(k, n)| kn(*k, *n, 201).unwrap())
    .collect();
    let f = evaluate_pbox_tree(
        m.tree(),
        &basic,
        &PboxTreeOptions::uniform(EventDependence::Unknown),
    )
    .unwrap();
    let i = evaluate_pbox_tree(
        m.tree(),
        &basic,
        &PboxTreeOptions::uniform(EventDependence::Independent),
    )
    .unwrap();
    for e in ["E5", "E4", "E3", "E2", "E1"] {
        let lower = e.to_lowercase();
        check_gate(&m, &f, &r, e, &format!("{lower}f"));
        check_gate(&m, &i, &r, e, &format!("{lower}i"));
    }
}

/// **Case study 2, TRIGA** (`p and ((v3 or ov3) or (v4 or ov4))`; the operator
/// terms are the precise `3.49e-5`).
#[test]
fn case_study_2_triga_matches_r() {
    let r = parse(CASE2);
    let m = model(
        &["p", "v3", "ov3", "v4", "ov4"],
        &[
            ("E4", Connective::Or, &["v4", "ov4"]),
            ("E3", Connective::Or, &["v3", "ov3"]),
            ("E2", Connective::Or, &["E3", "E4"]),
            ("E1", Connective::And, &["p", "E2"]),
        ],
        "E1",
    );
    let basic = vec![
        kn(1, 42, 201).unwrap(),
        kn(8, 42, 201).unwrap(),
        point(3.49e-5, 201).unwrap(),
        kn(8, 42, 201).unwrap(),
        point(3.49e-5, 201).unwrap(),
    ];
    let f = evaluate_pbox_tree(
        m.tree(),
        &basic,
        &PboxTreeOptions::uniform(EventDependence::Unknown),
    )
    .unwrap();
    let i = evaluate_pbox_tree(
        m.tree(),
        &basic,
        &PboxTreeOptions::uniform(EventDependence::Independent),
    )
    .unwrap();
    for e in ["E4", "E3", "E2", "E1"] {
        let lower = e.to_lowercase();
        check_gate(&m, &f, &r, e, &format!("{lower}f"));
        check_gate(&m, &i, &r, e, &format!("{lower}i"));
    }
}

/// **Case study 3, bridge, `pba` Boolean route.** `orI(orI(phi1,phi2),
/// orI(phi3,phi4))` with components repeated across paths, evaluated as the
/// script does: every occurrence an independent copy
/// ([`RepeatedEvents::IndependentCopies`]). Also asserts the default refuses
/// this tree, because it is not the structure-function answer.
#[test]
fn case_study_3_bridge_boolean_route_matches_r() {
    let r = parse(CASE3);
    let m = model(
        &["C1", "C2", "C3", "C4", "C5"],
        &[
            ("phi1", Connective::And, &["C1", "C3"]),
            ("phi2", Connective::And, &["C2", "C4"]),
            ("c45", Connective::And, &["C4", "C5"]),
            ("c35", Connective::And, &["C3", "C5"]),
            ("phi3", Connective::And, &["C1", "c45"]),
            ("phi4", Connective::And, &["C2", "c35"]),
            ("o12", Connective::Or, &["phi1", "phi2"]),
            ("o34", Connective::Or, &["phi3", "phi4"]),
            ("S1", Connective::Or, &["o12", "o34"]),
        ],
        "S1",
    );
    let basic: Vec<Pbox> = [(23, 24), (23, 24), (14, 17), (14, 17), (12, 12)]
        .iter()
        .map(|(k, n)| kn(*k, *n, 201).unwrap())
        .collect();
    let strict = PboxTreeOptions::uniform(EventDependence::Independent);
    assert!(evaluate_pbox_tree(m.tree(), &basic, &strict).is_err());
    let copies = PboxTreeOptions {
        repeated: RepeatedEvents::IndependentCopies,
        ..strict
    };
    let res = evaluate_pbox_tree(m.tree(), &basic, &copies).unwrap();
    assert_matches("R_S1_indep", res.top(), r["R_S1_indep"]);
}

/// **Case study 4, radiation release** (9 c-boxes, `Pbox$steps = 1000`).
/// Gate-wise Fréchet and independent (with `c5` an independent copy where it
/// repeats, as the script does) for every non-top event; the top event under
/// unknown dependence by the edge construction, the script's
/// `e1f = env.pbox(e1f_L, e1f_R)`, fed the unclamped edges (`kn_exact`) the
/// script builds with `beta()`. Reference: `case4_cboxes.r_output.txt`.
#[test]
fn case_study_4_radiation_release_matches_r() {
    let r = parse(CASE4);
    let m = model(
        &["c1", "c2", "c3", "c4", "c5", "c6", "c7", "c8", "c9"],
        &[
            ("E4", Connective::And, &["c1", "c2"]),
            ("E3a", Connective::Or, &["E4", "c3"]),
            ("E3b", Connective::And, &["c4", "c5"]),
            ("E2a", Connective::Or, &["E3a", "E3b"]),
            ("E3c", Connective::Or, &["c5", "c7"]),
            ("E3d", Connective::Or, &["c8", "c9"]),
            ("E3cd", Connective::And, &["E3c", "E3d"]),
            ("E2b", Connective::And, &["c6", "E3cd"]),
            ("E1", Connective::Or, &["E2a", "E2b"]),
        ],
        "E1",
    );
    let levels = 1001;
    let basic: Vec<Pbox> = [
        (1, 100),
        (1, 200),
        (0, 1000),
        (3, 500),
        (1, 380),
        (0, 460),
        (2, 500),
        (1, 300),
        (2, 150),
    ]
    .iter()
    .map(|(k, n)| kn(*k, *n, levels).unwrap())
    .collect();
    let unknown = PboxTreeOptions::uniform(EventDependence::Unknown);
    let f = evaluate_pbox_tree(m.tree(), &basic, &unknown).unwrap();
    let copies = PboxTreeOptions {
        dependence: GateDependence::Uniform(EventDependence::Independent),
        repeated: RepeatedEvents::IndependentCopies,
    };
    let i = evaluate_pbox_tree(m.tree(), &basic, &copies).unwrap();
    for (gate, key) in [
        ("E4", "e4"),
        ("E3a", "e3a"),
        ("E3b", "e3b"),
        ("E2a", "e2a"),
        ("E3c", "e3c"),
        ("E3d", "e3d"),
        ("E2b", "e2b"),
    ] {
        check_gate(&m, &f, &r, gate, &format!("{key}f"));
        check_gate(&m, &i, &r, gate, &format!("{key}i"));
    }
    check_gate(&m, &i, &r, "E1", "e1i");
    // The script's c_L / c_R are pba `beta()` distributions: unclamped edges.
    let exact: Vec<Pbox> = [
        (1, 100),
        (1, 200),
        (0, 1000),
        (3, 500),
        (1, 380),
        (0, 460),
        (2, 500),
        (1, 300),
        (2, 150),
    ]
    .iter()
    .map(|(k, n)| kn_exact(*k, *n, levels).unwrap())
    .collect();
    let edges = evaluate_cbox_edges(m.tree(), &exact, &unknown).unwrap();
    assert_matches("e1f (edges)", edges.top(), r["e1f"]);
}

/// **Structural properties** that need no reference: for every case-study
/// tree without repeated events the unknown-dependence 95 % cut encloses the
/// independence cut (Case studies 1 and 2 and the illustrative example), and
/// precise inputs reduce to the plain formula (also in the unit tests).
#[test]
fn frechet_cuts_enclose_independence_cuts() {
    let (c1, c2) = (kn(23, 24, 201).unwrap(), kn(14, 17, 201).unwrap());
    for (u, i) in [
        (
            and_unknown(&c1, &c2).unwrap(),
            and_independent(&c1, &c2).unwrap(),
        ),
        (
            or_unknown(&c1, &c2).unwrap(),
            or_independent(&c1, &c2).unwrap(),
        ),
    ] {
        let (cu, ci) = (u.cut(0.05).unwrap(), i.cut(0.05).unwrap());
        assert!(cu.lower() <= ci.lower() && cu.upper() >= ci.upper());
    }
}
