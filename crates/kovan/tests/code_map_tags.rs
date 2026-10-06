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
//!   library root, `a::b`), `level` and `why`.
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
//!   crate itself, and says why.

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
        let level = |v: &serde_json::Value| v.as_u64().filter(|f| *f <= 4);
        match &t["fidelity"] {
            serde_json::Value::Null => assert!(
                matches!(topic, "utility" | "knowledge-management"),
                "{name}: fidelity is required outside utilities and knowledge management"
            ),
            serde_json::Value::Array(r) => {
                let (lo, hi) = match r.as_slice() {
                    [lo, hi] => (level(lo), level(hi)),
                    _ => (None, None),
                };
                assert!(
                    matches!((lo, hi), (Some(lo), Some(hi)) if lo < hi),
                    "{name}: fidelity range {r:?} must be [lo, hi], 0 <= lo < hi <= 4"
                );
            }
            v => assert!(level(v).is_some(), "{name}: fidelity {v} must be 0..=4"),
        }
        assert!(row <= 4, "{name}: row {row}");
        assert!(TOPICS.contains(&topic), "{name}: topic {topic}");
        assert_eq!(row == 4, topic == "app", "{name}: row 4 is the app row, and only it");
        if topic == "utility" {
            assert!(row <= 1, "{name}: a utility sits in row 0 or 1");
        }
        let maturity = t["maturity"]
            .as_u64()
            .filter(|m| *m <= 4)
            .unwrap_or_else(|| panic!("{name}: maturity must be 0..=4"));
        if let Some(list) = t.get("maturity_modules") {
            let lib = p["targets"]
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["kind"].as_array().unwrap().iter().any(|k| k == "lib"))
                .unwrap_or_else(|| panic!("{name}: maturity_modules needs a library"));
            let src = PathBuf::from(lib["src_path"].as_str().unwrap());
            let base = src.parent().unwrap();
            for m in list.as_array().unwrap() {
                let module = m["module"].as_str().unwrap_or_else(|| panic!("{name}: module"));
                let level = m["level"].as_u64().unwrap_or_else(|| panic!("{name}: {module} level"));
                let why = m["why"].as_str().unwrap_or_default();
                assert!(level <= 4 && level > maturity, "{name}: {module} level {level} must be above the crate's {maturity}");
                assert!(!why.trim().is_empty(), "{name}: {module} needs a why");
                let rel: PathBuf = module.split("::").collect();
                let exists = base.join(&rel).with_extension("rs").is_file()
                    || base.join(&rel).join("mod.rs").is_file();
                assert!(exists, "{name}: module {module} not found under {}", base.display());
            }
        }
        tags.insert(name, Tag { row });
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
