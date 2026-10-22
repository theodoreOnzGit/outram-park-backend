//! Tests of the registry lifecycle appenders (GitHub #1019, the User
//! management tab): admitting a registered reviewer, appending signed key
//! events (endorse, retire, un-retire, compromise) and a reviewer
//! revocation, as text, to a commented `kovan_root.toml`, signed with real
//! keys ([`generate`]: argon2id + AES-256-GCM + ed25519) and judged by the
//! #762 [`Registry`].

use super::*;
use crate::review::signing::keystore::generate;
use crate::review::signing::registry::{
    retire_key, Admission, HistoryProblem, InactiveKind, KeyProblem, KeyStatus, Registry, Warning,
};

const PASS: &str = "test passphrase, not a real one";
const DAY: &str = "2026-10-22";
const LATER: &str = "2026-10-23";

/// A commented root: the founder `github:m` (key `k1`), then `github:a`
/// registered by its own stamp dialog and not admitted (key `a1`), whose
/// table has a multi-line `scope` array and a comment, then a trailing
/// table.
fn fixture() -> (
    String,
    crate::review::signing::keystore::UnlockedKey,
    crate::review::signing::keystore::UnlockedKey,
) {
    let (mf, mk) = generate("github:m", "k1", DAY, PASS).unwrap();
    let (af, ak) = generate("github:a", "a1", DAY, PASS).unwrap();
    let mut text = String::from(
        "# Root. Comments must survive.\nschema_version = 1\n\n[code_review]\nfounder = \"github:m\" # kept\n",
    );
    text = append_reviewer(&text, &founding_reviewer("github:m", None, mf.reviewer_key(), DAY)).unwrap();
    text = append_reviewer(&text, &unadmitted_reviewer("github:a", Some("Alice"), af.reviewer_key())).unwrap();
    // A hand-written multi-line scope and a comment in Alice's own table.
    text = text.replace(
        "name = \"Alice\"\n",
        "name = \"Alice\"\n# asked for njoy\nscope = [\n    \"crates/old/**\", # was\n]\n",
    );
    text.push_str("\n# Trailing comment.\n[library]\nid = \"demo\"\n");
    assert_eq!(ReviewRoot::parse(&text).unwrap().reviewers[1].scope, vec!["crates/old/**"]);
    (text, mk, ak)
}

fn lines_kept(before: &str, after: &str, dropped: &[&str]) {
    let mut rest = after.lines();
    for l in before.lines().filter(|l| !dropped.iter().any(|d| l.trim_start().starts_with(d))) {
        assert!(rest.any(|a| a == l), "line {l:?} lost or reordered in:\n{after}");
    }
}

