//! **Rung 5: independent verification and validation (IV&V)** (GitHub
//! #809; maintainer decisions, 2026-10-08).
//!
//! # The definition
//!
//! NUREG/BR-0167 (1993), §3.1 p. 6, read from the standard-corpus PDF
//! (`nureg-br-0167`):
//!
//! > Independent verification and validation (IV&V) is verification and
//! > validation by an organization that is both technically and managerially
//! > separate from the organization responsible for developing the software.
//!
//! and the glossary (Appendix B, p. 55): "Verification and validation by an
//! organization that is both technically and managerially separate from the
//! organization responsible for developing the software." BR-0167 treats
//! IV&V as optional and is written for NRC staff software, not plant
//! software, so Kovan uses it by analogy.
//!
//! ~~Rung 5 is a second valid review by a different reviewer who is not a
//! code author, answered `independence = someone_else` and is qualified in
//! every concept area of the function~~ **CORRECTED 2026-10-08** (#809):
//! that was an independent *review* by an *individual*, not IV&V by a
//! separate *organisation*. A second review stamp alone no longer reaches
//! rung 5.
//!
//! # What rung 5 needs now
//!
//! A function is at rung 5 when one of its valid reviews (a *candidate*)
//! meets **all** of these, each judged at the review's `date`:
//!
//! 1. **An independent V&V case**: the review derives rung 4 on its own (a
//!    qualifying `vv_evidence` answer, `vv_case_author =
//!    human_wrote_and_verified`, and git showing no agent trailer on the
//!    reaching tests' commits at the review commit): the same human-only
//!    gate as rung 4 ([`vv_case_problems`]). The ladder only adds
//!    requirements going up (maintainer decision 2).
//! 2. **Qualified** in every concept area of the function (enforced as
//!    before; decision 1). A function with no known concept area cannot
//!    reach rung 5.
//! 3. **Not the code's author** (from git) and **not the first reviewer**
//!    (the earliest valid review's reviewer), and its `independence` answer
//!    is not `not_independent`.
//! 4. **A different organisation.** The reviewer's
//!    `[[reviewer.organisation]]` in force at the review date differs from
//!    the developing organisation in force for the function's crate
//!    (`[[code_review.developing_organisation]]`, a per-crate entry
//!    overriding the workspace one; decision 3). Both are maintainer-signed,
//!    append-only registry records. Names are compared after trimming,
//!    collapsing white space and ignoring case ([`same_organisation`]);
//!    anything else is the signing maintainer's spelling.
//! 5. **A valid signed separation attestation.** The review names one
//!    (`[review] separation_attestation`, signed into the stamp) of the
//!    reviewer's `[[reviewer.separation]]` entries; it verifies against the
//!    reviewer's own trusted key (no countersignature), is dated on or
//!    before the review, names the same two organisations, and names a
//!    public GitHub issue as its audit record, open or closed
//!    ([`parse_audit_record`]).
//!
//! # Leak Before Break
//!
//! Every reason a candidate misses is a [`Rung5Miss`], kept on the
//! function's [`IndependentVv`] report; nothing is silently downgraded. A
//! review that names an attestation and still misses also raises the
//! function flag "independent V&V not counted"
//! ([`super::engine::FunctionFlag::IndependentVvNotCounted`]). A
//! registry record (organisation or attestation) that does not verify is
//! listed in [`super::engine::Evaluation::ivv_warnings`] even when no review
//! relies on it.
//!
//! **What kovan cannot check.** The audit record's issue is shown, never
//! fetched: kovan is offline, so its existence and content are not verified,
//! and it is labelled [`AUDIT_RECORD_LABEL`]. Whether the two organisations
//! really are separate is the reviewer's signed attestation, not a fact
//! kovan derives.
//!
//! **Signature policy.** With [`SignaturePolicy::NotChecked`] the
//! organisation and attestation signatures are not checked either (as for
//! stamps); every other rule above still applies.
//!
//! No migration: rungs are derived, never stored, and no stamp claimed the
//! old rung 5 (maintainer decision 4).

use std::collections::{BTreeMap, BTreeSet};

