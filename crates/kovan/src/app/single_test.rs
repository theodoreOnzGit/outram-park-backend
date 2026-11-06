//! **Run this test** (GitHub #1165, maintainer 2026-11-03: "a button to
//! run this single test. and have it partially run. only if it is a test.
//! printed output should be available to see in a popup window which i
//! can then close").
//!
//! The review bar offers the button on a `#[test]` function only
//! (`kovan_web::model::libtest_name`) and sends
//! `HostRequest::RunTest { package, test }`. This runs
//!
//! ```text
//! kovan-cli test -p <package> -- --exact --show-output <libtest name>
//! ```
//!
//! through [`crate::commands::test_evidence::run_with`], the CLI's own
//! code, so it has the same refusal (uncommitted build inputs). The name
//! filter makes it a **partial** run: never counted as test evidence. It is
//! recorded in its own file, [`RECORD`], so the last workspace run's
//! `test_evidence_last.toml` is kept. `--show-output` prints a passing
//! test's output too (`--nocapture` is refused by `kovan-cli test`, whose
//! parser needs libtest's captured form).
//!
//! **Run again** (maintainer, same day: for a test that may be racy, "let
//! it fail ... and can try again"): once a run ends, the window offers it;
//! the earlier outcomes stay listed, so a test that fails and then passes
//! shows as flaky rather than as passing.
//!
//! **No lag (root `CLAUDE.md`, HARD RULE).** cargo runs on a thread; a
//! frame only copies the output gathered so far.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;

use crate::commands::index_control::RunControl;
use crate::commands::test_evidence::run_with;

/// Where a single-test run is recorded, under the workspace root.
pub(crate) const RECORD: &str = "target/kovan/single_test_last.toml";
/// Output lines kept (a test that prints more keeps its last ones).
const KEEP: usize = 20_000;

/// How a finished run went, from the run's own summary line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Outcome {
    Passed,
    Failed,
    /// Nothing ran: the name matched no test (or the build failed).
    NotRun,
    /// Refused or failed before a summary (e.g. uncommitted build inputs).
    Error(String),
    Cancelled,
}

impl Outcome {
    /// One word, for the list of earlier attempts.
    pub(crate) fn word(&self) -> &'static str {
        match self {
            Outcome::Passed => "passed",
            Outcome::Failed => "FAILED",
            Outcome::NotRun => "not run",
            Outcome::Error(_) => "error",
            Outcome::Cancelled => "cancelled",
        }
    }
}

/// One single-test run and its window.
pub(crate) struct SingleTest {
    pub(crate) test: String,
    pub(crate) package: String,
    root: PathBuf,
    /// The outcomes of the earlier attempts, oldest first (Run again).
    pub(crate) history: Vec<Outcome>,
    ctl: RunControl,
    lines: Arc<Mutex<Vec<String>>>,
    job: Option<JoinHandle<Result<(), String>>>,
    started: Instant,
    pub(crate) outcome: Option<Outcome>,
}

/// The cargo arguments after `kovan-cli test` (module doc).
pub(crate) fn args(package: &str, test: &str) -> Vec<String> {
    ["-p", package, "--", "--exact", "--show-output", test].iter().map(|s| s.to_string()).collect()
}

/// The outcome the run's log says, given how the thread ended.
pub(crate) fn outcome(log: &[String], result: &Result<(), String>) -> Outcome {
    let summary = log.iter().rev().find(|l| l.starts_with("kovan-cli test:") && l.contains(" binaries, "));
    let count = |what: &str| {
        summary.and_then(|l| {
            let (before, _) = l.split_once(&format!(" {what}"))?;
            before.rsplit(' ').next()?.parse::<usize>().ok()
        })
    };
    match (summary, count("passed"), count("failed")) {
        (Some(_), _, Some(f)) if f > 0 => Outcome::Failed,
        (Some(_), Some(p), _) if p > 0 => Outcome::Passed,
        (Some(_), ..) => Outcome::NotRun,
        _ => match result {
            Err(e) if e == crate::commands::index_control::CANCELLED => Outcome::Cancelled,
            Err(e) => Outcome::Error(e.clone()),
            Ok(()) => Outcome::NotRun,
        },
    }
}

impl SingleTest {
    /// Start running `test` of `package` in `root` on a thread.
    pub(crate) fn start(root: PathBuf, package: String, test: String) -> SingleTest {
        let ctl = RunControl::default();
        let lines: Arc<Mutex<Vec<String>>> = Arc::default();
        let (worker, sink, a, at) = (ctl.clone(), lines.clone(), args(&package, &test), root.clone());
        let job = std::thread::spawn(move || {
            let (root, out) = (at.clone(), Some(at.join(RECORD)));
            run_with(&root, &a, false, out, &worker, |l| {
                if let Ok(mut v) = sink.lock() {
                    v.push(l.to_string());
                    let n = v.len();
                    if n > KEEP {
                        v.drain(..n - KEEP);
                    }
                }
            })
        });
        SingleTest {
            test,
            package,
            root,
            history: Vec::new(),
            ctl,
            lines,
            job: Some(job),
            started: Instant::now(),
            outcome: None,
        }
    }

    pub(crate) fn running(&self) -> bool {
        self.job.is_some()
    }

    pub(crate) fn cancel(&self) {
        self.ctl.cancel();
    }

    /// The output so far.
    pub(crate) fn lines(&self) -> Vec<String> {
        self.lines.lock().map(|v| v.clone()).unwrap_or_default()
    }

