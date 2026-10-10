//! **Review mode** (GitHub #770; the binding design is #740 decisions 1–11
//! and walkthrough steps U2–U4): everything desktop kovan's review panel
//! shows and does, without egui. The drawing is `app/review_mode_panel.rs`.
//!
//! ```text
//!  Snapshot::load(root)  (worker: kovan.toml refreshed in memory, review.md,
//!       │                 git facts, the staleness engine)
//!       ├──> function_view(root, &snap, function) ──> FunctionView
//!       │       source now · unified diff since the last review (own, and the
//!       │       callees' for an inherited stale) · this function's review.md
//!       │       sections · highlights re-anchored · state, blockers, flags
//!       │       (#783 implausible signed_at among them) · upstream link
//!       ├──> plan_walk(&snap, function) ──> walk::WalkPlan (bottom-up, size)
//!       └──> blockers_of(&snap, fn id)  ──> the enforced bottom-up rule,
//!                                            used by the stamp dialog
//!  Session (review.md as it was on entering) ── Save keeps, Cancel restores
//! ```
//!
//! | #740 item | here |
//! |---|---|
//! | source or unified diff in the main view (decisions 2, 8) | [`diff`], [`FunctionView::own_diff`], [`FunctionView::callee_diffs`] |
//! | sidebar edits only this function's section (U3) | [`section`] |
//! | highlights as annotation artifacts (decision 3) | [`highlight`] |
//! | walk: bottom-up enforced, size, huge warning, trail, summary (U2, U4, decision 10) | [`walk`], [`blockers_of`] |
//! | no-concept reason; port prefill and architecture suggestion (U3, #760 q11) | [`prefill`] |
//! | upstream opens GitHub/GitLab (U3) | [`prefill::upstream_url`] |
//! | kvim for quick fixes only (correction of 2026-10-07) | [`quick_fix`] |
//! | Save writes `review.md` only; Cancel discards since entering (decisions 5, 6) | [`Session`] |
//! | implausible `signed_at` shown with the state (#783, deferred here) | [`FunctionView::flags`] |
//!
//! Every function here reads files or runs git: call it off the UI thread
//! (root `CLAUDE.md`, no-lag HARD RULE).

pub mod diff;
pub mod highlight;
pub mod prefill;
pub mod quick_fix;
pub mod section;
pub mod walk;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kovan_common::review::engine::{FunctionFlag, StampState};
use kovan_common::review::review_md::ReviewEntry;
use kovan_common::review::signed_at::now_local;
use kovan_common::review::state::StateKind;

use super::{evaluate_workspace, join, load_workspace, Workspace, WorkspaceEvaluation, REVIEW_MD};
use diff::FnDiff;
use highlight::Highlight;
use section::FunctionSection;
use walk::{WalkItem, WalkPlan};

/// The workspace and the engine's verdict on it, loaded once per refresh.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub ws: Workspace,
    pub we: WorkspaceEvaluation,
}

impl Snapshot {
    /// Load the workspace and evaluate it (git; a worker's job).
    pub fn load(root: &Path) -> Result<Snapshot, String> {
        Ok(Snapshot {
            ws: load_workspace(root)?,
            we: evaluate_workspace(root)?,
        })
    }

    /// `fn:` id -> its state kind (`New` when the engine has none).
    pub fn kind(&self, fn_id: &str) -> StateKind {
        self.we
            .evaluation
            .functions
            .get(fn_id)
            .map(|f| f.state.kind())
            .unwrap_or(StateKind::New)
    }

    /// Every function's workspace callees, by `fn:` id.
    pub fn callees(&self) -> BTreeMap<String, Vec<String>> {
        self.ws
            .indexes
            .iter()
            .flat_map(|i| {
                i.functions()
                    .map(|(_, f)| (f.id.clone(), f.callees.clone()))
            })
            .collect()
    }

