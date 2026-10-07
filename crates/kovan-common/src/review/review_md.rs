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
//! review.
//!
//! **Which function** (maintainer, #764, 2026-10-07, the hybrid id;
//! [`super::id`]): `[kovan] target = "fn:<opaque>"` is the join key and
//! never changes; `path` in the entry's own table (`file.rs::Type::name`)
//! is the current location, updated when a move is acknowledged, and each
//! acknowledged move appends `[[review.moved]]` (from, to, commit).
//! ~~The join key is `[review] function`, the call-graph key, with `target`
//! a `code:` link~~ **CORRECTED 2026-10-07**: that first-version form still
//! reads and is migrated in memory ([`ReviewDocument::migrated`]).
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
//! target = "fn:3f2a9c0d1e4b5a67"
//!
//! [review]
//! path = "crates/tampines/src/steam.rs::SteamTable::flash"
//! by = "github:theodoreOnzGit"
//! rung = 3
//! date = "2026-10-07"
//! signed_at = "2026-10-07T14:03:09+08:00"   (since #783; absent on v1 stamps)
//! commit = "<40 hex>"
//! hash = "sha256:<64 hex>"
//! doc_hash = "sha256:<64 hex>"
//! cargo_lock = "sha256:<64 hex>"
//!
//! [review.callees]
//! "crates/tampines/src/steam.rs::saturation" = "sha256:<64 hex>"
//!
//! [review.checklist]
//! doc_matches_behaviour = "yes"
//! vv_evidence = "reference_code_to_code"
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

use super::id::{is_fn_id, is_fn_path, mint_fn_id};
use super::signing::Signature;
use super::types::{
    check_commit, check_date, check_hash, check_pinned_url, check_text, reviewer_id_kind,
    ChangeAuthorship, FieldError,
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
    /// The function's stable id, `fn:<opaque>` ([`super::id`]); folder-level
    /// entries have none. A first-version entry has a `code:` target here
    /// (migrated on read).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

/// `[[review.moved]]`: a machine-written record that the review was moved
/// with its function (#739 decision 14).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveRecord {
    /// The function's previous path.
    pub from: String,
    /// Its path after the move (added 2026-10-07 with the hybrid id;
    /// optional so a first-version record still reads).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// The commit the move was acknowledged at.
    pub commit: String,
}

/// `[review]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewBody {
    /// First-version join key (the call-graph key); only on an entry
    /// written before the hybrid id, and cleared by the migration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<String>,
    /// The function's current location, `file.rs::Type::name`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// `github:` / `gitlab:` / `orcid:` / email.
    pub by: String,
    /// 3 human reviewed, 4 human V&V (gated on `vv_evidence` and
    /// `independence`: [`super::wizard::stamp_gate`]). 5 is derived.
    pub rung: u8,
    /// `YYYY-MM-DD`.
    pub date: String,
    /// When the stamp was signed: RFC 3339 to the second, with its UTC
    /// offset (GitHub #783; [`super::signed_at`]). Signed (the v2 signed
    /// bytes); absent on a stamp signed before #783, which stays v1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signed_at: Option<String>,
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
    /// Wizard answers by question key (`doc_matches_behaviour`, …; the set
    /// and the stamp gate are [`super::wizard`], #769). Parsing accepts any
    /// key; #764's placeholder keys `q1` … `q10` are refused by the wizard,
    /// not here, so an entry holding them stays readable.
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
    /// First-version join key (the call-graph key); only on an entry
    /// written before the hybrid id, and cleared by the migration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<String>,
    /// The function's current location, `file.rs::Type::name`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
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
    /// First-version join key (the call-graph key); only on an entry
    /// written before the hybrid id, and cleared by the migration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<String>,
    /// The function's current location, `file.rs::Type::name`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
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
    /// The upstream commit ported from (a port: required). The only key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// The tag that pointed at `commit` when it was recorded, shown beside
    /// it (`v2016.53 (9a2951f)`, [`super::types::display_pin`]).
    /// Informational and never used to resolve (#764, 2026-10-07).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
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
    /// When it was signed, as a review's `signed_at` (GitHub #783).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signed_at: Option<String>,
    pub commit: String,
    /// The stable function ids that make up the node.
    #[serde(default)]
    pub members: Vec<String>,
    /// The members' current locations (`file.rs::item`), for display and
    /// the scope check; location metadata, updated on a move acknowledge
    /// and not signed (added 2026-10-07 with the hybrid id).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub member_paths: Vec<String>,
    /// The upstream structure it follows, as the existing attribution type
    /// ([`Upstream`]); its `commit` is required and any `url` pinned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream: Option<Upstream>,
    /// The tag label of the upstream commit (informational, unsigned).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_tag: Option<String>,
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
    /// The function, when it could be read: the `[kovan] target` (an
    /// `fn:` id), else `path`, else a first-version `function` key.
    pub function: Option<String>,
    /// The reviewer, when it could be read.
    pub by: Option<String>,
}