use super::engine::{ConceptAreas, ReviewReport, SignaturePolicy};
use super::root::{KeySigner, ReviewRoot, SeparationAttestation};
use super::signing::registry::{find_signer, maintainers, Registry, SignerProblem};
use super::signing::{
    developing_organisation_bytes, is_date, reviewer_organisation_bytes,
    separation_attestation_bytes,
};
use super::state::StateKind;
use super::wizard::{parse_answer, Effect, ReviewWizard, TestAuthorship};

/// How the audit record is labelled wherever it is shown: kovan checks the
/// URL's form only.
pub const AUDIT_RECORD_LABEL: &str = "audit record (not verified by kovan)";

/// A public GitHub issue named as an attestation's audit record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRecord {
    pub url: String,
    pub owner: String,
    pub repo: String,
    pub number: u64,
}

impl AuditRecord {
    /// The link as it is shown: `audit record (not verified by kovan): <url>`.
    pub fn shown(&self) -> String {
        format!("{AUDIT_RECORD_LABEL}: {}", self.url)
    }
}

/// Why an audit record is not a GitHub issue URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditRecordProblem {
    /// The attestation names none.
    Missing,
    /// Not of the form `https://github.com/<owner>/<repo>/issues/<n>`
    /// (another host, `http`, a pull request, a trailing path, a query or a
    /// fragment, white space).
    NotAnIssueUrl(String),
    /// The owner is not a GitHub user or organisation name (1 to 39 ASCII
    /// letters, digits or hyphens, not starting or ending with a hyphen).
    BadOwner(String),
    /// The repository is not a GitHub repository name (1 to 100 ASCII
    /// letters, digits, `.`, `_` or `-`; not `.` or `..`).
    BadRepo(String),
    /// The issue number is not a positive decimal integer without a leading
    /// zero.
    BadNumber(String),
}

/// Check that `url` is `https://github.com/<owner>/<repo>/issues/<n>`.
pub fn parse_audit_record(url: &str) -> Result<AuditRecord, AuditRecordProblem> {
    let not_issue = || AuditRecordProblem::NotAnIssueUrl(url.to_string());
    let rest = url
        .strip_prefix("https://github.com/")
        .ok_or_else(not_issue)?;
    let parts: Vec<&str> = rest.split('/').collect();
    let [owner, repo, issues, n] = parts.as_slice() else {
        return Err(not_issue());
    };
    if *issues != "issues" {
        return Err(not_issue());
    }
    let owner_ok = (1..=39).contains(&owner.len())
        && owner
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && !owner.starts_with('-')
        && !owner.ends_with('-');
    if !owner_ok {
        return Err(AuditRecordProblem::BadOwner(owner.to_string()));
    }
    let repo_ok = (1..=100).contains(&repo.len())
        && repo
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        && *repo != "."
        && *repo != "..";
    if !repo_ok {
        return Err(AuditRecordProblem::BadRepo(repo.to_string()));
    }
    let number = (!n.is_empty() && !n.starts_with('0') && n.bytes().all(|b| b.is_ascii_digit()))
        .then(|| n.parse::<u64>().ok())
        .flatten()
        .ok_or_else(|| AuditRecordProblem::BadNumber(n.to_string()))?;
    Ok(AuditRecord {
        url: url.to_string(),
        owner: owner.to_string(),
        repo: repo.to_string(),
        number,
    })
}

/// Why a review's V&V case is not an independent, hand-written one (rung
/// 5's condition 1; the rung-4 gate, [`super::wizard::derived_rung`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VvCaseProblem {
    /// `vv_evidence` is not code-to-code, analytical or convergence.
    NoQualifyingEvidence,
    /// `vv_case_author` is not `human_wrote_and_verified`.
    NotWrittenByHand,
    /// Git shows the agent trailer on a commit that added a reaching test
    /// (an AI-authored V&V case).
    TestsByAgent,
    /// Git's view of the reaching tests' authorship is not known (no
    /// reaching test, or no commit facts).
    TestAuthorshipUnknown,
}

