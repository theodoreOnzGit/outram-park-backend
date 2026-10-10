//! The **Code Review** view (GitHub #820): `kovan-web`'s Code Review UI
//! embedded in desktop kovan, over the workspace the Code Map view shows.
//!
//! ```text
//!  code map ─► crate (module tree) ─► module (function ring) ─► function:
//!                                     source panel and review bar
//! ```
//!
//! It is the same egui UI the site publishes
//! ([`kovan_web::ui::CodeReview`]), in [`kovan_web::Mode::Desktop`], reading
//! a data folder [`crate::code_review_data`] builds from the SCIP index
//! "Index fresh" leaves in the workspace, and source files from the
//! checkout. **Nothing is picked by hand**: opening the view, or finishing
//! an "Index fresh", checks the data and builds or rebuilds it when the
//! index is newer.
//!
//! ~~**Stamping: the hand-off is wired, the dialog is not.**~~ **CORRECTED
//! 2026-10-10** (#770): **Stamping is wired end to end.** The review bar's
//! Stamp and Needs fix (enabled in `Mode::Desktop`) queue a
//! [`kovan_web::ui::HostRequest`] naming the function
//! ([`kovan_web::ui::FunctionRef`]); this view takes it after each frame and
//! opens the stamp dialog over it (`super::stamp_dialog`, whose logic is
//! [`crate::stamping::flow::StampFlow`]): key set-up, the review wizard,
//! sign and write `review.md`, or a needs-fix. After a write the flow
//! recomputes the states from `review.md`, and
//! [`CodeReviewView::set_stamps`] recolours the bar and the cards without
//! reloading the data.
//!
//! **Review mode (2026-10-10, #770, #740).** The strip's "Review >" and
//! "Review this walk >" act on the function selected in the embedded UI
//! ([`kovan_web::ui::CodeReview::selected_function`]); review mode
//! (`super::review_mode_panel`, logic in
//! [`crate::stamping::review_mode`]) is then drawn in place of the map,
//! and its Stamp / Needs fix open the same stamp dialog.
//!
//! **Stamp states come from `review.md`.** Once the data is loaded, the
//! view replaces the data folder's `stamps` (the legacy
//! `review/stamps.toml` path of `kovan-cli call-graph --split-dir`, left
//! as it is: changing the CLI output is a separate decision) with
//! [`crate::stamping::stamp_states`], computed on a worker.
//!
//! **No lag (root `CLAUDE.md`, HARD RULE).** Checking and building the data
//! (cargo metadata, reading the SCIP index, resolving every call, writing)
//! run on a worker thread; the UI thread takes the result with `try_write`
//! and reads progress with `RunControl::snapshot`, neither of which blocks.
//! `kovan-web` loads the folder on its own background threads.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use kovan_common::call_graph::split::StampState;
use kovan_common::review::signing::keystore::Keystore;
use kovan_web::ui::HostRequest;

use crate::code_review_data::{build, data_dir, data_state, BuildReport, DataState};
use crate::commands::index_control::{Progress, RunControl};
use super::review_mode_panel::PanelRequest;
use crate::stamping::flow::{Purpose, StampFlow, StatesResult, Target, Worker};

/// What the worker found or built.
type Prepared = Result<Option<BuildReport>, String>;

struct Job {
    ctl: RunControl,
    slot: Arc<RwLock<Option<Prepared>>>,
    last: Progress,
}

/// The view's state; owned by the app.
#[derive(Default)]
pub(crate) struct CodeReviewView {
    workspace: Option<PathBuf>,
    /// The embedded UI, once the data is ready.
    review: Option<kovan_web::ui::CodeReview>,
    job: Option<Job>,
    /// Whether the data of `workspace` has been checked since it was set.
    checked: bool,
    /// The last build's report, or why there is nothing to show.
    note: Option<Result<String, String>>,
    /// A Stamp / Needs-fix press taken from the embedded UI, until its
    /// dialog is closed.
    pending: Option<HostRequest>,
    /// The stamp dialog of `pending`.
    dialog: Option<StampFlow>,
    /// Stamp states from `review.md` being computed (after the data loads,
    /// or a refresh still running when the dialog closed).
    states_job: Option<Worker<StatesResult>>,
    /// Why the states could not be computed, or the dialog not opened.
    states_note: Option<String>,
    /// The keystore the dialog reads (tests inject a temporary one; `None`
    /// is kovan's default location, never read under `cargo test`).
    keystore: Option<Keystore>,
    /// Review mode and the review walk (#770, #740), drawn in place of the
    /// embedded map while a function is under review.
    review_mode: super::review_mode_panel::ReviewModePanel,
}

