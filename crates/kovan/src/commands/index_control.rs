//! Progress, cancellation and process priority for one `kovan-cli index`
//! run (GitHub #780), shared by the CLI and the desktop app's "Index fresh"
//! button so both drive the same indexer ([`super::index`]).
//!
//! The CLI uses [`RunControl::default`]: every message still goes to
//! stderr exactly as before, nothing is niced and nothing cancels. The app
//! builds one with [`RunControl::for_background`], runs the index on a
//! worker thread, and only *reads* [`RunControl::snapshot`] from the UI
//! thread (the no-lag rule, root `CLAUDE.md`). Cancel sets a flag; the
//! worker checks it between phases, and the `rust-analyzer scip` child,
//! which is the long part, is stopped by its own process id
//! ([`run_child`]). Nothing here ever looks a process up by name.

use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// How many log lines a [`Progress`] keeps (the oldest are dropped).
pub const LOG_CAP: usize = 400;

/// The `nice` increment the background run gives `rust-analyzer scip`, so
/// its many-core, ~15 GB load does not starve the GUI.
pub const NICE_LEVEL: i32 = 10;

/// What one run is doing now, for display.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Progress {
    /// The current phase, e.g. "rust-analyzer scip running (42 s)".
    pub phase: String,
    /// Items done in the current phase, when it counts (files written).
    pub done: usize,
    /// Items in the current phase, `0` when it does not count.
    pub total: usize,
    /// Every message of the run, oldest first, at most [`LOG_CAP`].
    pub log: Vec<String>,
}

/// Progress, cancellation and priority of one run. Cheap to clone: the
/// worker and the UI hold the same `Arc`s.
#[derive(Debug, Clone, Default)]
pub struct RunControl {
    progress: Arc<RwLock<Progress>>,
    cancel: Arc<AtomicBool>,
    /// Run `rust-analyzer scip` under `nice` (the app does; the CLI does
    /// not, so its behaviour is unchanged).
    pub nice: bool,
}

/// The error text of a cancelled run.
pub const CANCELLED: &str = "cancelled by the user; nothing more was written";

impl RunControl {
    /// The control the desktop app uses: `rust-analyzer` niced.
    pub fn for_background() -> RunControl {
        RunControl {
            nice: true,
            ..RunControl::default()
        }
    }

    /// Log one message: stderr (as the CLI always did) and the run's log.
    pub fn say(&self, msg: impl Into<String>) {
        let msg = msg.into();
        eprintln!("{msg}");
        if let Ok(mut p) = self.progress.write() {
            p.log.push(msg);
            let n = p.log.len();
            if n > LOG_CAP {
                p.log.drain(..n - LOG_CAP);
            }
        }
    }

    /// Start a phase (`total` 0 when it does not count items).
    pub fn phase(&self, phase: impl Into<String>, total: usize) {
        if let Ok(mut p) = self.progress.write() {
            p.phase = phase.into();
            p.done = 0;
            p.total = total;
        }
    }

    /// Items done so far in the current phase.
    pub fn step(&self, done: usize) {
        if let Ok(mut p) = self.progress.write() {
            p.done = done;
        }
    }

    /// A copy of the progress, for drawing. Never blocks: a write in
    /// flight yields `None` and the caller draws last frame's copy.
    pub fn snapshot(&self) -> Option<Progress> {
        self.progress.try_read().ok().map(|p| p.clone())
    }

    /// Ask the run to stop.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    /// Whether the run was asked to stop.
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    /// `Err(CANCELLED)` once cancel was pressed; called between phases.
    pub fn check(&self) -> Result<(), String> {
        if self.cancelled() {
            Err(CANCELLED.to_string())
        } else {
            Ok(())
        }
    }
}

