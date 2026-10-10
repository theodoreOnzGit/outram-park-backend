//! Tests of the one signed-bytes format, v3 (GitHub #825), and of
//! `signed_at` in it (GitHub #783). Since #825 any change to the signed
//! bytes fails here; from the first real stamp on, a change is a new
//! header, never an edit.
//!
//! Methodology. `fixtures/review_v3.md` holds a review that carries every
//! signed field (`signed_at`, `separation_attestation`, two callees, two
//! checklist answers, two authorship sessions written out of order,
//! `no_concept`, a relation) and an architecture node (`signed_at`, two
//! members, a pattern, a relation), signed on 2026-10-10 with the
//! deterministic ed25519 key whose seed is 32 bytes of `7` (public key
//! [`FIXTURE_PUBLIC`]); `fixtures/*_v3.signed.txt` are the exact bytes
//! signed. Each test parses the fixture with the ordinary reader, rebuilds
//! the signed bytes with today's code, and verifies against a registry
//! holding that key as the founder's. Pass criterion: byte equality with
//! the committed images; signing those bytes again with the seed key gives
//! the committed signature (ed25519 signatures are deterministic);
//! `SignatureCheck::Verified` for both entries; the file renders back byte
//! for byte; and the exact `UnverifiedReason` / `SignError` predicted for
//! each edit.
//!
//! ~~`fixtures/review_v1.md`, signed before #783, pinned the v1 bytes~~
//! **CORRECTED 2026-10-10** (#825): the v1 and v2 headers were dropped
//! (no signed `review.md` existed), and with them that fixture.
//!
//! Result (2026-10-07, v1 fixture): all pass. Result (2026-10-10, v3
//! fixture): all pass (`cargo test --release -j 6 -p kovan-common --lib`).

use ed25519_dalek::{Signer as _, SigningKey};

use super::keystore::SignError;
use super::tests::{key, person, root, signed, unverified, F, FOUNDER};
use super::*;
use crate::review::review_md::{parse_review_md, render_review_md, ReviewDocument};
use crate::review::root::{KeyEvent, KeyEventKind, ReviewerKey, Role};
use crate::review::signed_at::parse_rfc3339;

/// The public half of the fixture key (seed `[7u8; 32]`).
const FIXTURE_PUBLIC: &str = "6kpsY+KcUgq+9VB7Ey7F+ZVHdq6+vnuSQh7qaRRG0iw=";
const V3_MD: &str = include_str!("fixtures/review_v3.md");

fn seed_key() -> SigningKey {
    SigningKey::from_bytes(&[7u8; 32])
}

fn v3_doc() -> ReviewDocument {
    parse_review_md(V3_MD)
}

fn v3_registry() -> Registry {
    let k = ReviewerKey {
        id: "k1".into(),
        alg: ALG.into(),
        public: FIXTURE_PUBLIC.into(),
        created: "2026-10-10".into(),
        endorsed_by: None,
        reset: false,
        retired: false,
        retired_on: None,
        unretired: None,
        history: vec![KeyEvent::unsigned(KeyEventKind::Created, "2026-10-10")],
    };
    Registry::build(&root(vec![person(FOUNDER, Role::Maintainer, vec![k])]))
}

/// The v3 fixture loads with every entry readable, nothing left out, and
/// writes back byte for byte (it was written by the renderer).
#[test]
fn v3_fixture_loads_and_writes_back_unchanged() {
    let doc = v3_doc();
    assert!(doc.unreadable.is_empty(), "{:?}", doc.unreadable);
    let r = doc.reviews().next().unwrap();
    assert_eq!(r.review.signed_at.as_deref(), Some("2026-10-10T14:03:09+08:00"));
    assert_eq!(r.review.separation_attestation.as_deref(), Some("sep-2026-10-10"));
    let a = doc.architectures().next().unwrap();
    assert_eq!(a.architecture.signed_at.as_deref(), Some("2026-10-10T14:05:00+08:00"));
    assert_eq!(render_review_md(&doc.entries).unwrap(), V3_MD);
}

