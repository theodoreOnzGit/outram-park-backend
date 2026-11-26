//! The **tracked / derived split** of a folder's `kovan.toml` (GitHub
//! #1776, #1777; maintainer, 2026-11-26: "Git holds what the maintainer
//! needs to notice; the cache holds what a machine can rederive", and "I
//! don't want a clogged up git status").
//!
//! In memory a [`FolderIndex`] is whole, as before. On disk it is two files:
//!
//! | Tracked `<dir>/kovan.toml` (in git) | Derived cache [`derived_rel`] (gitignored, under `target/`) |
//! |---|---|
//! | `id`, `name`, `qual`, `item`/`shape`, `hash`, `doc_hash`, `test`, `physical_interface` | `lines` |
//! | `[test_run]`, `[targeted_run]`, `[upstream]`, `[[review]]`, `[[deleted_folder]]` | `callees`, `uses`, `index_out_of_date` |
//! | `reached_by` of a **reviewed** function only (below) | `reached_by` of every function |
//!
//! The cache file is the whole index in today's format, so it reads with
//! [`FolderIndex::parse`]; [`merge`] takes only the derived fields from it,
//! function by function, and only where the cache entry has the **same id
//! and the same `hash`** as the tracked one. Anything else (no cache, a
//! cache from older code) leaves the function marked `index_out_of_date`:
//! shown as such, never trusted silently.
//!
//! **Why reached_by stays tracked for reviewed functions** (maintainer,
//! 2026-11-26, the recommended choice on #1776): a stamp's rung is derived
//! from the tests that reached the function **as committed at the review
//! commit** (`kovan::stamping::states::tests_at`). Those must be in git.
//! Only functions with a `[[review]]` entry keep it, so the churn is limited
//! to reviewed functions whose reaching tests change.
//!
//! **Order.** The tracked file lists a module's functions by `(qual, id)`,
//! so moving code around changes the cache only; [`FolderIndex::normalise`]
//! sorts by line once the cache supplies the lines.
//!
//! **Old files still load** (kovan schemas never break): a `kovan.toml`
//! written before this split carries the derived fields inline; [`merge`]
//! keeps them when no matching cache entry exists. The next write splits it.
//! A kovan build from before the split reads a tracked file as malformed
//! (no `lines`) and regenerates it, as the module doc of
//! [`crate::review::index`] says it does for any unreadable cache.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::review::index::{lines_unknown, FolderIndex, FunctionIndex, IndexError};

/// The cache folder, workspace-relative. Under `target/`, so gitignored.
pub const DERIVED_DIR: &str = "target/kovan/derived";

/// The cache file's name in its mirror folder.
pub const DERIVED_FILE: &str = "kovan.derived.toml";

/// Where folder `dir`'s cache lives, workspace-relative.
pub fn derived_rel(dir: &str) -> String {
    if dir.is_empty() {
        format!("{DERIVED_DIR}/{DERIVED_FILE}")
    } else {
        format!("{DERIVED_DIR}/{dir}/{DERIVED_FILE}")
    }
}

/// The ids of the functions with a cached review (`[[review]]`), whose
/// `reached_by` stays tracked (module doc).
fn reviewed(idx: &FolderIndex) -> BTreeSet<String> {
    idx.reviews.iter().map(|r| r.function.clone()).collect()
}

/// `idx` with only the tracked fields (module doc table), functions in
/// `(qual, id)` order.
pub fn tracked(idx: &FolderIndex) -> FolderIndex {
    let keep = reviewed(idx);
    let mut t = idx.clone();
    for m in t.modules.values_mut() {
        m.functions.sort_by(|a, b| (&a.qual, &a.id).cmp(&(&b.qual, &b.id)));
        for f in &mut m.functions {
            f.lines = [0, 0];
            f.callees.clear();
            f.uses.clear();
            f.index_out_of_date = false;
            if !keep.contains(&f.id) {
                f.reached_by.clear();
            }
        }
    }
    t
}

/// The tracked `kovan.toml` text of `idx`: what git sees. Deterministic,
/// so an unchanged tracked field set gives the same bytes.
pub fn tracked_text(idx: &FolderIndex) -> Result<String, IndexError> {
    tracked(idx).to_toml()
}

