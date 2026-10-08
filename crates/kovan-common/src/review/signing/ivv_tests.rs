//! Tests of the GitHub #809 signed statements: the developing organisation,
//! a reviewer's organisation, the separation attestation, and the v3 review
//! bytes that name an attestation.
//!
//! Methodology. Keys come from [`keystore::generate`] (no mocks); the
//! founder `FOUNDER` (key `k1`) is the maintainer and `ALICE` (key `a1`) an
//! admitted reviewer ([`founder_and_alice`]). Each statement is signed with
//! the keystore, then judged by [`crate::review::ivv`] with
//! [`SignaturePolicy::Enforce`]. Pass criterion: the exact verdict the rule
//! predicts: a signed statement verifies; any edit to a signed field
//! fails with `BadSignature`; a signature over one kind of statement never
//! verifies as another (distinct first lines, and the `crate` line keeps a
//! workspace entry from passing as an override); a stamp's v1 and v2 bytes
//! do not change when no attestation is named.
//!
//! Result (2026-10-08): all pass.

use super::keystore::SignError;
use super::registry::{Registry, SignerProblem};
use super::tests::{founder_and_alice, review_entry, root, unverified, ALICE, F, FOUNDER};
use super::*;
use crate::review::engine::SignaturePolicy;
use crate::review::ivv::{
    check_attestation, developing_organisation, reviewer_organisation, AttestationProblem,
    AuditRecordProblem, OrganisationProblem,
};
use crate::review::root::{DevelopingOrganisation, ReviewerOrganisation, SeparationAttestation};

const URL: &str = "https://github.com/theodoreOnzGit/outram-park-backend/issues/809";
const E: SignaturePolicy = SignaturePolicy::Enforce;

fn att() -> SeparationAttestation {
    SeparationAttestation {
        id: "sep-1".into(),
        organisation: "Example IV&V Ltd".into(),
        developing_organisation: "Outram Park project".into(),
        date: "2026-10-08".into(),
        audit_record: Some(URL.into()),
        key: None,
        signature: None,
    }
}

fn dev(krate: Option<&str>) -> DevelopingOrganisation {
    DevelopingOrganisation {
        krate: krate.map(str::to_string),
        name: "Outram Park project".into(),
        date: "2026-10-08".into(),
        signer: None,
        signature: None,
    }
}

fn org() -> ReviewerOrganisation {
    ReviewerOrganisation {
        name: "Outram Park project".into(),
        date: "2026-10-08".into(),
        signer: None,
        signature: None,
    }
}

/// An attestation signed by its own reviewer alone verifies; editing any
/// signed field (organisation, developing organisation, date, audit record)
/// breaks it; another reviewer's signature, or none, does not count.
#[test]
fn attestation_verifies_and_tampering_fails() {
    let (founder, f, mut alice, a) = founder_and_alice();
    let mut good = att();
    a.attest_separation(&mut good).unwrap();
    assert_eq!(good.key.as_deref(), Some("a1"));
    let judge = |alice: &crate::review::root::Reviewer, x: &SeparationAttestation| {
        let mut alice = alice.clone();
        alice.separations = vec![x.clone()];
        let rt = root(vec![founder.clone(), alice]);
        check_attestation(&rt, &Registry::build(&rt), ALICE, "sep-1", E).problems
    };
    assert_eq!(judge(&alice, &good), vec![]);
    let bad = |p| vec![AttestationProblem::Unverified(p)];
    let edits: [fn(&mut SeparationAttestation); 4] = [
        |x| x.organisation = "Outram Park project".into(),
        |x| x.developing_organisation = "Example IV&V Ltd".into(),
        |x| x.date = "2026-10-07".into(),
        |x| x.audit_record = Some(URL.replace("809", "810")),
    ];
    for edit in edits {
        let mut t = good.clone();
        edit(&mut t);
        assert_eq!(judge(&alice, &t), bad(SignerProblem::BadSignature), "{t:?}");
    }
    // The founder signs it with k1, filed as Alice's: Alice has no key k1.
    let mut by_founder = att();
    f.attest_separation(&mut by_founder).unwrap();
    assert_eq!(
        judge(&alice, &by_founder),
        bad(SignerProblem::UnknownKey { key: "k1".into() })
    );
    let mut unsigned = good.clone();
    unsigned.signature = None;
    assert_eq!(judge(&alice, &unsigned), bad(SignerProblem::Unsigned));
    // Revoked before the attestation: it does not count.
    alice.revoked = Some(Revocation {
        date: "2026-10-08".into(),
        compromised_from: None,
        by: FOUNDER.into(),
        signature: None,
    });
    assert!(matches!(
        judge(&alice, &good)[0],
        AttestationProblem::Unverified(SignerProblem::SignerRevoked { .. })
    ));
}

