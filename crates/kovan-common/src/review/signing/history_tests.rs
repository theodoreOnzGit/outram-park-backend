//! Tests of the append-only key history (maintainer on #762, 2026-10-07):
//! state derived by replay, retire/unretire cycles, the possession proof,
//! key revocation, tampered entries in the warnings, retired keys signing
//! nothing, and the v1 legacy fields migrating on load.
//!
//! Methodology: as in `tests.rs` (real argon2id + AES-256-GCM + ed25519
//! keys, real signed bytes, exact typed results). The v1 migration test
//! signs the v1 statement bytes directly with `ed25519-dalek`, the way #762
//! v1 wrote them, and loads the committed v1 fixture.
//!
//! Result (2026-10-07): all pass (`cargo test --release -j 1 -p kovan-common
//! --lib --tests`).

use ed25519_dalek::{Signer as _, SigningKey};

use super::keystore::{self, Keystore, SignError};
use super::registry::{
    open_retirement, retire_key, Admission, AdmissionProblem, FounderProblem, HistoryProblem,
    InactiveKind, KeyProblem, KeyStatus, LifecycleError, Registry, RevocationProblem,
    SignerProblem, Warning, Window,
};
use super::tests::{
    founder_and_alice, key, person, root, signed, unverified, ALICE, F, FOUNDER, PASS,
};
use super::*;
use crate::review::root::{
    KeyEvent, KeyEventKind, KeySignature, KeySigner, ReviewRoot, ReviewerKey, Revocation, Role,
    Unretirement,
};

fn history_warnings(reg: &Registry, reviewer: &str, key: &str) -> Vec<HistoryProblem> {
    reg.warnings()
        .into_iter()
        .filter_map(|w| match w {
            Warning::KeyHistory { reviewer: r, key: k, problem } if r == reviewer && k == key => Some(problem),
            _ => None,
        })
        .collect()
}

fn founder_with(keys: Vec<ReviewerKey>) -> Registry {
    Registry::build(&root(vec![person(FOUNDER, Role::Maintainer, keys)]))
}

/// Retire, un-retire, retire, un-retire: every event is kept, each gap is
/// void, and the un-retirements need the key unlocked from its encrypted
/// file (the keystore round trip is the possession proof).
#[test]
fn retire_unretire_cycles_keep_every_event() {
    let dir = tempfile::tempdir().unwrap();
    let ks = Keystore::at(dir.path());
    let (file, k) = keystore::generate(FOUNDER, "k1", "2026-10-07", PASS).unwrap();
    ks.save(&file).unwrap();
    drop(k); // the app was closed
    let mut k1 = file.reviewer_key();
    retire_key(&mut k1, "2026-10-10").unwrap();
    assert_eq!(retire_key(&mut k1, "2026-10-11"), Err(LifecycleError::AlreadyRetired));
    assert_eq!(open_retirement(&k1).as_deref(), Some("2026-10-10"));
    let old = ks.load(FOUNDER, "k1").unwrap().unlock(PASS).unwrap();
    assert_eq!(
        old.unretire(&mut k1, "2026-10-09"),
        Err(SignError::Lifecycle(LifecycleError::BadDate("2026-10-09".into())))
    );
    old.unretire(&mut k1, "2026-10-15").unwrap();
    assert_eq!(old.unretire(&mut k1, "2026-10-16"), Err(SignError::NotRetired));
    retire_key(&mut k1, "2026-10-20").unwrap();
    old.unretire(&mut k1, "2026-10-25").unwrap();
    assert_eq!(retire_key(&mut k1, "today"), Err(LifecycleError::BadDate("today".into())));
    let kinds: Vec<KeyEventKind> = k1.history.iter().map(|e| e.event).collect();
    use KeyEventKind::*;
    assert_eq!(kinds, vec![Created, Retired, Unretired, Retired, Unretired]);

    let reg = founder_with(vec![k1.clone()]);
    assert!(reg.warnings().is_empty(), "{:?}", reg.warnings());
    let kt = reg.reviewer(FOUNDER).unwrap().key("k1").unwrap();
    assert!(!kt.is_retired());
    assert_eq!(
        kt.windows,
        vec![
            Window { kind: InactiveKind::Retired, from: Some("2026-10-10".into()), to: Some("2026-10-15".into()) },
            Window { kind: InactiveKind::Retired, from: Some("2026-10-20".into()), to: Some("2026-10-25".into()) },
        ]
    );
    for (date, ok) in [
        ("2026-10-09", true),
        ("2026-10-12", false),
        ("2026-10-16", true),
        ("2026-10-22", false),
        ("2026-10-26", true),
    ] {
        assert_eq!(verify_review(&signed(&old, F, date), &reg).is_verified(), ok, "{date}");
    }
    assert_eq!(
        unverified(verify_review(&signed(&old, F, "2026-10-22"), &reg)),
        UnverifiedReason::KeyRetired { key: "k1".into(), since: Some("2026-10-20".into()) }
    );
    // Retired again, and still retired.
    retire_key(&mut k1, "2026-11-01").unwrap();
    let reg = founder_with(vec![k1]);
    assert!(reg.reviewer(FOUNDER).unwrap().key("k1").unwrap().is_retired());
}

