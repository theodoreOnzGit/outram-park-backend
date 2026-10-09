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
//! - a backronym spells the crate's name: the capital initials of its words
//!   (a hyphenated word counts once) are the name in capitals. sembawang
//!   carries one.
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

/// The capital initials of a backronym's words, a hyphenated word counting
/// once: "Severe-accident Evolution and Melt ..." gives "SEM...".
fn initials(backronym: &str) -> String {
    backronym.split_whitespace().filter_map(|w| w.chars().next()).filter(|c| c.is_uppercase()).collect()
}

#[test]
fn every_backronym_spells_its_crate_name() {
    let map = workspace();
    let mut with = Vec::new();
    for c in &map.crates {
        if let Some(b) = &c.backronym {
            let name: String = c.name.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_uppercase();
            assert_eq!(initials(b), name, "{}: backronym {b:?} does not spell the name", c.name);
            with.push(c.name.as_str());
        }
    }
    assert!(with.contains(&"sembawang"), "sembawang carries its backronym (#815): {with:?}");
}
