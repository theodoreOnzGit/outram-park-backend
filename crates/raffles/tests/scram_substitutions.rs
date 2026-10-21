//! # Substitutions — delete terms, recovery rules and exchange events
//!
//! A fault tree says how a system fails. A substitution says something the
//! tree cannot: that two events are mutually exclusive, that a recovery action
//! applies when a particular combination occurs, or that one event stands in
//! for another under some condition. Upstream ships **two** models for them,
//! and both are checked here.
//!
//! ## The two kinds are applied in two different layers, and that is the point
//!
//! | kind | upstream | here |
//! |---|---|---|
//! | **declarative** (no `<source>`) | `Pdag::ConstructSubstitution` — a graph rewrite | [`MefModel::fault_tree`], automatically |
//! | **non-declarative** (with `<source>`) | `Zbdd::ApplySubstitutions` — a pass over products | [`apply_to_products`], run by the caller |
//!
//! A declarative substitution is a logical implication, `hypothesis ->
//! target`, and conjoining it with the root makes the tree **non-coherent** —
//! which shows up immediately in `TwoTrain/substitutions`' negative importance
//! factors.
//!
//! ## Upstream refuses an exact analysis of a non-declarative model
//!
//! ```text
//! scram::mef::ValidityError
//! Non-declarative substitutions do not apply to exact analyses.
//! ```
//!
//! The rewritten product list is no longer the minimal cut sets of any Boolean
//! function, so inclusion-exclusion over it means nothing. That refusal is
//! **measured** — the generator runs the binary and records that it produced
//! no exact total — and [`the_non_declarative_model_has_no_exact_total`]
//! asserts the fixture still says so.
//!
//! | | |
//! |---|---|
//! | Upstream | <https://github.com/rakhimov/scram> @ `b85b7894` (2019-07-03) |
//! | Oracle | `reference-data/scram/oracle-substitutions.txt` |
//! | Generator | `reference-data/scram/extract_substitution_oracle.sh` |

use raffles::scram::bdd::Bdd;
use raffles::scram::importance::{importance_factors, importance_factors_from_bdd};
use raffles::scram::mef::MefModel;
use raffles::scram::mocus::{minimal_cut_sets, DEFAULT_LIMIT_ORDER};
use raffles::scram::probability::{top_event_probability, Approximation, EXACT_CUT_SET_LIMIT};
use raffles::scram::substitution::{apply_to_products, SubstitutionType};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/scram")
}

fn model_path(model: &str) -> PathBuf {
    data_dir().join(format!("upstream-input/{model}.xml"))
}

struct Oracle {
    name: String,
    top: String,
    events: HashMap<String, f64>,
    products: Vec<BTreeSet<String>>,
    totals: HashMap<String, f64>,
    importance: HashMap<String, (usize, f64, f64, f64, f64, f64)>,
}

fn load() -> Vec<Oracle> {
    let text =
        std::fs::read_to_string(data_dir().join("oracle-substitutions.txt")).expect("fixture");
    let mut out: Vec<Oracle> = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        let Some(&tag) = f.first() else { continue };
        match tag {
            "MODEL" => out.push(Oracle {
                name: f[1].to_string(),
                top: String::new(),
                events: HashMap::new(),
                products: Vec::new(),
                totals: HashMap::new(),
                importance: HashMap::new(),
            }),
            "TOPNAME" => out.last_mut().unwrap().top = f[1].to_string(),
            "EVENT" => {
                let o = out.last_mut().unwrap();
                o.events.insert(f[1].to_string(), f[2].parse().unwrap());
            }
            "PRODUCT" => {
                let o = out.last_mut().unwrap();
                o.products
                    .push(f[2..].iter().map(|s| s.to_string()).collect());
            }
            "TOTAL" => {
                let o = out.last_mut().unwrap();
                o.totals.insert(f[1].to_string(), f[2].parse().unwrap());
            }
            "IMPORTANCE" => {
                let o = out.last_mut().unwrap();
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
            _ => {}
        }
    }
    assert_eq!(out.len(), 2, "both of upstream's substitution models");
    out
}

