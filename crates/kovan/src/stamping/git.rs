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

use std::path::Path;
use std::process::Command;

/// Run `git` in `dir`; stdout on success, the command and stderr otherwise.
pub fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .current_dir(dir)
        .args(["-c", "core.quotePath=false"])
        .args(args)
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;
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
}
