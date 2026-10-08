//! Unit tests of [`super`] (GitHub #809): the audit-record form, the
//! organisation-name comparison, the V&V-case reasons against the rung-4
//! gate, the miss reasons, and attestation lookup. The engine scenarios are
//! `engine_ivv_tests.rs`; the signed statements are `signing/ivv_tests.rs`.

use super::*;
use crate::review::wizard::{derived_rung, Rung};

/// Methodology: the audit record must be exactly
/// `https://github.com/<owner>/<repo>/issues/<n>` (maintainer, #809,
/// 2026-10-08: "publicly available audit record in the form of gh issue
/// (closed or otherwise)"). Each malformed form is refused with its typed
/// reason.
///
/// Result (2026-10-08): passes.
#[test]
fn audit_record_form() {
    let ok = parse_audit_record("https://github.com/theodoreOnzGit/outram-park-backend/issues/809")
        .unwrap();
    assert_eq!(
        (ok.owner.as_str(), ok.repo.as_str(), ok.number),
        ("theodoreOnzGit", "outram-park-backend", 809)
    );
    assert!(parse_audit_record("https://github.com/a/b.c_d-e/issues/1").is_ok());
    let not = |u: &str| AuditRecordProblem::NotAnIssueUrl(u.to_string());
    for u in [
        "http://github.com/o/r/issues/1",
        "https://gitlab.com/o/r/issues/1",
        "https://github.com/o/r/pull/1",
        "https://github.com/o/r/issues/1/",
        "https://github.com/o/r/issues",
        " https://github.com/o/r/issues/1",
        "",
    ] {
        assert_eq!(parse_audit_record(u), Err(not(u)), "{u:?}");
    }
    assert_eq!(
        parse_audit_record("https://github.com/-o/r/issues/1"),
        Err(AuditRecordProblem::BadOwner("-o".into()))
    );
    assert_eq!(
        parse_audit_record("https://github.com//r/issues/1"),
        Err(AuditRecordProblem::BadOwner(String::new()))
    );
    assert_eq!(
        parse_audit_record("https://github.com/o/../issues/1"),
        Err(AuditRecordProblem::BadRepo("..".into()))
    );
    assert_eq!(
        parse_audit_record("https://github.com/o/r r/issues/1"),
        Err(AuditRecordProblem::BadRepo("r r".into()))
    );
    for n in [
        "0",
        "01",
        "1a",
        "",
        "99999999999999999999999",
        "1?x=1",
        "1#c2",
    ] {
        let u = format!("https://github.com/o/r/issues/{n}");
        assert_eq!(
            parse_audit_record(&u),
            Err(AuditRecordProblem::BadNumber(n.into())),
            "{n:?}"
        );
    }
}

/// Methodology: organisation names compare after trimming, collapsing
/// white space and ignoring case, and nothing more.
///
/// Result (2026-10-08): passes.
#[test]
fn organisation_names_compare_loosely_on_spacing_and_case_only() {
    assert!(same_organisation(
        "  Outram  Park\tproject ",
        "outram park PROJECT"
    ));
    assert!(!same_organisation(
        "NUS",
        "National University of Singapore"
    ));
    assert!(!same_organisation("Outram Park", "Outram-Park"));
}

/// Methodology: [`vv_case_problems`] is empty exactly when the wizard
/// derives rung 4, for every combination of the evidence answer, the author
/// answer and git's view; each missing half is named.
///
/// Result (2026-10-08): passes (24 combinations).
#[test]
fn vv_case_problems_match_the_rung_4_gate() {
    let mut n = 0;
    for ev in [
        "analytical_case",
        "reference_code_to_code",
        "convergence_order_study",
        "unit_tests_only",
    ] {
        for au in ["human_wrote_and_verified", "agent_wrote_or_cowrote"] {
            for t in [
                TestAuthorship::Human,
                TestAuthorship::Agent,
                TestAuthorship::Unknown,
            ] {
                let a: BTreeMap<String, String> = [
                    ("vv_evidence".to_string(), ev.to_string()),
                    ("vv_case_author".to_string(), au.to_string()),
                ]
                .into();
                let p = vv_case_problems(&a, t);
                assert_eq!(
                    p.is_empty(),
                    derived_rung(&a, t) == Rung::Four,
                    "{ev} {au} {t:?}"
                );
                assert_eq!(
                    p.contains(&VvCaseProblem::NoQualifyingEvidence),
                    ev == "unit_tests_only"
                );
                assert_eq!(
                    p.contains(&VvCaseProblem::NotWrittenByHand),
                    au != "human_wrote_and_verified"
                );
                assert_eq!(
                    p.contains(&VvCaseProblem::TestsByAgent),
                    t == TestAuthorship::Agent
                );
                assert_eq!(
                    p.contains(&VvCaseProblem::TestAuthorshipUnknown),
                    t == TestAuthorship::Unknown
                );
                n += 1;
            }
        }
    }
    assert_eq!(n, 24);
}

