//! **Self-healing** of `kovan.toml` (#739: "kovan.toml is a disposable
//! cache"; #767). `kovan-cli index` regenerates every folder's file and
//! writes it **without asking** when it is missing, malformed, hand-edited,
//! stale or holds merge-conflict markers, and removes an orphan (a
//! code-folder `kovan.toml` whose folder no longer has indexed `.rs`
//! files). "Silently" means no prompt, not no trace: every write and
//! removal is counted by reason in the run's summary (Leak Before Break,
//! `docs/kovan.md`).
//!
//! Two cases are **never** touched, only reported:
//!
//! - a `kovan.toml` of another kind (a literature entity, `kind = "paper"`)
//!   in an indexed folder ([`Existing::Foreign`]);
//! - a code-folder `kovan.toml` that was not written for the folder it is
//!   in (its `dir` names another folder: a test fixture, a copy). Only a
//!   file that says `dir = "<its own folder>"` is ever removed as an orphan
//!   ([`is_own_orphan`]).
//!
//! `review.md` is human-owned and is never written here.

use crate::review::index::{FolderIndex, IndexError, CODE_FOLDER_KIND};

/// What is on disk where a folder's `kovan.toml` goes, compared with the
/// freshly generated text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Existing {
    Missing,
    /// Holds `<<<<<<<` / `=======` / `>>>>>>>` lines.
    Conflicted,
    /// Not readable as a code-folder index (the reason).
    Malformed(String),
    /// A `kovan.toml` of another kind: left alone.
    Foreign { kind: String },
    /// Readable, but not the bytes this run generates (stale or edited by
    /// hand; the two cannot be told apart, and both are regenerated).
    Changed,
    Unchanged,
}

impl Existing {
    /// Whether the generated text is written.
    pub fn write(&self) -> bool {
        !matches!(self, Existing::Unchanged | Existing::Foreign { .. })
    }

    /// A short label for the summary.
    pub fn label(&self) -> &'static str {
        match self {
            Existing::Missing => "missing",
            Existing::Conflicted => "conflict markers",
            Existing::Malformed(_) => "malformed",
            Existing::Foreign { .. } => "another kind (left alone)",
            Existing::Changed => "stale or hand-edited",
            Existing::Unchanged => "unchanged",
        }
    }
}

/// Whether `text` holds git merge-conflict markers.
pub fn has_conflict_markers(text: &str) -> bool {
    text.lines().any(|l| {
        l.starts_with("<<<<<<< ") || l == "<<<<<<<" || l.starts_with(">>>>>>> ") || l == "======="
    })
}

/// Classify the file on disk (`None` when absent) against `generated`.
pub fn classify(existing: Option<&str>, generated: &str) -> Existing {
    let Some(text) = existing else {
        return Existing::Missing;
    };
    if text == generated {
        return Existing::Unchanged;
    }
    if has_conflict_markers(text) {
        return Existing::Conflicted;
    }
    match FolderIndex::parse(text) {
        Ok(_) => Existing::Changed,
        Err(IndexError::NotACodeFolder { kind }) if !kind.is_empty() => Existing::Foreign { kind },
        Err(e) => Existing::Malformed(e.to_string()),
    }
}

/// Whether `text`, found at `dir/kovan.toml` in a folder with no indexed
/// `.rs` file, is an orphan this tool wrote for that folder (and so may be
/// removed). Read line by line, so a conflicted file is still recognised.
pub fn is_own_orphan(text: &str, dir: &str) -> bool {
    let kind = format!("kind = \"{CODE_FOLDER_KIND}\"");
    let own = format!("dir = \"{dir}\"");
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    lines.contains(&kind.as_str()) && lines.contains(&own.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: every case of [`classify`] on a generated index: the
    /// same bytes, nothing, a reordered but readable file, a file with
    /// conflict markers, broken TOML, and a literature `kovan.toml`; which
    /// of them are written; and the orphan rule (own `dir` only, conflicted
    /// files included, a fixture naming another folder never).
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn existing_files_are_classified_and_only_ours_are_touched() {
        let idx = FolderIndex::new("x", "crates/x/src");
        let gen = idx.to_toml().unwrap();
        assert_eq!(classify(Some(&gen), &gen), Existing::Unchanged);
        assert_eq!(classify(None, &gen), Existing::Missing);
        let edited = format!("{gen}\n# a hand note\n");
        assert_eq!(classify(Some(&edited), &gen), Existing::Changed);
        let conflicted = format!("<<<<<<< HEAD\n{gen}=======\n{gen}>>>>>>> other\n");
        assert_eq!(classify(Some(&conflicted), &gen), Existing::Conflicted);
        assert!(matches!(classify(Some("[[[ not toml"), &gen), Existing::Malformed(_)));
        let paper = "schema_version = 1\nid = \"smith2020\"\nkind = \"paper\"\n";
        let foreign = classify(Some(paper), &gen);
        assert_eq!(foreign, Existing::Foreign { kind: "paper".into() });
        assert!(!foreign.write() && !Existing::Unchanged.write());
        for e in [Existing::Missing, Existing::Conflicted, Existing::Changed] {
            assert!(e.write(), "{}", e.label());
        }
        assert!(is_own_orphan(&gen, "crates/x/src"));
        assert!(is_own_orphan(&conflicted, "crates/x/src"));
        assert!(!is_own_orphan(&gen, "crates/x/tests/fixtures"));
        assert!(!is_own_orphan(paper, "crates/x/src"));
    }
}
