//! **Run workspace test** and **Test changed crates** in the Code Review
//! tab (maintainer, 2026-10-29): `kovan-cli test` without a terminal.
//! After a `Cargo.lock` change the stamps of every crate whose dependency
//! tree moved read "pending workspace test" (#1048) until a full run at
//! the new lock is recorded; the first button runs the whole workspace,
//! the second only those crates, with `-p` (#1049).
//!
//! The run is [`crate::commands::test_evidence::run_with`], the code the CLI
//! runs, with the same refusals (uncommitted build inputs) and the same
//! records (`[test_run]` in the code-folder `kovan.toml`s of the crates
//! run when counted, `target/kovan/test_evidence_last.toml` always). Every
//! line the CLI would print goes to the run's [`RunControl`] log instead.
//!
//! **Progress (#1050).** [`WorkspaceTest::progress_line`] is a short,
//! fixed-format line (phase, test binaries started of the last run's
//! count, elapsed), so the strip does not re-wrap as cargo's lines change
//! length; the newest cargo line is the tooltip.
//!
//! **No lag (root `CLAUDE.md`, HARD RULE).** The run (a full release
//! `cargo test`, hours on this workspace) is on a thread; a frame only
//! reads the log with `RunControl::snapshot` and checks whether the thread
//! finished. Cancel kills cargo at its next output line and records
//! nothing.

use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use kovan_common::review::evidence::TestEvidence;

use crate::commands::index_control::{Progress, RunControl};
use crate::commands::test_evidence::run_with;

/// What a run covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scope {
    /// `cargo test --workspace …`.
    Workspace,
    /// `-p` for each crate [`crate::stamping::crates_needing_test`]
    /// finds (#1049).
    ChangedCrates,
}

/// The workspace test's state: at most one run.
#[derive(Default)]
pub(crate) struct WorkspaceTest {
    job: Option<(RunControl, JoinHandle<Result<(), String>>, Instant)>,
    /// The log as last read (`snapshot` may miss a frame).
    pub(crate) last: Progress,
    /// The last run's outcome, shown beside the buttons.
    pub(crate) message: Option<Result<String, String>>,
}

/// The test binary count of the last recorded run in `root`, for "n of
/// ~N"; `None` when there is none or it covered other crates.
fn last_binary_count(root: &Path, packages: &[String]) -> Option<usize> {
    let text = std::fs::read_to_string(root.join("target/kovan/test_evidence_last.toml")).ok()?;
    let ev = TestEvidence::parse(&text).ok()?;
    (ev.packages == packages && !ev.binaries.is_empty()).then_some(ev.binaries.len())
}

/// The body of a run, on its thread: pick the crates (for
/// [`Scope::ChangedCrates`]), then run cargo, counting test binaries as
/// they start into `ctl`'s progress.
fn work(root: &Path, scope: Scope, ctl: &RunControl) -> Result<(), String> {
    let packages = match scope {
        Scope::Workspace => Vec::new(),
        Scope::ChangedCrates => {
            ctl.phase("finding the crates whose dependencies changed", 0);
            let p = crate::stamping::crates_needing_test(root)?;
            if p.is_empty() {
                ctl.say("kovan-cli test: no crate is due a test run (every crate's evidence is at its current dependencies)");
                return Ok(());
            }
            ctl.say(format!("kovan-cli test: crates due a run: {}", p.join(", ")));
            p
        }
    };
    let extra: Vec<String> = packages.iter().flat_map(|p| ["-p".to_string(), p.clone()]).collect();
    let total = last_binary_count(root, &packages).unwrap_or(0);
    ctl.phase("compiling", total);
    let mut started = 0;
    run_with(root, &extra, false, None, ctl, |l| {
        if l.trim_start().starts_with("Running ") {
            if started == 0 {
                ctl.phase("testing", total);
            }
            started += 1;
            ctl.step(started);
        }
        ctl.say(l);
    })
}

/// `1h02m` / `4m05s` / `12s`.
fn clock(d: Duration) -> String {
    let s = d.as_secs();
    match (s / 3600, (s / 60) % 60, s % 60) {
        (0, 0, s) => format!("{s}s"),
        (0, m, s) => format!("{m}m{s:02}s"),
        (h, m, _) => format!("{h}h{m:02}m"),
    }
}

/// The strip's progress line (module doc).
pub(crate) fn progress_text(p: &Progress, elapsed: Duration) -> String {
    let phase = if p.phase.is_empty() { "starting" } else { p.phase.as_str() };
    let count = match (p.done, p.total) {
        (0, _) => String::new(),
        (d, 0) => format!(" \u{b7} {d} binaries"),
        (d, t) => format!(" \u{b7} binary {d} of ~{t}"),
    };
    format!("test: {phase}{count} \u{b7} {}", clock(elapsed))
}