/// Every reason the answers and git do not give a hand-written V&V case;
/// empty exactly when the review derives rung 4.
pub fn vv_case_problems(
    answers: &BTreeMap<String, String>,
    tests: TestAuthorship,
) -> Vec<VvCaseProblem> {
    let w = ReviewWizard::embedded();
    let effect = |effect: Effect| {
        answers.iter().any(|(q, raw)| {
            w.question(q)
                .and_then(|question| question.option(parse_answer(raw).0))
                .is_some_and(|o| o.effect == effect)
        })
    };
    let mut out = Vec::new();
    if !effect(Effect::GateRung4) {
        out.push(VvCaseProblem::NoQualifyingEvidence);
    }
    if !effect(Effect::GateRung4Author) {
        out.push(VvCaseProblem::NotWrittenByHand);
    }
    match tests {
        TestAuthorship::Human => {}
        TestAuthorship::Agent => out.push(VvCaseProblem::TestsByAgent),
        TestAuthorship::Unknown => out.push(VvCaseProblem::TestAuthorshipUnknown),
    }
    out
}

/// Organisation names compare equal after trimming, collapsing white space
/// and ignoring case.
pub fn same_organisation(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        s.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    };
    norm(a) == norm(b)
}

/// Which record gave an organisation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrganisationScope {
    /// The workspace-wide `[[code_review.developing_organisation]]`.
    Workspace,
    /// A per-crate override (`crate = "<name>"`).
    Crate(String),
    /// A reviewer's `[[reviewer.organisation]]`.
    Reviewer(String),
}

/// The organisation record in force at a date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganisationInForce {
    pub name: String,
    /// The record's own date.
    pub date: String,
    pub scope: OrganisationScope,
    /// The maintainer (reviewer id, key id) whose signature verified;
    /// `None` when signatures are not checked.
    pub signed_by: Option<(String, String)>,
}

/// Why no organisation is in force.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrganisationProblem {
    /// No record dated on or before the date.
    NotRecorded,
    /// The record in force has a date that is not `YYYY-MM-DD`.
    BadDate(String),
    /// The record in force is unsigned or its signature does not count.
    Unverified {
        date: String,
        problem: SignerProblem,
    },
}

/// The last of `entries` dated on or before `as_of`. An entry whose date is
/// malformed is never skipped: it is taken as in force, and fails there
/// (Leak Before Break).
fn in_force<'e, T>(
    entries: impl Iterator<Item = &'e T>,
    date: impl Fn(&T) -> &str,
    as_of: &str,
) -> Option<&'e T> {
    entries
        .filter(|e| !is_date(date(e)) || date(e) <= as_of)
        .last()
}

/// Check a maintainer's signature over `msg`, dated `date`.
fn maintainer_signature(
    registry: &Registry,
    signer: Option<&KeySigner>,
    signature: Option<&str>,
    msg: &[u8],
    date: &str,
) -> Result<(String, String), SignerProblem> {
    let (Some(sg), Some(sig)) = (signer, signature) else {
        return Err(SignerProblem::Unsigned);
    };
    let eligible = maintainers(&registry.reviewers, None);
    find_signer(
        &registry.reviewers,
        &eligible,
        sg,
        sig,
        msg,
        Some(date),
        date,
    )
}

/// Judge a record's date and signature into an [`OrganisationInForce`].
#[allow(clippy::too_many_arguments)]
fn judged(
    name: &str,
    date: &str,
    scope: OrganisationScope,
    signer: Option<&KeySigner>,
    signature: Option<&str>,
    msg: &[u8],
    registry: &Registry,
    policy: SignaturePolicy,
) -> Result<OrganisationInForce, OrganisationProblem> {
    if !is_date(date) {
        return Err(OrganisationProblem::BadDate(date.to_string()));
    }
    let signed_by = match policy {
        SignaturePolicy::NotChecked => None,
        SignaturePolicy::Enforce => Some(
            maintainer_signature(registry, signer, signature, msg, date).map_err(|problem| {
                OrganisationProblem::Unverified {
                    date: date.to_string(),
                    problem,
                }
            })?,
        ),
    };
    Ok(OrganisationInForce {
        name: name.to_string(),
        date: date.to_string(),
        scope,
        signed_by,
    })
}

/// Workspace-wide, or the crate a developing-organisation entry overrides.
fn dev_scope(e: &super::root::DevelopingOrganisation) -> OrganisationScope {
    match &e.krate {
        Some(k) => OrganisationScope::Crate(k.clone()),
        None => OrganisationScope::Workspace,
    }
}

