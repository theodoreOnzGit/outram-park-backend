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
//!    file in scope, else **outside scope**.
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
//! **Rungs.** A review's rung 4 counts only when the wizard's gate opens it
//! ([`effective_rung`]); otherwise it counts as rung 3 and is flagged. A
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
//! **Test evidence** is the folder's `[test_run]` ([`TestVerdict`]): only a
//! full-suite run at the current `Cargo.lock` that the caller says is
//! current counts, and tests edited in the change never count.

use std::collections::{BTreeMap, BTreeSet};

use crate::artifact::relation::CodeTarget;

use super::index::{FolderIndex, FunctionIndex, Suite, TestRun};
use super::review_md::{DeletedFunction, FixStatus, ReviewDocument, ReviewEntry};
use super::root::{ReviewRoot, Role};
use super::scope::in_scope;
use super::signing::registry::Registry;
use super::signing::{verify_review, SignatureCheck, UnverifiedReason as SigReason};
use super::wizard::{stamp_gate, Applicability};
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
}

/// What git says about one stamp (computed by the caller).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StampFacts {
    /// The function's `hash` recomputed at the certified commit; `None` when
    /// it could not be found there.
    pub hash_at_commit: Option<String>,
    /// `None` when the stamp is not committed yet.
    pub added_in: Option<StampCommit>,
}

/// Git, as data.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitFacts {
    pub head: String,
    /// `sha256:` hash of the current `Cargo.lock`.
    pub cargo_lock: String,
    /// No code changed between the `[test_run]` commit and now (the caller
    /// checks; kovan's own files do not count as code).
    pub test_run_current: bool,
    pub stamps: BTreeMap<ReviewKey, StampFacts>,
    /// Function id -> the reviewer ids of the people who wrote its code.
    pub code_authors: BTreeMap<String, BTreeSet<String>>,
    /// Deleted function id -> the commit that deleted it.
    pub deleted_in: BTreeMap<String, String>,
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
    /// No test reaches it (or only tests edited in the change do).
    NoTests { edited_only: bool },
    /// Every counting reaching test passed.
    Passed { tests: usize },
    /// These counting reaching tests failed.
    Failing(Vec<String>),
    /// No usable evidence yet.
    Pending(EvidenceGap),
}

/// Why the test evidence does not count yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceGap {
    NoTestRun,
    NotFullSuite,
    /// Code changed since the run.
    NotCurrent,
    /// The run was at a different `Cargo.lock`.
    OtherLock,
    /// These reaching tests are in neither the passed nor the failed list.
    NotRun(Vec<String>),
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
        }
    }
}

