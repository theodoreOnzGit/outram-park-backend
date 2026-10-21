//! # Alignments — the same model in several operating configurations
//!
//! A plant is not in one state all year. `TwoTrain/two_train_alignment` says
//! it runs normally 99.452 % of the time, with `PumpOne` out for maintenance
//! 0.274 % of it and `PumpTwo` for another 0.274 %, and the risk is different
//! in each. There is **no single answer** for such a model: upstream analyses
//! it once per phase and tags each result set with the alignment and phase
//! names, and so does this.
//!
//! ## What a phase changes, and why both have to be checked
//!
//! 1. The **mission time** becomes `time_fraction x mission_time`. The
//!    `Normal` phase runs over 8711.9952 h rather than 8760, and the
//!    maintenance phases over 24.0024 h — a 365-fold difference that every
//!    `<exponential>` in the model sees.
//! 2. The phase's `<set-house-event>` instructions flip house events, which
//!    prunes whole branches. `PumpOneMaintenance` going true removes
//!    `PumpOne` from the tree entirely, and the phase drops from four products
//!    to two.
//!
//! Checking only the probability would not separate the two: this model's
//! basic events are literals, so the mission-time change moves nothing in it.
//! The **product sets** are what show the pruning, and they are compared
//! set-for-set.
//!
//! ## The model also has CCF groups, so both of this port's paths are checked
//!
//! `two_train_alignment.xml` declares two `beta-factor` groups as well as
//! three phases. This port applies CCF groups **by default** where upstream
//! needs `--ccf`, so the fixture carries **both** runs — six records — and
//! each of this port's two paths is compared against the upstream run that
//! answers the same question.
//!
//! | | |
//! |---|---|
//! | Upstream | <https://github.com/rakhimov/scram> @ `b85b7894` (2019-07-03) |
//! | Oracle | `reference-data/scram/oracle-alignments.txt` |
//! | Generator | `reference-data/scram/extract_alignment_oracle.sh`, run with and without `--ccf` |

use raffles::scram::mef::MefModel;
use raffles::scram::mocus::{minimal_cut_sets, DEFAULT_LIMIT_ORDER};
use raffles::scram::probability::{top_event_probability, Approximation};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/scram")
}

/// One (alignment, phase, CCF setting) record.
struct Oracle {
    model: String,
    ccf: bool,
    alignment: String,
    phase: String,
    top: String,
    products: Vec<BTreeSet<String>>,
    exact: f64,
}

fn load() -> Vec<Oracle> {
    let text = std::fs::read_to_string(data_dir().join("oracle-alignments.txt")).expect("fixture");
    let mut out: Vec<Oracle> = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        let Some(&tag) = f.first() else { continue };
        match tag {
            "MODEL" => out.push(Oracle {
                model: f[1].to_string(),
                ccf: false,
                alignment: String::new(),
                phase: String::new(),
                top: String::new(),
                products: Vec::new(),
                exact: f64::NAN,
            }),
            "CCF" => out.last_mut().unwrap().ccf = f[1] == "on",
            "ALIGNMENT" => out.last_mut().unwrap().alignment = f[1].to_string(),
            "PHASE" => out.last_mut().unwrap().phase = f[1].to_string(),
            "TOPNAME" => out.last_mut().unwrap().top = f[1].to_string(),
            "TOTAL" => out.last_mut().unwrap().exact = f[2].parse().unwrap(),
            "PRODUCT" => {
                let o = out.last_mut().unwrap();
                o.products
                    .push(f[2..].iter().map(|s| s.to_string()).collect());
            }
            _ => {}
        }
    }
    assert_eq!(out.len(), 6, "three phases, with and without CCF");
    out
}

/// The fixture's spelling of a CCF event's id — see `scram_ccf`.
fn as_fixture_name(id: &str) -> String {
    id.replace(' ', ",")
}

fn agrees(ours: f64, theirs: f64, tol: f64) -> bool {
    let scale = theirs.abs().max(1.0);
    (ours - theirs).abs() / scale < tol
}

