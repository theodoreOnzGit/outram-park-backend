//! **Run workspace test** in the Code Review tab (maintainer, 2026-10-29):
//! `kovan-cli test` without a terminal. After a `Cargo.lock` change every
//! stamp reads "pending workspace test" until a full run at the new lock
//! is recorded; this button runs that full run.
//!
//! The run is [`crate::commands::test_evidence::run_with`], the code the CLI
//! runs, with the same refusals (uncommitted build inputs) and the same
//! records (`[test_run]` in every code-folder `kovan.toml` when counted,
//! `target/kovan/test_evidence_last.toml` always). Every line the CLI would
//! print goes to the run's [`RunControl`] log instead.
//!
//! **No lag (root `CLAUDE.md`, HARD RULE).** The run (a full release
//! `cargo test`, hours on this workspace) is on a thread; a frame only
//! reads the log with `RunControl::snapshot` and checks whether the thread
//! finished. Cancel kills cargo at its next output line and records
//! nothing.

use std::path::PathBuf;
use std::thread::JoinHandle;

use crate::commands::index_control::{Progress, RunControl};
use crate::commands::test_evidence::run_with;

/// The workspace test's state: at most one run.
#[derive(Default)]
pub(crate) struct WorkspaceTest {
    job: Option<(RunControl, JoinHandle<Result<(), String>>)>,
    /// The log as last read (`snapshot` may miss a frame).
    pub(crate) last: Progress,
    /// The last run's outcome, shown beside the button.
    pub(crate) message: Option<Result<String, String>>,
}

impl WorkspaceTest {
    /// A run is going.
    pub(crate) fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// Start the full suite in `root` on a thread (ignored while busy).
    pub(crate) fn start(&mut self, root: PathBuf) {
        if self.busy() {
            return;
        }
        self.message = None;
        self.last = Progress::default();
        let ctl = RunControl::default();
        let worker = ctl.clone();
        let handle = std::thread::spawn(move || {
            run_with(&root, &[], false, None, &worker, |l| worker.say(l))
        });
        self.job = Some((ctl, handle));
    }

    /// Ask the running test to stop.
    pub(crate) fn cancel(&self) {
        if let Some((ctl, _)) = &self.job {
            ctl.cancel();
        }
    }

    /// The newest log line, for the strip.
    pub(crate) fn latest(&self) -> Option<&str> {
        self.last.log.last().map(String::as_str)
    }

    /// Read the log and take a finished run. Returns whether a run just
    /// finished, so the host recomputes the review states.
    pub(crate) fn poll(&mut self) -> bool {
        let Some((ctl, handle)) = &self.job else { return false };
        if let Some(p) = ctl.snapshot() {
            self.last = p;
        }
        if !handle.is_finished() {
            return false;
        }
        let Some((ctl, handle)) = self.job.take() else { return false };
        if let Some(p) = ctl.snapshot() {
            self.last = p;
        }
        let result = handle
            .join()
            .unwrap_or_else(|_| Err("the workspace test failed unexpectedly".into()));
        // The verdict is the run's own "COUNTED" / "NOT counted" line.
        let verdict = self
            .last
            .log
            .iter()
            .filter(|l| l.contains("COUNTED") || l.contains("NOT counted"))
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

    /// A folder that is not a git checkout fails at the dirty check, before
    /// cargo starts; the failure reaches the strip's message and the run
    /// counts as finished.
    #[test]
    fn a_run_outside_a_checkout_fails_and_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let mut t = WorkspaceTest::default();
        t.start(dir.path().to_path_buf());
        assert!(t.busy());
        t.start(dir.path().to_path_buf()); // ignored while busy
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !t.poll() {
            assert!(std::time::Instant::now() < deadline, "the run never finished");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(!t.busy());
        t.cancel(); // nothing running: a no-op
        assert!(matches!(&t.message, Some(Err(e)) if e.starts_with("Workspace test: ")), "{:?}", t.message);
        assert_eq!(t.latest(), None, "nothing was said before the refusal");
    }
}
