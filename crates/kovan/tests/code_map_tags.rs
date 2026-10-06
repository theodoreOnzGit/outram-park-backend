//! Every workspace member carries a well-formed code-map tag, and the tags
//! agree with the dependency graph (GitHub #729, #733; maintainer decisions
//! 2026-10-06).
//!
//! Each crate's `Cargo.toml` has `[package.metadata.kovan]` with:
//!
//! - `row`, its degree of integration: 0-1 utilities and standalone crates,
//!   2 domain solvers, 3 coupled multiphysics, 4 integrated GUI apps;
//! - `topic`, the box kovan's code map draws it in;
//! - `fidelity`, its order in that box (1 = highest fidelity), or `"all"`
//!   for a crate spanning every fidelity level (raffles);
//! - optionally `maturity = "placeholder"`, drawn greyed.
//!
//! What is checked, reading the tags through `cargo metadata` so nothing is
//! copied by hand:
//!
//! - every member has a tag, with a known topic and a row in 0..=4;
//! - row 4 is exactly the `app` topic, and utilities sit in rows 0-1;
//! - no crate sits in a lower row than a required dependency (optional and
//!   dev-dependencies do not count: the row is what the crate needs to
//!   build);
//! - no two crates share a slot (topic, row, fidelity).

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::Command;

const TOPICS: [&str; 11] = [
    "app",
    "neutronics",
    "thermal-hydraulics",
    "fuel-performance",
    "structural-mechanics",
    "chemistry",
    "fuel-cycle",
    "granular-dem",
    "risk",
    "utility",
    "knowledge-management",
];

struct Tag {
    row: u64,
    topic: String,
    fidelity: String,
}

fn workspace() -> serde_json::Value {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(cargo)
        .current_dir(&root)
        .args(["metadata", "--format-version", "1", "--no-deps", "--offline"])
        .output()
        .expect("cargo metadata runs");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).expect("cargo metadata is JSON")
}

#[test]
fn every_crate_is_placed_on_the_code_map_consistently_with_its_dependencies() {
    let meta = workspace();
    let packages = meta["packages"].as_array().unwrap();
    let names: BTreeSet<&str> = packages.iter().map(|p| p["name"].as_str().unwrap()).collect();

    let mut tags: BTreeMap<&str, Tag> = BTreeMap::new();
    for p in packages {
        let name = p["name"].as_str().unwrap();
        let t = &p["metadata"]["kovan"];
        assert!(t.is_object(), "{name}: no [package.metadata.kovan] tag");
        let row = t["row"].as_u64().unwrap_or_else(|| panic!("{name}: row"));
        let topic = t["topic"].as_str().unwrap_or_else(|| panic!("{name}: topic"));
        let fidelity = match &t["fidelity"] {
            serde_json::Value::Number(n) if n.as_u64().is_some_and(|f| f >= 1) => n.to_string(),
            serde_json::Value::String(s) if s == "all" => s.clone(),
            other => panic!("{name}: fidelity {other}"),
        };
        assert!(row <= 4, "{name}: row {row}");
        assert!(TOPICS.contains(&topic), "{name}: topic {topic}");
        assert_eq!(row == 4, topic == "app", "{name}: row 4 is the app row, and only it");
        if topic == "utility" {
            assert!(row <= 1, "{name}: a utility sits in row 0 or 1");
        }
        if let Some(m) = t.get("maturity") {
            assert_eq!(m.as_str(), Some("placeholder"), "{name}: maturity");
        }
        tags.insert(
            name,
            Tag {
                row,
                topic: topic.to_string(),
                fidelity,
            },
        );
    }

    let mut slots = BTreeSet::new();
    for (name, t) in &tags {
        assert!(
            slots.insert((t.topic.clone(), t.row, t.fidelity.clone())),
            "{name}: shares topic {} row {} fidelity {} with another crate",
            t.topic,
            t.row,
            t.fidelity
        );
    }

    for p in packages {
        let name = p["name"].as_str().unwrap();
        for d in p["dependencies"].as_array().unwrap() {
            let dep = d["name"].as_str().unwrap();
            let required = d["kind"].is_null() && !d["optional"].as_bool().unwrap_or(false);
            if !required || !names.contains(dep) {
                continue;
            }
            assert!(
                tags[name].row >= tags[dep].row,
                "{name} (row {}) sits below {dep} (row {}), which it depends on",
                tags[name].row,
                tags[dep].row
            );
        }
    }
}
