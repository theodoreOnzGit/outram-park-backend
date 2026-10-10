//! **A function's unified diff since its last review** (GitHub #770; #740
//! decision 8: "a unified diff, as git or lazygit shows it: the full
//! current source, with removed lines struck and added lines marked,
//! against the code at the commit in its last review").
//!
//! ```text
//!   git show <review commit>:<file> ──> the function's lines then  ─┐
//!                                                                   ├─> line_diff ──> [Same | Removed | Added]
//!   <working tree>/<file>          ──> the function's lines now  ──┘
//! ```
//!
//! The function is found in each text with the same `syn` hasher the index
//! uses ([`kovan_common::review::hash::hash_functions`]), by its qualified
//! name and `#k` duplicate index ([`super::super::states::fn_at`]'s rule),
//! so the diff covers exactly the lines (doc comment included) that the
//! hash covers.
//!
//! # Why a diff of our own
//!
//! Searched first (2026-10-10): `commands::code_walk::lesson::line_diff`
//! is a set difference (order lost, repeated lines missed), and
//! `review_stamps::git::diff_hunks` gives zero-context hunk ranges per
//! file, not the lines. `similar` is in `Cargo.lock` only as a transitive
//! dependency; making it a direct one is a dependency decision for the
//! maintainer. A function is at most a few hundred lines, so the classic
//! longest-common-subsequence table is fast enough; above
//! [`MAX_CELLS`] table cells the diff degrades, visibly, to "every old
//! line removed, every new line added" ([`LineDiff::coarse`]), never to a
//! wrong alignment.

use std::path::Path;

use kovan_common::review::hash::{hash_functions, HashedFn};

use crate::review_stamps::git;

/// The largest LCS table (old lines x new lines) computed; beyond it the
/// diff is coarse (module doc). 4 M cells is a 2000 x 2000-line function.
pub const MAX_CELLS: usize = 4_000_000;

/// One line of a unified diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLine {
    /// In both; old and new 1-based line numbers within the function.
    Same {
        old: usize,
        new: usize,
        text: String,
    },
    /// Only in the old text (struck through in the view).
    Removed { old: usize, text: String },
    /// Only in the new text (marked in the view).
    Added { new: usize, text: String },
}

/// A line diff.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LineDiff {
    pub lines: Vec<DiffLine>,
    /// The texts were too long for the table: every old line is shown
    /// removed and every new line added (module doc).
    pub coarse: bool,
}

impl LineDiff {
    /// No line was added or removed.
    pub fn unchanged(&self) -> bool {
        self.lines
            .iter()
            .all(|l| matches!(l, DiffLine::Same { .. }))
    }

    /// `(added, removed)` line counts.
    pub fn counts(&self) -> (usize, usize) {
        self.lines.iter().fold((0, 0), |(a, r), l| match l {
            DiffLine::Added { .. } => (a + 1, r),
            DiffLine::Removed { .. } => (a, r + 1),
            DiffLine::Same { .. } => (a, r),
        })
    }

    /// The diff as `git diff` prints a hunk body: `' '`, `'-'` or `'+'`
    /// then the line.
    pub fn to_unified(&self) -> String {
        let mut s = String::new();
        for l in &self.lines {
            let (c, t) = match l {
                DiffLine::Same { text, .. } => (' ', text),
                DiffLine::Removed { text, .. } => ('-', text),
                DiffLine::Added { text, .. } => ('+', text),
            };
            s.push(c);
            s.push_str(t);
            s.push('\n');
        }
        s
    }
}

