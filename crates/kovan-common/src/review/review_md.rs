//! **`review.md`**: the human-owned review record of one folder (maintainer,
//! #739 decisions 6, 7, 10 and later comments, 2026-10-07), in the shared
//! kovan artifact format (#743): every entry is a `#` heading followed by a
//! ```` ```toml ```` block holding `[kovan]`, read with
//! [`crate::artifact::scan_blocks`].
//!
//! # Entry kinds (`[kovan] kind`)
//!
//! | kind | table | what it is |
//! |---|---|---|
//! | `review` | `[review]` | one reviewer's standing review of one function; at most one per (function, reviewer) |
//! | `needs_fix` | `[needs_fix]` | an open or resolved concern; an open one blocks the function whatever its stamps |
//! | `annotation` | `[annotation]` | a highlight in the function's source, anchored with Hypothesis selectors (#754) |
//! | `upstream` | `[upstream]` | the folder-level upstream confirmation: is it a port, of what, at which commit |
//! | `deleted_functions` | `[[deleted]]` | the deleted-functions history table (#740 U5) |
//! | `architecture` | `[architecture]` | an architecture node, in a crate-level `review.md`: its member functions, the upstream it follows (commit-pinned) and the generic pattern it is (maintainer, #764, 2026-10-07); signed like a review. Members' reviews point at it with a `part_of` relation |
//!
//! Any other kind (a `note`, say) is kept as [`Entry::Other`] and ignored by
//! review. A review's `[kovan]` table also has `target`, the `code:` target
//! of the function in the #743 form (display and navigation); the join key
//! is `[review] function`, the stable id in the folder's `kovan.toml`
//! ("the `name` is for display only").
//!
//! ````markdown
//! # Review: SteamTable::flash (github:theodoreOnzGit)
//!
//! ```toml
//! [kovan]
//! id = "review-flash-theodoreonzgit"
//! kind = "review"
//! origin = "human"
//! created = "2026-10-07T10:00:00+08:00"
//! modified = "2026-10-07T10:00:00+08:00"
//! target = "code:crates/tampines/src/steam.rs::SteamTable::flash"
//!
//! [review]
//! function = "crates/tampines/src/steam.rs::SteamTable::flash"
//! by = "github:theodoreOnzGit"
//! rung = 3
//! date = "2026-10-07"
//! commit = "<40 hex>"
//! hash = "sha256:<64 hex>"
//! doc_hash = "sha256:<64 hex>"
//! cargo_lock = "sha256:<64 hex>"
//!
//! [review.callees]
//! "crates/tampines/src/steam.rs::saturation" = "sha256:<64 hex>"
//!
//! [review.checklist]
//! q1 = "yes"
//! q8 = "reference_code_to_code"
//!
//! [review.authorship]
//! kind = "agent"
//! sessions = ["https://claude.ai/code/session_…"]
//!
//! [[relation]]
//! target = "artifact:iapws-if97#eq-7"
//! kind = "implements"
//! ```
//!
//! ## Comments
//!
//! ## Sign-off
//! ````
//!
//! # Parsing is per entry
//!
//! A malformed entry (bad TOML, a missing field, a bad hash, an unpinned
//! upstream link, a second standing review by the same reviewer of the same
//! function) is an [`Unreadable`] entry, which counts as **no review**
//! (maintainer, 2026-10-07: "just redo"); every other entry still loads.
//! What can still be read of a malformed entry (its function and reviewer)
//! is kept so the function goes back into the queue and the wizard can
//! pre-fill.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::anchoring::selector::Selector;
use crate::call_graph::upstream::Upstream;
use crate::artifact::relation::{CodeTarget, RelationRecord};
use crate::artifact::{render_block, scan_blocks_with, UnparseableFence, ARTIFACT_LEVEL};

use super::signing::Signature;
use super::types::{
    check_commit, check_hash, check_pinned_url, check_text, reviewer_id_kind, ChangeAuthorship,
    FieldError,
};

/// The `[kovan]` table of a review entry: the shared artifact identity, plus
/// `target`. Unknown keys are tolerated (additive schema).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryMeta {
    pub id: String,
    pub kind: String,
    /// `human` for reviews ("AI never stamps"); kept as written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    pub created: String,
    pub modified: String,
    /// `code:<file>::<item>[@L<line>]` (#743 `CodeTarget`); folder-level
    /// entries have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

/// `[[review.moved]]`: a machine-written record that the review was moved
/// with its function (#739 decision 14).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveRecord {
    /// The function's previous code-walk path.
    pub from: String,
    /// The commit the move was acknowledged at.
    pub commit: String,
}

