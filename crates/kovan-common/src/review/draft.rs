//! **Drafting an unsigned stamp** for one function (GitHub #762, #770,
//! #765): the pure part of "stamp this function" in desktop kovan's review
//! dialog. The caller gathers the facts (the function's `kovan.toml` entry,
//! `HEAD`, the `Cargo.lock` hash, its callees' hashes, the wizard answers,
//! git's view of who wrote the reaching tests) and passes them in as plain
//! data, so this builds for wasm; the gathering is
//! `kovan::stamping` (native).
//!
//! ```text
//!   kovan.toml FunctionIndex ─┐
//!   HEAD, Cargo.lock hash ────┤
//!   callee id -> hash ────────┼─[draft_review]─> ReviewEntry (unsigned)
//!   wizard answers ───────────┤        │
//!   TestAuthorship (git) ─────┘        └─> UnlockedKey::sign_review ─> review.md
//! ```
//!
//! # What the draft is checked against
//!
//! The draft is made so that the staleness engine
//! ([`super::engine::evaluate`]) judges a freshly committed, signed copy
//! **valid**: every field the engine compares is taken from the same source
//! the engine reads.
//!
//! | field | from | engine rule |
//! |---|---|---|
//! | `[kovan] target` | the function's `fn:` id in `kovan.toml` | 1 (find) |
//! | `path` | its file and `qual` | 5 (location) |
//! | `rung` | the wizard gate with git's [`TestAuthorship`] ([`super::wizard::stamp_gate`]) | 3b (derived rung) |
//! | `commit` | `HEAD` | 2 (hash at the certified commit) |
//! | `hash`, `doc_hash` | the `kovan.toml` entry, **not** a fresh hash of the source: the engine compares the index's hash (rule 4), and the caller checks the index against the source at `HEAD` first | 4, 7 |
//! | `callees` | every id in the entry's `callees`, with that callee's index hash now | 4 (same set), 6 (same hashes) |
//! | `cargo_lock` | `Cargo.lock` now | 8 |
//! | `checklist` | the wizard answers, gate re-run here | 3b (gate re-run on read) |
//!
//! A draft is refused ([`DraftError`]) when the engine would refuse it: the
//! gate blocks, a callee has no hash, a field is malformed, or the
//! function's index entry is out of date (its callees could not be
//! recomputed without rust-analyzer, so the recorded set would be a guess:
//! Leak Before Break).
//!
//! **Conservative choice** (not settled by #764/#769): the gate is run with
//! `physical_interface` true when the index marks the function as one
//! **or** the reviewer answered `units_documented`; the engine re-runs it
//! with the second condition only, so the draft's gate is never weaker.
//!
//! # AI agents never stamp
//!
//! A draft is unsigned and records nothing until a human unlocks a key and
//! signs it (`UnlockedKey::sign_review`). Nothing here signs or writes.

use std::collections::BTreeMap;

use crate::artifact::relation::RelationRecord;

use super::id::is_fn_id;
use super::index::FunctionIndex;
use super::review_md::{
    validate_review, EntryMeta, FixStatus, NeedsFixBody, NeedsFixEntry, ReviewBody, ReviewEntry,
};
use super::types::{check_commit, check_hash, check_text, ChangeAuthorship, FieldError};
use super::wizard::{stamp_gate, Applicability, GateReason, TestAuthorship};

/// Everything [`draft_review`] needs, gathered by the caller (owned: no
/// borrowed struct, workspace Rust rules).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftInput {
    /// The function's entry in its folder's `kovan.toml`.
    pub function: FunctionIndex,
    /// The function's file, workspace-relative (`FolderIndex::file_path`).
    pub file: String,
    /// Callee id -> its index hash now; must hold every id in
    /// `function.callees`.
    pub callee_hashes: BTreeMap<String, String>,
    /// The reviewer id (`github:…`); must be the signing key's reviewer.
    pub by: String,
    /// `YYYY-MM-DD`, the local date of signing.
    pub date: String,
    /// RFC 3339 time for `[kovan] created`/`modified`.
    pub now: String,
    /// `HEAD`: the commit the review certifies.
    pub commit: String,
    /// `sha256:` of `Cargo.lock`, when the workspace has one.
    pub cargo_lock: Option<String>,
    /// Wizard answers by question key.
    pub checklist: BTreeMap<String, String>,
    /// Git's view of who wrote the tests reaching the function at `commit`.
    pub tests: TestAuthorship,
    /// The folder's `review.md` upstream says it is a port.
    pub is_port: bool,
    /// Who authored the change reviewed (#764 from #771).
    pub authorship: Option<ChangeAuthorship>,
    /// The "no concept" reason, when the function links no concept.
    pub no_concept: Option<String>,
    /// `implements` / `part_of` relations.
    pub relations: Vec<RelationRecord>,
    /// This reviewer's earlier entry for the function, when re-stamping:
    /// its `[kovan] id` and `created` are kept, `modified` is `now`.
    pub previous: Option<EntryMeta>,
}

/// Why a draft is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DraftError {
    /// The function's `kovan.toml` entry is marked out of date: run
    /// `kovan-cli index` first.
    IndexOutOfDate { path: String },
    /// The entry's id is not an `fn:` id (a first-version index).
    NotAFunctionId(String),
    /// A callee of the function has no hash in the input.
    MissingCalleeHash(String),
    /// The wizard gate blocks the answers.
    Gate(Vec<GateReason>),
    /// A field is malformed.
    Field(FieldError),
}

