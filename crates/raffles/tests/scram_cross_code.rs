//! # Code-to-code verification of the SCRAM port against compiled SCRAM
//!
//! Every reference number in this file was produced by **building upstream
//! SCRAM from source and running it**, not by reading its source and
//! reasoning about what it would do. Methodology, the build patch that was
//! needed, and the full oracle transcripts are in
//! `crates/raffles/docs/scram-port-verification.md`.
//!
//! | | |
//! |---|---|
//! | Upstream | <https://github.com/rakhimov/scram> |
//! | Commit | `b85b78940de38996eeffec54d946824bd4280a1c` (2019-07-03) |
//! | Version reported | SCRAM 0.16.2 |
//! | Built with | GCC, Boost 1.83.0, libxml2 2.9.14, `-DCMAKE_BUILD_TYPE=Release` |
//! | Oracle command | `scram --probability [--importance] [--rare-event\|--mcub] <input>` |
//!
//! The reference model is upstream's own `input/TwoTrain/two_train.xml`, which
//! ships with SCRAM — so the inputs are upstream's too, not ones this port
//! invented to suit itself.

use raffles::scram::probability::{cut_set_probability, top_event_probability, Approximation, CutSet};
use raffles::scram::importance::importance_factors;

/// Basic-event indices for the TwoTrains model, in the order SCRAM lists them.
const VALVE_ONE: usize = 0;
const VALVE_TWO: usize = 1;
const PUMP_ONE: usize = 2;
const PUMP_TWO: usize = 3;

/// `p[i]` for the four basic events, verbatim from `two_train.xml`.
fn two_train_probabilities() -> Vec<f64> {
    let mut p = vec![0.0; 4];
    p[VALVE_ONE] = 0.5;
    p[VALVE_TWO] = 0.5;
    p[PUMP_ONE] = 0.7;
    p[PUMP_TWO] = 0.7;
    p
}

/// The four minimal cut sets **as SCRAM reported them**, not as derived here.
///
/// Upstream's report lists, each of order 2:
/// `{ValveOne, ValveTwo}`, `{ValveOne, PumpTwo}`, `{PumpOne, ValveTwo}`,
/// `{PumpOne, PumpTwo}`.
fn two_train_cut_sets() -> Vec<CutSet> {
    vec![
        CutSet::new(&[VALVE_ONE, VALVE_TWO]).unwrap(),
        CutSet::new(&[VALVE_ONE, PUMP_TWO]).unwrap(),
        CutSet::new(&[PUMP_ONE, VALVE_TWO]).unwrap(),
        CutSet::new(&[PUMP_ONE, PUMP_TWO]).unwrap(),
    ]
}

/// **Methodology.** SCRAM's report gives a `probability` attribute on every
/// product (cut set). This checks the per-cut-set product — upstream's
/// `CutSetProbabilityCalculator::Calculate` — against those four values.
///
/// **Oracle** (`scram --probability input/TwoTrain/two_train.xml`):
///
/// | cut set | SCRAM `probability` |
/// |---|---|
/// | `{ValveOne, ValveTwo}` | 0.25 |
/// | `{ValveOne, PumpTwo}` | 0.35 |
/// | `{PumpOne, ValveTwo}` | 0.35 |
/// | `{PumpOne, PumpTwo}` | 0.49 |
///
/// **Result** (2026-09-21): all four reproduced exactly (0.0 absolute
/// difference); these are products of one- and two-digit decimals, so exact
/// agreement is the right expectation and any drift would be a real defect.
#[test]
fn cut_set_probabilities_match_scram() {
    let p = two_train_probabilities();
    let cut_sets = two_train_cut_sets();
    let oracle = [0.25, 0.35, 0.35, 0.49];

    for (cut_set, expected) in cut_sets.iter().zip(oracle) {
        let ours = cut_set_probability(cut_set, &p).unwrap();
        println!(
            "cut set {:?}: ours {ours} scram {expected}",
            cut_set.members()
        );
        assert!(
            (ours - expected).abs() < 1e-12,
            "cut set {:?}: ours {ours}, SCRAM {expected}",
            cut_set.members()
        );
    }
}