/// The developing organisation in force for `krate` at `as_of`: the last
/// override for `krate` dated on or before it, else the last workspace-wide
/// entry.
pub fn developing_organisation(
    root: &ReviewRoot,
    registry: &Registry,
    krate: &str,
    as_of: &str,
    policy: SignaturePolicy,
) -> Result<OrganisationInForce, OrganisationProblem> {
    let all = root
        .code_review
        .as_ref()
        .map(|c| c.developing_organisation.as_slice())
        .unwrap_or(&[]);
    let override_ = in_force(
        all.iter().filter(|e| e.krate.as_deref() == Some(krate)),
        |e| &e.date,
        as_of,
    );
    let e = override_
        .or_else(|| in_force(all.iter().filter(|e| e.krate.is_none()), |e| &e.date, as_of))
        .ok_or(OrganisationProblem::NotRecorded)?;
    let scope = dev_scope(e);
    let msg = developing_organisation_bytes(e);
    judged(
        &e.name,
        &e.date,
        scope,
        e.signer.as_ref(),
        e.signature.as_deref(),
        &msg,
        registry,
        policy,
    )
}

/// The organisation `reviewer` belongs to at `as_of`.
pub fn reviewer_organisation(
    root: &ReviewRoot,
    registry: &Registry,
    reviewer: &str,
    as_of: &str,
    policy: SignaturePolicy,
) -> Result<OrganisationInForce, OrganisationProblem> {
    let r = root
        .reviewer(reviewer)
        .ok_or(OrganisationProblem::NotRecorded)?;
    let e = in_force(r.organisations.iter(), |e| &e.date, as_of)
        .ok_or(OrganisationProblem::NotRecorded)?;
    let msg = reviewer_organisation_bytes(reviewer, e);
    let scope = OrganisationScope::Reviewer(reviewer.to_string());
    judged(
        &e.name,
        &e.date,
        scope,
        e.signer.as_ref(),
        e.signature.as_deref(),
        &msg,
        registry,
        policy,
    )
}

/// Why a separation attestation does not count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttestationProblem {
    /// The reviewer has no `[[reviewer.separation]]` with this id.
    NotFound { id: String },
    /// More than one has this id: which one the stamp means is unknown.
    Ambiguous { id: String },
    /// Its date is not `YYYY-MM-DD`.
    BadDate(String),
    /// Unsigned, or its signature does not verify against one of the
    /// reviewer's own trusted, active keys.
    Unverified(SignerProblem),
    /// Dated after the review that relies on it.
    DatedAfterReview { attested: String, reviewed: String },
    /// It names organisations other than those in force at the review.
    OtherOrganisations {
        organisation: String,
        developing_organisation: String,
    },
    /// Its audit record is missing or is not a GitHub issue URL.
    AuditRecord(AuditRecordProblem),
}

/// One attestation, judged on its own (lookup, date, signature, audit
/// record); the review-dependent checks are [`judge_function`]'s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttestationCheck {
    pub attestation: Option<SeparationAttestation>,
    pub audit_record: Option<AuditRecord>,
    pub problems: Vec<AttestationProblem>,
}

/// Look up `reviewer`'s attestation `id` and check it on its own.
pub fn check_attestation(
    root: &ReviewRoot,
    registry: &Registry,
    reviewer: &str,
    id: &str,
    policy: SignaturePolicy,
) -> AttestationCheck {
    let found: Vec<&SeparationAttestation> = root
        .reviewer(reviewer)
        .map(|r| r.separations.iter().filter(|a| a.id == id).collect())
        .unwrap_or_default();
    let a = match found.as_slice() {
        [] => {
            return AttestationCheck {
                attestation: None,
                audit_record: None,
                problems: vec![AttestationProblem::NotFound { id: id.to_string() }],
            }
        }
        [a] => *a,
        _ => {
            return AttestationCheck {
                attestation: None,
                audit_record: None,
                problems: vec![AttestationProblem::Ambiguous { id: id.to_string() }],
            }
        }
    };
    let mut problems = attestation_problems(registry, reviewer, a, policy);
    let audit_record = match a.audit_record.as_deref() {
        None => {
            problems.push(AttestationProblem::AuditRecord(AuditRecordProblem::Missing));
            None
        }
        Some(url) => match parse_audit_record(url) {
            Ok(r) => Some(r),
            Err(p) => {
                problems.push(AttestationProblem::AuditRecord(p));
                None
            }
        },
    };
    AttestationCheck {
        attestation: Some(a.clone()),
        audit_record,
        problems,
    }
}

