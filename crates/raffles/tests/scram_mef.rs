//! # Code-to-code verification of the MEF reader, against SCRAM's own input models
//!
//! [`scram_mocus_oracle`](../scram_mocus_oracle/index.html) builds its fault
//! trees from `reference-data/scram/models.txt`, a **shell transcription** of
//! upstream's input models produced by `extract_models.sh`. That transcription
//! is a flat subset of the Model Exchange Format, and it says so: three of its
//! records are `CANNOT-PARSE`, and `ThreeMotor/three_motor` — a model with a
//! full oracle, upstream's own `RiskAnalysisTest.ThreeMotor`, and no analysis
//! route in this crate — is one of them.
//!
//! This file removes the transcription. [`raffles::scram::mef`] reads
//! upstream's XML directly, so every test here goes **input model to answer**
//! with nothing hand-written in between:
//!
//! | | comes from |
//! |---|---|
//! | the question (gates, connectives, references, expressions) | upstream's own `.xml`, read here |
//! | the answers (products, totals, importance) | SCRAM's own XML report, via `extract_oracle.sh` |
//!
//! Basic-event probabilities are now on the **question** side too: they are
//! computed here by evaluating the model's own expressions, and checked
//! against the probabilities SCRAM printed. The transcription route could not
//! do that — it took them from the oracle, because it had no expression
//! evaluator.
//!
//! ## What this unlocks, and what it still cannot read
//!
//! * **`ThreeMotor/three_motor`** — verified end to end for the first time.
//!   It declares `E1` twice, publicly and inside `<define-component
//!   name="t" role="private">`, and a reader that merges namespaces builds a
//!   different tree. See [`three_motor_is_unlocked_by_the_private_namespace_rule`].
//! * **`<xi:include>`** — `TransTest/trans_one`, and the cross-fault-tree
//!   references of `ThreeLevels/top`, against a new per-tree fixture.
//! * **`<iff>`, `<imply>`, `<cardinality>`, `<constant>`, `<event type="…">`**
//!   — five constructs **no upstream input model uses**, so a model written
//!   for this port carries them and SCRAM supplies the answers.
//! * **The seven random deviates** — `SmallTree/SmallTree` and `BSCU/BSCU`
//!   were out of reach until they landed, and the other five appear in no
//!   upstream input at all, so `models-for-this-port/deviates.xml` carries
//!   them with SCRAM's own answers in `oracle-deviates.txt`. **Every record in
//!   every fixture now reads**; there is no allowance left for one that does
//!   not.
//!
//! | | |
//! |---|---|
//! | Upstream | <https://github.com/rakhimov/scram> @ `b85b7894` (2019-07-03) |
//! | Version | SCRAM 0.16.2 |
//! | Models | `reference-data/scram/upstream-input/`, copied verbatim from upstream's `input/` |
//!
//! Full record: `crates/raffles/docs/scram-port-verification.md`.

use raffles::scram::bdd::Bdd;
use raffles::scram::fault_tree::FaultTreeModel;
use raffles::scram::importance::importance_factors;
use raffles::scram::mef::MefModel;
use raffles::scram::mocus::{minimal_cut_sets, DEFAULT_LIMIT_ORDER};
use raffles::scram::probability::{top_event_probability, Approximation, EXACT_CUT_SET_LIMIT};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// The `reference-data/scram` directory.
fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/scram")
}

/// The input model a `MODEL <dir>/<stem>` record names.
///
/// Upstream's models are copied verbatim under `upstream-input/`; the handful
/// written for this port live beside them in `models-for-this-port/`.
fn model_path(model: &str) -> PathBuf {
    if model.starts_with("models-for-this-port/") {
        data_dir().join(format!("{model}.xml"))
    } else {
        data_dir().join(format!("upstream-input/{model}.xml"))
    }
}

/// One model's oracle, as SCRAM reported it.
struct Oracle {
    name: String,
    /// The fault tree SCRAM analysed, for a fixture with several per model.
    tree: Option<String>,
    top: Option<String>,
    events: HashMap<String, f64>,
    products: Vec<BTreeSet<String>>,
    totals: HashMap<String, f64>,
    /// `name -> (occurrence, mif, cif, dif, raw, rrw)`.
    importance: HashMap<String, (usize, f64, f64, f64, f64, f64)>,
}