fn agrees(ours: f64, theirs: f64, tol: f64) -> bool {
    let scale = theirs.abs().max(1.0);
    (ours - theirs).abs() / scale < tol
}

/// **Methodology.** For both models, read the XML, build the tree (which
/// applies the declarative substitutions), generate the cut sets, apply the
/// non-declarative ones with [`apply_to_products`], and compare the resulting
/// products against the ones SCRAM reported.
///
/// This is the whole of the substitution machinery in one comparison: a
/// mistake in the implication rewrite, in the product pass, or in the
/// re-minimisation after it changes which products come out.
///
/// **Results** (2026-09-22), both exact set equality:
///
/// | model | products |
/// |---|---|
/// | `TwoTrain/substitutions` | 3 — `{PumpOne, ValveTwo}`, `{PumpTwo, ValveOne}`, `{ColdBackupPump, HotBackupPump, PumpOne, PumpTwo}` |
/// | `TwoTrain/nondeclarative_substitutions` | 4 — `{HotBackupPump}`, `{PumpOne, ValveTwo}`, `{PumpTwo, ValveOne}`, `{ValveOne, ValveThree}` |
///
/// The second is worth reading closely. The unsubstituted tree gives
/// `{V1,V2} {V1,P2} {P1,V2} {P1,P2}`; the exchange rule turns `{V1,V2}` into
/// `{V1,V3}`, and the recovery rule collapses `{P1,P2}` to `{HotBackupPump}` —
/// an **order-1** product where there were two order-2 ones.
#[test]
fn products_after_substitution_match_scrams() {
    let mut checked = 0;
    for oracle in load() {
        let model = MefModel::from_file(&model_path(&oracle.name))
            .unwrap_or_else(|e| panic!("{}: {e}", oracle.name));
        let tree = model.fault_tree(&oracle.top).unwrap();
        let names = tree.basic_event_names();

        let cut_sets = minimal_cut_sets(tree.tree(), DEFAULT_LIMIT_ORDER).unwrap();
        let rules = model.product_substitutions(&tree).unwrap();
        let cut_sets = apply_to_products(&cut_sets, &rules).unwrap();

        let ours: BTreeSet<BTreeSet<String>> = cut_sets
            .iter()
            .map(|c| c.members().iter().map(|&m| names[m].clone()).collect())
            .collect();
        let theirs: BTreeSet<BTreeSet<String>> = oracle.products.iter().cloned().collect();
        println!("{:<44} {} products", oracle.name, theirs.len());
        assert_eq!(
            ours,
            theirs,
            "{}: SCRAM found {} products, this port {}",
            oracle.name,
            theirs.len(),
            ours.len()
        );

        // The probabilities the substituted tree carries, including the
        // events that reach it only through a substitution.
        for (i, name) in names.iter().enumerate() {
            if let Some(&theirs) = oracle.events.get(name) {
                assert!(
                    agrees(tree.probabilities()[i], theirs, 5e-6),
                    "{} {name}: {} vs {theirs}",
                    oracle.name,
                    tree.probabilities()[i]
                );
            }
        }
        checked += 1;
    }
    assert_eq!(checked, 2);
}

