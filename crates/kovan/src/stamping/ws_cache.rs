//! The **workspace cache** behind [`super::load_workspace`] (GitHub #856;
//! maintainer, 2026-10-13: "this is key").
//!
//! Every stamp step and every Add connection loads the workspace, and a
//! load refreshes every indexed folder: it parses every `.rs` file. On this
//! workspace (801 folders) that was 14.1 s on one thread and 2.5 s shared
//! over the cores (2026-10-13), paid again at each step of one stamp.
//!
//! This module keeps, per workspace root and per folder, what the last load
//! computed, and reuses it when everything that result depends on is the
//! same:
//!
//! ```text
//!  a folder's refreshed index depends on
//!    its kovan.toml            (the previous index: ids, callees, tests)
//!    its .rs files             (names and text)
//!    its review.md             ([upstream], cached reviews)
//!    the claims that can name one of its functions
//!                              (by path in the folder, by a hash one of
//!                               its functions has, or by an id it holds)
//!    HEAD                      only when an id was minted for a function
//!                              that no kovan.toml or review.md names yet
//! ```
//!
//! **A file is "the same"** when its length and modification time are those
//! seen before *and* the modification is at least [`RACY`] older than the
//! moment it was cached; otherwise its text is read and compared by hash.
//! (A file rewritten with the same length inside one timestamp tick would
//! otherwise be missed; git guards its index against the same race.)
//!
//! **Leak Before Break.** The cache can only return what a full load would
//! compute from the same inputs: the test `the_cache_gives_what_a_full_load_
//! gives` runs both after each kind of change and compares them. Nothing is
//! written to disk; the cache lives in the process and is dropped with it.

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use kovan_common::code_index::ids::Claim;
use kovan_common::review::index::FolderIndex;

use super::ReviewMdFile;

/// A file modified less than this before it was cached is compared by
/// content, not by timestamp (module doc).
const RACY: Duration = Duration::from_secs(2);

/// At most this many workspace roots are remembered (tests open many
/// throwaway ones); beyond it the cache starts again.
const MAX_ROOTS: usize = 16;

/// A file as last seen: length, modification time, content hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Seen {
    len: u64,
    modified: Option<SystemTime>,
    hash: u64,
}

