//! **Review mode** in desktop kovan's Code Review tab (GitHub #770; #740
//! decisions 1–11, U2–U4): the drawing. Every decision is in
//! [`crate::stamping::review_mode`] (GUI-free, tested there); this file
//! draws the current state and turns clicks into calls.
//!
//! ```text
//!  map (kovan-web)  ──[Review >] / [Review this walk >]──>  review mode
//!  ┌──────────────────────────────────────────┬──────────────────────────┐
//!  │ name · state · flags (#783) · blocked by │ review.md (this folder)  │
//!  │ [Source] [Diff since review] [Callees]   │  this function's entries │
//!  │ [Upstream] [Quick fix in kvim]           │  banded; Edit = comments │
//!  │                                          │  only (literature UX)    │
//!  │  source with highlights, or the unified  │ Comments for your stamp  │
//!  │  diff; click line numbers to select,     │ [Stamp] [Needs fix]      │
//!  │  then Annotate                           │ [Save, back to map]      │
//!  │                                          │ [Cancel] walk: trail,    │
//!  │                                          │ [Skip] [Next >], summary │
//!  └──────────────────────────────────────────┴──────────────────────────┘
//! ```
//!
//! Added 2026-10-10 (#770): [Re-confirm] on an inherited-stale function
//! (#740 decision 8; the dialog's re-confirm step), the linked concepts and
//! their implemented formulas above `review.md` (#739 decision 21), and the
//! function's file watched ([`FnWatch`]): a saved fix reloads the view, so
//! ⛔ needs fix turns into ✏ fixed without reopening it.
//!
//! The map, the rings and the source panel stay kovan-web's (maintainer:
//! "reusing code from web kovan"); review mode is drawn by the host in
//! their place and always returns to the map (#740 decision 5).
//!
//! **No lag (root `CLAUDE.md`, HARD RULE).** Loading the view (git, the
//! staleness engine), planning a walk, saving a section, writing a
//! highlight and the quick-fix reads and writes all run on
//! [`Worker`] threads; a frame only polls them with `try_write`.

use std::path::PathBuf;

use egui::{Color32, RichText};
use kovan_common::review::signing::keystore::Keystore;
use kovan_web::ui::{FunctionRef, HostRequest};

use super::kvim_editor::KvimEditorState;
use crate::stamping::flow::{Purpose, Worker};
use crate::stamping::review_mode::diff::{DiffLine, FnDiff};
use crate::stamping::review_mode::quick_fix::{self, QuickFix};
use crate::stamping::review_mode::walk::{
    huge_warning, next, Next, StepOutcome, Trail, WalkPlan, WalkSummary,
};
use crate::stamping::review_mode::watch::{FnWatch, POLL_INTERVAL};
use crate::stamping::review_mode::{
    add_highlight, function_view, plan_walk, save_section, FunctionView, Session, Snapshot,
};

/// Which code the main view shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CodeTab {
    Source,
    Diff,
    Callees,
}

/// A finished side job: a notice, or an error.
type Done = Result<String, String>;

/// What review mode asks the host to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PanelRequest {
    /// Open the stamp dialog (Stamp or Needs fix), with the sidebar's
    /// comments as its seed.
    Dialog(HostRequest, String),
    /// Open the stamp dialog on its re-confirm step (added 2026-10-10,
    /// #770, #740 decision 8).
    Reconfirm(FunctionRef),
    /// Move kovan-web's map to this function (call-graph id).
    Goto(String),
}

/// One function in review mode.
pub(crate) struct Active {
    pub(crate) target: FunctionRef,
    session: Option<Session>,
    pub(crate) view: Option<FunctionView>,
    load: Option<Worker<Result<FunctionView, String>>>,
    pub(crate) tab: CodeTab,
    md: KvimEditorState,
    md_loaded: String,
    /// The entry being edited (its heading line) and its comments buffer.
    editing: Option<(usize, KvimEditorState)>,
    /// Comments for the stamp (the wizard's Comments seed).
    draft: KvimEditorState,
    quick: Option<(QuickFix, KvimEditorState)>,
    quick_load: Option<Worker<Result<(QuickFix, bool), String>>>,
    job: Option<Worker<Done>>,
    /// Selected source lines (1-based within the function) for a highlight.
    sel: Option<(usize, usize)>,
    note: String,
    confirm_cancel: bool,
    /// The function's file, polled for a saved change (#740).
    watch: Option<FnWatch>,
}

impl Active {
    fn new(target: FunctionRef) -> Active {
        Active {
            target,
            session: None,
            view: None,
            load: None,
            tab: CodeTab::Source,
            md: KvimEditorState::default(),
            md_loaded: String::new(),
            editing: None,
            draft: KvimEditorState::default(),
            quick: None,
            quick_load: None,
            job: None,
            sel: None,
            note: String::new(),
            confirm_cancel: false,
            watch: None,
        }
    }
}

/// A walk in progress.
pub(crate) struct WalkState {
    pub(crate) target: String,
    pub(crate) plan: Option<WalkPlan>,
    pub(crate) trail: Trail,
    pub(crate) started: bool,
    job: Option<Worker<Result<WalkPlan, String>>>,
}

/// Review mode's state; owned by the Code Review view.
#[derive(Default)]
pub(crate) struct ReviewModePanel {
    root: Option<PathBuf>,
    keystore: Option<Keystore>,
    pub(crate) active: Option<Active>,
    pub(crate) walk: Option<WalkState>,
    pub(crate) message: Option<Result<String, String>>,
    requests: Vec<PanelRequest>,
    /// How often the watched file is read (`None`: [`POLL_INTERVAL`];
    /// tests shorten it).
    pub(crate) watch_interval: Option<std::time::Duration>,
}

