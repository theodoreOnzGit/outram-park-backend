//! # End-to-end verification: cut sets generated here vs cut sets SCRAM found
//!
//! The other two SCRAM test files start from cut sets SCRAM already computed.
//! This one does not: it builds the fault tree from upstream's own input
//! model, generates the minimal cut sets with [`raffles::scram::mocus`], and
//! checks **the generated set against the set SCRAM reported** — then
//! quantifies it and checks the totals too. That closes the loop from input
//! model to top-event probability without SCRAM's answer entering anywhere
//! except as the thing being compared to.
//!
//! ## Two fixtures, and why they are separate
//!
//! | file | parsed from | what it is trusted for |
//! |---|---|---|
//! | `reference-data/scram/oracle.txt` | SCRAM's own **XML report** | the answers — cut sets, probabilities, importance |
//! | `reference-data/scram/models.txt` | SCRAM's own **input models** | the question — gates, connectives, arguments |
//!
//! The split is deliberate. `extract_oracle.sh` reads nothing from the inputs,
//! so no answer in it can have been copied from the question;
//! `extract_models.sh` emits no answer, so nothing in it can prejudge the
//! comparison. Basic-event probabilities come from the **oracle**, not the
//! models, because several models define them through expressions
//! (`periodic-test`, `GLM`) that SCRAM evaluates and this crate does not.
//!
//! ## The algorithms are unrelated, which is the point
//!
//! SCRAM generates cut sets with a ZBDD over a Boolean graph that a
//! 2,400-line preprocessor has rewritten. This crate runs the classical
//! top-down MOCUS expansion with absorption. Agreement between them is
//! evidence; agreement between a translation and its original would be much
//! weaker.
//!
//! | | |
//! |---|---|
//! | Upstream | <https://github.com/rakhimov/scram> @ `b85b7894` (2019-07-03) |
//! | Version | SCRAM 0.16.2 |
//! | Oracle command | `scram --probability [--importance] <input>` |
//!
//! Full record: `crates/raffles/docs/scram-port-verification.md`.

use raffles::scram::fault_tree::{Connective, FaultTreeBuilder, FaultTreeModel};
use raffles::scram::mocus::{minimal_cut_sets, DEFAULT_LIMIT_ORDER};
use raffles::scram::probability::{top_event_probability, Approximation, EXACT_CUT_SET_LIMIT};
use std::collections::{BTreeSet, HashMap};

// ---------------------------------------------------------------------------
// Fixture loading
// ---------------------------------------------------------------------------

/// One model's structure, as upstream's input model declares it.
struct ModelSpec {
    name: String,
    /// `(gate name, connective, min-or-none, arg specs as "g:Name"/"b:Name")`.
    gates: Vec<(String, String, Option<usize>, Vec<String>)>,
    top: Option<String>,
    /// Gates the extractor refused to parse, with its reason.
    unparsed: Vec<String>,
}

/// One model's answers, as SCRAM reported them.
struct Oracle {
    events: HashMap<String, f64>,
    products: Vec<BTreeSet<String>>,
    totals: HashMap<String, f64>,
}

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/scram")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

fn load_models() -> Vec<ModelSpec> {
    let mut out: Vec<ModelSpec> = Vec::new();
    for line in fixture("models.txt").lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.is_empty() {
            continue;
        }
        match f[0] {
            "MODEL" => out.push(ModelSpec {
                name: f[1].to_string(),
                gates: Vec::new(),
                top: None,
                unparsed: Vec::new(),
            }),
            "GATE" => {
                let m = out.last_mut().expect("GATE before MODEL");
                let min = if f[3] == "-" {
                    None
                } else {
                    Some(f[3].parse().expect("atleast min is a number"))
                };
                m.gates.push((
                    f[1].to_string(),
                    f[2].to_string(),
                    min,
                    f[4..].iter().map(|s| s.to_string()).collect(),
                ));
            }
            "TOP" => out.last_mut().expect("TOP before MODEL").top = Some(f[1].to_string()),
            "CANNOT-PARSE" => out
                .last_mut()
                .expect("CANNOT-PARSE before MODEL")
                .unparsed
                .push(f[1..].join(" ")),
            _ => {}
        }
    }
    out
}

