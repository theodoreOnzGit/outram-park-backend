//! The **⚑ need-you queue**, **recently reviewed** and **commit and push**
//! panel of the Code Review tab (GitHub #771, #740 U1/U5). Every decision
//! is in `crate::stamping` (`queue`, `relocate`, `recent`, `commit_push`),
//! tested there; this file draws the rows and turns clicks into those
//! calls.
//!
//! ```text
//! ┌ ⚑ 7 need you ────────────── [Recently reviewed ▾] ┐
//! │ Filter: (all) (agent only) (human only)  [Refresh] │
//! │ [Commit and push review.md]  2 files written       │
//! │ ── boon-lay ── Maturity 2: AI V&V (was 3: …). …    │
//! │ leaf — changed since review            [Expand ▾]  │
//! │ 3 moved functions · a → b · 3 ready  [Acknowledge] │
//! │ ＋ 140 new functions · b81e · 31 untested          │
//! │                       [Expand ▾] [Open on map ▸]   │
//! └────────────────────────────────────────────────────┘
//! ```
//!
//! Clicking a row's title (or a function in its details) opens that
//! function in the embedded Code Review UI
//! ([`kovan_web::ui::CodeReview::navigate`]). Authorship is not a label on
//! every row: it is in the details, and in the filter (#771, 2026-10-07).
//!
//! **No lag (root `CLAUDE.md`, HARD RULE).** Building the queue (git log,
//! `git log -L`, the engine), acknowledging, recording deletions, marking a
//! folded row opened and committing all run on [`Worker`] threads; the
//! frame only polls them with `try_write`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use egui::RichText;
use kovan_web::model::DeepLink;

use crate::stamping::commit_push::CommitPushJob;
use crate::stamping::flow::Worker;
use crate::stamping::queue::{
    build, mark_seen, seen_path, Filter, NeedYou, QueueOptions, QueueRow, RowKind,
};
use crate::stamping::relocate::{acknowledge_moves, record_deletions, RelocateReport};

type Built = Result<NeedYou, String>;
type Acted = Result<RelocateReport, String>;

/// The panel's state; owned by the Code Review view.
#[derive(Default)]
pub(crate) struct NeedYouView {
    /// The panel is shown.
    pub open: bool,
    /// The queue as last built.
    pub queue: Option<NeedYou>,
    pub error: Option<String>,
    pub filter: Filter,
    /// Expanded row keys.
    pub expanded: BTreeSet<String>,
    /// The recently-reviewed corner tab is unfolded.
    pub recent_open: bool,
    /// Rebuild before the next frame shown (after a write or a commit).
    pub dirty: bool,
    /// Messages from the last acknowledge or record.
    pub notes: Vec<String>,
    build_job: Option<Worker<Built>>,
    action: Option<Worker<Acted>>,
    /// The workspace the queue is for.
    root: Option<PathBuf>,
}

impl NeedYouView {
    /// The workspace changed: forget the queue.
    pub(crate) fn set_root(&mut self, root: Option<&Path>) {
        if self.root.as_deref() != root {
            let open = self.open;
            *self = Self {
                open,
                root: root.map(Path::to_path_buf),
                dirty: true,
                ..Self::default()
            };
        }
    }

    /// Build the queue on a worker (one at a time).
    pub(crate) fn rebuild(&mut self) {
        if self.build_job.is_some() {
            return;
        }
        if let Some(root) = self.root.clone() {
            self.dirty = false;
            self.build_job = Some(Worker::spawn(move || {
                build(&root, &QueueOptions::default())
            }));
        }
    }

    pub(crate) fn busy(&self) -> bool {
        self.build_job.is_some() || self.action.is_some()
    }