/// The view of `function` over `root`, on a worker.
fn load_view(root: PathBuf, function: String) -> Worker<Result<FunctionView, String>> {
    Worker::spawn(move || {
        let snap = Snapshot::load(&root)?;
        function_view(&root, &snap, &function)
    })
}

/// The walk to `function` over `root`, on a worker.
fn load_plan(root: PathBuf, function: String) -> Worker<Result<WalkPlan, String>> {
    Worker::spawn(move || {
        let snap = Snapshot::load(&root)?;
        plan_walk(&snap, &function)
    })
}

impl ReviewModePanel {
    /// The workspace and the keystore highlights take their reviewer from
    /// (`None`: no keystore, so no highlight can be written).
    pub(crate) fn set_root(&mut self, root: Option<PathBuf>, keystore: Option<Keystore>) {
        if self.root != root {
            *self = ReviewModePanel {
                root,
                keystore,
                ..ReviewModePanel::default()
            };
        } else {
            self.keystore = keystore;
        }
    }

    /// Requests for the host, once.
    pub(crate) fn take_requests(&mut self) -> Vec<PanelRequest> {
        std::mem::take(&mut self.requests)
    }

    /// Enter review mode on `f` (entering records `review.md` as it is, for
    /// Cancel; on a worker with the view).
    pub(crate) fn enter(&mut self, f: FunctionRef) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let mut a = Active::new(f.clone());
        a.load = Some(load_view(root, f.id.clone()));
        self.active = Some(a);
        self.message = None;
    }

    /// Plan the walk to `f` (shown with its size before it starts).
    pub(crate) fn plan(&mut self, f: &FunctionRef) {
        let Some(root) = self.root.clone() else {
            return;
        };
        self.walk = Some(WalkState {
            target: f.id.clone(),
            plan: None,
            trail: Trail::default(),
            started: false,
            job: Some(load_plan(root, f.id.clone())),
        });
    }

    /// The stamp dialog finished with `what` on the active function: record
    /// it on the walk and reload the view and the plan.
    pub(crate) fn outcome(&mut self, what: Purpose) {
        let Some(a) = &self.active else { return };
        if let (Some(w), Some(v)) = (self.walk.as_mut(), &a.view) {
            let o = match what {
                Purpose::Stamp | Purpose::Reconfirm => StepOutcome::Stamped,
                Purpose::NeedsFix => StepOutcome::NeedsFix,
            };
            w.trail.record(&v.fn_id, &v.qual, o);
        }
        self.reload();
    }

    /// Reload the active view and the walk's plan (after a write, or after
    /// the dialog closed without one, so a changed function's new diff is
    /// shown first). A no-op outside review mode and with no walk.
    pub(crate) fn reload(&mut self) {
        let Some(root) = self.root.clone() else {
            return;
        };
        if let Some(a) = self.active.as_mut() {
            a.load = Some(load_view(root.clone(), a.target.id.clone()));
        }
        if let Some(w) = self.walk.as_mut() {
            w.job = Some(load_plan(root, w.target.clone()));
        }
    }

    /// Poll every worker; never blocks. Returns whether one still runs.
    pub(crate) fn poll(&mut self) -> bool {
        let mut busy = false;
        let root = self.root.clone();
        if let Some(w) = self.walk.as_mut() {
            if let Some(j) = &w.job {
                match j.try_take() {
                    Some(Ok(p)) => {
                        w.plan = Some(p);
                        w.job = None;
                    }
                    Some(Err(e)) => {
                        self.message = Some(Err(format!("Walk: {e}")));
                        w.job = None;
                    }
                    None => busy = true,
                }
            }
        }
        let Some(a) = self.active.as_mut() else {
            return busy;
        };
        if let Some(j) = &a.load {
            match j.try_take() {
                Some(Ok(v)) => {
                    if a.session.is_none() {
                        if let Some(r) = &root {
                            a.session = Some(Session::enter(r, &v.review_md));
                        }
                    }
                    if a.md_loaded != v.review_md_text {
                        a.md.load_text(&v.review_md_text);
                        if let Some(s) = v.sections.first() {
                            a.md.jump_to_line(s.line);
                        }
                        a.md_loaded = v.review_md_text.clone();
                    }
                    if a.view.is_none()
                        && v.own_diff
                            .as_ref()
                            .is_some_and(|d| d.as_ref().is_ok_and(|d| !d.diff.unchanged()))
                    {
                        a.tab = CodeTab::Diff;
                    }
                    if let Some(r) = &root {
                        if !a.watch.as_ref().is_some_and(|w| w.watches(&v.file, &v.qual)) {
                            a.watch = Some(FnWatch::new(
                                r.clone(),
                                v.file.clone(),
                                v.qual.clone(),
                                self.watch_interval.unwrap_or(POLL_INTERVAL),
                            ));
                        }
                    }
                    a.view = Some(v);
                    a.load = None;
                }
                Some(Err(e)) => {
                    self.message = Some(Err(e));
                    a.load = None;
                }
                None => busy = true,
            }
        }
        if let Some(j) = &a.job {
            match j.try_take() {
                Some(r) => {
                    a.job = None;
                    self.message = Some(r);
                    if let Some(root) = root.clone() {
                        a.load = Some(load_view(root, a.target.id.clone()));
                    }
                }
                None => busy = true,
            }
        }
        if let Some(w) = a.watch.as_mut() {
            if w.poll(std::time::Instant::now()) && a.load.is_none() {
                if let Some(root) = root.clone() {
                    a.load = Some(load_view(root, a.target.id.clone()));
                    self.message = Some(Ok(format!(
                        "{} changed on disk: its state was re-evaluated.",
                        a.view.as_ref().map(|v| v.file.as_str()).unwrap_or("the file")
                    )));
                }
            }
        }
        if let Some(j) = &a.quick_load {
            match j.try_take() {
                Some(Ok((q, template))) => {
                    let mut ed = KvimEditorState::default();
                    let text = if template {
                        quick_fix::with_deviation_template(&q.original)
                    } else {
                        q.original.clone()
                    };
                    ed.load_text(&text);
                    a.quick = Some((q, ed));
                    a.quick_load = None;
                }
                Some(Err(e)) => {
                    self.message = Some(Err(e));
                    a.quick_load = None;
                }
                None => busy = true,
            }
        }
        busy
    }

    /// Leave review mode. `discard`: Cancel, putting `review.md` back as it
    /// was on entering (#740 decision 6); else Save (what was written stays,
    /// nothing is committed).
    fn leave(&mut self, discard: bool) {
        if let (true, Some(a), Some(root)) = (discard, &self.active, &self.root) {
            if let Some(s) = &a.session {
                self.message = Some(match s.discard(root) {
                    Ok(true) => Ok(format!("Cancelled: {} put back as it was.", s.review_md)),
                    Ok(false) => Ok("Cancelled: nothing had changed.".into()),
                    Err(e) => Err(e),
                });
            }
        }
        self.active = None;
    }

    /// Open the next function of the walk (or end it with the summary).
    fn walk_next(&mut self) {
        let Some(w) = &self.walk else { return };
        let Some(p) = &w.plan else { return };
        if let Next::Review(i) = next(p, &w.trail) {
            let f = FunctionRef {
                id: i.call_graph_id.clone(),
                krate: String::new(),
                file: i
                    .call_graph_id
                    .split_once("::")
                    .map(|(f, _)| f.to_string())
                    .unwrap_or_default(),
                name: i.name.clone(),
                callees: None,
                lines: None,
            };
            self.requests.push(PanelRequest::Goto(f.id.clone()));
            self.enter(f);
        } else {
            self.active = None;
        }
    }

    /// Skip the active function on the walk.
    fn skip(&mut self) {
        if let (Some(w), Some(v)) = (
            self.walk.as_mut(),
            self.active.as_ref().and_then(|a| a.view.as_ref()),
        ) {
            w.trail.record(&v.fn_id, &v.qual, StepOutcome::Skipped);
        }
        self.walk_next();
    }

    // ---- drawing ------------------------------------------------------------

    /// The walk card above the map: the planned walk's size and warning
    /// before it starts, or the trail and the summary. GUI drawing code
    /// (exempt from the test rule; the logic is `review_mode::walk`).
    pub(crate) fn walk_strip(&mut self, ui: &mut egui::Ui) {
        let mut start = false;
        let mut close = false;
        let mut resume = false;
        let Some(w) = &self.walk else { return };
        ui.horizontal_wrapped(|ui| {
            let Some(p) = &w.plan else {
                ui.spinner();
                ui.weak("Planning the walk\u{2026}");
                if ui.button("Close").clicked() {
                    close = true;
                }
                return;
            };
            let s = p.size();
            let name = p.target_item().map(|i| i.name.clone()).unwrap_or_default();
            if !w.started {
                ui.label(RichText::new(format!("Walk to {name}")).strong());
                ui.label(format!(
                    "{} functions to review ({} lines, {} with an open needs fix); {} already valid of {}",
                    s.functions, s.lines, s.needs_fix, s.already_valid, s.total
                ));
                if let Some(warn) = huge_warning(&s) {
                    ui.colored_label(ui.visuals().warn_fg_color, warn);
                }
                if ui.button("Start the walk").clicked() {
                    start = true;
                }
            } else {
                ui.label(RichText::new(format!("Walk to {name}:")).strong());
                if w.trail.steps.is_empty() {
                    ui.weak("nothing reviewed yet");
                } else {
                    ui.monospace(w.trail.breadcrumb());
                }
                match next(p, &w.trail) {
                    Next::Done => {
                        ui.label(RichText::new(format!("Walk finished: {}", WalkSummary::new(p, &w.trail).text())).strong());
                    }
                    Next::Stuck(s) => {
                        ui.colored_label(
                            ui.visuals().warn_fg_color,
                            format!("{} function(s) wait on callees not yet valid (skipped or needs fix).", s.len()),
                        );
                        ui.label(WalkSummary::new(p, &w.trail).text());
                    }
                    Next::Review(i) => {
                        if self.active.is_none() && ui.button(format!("Next > {}", i.name)).clicked() {
                            resume = true;
                        }
                    }
                }
            }
            if ui.button(if w.started { "Done" } else { "Close" }).clicked() {
                close = true;
            }
        });
        if start {
            if let Some(w) = self.walk.as_mut() {
                w.started = true;
            }
            self.walk_next();
        }
        if resume {
            self.walk_next();
        }
        if close {
            self.walk = None;
        }
    }

    /// Draw review mode in `ui` (the Code Review tab's area). GUI drawing
    /// code (exempt from the test rule; the logic is
    /// `stamping::review_mode`, and `code_review_view`'s headless test
    /// draws it).
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui) {
        if self.poll() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(150));
        } else if self.active.is_some() {
            // The file watch (#740) reads on its own; wake to take it.
            ui.ctx()
                .request_repaint_after(self.watch_interval.unwrap_or(POLL_INTERVAL));
        }
        let root = self.root.clone();
        let keystore = self.keystore.clone();
        let walking = self.walk.as_ref().is_some_and(|w| w.started);
        let mut leave: Option<bool> = None;
        let mut skip = false;
        let mut enter: Option<FunctionRef> = None;
        let message = self.message.clone();
        let Some(a) = self.active.as_mut() else {
            return;
        };
        let mut requests: Vec<PanelRequest> = Vec::new();
        let width = ui.max_rect().width();
        egui::Panel::right("review_mode_sidebar")
            .default_size((width * 0.38).clamp(300.0, 560.0))
            .min_size(260.0)
            .resizable(true)
            .show(ui, |ui| {
                sidebar(
                    ui,
                    a,
                    root.as_ref(),
                    walking,
                    &mut requests,
                    &mut leave,
                    &mut skip,
                );
            });
        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(m) = &message {
                match m {
                    Ok(t) => ui.weak(t),
                    Err(e) => ui.colored_label(ui.visuals().error_fg_color, e),
                };
            }
            main_view(ui, a, root.as_ref(), keystore.as_ref(), &mut enter);
        });
        self.requests.extend(requests);
        if let Some(f) = enter {
            self.requests.push(PanelRequest::Goto(f.id.clone()));
            self.enter(f);
        } else if let Some(discard) = leave {
            self.leave(discard);
        } else if skip {
            self.skip();
        }
    }
}

