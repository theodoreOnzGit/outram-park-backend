//! **Rung 5 as IV&V** (GitHub #809; maintainer decisions 2026-10-08):
//! engine scenarios, one per rule, with the expected rung and every reason
//! pinned.
//!
//! Methodology. Each test builds a real registry: founder maintainer `M`
//! (key `k1`) and reviewer `R` (key `r1`, admitted by `M`, scoped to
//! `crates/x/**`, qualified in `concept:thermal-hydraulics` by a degree
//! with evidence), with keys from [`keystore::generate`] (argon2id +
//! ed25519, no mocks). `M` records the workspace developing organisation
//! "Outram Park project" and `R`'s organisation "Example IV&V Ltd"; `R`
//! signs separation attestation `sep-1` naming both, with the audit record
//! `https://github.com/theodoreOnzGit/outram-park-backend/issues/809`.
//! Function `f` (concept `thermal-hydraulics/natural-circulation`) has a
//! first review by `M` (rung 3) and `R`'s review: a hand-written analytical
//! V&V case (rung 4; git shows the reaching test added by a human commit),
//! `independence = someone_else`, naming `sep-1`. Every stamp and record is
//! signed and judged with [`SignaturePolicy::Enforce`] unless the test
//! says otherwise. Each scenario changes one thing and pins the function's
//! rung, `R`'s misses and the flag.
//!
//! Result (2026-10-08): all pass. The full setup is rung 5; same
//! organisation, no attestation, a malformed or missing audit record, an
//! AI-authored V&V case and an unqualified reviewer each leave it at the
//! reviews' own rung (4, or 3 for the AI case) with the reason shown; a
//! per-crate override is applied to its crate only.

use super::tests::{eval, fid, folder, fun, git_for, review, reviews_in, D, F, M, R};
use super::*;
use crate::review::ivv::{
    AttestationProblem, AuditRecordProblem, OrganisationProblem, OrganisationScope, RecordList,
    RecordProblem, Rung5Miss, VvCaseProblem, AUDIT_RECORD_LABEL,
};
use crate::review::root::{
    CodeReviewSettings, DevelopingOrganisation, Qualification, QualificationBasis,
    QualificationRecord, Reviewer, ReviewerOrganisation, SeparationAttestation,
};
use crate::review::signing::keystore::{self, UnlockedKey};
use crate::review::signing::registry::SignerProblem;

const DEV: &str = "Outram Park project";
const IVV: &str = "Example IV&V Ltd";
const URL: &str = "https://github.com/theodoreOnzGit/outram-park-backend/issues/809";
const AT: &str = "2026-10-07T12:00:00+08:00";
const DAY: &str = "2026-10-07";

/// Everything one scenario needs; mutate, then [`Setup::run`].
struct Setup {
    mk: UnlockedKey,
    rk: UnlockedKey,
    root: ReviewRoot,
    first: ReviewEntry,
    ivv: ReviewEntry,
    /// The commit message that added the test reaching `f`, as of `R`'s
    /// review commit.
    test_commit: String,
    policy: SignaturePolicy,
}

fn person(id: &str, role: Role) -> Reviewer {
    Reviewer {
        id: id.into(),
        name: None,
        role,
        scope: vec![],
        qualification: vec![],
        admitted: None,
        admitted_by: None,
        keys: vec![],
        revoked: None,
        organisations: vec![],
        separations: vec![],
    }
}

fn dev_org(krate: Option<&str>, name: &str, k: &UnlockedKey) -> DevelopingOrganisation {
    let mut e = DevelopingOrganisation {
        krate: krate.map(str::to_string),
        name: name.into(),
        date: DAY.into(),
        signer: None,
        signature: None,
    };
    k.sign_developing_organisation(&mut e).unwrap();
    e
}

fn reviewer_org(reviewer: &str, name: &str, k: &UnlockedKey) -> ReviewerOrganisation {
    let mut e = ReviewerOrganisation {
        name: name.into(),
        date: DAY.into(),
        signer: None,
        signature: None,
    };
    k.sign_reviewer_organisation(reviewer, &mut e).unwrap();
    e
}

fn attestation(organisation: &str, developing: &str, url: Option<&str>) -> SeparationAttestation {
    SeparationAttestation {
        id: "sep-1".into(),
        organisation: organisation.into(),
        developing_organisation: developing.into(),
        date: DAY.into(),
        audit_record: url.map(str::to_string),
        key: None,
        signature: None,
    }
}