    /// Take finished jobs; never blocks.
    pub(crate) fn poll(&mut self, session: &mut BTreeSet<String>) {
        if let Some(r) = self.build_job.as_ref().and_then(Worker::try_take) {
            self.build_job = None;
            match r {
                Ok(q) => {
                    self.queue = Some(q);
                    self.error = None;
                }
                Err(e) => self.error = Some(e),
            }
        }
        if let Some(r) = self.action.as_ref().and_then(Worker::try_take) {
            self.action = None;
            match r {
                Ok(rep) => {
                    session.extend(rep.written.iter().cloned());
                    self.notes = vec![format!("Done: {}.", rep.done.len())];
                    self.notes.extend(
                        rep.skipped
                            .iter()
                            .map(|(id, why)| format!("Skipped {id}: {why}")),
                    );
                    self.dirty = true;
                }
                Err(e) => self.notes = vec![e],
            }
        }
    }

    /// The number shown on the ⚑ button (rows under the current filter).
    pub(crate) fn count(&self) -> Option<usize> {
        self.queue.as_ref().map(|q| q.filtered(self.filter).len())
    }

    /// Acknowledge the moves of `ids` (worker).
    pub(crate) fn acknowledge(&mut self, ids: Vec<String>) {
        if let (Some(root), None) = (self.root.clone(), &self.action) {
            self.action = Some(Worker::spawn(move || acknowledge_moves(&root, &ids)));
        }
    }

    /// Record the deletions of `ids` (worker).
    pub(crate) fn record(&mut self, ids: Vec<String>) {
        if let (Some(root), None) = (self.root.clone(), &self.action) {
            self.action = Some(Worker::spawn(move || record_deletions(&root, &ids)));
        }
    }

    /// A folded new-code row was opened: it clears from the next build
    /// (the record is written on a worker; a failure only means the row
    /// shows again).
    fn opened(&mut self, row: &QueueRow) {
        if let (RowKind::NewCode { commit, .. }, Some(root)) = (&row.kind, self.root.clone()) {
            let commit = commit.clone();
            let _ = Worker::spawn(move || mark_seen(&seen_path(&root), &commit));
        }
    }
}

