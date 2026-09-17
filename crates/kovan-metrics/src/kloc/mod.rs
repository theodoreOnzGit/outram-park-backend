//! `kovan kloc` — the productivity accounting behind the Outram Park paper.
//!
//! Reproduces the tables and figure the manuscript reports: how many lines the
//! pre-agentic repositories hold and over how many active days, how many lines
//! the agentic month produced, and what that output is made of.
//!
//! # Why this exists at all
//!
//! The manuscript's tables were originally compiled with AI assistance from
//! repository measurements. This code exists so those numbers can be
//! **re-derived from the repositories themselves, by anyone**, without trusting
//! that summary — which is what a journal asks for when a table or figure is
//! "directly derived from underlying data using reproducible analytical,
//! computational, or statistical methods".
//!
//! It is a Rust port of the retired `scripts/kloc_accounting.py`, which was
//! deleted under the workspace's "no Python for documentation or accounting"
//! rule. **The port is gated on byte-for-byte parity** with that script's
//! output, frozen in `docs/kloc-parity-baseline/` — a capture that reproduces
//! every published figure exactly, all eight drift-check deltas `+0`.
//!
//! # Source of truth
//!
//! The git repositories. Nothing is estimated and nothing is hard-coded from
//! the manuscript. The single editorial input is the classification of each
//! crate as translated, original or an extension, which lives in [`config`]
//! beside each crate's own `Cargo.toml` description so it can be audited.
//!
//! The one set of numbers copied from the manuscript,
//! [`MANUSCRIPT`](config::MANUSCRIPT), is used **only** to report drift and
//! never in a computation.
//!
//! # The trap worth knowing about
//!
//! An extension crate subtracts its standalone pre-agentic original, so a
//! **missing baseline repository silently moves those lines into the agentic
//! total** rather than merely shrinking the baseline. Measured on this
//! workspace, three absent repositories moved the baseline from 181,298 to
//! 162,163 code lines and the agentic total from 175,997 to 187,378. The report
//! refuses to validate its drift check while any repository is missing; do not
//! quote a run that carries that warning.

pub mod config;
pub mod figure;
pub mod measure;
pub mod outputs;
pub mod repo;
pub mod report;
pub mod source;

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

pub use config::{Provenance, AGENTIC_KEY, AGENTIC_MEASURE_REF, BASELINE_REPOS, GITHUB_USER};
pub use measure::{
    measure_agentic, measure_baseline, AgenticSummary, BaselineTotals, CrateStats, RepoStats,
};
pub use report::report;
pub use source::{strip_rust_comments, LineCount, SKIP_DIRS};

/// Where to look for an already-present checkout before falling back to the
/// vendor directory.
///
/// Relative to the user's home directory. A checkout the author already has is
/// preferred over a fresh clone because it is what they are actually working
/// in — and it is never written to.
pub const REPO_SEARCH_SUBDIRS: &[&str] = &["Documents/research", "Desktop"];

/// How a run is configured.
#[derive(Clone, Debug)]
pub struct Options {
    /// Where the CSVs, LaTeX tables, summary and figure are written.
    pub out_dir: PathBuf,
    /// Directories searched for existing checkouts, in order.
    pub search_dirs: Vec<PathBuf>,
    /// Where cloned repositories live, and the last place searched.
    pub vendor_dir: PathBuf,
    /// Ignore local checkouts entirely and measure only the vendor clones.
    ///
    /// This is the **reproduction path**: it needs nothing on the machine but
    /// git and network access, so a reader's run cannot accidentally pick up
    /// the author's working copies.
    pub github_only: bool,
    /// Add the drift comparison against the manuscript's published figures.
    pub check: bool,
    /// Emit the SVG figure.
    pub figure: bool,
}

impl Options {
    /// Defaults writing into `out_dir`, searching the author's usual locations.
    pub fn new(out_dir: PathBuf) -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        Self {
            search_dirs: REPO_SEARCH_SUBDIRS
                .iter()
                .map(|sub| home.join(sub))
                .collect(),
            vendor_dir: out_dir.join("vendor"),
            out_dir,
            github_only: false,
            check: false,
            figure: true,
        }
    }
}

/// What a run produced.
#[derive(Clone, Debug)]
pub struct Outcome {
    /// The console report, also written to `summary.txt`.
    pub report: String,
    /// Files written, in the order they were written.
    pub written: Vec<PathBuf>,
    /// Repositories that could not be found.
    pub missing: Vec<String>,
}

/// The clone URL for a repository key.
pub fn repo_url(key: &str) -> String {
    format!("https://github.com/{GITHUB_USER}/{key}.git")
}

/// Locate a checkout of `key`, or `None`.
///
/// Searches `search_dirs` in order, then `vendor_dir`. With `github_only`, only
/// the vendor directory is consulted.
pub fn find_repo(options: &Options, key: &str) -> Option<PathBuf> {
    let mut bases: Vec<&Path> = Vec::new();
    if !options.github_only {
        bases.extend(options.search_dirs.iter().map(PathBuf::as_path));
    }
    bases.push(&options.vendor_dir);
    for base in bases {
        let candidate = base.join(key);
        if candidate.join(".git").exists() {
            return Some(candidate);
        }
    }
    None
}