/// An attestation's date and signature (the reviewer's own trusted key,
/// active at its date, the reviewer not revoked by then).
fn attestation_problems(
    registry: &Registry,
    reviewer: &str,
    a: &SeparationAttestation,
    policy: SignaturePolicy,
) -> Vec<AttestationProblem> {
    if !is_date(&a.date) {
        return vec![AttestationProblem::BadDate(a.date.clone())];
    }
    if policy == SignaturePolicy::NotChecked {
        return Vec::new();
    }
    let checked = match (
        &a.key,
        &a.signature,
        registry.reviewers.iter().position(|r| r.id == reviewer),
    ) {
        (Some(key), Some(sig), Some(i)) => {
            let signer = KeySigner {
                reviewer: Some(reviewer.to_string()),
                key: key.clone(),
            };
            let msg = separation_attestation_bytes(reviewer, a);
            find_signer(
                &registry.reviewers,
                &[i],
                &signer,
                sig,
                &msg,
                Some(&a.date),
                &a.date,
            )
            .map(|_| ())
        }
        (_, _, None) => Err(SignerProblem::NotEligible(reviewer.to_string())),
        _ => Err(SignerProblem::Unsigned),
    };
    match checked {
        Ok(()) => Vec::new(),
        Err(p) => vec![AttestationProblem::Unverified(p)],
    }
}

/// Why one candidate review does not give rung 5. Every one is shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rung5Miss {
    /// The reviewer gave the function's earliest valid review.
    FirstReviewer,
    /// The reviewer is one of the code's authors (git).
    CodeAuthor,
    /// The `independence` answer is `not_independent` (a self-check or an
    /// unstated author), or unanswered.
    NotIndependent,
    /// The review is not an independent, hand-written V&V case.
    NoHumanVvCase(Vec<VvCaseProblem>),
    /// The function has no known concept area.
    NoConceptArea,
    /// No qualification covers these concept areas.
    NotQualified {
        areas: Vec<String>,
    },
    ReviewerOrganisation(OrganisationProblem),
    DevelopingOrganisation(OrganisationProblem),
    /// The reviewer's organisation is the developing organisation.
    SameOrganisation {
        organisation: String,
    },
    /// The review names no separation attestation.
    NoAttestation,
    Attestation(AttestationProblem),
}

impl Rung5Miss {
    /// Plain-English reason, for every view.
    pub fn reason(&self) -> String {
        match self {
            Self::FirstReviewer => {
                "the reviewer gave the first review; IV&V is a later, separate one".into()
            }
            Self::CodeAuthor => "the reviewer wrote this code".into(),
            Self::NotIndependent => "the independence answer is not \"someone else\"".into(),
            Self::NoHumanVvCase(p) => format!("no V&V case written and verified by hand ({p:?})"),
            Self::NoConceptArea => "the function has no known concept area".into(),
            Self::NotQualified { areas } => {
                format!("the reviewer is not qualified in {}", areas.join(", "))
            }
            Self::ReviewerOrganisation(p) => format!("the reviewer's organisation: {p:?}"),
            Self::DevelopingOrganisation(p) => format!("the developing organisation: {p:?}"),
            Self::SameOrganisation { organisation } => {
                format!(
                    "the reviewer's organisation is the developing organisation ({organisation})"
                )
            }
            Self::NoAttestation => "the review names no separation attestation".into(),
            Self::Attestation(AttestationProblem::AuditRecord(AuditRecordProblem::Missing)) => {
                "the separation attestation names no audit record (a GitHub issue URL)".into()
            }
            Self::Attestation(AttestationProblem::AuditRecord(p)) => {
                format!("the separation attestation's audit record is malformed: {p:?}")
            }
            Self::Attestation(p) => format!("the separation attestation: {p:?}"),
        }
    }
}

/// One valid review judged as a rung-5 candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rung5Candidate {
    /// The review's artifact id.
    pub review: Option<String>,
    pub by: String,
    pub date: Option<String>,
    /// The attestation the review names.
    pub attestation: Option<String>,
    /// Empty: this review gives rung 5.
    pub misses: Vec<Rung5Miss>,
}