/// The Commit-and-push button and its status, shared by this panel and the
/// stamp dialog's last step. GUI drawing code (exempt from the test rule):
/// the logic is `stamping::commit_push`, tested end to end there.
pub(crate) fn commit_push_controls(
    ui: &mut egui::Ui,
    job: &mut CommitPushJob,
    root: Option<&Path>,
    files: &BTreeSet<String>,
) {
    ui.horizontal_wrapped(|ui| {
        let can = root.is_some() && !files.is_empty() && !job.busy();
        let b = ui
            .add_enabled(can, egui::Button::new("Commit and push review.md"))
            .on_hover_text(
                "Commit only the review files kovan wrote in this session and push them to the \
                 current branch (never main). If the remote moved on, its commits are merged in \
                 first; nothing is ever reset.",
            )
            .on_disabled_hover_text(if files.is_empty() {
                "kovan has written nothing to commit in this session"
            } else {
                "A commit is running"
            });
        if b.clicked() {
            if let Some(root) = root {
                job.start(root, files.clone());
            }
        }
        if job.busy() {
            ui.spinner();
            ui.weak("Committing and pushing\u{2026}");
        } else if files.is_empty() {
            ui.weak("Nothing written this session.");
        } else {
            ui.weak(format!(
                "{} file{} written: {}",
                files.len(),
                if files.len() == 1 { "" } else { "s" },
                files.iter().cloned().collect::<Vec<_>>().join(", ")
            ));
        }
    });
    match job.status() {
        Some(Ok(t)) => {
            ui.label(t);
        }
        Some(Err(e)) => {
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
        None => {}
    }
}

/// What a click asked for.
enum Click {
    Open(String),
    OpenModule(String),
    Toggle(String),
    Opened(QueueRow),
    Acknowledge(Vec<String>),
    Record(Vec<String>),
}

/// One row. GUI drawing code (exempt from the test rule).
fn row_ui(ui: &mut egui::Ui, row: &QueueRow, expanded: bool, out: &mut Vec<Click>) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let colour = match row.kind {
            RowKind::Stale => ui.visuals().warn_fg_color,
            RowKind::Deleted | RowKind::Invalid | RowKind::Unreadable => ui.visuals().error_fg_color,
            _ => ui.visuals().text_color(),
        };
        let title = ui
            .add(egui::Label::new(RichText::new(&row.title).color(colour)).sense(egui::Sense::click()))
            .on_hover_text("Open on the map");
        if title.clicked() {
            if let Some(t) = row.open_target() {
                out.push(Click::Open(t.to_string()));
            }
            out.push(Click::Opened(row.clone()));
        }
        ui.horizontal_wrapped(|ui| {
            let label = if expanded { "Collapse \u{25b4}" } else { "Expand \u{25be}" };
            if ui.small_button(label).clicked() {
                out.push(Click::Toggle(row.key.clone()));
                out.push(Click::Opened(row.clone()));
            }
            if let RowKind::NewCode { .. } = row.kind {
                if ui.small_button("Open on map \u{25b8}").clicked() {
                    if let Some(f) = row.functions.first() {
                        let file = f.path.split_once("::").map_or(f.path.as_str(), |(a, _)| a);
                        out.push(Click::OpenModule(file.to_string()));
                    }
                    out.push(Click::Opened(row.clone()));
                }
            }
            if let RowKind::Moved { ready, .. } = &row.kind {
                if ui
                    .add_enabled(!ready.is_empty(), egui::Button::new(format!("Acknowledge {} \u{25b8}", ready.len())).small())
                    .on_hover_text("The regression tests that reach these functions passed: move their reviews with them (between review.md files when the folder changed).")
                    .on_disabled_hover_text("No function here is ready: its reaching tests have not passed, or identical code is at several places.")
                    .clicked()
                {
                    out.push(Click::Acknowledge(ready.clone()));
                }
            }
            if row.kind == RowKind::Deleted
                && ui
                    .small_button("Record deletion")
                    .on_hover_text("Remove its entries from review.md and add a row to the deleted-functions history (the old review stays in git).")
                    .clicked()
            {
                out.push(Click::Record(row.functions.iter().map(|f| f.id.clone()).collect()));
            }
        });
        if expanded {
            for d in &row.detail {
                ui.weak(d);
            }
            match row.authorship_line() {
                Some(a) => ui.weak(format!("authorship: {a}")),
                None => ui.weak("authorship: not known from git"),
            };
            for f in row.functions.iter().take(200) {
                if f.call_graph_id.is_empty() {
                    ui.monospace(&f.path);
                } else if ui.link(RichText::new(&f.path).monospace()).clicked() {
                    out.push(Click::Open(f.call_graph_id.clone()));
                }
            }
            if row.functions.len() > 200 {
                ui.weak(format!("\u{2026} and {} more", row.functions.len() - 200));
            }
        }
    });
}

/// The recently-reviewed corner tab. GUI drawing code (exempt).
fn recent_ui(ui: &mut egui::Ui, v: &mut NeedYouView, out: &mut Vec<Click>) {
    let Some(q) = &v.queue else { return };
    let n = q.recent.len();
    let label = if v.recent_open {
        "Recently reviewed \u{25b4}"
    } else {
        "Recently reviewed \u{25be}"
    };
    if ui.small_button(format!("{label} ({n})")).clicked() {
        v.recent_open = !v.recent_open;
    }
    if v.recent_open {
        if n == 0 {
            ui.weak("Nothing reviewed yet.");
        }
        for e in &q.recent {
            if e.call_graph_id.is_empty() {
                ui.weak(e.line());
            } else if ui.link(e.line()).on_hover_text(&e.path).clicked() {
                out.push(Click::Open(e.call_graph_id.clone()));
            }
        }
    }
}

