//! # Common-cause failure groups, against SCRAM's own `--ccf` run
//!
//! A fault tree that treats two pumps as independent understates the risk: one
//! flood, one maintenance error or one bad batch fails both. A CCF group says
//! so, and applying it **rewrites the tree** — each member becomes a proxy
//! gate over the shared-failure events it belongs to — rather than adjusting a
//! number. The shared failure then appears as a *single* event in the cut
//! sets, which is why `TwoTrain/common_cause` goes from four order-2 products
//! to two order-1 and four order-2 ones, and its probability from `0.0361` to
//! `0.0622587`.
//!
//! ## Applied by default, ablated explicitly
//!
//! Upstream applies CCF groups only under `scram --ccf`. **This port applies
//! them by default**, following the workspace rule that physics the data
//! supplies is applied unless a caller explicitly ablates it: a model that
//! declares a CCF group has said its components are coupled, and a run that
//! quietly ignores that reports a risk the model has already said is wrong.
//! The ablation is [`MefModel::without_ccf`], and **both paths are checked
//! here against the corresponding SCRAM run** — the applied one against
//! `oracle-ccf.txt` (`--ccf`), the ablated one against `oracle.txt`'s
//! independent analysis.
//!
//! ## Three of the four models have no upstream model to check against
//!
//! Upstream's input suite uses `beta-factor` and nothing else.
//! `models-for-this-port/ccf_models.xml` uses all four — beta-factor,
//! MGL, alpha-factor and phi-factor — at group size 3 for the three
//! multi-level ones, so the `1/C(n-1, i)` combination reciprocal is exercised
//! at a value other than 1.
//!
//! | | |
//! |---|---|
//! | Upstream | <https://github.com/rakhimov/scram> @ `b85b7894` (2019-07-03) |
//! | Oracle | `reference-data/scram/oracle-ccf.txt`, from `scram --probability --importance --ccf` |
//! | Generator | `reference-data/scram/extract_ccf_oracle.sh` |

use raffles::scram::ccf::CcfModel;
use raffles::scram::fault_tree::FaultTreeModel;
use raffles::scram::importance::importance_factors;
use raffles::scram::mef::MefModel;
use raffles::scram::mocus::{minimal_cut_sets, DEFAULT_LIMIT_ORDER};
use raffles::scram::probability::{top_event_probability, Approximation, EXACT_CUT_SET_LIMIT};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/scram")
}

fn model_path(model: &str) -> PathBuf {
    if model.starts_with("models-for-this-port/") {
        data_dir().join(format!("{model}.xml"))
    } else {
        data_dir().join(format!("upstream-input/{model}.xml"))
    }
}

/// One model's oracle.
struct Oracle {
    name: String,
    top: String,
    events: HashMap<String, f64>,
    products: Vec<BTreeSet<String>>,
    totals: HashMap<String, f64>,
    importance: HashMap<String, (usize, f64, f64, f64, f64, f64)>,
}

fn load(fixture: &str) -> Vec<Oracle> {
    let text = std::fs::read_to_string(data_dir().join(fixture)).expect("fixture");
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
    assert!(!out.is_empty(), "{fixture} is empty");
    out
}

/// The fixture's spelling of a CCF event's id.
///
/// Upstream's `CcfEvent::MakeName` joins the member names with a **space**
/// inside square brackets, and this port uses that name verbatim. The fixture
/// records are whitespace-separated fields, so `extract_ccf_oracle.sh` writes
/// the same name with a comma instead. The difference is the fixture
/// format's, not the name's, and this is the one place it is bridged.
fn as_fixture_name(id: &str) -> String {
    id.replace(' ', ",")
}

fn named_cut_sets(model: &FaultTreeModel) -> Vec<BTreeSet<String>> {
    let names = model.basic_event_names();
    minimal_cut_sets(model.tree(), DEFAULT_LIMIT_ORDER)
        .expect("cut sets")
        .iter()
        .map(|c| {
            c.members()
                .iter()
                .map(|&m| as_fixture_name(&names[m]))
                .collect()
        })
        .collect()
}

fn agrees(ours: f64, theirs: f64, tol: f64) -> bool {
    let scale = theirs.abs().max(1.0);
    (ours - theirs).abs() / scale < tol
}