fn setup() -> Setup {
    let pass = "correct horse battery staple";
    let (mf, mk) = keystore::generate(M, "k1", DAY, pass).unwrap();
    let (rf, rk) = keystore::generate(R, "r1", DAY, pass).unwrap();
    let mut m = person(M, Role::Maintainer);
    m.keys = vec![mf.reviewer_key()];
    let mut r = person(R, Role::Reviewer);
    r.scope = vec!["crates/x/**".into()];
    r.keys = vec![rf.reviewer_key()];
    r.admitted = Some(DAY.into());
    mk.admit(&mut r).unwrap();
    r.qualification = vec![Qualification::Record(QualificationRecord {
        area: "concept:thermal-hydraulics".into(),
        basis: QualificationBasis::Degree,
        evidence: vec!["https://doi.org/10.1/thesis".into()],
        endorsed_by: None,
        self_declared: false,
    })];
    r.organisations = vec![reviewer_org(R, IVV, &mk)];
    let mut a = attestation(IVV, DEV, Some(URL));
    rk.attest_separation(&mut a).unwrap();
    r.separations = vec![a];
    let root = ReviewRoot {
        code_review: Some(CodeReviewSettings {
            founder: Some(M.into()),
            developing_organisation: vec![dev_org(None, DEV, &mk)],
            ..CodeReviewSettings::default()
        }),
        reviewers: vec![m, r],
        deleted_crates: vec![],
    };
    let first = review(F, M, 'a', &[]);
    let mut ivv = review(F, R, 'a', &[]);
    ivv.review.rung = 4;
    ivv.review
        .checklist
        .insert("vv_evidence".into(), "analytical_case".into());
    ivv.review
        .checklist
        .insert("vv_case_author".into(), "human_wrote_and_verified".into());
    ivv.review.separation_attestation = Some("sep-1".into());
    let mut s = Setup {
        mk,
        rk,
        root,
        first,
        ivv,
        test_commit: "add the analytical natural-circulation case".into(),
        policy: SignaturePolicy::Enforce,
    };
    s.sign();
    s
}

impl Setup {
    /// Re-sign both stamps after an edit.
    fn sign(&mut self) {
        self.mk.sign_review_at(&mut self.first, AT).unwrap();
        self.rk.sign_review_at(&mut self.ivv, AT).unwrap();
    }

    fn r(&mut self) -> &mut Reviewer {
        &mut self.root.reviewers[1]
    }

    /// Replace `R`'s attestation, signed by `R`.
    fn attest(&mut self, a: SeparationAttestation) {
        let mut a = a;
        self.rk.attest_separation(&mut a).unwrap();
        self.r().separations = vec![a];
    }

    fn evaluation(&self, krate: &str, dir: &str) -> Evaluation {
        let file = format!("{dir}/a.rs::f");
        let (first, ivv) = if dir == D {
            (self.first.clone(), self.ivv.clone())
        } else {
            let mut first = review(&file, M, 'a', &[]);
            let mut ivv = self.ivv.clone();
            ivv.kovan.target = Some(fid(&file));
            ivv.review.path = Some(file.clone());
            self.mk.sign_review_at(&mut first, AT).unwrap();
            self.rk.sign_review_at(&mut ivv, AT).unwrap();
            (first, ivv)
        };
        let revs = [reviews_in(krate, dir, vec![first, ivv])];
        let idx = [folder(
            krate,
            dir,
            &[("a.rs", vec![fun(&file, 'a', &[], &["t::ok"])])],
        )];
        let mut g = git_for(&[&revs[0]]);
        g.stamps
            .get_mut(&ReviewKey {
                function: fid(&file),
                by: R.into(),
            })
            .unwrap()
            .tests_at_review = Some(TestsAtReview {
            reached_by: vec!["t::ok".into()],
            commit_messages: [("t::ok".to_string(), vec![self.test_commit.clone()])].into(),
        });
        let areas: ConceptAreas = [(
            fid(&file),
            ["concept:thermal-hydraulics/natural-circulation".to_string()].into(),
        )]
        .into();
        eval(&revs, &idx, &self.root, &g, &areas, self.policy)
    }

    fn run(&self) -> FunctionReport {
        self.evaluation("x", D).functions[&fid(F)].clone()
    }
}

/// `R`'s misses.
fn misses(f: &FunctionReport) -> Vec<Rung5Miss> {
    f.independent_vv
        .candidates
        .iter()
        .find(|c| c.by == R)
        .map(|c| c.misses.clone())
        .unwrap_or_default()
}

/// The "independent V&V not counted" flag, with its misses.
fn flagged(f: &FunctionReport) -> Option<Vec<Rung5Miss>> {
    f.flags.iter().find_map(|fl| match fl {
        FunctionFlag::IndependentVvNotCounted { misses, .. } => Some(misses.clone()),
        _ => None,
    })
}