/// **Methodology.** The top-event probability under all three quantifications,
/// against SCRAM run three ways on the same model. SCRAM's exact path is a
/// BDD; this port's is inclusion-exclusion over the same cut sets. They are
/// different algorithms for the same quantity, so agreement here is a real
/// cross-check rather than a re-run of identical code.
///
/// **Oracle** (2026-09-21, SCRAM 0.16.2):
///
/// | mode | flag | SCRAM |
/// |---|---|---|
/// | exact | *(none — BDD)* | **0.7225** |
/// | rare-event | `--rare-event` | **1** |
/// | MCUB | `--mcub` | **0.838394** |
///
/// **Result**: exact `0.7225` reproduced to 1e-12; MCUB `0.83839375` against
/// upstream's 6-significant-figure `0.838394`; rare-event `1` exactly.
///
/// **The rare-event row is the interesting one.** The raw sum is
/// `0.25 + 0.35 + 0.35 + 0.49 = 1.44`, and SCRAM reports 1 — so this case
/// actually exercises the clamp `return sum > 1 ? 1 : sum;` rather than
/// leaving it as untested ported code. It also shows the approximation
/// failing loudly: these basic events are far too likely for "rare event" to
/// mean anything, and the honest reading of a reported 1 is "this
/// approximation does not apply here", not "the system always fails".
#[test]
fn top_event_probability_matches_scram_in_all_three_modes() {
    let p = two_train_probabilities();
    let cut_sets = two_train_cut_sets();

    let exact = top_event_probability(&cut_sets, &p, Approximation::Exact).unwrap();
    println!("exact:      ours {exact}  scram 0.7225");
    assert!(
        (exact - 0.7225).abs() < 1e-12,
        "exact: ours {exact}, SCRAM 0.7225"
    );

    let rare = top_event_probability(&cut_sets, &p, Approximation::RareEvent).unwrap();
    println!("rare-event: ours {rare}  scram 1  (unclamped sum is 1.44)");
    assert!(
        (rare - 1.0).abs() < 1e-12,
        "rare-event: ours {rare}, SCRAM 1"
    );

    let mcub = top_event_probability(&cut_sets, &p, Approximation::Mcub).unwrap();
    println!("mcub:       ours {mcub}  scram 0.838394");
    assert!(
        (mcub - 0.838394).abs() < 5e-7,
        "mcub: ours {mcub}, SCRAM 0.838394"
    );
}

