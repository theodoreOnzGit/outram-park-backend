//! The **staleness engine** (GitHub #765): a pure function from the folders'
//! `review.md` entries, the current `kovan.toml` indexes, the reviewer
//! registry and git facts **passed in as data** to each function's stamp
//! state. No filesystem, no git, no clock: desktop kovan, web-kovan and CI
//! call it with the same inputs and get the same answer.
//!
//! # The rules (#739 D6 and the 2026-10-07 comments, #740 U4/U5)
//!
//! Each standing review is judged on its own, in this order; the first rule
//! that applies gives its state.
//!
//! 1. **Find the function.** By the review's stable id; failing that, by
//!    hash among current functions that no review claims by id (a rename or
//!    move whose id the index did not keep). One candidate: found, and the
//!    match is reported in [`Evaluation::id_matches`]. Several: moved, with
//!    the candidates, for the maintainer to say which is the original.
//!    None: **deleted** (a rename and edit in one commit lands here: the old
//!    review goes to history and the function itself is **new**).
//! 2. **Authenticity** ([`UnverifiedReason`]). The git facts for the stamp
//!    must exist; the commit that added it must not carry the agent
//!    attribution trailer; it must come after the commit it certifies; and
//!    the function's hash recomputed **at that certified commit** must equal
//!    the recorded hash (time-bound stamps). The reviewer must be registered
//!    in `kovan_root.toml`, and the stamp dated before any revocation (or
//!    compromise). With [`SignaturePolicy::Enforce`] the signature must
//!    verify against the registry (#762): a scope refusal there is **outside
//!    scope**, a revocation or compromise is unverified with that reason,
//!    anything else unverified with the signature's reason.
//! 3. **Scope.** A `reviewer` (not a `maintainer`) must have the function's
//!    file in scope, else **outside scope**. Reviewer and key state are
//!    read only through the #762 [`Registry`].
//! 3b. **Rung.** The recorded rung must equal the derived one (see
//!    **Rungs** below), else **invalid**: shown, never counted.
//! 4. **Own code.** Hash changed: **directly stale**. Same hash but the
//!    resolved callees differ from those recorded: also directly stale (a
//!    callee resolving differently, #739 decision 14).
//! 5. **Location.** Found somewhere other than the review's `path`:
//!    **moved**, carrying the reaching-test verdict for the acknowledge.
//! 6. **Callees.** A recorded callee's hash differs now (or it is gone):
//!    **inherited stale**, one level only. It always needs a human
//!    re-confirm; the re-confirm is **blocked** unless the reaching tests
//!    pass (or none reach it, which is the separate "untested" flag).
//! 7. **Doc.** Only `doc_hash` changed: **doc changed**.
//! 8. **Cargo.lock.** The review recorded a different lock: **pending
//!    workspace test**, until a full run at the current lock; then **valid**
//!    if no reaching test failed, else inherited stale (blocked).
//! 9. Otherwise **valid**.
//!
//! Per function, **a concern beats an approval**: an open needs-fix gives
//! **needs fix** while the hash is the one it was raised against and
//! **fixed** once the code changed, whatever the reviews say. Otherwise the
//! function is valid if any review is, else it takes the most actionable
//! review state ([`AGGREGATE_ORDER`]), else **new**. An unreadable entry is
//! no review: it only shows as **review unreadable** when nothing else
//! stands.
//!
//! **Rungs.** A stamp's rung is **derived, never chosen** (maintainer,
//! #769, 2026-10-07; [`crate::review::wizard::derived_rung`]): 4 when the
//! V&V answers qualify, the V&V case was written and verified by hand, and
//! git shows no agent trailer on the commits that added the tests that
//! reached the function **at the review commit**, with git facts as of that
//! commit ([`StampFacts::tests_at_review`]; maintainer on #765, 2026-10-07).
//! ~~the tests reaching the function now (`GitFacts::test_commit_messages`)~~
//! **CORRECTED 2026-10-07**: today's reach would let a later test change a
//! past review's rung. Else 3. The engine recomputes it on read, and a
//! recorded `rung` that differs makes the review **invalid**: shown, never
//! counted (Leak Before Break).
//!
//! **The wizard gate is re-run on read** (maintainer on #765, 2026-10-07):
//! a stamp whose answers the gate now blocks (an unanswered applicable
//! question, a blocking answer, a legacy or unknown key) is **invalid**.
//! Applicability: a port when the folder's upstream says so; a physical
//! interface when the review answered `units_documented` (the index does
//! not record interfaces, so the reviewer's own judgement is taken).
//!
//! **Flags** ([`FunctionFlag`]) never void a stamp: a test that reaches the
//! function now but did not at the review commit ("new test reaches
//! reviewed function": flagged for review, and itself unreviewed code), and
//! identical copies of reviewed code ("duplicate code": the review shows on
//! **every** candidate; maintainer on #765, 2026-10-07), and an implausible
//! `signed_at` ("implausible signing time", GitHub #783: signed before the
//! reviewed commit, after the commit that added the stamp, or on another
//! day than `date`, each with 5 minutes' clock skew;
//! [`super::signed_at`]). The git times for the last come in as data
//! ([`StampFacts::reviewed_commit_time`], [`StampCommit::committer_time`]);
//! "the commit that introduced the stamp" is [`StampFacts::added_in`], the
//! same commit the authenticity rule checks. A v1 stamp (no `signed_at`) is
//! never flagged.
//! ~~A review's rung 4 counts only when the wizard's gate opens it,
//! otherwise it counts as rung 3 and is flagged~~ **CORRECTED 2026-10-07**. A
//! function is at **rung 5** when, besides its earliest valid review, a
//! valid review exists by a different reviewer who is not one of the code's
//! authors (from git), whose wizard answer to `independence` is
//! `someone_else` ([`ReviewReport::independent`]; maintainer on #769,
//! 2026-10-07: independence gates rung 5, not rung 4) **and** who holds a
//! qualification covering every concept
//! area of the function ([`ConceptAreas`]; maintainer, #739, 2026-10-07:
//! "only rung 5 enforces qualification"). A function with no known concept
//! area cannot reach rung 5. Below rung 5 qualification is shown
//! ([`ReviewReport::qualifications`]) and never enforced; scope is.
//!
//! **Test evidence** is the folder's `[test_run]`, judged by #766's
//! [`reach_verdict`] (full suite only, current `Cargo.lock`, edited tests
//! never count as passes), plus the engine's own check that the function's
//! hash at the run's commit is its hash now ([`GitFacts::hashes_at_test_run`];
//! [`TestVerdict::ChangedSinceRun`] otherwise). ~~A run "the caller says is
//! current" counts~~ **CORRECTED 2026-10-07**: the check is per function.