/// **Methodology.** For every model with CCF groups, read it, let
/// [`MefModel::fault_tree`] apply the groups (the default), and compare the
/// **generated CCF events and their probabilities** against the ones SCRAM
/// reported under `--ccf`.
///
/// This is the check that the four `CalculateProbabilities` formulas are
/// right. They differ in shape, not only in constants, so a mistake in one
/// does not look like a mistake in another.
///
/// **Results** (2026-09-22): 30 CCF events across the two models, every
/// probability agreeing to `5e-6` relative. Worked instances:
///
/// | group | model | level | this crate | SCRAM |
/// |---|---|---|---|---|
/// | `Valves` | beta-factor | 1 | `(1-0.2) x 0.1 = 0.08` | 0.08 |
/// | `Valves` | beta-factor | 2 | `0.2 x 0.1 = 0.02` | 0.02 |
/// | `Mgl` | MGL | 2 | 0.0015 | 0.0015 |
/// | `Mgl` | MGL | 3 | 0.002 | 0.002 |
/// | `Alpha` | alpha-factor | 1 | 0.039823 | 0.039823 |
/// | `Alpha` | alpha-factor | 3 | 0.0039823 | 0.0039823 |
/// | `Phi` | phi-factor | 2 | 0.0075 | 0.0075 |
#[test]
fn the_generated_ccf_events_match_scrams() {
    let mut checked = 0;
    for oracle in load("oracle-ccf.txt") {
        let model = MefModel::from_file(&model_path(&oracle.name))
            .unwrap_or_else(|e| panic!("{}: {e}", oracle.name));
        assert!(
            !model.ccf_groups.is_empty(),
            "{}: the model declares CCF groups",
            oracle.name
        );
        let tree = model.fault_tree(&oracle.top).unwrap();
        for (i, name) in tree.basic_event_names().iter().enumerate() {
            let key = as_fixture_name(name);
            let theirs = *oracle
                .events
                .get(&key)
                .unwrap_or_else(|| panic!("{}: SCRAM has no `{key}`", oracle.name));
            let ours = tree.probabilities()[i];
            println!(
                "{:<34} {key:<22} ours {ours:.9}  scram {theirs:.9}",
                oracle.name
            );
            assert!(
                agrees(ours, theirs, 5e-6),
                "{} {key}: ours {ours}, SCRAM {theirs}",
                oracle.name
            );
            checked += 1;
        }
        assert_eq!(
            tree.basic_event_names().len(),
            oracle.events.len(),
            "{}: this port generated {} events, SCRAM {}",
            oracle.name,
            tree.basic_event_names().len(),
            oracle.events.len()
        );
    }
    println!("{checked} CCF events checked");
    assert!(checked >= 30, "only checked {checked}");
}

/// **Methodology.** The rewritten tree's cut sets, totals and importance
/// factors against SCRAM's `--ccf` run.
///
/// Getting the CCF *probabilities* right is not enough: the proxy-gate rewrite
/// has to put each event under every member it couples, and a mistake there
/// changes which products come out rather than what they are worth.
///
/// **Results** (2026-09-22): both models' products match set-for-set — 6 for
/// `TwoTrain/common_cause`, 26 for `ccf_models` — with all three totals and
/// every importance factor.
#[test]
fn the_rewritten_tree_matches_scrams() {
    let mut products = 0;
    let mut factors = 0;
    for oracle in load("oracle-ccf.txt") {
        let model = MefModel::from_file(&model_path(&oracle.name)).unwrap();
        let tree = model.fault_tree(&oracle.top).unwrap();

        let ours: BTreeSet<BTreeSet<String>> = named_cut_sets(&tree).into_iter().collect();
        let theirs: BTreeSet<BTreeSet<String>> = oracle.products.iter().cloned().collect();
        assert_eq!(
            ours,
            theirs,
            "{}: SCRAM found {} products, this port {}",
            oracle.name,
            theirs.len(),
            ours.len()
        );
        products += theirs.len();

        let cut_sets = minimal_cut_sets(tree.tree(), DEFAULT_LIMIT_ORDER).unwrap();
        for (mode, approximation) in [
            ("exact", Approximation::Exact),
            ("rare-event", Approximation::RareEvent),
            ("mcub", Approximation::Mcub),
        ] {
            if approximation == Approximation::Exact && cut_sets.len() > EXACT_CUT_SET_LIMIT {
                continue;
            }
            let ours =
                top_event_probability(&cut_sets, tree.probabilities(), approximation).unwrap();
            let theirs = oracle.totals[mode];
            println!(
                "{:<34} {mode:<11} ours {ours:.9}  scram {theirs:.9}",
                oracle.name
            );
            assert!(
                agrees(ours, theirs, 5e-6),
                "{} [{mode}]: ours {ours}, SCRAM {theirs}",
                oracle.name
            );
        }

        if cut_sets.len() <= EXACT_CUT_SET_LIMIT {
            for (name, &(occ, mif, cif, dif, raw, rrw)) in &oracle.importance {
                let Some(i) = tree
                    .basic_event_names()
                    .iter()
                    .position(|n| as_fixture_name(n) == *name)
                else {
                    continue;
                };
                let ours =
                    importance_factors(i, &cut_sets, tree.probabilities(), Approximation::Exact)
                        .unwrap();
                assert_eq!(ours.occurrence, occ, "{} {name}: occurrence", oracle.name);
                for (label, ours, theirs) in [
                    ("MIF", ours.mif, mif),
                    ("CIF", ours.cif, cif),
                    ("DIF", ours.dif, dif),
                    ("RAW", ours.raw, raw),
                    ("RRW", ours.rrw, rrw),
                ] {
                    assert!(
                        agrees(ours, theirs, 5e-6),
                        "{} {name}: {label} {ours} vs {theirs}",
                        oracle.name
                    );
                }
                factors += 1;
            }
        }
    }
    println!("{products} products, {factors} importance events");
    assert!(products >= 30 && factors >= 6);
}

