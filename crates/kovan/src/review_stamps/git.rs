//! The few `git` queries the stamps need, through the `git` command line
//! (as `advanced_git` and `corpus_repos` do): file content at a revision,
//! whether a file is clean, `HEAD`, and the changed line ranges of a diff.
//!
//! Every path is relative to the workspace directory, `/`-separated; `git`
//! runs with that directory as its working directory, `rev:./path` reads a
//! file relative to it and `git diff --relative` reports paths relative to it,
//! so a workspace checked out inside another repository still works.

use std::collections::BTreeMap;
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

/// One hunk of a zero-context diff: the old and new line ranges, 1-based
/// `(start, count)`. A count of zero is a pure insertion or deletion; the
/// other side then carries the lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hunk {
    pub old: (u32, u32),
    pub new: (u32, u32),
}

impl Hunk {
    /// Whether the hunk changes any old line in `[a, b]`.
    pub fn touches_old(&self, a: u32, b: u32) -> bool {
        overlaps(self.old, a, b)
    }
    /// Whether the hunk changes any new line in `[a, b]`.
    pub fn touches_new(&self, a: u32, b: u32) -> bool {
        overlaps(self.new, a, b)
    }
}

fn overlaps((start, count): (u32, u32), a: u32, b: u32) -> bool {
    count > 0 && start <= b && start + count > a
}

/// The two revisions a `--diff` range compares: `A..B`, `A...B` (from the
/// merge base) or `A..` / `A...` (to `HEAD`). Both resolved to commit ids.
pub fn resolve_range(dir: &Path, range: &str) -> Result<(String, String), String> {
    let (a, b, symmetric) = if let Some((a, b)) = range.split_once("...") {
        (a, b, true)
    } else if let Some((a, b)) = range.split_once("..") {
        (a, b, false)
    } else {
        return Err(format!(
            "`{range}` is not a revision range; give A..B (e.g. HEAD~1..HEAD)"
        ));
    };
    let a = if a.is_empty() { "HEAD" } else { a };
    let b = if b.is_empty() { "HEAD" } else { b };
    let new = rev_parse(dir, b)?;
    let old = if symmetric {
        git(dir, &["merge-base", a, b])?.trim().to_string()
    } else {
        rev_parse(dir, a)?
    };
    Ok((old, new))
}

/// The changed hunks per file between two commits, from
/// `git diff --unified=0 --no-renames --relative`: a renamed file is a
/// deletion plus an addition, so a stamp on the old path is in scope.
/// Paths are relative to `dir`.
pub fn diff_hunks(dir: &Path, old: &str, new: &str) -> Result<BTreeMap<String, Vec<Hunk>>, String> {
    let text = git(
        dir,
        &[
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--unified=0",
            "--no-renames",
            "--relative",
            old,
            new,
        ],
    )?;
    Ok(parse_diff(&text))
}

/// Parse unified-diff text into hunks per file. A file is keyed by its old
/// path when it has one (a deletion or modification) and by its new path
/// otherwise; with `--no-renames` the two are the same for a modification.
pub fn parse_diff(text: &str) -> BTreeMap<String, Vec<Hunk>> {
    let mut out: BTreeMap<String, Vec<Hunk>> = BTreeMap::new();
    let mut old_path: Option<String> = None;
    let mut current: Option<String> = None;
    for line in text.lines() {
        if line.starts_with("diff --git ") {
            old_path = None;
            current = None;
        } else if let Some(p) = line.strip_prefix("--- ") {
            old_path = strip_side(p, "a/");
        } else if let Some(p) = line.strip_prefix("+++ ") {
            let key = old_path.clone().or_else(|| strip_side(p, "b/"));
            if let Some(k) = &key {
                out.entry(k.clone()).or_default();
            }
            current = key;
        } else if line.starts_with("@@ ") {
            if let (Some(file), Some(h)) = (&current, parse_hunk_header(line)) {
                out.entry(file.clone()).or_default().push(h);
            }
        }
    }
    out
}

fn strip_side(p: &str, prefix: &str) -> Option<String> {
    let p = p.trim_end();
    if p == "/dev/null" {
        return None;
    }
    Some(p.strip_prefix(prefix).unwrap_or(p).to_string())
}

/// `@@ -a[,b] +c[,d] @@ ...` (a missing count is 1).
fn parse_hunk_header(line: &str) -> Option<Hunk> {
    let mut parts = line.split_whitespace().skip(1);
    let old = range(parts.next()?.strip_prefix('-')?)?;
    let new = range(parts.next()?.strip_prefix('+')?)?;
    Some(Hunk { old, new })
}

fn range(s: &str) -> Option<(u32, u32)> {
    match s.split_once(',') {
        Some((a, b)) => Some((a.parse().ok()?, b.parse().ok()?)),
        None => Some((s.parse().ok()?, 1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: a two-file zero-context diff (a modification with an
    /// insertion, a change and a deletion; a deleted file) parsed into hunks,
    /// and the overlap rule on each side.
    ///
    /// Result (2026-10-06): passes.
    #[test]
    fn parses_zero_context_hunks() {
        let text = "\
diff --git a/src/a.rs b/src/a.rs
index 1..2 100644
--- a/src/a.rs
+++ b/src/a.rs
@@ -3,0 +4,2 @@ fn x() {
+    let y = 1;
+    let z = 2;
@@ -10 +12 @@
-a
+b
@@ -20,3 +21,0 @@
-x
-y
-z
diff --git a/src/gone.rs b/src/gone.rs
deleted file mode 100644
--- a/src/gone.rs
+++ /dev/null
@@ -1,2 +0,0 @@
-fn f() {}
-
";
        let d = parse_diff(text);
        assert_eq!(d.keys().collect::<Vec<_>>(), ["src/a.rs", "src/gone.rs"]);
        let a = &d["src/a.rs"];
        assert_eq!(
            a[0],
            Hunk {
                old: (3, 0),
                new: (4, 2)
            }
        );
        assert_eq!(
            a[1],
            Hunk {
                old: (10, 1),
                new: (12, 1)
            }
        );
        assert!(!a[0].touches_old(1, 100) && a[0].touches_new(5, 9) && !a[0].touches_new(6, 9));
        assert!(a[2].touches_old(22, 30) && !a[2].touches_old(23, 30));
        assert_eq!(
            d["src/gone.rs"],
            vec![Hunk {
                old: (1, 2),
                new: (0, 0)
            }]
        );
    }
}
