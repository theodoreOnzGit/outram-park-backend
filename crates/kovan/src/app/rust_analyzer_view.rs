//! The Code Map view's **rust-analyzer check** (GitHub #820): when the view
//! is first shown, kovan checks that rust-analyzer is installed and, when
//! it is not, asks whether to install it.
//!
//! The map itself never needs rust-analyzer (it is drawn from
//! `cargo metadata`); "Index fresh" (#780) does, and before this check the
//! user learned that only from the confirm dialog of a run that could not
//! start. The logic is [`crate::rust_analyzer_setup`] (`probe`, `install`).
//!
//! **No lag (root `CLAUDE.md`, HARD RULE).** The probe and the install each
//! spawn a process, so both run on a worker thread; the UI thread only
//! takes a finished result with `try_write` and draws. The install uses the
//! network and starts only from the user's click, with the command shown.
//! "Not now" leaves a one-line reminder with an "Install…" button; the
//! check runs once per start of kovan, and again on "Check again".

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use crate::rust_analyzer_setup::{install, install_command_line, probe, rustup_on_path, Probe};

type Slot<T> = Arc<RwLock<Option<T>>>;

/// A missing rust-analyzer, and whether kovan can install it (rustup).
#[derive(Debug, Clone)]
struct Missing {
    probe: Probe,
    rustup: bool,
    /// The last install's error, shown above the buttons.
    error: Option<String>,
}

enum State {
    Unchecked,
    Checking(Slot<(Probe, bool)>),
    Installed,
    /// The prompt is open.
    Prompt(Missing),
    /// The user chose "Not now": a one-line reminder stays.
    Dismissed(Missing),
    Installing {
        missing: Missing,
        slot: Slot<Result<Probe, String>>,
    },
    /// Just installed: said once, until dismissed.
    JustInstalled(String),
}

/// The check's state; owned by the Code Map view.
pub(crate) struct RustAnalyzerPanel {
    state: State,
}

impl Default for RustAnalyzerPanel {
    fn default() -> Self {
        Self {
            state: State::Unchecked,
        }
    }
}

fn take<T>(slot: &Slot<T>) -> Option<T> {
    slot.try_write().ok().and_then(|mut s| s.take())
}

/// Probe on a worker thread. GUI glue: the probe is
/// `rust_analyzer_setup::probe`, tested there.
fn start_check(dir: Option<PathBuf>) -> State {
    let slot: Slot<(Probe, bool)> = Arc::new(RwLock::new(None));
    let worker = slot.clone();
    std::thread::spawn(move || {
        let found = (probe(dir.as_deref()), rustup_on_path());
        if let Ok(mut s) = worker.write() {
            *s = Some(found);
        }
    });
    State::Checking(slot)
}

/// Install on a worker thread. GUI glue: the install is
/// `rust_analyzer_setup::install`, tested there through `install_with`.
fn start_install(missing: Missing, dir: Option<PathBuf>) -> State {
    let slot: Slot<Result<Probe, String>> = Arc::new(RwLock::new(None));
    let worker = slot.clone();
    std::thread::spawn(move || {
        let r = install(dir.as_deref());
        if let Ok(mut s) = worker.write() {
            *s = Some(r);
        }
    });
    State::Installing { missing, slot }
}

/// What the prompt's buttons ask for.
enum Answer {
    Wait,
    Install,
    CheckAgain,
    NotNow,
}

impl RustAnalyzerPanel {
    /// Check on first show, then draw the prompt, the progress or the
    /// reminder. `dir` is the workspace the map shows: under rustup its
    /// `rust-toolchain.toml` chooses the toolchain. GUI drawing code
    /// (exempt from the test rule): its logic is `rust_analyzer_setup`.
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, dir: Option<&Path>) {
        let dir = dir.map(Path::to_path_buf);
        let state = std::mem::replace(&mut self.state, State::Unchecked);
        self.state = match state {
            State::Unchecked => start_check(dir),
            State::Checking(slot) => {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(150));
                match take(&slot) {
                    None => State::Checking(slot),
                    Some((p, _)) if p.is_installed() => State::Installed,
                    Some((probe, rustup)) => State::Prompt(Missing {
                        probe,
                        rustup,
                        error: None,
                    }),
                }
            }
            State::Installed => State::Installed,
            State::Prompt(missing) => match prompt(ui.ctx(), &missing) {
                Answer::Wait => State::Prompt(missing),
                Answer::Install => start_install(missing, dir),
                Answer::CheckAgain => start_check(dir),
                Answer::NotNow => State::Dismissed(missing),
            },
            State::Dismissed(missing) => {
                let mut reopen = false;
                ui.horizontal_wrapped(|ui| {
                    ui.colored_label(
                        ui.visuals().warn_fg_color,
                        "rust-analyzer is not installed: \"Index fresh\" cannot run.",
                    );
                    reopen = ui.button("Install\u{2026}").clicked();
                });
                if reopen {
                    State::Prompt(missing)
                } else {
                    State::Dismissed(missing)
                }
            }
            State::Installing { missing, slot } => {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(200));
                match take(&slot) {
                    None => {
                        ui.horizontal_wrapped(|ui| {
                            ui.spinner();
                            ui.label(format!(
                                "Installing rust-analyzer ({})\u{2026} kovan stays usable.",
                                install_command_line()
                            ));
                        });
                        State::Installing { missing, slot }
                    }
                    Some(Ok(p)) => State::JustInstalled(p.describe()),
                    Some(Err(e)) => State::Prompt(Missing {
                        error: Some(e),
                        ..missing
                    }),
                }
            }
            State::JustInstalled(text) => {
                let mut close = false;
                ui.horizontal_wrapped(|ui| {
                    ui.label(&text);
                    close = ui.button("Dismiss").clicked();
                });
                if close {
                    State::Installed
                } else {
                    State::JustInstalled(text)
                }
            }
        };
    }
}

/// The install prompt. GUI drawing code (exempt).
fn prompt(ctx: &egui::Context, m: &Missing) -> Answer {
    let mut answer = Answer::Wait;
    egui::Modal::new(egui::Id::new("rust-analyzer-install")).show(ctx, |ui| {
        ui.set_max_width(520.0_f32.min(ctx.content_rect().width() - 32.0));
        ui.heading("Install rust-analyzer?");
        ui.label(m.probe.describe());
        ui.label(
            "The code map does not need it. \"Index fresh\" does: it builds the index \
             (kovan.toml, the link index) with rust-analyzer.",
        );
        if let Some(e) = &m.error {
            ui.add_space(4.0);
            ui.colored_label(ui.visuals().error_fg_color, format!("The install did not work: {e}"));
        }
        ui.add_space(4.0);
        if m.rustup {
            ui.label("kovan can install it with rustup. This uses the network and runs:");
            ui.monospace(install_command_line());
        } else {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "rustup is not on PATH, so kovan cannot install it for you.",
            );
            ui.label("Install rustup from https://rustup.rs, then run:");
            ui.monospace(install_command_line());
            ui.label("or put a rust-analyzer binary on PATH, and choose \"Check again\".");
        }
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            if m.rustup && ui.button("Install").clicked() {
                answer = Answer::Install;
            }
            if ui.button("Check again").clicked() {
                answer = Answer::CheckAgain;
            }
            if ui.button("Not now").clicked() {
                answer = Answer::NotNow;
            }
        });
    });
    answer
}