/// **Methodology — the default, pinned, and the ablation, checked against the
/// run it claims to reproduce.**
///
/// The workspace rule is that a default nothing asserts will drift back off
/// silently. So this asserts both halves on `TwoTrain/common_cause`:
///
/// 1. [`MefModel::fault_tree`] with no argument applies the groups, giving
///    SCRAM's `--ccf` answer;
/// 2. [`MefModel::without_ccf`] gives SCRAM's answer **without** `--ccf`,
///    which is a different and strictly more optimistic question — and the
///    members keep the group's distribution as their own independent
///    probability, which is what upstream's `AddDistribution` assigns them.
///
/// **Results** (2026-09-22):
///
/// | | products | orders | probability |
/// |---|---|---|---|
/// | CCF applied (default) | 6 | 2 of order 1, 4 of order 2 | **0.0622587** |
/// | CCF ablated | 4 | all order 2 | **0.0361** |
///
/// Both match SCRAM's corresponding run exactly. The gap is `+72 %`, which is
/// the point: ignoring a declared CCF group is not a small approximation.
#[test]
fn ccf_is_applied_by_default_and_the_ablation_is_the_independent_analysis() {
    let name = "TwoTrain/common_cause";
    let model = MefModel::from_file(&model_path(name)).unwrap();

    // 1. The default.
    let applied = model.fault_tree("TopEvent").unwrap();
    let with = minimal_cut_sets(applied.tree(), DEFAULT_LIMIT_ORDER).unwrap();
    let p_with =
        top_event_probability(&with, applied.probabilities(), Approximation::Exact).unwrap();
    assert_eq!(with.len(), 6, "SCRAM --ccf reports 6 products");
    assert_eq!(
        with.iter().filter(|c| c.members().len() == 1).count(),
        2,
        "two of them are order 1 -- the whole-group failures"
    );
    assert!(
        agrees(p_with, 0.0622587, 5e-6),
        "with CCF: {p_with}, SCRAM 0.0622587"
    );
    assert!(
        applied
            .basic_event_names()
            .iter()
            .any(|n| n == "[ValveOne ValveTwo]"),
        "the generated events carry upstream's own bracketed name: {:?}",
        applied.basic_event_names()
    );

    // 2. The ablation.
    let ablated = model.without_ccf().fault_tree("TopEvent").unwrap();
    let without = minimal_cut_sets(ablated.tree(), DEFAULT_LIMIT_ORDER).unwrap();
    let p_without =
        top_event_probability(&without, ablated.probabilities(), Approximation::Exact).unwrap();
    assert_eq!(without.len(), 4, "SCRAM without --ccf reports 4 products");
    assert!(
        without.iter().all(|c| c.members().len() == 2),
        "all order 2 -- nothing fails together"
    );
    assert!(
        agrees(p_without, 0.0361, 5e-6),
        "without CCF: {p_without}, SCRAM 0.0361"
    );
    for (i, name) in ablated.basic_event_names().iter().enumerate() {
        assert!(
            agrees(ablated.probabilities()[i], 0.1, 1e-12),
            "{name}: the ablation leaves the group's distribution as the member's own \
             probability, which is what upstream's AddDistribution assigns"
        );
    }

    let gap = 100.0 * (p_with - p_without) / p_without;
    println!("CCF applied {p_with:.7}, ablated {p_without:.7}, +{gap:.1} %");
    assert!(
        gap > 70.0,
        "ignoring a declared CCF group is not a small approximation; measured +{gap:.1} %"
    );
}

