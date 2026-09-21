//! # Prime implicants, verified against `scram --prime-implicants`
//!
//! A **minimal cut set** says which components failing is enough. A **prime
//! implicant** also says which must be *working*, so it carries complemented
//! literals and describes the tree's function exactly rather than
//! conservatively. On a coherent tree the two coincide; on a non-coherent one
//! they do not, and the gap is the conservatism every other cut-set result in
//! this crate carries.
//!
//! [`raffles::scram::zbdd::prime_implicants`] ports upstream's
//! `Zbdd::ConvertBddPrimeImplicants` together with `Bdd::Consensus`. This file
//! checks it against `scram --prime-implicants` on the same models.
//!
//! | | |
//! |---|---|
//! | Upstream | <https://github.com/rakhimov/scram> @ `b85b7894` (2019-07-03) |
//! | Oracle command | `scram --probability --importance --prime-implicants <input>` |
//! | Fixture | `reference-data/scram/oracle-prime-implicants.txt` |
//!
//! **`Aralia/das9601` is absent, and not because of this port.** Upstream
//! itself does not finish `--prime-implicants` on it within five minutes,
//! where its minimal cut sets take under a second. The consensus term adds a
//! third recursive call at every node, and that is what it costs.

use raffles::scram::bdd::Bdd;
use raffles::scram::fault_tree::{Connective, FaultTreeBuilder, FaultTreeModel};
use raffles::scram::zbdd::{self, prime_implicants};
use std::collections::{BTreeSet, HashMap};

/// A prime implicant as SCRAM printed it: `+name` and `-name`.
type NamedImplicant = (BTreeSet<String>, BTreeSet<String>);

struct Oracle {
    events: HashMap<String, f64>,
    implicants: BTreeSet<NamedImplicant>,
    exact: f64,
}