/// An un-retirement that is unsigned, signed by another key, or names
/// another signer does not count: the key stays retired and the entry is a
/// warning. Another key cannot even produce one.
#[test]
fn unretire_needs_possession() {
    let (mut k1, old) = key(FOUNDER, "k1");
    retire_key(&mut k1, "2026-10-10").unwrap();
    let (_, impostor) = key(FOUNDER, "k1");
    assert_eq!(impostor.unretire(&mut k1.clone(), "2026-10-15"), Err(SignError::WrongKey));

    let mut by_hand = k1.clone();
    by_hand.history.push(KeyEvent::unsigned(KeyEventKind::Unretired, "2026-10-15"));
    let mut forged = k1.clone();
    forged.history.push(KeyEvent {
        event: KeyEventKind::Unretired,
        date: "2026-10-15".into(),
        signer: Some(KeySigner { reviewer: Some(FOUNDER.into()), key: "k1".into() }),
        signature: Some(encode_b64(&[7u8; 64])),
        legacy: false,
    });
    let mut other_signer = k1.clone();
    other_signer.history.push(KeyEvent {
        signer: Some(KeySigner { reviewer: Some(FOUNDER.into()), key: "k2".into() }),
        ..forged.history.last().unwrap().clone()
    });
    for (k, want) in [
        (by_hand, SignerProblem::Unsigned),
        (forged, SignerProblem::BadSignature),
        (other_signer, SignerProblem::UnknownKey { key: "k2".into() }),
    ] {
        let reg = founder_with(vec![k]);
        assert!(reg.reviewer(FOUNDER).unwrap().key("k1").unwrap().is_retired());
        assert_eq!(
            history_warnings(&reg, FOUNDER, "k1"),
            vec![HistoryProblem::BadEventSignature { index: 2, event: KeyEventKind::Unretired, problem: want }]
        );
        assert!(!verify_review(&signed(&old, F, "2026-10-20"), &reg).is_verified());
    }
}