/// The full pass: every condition holds, so the function is at rung 5; the
/// pass names both organisations, the attestation and the audit record
/// (labelled as not verified); nothing is flagged; `M`'s first review is a
/// candidate that misses (shown, never hidden).
#[test]
fn full_pass_is_rung_5() {
    let s = setup();
    let ev = s.evaluation("x", D);
    let f = &ev.functions[&fid(F)];
    assert_eq!(f.state, StampState::Valid);
    assert!(
        f.reviews.iter().all(|r| r.state == StampState::Valid),
        "{:?}",
        f.reviews
    );
    assert_eq!(f.rung, Some(5));
    assert_eq!(misses(f), vec![]);
    let p = f.independent_vv.passed.as_ref().unwrap();
    assert_eq!((p.by.as_str(), p.attestation.as_str()), (R, "sep-1"));
    assert_eq!(
        (
            p.organisation.name.as_str(),
            p.developing_organisation.name.as_str()
        ),
        (IVV, DEV)
    );
    assert_eq!(
        p.developing_organisation.scope,
        OrganisationScope::Workspace
    );
    assert_eq!(
        p.organisation.signed_by,
        Some((M.to_string(), "k1".to_string()))
    );
    assert_eq!(p.audit_record.number, 809);
    assert_eq!(
        p.audit_record.shown(),
        format!("{AUDIT_RECORD_LABEL}: {URL}")
    );
    assert_eq!(AUDIT_RECORD_LABEL, "audit record (not verified by kovan)");
    assert_eq!(flagged(f), None);
    assert!(ev.ivv_warnings.is_empty(), "{:?}", ev.ivv_warnings);
    let by_m = f
        .independent_vv
        .candidates
        .iter()
        .find(|c| c.by == M)
        .unwrap();
    assert!(by_m.misses.contains(&Rung5Miss::FirstReviewer));
    assert!(by_m.misses.contains(&Rung5Miss::NoAttestation));
}

/// Same organisation: `R` is recorded in the developing organisation
/// (spelled with other case and spacing). Rung 4, with the reason; the
/// attestation now names another organisation than `R`'s, which is shown
/// too; the review named an attestation, so the function is flagged.
#[test]
fn same_organisation_is_not_rung_5() {
    let mut s = setup();
    let o = reviewer_org(R, "outram  PARK project", &s.mk);
    s.r().organisations.push(o);
    let f = s.run();
    assert_eq!(f.rung, Some(4));
    let want = vec![
        Rung5Miss::SameOrganisation {
            organisation: DEV.into(),
        },
        Rung5Miss::Attestation(AttestationProblem::OtherOrganisations {
            organisation: IVV.into(),
            developing_organisation: DEV.into(),
        }),
    ];
    assert_eq!(misses(&f), want);
    assert_eq!(flagged(&f), Some(want));
    assert_eq!(
        f.flags.iter().map(FunctionFlag::kind).collect::<Vec<_>>(),
        [FlagKind::IndependentVvNotCounted]
    );
}

/// Separate organisation, no attestation named: rung 4, reason
/// "no attestation"; not flagged (the review claims nothing).
#[test]
fn separate_organisation_without_attestation_is_not_rung_5() {
    let mut s = setup();
    s.ivv.review.separation_attestation = None;
    s.sign();
    let f = s.run();
    assert_eq!(f.rung, Some(4));
    assert_eq!(misses(&f), vec![Rung5Miss::NoAttestation]);
    assert_eq!(flagged(&f), None);
    // Naming an attestation that does not exist is its own reason.
    s.ivv.review.separation_attestation = Some("sep-9".into());
    s.sign();
    let f = s.run();
    assert_eq!(
        misses(&f),
        vec![Rung5Miss::Attestation(AttestationProblem::NotFound {
            id: "sep-9".into()
        })]
    );
    assert!(flagged(&f).is_some());
}