use std::collections::{BTreeMap, BTreeSet};

use crate::artifact::relation::CodeTarget;

use super::evidence::verdict::{reach_verdict, ReachVerdict};
use super::index::{FolderIndex, FunctionIndex, Suite, TestRun};
use super::review_md::{DeletedFunction, FixStatus, ReviewDocument, ReviewEntry};
use super::root::{ReviewRoot, Role};
use super::scope::in_scope;
use super::signed_at::{plausibility, SignedAtProblem};
use super::signing::registry::Registry;
use super::signing::{verify_review, SignatureCheck, UnverifiedReason as SigReason};
use super::state::FlagKind;
use super::wizard::{stamp_gate, Applicability, GateReason, TestAuthorship};
use super::state::StateKind;
use super::types::{check_tag, TagCheck};


/// One folder's `review.md`, with where it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderReviews {
    pub krate: String,
    /// Workspace-relative folder.
    pub dir: String,
    pub doc: ReviewDocument,
}

/// A standing review's key: one per reviewer per function.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReviewKey {
    pub function: String,
    pub by: String,
}

/// The commit that added a stamp to `review.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StampCommit {
    pub commit: String,
    /// It is a strict descendant of the commit the stamp certifies.
    pub after_certified: bool,
    /// Its message carries `Co-Authored-By: Claude…` or `Claude-Session:`.
    pub agent_trailer: bool,
    /// Its committer time, seconds since the epoch (`git log -1
    /// --format=%ct`), for the `signed_at` plausibility flag (#783); `None`
    /// when not looked up (the check is skipped).
    pub committer_time: Option<i64>,
}

/// What git says about one stamp (computed by the caller).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StampFacts {
    /// The function's `hash` recomputed at the certified commit; `None` when
    /// it could not be found there.
    pub hash_at_commit: Option<String>,
    /// `None` when the stamp is not committed yet.
    pub added_in: Option<StampCommit>,
    /// The committer time of the commit the stamp certifies (its
    /// `commit`), seconds since the epoch, for the `signed_at`
    /// plausibility flag (#783); `None` when not looked up.
    pub reviewed_commit_time: Option<i64>,
    /// The tests that reached the function at the certified commit, with
    /// git facts as of that commit; `None` when not computed (then git's
    /// view is unknown: rung 3, and no new-test flags).
    pub tests_at_review: Option<TestsAtReview>,
}

/// The tests reaching a function at its review commit (maintainer on #765,
/// 2026-10-07: "judge the rung from the tests that reached the function at
/// the review commit").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TestsAtReview {
    /// Test ids (the call graph's reach at that commit).
    pub reached_by: Vec<String>,
    /// Test id -> the messages of the commits, up to the review commit, that
    /// added or changed it.
    pub commit_messages: BTreeMap<String, Vec<String>>,
}

impl TestsAtReview {
    /// Git's view of who wrote these tests: human when every one has commit
    /// facts and none carries the agent trailer.
    pub fn authorship(&self) -> TestAuthorship {
        if self.reached_by.is_empty() {
            return TestAuthorship::Unknown;
        }
        let mut messages = Vec::new();
        for t in &self.reached_by {
            match self.commit_messages.get(t) {
                Some(m) if !m.is_empty() => messages.extend(m.iter().cloned()),
                _ => return TestAuthorship::Unknown,
            }
        }
        TestAuthorship::from_messages(&messages)
    }
}

/// A publish-time verification record (#773, not built yet): what a
/// crates.io copy carries instead of git history. Read into
/// [`GitFacts::publish_records`]; until #773 defines and checks it, a stamp
/// that has only a record is [`UnverifiedReason::PublishRecordNotChecked`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishRecord {
    /// The commit the package was published from.
    pub published_from: String,
    /// The record as stored, opaque until #773.
    pub raw: String,
}

/// Git, as data.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitFacts {
    pub head: String,
    /// `sha256:` hash of the current `Cargo.lock`.
    pub cargo_lock: String,
    /// Function id -> its hash at the `[test_run]` commit (#766: "the engine
    /// still checks that the function hash is unchanged since the evidence
    /// commit"). A function missing here is treated as changed: the run
    /// does not speak for it.
    pub hashes_at_test_run: BTreeMap<String, String>,
    pub stamps: BTreeMap<ReviewKey, StampFacts>,
    /// Function id -> the reviewer ids of the people who wrote its code.
    pub code_authors: BTreeMap<String, BTreeSet<String>>,
    /// Deleted function id -> the commit that deleted it.
    pub deleted_in: BTreeMap<String, String>,
    /// ~~`test_commit_messages`: test id -> commit messages, for today's
    /// reaching tests~~ **CORRECTED 2026-10-07**: replaced by
    /// [`StampFacts::tests_at_review`], as of the review commit.
    ///
    /// Publish-time verification records for copies without git (#773).
    pub publish_records: BTreeMap<ReviewKey, PublishRecord>,
    /// The previous committed `kovan_root.toml` (parsed), for the
    /// append-only check of key histories ([`HistoryWarning`]). `None` when
    /// there is no earlier commit of it.
    pub previous_root: Option<ReviewRoot>,
    /// (repository URL, tag) -> the commit the tag points at now, for the
    /// tags the caller could look up (local or vendored clone, `git
    /// ls-remote`). Offline, it is empty and tags show as unchecked.
    pub tag_commits: BTreeMap<(String, String), String>,
}