/// `[review]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewBody {
    /// The stable function id in `kovan.toml` (the join key).
    pub function: String,
    /// `github:` / `gitlab:` / `orcid:` / email.
    pub by: String,
    /// 3 human reviewed, 4 human V&V (gated on Q8). 5 is derived.
    pub rung: u8,
    /// `YYYY-MM-DD`.
    pub date: String,
    /// The commit the review certifies.
    pub commit: String,
    pub hash: String,
    pub doc_hash: String,
    /// `Cargo.lock` hash at review time (external dependency updates, #739).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cargo_lock: Option<String>,
    /// Each workspace callee's id and its hash at review time.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub callees: BTreeMap<String, String>,
    /// Wizard answers by question key (`q1` … `q10`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub checklist: BTreeMap<String, String>,
    /// The "no concept" reason, when the function links no concept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_concept: Option<String>,
    /// Who authored the change reviewed (#764, from #771, 2026-10-07).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorship: Option<ChangeAuthorship>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub moved: Vec<MoveRecord>,
    /// Ed25519 over [`super::signing::signed_bytes`]; checked by
    /// [`super::signing::verify_review`] (#762).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<Signature>,
}

/// A `review` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewEntry {
    pub kovan: EntryMeta,
    pub review: ReviewBody,
    #[serde(default, rename = "relation", skip_serializing_if = "Vec::is_empty")]
    pub relations: Vec<RelationRecord>,
}

/// Open or resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixStatus {
    Open,
    /// Closed by a re-review stamp (`resolved_by`), kept for history.
    Resolved,
}

/// `[needs_fix]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeedsFixBody {
    pub function: String,
    pub by: String,
    pub date: String,
    /// The commit and hash the concern was raised against: an edit after it
    /// turns ⛔ into ✏ ("fixed").
    pub commit: String,
    pub hash: String,
    /// One line, at least two characters (#740 U4).
    pub note: String,
    pub status: FixStatus,
    /// The review entry id that resolved it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_by: Option<String>,
    /// Ids of the annotation entries marking where.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub highlights: Vec<String>,
}

/// A `needs_fix` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeedsFixEntry {
    pub kovan: EntryMeta,
    pub needs_fix: NeedsFixBody,
}

/// `[annotation]`: a highlight in a function's source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnnotationBody {
    pub function: String,
    pub by: String,
    /// The commit whose source the selectors were taken on.
    pub commit: String,
    /// W3C/Hypothesis selectors (#754), positions relative to the
    /// function's source text.
    #[serde(default)]
    pub selector: Vec<Selector>,
    /// The needs-fix entry this highlight belongs to, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needs_fix: Option<String>,
}

/// An `annotation` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnnotationEntry {
    pub kovan: EntryMeta,
    pub annotation: AnnotationBody,
}

/// `[upstream]`: the folder's upstream confirmation (#740 wizard; cached in
/// `kovan.toml`). Every link must be pinned ([`check_pinned_url`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpstreamTable {
    /// Whether the folder ports upstream code.
    pub is_port: bool,
    /// Repository URL (a port).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    /// The upstream commit ported from (a port: required).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// Our file name -> the upstream file (path or pinned URL).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub files: BTreeMap<String, String>,
    /// Function id -> upstream routine (name or pinned URL).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub routines: BTreeMap<String, String>,
    pub confirmed_by: String,
    pub date: String,
}

/// An `upstream` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpstreamEntry {
    pub kovan: EntryMeta,
    pub upstream: UpstreamTable,
}

/// One deleted function's history row (#740 U5): enough to find its last
/// review in git.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DeletedFunction {
    pub function: String,
    /// The code-walk path it had.
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The commit of its last review.
    pub last_review_commit: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reviewers: Vec<String>,
}

/// A `deleted_functions` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeletedFunctionsEntry {
    pub kovan: EntryMeta,
    #[serde(default, rename = "deleted")]
    pub deleted: Vec<DeletedFunction>,
}

/// `[architecture]`: an architecture node (maintainer, #764, 2026-10-07).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureBody {
    /// Who recorded it, when, and at which commit (signed like a review).
    pub by: String,
    pub date: String,
    pub commit: String,
    /// The stable function ids that make up the node.
    #[serde(default)]
    pub members: Vec<String>,
    /// The upstream structure it follows, as the existing attribution type
    /// ([`Upstream`]); its `commit` is required and any `url` pinned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream: Option<Upstream>,
    /// For a generic node: the concept id of the pattern it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<Signature>,
}

/// An `architecture` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureEntry {
    pub kovan: EntryMeta,
    pub architecture: ArchitectureBody,
    #[serde(default, rename = "relation", skip_serializing_if = "Vec::is_empty")]
    pub relations: Vec<RelationRecord>,
}

