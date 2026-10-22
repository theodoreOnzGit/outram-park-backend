//! **Review states refresh in the background** (GitHub #1018; maintainer,
//! 2026-10-22: "one shouldn't block making reviews as the refresh is
//! happening in general. Refreshes ought to happen in the background";
//! "starting a stamp should not wait on refresh").
//!
//! One [`Refresher`] per Code Review view owns every recomputation of the
//! stamp states ([`super::stamp_states_timed`], the whole workspace):
//!
//! ```text
//!  stamp written ──> stamped(): optimistic "stamped, updating…" row now
//!                    + request(): due in DEBOUNCE (pushed back by each stamp)
//!  Refresh button / data loaded ──> request_now()
//!  poll() each frame:
//!    running finished ── a newer request came meanwhile (stale)? drop it, run once more
//!                    └── else: these are the states; optimistic rows cleared
//!    nothing running and due ──> start ONE refresh
//! ```
//!
//! - **At most one refresh runs**; several quick stamps start one, after
//!   the last (debounced). A request while one runs marks it stale: its
//!   result is dropped (it predates the newer stamp) and one more runs
//!   after it. A thread cannot be cancelled, so "supersede" is this.
//! - **Nothing waits on it.** It shares no lock with a stamp dialog: the
//!   dialog's preparation loads the workspace itself, from a copy of the
//!   workspace cache ([`super::ws_cache::take`]), and the result here only
//!   replaces the states shown. It never touches an open dialog.
//! - **Every refresh is timed** ([`super::RefreshTimings`]) and the line is
//!   written to kovan's log (stderr) and kept for the strip's tooltip, so
//!   the cost can be measured on Windows.
//!
//! No egui. The clock ([`SessionClock`]) and the work ([`RefreshRunner`])
//! are injectable, so the debounce and supersede rules are unit-tested.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use kovan_common::call_graph::split::{StampState as WebStamp, StampVerdict};
use kovan_common::review::signing::session::SessionClock;
use kovan_common::review::state::StateKind;

use super::flow::Worker;
use super::{stamp_states_timed, RefreshTimings};

/// How long after the last stamp the refresh starts.
pub const DEBOUNCE: Duration = Duration::from_millis(1000);

/// What a refresh returns.
pub type Timed = Result<(Vec<WebStamp>, RefreshTimings), String>;

/// The work a refresh does. An enum, not a trait object (workspace rule).
#[derive(Debug, Clone, Default)]
pub enum RefreshRunner {
    /// [`stamp_states_timed`] over the workspace.
    #[default]
    StampStates,
    /// Tests: wait until the gate opens, then do the real work (a slow
    /// refresh the test controls).
    #[cfg(test)]
    Gated(std::sync::Arc<std::sync::atomic::AtomicBool>),
}

impl RefreshRunner {
    fn run(&self, root: &Path) -> Timed {
        match self {
            Self::StampStates => stamp_states_timed(root),
            #[cfg(test)]
            Self::Gated(open) => {
                while !open.load(std::sync::atomic::Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(5));
                }
                stamp_states_timed(root)
            }
        }
    }
}

/// The optimistic row for a function just stamped: "stamped, updating…"
/// until the engine's verdict arrives.
pub fn optimistic_row(function: &str, reviewer: &str, date: &str) -> WebStamp {
    WebStamp {
        function: function.to_string(),
        verdict: StampVerdict::Stale,
        reason: "written to review.md; the review states are being recomputed in the background".into(),
        rung: 0,
        reviewer: reviewer.to_string(),
        date: date.to_string(),
        note: String::new(),
        permalink: String::new(),
        state: Some(StateKind::Updating),
        ivv: None,
    }
}

/// The background refresh of the review states (module doc).
pub struct Refresher {
    root: Option<PathBuf>,
    clock: SessionClock,
    debounce: Duration,
    runner: RefreshRunner,
    /// When the next refresh is due (`None`: none asked for).
    due: Option<Instant>,
    running: Option<Worker<Timed>>,
    /// A request arrived while `running` ran: drop its result, run again.
    stale: bool,
    /// The last completed states.
    base: Option<Vec<WebStamp>>,
    /// Function -> its optimistic row, until the next completed refresh.
    optimistic: BTreeMap<String, WebStamp>,
    /// Refreshes started, and results dropped as stale (for the tests and
    /// the strip).
    pub started: usize,
    pub superseded: usize,
    /// The last refresh's phases.
    pub last_timings: Option<RefreshTimings>,
    /// The last refresh's error, until the host takes it.
    pub error: Option<String>,
}

impl Default for Refresher {
    fn default() -> Self {
        Refresher::new(SessionClock::System, DEBOUNCE, RefreshRunner::StampStates)
    }
}

impl Refresher {
    pub fn new(clock: SessionClock, debounce: Duration, runner: RefreshRunner) -> Refresher {
        Refresher {
            root: None,
            clock,
            debounce,
            runner,
            due: None,
            running: None,
            stale: false,
            base: None,
            optimistic: BTreeMap::new(),
            started: 0,
            superseded: 0,
            last_timings: None,
            error: None,
        }
    }

