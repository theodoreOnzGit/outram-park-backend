//! Every workspace member carries a well-formed code-map tag, and the tags
//! agree with the dependency graph (GitHub #729, #733; maintainer decisions
//! 2026-10-06).
//!
//! Each crate's `Cargo.toml` has `[package.metadata.kovan]` with:
//!
//! - `row`, its degree of integration: 0-1 utilities and standalone crates,
//!   2 domain solvers, 3 coupled multiphysics, 4 integrated GUI apps;
//! - `topic`, the box kovan's code map draws it in;
//! - `fidelity` (maintainer, 2026-10-06): 0 lumped (no spatial
//!   discretisation), 1 one resolved dimension (system-code-like), 2 two
//!   (subchannel), 3 three (full CFD, diffusion), 4 brute force (Monte Carlo,
//!   first-principles data). A resolved non-spatial axis (nuclides, pointwise
//!   energy, time) counts as a dimension. `[lo, hi]` when the crate spans
//!   levels (changi, raffles); absent for utilities and knowledge management.
//!   The map draws a box's crates highest fidelity first;
//! - `maturity`, 0 concept, 1 AI draft, 2 AI V&V, 3 human reviewed, 4 human
//!   V&V (ready): the crate's **lowest** part (maintainer, 2026-10-06: "by
//!   default, we go with lowest level to be fair");
//! - optionally `[[package.metadata.kovan.maturity_modules]]`, the modules
//!   rated higher than the crate, each with `module` (a path from the
//!   library root, `a::b`), `level` and `why`;
//! - optionally `backronym` (GitHub #815), the MRT-station backronym spelled
//!   out as `docs/ecosystem-naming.md` sets it. The code map shows it in the
//!   crate's tooltip and detail panel.
//!
//! What is checked, reading the tags through `cargo metadata` so nothing is
//! copied by hand:
//!
//! - every member has a tag, with a known topic and a row in 0..=4;
//! - row 4 is exactly the `app` topic, and utilities sit in rows 0-1;
//! - no crate sits in a lower row than a required dependency (optional and
//!   dev-dependencies do not count: the row is what the crate needs to
//!   build);
//! - every listed module exists in the crate's source, is rated above the
//!   crate itself, and says why;
//! - a backronym spells the crate's name, by the letters its text marks
//!   (`spells` below). ~~sembawang carries one.~~ **CORRECTED 2026-10-13**:
//!   seventeen crates carry one.
//!
//! Since 2026-10-06 (#734) the tags are read by the same parser the map is
//! drawn from, `kovan::code_map::CodeMap::from_cargo_metadata` (which reports
//! every malformed tag), and the placement rules are
//! `CodeMap::placement_problems`. A second test lays the real workspace out
//! and runs `kovan::code_map::layout::check` on it: every crate placed once,
//! no overlapping cards, fidelity high to low left to right, raffles across
//! the whole Risk box.

use std::path::{Path, PathBuf};

use kovan::code_map::{layout, CodeMap, Topic};

fn workspace() -> CodeMap {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let json = kovan::code_map::run_cargo_metadata(&root, true).expect("cargo metadata runs");
    CodeMap::from_cargo_metadata(&json).unwrap_or_else(|errors| panic!("{}", errors.join("\n")))
}

#[test]
fn every_crate_is_placed_on_the_code_map_consistently_with_its_dependencies() {
    let map = workspace();
    let problems = map.placement_problems();
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    for c in &map.crates {
        if c.maturity_modules.is_empty() {
            continue;
        }
        let base = PathBuf::from(
            c.lib_dir.as_deref().unwrap_or_else(|| panic!("{}: maturity_modules needs a library", c.name)),
        );
        for m in &c.maturity_modules {
            let rel: PathBuf = m.module.split("::").collect();
            let exists = base.join(&rel).with_extension("rs").is_file()
                || base.join(&rel).join("mod.rs").is_file();
            assert!(exists, "{}: module {} not found under {}", c.name, m.module, Path::new(&base).display());
        }
    }
}

#[test]
fn the_real_workspace_lays_out_cleanly() {
    let map = workspace();
    let l = layout::layout(&map);
    let problems = layout::check(&map, &l);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    let risk = l.frames.iter().find(|f| f.topic == Topic::Risk).unwrap().rect;
    let raffles = l.card("raffles").expect("raffles is a member").rect;
    assert!((raffles.x - (risk.x + layout::PAD + layout::ROW_TAG)).abs() < 1e-6 && (raffles.right() - (risk.right() - layout::PAD)).abs() < 1e-6,
        "raffles spans the whole Risk box: {raffles:?} in {risk:?}");
}