/// The derived cache text of `idx`: the whole index.
pub fn derived_text(idx: &FolderIndex) -> Result<String, IndexError> {
    idx.to_toml()
}

/// The whole index from a tracked one and its cache (module doc).
pub fn merge(mut tracked: FolderIndex, cache: Option<FolderIndex>) -> FolderIndex {
    let cached: BTreeMap<String, FunctionIndex> = cache
        .filter(|c| c.dir == tracked.dir)
        .map(|c| {
            c.modules
                .into_values()
                .flat_map(|m| m.functions)
                .map(|f| (f.id.clone(), f))
                .collect()
        })
        .unwrap_or_default();
    for m in tracked.modules.values_mut() {
        for f in &mut m.functions {
            match cached.get(&f.id) {
                Some(c) if c.hash == f.hash => {
                    f.lines = c.lines;
                    f.callees = c.callees.clone();
                    f.uses = c.uses.clone();
                    f.reached_by = c.reached_by.clone();
                    f.index_out_of_date = c.index_out_of_date;
                }
                // A file from before the split: its inline fields stand.
                _ if !lines_unknown(&f.lines) => {}
                _ => f.index_out_of_date = true,
            }
        }
    }
    tracked.normalise();
    tracked
}

/// Read a tracked `kovan.toml`'s text and merge `root`'s cache for its
/// folder. The cache is optional: missing or unreadable, it is ignored.
pub fn load(root: &Path, text: &str) -> Result<FolderIndex, IndexError> {
    let t = FolderIndex::parse(text)?;
    let cache = std::fs::read_to_string(root.join(derived_rel(&t.dir)))
        .ok()
        .and_then(|s| FolderIndex::parse(&s).ok());
    Ok(merge(t, cache))
}

/// Write `idx`'s cache under `root` when its text changed. Returns whether
/// it wrote. Never touches git-tracked files.
pub fn write_cache(root: &Path, idx: &FolderIndex) -> Result<bool, String> {
    let path = root.join(derived_rel(&idx.dir));
    let text = derived_text(idx).map_err(|e| e.to_string())?;
    if std::fs::read_to_string(&path).ok().as_deref() == Some(text.as_str()) {
        return Ok(false);
    }
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    // Ignored by git wherever it is (a `*` that also ignores itself), even
    // in a repository whose .gitignore leaves `target/` out.
    let ignore = root.join(DERIVED_DIR).join(".gitignore");
    if !ignore.is_file() {
        std::fs::write(&ignore, "*\n").map_err(|e| format!("{}: {e}", ignore.display()))?;
    }
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(true)
}

/// Write `idx` as its tracked file at `root/rel` (only when the tracked
/// text changed) and its cache. Returns whether the tracked file changed.
pub fn write(root: &Path, rel: &str, idx: &FolderIndex) -> Result<bool, String> {
    let text = tracked_text(idx).map_err(|e| e.to_string())?;
    let path = root.join(rel);
    let changed = std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str());
    if changed {
        std::fs::write(&path, &text).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    write_cache(root, idx)?;
    Ok(changed)
}

/// After a review is written to `<dir>/review.md` (text `review_md`),
/// bring the folder's `[[review]]` cache in line with it and rewrite the
/// tracked `kovan.toml`, so the stamped function's `reached_by` is in git
/// in the same commit as the stamp (module doc: the rung reads it there).
/// The list is built as [`super::folders`] builds it. Returns whether the
/// tracked file changed; `Ok(false)` when the folder has no code index.
pub fn sync_reviews(root: &Path, dir: &str, review_md: &str) -> Result<bool, String> {
    let rel = if dir.is_empty() { "kovan.toml".to_string() } else { format!("{dir}/kovan.toml") };
    let Ok(text) = std::fs::read_to_string(root.join(&rel)) else {
        return Ok(false);
    };
    let Ok(mut fi) = load(root, &text) else {
        return Ok(false);
    };
    if fi.dir != dir {
        return Ok(false);
    }
    fi.reviews = crate::review::review_md::parse_review_md(review_md)
        .reviews()
        .map(|r| crate::review::index::CachedReview {
            function: r.function_id(),
            by: r.review.by.clone(),
            artifact: r.kovan.id.clone(),
        })
        .collect();
    write(root, &rel, &fi)
}

#[cfg(test)]
#[path = "derived_tests.rs"]
mod tests;