/// The command that runs `program args`. With `nice` (the app's background
/// run) it runs under `nice -n NICE_LEVEL` when a `nice` program exists
/// (it `exec`s, so the child's process id is the program's own) and leads
/// its own process group.
pub fn command(program: &str, args: &[&std::ffi::OsStr], nice: bool) -> Command {
    let use_nice = nice && cfg!(unix) && which::which("nice").is_ok();
    let mut c = if use_nice {
        let mut c = Command::new("nice");
        c.arg("-n").arg(NICE_LEVEL.to_string()).arg(program);
        c
    } else {
        Command::new(program)
    };
    c.args(args);
    // In the background (the app) it leads its own process group, so a
    // cancel also reaches the children it spawns (cargo, build scripts) by
    // that group's id, which is the child's own process id. The CLI keeps
    // the terminal's group, so Ctrl-C still reaches rust-analyzer.
    #[cfg(unix)]
    if nice {
        use std::os::unix::process::CommandExt;
        c.process_group(0);
    }
    c
}

/// Run `cmd` to completion with stdout and stderr appended to `log`,
/// polling every 100 ms: the phase shows the elapsed time and the log's
/// last line, and a cancel stops the child **by its own process id** (and
/// its process group, which [`command`] made it lead).
pub fn run_child(
    mut cmd: Command,
    log: &Path,
    label: &str,
    ctl: &RunControl,
) -> Result<ExitStatus, String> {
    let file =
        std::fs::File::create(log).map_err(|e| format!("creating {}: {e}", log.display()))?;
    let file2 = file
        .try_clone()
        .map_err(|e| format!("{}: {e}", log.display()))?;
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(file)
        .stderr(file2)
        .spawn()
        .map_err(|e| format!("running {label}: {e}"))?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(e) => return Err(format!("{label}: {e}")),
        }
        if ctl.cancelled() {
            stop_child(&mut child, ctl.nice);
            return Err(CANCELLED.to_string());
        }
        let tail = last_line(log).map(|l| format!(": {l}")).unwrap_or_default();
        ctl.phase(
            format!("{label} running ({} s){tail}", started.elapsed().as_secs()),
            0,
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Stop a child this process started: its process group by the child's
/// own id when [`command`] made it a group leader (unix), then the child
/// itself, then reap it.
fn stop_child(child: &mut std::process::Child, own_group: bool) {
    #[cfg(unix)]
    if own_group {
        let _ = Command::new("kill")
            .args(["-TERM", "--", &format!("-{}", child.id())])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// The last non-empty line of a log file (its last 4 KiB only), trimmed to
/// 160 characters.
pub fn last_line(path: &Path) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    f.seek(SeekFrom::Start(len.saturating_sub(4096))).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf);
    let line = text.lines().rev().map(str::trim).find(|l| !l.is_empty())?;
    Some(line.chars().take(160).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn say_logs_and_caps_and_phase_counts() {
        let c = RunControl::default();
        for i in 0..(LOG_CAP + 5) {
            c.say(format!("m{i}"));
        }
        c.phase("writing", 3);
        c.step(2);
        let p = c.snapshot().unwrap();
        assert_eq!(p.log.len(), LOG_CAP);
        assert_eq!(p.log[0], "m5");
        assert_eq!((p.phase.as_str(), p.done, p.total), ("writing", 2, 3));
        assert!(c.check().is_ok());
        let ui = c.clone();
        ui.cancel();
        assert_eq!(c.check(), Err(CANCELLED.to_string()));
    }

    #[test]
    fn last_line_reads_the_tail() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("log");
        std::fs::write(&p, "a\nb\n\n  c  \n\n").unwrap();
        assert_eq!(last_line(&p).as_deref(), Some("c"));
        assert_eq!(last_line(&d.path().join("none")), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_child_finishes_or_is_cancelled_by_its_own_pid() {
        let d = tempfile::tempdir().unwrap();
        let log = d.path().join("log");
        let ctl = RunControl::for_background();
        let ok = run_child(
            command("sh", &["-c".as_ref(), "echo hi".as_ref()], ctl.nice),
            &log,
            "sh",
            &ctl,
        )
        .unwrap();
        assert!(ok.success());
        assert_eq!(last_line(&log).as_deref(), Some("hi"));
        let ctl = RunControl::default();
        let c2 = ctl.clone();
        let t = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            c2.cancel();
        });
        let started = Instant::now();
        let r = run_child(
            command("sleep", &["30".as_ref()], false),
            &log,
            "sleep",
            &ctl,
        );
        t.join().unwrap();
        assert_eq!(r, Err(CANCELLED.to_string()));
        assert!(started.elapsed() < Duration::from_secs(10));
    }
}