    /// The display name (`qual`) of a function id.
    pub fn name(&self, fn_id: &str) -> String {
        self.ws
            .find(fn_id)
            .map(|(_, _, f)| f.qual.clone())
            .unwrap_or_else(|| fn_id.to_string())
    }
}

/// The callees that block stamping `fn_id` under the enforced bottom-up
/// rule ([`walk::blockers`]), as (`fn:` id, name).
pub fn blockers_of(snap: &Snapshot, fn_id: &str) -> Vec<(String, String)> {
    walk::blockers(fn_id, &snap.callees(), |c| snap.kind(c).counts())
        .into_iter()
        .map(|c| {
            let n = snap.name(&c);
            (c, n)
        })
        .collect()
}

/// Plan the walk to `function` (an `fn:` or call-graph id).
pub fn plan_walk(snap: &Snapshot, function: &str) -> Result<WalkPlan, String> {
    let (_, _, target) = snap
        .ws
        .find(function)
        .ok_or_else(|| format!("{function} is not in any kovan.toml (run Index fresh)"))?;
    let items: BTreeMap<String, WalkItem> = snap
        .ws
        .indexes
        .iter()
        .flat_map(|idx| {
            idx.functions().map(move |(file, f)| {
                (
                    f.id.clone(),
                    WalkItem {
                        id: f.id.clone(),
                        call_graph_id: super::call_graph_id(idx, file, f),
                        name: f.qual.clone(),
                        lines: f.lines[1].saturating_sub(f.lines[0]) + 1,
                        state: StateKind::New,
                        callees: f.callees.clone(),
                    },
                )
            })
        })
        .map(|(k, mut v)| {
            v.state = snap.kind(&k);
            (k, v)
        })
        .collect();
    WalkPlan::new(&target.id, &items).ok_or_else(|| format!("{function}: not in the index"))
}

/// The review a diff is taken since.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SinceReview {
    pub by: String,
    pub date: String,
    pub commit: String,
    /// The path reviewed (`file.rs::qual`).
    pub path: String,
}

/// What the review panel shows for one function (module doc).
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionView {
    pub fn_id: String,
    pub call_graph_id: String,
    /// Workspace-relative file.
    pub file: String,
    pub qual: String,
    /// The function's source now (doc comment included) and its lines.
    pub source: String,
    pub lines: [u32; 2],
    /// The folder's `review.md`, workspace-relative, and its text now
    /// (empty when there is none).
    pub review_md: String,
    pub review_md_text: String,
    /// This function's entries in it.
    pub sections: Vec<FunctionSection>,
    pub highlights: Vec<Highlight>,
    /// The engine's state, its plain-English label and reason.
    pub state: StateKind,
    pub reason: String,
    /// Callees blocking a stamp (bottom-up), as (id, name).
    pub blocked_by: Vec<(String, String)>,
    /// Flags shown with the state (never void a stamp): the #783
    /// implausible `signed_at`, new reaching tests, duplicate code, …
    pub flags: Vec<String>,
    /// The newest review, when there is one.
    pub since: Option<SinceReview>,
    /// The function's own diff since `since` (`None`: never reviewed);
    /// `Err` says why it could not be made.
    pub own_diff: Option<Result<FnDiff, String>>,
    /// Inherited stale: each changed callee's diff since the review.
    pub callee_diffs: Vec<(String, Result<FnDiff, String>)>,
    /// The upstream file in the browser (a confirmed port only).
    pub upstream_url: Option<String>,
    pub is_port: bool,
}

/// A flag in plain words.
pub fn describe_flag(f: &FunctionFlag) -> String {
    let label = f.kind().label();
    match f {
        FunctionFlag::NewReachingTests { tests, .. } => format!("{label}: {}", tests.join(", ")),
        FunctionFlag::DuplicateCode { copies } => format!("{label}: also at {}", copies.join(", ")),
        FunctionFlag::ImplausibleSignedAt { review, problems } => format!(
            "(!) {label} ({review}): {}",
            problems
                .iter()
                .map(|p| p.describe())
                .collect::<Vec<_>>()
                .join("; ")
        ),
        FunctionFlag::IndependentVvNotCounted { review, misses } => {
            format!("{label} ({review}): {misses:?}")
        }
    }
}