/// A whole `review.md`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReviewDocument {
    pub entries: Vec<ParsedEntry>,
    pub unreadable: Vec<Unreadable>,
    /// First-version keys migrated on read: (call-graph key, minted id).
    /// Non-empty means the next save rewrites the file in the new form.
    pub migrated: Vec<(String, String)>,
}

/// The hybrid-id accessors shared by the function-level entries.
macro_rules! function_ref {
    ($ty:ty, $body:ident) => {
        impl $ty {
            /// The join key: `[kovan] target` when it is an `fn:` id, else
            /// the first-version `function` key (before migration).
            pub fn function_id(&self) -> String {
                match &self.kovan.target {
                    Some(t) if is_fn_id(t) => t.clone(),
                    _ => self.$body.function.clone().unwrap_or_default(),
                }
            }

            /// The current location, `file.rs::Type::name`.
            pub fn path(&self) -> Option<String> {
                self.$body.path.clone().or_else(|| match &self.kovan.target {
                    Some(t) if !is_fn_id(t) => {
                        CodeTarget::parse(t).map(|c| format!("{}::{}", c.file, c.item))
                    }
                    _ => self.$body.function.clone(),
                })
            }
        }
    };
}
function_ref!(ReviewEntry, review);
function_ref!(NeedsFixEntry, needs_fix);
function_ref!(AnnotationEntry, annotation);

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

/// The function reference of an entry: either the hybrid form (`target` an
/// `fn:` id and a valid `path`) or the first-version form (a `function`
/// key, with `target` absent or a `code:` link).
fn check_function_ref(
    meta: &EntryMeta,
    function: &Option<String>,
    path: &Option<String>,
) -> Result<(), FieldError> {
    match (&meta.target, function) {
        (Some(t), _) if is_fn_id(t) => match path {
            Some(p) if is_fn_path(p) => Ok(()),
            Some(p) => Err(FieldError::BadTarget(p.clone())),
            None => Err(FieldError::BadTarget("path is missing".into())),
        },
        (Some(t), Some(_)) if CodeTarget::parse(t).is_some() => Ok(()),
        (None, Some(f)) if is_fn_path(f) => Ok(()),
        (Some(t), _) => Err(FieldError::BadTarget(t.clone())),
        (None, _) => Err(FieldError::BadTarget(String::new())),
    }
}

/// Field-level validation of one review.
pub fn validate_review(r: &ReviewEntry) -> Result<(), FieldError> {
    let b = &r.review;
    check_function_ref(&r.kovan, &b.function, &b.path)?;
    reviewer_id_kind(&b.by)?;
    check_date("review.date", &b.date)?;
    if !(3..=4).contains(&b.rung) {
        return Err(FieldError::BadRung(b.rung));
    }
    // ~~The rung is both `[review] rung` and the wizard's `rung` question:
    // they must agree~~ CORRECTED 2026-10-07: the rung is derived, never
    // answered; the staleness engine recomputes it and an entry whose
    // recorded rung differs is invalid (`engine::InvalidReason`).
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
    check_date("upstream.date", &u.date)?;
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
    check_date("architecture.date", &b.date)?;
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
            let check = check_function_ref(&e.kovan, &b.function, &b.path)
                .and_then(|_| reviewer_id_kind(&b.by).map(|_| ()))
                .and_then(|_| check_date("needs_fix.date", &b.date))
                .and_then(|_| check_commit("needs_fix.commit", &b.commit))
                .and_then(|_| check_hash("needs_fix.hash", &b.hash))
                .and_then(|_| check_text("needs_fix.note", &b.note));
            check.map_err(|e| fail(e.to_string()))?;
            Entry::NeedsFix(e)
        }
        "annotation" => {
            let e: AnnotationEntry = toml::from_str(text).map_err(|e| fail(e.to_string()))?;
            let a = &e.annotation;
            let check = check_function_ref(&e.kovan, &a.function, &a.path)
                .and_then(|_| reviewer_id_kind(&a.by).map(|_| ()))
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
            let tables = ["review", "needs_fix", "annotation"];
            let function = str_at(&v, &["kovan", "target"])
                .filter(|t| is_fn_id(t))
                .or_else(|| tables.iter().find_map(|t| str_at(&v, &[t, "path"])))
                .or_else(|| tables.iter().find_map(|t| str_at(&v, &[t, "function"])))
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
        migrated: Vec::new(),
    };
    migrate_legacy_ids(&mut doc);
    demote_duplicate_reviews(&mut doc);
    doc
}