/// **Methodology.** For each of the six records, put the model into that phase
/// with [`MefModel::in_phase`], take the CCF path the record was generated
/// with, generate the cut sets and compare the products **set-for-set** and
/// the exact probability against SCRAM's.
///
/// **Results** (2026-09-22):
///
/// | phase | CCF | products | probability | |
/// |---|---|---|---|---|
/// | `Normal` | off | 4 | 0.0361 | agree |
/// | `PumpOne` | off | 2 | 0.019 | agree |
/// | `PumpTwo` | off | 2 | 0.019 | agree |
/// | `Normal` | on | 6 | 0.0622587 | agree |
/// | `PumpOne` | on | 4 | 0.0333694 | agree |
/// | `PumpTwo` | on | 4 | 0.0333694 | agree |
///
/// The `Normal` row is `two_train`'s own answer, which is the check that a
/// phase with no instructions changes nothing it should not; the two
/// maintenance rows are half the products, which is the check that it changes
/// what it should.
#[test]
fn every_phase_matches_scrams() {
    let mut checked = 0;
    for oracle in load() {
        let model =
            MefModel::from_file(&data_dir().join(format!("upstream-input/{}.xml", oracle.model)))
                .unwrap_or_else(|e| panic!("{}: {e}", oracle.model));
        let phased = model.in_phase(&oracle.alignment, &oracle.phase).unwrap();
        let phased = if oracle.ccf {
            phased
        } else {
            phased.without_ccf()
        };
        let tree = phased.fault_tree(&oracle.top).unwrap();
        let names = tree.basic_event_names();
        let cut_sets = minimal_cut_sets(tree.tree(), DEFAULT_LIMIT_ORDER).unwrap();

        let ours: BTreeSet<BTreeSet<String>> = cut_sets
            .iter()
            .map(|c| {
                c.members()
                    .iter()
                    .map(|&m| as_fixture_name(&names[m]))
                    .collect()
            })
            .collect();
        let theirs: BTreeSet<BTreeSet<String>> = oracle.products.iter().cloned().collect();
        let p =
            top_event_probability(&cut_sets, tree.probabilities(), Approximation::Exact).unwrap();
        println!(
            "{}/{:<8} ccf {:<3} {} products  p {:.7}  scram {:.7}",
            oracle.alignment,
            oracle.phase,
            if oracle.ccf { "on" } else { "off" },
            theirs.len(),
            p,
            oracle.exact
        );
        assert_eq!(
            ours,
            theirs,
            "{}/{} [ccf {}]: SCRAM found {} products, this port {}",
            oracle.alignment,
            oracle.phase,
            oracle.ccf,
            theirs.len(),
            ours.len()
        );
        assert!(
            agrees(p, oracle.exact, 5e-6),
            "{}/{}: ours {p}, SCRAM {}",
            oracle.alignment,
            oracle.phase,
            oracle.exact
        );
        checked += 1;
    }
    assert_eq!(checked, 6);
}

