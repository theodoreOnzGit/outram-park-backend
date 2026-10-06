//! End-to-end check of `kovan-cli call-graph` against a real rust-analyzer
//! (GitHub issue #737).
//!
//! **Methodology.** A throwaway two-crate workspace is written to a temp
//! directory: `kern` (a lib with a submodule `geom`, an inherent method
//! `Shape::area`, a trait `Measure` and a `#[cfg(test)]` module) and `app`
//! (a lib, plus a multi-file example `examples/demo/` whose `physics`
//! module calls into `kern`). `call-graph --crates app,kern` must
//! - list the lib module tree (`""`, `geom`) and the example's (`""`,
//!   `physics`), the test module marked `test`;
//! - resolve the cross-crate call from the example's
//!   `physics.rs::Model::update` to `kern`'s `geom.rs::Shape::area`, with
//!   both call-site lines, and the function-value call `.map(leaf)`;
//! - mark the generic trait call `t.measure()` `UNRESOLVED(trait)`;
//! - aggregate a module edge `physics.rs -> geom.rs` and a crate edge
//!   `app -> kern`;
//! - give byte-identical JSON on a second run (determinism).
//!
//! **Gate.** Needs `rust-analyzer` on `PATH`; without it the test prints why
//! and passes without running, as `code_walk_rust_analyzer.rs` does. The
//! temp workspace's keep-warm daemon is stopped at the end.
//!
//! **Result (2026-10-06, rust-analyzer 1.98.0):** passes.

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

const KERN_LIB: &str = r#"pub mod geom;

pub trait Measure {
    fn measure(&self) -> f64;
}

/// The leaf.
pub fn leaf(x: f64) -> f64 {
    x * 2.0
}

pub fn generic<T: Measure>(t: &T) -> f64 {
    t.measure()
}

#[cfg(test)]
mod tests;
"#;

const KERN_GEOM: &str = r#"pub struct Shape(pub f64);

impl Shape {
    /// Area of the shape.
    pub fn area(&self) -> f64 {
        crate::leaf(self.0)
    }
}
"#;

const KERN_TESTS: &str = r#"#[test]
fn leaf_doubles() {
    assert_eq!(crate::leaf(1.0), 2.0);
}
"#;

const APP_LIB: &str = r#"pub fn run() -> f64 {
    kern::leaf(1.0)
}
"#;

const DEMO_MAIN: &str = r#"mod physics;

fn main() {
    let mut m = physics::Model { x: 1.0 };
    m.update();
}
"#;

const DEMO_PHYSICS: &str = r#"use kern::leaf;
pub struct Model {
    pub x: f64,
}

impl Model {
    /// Advance one step.
    pub fn update(&mut self) {
        let s = kern::geom::Shape(self.x);
        self.x = s.area();
        let v: Vec<f64> = vec![1.0].into_iter().map(leaf).collect();
        self.x += s.area() + v[0];
    }
}
"#;

fn write(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

#[test]
fn call_graph_against_a_real_rust_analyzer() {
    if which::which("rust-analyzer").is_err() {
        eprintln!("SKIPPED: rust-analyzer is not on PATH (rustup component add rust-analyzer)");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/kern\", \"crates/app\"]\nresolver = \"2\"\n",
    );
    write(
        root,
        "crates/kern/Cargo.toml",
        "[package]\nname = \"kern\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    write(root, "crates/kern/src/lib.rs", KERN_LIB);
    write(root, "crates/kern/src/geom.rs", KERN_GEOM);
    write(root, "crates/kern/src/tests.rs", KERN_TESTS);
    write(
        root,
        "crates/app/Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nkern = { path = \"../kern\" }\n",
    );
    write(root, "crates/app/src/lib.rs", APP_LIB);
    write(root, "crates/app/examples/demo/main.rs", DEMO_MAIN);
    write(root, "crates/app/examples/demo/physics.rs", DEMO_PHYSICS);
    let root_s = root.to_str().unwrap();
    let _daemon = DaemonGuard(root_s.to_string());

    let run = || {
        let out = kovan(&["call-graph", "--workspace", root_s, "--crates", "kern,app"]);
        assert!(
            out.status.success(),
            "call-graph failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        out.stdout
    };
    let first = run();
    let v: serde_json::Value = serde_json::from_slice(&first).expect("json");

    assert_eq!(v["scope"], serde_json::json!(["app", "kern"]));
    let app = &v["crates"][0];
    let kinds: Vec<(&str, &str)> = app["targets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| (t["kind"].as_str().unwrap(), t["name"].as_str().unwrap()))
        .collect();
    assert_eq!(kinds, vec![("lib", "app"), ("example", "demo")]);
    let demo_paths: Vec<&str> = app["targets"][1]["modules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["path"].as_str().unwrap())
        .collect();
    assert_eq!(demo_paths, vec!["", "physics"]);
    let kern_mods = v["crates"][1]["targets"][0]["modules"].as_array().unwrap();
    let kern_paths: Vec<(&str, bool)> = kern_mods
        .iter()
        .map(|m| {
            (
                m["path"].as_str().unwrap(),
                m["test"].as_bool().unwrap_or(false),
            )
        })
        .collect();
    assert_eq!(
        kern_paths,
        vec![("geom", false), ("", false), ("tests", true)]
    );

    let update = "crates/app/examples/demo/physics.rs::Model::update";
    let area = "crates/kern/src/geom.rs::Shape::area";
    let calls = v["calls"].as_array().unwrap();
    let find = |from: &str, to: &str| calls.iter().find(|c| c["from"] == from && c["to"] == to);
    let c = find(update, area).unwrap_or_else(|| {
        panic!(
            "update -> Shape::area not resolved: {}",
            String::from_utf8_lossy(&first)
        )
    });
    assert_eq!(c["lines"], serde_json::json!([10, 12]));
    let fv = find(update, "crates/kern/src/lib.rs::leaf").expect(".map(leaf) resolved");
    assert_eq!(fv["kind"], "fn_value");
    assert!(find(
        "crates/kern/src/geom.rs::Shape::area",
        "crates/kern/src/lib.rs::leaf"
    )
    .is_some());
    assert!(find("crates/app/examples/demo/main.rs::main", update).is_some());

    let generic = kern_mods[1]["functions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "generic")
        .unwrap();
    assert_eq!(generic["unresolved"][0]["kind"], "trait");
    let upd = app["targets"][1]["modules"][1]["functions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["id"] == update)
        .unwrap();
    assert_eq!(
        (
            upd["start_line"].as_u64(),
            upd["line"].as_u64(),
            upd["end_line"].as_u64()
        ),
        (Some(7), Some(8), Some(13))
    );
    assert!(upd["source"]
        .as_str()
        .unwrap()
        .starts_with("    /// Advance one step."));

    let mc = v["module_calls"].as_array().unwrap();
    assert!(mc
        .iter()
        .any(|m| m["from"] == "crates/app/examples/demo/physics.rs"
            && m["to"] == "crates/kern/src/geom.rs"
            && m["sites"] == 2));
    assert!(v["crate_calls"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["from"] == "app" && m["to"] == "kern"));
    let tests_fn = &kern_mods[2]["functions"][0];
    assert_eq!(tests_fn["test"], true);

    let second = run();
    assert!(
        first == second,
        "call-graph output differs between two runs"
    );
}
