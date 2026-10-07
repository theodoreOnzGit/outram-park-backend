//! **Stable function ids** for `kovan.toml` (#764 Q1, #767): recovered from
//! `review.md` and the previous `kovan.toml`, minted otherwise with
//! [`crate::review::id::mint_fn_id`].
//!
//! The id is the join key between a folder's `kovan.toml` and its
//! `review.md` (`[kovan] target = "fn:…"`), and a review's
//! `[review.callees]` names its callees by id too, so an unreviewed
//! callee's id must also survive a regeneration. The rule, in this order
//! (each id is given to at most one function, each function gets one id):
//!
//! 1. **Claims by path.** Every `review.md` entry that names a function
//!    (`review`, `needs_fix`, `annotation`, and an unreadable one whose id
//!    can still be read) is a [`Claim`]: its id
//!    ([`crate::review::review_md::ReviewEntry::function_id`]), its `path`
//!    and its hash. A `review.md` speaks only for **its own folder**: an
//!    entry whose path is in another folder is ignored, with its callees (a
//!    copied fixture must not capture the functions it names). Each claimed
//!    id, in sorted order, takes the one function its path names today
//!    (`file::qual`, or one of the `file::qual#k` twins, picked by an `@L`
//!    line when the path carries one).
//! 2. **The previous `kovan.toml` by path** ([`Prior`]): an unclaimed
//!    function keeps the id the previous index gave the same path.
//! 3. **Claims by hash.** An id still unplaced (an entry's, or a key of a
//!    review's `[review.callees]`, which records the callee's hash) takes
//!    the one unassigned function whose code `hash` equals the claim's: the
//!    function moved or was renamed without an edit. Several candidates is
//!    ambiguous and nothing is taken (reported, never guessed). Renamed
//!    **and** edited matches nothing: the function starts from new, as #739
//!    D6 decided.
//! 4. **The previous `kovan.toml` by hash**, the same way.
//! 5. **Minted**: `mint_fn_id(path id, hash, commit)` with the commit the
//!    index is built at (its first-seen commit).
//!
//! # Rebuildable and deterministic
//!
//! Steps 1 and 3 use only `review.md`, so every id a review depends on (its
//! function's and its callees') is recovered from `review.md` alone when
//! every `kovan.toml` is lost, as long as the code is unchanged; an edited
//! callee whose cache is lost gets a new id, and the review then shows its
//! callees resolving differently (directly stale, never silently valid).
//! Steps 2 and 4 keep unreviewed functions' ids stable while the cache
//! exists. The same inputs (source, `review.md`, previous `kovan.toml`,
//! commit) give the same ids, in any input order.

use std::collections::{BTreeMap, BTreeSet};

use crate::review::id::{is_fn_id, mint_fn_id};
use crate::review::index::FolderIndex;
use crate::review::review_md::{Entry, ReviewDocument};

/// One function named by a `review.md` entry (or a review's callee map).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Claim {
    /// The join key, `fn:…`.
    pub id: String,
    /// `file::item` (absent for a callee key).
    pub path: Option<String>,
    /// An `@L` line in the path, if any.
    pub line: Option<u32>,
    /// The code hash the entry was taken at.
    pub hash: Option<String>,
    /// The `review.md` it came from.
    pub source: String,
}

fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(d, _)| d)
}

fn split_line(path: &str) -> (String, Option<u32>) {
    match path.rsplit_once("@L") {
        Some((p, n)) => match n.parse::<u32>() {
            Ok(n) if n > 0 => (p.to_string(), Some(n)),
            _ => (path.to_string(), None),
        },
        None => (path.to_string(), None),
    }
}