/// Draw the panel (when open) as a right side panel of `ui`; returns where
/// to navigate the embedded Code Review UI. GUI drawing code (exempt from
/// the test rule): the queue's logic is `stamping::queue`, and
/// `code_review_view`'s headless test draws it with every row kind.
pub(crate) fn show(
    ui: &mut egui::Ui,
    v: &mut NeedYouView,
    session: &mut BTreeSet<String>,
    commit: &mut CommitPushJob,
) -> Option<DeepLink> {
    v.poll(session);
    if v.busy() || commit.busy() {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(200));
    }
    if !v.open {
        return None;
    }
    if v.dirty {
        v.rebuild();
    }
    let mut clicks = Vec::new();
    let width = (ui.available_width() * 0.85).min(420.0);
    egui::Panel::right("code-review-need-you")
        .resizable(true)
        .default_size(width)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(match v.count() {
                    Some(n) => format!("\u{2691} {n} need you"),
                    None => "\u{2691} need you".into(),
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    recent_ui(ui, v, &mut clicks);
                });
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Filter:");
                for f in Filter::ALL {
                    ui.selectable_value(&mut v.filter, f, f.label());
                }
                if ui
                    .add_enabled(v.build_job.is_none(), egui::Button::new("Refresh"))
                    .clicked()
                {
                    v.rebuild();
                }
                if v.busy() {
                    ui.spinner();
                }
            });
            commit_push_controls(ui, commit, v.root.as_deref(), session);
            if let Some(e) = &v.error {
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
            for n in &v.notes {
                ui.weak(n);
            }
            ui.separator();
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| {
                    let Some(q) = &v.queue else {
                        ui.weak("Building the queue\u{2026}");
                        return;
                    };
                    let rows = q.filtered(v.filter);
                    if rows.is_empty() {
                        ui.weak("Nothing needs you.");
                    }
                    let mut krate: Option<&str> = None;
                    let words: BTreeMap<&str, &str> = q
                        .maturity
                        .iter()
                        .map(|m| (m.krate.as_str(), m.words.as_str()))
                        .collect();
                    for row in rows {
                        if krate != Some(row.krate.as_str()) {
                            krate = Some(row.krate.as_str());
                            let name = if row.krate.is_empty() {
                                "new code"
                            } else {
                                row.krate.as_str()
                            };
                            ui.add_space(4.0);
                            ui.label(RichText::new(name).strong());
                            if let Some(w) = words.get(row.krate.as_str()) {
                                ui.weak(*w);
                            }
                        }
                        row_ui(ui, row, v.expanded.contains(&row.key), &mut clicks);
                    }
                });
        });
    let mut go = None;
    for c in clicks {
        match c {
            Click::Open(id) => go = Some(DeepLink::Function { id, source: true }),
            Click::OpenModule(file) => go = Some(DeepLink::Module(file)),
            Click::Toggle(k) => {
                if !v.expanded.remove(&k) {
                    v.expanded.insert(k);
                }
            }
            Click::Opened(row) => v.opened(&row),
            Click::Acknowledge(ids) => v.acknowledge(ids),
            Click::Record(ids) => v.record(ids),
        }
    }
    go
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stamping::queue::{CrateMaturity, QueueFn};
    use crate::stamping::recent::{RecentEntry, RecentWhat};
    use kovan_common::review::state::FlagKind;
    use kovan_common::review::types::{AuthorshipKind, ChangeAuthorship};

    fn row(key: &str, kind: RowKind, krate: &str) -> QueueRow {
        QueueRow {
            key: key.into(),
            title: format!("{key} \u{2014} {}", kind.label()),
            kind,
            krate: krate.into(),
            functions: vec![QueueFn {
                id: format!("fn:{key}"),
                call_graph_id: format!("crates/a/src/lib.rs::{key}"),
                path: format!("crates/a/src/lib.rs::{key}"),
            }],
            detail: vec!["why".into()],
            authorship: Some(ChangeAuthorship {
                kind: AuthorshipKind::Agent,
                sessions: vec!["https://claude.ai/code/session_x".into()],
            }),
            since: Some("2026-10-01".into()),
            review_commit: None,
        }
    }

    /// Headless: the panel draws every row kind expanded, the crate's
    /// plain-English maturity, the recently-reviewed tab unfolded and the
    /// commit controls, without a display or a panic, and never starts a
    /// build or a commit by itself; a closed panel draws nothing. The
    /// workers (acknowledge, record) run on a temporary folder that is not
    /// a workspace and report back through `poll`. Temporary folders only.
    #[test]
    fn the_panel_draws_every_row_kind_headless() {
        let d = tempfile::tempdir().unwrap();
        let mut v = NeedYouView::default();
        v.set_root(Some(d.path()));
        let kinds = [
            RowKind::Stale,
            RowKind::ReConfirm { blocked: true },
            RowKind::Fixed,
            RowKind::DocChanged,
            RowKind::Moved {
                from_dir: "a".into(),
                to_dir: "b".into(),
                ready: vec!["fn:m".into()],
            },
            RowKind::Deleted,
            RowKind::Unreadable,
            RowKind::Invalid,
            RowKind::Flag(FlagKind::IndependentVvNotCounted),
            RowKind::NewCode {
                commit: "b81e00".into(),
                untested: 31,
            },
        ];
        let rows: Vec<QueueRow> = kinds
            .into_iter()
            .enumerate()
            .map(|(i, k)| row(&format!("r{i}"), k, if i < 5 { "a" } else { "b" }))
            .collect();
        v.expanded = rows.iter().map(|r| r.key.clone()).collect();
        v.queue = Some(NeedYou {
            rows,
            maturity: vec![CrateMaturity {
                krate: "a".into(),
                recorded: 3,
                stale: 2,
                words: kovan_common::code_map::plain_maturity::plain_maturity(3, 2),
            }],
            recent: vec![RecentEntry {
                what: RecentWhat::Stamped,
                path: "crates/a/src/lib.rs::f".into(),
                call_graph_id: "crates/a/src/lib.rs::f".into(),
                by: "github:a".into(),
                date: "2026-10-10".into(),
                when: "2026-10-10T10:00:00+08:00".into(),
            }],
            head: String::new(),
        });
        v.dirty = false;
        v.recent_open = true;
        assert_eq!(v.count(), Some(10));
        v.filter = Filter::HumanOnly;
        assert_eq!(v.count(), Some(0));
        v.filter = Filter::All;
        let mut session = BTreeSet::from(["crates/a/src/review.md".to_string()]);
        let mut commit = CommitPushJob::default();
        let ctx = egui::Context::default();
        for open in [false, true, true] {
            v.open = open;
            let _ = ctx.run_ui(Default::default(), |ui| {
                assert_eq!(show(ui, &mut v, &mut session, &mut commit), None);
                commit_push_controls(ui, &mut commit, Some(d.path()), &session);
            });
        }
        assert!(
            !commit.busy() && commit.last.is_none(),
            "nothing starts by itself"
        );
        assert!(v.build_job.is_none());

        v.acknowledge(vec!["fn:m".into()]);
        let start = std::time::Instant::now();
        while v.busy() {
            assert!(start.elapsed() < std::time::Duration::from_secs(60));
            std::thread::sleep(std::time::Duration::from_millis(5));
            v.poll(&mut session);
        }
        assert!(!v.notes.is_empty());
        v.record(vec!["fn:gone".into()]);
        while v.busy() {
            assert!(start.elapsed() < std::time::Duration::from_secs(60));
            std::thread::sleep(std::time::Duration::from_millis(5));
            v.poll(&mut session);
        }
        assert!(!v.notes.is_empty(), "{:?}", v.notes);
        v.set_root(None);
        assert!(v.queue.is_none());
    }
}
