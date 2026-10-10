//! **Per-function stamp states from `review.md`** (GitHub #765, #770): the
//! native side of the staleness engine. Builds [`GitFacts`] with `git`
//! subprocess calls (nothing in the workspace built them before
//! 2026-10-10), runs [`kovan_common::review::engine::evaluate`] with the
//! registry of `kovan_root.toml` and signatures enforced, and maps the
//! result onto the [`StampState`] rows kovan-web's `Facts::new` reads, keyed
//! by call-graph id.
//!
//! # What git gives, field by field
//!
//! | `GitFacts` field | how |
//! |---|---|
//! | `head` | `git rev-parse HEAD` |
//! | `cargo_lock` | `sha256:` of `Cargo.lock` in the working tree |
//! | `stamps[..].added_in` | the newest commit whose diff changes the number of occurrences of the stamp's signature value in its `review.md` (`git log -1 -S<value>`), taken only when `HEAD`'s `review.md` holds it (else not committed); `after_certified` is "not the certified commit and a descendant of it" (`git merge-base --is-ancestor`); the trailer check is [`agent_trailer`] on its message |
//! | `stamps[..].hash_at_commit` | the function at the review's recorded `path`, re-hashed from `git show <commit>:<file>` with the same `syn` hasher |
//! | `stamps[..].tests_at_review` | `reached_by` of the function in the folder's `kovan.toml` **as committed at the review commit**, and for each test the messages of the commits up to it that touched the test's file (file-level: coarser than the function, so an agent commit anywhere in the file counts, the conservative side) |
//! | `hashes_at_test_run` | every function of a folder with `[test_run]`, re-hashed at the run's commit |
//! | `code_authors` | for reviewed functions only: `git blame` author e-mails of the function's current lines, plus the reviewer ids they map to (the same e-mail, or `github:<user>` for a `users.noreply.github.com` address; matched case-insensitively against the registry) |
//! | `previous_root` | `HEAD`'s `kovan_root.toml` when the working copy differs from it, else the version before the last commit that changed it |
//! | `deleted_in`, `publish_records`, `tag_commits` | left empty: deleted functions show without a deleting commit, there is no #773 record yet, and tags show as unchecked (offline) |
//!
//! [`ConceptAreas`] is passed **empty** (resolving a function's concept
//! areas from its reviews' `implements` relations is not built): no
//! function reaches rung 5 here, which the engine documents as the result
//! for a function with no known concept area. Conservative, never a silent
//! upgrade.
//!
//! A stamp without a signature has no pickaxe string and is never found
//! in git (unverified, "no git facts"); with signatures enforced it would be
//! unverified anyway.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use kovan_common::call_graph::split::{StampState as WebStamp, StampVerdict};
use kovan_common::review::engine::{
    evaluate, ConceptAreas, Evaluation, FolderReviews, GitFacts, ReviewKey, SignaturePolicy,
    StampCommit, StampFacts, StampState, TestsAtReview,
};
use kovan_common::review::hash::{hash_functions, HashedFn};
use kovan_common::review::index::FolderIndex;
use kovan_common::review::ivv_view::summarise;
use kovan_common::review::review_md::ReviewEntry;
use kovan_common::review::root::ReviewRoot;
use kovan_common::review::state::StateKind;
use kovan_common::review::types::agent_trailer;

use super::{
    call_graph_id, cargo_lock_hash, join, load_workspace, Workspace, KOVAN_TOML, REVIEW_MD,
    ROOT_FILE,
};
use crate::review_stamps::git;

/// (revision, file) -> the file's hashed functions there (`None`: absent or
/// not Rust that parses).
pub(crate) type FileCache = BTreeMap<(String, String), Option<Vec<HashedFn>>>;
/// (revision, file) -> commit messages up to the revision touching it.
pub(crate) type MessageCache = BTreeMap<(String, String), Vec<String>>;

/// `qual` split into the hasher's qualified name and the call graph's
/// 1-based `#k` duplicate index.
fn split_qual(qual: &str) -> (String, Option<usize>) {
    let squash = |s: &str| s.split_whitespace().collect::<String>();
    match qual.rsplit_once('#') {
        Some((base, k)) if !k.is_empty() && k.bytes().all(|b| b.is_ascii_digit()) => {
            (squash(base), k.parse().ok())
        }
        _ => (squash(qual), None),
    }
}