fn load_oracles(fixture: &str) -> Vec<Oracle> {
    let text = std::fs::read_to_string(data_dir().join(fixture))
        .unwrap_or_else(|e| panic!("{fixture}: {e}"));
    let mut out = Vec::new();
    let mut cur: Option<Oracle> = None;
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        let Some(&tag) = f.first() else { continue };
        match tag {
            "MODEL" => {
                cur = Some(Oracle {
                    name: f[1].to_string(),
                    tree: None,
                    top: None,
                    events: HashMap::new(),
                    products: Vec::new(),
                    totals: HashMap::new(),
                    importance: HashMap::new(),
                })
            }
            "TREE" => {
                if let Some(o) = cur.as_mut() {
                    o.tree = Some(f[1].to_string());
                    o.top = Some(f[1].to_string());
                }
            }
            "TOPNAME" => {
                if let Some(o) = cur.as_mut() {
                    o.top = Some(f[1].to_string());
                }
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
            "IMPORTANCE" => {
                if let Some(o) = cur.as_mut() {
                    o.importance.insert(
                        f[1].to_string(),
                        (
                            f[2].parse().unwrap(),
                            f[3].parse().unwrap(),
                            f[4].parse().unwrap(),
                            f[5].parse().unwrap(),
                            f[6].parse().unwrap(),
                            f[7].parse().unwrap(),
                        ),
                    );
                }
            }
            "END" => {
                if let Some(o) = cur.take() {
                    out.push(o)
                }
            }
            _ => {}
        }
    }
    assert!(!out.is_empty(), "{fixture} is empty");
    out
}

/// Every record of every coherent fixture this file checks.
fn all_oracles() -> Vec<Oracle> {
    let mut out = load_oracles("oracle.txt");
    out.extend(load_oracles("oracle-mef.txt"));
    out.extend(load_oracles("oracle-multi-tree.txt"));
    out.extend(load_oracles("oracle-deviates.txt"));
    out
}

/// Reads a model and builds the tree the oracle's answers belong to.
///
/// There is no allowance for a model that fails to read: every record in the
/// fixtures must go through. `SmallTree/SmallTree` and `BSCU/BSCU` were the
/// two exceptions until the random deviates landed on 2026-09-22.
fn tree_for(oracle: &Oracle) -> Option<FaultTreeModel> {
    let model = MefModel::from_file(&model_path(&oracle.name))
        .unwrap_or_else(|e| panic!("{}: the reader refused this model -- {e}", oracle.name));
    let top = oracle
        .top
        .as_ref()
        .unwrap_or_else(|| panic!("{}: the oracle records no top event", oracle.name));
    Some(
        model
            .fault_tree(top)
            .unwrap_or_else(|e| panic!("{} [{top}]: {e}", oracle.name)),
    )
}

/// The cut sets, as sets of basic-event names.
fn named_cut_sets(model: &FaultTreeModel) -> Vec<BTreeSet<String>> {
    let names = model.basic_event_names();
    minimal_cut_sets(model.tree(), DEFAULT_LIMIT_ORDER)
        .expect("cut sets")
        .iter()
        .map(|c| c.members().iter().map(|&m| names[m].clone()).collect())
        .collect()
}

/// Relative agreement, falling back to absolute near zero.
fn agrees(ours: f64, theirs: f64, tol: f64) -> bool {
    let scale = theirs.abs().max(1.0);
    (ours - theirs).abs() / scale < tol
}

// ---------------------------------------------------------------------------
// The tests
// ---------------------------------------------------------------------------

/// **Methodology.** Read every model in the oracles from its own XML, build
/// the fault tree the oracle's answers belong to, and compare the
/// probability this crate **computes for each basic event** — by evaluating
/// the model's own `<float>`, `<exponential>`, `<parameter>` and
/// `<system-mission-time>` elements — against the probability SCRAM printed
/// for that event in its report.
///
/// This is the check the transcription route could not make: `models.txt`
/// takes basic-event probabilities from the oracle, so a wrong expression
/// evaluation there is invisible. Here the two sides are independent.
///
/// Tolerance is `5e-6` relative, the resolution of upstream's
/// 6-significant-figure report.
///
/// **Results** (2026-09-22): all 17 records read, and all 113 basic-event
/// probabilities across them agree.
#[test]
fn probabilities_evaluated_from_the_xml_match_scrams() {
    let mut checked = 0;
    let mut models = 0;
    for o in all_oracles() {
        let Some(model) = tree_for(&o) else { continue };
        models += 1;
        for (i, name) in model.basic_event_names().iter().enumerate() {
            let Some(&theirs) = o.events.get(name) else {
                // SCRAM prints only the events its analysis reached; a
                // house-event branch can cut one out of the report.
                continue;
            };
            let ours = model.probabilities()[i];
            assert!(
                agrees(ours, theirs, 5e-6),
                "{} {name}: evaluated {ours}, SCRAM reported {theirs}",
                o.name
            );
            checked += 1;
        }
        println!(
            "{:<34} {} basic events priced",
            o.name,
            model.probabilities().len()
        );
    }
    println!("{checked} basic-event probabilities across {models} models");
    assert!(checked >= 110, "only checked {checked} probabilities");
}