/// **Methodology.** The four models' factor bookkeeping, which differs per
/// model and is where an off-by-one lives.
///
/// Upstream indexes factors from `min_level()`, which is the group's own size
/// for beta-factor, 2 for MGL and 1 for the other two, and deduces a missing
/// `level` attribute as one past the previous. A level below the minimum, a
/// level above the member count, a repeated level and a gap are each refused.
///
/// **Results** (2026-09-22): the four models read with the levels upstream
/// gives them, and all five malformed cases are refused.
#[test]
fn factor_levels_are_upstreams_and_the_malformed_are_refused() {
    let model = MefModel::from_file(&model_path("models-for-this-port/ccf_models")).unwrap();
    let by_name: HashMap<&str, &raffles::scram::ccf::CcfGroup> = model
        .ccf_groups
        .iter()
        .map(|g| (g.name.as_str(), g))
        .collect();

    let beta = by_name["Beta"];
    assert_eq!(beta.model, CcfModel::BetaFactor);
    assert_eq!(
        beta.factors.iter().map(|(l, _)| *l).collect::<Vec<_>>(),
        [2]
    );
    assert_eq!(
        CcfModel::BetaFactor.min_level(2),
        2,
        "the beta model's minimum level IS the group size"
    );

    assert_eq!(
        by_name["Mgl"]
            .factors
            .iter()
            .map(|(l, _)| *l)
            .collect::<Vec<_>>(),
        [2, 3],
        "MGL factors start at level 2"
    );
    assert_eq!(
        by_name["Alpha"]
            .factors
            .iter()
            .map(|(l, _)| *l)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(by_name["Phi"].model, CcfModel::PhiFactor);

    let refuse = |xml: &str, expected: &str| {
        let element = raffles::scram::mef::parse_xml(xml).expect("well-formed XML");
        let model = raffles::scram::mef::MefModel::from_element(&element);
        let err = match model {
            Err(e) => e.to_string(),
            // Some defects only surface when the group is applied, which is
            // where upstream validates too.
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
    // The gate references the same members the group declares, which is how
    // MEF models are written: the `<members>` element is the declaration.
    let group = |model: &str, members: &str, factors: &str| {
        format!(
            r#"<opsa-mef><define-fault-tree name="F">
                 <define-gate name="top"><and>{members}</and></define-gate>
               </define-fault-tree>
               <define-CCF-group name="G" model="{model}">
                 <members>{members}</members>
                 <distribution><float value="0.1"/></distribution>
                 {factors}
               </define-CCF-group></opsa-mef>"#
        )
    };

    // A level below the model's minimum.
    refuse(
        &group(
            "MGL",
            r#"<basic-event name="a"/><basic-event name="b"/>"#,
            r#"<factor level="1"><float value="0.1"/></factor>"#,
        ),
        "less than the minimum level",
    );
    // A level above the member count.
    refuse(
        &group(
            "alpha-factor",
            r#"<basic-event name="a"/><basic-event name="b"/>"#,
            r#"<factor level="3"><float value="0.1"/></factor>"#,
        ),
        "more than the number of members",
    );
    // A repeated level.
    refuse(
        &group(
            "alpha-factor",
            r#"<basic-event name="a"/><basic-event name="b"/>"#,
            r#"<factors><factor level="1"><float value="0.5"/></factor>
               <factor level="1"><float value="0.5"/></factor></factors>"#,
        ),
        "redefinition",
    );
    // A factor set that stops short. Upstream refuses this at expression
    // CONSTRUCTION rather than in `CcfGroup::Validate`: with one factor the
    // alpha model's weighted sum is an `Add` of a single argument, and
    // `EnsureMultivariateArgs` throws "Expression requires 2 or more
    // arguments." The compiled binary was run on exactly this input to find
    // that out rather than reading it off the asserts.
    refuse(
        &group(
            "alpha-factor",
            r#"<basic-event name="a"/><basic-event name="b"/>"#,
            r#"<factors><factor level="1"><float value="0.5"/></factor></factors>"#,
        ),
        "requires 2 or more arguments",
    );
    // Phi factors that do not sum to 1.
    refuse(
        &group(
            "phi-factor",
            r#"<basic-event name="a"/><basic-event name="b"/>"#,
            r#"<factors><factor level="1"><float value="0.5"/></factor>
               <factor level="2"><float value="0.2"/></factor></factors>"#,
        ),
        "must sum to 1",
    );
    // A model name outside the grammar.
    refuse(
        &group(
            "gamma-factor",
            r#"<basic-event name="a"/><basic-event name="b"/>"#,
            r#"<factor level="2"><float value="0.2"/></factor>"#,
        ),
        "is not a CCF model",
    );
}
