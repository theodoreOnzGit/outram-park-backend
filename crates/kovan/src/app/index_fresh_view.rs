//! The **"Index fresh…"** flow of the Code Map view (GitHub #780): plan,
//! confirm, run, report, for the workspace or crate the map shows. The work
//! is [`crate::index_fresh`]'s (`plan_fresh`, `run_fresh`), the same
//! functions `kovan-cli index --fresh` calls.
//!
//! **No lag (root `CLAUDE.md`, HARD RULE).** Everything that touches the
//! disk, git, cargo or rust-analyzer runs on a worker thread: the plan
//! (cargo metadata, the file scan, the git walk for a restorable root, the
//! keystore), and the run (SCIP, parsing, the call graph, hashing, writing).
//! The UI thread only reads an `Arc<RwLock<…>>` with `try_read`/`try_write`
//! (never blocking) and draws. The run is not modal: the rest of kovan
//! stays usable, and the map shown is the last good one until the run has
//! finished and the reloaded map is ready. Cancel stops `rust-analyzer` by
//! its own process id ([`crate::commands::index_control`]); it runs under
//! `nice`.

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crate::commands::index_control::{Progress, RunControl};
use crate::index_fresh::{
    plan_fresh, run_fresh, CorruptAction, FileAction, FreshChoices, FreshPlan, FreshReport,
    RootState,
};

type Slot<T> = Arc<RwLock<Option<Result<T, String>>>>;

/// What the corrupt-root choice is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CorruptChoice {
    Undecided,
    Restore,
    StartFresh,
}

struct Confirm {
    plan: FreshPlan,
    founder: Option<String>,
    corrupt: CorruptChoice,
    fresh_confirmed: bool,
}

enum State {
    Idle,
    Planning {
        dir: PathBuf,
        slot: Slot<FreshPlan>,
    },
    Confirm(Confirm),
    Running {
        dir: PathBuf,
        ctl: RunControl,
        slot: Slot<FreshReport>,
        last: Progress,
    },
    Done {
        dir: PathBuf,
        result: Result<Vec<String>, String>,
    },
}

/// The flow's state; owned by the Code Map view.
pub(crate) struct FreshPanel {
    state: State,
}

impl Default for FreshPanel {
    fn default() -> Self {
        Self { state: State::Idle }
    }
}

/// What the view should do after this frame.
pub(crate) enum FreshEvent {
    /// The run finished writing into `dir`: reload its map.
    Finished(PathBuf),
}

impl FreshPanel {
    /// Whether a plan or a run is in flight (the map keeps its last state).
    pub(crate) fn busy(&self) -> bool {
        matches!(self.state, State::Planning { .. } | State::Running { .. })
    }

    /// Start the run straight away, as the confirm dialog's button does
    /// (for the headless no-lag check in `code_map_view`).
    #[cfg(test)]
    pub(crate) fn run_now(&mut self, dir: PathBuf, choices: FreshChoices) {
        self.state = start_run(dir, choices);
    }

    /// Start planning for `dir` on a worker thread. GUI glue: the planning
    /// itself is `plan_fresh`, tested in `index_fresh::tests`.
    pub(crate) fn start(&mut self, dir: PathBuf) {
        if self.busy() {
            return;
        }
        let slot: Slot<FreshPlan> = Arc::new(RwLock::new(None));
        let worker = slot.clone();
        let d = dir.clone();
        std::thread::spawn(move || {
            let r = plan_fresh(&d);
            if let Ok(mut s) = worker.write() {
                *s = Some(r);
            }
        });
        self.state = State::Planning { dir, slot };
    }