/// **Methodology.** Generate the minimal cut sets with
/// [`minimal_cut_sets`] from the tree read out of upstream's XML, and compare
/// the set of products against the set SCRAM reported. Set equality, not a
/// count: a missing product and a spurious one would cancel in a count.
///
/// **Results** (2026-09-22): every product of every model matches,
/// `ThreeMotor/three_motor` included — 17 records. The largest is
/// `Aralia/chinese` at 392 products.
#[test]
fn cut_sets_generated_from_the_xml_match_scrams() {
    let mut checked = 0;
    for o in all_oracles() {
        let Some(model) = tree_for(&o) else { continue };
        let ours: BTreeSet<BTreeSet<String>> = named_cut_sets(&model).into_iter().collect();
        let theirs: BTreeSet<BTreeSet<String>> = o.products.iter().cloned().collect();
        println!("{:<34} {} products", o.name, theirs.len());
        let missing: Vec<&BTreeSet<String>> = theirs.difference(&ours).collect();
        let extra: Vec<&BTreeSet<String>> = ours.difference(&theirs).collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "{}: SCRAM found {} products, this port found {}. Missing {missing:?}, extra {extra:?}",
            o.name,
            theirs.len(),
            ours.len()
        );
        checked += 1;
    }
    assert!(checked >= 17, "only checked {checked} models");
}

/// **Methodology.** Quantify the tree read from the XML in all three of
/// upstream's modes and compare against SCRAM's own three runs.
///
/// The two approximations are computed **from the cut sets** by both codes,
/// so they must agree for every model. The exact value is not: for a
/// **non-coherent** tree SCRAM's unflagged `probability` is the true
/// function's, from its BDD, while inclusion-exclusion over the minimal cut
/// sets describes a strictly larger function (deleting negative literals
/// throws away the requirement that something be working). So the exact value
/// is taken from this port's own BDD where the tree is non-coherent, and by
/// inclusion-exclusion where it is coherent and within
/// [`EXACT_CUT_SET_LIMIT`], since that is `2^n`.
///
/// **Results** (2026-09-22): 50 model/mode combinations, every one agreeing
/// to `5e-6` relative — including `models-for-this-port/mef_features`, whose
/// `<imply>`, `<iff>` and `<cardinality>` gates make it non-coherent and
/// whose exact value therefore comes from the BDD.
#[test]
fn totals_from_the_xml_match_scrams() {
    let mut checked = 0;
    for o in all_oracles() {
        let Some(model) = tree_for(&o) else { continue };
        let cut_sets = minimal_cut_sets(model.tree(), DEFAULT_LIMIT_ORDER).unwrap();
        let coherent = model.tree().is_coherent();
        for (mode, approximation) in [
            ("exact", Approximation::Exact),
            ("rare-event", Approximation::RareEvent),
            ("mcub", Approximation::Mcub),
        ] {
            let Some(&theirs) = o.totals.get(mode) else {
                continue;
            };
            let ours = if approximation == Approximation::Exact {
                if coherent {
                    if cut_sets.len() > EXACT_CUT_SET_LIMIT {
                        continue;
                    }
                    top_event_probability(&cut_sets, model.probabilities(), approximation).unwrap()
                } else {
                    Bdd::build(model.tree())
                        .and_then(|b| b.probability(model.probabilities()))
                        .unwrap_or_else(|e| panic!("{}: BDD: {e}", o.name))
                }
            } else {
                top_event_probability(&cut_sets, model.probabilities(), approximation).unwrap()
            };
            assert!(
                agrees(ours, theirs, 5e-6),
                "{} [{mode}{}]: ours {ours}, SCRAM {theirs}",
                o.name,
                if coherent { "" } else { ", non-coherent" }
            );
            checked += 1;
        }
    }
    println!("checked {checked} model/mode combinations");
    assert!(checked >= 45, "only checked {checked} combinations");
}

