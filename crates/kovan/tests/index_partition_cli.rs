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
//! index; and the merged call graph equals the monolithic one (`calls`,
//! with kinds and lines, and `uses`) **plus exactly one edge**: a call into
//! a file another package holds, loaded through `#[path]`, which the
//! monolithic run leaves unresolved and the partition resolves (rustc
//! compiles that call). Same-package `#[path]` files resolve in both.
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
    // `#[path]` modules, the two cases the full-workspace comparison found
    // (2026-11-19): one elsewhere in the same package, one in another
    // package (as `dhoby-ghaut`'s examples load `outram-mc-libs` files).
    write(root, "crates/app/extra/util.rs", "pub fn twice(x: f64) -> f64 {\n    2.0 * x\n}\n");
    write(root, "crates/kern/shared/helper.rs", "pub fn half(x: f64) -> f64 {\n    x / 2.0\n}\n");
    write(
        root,
        "crates/app/tests/it.rs",
        "#[path = \"../extra/util.rs\"]\nmod util;\n#[path = \"../../kern/shared/helper.rs\"]\nmod helper;\n\n#[test]\nfn both() {\n    assert!(util::twice(helper::half(app::run(1.0))) > 0.0);\n}\n",
    );
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
    for to in ["crates/app/extra/util.rs::twice", "crates/kern/shared/helper.rs::half", "crates/app/src/lib.rs::run"] {
        assert!(cross.contains(&("crates/app/tests/it.rs::both".into(), to.into())), "it::both -> {to} missing: {cross:?}");
    }
    // Merged = monolithic, plus exactly the cross-package `#[path]` edge:
    // the monolithic run does not resolve a `#[path]` file in another
    // package's source root (rustc compiles it, and `both` does call
    // `half`); the partition includes that folder in `app`'s test target.
    let mut want = calls(&from_mono);
    want.push(("crates/app/tests/it.rs::both".into(), "crates/kern/shared/helper.rs::half".into()));
    want.sort();
    assert_eq!(calls(&from_parts), want, "merged calls are not monolithic plus the cross-package #[path] edge");
    let without = |v: &serde_json::Value| -> Vec<serde_json::Value> {
        v["calls"].as_array().unwrap().iter().filter(|c| c["to"] != "crates/kern/shared/helper.rs::half").cloned().collect()
    };
    assert_eq!(without(&from_parts), without(&from_mono), "every other edge identical, kinds and lines included");
    assert_eq!(from_parts["uses"], from_mono["uses"], "uses differ between merged and monolithic");
    assert_eq!(from_parts["unresolved"], from_mono["unresolved"]);

    // Local kovan (Phase F): with no index given, `call-graph --backend
    // scip` runs the partitioned orchestrator by default (unix) into
    // target/kovan-scip/, and a second run reuses every unchanged partition.
    let local = |env: &[(&str, &str)]| -> (serde_json::Value, String) {
        let mut c = Command::new(env!("CARGO_BIN_EXE_kovan-cli"));
        c.args(["call-graph", "--workspace", r, "--backend", "scip"]);
        for (k, v) in env {
            c.env(k, v);
        }
        let out = c.output().unwrap();
        ok(&out, "call-graph --backend scip");
        (serde_json::from_slice(&out.stdout).unwrap(), String::from_utf8_lossy(&out.stderr).into_owned())
    };
    let (first, log) = local(&[("KOVAN_INDEX_MAX_CLOSURE", "1"), ("KOVAN_INDEX_JOBS", "2")]);
    assert!(log.contains("partitioned rust-analyzer index, 2 partitions"), "{log}");
    assert!(root.join("target/kovan-scip/parts/p00/partition.json").is_file());
    assert_eq!(first["calls"], from_parts["calls"], "local partitioned calls differ from the merged CLI run");
    assert_eq!(first["uses"], from_parts["uses"]);
    let (_, again) = local(&[("KOVAN_INDEX_MAX_CLOSURE", "1")]);
    assert_eq!(again.matches("unchanged, reused").count(), 2, "{again}");
    // `KOVAN_INDEXER=monolithic` keeps the whole-workspace run.
    let (mono_local, log) = local(&[("KOVAN_INDEXER", "monolithic")]);
    assert!(!log.contains("partitioned"), "{log}");
    assert_eq!(mono_local["calls"], from_mono["calls"]);
}