fn load_oracles() -> HashMap<String, Oracle> {
    let mut out = HashMap::new();
    let mut name = String::new();
    let mut cur: Option<Oracle> = None;
    for line in fixture("oracle.txt").lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.is_empty() {
            continue;
        }
        match f[0] {
            "MODEL" => {
                name = f[1].to_string();
                cur = Some(Oracle {
                    events: HashMap::new(),
                    products: Vec::new(),
                    totals: HashMap::new(),
                });
            }
            "EVENT" => {
                if let Some(o) = cur.as_mut() {
                    o.events.insert(f[1].to_string(), f[2].parse().unwrap());
                }
            }
            "PRODUCT" => {
                if let Some(o) = cur.as_mut() {
                    o.products
                        .push(f[2..].iter().map(|s| s.to_string()).collect());
                }
            }
            "TOTAL" => {
                if let Some(o) = cur.as_mut() {
                    o.totals.insert(f[1].to_string(), f[2].parse().unwrap());
                }
            }
            "END" => {
                if let Some(o) = cur.take() {
                    out.insert(std::mem::take(&mut name), o);
                }
            }
            _ => {}
        }
    }
    out
}

/// Sentinel probability for a basic event the model declares but SCRAM's
/// report never mentions.
///
/// SCRAM reports importance only for events that appear in some product, so an
/// event absorbed out of every minimal cut set — `Lift`'s `W_1` and `LMD_1_1`
/// are the two — has no probability in the oracle. Rather than invent one,
/// these are declared with an obviously arbitrary value and every test then
/// **asserts that no generated cut set contains one**. If that assertion ever
/// fails, the value here mattered and the result is void; while it holds, the
/// value is unreachable and cannot influence anything.
const UNPRICED: f64 = 0.5;

/// Builds the tree from the structure fixture, taking probabilities from the
/// oracle.
///
/// Returns the model and the indices of any [`UNPRICED`] basic events, or
/// `None` when the extractor could not read the model — `ThreeMotor` uses
/// house events and has four candidate top gates, neither of which is ported.
/// That is reported as a skip by the tests rather than silently dropped.
fn build(spec: &ModelSpec, oracle: &Oracle) -> Option<(FaultTreeModel, Vec<usize>)> {
    if !spec.unparsed.is_empty() {
        return None;
    }
    let top = spec.top.as_ref()?;

    // Every basic event the model's gates name, whether or not SCRAM priced it.
    let gate_names: BTreeSet<&str> = spec.gates.iter().map(|(n, ..)| n.as_str()).collect();
    let mut referenced: BTreeSet<&str> = BTreeSet::new();
    for (_, _, _, args) in &spec.gates {
        for a in args {
            let name = &a[2..];
            if !gate_names.contains(name) {
                referenced.insert(name);
            }
        }
    }
    let mut event_names: Vec<&str> = oracle.events.keys().map(|s| s.as_str()).collect();
    for name in &referenced {
        if !oracle.events.contains_key(*name) {
            event_names.push(name);
        }
    }
    event_names.sort_unstable();

    let mut b = FaultTreeBuilder::new();
    let mut unpriced = Vec::new();
    for (i, n) in event_names.iter().enumerate() {
        let p = match oracle.events.get(*n) {
            Some(&p) => p,
            None => {
                unpriced.push(i);
                UNPRICED
            }
        };
        b.basic_event(n, p).expect("valid probability");
    }
    for (name, connective, min, args) in &spec.gates {
        let connective = match connective.as_str() {
            "and" => Connective::And,
            "or" => Connective::Or,
            "null" => Connective::Null,
            "atleast" => Connective::Atleast {
                min: min.expect("atleast carries its min"),
            },
            other => panic!("{}: unhandled connective `{other}`", spec.name),
        };
        let arg_names: Vec<&str> = args.iter().map(|a| &a[2..]).collect();
        b.gate(name, connective, &arg_names)
            .unwrap_or_else(|e| panic!("{} gate `{name}`: {e}", spec.name));
    }
    Some((
        b.build(top)
            .unwrap_or_else(|e| panic!("{}: {e}", spec.name)),
        unpriced,
    ))
}

/// Asserts that no generated cut set touches an [`UNPRICED`] basic event.
///
/// See [`UNPRICED`]: this is what keeps the sentinel from ever reaching an
/// arithmetic result.
fn assert_no_unpriced(name: &str, cut_sets: &[raffles::scram::CutSet], unpriced: &[usize]) {
    for c in cut_sets {
        for m in c.members() {
            assert!(
                !unpriced.contains(m),
                "{name}: cut set {:?} contains a basic event SCRAM never priced, so the \
                 sentinel probability would reach the arithmetic",
                c.members()
            );
        }
    }
}