/// **Methodology.** Every importance factor of every basic event, computed
/// from the tree read out of the XML, against `scram --probability
/// --importance`. Models with more cut sets than [`EXACT_CUT_SET_LIMIT`] are
/// skipped, as importance needs the exact total to match upstream.
///
/// RRW at the singular points is the documented divergence pinned by
/// `scram_oracle_suite::rrw_diverges_from_upstream_only_at_the_singularity`,
/// and is passed over here rather than re-litigated.
///
/// **Results** (2026-09-22): 78 basic events checked, every factor of every
/// one agreeing to `5e-6` relative.
#[test]
fn importance_from_the_xml_matches_scrams() {
    let mut checked = 0;
    for o in all_oracles() {
        let Some(model) = tree_for(&o) else { continue };
        let cut_sets = minimal_cut_sets(model.tree(), DEFAULT_LIMIT_ORDER).unwrap();
        if cut_sets.len() > EXACT_CUT_SET_LIMIT || !model.tree().is_coherent() {
            // A non-coherent tree's factors are derived from an exact total
            // the cut sets cannot reproduce, so comparing them here would be
            // comparing two different quantities. `scram_noncoherent` owns
            // that case.
            continue;
        }
        for (name, &(occ, mif, cif, dif, raw, rrw)) in &o.importance {
            let Some(i) = model.basic_event_index(name) else {
                continue;
            };
            let ours =
                importance_factors(i, &cut_sets, model.probabilities(), Approximation::Exact)
                    .unwrap();
            assert_eq!(ours.occurrence, occ, "{} {name}: occurrence", o.name);
            for (label, ours, theirs) in [
                ("MIF", ours.mif, mif),
                ("CIF", ours.cif, cif),
                ("DIF", ours.dif, dif),
                ("RAW", ours.raw, raw),
            ] {
                assert!(
                    agrees(ours, theirs, 5e-6),
                    "{} {name}: {label} {ours} vs {theirs}",
                    o.name
                );
            }
            if !ours.rrw.is_infinite() {
                assert!(
                    agrees(ours.rrw, rrw, 5e-6),
                    "{} {name}: RRW {} vs {rrw}",
                    o.name,
                    ours.rrw
                );
            }
            checked += 1;
        }
    }
    println!("checked {checked} basic events");
    assert!(checked >= 70, "only checked {checked} events");
}