/// A tampered or misplaced entry shows in the warnings: an edited endorsed
/// date (untrusted), a tampered second endorsement while the first still
/// holds (trusted, warned), a moved retirement under a signed un-retirement
/// (whose signature binds the retirement date), out-of-order and malformed
/// dates, a missing or repeated `created`, a double retirement, a deleted
/// retirement, and `admitted` on a later key.
#[test]
fn tampered_entries_show_in_warnings() {
    let (k1, u1) = key(FOUNDER, "k1");
    let (mut k2, _) = key(FOUNDER, "k2");
    u1.endorse(FOUNDER, &mut k2, "2026-10-08").unwrap();

    let mut edited = k2.clone();
    edited.history[1].date = "2026-10-09".into();
    let reg = founder_with(vec![k1.clone(), edited]);
    let bad = HistoryProblem::BadEventSignature {
        index: 1,
        event: KeyEventKind::Endorsed,
        problem: SignerProblem::BadSignature,
    };
    assert_eq!(history_warnings(&reg, FOUNDER, "k2"), vec![bad]);
    assert!(!reg.reviewer(FOUNDER).unwrap().key("k2").unwrap().status.is_trusted());

    let mut second = k2.clone();
    u1.endorse(FOUNDER, &mut second, "2026-10-09").unwrap();
    second.history[2].date = "2026-10-10".into();
    let reg = founder_with(vec![k1.clone(), second]);
    assert!(reg.reviewer(FOUNDER).unwrap().key("k2").unwrap().status.is_trusted());
    assert_eq!(
        history_warnings(&reg, FOUNDER, "k2"),
        vec![HistoryProblem::BadEventSignature {
            index: 2,
            event: KeyEventKind::Endorsed,
            problem: SignerProblem::BadSignature
        }]
    );

    // Moving the retirement under a signed un-retirement breaks it.
    let (mut k3, u3) = key(FOUNDER, "k3");
    u1.endorse(FOUNDER, &mut k3, "2026-10-07").unwrap();
    retire_key(&mut k3, "2026-10-10").unwrap();
    u3.unretire(&mut k3, "2026-10-15").unwrap();
    let mut moved = k3.clone();
    moved.history[2].date = "2026-10-12".into();
    let reg = founder_with(vec![k1.clone(), moved]);
    let kt = reg.reviewer(FOUNDER).unwrap().key("k3").unwrap();
    assert!(kt.is_retired());
    assert!(matches!(
        history_warnings(&reg, FOUNDER, "k3")[..],
        [HistoryProblem::BadEventSignature { index: 3, event: KeyEventKind::Unretired, .. }]
    ));
    // Deleting the retirement leaves an un-retirement with nothing open.
    let mut deleted = k3.clone();
    deleted.history.remove(2);
    let reg = founder_with(vec![k1.clone(), deleted]);
    assert_eq!(history_warnings(&reg, FOUNDER, "k3"), vec![HistoryProblem::UnretiredWhileActive { index: 2 }]);

    // Structural problems.
    let mut h = k1.clone();
    h.history.push(KeyEvent::unsigned(KeyEventKind::Retired, "2026-10-05"));
    h.history.push(KeyEvent::unsigned(KeyEventKind::Retired, "2026-10-06"));
    h.history.push(KeyEvent::unsigned(KeyEventKind::Created, "2026-10-07"));
    let reg = founder_with(vec![h]);
    assert_eq!(
        history_warnings(&reg, FOUNDER, "k1"),
        vec![
            HistoryProblem::OutOfOrder { index: 1 },
            HistoryProblem::RetiredWhileRetired { index: 2 },
            HistoryProblem::CreatedNotFirst { index: 3 },
        ]
    );
    let mut no_created = k1.clone();
    no_created.history = vec![KeyEvent::unsigned(KeyEventKind::Retired, "then")];
    let reg = founder_with(vec![no_created]);
    assert_eq!(
        history_warnings(&reg, FOUNDER, "k1"),
        vec![HistoryProblem::MissingCreated, HistoryProblem::BadDate { index: 0, date: "then".into() }]
    );
    // A malformed retirement date voids the key from the start.
    assert_eq!(
        unverified(verify_review(&signed(&u1, F, "2026-01-01"), &reg)),
        UnverifiedReason::KeyRetired { key: "k1".into(), since: None }
    );

    // `admitted` belongs on the first key only.
    let (founder, f, mut alice, _a) = founder_and_alice();
    let (mut a2, _) = key(ALICE, "a2");
    let mut tmp = alice.clone();
    tmp.keys = vec![a2.clone()];
    f.admit(&mut tmp).unwrap();
    a2 = tmp.keys.remove(0);
    alice.keys.push(a2);
    let reg = Registry::build(&root(vec![founder, alice]));
    assert!(history_warnings(&reg, ALICE, "a2").contains(&HistoryProblem::AdmittedNotOnFirstKey { index: 1 }));
}