/// The header, the code tabs and the code. GUI drawing code (exempt).
fn main_view(
    ui: &mut egui::Ui,
    a: &mut Active,
    root: Option<&PathBuf>,
    keystore: Option<&Keystore>,
    enter: &mut Option<FunctionRef>,
) {
    ui.horizontal_wrapped(|ui| {
        ui.heading(&a.target.name);
        ui.monospace(&a.target.id);
        if a.load.is_some() || a.job.is_some() || a.quick_load.is_some() {
            ui.spinner();
        }
    });
    let Some(v) = a.view.clone() else {
        ui.weak("Loading the function, its review and its diff\u{2026}");
        return;
    };
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(format!("State: {}", v.state.label())).strong());
        if !v.reason.is_empty() {
            ui.weak(&v.reason);
        }
        if let Some(s) = &v.since {
            ui.weak(format!(
                "last reviewed by {} on {} at {}",
                s.by,
                s.date,
                s.commit.chars().take(10).collect::<String>()
            ));
        } else {
            ui.weak("never reviewed");
        }
    });
    for f in &v.flags {
        ui.colored_label(ui.visuals().warn_fg_color, f);
    }
    if !v.blocked_by.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.colored_label(
                ui.visuals().error_fg_color,
                "Blocked (bottom-up): review first",
            );
            for (id, name) in &v.blocked_by {
                if ui
                    .link(RichText::new(name).monospace())
                    .on_hover_text(id)
                    .clicked()
                {
                    *enter = Some(FunctionRef {
                        id: id.clone(),
                        krate: String::new(),
                        file: String::new(),
                        name: name.clone(),
                        callees: None,
                        lines: None,
                    });
                }
            }
        });
    }
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(&mut a.tab, CodeTab::Source, "Source");
        let diff_label = match &v.own_diff {
            None => "Diff (never reviewed)".to_string(),
            Some(Ok(d)) => {
                let (p, m) = d.diff.counts();
                format!("Diff since review (+{p} -{m})")
            }
            Some(Err(_)) => "Diff (unavailable)".to_string(),
        };
        ui.selectable_value(&mut a.tab, CodeTab::Diff, diff_label);
        if !v.callee_diffs.is_empty() {
            ui.selectable_value(&mut a.tab, CodeTab::Callees, format!("Changed callees ({})", v.callee_diffs.len()));
        }
        if let Some(url) = &v.upstream_url {
            if ui.button("Upstream").on_hover_text(url).clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(url));
            }
        }
        let idle = a.quick.is_none() && a.quick_load.is_none();
        if let Some(root) = root {
            if ui
                .add_enabled(idle, egui::Button::new("Quick fix in kvim"))
                .on_hover_text("Edit this function's own lines in kvim (quick fixes only; your own Neovim is opened by you, from the command line)")
                .clicked()
            {
                let (r, file, qual) = (root.clone(), v.file.clone(), v.qual.clone());
                a.quick_load = Some(Worker::spawn(move || quick_fix::open(&r, &file, &qual).map(|q| (q, false))));
            }
            if v.is_port
                && ui
                    .add_enabled(idle, egui::Button::new("Document a deviation"))
                    .on_hover_text("Open kvim at the doc comment with a deviation template (what differs, why)")
                    .clicked()
            {
                let (r, file, qual) = (root.clone(), v.file.clone(), v.qual.clone());
                a.quick_load = Some(Worker::spawn(move || quick_fix::open(&r, &file, &qual).map(|q| (q, true))));
            }
        }
    });
    ui.separator();
    if let Some((q, ed)) = a.quick.as_mut() {
        let mut close = false;
        let mut save = false;
        ui.horizontal(|ui| {
            ui.strong(format!(
                "Quick fix: {} lines {}-{}",
                q.file, q.lines[0], q.lines[1]
            ));
            if ui.button("Save").clicked() {
                save = true;
            }
            if ui.button("Close").clicked() {
                close = true;
            }
        });
        ui.weak("Saving changes the function's hash: commit it, and the review shows the new diff before a stamp.");
        if let Some(sig) = ed.ui(ui, None) {
            use super::kvim_editor::EditorSignal;
            match sig {
                EditorSignal::Write => save = true,
                EditorSignal::Quit => close = true,
                _ => {}
            }
        }
        if save {
            if let Some(root) = root {
                let (r, q2, text) = (root.clone(), q.clone(), ed.text());
                a.job = Some(Worker::spawn(move || {
                    quick_fix::save(&r, &q2, &text)
                        .map(|_| format!("Saved {} (not committed).", q2.file))
                }));
            }
            a.quick = None;
        } else if close {
            a.quick = None;
        }
        return;
    }
    match a.tab {
        CodeTab::Source => source_view(ui, a, &v, root, keystore),
        CodeTab::Diff => match &v.own_diff {
            None => {
                ui.weak("Never reviewed: there is no earlier review to diff against. The Source tab shows the code.");
            }
            Some(Err(e)) => {
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
            Some(Ok(d)) => diff_view(ui, d, "own"),
        },
        CodeTab::Callees => {
            egui::ScrollArea::vertical().id_salt("review_mode_callees").show(ui, |ui| {
                ui.weak("Inherited stale: these callees changed since the review. This function is read-only alongside; re-confirm or re-review it.");
                for (name, d) in &v.callee_diffs {
                    ui.label(RichText::new(name).strong().monospace());
                    match d {
                        Ok(d) => diff_view(ui, d, name),
                        Err(e) => {
                            ui.colored_label(ui.visuals().error_fg_color, e);
                        }
                    }
                    ui.separator();
                }
            });
        }
    }
}

