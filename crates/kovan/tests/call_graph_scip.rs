//! End-to-end check of `kovan-cli call-graph --backend scip` against the LSP
//! backend on a throwaway workspace (GitHub issue #757).
//!
//! **Methodology.** A two-crate workspace is written to a temp directory with
//! one case for each difference the #757 measurement found between a graph
//! built from one `rust-analyzer scip` index and the LSP-built one:
//!
//! - an **operator** call, `a + b` on a type with a workspace `impl Add`
//!   (invisible to the text scanner, so only SCIP may find it);
//! - a function **passed as a path**, `.map(kern::leaf)` (the LSP backend's
//!   documented blind spot);
//! - a call through a **function-typed parameter**, `f(x)` inside
//!   `fn apply(f: impl Fn(f64) -> f64, x: f64)`, which the LSP-built graph
//!   pinned to `apply` itself as a false recursive edge before #757;
//! - a call to a **derived** method, `Pt::default()` (`UNRESOLVED(other)`);
//! - a **symbol collision**: `src/util.rs::helper` and the example's own
//!   `examples/demo/util.rs::helper` share one SCIP symbol string, and each
//!   caller must reach the one in its own target (matched by location).
//!
//! Both backends run over the same workspace. Pass criteria: every LSP edge
//! is in the SCIP graph with the same lines; the SCIP graph has the operator
//! edge (kind `operator`) and the path edge (kind `fn_value`) and nothing
//! else extra; neither has an `apply -> apply` edge and both mark `f(x)`
//! `UNRESOLVED(closure)`; both mark the derived call `UNRESOLVED(other)`;
//! the collision resolves per target; `generator` records the backend and
//! the rust-analyzer version; a second SCIP build from the same index is
//! byte-identical.
//!
//! **Gate.** Needs `rust-analyzer` on `PATH`; without it the test prints why
//! and passes without running. The LSP daemon is stopped at the end.
//!
//! **Result (2026-10-07, rust-analyzer 1.98.0):** passes.

use std::path::Path;
use std::process::Command;

fn kovan(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_kovan-cli"))
        .args(args)
        .env("KOVAN_RA_TIMEOUT_SECS", "900")
        .output()
        .expect("spawn kovan-cli")
}

struct DaemonGuard(String);

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        let _ = kovan(&["lsp-daemon-stop", "--root", &self.0]);
    }
}

const KERN_LIB: &str = r#"pub mod util;

pub fn leaf(x: f64) -> f64 {
    x * 2.0
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Pt(pub f64);

impl std::ops::Add for Pt {
    type Output = Pt;
    fn add(self, o: Pt) -> Pt {
        Pt(self.0 + o.0)
    }
}

pub fn apply(f: impl Fn(f64) -> f64, x: f64) -> f64 {
    f(x)
}

pub fn sum(a: Pt, b: Pt) -> Pt {
    let z = Pt::default();
    a + b + z
}

pub fn doubled(v: Vec<f64>) -> Vec<f64> {
    v.into_iter().map(crate::leaf).collect()
}

pub fn twice(x: f64) -> f64 {
    apply(leaf, x) + util::helper(x)
}
"#;

const KERN_UTIL: &str = r#"pub fn helper(x: f64) -> f64 {
    x + 1.0
}
"#;

const DEMO_MAIN: &str = r#"mod util;

fn main() {
    let y = util::helper(1.0) + kern::twice(2.0);
    println!("{y}");
}
"#;

const DEMO_UTIL: &str = r#"pub fn helper(x: f64) -> f64 {
    kern::leaf(x)
}
"#;