/// A `revoked` or `compromised` event voids a key from its date for good;
/// signed by a maintainer, the owner or the key itself; an unsigned one is
/// honoured and warned; a revoked key signs nothing afterwards; another
/// reviewer is not eligible to sign it.
#[test]
fn key_revocation_events() {
    let (founder, f, mut alice, a) = founder_and_alice();
    f.revoke_key(ALICE, &mut alice.keys[0], "2026-11-01", false).unwrap();
    let reg = Registry::build(&root(vec![founder.clone(), alice.clone()]));
    assert!(reg.warnings().is_empty(), "{:?}", reg.warnings());
    assert!(verify_review(&signed(&a, F, "2026-10-31"), &reg).is_verified());
    assert_eq!(
        unverified(verify_review(&signed(&a, F, "2026-11-01"), &reg)),
        UnverifiedReason::KeyRevoked { key: "a1".into(), date: Some("2026-11-01".into()) }
    );

    // Self-signed compromise, and the owner's own revocation.
    let (founder2, _f2, mut alice2, a2) = founder_and_alice();
    a2.revoke_key(ALICE, &mut alice2.keys[0], "2026-10-20", true).unwrap();
    let reg = Registry::build(&root(vec![founder2, alice2]));
    assert!(reg.warnings().is_empty(), "{:?}", reg.warnings());
    assert_eq!(
        unverified(verify_review(&signed(&a2, F, "2026-10-25"), &reg)),
        UnverifiedReason::KeyCompromised { key: "a1".into(), from: Some("2026-10-20".into()) }
    );

    // Unsigned: honoured, and a warning.
    let mut unsigned = alice.clone();
    unsigned.keys[0].history.pop();
    unsigned.keys[0].history.push(KeyEvent::unsigned(KeyEventKind::Revoked, "2026-11-01"));
    let reg = Registry::build(&root(vec![founder.clone(), unsigned]));
    assert!(!verify_review(&signed(&a, F, "2026-11-02"), &reg).is_verified());
    assert_eq!(
        history_warnings(&reg, ALICE, "a1"),
        vec![HistoryProblem::BadEventSignature {
            index: 2,
            event: KeyEventKind::Revoked,
            problem: SignerProblem::Unsigned
        }]
    );

    // A compromised maintainer key cannot admit anyone afterwards.
    let (mut fk, fu) = key(FOUNDER, "k1");
    fu.revoke_key(FOUNDER, &mut fk, "2026-10-08", true).unwrap();
    let (bk, _) = key("github:bob", "b1");
    let mut bob = person("github:bob", Role::Reviewer, vec![bk]);
    bob.admitted = Some("2026-10-09".into());
    fu.admit(&mut bob).unwrap();
    let reg = Registry::build(&root(vec![person(FOUNDER, Role::Maintainer, vec![fk]), bob.clone()]));
    assert_eq!(
        reg.reviewer("github:bob").unwrap().admission,
        Admission::NotAdmitted(AdmissionProblem::Signer(SignerProblem::SignerKeyRevoked {
            signer: FOUNDER.into(),
            key: "k1".into()
        }))
    );

    // Bob (another reviewer) is not eligible to revoke Alice's key.
    let (b_entry, bu) = key("github:bob", "b1");
    let mut bob = person("github:bob", Role::Reviewer, vec![b_entry]);
    bob.admitted = Some("2026-10-07".into());
    f.admit(&mut bob).unwrap();
    let mut victim = founder_and_alice_from(&founder, &f);
    bu.revoke_key(ALICE, &mut victim.keys[0], "2026-11-01", false).unwrap();
    let reg = Registry::build(&root(vec![founder, victim, bob]));
    assert_eq!(
        history_warnings(&reg, ALICE, "a1"),
        vec![HistoryProblem::BadEventSignature {
            index: 2,
            event: KeyEventKind::Revoked,
            problem: SignerProblem::NotEligible("github:bob".into())
        }]
    );
}

