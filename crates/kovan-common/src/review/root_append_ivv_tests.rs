//! Tests of the IV&V appenders (GitHub #810): developing organisation,
//! reviewer organisation and separation attestation, appended as text to a
//! commented `kovan_root.toml`, signed with real keys, and judged by the
//! #809 registry checks.

use super::*;
use crate::review::engine::SignaturePolicy;
use crate::review::ivv::record_warnings;
use crate::review::signing::keystore::generate;
use crate::review::signing::registry::Registry;

const PASS: &str = "test passphrase, not a real one";
const DAY: &str = "2026-10-10";
const URL: &str = "https://github.com/theodoreOnzGit/outram-park-backend/issues/810";

/// A commented root with the founder `github:m` (key `k1`) and reviewer
/// `github:v` (key `v1`, admitted by `github:m`), then a trailing table.
fn fixture() -> (
    String,
    crate::review::signing::keystore::UnlockedKey,
    crate::review::signing::keystore::UnlockedKey,
) {
    let (mf, mk) = generate("github:m", "k1", DAY, PASS).unwrap();
    let (vf, vk) = generate("github:v", "v1", DAY, PASS).unwrap();
    let mut text = String::from(
        "# Root. Comments must survive.\nschema_version = 1\n\n[code_review]\nfounder = \"github:m\" # kept\n",
    );
    let m = founding_reviewer("github:m", None, mf.reviewer_key(), DAY);
    text = append_reviewer(&text, &m).unwrap();
    let mut v = unadmitted_reviewer("github:v", Some("Verifier"), vf.reviewer_key());
    v.admitted = Some(DAY.into());
    mk.admit(&mut v).unwrap();
    text = append_reviewer(&text, &v).unwrap();
    text.push_str("\n# Trailing comment.\n[library]\nid = \"demo\"\n");
    (text, mk, vk)
}

/// Methodology: append, to the fixture, a workspace developing
/// organisation and a per-crate override (signed by the maintainer), the
/// verifier's organisation (signed by the maintainer, spliced into the
/// second reviewer's section, before the trailing table) and a separation
/// attestation
/// (signed by the verifier). Pass: every original line kept in order, each
/// record reads back where it belongs, the registry finds no warning (all
/// signatures verify), and refusals are typed: a duplicate attestation id,
/// an unknown reviewer.
///
/// Result (2026-10-10): passes.
#[test]
fn ivv_records_append_signed_and_verify() {
    let (base, mk, vk) = fixture();
    let mut dev = DevelopingOrganisation {
        krate: None,
        name: "Outram Park project".into(),
        date: DAY.into(),
        signer: None,
        signature: None,
    };
    mk.sign_developing_organisation(&mut dev).unwrap();
    let t = append_developing_organisation(&base, &dev).unwrap();
    assert!(t.starts_with(&base));
    let mut over = DevelopingOrganisation {
        krate: Some("tampines".into()),
        name: "Tampines group".into(),
        ..dev.clone()
    };
    mk.sign_developing_organisation(&mut over).unwrap();
    let t = append_developing_organisation(&t, &over).unwrap();

    let mut org = ReviewerOrganisation {
        name: "Example IV&V Ltd".into(),
        date: DAY.into(),
        signer: None,
        signature: None,
    };
    mk.sign_reviewer_organisation("github:v", &mut org).unwrap();
    let t = append_reviewer_organisation(&t, "github:v", &org).unwrap();

    let mut a = SeparationAttestation {
        id: "sep-1".into(),
        organisation: org.name.clone(),
        developing_organisation: dev.name.clone(),
        date: DAY.into(),
        audit_record: Some(URL.into()),
        key: None,
        signature: None,
    };
    vk.attest_separation(&mut a).unwrap();
    let t = append_separation_attestation(&t, "github:v", &a).unwrap();

    let mut rest = t.as_str();
    for l in base.lines().filter(|l| !l.is_empty()) {
        let at = rest
            .find(l)
            .unwrap_or_else(|| panic!("lost or reordered {l:?} in\n{t}"));
        rest = &rest[at + l.len()..];
    }
    assert!(
        t.find("id = \"sep-1\"").unwrap() < t.find("# Trailing comment.").unwrap(),
        "spliced into the reviewer's section:\n{t}"
    );
    let r = ReviewRoot::parse(&t).unwrap();
    let cr = r.code_review.as_ref().unwrap();
    assert_eq!(cr.developing_organisation, vec![dev, over]);
    assert_eq!(cr.founder.as_deref(), Some("github:m"));
    let v = r.reviewer("github:v").unwrap();
    assert_eq!(v.organisations, vec![org]);
    assert_eq!(v.separations, vec![a.clone()]);
    assert!(r.reviewer("github:m").unwrap().organisations.is_empty());
    let reg = Registry::build(&r);
    assert_eq!(
        record_warnings(&r, &reg, SignaturePolicy::Enforce),
        vec![],
        "every record verifies"
    );

    assert!(matches!(
        append_separation_attestation(&t, "github:v", &a),
        Err(AppendError::AttestationExists { .. })
    ));
    assert!(AppendError::AttestationExists {
        reviewer: "github:v".into(),
        id: "sep-1".into()
    }
    .to_string()
    .contains("sep-1"));
    assert!(matches!(
        append_reviewer_organisation(&t, "github:nobody", &v.organisations[0]),
        Err(AppendError::UnknownReviewer(_))
    ));
    assert!(matches!(
        append_separation_attestation(&t, "github:nobody", &a),
        Err(AppendError::UnknownReviewer(_))
    ));
}

/// Methodology: a developing organisation appended to a root with no
/// `[code_review]` table, and a signer with no reviewer id (legacy shape).
/// Pass: both read back as written.
///
/// Result (2026-10-10): passes.
#[test]
fn developing_organisation_without_code_review_table() {
    let e = DevelopingOrganisation {
        krate: None,
        name: "A \"quoted\" org".into(),
        date: DAY.into(),
        signer: Some(KeySigner {
            reviewer: None,
            key: "k1".into(),
        }),
        signature: Some("c2ln".into()),
    };
    let t = append_developing_organisation("# only a comment\n", &e).unwrap();
    let r = ReviewRoot::parse(&t).unwrap();
    assert_eq!(r.code_review.unwrap().developing_organisation, vec![e]);
}
