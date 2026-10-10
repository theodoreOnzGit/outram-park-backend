//! **The ⚑ need-you queue** (GitHub #771, #740 U1/U5; decisions of
//! 2026-10-07 on #771): what needs the maintainer, built from the staleness
//! engine's verdict ([`super::evaluate_loaded`]) and git. Plain data and
//! functions, no egui; the drawing is `app/need_you_view.rs`.
//!
//! ```text
//!  Workspace + Evaluation ──rows_from_evaluation (pure)──> state rows
//!     │                         stale, re-confirm, fixed, doc changed,
//!     │                         moves (one row per folder pair: batch
//!     │                         acknowledge), deletions, unreadable,
//!     │                         invalid, flags
//!  git log ──new_code::scan──> fold (pure) ──> one row per commit of new
//!     │                                       functions ("＋ N new
//!     │                                       functions · b81e · K untested")
//!  git log -L ──authorship::change_authorship──> each row's authorship
//!     v
//!  order_rows (pure): directly stale on top, then by crate, oldest first;
//!  new-code rows last, newest commit first
//! ```
//!
//! # Decisions this follows
//!
//! - **Rows** (#740 U1): directly stale stamps, inherited-stale stamps (every
//!   caller of a changed function needs a re-confirm, #739 D6 corrected
//!   2026-10-07; a failing reaching test blocks it), ✏ fixes awaiting
//!   re-review, moved reviews (U5: they stay until acknowledged, one-click
//!   batch acknowledge per folder pair), deletions (U5: the review leaves
//!   `review.md` for the deleted-functions history), and new code.
//!   [`StateKind::needs_person`] and [`FlagKind::needs_person`] decide which
//!   engine states and flags are rows.
//! - **New functions fold into one row per commit** and **a folded row
//!   clears once opened** (#771, 2026-10-07): opened commits are kept in a
//!   small file under `target/` ([`seen_path`]), never committed.
//! - **Authorship** (#771, 2026-10-07): no label on every row; a filter
//!   ([`Filter`]: all / agent only / human only) and the session link plus
//!   authorship in the row's details. Agent authorship is read from the
//!   commit trailer. A row whose authorship git cannot give (a deletion, a
//!   function whose lines no commit in range touched) shows under **All**
//!   only.
//! - **Maturity in plain English** per crate
//!   ([`kovan_common::code_map::plain_maturity`]).
//!
//! # The #810 hook (rung-5 IV&V misses)
//!
//! A review that names a separation attestation but misses rung 5 raises
//! the engine flag `FunctionFlag::IndependentVvNotCounted`, which is
//! already a row here ([`RowKind::Flag`]). Its detail lines come from
//! [`flag_detail`]; that `match` arm is where #810's plain-English miss
//! reasons go (today it lists the `Rung5Miss` values as the engine names
//! them).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use kovan_common::code_map::plain_maturity::plain_maturity;
use kovan_common::code_map::CodeMap;
use kovan_common::review::engine::{FunctionFlag, FunctionReport, InheritedCause, StampState};
use kovan_common::review::state::{FlagKind, StateKind};
use kovan_common::review::types::{AuthorshipKind, ChangeAuthorship};

use super::authorship::{change_messages, describe};
use super::new_code::{fold, scan, NewCommit, NewFn};
use super::recent::{recently_reviewed, RecentEntry};
use super::{evaluate_loaded, load_workspace, Workspace, WorkspaceEvaluation};
use kovan_common::review::types::authorship_from_messages;

