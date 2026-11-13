//! **wgpu only, never glow** — the workspace hard rule, pinned so it cannot
//! drift back silently (maintainer, 2026-10-27: "refactor glow out completely,
//! just use WGPU in all crates at all times ... this will be a hard rule for
//! this workspace").
//!
//! The root `CLAUDE.md` states the rule. This test is why it holds: a rule
//! that nothing asserts drifts back the first time someone copies an old demo
//! (the root `CLAUDE.md` makes the same point about physics defaults — "a
//! default that nothing asserts will drift back off silently").
//!
//! # What it checks
//!
//! Every `Cargo.toml` and every `.rs` file in the workspace, excluding
//! `target/` and `vendor/`:
//!
//! 1. No dependency asks for eframe's (or anything's) `"glow"` feature.
//! 2. No direct dependency on the `glow` crate.
//! 3. No code selects `Renderer::Glow`.
//! 4. The workspace's own `eframe` entry enables `"wgpu"`.
//!
//! Comment lines are skipped, so the struck-through history notes that record
//! *why* glow was removed do not trip it.
//!
//! # What it deliberately allows
//!
//! The `glow` crate still appears in `Cargo.lock` on wasm32, as a dependency
//! of **`wgpu-hal`**: it is how wgpu itself drives WebGL2, its fallback where
//! a browser has no WebGPU. That is wgpu's internal implementation, not a
//! crate in this workspace choosing glow, and removing it would mean
//! WebGPU-only builds that fail on every browser without WebGPU. So this test
//! looks at what the workspace *asks for*, never at the lock file.
//!
//! **Result (2026-10-27):** passes, after the refactor that removed glow from
//! `dhoby-ghaut` (its wasm `eframe` feature and six web demos: Monte Carlo,
//! nuclear data, dispersion, delta tracking, DEM, TRISO-ATOPS) and
//! `kovan-web`. Before that refactor it would have reported nine violations:
//! two `"glow"` features and seven `Renderer::Glow` call sites.

use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    // crates/outram-park-digital-twin-engine -> workspace root
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

/// Every file under `dir` with extension `ext`, skipping build output,
/// vendored upstream sources and the literature submodule.
fn files(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if matches!(name.as_ref(), "target" | "vendor" | ".git" | "reactor-literature" | "node_modules") {
                continue;
            }
            files(&path, ext, out);
        } else if path.extension().is_some_and(|e| e == ext) {
            out.push(path);
        }
    }
}

/// The non-comment lines of a file, with their 1-based numbers.
fn code_lines(text: &str, comment: &str) -> Vec<(usize, String)> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim_start().starts_with(comment))
        .map(|(i, l)| (i + 1, l.to_owned()))
        .collect()
}

#[test]
fn no_crate_selects_glow() {
    let root = workspace_root();
    let mut violations = Vec::new();

    let mut tomls = vec![root.join("Cargo.toml")];
    files(&root.join("crates"), "toml", &mut tomls);
    for path in &tomls {
        if path.file_name().is_none_or(|n| n != "Cargo.toml") {
            continue;
        }
        let text = std::fs::read_to_string(path).unwrap_or_default();
        for (n, line) in code_lines(&text, "#") {
            // Drop a trailing comment before matching.
            let code = line.split('#').next().unwrap_or("");
            if code.contains(&format!("\"{}\"", "glow")) {
                violations.push(format!("{}:{n}: asks for the `glow` feature", path.display()));
            }
            if code.trim_start().starts_with("glow ") || code.trim_start().starts_with("glow=") {
                violations.push(format!("{}:{n}: depends on the `glow` crate directly", path.display()));
            }
        }
    }

    let mut sources = Vec::new();
    files(&root.join("crates"), "rs", &mut sources);
    // Built at run time so this file does not match its own needle.
    let needle = format!("Renderer::{}", "Glow");
    for path in &sources {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        for (n, line) in code_lines(&text, "//") {
            if line.contains(&needle) {
                violations.push(format!("{}:{n}: selects `{needle}`", path.display()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "the workspace is wgpu-only (root CLAUDE.md, hard rule); glow found:\n{}",
        violations.join("\n")
    );
    // The scan must actually have looked at something, or it proves nothing.
    assert!(tomls.len() > 40, "only {} manifests scanned", tomls.len());
    assert!(sources.len() > 500, "only {} sources scanned", sources.len());
}

#[test]
fn the_workspace_eframe_is_on_wgpu() {
    let text = std::fs::read_to_string(workspace_root().join("Cargo.toml")).expect("root Cargo.toml");
    // The `eframe = { ... features = [ ... ] }` entry, possibly over lines.
    let start = text
        .lines()
        .position(|l| l.trim_start().starts_with("eframe"))
        .expect("an eframe entry in [workspace.dependencies]");
    let entry: String = text
        .lines()
        .skip(start)
        .take_while(|l| !l.trim_start().starts_with(']') || l.contains("features"))
        .take(12)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(entry.contains("\"wgpu\""), "workspace eframe does not enable wgpu:\n{entry}");
    assert!(!entry.contains("\"glow\""), "workspace eframe enables glow:\n{entry}");
}
