//! Tests of **signed key retirement** (GitHub #1249, maintainer
//! 2026-11-05): from [`SIGNED_KEY_RETIREMENT_FROM`] a `retired` event counts
//! only when signed by the key's owner, a founder or a maintainer active on
//! its date; an unsigned (or otherwise unsigned-for-the-rule) new one has no
//! effect and is a warning; an old unsigned one keeps counting, flagged.
//!
//! Methodology, for every test: throwaway keys from [`keystore::generate`]
//! (real argon2id + AES-256-GCM + ed25519, in memory; nothing touches a
//! real keystore), retirements signed with [`UnlockedKey::retire_key`] over
//! the real [`key_event_bytes`], every registry judged by
//! [`Registry::build`], and each verdict checked against the rule in the
//! registry's module doc: whether the key is retired
//! ([`super::registry::KeyTrust::is_retired`]), whether a stamp after the
//! date verifies, and the exact [`HistoryProblem`] shown.

use super::keystore::UnlockedKey;
use super::registry::{
    retire_key, retirement_is_legacy, HistoryProblem, Registry, SignerProblem, Warning,
    SIGNED_KEY_RETIREMENT_FROM,
};
use super::tests::{founder_and_alice, key, person, root, signed, ALICE, F, FOUNDER};
use super::*;
use crate::review::root::{KeyEvent, KeyEventKind, KeySigner, ReviewRoot, Reviewer, ReviewerKey, Role};

/// On and after the cut-off.
const N1: &str = "2026-11-10";
const N2: &str = "2026-11-12";
const AFTER: &str = "2026-11-20";
/// Before it.
const OLD: &str = "2026-10-10";

fn history_warnings(reg: &Registry, reviewer: &str, key: &str) -> Vec<HistoryProblem> {
    reg.warnings()
        .into_iter()
        .filter_map(|w| match w {
            Warning::KeyHistory { reviewer: r, key: k, problem } if r == reviewer && k == key => Some(problem),
            _ => None,
        })
        .collect()
}

/// A reviewer `id` (role `role`, scope tampines) admitted by `by` on
/// 2026-10-07, with one fresh key `kid`.
fn admitted(by: &UnlockedKey, id: &str, kid: &str, role: Role) -> (Reviewer, UnlockedKey) {
    let (k, u) = key(id, kid);
    let mut r = person(id, role, vec![k]);
    if role == Role::Reviewer {
        r.scope = vec!["crates/tampines/**".into()];
    }
    r.admitted = Some("2026-10-07".into());
    by.admit(&mut r).unwrap();
    (r, u)
}

fn retired(reg: &Registry, id: &str, kid: &str) -> bool {
    reg.reviewer(id).unwrap().key(kid).unwrap().is_retired()
}