/// What a row asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowKind {
    /// Its own code changed since the review: re-review.
    Stale,
    /// A callee changed: re-confirm; `blocked` while a reaching test fails.
    ReConfirm { blocked: bool },
    /// Edited after a needs-fix: re-review.
    Fixed,
    /// Only the doc comment changed: a quick look.
    DocChanged,
    /// Reviews found at a new place, one row per (from folder, to folder):
    /// batch acknowledge. `ready` are the functions whose reaching tests
    /// allow it; the others wait (failing or pending tests, or several
    /// identical candidates).
    Moved {
        from_dir: String,
        to_dir: String,
        ready: Vec<String>,
    },
    /// The function is gone: record it in the deleted-functions history.
    Deleted,
    /// A review entry cannot be read.
    Unreadable,
    /// A review contradicts what is derived (shown, never counted).
    Invalid,
    /// An engine flag (never voids a stamp).
    Flag(FlagKind),
    /// New functions one commit introduced (folded).
    NewCode { commit: String, untested: usize },
}

impl RowKind {
    /// Sort tier: stale on top (#771), new code last.
    fn tier(&self) -> u8 {
        match self {
            Self::Stale => 0,
            Self::NewCode { .. } => 2,
            _ => 1,
        }
    }

    /// The short words the row starts with.
    pub fn label(&self) -> String {
        match self {
            Self::Stale => "changed since review".into(),
            Self::ReConfirm { blocked: false } => "a callee changed: re-confirm".into(),
            Self::ReConfirm { blocked: true } => {
                "a callee changed: re-confirm (blocked until its tests pass)".into()
            }
            Self::Fixed => "fixed, awaiting re-review".into(),
            Self::DocChanged => "doc changed".into(),
            Self::Moved { .. } => "moved".into(),
            Self::Deleted => "deleted".into(),
            Self::Unreadable => "review unreadable".into(),
            Self::Invalid => "review invalid".into(),
            Self::Flag(k) => k.label().into(),
            Self::NewCode { .. } => "new code".into(),
        }
    }
}

/// A function a row names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueFn {
    /// `fn:` id.
    pub id: String,
    /// kovan-web's key (`file.rs::T::f`); empty for a deleted function.
    pub call_graph_id: String,
    /// `file.rs::qual` (the review's path for a deleted function).
    pub path: String,
}

/// One row of the queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueRow {
    /// Stable key (expand state, the seen list for new code).
    pub key: String,
    pub kind: RowKind,
    /// The crate (empty for a new-code row, which can span crates).
    pub krate: String,
    /// The one-line text.
    pub title: String,
    pub functions: Vec<QueueFn>,
    /// The details shown when the row is expanded.
    pub detail: Vec<String>,
    /// Who authored the change the row is about (`None`: git cannot say).
    pub authorship: Option<ChangeAuthorship>,
    /// The oldest review date it concerns (`YYYY-MM-DD`), for oldest-first.
    pub since: Option<String>,
    /// The review commit the change is measured from (state rows).
    pub review_commit: Option<String>,
}

impl QueueRow {
    /// The function to open on the map: the first one with a call-graph id.
    pub fn open_target(&self) -> Option<&str> {
        self.functions
            .iter()
            .map(|f| f.call_graph_id.as_str())
            .find(|c| !c.is_empty())
    }

    /// The authorship line for the details ("agent-authored, session …").
    pub fn authorship_line(&self) -> Option<String> {
        self.authorship.as_ref().map(describe)
    }
}

/// The queue's authorship filter (#771, 2026-10-07).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Filter {
    #[default]
    All,
    /// Rows whose change an agent took part in (agent or mixed).
    AgentOnly,
    /// Rows whose change no agent took part in.
    HumanOnly,
}

impl Filter {
    pub const ALL: [Filter; 3] = [Filter::All, Filter::AgentOnly, Filter::HumanOnly];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::AgentOnly => "agent only",
            Self::HumanOnly => "human only",
        }
    }

    /// Whether `row` shows under this filter (module doc: unknown
    /// authorship shows under All only).
    pub fn keeps(self, row: &QueueRow) -> bool {
        let kind = row.authorship.as_ref().map(|a| a.kind);
        match self {
            Self::All => true,
            Self::AgentOnly => matches!(kind, Some(AuthorshipKind::Agent | AuthorshipKind::Mixed)),
            Self::HumanOnly => kind == Some(AuthorshipKind::Human),
        }
    }
}

