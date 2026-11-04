//! **Never declare modules through a macro** — the workspace hard rule
//! (maintainer, 2026-11-04), pinned so it cannot drift back silently.
//!
//! kovan builds its code tree by reading `mod` lines from source text
//! (`kovan_common::call_graph::modules`) and does not expand macros, so a
//! module a macro declares (`mod $m;` in a `macro_rules!`) is missing from
//! the review. The root `CLAUDE.md` states the rule.
//!
//! # What it checks
//!
//! Every `.rs` file in the workspace, excluding `target/`, `vendor/`, `.git`,
//! `node_modules` and the literature submodule: no non-comment line holds
//! `mod $<ident>`, the form a `macro_rules!` uses to declare a module from a
//! macro argument.
//!
//! **Result (2026-11-04):** passes, after `monte_carlo_web`'s `rung_table!`
//! stopped declaring its eleven rung modules (they are plain `mod` lines in
//! its `main.rs` now). Before that change it reported one violation,
//! `crates/dhoby-ghaut/examples/monte_carlo_web/rungs.rs:292`.

use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    // crates/kovan -> workspace root
    dunce::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")).expect("workspace root")
}

/// Every `.rs` file under `dir`, skipping build output, vendored upstream
/// sources and the literature submodule.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if path.is_dir() {
            if !matches!(name.to_string_lossy().as_ref(), "target" | "vendor" | ".git" | "reactor-literature" | "node_modules") {
                rust_files(&path, out);
            }
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The 1-based lines of `text` that declare a module from a macro argument
/// (`mod $name`), comment lines skipped.
fn macro_declared_mods(text: &str) -> Vec<usize> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim_start().starts_with("//"))
        .filter(|(_, l)| {
            l.match_indices("mod").any(|(i, _)| {
                let before = l[..i].chars().next_back();
                let rest = l[i + 3..].trim_start();
                !before.is_some_and(|c| c.is_alphanumeric() || c == '_')
                    && l[i + 3..].starts_with(char::is_whitespace)
                    && rest.starts_with('$')
            })
        })
        .map(|(n, _)| n + 1)
        .collect()
}

#[test]
fn the_scanner_sees_a_macro_declared_module_and_nothing_else() {
    assert_eq!(macro_declared_mods("macro_rules! t {\n  ($($m:ident),+) => { $( mod $m; )+ };\n}"), [2]);
    assert_eq!(macro_declared_mods("#[cfg(x)] mod  $name;"), [1]);
    assert!(macro_declared_mods("mod godiva;\n// mod $m; in a comment\nlet modx = $y;\nfn model() {}").is_empty());
}

#[test]
fn no_module_in_the_workspace_is_declared_by_a_macro() {
    let root = workspace_root();
    let mut files = Vec::new();
    rust_files(&root.join("crates"), &mut files);
    assert!(files.len() > 1000, "the scan found only {} files: wrong root?", files.len());
    let this = Path::new(file!()).file_name();
    let bad: Vec<String> = files
        .iter()
        .filter(|p| p.file_name() != this)
        .flat_map(|p| {
            let text = std::fs::read_to_string(p).unwrap_or_default();
            let rel = p.strip_prefix(&root).unwrap_or(p).display().to_string();
            macro_declared_mods(&text).into_iter().map(move |n| format!("{rel}:{n}"))
        })
        .collect();
    assert!(bad.is_empty(), "modules declared through a macro (root CLAUDE.md, HARD RULE): kovan cannot see them\n{}", bad.join("\n"));
}