/// Check the data of `workspace` and build it when missing or stale (or
/// always, with `force`), on a worker thread. GUI glue: the work is
/// `code_review_data::{data_state, build}`, tested there.
fn start(workspace: PathBuf, force: bool) -> Job {
    let ctl = RunControl::for_background();
    let slot: Arc<RwLock<Option<Prepared>>> = Arc::new(RwLock::new(None));
    let (worker_ctl, worker_slot) = (ctl.clone(), slot.clone());
    std::thread::spawn(move || {
        let result = match data_state(&workspace) {
            DataState::Current if !force => Ok(None),
            _ => build(&workspace, &worker_ctl).map(Some),
        };
        if let Ok(mut s) = worker_slot.write() {
            *s = Some(result);
        }
    });
    Job {
        ctl,
        slot,
        last: Progress::default(),
    }
}

impl CodeReviewView {
    /// Show `workspace` (the Code Map view's). A change drops the UI of the
    /// previous one; the new one's data is checked on the next frame shown.
    pub(crate) fn set_workspace(&mut self, workspace: Option<&Path>) {
        if self.workspace.as_deref() != workspace {
            *self = Self {
                workspace: workspace.map(Path::to_path_buf),
                ..Self::default()
            };
        }
    }

    /// An "Index fresh" of `dir` finished: rebuild its data now, in the
    /// background, so the view is ready when it is opened.
    pub(crate) fn index_finished(&mut self, dir: &Path) {
        self.set_workspace(Some(dir));
        self.rebuild(true);
    }

    /// Push fresh stamp states into the embedded UI (after the stamp dialog
    /// wrote one), so it recolours without reloading. A no-op while there is
    /// no UI: the next data build carries the stamps.
    pub(crate) fn set_stamps(&mut self, stamps: Vec<StampState>) {
        if let Some(review) = &mut self.review {
            review.set_stamps(stamps);
        }
    }

    /// Take the embedded UI's Stamp / Needs-fix press, if any, and open its
    /// dialog. A press while a dialog is open is dropped (the modal blocks
    /// the bar anyway).
    fn take_request(&mut self) {
        if let Some(r) = self.review.as_mut().and_then(|r| r.take_host_request()) {
            if self.pending.is_none() {
                self.open(r);
            }
        }
    }

    /// The keystore the dialog uses: the injected one, else kovan's default
    /// location; under `cargo test` never the user's.
    fn keystore(&self) -> Result<Keystore, String> {
        match &self.keystore {
            Some(k) => Ok(k.clone()),
            None if cfg!(test) => Err("tests never read the real keystore".into()),
            None => Keystore::default_location().map_err(|e| e.to_string()),
        }
    }

    /// Open the stamp dialog for `req` (its first job starts on a worker).
    fn open(&mut self, req: HostRequest) {
        let (purpose, f) = match &req {
            HostRequest::Stamp(f) => (Purpose::Stamp, f),
            HostRequest::NeedsFix(f) => (Purpose::NeedsFix, f),
        };
        let target = Target {
            function: f.id.clone(),
            name: f.name.clone(),
        };
        let (Some(root), ks) = (self.workspace.clone(), self.keystore()) else {
            return;
        };
        match ks {
            Ok(ks) => {
                self.dialog = Some(StampFlow::new(root, ks, purpose, target));
                self.pending = Some(req);
            }
            Err(e) => self.states_note = Some(format!("Cannot open the stamp dialog: {e}")),
        }
    }

    /// Close the dialog, keeping a refresh it started.
    fn close_dialog(&mut self) {
        if let Some(mut d) = self.dialog.take() {
            if let crate::stamping::flow::Step::Done(o) = &d.step {
                self.review_mode.outcome(o.what);
            } else {
                // A refusal (the code changed while reviewed, #740
                // decision 7): show the new diff first.
                self.review_mode.reload();
            }
            if let Some(s) = d.take_states() {
                self.set_stamps(s);
            }
            if let Some(w) = d.take_refresh() {
                self.states_job = Some(w);
            }
        }
        self.pending = None;
    }