/// **Methodology.** The declarative model's totals.
///
/// Conjoining an implication with the root makes the tree **non-coherent**, so
/// its exact probability is the BDD's and the cut sets are conservative — the
/// same distinction `scram_noncoherent` documents. SCRAM's unflagged total is
/// therefore compared against this port's BDD, and the two approximations
/// against the cut sets, which is what both codes compute them from.
///
/// **Results** (2026-09-22):
///
/// | quantity | RAFFLES | SCRAM | |
/// |---|---|---|---|
/// | exact (BDD) | 0.3291750 | 0.329175 | agree |
/// | rare-event | 1 | 1 | agree — both clamp |
/// | MCUB | 0.7079258 | 0.707926 | agree |
/// | "exact" over cut sets | 0.6546750 | — | conservative, as it must be |
#[test]
fn the_declarative_model_totals_match_scrams() {
    let oracle = load()
        .into_iter()
        .find(|o| o.name == "TwoTrain/substitutions")
        .unwrap();
    let model = MefModel::from_file(&model_path(&oracle.name)).unwrap();
    let tree = model.fault_tree(&oracle.top).unwrap();
    assert!(
        !tree.tree().is_coherent(),
        "a declarative substitution conjoins an implication, which carries complements"
    );
    let cut_sets = minimal_cut_sets(tree.tree(), DEFAULT_LIMIT_ORDER).unwrap();

    let exact = Bdd::build(tree.tree())
        .and_then(|b| b.probability(tree.probabilities()))
        .expect("BDD");
    println!("exact (BDD) {exact:.7}  scram {}", oracle.totals["exact"]);
    assert!(
        agrees(exact, oracle.totals["exact"], 5e-6),
        "exact: ours {exact}, SCRAM {}",
        oracle.totals["exact"]
    );

    for (mode, approximation) in [
        ("rare-event", Approximation::RareEvent),
        ("mcub", Approximation::Mcub),
    ] {
        let ours = top_event_probability(&cut_sets, tree.probabilities(), approximation).unwrap();
        let theirs = oracle.totals[mode];
        println!("{mode:<11} {ours:.7}  scram {theirs:.7}");
        assert!(agrees(ours, theirs, 5e-6), "{mode}: {ours} vs {theirs}");
    }

    // The conservatism, stated rather than hidden.
    let conservative =
        top_event_probability(&cut_sets, tree.probabilities(), Approximation::Exact).unwrap();
    assert!(
        conservative > exact,
        "cut-set quantification of a non-coherent tree must bound the truth from above; \
         got {conservative} against the BDD's {exact}"
    );
    assert!(cut_sets.len() <= EXACT_CUT_SET_LIMIT);
}

