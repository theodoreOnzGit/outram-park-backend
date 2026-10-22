//! Download progress for corpus clones (GitHub issue #1016).
//!
//! The standard corpus is a large Git repository (the workspace's
//! `reactor-literature` checkout is about 1.1 GB), and before #1016 every
//! clone ran through `Command::output()`, which waits and throws Git's
//! progress away: the user saw nothing for minutes.
//!
//! Now the clone runs with `--progress`, its stderr is read as it arrives
//! ([`run_git`]), and each `\r`- or `\n`-terminated line Git writes
//! (`Receiving objects:  42% (1234/2938), 210.00 MiB | 5.10 MiB/s`) is parsed
//! ([`parse_line`]) into a [`CloneProgress`] held in a shared
//! [`DownloadProgress`], an `Arc<RwLock<_>>` (the workspace rule for shared
//! mutable state; no channel). The background job writes it; the GUI reads a
//! [`DownloadProgress::snapshot`] each frame, and `kovan-cli` prints one
//! updating line from it ([`run_reporting`]).
//!
//! The module's safety rules still hold: failures are values, a poisoned lock
//! is read through rather than panicking, and the error text a failed clone
//! reports ([`super::CorpusRepoError::Clone`]) is Git's stderr **without** the
//! progress lines, as it was before `--progress` was passed.

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// One phase line of Git's progress output, such as
/// `Receiving objects:  42% (1234/2938), 210.00 MiB | 5.10 MiB/s`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PhaseUpdate {
    /// Sent by the remote (`remote: Counting objects: …`), not computed
    /// locally.
    pub remote: bool,
    /// The phase's name, e.g. `Receiving objects`.
    pub phase: String,
    /// The percentage, when Git knows the total.
    pub percent: Option<u8>,
    /// Items done so far (objects, deltas, files).
    pub count: u64,
    /// The total number of items, when Git knows it.
    pub total: Option<u64>,
    /// Bytes received so far (`Receiving objects` only).
    pub bytes: Option<u64>,
    /// Throughput, in bytes per second.
    pub rate: Option<u64>,
    /// The phase's last line (`…, done.`).
    pub done: bool,
}

/// One line of `git --progress` stderr, classified by [`parse_line`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgressLine {
    /// A progress phase; never part of the error text.
    Phase(PhaseUpdate),
    /// The remote's `Total 4 (delta 0), reused …` summary; never part of the
    /// error text either, since Git prints it only with `--progress`.
    Summary,
    /// `Cloning into '<path>'...`: Git has started on a repository, the
    /// superproject or a submodule.
    Repository(String),
    /// A `fatal:` or `error:` line.
    Error(String),
    /// Anything else, kept verbatim in the error text.
    Other(String),
}

/// Classify one line of Git's stderr (one `\r`- or `\n`-terminated segment).
pub fn parse_line(raw: &str) -> ProgressLine {
    let line = raw.trim_end();
    let (remote, body) = match line.strip_prefix("remote:") {
        Some(b) => (true, b.trim_start()),
        None => (false, line),
    };
    if let Some(rest) = body.strip_prefix("Cloning into ") {
        let path = rest.trim_end_matches('.').trim_matches(['\'', '"']);
        return ProgressLine::Repository(path.to_string());
    }
    if body.starts_with("fatal: ") || body.starts_with("error: ") {
        return ProgressLine::Error(body.to_string());
    }
    if remote && body.starts_with("Total ") {
        return ProgressLine::Summary;
    }
    match parse_phase(body) {
        Some(mut p) => {
            p.remote = remote;
            ProgressLine::Phase(p)
        }
        None => ProgressLine::Other(line.to_string()),
    }
}

/// `<Phase>: <pct>% (<n>/<total>)[, <size> | <rate>/s][, done.]` or
/// `<Phase>: <n>[, done.]`; `None` for anything else.
fn parse_phase(body: &str) -> Option<PhaseUpdate> {
    let (name, rest) = body.split_once(": ")?;
    if name.is_empty() || name.chars().any(|c| c.is_ascii_digit() || c == '\'') {
        return None;
    }
    let rest = rest.trim();
    let (rest, done) = match rest.strip_suffix(", done.") {
        Some(r) => (r, true),
        None => (rest, false),
    };
    let (head, tail) = rest.split_once(", ").unwrap_or((rest, ""));
    let mut p = PhaseUpdate {
        phase: name.to_string(),
        done,
        ..PhaseUpdate::default()
    };
    if let Some((pct, frac)) = head.split_once('%') {
        p.percent = Some(pct.trim().parse::<u8>().ok()?.min(100));
        let frac = frac.trim().strip_prefix('(')?.strip_suffix(')')?;
        let (n, total) = frac.split_once('/')?;
        p.count = n.trim().parse().ok()?;
        p.total = Some(total.trim().parse().ok()?);
    } else {
        p.count = head.trim().parse().ok()?;
    }
    if !tail.is_empty() {
        let (size, rate) = tail.split_once(" | ").unwrap_or((tail, ""));
        p.bytes = parse_size(size);
        p.rate = rate.trim().strip_suffix("/s").and_then(parse_size);
    }
    Some(p)
}

