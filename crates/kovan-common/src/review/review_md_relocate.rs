//! **Moving and retiring a function's `review.md` entries** (GitHub #771,
//! #740 U5, #739 decision 14): the pure text side of the need-you queue's
//! *acknowledge move* and *record deletion*. Text in, text out; no
//! filesystem, no git.
//!
//! # Acknowledging a move
//!
//! A function found at a new place with the same hash and callees keeps
//! its review (#739 D14: "the review should go with it"). Acknowledging
//! the move ([`acknowledge_move`]) rewrites **every** entry that names the
//! function (each reviewer's review, needs-fix notes, annotations):
//!
//! - `path` becomes the new location;
//! - a review gains a machine-written `[[review.moved]]` record (`from`,
//!   `to`, the commit the move was acknowledged at);
//! - when the new location is in **another folder**, the entries leave the
//!   old folder's `review.md` and are appended to the new folder's (a
//!   `review.md` speaks only for its own folder:
//!   [`crate::code_index::ids`]), the old file keeping every other byte.
//!
//! `path` and `moved` are location metadata and are **not signed**
//! ([`super::signing::signed_bytes`] covers the target id, never the
//! path), so a moved review still verifies.
//!
//! # Recording a deletion
//!
//! "Deleted functions are removed from `review.md`; their review lives on
//! only in git history" (#740 U5). [`record_deletion`] removes every entry
//! of the function and adds the engine's history row
//! ([`super::engine::HistoryRow`]) to the folder's `deleted_functions`
//! table, creating it when absent.
//!
//! # Splicing, as the stamp dialog's writer does
//!
//! Like [`super::review_md_write`], these functions splice: only the spans
//! of the entries concerned change, unreadable entries are kept byte for
//! byte, and the result is parsed again. A result that does not read back
//! (the moved entries equal to what was intended, the unreadable count
//! unchanged) is an error and nothing is returned.

use crate::artifact::{heading_span, render_block, ARTIFACT_LEVEL};

use super::draft::{needs_fix_heading, review_heading};
use super::review_md::{
    parse_review_md, DeletedFunction, DeletedFunctionsEntry, Entry, EntryMeta, MoveRecord,
    ParsedEntry,
};
use super::review_md_write::splice;

/// Why entries could not be moved or retired.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelocateError {
    /// No readable entry names the function.
    NotFound(String),
    /// TOML serialisation failed.
    Emit(String),
    /// The result would not read back as intended; nothing is returned.
    NotReadBack(String),
}

impl std::fmt::Display for RelocateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "no readable review.md entry names {id}"),
            Self::Emit(e) => write!(f, "{e}"),
            Self::NotReadBack(m) => {
                write!(
                    f,
                    "review.md would not read back after the change: {m}; nothing written"
                )
            }
        }
    }
}

impl std::error::Error for RelocateError {}

/// The new texts of the two files a move touches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relocated {
    /// The old folder's `review.md`.
    pub from_text: String,
    /// The new folder's `review.md`; `None` when the move stays in the
    /// folder (everything is in `from_text`).
    pub to_text: Option<String>,
    /// How many entries were moved.
    pub entries: usize,
}

/// The function-level entries naming `function_id`, in file order.
fn entries_of<'d>(entries: &'d [ParsedEntry], function_id: &str) -> Vec<&'d ParsedEntry> {
    entries
        .iter()
        .filter(|e| match &e.entry {
            Entry::Review(r) => r.function_id() == function_id,
            Entry::NeedsFix(n) => n.function_id() == function_id,
            Entry::Annotation(a) => a.function_id() == function_id,
            _ => false,
        })
        .collect()
}

/// The item part of `file.rs::item`.
fn qual_of(path: &str) -> &str {
    path.split_once(".rs::").map_or(path, |(_, q)| q)
}

/// `e` with its location set to `to` (a review also records the move).
fn moved_entry(e: &ParsedEntry, to: &str, commit: &str) -> (String, Entry) {
    match &e.entry {
        Entry::Review(r) => {
            let mut r = r.clone();
            let from = r.path().unwrap_or_default();
            if from != to {
                r.review.moved.push(MoveRecord {
                    from,
                    to: Some(to.to_string()),
                    commit: commit.to_string(),
                });
            }
            r.review.path = Some(to.to_string());
            (review_heading(qual_of(to), &r.review.by), Entry::Review(r))
        }
        Entry::NeedsFix(n) => {
            let mut n = n.clone();
            n.needs_fix.path = Some(to.to_string());
            (
                needs_fix_heading(qual_of(to), &n.needs_fix.by),
                Entry::NeedsFix(n),
            )
        }
        Entry::Annotation(a) => {
            let mut a = a.clone();
            a.annotation.path = Some(to.to_string());
            (e.heading.clone(), Entry::Annotation(a))
        }
        other => (e.heading.clone(), other.clone()),
    }
}

fn block(heading: &str, entry: &Entry, body: &str) -> Result<String, RelocateError> {
    let toml = entry
        .to_toml()
        .map_err(|e| RelocateError::Emit(e.to_string()))?;
    Ok(render_block(ARTIFACT_LEVEL, heading, &toml, body))
}

/// `md` without the entry whose heading is on 1-based `line`.
fn remove(md: &str, line: usize) -> String {
    let span = heading_span(md, line, ARTIFACT_LEVEL);
    let lines: Vec<&str> = md.split_inclusive('\n').collect();
    let mut out: String = lines[..span.start.min(lines.len())].concat();
    out.push_str(&lines[span.end.min(lines.len())..].concat());
    if out.trim().is_empty() {
        String::new()
    } else {
        out
    }
}