/// **Methodology.** The two things a phase changes, asserted directly rather
/// than inferred from the answers agreeing.
///
/// This model's basic events are literals, so its **mission time** could be
/// scaled wrongly — or not at all — without moving a single probability. A
/// test that only compared totals would pass either way. So the scaling is
/// checked on the model, and the house-event states with it.
///
/// **Results** (2026-09-22): `Normal` gets `0.99452 x 8760 = 8711.9952 h` and
/// the two maintenance phases `0.00274 x 8760 = 24.0024 h`; each maintenance
/// phase sets exactly its own house event and leaves the other alone; and the
/// fractions sum to 1, which is upstream's `Alignment::Validate`.
#[test]
fn a_phase_scales_the_mission_time_and_sets_its_house_events() {
    let model =
        MefModel::from_file(&data_dir().join("upstream-input/TwoTrain/two_train_alignment.xml"))
            .unwrap();
    assert_eq!(model.mission_time, 8760.0, "the model's own mission time");
    assert_eq!(model.alignments.len(), 1);
    let alignment = &model.alignments[0];
    assert_eq!(alignment.name, "Maintenance");
    alignment.validate().expect("the fractions sum to 1");
    let sum: f64 = alignment.phases.iter().map(|p| p.time_fraction).sum();
    assert!((sum - 1.0).abs() < 1e-9, "fractions sum to {sum}");

    for (phase, fraction, set) in [
        ("Normal", 0.99452, None),
        ("PumpOne", 0.00274, Some("PumpOneMaintenance")),
        ("PumpTwo", 0.00274, Some("PumpTwoMaintenance")),
    ] {
        let phased = model.in_phase("Maintenance", phase).unwrap();
        let expected = 8760.0 * fraction;
        assert!(
            (phased.mission_time - expected).abs() < 1e-9,
            "{phase}: mission time {} is not {expected}",
            phased.mission_time
        );
        println!("{phase:<8} mission {:.4} h", phased.mission_time);
        for house in ["PumpOneMaintenance", "PumpTwoMaintenance"] {
            let state = phased.house_events[house];
            assert_eq!(
                state,
                Some(house) == set,
                "{phase}: `{house}` should be {}",
                Some(house) == set
            );
        }
        // The phased model has no alignment left, so it has one answer.
        assert!(phased.alignments.is_empty());
        assert!(phased.phases().is_empty());
    }
    assert_eq!(
        model.phases(),
        vec![
            ("Maintenance", "Normal"),
            ("Maintenance", "PumpOne"),
            ("Maintenance", "PumpTwo")
        ]
    );
}

/// **Methodology.** Upstream's `Phase::Phase` and `Alignment::Validate`, and
/// the one instruction a phase may carry.
///
/// Upstream types a phase's instructions `std::vector<SetHouseEvent*>`, so
/// nothing else in its instruction language can appear there. That is refused
/// by name rather than skipped, since a skipped instruction would give a
/// phase that analyses cleanly and describes a different configuration.
///
/// **Results** (2026-09-22): all five refusals hold.
#[test]
fn the_malformed_are_refused_with_upstreams_reasons() {
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
    let model = |alignment: &str| {
        format!(
            r#"<opsa-mef><define-fault-tree name="F">
                 <define-gate name="top"><or><basic-event name="a"/><house-event name="h"/></or></define-gate>
                 <define-basic-event name="a"><float value="0.1"/></define-basic-event>
                 <define-house-event name="h"><constant value="false"/></define-house-event>
               </define-fault-tree>
               {alignment}</opsa-mef>"#
        )
    };

    // Fractions that do not sum to 1.
    refuse(
        &model(
            r#"<define-alignment name="A">
                 <define-phase name="p1" time-fraction="0.5"/>
                 <define-phase name="p2" time-fraction="0.3"/>
               </define-alignment>"#,
        ),
        "do not sum to 1",
    );
    // A fraction outside (0, 1].
    refuse(
        &model(
            r#"<define-alignment name="A">
                 <define-phase name="p1" time-fraction="0"/>
                 <define-phase name="p2" time-fraction="1"/>
               </define-alignment>"#,
        ),
        "must be in (0, 1]",
    );
    // No phase at all.
    refuse(
        &model(r#"<define-alignment name="A"/>"#),
        "declares no phase",
    );
    // An instruction a phase cannot carry.
    refuse(
        &model(
            r#"<define-alignment name="A">
                 <define-phase name="p1" time-fraction="1">
                   <collect-formula><basic-event name="a"/></collect-formula>
                 </define-phase>
               </define-alignment>"#,
        ),
        "only <set-house-event> instructions",
    );
    // A `<set-house-event>` naming something that is not a house event.
    refuse(
        &model(
            r#"<define-alignment name="A">
                 <define-phase name="p1" time-fraction="1">
                   <set-house-event name="a"><constant value="true"/></set-house-event>
                 </define-phase>
               </define-alignment>"#,
        ),
        "undefined house-event",
    );
}
