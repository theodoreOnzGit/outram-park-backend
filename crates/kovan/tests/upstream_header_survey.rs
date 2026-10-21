//! Survey of attribution headers across the whole workspace with the call
//! graph's parser (`kovan::call_graph::upstream::scan`, GitHub #746 part 2).
//!
//! **Methodology.** Every `.rs` file under `crates/*/{src,examples,tests}`
//! of the workspace this crate sits in (vendored `upstream_source/`,
//! `vendor/` and `target/` skipped) is scanned; the test prints, per crate,
//! files / parsed / with a URL / unparsed, then the distinct
//! `(project, repository?, commit?)` triples and every unparsed reason. It
//! asserts nothing about the counts (they move as files are added); it is a
//! measuring instrument, `#[ignore]`d so the suite stays quick:
//!
//! ```text
//! cargo test --release -j 6 -p knowledge-oriented-vv-analysis-for-nuclear-sciences-kovan --test upstream_header_survey -- --ignored --nocapture
//! ```
//!
//! **Result (2026-10-06, develop at 2f2cf7d599):** recorded in
//! `crates/kovan/DECISIONS.md`, "Tests that reach a function, upstream
//! counterparts, citing pages and history (schema 2)".

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kovan::call_graph::upstream::{scan, Scan};

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if p.is_dir() {
            if !matches!(name.as_str(), "target" | "vendor" | "upstream_source")
                && !name.starts_with('.')
            {
                rs_files(&p, out);
            }
        } else if name.ends_with(".rs") {
            out.push(p);
        }
    }
}

#[test]
#[ignore = "measuring instrument; run with --ignored --nocapture"]
fn survey_attribution_headers() {
    let crates_dir = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut per_crate: BTreeMap<String, [usize; 4]> = BTreeMap::new();
    let mut triples: BTreeMap<String, usize> = BTreeMap::new();
    let mut unparsed: Vec<String> = Vec::new();
    let mut names: Vec<_> = std::fs::read_dir(crates_dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    names.sort();
    for c in names {
        let krate = c.file_name().unwrap().to_string_lossy().to_string();
        let mut files = Vec::new();
        for sub in ["src", "examples", "tests"] {
            rs_files(&c.join(sub), &mut files);
        }
        files.sort();
        for f in files {
            let Ok(text) = std::fs::read_to_string(&f) else {
                continue;
            };
            let lines: Vec<String> = text.lines().map(str::to_string).collect();
            let e = per_crate.entry(krate.clone()).or_default();
            e[0] += 1;
            match scan(&lines) {
                Scan::None => {}
                Scan::Parsed(u) => {
                    e[1] += 1;
                    e[2] += usize::from(u.url.is_some());
                    *triples
                        .entry(format!(
                            "{:?} | {} | {} | {} | url={}",
                            u.style,
                            u.project.unwrap_or_default(),
                            u.repository.unwrap_or_default(),
                            u.commit.unwrap_or_default(),
                            u.url.is_some()
                        ))
                        .or_default() += 1;
                }
                Scan::Unparsed(why) => {
                    e[3] += 1;
                    let rel = f.strip_prefix(crates_dir).unwrap().display().to_string();
                    unparsed.push(format!(
                        "{rel}: {}",
                        why.chars().take(120).collect::<String>()
                    ));
                }
            }
        }
    }
    let mut tot = [0usize; 4];
    println!("crate | files | parsed | url | unparsed");
    for (k, v) in &per_crate {
        if v[1] + v[3] > 0 {
            println!("{k} | {} | {} | {} | {}", v[0], v[1], v[2], v[3]);
        }
        for i in 0..4 {
            tot[i] += v[i];
        }
    }
    println!("TOTAL | {} | {} | {} | {}", tot[0], tot[1], tot[2], tot[3]);
    println!("\n(style | project | repository | commit | url) -> files");
    for (k, v) in &triples {
        println!("{v:5} {k}");
    }
    println!("\nunparsed:");
    for u in &unparsed {
        println!("  {u}");
    }
}