/// A crate's maturity in plain words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateMaturity {
    pub krate: String,
    /// The tag's level (`[package.metadata.kovan] maturity`).
    pub recorded: u8,
    /// Reviewed functions changed since their review.
    pub stale: usize,
    /// [`plain_maturity`]'s words.
    pub words: String,
}

/// Where a current function is, for the rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnInfo {
    pub krate: String,
    pub file: String,
    pub qual: String,
    pub lines: [u32; 2],
    pub call_graph_id: String,
}

/// Every current function's [`FnInfo`], by `fn:` id.
pub fn fn_info(ws: &Workspace) -> BTreeMap<String, FnInfo> {
    let mut out = BTreeMap::new();
    for idx in &ws.indexes {
        for (file, f) in idx.functions() {
            out.insert(
                f.id.clone(),
                FnInfo {
                    krate: idx.krate.clone(),
                    file: idx.file_path(file),
                    qual: f.qual.clone(),
                    lines: f.lines,
                    call_graph_id: super::call_graph_id(idx, file, f),
                },
            );
        }
    }
    out
}

/// The detail lines of a flag (module doc: the #810 hook).
pub fn flag_detail(flag: &FunctionFlag) -> Vec<String> {
    match flag {
        FunctionFlag::NewReachingTests { review, tests } => vec![format!(
            "{} test(s) reach it since review {review}, themselves unreviewed: {}",
            tests.len(),
            tests.join(", ")
        )],
        FunctionFlag::DuplicateCode { copies } => {
            vec![format!("the same code is also at: {}", copies.join(", "))]
        }
        FunctionFlag::ImplausibleSignedAt { review, problems } => {
            vec![format!("review {review}: signing time {problems:?}")]
        }
        // #810 hook: rung-5 (IV&V) miss reasons, to be worded in plain
        // English by the IV&V display work.
        FunctionFlag::IndependentVvNotCounted { review, misses } => misses
            .iter()
            .map(|m| format!("review {review}: independent V&V not counted: {m:?}"))
            .collect(),
    }
}

/// One line on why a state needs a person.
fn state_detail(s: &StampState) -> Vec<String> {
    match s {
        StampState::DirectlyStale(w) => vec![format!("its own code changed: {w:?}")],
        StampState::InheritedStale { cause, tests, .. } => {
            let what = match cause {
                InheritedCause::Callees(ids) => format!("callee(s) changed: {}", ids.join(", ")),
                InheritedCause::LockTestFailed => {
                    "a reaching test failed after Cargo.lock changed".into()
                }
            };
            vec![what, format!("reaching tests: {tests:?}")]
        }
        StampState::Fixed { note, .. } => vec![format!("needs-fix note: {note}")],
        StampState::Moved {
            from,
            to,
            tests,
            candidates,
        } => {
            let mut v = vec![format!(
                "from {} to {}::{}",
                from.as_ref()
                    .map(|l| format!("{}::{}", l.file, l.qual))
                    .unwrap_or_else(|| "?".into()),
                to.file,
                to.qual
            )];
            v.push(format!("regression tests: {tests:?}"));
            if !candidates.is_empty() {
                v.push(format!(
                    "identical code at several places; say which is the original: {}",
                    candidates.join(", ")
                ));
            }
            v
        }
        StampState::Unreadable { message } => vec![message.clone()],
        StampState::Invalid(why) => vec![format!("{why:?}")],
        other => vec![other.kind().label().to_string()],
    }
}

/// The review date of the oldest review of `fr` in its function's state.
fn oldest_date(fr: &FunctionReport) -> Option<String> {
    fr.reviews
        .iter()
        .filter(|r| r.state.kind() == fr.state.kind())
        .filter_map(|r| r.date.clone())
        .min()
        .or_else(|| fr.reviews.iter().filter_map(|r| r.date.clone()).min())
}