/// Every repository the accounting needs, baseline and agentic.
pub fn all_repo_keys() -> Vec<&'static str> {
    BASELINE_REPOS
        .iter()
        .map(|r| r.key)
        .chain(std::iter::once(AGENTIC_KEY))
        .collect()
}

/// Resolve every repository to a path, where one can be found.
pub fn resolve(options: &Options) -> BTreeMap<String, PathBuf> {
    all_repo_keys()
        .into_iter()
        .filter_map(|key| find_repo(options, key).map(|path| (key.to_string(), path)))
        .collect()
}

/// Measure, render, and write every artifact.
pub fn run(options: &Options) -> io::Result<Outcome> {
    let paths = resolve(options);

    let (base_stats, base_totals) = measure_baseline(BASELINE_REPOS, &paths);
    let by_key: BTreeMap<String, RepoStats> = base_stats
        .iter()
        .map(|s| (s.key.clone(), s.clone()))
        .collect();
    let (crate_rows, agentic) = measure_agentic(
        paths.get(AGENTIC_KEY).map(PathBuf::as_path),
        AGENTIC_MEASURE_REF,
        &by_key,
    );

    let text = report(
        &base_stats,
        &base_totals,
        &crate_rows,
        &agentic,
        options.check,
    );

    std::fs::create_dir_all(&options.out_dir)?;
    let mut written = Vec::new();
    let write = |name: &str, body: &str, written: &mut Vec<PathBuf>| -> io::Result<()> {
        let path = options.out_dir.join(name);
        std::fs::write(&path, body)?;
        written.push(path);
        Ok(())
    };

    // Order matches the Python's, so a reader following the console output sees
    // the same sequence.
    write("summary.txt", &format!("{text}\n"), &mut written)?;
    write(
        "baseline_repositories.csv",
        &outputs::baseline_csv(&base_stats),
        &mut written,
    )?;
    write(
        "agentic_crates.csv",
        &outputs::agentic_csv(&crate_rows),
        &mut written,
    )?;
    write(
        "baseline_table.tex",
        &outputs::baseline_table_tex(&base_stats, &base_totals),
        &mut written,
    )?;
    write(
        "rate_table.tex",
        &outputs::rate_table_tex(&base_stats, &base_totals, &agentic),
        &mut written,
    )?;
    if !agentic.missing {
        write(
            "agentic_table.tex",
            &outputs::agentic_table_tex(&crate_rows, &agentic),
            &mut written,
        )?;
    }
    if options.figure && !agentic.missing {
        write(
            "fig_kloc_productivity.svg",
            &figure::productivity_svg(&base_stats, &base_totals, &agentic),
            &mut written,
        )?;
    }

    Ok(Outcome {
        report: text,
        written,
        missing: base_totals.missing.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clone_url_points_at_the_authors_account() {
        assert_eq!(
            repo_url("teh-o-prke"),
            "https://github.com/theodoreOnzGit/teh-o-prke.git"
        );
    }

    #[test]
    fn every_repository_the_accounting_needs_is_enumerated() {
        let keys = all_repo_keys();
        assert_eq!(keys.len(), BASELINE_REPOS.len() + 1);
        assert!(keys.contains(&AGENTIC_KEY));
        for spec in BASELINE_REPOS {
            assert!(keys.contains(&spec.key));
        }
    }

    /// The reproduction path must not silently pick up the author's working
    /// copies -- that is the whole point of `--from-github`.
    #[test]
    fn github_only_ignores_local_checkouts() {
        let tmp = tempfile::tempdir().unwrap();
        let local = tmp.path().join("local");
        std::fs::create_dir_all(local.join("teh-o-prke/.git")).unwrap();

        let mut options = Options::new(tmp.path().join("out"));
        options.search_dirs = vec![local];
        options.vendor_dir = tmp.path().join("vendor");

        assert!(find_repo(&options, "teh-o-prke").is_some());
        options.github_only = true;
        assert!(
            find_repo(&options, "teh-o-prke").is_none(),
            "--from-github must consult only the vendor directory"
        );
    }

    #[test]
    fn a_run_with_no_repositories_still_writes_its_artifacts() {
        let tmp = tempfile::tempdir().unwrap();
        let mut options = Options::new(tmp.path().join("out"));
        options.search_dirs = vec![tmp.path().join("nowhere")];
        options.vendor_dir = tmp.path().join("nowhere");

        let outcome = run(&options).unwrap();
        assert_eq!(outcome.missing.len(), BASELINE_REPOS.len());
        assert!(outcome.report.contains("MISSING"));
        // The agentic table and figure are skipped when the repository is
        // absent; the rest are still written.
        let names: Vec<String> = outcome
            .written
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&"summary.txt".to_string()));
        assert!(names.contains(&"baseline_repositories.csv".to_string()));
        assert!(!names.contains(&"agentic_table.tex".to_string()));
    }
}