/// Whether a backronym spells `name` (letters only, any case).
///
/// Each word gives its first letter, or nothing. It may give further letters
/// only where the text itself marks them: capitals after the first letter
/// ("BOmbardment" gives BO, "INtegrated" gives IN, "RisK" gives RK), or the
/// leading letters of a word written wholly in lower case ("by" gives BY). Words split on spaces and hyphens, so
/// "Multi-Phase" gives M and P. The letters given, in order, must be the name.
///
/// ~~The rule was "the capital initials of the words are the name".~~
/// **CORRECTED 2026-10-13**: that rule fitted sembawang but refused backronyms
/// the maintainer set, BOON LAY (BOmbardment ... and ... Yields), DHOBY GHAUT
/// (... by GUI ...), TAMPINES (Multi-Phase INtegrated) and PASIR RIS (Risk &
/// Reliability, where Reliability gives nothing).
fn spells(backronym: &str, name: &str) -> bool {
    let name: Vec<char> = name.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_uppercase).collect();
    let mut reach = vec![false; name.len() + 1];
    reach[0] = true;
    for word in backronym.split(|c: char| c.is_whitespace() || c == '-') {
        let w: Vec<char> = word.chars().collect();
        let lower = w.iter().all(|c| !c.is_uppercase());
        let mut next = reach.clone();
        for p in (0..name.len()).filter(|&p| reach[p]) {
            let mut q = p;
            for (i, c) in w.iter().enumerate() {
                let marked = i == 0 || lower || c.is_uppercase();
                if !marked && !lower {
                    continue; // an unmarked letter inside the word is skipped
                }
                if q >= name.len() || c.to_uppercase().next() != Some(name[q]) {
                    break;
                }
                q += 1;
                next[q] = true;
            }
        }
        reach = next;
    }
    reach[name.len()]
}

/// The rule itself, on the maintainer's backronyms and on text that must fail.
/// NEE SOON's and FARRER PARK's earlier README lettering is among the
/// failures: it spelled NESOOON and FARRERPAR. Both READMEs were corrected on
/// 2026-10-13 (NEutron, RisK) and the corrected texts pass.
#[test]
fn the_spelling_rule_accepts_marked_letters_only() {
    assert!(spells("Severe-accident Evolution and Melt Behaviour Analysis Workbench for Advanced Nuclear Geometries", "sembawang"));
    assert!(spells("BOmbardment of neutrons On Nuclides with Lagrangian transport and transmutation Yields", "boon-lay"));
    assert!(spells("Digital High-fidelity Orchestration by GUI for a Human-friendly Automated Unified Toolkit", "dhoby-ghaut"));
    assert!(spells("Thermal-hydraulic Artificial-intelligence Multi-Phase INtegrated Emulator System", "tampines"));
    assert!(!spells("Severe-accident Evolution and Melt Behaviour", "sembawang"), "too short");
    assert!(spells("NEutron Energy-dependent Simulation using Open-source Object-Oriented Numerics", "nee_soon"));
    assert!(spells("Finite-element Analysis for Reactor Reliability, Engineering Response, Plasticity And RisK", "farrer-park"));
    assert!(!spells("Neutron Energy-dependent Simulation using Open-source Object-Oriented Numerics", "nee_soon"));
    assert!(!spells("Finite-element Analysis for Reactor Reliability, Engineering Response, Plasticity And Risk", "farrer-park"));
    assert!(!spells("Consequence and Hazard Analysis for Nuclear Ground-level and atmospheric Impacts", "changhi"));
}

/// Every backronym spells its crate's name, or the name's first part when the
/// crate name goes on to say what the crate is (`tuas_boussinesq_solver` is
/// TUAS), or its last part when the package name spells the backronym out in
/// front of the short name
/// (`deck-based-open-source-visualisation-engine-for-reactors-dover` is DOVER;
/// renamed 2026-10-20 because `dover` on crates.io belongs to another
/// project). Seventeen crates carry one (2026-10-13; sembawang alone before).
#[test]
fn every_backronym_spells_its_crate_name() {
    let map = workspace();
    let mut with = Vec::new();
    for c in &map.crates {
        if let Some(b) = &c.backronym {
            let first = c.name.split(['-', '_']).next().unwrap_or(&c.name);
            let last = c.name.rsplit(['-', '_']).next().unwrap_or(&c.name);
            assert!(
                spells(b, &c.name) || spells(b, first) || spells(b, last),
                "{}: backronym {b:?} does not spell the name",
                c.name
            );
            with.push(c.name.as_str());
        }
    }
    with.sort_unstable();
    assert_eq!(with, [
        "bedok", "bishan", "boon-lay", "buangkok", "changi",
        "deck-based-open-source-visualisation-engine-for-reactors-dover", "dhoby-ghaut", "farrer-park", "kaki-bukit",
        "nee_soon", "pasir-ris", "petir", "raffles", "redhill", "sembawang", "tampines", "tuas_boussinesq_solver",
    ]);
}