impl std::fmt::Display for DraftError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IndexOutOfDate { path } => write!(
                f,
                "{path}: its kovan.toml entry is out of date (callees unknown); run kovan-cli index first"
            ),
            Self::NotAFunctionId(id) => write!(f, "{id} is not an fn: id; run kovan-cli index first"),
            Self::MissingCalleeHash(c) => write!(f, "callee {c} has no hash in any kovan.toml"),
            Self::Gate(r) => write!(f, "the review wizard blocks this stamp: {r:?}"),
            Self::Field(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for DraftError {}

/// Lower-case ASCII letters and digits of `s`, runs of anything else as one
/// `-`, trimmed; for entry ids.
fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

/// The reviewer id without its `github:`/`gitlab:`/`orcid:` prefix.
fn short_by(by: &str) -> &str {
    by.split_once(':').map_or(by, |(_, r)| r)
}

/// The `[kovan] id` of a new review entry: `review-<name>-<reviewer>-<first
/// 8 hex of the fn id>`, unique per (function, reviewer) in a folder.
pub fn review_entry_id(name: &str, by: &str, fn_id: &str) -> String {
    let hex = fn_id.strip_prefix(super::id::FN_ID_PREFIX).unwrap_or(fn_id);
    format!(
        "review-{}-{}-{}",
        slug(name),
        slug(short_by(by)),
        &hex[..hex.len().min(8)]
    )
}

/// The `#` heading of a review entry, as `review.md`'s module example
/// writes it: `Review: <qual> (<reviewer>)`.
pub fn review_heading(qual: &str, by: &str) -> String {
    format!("Review: {qual} ({by})")
}

/// The `#` heading of a needs-fix entry: `Needs fix: <qual> (<reviewer>)`.
pub fn needs_fix_heading(qual: &str, by: &str) -> String {
    format!("Needs fix: {qual} ({by})")
}

/// Draft the unsigned review entry (module doc).
pub fn draft_review(input: &DraftInput) -> Result<ReviewEntry, DraftError> {
    let f = &input.function;
    let path = format!("{}::{}", input.file, f.qual);
    if !is_fn_id(&f.id) {
        return Err(DraftError::NotAFunctionId(f.id.clone()));
    }
    if f.index_out_of_date {
        return Err(DraftError::IndexOutOfDate { path });
    }
    let mut callees = BTreeMap::new();
    for c in &f.callees {
        let h = input
            .callee_hashes
            .get(c)
            .ok_or_else(|| DraftError::MissingCalleeHash(c.clone()))?;
        check_hash("review.callees", h).map_err(DraftError::Field)?;
        callees.insert(c.clone(), h.clone());
    }
    check_commit("review.commit", &input.commit).map_err(DraftError::Field)?;
    let ctx = Applicability {
        is_port: input.is_port,
        physical_interface: f.physical_interface
            || input.checklist.contains_key("units_documented"),
        tests: input.tests,
    };
    let gate = stamp_gate(&input.checklist, ctx);
    if !gate.stampable() {
        return Err(DraftError::Gate(gate.blocked_by));
    }
    let (id, created) = match &input.previous {
        Some(p) => (p.id.clone(), p.created.clone()),
        None => (
            review_entry_id(&f.name, &input.by, &f.id),
            input.now.clone(),
        ),
    };
    let entry = ReviewEntry {
        kovan: EntryMeta {
            id,
            kind: "review".into(),
            origin: Some("human".into()),
            created,
            modified: input.now.clone(),
            target: Some(f.id.clone()),
        },
        review: ReviewBody {
            function: None,
            path: Some(path),
            by: input.by.clone(),
            rung: gate.rung.as_u8(),
            date: input.date.clone(),
            signed_at: None,
            commit: input.commit.clone(),
            hash: f.hash.clone(),
            doc_hash: f.doc_hash.clone(),
            cargo_lock: input.cargo_lock.clone(),
            callees,
            checklist: input.checklist.clone(),
            no_concept: input.no_concept.clone(),
            authorship: input.authorship.clone(),
            moved: Vec::new(),
            separation_attestation: None,
            signature: None,
        },
        relations: input.relations.clone(),
    };
    validate_review(&entry).map_err(DraftError::Field)?;
    Ok(entry)
}

/// Draft an open needs-fix entry against the function as it is at `commit`
/// (its index hash): an edit after it turns "needs fix" into "fixed". The
/// id carries the digits of `now`, so one reviewer may raise several.
#[allow(clippy::too_many_arguments)]
pub fn draft_needs_fix(
    function: &FunctionIndex,
    file: &str,
    by: &str,
    date: &str,
    now: &str,
    commit: &str,
    note: &str,
) -> Result<NeedsFixEntry, DraftError> {
    if !is_fn_id(&function.id) {
        return Err(DraftError::NotAFunctionId(function.id.clone()));
    }
    check_commit("needs_fix.commit", commit).map_err(DraftError::Field)?;
    check_text("needs_fix.note", note).map_err(DraftError::Field)?;
    let stamp: String = now.chars().filter(char::is_ascii_digit).collect();
    let base = review_entry_id(&function.name, by, &function.id);
    Ok(NeedsFixEntry {
        kovan: EntryMeta {
            id: format!("needs-fix{}-{stamp}", base.trim_start_matches("review")),
            kind: "needs_fix".into(),
            origin: Some("human".into()),
            created: now.into(),
            modified: now.into(),
            target: Some(function.id.clone()),
        },
        needs_fix: NeedsFixBody {
            function: None,
            path: Some(format!("{file}::{}", function.qual)),
            by: by.into(),
            date: date.into(),
            commit: commit.into(),
            hash: function.hash.clone(),
            note: note.into(),
            status: FixStatus::Open,
            resolved_by: None,
            highlights: Vec::new(),
        },
    })
}

#[cfg(test)]
#[path = "draft_tests.rs"]
pub(crate) mod tests;