/// **The pin (#825).** The v3 signed bytes are exactly the committed
/// images; the seed key reproduces the committed signatures over them; both
/// entries verify; and removing `signed_at` or the attestation from the
/// signed review is an edit to signed bytes.
#[test]
fn v3_fixture_bytes_pinned_and_verified() {
    let doc = v3_doc();
    let reg = v3_registry();
    let sk = seed_key();
    assert_eq!(encode_b64(sk.verifying_key().as_bytes()), FIXTURE_PUBLIC);

    let r = doc.reviews().next().unwrap();
    let bytes = signed_bytes(r);
    assert_eq!(
        String::from_utf8(bytes.clone()).unwrap(),
        include_str!("fixtures/review_v3.signed.txt")
    );
    assert_eq!(
        encode_b64(&sk.sign(&bytes).to_bytes()),
        r.review.signature.as_ref().unwrap().value
    );
    let SignatureCheck::Verified(v) = verify_review(r, &reg) else {
        panic!("v3 review: {:?}", verify_review(r, &reg))
    };
    assert_eq!((v.reviewer.as_str(), v.key.as_str()), (FOUNDER, "k1"));

    let a = doc.architectures().next().unwrap();
    let bytes = architecture_signed_bytes(a);
    assert_eq!(
        String::from_utf8(bytes.clone()).unwrap(),
        include_str!("fixtures/architecture_v3.signed.txt")
    );
    assert_eq!(
        encode_b64(&sk.sign(&bytes).to_bytes()),
        a.architecture.signature.as_ref().unwrap().value
    );
    assert!(verify_architecture(a, &reg).is_verified());

    let mut t = r.clone();
    t.review.signed_at = None;
    assert_eq!(unverified(verify_review(&t, &reg)), UnverifiedReason::BadSignature);
    let mut t = r.clone();
    t.review.separation_attestation = None;
    assert_eq!(unverified(verify_review(&t, &reg)), UnverifiedReason::BadSignature);
    let mut t = a.clone();
    t.architecture.signed_at = None;
    assert_eq!(unverified(verify_architecture(&t, &reg)), UnverifiedReason::BadSignature);
}

/// `signed_at` is in the signed bytes under the v3 header (~~v2~~, #825),
/// right after `date`; changing or stripping it breaks the signature; it
/// survives a write and re-read.
#[test]
fn signed_at_is_signed() {
    let (fk, f) = key(FOUNDER, "k1");
    let reg = Registry::build(&root(vec![person(FOUNDER, Role::Maintainer, vec![fk])]));
    let at = "2026-10-07T14:03:09+08:00";
    let mut r = crate::review::signing::tests::review_entry(F, FOUNDER, "2026-10-07");
    f.sign_review_at(&mut r, at).unwrap();
    assert_eq!(r.review.signed_at.as_deref(), Some(at));
    let text = String::from_utf8(signed_bytes(&r)).unwrap();
    assert!(text.starts_with("kovan-review-signature-v3\n"), "{text}");
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

/// `sign_review` and `sign_architecture` stamp the clock in local time with
/// the machine's offset (maintainer, 2026-10-07: `chrono`); the `_at`
/// forms refuse a timestamp that is not RFC 3339 and leave the entry alone.
#[test]
fn signing_sets_signed_at_from_the_clock() {
    let (fk, f) = key(FOUNDER, "k1");
    let reg = Registry::build(&root(vec![person(FOUNDER, Role::Maintainer, vec![fk])]));
    let r = signed(&f, F, "2026-10-07");
    let at = r.review.signed_at.clone().unwrap();
    let local = chrono::Local::now().offset().local_minus_utc() / 60;
    assert_eq!(
        parse_rfc3339(&at).map(|t| t.offset_minutes),
        Some(local),
        "{at}"
    );
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
        .starts_with("kovan-review-signature-v3\n"));
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