/// Git's human-readable sizes (`strbuf_humanise_bytes`): `512 bytes`,
/// `1 byte`, `3.50 KiB`, `210.00 MiB`, `1.07 GiB`, in bytes.
fn parse_size(s: &str) -> Option<u64> {
    let mut parts = s.split_whitespace();
    let number: f64 = parts.next()?.parse().ok()?;
    let scale: f64 = match parts.next()? {
        "byte" | "bytes" => 1.0,
        "KiB" => 1024.0,
        "MiB" => 1024.0 * 1024.0,
        "GiB" => 1024.0 * 1024.0 * 1024.0,
        "TiB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    Some((number * scale).round() as u64)
}

/// The state of a download, as the GUI and the CLI show it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CloneProgress {
    /// A Git download is running now.
    pub active: bool,
    /// What is being downloaded, in the user's words (`standard corpus`).
    pub subject: String,
    /// The repository Git is on (its `Cloning into` path): the superproject,
    /// then each submodule in turn.
    pub repository: String,
    /// The current phase (`Receiving objects`), or a step such as
    /// `checking for updates` while no Git progress is available.
    pub phase: String,
    /// The current phase's percentage, when Git reports one.
    pub percent: Option<u8>,
    /// Bytes received so far by the current repository.
    pub bytes: Option<u64>,
    /// Throughput, in bytes per second.
    pub rate: Option<u64>,
    /// Every phase that reached 100 % or `done`, in order, once each.
    pub completed_phases: Vec<String>,
    /// The download can be cancelled (a plain clone, whose folder is put
    /// back as it was; see [`super::clone`]).
    pub cancellable: bool,
    /// The user asked to cancel.
    pub cancel_requested: bool,
}

impl CloneProgress {
    /// Fold one parsed line into the state.
    pub fn apply(&mut self, line: &ProgressLine) {
        match line {
            ProgressLine::Phase(p) => {
                if self.phase != p.phase {
                    self.bytes = None;
                    self.rate = None;
                }
                self.phase = p.phase.clone();
                self.percent = p.percent;
                if p.bytes.is_some() {
                    self.bytes = p.bytes;
                }
                if p.rate.is_some() {
                    self.rate = p.rate;
                }
                if (p.done || p.percent == Some(100)) && !self.completed_phases.contains(&p.phase) {
                    self.completed_phases.push(p.phase.clone());
                }
            }
            ProgressLine::Repository(path) => {
                self.repository = path.clone();
                self.phase.clear();
                self.percent = None;
                self.bytes = None;
                self.rate = None;
            }
            ProgressLine::Summary | ProgressLine::Error(_) | ProgressLine::Other(_) => {}
        }
    }

    /// The bar's fill, `0.0..=1.0`; `None` when there is no percentage to
    /// show (the bar is then drawn as busy).
    pub fn fraction(&self) -> Option<f32> {
        self.percent.map(|p| f32::from(p) / 100.0)
    }

    /// One line of text describing the download, e.g.
    /// `standard corpus (reactor-literature): Receiving objects 42% · 210.0 MiB · 5.1 MiB/s`.
    pub fn label(&self) -> String {
        let mut out = if self.subject.is_empty() {
            "downloading".to_string()
        } else {
            self.subject.clone()
        };
        let repo = std::path::Path::new(&self.repository)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !repo.is_empty() {
            out.push_str(&format!(" ({repo})"));
        }
        if !self.phase.is_empty() {
            out.push_str(&format!(": {}", self.phase));
        }
        if let Some(p) = self.percent {
            out.push_str(&format!(" {p}%"));
        }
        if let Some(b) = self.bytes {
            out.push_str(&format!(" · {}", mib(b)));
        }
        if let Some(r) = self.rate {
            out.push_str(&format!(" · {}/s", mib(r)));
        }
        out
    }
}