/// A state's one-line reason.
fn reason(s: &StampState) -> String {
    match s {
        StampState::Valid | StampState::New => String::new(),
        StampState::NeedsFixOpen { note, .. } | StampState::Fixed { note, .. } => note.clone(),
        StampState::Unreadable { message } => message.clone(),
        other => format!("{other:?}"),
    }
}

/// The newest review of `fn_id` among `reviews` (by date, then position).
fn newest<'a>(
    reviews: impl Iterator<Item = &'a ReviewEntry>,
    fn_id: &str,
) -> Option<&'a ReviewEntry> {
    reviews.filter(|r| r.function_id() == fn_id).max_by(|a, b| {
        (a.review.signed_at.as_deref().unwrap_or(&a.review.date))
            .cmp(b.review.signed_at.as_deref().unwrap_or(&b.review.date))
    })
}

/// Build the review panel's view of `function` (an `fn:` or call-graph id)
/// (module doc). Reads the working tree and git.
pub fn function_view(root: &Path, snap: &Snapshot, function: &str) -> Result<FunctionView, String> {
    let (idx, file_name, f) = snap
        .ws
        .find(function)
        .ok_or_else(|| format!("{function} is not in any kovan.toml (run Index fresh)"))?;
    let file = idx.file_path(file_name);
    let text = std::fs::read_to_string(root.join(&file)).map_err(|e| format!("{file}: {e}"))?;
    let (source, lines) = diff::fn_source(&text, &f.qual)
        .ok_or_else(|| format!("{file}::{}: not found in the working tree", f.qual))?;
    let folder = snap.ws.reviews.get(&idx.dir);
    let review_md_text = folder.map(|m| m.text.clone()).unwrap_or_default();
    let doc = folder.map(|m| m.doc.clone()).unwrap_or_default();
    let report = snap.we.evaluation.functions.get(&f.id);
    let state = report.map(|r| r.state.kind()).unwrap_or(StateKind::New);
    let last = newest(doc.reviews(), &f.id);
    let since = last.map(|r| SinceReview {
        by: r.review.by.clone(),
        date: r.review.date.clone(),
        commit: r.review.commit.clone(),
        path: r.path().unwrap_or_default(),
    });
    let own_diff = since.as_ref().map(|s| {
        diff::function_diff(root, &file, &f.qual, &s.path, &s.commit).map_err(|e| e.to_string())
    });
    let hashes = snap.ws.hashes();
    let callee_diffs = last
        .map(|r| {
            r.review
                .callees
                .iter()
                .filter(|(c, h)| hashes.get(*c).is_some_and(|now| now != *h))
                .map(|(c, _)| {
                    let d = match snap.ws.find(c) {
                        Some((ci, cf, cfn)) => {
                            let cfile = ci.file_path(cf);
                            let cpath = format!("{cfile}::{}", cfn.qual);
                            diff::function_diff(root, &cfile, &cfn.qual, &cpath, &r.review.commit)
                                .map_err(|e| e.to_string())
                        }
                        None => Err(format!("{c}: not in the index")),
                    };
                    (snap.name(c), d)
                })
                .collect()
        })
        .unwrap_or_default();
    let upstream = doc.upstream();
    Ok(FunctionView {
        fn_id: f.id.clone(),
        call_graph_id: super::call_graph_id(idx, file_name, f),
        file: file.clone(),
        qual: f.qual.clone(),
        sections: section::function_sections(&review_md_text, &f.id),
        highlights: highlight::highlights(&doc, &f.id, &source),
        source,
        lines,
        review_md: join(&idx.dir, REVIEW_MD),
        review_md_text,
        state,
        reason: report.map(|r| reason(&r.state)).unwrap_or_default(),
        blocked_by: blockers_of(snap, &f.id),
        flags: report
            .map(|r| r.flags.iter().map(describe_flag).collect())
            .unwrap_or_default(),
        since,
        own_diff,
        callee_diffs,
        upstream_url: upstream.and_then(|u| prefill::upstream_url(u, file_name, &f.id)),
        is_port: upstream.is_some_and(|u| u.is_port),
    })
}

