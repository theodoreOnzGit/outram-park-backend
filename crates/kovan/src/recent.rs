// SPDX-License-Identifier: AGPL-3.0-only
//! **Recently opened papers** — a most-recent-first list, persisted locally.
//!
//! # Why this is state and not knowledge
//!
//! Everything else the map draws is a *classification*: a paper is under a
//! topic because someone filed it there, and that fact belongs in the
//! library and is worth committing. "I read this yesterday" is neither — it
//! is a property of this machine and this user, it changes on every open, and
//! two people sharing a library should not fight over it.
//!
//! So it lives in `.kovan/`, which `kovan_root.toml`'s `.gitignore` entry
//! describes as *"derived/local state — fully rebuildable, safe to delete"*.
//! Deleting it loses nothing but convenience, which is the correct blast
//! radius for a recents list.
//!
//! # Why it is capped
//!
//! A recents list that grows without limit stops being recents. [`CAP`] keeps
//! it to what fits on screen next to Unsorted; beyond that the Wiki's own
//! search is the right tool, not a longer list.

use serde::{Deserialize, Serialize};

use crate::root::KovanRoot;

/// How many papers the list remembers.
///
/// Twelve rather than a round ten: enough to cover a working session of
/// cross-referencing without the node becoming a second library index.
pub const CAP: usize = 12;

/// Written at the top of the file so a human who opens it knows it is
/// derived and safe to delete.
const CACHE_HEADER: &str = "# kovan: recently opened papers. Local, derived, safe to delete.\n\
     # Most recent first. Regenerated as papers are opened.";

/// The recents list: citekeys, most recently opened first.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentPapers {
    /// Citekeys, most recent first, at most [`CAP`] long.
    #[serde(default)]
    pub citekeys: Vec<String>,
}

impl RecentPapers {
    /// Record `citekey` as just opened.
    ///
    /// Moves it to the front if already present rather than duplicating, so
    /// re-reading the same paper does not evict everything else — which is
    /// exactly what happens when one paper is being worked on.
    pub fn record(&mut self, citekey: &str) {
        if citekey.trim().is_empty() {
            return;
        }
        self.citekeys.retain(|c| c != citekey);
        self.citekeys.insert(0, citekey.to_string());
        self.citekeys.truncate(CAP);
    }

    /// Whether anything has been opened yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.citekeys.is_empty()
    }

    /// How many papers are remembered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.citekeys.len()
    }

    /// Drop a citekey — for a paper that has been deleted or renamed, so the
    /// list cannot point at something that is no longer there.
    pub fn forget(&mut self, citekey: &str) {
        self.citekeys.retain(|c| c != citekey);
    }

    /// Read the saved list. `None` on any failure, exactly like the graph and
    /// index caches: a missing or corrupt recents file is not an error worth
    /// surfacing, it just means nothing is remembered yet.
    #[must_use]
    pub fn load(root: &KovanRoot) -> Option<Self> {
        let path = root.state_dir().join("recent.toml");
        let text = std::fs::read_to_string(path).ok()?;
        toml::from_str(&text).ok()
    }

    /// Read the saved list, or an empty one.
    #[must_use]
    pub fn load_or_default(root: &KovanRoot) -> Self {
        Self::load(root).unwrap_or_default()
    }

    /// Persist to `.kovan/recent.toml`, atomically — same tmp-then-rename
    /// convention as the graph and index caches, so a crash mid-write cannot
    /// leave a half-file that then fails to parse.
    ///
    /// # Errors
    /// Any I/O or serialisation failure, for a caller that wants to report
    /// it. Most callers should ignore it: failing to remember a recents entry
    /// must never interrupt opening a paper.
    pub fn save(&self, root: &KovanRoot) -> std::io::Result<()> {
        let dir = root.state_dir();
        std::fs::create_dir_all(&dir)?;
        let final_path = dir.join("recent.toml");
        let tmp_path = dir.join("recent.toml.tmp");
        let body = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        std::fs::write(&tmp_path, format!("{CACHE_HEADER}\n{body}"))?;
        std::fs::rename(&tmp_path, &final_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_most_recent_is_first() {
        let mut r = RecentPapers::default();
        r.record("a");
        r.record("b");
        assert_eq!(r.citekeys, ["b", "a"]);
    }

    #[test]
    fn reopening_moves_to_the_front_rather_than_duplicating() {
        let mut r = RecentPapers::default();
        for k in ["a", "b", "c"] {
            r.record(k);
        }
        r.record("a");
        assert_eq!(
            r.citekeys,
            ["a", "c", "b"],
            "re-reading one paper must not evict the rest"
        );
        assert_eq!(r.len(), 3, "no duplicate entry");
    }

    #[test]
    fn the_list_is_capped() {
        let mut r = RecentPapers::default();
        for i in 0..(CAP + 5) {
            r.record(&format!("paper{i}"));
        }
        assert_eq!(r.len(), CAP);
        assert_eq!(
            r.citekeys.first().map(String::as_str),
            Some(format!("paper{}", CAP + 4).as_str()),
            "the newest survives"
        );
        assert!(
            !r.citekeys.iter().any(|c| c == "paper0"),
            "the oldest is evicted"
        );
    }

    #[test]
    fn an_empty_citekey_is_not_recorded() {
        let mut r = RecentPapers::default();
        r.record("");
        r.record("   ");
        assert!(r.is_empty(), "a blank key would draw a nameless box");
    }

    #[test]
    fn forgetting_removes_a_deleted_paper() {
        let mut r = RecentPapers::default();
        for k in ["a", "b"] {
            r.record(k);
        }
        r.forget("a");
        assert_eq!(r.citekeys, ["b"]);
        r.forget("nope");
        assert_eq!(r.citekeys, ["b"], "forgetting an absent key is a no-op");
    }

    #[test]
    fn a_missing_or_corrupt_file_is_none_rather_than_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(
            tmp.path(),
            crate::root::RootConfig::new("lib", "Lib"),
            false,
        )
        .unwrap();
        assert!(RecentPapers::load(&root).is_none(), "nothing saved yet");
        assert!(RecentPapers::load_or_default(&root).is_empty());

        std::fs::create_dir_all(root.state_dir()).unwrap();
        std::fs::write(root.state_dir().join("recent.toml"), "this is not toml {{").unwrap();
        assert!(
            RecentPapers::load(&root).is_none(),
            "a corrupt file must not be an error the user has to deal with"
        );
    }

    #[test]
    fn it_round_trips_through_the_file() {
        let tmp = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(
            tmp.path(),
            crate::root::RootConfig::new("lib", "Lib"),
            false,
        )
        .unwrap();
        let mut r = RecentPapers::default();
        for k in ["liu2002", "choo2023", "li2014"] {
            r.record(k);
        }
        r.save(&root).unwrap();

        let back = RecentPapers::load(&root).expect("saved list reloads");
        assert_eq!(back, r);
        assert_eq!(back.citekeys.first().map(String::as_str), Some("li2014"));
        assert!(
            !root.state_dir().join("recent.toml.tmp").exists(),
            "the atomic write leaves no temp file behind"
        );
    }
}
