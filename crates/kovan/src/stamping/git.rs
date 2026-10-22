//! The few `git` queries stamping needs, through the `git` command line (as
//! `advanced_git` and `corpus_repos` do): run `git`, resolve a revision, read
//! a file at a revision, and ask whether a file is clean.
//!
//! Moved here from `review_stamps::git` on 2026-10-10 (GitHub #825), when
//! the `review/stamps.toml` stamping path was removed; the diff-hunk parser
//! that only `stamps-check --diff` used went with it.
//!
//! Every path is relative to the workspace directory, `/`-separated; `git`
//! runs with that directory as its working directory and `rev:./path` reads a
//! file relative to it, so a workspace checked out inside another repository
//! still works.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// `git` processes started through this module since kovan started
/// (GitHub #1023: every spawn on the review-state refresh path goes
/// through here, so the refresh can say how many it started).
static SPAWNS: AtomicUsize = AtomicUsize::new(0);
/// The same, by call kind ([`call_kind`]): how many, and their wall time.
static TALLY: Mutex<BTreeMap<String, (usize, Duration)>> = Mutex::new(BTreeMap::new());

/// How many `git` processes this module has started, process-wide (a
/// refresh reports the difference over its run; another thread's `git`
/// calls in the same window are counted too).
pub fn spawns() -> usize {
    SPAWNS.load(Ordering::Relaxed)
}

/// [`spawns`] by call kind, with the time spent in them, for listing the
/// call sites of a refresh.
pub fn spawn_tally() -> BTreeMap<String, (usize, Duration)> {
    TALLY.lock().map(|t| t.clone()).unwrap_or_default()
}

/// The kind of a call: the subcommand and its option names, no values
/// (`log -1 --format -S`, `merge-base --is-ancestor`, `show`).
pub fn call_kind(args: &[&str]) -> String {
    let mut out = args.first().copied().unwrap_or_default().to_string();
    for a in args.iter().skip(1).filter(|a| a.starts_with('-')) {
        let name = if a.starts_with("--") { a.split('=').next().unwrap_or(a) } else { a.get(..2).unwrap_or(a) };
        out.push(' ');
        out.push_str(name);
    }
    out
}

/// Count one spawn of kind `args`, which took `took`.
pub(crate) fn count_spawn(args: &[&str], took: Duration) {
    SPAWNS.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut t) = TALLY.lock() {
        let e = t.entry(call_kind(args)).or_default();
        e.0 += 1;
        e.1 += took;
    }
}

/// Run `git` in `dir`; stdout on success, the command and stderr otherwise.
pub fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let started = Instant::now();
    let out = Command::new("git")
        .current_dir(dir)
        .args(["-c", "core.quotePath=false"])
        .args(args)
        .output();
    count_spawn(args, started.elapsed());
    let out = out.map_err(|e| format!("could not run git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The full commit id `rev` names.
pub fn rev_parse(dir: &Path, rev: &str) -> Result<String, String> {
    Ok(git(
        dir,
        &["rev-parse", "--verify", &format!("{rev}^{{commit}}")],
    )?
    .trim()
    .to_string())
}

/// `file` (relative to `dir`) as it is at `rev`, or `None` when the
/// revision does not contain it (or the revision is not in this clone).
pub fn show(dir: &Path, rev: &str, file: &str) -> Option<String> {
    git(dir, &["show", &format!("{rev}:./{file}")]).ok()
}

/// Whether `file` differs from `HEAD` in the index or the working tree, or
/// is untracked.
pub fn is_dirty(dir: &Path, file: &str) -> Result<bool, String> {
    let s = git(
        dir,
        &["status", "--porcelain", "--untracked-files=all", "--", file],
    )?;
    Ok(!s.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: a throwaway repository with one commit; each query is
    /// checked against what git itself says: `rev_parse` gives the 40-hex
    /// id, `show` the committed text (and `None` for a missing file),
    /// `is_dirty` false on a clean file and true after an edit or for an
    /// untracked one, and a failing `git` command is an `Err` naming it.
    ///
    /// Result (2026-10-10): passes.
    #[test]
    fn queries_answer_as_git_does() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        let run = |a: &[&str]| git(p, a).unwrap();
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.email", "t@example.org"]);
        run(&["config", "user.name", "T"]);
        run(&["config", "commit.gpgsign", "false"]);
        std::fs::write(p.join("a.rs"), "fn a() {}\n").unwrap();
        run(&["add", "a.rs"]);
        run(&["commit", "-q", "-m", "one"]);
        let head = rev_parse(p, "HEAD").unwrap();
        assert_eq!(head.len(), 40, "{head}");
        assert_eq!(show(p, &head, "a.rs").as_deref(), Some("fn a() {}\n"));
        assert_eq!(show(p, &head, "missing.rs"), None);
        assert!(!is_dirty(p, "a.rs").unwrap());
        std::fs::write(p.join("a.rs"), "fn a() { 1; }\n").unwrap();
        assert!(is_dirty(p, "a.rs").unwrap());
        std::fs::write(p.join("b.rs"), "").unwrap();
        assert!(is_dirty(p, "b.rs").unwrap());
        let e = rev_parse(p, "no-such-rev").unwrap_err();
        assert!(e.contains("rev-parse"), "{e}");
    }

    /// Methodology (GitHub #1023): a call's kind keeps the subcommand and
    /// option names and drops values; a `git` call raises [`spawns`] and its
    /// kind's tally.
    ///
    /// Result (2026-10-22): passes.
    #[test]
    fn spawns_are_counted_by_kind() {
        assert_eq!(call_kind(&["log", "-1", "--format=%H", "-Sabc", "--", "x"]), "log -1 --format -S --");
        assert_eq!(call_kind(&["merge-base", "--is-ancestor", "a", "b"]), "merge-base --is-ancestor");
        assert_eq!(call_kind(&["show", "HEAD:./a"]), "show");
        let before = spawns();
        let kind = call_kind(&["--version"]);
        let tally = spawn_tally().get(&kind).map_or(0, |t| t.0);
        git(Path::new("."), &["--version"]).unwrap();
        assert!(spawns() > before);
        assert!(spawn_tally()[&kind].0 > tally);
    }
}