/// Maintainer-signed organisation records verify; a reviewer cannot sign
/// one; a tampered name fails; an unsigned one is reported as unsigned.
#[test]
fn organisation_records_verify_and_need_a_maintainer() {
    let (founder, f, mut alice, a) = founder_and_alice();
    let mut o = org();
    f.sign_reviewer_organisation(ALICE, &mut o).unwrap();
    alice.organisations = vec![o.clone()];
    let mut d = dev(None);
    f.sign_developing_organisation(&mut d).unwrap();
    let mut rt = root(vec![founder.clone(), alice.clone()]);
    rt.code_review.as_mut().unwrap().developing_organisation = vec![d.clone()];
    let reg = Registry::build(&rt);
    let ro = reviewer_organisation(&rt, &reg, ALICE, "2026-10-08", E).unwrap();
    assert_eq!(ro.signed_by, Some((FOUNDER.to_string(), "k1".to_string())));
    assert!(developing_organisation(&rt, &reg, "any", "2026-10-08", E).is_ok());
    // A reviewer signs its own organisation: not eligible.
    let mut own = org();
    a.sign_reviewer_organisation(ALICE, &mut own).unwrap();
    rt.reviewers[1].organisations = vec![own];
    let reg = Registry::build(&rt);
    assert_eq!(
        reviewer_organisation(&rt, &reg, ALICE, "2026-10-08", E),
        Err(OrganisationProblem::Unverified {
            date: "2026-10-08".into(),
            problem: SignerProblem::NotEligible(ALICE.into())
        })
    );
    let mut t = d.clone();
    t.name = "Example IV&V Ltd".into();
    rt.code_review.as_mut().unwrap().developing_organisation = vec![t];
    let reg = Registry::build(&rt);
    assert!(matches!(
        developing_organisation(&rt, &reg, "any", "2026-10-08", E),
        Err(OrganisationProblem::Unverified {
            problem: SignerProblem::BadSignature,
            ..
        })
    ));
    rt.code_review.as_mut().unwrap().developing_organisation = vec![dev(None)];
    assert!(matches!(
        developing_organisation(&rt, &Registry::build(&rt), "any", "2026-10-08", E),
        Err(OrganisationProblem::Unverified {
            problem: SignerProblem::Unsigned,
            ..
        })
    ));
    rt.code_review.as_mut().unwrap().developing_organisation = vec![DevelopingOrganisation {
        date: "8 Oct".into(),
        ..dev(None)
    }];
    assert_eq!(
        developing_organisation(&rt, &Registry::build(&rt), "any", "2026-10-08", E),
        Err(OrganisationProblem::BadDate("8 Oct".into()))
    );
}

/// A signature over one kind of statement never verifies as another: a
/// reviewer-organisation signature reused on a developing-organisation
/// record with the same name, date and signer; a workspace developing
/// organisation reused as a crate override; an attestation's signature
/// reused on a review. Every statement kind has its own first line.
#[test]
fn statements_cannot_be_replayed_as_another_kind() {
    let (founder, f, mut alice, a) = founder_and_alice();
    let mut o = org();
    f.sign_reviewer_organisation(ALICE, &mut o).unwrap();
    let replayed = DevelopingOrganisation {
        signer: o.signer.clone(),
        signature: o.signature.clone(),
        ..dev(None)
    };
    let mut rt = root(vec![founder.clone(), alice.clone()]);
    rt.code_review.as_mut().unwrap().developing_organisation = vec![replayed];
    assert!(matches!(
        developing_organisation(&rt, &Registry::build(&rt), "x", "2026-10-08", E),
        Err(OrganisationProblem::Unverified {
            problem: SignerProblem::BadSignature,
            ..
        })
    ));
    let mut ws = dev(None);
    f.sign_developing_organisation(&mut ws).unwrap();
    let as_override = DevelopingOrganisation {
        krate: Some("x".into()),
        ..ws.clone()
    };
    rt.code_review.as_mut().unwrap().developing_organisation = vec![as_override];
    assert!(matches!(
        developing_organisation(&rt, &Registry::build(&rt), "x", "2026-10-08", E),
        Err(OrganisationProblem::Unverified {
            problem: SignerProblem::BadSignature,
            ..
        })
    ));
    // An attestation's signature on Alice's review.
    let mut x = att();
    a.attest_separation(&mut x).unwrap();
    let mut r = review_entry("crates/tampines/src/steam.rs::flash", ALICE, "2026-10-08");
    r.review.signature = Some(Signature {
        key: "a1".into(),
        alg: ALG.into(),
        value: x.signature.clone().unwrap(),
    });
    alice.separations = vec![x];
    let reg = Registry::build(&root(vec![founder, alice]));
    assert_eq!(
        unverified(verify_review(&r, &reg)),
        UnverifiedReason::BadSignature
    );
    let firsts: std::collections::BTreeSet<Vec<u8>> = [
        developing_organisation_bytes(&ws),
        reviewer_organisation_bytes(ALICE, &o),
        separation_attestation_bytes(ALICE, &att()),
        signed_bytes(&r),
        admission_bytes(&crate::review::signing::tests::person(
            ALICE,
            Role::Reviewer,
            vec![],
        )),
    ]
    .iter()
    .map(|b| b.split(|c| *c == b'\n').next().unwrap().to_vec())
    .collect();
    assert_eq!(firsts.len(), 5);
    let text = String::from_utf8(separation_attestation_bytes(ALICE, &att())).unwrap();
    assert!(
        text.starts_with("kovan-separation-attestation-v1\n"),
        "{text}"
    );
    assert!(
        text.contains("both technically and managerially separate"),
        "{text}"
    );
}

