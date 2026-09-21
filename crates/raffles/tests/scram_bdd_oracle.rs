//! # Exact top-event probability: this port's BDD against SCRAM's
//!
//! Every other exact-probability check in this crate goes through
//! inclusion-exclusion over the minimal cut sets, which is `2^n` in their
//! number and refuses past 20. That left the strongest quantification result —
//! BDD versus inclusion-exclusion agreeing — resting on models of at most 12
//! cut sets, and left `Aralia/chinese` (392 products) and `Aralia/das9601`
//! (4,259) unchecked exactly at all.
//!
//! This file closes that. [`raffles::scram::bdd`] evaluates the Boolean
//! function directly, so it has no cut-set ceiling, and it is compared against
//! the value SCRAM's own BDD produced for the same model.
//!
//! ## Why this is still a cross-check and not a re-run
//!
//! Both sides are BDDs, so the *representation* agrees by construction. What
//! does not is everything around it: SCRAM builds its diagram from a `Pdag`
//! that a 2,411-line preprocessor has rewritten, with modules, complement
//! edges and its own variable ordering; this port builds a plain Bryant
//! diagram straight from the tree, with explicit terminals and a
//! first-appearance order. Agreement to upstream's reported precision across
//! 11 models says the two arrive at the same function by different routes.
//!
//! It is also the **only** check here that reaches the non-coherent exact
//! value, which cut sets cannot express at all.
//!
//! | | |
//! |---|---|
//! | Upstream | <https://github.com/rakhimov/scram> @ `b85b7894` (2019-07-03) |
//! | Version | SCRAM 0.16.2 |
//! | Oracle command | `scram --probability <input>` (no flag: the exact BDD path) |
//!
//! Full record: `crates/raffles/docs/scram-port-verification.md`.

use raffles::scram::bdd::Bdd;
use raffles::scram::fault_tree::{Connective, FaultTreeBuilder, FaultTreeModel};
use raffles::scram::mocus::{minimal_cut_sets, DEFAULT_LIMIT_ORDER};
use raffles::scram::probability::{top_event_probability, Approximation, EXACT_CUT_SET_LIMIT};
use std::collections::{BTreeSet, HashMap};

// ---------------------------------------------------------------------------
// Fixtures — both oracles, since the BDD is the one thing that handles either.
// ---------------------------------------------------------------------------

struct Oracle {
    coherent: bool,
    events: HashMap<String, f64>,
    exact: f64,
    product_count: usize,
}

struct ModelSpec {
    /// House events the model declares: a condition fixed for the analysis,
    /// `true` or `false`, with no probability. Gate arguments name them with
    /// an `h:` prefix.
    houses: Vec<(String, bool)>,
    gates: Vec<(String, String, Option<usize>, Vec<String>)>,
    /// Probabilities the *model* declares, for basic events given a direct
    /// `<float>`. See [`build`] for why the BDD needs these and the cut-set
    /// tests do not.
    params: HashMap<String, f64>,
    top: Option<String>,
    unparsed: Vec<String>,
}

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/scram")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

fn load_oracles() -> Vec<(String, Oracle)> {
    let mut out = Vec::new();
    for (file, coherent) in [("oracle.txt", true), ("oracle-noncoherent.txt", false)] {
        let mut name = String::new();
        let mut events = HashMap::new();
        let mut exact = None;
        let mut products = 0usize;
        for line in fixture(file).lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.is_empty() {
                continue;
            }
            match f[0] {
                "MODEL" => {
                    name = f[1].to_string();
                    events = HashMap::new();
                    exact = None;
                    products = 0;
                }
                "EVENT" => {
                    events.insert(f[1].to_string(), f[2].parse().unwrap());
                }
                "PRODUCT" => products += 1,
                "TOTAL" if f[1] == "exact" => exact = Some(f[2].parse().unwrap()),
                "END" => out.push((
                    std::mem::take(&mut name),
                    Oracle {
                        coherent,
                        events: std::mem::take(&mut events),
                        exact: exact.expect("every model records an exact total"),
                        product_count: products,
                    },
                )),
                _ => {}
            }
        }
    }
    out
}