    /// Run the same test again (ignored while one runs): fresh output, the
    /// finished attempt's outcome kept in `history`.
    pub(crate) fn again(&mut self) {
        if self.running() {
            return;
        }
        let mut next = SingleTest::start(self.root.clone(), self.package.clone(), self.test.clone());
        next.history = std::mem::take(&mut self.history);
        next.history.extend(self.outcome.take());
        *self = next;
    }

    /// Take the finished run; true when it just finished.
    pub(crate) fn poll(&mut self) -> bool {
        if !self.job.as_ref().is_some_and(|j| j.is_finished()) {
            return false;
        }
        let Some(job) = self.job.take() else { return false };
        let result = job.join().unwrap_or_else(|_| Err("the test run failed unexpectedly".into()));
        self.outcome = Some(outcome(&self.lines(), &result));
        true
    }
}

/// The window: status, Cancel while running, the output, Close. Returns
/// false once closed (a running test is cancelled first). GUI drawing code
/// (exempt from the reaching-test rule; the logic is `outcome`, `args` and
/// `SingleTest`, tested below).
pub(crate) fn show(ctx: &egui::Context, t: &mut SingleTest, root: &Path) -> bool {
    let mut open = true;
    let mut close = false;
    let mut again = false;
    egui::Window::new(format!("Test: {}", t.test))
        .id(egui::Id::new("kovan_single_test"))
        .open(&mut open)
        .default_size([760.0, 480.0])
        .resizable(true)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.monospace(format!("-p {}", t.package));
                match &t.outcome {
                    None => {
                        ui.spinner();
                        ui.label(format!("running \u{b7} {}s", t.started.elapsed().as_secs()));
                        if ui.button("Cancel").clicked() {
                            t.cancel();
                        }
                    }
                    Some(Outcome::Passed) => {
                        ui.colored_label(egui::Color32::from_rgb(90, 190, 110), "passed");
                    }
                    Some(Outcome::Failed) => {
                        ui.colored_label(ui.visuals().error_fg_color, "FAILED");
                    }
                    Some(Outcome::NotRun) => {
                        ui.colored_label(ui.visuals().warn_fg_color, "no test ran (name not found, or the build failed)");
                    }
                    Some(Outcome::Error(e)) => {
                        ui.colored_label(ui.visuals().error_fg_color, e);
                    }
                    Some(Outcome::Cancelled) => {
                        ui.weak("cancelled");
                    }
                }
                if t.outcome.is_some() && ui.button("Run again").on_hover_text("Run the same test again, for a result that may be a race").clicked() {
                    again = true;
                }
            });
            if !t.history.is_empty() {
                let earlier: Vec<&str> = t.history.iter().map(Outcome::word).collect();
                ui.weak(format!("attempt {} \u{b7} earlier: {}", t.history.len() + 1, earlier.join(", ")));
            }
            ui.weak(format!(
                "Partial run: never counted as test evidence. Recorded in {}.",
                root.join(RECORD).display()
            ));
            ui.separator();
            let lines = t.lines();
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .max_height(ui.available_height() - 36.0)
                .show(ui, |ui| {
                    let mut text = lines.join("\n");
                    ui.add(egui::TextEdit::multiline(&mut text).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY).interactive(true));
                });
            ui.separator();
            if ui.button("Close").clicked() {
                close = true;
            }
        });
    if again {
        t.again();
    }
    if !open || close {
        if t.running() {
            t.cancel();
        }
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: the run's own summary line decides the outcome (a
    /// failure wins over passes; no test passed or failed is "not run");
    /// without one, the thread's error is the outcome. The arguments are a
    /// package, an exact name filter and `--show-output`.
    #[test]
    fn the_summary_line_gives_the_outcome() {
        let line = |p: usize, f: usize| vec![format!("kovan-cli test: 7 binaries, {p} passed, {f} failed, 0 ignored at abc (Cargo.lock sha256:x)")];
        assert_eq!(outcome(&line(1, 0), &Ok(())), Outcome::Passed);
        assert_eq!(outcome(&line(1, 1), &Err("cargo test exited".into())), Outcome::Failed);
        assert_eq!(outcome(&line(0, 0), &Ok(())), Outcome::NotRun);
        assert_eq!(outcome(&[], &Err("refusing to run: uncommitted".into())), Outcome::Error("refusing to run: uncommitted".into()));
        let cancelled = Err(crate::commands::index_control::CANCELLED.to_string());
        assert_eq!(outcome(&[], &cancelled), Outcome::Cancelled);
        assert_eq!(args("boon-lay", "t"), ["-p", "boon-lay", "--", "--exact", "--show-output", "t"]);
        assert!(kovan_common::review::evidence::args::plan(&args("boon-lay", "t")).is_ok(), "kovan-cli test accepts them");
    }

    /// A folder that is not a git checkout: the run fails before cargo and
    /// the window's outcome is that error.
    #[test]
    fn a_run_outside_a_checkout_ends_in_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let mut t = SingleTest::start(dir.path().to_path_buf(), "x".into(), "t".into());
        let deadline = Instant::now() + std::time::Duration::from_secs(30);
        while !t.poll() {
            assert!(Instant::now() < deadline, "the run never finished");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(!t.running());
        assert!(matches!(t.outcome, Some(Outcome::Error(_))), "{:?}", t.outcome);
        // Run again: a fresh attempt, the first one's outcome kept.
        t.again();
        assert!(t.running() && t.outcome.is_none());
        t.again(); // ignored while running
        while !t.poll() {
            assert!(Instant::now() < deadline, "the second run never finished");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(t.history.iter().map(Outcome::word).collect::<Vec<_>>(), ["error"]);
        assert!(matches!(t.outcome, Some(Outcome::Error(_))));
    }
}
