//! **Recent history** of each source file (GitHub #746, item 8): the last
//! [`HISTORY_LEN`] commits that touched it, from **one** `git log` pass over
//! the scope rather than one call per file.
//!
//! The command runs (see [`GIT_LOG_ARGS`])
//!
//! ```text
//! git -c core.quotepath=off log --no-merges --no-renames --no-color \
//!     --format=%x1e%H%x1f%aI%x1f%an%x1f%s --name-only HEAD -- <crate dirs>
//! ```
//!
//! and [`parse`] keeps, per path, the first [`HISTORY_LEN`] commits in the
//! order git prints them (newest first by commit date, a fixed traversal for
//! a given HEAD), so the result is deterministic for a given commit.
//! `--no-renames` makes a rename list both paths, independent of the
//! user's `diff.renames`; history is per path and does not follow a file
//! across a rename. Merge commits are skipped (they list no files without
//! `-m`). No diffs are stored: per-function history and the diff since a
//! review stamp are computed by the reader from the two raw files at the
//! two commits.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Commits kept per file.
pub const HISTORY_LEN: usize = 10;

/// The `git` arguments before the pathspec.
pub const GIT_LOG_ARGS: &[&str] = &[
    "-c",
    "core.quotepath=off",
    "log",
    "--no-merges",
    "--no-renames",
    "--no-color",
    "--format=%x1e%H%x1f%aI%x1f%an%x1f%s",
    "--name-only",
    "HEAD",
    "--",
];

/// One commit that touched a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitRef {
    /// Full hash.
    pub sha: String,
    /// Author date, ISO 8601 with offset as recorded.
    pub date: String,
    pub author: String,
    pub subject: String,
}

/// Path -> its newest [`HISTORY_LEN`] commits, from `git log` output in the
/// [`GIT_LOG_ARGS`] format.
pub fn parse(out: &str) -> BTreeMap<String, Vec<CommitRef>> {
    let mut map: BTreeMap<String, Vec<CommitRef>> = BTreeMap::new();
    for rec in out.split('\u{1e}') {
        let mut lines = rec.lines();
        let Some(head) = lines.next() else { continue };
        let f: Vec<&str> = head.splitn(4, '\u{1f}').collect();
        if f.len() != 4 {
            continue;
        }
        let c = CommitRef {
            sha: f[0].to_string(),
            date: f[1].to_string(),
            author: f[2].to_string(),
            subject: f[3].to_string(),
        };
        for path in lines.map(str::trim).filter(|l| !l.is_empty()) {
            let v = map.entry(path.to_string()).or_default();
            if v.len() < HISTORY_LEN && !v.iter().any(|x| x.sha == c.sha) {
                v.push(c.clone());
            }
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: three commits in `git log` order; `a.rs` is touched by
    /// all three, `b.rs` by one. The cap is checked with 12 commits.
    ///
    /// Result (2026-10-06): passes; order is git's (newest first).
    #[test]
    fn parses_and_caps() {
        let rec = |sha: &str, files: &str| {
            format!("\u{1e}{sha}\u{1f}2026-10-06T10:00:00+08:00\u{1f}A B\u{1f}subj: x {sha}\n\n{files}\n")
        };
        let out = format!(
            "{}{}{}",
            rec("c3", "a.rs\nb.rs"),
            rec("c2", "a.rs"),
            rec("c1", "a.rs")
        );
        let m = parse(&out);
        let a: Vec<&str> = m["a.rs"].iter().map(|c| c.sha.as_str()).collect();
        assert_eq!(a, vec!["c3", "c2", "c1"]);
        assert_eq!(m["b.rs"].len(), 1);
        assert_eq!(m["b.rs"][0].subject, "subj: x c3");
        assert_eq!(m["b.rs"][0].author, "A B");
        let many: String = (0..12).map(|k| rec(&format!("s{k:02}"), "z.rs")).collect();
        let m = parse(&many);
        assert_eq!(m["z.rs"].len(), HISTORY_LEN);
        assert_eq!(m["z.rs"][0].sha, "s00");
    }
}
