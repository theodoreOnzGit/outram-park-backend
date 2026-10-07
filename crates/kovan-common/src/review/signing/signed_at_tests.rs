//! Tests of `signed_at` in the signed bytes (GitHub #783): v1 stamps signed
//! before #783 still load and verify byte for byte; v2 stamps sign
//! `signed_at` and any edit to it, or its removal, breaks the signature.
//!
//! Methodology. `fixtures/review_v1.md` holds a review and an architecture
//! node signed **before** #783 by the code then in `signing.rs`, with the
//! deterministic ed25519 key whose seed is 32 bytes of `7` (public key
//! [`V1_PUBLIC`]); `fixtures/*_v1.signed.txt` are the exact bytes that code
//! signed. Each test parses the fixture with the ordinary reader, rebuilds
//! the signed bytes with today's code and verifies against a registry
//! holding that key as the founder's. Pass criterion: byte equality with
//! the recorded images, `SignatureCheck::Verified` for both v1 entries, and
//! the exact `UnverifiedReason` / `SignError` predicted for each edit.
//!
//! Result (2026-10-07): all pass (`cargo test --release -j 12 -p
//! kovan-common --lib --tests`).

use super::keystore::SignError;
use super::tests::{key, person, root, signed, unverified, F, FOUNDER};
use super::*;
use crate::review::review_md::{parse_review_md, render_review_md, ReviewDocument};
use crate::review::root::{KeyEvent, KeyEventKind, ReviewerKey, Role};
use crate::review::signed_at::parse_rfc3339;

/// The public half of the fixture key (seed `[7u8; 32]`).
const V1_PUBLIC: &str = "6kpsY+KcUgq+9VB7Ey7F+ZVHdq6+vnuSQh7qaRRG0iw=";
const V1_MD: &str = include_str!("fixtures/review_v1.md");

fn v1_doc() -> ReviewDocument {
    parse_review_md(V1_MD)
}

fn v1_registry() -> Registry {
    let k = ReviewerKey {
        id: "k1".into(),
        alg: ALG.into(),
        public: V1_PUBLIC.into(),
        created: "2026-10-07".into(),
        endorsed_by: None,
        reset: false,
        retired: false,
        retired_on: None,
        unretired: None,
        history: vec![KeyEvent::unsigned(KeyEventKind::Created, "2026-10-07")],
    };
    Registry::build(&root(vec![person(FOUNDER, Role::Maintainer, vec![k])]))
}

/// Schemas are additive (#783): a `review.md` written before `signed_at`
/// existed loads with every entry readable, no `signed_at`, nothing
/// migrated, and writes back byte for byte.
#[test]
fn old_review_md_loads_unchanged() {
    let doc = v1_doc();
    assert!(doc.unreadable.is_empty(), "{:?}", doc.unreadable);
    assert!(doc.migrated.is_empty());
    let r = doc.reviews().next().unwrap();
    assert_eq!(r.review.signed_at, None);
    let a = doc.architectures().next().unwrap();
    assert_eq!(a.architecture.signed_at, None);
    let out = render_review_md(&doc.entries).unwrap();
    assert!(!out.contains("signed_at ="));
    assert_eq!(out, V1_MD);
}

/// v1 signed bytes are byte-identical to what was signed before #783, and
/// both v1 entries still verify.
#[test]
fn v1_fixture_bytes_identical_and_verified() {
    let doc = v1_doc();
    let reg = v1_registry();
    let r = doc.reviews().next().unwrap();
    assert_eq!(
        signed_bytes(r),
        include_bytes!("fixtures/review_v1.signed.txt")
    );
    let SignatureCheck::Verified(v) = verify_review(r, &reg) else {
        panic!("v1 review: {:?}", verify_review(r, &reg))
    };
    assert_eq!((v.reviewer.as_str(), v.key.as_str()), (FOUNDER, "k1"));
    let a = doc.architectures().next().unwrap();
    assert_eq!(
        architecture_signed_bytes(a),
        include_bytes!("fixtures/architecture_v1.signed.txt")
    );
    assert!(verify_architecture(a, &reg).is_verified());

    // Adding a signed_at to a v1 stamp is an edit to signed bytes.
    let mut t = r.clone();
    t.review.signed_at = Some("2026-10-07T10:00:00+08:00".into());
    assert_eq!(
        unverified(verify_review(&t, &reg)),
        UnverifiedReason::BadSignature
    );
    let mut t = a.clone();
    t.architecture.signed_at = Some("2026-10-07T10:00:00+08:00".into());
    assert_eq!(
        unverified(verify_architecture(&t, &reg)),
        UnverifiedReason::BadSignature
    );
}