/// An upstream tag label, checked (#764, 2026-10-07: "a tag moved state").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpstreamTagReport {
    /// The folder whose `review.md` holds it.
    pub dir: String,
    /// The architecture entry it is on; `None` for the folder's upstream.
    pub architecture: Option<String>,
    pub repository: Option<String>,
    pub tag: String,
    pub commit: String,
    pub check: TagCheck,
}

/// Function id -> the concept-tree areas it implements (resolved by the
/// caller from the review's `implements` relations, #739 decision 21).
pub type ConceptAreas = BTreeMap<String, BTreeSet<String>>;

/// Whether signatures are required.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignaturePolicy {
    /// A stamp counts only with a signature that verifies against the
    /// `kovan_root.toml` registry (#762). The normal setting.
    Enforce,
    /// Signatures are not checked; every other authenticity rule (git
    /// facts, trailer, time-bound, registered and unrevoked reviewer) still
    /// applies. ~~`AwaitingCrypto`, until #762 lands~~ **CORRECTED
    /// 2026-10-07**: #762 has landed; this is for tools and tests that judge
    /// staleness alone, and its results must not be shown as reviewed.
    NotChecked,
}

/// Where a function is.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Location {
    /// Workspace-relative file.
    pub file: String,
    /// `name` or `Type::name`.
    pub qual: String,
}

/// Why a stamp's authenticity does not hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnverifiedReason {
    /// No git facts for the stamp (a copy without history, or not looked up).
    NoGitFacts,
    /// Not committed yet.
    NotCommitted,
    /// Added in a commit with the agent attribution trailer ("AI never
    /// stamps").
    AgentTrailer,
    /// Added in, or before, the commit it certifies.
    NotAfterCertifiedCommit,
    /// The function at the certified commit does not hash to the recorded
    /// hash (an edit copied into a stamp, or the wrong commit).
    HashMismatchAtCommit { at_commit: Option<String> },
    /// `by` is not a `[[reviewer]]` of `kovan_root.toml`.
    UnknownReviewer,
    /// Dated on or after the reviewer's revocation.
    Revoked,
    /// Dated on or after the key's compromise date.
    Compromised,
    /// No git facts, but a publish-time record (#773) that this engine
    /// cannot check yet.
    PublishRecordNotChecked,
    Signature(SigReason),
}

/// Why a review does not count before its content is looked at.
enum Denial {
    Unverified(UnverifiedReason),
    OutsideScope,
}

/// The reaching-test verdict for one function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestVerdict {
    /// #766's verdict ([`reach_verdict`]), for a function whose hash is
    /// unchanged since the run.
    Reach(ReachVerdict),
    /// The function's hash at the run's commit differs from now, or is not
    /// known: the recorded run cannot speak for it (pending).
    ChangedSinceRun,
}

impl TestVerdict {
    /// Whether a re-confirm may be given: every counting reaching test
    /// passed, or no test reaches the function (the separate "untested"
    /// flag). Passing tests are necessary, never sufficient (#739 D6).
    pub fn allows_reconfirm(&self) -> bool {
        matches!(
            self,
            Self::Reach(ReachVerdict::Passed { .. } | ReachVerdict::NoReachingTests)
        )
    }
}

/// Why a review is directly stale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaleWhy {
    CodeChanged { reviewed: String, now: String },
    /// Same code, but callees resolve differently.
    CalleesResolveDifferently { added: Vec<String>, removed: Vec<String> },
}

/// What made a review inherited-stale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InheritedCause {
    /// These callees' hashes changed (or they are gone).
    Callees(Vec<String>),
    /// A reaching test failed at the new `Cargo.lock`.
    LockTestFailed,
}

/// A state with its details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StampState {
    Valid,
    DirectlyStale(StaleWhy),
    DocChanged,
    InheritedStale {
        cause: InheritedCause,
        tests: TestVerdict,
        /// Re-confirm is disabled until the reaching tests pass.
        blocked: bool,
    },
    Moved {
        from: Option<Location>,
        to: Location,
        tests: TestVerdict,
        /// More than one identical candidate: which is the original?
        candidates: Vec<String>,
    },
    Deleted,
    New,
    NeedsFixOpen { entry: String, note: String },
    Fixed { entry: String, note: String },
    Unverified(UnverifiedReason),
    OutsideScope,
    Unreadable { message: String },
    PendingWorkspaceTest { reviewed_lock: String, current_lock: String },
    /// The entry reads but contradicts what can be derived: shown, never
    /// counted (Leak Before Break).
    Invalid(InvalidReason),
}

/// Why a readable review is invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidReason {
    /// `[review] rung` is not the rung derived from the answers and git.
    RungMismatch {
        recorded: u8,
        derived: u8,
        tests: TestAuthorship,
    },
    /// The wizard gate, re-run on read, blocks the recorded answers.
    GateBlocked(Vec<GateReason>),
}

/// A flag on a function: shown, queued for a person, never voids a stamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionFlag {
    /// Tests reaching the function now that did not reach it at a review's
    /// commit: flagged for review, and themselves unreviewed code.
    NewReachingTests { review: String, tests: Vec<String> },
    /// Identical code (same hash) found more than once: the review shows on
    /// every copy; `copies` are the other candidates.
    DuplicateCode { copies: Vec<String> },
    /// A review's `signed_at` is implausible against git or its own `date`
    /// (#783, [`super::signed_at::plausibility`]): tamper evidence, never a
    /// void.
    ImplausibleSignedAt { review: String, problems: Vec<SignedAtProblem> },
}

impl FunctionFlag {
    pub fn kind(&self) -> FlagKind {
        match self {
            Self::NewReachingTests { .. } => FlagKind::NewReachingTest,
            Self::DuplicateCode { .. } => FlagKind::DuplicateCode,
            Self::ImplausibleSignedAt { .. } => FlagKind::ImplausibleSigningTime,
        }
    }
}

/// A test function that started reaching a reviewed function after its
/// review (an engine output, for the queue and for the test's own review).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NewReachingTest {
    pub test: String,
    pub function: String,
    pub review: String,
}