    /// Compute the stamp states from `review.md` on a worker.
    fn load_states(&mut self) {
        if let Some(dir) = self.workspace.clone() {
            self.states_job = Some(Worker::spawn(move || crate::stamping::stamp_states(&dir)));
        }
    }

    /// Take the states job's result, if any, without blocking.
    fn poll_states(&mut self) {
        let Some(r) = self.states_job.as_ref().and_then(Worker::try_take) else {
            return;
        };
        self.states_job = None;
        match r {
            Ok(s) => {
                self.states_note = None;
                self.set_stamps(s);
            }
            Err(e) => self.states_note = Some(format!("Review states from review.md: {e}")),
        }
    }

    fn rebuild(&mut self, force: bool) {
        if let (Some(dir), None) = (self.workspace.clone(), &self.job) {
            self.review = None;
            self.checked = true;
            self.job = Some(start(dir, force));
        }
    }

    /// Take the worker's result, if it has one, without blocking.
    fn poll(&mut self) {
        let Some(job) = &mut self.job else { return };
        if let Some(p) = job.ctl.snapshot() {
            job.last = p;
        }
        let Some(result) = job.slot.try_write().ok().and_then(|mut s| s.take()) else {
            return;
        };
        self.job = None;
        let Some(dir) = self.workspace.clone() else {
            return;
        };
        match result {
            Ok(report) => {
                self.note = report.map(|r| Ok(r.describe()));
                let source = kovan_web::data::DataSource::Dir {
                    data: data_dir(&dir),
                    workspace: dir,
                };
                self.review = Some(kovan_web::ui::CodeReview::new(
                    kovan_web::Mode::Desktop,
                    source,
                ));
                self.load_states();
            }
            Err(e) => self.note = Some(Err(e)),
        }
    }