fn write(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

#[test]
fn scip_backend_agrees_with_lsp_and_adds_operators() {
    if which::which("rust-analyzer").is_err() {
        eprintln!("SKIPPED: rust-analyzer is not on PATH (rustup component add rust-analyzer)");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/kern\"]\nresolver = \"2\"\n",
    );
    write(
        root,
        "crates/kern/Cargo.toml",
        "[package]\nname = \"kern\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    write(root, "crates/kern/src/lib.rs", KERN_LIB);
    write(root, "crates/kern/src/util.rs", KERN_UTIL);
    write(root, "crates/kern/examples/demo/main.rs", DEMO_MAIN);
    write(root, "crates/kern/examples/demo/util.rs", DEMO_UTIL);
    let root_s = root.to_str().unwrap();
    let _daemon = DaemonGuard(root_s.to_string());

    let build = |extra: &[&str]| -> serde_json::Value {
        let mut args = vec!["call-graph", "--workspace", root_s, "--crates", "kern"];
        args.extend_from_slice(extra);
        let out = kovan(&args);
        assert!(
            out.status.success(),
            "call-graph {extra:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).expect("json")
    };
    let lsp = build(&["--backend", "lsp"]);
    let scip = build(&["--backend", "scip"]);
    let index = root.join("target/kovan-scip/index.scip");
    assert!(index.is_file(), "the index is written under target/");

    assert_eq!(scip["schema"], 3);
    assert_eq!(lsp["generator"]["backend"], "lsp");
    assert_eq!(scip["generator"]["backend"], "scip");
    assert!(scip["generator"]["rust_analyzer"]
        .as_str()
        .unwrap()
        .starts_with("rust-analyzer"));

    type Edge = (String, String, String, Vec<u64>);
    let edges = |v: &serde_json::Value| -> Vec<Edge> {
        v["calls"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                (
                    c["from"].as_str().unwrap().to_string(),
                    c["to"].as_str().unwrap().to_string(),
                    c["kind"].as_str().unwrap().to_string(),
                    c["lines"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|l| l.as_u64().unwrap())
                        .collect(),
                )
            })
            .collect()
    };
    let (el, es) = (edges(&lsp), edges(&scip));
    for e in &el {
        assert!(
            es.contains(e),
            "LSP edge missing from SCIP: {e:?}\nSCIP: {es:#?}"
        );
    }
    let lib = "crates/kern/src/lib.rs";
    let extra: Vec<&Edge> = es.iter().filter(|e| !el.contains(e)).collect();
    let want_op = (
        format!("{lib}::sum"),
        format!("{lib}::Pt::add"),
        "operator".to_string(),
        vec![23],
    );
    let want_path = (
        format!("{lib}::doubled"),
        format!("{lib}::leaf"),
        "fn_value".to_string(),
        vec![27],
    );
    assert_eq!(extra, vec![&want_path, &want_op], "SCIP-only edges");

    for v in [&lsp, &scip] {
        assert!(
            !edges(v)
                .iter()
                .any(|e| e.0 == format!("{lib}::apply") && e.1 == format!("{lib}::apply")),
            "no false recursive edge through the fn-typed parameter"
        );
        let fns: Vec<&serde_json::Value> = v["crates"][0]["targets"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|t| t["modules"].as_array().unwrap())
            .flat_map(|m| m["functions"].as_array().unwrap())
            .collect();
        let gap = |id: &str| -> Vec<(u64, String)> {
            fns.iter().find(|f| f["id"] == id).unwrap()["unresolved"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|u| {
                            (
                                u["line"].as_u64().unwrap(),
                                u["kind"].as_str().unwrap().to_string(),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        assert_eq!(
            gap(&format!("{lib}::apply")),
            vec![(18, "closure".to_string())]
        );
        assert_eq!(gap(&format!("{lib}::sum")), vec![(22, "other".to_string())]);
        // The collision: each `helper` call reaches its own target's copy.
        let e = edges(v);
        assert!(e
            .iter()
            .any(|e| e.0 == "crates/kern/examples/demo/main.rs::main"
                && e.1 == "crates/kern/examples/demo/util.rs::helper"));
        assert!(e
            .iter()
            .any(|e| e.0 == format!("{lib}::twice") && e.1 == "crates/kern/src/util.rs::helper"));
    }

    let again = build(&["--scip", index.to_str().unwrap()]);
    assert_eq!(
        again, scip,
        "a second build from the same index is identical"
    );
}