impl StampState {
    pub fn kind(&self) -> StateKind {
        match self {
            Self::Valid => StateKind::Valid,
            Self::DirectlyStale(_) => StateKind::DirectlyStale,
            Self::DocChanged => StateKind::DocChanged,
            Self::InheritedStale { .. } => StateKind::InheritedStale,
            Self::Moved { .. } => StateKind::Moved,
            Self::Deleted => StateKind::Deleted,
            Self::New => StateKind::New,
            Self::NeedsFixOpen { .. } => StateKind::NeedsFixOpen,
            Self::Fixed { .. } => StateKind::Fixed,
            Self::Unverified(_) => StateKind::Unverified,
            Self::OutsideScope => StateKind::OutsideScope,
            Self::Unreadable { .. } => StateKind::Unreadable,
            Self::PendingWorkspaceTest { .. } => StateKind::PendingWorkspaceTest,
            Self::Invalid(_) => StateKind::Invalid,
        }
    }
}

/// When no review is valid, the function shows the first of these its
/// reviews have.
pub const AGGREGATE_ORDER: [StateKind; 11] = [
    StateKind::DirectlyStale,
    StateKind::InheritedStale,
    StateKind::Moved,
    StateKind::DocChanged,
    StateKind::PendingWorkspaceTest,
    StateKind::Unverified,
    StateKind::Invalid,
    StateKind::OutsideScope,
    StateKind::Deleted,
    StateKind::Unreadable,
    StateKind::New,
];

/// One review, judged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewReport {
    pub by: String,
    /// The `review.md` artifact id (`None` for an unreadable entry).
    pub artifact: Option<String>,
    pub date: Option<String>,
    pub state: StampState,
    /// The rung recorded (3 or 4); it counts only when it equals the
    /// derived rung (else the state is invalid).
    pub rung: Option<u8>,
    /// The reviewer's qualification labels, shown beside the stamp
    /// (self-declared ones say so).
    pub qualifications: Vec<String>,
    /// The wizard says the reviewer is independent of the code
    /// (`independence = "someone_else"`): may be rung 5's second review.
    pub independent: bool,
}

/// One function, judged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionReport {
    pub id: String,
    /// Where it is now (`None` for a deleted function).
    pub location: Option<Location>,
    pub state: StampState,
    pub reviews: Vec<ReviewReport>,
    /// 3, 4 or 5 when valid.
    pub rung: Option<u8>,
    /// The standing "no test reaches this function" flag.
    pub untested: bool,
    /// Workspace callees whose own state does not count (bottom-up).
    pub blocked_by: Vec<String>,
    /// Flags that never void a stamp ([`FunctionFlag`]).
    pub flags: Vec<FunctionFlag>,
}

/// Where a deleted function's history row goes (#739 D6).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum HistoryPlacement {
    /// The folder still exists: its `review.md` deleted-functions table.
    FolderReviewMd { dir: String },
    /// The folder is gone, the crate is not: the crate root `kovan.toml`.
    CrateIndex { krate: String, dir: String, deleted_folder: String },
    /// The crate is gone: `kovan_root.toml`.
    WorkspaceRoot { krate: String, dir: String },
}

/// A deleted function's history row and where it goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryRow {
    pub placement: HistoryPlacement,
    pub row: DeletedFunction,
}

/// A review id matched back to a current function by hash.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct IdMatch {
    pub review_function: String,
    pub current_id: String,
}

/// An unreadable entry that names no current function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrphanUnreadable {
    pub dir: String,
    pub heading: String,
    pub line: usize,
    pub message: String,
}

/// The engine's result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Evaluation {
    /// Every current function, by current id.
    pub functions: BTreeMap<String, FunctionReport>,
    /// Functions whose reviews remain but which are gone.
    pub deleted: Vec<FunctionReport>,
    pub history: Vec<HistoryRow>,
    pub id_matches: Vec<IdMatch>,
    pub orphan_unreadable: Vec<OrphanUnreadable>,
    /// Every upstream tag label with its check; informational only, the
    /// commit pin is what counts.
    pub upstream_tags: Vec<UpstreamTagReport>,
    /// Key-history entries that were committed before and are now gone or
    /// changed: loud warnings (append-only check against git).
    pub history_warnings: Vec<HistoryWarning>,
    /// Tests that started reaching a reviewed function after its review.
    pub new_reaching_tests: Vec<NewReachingTest>,
}

/// A breach of the append-only key history (#762 follow-up, 2026-10-07):
/// the file alone cannot show that its LAST entry was deleted, so the
/// previous committed `kovan_root.toml` is compared. Loud; it does not by
/// itself change a stamp's state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryWarning {
    /// A reviewer committed before is gone.
    ReviewerRemoved { reviewer: String },
    /// A key committed before is gone.
    KeyRemoved { reviewer: String, key: String },
    /// History entry `index` (0-based) of the key is gone.
    EntryRemoved { reviewer: String, key: String, index: usize },
    /// History entry `index` was changed.
    EntryChanged { reviewer: String, key: String, index: usize },
}

/// Compare the previous committed root with the current one: every
/// reviewer, key and key-history entry present before must be present now,
/// unchanged, at the same position (appending is the only change allowed).
pub fn history_append_only(previous: &ReviewRoot, current: &ReviewRoot) -> Vec<HistoryWarning> {
    let mut out = Vec::new();
    for pr in &previous.reviewers {
        let Some(cr) = current.reviewer(&pr.id) else {
            out.push(HistoryWarning::ReviewerRemoved { reviewer: pr.id.clone() });
            continue;
        };
        for pk in &pr.keys {
            let Some(ck) = cr.keys.iter().find(|k| k.id == pk.id) else {
                out.push(HistoryWarning::KeyRemoved { reviewer: pr.id.clone(), key: pk.id.clone() });
                continue;
            };
            for (i, e) in pk.history.iter().enumerate() {
                let (reviewer, key) = (pr.id.clone(), pk.id.clone());
                match ck.history.get(i) {
                    None => out.push(HistoryWarning::EntryRemoved { reviewer, key, index: i }),
                    Some(c) if c != e => out.push(HistoryWarning::EntryChanged { reviewer, key, index: i }),
                    Some(_) => {}
                }
            }
        }
    }
    out
}