/// The source with line numbers, highlights tinted, line selection and
/// Annotate. GUI drawing code (exempt).
fn source_view(
    ui: &mut egui::Ui,
    a: &mut Active,
    v: &FunctionView,
    root: Option<&PathBuf>,
    keystore: Option<&Keystore>,
) {
    if let Some((s, e)) = a.sel {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Lines {s}-{e} selected. Note:"));
            ui.add(egui::TextEdit::singleline(&mut a.note).desired_width(260.0));
            let can = root.is_some()
                && keystore.is_some()
                && a.job.is_none()
                && a.note.trim().chars().count() >= 2;
            if ui
                .add_enabled(can, egui::Button::new("Annotate"))
                .on_disabled_hover_text(
                    "Write a note of at least 2 characters (a key must be set up: Stamp once)",
                )
                .clicked()
            {
                if let (Some(r), Some(ks)) = (root, keystore) {
                    let (r, ks, f, note) = (
                        r.clone(),
                        ks.clone(),
                        a.target.id.clone(),
                        a.note.trim().to_string(),
                    );
                    a.job = Some(Worker::spawn(move || {
                        let by = ks
                            .list()
                            .map_err(|e| e.to_string())?
                            .into_iter()
                            .next()
                            .map(|k| k.reviewer)
                            .ok_or("no key in the keystore: set one up with Stamp first")?;
                        add_highlight(&r, &f, &by, s, e, &note).map(|p| {
                            format!("Highlight written to {} (not committed).", p.display())
                        })
                    }));
                    a.note.clear();
                    a.sel = None;
                }
            }
            if ui.button("Clear").clicked() {
                a.sel = None;
            }
        });
    } else {
        ui.weak("Click a line number to select it (shift-click to extend), then Annotate.");
    }
    let orphans: Vec<&str> = v
        .highlights
        .iter()
        .filter(|h| h.lines.is_none())
        .map(|h| h.note.as_str())
        .collect();
    if !orphans.is_empty() {
        ui.colored_label(
            ui.visuals().warn_fg_color,
            format!(
                "{} orphaned highlight(s): the code they marked is gone.",
                orphans.len()
            ),
        );
    }
    let tint = Color32::from_rgba_unmultiplied(255, 220, 60, 40);
    let shift = ui.input(|i| i.modifiers.shift);
    egui::ScrollArea::both()
        .id_salt("review_mode_source")
        .auto_shrink(false)
        .show(ui, |ui| {
            for (i, line) in v.source.lines().enumerate() {
                let n = i + 1;
                let hl = v
                    .highlights
                    .iter()
                    .find(|h| h.lines.is_some_and(|(a, b)| a <= n && n <= b));
                let selected = a.sel.is_some_and(|(s, e)| s <= n && n <= e);
                ui.horizontal(|ui| {
                    let num = ui.add(
                        egui::Label::new(
                            RichText::new(format!("{:>4}", v.lines[0] as usize + i))
                                .monospace()
                                .weak(),
                        )
                        .sense(egui::Sense::click()),
                    );
                    if num.clicked() {
                        a.sel = match (a.sel, shift) {
                            (Some((s, _)), true) => Some((s.min(n), s.max(n))),
                            _ => Some((n, n)),
                        };
                    }
                    let mut t = RichText::new(line).monospace();
                    if selected {
                        t = t.background_color(ui.visuals().selection.bg_fill);
                    } else if hl.is_some() {
                        t = t.background_color(tint);
                    }
                    let r = ui.label(t);
                    if let Some(h) = hl {
                        r.on_hover_text(format!(
                            "{}: {}{}",
                            h.by,
                            h.note,
                            if h.fuzzy {
                                " (re-anchored: the code here changed)"
                            } else {
                                ""
                            }
                        ));
                    }
                });
            }
        });
}