/// The engine's verdict as queue rows (module doc), unordered. `info`
/// locates current functions; `review_commits` is each function's oldest
/// standing review commit (the change is measured from it).
pub fn rows_from_evaluation(
    we: &WorkspaceEvaluation,
    info: &BTreeMap<String, FnInfo>,
    review_commits: &BTreeMap<String, String>,
) -> Vec<QueueRow> {
    let mut rows = Vec::new();
    let mut moves: BTreeMap<
        (String, String),
        (
            Vec<QueueFn>,
            Vec<String>,
            Vec<String>,
            Option<String>,
            String,
        ),
    > = BTreeMap::new();
    for (id, fr) in &we.evaluation.functions {
        let Some(fi) = info.get(id) else { continue };
        let qf = QueueFn {
            id: id.clone(),
            call_graph_id: fi.call_graph_id.clone(),
            path: format!("{}::{}", fi.file, fi.qual),
        };
        let since = oldest_date(fr);
        let kind = match &fr.state {
            StampState::DirectlyStale(_) => Some(RowKind::Stale),
            StampState::InheritedStale { blocked, .. } => {
                Some(RowKind::ReConfirm { blocked: *blocked })
            }
            StampState::Fixed { .. } => Some(RowKind::Fixed),
            StampState::DocChanged => Some(RowKind::DocChanged),
            StampState::Unreadable { .. } => Some(RowKind::Unreadable),
            StampState::Invalid(_) => Some(RowKind::Invalid),
            StampState::Moved {
                from,
                to,
                tests,
                candidates,
            } => {
                let from_dir = from
                    .as_ref()
                    .map(|l| parent(&l.file).to_string())
                    .unwrap_or_default();
                let to_dir = parent(&to.file).to_string();
                let e = moves.entry((from_dir, to_dir)).or_insert_with(|| {
                    (Vec::new(), Vec::new(), Vec::new(), None, fi.krate.clone())
                });
                e.0.push(qf.clone());
                if tests.allows_reconfirm() && candidates.is_empty() {
                    e.1.push(id.clone());
                }
                e.2.extend(state_detail(&fr.state));
                e.3 = match (e.3.take(), since.clone()) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (a, b) => a.or(b),
                };
                None
            }
            _ => None,
        };
        debug_assert!(kind.as_ref().is_none_or(|_| fr.state.kind().needs_person()));
        if let Some(kind) = kind {
            rows.push(QueueRow {
                key: format!("{}:{id}", fr.state.kind().label()),
                title: format!("{} \u{2014} {}", fi.qual, kind.label()),
                kind,
                krate: fi.krate.clone(),
                functions: vec![qf.clone()],
                detail: state_detail(&fr.state),
                authorship: None,
                since: since.clone(),
                review_commit: review_commits.get(id).cloned(),
            });
        }
        for flag in &fr.flags {
            if !flag.kind().needs_person() {
                continue;
            }
            rows.push(QueueRow {
                key: format!("flag:{:?}:{id}", flag.kind()),
                kind: RowKind::Flag(flag.kind()),
                krate: fi.krate.clone(),
                title: format!("{} \u{2014} {}", fi.qual, flag.kind().label()),
                functions: vec![qf.clone()],
                detail: flag_detail(flag),
                authorship: None,
                since: since.clone(),
                review_commit: None,
            });
        }
    }
    for ((from_dir, to_dir), (functions, ready, detail, since, krate)) in moves {
        let n = functions.len();
        rows.push(QueueRow {
            key: format!("moved:{from_dir}->{to_dir}"),
            title: format!(
                "{n} moved function{} \u{b7} {} \u{2192} {} \u{b7} {} ready to acknowledge",
                if n == 1 { "" } else { "s" },
                if from_dir.is_empty() { "?" } else { &from_dir },
                to_dir,
                ready.len()
            ),
            kind: RowKind::Moved {
                from_dir,
                to_dir,
                ready,
            },
            krate,
            functions,
            detail,
            authorship: None,
            since,
            review_commit: None,
        });
    }
    for fr in &we.evaluation.deleted {
        let path = fr
            .location
            .as_ref()
            .map(|l| format!("{}::{}", l.file, l.qual))
            .or_else(|| {
                we.evaluation
                    .history
                    .iter()
                    .find(|h| h.row.function == fr.id)
                    .map(|h| h.row.path.clone())
            })
            .unwrap_or_else(|| fr.id.clone());
        let krate = we
            .evaluation
            .history
            .iter()
            .find(|h| h.row.function == fr.id)
            .map(|h| match &h.placement {
                kovan_common::review::engine::HistoryPlacement::FolderReviewMd { dir } => {
                    krate_of_dir(info, dir)
                }
                kovan_common::review::engine::HistoryPlacement::CrateIndex { krate, .. }
                | kovan_common::review::engine::HistoryPlacement::WorkspaceRoot { krate, .. } => {
                    krate.clone()
                }
            })
            .unwrap_or_default();
        rows.push(QueueRow {
            key: format!("deleted:{}", fr.id),
            kind: RowKind::Deleted,
            krate,
            title: format!("{path} \u{2014} deleted: record it in the history"),
            functions: vec![QueueFn {
                id: fr.id.clone(),
                call_graph_id: String::new(),
                path,
            }],
            detail: vec![
                "the function is gone; its reviews leave review.md for the deleted-functions \
                 history and live on in git"
                    .into(),
            ],
            authorship: None,
            since: oldest_date(fr),
            review_commit: None,
        });
    }
    rows
}