/// The products SCRAM reported, by model, as sets of names.
fn load_products() -> HashMap<String, BTreeSet<BTreeSet<String>>> {
    let mut out = HashMap::new();
    for file in ["oracle.txt", "oracle-noncoherent.txt"] {
        let mut name = String::new();
        let mut sets: BTreeSet<BTreeSet<String>> = BTreeSet::new();
        for line in fixture(file).lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.is_empty() {
                continue;
            }
            match f[0] {
                "MODEL" => {
                    name = f[1].to_string();
                    sets = BTreeSet::new();
                }
                "PRODUCT" => {
                    sets.insert(f[2..].iter().map(|s| s.to_string()).collect());
                }
                "END" => {
                    out.insert(std::mem::take(&mut name), std::mem::take(&mut sets));
                }
                _ => {}
            }
        }
    }
    out
}

fn load_models() -> HashMap<String, ModelSpec> {
    let mut out = HashMap::new();
    let mut name = String::new();
    let mut cur: Option<ModelSpec> = None;
    for line in fixture("models.txt").lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.is_empty() {
            continue;
        }
        match f[0] {
            "MODEL" => {
                name = f[1].to_string();
                cur = Some(ModelSpec {
                    houses: Vec::new(),
                    gates: Vec::new(),
                    params: HashMap::new(),
                    top: None,
                    unparsed: Vec::new(),
                });
            }
            "HOUSE" => {
                if let Some(m) = cur.as_mut() {
                    m.houses.push((f[1].to_string(), f[2] == "true"));
                }
            }
            "GATE" => {
                if let Some(m) = cur.as_mut() {
                    let min = (f[3] != "-").then(|| f[3].parse().expect("a number"));
                    m.gates.push((
                        f[1].to_string(),
                        f[2].to_string(),
                        min,
                        f[4..].iter().map(|s| s.to_string()).collect(),
                    ));
                }
            }
            "PARAM" => {
                if let Some(m) = cur.as_mut() {
                    m.params.insert(f[1].to_string(), f[2].parse().unwrap());
                }
            }
            "TOP" => {
                if let Some(m) = cur.as_mut() {
                    m.top = Some(f[1].to_string())
                }
            }
            "CANNOT-PARSE" => {
                if let Some(m) = cur.as_mut() {
                    m.unparsed.push(f[1..].join(" "))
                }
            }
            "END" => {
                if let Some(m) = cur.take() {
                    out.insert(std::mem::take(&mut name), m);
                }
            }
            _ => {}
        }
    }
    out
}

