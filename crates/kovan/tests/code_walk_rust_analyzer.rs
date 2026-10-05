//! End-to-end check of `kovan-cli code-walk` against a real rust-analyzer
//! (GitHub issue #523).
//!
//! **Methodology.** A throwaway Cargo crate is written to a temp directory:
//! `entry -> middle -> Shape::area (a method call) -> leaf`, a shorter route
//! `entry -> other -> leaf`, a trait-method call through a generic
//! (`T: Measure`), a call through a local closure, a tuple-struct
//! constructor, and std calls. `code-walk --from entry --to leaf --format
//! json` must return exactly the 2-hop chain (not the 3-hop one); the tree
//! from `entry` must reach `Shape::area` through the method call; the trait
//! call must be a `trait` gap and the closure call a `closure` gap; no std
//! function may appear among the nodes; lines are 1-based. Then a lesson file with a
//! `<!-- code-walk: ... -->` block and a hand-filled hop is written,
//! `code-walk-check --update` fills it, `code-walk-check` passes on it, and
//! after deleting the hand hop's target function the check fails.
//!
//! **Gate.** Needs `rust-analyzer` on `PATH`; when it is absent the test
//! prints why and returns (passes) without running anything, so the suite
//! stays green on machines and CI runners without it. The temp crate's
//! keep-warm daemon is stopped at the end.
//!
//! **Result (2026-10-04, rust-analyzer 1.98.0, one core):** passes; the
//! first query includes indexing the temp crate and std (tens of seconds).

use std::path::Path;
use std::process::Command;

fn kovan(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_kovan-cli"))
        .args(args)
        .env("KOVAN_RA_TIMEOUT_SECS", "900")
        .output()
        .expect("spawn kovan-cli")
}

const LIB: &str = r#"pub trait Measure {
    fn measure(&self) -> f64;
}

pub struct Shape(pub f64);

impl Shape {
    /// Area of the shape.
    pub fn area(&self) -> f64 {
        leaf(self.0)
    }
}

/// The concept function.
pub fn leaf(x: f64) -> f64 {
    x * x
}

pub fn middle(s: &Shape) -> f64 {
    s.area()
}

pub fn other(x: f64) -> f64 {
    leaf(x)
}

pub fn generic<T: Measure>(t: &T) -> f64 {
    t.measure()
}

/// Entry point.
pub fn entry() -> f64 {
    let s = Shape(2.0);
    let f = |y: f64| y + 1.0;
    let v = vec![1.0_f64, 2.0];
    let total: f64 = v.iter().sum();
    middle(&s) + other(3.0) + f(total)
}
"#;

#[test]
fn code_walk_against_a_real_rust_analyzer() {
    if which::which("rust-analyzer").is_err() {
        eprintln!("SKIPPED: rust-analyzer is not on PATH (rustup component add rust-analyzer)");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"walkfixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
    )
    .unwrap();
    std::fs::write(root.join("src/lib.rs"), LIB).unwrap();
    let root_s = root.to_str().unwrap();
    let _daemon = DaemonGuard(root_s.to_string());

    let out = kovan(&[
        "code-walk", "--root", root_s, "--from", "src/lib.rs::entry", "--to", "src/lib.rs::leaf",
        "--format", "json",
    ]);
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(out.status.success(), "code-walk failed: {stderr}");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let names: Vec<String> = v["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["qualname"].as_str().unwrap().to_string())
        .collect();
    let chains = v["chains"].as_array().unwrap();
    let chain_names: Vec<Vec<&str>> = chains
        .iter()
        .map(|c| {
            c.as_array()
                .unwrap()
                .iter()
                .map(|i| names[i.as_u64().unwrap() as usize].as_str())
                .collect()
        })
        .collect();
    assert_eq!(chain_names, vec![vec!["entry", "other", "leaf"]], "{stderr}");
    run_rest(root, root_s, &v, &names);
}

/// Stops the temp crate's keep-warm daemon even when an assertion fails.
struct DaemonGuard(String);

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        let _ = kovan(&["lsp-daemon-stop", "--root", &self.0]);
    }
}

fn run_rest(root: &Path, root_s: &str, _v: &serde_json::Value, _names: &[String]) {
    // The longer route resolves the method call `s.area()` to `Shape::area`.
    let out = kovan(&["code-walk", "--root", root_s, "--from", "src/lib.rs::entry", "--depth", "3", "--format", "json"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let q: Vec<&str> = v["nodes"].as_array().unwrap().iter().map(|n| n["qualname"].as_str().unwrap()).collect();
    for want in ["entry", "middle", "other", "Shape::area", "leaf"] {
        assert!(q.contains(&want), "{want} missing from {q:?}");
    }

    // Tree mode: the trait call inside `generic` is not reachable from
    // `entry`, so walk `generic` itself; the closure gap sits in `entry`.
    let out = kovan(&[
        "code-walk", "--root", root_s, "--from", "src/lib.rs::generic", "--format", "json",
    ]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let kinds: Vec<&str> = v["gaps"].as_array().unwrap().iter().map(|g| g["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, vec!["trait"]);

    let out = kovan(&["code-walk", "--root", root_s, "--from", "src/lib.rs::entry", "--depth", "1", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let gaps: Vec<(&str, &str)> = v["gaps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| (g["kind"].as_str().unwrap(), g["callee"].as_str().unwrap()))
        .collect();
    assert_eq!(gaps, vec![("closure", "f")]);
    let files: Vec<&str> = v["nodes"].as_array().unwrap().iter().map(|n| n["file"].as_str().unwrap()).collect();
    assert!(files.iter().all(|f| *f == "src/lib.rs"), "std leaked in: {files:?}");
    assert!(v["external_calls"].as_u64().unwrap() >= 1, "vec!/iter/sum are external");
    let entry_line = v["nodes"][0]["line"].as_u64().unwrap();
    assert_eq!(entry_line, 32, "1-based declaration line of `entry`");

    // Lesson block: generate, check, then break the hand hop.
    let lesson = root.join("lesson.md");
    std::fs::write(
        &lesson,
        "# L\n\n<!-- code-walk: from=src/lib.rs::generic to=src/lib.rs::leaf\nhand: src/lib.rs::generic -> src/lib.rs::other | Measure is implemented by a type that calls other\n-->\n",
    )
    .unwrap();
    let lesson_s = lesson.to_str().unwrap();
    let out = kovan(&["code-walk-check", "--root", root_s, "--update", lesson_s]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = std::fs::read_to_string(&lesson).unwrap();
    assert!(text.contains("**filled by hand**: Measure is implemented"), "{text}");
    assert!(text.contains("UNRESOLVED(trait)"), "{text}");
    // Each hop's code inline, relative to the lesson (here at the root), with
    // the check the Pages build runs without rust-analyzer.
    assert!(text.contains("{{#include src/lib.rs:"), "{text}");
    assert!(text.contains("<!-- snippet-check: src/lib.rs:"), "{text}");
    assert!(text.contains("<!-- /code-walk -->"));
    let out = kovan(&["code-walk-check", "--root", root_s, lesson_s]);
    assert!(out.status.success(), "fresh block must pass: {}", String::from_utf8_lossy(&out.stderr));

    std::fs::write(root.join("src/lib.rs"), LIB.replace("pub fn other(", "pub fn renamed(").replace("other(3.0)", "renamed(3.0)")).unwrap();
    let out = kovan(&["code-walk-check", "--root", root_s, lesson_s]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "a vanished hand-hop target must fail the check");
    assert!(err.contains("hand-filled hop"), "{err}");
}