/// **Methodology — the model this reader was written for.**
///
/// `ThreeMotor/three_motor` declares `E1` twice: once at the top of
/// `<define-fault-tree name="ThreeMotor">`, and once inside
/// `<define-component name="t" role="private">`. Upstream keeps them apart
/// with `Id::id()` — a public element keeps its bare name, a private one is
/// known by its full path — and resolves a reference by looking in the local
/// path first and the model's public names second (`Initializer::GetEntity`).
///
/// This asserts each half of that rule separately, so a reader that got the
/// right cut sets by accident would still fail:
///
/// 1. both gates exist, under `E1` and `ThreeMotor.t.E1`, with **different
///    formulas**;
/// 2. the outer tree's `<gate name="t.E1"/>` reaches the **private** gate,
///    by local path — not a public `t.E1`, which does not exist;
/// 3. `<basic-event name="K1"/>` written *inside* the component resolves
///    **outward** to the public `K1`, because the component declares none;
/// 4. the cut sets and the total then match SCRAM.
///
/// **Results** (2026-09-22): all four hold. The model reads 18 gates (11
/// public, 7 inside the private component), 15 basic events and 3 house
/// events, and SCRAM's 12 products are reproduced exactly.
#[test]
fn three_motor_is_unlocked_by_the_private_namespace_rule() {
    use raffles::scram::mef::FormulaArg;

    let model =
        MefModel::from_file(&model_path("ThreeMotor/three_motor")).expect("ThreeMotor reads");

    // 1. Two distinct E1 declarations.
    let public = model.gates.get("E1").expect("public E1");
    let private = model
        .gates
        .get("ThreeMotor.t.E1")
        .expect("private E1, known by its full path");
    assert_ne!(
        public, private,
        "the two E1 declarations were merged, which is the exact failure this reader exists \
         to prevent"
    );

    // 2. The outer reference `t.E1` reaches the private gate.
    let e4 = model.gates.get("E4").expect("E4");
    assert!(
        e4.args.iter().any(|a| matches!(
            a,
            FormulaArg::Event { name, .. } if name == "ThreeMotor.t.E1"
        )),
        "E4's <gate name=\"t.E1\"/> did not resolve to the private gate: {:?}",
        e4.args
    );

    // 3. A reference inside the component to a name it does not declare
    //    resolves outward to the public declaration.
    assert!(
        private.args.iter().any(|a| matches!(
            a,
            FormulaArg::Event { name, .. } if name == "K1"
        )),
        "the private E1's <basic-event name=\"K1\"/> did not resolve outward: {:?}",
        private.args
    );
    assert!(
        !model.basic_events.contains_key("ThreeMotor.t.K1"),
        "the component declares no K1, so a private one must not have been invented"
    );

    println!(
        "ThreeMotor: {} gates, {} basic events, {} house events",
        model.gates.len(),
        model.basic_events.len(),
        model.house_events.len()
    );

    // 4. The answers.
    let oracle = load_oracles("oracle.txt")
        .into_iter()
        .find(|o| o.name == "ThreeMotor/three_motor")
        .expect("ThreeMotor is in the oracle");
    let tree = model.fault_tree(oracle.top.as_ref().unwrap()).unwrap();
    let ours: BTreeSet<BTreeSet<String>> = named_cut_sets(&tree).into_iter().collect();
    let theirs: BTreeSet<BTreeSet<String>> = oracle.products.iter().cloned().collect();
    assert_eq!(
        ours,
        theirs,
        "ThreeMotor's cut sets do not match SCRAM's {} products",
        theirs.len()
    );
    println!("ThreeMotor: {} products reproduced", theirs.len());
}

/// **Methodology.** `<xi:include>` and references that cross fault trees.
///
/// `TransTest/trans_one` splices two documents with
/// `<xi:include href="…" xpointer="xpointer(/opsa-mef/*)"/>`; upstream lets
/// libxml2 do it, this port splices the children of the included
/// `<opsa-mef>` into the including element. `ThreeLevels/top` chains three
/// fault trees, each referencing the next by bare public name.
///
/// Both models define several fault trees, so `extract_oracle.sh` refuses
/// them — its record format has one top event per model. The per-tree
/// fixture `oracle-multi-tree.txt` (from `extract_multi_tree_oracle.sh`)
/// exists for exactly this, and both are checked through it by the sweeps
/// above. This adds the structural assertions those sweeps cannot make.
///
/// **Results** (2026-09-22): `trans_one` reads 2 gates and 3 basic events,
/// none of which is declared in `trans_one.xml` itself; `ThreeLevels/top`
/// resolves `Top -> Middle -> Bottom` across three `<define-fault-tree>`
/// elements.
#[test]
fn xi_include_and_cross_fault_tree_references_resolve() {
    let model = MefModel::from_file(&model_path("TransTest/trans_one")).expect("trans_one reads");
    assert!(
        model.gates.contains_key("TransTwo"),
        "the included document's gate is missing, so <xi:include> was not spliced"
    );
    for event in ["A", "B", "C"] {
        assert!(
            model.basic_events.contains_key(event),
            "`{event}` comes from the second include and is missing"
        );
    }
    let tree = model.fault_tree("TransOne").unwrap();
    assert_eq!(
        named_cut_sets(&tree),
        vec![["A", "B", "C"]
            .iter()
            .map(|s| s.to_string())
            .collect::<BTreeSet<String>>()]
    );

    let model = MefModel::from_file(&model_path("ThreeLevels/top")).expect("ThreeLevels reads");
    let tree = model.fault_tree("Top").unwrap();
    // Top -> Middle -> Bottom, each in its own <define-fault-tree>.
    assert!(tree.gate_names().contains(&"Middle".to_string()));
    assert!(tree.gate_names().contains(&"Bottom".to_string()));
}