/// Separate organisation and an attestation, but its audit record is a
/// pull request, or absent: no rung 5, and the reason says why. Judged
/// without signatures (the keystore refuses to sign either), so the URL
/// rule is all that differs from a pass; the same record under
/// [`SignaturePolicy::Enforce`] also lists the signature it lacks.
#[test]
fn malformed_audit_record_is_not_rung_5() {
    let mut s = setup();
    s.policy = SignaturePolicy::NotChecked;
    assert_eq!(s.run().rung, Some(5), "the unsigned pass, for contrast");
    let pr = "https://github.com/theodoreOnzGit/outram-park-backend/pull/809";
    s.r().separations = vec![attestation(IVV, DEV, Some(pr))];
    let f = s.run();
    assert_eq!(f.rung, Some(4));
    let want = vec![Rung5Miss::Attestation(AttestationProblem::AuditRecord(
        AuditRecordProblem::NotAnIssueUrl(pr.into()),
    ))];
    assert_eq!(misses(&f), want);
    assert_eq!(flagged(&f), Some(want.clone()));
    assert!(
        want[0].reason().contains("malformed"),
        "{}",
        want[0].reason()
    );
    s.r().separations = vec![attestation(IVV, DEV, None)];
    let f = s.run();
    let missing =
        Rung5Miss::Attestation(AttestationProblem::AuditRecord(AuditRecordProblem::Missing));
    assert_eq!(misses(&f), vec![missing.clone()]);
    assert!(missing.reason().contains("no audit record"));
    s.policy = SignaturePolicy::Enforce;
    let f = s.run();
    assert_eq!(
        misses(&f),
        vec![
            Rung5Miss::Attestation(AttestationProblem::Unverified(SignerProblem::Unsigned)),
            missing
        ]
    );
}

/// Separate organisation and a valid attestation, but the V&V case was
/// written by an AI agent: answered honestly, the review is valid at rung 3
/// and misses rung 5 with both reasons; answered "human" while git shows
/// the trailer, the review is invalid (the rung-4 rule) and is no
/// candidate, so the function stays at `M`'s rung 3.
#[test]
fn ai_authored_vv_case_is_not_rung_5() {
    let mut s = setup();
    s.test_commit = "add test\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>".into();
    s.ivv.review.rung = 3;
    s.ivv
        .review
        .checklist
        .insert("vv_case_author".into(), "agent_wrote_or_cowrote".into());
    s.sign();
    let f = s.run();
    assert_eq!(f.rung, Some(3));
    let want = vec![Rung5Miss::NoHumanVvCase(vec![
        VvCaseProblem::NotWrittenByHand,
        VvCaseProblem::TestsByAgent,
    ])];
    assert_eq!(misses(&f), want);
    assert_eq!(flagged(&f), Some(want));
    s.ivv.review.rung = 4;
    s.ivv
        .review
        .checklist
        .insert("vv_case_author".into(), "human_wrote_and_verified".into());
    s.sign();
    let f = s.run();
    assert_eq!(f.rung, Some(3));
    assert_eq!(
        f.reviews.iter().find(|r| r.by == R).unwrap().state.kind(),
        StateKind::Invalid
    );
    assert!(f.independent_vv.candidates.iter().all(|c| c.by != R));
}

/// Separate organisation, a valid attestation and a human V&V case, but
/// `R` holds no qualification covering the function's area: rung 4.
#[test]
fn unqualified_reviewer_is_not_rung_5() {
    let mut s = setup();
    s.r().qualification.clear();
    let f = s.run();
    assert_eq!(f.rung, Some(4));
    let want = vec![Rung5Miss::NotQualified {
        areas: vec!["concept:thermal-hydraulics/natural-circulation".into()],
    }];
    assert_eq!(misses(&f), want);
    assert_eq!(flagged(&f), Some(want));
}

/// A per-crate override: crate `x` is developed by `R`'s own organisation,
/// so in `x` the reviewer is not separate (and the attestation names the
/// workspace organisation, not `x`'s); crate `y`, with no override, keeps
/// the workspace organisation and reaches rung 5.
#[test]
fn per_crate_override_applies_to_its_crate() {
    let mut s = setup();
    let o = dev_org(Some("x"), IVV, &s.mk);
    s.root
        .code_review
        .as_mut()
        .unwrap()
        .developing_organisation
        .push(o);
    let f = s.run();
    assert_eq!(f.rung, Some(4));
    assert_eq!(
        misses(&f),
        vec![
            Rung5Miss::SameOrganisation {
                organisation: IVV.into()
            },
            Rung5Miss::Attestation(AttestationProblem::OtherOrganisations {
                organisation: IVV.into(),
                developing_organisation: DEV.into(),
            }),
        ]
    );
    let y = s.evaluation("y", "crates/y/src");
    let fy = &y.functions[&fid("crates/y/src/a.rs::f")];
    // R's scope is crates/x/**: give it y too, so only the override differs.
    assert_eq!(
        fy.reviews.iter().find(|r| r.by == R).unwrap().state,
        StampState::OutsideScope
    );
    // The scope change needs a new admission (it is signed).
    let r = &mut s.root.reviewers[1];
    r.scope.push("crates/y/**".into());
    r.keys[0]
        .history
        .retain(|e| e.event != crate::review::root::KeyEventKind::Admitted);
    s.mk.admit(r).unwrap();
    let y = s.evaluation("y", "crates/y/src");
    let fy = &y.functions[&fid("crates/y/src/a.rs::f")];
    assert_eq!(fy.rung, Some(5));
    let p = fy.independent_vv.passed.as_ref().unwrap();
    assert_eq!(
        p.developing_organisation.scope,
        OrganisationScope::Workspace
    );
    // In x, the override is the organisation in force, and says so.
    let reg = crate::review::signing::registry::Registry::build(&s.root);
    let d = crate::review::ivv::developing_organisation(
        &s.root,
        &reg,
        "x",
        DAY,
        SignaturePolicy::Enforce,
    )
    .unwrap();
    assert_eq!(
        (d.name.as_str(), d.scope),
        (IVV, OrganisationScope::Crate("x".into()))
    );
    // Before the override's date, the workspace entry is in force.
    let d = crate::review::ivv::developing_organisation(
        &s.root,
        &reg,
        "x",
        "2026-10-06",
        SignaturePolicy::Enforce,
    );
    assert_eq!(d, Err(OrganisationProblem::NotRecorded));
}