/// The review that gives rung 5, and what it rests on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rung5Pass {
    pub review: Option<String>,
    pub by: String,
    pub organisation: OrganisationInForce,
    pub developing_organisation: OrganisationInForce,
    /// The attestation's id.
    pub attestation: String,
    /// Shown with [`AUDIT_RECORD_LABEL`]; never fetched.
    pub audit_record: AuditRecord,
}

/// A function's rung-5 judgement: the pass, if any, and every valid review
/// with its misses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndependentVv {
    pub passed: Option<Rung5Pass>,
    pub candidates: Vec<Rung5Candidate>,
}

/// The concept areas of the function that `by`'s qualifications do not
/// cover; `None` when the function has no known area.
fn unqualified_areas(
    root: &ReviewRoot,
    by: &str,
    areas: Option<&BTreeSet<String>>,
) -> Option<Vec<String>> {
    let areas = areas.filter(|a| !a.is_empty())?;
    let quals = root
        .reviewer(by)
        .map(|r| r.qualification.as_slice())
        .unwrap_or(&[]);
    Some(
        areas
            .iter()
            .filter(|c| !quals.iter().any(|q| q.qualifies_for(c)))
            .cloned()
            .collect(),
    )
}

/// Judge every valid review of function `id` (in crate `krate`) as a rung-5
/// candidate (module doc). Pure.
#[allow(clippy::too_many_arguments)]
pub fn judge_function(
    id: &str,
    krate: &str,
    reviews: &[ReviewReport],
    authors: &BTreeSet<String>,
    root: &ReviewRoot,
    registry: &Registry,
    concepts: &ConceptAreas,
    policy: SignaturePolicy,
) -> IndependentVv {
    let mut valid: Vec<&ReviewReport> = reviews
        .iter()
        .filter(|r| r.state.kind() == StateKind::Valid)
        .collect();
    valid.sort_by(|a, b| (&a.date, &a.by).cmp(&(&b.date, &b.by)));
    let Some(first) = valid.first().map(|r| r.by.clone()) else {
        return IndependentVv::default();
    };
    let mut out = IndependentVv::default();
    for r in valid {
        let mut misses = Vec::new();
        if r.by == first {
            misses.push(Rung5Miss::FirstReviewer);
        }
        if authors.contains(&r.by) {
            misses.push(Rung5Miss::CodeAuthor);
        }
        if !r.independent {
            misses.push(Rung5Miss::NotIndependent);
        }
        if !r.vv_case.is_empty() {
            misses.push(Rung5Miss::NoHumanVvCase(r.vv_case.clone()));
        }
        match unqualified_areas(root, &r.by, concepts.get(id)) {
            None => misses.push(Rung5Miss::NoConceptArea),
            Some(a) if !a.is_empty() => misses.push(Rung5Miss::NotQualified { areas: a }),
            Some(_) => {}
        }
        let date = r.date.clone().unwrap_or_default();
        let org = reviewer_organisation(root, registry, &r.by, &date, policy);
        let dev = developing_organisation(root, registry, krate, &date, policy);
        if let Err(p) = &org {
            misses.push(Rung5Miss::ReviewerOrganisation(p.clone()));
        }
        if let Err(p) = &dev {
            misses.push(Rung5Miss::DevelopingOrganisation(p.clone()));
        }
        if let (Ok(o), Ok(d)) = (&org, &dev) {
            if same_organisation(&o.name, &d.name) {
                misses.push(Rung5Miss::SameOrganisation {
                    organisation: d.name.clone(),
                });
            }
        }
        let mut audit = None;
        match &r.separation_attestation {
            None => misses.push(Rung5Miss::NoAttestation),
            Some(aid) => {
                let c = check_attestation(root, registry, &r.by, aid, policy);
                misses.extend(c.problems.into_iter().map(Rung5Miss::Attestation));
                if let Some(a) = &c.attestation {
                    if is_date(&a.date) && a.date.as_str() > date.as_str() {
                        misses.push(Rung5Miss::Attestation(
                            AttestationProblem::DatedAfterReview {
                                attested: a.date.clone(),
                                reviewed: date.clone(),
                            },
                        ));
                    }
                    let other_org = org
                        .as_ref()
                        .is_ok_and(|o| !same_organisation(&a.organisation, &o.name));
                    let other_dev = dev
                        .as_ref()
                        .is_ok_and(|d| !same_organisation(&a.developing_organisation, &d.name));
                    if other_org || other_dev {
                        misses.push(Rung5Miss::Attestation(
                            AttestationProblem::OtherOrganisations {
                                organisation: a.organisation.clone(),
                                developing_organisation: a.developing_organisation.clone(),
                            },
                        ));
                    }
                }
                audit = c.audit_record;
            }
        }
        if misses.is_empty() && out.passed.is_none() {
            if let (Ok(o), Ok(d), Some(a), Some(aid)) =
                (&org, &dev, &audit, &r.separation_attestation)
            {
                out.passed = Some(Rung5Pass {
                    review: r.artifact.clone(),
                    by: r.by.clone(),
                    organisation: o.clone(),
                    developing_organisation: d.clone(),
                    attestation: aid.clone(),
                    audit_record: a.clone(),
                });
            }
        }
        out.candidates.push(Rung5Candidate {
            review: r.artifact.clone(),
            by: r.by.clone(),
            date: r.date.clone(),
            attestation: r.separation_attestation.clone(),
            misses,
        });
    }
    out
}