/// Save edited comments of the entry on `line` of `review_md` (workspace-
/// relative), provided the file is still `expected` (the text the editor
/// opened on). Writes `review.md` only; never commits (#740 decision 6).
/// Returns the new text.
pub fn save_section(
    root: &Path,
    review_md: &str,
    expected: &str,
    line: usize,
    comments: &str,
) -> Result<String, String> {
    let path = root.join(review_md);
    let now = std::fs::read_to_string(&path).unwrap_or_default();
    if now != expected {
        return Err(format!(
            "{review_md} changed since it was opened: reload (nothing was saved)"
        ));
    }
    let text = section::splice_comments(&now, line, comments).map_err(|e| e.to_string())?;
    std::fs::write(&path, &text).map_err(|e| format!("{review_md}: {e}"))?;
    Ok(text)
}

/// Write a highlight on lines `first..=last` of `function`'s source (as
/// [`function_view`] shows it) by `by`, with `note`, into its folder's
/// `review.md` (appended; never commits). Returns the `review.md` path.
pub fn add_highlight(
    root: &Path,
    function: &str,
    by: &str,
    first: usize,
    last: usize,
    note: &str,
) -> Result<PathBuf, String> {
    let ws = load_workspace(root)?;
    if ws.head.is_empty() {
        return Err("no commit: a highlight records the commit it was made at".into());
    }
    let (idx, file_name, f) = ws
        .find(function)
        .ok_or_else(|| format!("{function} is not in any kovan.toml"))?;
    let file = idx.file_path(file_name);
    let text = std::fs::read_to_string(root.join(&file)).map_err(|e| format!("{file}: {e}"))?;
    let (source, _) =
        diff::fn_source(&text, &f.qual).ok_or_else(|| format!("{file}::{}: not found", f.qual))?;
    let entry =
        highlight::draft_highlight(f, &file, by, &ws.head, &now_local(), &source, first, last)?;
    let path = root.join(join(&idx.dir, REVIEW_MD));
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let new = highlight::append_highlight(&old, &entry, note)?;
    std::fs::write(&path, new).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

/// Review mode's Save / Cancel (#740 decisions 5, 6): the folder's
/// `review.md` as it was on entering. Save keeps what was written (and
/// never commits); Cancel puts the file back as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    /// Workspace-relative `review.md`.
    pub review_md: String,
    /// Its text on entering; `None`: it did not exist.
    pub entered: Option<String>,
}

impl Session {
    /// Enter review mode on the folder `review.md` (workspace-relative).
    pub fn enter(root: &Path, review_md: &str) -> Session {
        Session {
            review_md: review_md.to_string(),
            entered: std::fs::read_to_string(root.join(review_md)).ok(),
        }
    }

    /// Whether `review.md` changed since entering.
    pub fn changed(&self, root: &Path) -> bool {
        std::fs::read_to_string(root.join(&self.review_md)).ok() != self.entered
    }

    /// Cancel: put `review.md` back as it was on entering (removing it if
    /// it did not exist). Returns whether anything was undone.
    pub fn discard(&self, root: &Path) -> Result<bool, String> {
        if !self.changed(root) {
            return Ok(false);
        }
        let path = root.join(&self.review_md);
        match &self.entered {
            Some(t) => std::fs::write(&path, t),
            None => std::fs::remove_file(&path),
        }
        .map_err(|e| format!("{}: {e}", self.review_md))?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests;