/// One readable entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    Review(ReviewEntry),
    NeedsFix(NeedsFixEntry),
    Annotation(AnnotationEntry),
    Upstream(UpstreamEntry),
    DeletedFunctions(DeletedFunctionsEntry),
    Architecture(ArchitectureEntry),
    /// Any other kind; ignored by review, kept verbatim.
    Other { kind: String, toml: String },
}

impl Entry {
    /// The wire kind.
    pub fn kind(&self) -> &str {
        match self {
            Self::Review(_) => "review",
            Self::NeedsFix(_) => "needs_fix",
            Self::Annotation(_) => "annotation",
            Self::Upstream(_) => "upstream",
            Self::DeletedFunctions(_) => "deleted_functions",
            Self::Architecture(_) => "architecture",
            Self::Other { kind, .. } => kind,
        }
    }

    /// The entry's TOML text, as [`render_review_md`] writes it.
    pub fn to_toml(&self) -> Result<String, ReviewMdError> {
        let r = match self {
            Self::Review(e) => toml::to_string_pretty(e),
            Self::NeedsFix(e) => toml::to_string_pretty(e),
            Self::Annotation(e) => toml::to_string_pretty(e),
            Self::Upstream(e) => toml::to_string_pretty(e),
            Self::DeletedFunctions(e) => toml::to_string_pretty(e),
            Self::Architecture(e) => toml::to_string_pretty(e),
            Self::Other { toml, .. } => return Ok(toml.clone()),
        };
        r.map_err(|e| ReviewMdError::Emit(e.to_string()))
    }
}

/// One entry with its place in the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedEntry {
    pub heading: String,
    pub line: usize,
    pub entry: Entry,
    pub body: String,
}

/// An entry that could not be read: it counts as no review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unreadable {
    pub heading: String,
    pub line: usize,
    pub message: String,
    /// `[kovan] kind`, when it could be read.
    pub kind: Option<String>,
    /// The function id (`[review|needs_fix|annotation] function`) or, failing
    /// that, the `[kovan] target`, when either could be read.
    pub function: Option<String>,
    /// The reviewer, when it could be read.
    pub by: Option<String>,
}

/// A whole `review.md`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReviewDocument {
    pub entries: Vec<ParsedEntry>,
    pub unreadable: Vec<Unreadable>,
}

impl ReviewDocument {
    /// The review entries, in file order.
    pub fn reviews(&self) -> impl Iterator<Item = &ReviewEntry> {
        self.entries.iter().filter_map(|e| match &e.entry {
            Entry::Review(r) => Some(r),
            _ => None,
        })
    }

    /// The needs-fix entries.
    pub fn needs_fixes(&self) -> impl Iterator<Item = &NeedsFixEntry> {
        self.entries.iter().filter_map(|e| match &e.entry {
            Entry::NeedsFix(r) => Some(r),
            _ => None,
        })
    }

    /// The architecture nodes (crate-level `review.md`).
    pub fn architectures(&self) -> impl Iterator<Item = &ArchitectureEntry> {
        self.entries.iter().filter_map(|e| match &e.entry {
            Entry::Architecture(a) => Some(a),
            _ => None,
        })
    }

    /// The folder's upstream confirmation, if any.
    pub fn upstream(&self) -> Option<&UpstreamTable> {
        self.entries.iter().find_map(|e| match &e.entry {
            Entry::Upstream(u) => Some(&u.upstream),
            _ => None,
        })
    }
}

/// Why writing `review.md` failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewMdError {
    /// TOML serialisation failed (not expected for these field types).
    Emit(String),
}

impl std::fmt::Display for ReviewMdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Emit(e) => write!(f, "review.md: {e}"),
        }
    }
}

impl std::error::Error for ReviewMdError {}

fn str_at<'v>(v: &'v toml::Value, path: &[&str]) -> Option<&'v str> {
    let mut cur = v;
    for p in path {
        cur = cur.get(p)?;
    }
    cur.as_str()
}

fn check_target(meta: &EntryMeta) -> Result<(), FieldError> {
    match &meta.target {
        Some(t) if CodeTarget::parse(t).is_none() => Err(FieldError::BadTarget(t.clone())),
        _ => Ok(()),
    }
}

fn need_target(meta: &EntryMeta) -> Result<(), FieldError> {
    match &meta.target {
        None => Err(FieldError::BadTarget(String::new())),
        Some(_) => check_target(meta),
    }
}