/// Check that `text` holds each of `want` as a readable entry and has
/// `unreadable` unreadable entries.
fn reads_back(text: &str, want: &[Entry], unreadable: usize) -> Result<(), RelocateError> {
    let doc = parse_review_md(text);
    if doc.unreadable.len() != unreadable {
        return Err(RelocateError::NotReadBack(
            doc.unreadable
                .last()
                .map(|u| u.message.clone())
                .unwrap_or_else(|| "an unreadable entry was lost".into()),
        ));
    }
    for w in want {
        if !doc.entries.iter().any(|e| &e.entry == w) {
            return Err(RelocateError::NotReadBack(format!(
                "a moved {} entry is missing or differs after parsing",
                w.kind()
            )));
        }
    }
    Ok(())
}

/// Acknowledge that `function_id` moved to `to_path` (`file.rs::item`), at
/// `commit` (module doc). `from_md` is the `review.md` holding its entries;
/// `to_md` is the new folder's `review.md` text (`Some("")` when it does
/// not exist yet) or `None` when the new location is in the same folder.
pub fn acknowledge_move(
    from_md: &str,
    to_md: Option<&str>,
    function_id: &str,
    to_path: &str,
    commit: &str,
) -> Result<Relocated, RelocateError> {
    let before = parse_review_md(from_md);
    let mine = entries_of(&before.entries, function_id);
    if mine.is_empty() {
        return Err(RelocateError::NotFound(function_id.to_string()));
    }
    let mut blocks = Vec::new();
    let mut want = Vec::new();
    for e in &mine {
        let (heading, entry) = moved_entry(e, to_path, commit);
        blocks.push((e.line, block(&heading, &entry, &e.body)?));
        want.push(entry);
    }
    let n = blocks.len();
    match to_md {
        None => {
            let mut text = from_md.to_string();
            // Last first, so earlier headings keep their line numbers.
            for (line, b) in blocks.iter().rev() {
                text = splice(&text, Some(*line), b);
            }
            reads_back(&text, &want, before.unreadable.len())?;
            Ok(Relocated {
                from_text: text,
                to_text: None,
                entries: n,
            })
        }
        Some(to) => {
            let to_before = parse_review_md(to).unreadable.len();
            let mut from = from_md.to_string();
            for (line, _) in blocks.iter().rev() {
                from = remove(&from, *line);
            }
            let mut to_text = to.to_string();
            for (_, b) in &blocks {
                to_text = splice(&to_text, None, b);
            }
            reads_back(&to_text, &want, to_before)?;
            reads_back(&from, &[], before.unreadable.len())?;
            if !entries_of(&parse_review_md(&from).entries, function_id).is_empty() {
                return Err(RelocateError::NotReadBack(
                    "an entry of the function is still in the old review.md".into(),
                ));
            }
            Ok(Relocated {
                from_text: from,
                to_text: Some(to_text),
                entries: n,
            })
        }
    }
}

/// The heading of the `deleted_functions` entry this module creates.
pub const DELETED_HEADING: &str = "Deleted functions";

/// Remove every entry of `function_id` from `md` and add `row` to its
/// `deleted_functions` table (module doc). `now` is an RFC 3339 time for a
/// new table's `created`/`modified`. A row for the same function and last
/// review commit already in the table is not added twice.
pub fn record_deletion(
    md: &str,
    function_id: &str,
    row: &DeletedFunction,
    now: &str,
) -> Result<String, RelocateError> {
    let before = parse_review_md(md);
    let mine: Vec<usize> = entries_of(&before.entries, function_id)
        .iter()
        .map(|e| e.line)
        .collect();
    if mine.is_empty() {
        return Err(RelocateError::NotFound(function_id.to_string()));
    }
    let mut text = md.to_string();
    for line in mine.iter().rev() {
        text = remove(&text, *line);
    }
    let doc = parse_review_md(&text);
    let table = doc.entries.iter().find_map(|e| match &e.entry {
        Entry::DeletedFunctions(d) => Some((e, d.clone())),
        _ => None,
    });
    let (line, heading, body, mut entry) = match table {
        Some((e, d)) => (Some(e.line), e.heading.clone(), e.body.clone(), d),
        None => (
            None,
            DELETED_HEADING.to_string(),
            "Functions deleted from this folder. Their reviews live on in git history: \
             `git show <last_review_commit>:<folder>/review.md`."
                .to_string(),
            DeletedFunctionsEntry {
                kovan: EntryMeta {
                    id: "deleted-functions".into(),
                    kind: "deleted_functions".into(),
                    origin: None,
                    created: now.to_string(),
                    modified: now.to_string(),
                    target: None,
                },
                deleted: Vec::new(),
            },
        ),
    };
    let dup = entry
        .deleted
        .iter()
        .any(|d| d.function == row.function && d.last_review_commit == row.last_review_commit);
    if !dup {
        entry.deleted.push(row.clone());
        entry.kovan.modified = now.to_string();
    }
    let want = Entry::DeletedFunctions(entry);
    let text = splice(&text, line, &block(&heading, &want, &body)?);
    reads_back(&text, &[want], before.unreadable.len())?;
    Ok(text)
}

#[cfg(test)]
#[path = "review_md_relocate_tests.rs"]
mod tests;