/// A unified diff: removed lines struck, added lines marked. GUI drawing
/// code (exempt).
pub(crate) fn diff_view(ui: &mut egui::Ui, d: &FnDiff, salt: &str) {
    ui.weak(format!(
        "{} since commit {}{}",
        d.path,
        d.since.chars().take(10).collect::<String>(),
        if d.diff.coarse {
            " (too long to align: every old line removed, every new line added)"
        } else {
            ""
        }
    ));
    if d.diff.unchanged() {
        ui.weak("Unchanged since the review.");
    }
    let added = Color32::from_rgb(90, 190, 110);
    let removed = ui.visuals().error_fg_color;
    egui::ScrollArea::both()
        .id_salt(("review_mode_diff", salt))
        .auto_shrink(false)
        .show(ui, |ui| {
            for l in &d.diff.lines {
                let t = match l {
                    DiffLine::Same { new, text, .. } => {
                        RichText::new(format!("{:>4}   {text}", d.lines_now[0] as usize + new - 1))
                            .monospace()
                    }
                    DiffLine::Removed { text, .. } => RichText::new(format!("     - {text}"))
                        .monospace()
                        .color(removed)
                        .strikethrough(),
                    DiffLine::Added { new, text } => {
                        RichText::new(format!("{:>4} + {text}", d.lines_now[0] as usize + new - 1))
                            .monospace()
                            .color(added)
                    }
                };
                ui.label(t);
            }
        });
}