/// **Methodology.** All five importance measures for both distinct basic-event
/// probabilities in the model, against `scram --probability --importance`.
///
/// Upstream computes the Birnbaum factor by differentiating its BDD; this port
/// computes it from its definition, `P(top | event) - P(top | not event)`, on
/// the cut sets. **Those are different algorithms**, so matching MIF is the
/// strongest single result in this file — and every other factor is derived
/// from MIF, so if MIF were wrong all five would move together.
///
/// **Oracle** (2026-09-21, SCRAM 0.16.2), `p_total = 0.7225`:
///
/// | event | p | occurrence | MIF | CIF | DIF | RAW | RRW |
/// |---|---|---|---|---|---|---|---|
/// | ValveOne | 0.5 | 2 | 0.255 | 0.176471 | 0.588235 | 1.17647 | 1.21429 |
/// | PumpOne | 0.7 | 2 | 0.425 | 0.411765 | 0.823529 | 1.17647 | 1.7 |
///
/// **Result**: every one of the ten numbers reproduced within 5e-6, the
/// resolution of upstream's 6-significant-figure report. The occurrence counts
/// (2 and 2) match exactly.
///
/// Note RAW is identical for both events at 1.17647 while RRW differs sharply
/// (1.21 against 1.70) — the two measures disagree about which component to
/// care about, which is exactly why this port keeps all five rather than
/// collapsing them.
#[test]
fn importance_factors_match_scram() {
    let p = two_train_probabilities();
    let cut_sets = two_train_cut_sets();

    // (event, p, occurrence, mif, cif, dif, raw, rrw) straight from the report.
    let oracle: [(usize, &str, usize, f64, f64, f64, f64, f64); 2] = [
        (
            VALVE_ONE, "ValveOne", 2, 0.255, 0.176471, 0.588235, 1.17647, 1.21429,
        ),
        (
            PUMP_ONE, "PumpOne", 2, 0.425, 0.411765, 0.823529, 1.17647, 1.7,
        ),
    ];

    for (event, name, occ, mif, cif, dif, raw, rrw) in oracle {
        let ours = importance_factors(event, &cut_sets, &p, Approximation::Exact).unwrap();
        println!(
            "{name}: occ {} (scram {occ}) mif {:.6} ({mif}) cif {:.6} ({cif}) \
             dif {:.6} ({dif}) raw {:.6} ({raw}) rrw {:.6} ({rrw})",
            ours.occurrence, ours.mif, ours.cif, ours.dif, ours.raw, ours.rrw
        );
        assert_eq!(ours.occurrence, occ, "{name}: occurrence");
        assert!(
            (ours.mif - mif).abs() < 5e-6,
            "{name}: MIF ours {} scram {mif}",
            ours.mif
        );
        assert!(
            (ours.cif - cif).abs() < 5e-6,
            "{name}: CIF ours {} scram {cif}",
            ours.cif
        );
        assert!(
            (ours.dif - dif).abs() < 5e-6,
            "{name}: DIF ours {} scram {dif}",
            ours.dif
        );
        assert!(
            (ours.raw - raw).abs() < 5e-6,
            "{name}: RAW ours {} scram {raw}",
            ours.raw
        );
        assert!(
            (ours.rrw - rrw).abs() < 5e-6,
            "{name}: RRW ours {} scram {rrw}",
            ours.rrw
        );
    }
}

/// **Methodology.** The two approximations must bound the exact answer in the
/// documented direction, and MCUB must be the tighter of the two. This is a
/// property check rather than an oracle comparison — it would catch a sign or
/// ordering error that happened to reproduce one model's numbers.
///
/// **Result** (2026-09-21): exact 0.7225 < MCUB 0.838394 < rare-event 1.
#[test]
fn the_approximations_bound_the_exact_answer_from_above() {
    let p = two_train_probabilities();
    let cut_sets = two_train_cut_sets();
    let exact = top_event_probability(&cut_sets, &p, Approximation::Exact).unwrap();
    let mcub = top_event_probability(&cut_sets, &p, Approximation::Mcub).unwrap();
    let rare = top_event_probability(&cut_sets, &p, Approximation::RareEvent).unwrap();
    println!("exact {exact} <= mcub {mcub} <= rare {rare}");
    assert!(
        exact <= mcub,
        "MCUB {mcub} is below the exact value {exact}"
    );
    assert!(mcub <= rare, "rare-event {rare} is below MCUB {mcub}");
}

/// **Methodology.** Malformed input must be refused rather than silently
/// producing a number: an empty cut set, no cut sets at all, an out-of-range
/// member index, a probability outside `[0, 1]`, and an exact request beyond
/// the inclusion-exclusion limit.
///
/// **Result** (2026-09-21): all five rejected.
#[test]
fn malformed_input_is_refused() {
    let p = two_train_probabilities();
    let cut_sets = two_train_cut_sets();

    assert!(CutSet::new(&[]).is_err(), "empty cut set");
    assert!(
        top_event_probability(&[], &p, Approximation::Exact).is_err(),
        "no cut sets"
    );
    let bad_index = vec![CutSet::new(&[99]).unwrap()];
    assert!(
        top_event_probability(&bad_index, &p, Approximation::Exact).is_err(),
        "out-of-range member"
    );
    let bad_p = vec![1.5, 0.5, 0.7, 0.7];
    assert!(
        top_event_probability(&cut_sets, &bad_p, Approximation::Exact).is_err(),
        "probability above 1"
    );
    let many: Vec<CutSet> = (0..25)
        .map(|_| CutSet::new(&[VALVE_ONE]).unwrap())
        .collect();
    assert!(
        top_event_probability(&many, &p, Approximation::Exact).is_err(),
        "exact beyond the cut-set limit"
    );
}
