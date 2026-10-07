//! The **rust-analyzer-free refresh** of a folder's `kovan.toml`
//! (maintainer requirement on #767, 2026-10-07: "locally, kovan must NOT
//! rely on rust-analyzer").
//!
//! The committed index (every `kovan.toml` plus each crate's link index) is
//! enough for desktop kovan and `kovan-cli` to work without rust-analyzer;
//! rust-analyzer is needed only to **regenerate** it (`kovan-cli index`, or
//! CI). Between regenerations, edited files are refreshed here with the
//! pure-Rust `syn` hasher only:
//!
//! - every function's `lines`, `hash` and `doc_hash` are recomputed from the
//!   current source, and its id is kept by the same rule as a full run
//!   ([`super::ids`], the previous `kovan.toml` serving as the priors);
//! - a function whose code is **unchanged** (same id, same hash as the
//!   previous index) keeps its committed `callees` and `reached_by`, and
//!   its previous `index_out_of_date` flag;
//! - a function that is **new or edited** cannot have its callees or
//!   reaching tests recomputed without rust-analyzer: it keeps the previous
//!   lists (an edited one) or none (a new one) and is marked
//!   `index_out_of_date = true`, shown as "index out of date: run kovan-cli
//!   index", never silently trusted (Leak Before Break);
//! - a file that no longer parses keeps its previous entries, every one
//!   marked out of date;
//! - `[test_run]`, `crate_root` and `[[deleted_folder]]` are kept;
//!   `[upstream]` and `[[review]]` are re-read from `review.md` when the
//!   caller passes it.
//!
//! A new file's module key is unknown without the module tree (the call
//! graph): it is written as `?` until the next full index.
//!
//! The link index is not rewritten by a refresh: each file in it carries
//! the hash of the text it was built from ([`super::links`]), so a reader
//! sees which files' links are out of date.

use std::collections::BTreeMap;

use crate::call_graph::function_ids;
use crate::review::hash::hash_functions;
use crate::review::index::{CachedReview, FolderIndex, FunctionIndex, ItemKind, ModuleIndex};
use crate::review::review_md::ReviewDocument;

use super::ids::{assign_ids, claims_from_review_md, priors_of, Claim, Located};

/// What a refresh did to one folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RefreshReport {
    /// `file::qual` of every function marked out of date.
    pub out_of_date: Vec<String>,
    /// Files that did not parse (kept from the previous index).
    pub unparsed: Vec<String>,
}

/// Refresh folder `dir` of crate `krate`. `files` maps every `.rs` file name
/// in the folder to its text; `previous` is the readable committed index,
/// if any; `review_md` the folder's parsed `review.md`, if any; `claims`
/// every claim of the run (so ids match what a full run gives); `commit`
/// is used only to mint new ids.
pub fn refresh_folder(
    krate: &str,
    dir: &str,
    previous: Option<&FolderIndex>,
    files: &BTreeMap<String, String>,
    review_md: Option<&ReviewDocument>,
    claims: &[Claim],
    commit: &str,
) -> (FolderIndex, RefreshReport) {
    let mut report = RefreshReport::default();
    let mut idx = match previous {
        Some(p) => {
            let mut c = p.clone();
            c.modules.clear();
            c
        }
        None => FolderIndex::new(krate, dir),
    };
    let prev_by_id: BTreeMap<&str, &FunctionIndex> = previous
        .map(|p| p.functions().map(|(_, f)| (f.id.as_str(), f)).collect())
        .unwrap_or_default();
    let priors = previous.map(priors_of).unwrap_or_default();
    let mut parsed = Vec::new();
    let mut located = Vec::new();
    for (file, text) in files {
        let path = idx.file_path(file);
        match hash_functions(text) {
            Ok(fns) => {
                let quals: Vec<String> = fns.iter().map(|h| h.entry.qualname()).collect();
                let ids = function_ids(&path, &quals);
                for (h, (pid, _)) in fns.iter().zip(ids) {
                    located.push(Located {
                        path_id: pid.clone(),
                        lines: h.entry.lines,
                        hash: h.hashes.hash.clone(),
                    });
                    parsed.push((file.clone(), pid, h.clone()));
                }
                let path_key = previous
                    .and_then(|p| p.modules.get(file))
                    .map_or_else(|| "?".to_string(), |m| m.path.clone());
                idx.modules.insert(
                    file.clone(),
                    ModuleIndex {
                        path: path_key,
                        functions: Vec::new(),
                    },
                );
            }
            Err(_) => {
                report.unparsed.push(path.clone());
                if let Some(m) = previous.and_then(|p| p.modules.get(file)) {
                    let mut m = m.clone();
                    for f in &mut m.functions {
                        f.index_out_of_date = true;
                        report.out_of_date.push(format!("{path}::{}", f.qual));
                    }
                    idx.modules.insert(file.clone(), m);
                }
            }
        }
    }
    let a = assign_ids(&located, claims, &priors, commit);
    // Aliases from this folder only: a refresh does not read the workspace,
    // and an unchanged function keeps the full run's answer.
    let mut quantities = super::physical::QuantityNames::default();
    quantities.learn(files.values().map(String::as_str));
    for (file, pid, h) in parsed {
        let id = a.ids[&pid].clone();
        let prev = prev_by_id.get(id.as_str()).copied();
        let same = prev.is_some_and(|p| p.hash == h.hashes.hash);
        let qual = pid
            .strip_prefix(&format!("{}::", idx.file_path(&file)))
            .unwrap_or(&pid)
            .to_string();
        let out_of_date = if same {
            prev.is_some_and(|p| p.index_out_of_date)
        } else {
            true
        };
        if out_of_date {
            report.out_of_date.push(pid.clone());
        }
        let physical_interface = match prev {
            Some(p) if same => p.physical_interface,
            _ => quantities.is_physical(&h.entry.code),
        };
        let f = FunctionIndex {
            physical_interface,
            id,
            name: h.entry.name.clone(),
            qual,
            item: ItemKind::Fn,
            lines: h.entry.lines,
            hash: h.hashes.hash.clone(),
            doc_hash: h.hashes.doc_hash.clone(),
            callees: prev.map(|p| p.callees.clone()).unwrap_or_default(),
            reached_by: prev.map(|p| p.reached_by.clone()).unwrap_or_default(),
            test: h.entry.is_test,
            index_out_of_date: out_of_date,
        };
        if let Some(m) = idx.modules.get_mut(&file) {
            m.functions.push(f);
        }
    }
    if let Some(rd) = review_md {
        idx.upstream = rd.upstream().cloned();
        idx.reviews = rd
            .reviews()
            .map(|r| CachedReview {
                function: r.function_id(),
                by: r.review.by.clone(),
                artifact: r.kovan.id.clone(),
            })
            .collect();
    }
    idx.normalise();
    report.out_of_date.sort();
    (idx, report)
}

/// [`claims_from_review_md`] over every `review.md` of a run.
pub fn all_claims(reviews: &BTreeMap<String, ReviewDocument>) -> Vec<Claim> {
    reviews
        .iter()
        .flat_map(|(p, d)| claims_from_review_md(p, d))
        .collect()
}

#[cfg(test)]
#[path = "refresh_tests.rs"]
mod tests;
