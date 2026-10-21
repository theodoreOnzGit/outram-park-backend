//! **New functions, folded per commit** (GitHub #771; maintainer
//! 2026-10-07: "new functions from agents appear as one folded row per
//! commit, e.g. ＋ 140 new functions · b81e · 31 untested [Expand ▾] [Open
//! on map ▸]").
//!
//! ```text
//!  git log (newest first, no merges, at most `max_commits`)
//!     │ for each commit C: the .rs files it added, modified or renamed
//!     │ (git diff-tree), parsed at C and at C^ with the index's syn hasher
//!     v
//!  Introduced { commit, functions: {(file, qualified name)} }   <- git side: scan()
//!     │ + the current functions the engine judges new (never reviewed)
//!     v
//!  fold(): each new function goes to the NEWEST commit that introduced
//!  it; one NewCommit per commit with its count, untested count and the
//!  commit's own authorship (trailer)                            <- pure
//! ```
//!
//! **The window is the last `max_commits` non-merge commits** (default 50,
//! [`super::queue::QueueOptions`]). A function introduced before the window
//! (or never committed) is in no folded row: it is still unreviewed, and
//! walks find it; the queue shows what arrived recently. A merge commit is
//! skipped because the functions it brings in were introduced by the
//! branch's own commits, which the log lists.
//!
//! A function counts as introduced by `C` when its qualified name (the
//! call graph's `#k` twin index dropped) appears in the file at `C` but not
//! at `C^`: a rename shows as new under its new name, an edit does not.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use kovan_common::review::hash::hash_functions;
use kovan_common::review::types::{agent_trailer, authorship_from_messages, ChangeAuthorship};

use super::git;

/// A commit and the functions it introduced, as `(file, qualified name)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Introduced {
    pub commit: String,
    /// The first line of its message.
    pub summary: String,
    /// The whole message (for the trailer).
    pub message: String,
    pub functions: BTreeSet<(String, String)>,
}

/// A current function the engine judges new.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewFn {
    /// `fn:` id.
    pub id: String,
    /// Workspace-relative file.
    pub file: String,
    /// The index's `qual` (may carry `#k`).
    pub qual: String,
    /// No test reaches it.
    pub untested: bool,
}

/// One folded row's data: the new functions one commit introduced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCommit {
    pub commit: String,
    pub summary: String,
    /// From the commit's trailer: agent (with its session) or human.
    pub authorship: Option<ChangeAuthorship>,
    /// The new functions' ids, sorted.
    pub functions: Vec<String>,
    /// How many of them no test reaches.
    pub untested: usize,
}

/// `qual` without the `#k` twin index, whitespace squashed (as the
/// hasher's `qualname`).
fn base_qual(qual: &str) -> String {
    let base = match qual.rsplit_once('#') {
        Some((b, k)) if !k.is_empty() && k.bytes().all(|c| c.is_ascii_digit()) => b,
        _ => qual,
    };
    base.split_whitespace().collect()
}

/// Fold the new functions into one [`NewCommit`] per commit (module doc).
/// `commits` is newest first; a function goes to the first (newest) commit
/// that introduced it. Commits that introduced none of `new` are left out.
/// Output order is `commits`' order.
pub fn fold(new: &[NewFn], commits: &[Introduced]) -> Vec<NewCommit> {
    let mut by_commit: BTreeMap<usize, Vec<&NewFn>> = BTreeMap::new();
    for f in new {
        let key = (f.file.clone(), base_qual(&f.qual));
        if let Some(i) = commits.iter().position(|c| c.functions.contains(&key)) {
            by_commit.entry(i).or_default().push(f);
        }
    }
    by_commit
        .into_iter()
        .map(|(i, fns)| {
            let c = &commits[i];
            let mut functions: Vec<String> = fns.iter().map(|f| f.id.clone()).collect();
            functions.sort();
            functions.dedup();
            NewCommit {
                commit: c.commit.clone(),
                summary: c.summary.clone(),
                authorship: authorship_from_messages(std::slice::from_ref(&c.message)),
                untested: fns.iter().filter(|f| f.untested).count(),
                functions,
            }
        })
        .collect()
}

/// The qualified names of the functions in `file` at `rev` (empty when the
/// file is absent there or does not parse).
fn names_at(
    cache: &mut BTreeMap<(String, String), BTreeSet<String>>,
    root: &Path,
    rev: &str,
    file: &str,
) -> BTreeSet<String> {
    cache
        .entry((rev.to_string(), file.to_string()))
        .or_insert_with(|| {
            git::show(root, rev, file)
                .and_then(|t| hash_functions(&t).ok())
                .map(|fns| {
                    fns.iter()
                        .map(|h| h.entry.qualname().split_whitespace().collect())
                        .collect()
                })
                .unwrap_or_default()
        })
        .clone()
}