/// **Methodology — the five constructs upstream's own inputs never use.**
///
/// A grep over all of `input/` finds no `<iff>`, `<imply>`, `<cardinality>`,
/// `<constant>` formula argument or `<event type="…">`, so none of them could
/// be verified against a model SCRAM ships.
/// `models-for-this-port/mef_features.xml` uses all five, plus a second,
/// independent instance of the private-namespace rule, and SCRAM supplies the
/// answers in `oracle-mef.txt`.
///
/// The three MEF-only connectives are the interesting ones: they have no PDAG
/// representation, and `Pdag::ConstructComplexGate` rewrites each into the
/// eight analysis connectives. This port has to perform the identical
/// rewrite, and a wrong one changes the products.
///
/// **Results** (2026-09-22): all 9 of SCRAM's products reproduced, all three
/// totals agree, and the two `rate` parameters resolve to their own scopes —
/// `MefFeatures.sub.p` at `0.295594` from the private `4.0e-5` and `shared`
/// at `0.0838727` from the public `1.0e-5`, both as SCRAM printed them.
#[test]
fn the_mef_only_constructs_match_scram() {
    let oracle = load_oracles("oracle-mef.txt").pop().unwrap();
    let model = MefModel::from_file(&model_path(&oracle.name)).expect("mef_features reads");

    // The shadowed parameter, checked by value rather than by name: the two
    // events differ only in which `rate` their expression resolved to.
    let tree = model.fault_tree("top").unwrap();
    for (event, expected) in [("MefFeatures.sub.p", 0.295594), ("shared", 0.0838727)] {
        let i = tree
            .basic_event_index(event)
            .unwrap_or_else(|| panic!("`{event}` is in the tree"));
        assert!(
            agrees(tree.probabilities()[i], expected, 5e-6),
            "{event}: {} evaluated, SCRAM printed {expected} -- the two `rate` parameters \
             did not resolve to their own scopes",
            tree.probabilities()[i]
        );
    }

    let ours: BTreeSet<BTreeSet<String>> = named_cut_sets(&tree).into_iter().collect();
    let theirs: BTreeSet<BTreeSet<String>> = oracle.products.iter().cloned().collect();
    assert_eq!(ours, theirs, "the MEF-only connectives rewrote differently");

    let cut_sets = minimal_cut_sets(tree.tree(), DEFAULT_LIMIT_ORDER).unwrap();
    for (mode, approximation) in [
        ("rare-event", Approximation::RareEvent),
        ("mcub", Approximation::Mcub),
    ] {
        let theirs = oracle.totals[mode];
        let ours = top_event_probability(&cut_sets, tree.probabilities(), approximation).unwrap();
        assert!(
            agrees(ours, theirs, 5e-6),
            "mef_features [{mode}]: ours {ours}, SCRAM {theirs}"
        );
    }
    // `<imply>` and `<iff>` put complements into the tree, so it is
    // non-coherent and its exact probability is the BDD's, not the cut sets'.
    assert!(
        !tree.tree().is_coherent(),
        "the MEF-only connectives should have made this tree non-coherent"
    );
    let exact = Bdd::build(tree.tree())
        .and_then(|b| b.probability(tree.probabilities()))
        .expect("BDD");
    assert!(
        agrees(exact, oracle.totals["exact"], 5e-6),
        "mef_features [exact, BDD]: ours {exact}, SCRAM {}",
        oracle.totals["exact"]
    );
}