/// **Methodology — a second instance of upstream's MIF sign defect, and the
/// hand calculation that settles which side is right.**
///
/// `scram_bdd_oracle` records 108 importance factors that agree with SCRAM in
/// **magnitude** and differ in **sign**, all of them `Aralia/das9601` and all
/// of it, and offers as a candidate cause that
/// `ProbabilityAnalyzer<Bdd>::CalculateTotalProbability` ends with
///
/// ```cpp
/// if (bdd_graph_->root().complement) prob = 1 - prob;
/// ```
///
/// while `ImportanceAnalyzer<Bdd>::CalculateMif` reads the same root and
/// applies no corresponding negation — though `P(top) = 1 - P(f)` implies
/// `dP(top)/dx = -dP(f)/dx`. That note ends "this has **not been established
/// either way**", because a 108-event model is not something a disagreement
/// can be diagnosed from.
///
/// `TwoTrain/substitutions` is. It has **six** basic events, every one of them
/// inverted (ratio exactly `-1.000000`), and its top-event probability agrees
/// with SCRAM's to the last digit — so the diagram is not in question, only
/// the sign. At that size the Birnbaum factor can be computed **by hand**, and
/// two of them were, in opposite directions:
///
/// ```text
/// PumpTwo = 1:  (V1|P1) & ~(V1&V2) & (~P1|HB) & (~HB|CB)
///               P1=1 -> HB=1 -> CB=1:  0.7 x 0.7 x 0.9 x 0.75 = 0.33075
///               P1=0 -> V1=1, V2=0:    0.3 x 0.5 x 0.5 x 0.93 = 0.06975
///                                                       total = 0.40050
/// PumpTwo = 0:  P1 & ~V1 & V2 & (~HB|CB)
///                             0.7 x 0.5 x 0.5 x 0.93       = 0.16275
/// MIF(PumpTwo) = 0.40050 - 0.16275 = +0.23775     SCRAM: -0.23775
///
/// ValveOne = 1: V2=0, P2=1 (0.35) x P(~P1|HB & ~HB|CB) (0.72) = 0.25200
/// ValveOne = 0: P1 x [P2=1: 0.441 + P2=0: 0.1395]            = 0.40635
/// MIF(ValveOne) = 0.25200 - 0.40635 = -0.15435     SCRAM: +0.15435
/// ```
///
/// Both derivations are the definition `P(top | e) - P(top | not e)`, and both
/// land on this port's sign. **The defect is upstream's**, the pattern is a
/// uniform global inversion, and the candidate cause now has a second model
/// and an independent calculation behind it.
///
/// What is still **not** established: that upstream's BDD root is in fact
/// complemented on these two models. Showing that needs SCRAM instrumented,
/// not read. The evidence is the pattern a root complement predicts —
/// magnitudes untouched, every sign flipped, the total probability unaffected
/// because `CalculateTotalProbability` does apply the negation — observed now
/// on two unrelated models and nowhere else.
///
/// **Results** (2026-09-22): all six magnitudes agree to `5e-6` relative and
/// all six signs are opposite; CIF, DIF, RAW and RRW, which are derived from
/// MIF, carry the same inversion.
#[test]
fn the_declarative_model_importance_is_upstreams_magnitudes_with_the_signs_flipped() {
    let oracle = load()
        .into_iter()
        .find(|o| o.name == "TwoTrain/substitutions")
        .unwrap();
    let model = MefModel::from_file(&model_path(&oracle.name)).unwrap();
    let tree = model.fault_tree(&oracle.top).unwrap();
    let cut_sets = minimal_cut_sets(tree.tree(), DEFAULT_LIMIT_ORDER).unwrap();
    let diagram = Bdd::build(tree.tree()).expect("BDD");

    // The diagram itself is not in question.
    assert!(
        agrees(
            diagram.probability(tree.probabilities()).unwrap(),
            oracle.totals["exact"],
            5e-6
        ),
        "the top-event probability must agree before a sign claim means anything"
    );

    let mut inverted = 0;
    for (name, &(occ, mif, _cif, _dif, _raw, _rrw)) in &oracle.importance {
        let i = tree.basic_event_index(name).expect("in the tree");
        let ours = importance_factors_from_bdd(i, &diagram, tree.probabilities()).unwrap();
        // The occurrence count is a property of the cut sets, which the BDD
        // route never builds, so it comes from the cut-set route.
        let occurrence =
            importance_factors(i, &cut_sets, tree.probabilities(), Approximation::Exact)
                .unwrap()
                .occurrence;
        assert_eq!(occurrence, occ, "{name}: occurrence");
        println!("{name:<16} ours {:+.6}  scram {mif:+.6}", ours.mif);
        assert!(
            agrees(ours.mif.abs(), mif.abs(), 5e-6),
            "{name}: magnitudes differ, so this is NOT the sign defect -- {} vs {mif}",
            ours.mif
        );
        assert_ne!(
            ours.mif.is_sign_negative(),
            mif.is_sign_negative(),
            "{name}: the signs agree, so the inversion is no longer global and the \
             explanation above needs revisiting"
        );
        inverted += 1;
    }
    println!("{inverted} factors, every one inverted");
    assert_eq!(inverted, 6);
}

