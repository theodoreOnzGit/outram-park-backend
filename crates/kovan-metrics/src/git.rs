//! The git queries this crate needs, and nothing more.
//!
//! **Reuse note (workspace search-first rule).** Repository-root discovery goes
//! through [`kovan_discovery::git::GitProvider`], which already wraps gitoxide —
//! this crate does not open a second git layer for that. What `kovan-discovery`
//! does *not* expose is arbitrary `git log --format=…` / `--numstat` queries,
//! because its `CommitInfo` is a fixed typed record with no message body and no
//! diff statistics. Those two needs are served here by [`git_output`], a thin
//! wrapper over the `git` binary.
//!
//! **Why the binary rather than gitoxide for those.** This code runs inside
//! `prepare-commit-msg` and `post-commit`, so `git` is by construction present
//! and already in the process's environment. Shelling out also keeps the
//! rename-compaction and date-window semantics byte-identical to the Python
//! implementation being replaced, rather than re-deriving them.
//!
//! Every helper here **degrades to empty output rather than failing**. That is
//! deliberate and load-bearing: the hook path must never block a commit.

use std::path::{Path, PathBuf};
use std::process::Command;

/// ASCII unit separator — delimits fields within one commit record.
pub const FS: char = '\u{1f}';
/// ASCII record separator — delimits commit records from each other.
pub const RS: char = '\u{1e}';

/// Run `git` with `args` and return its stdout, or an empty string on any
/// failure (git missing, non-zero exit, non-UTF-8 output).
///
/// Mirrors the Python `git()` helper: errors are swallowed, never raised.
pub fn git_output(args: &[&str]) -> String {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

/// Absolute path to the repository working-tree root.
///
/// Tries `kovan-discovery`'s gitoxide-backed discovery first (the workspace's
/// canonical git layer), then falls back to `git rev-parse --show-toplevel`,
/// then to the current directory.
pub fn repo_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if let Ok(provider) = kovan_discovery::git::GitProvider::open(&cwd) {
        return provider.repo_root().to_path_buf();
    }
    let out = git_output(&["rev-parse", "--show-toplevel"]);
    let trimmed = out.trim();
    if trimmed.is_empty() {
        cwd
    } else {
        PathBuf::from(trimmed)
    }
}

/// Absolute path to the `.git` directory (resolved against the repo root when
/// git reports it relatively, as it does from inside a hook).
pub fn git_dir() -> PathBuf {
    let out = git_output(&["rev-parse", "--git-dir"]);
    let raw = out.trim();
    let raw = if raw.is_empty() { ".git" } else { raw };
    let p = Path::new(raw);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        repo_root().join(p)
    }
}

/// Resolve a branch name to the ref that should be reported on, preferring the
/// remote-tracking ref so a report reflects what is actually published.
///
/// Tries `origin/<branch>`, then `<branch>`, and returns `<branch>` unchanged
/// when neither resolves (letting git produce the eventual error).
pub fn ref_for_branch(branch: &str) -> String {
    for cand in [format!("origin/{branch}"), branch.to_string()] {
        if !git_output(&["rev-parse", "--verify", "--quiet", &cand])
            .trim()
            .is_empty()
        {
            return cand;
        }
    }
    branch.to_string()
}

/// One commit as this crate reads it: enough to attribute tokens and render a
/// ledger row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitRecord {
    /// Abbreviated commit hash (`%h`).
    pub short: String,
    /// Author date as `YYYY-MM-DD` — the first 10 characters of `%aI`.
    pub date: String,
    /// Commit subject, the first line of the message (`%s`).
    pub subject: String,
    /// Commit message body (`%b`) — where the `API-Usage-*` trailers live.
    pub body: String,
}

/// Parse `git log`/`git show` output formatted as
/// `%h<FS>%aI<FS>%s<FS>%b<RS>` into records.
///
/// Records with fewer than three fields are skipped. Anything after the third
/// separator is re-joined into `body`, so a body containing a literal unit
/// separator cannot truncate the record.
pub fn parse_records(raw: &str) -> Vec<CommitRecord> {
    let mut out = Vec::new();
    for chunk in raw.split(RS) {
        let chunk = chunk.trim_matches('\n').trim();
        if chunk.is_empty() {
            continue;
        }
        let parts: Vec<&str> = chunk.split(FS).collect();
        if parts.len() < 3 {
            continue;
        }
        out.push(CommitRecord {
            short: parts[0].to_string(),
            date: parts[1].chars().take(10).collect(),
            subject: parts[2].to_string(),
            body: if parts.len() > 3 {
                parts[3..].join(&FS.to_string())
            } else {
                String::new()
            },
        });
    }
    out
}

/// The `--format` string that [`parse_records`] expects.
pub fn record_format() -> String {
    format!("--format=%h{FS}%aI{FS}%s{FS}%b{RS}")
}