/// Relative agreement, falling back to absolute near zero.
fn agrees(ours: f64, theirs: f64, tol: f64) -> bool {
    let scale = theirs.abs().max(1.0);
    (ours - theirs).abs() / scale < tol
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// **Methodology.** For every model whose structure the extractor could read,
/// build the tree, run [`minimal_cut_sets`] at upstream's own default order
/// limit of 20, and compare the result **as a set of sets** against the
/// `<product>` elements SCRAM reported for the same model.
///
/// Set equality, not a count and not a subset: a missing cut set and a
/// spurious one are different defects and both must fail. Names are used for
/// the comparison rather than indices, so a mis-ordered index mapping cannot
/// hide.
///
/// **Result** (2026-09-21): 7 models compared, and every one produced exactly
/// SCRAM's set of products — **46 cut sets in total, none missing and none
/// spurious**.
///
/// | model | cut sets, both |
/// |---|---|
/// | `TwoTrain/two_train` | 4 |
/// | `Theatre/theatre` | 2 |
/// | `SmallTree/SmallTree` | 2 |
/// | `BSCU/BSCU` | 10 |
/// | `Lift/lift` | 12 |
/// | `HIPPS/HIPPS` | 9 |
/// | `ne574/ne574` | 7 |
///
/// Three models are skipped, each for a stated reason rather than silently:
/// `ThreeMotor/three_motor` uses house events and has four candidate top
/// gates; `ThreeLevels/top` and `TransTest/trans_one` define several fault
/// trees in one file, so `extract_oracle.sh` refuses to attribute their
/// products to a top event and they have no oracle entry.
///
/// **`HIPPS` is the one that earns its place.** It is the only model here with
/// an `atleast` gate (`min="2"` of three pressure switches), so its nine cut
/// sets are the only check that the combination expansion is right — and it is
/// also the only model whose basic-event probabilities SCRAM computes from
/// `periodic-test` and `GLM` expressions rather than reading as literals.
#[test]
fn generated_cut_sets_match_the_ones_scram_found() {
    let specs = load_models();
    let oracles = load_oracles();
    assert!(!specs.is_empty(), "model fixture is empty");

    let mut compared = 0;
    let mut total_cut_sets = 0;
    let mut skipped = Vec::new();
    for spec in &specs {
        let Some(oracle) = oracles.get(&spec.name) else {
            skipped.push(format!("{}: no oracle entry", spec.name));
            continue;
        };
        let Some((model, unpriced)) = build(spec, oracle) else {
            skipped.push(format!("{}: {}", spec.name, spec.unparsed.join("; ")));
            continue;
        };

        let ours = minimal_cut_sets(model.tree(), DEFAULT_LIMIT_ORDER)
            .unwrap_or_else(|e| panic!("{}: {e}", spec.name));
        assert_no_unpriced(&spec.name, &ours, &unpriced);
        let ours_named: BTreeSet<BTreeSet<String>> = ours
            .iter()
            .map(|c| {
                c.members()
                    .iter()
                    .map(|&i| model.basic_event_names()[i].clone())
                    .collect()
            })
            .collect();
        let theirs: BTreeSet<BTreeSet<String>> = oracle.products.iter().cloned().collect();

        println!(
            "{:<24} ours {:>3} cut sets, scram {:>3}",
            spec.name,
            ours_named.len(),
            theirs.len()
        );
        let missing: Vec<_> = theirs.difference(&ours_named).collect();
        let spurious: Vec<_> = ours_named.difference(&theirs).collect();
        assert!(
            missing.is_empty() && spurious.is_empty(),
            "{}: missing {missing:?}, spurious {spurious:?}",
            spec.name
        );
        compared += 1;
        total_cut_sets += ours_named.len();
    }

    for s in &skipped {
        println!("skipped {s}");
    }
    println!("compared {compared} models, {total_cut_sets} cut sets");
    assert!(
        compared >= 7,
        "expected at least 7 models compared, got {compared}"
    );
    assert_eq!(
        skipped.len(),
        3,
        "expected exactly three skips: {skipped:?}"
    );
}

/// **Methodology.** The whole pipeline in one go — input model to top-event
/// probability — with nothing of SCRAM's used except the tree structure and
/// the basic-event probabilities.
///
/// Cut sets are **generated here**, not read from the oracle, then quantified
/// with [`top_event_probability`] and compared against SCRAM's totals from its
/// three separate runs. This is strictly stronger than
/// `scram_oracle_suite::every_model_total_probability_matches_scram`, which
/// hands the quantifier SCRAM's own cut sets: a cut-set generation defect that
/// happened to cancel against a quantification defect would pass there and
/// fail here.
///
/// Tolerance `5e-6` relative — the resolution of upstream's
/// 6-significant-figure report.
///
/// **Result** (2026-09-21): **21 model/mode combinations checked across the 7
/// readable models, all agreeing.** `Exact` is skipped where a model has more
/// than [`EXACT_CUT_SET_LIMIT`] cut sets, since inclusion-exclusion is `2^n`;
/// no model in the present fixture trips that, the largest being `BSCU` and
/// `Lift` at 10 and 12.
///
/// Spot values, ours against SCRAM:
///
/// | model | exact | rare-event | MCUB |
/// |---|---|---|---|
/// | `TwoTrain` | 0.722500000 / 0.722500000 | 1 / 1 | 0.838393750 / 0.838394 |
/// | `BSCU` | 0.112408535 / 0.112409 | 0.135371755 / 0.135372 | 0.128587773 / 0.128588 |
/// | `HIPPS` | 0.001620905 / 0.00162091 | 0.001621881 / 0.00162188 | 0.001620905 / 0.00162091 |
/// | `ne574` | 0.662208000 / 0.662208 | 1 / 1 | 0.726479149 / 0.726479 |
#[test]
fn quantifying_generated_cut_sets_reproduces_scrams_totals() {
    let specs = load_models();
    let oracles = load_oracles();
    let mut checked = 0;

    for spec in &specs {
        let Some(oracle) = oracles.get(&spec.name) else {
            continue;
        };
        let Some((model, unpriced)) = build(spec, oracle) else {
            continue;
        };
        let cut_sets = minimal_cut_sets(model.tree(), DEFAULT_LIMIT_ORDER).unwrap();
        assert_no_unpriced(&spec.name, &cut_sets, &unpriced);

        for (mode, approximation) in [
            ("exact", Approximation::Exact),
            ("rare-event", Approximation::RareEvent),
            ("mcub", Approximation::Mcub),
        ] {
            let Some(&theirs) = oracle.totals.get(mode) else {
                continue;
            };
            if approximation == Approximation::Exact && cut_sets.len() > EXACT_CUT_SET_LIMIT {
                println!(
                    "{:<24} exact skipped, {} cut sets exceeds the 2^n limit",
                    spec.name,
                    cut_sets.len()
                );
                continue;
            }
            let ours = top_event_probability(&cut_sets, model.probabilities(), approximation)
                .unwrap_or_else(|e| panic!("{} [{mode}]: {e}", spec.name));
            println!(
                "{:<24} {mode:<11} ours {ours:.9}  scram {theirs:.9}",
                spec.name
            );
            assert!(
                agrees(ours, theirs, 5e-6),
                "{} [{mode}]: ours {ours}, SCRAM {theirs}",
                spec.name
            );
            checked += 1;
        }
    }
    println!("checked {checked} model/mode combinations end to end");
    assert!(checked >= 18, "expected a broad sweep, only {checked}");
}

/// **Methodology.** Properties every correct minimal-cut-set generator must
/// satisfy, checked on every model. These would catch a defect that happened
/// to agree with SCRAM on these particular models.
///
/// 1. **Minimality** — no cut set properly contains another. A generator that
///    forgets absorption produces a superset-laden list that still quantifies
///    to roughly the right answer under the rare-event approximation, so this
///    is not implied by the totals matching.
/// 2. **Distinctness** — no cut set appears twice.
/// 3. **Sufficiency** — every cut set really does cause the top event, checked
///    by evaluating the tree with exactly that cut set's events set true and
///    all others false.
/// 4. **Necessity** — removing any single member of any cut set makes the tree
///    evaluate false. Together with (3) this is the definition of minimal.
///
/// **Result** (2026-09-21): all four hold for all **46 cut sets** of the 7
/// readable models. Properties 3 and 4 between them evaluate the tree **120
/// times** (46 sufficiency checks and 74 necessity checks, one per member of
/// each cut set).
#[test]
fn generated_cut_sets_are_minimal_by_construction_not_by_luck() {
    let specs = load_models();
    let oracles = load_oracles();
    let mut evaluations = 0;
    let mut cut_sets_checked = 0;

    for spec in &specs {
        let Some(oracle) = oracles.get(&spec.name) else {
            continue;
        };
        let Some((model, _)) = build(spec, oracle) else {
            continue;
        };
        let cut_sets = minimal_cut_sets(model.tree(), DEFAULT_LIMIT_ORDER).unwrap();

        for (i, a) in cut_sets.iter().enumerate() {
            for (j, b) in cut_sets.iter().enumerate() {
                if i == j {
                    continue;
                }
                assert_ne!(a, b, "{}: duplicate cut set {:?}", spec.name, a.members());
                let a_set: BTreeSet<usize> = a.members().iter().copied().collect();
                let b_set: BTreeSet<usize> = b.members().iter().copied().collect();
                assert!(
                    !a_set.is_subset(&b_set),
                    "{}: {:?} contains {:?}, so the latter is not minimal",
                    spec.name,
                    b.members(),
                    a.members()
                );
            }

            // Sufficiency.
            let members: Vec<usize> = a.members().to_vec();
            assert!(
                evaluate(&model, &members),
                "{}: cut set {:?} does not cause the top event",
                spec.name,
                a.members()
            );
            evaluations += 1;

            // Necessity.
            for &drop in a.members() {
                let reduced: Vec<usize> = members.iter().copied().filter(|&m| m != drop).collect();
                assert!(
                    !evaluate(&model, &reduced),
                    "{}: cut set {:?} still causes the top event without event {}, \
                     so it is not minimal",
                    spec.name,
                    a.members(),
                    model.basic_event_names()[drop]
                );
                evaluations += 1;
            }
            cut_sets_checked += 1;
        }
    }
    println!("{cut_sets_checked} cut sets, {evaluations} tree evaluations");
    assert!(cut_sets_checked >= 45, "only {cut_sets_checked} cut sets");
}

/// Evaluates the fault tree with exactly `true_events` set true.
///
/// Written out here rather than in the library on purpose: an independent
/// evaluator is what makes the minimality check above a real check. If the
/// same code both generated and verified the cut sets, it would only be
/// testing self-consistency.
fn evaluate(model: &FaultTreeModel, true_events: &[usize]) -> bool {
    fn eval(model: &FaultTreeModel, gate: usize, on: &[usize]) -> bool {
        use raffles::scram::fault_tree::Arg;
        let g = &model.tree().gates()[gate];
        let value = |arg: &Arg| match arg {
            Arg::BasicEvent(e) => on.contains(e),
            Arg::Gate(sub) => eval(model, *sub, on),
        };
        match g.connective() {
            Connective::And => g.args().iter().all(value),
            Connective::Or => g.args().iter().any(value),
            Connective::Null => g.args().iter().all(value),
            Connective::Atleast { min } => g.args().iter().filter(|a| value(a)).count() >= min,
            other => panic!("evaluator does not handle `{}`", other.as_str()),
        }
    }
    eval(model, model.tree().top(), true_events)
}

/// **Methodology.** The refusals, each of which is a deliberate limit rather
/// than an oversight: a non-coherent tree (complement elimination not ported),
/// a zero order limit, a cyclic tree, an undeclared argument name, a duplicate
/// name, and the arity rules.
///
/// **Result** (2026-09-21): all refused.
#[test]
fn the_unported_and_the_malformed_are_refused_not_guessed() {
    // Non-coherent: NOT is representable so that it can be refused clearly.
    let mut b = FaultTreeBuilder::new();
    b.basic_event("A", 0.1).unwrap();
    b.gate("Top", Connective::Not, &["A"]).unwrap();
    let model = b.build("Top").unwrap();
    assert!(!model.tree().is_coherent());
    let err = minimal_cut_sets(model.tree(), DEFAULT_LIMIT_ORDER).unwrap_err();
    let message = err.to_string();
    assert!(
        message.contains("non-coherent") && message.contains("Complement elimination"),
        "the refusal should name what is missing, got: {message}"
    );

    // A zero order limit admits nothing at all.
    let mut b = FaultTreeBuilder::new();
    b.basic_event("A", 0.1).unwrap();
    b.gate("Top", Connective::Or, &["A"]).unwrap();
    let model = b.build("Top").unwrap();
    assert!(minimal_cut_sets(model.tree(), 0).is_err());
    assert_eq!(minimal_cut_sets(model.tree(), 1).unwrap().len(), 1);

    // A cycle would make expansion non-terminating, so it is caught at build.
    let mut b = FaultTreeBuilder::new();
    b.basic_event("A", 0.1).unwrap();
    b.gate("Top", Connective::Or, &["A", "Loop"]).unwrap();
    b.gate("Loop", Connective::Or, &["Top"]).unwrap();
    assert!(b.build("Top").is_err(), "a cycle must be refused");

    // An undeclared name is an error, not an implicit basic event.
    let mut b = FaultTreeBuilder::new();
    b.gate("Top", Connective::Or, &["Ghost"]).unwrap();
    assert!(
        b.build("Top").is_err(),
        "an undeclared argument must be refused"
    );

    // Names share one namespace, as they do in a SCRAM input model.
    let mut b = FaultTreeBuilder::new();
    b.basic_event("A", 0.1).unwrap();
    assert!(
        b.gate("A", Connective::Or, &["A"]).is_err(),
        "duplicate name"
    );

    // Arity.
    let mut b = FaultTreeBuilder::new();
    b.basic_event("A", 0.1).unwrap();
    b.basic_event("B", 0.1).unwrap();
    assert!(
        b.gate("G1", Connective::Null, &["A", "B"]).is_err(),
        "null arity"
    );
    assert!(b.gate("G2", Connective::Xor, &["A"]).is_err(), "xor arity");
    assert!(
        b.gate("G3", Connective::Atleast { min: 3 }, &["A", "B"])
            .is_err(),
        "atleast min above the argument count"
    );
    assert!(b.gate("G4", Connective::And, &[]).is_err(), "no arguments");

    // A top gate that was never declared.
    let mut b = FaultTreeBuilder::new();
    b.basic_event("A", 0.1).unwrap();
    b.gate("Top", Connective::Or, &["A"]).unwrap();
    assert!(b.build("Nowhere").is_err(), "undeclared top gate");
}

/// **Methodology.** Truncation must behave as an order limit, not as an
/// arbitrary cut-off: at limit `k` the result must be exactly the cut sets of
/// order `<= k` from the untruncated run.
///
/// Checked on every readable model, at every limit from 1 up to the largest
/// cut-set order the model has.
///
/// **Result** (2026-09-21): exact agreement at every limit on every model —
/// **14 (model, limit) cases**, the cut-set orders being `TwoTrain` 2,
/// `Theatre` 2, `SmallTree` 2, `BSCU` 2, `Lift` 1, `HIPPS` 2, `ne574` 3.
///
/// This matters because truncation is applied *during* expansion, where a
/// too-eager prune could drop a partial set that would still have reached a
/// short cut set — the check is that it does not.
///
/// The orders are all small, so this is a **weak** check of truncation and is
/// recorded as such: nothing here exercises a limit deep inside a long cut
/// set, which is where a real model's truncation actually bites.
#[test]
fn truncating_by_order_drops_exactly_the_long_cut_sets() {
    let specs = load_models();
    let oracles = load_oracles();
    let mut cases = 0;

    for spec in &specs {
        let Some(oracle) = oracles.get(&spec.name) else {
            continue;
        };
        let Some((model, _)) = build(spec, oracle) else {
            continue;
        };
        let full = minimal_cut_sets(model.tree(), DEFAULT_LIMIT_ORDER).unwrap();
        let max_order = full.iter().map(|c| c.order()).max().unwrap_or(0);

        for limit in 1..=max_order {
            let truncated = minimal_cut_sets(model.tree(), limit).unwrap();
            let expected: Vec<_> = full
                .iter()
                .filter(|c| c.order() <= limit)
                .cloned()
                .collect();
            assert_eq!(
                truncated, expected,
                "{} at limit {limit}: truncation is not order-based",
                spec.name
            );
            cases += 1;
        }
        println!("{:<24} max order {max_order}", spec.name);
    }
    println!("{cases} (model, limit) cases");
    assert!(cases >= 7, "only {cases} truncation cases");
}