/// Methodology: on the fixture, the founder admits Alice as a reviewer
/// scoped to `crates/njoy-*/**` (`UnlockedKey::admit`, then
/// [`record_admission`]); Alice registers a second key and endorses it
/// with her first ([`append_key_event`]); the second key is retired
/// (unsigned), un-retired by itself, then marked compromised by the
/// founder; finally the founder revokes Alice ([`append_revocation`]).
/// Pass: after each step every original line is kept in order (bar Alice's
/// replaced `role` and `scope` lines), the file reads back as exactly the
/// change, and the registry judges it as the rule says: Alice admitted by
/// `github:m` key `k1` with the new scope, `a2` endorsed by `a1`, the
/// retirement window closed at the un-retirement, a compromise window
/// from `LATER`, a revocation signed by `k1`, and no tampered event.
///
/// Result (2026-10-22): passes.
#[test]
fn admit_endorse_retire_unretire_compromise_and_revoke_as_text() {
    let (base, mk, ak) = fixture();
    let root = ReviewRoot::parse(&base).unwrap();
    let mut alice = root.reviewer("github:a").unwrap().clone();
    alice.role = Role::Reviewer;
    alice.scope = vec!["crates/njoy-*/**".into()];
    alice.admitted = Some(DAY.into());
    mk.admit(&mut alice).unwrap();
    let t1 = record_admission(&base, &alice).unwrap();
    lines_kept(&base, &t1, &["role", "scope", "\"crates/old", "]"]);
    assert!(t1.contains("# asked for njoy\n"), "{t1}");
    let reg = Registry::build(&ReviewRoot::parse(&t1).unwrap());
    let a = reg.reviewer("github:a").unwrap();
    assert_eq!(a.admission, Admission::Admitted { by: "github:m".into(), key: "k1".into() });
    assert_eq!(a.scope, vec!["crates/njoy-*/**"]);
    assert!(reg.warnings().is_empty(), "{:?}", reg.warnings());
    // Refusals: nothing changes.
    assert_eq!(record_admission(&t1, &alice), Err(AppendError::AlreadyAdmitted("github:a".into())));
    let mut founder = ReviewRoot::parse(&t1).unwrap().reviewer("github:m").unwrap().clone();
    founder.admitted = Some(DAY.into());
    assert!(matches!(record_admission(&t1, &founder), Err(AppendError::AlreadyAdmitted(_))));
    let mut renamed = ReviewRoot::parse(&base).unwrap().reviewer("github:a").unwrap().clone();
    renamed.name = Some("Mallory".into());
    renamed.admitted = Some(DAY.into());
    mk.admit(&mut renamed).unwrap();
    assert!(matches!(record_admission(&base, &renamed), Err(AppendError::NotReadBack(_))));

    // Alice's second key, registered bare, then endorsed by a1.
    let (a2f, a2k) = generate("github:a", "a2", DAY, PASS).unwrap();
    let t2 = append_reviewer_key(&t1, "github:a", &a2f.reviewer_key()).unwrap();
    let reg = Registry::build(&ReviewRoot::parse(&t2).unwrap());
    assert_eq!(reg.reviewer("github:a").unwrap().key("a2").unwrap().status, KeyStatus::Untrusted(KeyProblem::Unendorsed));
    let mut k2 = ReviewRoot::parse(&t2).unwrap().reviewer("github:a").unwrap().keys[1].clone();
    ak.endorse("github:a", &mut k2, DAY).unwrap();
    let t3 = append_key_event(&t2, "github:a", "a2", k2.history.last().unwrap()).unwrap();
    lines_kept(&t2, &t3, &[]);
    let reg = Registry::build(&ReviewRoot::parse(&t3).unwrap());
    assert_eq!(
        reg.reviewer("github:a").unwrap().key("a2").unwrap().status,
        KeyStatus::Endorsed { by_reviewer: "github:a".into(), by_key: "a1".into() }
    );

    // Retire (unsigned), un-retire (by the key itself), compromise.
    retire_key(&mut k2, DAY).unwrap();
    let t4 = append_key_event(&t3, "github:a", "a2", k2.history.last().unwrap()).unwrap();
    assert!(Registry::build(&ReviewRoot::parse(&t4).unwrap()).reviewer("github:a").unwrap().key("a2").unwrap().is_retired());
    a2k.unretire(&mut k2, LATER).unwrap();
    let t5 = append_key_event(&t4, "github:a", "a2", k2.history.last().unwrap()).unwrap();
    mk.revoke_key("github:a", &mut k2, LATER, true).unwrap();
    let t6 = append_key_event(&t5, "github:a", "a2", k2.history.last().unwrap()).unwrap();
    lines_kept(&t3, &t6, &[]);
    let reg = Registry::build(&ReviewRoot::parse(&t6).unwrap());
    let kt = reg.reviewer("github:a").unwrap().key("a2").unwrap();
    assert!(!kt.is_retired());
    assert_eq!(kt.windows.len(), 2);
    assert_eq!(kt.windows[0].to.as_deref(), Some(LATER));
    assert_eq!((kt.windows[1].kind, kt.windows[1].from.as_deref()), (InactiveKind::Compromised, Some(LATER)));
    assert!(
        !reg.warnings().iter().any(|w| matches!(w, Warning::KeyHistory { problem: HistoryProblem::BadEventSignature { .. }, .. })),
        "{:?}",
        reg.warnings()
    );

    // The founder revokes Alice.
    let mut rev = Revocation { date: LATER.into(), compromised_from: None, by: "github:m".into(), signature: None };
    mk.revoke("github:a", &mut rev).unwrap();
    let t7 = append_revocation(&t6, "github:a", &rev).unwrap();
    lines_kept(&t6, &t7, &[]);
    assert!(t7.ends_with("# Trailing comment.\n[library]\nid = \"demo\"\n"), "{t7}");
    let reg = Registry::build(&ReviewRoot::parse(&t7).unwrap());
    assert_eq!(reg.reviewer("github:a").unwrap().revocation.as_ref().unwrap().signed, Ok("k1".into()));
    assert_eq!(append_revocation(&t7, "github:a", &rev), Err(AppendError::AlreadyRevoked("github:a".into())));
    // A key event still lands on the key after the revocation table.
    let mut k1 = ReviewRoot::parse(&t7).unwrap().reviewer("github:a").unwrap().keys[0].clone();
    retire_key(&mut k1, LATER).unwrap();
    let t8 = append_key_event(&t7, "github:a", "a1", k1.history.last().unwrap()).unwrap();
    assert!(ReviewRoot::parse(&t8).unwrap().reviewer("github:a").unwrap().revoked.is_some());
}

/// Methodology: the refusals of [`append_key_event`] and
/// [`append_revocation`]. Pass: an unknown reviewer, an unknown key and a
/// key whose lifecycle is still in the v1 legacy fields (no history in the
/// file) are typed errors, with nothing returned to write; the error
/// messages name the reviewer and key.
///
/// Result (2026-10-22): passes.
#[test]
fn key_events_are_refused_on_unknown_and_legacy_keys() {
    let (base, _, _) = fixture();
    let ev = KeyEvent::unsigned(KeyEventKind::Retired, LATER);
    assert_eq!(append_key_event(&base, "github:x", "a1", &ev), Err(AppendError::UnknownReviewer("github:x".into())));
    let e = append_key_event(&base, "github:a", "zz", &ev).unwrap_err();
    assert_eq!(e, AppendError::UnknownKey { reviewer: "github:a".into(), key: "zz".into() });
    assert!(e.to_string().contains("zz"));
    let legacy = "[[reviewer]]\nid = \"github:l\"\nrole = \"maintainer\"\n\n[[reviewer.key]]\nid = \"l1\"\nalg = \"ed25519\"\npublic = \"AAAA\"\ncreated = \"2026-10-07\"\nretired = true\n";
    let e = append_key_event(legacy, "github:l", "l1", &ev).unwrap_err();
    assert_eq!(e, AppendError::LegacyKey { reviewer: "github:l".into(), key: "l1".into() });
    assert!(e.to_string().contains("v1"));
    let rev = Revocation { date: LATER.into(), compromised_from: None, by: "github:m".into(), signature: None };
    assert_eq!(append_revocation(&base, "github:x", &rev), Err(AppendError::UnknownReviewer("github:x".into())));
    // Unsigned: written, honoured, and listed as unverified.
    let t = append_revocation(&base, "github:a", &rev).unwrap();
    let reg = Registry::build(&ReviewRoot::parse(&t).unwrap());
    assert!(reg.warnings().iter().any(|w| matches!(w, Warning::RevocationUnverified { .. })));
    for e in [AppendError::AlreadyAdmitted("x".into()), AppendError::AlreadyRevoked("x".into())] {
        assert!(e.to_string().contains("already"));
    }
}