/// Pass, each predicted from the rule: Alice's own retirement of her key
/// (signed by that key), Bob's (a maintainer) of Carol's key and the
/// founder's of Dave's key all count (retired, a stamp after the date does
/// not verify, no warning about them); Eve's unsigned retirement dated N1
/// has **no effect** (not retired, her later stamp verifies,
/// `RetirementNoEffect { Unsigned }`); Frank's key retired by Alice (a
/// reviewer, not his owner) has no effect (`NotEligible`); Gina's
/// retirement with a forged signature has no effect (`BadSignature`);
/// Hal's key retired by Bob on the day Bob's own key is retired has no
/// effect (`SignerRetired`); Ivan's **old** unsigned retirement (before
/// the cut-off) still counts and is flagged `UnsignedRetirementLegacy`.
///
/// Result (2026-11-05): passes.
#[test]
fn signed_retirements_count_and_unsigned_new_ones_have_no_effect() {
    let (founder, f, mut alice, a) = founder_and_alice();
    let (mut bob, b) = admitted(&f, "github:bob", "b1", Role::Maintainer);
    let mut people = Vec::new();
    for (id, kid) in [
        ("github:carol", "c1"),
        ("github:dave", "d1"),
        ("github:eve", "e1"),
        ("github:frank", "f1"),
        ("github:gina", "g1"),
        ("github:hal", "h1"),
        ("github:ivan", "i1"),
    ] {
        people.push(admitted(&f, id, kid, Role::Reviewer));
    }
    a.retire_key(ALICE, &mut alice.keys[0], N1).unwrap();
    b.retire_key("github:carol", &mut people[0].0.keys[0], N1).unwrap();
    f.retire_key("github:dave", &mut people[1].0.keys[0], N1).unwrap();
    people[2].0.keys[0].history.push(KeyEvent::unsigned(KeyEventKind::Retired, N1));
    a.retire_key("github:frank", &mut people[3].0.keys[0], N1).unwrap();
    let mut forged = KeyEvent::unsigned(KeyEventKind::Retired, N1);
    forged.signer = Some(KeySigner { reviewer: Some(FOUNDER.into()), key: "k1".into() });
    forged.signature = Some(encode_b64(&[7u8; 64]));
    people[4].0.keys[0].history.push(forged);
    // Bob retires his own key on N2, and signs Hal's retirement with it
    // that same day (the key is void from N2).
    b.retire_key("github:bob", &mut bob.keys[0], N2).unwrap();
    b.retire_key("github:hal", &mut people[5].0.keys[0], N2).unwrap();
    retire_key(&mut people[6].0.keys[0], OLD).unwrap();

    let mut all = vec![founder, alice, bob];
    all.extend(people.iter().map(|(r, _)| r.clone()));
    let reg = Registry::build(&root(all));
    for (id, kid) in [(ALICE, "a1"), ("github:carol", "c1"), ("github:dave", "d1"), ("github:bob", "b1")] {
        assert!(retired(&reg, id, kid), "{id}");
        assert_eq!(history_warnings(&reg, id, kid), vec![], "{id}");
    }
    assert!(!verify_review(&signed(&people[0].1, F, AFTER), &reg).is_verified());
    assert!(!verify_review(&signed(&people[1].1, F, AFTER), &reg).is_verified());
    let no_effect = |id: &str, kid: &str, p: SignerProblem| {
        assert!(!retired(&reg, id, kid), "{id}");
        assert_eq!(history_warnings(&reg, id, kid), vec![HistoryProblem::RetirementNoEffect { index: 2, problem: p }], "{id}");
    };
    no_effect("github:eve", "e1", SignerProblem::Unsigned);
    no_effect("github:frank", "f1", SignerProblem::NotEligible(ALICE.into()));
    no_effect("github:gina", "g1", SignerProblem::BadSignature);
    no_effect("github:hal", "h1", SignerProblem::SignerRetired { signer: "github:bob".into(), key: "b1".into() });
    assert!(verify_review(&signed(&people[2].1, F, AFTER), &reg).is_verified(), "Eve's key stays active");
    assert!(retired(&reg, "github:ivan", "i1"), "an old unsigned retirement still counts");
    assert_eq!(
        history_warnings(&reg, "github:ivan", "i1"),
        vec![HistoryProblem::UnsignedRetirementLegacy { index: 2 }]
    );
    assert!(!verify_review(&signed(&people[6].1, F, AFTER), &reg).is_verified());
}