impl WorkspaceTest {
    /// A run is going.
    pub(crate) fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// Start a run of `scope` in `root` on a thread (ignored while busy).
    pub(crate) fn start(&mut self, root: PathBuf, scope: Scope) {
        if self.busy() {
            return;
        }
        self.message = None;
        self.last = Progress::default();
        let ctl = RunControl::default();
        let worker = ctl.clone();
        let handle = std::thread::spawn(move || work(&root, scope, &worker));
        self.job = Some((ctl, handle, Instant::now()));
    }

    /// Ask the running test to stop.
    pub(crate) fn cancel(&self) {
        if let Some((ctl, ..)) = &self.job {
            ctl.cancel();
        }
    }

    /// The newest log line, for the tooltip.
    pub(crate) fn latest(&self) -> Option<&str> {
        self.last.log.last().map(String::as_str)
    }

    /// The short progress line while a run is going.
    pub(crate) fn progress_line(&self) -> Option<String> {
        self.job
            .as_ref()
            .map(|(.., t)| progress_text(&self.last, t.elapsed()))
    }

    /// Read the log and take a finished run. Returns whether a run just
    /// finished, so the host recomputes the review states.
    pub(crate) fn poll(&mut self) -> bool {
        let Some((ctl, handle, _)) = &self.job else { return false };
        if let Some(p) = ctl.snapshot() {
            self.last = p;
        }
        if !handle.is_finished() {
            return false;
        }
        let Some((ctl, handle, _)) = self.job.take() else { return false };
        if let Some(p) = ctl.snapshot() {
            self.last = p;
        }
        let result = handle
            .join()
            .unwrap_or_else(|_| Err("the workspace test failed unexpectedly".into()));
        // The verdict is the run's own "COUNTED" / "NOT counted" line, or
        // "no crate is due".
        let verdict = self
            .last
            .log
            .iter()
            .filter(|l| l.contains("COUNTED") || l.contains("NOT counted") || l.contains("no crate is due"))
            .cloned()
            .collect::<Vec<_>>()
            .join("; ");
        self.message = Some(match result {
            Ok(()) if verdict.is_empty() => Ok("Workspace test finished".into()),
            Ok(()) => Ok(verdict),
            Err(e) if verdict.is_empty() => Err(format!("Workspace test: {e}")),
            Err(e) => Err(format!("Workspace test: {e} ({verdict})")),
        });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finish(t: &mut WorkspaceTest) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !t.poll() {
            assert!(Instant::now() < deadline, "the run never finished");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// A folder that is not a git checkout fails before cargo starts, for
    /// both scopes; the failure reaches the strip's message, the run counts
    /// as finished, and the progress line exists only while it runs.
    #[test]
    fn a_run_outside_a_checkout_fails_and_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        for scope in [Scope::Workspace, Scope::ChangedCrates] {
            let mut t = WorkspaceTest::default();
            t.start(dir.path().to_path_buf(), scope);
            assert!(t.busy());
            assert!(t.progress_line().is_some_and(|l| l.starts_with("test: ")));
            t.start(dir.path().to_path_buf(), scope); // ignored while busy
            finish(&mut t);
            assert!(!t.busy() && t.progress_line().is_none());
            t.cancel(); // nothing running: a no-op
            // The whole workspace is refused at the dirty check; finding
            // changed crates in an empty folder may instead find none.
            let ok = match (&t.message, scope) {
                (Some(Err(e)), _) => e.starts_with("Workspace test: "),
                (Some(Ok(m)), Scope::ChangedCrates) => m.contains("no crate is due"),
                _ => false,
            };
            assert!(ok, "{scope:?}: {:?}", t.message);
        }
        assert_eq!(last_binary_count(dir.path(), &[]), None);
    }

    /// The progress line keeps one shape whatever cargo prints (#1050).
    #[test]
    fn the_progress_line_is_short_and_fixed_format() {
        let mut p = Progress::default();
        assert_eq!(progress_text(&p, Duration::from_secs(5)), "test: starting \u{b7} 5s");
        p.phase = "testing".into();
        p.done = 37;
        p.total = 210;
        assert_eq!(
            progress_text(&p, Duration::from_secs(3725)),
            "test: testing \u{b7} binary 37 of ~210 \u{b7} 1h02m"
        );
        p.total = 0;
        assert_eq!(progress_text(&p, Duration::from_secs(245)), "test: testing \u{b7} 37 binaries \u{b7} 4m05s");
    }
}