/// Leak Before Break for the records themselves: an organisation record
/// signed by a non-maintainer, a tampered developing-organisation record,
/// an attestation dated after the review and a record edited in place each
/// show (as a miss, in `ivv_warnings`, and as an append-only warning).
#[test]
fn unverified_and_edited_records_are_loud() {
    let mut s = setup();
    // R signs its own organisation: not a maintainer.
    let o = reviewer_org(R, IVV, &s.rk);
    s.r().organisations = vec![o];
    let ev = s.evaluation("x", D);
    let f = &ev.functions[&fid(F)];
    assert_eq!(f.rung, Some(4));
    assert_eq!(
        misses(f)[0],
        Rung5Miss::ReviewerOrganisation(OrganisationProblem::Unverified {
            date: DAY.into(),
            problem: SignerProblem::NotEligible(R.into()),
        })
    );
    assert_eq!(ev.ivv_warnings.len(), 1);
    assert_eq!(
        ev.ivv_warnings[0].list,
        RecordList::ReviewerOrganisation { reviewer: R.into() }
    );

    let mut s = setup();
    s.root.code_review.as_mut().unwrap().developing_organisation[0].name = "Someone else".into();
    let ev = s.evaluation("x", D);
    assert!(matches!(
        misses(&ev.functions[&fid(F)])[0],
        Rung5Miss::DevelopingOrganisation(OrganisationProblem::Unverified {
            problem: SignerProblem::BadSignature,
            ..
        })
    ));
    assert!(matches!(
        ev.ivv_warnings[0].problem,
        RecordProblem::Organisation(_)
    ));

    let mut s = setup();
    s.attest(SeparationAttestation {
        date: "2026-10-08".into(),
        ..attestation(IVV, DEV, Some(URL))
    });
    let f = s.run();
    assert_eq!(
        misses(&f),
        vec![Rung5Miss::Attestation(
            AttestationProblem::DatedAfterReview {
                attested: "2026-10-08".into(),
                reviewed: DAY.into(),
            }
        )]
    );

    // Append-only: editing a committed record warns.
    let s = setup();
    let mut edited = s.root.clone();
    edited.code_review.as_mut().unwrap().developing_organisation[0].date = "2026-10-06".into();
    edited.reviewers[1].organisations.clear();
    edited.reviewers[1].separations[0].audit_record = None;
    assert_eq!(
        history_append_only(&s.root, &edited),
        vec![
            HistoryWarning::RecordChanged {
                list: RecordList::DevelopingOrganisation,
                index: 0
            },
            HistoryWarning::RecordRemoved {
                list: RecordList::ReviewerOrganisation { reviewer: R.into() },
                index: 0
            },
            HistoryWarning::RecordChanged {
                list: RecordList::SeparationAttestation { reviewer: R.into() },
                index: 0
            },
        ]
    );
    let mut appended = s.root.clone();
    appended.reviewers[1]
        .organisations
        .push(reviewer_org(R, "Other Ltd", &s.mk));
    assert!(history_append_only(&s.root, &appended).is_empty());
    assert_eq!(
        FlagKind::IndependentVvNotCounted.label(),
        "independent V&V not counted"
    );
    assert!(FlagKind::ALL.contains(&FlagKind::IndependentVvNotCounted));
}