/// The function `qual` of `file` at `rev`, hashed as the index hashes it
/// (the same `#k` rule as `kovan_common::call_graph::function_ids`).
pub(crate) fn fn_at(
    cache: &mut FileCache,
    root: &Path,
    rev: &str,
    file: &str,
    qual: &str,
) -> Option<HashedFn> {
    let fns = cache
        .entry((rev.to_string(), file.to_string()))
        .or_insert_with(|| git::show(root, rev, file).and_then(|t| hash_functions(&t).ok()))
        .as_ref()?;
    let (base, k) = split_qual(qual);
    let same: Vec<&HashedFn> = fns
        .iter()
        .filter(|h| h.entry.qualname().split_whitespace().collect::<String>() == base)
        .collect();
    match (k, same.as_slice()) {
        (None, [one]) => Some((*one).clone()),
        (Some(k), many) if k >= 1 && many.len() > 1 => many.get(k - 1).map(|h| (*h).clone()),
        _ => None,
    }
}

/// Messages of the commits up to `rev` that touched `file`.
fn messages(cache: &mut MessageCache, root: &Path, rev: &str, file: &str) -> Vec<String> {
    cache
        .entry((rev.to_string(), file.to_string()))
        .or_insert_with(|| {
            git::git(root, &["log", "--format=%B%x1e", rev, "--", file])
                .map(|out| {
                    out.split('\x1e')
                        .map(str::trim)
                        .filter(|m| !m.is_empty())
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default()
        })
        .clone()
}

/// The tests that reached function `id` at `commit`, from the folder's
/// `kovan.toml` as committed then (module doc); `None` when it was not
/// committed or does not list the function.
pub(crate) fn tests_at(
    root: &Path,
    commit: &str,
    dir: &str,
    id: &str,
    cache: &mut MessageCache,
) -> Option<TestsAtReview> {
    let text = git::show(root, commit, &join(dir, KOVAN_TOML))?;
    let idx = FolderIndex::parse(&text).ok()?;
    let f = idx.functions().find(|(_, f)| f.id == id)?.1.clone();
    let mut commit_messages = BTreeMap::new();
    for t in &f.reached_by {
        if let Some((file, _)) = t.split_once(".rs::") {
            commit_messages.insert(
                t.clone(),
                messages(cache, root, commit, &format!("{file}.rs")),
            );
        }
    }
    Some(TestsAtReview {
        reached_by: f.reached_by,
        commit_messages,
    })
}

/// The stamp's pickaxe string: its signature value.
fn needle(r: &ReviewEntry) -> Option<&str> {
    r.review.signature.as_ref().map(|s| s.value.as_str())
}

fn committer_time(root: &Path, rev: &str) -> Option<i64> {
    git::git(root, &["show", "-s", "--format=%ct", rev])
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// The commit that added `r` to `review_md` (module doc), if committed.
fn added_in(root: &Path, review_md: &str, r: &ReviewEntry) -> Option<StampCommit> {
    let needle = needle(r)?;
    if !git::show(root, "HEAD", review_md)?.contains(needle) {
        return None;
    }
    let out = git::git(
        root,
        &[
            "log",
            "-1",
            "--format=%H%x1f%ct%x1f%B",
            &format!("-S{needle}"),
            "--",
            review_md,
        ],
    )
    .ok()?;
    let mut parts = out.splitn(3, '\x1f');
    let commit = parts.next()?.trim().to_string();
    let time = parts.next().and_then(|t| t.trim().parse().ok());
    let body = parts.next().unwrap_or("");
    if commit.is_empty() {
        return None;
    }
    let certified = &r.review.commit;
    let descendant = git::git(root, &["merge-base", "--is-ancestor", certified, &commit]).is_ok();
    let same = git::rev_parse(root, certified).is_ok_and(|c| c == commit);
    Some(StampCommit {
        commit,
        after_certified: descendant && !same,
        agent_trailer: agent_trailer(body).0,
        committer_time: time,
    })
}

/// Reviewer ids a git author e-mail stands for (module doc).
fn author_ids(email: &str, root: &ReviewRoot) -> BTreeSet<String> {
    let mut out = BTreeSet::from([email.to_string()]);
    let lower = email.to_ascii_lowercase();
    let mut candidates = vec![lower.clone()];
    if let Some(local) = lower.strip_suffix("@users.noreply.github.com") {
        let user = local.rsplit('+').next().unwrap_or(local);
        candidates.push(format!("github:{user}"));
    }
    for c in &candidates {
        match root
            .reviewers
            .iter()
            .find(|r| r.id.to_ascii_lowercase() == *c)
        {
            Some(r) => out.insert(r.id.clone()),
            None => out.insert(c.clone()),
        };
    }
    out
}

/// `git blame` author e-mails of `file` lines `a..=b` in the working tree.
fn blame_authors(root: &Path, file: &str, a: u32, b: u32) -> Vec<String> {
    let range = format!("{a},{b}");
    let Ok(out) = git::git(
        root,
        &["blame", "--line-porcelain", "-L", &range, "--", file],
    ) else {
        return Vec::new();
    };
    let mut v: Vec<String> = out
        .lines()
        .filter_map(|l| l.strip_prefix("author-mail "))
        .map(|m| {
            m.trim()
                .trim_start_matches('<')
                .trim_end_matches('>')
                .to_string()
        })
        .filter(|m| !m.is_empty() && m != "not.committed.yet")
        .collect();
    v.sort();
    v.dedup();
    v
}

/// Where the review's function was **at the commit it certifies**: the
/// first acknowledged move's `from` (GitHub #771: an acknowledged move
/// rewrites `path` to the new location, but the certified commit still
/// holds the function at the old one), else `path`. The hash at the
/// certified commit, the tests at review time and the permalink are all
/// read there.
pub(crate) fn certified_path(r: &ReviewEntry) -> String {
    r.review
        .moved
        .first()
        .map(|m| m.from.clone())
        .or_else(|| r.path())
        .unwrap_or_default()
}

/// The previous committed `kovan_root.toml` (module doc).
fn previous_root(root: &Path) -> Option<ReviewRoot> {
    let head = git::show(root, "HEAD", ROOT_FILE)?;
    let now = std::fs::read_to_string(root.join(ROOT_FILE)).unwrap_or_default();
    let text = if now != head {
        head
    } else {
        let last = git::git(root, &["log", "-1", "--format=%H", "--", ROOT_FILE]).ok()?;
        git::show(root, &format!("{}^", last.trim()), ROOT_FILE)?
    };
    ReviewRoot::parse(&text).ok()
}

/// The workspace's [`GitFacts`] (module doc).
pub fn git_facts(root: &Path, ws: &Workspace) -> GitFacts {
    let mut files = FileCache::new();
    let mut msgs = MessageCache::new();
    let mut g = GitFacts {
        head: ws.head.clone(),
        cargo_lock: cargo_lock_hash(root).unwrap_or_default(),
        previous_root: previous_root(root),
        ..GitFacts::default()
    };
    for idx in &ws.indexes {
        if let Some(run) = &idx.test_run {
            for (file, f) in idx.functions() {
                if let Some(h) = fn_at(&mut files, root, &run.commit, &idx.file_path(file), &f.qual)
                {
                    g.hashes_at_test_run.insert(f.id.clone(), h.hashes.hash);
                }
            }
        }
    }
    let current: BTreeMap<&str, (&FolderIndex, &str)> = ws
        .indexes
        .iter()
        .flat_map(|i| {
            i.functions()
                .map(move |(file, f)| (f.id.as_str(), (i, file)))
        })
        .collect();
    for (dir, m) in &ws.reviews {
        let review_md = join(dir, REVIEW_MD);
        for r in m.doc.reviews() {
            let path = certified_path(r);
            let at = path.split_once(".rs::").and_then(|(file, qual)| {
                fn_at(
                    &mut files,
                    root,
                    &r.review.commit,
                    &format!("{file}.rs"),
                    qual,
                )
            });
            let reviewed_dir = path
                .split_once(".rs::")
                .map_or("", |(f, _)| f.rsplit_once('/').map_or("", |(d, _)| d));
            let facts = StampFacts {
                hash_at_commit: at.map(|h| h.hashes.hash),
                added_in: added_in(root, &review_md, r),
                reviewed_commit_time: committer_time(root, &r.review.commit),
                tests_at_review: tests_at(
                    root,
                    &r.review.commit,
                    reviewed_dir,
                    &r.function_id(),
                    &mut msgs,
                ),
            };
            g.stamps.insert(
                ReviewKey {
                    function: r.function_id(),
                    by: r.review.by.clone(),
                },
                facts,
            );
            if let Some((idx, file)) = current.get(r.function_id().as_str()) {
                let f = idx
                    .functions()
                    .find(|(_, f)| f.id == r.function_id())
                    .map(|(_, f)| f);
                if let Some(f) = f {
                    let ids: BTreeSet<String> =
                        blame_authors(root, &idx.file_path(file), f.lines[0], f.lines[1])
                            .iter()
                            .flat_map(|e| author_ids(e, &ws.review_root))
                            .collect();
                    g.code_authors.entry(f.id.clone()).or_default().extend(ids);
                }
            }
        }
    }
    g
}

/// The engine's verdict on the workspace, with the call-graph id of every
/// current function.
#[derive(Debug, Clone)]
pub struct WorkspaceEvaluation {
    pub evaluation: Evaluation,
    /// Function id -> call-graph id (`file.rs::Type::f#2`).
    pub call_graph_ids: BTreeMap<String, String>,
    /// Review key -> permalink of the reviewed code at the review commit.
    pub permalinks: BTreeMap<ReviewKey, String>,
    /// `kovan_root.toml`'s review sections as judged (for the IV&V views,
    /// #810: the audit records as written).
    pub review_root: ReviewRoot,
}

/// Load the workspace, build the git facts and run the engine with
/// signatures enforced (module doc), with no concept areas.
pub fn evaluate_workspace(root: &Path) -> Result<WorkspaceEvaluation, String> {
    evaluate_workspace_with(root, &ConceptAreas::new())
}

/// [`evaluate_workspace`] with the function -> concept-area map given
/// (GitHub #810). Nothing in kovan resolves concept areas yet, so every
/// caller but a test passes none; with none, rung 5 is unreachable
/// ([`kovan_common::review::ivv::Rung5Miss::NoConceptArea`]).
pub fn evaluate_workspace_with(
    root: &Path,
    concepts: &ConceptAreas,
) -> Result<WorkspaceEvaluation, String> {
    let ws = load_workspace(root)?;
    Ok(evaluate_loaded_with(root, &ws, concepts))
}

/// [`evaluate_workspace`] over a workspace already loaded (the need-you
/// queue reads the same [`Workspace`] for crates and line ranges), with no
/// concept areas.
pub fn evaluate_loaded(root: &Path, ws: &Workspace) -> WorkspaceEvaluation {
    evaluate_loaded_with(root, ws, &ConceptAreas::new())
}

/// [`evaluate_loaded`] with the function -> concept-area map given (#810).
pub fn evaluate_loaded_with(root: &Path, ws: &Workspace, concepts: &ConceptAreas) -> WorkspaceEvaluation {
    let git = git_facts(root, ws);
    let krate_of = |dir: &str| {
        ws.indexes
            .iter()
            .find(|i| i.dir == dir)
            .map(|i| i.krate.clone())
            .unwrap_or_default()
    };
    let folders: Vec<FolderReviews> = ws
        .reviews
        .iter()
        .map(|(dir, m)| FolderReviews {
            krate: krate_of(dir),
            dir: dir.clone(),
            doc: m.doc.clone(),
        })
        .collect();
    let evaluation = evaluate(
        &folders,
        &ws.indexes,
        &ws.review_root,
        &git,
        concepts,
        SignaturePolicy::Enforce,
    );
    let call_graph_ids = ws
        .indexes
        .iter()
        .flat_map(|i| {
            i.functions()
                .map(move |(file, f)| (f.id.clone(), call_graph_id(i, file, f)))
        })
        .collect();
    let mut files = FileCache::new();
    let mut permalinks = BTreeMap::new();
    for m in ws.reviews.values() {
        for r in m.doc.reviews() {
            let path = certified_path(r);
            if let Some((file, qual)) = path.split_once(".rs::") {
                let file = format!("{file}.rs");
                let lines =
                    fn_at(&mut files, root, &r.review.commit, &file, qual).map(|h| h.entry.lines);
                let anchor = lines
                    .map(|[a, b]| format!("#L{a}-L{b}"))
                    .unwrap_or_default();
                permalinks.insert(
                    ReviewKey {
                        function: r.function_id(),
                        by: r.review.by.clone(),
                    },
                    format!(
                        "{}/blob/{}/{file}{anchor}",
                        crate::review_stamps::DEFAULT_REPO_URL.trim_end_matches('/'),
                        r.review.commit
                    ),
                );
            }
        }
    }
    WorkspaceEvaluation {
        evaluation,
        call_graph_ids,
        permalinks,
        review_root: ws.review_root.clone(),
    }
}

/// A one-line reason for a state that is not valid.
fn describe(s: &StampState) -> String {
    match s {
        StampState::Valid => String::new(),
        StampState::DirectlyStale(w) => format!("code changed since the review: {w:?}"),
        StampState::InheritedStale { cause, blocked, .. } => {
            format!(
                "a callee changed: {cause:?}{}",
                if *blocked {
                    " (re-confirm blocked until tests pass)"
                } else {
                    ""
                }
            )
        }
        StampState::NeedsFixOpen { note, .. } => format!("needs fix: {note}"),
        StampState::Fixed { note, .. } => format!("fixed, awaiting re-review: {note}"),
        StampState::Unverified(why) => format!("unverified: {why:?}"),
        StampState::Unreadable { message } => format!("unreadable: {message}"),
        StampState::Invalid(why) => format!("invalid: {why:?}"),
        StampState::Moved { to, .. } => format!("moved to {}::{}", to.file, to.qual),
        StampState::PendingWorkspaceTest { .. } => {
            "pending a full workspace test at the new Cargo.lock".into()
        }
        other => other.kind().label().to_string(),
    }
}

/// Every reviewed (or flagged) function's state, in the form kovan-web
/// reads (`Facts::new`), keyed by call-graph id. Functions with nothing
/// recorded are left out (the web shows them as unreviewed). Plain
/// function, no UI: call it off the UI thread.
pub fn stamp_states(root: &Path) -> Result<Vec<WebStamp>, String> {
    let we = evaluate_workspace(root)?;
    let mut out = Vec::new();
    for (id, fr) in &we.evaluation.functions {
        if fr.reviews.is_empty() && fr.state.kind() == StateKind::New {
            continue;
        }
        let Some(cg) = we.call_graph_ids.get(id) else {
            continue;
        };
        let chosen = fr
            .reviews
            .iter()
            .find(|r| r.state == fr.state)
            .or_else(|| fr.reviews.first());
        let kind = fr.state.kind();
        let note = match &fr.state {
            StampState::NeedsFixOpen { note, .. } | StampState::Fixed { note, .. } => note.clone(),
            _ => String::new(),
        };
        let key = chosen.map(|r| ReviewKey {
            function: id.clone(),
            by: r.by.clone(),
        });
        out.push(WebStamp {
            function: cg.clone(),
            verdict: if kind == StateKind::Valid {
                StampVerdict::Valid
            } else {
                StampVerdict::Stale
            },
            reason: describe(&fr.state),
            rung: fr.rung.or_else(|| chosen.and_then(|r| r.rung)).unwrap_or(0),
            reviewer: chosen.map(|r| r.by.clone()).unwrap_or_default(),
            date: chosen.and_then(|r| r.date.clone()).unwrap_or_default(),
            note,
            permalink: key
                .and_then(|k| we.permalinks.get(&k).cloned())
                .unwrap_or_default(),
            state: Some(kind),
            ivv: summarise(fr, &we.review_root, &we.evaluation.ivv_warnings)
                .map(std::sync::Arc::new),
        });
    }
    out.sort_by(|a, b| a.function.cmp(&b.function));
    Ok(out)
}