    /// The workspace refreshed. A change forgets everything of the old one
    /// (a refresh still running for it is dropped when it ends).
    pub fn set_root(&mut self, root: Option<PathBuf>) {
        if self.root != root {
            *self = Refresher { root, ..Refresher::new(self.clock.clone(), self.debounce, self.runner.clone()) };
        }
    }

    /// Ask for a refresh after the debounce (a stamp was written); each
    /// call pushes it back. A refresh running now is marked stale.
    pub fn request(&mut self) {
        self.due = Some(self.clock.now() + self.debounce);
        if self.running.is_some() {
            self.stale = true;
        }
    }

    /// Ask for a refresh now (the Refresh button, the data loaded). While
    /// one runs, it is marked stale and the new one runs right after.
    pub fn request_now(&mut self) {
        let now = self.clock.now();
        self.due = Some(self.due.map_or(now, |d| d.min(now)));
        if self.running.is_some() {
            self.stale = true;
        }
    }

    /// A stamp (or needs-fix) was just written for `function`: show it as
    /// "stamped, updating…" at once and ask for a refresh. Returns the
    /// states to show now, when there are any to merge into.
    pub fn stamped(&mut self, function: &str, reviewer: &str, date: &str) -> Option<Vec<WebStamp>> {
        self.optimistic
            .insert(function.to_string(), optimistic_row(function, reviewer, date));
        self.request();
        self.shown()
    }

    /// The last completed states with the optimistic rows on top (`None`
    /// before the first refresh completes).
    pub fn shown(&self) -> Option<Vec<WebStamp>> {
        let base = self.base.as_ref()?;
        let mut out: Vec<WebStamp> = base
            .iter()
            .filter(|s| !self.optimistic.contains_key(&s.function))
            .cloned()
            .collect();
        out.extend(self.optimistic.values().cloned());
        out.sort_by(|a, b| a.function.cmp(&b.function));
        Some(out)
    }

    /// A refresh is running.
    pub fn busy(&self) -> bool {
        self.running.is_some()
    }

    /// A refresh is running or due.
    pub fn outstanding(&self) -> bool {
        self.running.is_some() || self.due.is_some()
    }

    /// Take a finished refresh and start a due one; never blocks. Returns
    /// the states to show when they changed.
    pub fn poll(&mut self) -> Option<Vec<WebStamp>> {
        let mut changed = None;
        if let Some(w) = &self.running {
            if let Some(r) = w.try_take() {
                self.running = None;
                if self.stale {
                    // It predates a newer stamp: drop it, run once more.
                    self.stale = false;
                    self.superseded += 1;
                    let now = self.clock.now();
                    self.due = Some(self.due.unwrap_or(now));
                } else {
                    match r {
                        Ok((states, t)) => {
                            eprintln!("kovan: {}", t.line());
                            self.last_timings = Some(t);
                            self.base = Some(states);
                            self.optimistic.clear();
                            self.error = None;
                            changed = self.shown();
                        }
                        Err(e) => self.error = Some(e),
                    }
                }
            }
        }
        if self.running.is_none() && self.due.is_some_and(|d| d <= self.clock.now()) {
            if let Some(root) = self.root.clone() {
                self.due = None;
                self.started += 1;
                let runner = self.runner.clone();
                self.running = Some(Worker::spawn(move || runner.run(&root)));
            }
        }
        changed
    }

