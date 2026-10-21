//! A **read-only** compatibility check of the artifact parser against a real
//! Kovan folder on this machine (GitHub issue #743).
//!
//! # Methodology
//!
//! Set `KOVAN_COMPAT_ROOT` to a Kovan folder. The test walks
//! `papers/**/*.md`, plus `mindmap.md` and `mindmap/**/*.md`, parses each
//! file with [`kovan::artifact::parse_document`] and prints:
//!
//! - files, artifacts and problems (blocks with `[kovan]` that failed);
//! - how many artifacts **re-render** (`toml::to_string_pretty` of the
//!   parsed `[kovan]` TOML) to the same text as their own fence;
//! - a digest over every artifact's (heading, level, line, re-rendered TOML,
//!   body) and every problem.
//!
//! Run it before and after a schema change: identical counts and an
//! identical digest mean the change did not alter how any existing note
//! parses or serialises. Nothing is written, and nothing from the folder is
//! copied into the repository; without the variable the test does nothing,
//! so CI is unaffected.
//!
//! # Results
//!
//! 2026-10-06, the maintainer's folder, before and after the #743 schema
//! additions: **39 files, 234 artifacts, 0 problems, 234/234 identical
//! re-renders, digest `43960595adad4454` both times.** The additions did not
//! change how any existing note parses or serialises. (Counts only; nothing
//! from the private folder is recorded.)

use std::path::{Path, PathBuf};

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().is_some_and(|x| x == "md") {
            out.push(p);
        }
    }
}

/// The text of the first fenced ```toml block at or after 1-based `line`.
fn fence_after(md: &str, line: usize) -> Option<String> {
    let lines: Vec<&str> = md.lines().collect();
    let mut i = line;
    while i < lines.len() && !lines[i].trim_start().starts_with("```toml") {
        i += 1;
    }
    let mut out = String::new();
    for l in lines.get(i + 1..)? {
        if l.trim_start().starts_with("```") {
            return Some(out);
        }
        out.push_str(l);
        out.push('\n');
    }
    None
}

#[test]
fn local_kovan_folder_parses_unchanged() {
    let Ok(root) = std::env::var("KOVAN_COMPAT_ROOT") else {
        eprintln!("KOVAN_COMPAT_ROOT unset; skipping the local read-only check");
        return;
    };
    let root = PathBuf::from(root);
    let mut files = Vec::new();
    walk(&root.join("papers"), &mut files);
    walk(&root.join("mindmap"), &mut files);
    if root.join("mindmap.md").is_file() {
        files.push(root.join("mindmap.md"));
    }
    files.sort();

    let (mut artifacts, mut problems, mut same) = (0usize, 0usize, 0usize);
    let mut digest = String::new();
    for f in &files {
        let md = std::fs::read_to_string(f).unwrap();
        let parsed = kovan::artifact::parse_document(&md);
        for a in &parsed.artifacts {
            artifacts += 1;
            let rendered = toml::to_string_pretty(&a.toml).unwrap();
            if fence_after(&md, a.line).as_deref() == Some(rendered.as_str()) {
                same += 1;
            }
            digest.push_str(&format!(
                "{}|{}|{}|{}|{}\n",
                a.heading, a.level, a.line, rendered, a.body
            ));
        }
        for p in &parsed.problems {
            problems += 1;
            eprintln!("problem: {}: {p}", f.strip_prefix(&root).unwrap().display());
            digest.push_str(&format!("problem {p}\n"));
        }
    }
    use sha2::Digest;
    let hash = sha2::Sha256::digest(digest.as_bytes());
    let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
    eprintln!(
        "files {} | artifacts {artifacts} | problems {problems} | \
         re-render identical {same}/{artifacts} | digest {}",
        files.len(),
        &hex[..16]
    );
}
