//! `kovan-cli stamps-check`, `stamps-levels` and `stamp`: the command-line
//! face of [`crate::review_stamps`] (GitHub #739).
//!
//! - `stamps-check [--workspace DIR] [--diff A..B]` prints one line per
//!   stamp in scope (VALID, VOID with the reason and the stamped permalink,
//!   STALE when a later valid stamp supersedes it, UNCHECKED for artifact
//!   stamps) and exits non-zero when any stamp in scope is VOID. It never
//!   edits `review/stamps.toml`.
//! - `stamps-levels <crate>` reports which human-rung maturity claims in the
//!   crate's `Cargo.toml` tag are supported by valid stamps. Report only.
//! - `stamp <function> --rung N --reviewer NAME --i-am-the-reviewer` creates
//!   a stamp. **AI agents must never run it**: a stamp records a human
//!   review and is the maintainer's alone. Without `--i-am-the-reviewer` it
//!   refuses.

use std::path::Path;

use crate::review_stamps::{self, Scope};

/// `stamps-check`: `Err` (a non-zero exit) when a stamp in scope is void.
pub fn run_check(root: &Path, diff: Option<String>, repo_url: &str) -> Result<(), String> {
    let scope = match diff {
        Some(r) => Scope::Diff(r),
        None => Scope::All,
    };
    let report = review_stamps::check(root, &scope)?;
    print!("{}", review_stamps::render_report(&report, repo_url));
    let fails = report.failures().count();
    if fails > 0 {
        return Err(format!(
            "{fails} review stamp(s) void; re-review and re-stamp, or accept them as stale"
        ));
    }
    Ok(())
}

/// `stamps-levels`.
pub fn run_levels(root: &Path, crate_name: &str) -> Result<(), String> {
    let d = review_stamps::derived_levels(root, crate_name)?;
    print!("{}", review_stamps::levels::render(&d));
    Ok(())
}

/// `stamp`. Refuses unless `i_am_the_reviewer`.
pub fn run_stamp(
    root: &Path,
    function: &str,
    rung: u8,
    reviewer: &str,
    note: &str,
    i_am_the_reviewer: bool,
) -> Result<(), String> {
    if !i_am_the_reviewer {
        return Err(
            "refusing to stamp: a stamp records a HUMAN review. Only the reviewer may \
                    create one, by passing --i-am-the-reviewer. AI agents must never stamp."
                .into(),
        );
    }
    let s = review_stamps::stamp_function(root, function, rung, reviewer, note)?;
    println!(
        "stamped {} at rung {} ({}, lines {}-{}, {}) in {}",
        s.target_name(),
        s.rung,
        s.hash,
        s.lines[0],
        s.lines[1],
        &s.commit[..s.commit.len().min(10)],
        review_stamps::STAMPS_FILE
    );
    Ok(())
}