/// A fresh Alice admitted by `f` (the founder's unlocked key).
fn founder_and_alice_from(_founder: &crate::review::root::Reviewer, f: &keystore::UnlockedKey) -> crate::review::root::Reviewer {
    let (ak, _) = key(ALICE, "a1");
    let mut alice = person(ALICE, Role::Reviewer, vec![ak]);
    alice.scope = vec!["crates/tampines/**".into()];
    alice.admitted = Some("2026-10-07".into());
    f.admit(&mut alice).unwrap();
    alice
}

/// A retired key may not sign registry statements dated in its retired
/// window (endorsement, admission, revocation), except its own
/// un-retirement; once un-retired it signs everything again (maintainer,
/// #762 Q4). Statements dated before the retirement stay valid, and a
/// refused entry stays in the history as a warning.
#[test]
fn retired_key_cannot_sign_statements() {
    let (mut k1, f) = key(FOUNDER, "k1");
    retire_key(&mut k1, "2026-10-10").unwrap();
    let retired = SignerProblem::SignerRetired { signer: FOUNDER.into(), key: "k1".into() };

    let (mut early, _) = key(FOUNDER, "k2");
    f.endorse(FOUNDER, &mut early, "2026-10-08").unwrap();
    let (mut late, _) = key(FOUNDER, "k3");
    f.endorse(FOUNDER, &mut late, "2026-10-12").unwrap();
    let (ak, _a) = key(ALICE, "a1");
    let mut alice = person(ALICE, Role::Reviewer, vec![ak]);
    alice.admitted = Some("2026-10-12".into());
    f.admit(&mut alice).unwrap();
    let bob_id = "github:bob";
    let (bk, _b) = key(bob_id, "b1");
    let mut bob = person(bob_id, Role::Reviewer, vec![bk]);
    bob.admitted = Some("2026-10-09".into());
    f.admit(&mut bob).unwrap();
    bob.revoked = Some(Revocation { date: "2026-10-12".into(), compromised_from: None, by: FOUNDER.into(), signature: None });
    f.revoke(bob_id, bob.revoked.as_mut().unwrap()).unwrap();

    let founder = person(FOUNDER, Role::Maintainer, vec![k1.clone(), early.clone(), late.clone()]);
    let reg = Registry::build(&root(vec![founder, alice.clone(), bob.clone()]));
    let r = reg.reviewer(FOUNDER).unwrap();
    assert_eq!(r.key("k2").unwrap().status, KeyStatus::Endorsed { by_reviewer: FOUNDER.into(), by_key: "k1".into() });
    assert_eq!(r.key("k3").unwrap().status, KeyStatus::Untrusted(KeyProblem::Endorsement(retired.clone())));
    assert_eq!(
        reg.reviewer(ALICE).unwrap().admission,
        Admission::NotAdmitted(AdmissionProblem::Signer(retired.clone()))
    );
    assert_eq!(reg.reviewer(bob_id).unwrap().admission, Admission::Admitted { by: FOUNDER.into(), key: "k1".into() });
    assert_eq!(
        reg.reviewer(bob_id).unwrap().revocation.as_ref().unwrap().signed,
        Err(RevocationProblem::Signer(retired.clone()))
    );

    // Its own un-retirement works while retired; afterwards new statements
    // count. The refused endorsement stays in k3's history as a warning.
    f.unretire(&mut k1, "2026-10-15").unwrap();
    f.endorse(FOUNDER, &mut late, "2026-10-16").unwrap();
    let mut alice2 = alice.clone();
    alice2.keys[0].history.pop();
    alice2.admitted = Some("2026-10-16".into());
    f.admit(&mut alice2).unwrap();
    let mut bob2 = bob.clone();
    bob2.revoked.as_mut().unwrap().date = "2026-10-16".into();
    f.revoke(bob_id, bob2.revoked.as_mut().unwrap()).unwrap();
    let founder = person(FOUNDER, Role::Maintainer, vec![k1.clone(), early, late]);
    let reg = Registry::build(&root(vec![founder.clone(), alice2, bob2]));
    let r = reg.reviewer(FOUNDER).unwrap();
    assert_eq!(r.key("k3").unwrap().status, KeyStatus::Endorsed { by_reviewer: FOUNDER.into(), by_key: "k1".into() });
    assert_eq!(
        history_warnings(&reg, FOUNDER, "k3"),
        vec![HistoryProblem::BadEventSignature { index: 1, event: KeyEventKind::Endorsed, problem: retired.clone() }]
    );
    assert_eq!(reg.reviewer(ALICE).unwrap().admission, Admission::Admitted { by: FOUNDER.into(), key: "k1".into() });
    assert_eq!(reg.reviewer(bob_id).unwrap().revocation.as_ref().unwrap().signed, Ok("k1".into()));
    // A statement dated inside the old retired window still does not count.
    let reg = Registry::build(&root(vec![founder, alice]));
    assert_eq!(
        reg.reviewer(ALICE).unwrap().admission,
        Admission::NotAdmitted(AdmissionProblem::Signer(retired))
    );
}