/// The last `max_commits` non-merge commits up to `head`, newest first,
/// each with the functions it introduced (module doc). Empty when there is
/// no git history.
pub fn scan(root: &Path, head: &str, max_commits: usize) -> Vec<Introduced> {
    if head.is_empty() || max_commits == 0 {
        return Vec::new();
    }
    let n = format!("-n{max_commits}");
    let Ok(log) = git::git(
        root,
        &[
            "log",
            "--no-merges",
            &n,
            "--format=%x1e%H%x1f%P%x1f%B",
            head,
        ],
    ) else {
        return Vec::new();
    };
    let mut cache = BTreeMap::new();
    let mut out = Vec::new();
    for rec in log.split('\x1e').filter(|r| !r.trim().is_empty()) {
        let mut parts = rec.splitn(3, '\x1f');
        let commit = parts.next().unwrap_or("").trim().to_string();
        let parent = parts
            .next()
            .unwrap_or("")
            .split_whitespace()
            .next()
            .map(str::to_string);
        let message = parts.next().unwrap_or("").trim().to_string();
        if commit.is_empty() {
            continue;
        }
        let files = git::git(
            root,
            &[
                "diff-tree",
                "--no-commit-id",
                "--root",
                "-r",
                "--name-only",
                "--diff-filter=AMR",
                &commit,
            ],
        )
        .unwrap_or_default();
        let mut functions = BTreeSet::new();
        for file in files.lines().map(str::trim).filter(|f| f.ends_with(".rs")) {
            let now = names_at(&mut cache, root, &commit, file);
            let before = match &parent {
                Some(p) => names_at(&mut cache, root, p, file),
                None => BTreeSet::new(),
            };
            for q in now.difference(&before) {
                functions.insert((file.to_string(), q.clone()));
            }
        }
        out.push(Introduced {
            summary: message.lines().next().unwrap_or("").to_string(),
            message,
            commit,
            functions,
        });
    }
    out
}

/// Whether the commit message carries the agent attribution trailer.
pub fn is_agent_commit(message: &str) -> bool {
    agent_trailer(message).0
}

#[cfg(test)]
mod tests {
    use super::*;
    use kovan_common::review::types::AuthorshipKind;

    fn intro(commit: &str, message: &str, fns: &[(&str, &str)]) -> Introduced {
        Introduced {
            commit: commit.into(),
            summary: message.lines().next().unwrap_or("").into(),
            message: message.into(),
            functions: fns
                .iter()
                .map(|(f, q)| (f.to_string(), q.to_string()))
                .collect(),
        }
    }

    fn new_fn(id: &str, file: &str, qual: &str, untested: bool) -> NewFn {
        NewFn {
            id: id.into(),
            file: file.into(),
            qual: qual.into(),
            untested,
        }
    }

    /// Methodology: three new functions, two commits (newest first) where
    /// both introduced `a` (it was deleted and re-added) and the older one
    /// introduced `b` and the `#2` twin of `T::f`. Pass: `a` goes to the
    /// newest commit; the older commit gets `b` and the twin (the `#k`
    /// index ignored); untested counts and the trailer's authorship (agent
    /// with session, human) are carried; a function no commit introduced is
    /// in no row.
    #[test]
    fn new_functions_fold_into_the_newest_commit_that_introduced_them() {
        let agent = "add pools\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_x";
        let commits = vec![
            intro("c2", agent, &[("a.rs", "a")]),
            intro(
                "c1",
                "human work",
                &[("a.rs", "a"), ("a.rs", "b"), ("a.rs", "T::f")],
            ),
        ];
        let new = vec![
            new_fn("fn:a", "a.rs", "a", true),
            new_fn("fn:b", "a.rs", "b", false),
            new_fn("fn:f2", "a.rs", "T::f#2", true),
            new_fn("fn:old", "a.rs", "old", true),
        ];
        let rows = fold(&new, &commits);
        assert_eq!(rows.len(), 2);
        assert_eq!(
            (
                rows[0].commit.as_str(),
                rows[0].functions.clone(),
                rows[0].untested
            ),
            ("c2", vec!["fn:a".to_string()], 1)
        );
        let a = rows[0].authorship.clone().unwrap();
        assert_eq!(a.kind, AuthorshipKind::Agent);
        assert_eq!(
            a.sessions,
            vec!["https://claude.ai/code/session_x".to_string()]
        );
        assert_eq!(
            rows[1].functions,
            vec!["fn:b".to_string(), "fn:f2".to_string()]
        );
        assert_eq!(rows[1].untested, 1);
        assert_eq!(
            rows[1].authorship.as_ref().unwrap().kind,
            AuthorshipKind::Human
        );
        assert!(is_agent_commit(agent) && !is_agent_commit("human work"));
    }
}