    /// Draw the view. GUI drawing code (exempt from the test rule): its
    /// logic is `code_review_data`, and the UI drawn is `kovan-web`'s.
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui) {
        if !self.checked {
            self.rebuild(false);
        }
        self.poll();
        self.poll_states();
        if self.states_job.is_some() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(200));
        }
        let mut rebuild = false;
        // Review mode (#770): its workspace and keystore, and the function
        // selected on the embedded map, which [Review >] starts from.
        self.review_mode
            .set_root(self.workspace.clone(), self.keystore().ok());
        let selected = self.review.as_ref().and_then(|r| r.selected_function());
        let mut review_now = None;
        let mut walk_now = None;
        egui::Panel::top("code-review-strip").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                match &self.workspace {
                    Some(dir) => ui.weak(dir.display().to_string()),
                    None => ui.weak("No workspace"),
                };
                if let (Some(f), true) = (&selected, self.review_mode.active.is_none()) {
                    if ui
                        .button(format!("Review {} >", f.name))
                        .on_hover_text("Review mode: its source or diff since the last review, and this folder's review.md beside it")
                        .clicked()
                    {
                        review_now = Some(f.clone());
                    }
                    if ui
                        .button("Review this walk >")
                        .on_hover_text("Every function under this one, bottom-up, with the walk's size shown first")
                        .clicked()
                    {
                        walk_now = Some(f.clone());
                    }
                }
                if self.workspace.is_some()
                    && ui
                        .add_enabled(self.job.is_none(), egui::Button::new("Rebuild data"))
                        .on_hover_text(
                            "Build this view's data again from the workspace's saved SCIP index \
                             (target/kovan-scip/index.scip). rust-analyzer is not run.",
                        )
                        .clicked()
                {
                    rebuild = true;
                }
                match &self.note {
                    Some(Ok(text)) => {
                        ui.weak(text);
                    }
                    Some(Err(e)) => {
                        ui.colored_label(ui.visuals().error_fg_color, e);
                    }
                    None => {}
                }
                if self.states_job.is_some() {
                    ui.spinner();
                    ui.weak("review states\u{2026}");
                }
                if let Some(e) = &self.states_note {
                    ui.colored_label(ui.visuals().error_fg_color, e);
                }
            });
        });
        if rebuild {
            self.rebuild(true);
        }
        if let Some(f) = review_now {
            self.review_mode.enter(f);
        }
        if let Some(f) = walk_now {
            self.review_mode.plan(&f);
        }
        if self.review_mode.walk.is_some() {
            egui::Panel::top("code-review-walk").show(ui, |ui| self.review_mode.walk_strip(ui));
        }
        if let Some(job) = &self.job {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(200));
            egui::CentralPanel::default().show(ui, |ui| {
                ui.add_space(24.0);
                ui.vertical_centered(|ui| {
                    ui.spinner();
                    ui.label(if job.last.phase.is_empty() {
                        "Checking the code review data\u{2026}"
                    } else {
                        job.last.phase.as_str()
                    });
                    ui.weak("This runs in the background; the other tabs stay usable.");
                });
            });
            return;
        }
        match &mut self.review {
            Some(review) => {
                if self.review_mode.active.is_some() {
                    self.review_mode.ui(ui);
                } else {
                    review.show(ui);
                }
                let panel_requests = self.review_mode.take_requests();
                for r in &panel_requests {
                    if let PanelRequest::Goto(id) = r {
                        review.open_function_later(id);
                    }
                }
                self.take_request();
                for r in panel_requests {
                    if let PanelRequest::Dialog(req, seed) = r {
                        if self.pending.is_none() {
                            self.open(req);
                            if let Some(d) = self.dialog.as_mut() {
                                d.comments_seed = seed;
                            }
                        }
                    }
                }
                let ctx = ui.ctx().clone();
                let mut closed = false;
                let mut fresh = None;
                if let Some(d) = self.dialog.as_mut() {
                    closed = super::stamp_dialog::show(&ctx, d);
                    fresh = d.take_states();
                }
                if let Some(s) = fresh {
                    self.set_stamps(s);
                }
                if closed {
                    self.close_dialog();
                }
            }
            None => {
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.add_space(24.0);
                    ui.vertical_centered(|ui| {
                        if self.workspace.is_none() {
                            ui.label("No workspace is open.");
                            ui.weak("Choose a Rust workspace or crate in the Code Map tab.");
                        } else {
                            ui.label("There is no code review data for this workspace yet.");
                            ui.weak("Run \"Index fresh\" in the Code Map tab; this view then loads by itself.");
                        }
                    });
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kovan_web::ui::FunctionRef;

    /// Headless: the view draws the embedded UI with a pending request (the
    /// placeholder modal on top) without a display or a panic, keeps the
    /// request until Cancel, and passes stamps through to the embedded UI.
    #[test]
    fn a_pending_request_is_held_and_drawn_headless() {
        let d = tempfile::tempdir().unwrap();
        let mut v = CodeReviewView {
            review: Some(kovan_web::ui::CodeReview::new(
                kovan_web::Mode::Desktop,
                kovan_web::data::DataSource::Dir {
                    data: d.path().to_path_buf(),
                    workspace: d.path().to_path_buf(),
                },
            )),
            ..CodeReviewView::default()
        };
        let req = HostRequest::Stamp(FunctionRef {
            id: "crates/a/src/lib.rs::f".into(),
            krate: "a".into(),
            file: "crates/a/src/lib.rs".into(),
            name: "f".into(),
            callees: None,
            lines: None,
        });
        v.pending = Some(req.clone());
        let ctx = egui::Context::default();
        for _ in 0..3 {
            let _ = ctx.run_ui(Default::default(), |ui| v.ui(ui));
        }
        assert_eq!(v.pending, Some(req), "held until Cancel");
        // Nothing pressed in the embedded UI: nothing new taken.
        v.pending = None;
        v.take_request();
        assert_eq!(v.pending, None);
        v.set_stamps(Vec::new());
    }

    fn function() -> FunctionRef {
        FunctionRef {
            id: "crates/a/src/lib.rs::f".into(),
            krate: "a".into(),
            file: "crates/a/src/lib.rs".into(),
            name: "f".into(),
            callees: None,
            lines: None,
        }
    }

    /// Headless (#770): with a function under review and a walk planned,
    /// the view draws review mode in place of the embedded map, and the walk
    /// card, while their workers run (and are refused over an empty
    /// folder), without a display or a panic. Temporary folders only.
    #[test]
    fn review_mode_replaces_the_map_headless() {
        let d = tempfile::tempdir().unwrap();
        let mut v = CodeReviewView {
            workspace: Some(d.path().to_path_buf()),
            review: Some(kovan_web::ui::CodeReview::new(
                kovan_web::Mode::Desktop,
                kovan_web::data::DataSource::Dir {
                    data: d.path().to_path_buf(),
                    workspace: d.path().to_path_buf(),
                },
            )),
            checked: true,
            ..CodeReviewView::default()
        };
        v.review_mode.set_root(Some(d.path().to_path_buf()), None);
        v.review_mode.enter(function());
        v.review_mode.plan(&function());
        let ctx = egui::Context::default();
        for _ in 0..3 {
            let _ = ctx.run_ui(Default::default(), |ui| v.ui(ui));
        }
        assert!(v.review_mode.active.is_some(), "still in review mode");
        assert!(v.review_mode.walk.is_some());
    }

    /// Headless: a Stamp request opens the dialog only with a keystore
    /// (never the real one under `cargo test`); the dialog draws every
    /// step without a display or a panic; closing it clears the request.
    /// Temporary folders only.
    #[test]
    fn the_stamp_dialog_opens_and_draws_every_step_headless() {
        use crate::stamping::flow::{Outcome, Step, WizardForm};
        use crate::stamping::StampContext;

        let d = tempfile::tempdir().unwrap();
        let store = tempfile::tempdir().unwrap();
        let mut v = CodeReviewView {
            workspace: Some(d.path().to_path_buf()),
            review: Some(kovan_web::ui::CodeReview::new(
                kovan_web::Mode::Desktop,
                kovan_web::data::DataSource::Dir {
                    data: d.path().to_path_buf(),
                    workspace: d.path().to_path_buf(),
                },
            )),
            checked: true,
            ..CodeReviewView::default()
        };
        v.open(HostRequest::Stamp(function()));
        assert!(v.dialog.is_none() && v.pending.is_none());
        assert!(v.states_note.as_deref().unwrap().contains("real keystore"));

        v.keystore = Some(Keystore::at(store.path()));
        v.open(HostRequest::NeedsFix(function()));
        assert!(v.dialog.is_some() && v.pending.is_some());
        v.dialog.as_mut().unwrap().wait();
        assert_eq!(v.dialog.as_ref().unwrap().step, Step::NoRoot);

        let ctx = egui::Context::default();
        let wizard = StampContext {
            fn_id: "fn:00".into(),
            call_graph_id: "crates/a/src/lib.rs::f".into(),
            path: "crates/a/src/lib.rs::f".into(),
            review_md: "crates/a/src/review.md".into(),
            applicability: Default::default(),
            answers: [("doc_matches_behaviour".to_string(), "partly".to_string())].into(),
            restamp: true,
            hash: String::new(),
            no_concept: Some("other: a test".into()),
            suggested_architecture: None,
        };
        let steps = [
            Step::Loading,
            Step::NoRoot,
            Step::SetupKey,
            Step::Generating,
            Step::PickKey,
            Step::Register,
            Step::Registering,
            Step::Preparing,
            Step::Refused {
                message: "a.rs has changes not committed".into(),
                hint: "Commit or stash".into(),
            },
            Step::Wizard,
            Step::Signing,
            Step::NeedsFixForm,
            Step::WritingNeedsFix,
            Step::Done(Outcome {
                what: crate::stamping::flow::Purpose::Stamp,
                review_md: "crates/a/src/review.md".into(),
                replaced: true,
            }),
            Step::Failed("unreadable".into()),
        ];
        for step in steps {
            {
                let f = v.dialog.as_mut().unwrap();
                f.wizard = Some(WizardForm::new(wizard.clone()));
                f.step = step.clone();
            }
            for _ in 0..2 {
                let _ = ctx.run_ui(Default::default(), |ui| v.ui(ui));
            }
            assert_eq!(v.dialog.as_ref().unwrap().step, step, "drawn, not moved");
        }
        v.close_dialog();
        assert!(v.dialog.is_none() && v.pending.is_none());
    }
}