/// The v1 fields (`endorsed_by`, `reset`, `retired`, `retired_on`,
/// `unretired`, the reviewer's `admitted_by`) still load: migrated into
/// history as `legacy` events whose v1 signatures still verify, with the
/// same meaning. A key with both forms keeps its history and is warned.
/// The committed v1 fixture still loads.
#[test]
fn legacy_v1_fields_migrate_into_history() {
    let sk = |n: u8| SigningKey::from_bytes(&[n; 32]);
    let pk = |s: &SigningKey| encode_b64(s.verifying_key().as_bytes());
    let sig = |s: &SigningKey, m: &[u8]| encode_b64(&s.sign(m).to_bytes());
    let (s1, s2, s3, sa) = (sk(1), sk(2), sk(3), sk(4));
    let v1 = |id: &str, s: &SigningKey| ReviewerKey {
        id: id.into(),
        alg: ALG.into(),
        public: pk(s),
        created: "2026-10-07".into(),
        endorsed_by: None,
        reset: false,
        retired: false,
        retired_on: None,
        unretired: None,
        history: vec![],
    };
    // Founder: k1 retired 10-10 and un-retired 10-15 (v1 form); k2 endorsed
    // by k1; k3 an unsigned founder reset.
    let mut k1 = v1("k1", &s1);
    k1.retired_on = Some("2026-10-10".into());
    k1.unretired = Some(Unretirement {
        date: "2026-10-15".into(),
        signature: sig(&s1, &unretire_bytes(FOUNDER, &k1, "2026-10-10", "2026-10-15")),
    });
    let mut k2 = v1("k2", &s2);
    k2.endorsed_by = Some(KeySignature { key: "k1".into(), signature: sig(&s1, &endorsement_bytes(FOUNDER, &k2)) });
    let mut k3 = v1("k3", &s3);
    k3.reset = true;
    let mut alice = person(ALICE, Role::Reviewer, vec![v1("a1", &sa)]);
    alice.admitted = Some("2026-10-07".into());
    alice.admitted_by = Some(KeySignature { key: "k1".into(), signature: sig(&s1, &admission_bytes(&alice)) });
    let v1_root = root(vec![person(FOUNDER, Role::Maintainer, vec![k1, k2, k3]), alice]);

    // Through TOML, as a v1 file on disk would be.
    let text = v1_root.write_into("schema_version = 1\n").unwrap();
    assert!(text.contains("retired_on") && text.contains("admitted_by") && !text.contains("history"));
    let loaded = ReviewRoot::parse(&text).unwrap();
    let reg = Registry::build(&loaded);
    assert_eq!(
        reg.warnings(),
        vec![Warning::KeyReset {
            reviewer: FOUNDER.into(),
            key: "k3".into(),
            status: KeyStatus::ResetTrustedOnFirstUse
        }]
    );
    let r = reg.reviewer(FOUNDER).unwrap();
    assert_eq!(r.key("k2").unwrap().status, KeyStatus::Endorsed { by_reviewer: FOUNDER.into(), by_key: "k1".into() });
    assert_eq!(
        r.key("k1").unwrap().windows,
        vec![Window { kind: InactiveKind::Retired, from: Some("2026-10-10".into()), to: Some("2026-10-15".into()) }]
    );
    assert_eq!(reg.reviewer(ALICE).unwrap().admission, Admission::Admitted { by: FOUNDER.into(), key: "k1".into() });

    // The migration itself: legacy events, legacy fields cleared, and the
    // migrated file judges the same.
    let mut migrated = loaded.clone();
    assert_eq!(migrated.migrate_key_history(), vec![]);
    let f = &migrated.reviewers[0];
    let kinds: Vec<KeyEventKind> = f.keys[0].history.iter().map(|e| e.event).collect();
    assert_eq!(kinds, vec![KeyEventKind::Created, KeyEventKind::Retired, KeyEventKind::Unretired]);
    assert!(f.keys[0].history[2].legacy);
    assert!(f.keys.iter().all(|k| k.endorsed_by.is_none() && !k.reset && !k.retired && k.retired_on.is_none() && k.unretired.is_none()));
    assert!(migrated.reviewers[1].admitted_by.is_none());
    let text2 = migrated.write_into("schema_version = 1\n").unwrap();
    assert!(text2.contains("[[reviewer.key.history]]") && text2.contains("legacy = true"));
    let reparsed = ReviewRoot::parse(&text2).unwrap();
    assert_eq!(reparsed, migrated);
    assert_eq!(Registry::build(&reparsed), reg);
    // Migrating twice changes nothing.
    let mut again = migrated.clone();
    assert_eq!(again.migrate_key_history(), vec![]);
    assert_eq!(again, migrated);

    // v1: retired with no date; un-retired but still marked retired.
    let mut undated = v1("k1", &s1);
    undated.retired = true;
    let reg = founder_with(vec![undated]);
    assert!(reg.reviewer(FOUNDER).unwrap().key("k1").unwrap().is_retired());
    let mut still = migrated.reviewers[0].keys[0].clone();
    still.history.clear();
    still.retired_on = Some("2026-10-10".into());
    still.unretired = loaded.reviewers[0].keys[0].unretired.clone();
    still.retired = true;
    let reg = founder_with(vec![still]);
    assert!(reg.reviewer(FOUNDER).unwrap().key("k1").unwrap().is_retired());

    // Both forms on one key: history wins, loudly.
    let mut both = migrated.reviewers[0].keys[0].clone();
    both.retired = true;
    let mut r = root(vec![person(FOUNDER, Role::Maintainer, vec![both])]);
    let conflicts = r.clone().migrate_key_history();
    assert_eq!(conflicts.len(), 1);
    assert_eq!((conflicts[0].reviewer.as_str(), conflicts[0].key.as_str()), (FOUNDER, "k1"));
    let reg = Registry::build(&r);
    assert_eq!(history_warnings(&reg, FOUNDER, "k1"), vec![HistoryProblem::LegacyFieldsIgnored]);
    assert!(!reg.reviewer(FOUNDER).unwrap().key("k1").unwrap().is_retired());
    r.migrate_key_history();
    assert!(r.reviewers[0].keys[0].retired, "conflicting legacy fields are kept, not merged");

    // The committed v1 fixture still loads and migrates.
    let fixture = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/review/v1/kovan_root.toml"),
    )
    .unwrap();
    let mut fx = ReviewRoot::parse(&fixture).unwrap();
    let reg = Registry::build(&fx);
    assert_eq!(reg.founder, Err(FounderProblem::NotDeclared));
    assert_eq!(fx.migrate_key_history(), vec![]);
    assert_eq!(fx.reviewers[0].keys[0].history, vec![KeyEvent::unsigned(KeyEventKind::Created, "2026-10-07")]);
    // The keyless reviewer keeps its v1 admitted_by (nothing to move it to).
    assert!(fx.reviewers[1].admitted_by.is_some());
    assert_eq!(
        reg.reviewer("orcid:0000-0002-1825-0097").unwrap().admission,
        Admission::NotAdmitted(AdmissionProblem::NoKey)
    );
}