/// Per-commit diff statistics, as used by the historian report.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NumStat {
    /// Lines added across all text files.
    pub added: u64,
    /// Lines removed across all text files.
    pub removed: u64,
    /// Lines added in `.rs` files only.
    pub rs_added: u64,
    /// Lines removed in `.rs` files only.
    pub rs_removed: u64,
    /// Lines added, keyed by the `crates/<name>/` directory they landed in.
    pub per_crate_added: Vec<(String, u64)>,
}

/// Normalise git's rename compaction to the **new** path.
///
/// git writes renames two ways in `--numstat`:
/// `crates/{old => new}/f.rs` and `old/path/f => new/path/f`. Both must resolve
/// to the post-rename path or the per-crate attribution lands on the old crate.
fn normalise_renamed_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    let mut rest = path;
    // Replace every `{... => X}` group with `X`.
    while let Some(open) = rest.find('{') {
        let Some(close_rel) = rest[open..].find('}') else {
            break;
        };
        let close = open + close_rel;
        let inner = &rest[open + 1..close];
        out.push_str(&rest[..open]);
        match inner.split("=>").nth(1) {
            Some(new) => out.push_str(new.trim()),
            None => out.push_str(inner),
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    // Then the un-braced `old => new` form.
    if let Some(idx) = out.find(" => ") {
        out = out[idx + 4..].to_string();
    }
    // Collapse any doubled slashes the substitution introduced.
    while out.contains("//") {
        out = out.replace("//", "/");
    }
    out.trim().to_string()
}

/// Diff statistics for a single commit, from `git show --numstat`.
///
/// Binary files (which git reports as `-` / `-`) are skipped rather than
/// counted as zero-line changes.
pub fn numstat(sha: &str) -> NumStat {
    let raw = git_output(&["show", "--numstat", "--format=", sha]);
    let mut stat = NumStat::default();
    let mut per_crate: Vec<(String, u64)> = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 3 {
            continue;
        }
        let (Ok(a), Ok(r)) = (cols[0].parse::<u64>(), cols[1].parse::<u64>()) else {
            continue; // binary file: git prints "-"
        };
        let path = normalise_renamed_path(cols[2]);
        stat.added += a;
        stat.removed += r;
        if path.ends_with(".rs") {
            stat.rs_added += a;
            stat.rs_removed += r;
        }
        if let Some(name) = path
            .strip_prefix("crates/")
            .and_then(|p| p.split('/').next())
        {
            if !name.is_empty() {
                match per_crate.iter_mut().find(|(k, _)| k == name) {
                    Some((_, v)) => *v += a,
                    None => per_crate.push((name.to_string(), a)),
                }
            }
        }
    }
    stat.per_crate_added = per_crate;
    stat
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_records_and_keeps_multiline_bodies() {
        let raw = format!(
            "abc1234{FS}2026-08-13T10:00:00+08:00{FS}subject one{FS}body line 1\nbody line 2{RS}\
             def5678{FS}2026-08-12T09:00:00+08:00{FS}subject two{FS}{RS}"
        );
        let recs = parse_records(&raw);
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].short, "abc1234");
        assert_eq!(recs[0].date, "2026-08-13");
        assert_eq!(recs[0].subject, "subject one");
        assert!(recs[0].body.contains("body line 2"));
        assert_eq!(recs[1].body, "");
    }

    #[test]
    fn a_body_containing_a_unit_separator_does_not_truncate_the_record() {
        let raw = format!("abc{FS}2026-08-13{FS}subj{FS}has{FS}separator{RS}");
        let recs = parse_records(&raw);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].body, format!("has{FS}separator"));
    }

    #[test]
    fn skips_malformed_and_empty_records() {
        let raw = format!("{RS}too{FS}few{RS}");
        assert!(parse_records(&raw).is_empty());
    }

    #[test]
    fn normalises_both_rename_forms_to_the_new_path() {
        assert_eq!(
            normalise_renamed_path("crates/{old => new}/f.rs"),
            "crates/new/f.rs"
        );
        assert_eq!(
            normalise_renamed_path("old/path/f.rs => new/path/f.rs"),
            "new/path/f.rs"
        );
        assert_eq!(
            normalise_renamed_path("crates/a/src/lib.rs"),
            "crates/a/src/lib.rs"
        );
    }

    #[test]
    fn rename_into_a_new_crate_attributes_to_the_new_crate() {
        // The failure this guards: counting the lines against `old-crate`.
        let p = normalise_renamed_path("crates/{old-crate => new-crate}/src/lib.rs");
        assert_eq!(p, "crates/new-crate/src/lib.rs");
        assert_eq!(
            p.strip_prefix("crates/").unwrap().split('/').next(),
            Some("new-crate")
        );
    }
}