/// Builds the tree, taking probabilities from the oracle where it has them
/// and from the model's own declaration where it does not.
///
/// **This is the one place in the SCRAM tests that needs the model's declared
/// probabilities, and the reason is worth stating.** SCRAM's report prices
/// only the basic events that survive into some product; an event absorbed
/// out of every minimal cut set is simply absent from it. The cut-set tests
/// can shrug at that — such an event is in no cut set, so no arithmetic
/// reaches it, and they assert exactly that. **A BDD cannot**: it evaluates
/// the whole Boolean function, and an event that appears in the function at
/// all changes the answer whether or not it survives into a cut set.
///
/// `Lift`'s `W_1` and `LMD_1_1`, and the small non-coherent model's `b`, are
/// that case. Getting this wrong is not theoretical: the first run of this
/// file substituted the cut-set tests' `0.5` sentinel for `b` and produced
/// `0.487` against SCRAM's `0.5032`, which is precisely the answer for
/// `p(b) = 0.5`.
///
/// Where both sources have a value they are asserted to agree, which
/// incidentally checks that the structure extractor reads the model
/// correctly.
///
/// Returns `None` when the structure extractor refused the model, or when an
/// event has no probability from either source — an expression-defined event
/// outside every product, which this port cannot evaluate and must not guess.
fn build(name: &str, spec: &ModelSpec, oracle: &Oracle) -> Option<(FaultTreeModel, Vec<usize>)> {
    if !spec.unparsed.is_empty() {
        return None;
    }
    let top = spec.top.as_ref()?;

    let gate_names: BTreeSet<&str> = spec.gates.iter().map(|(n, ..)| n.as_str()).collect();
    let mut event_names: Vec<&str> = oracle.events.keys().map(|s| s.as_str()).collect();
    for (_, _, _, args) in &spec.gates {
        for a in args {
            if a.starts_with("h:") {
                continue;
            }
            let n = &a[2..];
            if !gate_names.contains(n)
                && !oracle.events.contains_key(n)
                && !event_names.contains(&n)
            {
                event_names.push(n);
            }
        }
    }
    event_names.sort_unstable();

    let mut b = FaultTreeBuilder::new();
    let mut from_model_only = Vec::new();
    for (i, n) in event_names.iter().enumerate() {
        let p = match (oracle.events.get(*n), spec.params.get(*n)) {
            (Some(&reported), Some(&declared)) => {
                assert!(
                    agrees(reported, declared, 1e-4),
                    "{name} `{n}`: the report says {reported}, the model declares {declared}"
                );
                reported
            }
            (Some(&reported), None) => reported,
            (None, Some(&declared)) => {
                from_model_only.push(i);
                declared
            }
            (None, None) => {
                println!(
                    "{name}: `{n}` has no probability from the report or the model \
                     (expression-defined and outside every product); skipping the model \
                     rather than guessing"
                );
                return None;
            }
        };
        b.basic_event(n, p).expect("valid probability");
    }
    for (house, value) in &spec.houses {
        b.house_event(house, *value)
            .expect("a fresh house-event name");
    }
    for (gate, connective, min, args) in &spec.gates {
        let c = match connective.as_str() {
            "and" => Connective::And,
            "or" => Connective::Or,
            "null" => Connective::Null,
            "not" => Connective::Not,
            "nand" => Connective::Nand,
            "nor" => Connective::Nor,
            "xor" => Connective::Xor,
            "atleast" => Connective::Atleast {
                min: min.expect("atleast carries its min"),
            },
            other => panic!("{name}: unhandled connective `{other}`"),
        };
        let arg_names: Vec<&str> = args.iter().map(|a| &a[2..]).collect();
        b.gate(gate, c, &arg_names)
            .unwrap_or_else(|e| panic!("{name} gate `{gate}`: {e}"));
    }
    Some((
        b.build(top).unwrap_or_else(|e| panic!("{name}: {e}")),
        from_model_only,
    ))
}