/// **Methodology — a refusal of upstream's, asserted where it can be seen.**
///
/// `scram --probability` on `TwoTrain/nondeclarative_substitutions.xml`
/// fails with "Non-declarative substitutions do not apply to exact analyses",
/// so the fixture carries **no** exact total for that model, only rare-event
/// and MCUB. That absence is evidence, and a fixture regenerated against a
/// SCRAM that had changed its mind would gain one — so it is asserted rather
/// than left as a gap in a table.
///
/// **Results** (2026-09-22): the record has `rare-event` and `mcub` and no
/// `exact`; both agree with this port to `5e-6` relative. Rare-event is `1`
/// on both sides, which is the clamp — SCRAM even prints
/// `warning="Probability may have been adjusted to 1."`.
#[test]
fn the_non_declarative_model_has_no_exact_total() {
    let oracle = load()
        .into_iter()
        .find(|o| o.name == "TwoTrain/nondeclarative_substitutions")
        .unwrap();
    assert!(
        !oracle.totals.contains_key("exact"),
        "SCRAM refuses an exact analysis here; a fixture with one means upstream, or the \
         generator, has changed"
    );
    assert!(oracle.totals.contains_key("rare-event") && oracle.totals.contains_key("mcub"));

    let model = MefModel::from_file(&model_path(&oracle.name)).unwrap();
    let tree = model.fault_tree(&oracle.top).unwrap();
    let cut_sets = minimal_cut_sets(tree.tree(), DEFAULT_LIMIT_ORDER).unwrap();
    let rules = model.product_substitutions(&tree).unwrap();
    let cut_sets = apply_to_products(&cut_sets, &rules).unwrap();

    for (mode, approximation) in [
        ("rare-event", Approximation::RareEvent),
        ("mcub", Approximation::Mcub),
    ] {
        let ours = top_event_probability(&cut_sets, tree.probabilities(), approximation).unwrap();
        let theirs = oracle.totals[mode];
        println!("{mode:<11} {ours:.7}  scram {theirs:.7}");
        assert!(agrees(ours, theirs, 5e-6), "{mode}: {ours} vs {theirs}");
    }
}

/// **Methodology.** `Substitution::type()` — upstream's inference of which
/// "traditional" type a substitution amounts to — against the `type` attribute
/// the models themselves declare.
///
/// The attribute is optional and purely descriptive; nothing in the analysis
/// branches on it. That makes it a free oracle: where a model declares one,
/// the inference must agree, and the two arrive by completely different
/// routes.
///
/// **Results** (2026-09-22): four of the five declared types are reproduced.
/// The fifth substitution, `ColdBackupInitiation`, declares **no** type and
/// the inference returns none for it — its hypothesis is a bare event
/// reference, a `null` connective, and upstream's `type()` recognises a
/// declarative recovery rule only under `and`. Upstream agrees with itself
/// here; the model simply describes something the three traditional names do
/// not cover.
#[test]
fn the_inferred_substitution_type_matches_the_declared_one() {
    let mut agreed = 0;
    let mut untyped = 0;
    for name in [
        "TwoTrain/substitutions",
        "TwoTrain/nondeclarative_substitutions",
    ] {
        let model = MefModel::from_file(&model_path(name)).unwrap();
        for substitution in &model.substitutions {
            let inferred = substitution.inferred_type();
            println!(
                "{:<44} {:<22} declared {:?} inferred {:?}",
                name, substitution.name, substitution.declared_type, inferred
            );
            match substitution.declared_type {
                Some(declared) => {
                    assert_eq!(
                        Some(declared),
                        inferred,
                        "{}: the declared type and the inferred one disagree",
                        substitution.name
                    );
                    agreed += 1;
                }
                None => {
                    assert_eq!(
                        inferred, None,
                        "`{}` declares no type; upstream's inference returns none for it \
                         too, because its hypothesis is a bare reference rather than an \
                         `and`",
                        substitution.name
                    );
                    untyped += 1;
                }
            }
        }
    }
    assert_eq!((agreed, untyped), (4, 1));
    // And the three names round-trip.
    for t in [
        SubstitutionType::DeleteTerms,
        SubstitutionType::RecoveryRule,
        SubstitutionType::ExchangeEvent,
    ] {
        assert_eq!(SubstitutionType::parse(t.as_str()).unwrap(), t);
    }
    assert!(SubstitutionType::parse("delete-term").is_err());
}