    /// Draw the inline status and the confirm dialog; poll the workers.
    /// GUI drawing code (exempt from the test rule): its logic is
    /// `plan_fresh` and `run_fresh`, tested in `index_fresh::tests` and
    /// `tests/index_fresh_cli.rs`.
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui) -> Option<FreshEvent> {
        let mut event = None;
        let state = std::mem::replace(&mut self.state, State::Idle);
        self.state = match state {
            State::Idle => State::Idle,
            State::Planning { dir, slot } => {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(150));
                match take(&slot) {
                    None => {
                        ui.horizontal_wrapped(|ui| {
                            ui.spinner();
                            ui.label(format!("Index fresh: checking {}\u{2026}", dir.display()));
                        });
                        State::Planning { dir, slot }
                    }
                    Some(Err(e)) => State::Done {
                        dir,
                        result: Err(e),
                    },
                    Some(Ok(plan)) => {
                        let founder = plan.founders.default_founder();
                        State::Confirm(Confirm {
                            plan,
                            founder,
                            corrupt: CorruptChoice::Undecided,
                            fresh_confirmed: false,
                        })
                    }
                }
            }
            State::Confirm(mut c) => match confirm_dialog(ui.ctx(), &mut c) {
                Answer::Wait => State::Confirm(c),
                Answer::Cancel => State::Idle,
                Answer::Go(choices) => start_run(c.plan.dir.clone(), choices),
            },
            State::Running {
                dir,
                ctl,
                slot,
                mut last,
            } => {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(200));
                if let Some(p) = ctl.snapshot() {
                    last = p;
                }
                match take(&slot) {
                    Some(result) => {
                        if result.is_ok() {
                            event = Some(FreshEvent::Finished(dir.clone()));
                        }
                        State::Done {
                            dir,
                            result: result.map(|r| r.lines()),
                        }
                    }
                    None => {
                        running_strip(ui, &dir, &ctl, &last);
                        State::Running {
                            dir,
                            ctl,
                            slot,
                            last,
                        }
                    }
                }
            }
            State::Done { dir, result } => {
                if done_strip(ui, &dir, &result) {
                    State::Idle
                } else {
                    State::Done { dir, result }
                }
            }
        };
        event
    }
}

/// Take a finished worker's result without blocking.
fn take<T>(slot: &Slot<T>) -> Option<Result<T, String>> {
    slot.try_write().ok().and_then(|mut s| s.take())
}

fn start_run(dir: PathBuf, choices: FreshChoices) -> State {
    let ctl = RunControl::for_background();
    let slot: Slot<FreshReport> = Arc::new(RwLock::new(None));
    let (worker_ctl, worker_slot, d) = (ctl.clone(), slot.clone(), dir.clone());
    std::thread::spawn(move || {
        let r = run_fresh(&d, &choices, &worker_ctl).map_err(|e| e.to_string());
        if let Ok(mut s) = worker_slot.write() {
            *s = Some(r);
        }
    });
    State::Running {
        dir,
        ctl,
        slot,
        last: Progress::default(),
    }
}

/// The progress strip while a run is going. GUI drawing code (exempt).
fn running_strip(ui: &mut egui::Ui, dir: &std::path::Path, ctl: &RunControl, p: &Progress) {
    ui.group(|ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spinner();
            ui.strong(format!("Indexing {}", dir.display()));
            if ctl.cancelled() {
                ui.weak("cancelling\u{2026}");
            } else if ui
                .button("Cancel")
                .on_hover_text(
                    "Stop rust-analyzer (by its own process id); nothing more is written",
                )
                .clicked()
            {
                ctl.cancel();
            }
        });
        let phase = if p.total > 0 {
            format!("{} ({} of {})", p.phase, p.done, p.total)
        } else {
            p.phase.clone()
        };
        ui.label(phase);
        if p.total > 0 {
            ui.add(egui::ProgressBar::new(p.done as f32 / p.total as f32));
        }
        egui::CollapsingHeader::new("Log")
            .id_salt("index-fresh-log")
            .show(ui, |ui| {
                for l in p.log.iter().rev().take(12).rev() {
                    ui.weak(l);
                }
            });
        ui.weak("The map below is the last good one; kovan stays usable while this runs.");
    });
}

/// The result strip; returns true when dismissed. GUI drawing code (exempt).
fn done_strip(
    ui: &mut egui::Ui,
    dir: &std::path::Path,
    result: &Result<Vec<String>, String>,
) -> bool {
    let mut close = false;
    ui.group(|ui| {
        match result {
            Ok(lines) => {
                ui.strong(format!("Index fresh finished: {}", dir.display()));
                for l in lines {
                    ui.label(l);
                }
            }
            Err(e) => {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    format!("Index fresh did not finish: {e}"),
                );
                ui.weak("The last good map is kept.");
            }
        }
        if ui.button("Dismiss").clicked() {
            close = true;
        }
    });
    close
}

enum Answer {
    Wait,
    Cancel,
    Go(FreshChoices),
}

