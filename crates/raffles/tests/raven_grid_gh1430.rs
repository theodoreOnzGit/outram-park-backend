// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! **Differential test of raffles' `GridSampler` level construction against
//! RAVEN's `GridEntity`** (gh:#1430, case 2).
//!
//! Reference: RAVEN's real `GridEntity.initialize`, fed the `stepLength` that
//! `Grid` derives from the XML `<grid>` node (`GridEntities.py:307`, `:356`),
//! written by `verification_and_validation/raven_distributions_gh1430/
//! generate_grid_reference.py` into `references/grid.json`. Predictions
//! (`PREDICTIONS.md`, case 2) were committed before the run; measured
//! results and the finding are in `RESULTS.md` and `README.md`.
//!
//! RAVEN's Stratified (Latin hypercube) sampler is NOT compared: it draws
//! through numpy's MT19937 and raffles through `petir::rng::lcg`, and
//! `LatinHypercube::generate` takes a seed, not uniforms, so there is no
//! replay entry point (see the V&V README).

use std::path::PathBuf;

use raffles::samplers::GridSampler;
use serde_json::Value;

fn reference() -> Value {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/raven_distributions_gh1430/references/grid.json");
    serde_json::from_str(&std::fs::read_to_string(&p).expect("reference file")).expect("json")
}

fn levels(v: &Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect()
}

/// (count of levels compared, bit-identical, worst absolute difference)
fn compare(ours: &[f64], theirs: &[f64]) -> (usize, usize, f64) {
    assert_eq!(ours.len(), theirs.len(), "level count");
    let mut identical = 0;
    let mut worst = 0.0_f64;
    for (a, b) in ours.iter().zip(theirs) {
        if a.to_bits() == b.to_bits() {
            identical += 1;
        } else {
            worst = worst.max((a - b).abs());
        }
    }
    (ours.len(), identical, worst)
}

#[test]
fn grid_levels_against_raven_gridentity() {
    let r = reference();
    let (mut n, mut same, mut worst) = (0, 0, 0.0_f64);
    let mut skipped_at_one = 0;
    let (mut raven_same, mut raven_worst) = (0, 0.0_f64);
    for case in r["equal"].as_array().unwrap() {
        let lo = case["lower"].as_f64().unwrap();
        let hi = case["upper"].as_f64().unwrap();
        let steps = case["steps"].as_u64().unwrap() as usize;
        if hi >= 1.0 {
            // raffles refuses a level of exactly 1.0 (ppf(1) is +inf for an
            // unbounded variable); RAVEN's CDF grid accepts it.
            assert!(GridSampler::equally_spaced(1, steps, lo, hi).is_err());
            skipped_at_one += 1;
            continue;
        }
        let ours = GridSampler::equally_spaced(1, steps, lo, hi).unwrap();
        let theirs = levels(&case["levels"]);
        let (c, i, w) = compare(&ours.levels()[0], &theirs);
        let exact = GridSampler::equally_spaced_as_raven(1, steps, lo, hi).unwrap();
        let (_, ri, rw) = compare(&exact.levels()[0], &theirs);
        eprintln!(
            "equal [{lo}, {hi}] x {steps}: plain {i}/{c} (worst abs {w:.3e}); as_raven {ri}/{c} (worst abs {rw:.3e})"
        );
        n += c;
        same += i;
        worst = worst.max(w);
        raven_same += ri;
        raven_worst = raven_worst.max(rw);
    }
    eprintln!("AS_RAVEN TOTAL: {raven_same}/{n} bit-identical, worst abs {raven_worst:.3e}");
    // Pins, measured 2026-11-13 (RESULTS.md). Not to be relaxed.
    assert_eq!((raven_same, n), (1220, 1220), "equally_spaced_as_raven is RAVEN bit for bit");
    assert_eq!(raven_worst, 0.0);
    assert_eq!((same, n), (400, 1220), "plain equally_spaced bit-identical count");
    assert!(worst < 6e-14, "plain equally_spaced worst {worst}");
    assert_eq!(skipped_at_one, 5);
    eprintln!(
        "EQUAL TOTAL: {same}/{n} bit-identical, worst abs {worst:.3e}; {skipped_at_one} cases with upper = 1.0 refused by raffles"
    );

    let (mut cn, mut csame, mut cworst, mut refused) = (0, 0, 0.0_f64, 0);
    for case in r["custom"].as_array().unwrap() {
        let input = levels(&case["input"]);
        let ours = GridSampler::with_levels(vec![input.clone()]).unwrap();
        if case["levels"].is_null() {
            // RAVEN itself rejects this input (its own bounds check after the
            // 14-decimal running sum); raffles accepts it and keeps the
            // levels exactly.
            refused += 1;
            assert_eq!(ours.levels()[0], input);
            eprintln!(
                "custom {input:?}: RAVEN error: {}",
                case["raven_error"].as_str().unwrap()
            );
            continue;
        }
        let (c, i, w) = compare(&ours.levels()[0], &levels(&case["levels"]));
        eprintln!("custom {input:?}: {i}/{c} bit-identical, worst abs {w:.3e}");
        cn += c;
        csame += i;
        cworst = cworst.max(w);
    }
    eprintln!("CUSTOM TOTAL: {csame}/{cn} bit-identical, worst abs {cworst:.3e}, RAVEN refused {refused}");
    assert_eq!((csame, cn, refused), (9, 11, 2));
    assert!(cworst < 2e-16, "custom worst {cworst}");
}
