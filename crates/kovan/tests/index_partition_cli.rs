//! End-to-end check of the partitioned semantic index (GitHub #1668:
//! #1669 `index-partition`, #1670 `scip-merge`, #1671 `index-plan`) against
//! a real rust-analyzer, on a throwaway two-crate workspace.
//!
//! **Methodology.** `app` calls into `kern` (a function, a method through a
//! workspace type, an operator on a workspace type) and `kern` has calls of
//! its own. With `--max-closure 1` the plan must put each crate in its own
//! partition (`app`'s closure has two packages, over the budget, so it is
//! alone and reported). Each partition is indexed by `index-partition`,
//! which loads only that partition's closure; the two indexes are merged
//! with `scip-merge --plan`. The call graph built from the merged index is
//! then compared with the one built from a monolithic `rust-analyzer scip`
//! over the whole (tiny) workspace, the existing path.
//!
//! **Pass criteria.** Two partitions, every member once, each with its own
//! cache key in the matrix; each partition's index holds only its member's
//! documents (`ok`, nothing outside); the merge succeeds with the plan and
//! is refused, naming the member, with one partition missing; `app`'s
//! cross-crate calls resolve to `kern`'s functions through the merged
//! index; and the merged and monolithic call graphs have **identical**
//! `calls` and `uses`.
//!
//! **Gate.** Needs `rust-analyzer` (and its `rust-src`) on `PATH`; without
//! it the test prints why and passes without running. Unix (the worker's
//! view folder is made of symlinks).
//!
//! **Result (2026-11-19, rust-analyzer 1.99.1):** see the commit that adds
//! this test.

mod support;

use std::path::Path;
use std::process::Command;

fn kovan(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_kovan-cli"))
        .args(args)
        .output()
        .expect("spawn kovan-cli")
}