struct ModelSpec {
    /// House events the model declares: a condition fixed for the analysis,
    /// `true` or `false`, with no probability. Gate arguments name them with
    /// an `h:` prefix.
    houses: Vec<(String, bool)>,
    gates: Vec<(String, String, Option<usize>, Vec<String>)>,
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
    let mut name = String::new();
    let mut events = HashMap::new();
    let mut implicants: BTreeSet<NamedImplicant> = BTreeSet::new();
    let mut exact = None;
    for line in fixture("oracle-prime-implicants.txt").lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.is_empty() {
            continue;
        }
        match f[0] {
            "MODEL" => {
                name = f[1].to_string();
                events = HashMap::new();
                implicants = BTreeSet::new();
                exact = None;
            }
            "EVENT" => {
                events.insert(f[1].to_string(), f[2].parse().unwrap());
            }
            "PI" => {
                let mut pos = BTreeSet::new();
                let mut neg = BTreeSet::new();
                for lit in &f[2..] {
                    let (sign, ev) = lit.split_at(1);
                    match sign {
                        "+" => pos.insert(ev.to_string()),
                        "-" => neg.insert(ev.to_string()),
                        _ => panic!("unsigned literal `{lit}`"),
                    };
                }
                implicants.insert((pos, neg));
            }
            "TOTAL" if f[1] == "exact" => exact = Some(f[2].parse().unwrap()),
            "END" => out.push((
                std::mem::take(&mut name),
                Oracle {
                    events: std::mem::take(&mut events),
                    implicants: std::mem::take(&mut implicants),
                    exact: exact.expect("an exact total"),
                },
            )),
            _ => {}
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

/// Builds the tree, preferring the report's probabilities and falling back to
/// the model's declared ones.
///
/// Prime implicants need every probability for the same reason the BDD does —
/// a complemented literal prices a component that appears in no cut set. See
/// `scram_bdd_oracle`'s `build` for the measured consequence of getting this
/// wrong.
fn build(name: &str, spec: &ModelSpec, oracle: &Oracle) -> Option<FaultTreeModel> {
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
    for n in &event_names {
        let p = match (oracle.events.get(*n), spec.params.get(*n)) {
            (Some(&reported), _) => reported,
            (None, Some(&declared)) => declared,
            (None, None) => return None,
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
    Some(b.build(top).unwrap_or_else(|e| panic!("{name}: {e}")))
}

fn named(model: &FaultTreeModel, pis: &[zbdd::PrimeImplicant]) -> BTreeSet<NamedImplicant> {
    pis.iter()
        .map(|pi| {
            let name = |i: &usize| model.basic_event_names()[*i].clone();
            (
                pi.positive().iter().map(name).collect(),
                pi.negative().iter().map(name).collect(),
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// **Methodology.** Every model in the prime-implicant fixture: generate the
/// implicants here and compare them, **with their signs**, against those
/// SCRAM reported under `--prime-implicants`. Compared as a set of
/// (positive, negative) name-set pairs, so a sign error cannot hide.
///
/// **Result** (2026-09-21): printed below; all 9 models agree, 441 implicants
/// in total. The one that carries the signs is
/// `models-for-this-port/noncoherent_small` — `{+a,-b}`, `{+c,-d}`, `{-c,+d}`
/// — and it is the only model in the fixture with any negative literal at all,
/// since the other eight are coherent.
#[test]
fn prime_implicants_match_scram() {
    let oracles = load_oracles();
    let specs = load_models();
    let mut checked = 0;
    let mut total = 0;
    let mut signed = 0;

    for (name, oracle) in &oracles {
        let Some(spec) = specs.get(name) else {
            continue;
        };
        let Some(model) = build(name, spec, oracle) else {
            println!("skipped {name}");
            continue;
        };
        let ours = prime_implicants(model.tree()).unwrap_or_else(|e| panic!("{name}: {e}"));
        let ours_named = named(&model, &ours);
        let with_negatives = ours.iter().filter(|pi| !pi.negative().is_empty()).count();

        println!(
            "{name:<40} ours {:>4} prime implicants, scram {:>4}   ({with_negatives} carry a complement)",
            ours_named.len(),
            oracle.implicants.len()
        );
        let missing: Vec<_> = oracle.implicants.difference(&ours_named).collect();
        let spurious: Vec<_> = ours_named.difference(&oracle.implicants).collect();
        assert!(
            missing.is_empty() && spurious.is_empty(),
            "{name}: missing {missing:?}, spurious {spurious:?}"
        );
        checked += 1;
        total += ours_named.len();
        signed += with_negatives;
    }
    println!("checked {checked} models, {total} prime implicants, {signed} carrying a complement");
    assert!(checked >= 9, "only {checked} models");
    assert!(signed >= 3, "no complemented literal was exercised");
}

/// **Methodology.** On a **coherent** tree a prime implicant cannot contain a
/// complemented literal — a component working can never help cause failure —
/// so the prime implicants must be exactly the minimal cut sets.
///
/// This is an invariant rather than an oracle comparison, and it checks the
/// two code paths against each other: `zbdd::prime_implicants` runs the
/// consensus recursion, `zbdd::minimal_cut_sets` does not.
///
/// **Result** (2026-09-21): holds on all 8 coherent models in the fixture, 438
/// implicants, not one carrying a negative literal.
#[test]
fn on_a_coherent_tree_prime_implicants_are_the_minimal_cut_sets() {
    let oracles = load_oracles();
    let specs = load_models();
    let mut checked = 0;

    for (name, oracle) in &oracles {
        let Some(spec) = specs.get(name) else {
            continue;
        };
        let Some(model) = build(name, spec, oracle) else {
            continue;
        };
        if !model.tree().is_coherent() {
            println!("{name:<40} non-coherent, skipped");
            continue;
        }
        let pis = prime_implicants(model.tree()).unwrap();
        assert!(
            pis.iter().all(|pi| pi.negative().is_empty()),
            "{name}: a coherent tree produced a complemented literal"
        );
        let as_sets: BTreeSet<Vec<usize>> = pis.iter().map(|pi| pi.positive().to_vec()).collect();
        let cut_sets: BTreeSet<Vec<usize>> = zbdd::minimal_cut_sets(model.tree(), None)
            .unwrap()
            .iter()
            .map(|c| c.members().to_vec())
            .collect();
        println!(
            "{name:<40} {} implicants == {} cut sets",
            as_sets.len(),
            cut_sets.len()
        );
        assert_eq!(as_sets, cut_sets, "{name}");
        checked += 1;
    }
    println!("checked {checked} coherent models");
    assert!(checked >= 8, "only {checked}");
}

/// **Methodology.** Prime implicants describe the function *exactly*, so
/// summing their probabilities must come no lower than the true value — and
/// on a non-coherent tree it must come lower than the cut sets' rare-event
/// sum, since each implicant is the cut set narrowed by its complemented
/// literals.
///
/// **Result** (2026-09-21) on `noncoherent_small`, whose true probability is
/// `0.5032`:
///
/// | | sum of products |
/// |---|---|
/// | prime implicants `{+a,-b} {+c,-d} {-c,+d}` | 0.08 + 0.18 + 0.28 = **0.540000** |
/// | minimal cut sets `{a} {c} {d}` | 0.1 + 0.3 + 0.4 = **0.800000** |
///
/// Both bound `0.5032` from above — the rare-event approximation always does —
/// but the implicants are far tighter, which is the practical reason to want
/// them. Each implicant's own probability is checked against the value SCRAM
/// printed on the corresponding `<product>`.
#[test]
fn prime_implicants_are_tighter_than_cut_sets_on_a_non_coherent_tree() {
    let oracles = load_oracles();
    let specs = load_models();
    let name = "models-for-this-port/noncoherent_small";
    let oracle = &oracles
        .iter()
        .find(|(n, _)| n == name)
        .expect("in fixture")
        .1;
    let model = build(name, &specs[name], oracle).expect("readable");
    let p = model.probabilities();

    let pis = prime_implicants(model.tree()).unwrap();
    let pi_sum: f64 = pis.iter().map(|pi| pi.probability(p).unwrap()).sum();

    // SCRAM printed these on the corresponding <product> elements.
    let mut by_probability: Vec<f64> = pis.iter().map(|pi| pi.probability(p).unwrap()).collect();
    by_probability.sort_by(|a, b| a.partial_cmp(b).unwrap());
    for (ours, theirs) in by_probability.iter().zip([0.08, 0.18, 0.28]) {
        println!("implicant probability ours {ours:.6}  scram {theirs:.6}");
        assert!((ours - theirs).abs() < 5e-7, "ours {ours}, SCRAM {theirs}");
    }

    let cut_sets = zbdd::minimal_cut_sets(model.tree(), None).unwrap();
    let cs_sum: f64 = cut_sets
        .iter()
        .map(|c| raffles::scram::cut_set_probability(c, p).unwrap())
        .sum();
    let truth = Bdd::build(model.tree()).unwrap().probability(p).unwrap();

    println!("prime implicants sum {pi_sum:.6}, cut sets sum {cs_sum:.6}, truth {truth:.6}");
    assert!((pi_sum - 0.54).abs() < 1e-12, "implicant sum {pi_sum}");
    assert!((cs_sum - 0.80).abs() < 1e-12, "cut-set sum {cs_sum}");
    assert!((truth - oracle.exact).abs() < 5e-6);
    assert!(pi_sum > truth, "an upper bound must be above the truth");
    assert!(
        pi_sum < cs_sum,
        "prime implicants must bound more tightly than cut sets: {pi_sum} vs {cs_sum}"
    );
}