/// Field-level validation of one review.
pub fn validate_review(r: &ReviewEntry) -> Result<(), FieldError> {
    need_target(&r.kovan)?;
    let b = &r.review;
    reviewer_id_kind(&b.by)?;
    if !(3..=4).contains(&b.rung) {
        return Err(FieldError::BadRung(b.rung));
    }
    check_commit("review.commit", &b.commit)?;
    check_hash("review.hash", &b.hash)?;
    check_hash("review.doc_hash", &b.doc_hash)?;
    if let Some(l) = &b.cargo_lock {
        check_hash("review.cargo_lock", l)?;
    }
    for h in b.callees.values() {
        check_hash("review.callees", h)?;
    }
    for (k, v) in &b.checklist {
        check_text(&format!("review.checklist.{k}"), v)?;
    }
    if let Some(n) = &b.no_concept {
        check_text("review.no_concept", n)?;
    }
    for m in &b.moved {
        check_commit("review.moved.commit", &m.commit)?;
    }
    Ok(())
}

/// Field-level validation of an upstream table: a port names its commit, and
/// every link is pinned (#764, 2026-10-07).
pub fn validate_upstream(u: &UpstreamTable) -> Result<(), FieldError> {
    reviewer_id_kind(&u.confirmed_by)?;
    if u.is_port {
        match &u.commit {
            Some(c) => check_commit("upstream.commit", c)?,
            None => {
                return Err(FieldError::BadCommit {
                    field: "upstream.commit".into(),
                    value: String::new(),
                })
            }
        }
    }
    // `repository` is the repository's home URL, pinned by `commit`; file
    // and routine links must carry the pin themselves.
    let links = u.files.values().chain(u.routines.values());
    for l in links.filter(|l| l.contains("://")) {
        check_pinned_url(l).map_err(FieldError::UnpinnedUrl)?;
    }
    Ok(())
}

/// Field-level validation of an architecture node: reviewer, commit, at
/// least one member or a pattern, and a commit-pinned upstream.
pub fn validate_architecture(a: &ArchitectureEntry) -> Result<(), FieldError> {
    let b = &a.architecture;
    reviewer_id_kind(&b.by)?;
    check_commit("architecture.commit", &b.commit)?;
    if let Some(p) = &b.pattern {
        check_text("architecture.pattern", p)?;
    }
    if b.members.is_empty() && b.pattern.is_none() {
        return Err(FieldError::TooShort {
            field: "architecture.members".into(),
        });
    }
    if let Some(u) = &b.upstream {
        validate_upstream_ref(u)?;
    }
    Ok(())
}

/// An [`Upstream`] used in review data must name its commit, and its `url`,
/// when present, must be pinned (#764, 2026-10-07).
pub fn validate_upstream_ref(u: &Upstream) -> Result<(), FieldError> {
    match &u.commit {
        Some(c) => check_commit("upstream.commit", &c.to_ascii_lowercase())?,
        None => {
            return Err(FieldError::BadCommit {
                field: "upstream.commit".into(),
                value: String::new(),
            })
        }
    }
    if let Some(url) = &u.url {
        check_pinned_url(url).map_err(FieldError::UnpinnedUrl)?;
    }
    Ok(())
}

fn read_entry(text: &str) -> Result<Entry, (String, toml::Value)> {
    let value: toml::Value = toml::from_str(text).map_err(|e| (e.to_string(), toml::Value::Table(Default::default())))?;
    let fail = |m: String| (m, value.clone());
    let kind = str_at(&value, &["kovan", "kind"]).unwrap_or("").to_string();
    let entry = match kind.as_str() {
        "review" => {
            let e: ReviewEntry = toml::from_str(text).map_err(|e| fail(e.to_string()))?;
            validate_review(&e).map_err(|e| fail(e.to_string()))?;
            Entry::Review(e)
        }
        "needs_fix" => {
            let e: NeedsFixEntry = toml::from_str(text).map_err(|e| fail(e.to_string()))?;
            let b = &e.needs_fix;
            let check = need_target(&e.kovan)
                .and_then(|_| reviewer_id_kind(&b.by).map(|_| ()))
                .and_then(|_| check_commit("needs_fix.commit", &b.commit))
                .and_then(|_| check_hash("needs_fix.hash", &b.hash))
                .and_then(|_| check_text("needs_fix.note", &b.note));
            check.map_err(|e| fail(e.to_string()))?;
            Entry::NeedsFix(e)
        }
        "annotation" => {
            let e: AnnotationEntry = toml::from_str(text).map_err(|e| fail(e.to_string()))?;
            let check = need_target(&e.kovan)
                .and_then(|_| reviewer_id_kind(&e.annotation.by).map(|_| ()))
                .and_then(|_| check_commit("annotation.commit", &e.annotation.commit));
            check.map_err(|e| fail(e.to_string()))?;
            Entry::Annotation(e)
        }
        "upstream" => {
            let e: UpstreamEntry = toml::from_str(text).map_err(|e| fail(e.to_string()))?;
            validate_upstream(&e.upstream).map_err(|e| fail(e.to_string()))?;
            Entry::Upstream(e)
        }
        "deleted_functions" => {
            let e: DeletedFunctionsEntry = toml::from_str(text).map_err(|e| fail(e.to_string()))?;
            Entry::DeletedFunctions(e)
        }
        "architecture" => {
            let e: ArchitectureEntry = toml::from_str(text).map_err(|e| fail(e.to_string()))?;
            validate_architecture(&e).map_err(|e| fail(e.to_string()))?;
            Entry::Architecture(e)
        }
        "" => return Err(fail("[kovan] has no kind".into())),
        other => Entry::Other {
            kind: other.to_string(),
            toml: text.to_string(),
        },
    };
    Ok(entry)
}