/// The crate of the first function whose file is in `dir`.
fn krate_of_dir(info: &BTreeMap<String, FnInfo>, dir: &str) -> String {
    info.values()
        .find(|f| parent(&f.file) == dir)
        .map(|f| f.krate.clone())
        .unwrap_or_default()
}

fn parent(file: &str) -> &str {
    file.rsplit_once('/').map_or("", |(d, _)| d)
}

/// One folded row per [`NewCommit`] (module doc), minus the commits in
/// `seen` (opened before).
pub fn new_code_rows(
    commits: &[NewCommit],
    info: &BTreeMap<String, FnInfo>,
    seen: &BTreeSet<String>,
) -> Vec<QueueRow> {
    commits
        .iter()
        .filter(|c| !seen.contains(&c.commit))
        .map(|c| {
            let short: String = c.commit.chars().take(4).collect();
            let n = c.functions.len();
            let functions: Vec<QueueFn> = c
                .functions
                .iter()
                .map(|id| match info.get(id) {
                    Some(fi) => QueueFn {
                        id: id.clone(),
                        call_graph_id: fi.call_graph_id.clone(),
                        path: format!("{}::{}", fi.file, fi.qual),
                    },
                    None => QueueFn {
                        id: id.clone(),
                        call_graph_id: String::new(),
                        path: id.clone(),
                    },
                })
                .collect();
            QueueRow {
                key: format!("new:{}", c.commit),
                kind: RowKind::NewCode {
                    commit: c.commit.clone(),
                    untested: c.untested,
                },
                krate: String::new(),
                title: format!(
                    "\u{ff0b} {n} new function{} \u{b7} {short} \u{b7} {} untested",
                    if n == 1 { "" } else { "s" },
                    c.untested
                ),
                functions,
                detail: vec![format!("commit {}: {}", c.commit, c.summary)],
                authorship: c.authorship.clone(),
                since: None,
                review_commit: None,
            }
        })
        .collect()
}