/// Which append-only list a registry record is in.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RecordList {
    /// `[[code_review.developing_organisation]]`.
    DevelopingOrganisation,
    /// `[[reviewer.organisation]]` of this reviewer.
    ReviewerOrganisation { reviewer: String },
    /// `[[reviewer.separation]]` of this reviewer.
    SeparationAttestation { reviewer: String },
}

/// What is wrong with one registry record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordProblem {
    Organisation(OrganisationProblem),
    Attestation(AttestationProblem),
}

/// A registry record that does not verify (loud, whether or not a review
/// relies on it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordWarning {
    pub list: RecordList,
    /// 0-based position in its list.
    pub index: usize,
    pub problem: RecordProblem,
}

/// Every organisation record and attestation in `root` that does not
/// verify, or whose date or audit record is malformed, or whose id is not
/// unique. Nothing with [`SignaturePolicy::NotChecked`] but dates, audit
/// records and ids.
pub fn record_warnings(
    root: &ReviewRoot,
    registry: &Registry,
    policy: SignaturePolicy,
) -> Vec<RecordWarning> {
    let mut out = Vec::new();
    let devs = root
        .code_review
        .as_ref()
        .map(|c| c.developing_organisation.as_slice())
        .unwrap_or(&[]);
    for (index, e) in devs.iter().enumerate() {
        let scope = dev_scope(e);
        let msg = developing_organisation_bytes(e);
        if let Err(p) = judged(
            &e.name,
            &e.date,
            scope,
            e.signer.as_ref(),
            e.signature.as_deref(),
            &msg,
            registry,
            policy,
        ) {
            out.push(RecordWarning {
                list: RecordList::DevelopingOrganisation,
                index,
                problem: RecordProblem::Organisation(p),
            });
        }
    }
    for r in &root.reviewers {
        for (index, e) in r.organisations.iter().enumerate() {
            let scope = OrganisationScope::Reviewer(r.id.clone());
            let msg = reviewer_organisation_bytes(&r.id, e);
            if let Err(p) = judged(
                &e.name,
                &e.date,
                scope,
                e.signer.as_ref(),
                e.signature.as_deref(),
                &msg,
                registry,
                policy,
            ) {
                let list = RecordList::ReviewerOrganisation {
                    reviewer: r.id.clone(),
                };
                out.push(RecordWarning {
                    list,
                    index,
                    problem: RecordProblem::Organisation(p),
                });
            }
        }
        for (index, a) in r.separations.iter().enumerate() {
            let list = || RecordList::SeparationAttestation {
                reviewer: r.id.clone(),
            };
            let c = check_attestation(root, registry, &r.id, &a.id, policy);
            // An ambiguous id is reported on every copy; otherwise the
            // record's own problems.
            for p in c.problems {
                out.push(RecordWarning {
                    list: list(),
                    index,
                    problem: RecordProblem::Attestation(p),
                });
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "ivv_tests.rs"]
mod tests;