/// When no review is valid, the function shows the first of these its
/// reviews have.
pub const AGGREGATE_ORDER: [StateKind; 10] = [
    StateKind::DirectlyStale,
    StateKind::InheritedStale,
    StateKind::Moved,
    StateKind::DocChanged,
    StateKind::PendingWorkspaceTest,
    StateKind::Unverified,
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
    /// The rung it counts at (3 or 4), when readable.
    pub rung: Option<u8>,
    /// Recorded rung 4 without a qualifying Q8 answer: counted as 3.
    pub rung_capped: bool,
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

/// The reaching-test verdict of `f` from its folder's `test_run`.
pub fn test_verdict(f: &FunctionIndex, run: Option<&TestRun>, git: &GitFacts) -> TestVerdict {
    let edited: BTreeSet<&String> = run.map(|r| r.edited.iter().collect()).unwrap_or_default();
    let counting: Vec<&String> = f.reached_by.iter().filter(|t| !edited.contains(t)).collect();
    if counting.is_empty() {
        return TestVerdict::NoTests {
            edited_only: !f.reached_by.is_empty(),
        };
    }
    let Some(run) = run else {
        return TestVerdict::Pending(EvidenceGap::NoTestRun);
    };
    if run.suite != Suite::Full {
        return TestVerdict::Pending(EvidenceGap::NotFullSuite);
    }
    if !git.test_run_current {
        return TestVerdict::Pending(EvidenceGap::NotCurrent);
    }
    if run.cargo_lock != git.cargo_lock {
        return TestVerdict::Pending(EvidenceGap::OtherLock);
    }
    let failing: Vec<String> = counting
        .iter()
        .filter(|t| run.failed.contains(t))
        .map(|t| (*t).clone())
        .collect();
    if !failing.is_empty() {
        return TestVerdict::Failing(failing);
    }
    let not_run: Vec<String> = counting
        .iter()
        .filter(|t| !run.passed.contains(t))
        .map(|t| (*t).clone())
        .collect();
    if !not_run.is_empty() {
        return TestVerdict::Pending(EvidenceGap::NotRun(not_run));
    }
    TestVerdict::Passed {
        tests: counting.len(),
    }
}

fn authenticity(
    r: &ReviewEntry,
    root: &ReviewRoot,
    git: &GitFacts,
    registry: Option<&Registry>,
) -> Option<Denial> {
    authenticity_git(r, root, git)
        .map(Denial::Unverified)
        .or_else(|| registry.and_then(|reg| signature(r, reg)))
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

fn authenticity_git(r: &ReviewEntry, root: &ReviewRoot, git: &GitFacts) -> Option<UnverifiedReason> {
    let b = &r.review;
    let key = ReviewKey {
        function: r.function_id(),
        by: b.by.clone(),
    };
    let Some(facts) = git.stamps.get(&key) else {
        return Some(UnverifiedReason::NoGitFacts);
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
    let Some(reviewer) = root.reviewer(&b.by) else {
        return Some(UnverifiedReason::UnknownReviewer);
    };
    if let Some(rev) = &reviewer.revoked {
        if rev.compromised_from.as_deref().is_some_and(|c| b.date.as_str() >= c) {
            return Some(UnverifiedReason::Compromised);
        }
        if b.date >= rev.date {
            return Some(UnverifiedReason::Revoked);
        }
    }
    None
}

/// The rung a review counts at, and whether rung 4 was capped to 3: rung 4
/// counts only when the wizard's gate opens it (#769 `stamp_gate`:
/// `vv_evidence` is a reference/code-to-code or analytical comparison and
/// no answer closes it). ~~Gated on checklist `q8`~~ **CORRECTED
/// 2026-10-07**: `q8` was #764's placeholder; the wizard refuses it, so a
/// review that still carries it counts at rung 3.
pub fn effective_rung(r: &ReviewEntry) -> (u8, bool) {
    let b = &r.review;
    if b.rung == 4 {
        if stamp_gate(&b.checklist, Applicability::default()).rung4_allowed {
            (4, false)
        } else {
            (3, true)
        }
    } else {
        (b.rung, false)
    }
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

    let registry = (policy == SignaturePolicy::Enforce).then(|| Registry::build(root));
    let mut ev = Evaluation::default();
    let mut per_fn: BTreeMap<String, Vec<ReviewReport>> = BTreeMap::new();
    let mut deleted: BTreeMap<String, (Vec<ReviewReport>, &FolderReviews, Vec<&ReviewEntry>)> =
        BTreeMap::new();

    for fr in reviews {
        for r in fr.doc.reviews() {
            let b = &r.review;
            let fid = r.function_id();
            let (rung, capped) = effective_rung(r);
            let mut report = ReviewReport {
                by: b.by.clone(),
                artifact: Some(r.kovan.id.clone()),
                date: Some(b.date.clone()),
                state: StampState::Valid,
                rung: Some(rung),
                rung_capped: capped,
                qualifications: qualification_labels(root, &b.by),
                independent: stamp_gate(&b.checklist, Applicability::default()).independent,
            };
            // 1. Find the function.
            let (cur, candidates, matched) = match current.get(fid.as_str()) {
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
            let here = cur.location();
            let tests = test_verdict(cur.f, cur.index.test_run.as_ref(), git);
            report.state = judge(r, cur, &here, &candidates, matched, tests, &current, root, git, registry.as_ref());
            per_fn.entry(cur.f.id.clone()).or_default().push(report);
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
                    rung_capped: false,
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
        });
    }
    ev.upstream_tags = upstream_tags(reviews, git);
    ev.id_matches.sort();
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
    root: &ReviewRoot,
    git: &GitFacts,
    registry: Option<&Registry>,
) -> StampState {
    let b = &r.review;
    // 2. Authenticity.
    match authenticity(r, root, git, registry) {
        Some(Denial::Unverified(why)) => return StampState::Unverified(why),
        Some(Denial::OutsideScope) => return StampState::OutsideScope,
        None => {}
    }
    // 3. Scope.
    if let Some(rv) = root.reviewer(&b.by) {
        if rv.role == Role::Reviewer && !in_scope(&rv.scope, &here.file) {
            return StampState::OutsideScope;
        }
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
        let blocked = !matches!(tests, TestVerdict::Passed { .. } | TestVerdict::NoTests { .. });
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
                TestVerdict::Failing(_) => StampState::InheritedStale {
                    cause: InheritedCause::LockTestFailed,
                    tests,
                    blocked: true,
                },
                _ if full_run_at_current_lock(cur.index.test_run.as_ref(), git) => StampState::Valid,
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
    run.is_some_and(|r| r.suite == Suite::Full && r.cargo_lock == git.cargo_lock && git.test_run_current)
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