impl Evaluation {
    /// Moves awaiting acknowledge, grouped by (from folder, to folder), for
    /// a one-click batch acknowledge of a moved file or folder.
    pub fn move_batches(&self) -> BTreeMap<(String, String), Vec<String>> {
        let mut out: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
        for f in self.functions.values() {
            for r in &f.reviews {
                if let StampState::Moved { from, to, .. } = &r.state {
                    let from_dir = from.as_ref().map(|l| parent(&l.file)).unwrap_or_default();
                    out.entry((from_dir, parent(&to.file)))
                        .or_default()
                        .push(f.id.clone());
                }
            }
        }
        for v in out.values_mut() {
            v.sort();
            v.dedup();
        }
        out
    }
}

fn parent(file: &str) -> String {
    file.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default()
}

/// A current function and the folder it is in.
struct Current<'i> {
    index: &'i FolderIndex,
    file: String,
    f: &'i FunctionIndex,
}

impl Current<'_> {
    fn location(&self) -> Location {
        Location {
            file: self.index.file_path(&self.file),
            qual: self.f.qual.clone(),
        }
    }
}

/// The reaching-test verdict of `f` from its folder's `test_run`: #766's
/// [`reach_verdict`], unless a run exists and `f`'s hash at the run's commit
/// is not its hash now.
pub fn test_verdict(f: &FunctionIndex, run: Option<&TestRun>, git: &GitFacts) -> TestVerdict {
    let v = reach_verdict(&f.reached_by, run, &git.cargo_lock);
    match v {
        ReachVerdict::NoReachingTests | ReachVerdict::Pending(_) => TestVerdict::Reach(v),
        _ if !unchanged_since_run(f, git) => TestVerdict::ChangedSinceRun,
        _ => TestVerdict::Reach(v),
    }
}

fn unchanged_since_run(f: &FunctionIndex, git: &GitFacts) -> bool {
    git.hashes_at_test_run.get(&f.id) == Some(&f.hash)
}

fn authenticity(
    r: &ReviewEntry,
    git: &GitFacts,
    registry: &Registry,
    policy: SignaturePolicy,
) -> Option<Denial> {
    authenticity_git(r, git, registry)
        .map(Denial::Unverified)
        .or_else(|| (policy == SignaturePolicy::Enforce).then(|| signature(r, registry)).flatten())
}

/// The #762 signature check, mapped onto the engine's states.
fn signature(r: &ReviewEntry, registry: &Registry) -> Option<Denial> {
    match verify_review(r, registry) {
        SignatureCheck::Verified(_) => None,
        SignatureCheck::Unverified(why) => Some(match why {
            SigReason::OutsideScope { .. } => Denial::OutsideScope,
            SigReason::Revoked { .. } => Denial::Unverified(UnverifiedReason::Revoked),
            SigReason::Compromised { .. } => Denial::Unverified(UnverifiedReason::Compromised),
            SigReason::UnknownReviewer(_) => Denial::Unverified(UnverifiedReason::UnknownReviewer),
            other => Denial::Unverified(UnverifiedReason::Signature(other)),
        }),
    }
}

/// Git facts, then the reviewer's state, read only through the #762
/// [`Registry`] (registered, and not revoked or compromised at the stamp's
/// date).
fn authenticity_git(r: &ReviewEntry, git: &GitFacts, registry: &Registry) -> Option<UnverifiedReason> {
    let b = &r.review;
    let key = ReviewKey {
        function: r.function_id(),
        by: b.by.clone(),
    };
    let Some(facts) = git.stamps.get(&key) else {
        return Some(if git.publish_records.contains_key(&key) {
            UnverifiedReason::PublishRecordNotChecked
        } else {
            UnverifiedReason::NoGitFacts
        });
    };
    let Some(added) = &facts.added_in else {
        return Some(UnverifiedReason::NotCommitted);
    };
    if added.agent_trailer {
        return Some(UnverifiedReason::AgentTrailer);
    }
    if !added.after_certified {
        return Some(UnverifiedReason::NotAfterCertifiedCommit);
    }
    if facts.hash_at_commit.as_deref() != Some(b.hash.as_str()) {
        return Some(UnverifiedReason::HashMismatchAtCommit {
            at_commit: facts.hash_at_commit.clone(),
        });
    }
    let Some(reviewer) = registry.reviewer(&b.by) else {
        return Some(UnverifiedReason::UnknownReviewer);
    };
    if let Some(cut) = reviewer.revocation.as_ref().and_then(|v| v.cutoff(&b.date)) {
        return Some(match cut {
            SigReason::Compromised { .. } => UnverifiedReason::Compromised,
            _ => UnverifiedReason::Revoked,
        });
    }
    None
}