fn write(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

const KERN: &str = r#"use std::ops::Add;

#[derive(Clone, Copy)]
pub struct Pt(pub f64);

impl Add for Pt {
    type Output = Pt;
    fn add(self, o: Pt) -> Pt {
        Pt(scale(self.0) + o.0)
    }
}

impl Pt {
    pub fn norm(self) -> f64 {
        leaf(self.0)
    }
}

pub fn leaf(x: f64) -> f64 {
    x * 2.0
}

pub fn scale(x: f64) -> f64 {
    leaf(x) + 1.0
}
"#;

const APP: &str = r#"use kern::Pt;

pub fn run(x: f64) -> f64 {
    let p = Pt(x) + Pt(kern::scale(x));
    p.norm() + kern::leaf(x)
}

#[cfg(test)]
mod tests {
    #[test]
    fn runs() {
        assert!(super::run(1.0) > 0.0);
    }
}
"#;

#[cfg(unix)]
#[test]
fn partitions_merge_into_the_same_call_graph_as_the_monolithic_index() {
    if !support::rust_analyzer_runs() {
        eprintln!("SKIPPED: rust-analyzer is not on PATH (rustup component add rust-analyzer rust-src)");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "Cargo.toml", "[workspace]\nmembers = [\"crates/kern\", \"crates/app\"]\nresolver = \"2\"\n");
    write(root, "crates/kern/Cargo.toml", "[package]\nname = \"kern\"\nversion = \"0.1.0\"\nedition = \"2021\"\n");
    write(root, "crates/kern/src/lib.rs", KERN);
    write(
        root,
        "crates/app/Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nkern = { path = \"../kern\" }\n",
    );
    write(root, "crates/app/src/lib.rs", APP);
    let r = root.to_str().unwrap();
    let ok = |out: &std::process::Output, what: &str| {
        assert!(out.status.success(), "{what} failed: {}", String::from_utf8_lossy(&out.stderr));
    };

    // Plan: one partition per crate under a budget of one package.
    let plan_path = root.join("plan.json");
    let out = kovan(&["index-plan", "--workspace", r, "--max-closure", "1", "--out", plan_path.to_str().unwrap()]);
    ok(&out, "index-plan");
    assert!(String::from_utf8_lossy(&out.stderr).contains("app alone loads 2 packages"));
    let plan: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&plan_path).unwrap()).unwrap();
    let parts = plan["partitions"].as_array().unwrap();
    let members: Vec<&str> = parts.iter().map(|p| p["members"][0].as_str().unwrap()).collect();
    assert_eq!(members, ["app", "kern"], "largest closure first");
    let out = kovan(&["index-plan", "--workspace", r, "--max-closure", "1", "--matrix"]);
    ok(&out, "index-plan --matrix");
    let matrix: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let keys: Vec<&str> = matrix["include"].as_array().unwrap().iter().map(|m| m["key"].as_str().unwrap()).collect();
    assert!(keys.iter().all(|k| k.len() == 64) && keys[0] != keys[1], "{keys:?}");

    // One partition per crate, each loading only its own closure.
    for (id, krate) in [("p00", "app"), ("p01", "kern")] {
        let out_dir = root.join("parts").join(id);
        let out = kovan(&["index-partition", "--workspace", r, "--crates", krate, "--out", out_dir.to_str().unwrap()]);
        ok(&out, &format!("index-partition {krate}"));
        let rec: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(out_dir.join("partition.json")).unwrap()).unwrap();
        assert_eq!(rec["ok"], true, "{rec}");
        assert_eq!(rec["outside_members"].as_array().unwrap().len(), 0, "{rec}");
        assert!(rec["documents"][krate].as_u64().unwrap() >= 1, "{rec}");
        assert_eq!(rec["key"].as_str().unwrap(), keys[if krate == "app" { 0 } else { 1 }], "the matrix key is the record's");
        let key_only = kovan(&["index-partition", "--workspace", r, "--crates", krate, "--out", "unused", "--key-only"]);
        ok(&key_only, "--key-only");
        assert_eq!(String::from_utf8_lossy(&key_only.stdout).trim(), rec["key"].as_str().unwrap());
    }

    // Merge, checked against the plan; one partition missing is refused.
    let merged = root.join("merged.scip");
    let p = |id: &str| root.join("parts").join(id).to_str().unwrap().to_string();
    let out = kovan(&["scip-merge", "--workspace", r, "--plan", plan_path.to_str().unwrap(), "--out", merged.to_str().unwrap(), &p("p00"), &p("p01")]);
    ok(&out, "scip-merge");
    let bad = kovan(&["scip-merge", "--workspace", r, "--plan", plan_path.to_str().unwrap(), "--out", "x.scip", &p("p00")]);
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("INCOMPLETE") && String::from_utf8_lossy(&bad.stderr).contains("kern"));

    // The call graph from the merged index and from a monolithic one.
    let graph = |scip: &Path| -> serde_json::Value {
        let out = kovan(&["call-graph", "--workspace", r, "--scip", scip.to_str().unwrap()]);
        ok(&out, "call-graph --scip");
        serde_json::from_slice(&out.stdout).unwrap()
    };
    let from_parts = graph(&merged);
    let mono = root.join("mono.scip");
    let status = Command::new("rust-analyzer")
        .args(["scip", r, "--output", mono.to_str().unwrap()])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(status.status.success(), "{}", String::from_utf8_lossy(&status.stderr));
    let from_mono = graph(&mono);

    let calls = |v: &serde_json::Value| -> Vec<(String, String)> {
        let mut c: Vec<(String, String)> = v["calls"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| (c["from"].as_str().unwrap().to_string(), c["to"].as_str().unwrap().to_string()))
            .collect();
        c.sort();
        c
    };
    let cross = calls(&from_parts);
    for to in ["leaf", "scale", "Pt::norm", "Pt::add"] {
        assert!(
            cross.contains(&("crates/app/src/lib.rs::run".into(), format!("crates/kern/src/lib.rs::{to}"))),
            "app::run -> kern::{to} missing: {cross:?}"
        );
    }
    assert_eq!(from_parts["calls"], from_mono["calls"], "calls differ between merged and monolithic");
    assert_eq!(from_parts["uses"], from_mono["uses"], "uses differ between merged and monolithic");
    assert_eq!(from_parts["unresolved"], from_mono["unresolved"]);
}