/// v2: `signed_at` is in the signed bytes under the v2 header, right after
/// `date`; changing or stripping it breaks the signature; it survives a
/// write and re-read.
#[test]
fn v2_signs_signed_at() {
    let (fk, f) = key(FOUNDER, "k1");
    let reg = Registry::build(&root(vec![person(FOUNDER, Role::Maintainer, vec![fk])]));
    let at = "2026-10-07T14:03:09+08:00";
    let mut r = crate::review::signing::tests::review_entry(F, FOUNDER, "2026-10-07");
    f.sign_review_at(&mut r, at).unwrap();
    assert_eq!(r.review.signed_at.as_deref(), Some(at));
    let text = String::from_utf8(signed_bytes(&r)).unwrap();
    assert!(text.starts_with("kovan-review-signature-v2\n"), "{text}");
    assert!(
        text.contains("date=\"2026-10-07\"\nsigned_at=\"2026-10-07T14:03:09+08:00\"\ncommit="),
        "{text}"
    );
    assert!(verify_review(&r, &reg).is_verified());

    let mut t = r.clone();
    t.review.signed_at = Some("2026-10-07T14:03:10+08:00".into());
    assert_eq!(
        unverified(verify_review(&t, &reg)),
        UnverifiedReason::BadSignature
    );
    let mut t = r.clone();
    t.review.signed_at = None;
    assert_eq!(
        unverified(verify_review(&t, &reg)),
        UnverifiedReason::BadSignature
    );

    // Written and read back: the field round-trips and still verifies.
    let entry = crate::review::review_md::Entry::Review(r.clone());
    let toml = entry.to_toml().unwrap();
    assert!(
        toml.contains("signed_at = \"2026-10-07T14:03:09+08:00\""),
        "{toml}"
    );
    let md = format!("# Review\n\n```toml\n{toml}```\n");
    let back = parse_review_md(&md);
    assert!(back.unreadable.is_empty(), "{:?}", back.unreadable);
    let rr = back.reviews().next().unwrap();
    assert_eq!(rr.review.signed_at.as_deref(), Some(at));
    assert!(verify_review(rr, &reg).is_verified());
}

/// `sign_review` and `sign_architecture` stamp the clock in UTC; the `_at`
/// forms refuse a timestamp that is not RFC 3339 and leave the entry alone.
#[test]
fn signing_sets_signed_at_from_the_clock() {
    let (fk, f) = key(FOUNDER, "k1");
    let reg = Registry::build(&root(vec![person(FOUNDER, Role::Maintainer, vec![fk])]));
    let r = signed(&f, F, "2026-10-07");
    let at = r.review.signed_at.clone().unwrap();
    assert_eq!(
        parse_rfc3339(&at).map(|t| t.offset_minutes),
        Some(0),
        "{at}"
    );
    assert!(at.ends_with("+00:00"));
    assert!(verify_review(&r, &reg).is_verified());

    let mut a = crate::review::review_md::ArchitectureEntry {
        kovan: r.kovan.clone(),
        architecture: crate::review::review_md::ArchitectureBody {
            by: FOUNDER.into(),
            date: "2026-10-07".into(),
            signed_at: None,
            commit: r.review.commit.clone(),
            members: vec![r.function_id()],
            member_paths: vec![F.into()],
            upstream: None,
            upstream_tag: None,
            pattern: None,
            signature: None,
        },
        relations: vec![],
    };
    f.sign_architecture(&mut a).unwrap();
    assert!(a
        .architecture
        .signed_at
        .as_deref()
        .and_then(parse_rfc3339)
        .is_some());
    assert!(String::from_utf8(architecture_signed_bytes(&a))
        .unwrap()
        .starts_with("kovan-review-signature-v2\n"));
    assert!(verify_architecture(&a, &reg).is_verified());
    f.sign_architecture_at(&mut a, "2026-10-07T09:00:00Z")
        .unwrap();
    assert_eq!(
        a.architecture.signed_at.as_deref(),
        Some("2026-10-07T09:00:00Z")
    );
    assert!(verify_architecture(&a, &reg).is_verified());

    let mut b = crate::review::signing::tests::review_entry(F, FOUNDER, "2026-10-07");
    let before = b.clone();
    assert_eq!(
        f.sign_review_at(&mut b, "2026-10-07 09:00"),
        Err(SignError::BadSignedAt("2026-10-07 09:00".into()))
    );
    assert_eq!(b, before);
    let before = a.clone();
    assert_eq!(
        f.sign_architecture_at(&mut a, "noon"),
        Err(SignError::BadSignedAt("noon".into()))
    );
    assert_eq!(a, before);
    assert!(SignError::BadSignedAt("noon".into())
        .to_string()
        .contains("not RFC 3339"));
}
