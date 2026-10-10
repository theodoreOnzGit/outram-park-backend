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
//! **Stamping is not wired yet.** The review bar's Stamp action answers
//! "not implemented" (#740, #770); this view is the read-only half.
//!
//! **No lag (root `CLAUDE.md`, HARD RULE).** Checking and building the data
//! (cargo metadata, reading the SCIP index, resolving every call, writing)
//! run on a worker thread; the UI thread takes the result with `try_write`
//! and reads progress with `RunControl::snapshot`, neither of which blocks.
//! `kovan-web` loads the folder on its own background threads.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use crate::code_review_data::{build, data_dir, data_state, BuildReport, DataState};
use crate::commands::index_control::{Progress, RunControl};

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
        let Some(dir) = self.workspace.clone() else { return };
        match result {
            Ok(report) => {
                self.note = report.map(|r| Ok(r.describe()));
                let source = kovan_web::data::DataSource::Dir {
                    data: data_dir(&dir),
                    workspace: dir,
                };
                self.review = Some(kovan_web::ui::CodeReview::new(kovan_web::Mode::Desktop, source));
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
        let mut rebuild = false;
        egui::Panel::top("code-review-strip").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                match &self.workspace {
                    Some(dir) => ui.weak(dir.display().to_string()),
                    None => ui.weak("No workspace"),
                };
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
            });
        });
        if rebuild {
            self.rebuild(true);
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
            Some(review) => review.show(ui),
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