/// Order the rows (module doc): tier (directly stale first, new code
/// last), then crate, then oldest review first (rows with no date after),
/// then title. New-code rows keep their given (newest-first) order.
pub fn order_rows(rows: &mut [QueueRow]) {
    rows.sort_by(|a, b| {
        let tier = a.kind.tier().cmp(&b.kind.tier());
        if a.kind.tier() == 2 && b.kind.tier() == 2 {
            return tier;
        }
        tier.then_with(|| a.krate.cmp(&b.krate))
            .then_with(|| match (&a.since, &b.since) {
                (Some(x), Some(y)) => x.cmp(y),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            })
            .then_with(|| a.title.cmp(&b.title))
    });
}

/// Per crate: the recorded maturity and the count of reviewed functions
/// that changed since their review, in plain words. Crates with neither a
/// recorded level nor a review are left out.
pub fn crate_maturity(
    we: &WorkspaceEvaluation,
    info: &BTreeMap<String, FnInfo>,
    recorded: &BTreeMap<String, u8>,
) -> Vec<CrateMaturity> {
    let mut stale: BTreeMap<String, usize> = BTreeMap::new();
    let mut reviewed: BTreeSet<String> = BTreeSet::new();
    for (id, fr) in &we.evaluation.functions {
        let Some(fi) = info.get(id) else { continue };
        if fr.reviews.is_empty() {
            continue;
        }
        reviewed.insert(fi.krate.clone());
        if matches!(
            fr.state.kind(),
            StateKind::DirectlyStale
                | StateKind::InheritedStale
                | StateKind::Fixed
                | StateKind::DocChanged
                | StateKind::Moved
        ) {
            *stale.entry(fi.krate.clone()).or_default() += 1;
        }
    }
    reviewed
        .into_iter()
        .filter_map(|k| {
            let level = *recorded.get(&k)?;
            let n = stale.get(&k).copied().unwrap_or(0);
            Some(CrateMaturity {
                words: plain_maturity(level, n),
                krate: k,
                recorded: level,
                stale: n,
            })
        })
        .collect()
}

/// Options for [`build`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueOptions {
    /// How many recent non-merge commits are scanned for new functions.
    pub max_commits: usize,
    /// Rows beyond this many get no authorship (each costs a `git log
    /// -L`); they still show under All.
    pub max_authorship_rows: usize,
    /// How many recently reviewed places to list.
    pub recent: usize,
}

impl Default for QueueOptions {
    fn default() -> Self {
        QueueOptions {
            max_commits: 50,
            max_authorship_rows: 200,
            recent: 10,
        }
    }
}

/// The whole queue, as the view shows it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NeedYou {
    pub rows: Vec<QueueRow>,
    pub maturity: Vec<CrateMaturity>,
    pub recent: Vec<RecentEntry>,
    /// `HEAD` it was built at.
    pub head: String,
}

impl NeedYou {
    /// The rows `filter` keeps, in order.
    pub fn filtered(&self, filter: Filter) -> Vec<&QueueRow> {
        self.rows.iter().filter(|r| filter.keeps(r)).collect()
    }

    /// The plain-English maturity of `krate`, if known.
    pub fn maturity_of(&self, krate: &str) -> Option<&CrateMaturity> {
        self.maturity.iter().find(|m| m.krate == krate)
    }
}

/// `<root>/target/kovan-code-review/need-you-seen.txt`: the new-code
/// commits opened, one per line (a disposable cache under `target/`).
pub fn seen_path(root: &Path) -> PathBuf {
    root.join("target/kovan-code-review/need-you-seen.txt")
}