/// Migrate first-version entries (module doc) in memory. Each call-graph
/// key gets one id, minted from the key and the hash and commit of its
/// earliest review (by date, then commit); a key with no review uses its
/// needs-fix (hash and commit) or annotation (commit only). Architecture
/// members naming a migrated key are rewritten too. Deterministic.
fn migrate_legacy_ids(doc: &mut ReviewDocument) {
    let legacy = |t: &Option<String>| !t.as_deref().is_some_and(is_fn_id);
    // key -> (date, commit, hash) of the source the id is minted from.
    let mut seed: BTreeMap<String, (u8, String, String, String)> = BTreeMap::new();
    let mut offer = |key: &str, rank: u8, date: &str, commit: &str, hash: &str| {
        let cand = (rank, date.to_string(), commit.to_string(), hash.to_string());
        let e = seed.entry(key.to_string()).or_insert_with(|| cand.clone());
        if cand < *e {
            *e = cand;
        }
    };
    for e in &doc.entries {
        match &e.entry {
            Entry::Review(r) if legacy(&r.kovan.target) => {
                if let Some(k) = &r.review.function {
                    offer(k, 0, &r.review.date, &r.review.commit, &r.review.hash);
                }
            }
            Entry::NeedsFix(n) if legacy(&n.kovan.target) => {
                if let Some(k) = &n.needs_fix.function {
                    offer(k, 1, &n.needs_fix.date, &n.needs_fix.commit, &n.needs_fix.hash);
                }
            }
            Entry::Annotation(a) if legacy(&a.kovan.target) => {
                if let Some(k) = &a.annotation.function {
                    offer(k, 2, "", &a.annotation.commit, "");
                }
            }
            _ => {}
        }
    }
    let ids: BTreeMap<String, String> = seed
        .into_iter()
        .map(|(k, (_, _, commit, hash))| {
            let id = mint_fn_id(&k, &hash, &commit);
            (k, id)
        })
        .collect();
    if ids.is_empty() {
        return;
    }
    fn fix(meta: &mut EntryMeta, function: &mut Option<String>, path: &mut Option<String>, ids: &BTreeMap<String, String>) {
        if meta.target.as_deref().is_some_and(is_fn_id) {
            return;
        }
        if let Some(k) = function.take() {
            if let Some(id) = ids.get(&k) {
                meta.target = Some(id.clone());
                *path = Some(k);
            } else {
                *function = Some(k);
            }
        }
    }
    for e in &mut doc.entries {
        match &mut e.entry {
            Entry::Review(r) => fix(&mut r.kovan, &mut r.review.function, &mut r.review.path, &ids),
            Entry::NeedsFix(n) => fix(&mut n.kovan, &mut n.needs_fix.function, &mut n.needs_fix.path, &ids),
            Entry::Annotation(a) => fix(&mut a.kovan, &mut a.annotation.function, &mut a.annotation.path, &ids),
            Entry::Architecture(a) => {
                let b = &mut a.architecture;
                if b.members.iter().any(|m| ids.contains_key(m)) && b.member_paths.is_empty() {
                    b.member_paths = b.members.clone();
                }
                for m in &mut b.members {
                    if let Some(id) = ids.get(m) {
                        *m = id.clone();
                    }
                }
            }
            _ => {}
        }
    }
    doc.migrated = ids.into_iter().collect();
}

/// One standing review per (function, reviewer) (#739 "many maintainers"):
/// when a file holds two, neither is chosen silently; both become
/// unreadable, so the function returns to the queue.
fn demote_duplicate_reviews(doc: &mut ReviewDocument) {
    let mut count: BTreeMap<(String, String), usize> = BTreeMap::new();
    for r in doc.reviews() {
        *count
            .entry((r.function_id(), r.review.by.clone()))
            .or_default() += 1;
    }
    let mut kept = Vec::with_capacity(doc.entries.len());
    for e in std::mem::take(&mut doc.entries) {
        if let Entry::Review(r) = &e.entry {
            let key = (r.function_id(), r.review.by.clone());
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