/// **Methodology.** Upstream's `Substitution::Validate`, case by case.
///
/// Each of these is a distinct arm of it, and the declarative and
/// non-declarative halves forbid different things: a declarative substitution
/// with a `true` target has no effect, a non-declarative one with a `false`
/// target has an irrelevant source set, and the two allow different
/// connectives — `atleast` is coherent enough for a declarative hypothesis and
/// not for a non-declarative one.
///
/// **Results** (2026-09-22): all six refusals hold.
#[test]
fn the_malformed_are_refused_with_upstreams_reasons() {
    let refuse = |xml: &str, expected: &str| {
        let element = raffles::scram::mef::parse_xml(xml).expect("well-formed XML");
        let err = match MefModel::from_element(&element) {
            Err(e) => e.to_string(),
            Ok(m) => m
                .fault_tree("top")
                .expect_err("must be refused")
                .to_string(),
        };
        assert!(
            err.contains(expected),
            "expected a refusal naming `{expected}`, got: {err}"
        );
    };
    let model = |substitution: &str| {
        format!(
            r#"<opsa-mef><define-fault-tree name="F">
                 <define-gate name="top"><and><basic-event name="a"/><basic-event name="b"/></and></define-gate>
                 <define-basic-event name="a"><float value="0.1"/></define-basic-event>
                 <define-basic-event name="b"><float value="0.2"/></define-basic-event>
                 <define-basic-event name="c"><float value="0.3"/></define-basic-event>
               </define-fault-tree>
               {substitution}</opsa-mef>"#
        )
    };

    // A complemented hypothesis argument.
    refuse(
        &model(
            r#"<define-substitution name="S">
                 <hypothesis><and><basic-event name="a"/><not><basic-event name="b"/></not></and></hypothesis>
                 <target><constant value="false"/></target>
               </define-substitution>"#,
        ),
        "must be coherent",
    );
    // A declarative substitution whose target is `true` does nothing.
    refuse(
        &model(
            r#"<define-substitution name="S">
                 <hypothesis><and><basic-event name="a"/><basic-event name="b"/></and></hypothesis>
                 <target><constant value="true"/></target>
               </define-substitution>"#,
        ),
        "has no effect",
    );
    // A non-declarative substitution whose target is `false` has an
    // irrelevant source set.
    refuse(
        &model(
            r#"<define-substitution name="S">
                 <hypothesis><and><basic-event name="a"/><basic-event name="b"/></and></hypothesis>
                 <source><basic-event name="a"/></source>
                 <target><constant value="false"/></target>
               </define-substitution>"#,
        ),
        "source set is irrelevant",
    );
    // A non-coherent connective in a declarative hypothesis.
    refuse(
        &model(
            r#"<define-substitution name="S">
                 <hypothesis><xor><basic-event name="a"/><basic-event name="b"/></xor></hypothesis>
                 <target><constant value="false"/></target>
               </define-substitution>"#,
        ),
        "must be coherent",
    );
    // `atleast` is allowed for a declarative hypothesis but NOT for a
    // non-declarative one.
    refuse(
        &model(
            r#"<define-substitution name="S">
                 <hypothesis><atleast min="2"><basic-event name="a"/><basic-event name="b"/></atleast></hypothesis>
                 <source><basic-event name="a"/></source>
                 <target><basic-event name="c"/></target>
               </define-substitution>"#,
        ),
        "only allow AND/OR/NULL",
    );
    // A repeated source event.
    refuse(
        &model(
            r#"<define-substitution name="S">
                 <hypothesis><and><basic-event name="a"/><basic-event name="b"/></and></hypothesis>
                 <source><basic-event name="a"/><basic-event name="a"/></source>
                 <target><basic-event name="c"/></target>
               </define-substitution>"#,
        ),
        "twice as a source",
    );
}