/// Judge every review and function (module doc). Pure and deterministic.
pub fn evaluate(
    reviews: &[FolderReviews],
    indexes: &[FolderIndex],
    root: &ReviewRoot,
    git: &GitFacts,
    concepts: &ConceptAreas,
    policy: SignaturePolicy,
) -> Evaluation {
    let mut current: BTreeMap<&str, Current> = BTreeMap::new();
    for idx in indexes {
        for (file, f) in idx.functions() {
            current.insert(
                f.id.as_str(),
                Current {
                    index: idx,
                    file: file.to_string(),
                    f,
                },
            );
        }
    }
    let claimed: BTreeSet<String> = reviews
        .iter()
        .flat_map(|fr| fr.doc.reviews().map(|r| r.function_id()))
        .filter(|f| current.contains_key(f.as_str()))
        .collect();

    let registry = Registry::build(root);
    let mut ev = Evaluation::default();
    let mut per_fn: BTreeMap<String, Vec<ReviewReport>> = BTreeMap::new();
    let mut deleted: BTreeMap<String, (Vec<ReviewReport>, &FolderReviews, Vec<&ReviewEntry>)> =
        BTreeMap::new();
    let mut flags: BTreeMap<String, Vec<FunctionFlag>> = BTreeMap::new();

    for fr in reviews {
        for r in fr.doc.reviews() {
            let b = &r.review;
            let fid = r.function_id();
            let mut report = ReviewReport {
                by: b.by.clone(),
                artifact: Some(r.kovan.id.clone()),
                date: Some(b.date.clone()),
                state: StampState::Valid,
                rung: Some(b.rung),
                qualifications: qualification_labels(root, &b.by),
                independent: stamp_gate(&b.checklist, Applicability::default()).independent,
            };
            let is_port = fr.doc.upstream().is_some_and(|u| u.is_port);
            // 1. Find the function.
            let (cur, mut candidates, matched) = match current.get(fid.as_str()) {
                Some(c) => (Some(c), Vec::new(), false),
                None => {
                    let cands: Vec<&Current> = current
                        .values()
                        .filter(|c| c.f.hash == b.hash && !claimed.contains(&c.f.id))
                        .collect();
                    match cands.len() {
                        0 => (None, Vec::new(), false),
                        1 => (Some(cands[0]), Vec::new(), true),
                        _ => (
                            Some(cands[0]),
                            cands.iter().map(|c| c.f.id.clone()).collect(),
                            true,
                        ),
                    }
                }
            };
            let Some(cur) = cur else {
                report.state = StampState::Deleted;
                let e = deleted
                    .entry(fid.clone())
                    .or_insert_with(|| (Vec::new(), fr, Vec::new()));
                e.0.push(report);
                e.2.push(r);
                continue;
            };
            if matched && candidates.is_empty() {
                ev.id_matches.push(IdMatch {
                    review_function: fid.clone(),
                    current_id: cur.f.id.clone(),
                });
            }
            candidates.sort();
            // Identical copies: the review shows on every candidate, each
            // flagged as duplicate code (maintainer on #765, 2026-10-07).
            let targets: Vec<&Current> = if candidates.is_empty() {
                vec![cur]
            } else {
                candidates.iter().filter_map(|c| current.get(c.as_str())).collect()
            };
            let stamp_facts = git.stamps.get(&ReviewKey { function: fid.clone(), by: b.by.clone() });
            let at_review = stamp_facts.and_then(|f| f.tests_at_review.as_ref());
            // #783: flags only, judged once per review.
            let timing = plausibility(
                b.signed_at.as_deref(),
                &b.date,
                stamp_facts.and_then(|f| f.reviewed_commit_time),
                stamp_facts.and_then(|f| f.added_in.as_ref()).and_then(|a| a.committer_time),
            );
            for t in targets {
                let here = t.location();
                let tests = test_verdict(t.f, t.index.test_run.as_ref(), git);
                let mut rep = report.clone();
                rep.state = judge(r, t, &here, &candidates, matched, tests, &current, git, &registry, policy, is_port);
                if !candidates.is_empty() {
                    flags.entry(t.f.id.clone()).or_default().push(FunctionFlag::DuplicateCode {
                        copies: candidates.iter().filter(|c| **c != t.f.id).cloned().collect(),
                    });
                }
                if !timing.is_empty() {
                    flags.entry(t.f.id.clone()).or_default().push(FunctionFlag::ImplausibleSignedAt {
                        review: r.kovan.id.clone(),
                        problems: timing.clone(),
                    });
                }
                if let Some(at) = at_review {
                    let before: BTreeSet<&String> = at.reached_by.iter().collect();
                    let new: Vec<String> = t
                        .f
                        .reached_by
                        .iter()
                        .filter(|x| !before.contains(x))
                        .cloned()
                        .collect();
                    if !new.is_empty() {
                        for test in &new {
                            ev.new_reaching_tests.push(NewReachingTest {
                                test: test.clone(),
                                function: t.f.id.clone(),
                                review: r.kovan.id.clone(),
                            });
                        }
                        flags.entry(t.f.id.clone()).or_default().push(FunctionFlag::NewReachingTests {
                            review: r.kovan.id.clone(),
                            tests: new,
                        });
                    }
                }
                per_fn.entry(t.f.id.clone()).or_default().push(rep);
            }
        }
        // Unreadable entries are no review; attach them where they point.
        for u in &fr.doc.unreadable {
            let target = u.function.as_deref().and_then(|f| {
                if current.contains_key(f) {
                    Some(f.to_string())
                } else {
                    // A path (`file.rs::item`) or a first-version `code:` link.
                    let loc = CodeTarget::parse(f)
                        .map(|t| (t.file, t.item))
                        .or_else(|| f.split_once(".rs::").map(|(a, b)| (format!("{a}.rs"), b.to_string())));
                    loc.and_then(|(file, item)| {
                        current
                            .values()
                            .find(|c| c.location().file == file && c.f.qual == item)
                            .map(|c| c.f.id.clone())
                    })
                }
            });
            match target {
                Some(id) => per_fn.entry(id).or_default().push(ReviewReport {
                    by: u.by.clone().unwrap_or_default(),
                    artifact: None,
                    date: None,
                    state: StampState::Unreadable {
                        message: u.message.clone(),
                    },
                    rung: None,
                    qualifications: Vec::new(),
                    independent: false,
                }),
                None => ev.orphan_unreadable.push(OrphanUnreadable {
                    dir: fr.dir.clone(),
                    heading: u.heading.clone(),
                    line: u.line,
                    message: u.message.clone(),
                }),
            }
        }
    }

    // Open needs-fix notes, by current function id.
    let mut fixes: BTreeMap<String, StampState> = BTreeMap::new();
    for fr in reviews {
        for n in fr.doc.needs_fixes() {
            let b = &n.needs_fix;
            if b.status != FixStatus::Open {
                continue;
            }
            let nid = n.function_id();
            let id = ev
                .id_matches
                .iter()
                .find(|m| m.review_function == nid)
                .map(|m| m.current_id.clone())
                .unwrap_or(nid);
            let Some(cur) = current.get(id.as_str()) else {
                continue;
            };
            let st = if cur.f.hash == b.hash {
                StampState::NeedsFixOpen {
                    entry: n.kovan.id.clone(),
                    note: b.note.clone(),
                }
            } else {
                StampState::Fixed {
                    entry: n.kovan.id.clone(),
                    note: b.note.clone(),
                }
            };
            // An open needs-fix on unchanged code outranks a fixed one.
            let keep = matches!(fixes.get(&id), Some(StampState::NeedsFixOpen { .. }));
            if !keep {
                fixes.insert(id, st);
            }
        }
    }

    for (id, cur) in &current {
        let mut reviews = per_fn.remove(*id).unwrap_or_default();
        reviews.sort_by(|a, b| (&a.by, &a.artifact).cmp(&(&b.by, &b.artifact)));
        let rung = function_rung(id, &reviews, git, root, concepts);
        let state = match fixes.remove(*id) {
            Some(s) => s,
            None => aggregate(&reviews),
        };
        ev.functions.insert(
            id.to_string(),
            FunctionReport {
                id: id.to_string(),
                location: Some(cur.location()),
                rung: if state.kind().counts() { rung } else { None },
                state,
                reviews,
                untested: !cur.f.test && cur.f.reached_by.is_empty(),
                blocked_by: Vec::new(),
                flags: flags.remove(*id).unwrap_or_default(),
            },
        );
    }
    // Bottom-up: callees whose state does not count.
    let counts: BTreeMap<String, bool> = ev
        .functions
        .iter()
        .map(|(k, v)| (k.clone(), v.state.kind().counts()))
        .collect();
    for (id, cur) in &current {
        let blocked: Vec<String> = cur
            .f
            .callees
            .iter()
            .filter(|c| c.as_str() != *id && counts.get(*c) == Some(&false))
            .cloned()
            .collect();
        if let Some(f) = ev.functions.get_mut(*id) {
            f.blocked_by = blocked;
        }
    }

    for (function, (reports, fr, entries)) in deleted {
        let row = history_row(&function, &entries, git);
        ev.history.push(HistoryRow {
            placement: placement(fr, indexes),
            row,
        });
        ev.deleted.push(FunctionReport {
            id: function,
            location: None,
            state: StampState::Deleted,
            reviews: reports,
            rung: None,
            untested: false,
            blocked_by: Vec::new(),
            flags: Vec::new(),
        });
    }
    ev.upstream_tags = upstream_tags(reviews, git);
    ev.history_warnings = git
        .previous_root
        .as_ref()
        .map(|p| history_append_only(p, root))
        .unwrap_or_default();
    ev.id_matches.sort();
    ev.new_reaching_tests.sort();
    ev.new_reaching_tests.dedup();
    ev
}