/// The unified line diff of `old` against `new` (longest common
/// subsequence; at a tie a removal is listed before an addition, as git
/// does).
pub fn line_diff(old: &str, new: &str) -> LineDiff {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    let (n, m) = (a.len(), b.len());
    if n.saturating_mul(m) > MAX_CELLS {
        let mut lines: Vec<DiffLine> = a
            .iter()
            .enumerate()
            .map(|(i, t)| DiffLine::Removed {
                old: i + 1,
                text: t.to_string(),
            })
            .collect();
        lines.extend(b.iter().enumerate().map(|(j, t)| DiffLine::Added {
            new: j + 1,
            text: t.to_string(),
        }));
        return LineDiff {
            lines,
            coarse: true,
        };
    }
    // lcs[i][j]: the LCS length of a[i..] and b[j..], row-major.
    let w = m + 1;
    let mut lcs = vec![0u32; (n + 1) * w];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i * w + j] = if a[i] == b[j] {
                lcs[(i + 1) * w + j + 1] + 1
            } else {
                lcs[(i + 1) * w + j].max(lcs[i * w + j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut lines = Vec::with_capacity(n.max(m));
    while i < n || j < m {
        if i < n && j < m && a[i] == b[j] {
            lines.push(DiffLine::Same {
                old: i + 1,
                new: j + 1,
                text: a[i].to_string(),
            });
            i += 1;
            j += 1;
        } else if i < n && (j == m || lcs[(i + 1) * w + j] >= lcs[i * w + j + 1]) {
            lines.push(DiffLine::Removed {
                old: i + 1,
                text: a[i].to_string(),
            });
            i += 1;
        } else {
            lines.push(DiffLine::Added {
                new: j + 1,
                text: b[j].to_string(),
            });
            j += 1;
        }
    }
    LineDiff {
        lines,
        coarse: false,
    }
}

/// `qual` split into the hasher's qualified name and the 1-based `#k`
/// duplicate index (the rule of `states::split_qual`).
fn split_qual(qual: &str) -> (String, Option<usize>) {
    let squash = |s: &str| s.split_whitespace().collect::<String>();
    match qual.rsplit_once('#') {
        Some((base, k)) if !k.is_empty() && k.bytes().all(|b| b.is_ascii_digit()) => {
            (squash(base), k.parse().ok())
        }
        _ => (squash(qual), None),
    }
}

/// The function `qual` in the Rust `source`, as the index finds it.
pub fn find_fn(source: &str, qual: &str) -> Option<HashedFn> {
    let fns = hash_functions(source).ok()?;
    let (base, k) = split_qual(qual);
    let same: Vec<&HashedFn> = fns
        .iter()
        .filter(|h| h.entry.qualname().split_whitespace().collect::<String>() == base)
        .collect();
    match (k, same.as_slice()) {
        (None, [one]) => Some((*one).clone()),
        (Some(k), many) if k >= 1 && many.len() > 1 => many.get(k - 1).map(|h| (*h).clone()),
        _ => None,
    }
}

/// Lines `[a, b]` (1-based, inclusive) of `text`, joined with `\n`.
pub fn lines_of(text: &str, [a, b]: [u32; 2]) -> String {
    text.lines()
        .skip(a.saturating_sub(1) as usize)
        .take((b + 1).saturating_sub(a.max(1)) as usize)
        .collect::<Vec<_>>()
        .join("\n")
}

/// The source of function `qual` in `source` (doc comment included), with
/// its line range; `None` when the file does not parse or does not hold it.
pub fn fn_source(source: &str, qual: &str) -> Option<(String, [u32; 2])> {
    let f = find_fn(source, qual)?;
    Some((lines_of(source, f.entry.lines), f.entry.lines))
}

/// A function's diff since a review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnDiff {
    /// `file.rs::qual` now.
    pub path: String,
    /// The commit the old side was read at.
    pub since: String,
    /// The function's lines in the file now (1-based inclusive).
    pub lines_now: [u32; 2],
    pub diff: LineDiff,
}

/// Why a diff could not be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffError {
    /// The working-tree file cannot be read or does not hold the function.
    NotNow(String),
    /// The commit does not hold the file, or the file did not hold the
    /// function then (it was renamed or moved since: shown, not guessed).
    NotThen { path: String, commit: String },
}

impl std::fmt::Display for DiffError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotNow(p) => write!(f, "{p}: not found in the working tree"),
            Self::NotThen { path, commit } => {
                let short: String = commit.chars().take(10).collect();
                write!(
                    f,
                    "{path}: not found at commit {short} (moved or renamed since?)"
                )
            }
        }
    }
}