/// **Methodology.** Everything this reader refuses, refused for a reason it
/// names — never skipped.
///
/// A skipped declaration is the failure mode this whole port exists to avoid:
/// the model still parses, the analysis still runs, and the answer is for a
/// different, smaller model. Each case below is a construct upstream supports
/// and this chunk does not, or a document defect.
///
/// ~~A whole upstream model whose only unread construct is CCF groups —
/// `TwoTrain/common_cause`.~~ **CORRECTED 2026-09-22** — CCF groups landed
/// the same day, so that case moved to `scram_ccf`, where the model is read
/// and its answers checked rather than its refusal.
///
/// ~~A `<define-substitution>` refusal.~~ **CORRECTED 2026-09-22** —
/// substitutions landed the same day, so that case moved to
/// `scram_substitutions`, which checks their validation rather than their
/// refusal.
///
/// **Results** (2026-09-22): all eight refusals hold, each naming its cause.
#[test]
fn the_unported_and_the_malformed_are_refused_not_skipped() {
    let refuse = |xml: &str, expected: &str| {
        let element = raffles::scram::mef::parse_xml(xml).expect("well-formed XML");
        let err = MefModel::from_element(&element)
            .expect_err("must be refused")
            .to_string();
        assert!(
            err.contains(expected),
            "expected a refusal naming `{expected}`, got: {err}"
        );
    };

    // An element the reader does not know is an error, not a skip.
    refuse(
        r#"<opsa-mef><define-fault-tree name="F"><define-nonsense name="x"/></define-fault-tree></opsa-mef>"#,
        "unrecognised element",
    );
    // Event trees, alignments, substitutions, extern functions.
    refuse(
        r#"<opsa-mef><define-event-tree name="E"/></opsa-mef>"#,
        "event-tree analysis",
    );
    refuse(
        r#"<opsa-mef><define-alignment name="A"/></opsa-mef>"#,
        "alignments",
    );
    refuse(
        r#"<opsa-mef><define-extern-library name="L"/></opsa-mef>"#,
        "extern functions",
    );
    // A typed reference must name a declaration of that type. Upstream's
    // `GetEntity` searches one table per type, so `<gate name="b"/>` naming a
    // basic event is an undefined gate, not a silent success.
    refuse(
        r#"<opsa-mef><define-fault-tree name="F">
             <define-gate name="g"><or><gate name="b"/><gate name="b"/></or></define-gate>
             <define-basic-event name="b"><float value="0.1"/></define-basic-event>
           </define-fault-tree></opsa-mef>"#,
        "undefined gate",
    );
    // A private name is NOT reachable from outside by its bare name.
    refuse(
        r#"<opsa-mef><define-fault-tree name="F">
             <define-gate name="top"><or><gate name="inner"/><gate name="inner"/></or></define-gate>
             <define-component name="c" role="private">
               <define-gate name="inner"><or><basic-event name="x"/><basic-event name="x"/></or></define-gate>
             </define-component>
             <define-basic-event name="x"><float value="0.1"/></define-basic-event>
           </define-fault-tree></opsa-mef>"#,
        "undefined gate",
    );
    // MEF allows only two roles.
    refuse(
        r#"<opsa-mef><define-fault-tree name="F">
             <define-component name="c" role="protected"/>
           </define-fault-tree></opsa-mef>"#,
        "public",
    );
    // A connective cannot be an argument: MEF formulas do not nest.
    refuse(
        r#"<opsa-mef><define-fault-tree name="F">
             <define-gate name="top"><or><and><basic-event name="x"/></and></or></define-gate>
             <define-basic-event name="x"><float value="0.1"/></define-basic-event>
           </define-fault-tree></opsa-mef>"#,
        "formulas do not nest",
    );
}

/// **Methodology.** The two non-coherent models, read from their own XML and
/// priced against `oracle-noncoherent.txt`.
///
/// `Aralia/das9601` is the largest model in the fixture — 288 gates and 122
/// basic events as read here, 108 of them reached by SCRAM's analysis. Its
/// cut sets are out of MOCUS's reach (`scram_noncoherent::
/// das9601_is_beyond_this_algorithm_and_says_so` records why), so this checks
/// what *is* in reach and is not checked anywhere else: that the reader gets
/// the model, and that every probability it evaluates matches the one SCRAM
/// printed.
///
/// **Results** (2026-09-22): both models read; all 111 priced events agree.
#[test]
fn the_non_coherent_models_read_and_price_as_scram_does() {
    let mut checked = 0;
    for o in load_oracles("oracle-noncoherent.txt") {
        let model =
            MefModel::from_file(&model_path(&o.name)).unwrap_or_else(|e| panic!("{}: {e}", o.name));
        let tree = model
            .fault_tree(o.top.as_ref().unwrap())
            .unwrap_or_else(|e| panic!("{}: {e}", o.name));
        for (i, name) in tree.basic_event_names().iter().enumerate() {
            let Some(&theirs) = o.events.get(name) else {
                continue;
            };
            assert!(
                agrees(tree.probabilities()[i], theirs, 5e-6),
                "{} {name}: evaluated {}, SCRAM reported {theirs}",
                o.name,
                tree.probabilities()[i]
            );
            checked += 1;
        }
        println!(
            "{:<34} {} gates, {} basic events read",
            o.name,
            model.gates.len(),
            model.basic_events.len()
        );
    }
    println!("{checked} priced events");
    assert!(checked >= 100, "only checked {checked}");
}