/// The confirm dialog: what will be written, the cost, the founder, and
/// for a corrupt root the parse error and the user's choice. GUI drawing
/// code (exempt); the choices become [`FreshChoices`] for `run_fresh`.
fn confirm_dialog(ctx: &egui::Context, c: &mut Confirm) -> Answer {
    let mut answer = Answer::Wait;
    let plan = c.plan.clone();
    egui::Modal::new(egui::Id::new("index-fresh-confirm")).show(ctx, |ui| {
        ui.set_max_width(560.0_f32.min(ctx.content_rect().width() - 32.0));
        ui.heading("Index fresh?");
        ui.label(format!("{} \u{2014} {}", plan.dir.display(), plan.kind.label()));
        ui.add_space(4.0);
        egui::ScrollArea::vertical().max_height(ctx.content_rect().height() * 0.6).show(ui, |ui| {
            if plan.rust_analyzer.is_none() {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    "rust-analyzer is not installed (not on PATH), so the index cannot be built. Install it \
                     with `rustup component add rust-analyzer`. Nothing will be written.",
                );
            }
            ui.strong("Cost");
            ui.label(&plan.cost.message);
            ui.add_space(4.0);
            ui.strong("Files in this repository");
            let count = |a: FileAction| plan.files.iter().filter(|f| f.action == a).count();
            ui.label(format!(
                "{} to create, {} cache file(s) to overwrite if changed, {} kept as they are. kovan commits nothing.",
                count(FileAction::Create),
                count(FileAction::Regenerate),
                count(FileAction::Keep)
            ));
            egui::CollapsingHeader::new("Every file").id_salt("index-fresh-files").show(ui, |ui| {
                for f in &plan.files {
                    ui.horizontal_wrapped(|ui| {
                        ui.monospace(&f.path);
                        ui.weak(f.action.label());
                    });
                }
            });
            ui.add_space(4.0);
            ui.strong("kovan_root.toml");
            match &plan.root {
                RootState::Missing => {
                    ui.label("None yet: a new one is created (founder, an empty reviewer registry, an empty key history, the rust-analyzer version).");
                    founder_picker(ui, c);
                }
                RootState::Valid { .. } => {
                    ui.label("Present and readable: kept exactly as it is.");
                }
                RootState::Corrupt { error, .. } => corrupt_choice(ui, c, error),
            }
        });
        ui.separator();
        let blocked = c.plan.rust_analyzer.is_none()
            || matches!(c.plan.root, RootState::Corrupt { .. })
                && (c.corrupt == CorruptChoice::Undecided || c.corrupt == CorruptChoice::StartFresh && !c.fresh_confirmed);
        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(!blocked, egui::Button::new("Index fresh")).clicked() {
                let corrupt = match (c.corrupt, &c.plan.restore) {
                    (CorruptChoice::Restore, Ok(Some(r))) => Some(CorruptAction::Restore { commit: r.commit.clone() }),
                    (CorruptChoice::StartFresh, _) => Some(CorruptAction::StartFresh),
                    _ => None,
                };
                answer = Answer::Go(FreshChoices { founder: c.founder.clone(), corrupt, scip: None });
            }
            if ui.button("Cancel").clicked() {
                answer = Answer::Cancel;
            }
        });
    });
    answer
}

/// The founder for a new root: the keystore identities, or UNSET. GUI
/// drawing code (exempt).
fn founder_picker(ui: &mut egui::Ui, c: &mut Confirm) {
    let ids = c.plan.founders.ids();
    ui.horizontal_wrapped(|ui| {
        ui.label("Founder:");
        if ids.is_empty() {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "UNSET \u{2014} no sign-off identity in kovan's keystore; set [code_review] founder later",
            );
            return;
        }
        egui::ComboBox::from_id_salt("index-fresh-founder")
            .selected_text(c.founder.clone().unwrap_or_else(|| "UNSET".into()))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut c.founder, None, "UNSET");
                for id in &ids {
                    ui.selectable_value(&mut c.founder, Some(id.clone()), id);
                }
            });
    });
}

/// The corrupt-root choice (Leak Before Break). GUI drawing code (exempt).
fn corrupt_choice(ui: &mut egui::Ui, c: &mut Confirm, error: &str) {
    ui.colored_label(ui.visuals().error_fg_color, "kovan_root.toml is corrupt. It holds keys and an append-only history, so it is never silently overwritten.");
    ui.monospace(error);
    ui.label("Whatever you choose, the bad file is kept as kovan_root.toml.corrupt-<date>.");
    match &c.plan.restore {
        Ok(Some(r)) => {
            ui.radio_value(
                &mut c.corrupt,
                CorruptChoice::Restore,
                format!(
                    "Restore the last committed version that parses ({}, {})",
                    r.commit.get(..10).unwrap_or(&r.commit),
                    r.date
                ),
            );
        }
        Ok(None) => {
            ui.weak("No committed version parses, so there is nothing to restore.");
        }
        Err(e) => {
            ui.weak(format!("Nothing to restore: {e}"));
        }
    }
    ui.radio_value(
        &mut c.corrupt,
        CorruptChoice::StartFresh,
        "Start fresh (a new root; the old reviewers and keys are not carried over)",
    );
    if c.corrupt == CorruptChoice::StartFresh {
        ui.checkbox(
            &mut c.fresh_confirmed,
            "I understand: start a fresh kovan_root.toml",
        );
        founder_picker(ui, c);
    }
}