fn hash_of(text: &str) -> u64 {
    let mut h = DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

impl Seen {
    /// Record `text`, just read from `path`.
    pub(super) fn of(path: &Path, text: &str) -> Seen {
        let meta = std::fs::metadata(path).ok();
        Seen {
            len: meta.as_ref().map_or(text.len() as u64, |m| m.len()),
            modified: meta.and_then(|m| m.modified().ok()),
            hash: hash_of(text),
        }
    }

    /// Whether `path` still holds what was seen (module doc, "the same").
    pub(super) fn still(&self, path: &Path, cached_at: SystemTime) -> bool {
        let Ok(meta) = std::fs::metadata(path) else {
            return false;
        };
        if meta.len() != self.len {
            return false;
        }
        let modified = meta.modified().ok();
        let settled = modified.is_some_and(|m| m + RACY < cached_at);
        if modified == self.modified && settled {
            return true;
        }
        std::fs::read_to_string(path).is_ok_and(|t| hash_of(&t) == self.hash)
    }
}

/// One `review.md` as parsed.
#[derive(Debug, Clone)]
pub(super) struct CachedReview {
    pub(super) seen: Seen,
    pub(super) cached_at: SystemTime,
    pub(super) file: ReviewMdFile,
}

/// One folder's refreshed index and what it was computed from.
#[derive(Debug, Clone)]
pub(super) struct CachedFolder {
    pub(super) cached_at: SystemTime,
    pub(super) toml: Seen,
    /// The folder's `.rs` files by name.
    pub(super) files: BTreeMap<String, Seen>,
    /// Hash of the folder's `review.md` text; `None` without one.
    pub(super) review: Option<u64>,
    /// The claims that could name one of its functions, sorted.
    pub(super) claims: Vec<Claim>,
    /// `Some(head)` when an id was minted with it (module doc).
    pub(super) head: Option<String>,
    /// Ids of the `kovan.toml` on disk (the priors).
    pub(super) prior_ids: BTreeSet<String>,
    pub(super) fresh: FolderIndex,
}

impl CachedFolder {
    /// The claims of `all` that can affect this folder's ids: by a path in
    /// the folder, a hash one of its functions has, or an id it holds.
    pub(super) fn relevant(dir: &str, prior_ids: &BTreeSet<String>, fresh: &FolderIndex, all: &[Claim]) -> Vec<Claim> {
        let ids: BTreeSet<&str> = fresh.functions().map(|(_, f)| f.id.as_str()).collect();
        let hashes: BTreeSet<&str> = fresh.functions().map(|(_, f)| f.hash.as_str()).collect();
        let in_dir = |p: &str| {
            let file = p.split("::").next().unwrap_or("");
            file.rsplit_once('/').map_or("", |(d, _)| d) == dir
        };
        let mut v: Vec<Claim> = all
            .iter()
            .filter(|c| {
                ids.contains(c.id.as_str())
                    || prior_ids.contains(&c.id)
                    || c.hash.as_deref().is_some_and(|h| hashes.contains(h))
                    || c.path.as_deref().is_some_and(in_dir)
            })
            .cloned()
            .collect();
        v.sort();
        v
    }

    /// Whether the cached index is what a refresh would compute now.
    /// `names` are the folder's `.rs` files now; `review` its `review.md`
    /// text hash now.
    pub(super) fn valid(
        &self,
        root: &Path,
        dir: &str,
        names: &BTreeSet<String>,
        review: Option<u64>,
        all_claims: &[Claim],
        head: &str,
    ) -> bool {
        let at = |name: &str| root.join(dir).join(name);
        self.review == review
            && self.head.as_deref().is_none_or(|h| h == head)
            && names.len() == self.files.len()
            && names.iter().all(|n| self.files.contains_key(n))
            && self.toml.still(&at(super::KOVAN_TOML), self.cached_at)
            && self.files.iter().all(|(n, s)| s.still(&at(n), self.cached_at))
            && Self::relevant(dir, &self.prior_ids, &self.fresh, all_claims) == self.claims
    }
}

/// What is remembered of one workspace root.
#[derive(Debug, Clone, Default)]
pub(super) struct RootCache {
    /// Folder -> its parsed `review.md`.
    pub(super) reviews: BTreeMap<String, CachedReview>,
    /// Folder -> its refreshed index.
    pub(super) folders: BTreeMap<String, CachedFolder>,
    /// How many folders the last load had to refresh (the rest were
    /// reused).
    pub(super) refreshed_last: usize,
}

fn store() -> &'static Mutex<HashMap<PathBuf, RootCache>> {
    static STORE: OnceLock<Mutex<HashMap<PathBuf, RootCache>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Take `root`'s cache out (empty when there is none). The lock is held
/// only for the take, so loads of different workspaces do not wait for
/// each other; two loads of one workspace at once both compute, and the
/// later [`put`] wins.
pub(super) fn take(root: &Path) -> RootCache {
    store().lock().map(|mut m| m.remove(root).unwrap_or_default()).unwrap_or_default()
}

/// Remember `cache` for `root`.
pub(super) fn put(root: &Path, cache: RootCache) {
    if let Ok(mut m) = store().lock() {
        if m.len() >= MAX_ROOTS {
            m.clear();
        }
        m.insert(root.to_path_buf(), cache);
    }
}

/// Forget everything about `root`: the next load computes all of it.
pub fn forget(root: &Path) {
    if let Ok(mut m) = store().lock() {
        m.remove(root);
    }
}

/// How many folders the last load of `root` refreshed; `None` when the
/// root is not cached. For the view's status line and the tests.
pub fn refreshed_last(root: &Path) -> Option<usize> {
    store().lock().ok().and_then(|m| m.get(root).map(|c| c.refreshed_last))
}

/// The content hash [`CachedFolder::review`] holds.
pub(super) fn text_hash(text: &str) -> u64 {
    hash_of(text)
}