/// Someone edits `kovan_root.toml` and adds an unsigned retirement of the
/// founder's key dated N1; the founder then retires Alice's key on N2,
/// signed with that same key. Pass: the unsigned retirement has no effect,
/// so the founder's key is active on N2 and Alice's retirement counts (the
/// registry's settle loop: the first round, with every retirement in
/// effect, would have rejected it as `SignerRetired`). `retire_key` on the
/// keystore side accepts a key with such an open, ineffective retirement.
/// An un-retirement after an ineffective retirement is not a problem.
///
/// Result (2026-11-05): passes.
#[test]
fn an_unsigned_retirement_cannot_switch_off_a_signer() {
    let (mut founder, f, mut alice, _a) = founder_and_alice();
    founder.keys[0].history.push(KeyEvent::unsigned(KeyEventKind::Retired, N1));
    f.retire_key(ALICE, &mut alice.keys[0], N2).unwrap();
    let reg = Registry::build(&root(vec![founder.clone(), alice.clone()]));
    assert!(!retired(&reg, FOUNDER, "k1"));
    assert!(retired(&reg, ALICE, "a1"));
    assert_eq!(
        history_warnings(&reg, FOUNDER, "k1"),
        vec![HistoryProblem::RetirementNoEffect { index: 1, problem: SignerProblem::Unsigned }]
    );
    assert_eq!(history_warnings(&reg, ALICE, "a1"), vec![]);
    // The founder signs their own retirement after the bogus one.
    let mut k1 = founder.keys[0].clone();
    f.retire_key(FOUNDER, &mut k1, AFTER).unwrap();
    founder.keys[0] = k1.clone();
    let reg = Registry::build(&root(vec![founder.clone(), alice.clone()]));
    assert!(retired(&reg, FOUNDER, "k1"));
    assert_eq!(reg.reviewer(FOUNDER).unwrap().key("k1").unwrap().windows[0].from.as_deref(), Some(AFTER));
    // An unsigned retirement followed by an un-retirement: both silent.
    let (mut k2, u2) = key(FOUNDER, "k2");
    k2.history.push(KeyEvent::unsigned(KeyEventKind::Retired, N1));
    u2.unretire(&mut k2, N2).unwrap();
    founder.keys[0] = k2;
    let rt = root(vec![founder]);
    let reg = Registry::build(&rt);
    assert!(!retired(&reg, FOUNDER, "k2"));
    assert_eq!(
        history_warnings(&reg, FOUNDER, "k2"),
        vec![HistoryProblem::RetirementNoEffect { index: 1, problem: SignerProblem::Unsigned }]
    );
}

/// **Schemas never break.** Methodology: a `kovan_root.toml` written as
/// before GitHub #1249 (an unsigned `retired` event dated before the
/// cut-off, as `retire_key` wrote it, plus a v1 `retired = true` with no
/// date) is parsed from TOML text. Pass: it parses, both keys are retired
/// as before, and each shows `UnsignedRetirementLegacy`; the signed
/// retirement event uses only fields `[[reviewer.key.history]]` already had
/// (`signer`, `signature`), so it reads back through the same struct and
/// in a reader shaped like the pre-#1249 event; `retirement_is_legacy`
/// classifies the cut-off itself, the day before, the empty v1 date and a
/// malformed date.
///
/// Result (2026-11-05): passes.
#[test]
fn old_unsigned_retirements_load_and_keep_counting() {
    let (founder, f, mut alice, _a) = founder_and_alice();
    retire_key(&mut alice.keys[0], OLD).unwrap();
    let (mut v1, _) = key(FOUNDER, "k9");
    v1.history.clear();
    v1.retired = true;
    v1.endorsed_by = None;
    let mut founder2 = founder.clone();
    founder2.keys.push(v1);
    let text = root(vec![founder2, alice]).write_into("schema_version = 1\n").unwrap();
    let parsed = ReviewRoot::parse(&text).unwrap();
    let reg = Registry::build(&parsed);
    assert!(retired(&reg, ALICE, "a1"));
    assert_eq!(history_warnings(&reg, ALICE, "a1"), vec![HistoryProblem::UnsignedRetirementLegacy { index: 2 }]);
    let k9 = reg.reviewer(FOUNDER).unwrap().key("k9").unwrap();
    assert!(k9.is_retired());
    assert!(k9.history.contains(&HistoryProblem::UnsignedRetirementLegacy { index: 1 }), "{:?}", k9.history);

    // A signed retirement round-trips, and an old-shaped reader reads it.
    let (mut k, _) = key(ALICE, "a2");
    f.retire_key(ALICE, &mut k, N1).unwrap();
    #[derive(serde::Deserialize)]
    struct OldEvent {
        event: String,
        date: String,
    }
    #[derive(serde::Deserialize)]
    struct OldKey {
        history: Vec<OldEvent>,
    }
    let t = toml::to_string(&k).unwrap();
    let back: ReviewerKey = toml::from_str(&t).unwrap();
    assert_eq!(back, k);
    let old: OldKey = toml::from_str(&t).unwrap();
    assert_eq!((old.history[1].event.as_str(), old.history[1].date.as_str()), ("retired", N1));

    assert!(!retirement_is_legacy(SIGNED_KEY_RETIREMENT_FROM));
    assert!(retirement_is_legacy("2026-11-05"));
    assert!(retirement_is_legacy(""));
    assert!(!retirement_is_legacy("someday"), "a malformed date is not an old retirement");
}