/// Read a whole `review.md` (module doc). Never fails as a whole.
pub fn parse_review_md(markdown: &str) -> ReviewDocument {
    let scan = scan_blocks_with(markdown, UnparseableFence::Report, |heading, line, text| {
        read_entry(text).map_err(|(message, v)| {
            let function = ["review", "needs_fix", "annotation"]
                .iter()
                .find_map(|t| str_at(&v, &[t, "function"]))
                .or_else(|| str_at(&v, &["kovan", "target"]))
                .map(str::to_string);
            let by = ["review", "needs_fix", "annotation"]
                .iter()
                .find_map(|t| str_at(&v, &[t, "by"]))
                .map(str::to_string);
            Unreadable {
                heading: heading.to_string(),
                line,
                message,
                kind: str_at(&v, &["kovan", "kind"]).map(str::to_string),
                function,
                by,
            }
        })
    });
    let mut doc = ReviewDocument {
        entries: scan
            .blocks
            .into_iter()
            .map(|b| ParsedEntry {
                heading: b.heading,
                line: b.line,
                entry: b.payload,
                body: b.body,
            })
            .collect(),
        unreadable: scan.problems,
    };
    demote_duplicate_reviews(&mut doc);
    doc
}

/// One standing review per (function, reviewer) (#739 "many maintainers"):
/// when a file holds two, neither is chosen silently; both become
/// unreadable, so the function returns to the queue.
fn demote_duplicate_reviews(doc: &mut ReviewDocument) {
    let mut count: BTreeMap<(String, String), usize> = BTreeMap::new();
    for r in doc.reviews() {
        *count
            .entry((r.review.function.clone(), r.review.by.clone()))
            .or_default() += 1;
    }
    let mut kept = Vec::with_capacity(doc.entries.len());
    for e in std::mem::take(&mut doc.entries) {
        if let Entry::Review(r) = &e.entry {
            let key = (r.review.function.clone(), r.review.by.clone());
            if count[&key] > 1 {
                doc.unreadable.push(Unreadable {
                    heading: e.heading.clone(),
                    line: e.line,
                    message: format!(
                        "{} has {} standing reviews of {}; one per reviewer",
                        key.1, count[&key], key.0
                    ),
                    kind: Some("review".into()),
                    function: Some(key.0),
                    by: Some(key.1),
                });
                continue;
            }
        }
        kept.push(e);
    }
    doc.entries = kept;
    doc.unreadable.sort_by_key(|u| u.line);
}

/// The generated last `##` of a review: its sign-off line (#743: generated
/// from the stamp, excluded from any content hash).
pub fn sign_off(r: &ReviewBody) -> String {
    let short: String = r.commit.chars().take(10).collect();
    format!(
        "## Sign-off\n\nReviewed by {} at rung {} on {}, commit {short}.",
        r.by, r.rung, r.date
    )
}

/// Write entries back as a `review.md`, each as `# heading`, its TOML block
/// and its body, separated by a blank line. Unreadable entries are not
/// written by this function: `review.md` is never auto-fixed, so a caller
/// that rewrites a file holding unreadable entries must splice its changes
/// in place instead (see [`crate::artifact::heading_span`]).
pub fn render_review_md(entries: &[ParsedEntry]) -> Result<String, ReviewMdError> {
    let mut out = String::new();
    for (i, e) in entries.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&render_block(ARTIFACT_LEVEL, &e.heading, &e.entry.to_toml()?, &e.body));
    }
    Ok(out)
}

#[cfg(test)]
#[path = "review_md_tests.rs"]
mod tests;