/// The opened commits recorded at `path` (empty when absent).
pub fn load_seen(path: &Path) -> BTreeSet<String> {
    std::fs::read_to_string(path)
        .map(|t| {
            t.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Record `commit` as opened at `path` (module doc).
pub fn mark_seen(path: &Path, commit: &str) -> Result<(), String> {
    let mut seen = load_seen(path);
    if !seen.insert(commit.to_string()) {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let text: String = seen.iter().map(|c| format!("{c}\n")).collect();
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

/// The recorded maturity of every crate from the Code Review data's
/// `code_map.json` (empty when it is not built).
fn recorded_maturity(root: &Path) -> BTreeMap<String, u8> {
    std::fs::read_to_string(crate::code_review_data::data_dir(root).join("code_map.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<CodeMap>(&t).ok())
        .map(|m| m.crates.into_iter().map(|c| (c.name, c.maturity)).collect())
        .unwrap_or_default()
}

/// Each function's oldest standing review commit, by `fn:` id.
fn review_commits(ws: &Workspace) -> BTreeMap<String, String> {
    let mut out: BTreeMap<String, (String, String)> = BTreeMap::new();
    for m in ws.reviews.values() {
        for r in m.doc.reviews() {
            let key = (r.review.date.clone(), r.review.commit.clone());
            out.entry(r.function_id())
                .and_modify(|v| {
                    if key < *v {
                        *v = key.clone();
                    }
                })
                .or_insert(key);
        }
    }
    out.into_iter().map(|(k, (_, c))| (k, c)).collect()
}

/// Fill each state row's authorship from git (module doc): the commits
/// since its review commit that touched the function's lines, or, for a
/// re-confirm, the changed callees' lines.
fn fill_authorship(
    root: &Path,
    head: &str,
    rows: &mut [QueueRow],
    info: &BTreeMap<String, FnInfo>,
    we: &WorkspaceEvaluation,
    max: usize,
) {
    for row in rows
        .iter_mut()
        .filter(|r| r.review_commit.is_some())
        .take(max)
    {
        let since = row.review_commit.clone();
        let Some(f) = row.functions.first() else {
            continue;
        };
        let mut targets: Vec<&FnInfo> = Vec::new();
        if let Some(StampState::InheritedStale {
            cause: InheritedCause::Callees(ids),
            ..
        }) = we.evaluation.functions.get(&f.id).map(|fr| &fr.state)
        {
            targets.extend(ids.iter().filter_map(|c| info.get(c)));
        } else if let Some(fi) = info.get(&f.id) {
            targets.push(fi);
        }
        let messages: Vec<String> = targets
            .iter()
            .flat_map(|t| change_messages(root, head, &t.file, t.lines, since.as_deref()))
            .collect();
        row.authorship = authorship_from_messages(&messages);
        if let Some(line) = row.authorship_line() {
            row.detail.push(format!("change: {line}"));
        }
    }
}

/// Build the queue for the workspace at `root` (module doc). Reads git and
/// every `review.md`; call it on a worker thread.
pub fn build(root: &Path, opts: &QueueOptions) -> Result<NeedYou, String> {
    let ws = load_workspace(root)?;
    let we = evaluate_loaded(root, &ws);
    let info = fn_info(&ws);
    let mut rows = rows_from_evaluation(&we, &info, &review_commits(&ws));
    fill_authorship(
        root,
        &ws.head,
        &mut rows,
        &info,
        &we,
        opts.max_authorship_rows,
    );
    let new: Vec<NewFn> = we
        .evaluation
        .functions
        .iter()
        .filter(|(_, fr)| fr.state.kind() == StateKind::New)
        .filter_map(|(id, fr)| {
            info.get(id).map(|fi| NewFn {
                id: id.clone(),
                file: fi.file.clone(),
                qual: fi.qual.clone(),
                untested: fr.untested,
            })
        })
        .collect();
    let commits = fold(&new, &scan(root, &ws.head, opts.max_commits));
    rows.extend(new_code_rows(&commits, &info, &load_seen(&seen_path(root))));
    order_rows(&mut rows);
    Ok(NeedYou {
        rows,
        maturity: crate_maturity(&we, &info, &recorded_maturity(root)),
        recent: recently_reviewed(&ws, &info, opts.recent),
        head: ws.head.clone(),
    })
}