    /// The strip's words while a refresh is outstanding.
    pub fn status(&self) -> Option<&'static str> {
        match (self.running.is_some(), self.due.is_some()) {
            (true, _) => Some("review states\u{2026}"),
            (false, true) => Some("review states soon\u{2026}"),
            _ => None,
        }
    }

    /// Poll until nothing is outstanding, moving a manual clock past the
    /// debounce (tests only; the UI never waits).
    #[cfg(test)]
    pub fn settle(&mut self) -> Option<Vec<WebStamp>> {
        let start = Instant::now();
        let mut last = None;
        while self.outstanding() {
            assert!(start.elapsed() < Duration::from_secs(120), "refresh did not settle");
            self.clock.advance(self.debounce);
            if let Some(s) = self.poll() {
                last = Some(s);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        last
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use super::super::tests::{founder_key, Repo, LIB};
    use super::*;

    fn wait_running(r: &mut Refresher) {
        let start = Instant::now();
        while !r.busy() {
            assert!(start.elapsed() < Duration::from_secs(10));
            r.poll();
        }
    }

    /// Methodology (GitHub #1018): a manual clock and a gated runner over
    /// the stamp layer's git fixture. Pass, in order: a request waits for
    /// the debounce (nothing starts before it; each further request pushes
    /// it back); a request-now starts at once; requests while one runs mark
    /// it stale, its result is dropped and exactly one more runs after it;
    /// only the last result is shown, with the optimistic row cleared;
    /// before any completed refresh there is nothing to merge into; the
    /// timing line names every phase; a new root forgets the old one.
    ///
    /// Result (2026-10-22): passes.
    #[test]
    fn debounce_coalesce_and_supersede() {
        let repo = Repo::new();
        let store = tempfile::tempdir().unwrap();
        let key = founder_key(&repo, store.path());
        repo.stamp(&key, "twice", "");
        let clock = SessionClock::manual();
        let gate = Arc::new(AtomicBool::new(true));
        let mut r = Refresher::new(clock.clone(), Duration::from_secs(1), RefreshRunner::Gated(gate.clone()));
        r.set_root(Some(repo.path().to_path_buf()));
        assert_eq!(r.stamped(&format!("{LIB}::leaf"), "github:tester", "2026-10-22"), None, "nothing to merge into yet");
        r.poll();
        assert_eq!((r.started, r.status()), (0, Some("review states soon\u{2026}")), "debounced");
        clock.advance(Duration::from_millis(600));
        r.request();
        clock.advance(Duration::from_millis(600));
        r.poll();
        assert_eq!(r.started, 0, "pushed back by the second request");
        clock.advance(Duration::from_millis(500));
        r.poll();
        assert_eq!(r.started, 1);
        let first = r.settle().expect("states");
        assert!(first.iter().any(|s| s.function.ends_with("::twice") && s.state == Some(StateKind::Valid)));
        assert!(!first.iter().any(|s| s.state == Some(StateKind::Updating)), "optimistic row cleared");
        let line = r.last_timings.unwrap().line();
        for phase in ["cache check", "folder refresh", "git", "engine", "permalinks"] {
            assert!(line.contains(phase), "{line}");
        }

        // A slow refresh, then two stamps while it runs: one rerun.
        gate.store(false, Ordering::SeqCst);
        r.request_now();
        wait_running(&mut r);
        let shown = r.stamped(&format!("{LIB}::leaf"), "github:tester", "2026-10-22").unwrap();
        assert!(shown.iter().any(|s| s.function.ends_with("::leaf") && s.state == Some(StateKind::Updating)));
        r.stamped(&format!("{LIB}::other"), "github:tester", "2026-10-22");
        r.request_now();
        assert_eq!(r.started, 2);
        gate.store(true, Ordering::SeqCst);
        let last = r.settle().expect("states");
        assert_eq!((r.started, r.superseded), (3, 1), "the stale one dropped, one rerun");
        assert!(!r.outstanding());
        assert!(!last.iter().any(|s| s.state == Some(StateKind::Updating)));

        r.set_root(Some(store.path().to_path_buf()));
        assert_eq!((r.started, r.shown()), (0, None), "a new root starts afresh");
    }

    /// Methodology (GitHub #1018, "measure first"): time a cold refresh
    /// (the workspace cache forgotten) and a warm one (cache reused, as
    /// after a stamp) with [`stamp_states_timed`]. On the fixture: the
    /// phases add up to no more than the total, and the warm load refreshes
    /// no folder. With `KOVAN_REFRESH_TIMING_ROOT` set, the same on that
    /// workspace, the two log lines printed (the numbers on GitHub #1018).
    ///
    /// Result (2026-10-22): passes. On this workspace (Linux, 4 cores,
    /// release, 806 folders): cold 24.1 s (cache check 0.12, folder refresh
    /// 12.0, git 10.8, engine 0.61, permalinks 0.20); warm 6.1 s (folder
    /// refresh 0.001 s for 4 folders, git 4.5, engine 0.76, permalinks
    /// 0.19). The git facts dominate a refresh after a stamp.
    #[test]
    fn refresh_phases_are_timed_cold_and_warm() {
        let repo = Repo::new();
        let given = std::env::var_os("KOVAN_REFRESH_TIMING_ROOT").map(PathBuf::from);
        let root = given.clone().unwrap_or_else(|| repo.path().to_path_buf());
        super::super::ws_cache::forget(&root);
        let tally = super::super::git::spawn_tally;
        let calls = |before: &BTreeMap<String, (usize, Duration)>| -> Vec<String> {
            tally()
                .into_iter()
                .filter_map(|(k, (n, t))| {
                    let (n0, t0) = before.get(&k).copied().unwrap_or_default();
                    (n > n0).then(|| format!("{k}: {} calls, {:.3} s", n - n0, (t - t0).as_secs_f64()))
                })
                .collect()
        };
        let before = tally();
        let (_, cold) = stamp_states_timed(&root).unwrap();
        let cold_calls = calls(&before);
        let before = tally();
        let (_, warm) = stamp_states_timed(&root).unwrap();
        let warm_calls = calls(&before);
        println!("cold: {}\n  {}\nwarm: {}\n  {}", cold.line(), cold_calls.join("\n  "), warm.line(), warm_calls.join("\n  "));
        assert!(cold.line().contains("git processes"), "{}", cold.line());
        for t in [cold, warm] {
            let parts = t.load.cache_check + t.load.folder_refresh + t.concepts + t.git + t.engine + t.permalinks + t.states;
            assert!(parts <= t.total, "{}", t.line());
        }
        if given.is_none() {
            // (A real workspace may refresh a few folders warm: files
            // written in the last two seconds are re-read, ws_cache RACY.)
            assert_eq!(warm.load.folders_refreshed, 0, "{}", warm.line());
        }
    }
}