/// The linked concepts and their implemented formulas, above the review
/// (#739 decision 21). GUI drawing code (exempt; the lookup is
/// `stamping::concepts`, tested there).
fn concepts(ui: &mut egui::Ui, v: &FunctionView) {
    if v.concepts.is_empty() {
        ui.weak("No concept linked: link one from the stamp wizard's concept finder.");
        return;
    }
    for c in &v.concepts {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("Implements: {}", c.title)).strong());
            ui.monospace(&c.id);
        });
        if c.formulas.is_empty() {
            ui.weak("No formula recorded for this concept.");
        }
        for f in &c.formulas {
            ui.label(RichText::new(&f.heading).italics())
                .on_hover_text(format!("{} ({})", f.file, f.id));
            ui.add(egui::Label::new(RichText::new(&f.body).monospace()).wrap());
        }
    }
}

/// The `review.md` sidebar, the stamp comments, the actions and the walk.
/// GUI drawing code (exempt).
fn sidebar(
    ui: &mut egui::Ui,
    a: &mut Active,
    root: Option<&PathBuf>,
    walking: bool,
    requests: &mut Vec<PanelRequest>,
    leave: &mut Option<bool>,
    skip: &mut bool,
) {
    let Some(v) = a.view.clone() else {
        ui.weak("review.md: loading\u{2026}");
        if ui.button("Back to map").clicked() {
            *leave = Some(false);
        }
        return;
    };
    ui.horizontal_wrapped(|ui| {
        let target = || {
            let f = a.target.clone();
            FunctionRef { lines: Some((v.lines[0], v.lines[1])), file: if f.file.is_empty() { v.file.clone() } else { f.file }, ..f }
        };
        let blocked = !v.blocked_by.is_empty();
        if ui
            .add_enabled(!blocked, egui::Button::new("Stamp"))
            .on_disabled_hover_text("Bottom-up: review the blocking callees first")
            .clicked()
        {
            requests.push(PanelRequest::Dialog(HostRequest::Stamp(target()), a.draft.text()));
        }
        if let Some(test_failed) = v.reconfirm {
            let can = !test_failed && !blocked;
            let why = if test_failed {
                "A test reaching this function failed: re-confirm is enabled once the reaching tests pass"
            } else {
                "Bottom-up: review the blocking callees first"
            };
            if ui
                .add_enabled(can, egui::Button::new("Re-confirm"))
                .on_hover_text("Only callees changed: look at their diffs (Changed callees) and sign a re-confirmation of your previous answers, without the wizard")
                .on_disabled_hover_text(why)
                .clicked()
            {
                requests.push(PanelRequest::Reconfirm(target()));
            }
        }
        if ui.button("Needs fix").clicked() {
            requests.push(PanelRequest::Dialog(HostRequest::NeedsFix(target()), String::new()));
        }
        if walking && ui.button("Skip").clicked() {
            *skip = true;
        }
        if ui
            .button("Save, back to map")
            .on_hover_text("Keep what was written to review.md (never committed: commit it with git or lazygit)")
            .clicked()
        {
            *leave = Some(false);
        }
        if ui.button("Cancel").on_hover_text("Discard every change to review.md since entering review mode").clicked() {
            let changed = matches!((&a.session, root), (Some(s), Some(r)) if s.changed(r));
            if changed {
                a.confirm_cancel = true;
            } else {
                *leave = Some(true);
            }
        }
    });
    if a.confirm_cancel {
        ui.horizontal_wrapped(|ui| {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                format!(
                    "Discard every change to {} since entering review mode?",
                    v.review_md
                ),
            );
            if ui.button("Discard").clicked() {
                *leave = Some(true);
            }
            if ui.button("Keep").clicked() {
                a.confirm_cancel = false;
            }
        });
    }
    ui.separator();
    concepts(ui, &v);
    ui.separator();
    ui.label(RichText::new(format!("review.md: {}", v.review_md)).strong());
    if let Some((line, ed)) = a.editing.as_mut() {
        let line = *line;
        let mut save = false;
        let mut cancel = false;
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong(format!("editing the comments of the entry on line {line}"));
                if ui.button("Save").clicked() {
                    save = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
            ui.weak("Only the comments: the [kovan] block and the sign-off stay as signed.");
            ui.push_id(("review-mode-section", line), |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("review_mode_section_editor")
                    .max_height(200.0)
                    .show(ui, |ui| {
                        let _ = ed.ui(ui, None);
                    });
            });
        });
        if save {
            if let Some(r) = root {
                let (r, md, expected, text) = (
                    r.clone(),
                    v.review_md.clone(),
                    v.review_md_text.clone(),
                    ed.text(),
                );
                a.job = Some(Worker::spawn(move || {
                    save_section(&r, &md, &expected, line, &text)
                        .map(|_| format!("Saved {md} (not committed)."))
                }));
            }
            a.editing = None;
        } else if cancel {
            a.editing = None;
        }
    }
    if v.sections.is_empty() {
        ui.weak("No entry for this function yet: a stamp, needs fix or highlight adds one.");
    }
    for s in &v.sections {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("{} by {} (line {})", s.kind.label(), s.by, s.line));
            if a.editing.is_none() && ui.small_button("Edit comments").clicked() {
                let mut ed = KvimEditorState::default();
                ed.load_text(&s.comments);
                ed.begin_insert();
                a.editing = Some((s.line, ed));
                a.md.jump_to_line(s.line);
            }
        });
    }
    a.md.set_anchor_bands(crate::stamping::review_mode::section::bands(&v.sections));
    egui::CollapsingHeader::new("The whole review.md (read-only)")
        .id_salt("review_mode_md")
        .default_open(true)
        .show(ui, |ui| {
            egui::Frame::NONE.show(ui, |ui| {
                ui.set_max_height(220.0);
                if let Some(clicked) = a.md.ui_readonly(ui) {
                    let line = clicked + 1;
                    if let Some(s) = v.sections.iter().find(|s| s.span.contains(&clicked)) {
                        if a.editing.is_none() {
                            let mut ed = KvimEditorState::default();
                            ed.load_text(&s.comments);
                            ed.begin_insert();
                            a.editing = Some((s.line, ed));
                        }
                    } else {
                        let _ = line; // another function's entry: read-only
                    }
                }
            });
        });
    egui::CollapsingHeader::new("Comments for your stamp")
        .id_salt("review_mode_draft")
        .default_open(true)
        .show(ui, |ui| {
            ui.weak("Written under your review entry when you stamp (optional).");
            egui::Frame::NONE.show(ui, |ui| {
                ui.set_max_height(140.0);
                let _ = a.draft.ui(ui, None);
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use kovan_common::review::state::StateKind;

    use crate::stamping::review_mode::diff::line_diff;
    use crate::stamping::review_mode::highlight::Highlight;
    use crate::stamping::review_mode::section::{FunctionSection, SectionKind};
    use crate::stamping::review_mode::walk::WalkItem;
    use crate::stamping::review_mode::SinceReview;

    fn fref(name: &str) -> FunctionRef {
        FunctionRef {
            id: format!("crates/a/src/lib.rs::{name}"),
            krate: "a".into(),
            file: "crates/a/src/lib.rs".into(),
            name: name.into(),
            callees: None,
            lines: None,
        }
    }

    fn diff(old: &str, new: &str) -> FnDiff {
        FnDiff {
            path: "crates/a/src/lib.rs::twice".into(),
            since: "0123456789abcdef0123456789abcdef01234567".into(),
            lines_now: [6, 9],
            diff: line_diff(old, new),
        }
    }

    /// A view with every part filled: a diff, a changed callee, a section,
    /// an anchored and an orphaned highlight, a flag, a blocker, upstream.
    fn view() -> FunctionView {
        let src = "/// Twice.\nfn twice(x: f64) -> f64 {\n    leaf(x) + 2.0\n}";
        FunctionView {
            fn_id: "fn:0123456789abcdef".into(),
            call_graph_id: "crates/a/src/lib.rs::twice".into(),
            file: "crates/a/src/lib.rs".into(),
            qual: "twice".into(),
            source: src.into(),
            lines: [6, 9],
            review_md: "crates/a/src/review.md".into(),
            review_md_text: "# Review: twice (github:t)\n\nbody\n".into(),
            sections: vec![FunctionSection {
                kind: SectionKind::Review,
                heading: "Review: twice (github:t)".into(),
                line: 1,
                span: 0..3,
                by: "github:t".into(),
                comments: "body".into(),
            }],
            highlights: vec![
                Highlight {
                    id: "h1".into(),
                    line: 5,
                    by: "github:t".into(),
                    note: "why 2?".into(),
                    lines: Some((3, 3)),
                    fuzzy: true,
                },
                Highlight {
                    id: "h2".into(),
                    line: 9,
                    by: "github:t".into(),
                    note: "gone".into(),
                    lines: None,
                    fuzzy: false,
                },
            ],
            state: StateKind::DirectlyStale,
            reason: "code changed".into(),
            blocked_by: vec![("fn:1111111111111111".into(), "leaf".into())],
            flags: vec!["(!) implausible signing time (r): signed before the commit".into()],
            since: Some(SinceReview {
                by: "github:t".into(),
                date: "2026-10-10".into(),
                commit: "0123456789abcdef0123456789abcdef01234567".into(),
                path: "crates/a/src/lib.rs::twice".into(),
            }),
            own_diff: Some(Ok(diff("    leaf(x) + 1.0", "    leaf(x) + 2.0"))),
            callee_diffs: vec![
                ("leaf".into(), Ok(diff("x", "y"))),
                ("gone".into(), Err("not found".into())),
            ],
            upstream_url: Some("https://github.com/o/r/blob/0123456/src/a.f90#L3".into()),
            is_port: true,
            reconfirm: Some(false),
            concepts: vec![
                crate::stamping::review_mode::LinkedConcept {
                    id: "concept:a/b".into(),
                    title: "B".into(),
                    formulas: vec![crate::stamping::concepts::FormulaHit {
                        file: "p/one.md".into(),
                        id: "eq-1".into(),
                        heading: "Energy balance".into(),
                        body: "$$ q = m c_p \\Delta T $$".into(),
                    }],
                },
                crate::stamping::review_mode::LinkedConcept {
                    id: "concept:c".into(),
                    title: "C".into(),
                    formulas: vec![],
                },
            ],
        }
    }

    fn plan() -> WalkPlan {
        let items: BTreeMap<String, WalkItem> =
            [("leaf", vec![]), ("twice", vec!["leaf".to_string()])]
                .into_iter()
                .map(|(id, callees)| {
                    (
                        id.to_string(),
                        WalkItem {
                            id: id.into(),
                            call_graph_id: format!("crates/a/src/lib.rs::{id}"),
                            name: id.into(),
                            lines: 4,
                            state: StateKind::New,
                            callees,
                        },
                    )
                })
                .collect();
        WalkPlan::new("twice", &items).unwrap()
    }

    fn frames(p: &mut ReviewModePanel) {
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run_ui(Default::default(), |ui| {
                p.walk_strip(ui);
                p.ui(ui);
            });
        }
    }

    /// Methodology (#770; #740: "a saved fix re-hashes the function and
    /// turns ⛔ into ✏"): on the stamping fixture (temporary git repository,
    /// test passphrase), an open needs-fix on `other`, committed. Review
    /// mode is entered on `other` with a 20 ms watch interval; once its view
    /// shows "needs fix" and the watch has its baseline, `other`'s body is
    /// edited on disk (saved, not committed) and only `poll` is called, as
    /// a frame does. Pass: the view's state becomes "fixed, awaiting
    /// re-review" without re-entering, and the message says the file
    /// changed.
    ///
    /// Result (2026-10-10): passes.
    #[test]
    fn a_saved_fix_turns_needs_fix_into_fixed_without_reopening() {
        use crate::stamping::tests::{founder_key, Repo, BY, LIB};
        use crate::stamping::{draft_needs_fix_for, write_needs_fix};
        use std::time::{Duration, Instant};
        let r = Repo::new();
        let store = tempfile::tempdir().unwrap();
        founder_key(&r, store.path());
        let n = draft_needs_fix_for(r.path(), &format!("{LIB}::other"), BY, "returns a magic number")
            .unwrap();
        write_needs_fix(r.path(), &n, "").unwrap();
        r.commit("needs fix on other");
        let mut p = ReviewModePanel::default();
        p.set_root(Some(r.path().to_path_buf()), None);
        p.watch_interval = Some(Duration::from_millis(20));
        let other = FunctionRef {
            id: format!("{LIB}::other"),
            krate: "demo".into(),
            file: LIB.into(),
            name: "other".into(),
            callees: None,
            lines: None,
        };
        p.enter(other);
        let state = |p: &ReviewModePanel| {
            p.active
                .as_ref()
                .and_then(|a| a.view.as_ref())
                .map(|v| v.state)
        };
        fn wait(p: &mut ReviewModePanel, start: Instant, until: impl Fn(&ReviewModePanel) -> bool) {
            while !until(p) {
                p.poll();
                assert!(start.elapsed() < Duration::from_secs(180), "timed out");
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        let start = Instant::now();
        wait(&mut p, start, |p| state(p) == Some(StateKind::NeedsFixOpen));
        wait(&mut p, start, |p| {
            p.active
                .as_ref()
                .and_then(|a| a.watch.as_ref())
                .is_some_and(FnWatch::started)
        });
        let fixed = r.read(LIB).replace("    1\n}", "    2\n}");
        r.write(LIB, &fixed);
        wait(&mut p, start, |p| state(p) == Some(StateKind::Fixed));
        assert!(
            matches!(&p.message, Some(Ok(m)) if m.contains("changed on disk")),
            "{:?}",
            p.message
        );
    }

    /// Headless: review mode draws every tab (source with highlights and a
    /// selection, the unified diff, the changed callees), the section
    /// editor, the quick-fix editor and the Cancel confirmation, and the
    /// walk card before and during a walk, without a display or a panic.
    /// Then the dialog's outcome and Skip record the trail, and Skip moves
    /// to the next function with a Goto request for the map. Temporary
    /// folders only; no keystore.
    #[test]
    fn review_mode_draws_every_part_headless() {
        let d = tempfile::tempdir().unwrap();
        let mut p = ReviewModePanel::default();
        p.set_root(Some(d.path().to_path_buf()), None);
        let mut a = Active::new(fref("twice"));
        a.view = Some(view());
        a.sel = Some((2, 3));
        a.note = "a note".into();
        p.active = Some(a);
        p.walk = Some(WalkState {
            target: "twice".into(),
            plan: Some(plan()),
            trail: Trail::default(),
            started: false,
            job: None,
        });
        p.message = Some(Err("an error shown".into()));
        for tab in [CodeTab::Source, CodeTab::Diff, CodeTab::Callees] {
            p.active.as_mut().unwrap().tab = tab;
            frames(&mut p);
            assert_eq!(p.active.as_ref().unwrap().tab, tab, "drawn, not moved");
        }
        {
            let a = p.active.as_mut().unwrap();
            let mut ed = KvimEditorState::default();
            ed.load_text("body");
            a.editing = Some((1, ed));
            a.confirm_cancel = true;
        }
        frames(&mut p);
        {
            let a = p.active.as_mut().unwrap();
            let mut ed = KvimEditorState::default();
            ed.load_text("fn twice() {}");
            a.quick = Some((
                QuickFix {
                    file: "crates/a/src/lib.rs".into(),
                    lines: [6, 9],
                    original: "fn twice() {}".into(),
                },
                ed,
            ));
        }
        frames(&mut p);
        assert!(p.active.is_some());

        // The walk: started, the outcome of the dialog recorded, then Skip.
        p.walk.as_mut().unwrap().started = true;
        p.active.as_mut().unwrap().quick = None;
        frames(&mut p);
        p.outcome(Purpose::NeedsFix);
        let w = p.walk.as_ref().unwrap();
        assert_eq!(
            w.trail.outcome("fn:0123456789abcdef"),
            Some(StepOutcome::NeedsFix)
        );
        // The reload went to a worker over an empty folder: a refusal, not
        // a hang.
        p.walk.as_mut().unwrap().job = None;
        p.walk.as_mut().unwrap().plan = Some(plan());
        p.skip();
        let reqs = p.take_requests();
        assert!(
            reqs.iter()
                .any(|r| matches!(r, PanelRequest::Goto(id) if id.ends_with("::leaf"))),
            "{reqs:?}"
        );
        assert_eq!(p.active.as_ref().unwrap().target.name, "leaf");
        p.leave(false);
        assert!(p.active.is_none());
        frames(&mut p);
        // A new workspace resets everything.
        p.set_root(None, None);
        assert!(p.walk.is_none() && p.active.is_none());
    }
}