fn upstream_tags(reviews: &[FolderReviews], git: &GitFacts) -> Vec<UpstreamTagReport> {
    let mut out = Vec::new();
    let mut push = |dir: &str, arch: Option<String>, repo: Option<&String>, tag: Option<&String>, commit: Option<&String>| {
        if let (Some(tag), Some(commit)) = (tag, commit) {
            let now = repo.and_then(|r| git.tag_commits.get(&(r.clone(), tag.clone())));
            out.push(UpstreamTagReport {
                dir: dir.to_string(),
                architecture: arch,
                repository: repo.cloned(),
                tag: tag.clone(),
                commit: commit.clone(),
                check: check_tag(commit, now.map(String::as_str)),
            });
        }
    };
    for fr in reviews {
        if let Some(u) = fr.doc.upstream() {
            push(&fr.dir, None, u.repository.as_ref(), u.tag.as_ref(), u.commit.as_ref());
        }
        for a in fr.doc.architectures() {
            let b = &a.architecture;
            if let Some(u) = &b.upstream {
                push(&fr.dir, Some(a.kovan.id.clone()), u.repository.as_ref(), b.upstream_tag.as_ref(), u.commit.as_ref());
            }
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn judge(
    r: &ReviewEntry,
    cur: &Current,
    here: &Location,
    candidates: &[String],
    matched: bool,
    tests: TestVerdict,
    current: &BTreeMap<&str, Current>,
    git: &GitFacts,
    registry: &Registry,
    policy: SignaturePolicy,
    is_port: bool,
) -> StampState {
    let b = &r.review;
    // 2. Authenticity.
    match authenticity(r, git, registry, policy) {
        Some(Denial::Unverified(why)) => return StampState::Unverified(why),
        Some(Denial::OutsideScope) => return StampState::OutsideScope,
        None => {}
    }
    // 3. Scope, on the actual location (the registry's role and scope).
    if let Some(rv) = registry.reviewer(&b.by) {
        if rv.role == Role::Reviewer && !in_scope(&rv.scope, &here.file) {
            return StampState::OutsideScope;
        }
    }
    // 3b. The derived rung (Leak Before Break): a recorded rung that the
    // answers and git do not give is invalid, shown and never counted.
    // The gate and the rung are judged as of the review commit.
    let tests_by = git
        .stamps
        .get(&ReviewKey { function: r.function_id(), by: b.by.clone() })
        .and_then(|f| f.tests_at_review.as_ref())
        .map(TestsAtReview::authorship)
        .unwrap_or_default();
    let ctx = Applicability {
        is_port,
        physical_interface: b.checklist.contains_key("units_documented"),
        tests: tests_by,
    };
    let gate = stamp_gate(&b.checklist, ctx);
    if !gate.stampable() {
        return StampState::Invalid(InvalidReason::GateBlocked(gate.blocked_by));
    }
    let derived = gate.rung.as_u8();
    if b.rung != derived {
        return StampState::Invalid(InvalidReason::RungMismatch {
            recorded: b.rung,
            derived,
            tests: tests_by,
        });
    }
    // 4. Own code.
    if cur.f.hash != b.hash {
        return StampState::DirectlyStale(StaleWhy::CodeChanged {
            reviewed: b.hash.clone(),
            now: cur.f.hash.clone(),
        });
    }
    let recorded: BTreeSet<&String> = b.callees.keys().collect();
    let now: BTreeSet<&String> = cur.f.callees.iter().collect();
    if recorded != now {
        return StampState::DirectlyStale(StaleWhy::CalleesResolveDifferently {
            added: now.difference(&recorded).map(|s| (*s).clone()).collect(),
            removed: recorded.difference(&now).map(|s| (*s).clone()).collect(),
        });
    }
    // 5. Location.
    let reviewed_at = r.path().and_then(|p| {
        p.split_once(".rs::").map(|(f, q)| Location {
            file: format!("{f}.rs"),
            qual: q.to_string(),
        })
    });
    let elsewhere = reviewed_at
        .as_ref()
        .is_some_and(|l| l.file != here.file || l.qual != here.qual);
    if matched || elsewhere || !candidates.is_empty() {
        return StampState::Moved {
            from: reviewed_at,
            to: here.clone(),
            tests,
            candidates: candidates.to_vec(),
        };
    }
    // 6. Callees (one level).
    let changed: Vec<String> = b
        .callees
        .iter()
        .filter(|(id, h)| current.get(id.as_str()).map(|c| &c.f.hash) != Some(*h))
        .map(|(id, _)| id.clone())
        .collect();
    if !changed.is_empty() {
        let blocked = !tests.allows_reconfirm();
        return StampState::InheritedStale {
            cause: InheritedCause::Callees(changed),
            tests,
            blocked,
        };
    }
    // 7. Doc.
    if cur.f.doc_hash != b.doc_hash {
        return StampState::DocChanged;
    }
    // 8. Cargo.lock.
    if let Some(lock) = &b.cargo_lock {
        if *lock != git.cargo_lock {
            return match tests {
                TestVerdict::Reach(ReachVerdict::Failed { .. }) => StampState::InheritedStale {
                    cause: InheritedCause::LockTestFailed,
                    tests,
                    blocked: true,
                },
                _ if full_run_at_current_lock(cur.index.test_run.as_ref(), git)
                    && unchanged_since_run(cur.f, git) =>
                {
                    StampState::Valid
                }
                _ => StampState::PendingWorkspaceTest {
                    reviewed_lock: lock.clone(),
                    current_lock: git.cargo_lock.clone(),
                },
            };
        }
    }
    StampState::Valid
}

fn full_run_at_current_lock(run: Option<&TestRun>, git: &GitFacts) -> bool {
    run.is_some_and(|r| r.suite == Suite::Full && r.cargo_lock == git.cargo_lock)
}

fn aggregate(reviews: &[ReviewReport]) -> StampState {
    if let Some(v) = reviews.iter().find(|r| r.state.kind() == StateKind::Valid) {
        return v.state.clone();
    }
    for k in AGGREGATE_ORDER {
        if let Some(r) = reviews.iter().find(|r| r.state.kind() == k) {
            return r.state.clone();
        }
    }
    StampState::New
}

fn qualification_labels(root: &ReviewRoot, by: &str) -> Vec<String> {
    root.reviewer(by)
        .map(|r| r.qualification.iter().map(|q| q.label()).collect())
        .unwrap_or_default()
}

/// Whether `by` holds qualifications covering every concept area of `id`.
fn qualified_for(root: &ReviewRoot, by: &str, id: &str, concepts: &ConceptAreas) -> bool {
    let Some(areas) = concepts.get(id).filter(|a| !a.is_empty()) else {
        return false;
    };
    let Some(r) = root.reviewer(by) else {
        return false;
    };
    areas
        .iter()
        .all(|c| r.qualification.iter().any(|q| q.qualifies_for(c)))
}

fn function_rung(
    id: &str,
    reviews: &[ReviewReport],
    git: &GitFacts,
    root: &ReviewRoot,
    concepts: &ConceptAreas,
) -> Option<u8> {
    let mut valid: Vec<&ReviewReport> = reviews
        .iter()
        .filter(|r| r.state.kind() == StateKind::Valid)
        .collect();
    if valid.is_empty() {
        return None;
    }
    valid.sort_by(|a, b| (&a.date, &a.by).cmp(&(&b.date, &b.by)));
    let first = valid[0];
    let no_authors = BTreeSet::new();
    let authors = git.code_authors.get(id).unwrap_or(&no_authors);
    let independent = valid[1..]
        .iter()
        .any(|r| {
            r.independent
                && r.by != first.by
                && !authors.contains(&r.by)
                && qualified_for(root, &r.by, id, concepts)
        });
    if independent {
        return Some(5);
    }
    valid.iter().filter_map(|r| r.rung).max()
}

fn history_row(function: &str, entries: &[&ReviewEntry], git: &GitFacts) -> DeletedFunction {
    let latest = entries
        .iter()
        .max_by(|a, b| (&a.review.date, &a.review.commit).cmp(&(&b.review.date, &b.review.commit)));
    let path = latest
        .and_then(|r| r.path())
        .unwrap_or_else(|| function.to_string());
    let mut reviewers: Vec<String> = entries.iter().map(|r| r.review.by.clone()).collect();
    reviewers.sort();
    reviewers.dedup();
    DeletedFunction {
        function: function.to_string(),
        path,
        deleted_commit: git.deleted_in.get(function).cloned(),
        branch: None,
        last_review_commit: latest.map(|r| r.review.commit.clone()).unwrap_or_default(),
        reviewers,
    }
}

fn placement(fr: &FolderReviews, indexes: &[FolderIndex]) -> HistoryPlacement {
    if indexes.iter().any(|i| i.dir == fr.dir) {
        return HistoryPlacement::FolderReviewMd { dir: fr.dir.clone() };
    }
    let crate_folders: Vec<&FolderIndex> = indexes.iter().filter(|i| i.krate == fr.krate).collect();
    let root = crate_folders
        .iter()
        .find(|i| i.crate_root)
        .or_else(|| crate_folders.iter().min_by_key(|i| i.dir.len()));
    match root {
        Some(i) => HistoryPlacement::CrateIndex {
            krate: fr.krate.clone(),
            dir: i.dir.clone(),
            deleted_folder: fr.dir.clone(),
        },
        None => HistoryPlacement::WorkspaceRoot {
            krate: fr.krate.clone(),
            dir: fr.dir.clone(),
        },
    }
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