/// The v3 review bytes (#809): naming an attestation signs it under the v3
/// header; removing or changing it breaks the signature; a review that names
/// none keeps its v1 (no `signed_at`) or v2 bytes, so earlier signatures
/// still verify.
#[test]
fn review_v3_signs_the_attestation_and_v1_v2_are_unchanged() {
    let (founder, _f, alice, a) = founder_and_alice();
    let reg = Registry::build(&root(vec![founder, alice]));
    let v1 = String::from_utf8(signed_bytes(&review_entry(F, ALICE, "2026-10-08"))).unwrap();
    assert!(
        v1.starts_with("kovan-review-signature-v1\n") && !v1.contains("separation_attestation")
    );
    let mut r = review_entry(F, ALICE, "2026-10-08");
    a.sign_review_at(&mut r, "2026-10-08T10:00:00+08:00")
        .unwrap();
    let v2 = String::from_utf8(signed_bytes(&r)).unwrap();
    assert!(
        v2.starts_with("kovan-review-signature-v2\n") && !v2.contains("separation_attestation")
    );
    assert!(verify_review(&r, &reg).is_verified());
    r.review.separation_attestation = Some("sep-1".into());
    assert_eq!(
        unverified(verify_review(&r, &reg)),
        UnverifiedReason::BadSignature,
        "adding it is an edit"
    );
    a.sign_review_at(&mut r, "2026-10-08T10:00:00+08:00")
        .unwrap();
    let v3 = String::from_utf8(signed_bytes(&r)).unwrap();
    assert!(v3.starts_with("kovan-review-signature-v3\n"), "{v3}");
    assert!(
        v3.contains("signed_at=\"2026-10-08T10:00:00+08:00\"\n"),
        "{v3}"
    );
    assert!(
        v3.contains("no_concept=\"\"\nseparation_attestation=\"sep-1\"\n"),
        "{v3}"
    );
    assert!(verify_review(&r, &reg).is_verified());
    for edit in [None, Some("sep-2".to_string())] {
        let mut t = r.clone();
        t.review.separation_attestation = edit;
        assert_eq!(
            unverified(verify_review(&t, &reg)),
            UnverifiedReason::BadSignature
        );
    }
    // v3 without signed_at still carries the attestation line.
    let mut no_time = r.clone();
    no_time.review.signed_at = None;
    let b = String::from_utf8(signed_bytes(&no_time)).unwrap();
    assert!(b.starts_with("kovan-review-signature-v3\n") && !b.contains("signed_at="));
}

/// The keystore refuses what could never count: a malformed date on any of
/// the three statements, and an attestation with no audit record or a
/// malformed one. Nothing is changed on a refusal.
#[test]
fn keystore_refuses_bad_statements() {
    let (_founder, f, _alice, a) = founder_and_alice();
    let mut x = SeparationAttestation {
        date: "08-10-2026".into(),
        ..att()
    };
    assert_eq!(
        a.attest_separation(&mut x),
        Err(SignError::BadDate("08-10-2026".into()))
    );
    let mut x = SeparationAttestation {
        audit_record: None,
        ..att()
    };
    assert_eq!(
        a.attest_separation(&mut x),
        Err(SignError::BadAuditRecord(AuditRecordProblem::Missing))
    );
    let pr = "https://github.com/o/r/pull/1";
    let mut x = SeparationAttestation {
        audit_record: Some(pr.into()),
        ..att()
    };
    let e = a.attest_separation(&mut x).unwrap_err();
    assert_eq!(
        e,
        SignError::BadAuditRecord(AuditRecordProblem::NotAnIssueUrl(pr.into()))
    );
    assert!(e.to_string().contains("issues"));
    assert_eq!((x.key, x.signature), (None, None));
    let mut d = DevelopingOrganisation {
        date: "x".into(),
        ..dev(None)
    };
    assert_eq!(
        f.sign_developing_organisation(&mut d),
        Err(SignError::BadDate("x".into()))
    );
    let mut o = ReviewerOrganisation {
        date: "x".into(),
        ..org()
    };
    assert_eq!(
        f.sign_reviewer_organisation(ALICE, &mut o),
        Err(SignError::BadDate("x".into()))
    );
}