/// The diff of function `qual` of `file` between `old_path` (`file.rs::qual`
/// at commit `since`, usually the reviewed path) and the working tree now
/// (module doc). Runs `git show`: call it off the UI thread.
pub fn function_diff(
    root: &Path,
    file: &str,
    qual: &str,
    old_path: &str,
    since: &str,
) -> Result<FnDiff, DiffError> {
    let path = format!("{file}::{qual}");
    let now_text =
        std::fs::read_to_string(root.join(file)).map_err(|_| DiffError::NotNow(path.clone()))?;
    let (now, lines_now) =
        fn_source(&now_text, qual).ok_or_else(|| DiffError::NotNow(path.clone()))?;
    let not_then = || DiffError::NotThen {
        path: old_path.to_string(),
        commit: since.to_string(),
    };
    let (old_file, old_qual) = old_path
        .split_once(".rs::")
        .map(|(f, q)| (format!("{f}.rs"), q.to_string()))
        .ok_or_else(not_then)?;
    let then_text = git::show(root, since, &old_file).ok_or_else(not_then)?;
    let (then, _) = fn_source(&then_text, &old_qual).ok_or_else(not_then)?;
    Ok(FnDiff {
        path,
        since: since.to_string(),
        lines_now,
        diff: line_diff(&then, &now),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: a one-line edit in the middle, a deletion and an
    /// insertion at the ends, against a hand-written expected unified diff.
    /// Result (2026-10-10): exact match, as `git diff` would print the hunk
    /// body.
    #[test]
    fn line_diff_is_unified_and_ordered() {
        let old = "fn f() {\n    let a = 1;\n    a + 1\n}\n";
        let new = "/// Doc.\nfn f() {\n    let a = 2;\n    a + 1\n}\n";
        let d = line_diff(old, new);
        assert_eq!(
            d.to_unified(),
            "+/// Doc.\n fn f() {\n-    let a = 1;\n+    let a = 2;\n     a + 1\n }\n"
        );
        assert_eq!(d.counts(), (2, 1));
        assert!(!d.unchanged() && !d.coarse);
        assert!(line_diff(old, old).unchanged());
        assert_eq!(
            d.lines[2],
            DiffLine::Removed {
                old: 2,
                text: "    let a = 1;".into()
            }
        );
    }

    /// Methodology: repeated identical lines, which a set difference (the
    /// workspace's older `line_diff`) cannot see moving. Result: the
    /// removed duplicate is reported.
    #[test]
    fn repeated_lines_are_tracked_in_order() {
        let d = line_diff("x\ny\nx\n", "x\ny\n");
        assert_eq!(d.counts(), (0, 1));
        assert_eq!(d.to_unified(), " x\n y\n-x\n");
    }

    /// Methodology: a table larger than [`MAX_CELLS`] degrades to the
    /// coarse form, marked as such. Result: every line removed then added.
    #[test]
    fn huge_inputs_degrade_visibly() {
        let big: String = (0..2100).map(|i| format!("l{i}\n")).collect();
        let d = line_diff(&big, &big);
        assert!(d.coarse);
        assert_eq!(d.counts(), (2100, 2100));
    }

    /// Methodology: find a method and a free function in source with the
    /// index's `qual` rule, including a `#2` duplicate. Result: the right
    /// lines, doc comment included.
    #[test]
    fn fn_source_finds_items_by_qual() {
        let src = "/// One.\nfn a() {}\n\nstruct S;\nimpl S {\n    /// M.\n    fn m(&self) -> u8 {\n        1\n    }\n}\n#[cfg(test)]\nfn d() {}\n#[cfg(not(test))]\nfn d() {}\n";
        assert_eq!(
            fn_source(src, "a").unwrap(),
            ("/// One.\nfn a() {}".into(), [1, 2])
        );
        let (m, l) = fn_source(src, "S::m").unwrap();
        assert_eq!(l, [6, 9]);
        assert!(m.starts_with("    /// M.") && m.ends_with("    }"));
        assert_eq!(fn_source(src, "d#2").unwrap().1, [13, 14]);
        assert!(fn_source(src, "d").is_none(), "ambiguous without #k");
        assert!(fn_source(src, "zz").is_none());
        assert_eq!(lines_of("a\nb\nc", [2, 3]), "b\nc");
    }
}