/// Methodology: every miss has a distinct, non-empty plain-English reason
/// (the views show these; Leak Before Break).
///
/// Result (2026-10-08): passes.
#[test]
fn every_miss_has_a_reason() {
    let all = [
        Rung5Miss::FirstReviewer,
        Rung5Miss::CodeAuthor,
        Rung5Miss::NotIndependent,
        Rung5Miss::NoHumanVvCase(vec![VvCaseProblem::TestsByAgent]),
        Rung5Miss::NoConceptArea,
        Rung5Miss::NotQualified {
            areas: vec!["concept:a".into()],
        },
        Rung5Miss::ReviewerOrganisation(OrganisationProblem::NotRecorded),
        Rung5Miss::DevelopingOrganisation(OrganisationProblem::BadDate("x".into())),
        Rung5Miss::SameOrganisation {
            organisation: "O".into(),
        },
        Rung5Miss::NoAttestation,
        Rung5Miss::Attestation(AttestationProblem::AuditRecord(AuditRecordProblem::Missing)),
        Rung5Miss::Attestation(AttestationProblem::AuditRecord(
            AuditRecordProblem::BadNumber("0".into()),
        )),
        Rung5Miss::Attestation(AttestationProblem::Ambiguous { id: "s".into() }),
    ];
    let reasons: BTreeSet<String> = all.iter().map(Rung5Miss::reason).collect();
    assert_eq!(reasons.len(), all.len());
    assert!(reasons.iter().all(|r| !r.is_empty()));
}

/// Methodology: two attestations with one id are ambiguous (a stamp cannot
/// say which it means), and both copies are listed as warnings; an
/// attestation of an unknown reviewer is not found; without signature
/// checks only dates, ids and audit records are judged, with them an
/// unsigned attestation is reported as unsigned.
///
/// Result (2026-10-08): passes.
#[test]
fn ambiguous_and_unknown_attestations() {
    let a = SeparationAttestation {
        id: "s".into(),
        organisation: "A".into(),
        developing_organisation: "B".into(),
        date: "2026-10-08".into(),
        audit_record: Some("https://github.com/o/r/issues/1".into()),
        key: None,
        signature: None,
    };
    let text = "[[reviewer]]\nid = \"github:r\"\nrole = \"reviewer\"\n";
    let mut root = ReviewRoot::parse(text).unwrap();
    root.reviewers[0].separations = vec![a.clone(), a];
    let reg = Registry::build(&root);
    let c = check_attestation(&root, &reg, "github:r", "s", SignaturePolicy::NotChecked);
    assert_eq!(
        c.problems,
        vec![AttestationProblem::Ambiguous { id: "s".into() }]
    );
    let w = record_warnings(&root, &reg, SignaturePolicy::NotChecked);
    assert_eq!(w.iter().map(|w| w.index).collect::<Vec<_>>(), vec![0, 1]);
    let c = check_attestation(
        &root,
        &reg,
        "github:nobody",
        "s",
        SignaturePolicy::NotChecked,
    );
    assert_eq!(
        c.problems,
        vec![AttestationProblem::NotFound { id: "s".into() }]
    );
    root.reviewers[0].separations.truncate(1);
    assert!(record_warnings(&root, &reg, SignaturePolicy::NotChecked).is_empty());
    let c = check_attestation(&root, &reg, "github:r", "s", SignaturePolicy::Enforce);
    assert_eq!(
        c.problems,
        vec![AttestationProblem::Unverified(SignerProblem::Unsigned)]
    );
    let mut bad = root.clone();
    bad.reviewers[0].separations[0].date = "soon".into();
    let c = check_attestation(&bad, &reg, "github:r", "s", SignaturePolicy::Enforce);
    assert_eq!(c.problems, vec![AttestationProblem::BadDate("soon".into())]);
}