fn agrees(ours: f64, theirs: f64, tol: f64) -> bool {
    (ours - theirs).abs() / theirs.abs().max(1.0) < tol
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// **Methodology.** Build the BDD of every model whose structure the extractor
/// can read — coherent and non-coherent alike — and compare its exact
/// top-event probability against the value SCRAM's own BDD reported for the
/// same model, at `5e-6` relative (upstream's 6-significant-figure report).
///
/// The diagram's node count is printed alongside, because that is the number
/// that says whether the variable ordering is working, and a regression in it
/// would otherwise be invisible.
///
/// **Result** (2026-09-21): printed per model by the run below; every model
/// agrees. The two that matter most are the ones inclusion-exclusion cannot
/// reach at all — `Aralia/chinese` at 392 cut sets and `Aralia/das9601` at
/// 4,259 — and the non-coherent `noncoherent_small`, whose exact value cut
/// sets cannot express even in principle.
#[test]
fn exact_probability_matches_scrams_bdd_on_every_model() {
    let oracles = load_oracles();
    let specs = load_models();
    let mut checked = 0;
    let mut skipped = Vec::new();

    for (name, oracle) in &oracles {
        let Some(spec) = specs.get(name) else {
            skipped.push(format!("{name}: no structure entry"));
            continue;
        };
        let Some((model, _)) = build(name, spec, oracle) else {
            skipped.push(format!("{name}: {}", spec.unparsed.join("; ")));
            continue;
        };

        let bdd = Bdd::build(model.tree()).unwrap_or_else(|e| panic!("{name}: {e}"));
        let ours = bdd
            .probability(model.probabilities())
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        println!(
            "{name:<40} {:<14} ours {ours:.9}  scram {:.9}   ({:>6} nodes, {} products)",
            if oracle.coherent {
                "coherent"
            } else {
                "NON-coherent"
            },
            oracle.exact,
            bdd.node_count(),
            oracle.product_count,
        );
        assert!(
            agrees(ours, oracle.exact, 5e-6),
            "{name}: ours {ours}, SCRAM {}",
            oracle.exact
        );
        checked += 1;
    }

    for s in &skipped {
        println!("skipped {s}");
    }
    println!("checked {checked} models exactly");
    assert!(checked >= 10, "only {checked} models checked");
}

/// **Methodology.** Where inclusion-exclusion over the cut sets *can* run, it
/// and the BDD must give the same number on a **coherent** tree — they are two
/// unrelated algorithms for one quantity, and this crate now has both.
///
/// The cut sets are generated here too, so this closes the loop from input
/// model to exact probability twice over, by routes that share nothing beyond
/// the tree itself.
///
/// **Result** (2026-09-21): agreement to `1e-12` — far tighter than the `5e-6`
/// used against SCRAM, because here both numbers are this port's own and no
/// report precision intervenes. Checked on every coherent model within
/// [`EXACT_CUT_SET_LIMIT`].
#[test]
fn the_bdd_and_inclusion_exclusion_agree_where_both_can_run() {
    let oracles = load_oracles();
    let specs = load_models();
    let mut checked = 0;

    for (name, oracle) in &oracles {
        if !oracle.coherent {
            continue;
        }
        let Some(spec) = specs.get(name) else {
            continue;
        };
        let Some((model, _)) = build(name, spec, oracle) else {
            continue;
        };
        let cut_sets = minimal_cut_sets(model.tree(), DEFAULT_LIMIT_ORDER).unwrap();
        if cut_sets.is_empty() || cut_sets.len() > EXACT_CUT_SET_LIMIT {
            println!("{name:<40} skipped, {} cut sets", cut_sets.len());
            continue;
        }

        let by_cut_sets =
            top_event_probability(&cut_sets, model.probabilities(), Approximation::Exact).unwrap();
        let by_bdd = Bdd::build(model.tree())
            .unwrap()
            .probability(model.probabilities())
            .unwrap();
        println!("{name:<40} cut sets {by_cut_sets:.12}  bdd {by_bdd:.12}");
        assert!(
            (by_cut_sets - by_bdd).abs() < 1e-12,
            "{name}: inclusion-exclusion {by_cut_sets}, BDD {by_bdd}"
        );
        checked += 1;
    }
    println!("checked {checked} models both ways");
    assert!(checked >= 7, "only {checked} models checked both ways");
}

/// **Methodology.** On a **non-coherent** tree the two must *disagree*, in a
/// direction fixed by what each measures: the BDD evaluates the real function,
/// cut-set quantification bounds it from above. Asserting the inequality is
/// what turns a known discrepancy into a check.
///
/// **Result** (2026-09-21), `models-for-this-port/noncoherent_small`:
/// BDD `0.503200` — matching SCRAM's `0.5032` exactly — against `0.622000`
/// from the cut sets, `+23.6 %`. The BDD is the right answer, and this is the
/// first time this crate could produce it.
#[test]
fn the_bdd_gets_the_non_coherent_answer_that_cut_sets_cannot() {
    let oracles = load_oracles();
    let specs = load_models();
    let name = "models-for-this-port/noncoherent_small";
    let oracle = &oracles
        .iter()
        .find(|(n, _)| n == name)
        .expect("the small non-coherent model is in the fixture")
        .1;
    let (model, _) = build(name, &specs[name], oracle).expect("its structure is readable");

    let by_bdd = Bdd::build(model.tree())
        .unwrap()
        .probability(model.probabilities())
        .unwrap();
    let cut_sets = minimal_cut_sets(model.tree(), DEFAULT_LIMIT_ORDER).unwrap();
    let by_cut_sets =
        top_event_probability(&cut_sets, model.probabilities(), Approximation::Exact).unwrap();

    println!(
        "bdd {by_bdd:.6}  scram {:.6}  cut sets {by_cut_sets:.6}",
        oracle.exact
    );
    assert!(
        agrees(by_bdd, oracle.exact, 5e-6),
        "the BDD must reproduce SCRAM's exact value: ours {by_bdd}, SCRAM {}",
        oracle.exact
    );
    assert!(
        by_cut_sets > by_bdd,
        "cut-set quantification must bound the true probability from above: \
         {by_cut_sets} is not above {by_bdd}"
    );
    let excess = 100.0 * (by_cut_sets - by_bdd) / by_bdd;
    println!("cut sets are +{excess:.1} % conservative against the true value");
    assert!(
        (23.0..24.0).contains(&excess),
        "expected +23.6 %, got +{excess:.1} %"
    );
}

/// **Methodology.** Boolean identities the diagram must satisfy, checked on
/// trees small enough to reason about. These would catch a defect that
/// happened to agree with SCRAM on the fixture's particular models.
///
/// **Result** (2026-09-21): all hold.
#[test]
fn the_diagram_satisfies_the_identities_it_must() {
    let build_tree = |f: &dyn Fn(&mut FaultTreeBuilder)| {
        let mut b = FaultTreeBuilder::new();
        for (n, p) in [("A", 0.3), ("B", 0.5), ("C", 0.7)] {
            b.basic_event(n, p).unwrap();
        }
        f(&mut b);
        b.build("Top").unwrap()
    };
    let p_of = |m: &FaultTreeModel| {
        Bdd::build(m.tree())
            .unwrap()
            .probability(m.probabilities())
            .unwrap()
    };

    // A OR NOT A is a tautology; A AND NOT A is unsatisfiable. The diagram
    // must reduce both to a terminal, not merely to something that happens to
    // evaluate to 1 or 0.
    let m = build_tree(&|b| {
        b.gate("NotA", Connective::Not, &["A"]).unwrap();
        b.gate("Top", Connective::Or, &["A", "NotA"]).unwrap();
    });
    assert!(Bdd::build(m.tree()).unwrap().is_always());
    assert!((p_of(&m) - 1.0).abs() < 1e-15);

    let m = build_tree(&|b| {
        b.gate("NotA", Connective::Not, &["A"]).unwrap();
        b.gate("Top", Connective::And, &["A", "NotA"]).unwrap();
    });
    assert!(Bdd::build(m.tree()).unwrap().is_never());
    assert!(p_of(&m).abs() < 1e-15);

    // Independent OR: P = 1 - (1-a)(1-b)(1-c).
    let m = build_tree(&|b| {
        b.gate("Top", Connective::Or, &["A", "B", "C"]).unwrap();
    });
    assert!((p_of(&m) - (1.0 - 0.7 * 0.5 * 0.3)).abs() < 1e-12);

    // Independent AND: P = abc.
    let m = build_tree(&|b| {
        b.gate("Top", Connective::And, &["A", "B", "C"]).unwrap();
    });
    assert!((p_of(&m) - (0.3 * 0.5 * 0.7)).abs() < 1e-12);

    // XOR of two: P = a(1-b) + (1-a)b.
    let m = build_tree(&|b| {
        b.gate("Top", Connective::Xor, &["A", "B"]).unwrap();
    });
    assert!((p_of(&m) - (0.3 * 0.5 + 0.7 * 0.5)).abs() < 1e-12);

    // 2-of-3, by the closed form.
    let m = build_tree(&|b| {
        b.gate("Top", Connective::Atleast { min: 2 }, &["A", "B", "C"])
            .unwrap();
    });
    let (a, bb, c) = (0.3, 0.5, 0.7);
    let want = a * bb * (1.0 - c) + a * (1.0 - bb) * c + (1.0 - a) * bb * c + a * bb * c;
    assert!(
        (p_of(&m) - want).abs() < 1e-12,
        "2-of-3: {} vs {want}",
        p_of(&m)
    );

    // De Morgan: NAND is NOT(AND), NOR is NOT(OR).
    let nand = build_tree(&|b| {
        b.gate("Top", Connective::Nand, &["A", "B", "C"]).unwrap();
    });
    assert!((p_of(&nand) - (1.0 - a * bb * c)).abs() < 1e-12);
    let nor = build_tree(&|b| {
        b.gate("Top", Connective::Nor, &["A", "B", "C"]).unwrap();
    });
    assert!((p_of(&nor) - ((1.0 - a) * (1.0 - bb) * (1.0 - c))).abs() < 1e-12);

    // A probability of 0 or 1 on a variable must collapse the answer, which is
    // the cheapest check that the Shannon expansion is the right way round.
    let m = build_tree(&|b| {
        b.gate("Top", Connective::And, &["A", "B"]).unwrap();
    });
    let bdd = Bdd::build(m.tree()).unwrap();
    assert!(bdd.probability(&[0.0, 1.0, 0.5]).unwrap().abs() < 1e-15);
    assert!((bdd.probability(&[1.0, 1.0, 0.5]).unwrap() - 1.0).abs() < 1e-15);
}

/// **Methodology.** The same minimal cut sets, by two unrelated algorithms and
/// against SCRAM's products: [`raffles::scram::mocus`] expands top-down and
/// absorbs, [`raffles::scram::zbdd`] converts the BDD and subsumes. Compared
/// as sets of sets, by name.
///
/// This is the check that says the ZBDD path is not merely *fast* but
/// *right* — and it is also what lets `das9601`'s cut sets be verified at all,
/// since `mocus` cannot produce them.
///
/// **Result** (2026-09-21): printed below. Every model on which both run
/// agrees with the other and with SCRAM; on `Aralia/das9601`, where `mocus`
/// cannot, the ZBDD reproduces all **4,259** of SCRAM's products.
#[test]
fn zbdd_cut_sets_match_scram_and_mocus() {
    use raffles::scram::zbdd;

    let oracles = load_oracles();
    let specs = load_models();
    let products = load_products();
    let mut checked = 0;
    let mut total = 0usize;

    for (name, oracle) in &oracles {
        let Some(spec) = specs.get(name) else {
            continue;
        };
        let Some((model, _)) = build(name, spec, oracle) else {
            continue;
        };
        let theirs = &products[name];

        let bdd = Bdd::build(model.tree()).unwrap();
        let ours = zbdd::from_bdd(&bdd, None).unwrap_or_else(|e| panic!("{name}: {e}"));
        let ours_named: BTreeSet<BTreeSet<String>> = ours
            .iter()
            .map(|c| {
                c.members()
                    .iter()
                    .map(|&i| model.basic_event_names()[i].clone())
                    .collect()
            })
            .collect();

        // The count must come off the diagram without materialising, too.
        let counted = zbdd::count_minimal_cut_sets(model.tree()).unwrap();
        assert_eq!(
            counted as usize,
            ours_named.len(),
            "{name}: counted {counted}, listed {}",
            ours_named.len()
        );

        // `mocus` is attempted only where top-down expansion is known to
        // terminate. Product count is the cheap proxy: above a thousand, the
        // expansion blows its five-million-state ceiling and costs ~29 s to
        // say so, which this test should not pay for on every run. That
        // failure has its own reproduction in
        // `scram_noncoherent::das9601_is_beyond_this_algorithm_and_says_so`.
        let mocus_note = if theirs.len() > 1_000 {
            "mocus not attempted, see scram_noncoherent"
        } else {
            let sets = minimal_cut_sets(model.tree(), DEFAULT_LIMIT_ORDER)
                .unwrap_or_else(|e| panic!("{name}: mocus was expected to manage this: {e}"));
            let named: BTreeSet<BTreeSet<String>> = sets
                .iter()
                .map(|c| {
                    c.members()
                        .iter()
                        .map(|&i| model.basic_event_names()[i].clone())
                        .collect()
                })
                .collect();
            assert_eq!(named, ours_named, "{name}: mocus and zbdd disagree");
            "mocus agrees"
        };

        println!(
            "{name:<40} zbdd {:>5} cut sets, scram {:>5}   ({mocus_note})",
            ours_named.len(),
            theirs.len()
        );
        assert_eq!(
            ours_named, *theirs,
            "{name}: ZBDD cut sets differ from SCRAM's"
        );
        checked += 1;
        total += ours_named.len();
    }
    println!("checked {checked} models, {total} cut sets");
    assert!(checked >= 10, "only {checked} models");
    assert!(total >= 4700, "only {total} cut sets");
}