/// Bytes as MiB with one decimal, the unit Git itself uses for a corpus.
fn mib(bytes: u64) -> String {
    format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
}

/// A download's progress, shared between the job that runs it and whoever
/// shows it: `Arc<RwLock<CloneProgress>>` (rust-design-rules: shared mutable
/// state, no channel). Cloning the handle shares the state.
///
/// A poisoned lock is read and written through, never unwrapped, so a panic
/// elsewhere cannot turn into a panic here.
#[derive(Debug, Clone, Default)]
pub struct DownloadProgress(Arc<RwLock<CloneProgress>>);

impl DownloadProgress {
    /// A fresh, idle progress.
    pub fn new() -> Self {
        Self::default()
    }

    /// A copy of the current state.
    pub fn snapshot(&self) -> CloneProgress {
        self.0.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn update(&self, f: impl FnOnce(&mut CloneProgress)) {
        f(&mut self.0.write().unwrap_or_else(|e| e.into_inner()));
    }

    /// Name what is being downloaded next (`standard corpus`).
    pub fn set_subject(&self, subject: impl Into<String>) {
        let subject = subject.into();
        self.update(|p| p.subject = subject);
    }

    /// Mark a step that has no Git progress of its own (such as the
    /// fast-forward's fetch) as running, with `phase` as its label; the bar
    /// is drawn as busy.
    pub fn busy(&self, phase: impl Into<String>) {
        let phase = phase.into();
        self.update(|p| {
            p.active = true;
            p.repository.clear();
            p.phase = phase;
            p.percent = None;
            p.bytes = None;
            p.rate = None;
        });
    }

    /// A Git operation starts: clear the last one's numbers, keep the
    /// subject.
    fn start(&self, cancellable: bool) {
        self.update(|p| {
            let subject = std::mem::take(&mut p.subject);
            *p = CloneProgress {
                active: true,
                subject,
                cancellable,
                ..CloneProgress::default()
            };
        });
    }

    /// The operation has ended, however it ended.
    pub fn finish(&self) {
        self.update(|p| {
            p.active = false;
            p.cancellable = false;
        });
    }

    /// Ask the running download to stop. Honoured only while
    /// [`CloneProgress::cancellable`]; otherwise it is ignored.
    pub fn request_cancel(&self) {
        self.update(|p| p.cancel_requested = p.cancellable);
    }

    /// Whether another handle to this progress still exists, i.e. the job
    /// reporting into it has not ended yet. The GUI drops a bar once this
    /// is false.
    pub fn is_held_elsewhere(&self) -> bool {
        Arc::strong_count(&self.0) > 1
    }

    /// Whether a cancel was asked for.
    pub fn cancel_requested(&self) -> bool {
        self.0
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .cancel_requested
    }

    /// Parse one line of Git's stderr and fold it in.
    pub fn apply_line(&self, raw: &str) -> ProgressLine {
        let line = parse_line(raw);
        self.update(|p| p.apply(&line));
        line
    }
}

/// How a Git command run by [`run_git`] ended.
pub(super) struct GitRun {
    /// Git exited with status 0.
    pub success: bool,
    /// Git's stderr without its progress lines, trimmed: what
    /// `Command::output()` captured before `--progress` was passed.
    pub stderr: String,
}

/// Run `cmd` (a Git command already given `--progress`) with stderr read as
/// it arrives into `progress`. With `cancellable`, a
/// [`DownloadProgress::request_cancel`] kills the child and returns
/// `Ok(None)`; the caller decides what to do with what Git left behind.
///
/// stdin and stdout are closed: nothing here answers a prompt
/// (`GIT_TERMINAL_PROMPT=0` is the caller's), and the stdout lines
/// (`Submodule path …: checked out …`) were never reported.
pub(super) fn run_git(
    mut cmd: Command,
    progress: &DownloadProgress,
    cancellable: bool,
) -> std::io::Result<Option<GitRun>> {
    progress.start(cancellable);
    let result = run_git_inner(&mut cmd, progress, cancellable);
    progress.finish();
    result
}

fn run_git_inner(
    cmd: &mut Command,
    progress: &DownloadProgress,
    cancellable: bool,
) -> std::io::Result<Option<GitRun>> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    let stderr = child.stderr.take();
    let reader = {
        let progress = progress.clone();
        std::thread::spawn(move || read_stderr(stderr, &progress))
    };
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if cancellable && progress.cancel_requested() {
            // Not joining the reader: a grandchild (index-pack) may hold the
            // pipe open a little longer, and the text is not needed.
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let stderr = reader.join().unwrap_or_default();
    Ok(Some(GitRun {
        success: status.success(),
        stderr,
    }))
}

/// Read Git's stderr to its end, folding each `\r`/`\n` segment into
/// `progress`; return the non-progress lines, joined and trimmed.
fn read_stderr(stderr: Option<std::process::ChildStderr>, progress: &DownloadProgress) -> String {
    let Some(mut stderr) = stderr else {
        return String::new();
    };
    let mut kept: Vec<String> = Vec::new();
    let mut pending: Vec<u8> = Vec::new();
    let mut buf = [0u8; 4096];
    let take = |segment: &[u8], kept: &mut Vec<String>| {
        let text = String::from_utf8_lossy(segment);
        if text.trim().is_empty() {
            return;
        }
        match progress.apply_line(&text) {
            ProgressLine::Phase(_) | ProgressLine::Summary => {}
            _ => kept.push(text.trim_end().to_string()),
        }
    };
    loop {
        let n = match stderr.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        for &b in &buf[..n] {
            if b == b'\r' || b == b'\n' {
                take(&pending, &mut kept);
                pending.clear();
            } else {
                pending.push(b);
            }
        }
    }
    take(&pending, &mut kept);
    kept.join("\n").trim().to_string()
}

/// Run `job` (which reports into `progress`) on its own thread while this
/// one writes the progress as a single updating line to `out` every
/// `interval`, ending it with a newline; with `out` `None` (not a terminal,
/// a test, CI) nothing is written and `job` simply runs. Used by
/// `kovan-cli`'s literature fetch.
pub fn run_reporting<T: Send, W: std::io::Write>(
    progress: &DownloadProgress,
    mut out: Option<W>,
    interval: Duration,
    job: impl FnOnce() -> T + Send,
) -> T {
    std::thread::scope(|s| {
        let handle = s.spawn(job);
        let mut wrote = false;
        while !handle.is_finished() {
            let snap = progress.snapshot();
            if let (Some(w), true) = (out.as_mut(), snap.active) {
                // `\x1b[K` clears what a longer previous line left behind.
                let _ = write!(w, "\r{}\x1b[K", snap.label());
                let _ = w.flush();
                wrote = true;
            }
            std::thread::sleep(interval);
        }
        if let (Some(w), true) = (out.as_mut(), wrote) {
            let _ = writeln!(w);
        }
        // A panic in `job` is re-raised by `scope` itself; `join` here only
        // collects the value.
        match handle.join() {
            Ok(v) => v,
            Err(e) => std::panic::resume_unwind(e),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Recorded from `git 2.43.0 clone --progress --recurse-submodules` of a
    /// `file://` superproject with one submodule (2026-10-22), plus the
    /// larger-throughput and failure lines Git prints for a network clone.
    #[test]
    fn git_progress_lines_are_parsed() {
        assert_eq!(
            parse_line("Receiving objects:  42% (1234/2938), 210.00 MiB | 5.10 MiB/s"),
            ProgressLine::Phase(PhaseUpdate {
                remote: false,
                phase: "Receiving objects".into(),
                percent: Some(42),
                count: 1234,
                total: Some(2938),
                bytes: Some(220_200_960),
                rate: Some(5_347_738),
                done: false,
            })
        );
        let ProgressLine::Phase(p) =
            parse_line("Receiving objects: 100% (2938/2938), 1.07 GiB | 12.40 MiB/s, done.")
        else {
            panic!("not a phase")
        };
        assert_eq!((p.percent, p.done), (Some(100), true));
        assert_eq!(p.bytes, Some(1_148_903_752));
        let ProgressLine::Phase(p) = parse_line("Receiving objects:  33% (1/3)") else {
            panic!("not a phase")
        };
        assert_eq!(
            (p.percent, p.count, p.total, p.bytes),
            (Some(33), 1, Some(3), None)
        );
        let ProgressLine::Phase(p) = parse_line("Resolving deltas: 100% (10/10), done.") else {
            panic!("not a phase")
        };
        assert_eq!((p.phase.as_str(), p.done), ("Resolving deltas", true));
        let ProgressLine::Phase(p) = parse_line("remote: Counting objects:  25% (1/4)        ")
        else {
            panic!("not a phase")
        };
        assert!(p.remote);
        assert_eq!(
            (p.phase.as_str(), p.percent),
            ("Counting objects", Some(25))
        );
        let ProgressLine::Phase(p) = parse_line("remote: Enumerating objects: 4, done.        ")
        else {
            panic!("not a phase")
        };
        assert_eq!((p.percent, p.count, p.done), (None, 4, true));
        let ProgressLine::Phase(p) =
            parse_line("Receiving objects:  10% (5/50), 512 bytes | 1 byte/s")
        else {
            panic!("not a phase")
        };
        assert_eq!((p.bytes, p.rate), (Some(512), Some(1)));
        assert_eq!(
            parse_line("remote: Total 4 (delta 0), reused 0 (delta 0), pack-reused 0        "),
            ProgressLine::Summary
        );
        assert_eq!(
            parse_line("Cloning into '/tmp/gp/out2/lit'..."),
            ProgressLine::Repository("/tmp/gp/out2/lit".into())
        );
        assert_eq!(
            parse_line("fatal: repository '/nonexistent/x' does not exist"),
            ProgressLine::Error("fatal: repository '/nonexistent/x' does not exist".into())
        );
        for other in [
            "Submodule 'lit' (file:///tmp/gp/sub) registered for path 'lit'",
            "done.",
            "warning: You appear to have cloned an empty repository.",
        ] {
            assert_eq!(parse_line(other), ProgressLine::Other(other.into()));
        }
        assert_eq!(parse_size("3.50 KiB"), Some(3584));
        assert_eq!(parse_size("2 furlongs"), None);
    }

    /// Lines fold into the state the bar draws: the phase, its percentage,
    /// the bytes and rate, the submodule being fetched, and the phases that
    /// completed.
    #[test]
    fn lines_fold_into_the_progress_shown() {
        let progress = DownloadProgress::new();
        progress.set_subject("standard corpus");
        progress.start(true);
        progress.apply_line("Cloning into 'reactor-literature'...");
        progress.apply_line("Receiving objects:  42% (1234/2938), 210.00 MiB | 5.10 MiB/s");
        let s = progress.snapshot();
        assert!(s.active && s.cancellable);
        assert_eq!(s.fraction(), Some(0.42));
        assert_eq!(
            s.label(),
            "standard corpus (reactor-literature): Receiving objects 42% · 210.0 MiB · 5.1 MiB/s"
        );
        progress.apply_line("Receiving objects: 100% (2938/2938), 500.00 MiB | 5.10 MiB/s, done.");
        progress.apply_line("Resolving deltas:  50% (1/2)");
        let s = progress.snapshot();
        assert_eq!(s.completed_phases, ["Receiving objects"]);
        assert_eq!(
            s.label(),
            "standard corpus (reactor-literature): Resolving deltas 50%"
        );
        progress.apply_line("Cloning into '/x/reactor-literature/lit'...");
        assert_eq!(progress.snapshot().label(), "standard corpus (lit)");
        progress.request_cancel();
        assert!(progress.cancel_requested());
        progress.finish();
        let s = progress.snapshot();
        assert!(!s.active && !s.cancellable);
        // A cancel is ignored while nothing cancellable runs.
        let idle = DownloadProgress::new();
        idle.request_cancel();
        assert!(!idle.cancel_requested());
        assert!(!idle.is_held_elsewhere());
        let held = idle.clone();
        assert!(idle.is_held_elsewhere());
        drop(held);
        assert!(!idle.is_held_elsewhere());
        idle.busy("checking for updates");
        assert_eq!(idle.snapshot().fraction(), None);
        assert_eq!(idle.snapshot().label(), "downloading: checking for updates");
    }

    /// The CLI's reporter writes one updating line while the job runs and
    /// ends it with a newline; without a terminal it writes nothing.
    #[test]
    fn the_cli_reporter_writes_one_updating_line() {
        let progress = DownloadProgress::new();
        progress.set_subject("literature");
        let mut out: Vec<u8> = Vec::new();
        let value = run_reporting(&progress, Some(&mut out), Duration::from_millis(5), || {
            progress.start(false);
            progress.apply_line("Receiving objects:  50% (1/2)");
            std::thread::sleep(Duration::from_millis(60));
            progress.finish();
            7
        });
        assert_eq!(value, 7);
        let text = String::from_utf8(out).unwrap();
        assert!(
            text.contains("\rliterature: Receiving objects 50%"),
            "{text:?}"
        );
        assert!(text.ends_with('\n'));
        let quiet = run_reporting(&progress, None::<Vec<u8>>, Duration::from_millis(5), || 1);
        assert_eq!(quiet, 1);
    }
}