/// The claims in one `review.md` (workspace-relative path `review_md`).
pub fn claims_from_review_md(review_md: &str, doc: &ReviewDocument) -> Vec<Claim> {
    let dir = parent(review_md);
    let own = |path: &Option<String>| {
        path.as_deref()
            .is_none_or(|p| parent(p.split("::").next().unwrap_or("")) == dir)
    };
    let mut out = Vec::new();
    let mut push = |id: String, path: Option<String>, hash: Option<&str>| {
        let (path, line) = match path {
            Some(p) => {
                let (p, l) = split_line(&p);
                (Some(p), l)
            }
            None => (None, None),
        };
        out.push(Claim {
            id,
            path,
            line,
            hash: hash.map(str::to_string),
            source: review_md.to_string(),
        });
    };
    for e in &doc.entries {
        match &e.entry {
            Entry::Review(r) => {
                let path = r.path();
                if !own(&path) {
                    continue;
                }
                push(r.function_id(), path, Some(&r.review.hash));
                for (callee, h) in &r.review.callees {
                    if is_fn_id(callee) {
                        push(callee.clone(), None, Some(h));
                    }
                }
            }
            Entry::NeedsFix(n) => {
                let path = n.path();
                if own(&path) {
                    push(n.function_id(), path, Some(&n.needs_fix.hash));
                }
            }
            Entry::Annotation(a) => {
                let path = a.path();
                if own(&path) {
                    push(a.function_id(), path, None);
                }
            }
            _ => {}
        }
    }
    for u in &doc.unreadable {
        if let Some(f) = u.function.as_deref().filter(|f| is_fn_id(f)) {
            push(f.to_string(), None, None);
        }
    }
    out.retain(|c| !c.id.is_empty());
    out.sort();
    out.dedup();
    out
}

/// A function as the previous `kovan.toml` recorded it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Prior {
    pub id: String,
    /// `file::qual` (workspace-relative file).
    pub path_id: String,
    pub hash: String,
}

/// Every function of a previous, readable `kovan.toml`.
pub fn priors_of(idx: &FolderIndex) -> Vec<Prior> {
    idx.functions()
        .map(|(file, f)| Prior {
            id: f.id.clone(),
            path_id: format!("{}::{}", idx.file_path(file), f.qual),
            hash: f.hash.clone(),
        })
        .collect()
}

/// One function to give an id: its path id and where it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    /// The call graph's id, `file::qual[#k]`.
    pub path_id: String,
    pub lines: [u32; 2],
    pub hash: String,
}

/// A claimed id that found its function by hash, somewhere else.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Moved {
    pub id: String,
    /// Where `review.md` says it was (none for a callee key).
    pub from: Option<String>,
    /// Where it is now.
    pub to: String,
}

/// A claimed id that found no function.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Unmatched {
    pub id: String,
    pub path: Option<String>,
    /// `deleted, or renamed and edited`, or `ambiguous: k functions share its hash`.
    pub why: String,
}

/// The outcome of [`assign_ids`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Assignment {
    /// path id -> id, for every located function.
    pub ids: BTreeMap<String, String>,
    pub moved: Vec<Moved>,
    pub unmatched: Vec<Unmatched>,
}

fn strip_k(path_id: &str) -> &str {
    match path_id.rsplit_once('#') {
        Some((p, k)) if !k.is_empty() && k.bytes().all(|b| b.is_ascii_digit()) => p,
        _ => path_id,
    }
}

fn path_matches(fns: &[Located], p: &str, line: Option<u32>) -> Vec<usize> {
    let exact: Vec<usize> = (0..fns.len()).filter(|&i| fns[i].path_id == p).collect();
    if !exact.is_empty() {
        return exact;
    }
    let mut v: Vec<usize> = (0..fns.len())
        .filter(|&i| strip_k(&fns[i].path_id) == strip_k(p))
        .collect();
    if let Some(l) = line {
        v.retain(|&i| fns[i].lines[0] <= l && l <= fns[i].lines[1]);
    }
    v
}

/// Give every located function an id (module doc). `commit` is the commit
/// the index is built at, used only for minting.
pub fn assign_ids(fns: &[Located], claims: &[Claim], priors: &[Prior], commit: &str) -> Assignment {
    let mut by_id: BTreeMap<&str, Vec<&Claim>> = BTreeMap::new();
    for c in claims {
        by_id.entry(c.id.as_str()).or_default().push(c);
    }
    let mut taken: BTreeMap<usize, String> = BTreeMap::new();
    let mut used: BTreeSet<String> = BTreeSet::new();
    // 1. Claims by path.
    for (id, cs) in &by_id {
        let mut hits: BTreeSet<usize> = BTreeSet::new();
        for c in cs {
            if let Some(p) = c.path.as_deref() {
                if let [one] = path_matches(fns, p, c.line).as_slice() {
                    hits.insert(*one);
                }
            }
        }
        if hits.len() == 1 {
            let i = hits.into_iter().next().unwrap_or_default();
            if let std::collections::btree_map::Entry::Vacant(v) = taken.entry(i) {
                v.insert(id.to_string());
                used.insert(id.to_string());
            }
        }
    }
    let claimed: BTreeSet<&str> = by_id.keys().copied().collect();
    let mut priors: Vec<&Prior> = priors.iter().collect();
    priors.sort();
    // 2. The previous kovan.toml by path.
    for p in &priors {
        if used.contains(&p.id) || claimed.contains(p.id.as_str()) {
            continue;
        }
        if let Some(i) = (0..fns.len()).find(|&i| fns[i].path_id == p.path_id) {
            if let std::collections::btree_map::Entry::Vacant(v) = taken.entry(i) {
                v.insert(p.id.clone());
                used.insert(p.id.clone());
            }
        }
    }
    // 3. Claims by hash.
    let mut out = Assignment::default();
    for (id, cs) in &by_id {
        if used.contains(*id) {
            continue;
        }
        let hashes: BTreeSet<&str> = cs.iter().filter_map(|c| c.hash.as_deref()).collect();
        let from = cs.iter().find_map(|c| c.path.clone());
        let cands: Vec<usize> = (0..fns.len())
            .filter(|i| !taken.contains_key(i) && hashes.contains(fns[*i].hash.as_str()))
            .collect();
        match cands.as_slice() {
            [i] => {
                taken.insert(*i, id.to_string());
                used.insert(id.to_string());
                out.moved.push(Moved {
                    id: id.to_string(),
                    from,
                    to: fns[*i].path_id.clone(),
                });
            }
            [] => out.unmatched.push(Unmatched {
                id: id.to_string(),
                path: from,
                why: "deleted, or renamed and edited".into(),
            }),
            many => out.unmatched.push(Unmatched {
                id: id.to_string(),
                path: from,
                why: format!("ambiguous: {} functions share its hash", many.len()),
            }),
        }
    }
    // 4. The previous kovan.toml by hash.
    for p in &priors {
        if used.contains(&p.id) || claimed.contains(p.id.as_str()) {
            continue;
        }
        let cands: Vec<usize> = (0..fns.len())
            .filter(|i| !taken.contains_key(i) && fns[*i].hash == p.hash)
            .collect();
        if let [i] = cands.as_slice() {
            taken.insert(*i, p.id.clone());
            used.insert(p.id.clone());
        }
    }
    // 5. Minted.
    for (i, f) in fns.iter().enumerate() {
        let id = match taken.get(&i) {
            Some(id) => id.clone(),
            None => mint_fn_id(&f.path_id, &f.hash, commit),
        };
        out.ids.insert(f.path_id.clone(), id);
    }
    out.moved.sort();
    out.unmatched.sort();
    out
}

/// The id of a function outside the indexed scope (a cross-crate callee of
/// a scoped run): the one claim or prior whose path names it exactly, else
/// `None` (the callee is then left out of `callees`, and the run reports
/// it). A whole-workspace run has no outside functions.
pub fn id_outside(path_id: &str, claims: &[Claim], priors: &[Prior]) -> Option<String> {
    let ids: BTreeSet<&str> = claims
        .iter()
        .filter(|c| c.path.as_deref() == Some(path_id))
        .map(|c| c.id.as_str())
        .chain(priors.iter().filter(|p| p.path_id == path_id).map(|p| p.id.as_str()))
        .collect();
    match ids.len() {
        1 => ids.into_iter().next().map(str::to_string),
        _ => None,
    }
}

#[cfg(test)]
#[path = "ids_tests.rs"]
mod tests;
