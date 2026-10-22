//! Tests of the User management tab's logic (GitHub #1019).
//!
//! Methodology, for every test: the stamp layer's throwaway git fixture
//! ([`Repo`]: a crate with `leaf`, `twice` and `other`, a commented
//! `kovan_root.toml`) with real keys (argon2id + AES-256-GCM + ed25519)
//! in temporary keystores. Nothing reads the real keystore. The roster
//! and every action are judged by the #762 `Registry` as the file reads
//! afterwards.

use kovan_common::review::review_md::Verdict;
use kovan_common::review::root::{ReviewRoot, Role};
use kovan_common::review::signing::keystore::{generate, Keystore};
use kovan_common::review::signing::registry::{Admission, KeyProblem, KeyStatus, Registry};
use kovan_common::review::signing::session::SharedKeySession;
use kovan_common::review::state::StateKind;

use super::super::justify::{justify, JustifyRequest};
use super::super::tests::{founder_key, today, Repo, BY, LIB, PASS};
use super::super::{register_key, KeyRegistration, ROOT_FILE};
use super::*;

pub(crate) const B: &str = "github:bob";

/// Bob generates key `id` on his machine (the same temporary keystore
/// here) and registers it from his stamp dialog; committed.
pub(crate) fn bob_registers(r: &Repo, store: &Path, id: &str) -> KeyRegistration {
    let ks = Keystore::at(store);
    let (kf, _) = generate(B, id, &today(), PASS).unwrap();
    ks.save(&kf).unwrap();
    let reg = register_key(r.path(), &kf, Some("Bob"), &today()).unwrap();
    r.commit(&format!("bob registers {id}"));
    reg
}

pub(crate) fn registry(r: &Repo) -> Registry {
    Registry::build(&ReviewRoot::parse(&r.read(ROOT_FILE)).unwrap())
}

/// Choose the panel's signing key (reviewer, key).
pub(crate) fn choose(p: &mut UserPanel, reviewer: &str, key: &str) {
    p.chosen = p.keys.iter().position(|k| k.reviewer == reviewer && k.key == key).expect("key in keystore");
}

/// Methodology: the founder stamps `twice`; an agent justifies `other`
/// through `kovan-cli review justify`'s code path; Bob registers a key and
/// is not admitted. The roster is loaded as the tab loads it (worker).
/// Pass: the founder's row is "Maintainer · founder", all crates, active,
/// one key present on this machine with a `SHA256:` fingerprint and a
/// `created` history line, one valid stamp and coverage 1 of the 3
/// functions; Bob is awaiting admission and the warnings strip says he is
/// not admitted; `agent:claude` is listed apart, with its justification
/// and "code unchanged since"; no agent is a reviewer row.
///
/// Result (2026-10-22): passes.
#[test]
fn roster_lists_reviewers_keys_stamps_agents_and_warnings() {
    let r = Repo::new();
    let store = tempfile::tempdir().unwrap();
    let key = founder_key(&r, store.path());
    r.stamp(&key, "twice", "");
    justify(
        r.path(),
        &JustifyRequest {
            function: format!("{LIB}::other"),
            by: "agent:claude".into(),
            verdict: Verdict::Ok,
            evidence: vec!["kovan test: 3 passed".into()],
        },
    )
    .unwrap();
    r.commit("justify other");
    assert_eq!(bob_registers(&r, store.path(), "b1"), KeyRegistration::AwaitingAdmission);

    let mut p = UserPanel::new(r.path().into(), Keystore::at(store.path()), SharedKeySession::default());
    assert!(p.busy() && p.roster().is_none());
    p.wait();
    let ro = p.roster().expect("loaded").clone();
    let f = ro.reviewer(BY).unwrap();
    assert_eq!((f.role_label.as_str(), f.scope_label.as_str()), ("Maintainer \u{b7} founder", "all crates"));
    assert_eq!(f.status, Status::Active);
    assert_eq!(f.status.marker(), "\u{2714}");
    assert_eq!(p.key().unwrap().reviewer, BY, "an admitted maintainer's key is chosen first");
    assert!(f.admitted.contains("founder"));
    assert_eq!(f.keys.len(), 1);
    assert!(f.keys[0].here && f.keys[0].trusted && f.keys[0].fingerprint.starts_with("SHA256:"));
    assert!(f.keys[0].history[0].ends_with("created"), "{:?}", f.keys[0].history);
    assert_eq!(f.stamps.len(), 1, "{:?}", f.stamps);
    assert_eq!(f.stamps[0].state, StateKind::Valid);
    assert_eq!(f.coverage, Some((1, 3)));
    assert!(f.reviews_line().contains("1 stamp \u{b7} 1 valid \u{b7} coverage 33 % (1/3)"), "{}", f.reviews_line());
    let b = ro.reviewer(B).unwrap();
    assert!(matches!(b.status, Status::AwaitingAdmission(_)), "{:?}", b.status);
    assert!(b.admitted.starts_with("not admitted"));
    assert_eq!(b.scope_label, "no scope: stamps nowhere");
    assert!(ro.warnings.iter().any(|w| w.contains(B) && w.contains("not admitted")), "{:?}", ro.warnings);
    assert_eq!(ro.agents.len(), 1);
    assert_eq!(ro.agents[0].id, "agent:claude");
    let j = &ro.agents[0].justifications[0];
    assert_eq!((j.verdict.as_str(), j.code), ("ok", "code unchanged since"));
    assert!(j.function.ends_with("::other"));
    assert!(!ro.reviewers.iter().any(|x| x.id.starts_with("agent:")));
    // The stamp-less roster (no engine) still lists everyone.
    let bare = build_roster(&ReviewRoot::parse(&r.read(ROOT_FILE)).unwrap(), None, None, &[]);
    assert_eq!(bare.reviewers.len(), 2);
    assert!(bare.reviewers[0].reviews_line().contains("coverage not computed"));
    assert!(!bare.reviewers[0].keys[0].here);
}

/// Methodology: through the panel, as the tab drives it: the founder
/// admits Bob (role Reviewer, scope `crates/demo/**`); Bob registers a
/// second key `b2`, endorses it with `b1`, retires it, un-retires it with
/// `b2` itself; then the founder revokes Bob. Before each action the
/// preconditions are read as the buttons read them. Pass: each refusal
/// predicted (no passphrase; a non-maintainer admitting; an empty scope;
/// an un-retire signed by another key; acting twice) is listed and
/// `submit` starts nothing; each action writes, and the registry then
/// reads Bob admitted by the founder's `k1` with that scope, `b2`
/// unendorsed then endorsed by `b1`, retired then not, and Bob's
/// revocation signed by `k1`. The passphrase is asked once per key: the
/// session holds the last key unlocked.
///
/// Result (2026-10-22): passes.
#[test]
fn admit_endorse_retire_unretire_and_revoke_through_the_panel() {
    let r = Repo::new();
    let store = tempfile::tempdir().unwrap();
    founder_key(&r, store.path());
    bob_registers(&r, store.path(), "b1");
    let session = SharedKeySession::default();
    let mut p = UserPanel::new(r.path().into(), Keystore::at(store.path()), session.clone());
    p.wait();
    let admit = UserAction::Admit { reviewer: B.into(), role: Role::Reviewer, scope: vec!["crates/demo/**".into()], novice: false };
    choose(&mut p, B, "b1");
    let probs = p.problems(&admit);
    assert!(probs.iter().any(|x| x.contains("admitted maintainer")), "{probs:?}");
    assert!(probs.iter().any(|x| x.contains("passphrase")), "{probs:?}");
    assert!(!p.submit(admit.clone()));
    choose(&mut p, BY, "k1");
    let empty = UserAction::Admit { reviewer: B.into(), role: Role::Reviewer, scope: vec![" ".into()], novice: false };
    let novice_maintainer = UserAction::Admit { reviewer: B.into(), role: Role::Maintainer, scope: vec![], novice: true };
    assert!(p.problems(&novice_maintainer).iter().any(|x| x.contains("never a novice")));
    assert!(p.problems(&empty).iter().any(|x| x.contains("needs a scope")));
    p.passphrase = PASS.into();
    assert_eq!(p.problems(&admit), Vec::<String>::new());
    assert!(p.submit(admit.clone()));
    p.wait();
    assert!(p.error.is_none(), "{:?}", p.error);
    assert!(p.notices[0].contains("Admitted github:bob as reviewer"), "{:?}", p.notices);
    assert!(p.unlocked(), "the founder's key is held for the session");
    assert!(p.take_changed() && !p.take_changed(), "the host recomputes the states once");
    let reg = registry(&r);
    let bob = reg.reviewer(B).unwrap();
    assert_eq!(bob.admission, Admission::Admitted { by: BY.into(), key: "k1".into() });
    assert_eq!(bob.scope, vec!["crates/demo/**"]);
    assert!(p.problems(&admit).iter().any(|x| x.contains("admitted already")));
    r.commit("admit bob");

    // Bob's second key: unendorsed, then endorsed by b1.
    bob_registers(&r, store.path(), "b2");
    assert_eq!(registry(&r).reviewer(B).unwrap().key("b2").unwrap().status, KeyStatus::Untrusted(KeyProblem::Unendorsed));
    p.load();
    p.wait();
    let endorse = UserAction::Endorse { reviewer: B.into(), key: "b2".into() };
    assert!(p.problems(&endorse).iter().any(|x| x.contains("sign as github:bob")));
    choose(&mut p, B, "b1");
    p.passphrase = PASS.into();
    assert!(p.submit(endorse.clone()));
    p.wait();
    assert!(p.error.is_none(), "{:?}", p.error);
    assert_eq!(
        registry(&r).reviewer(B).unwrap().key("b2").unwrap().status,
        KeyStatus::Endorsed { by_reviewer: B.into(), by_key: "b1".into() }
    );
    assert!(p.problems(&endorse).iter().any(|x| x.contains("needs no endorsement")));

    // Retire b2 (unsigned, by its owner), un-retire it with b2 itself.
    let retire = UserAction::RetireKey { reviewer: B.into(), key: "b2".into() };
    assert!(p.submit(retire.clone()));
    p.wait();
    assert!(registry(&r).reviewer(B).unwrap().key("b2").unwrap().is_retired());
    let unretire = UserAction::UnretireKey { reviewer: B.into(), key: "b2".into() };
    assert!(p.problems(&unretire).iter().any(|x| x.contains("Only key b2 itself")));
    choose(&mut p, B, "b2");
    p.passphrase = PASS.into();
    assert!(p.submit(unretire));
    p.wait();
    assert!(p.error.is_none(), "{:?}", p.error);
    assert!(!registry(&r).reviewer(B).unwrap().key("b2").unwrap().is_retired());

    // Bob cannot revoke himself as a reviewer; the founder can.
    let revoke = UserAction::RevokeReviewer { reviewer: B.into(), compromised: false };
    assert!(!p.problems(&revoke).is_empty());
    choose(&mut p, BY, "k1");
    p.passphrase = PASS.into();
    assert!(p.submit(revoke.clone()));
    p.wait();
    assert!(p.error.is_none(), "{:?}", p.error);
    let reg = registry(&r);
    assert_eq!(reg.reviewer(B).unwrap().revocation.as_ref().unwrap().signed, Ok("k1".into()));
    assert!(p.problems(&revoke).iter().any(|x| x.contains("revoked already")));
    let ro = p.roster().unwrap();
    assert!(matches!(ro.reviewer(B).unwrap().status, Status::Revoked { .. }));
    assert!(ro.reviewer(B).unwrap().keys[1].history.iter().any(|h| h.contains("unretired") && h.ends_with('\u{2714}')));
    // A key revocation by the founder (compromised) on the founder's own
    // key would be self-inflicted; on Bob's b1 it is recorded.
    let rk = UserAction::RevokeKey { reviewer: B.into(), key: "b1".into(), compromised: true };
    assert_eq!(rk.label(), "Key compromised");
    assert!(p.submit(rk.clone()));
    p.wait();
    assert!(p.error.is_none(), "{:?}", p.error);
    assert!(p.problems(&rk).iter().any(|x| x.contains("revoked already")));
}

/// Methodology: the plain-word texts of the warnings strip and the list
/// markers, over every warning and status kind the registry can give.
/// Pass: each names the reviewer and key it is about, and says whether
/// stamps count.
///
/// Result (2026-10-22): passes.
#[test]
fn warnings_and_statuses_read_in_plain_words() {
    use kovan_common::review::signing::registry::{
        AdmissionProblem, FounderProblem, HistoryProblem, RevocationProblem, SignerProblem, Warning,
    };
    let ws = [
        Warning::NoFounder(FounderProblem::NotDeclared),
        Warning::NoFounder(FounderProblem::Unknown("x".into())),
        Warning::NoFounder(FounderProblem::NotMaintainer("x".into())),
        Warning::NoFounder(FounderProblem::HasAdmission("x".into())),
        Warning::ReviewerNotAdmitted { reviewer: "r".into(), problem: AdmissionProblem::Signer(SignerProblem::BadSignature) },
        Warning::KeyNotTrusted { reviewer: "r".into(), key: "k".into(), problem: KeyProblem::Unendorsed },
        Warning::KeyReset { reviewer: "r".into(), key: "k".into(), status: KeyStatus::ResetTrustedOnFirstUse },
        Warning::KeyHistory {
            reviewer: "r".into(),
            key: "k".into(),
            problem: HistoryProblem::BadEventSignature {
                index: 2,
                event: kovan_common::review::root::KeyEventKind::Endorsed,
                problem: SignerProblem::SignerRetired { signer: "m".into(), key: "k1".into() },
            },
        },
        Warning::RevocationUnverified { reviewer: "r".into(), problem: RevocationProblem::Signer(SignerProblem::Unsigned) },
    ];
    let texts: Vec<String> = ws.iter().map(warning_text).collect();
    assert!(texts[..4].iter().all(|t| t.contains("nothing is trusted")));
    assert!(texts[4].contains("does not verify") && texts[4].contains("don't count"));
    assert!(texts[5].contains("unendorsed") && texts[5].contains("key k"));
    assert!(texts[6].contains("reset") && texts[6].contains("trusted"));
    assert!(texts[7].contains("entry 2 (endorsed)") && texts[7].contains("retired"));
    assert!(texts[8].contains("honoured"));
    let revoked = Status::Revoked { date: "2026-11-02".into(), compromised_from: Some("2026-11-01".into()) };
    assert_eq!(revoked.marker(), "\u{2718}");
    assert!(revoked.label().contains("void"));
    assert!(Status::KeyAttention("key b2".into()).label().starts_with('\u{26a0}'));
    for p in [
        KeyProblem::DuplicateId,
        KeyProblem::ReviewerNotAdmitted,
        KeyProblem::BadCreatedDate("x".into()),
        KeyProblem::UnsupportedAlgorithm("rsa".into()),
    ] {
        assert!(!key_problem_text(&p).is_empty());
    }
    assert_eq!(AdmitForm { role: Role::Reviewer, novice: false, scope: "a/**, b/**\n c/**".into() }.globs(), vec!["a/**", "b/**", "c/**"]);
}

/// Every engine verdict for the functions under `prefix`, one line per
/// function and one per review (states with their details), for a
/// before/after comparison of a change to the trust rules.
pub(crate) fn verdict_lines(ev: &kovan_common::review::engine::Evaluation, prefix: &str) -> Vec<String> {
    let mut out = Vec::new();
    for f in ev.functions.values().chain(ev.deleted.iter()) {
        let at = f.location.as_ref().map_or(f.id.clone(), |l| format!("{}::{}", l.file, l.qual));
        if !at.starts_with(prefix) && !f.id.starts_with(prefix) {
            continue;
        }
        out.push(format!("{at} {} fn={:?} rung={:?}", f.id, f.state, f.rung));
        for r in &f.reviews {
            out.push(format!("  by={} {:?} {:?} {:?}", r.by, r.artifact, r.date, r.state));
        }
    }
    out
}

/// Methodology (GitHub #1019, the trust rules' no-change gate): run the
/// engine with signatures enforced (`evaluate_workspace`) and list every
/// verdict ([`verdict_lines`]). On the fixture with one stamp, the list
/// holds `twice` as valid. With `KOVAN_STAMP_DUMP_ROOT` (a real workspace)
/// and `KOVAN_STAMP_DUMP_OUT` set, the verdicts under
/// `KOVAN_STAMP_DUMP_PREFIX` there are written to that file, so the same
/// stamps can be judged before and after a change and the files compared.
/// With `KOVAN_STAMP_DUMP_FACTS` set as well, every [`GitFacts`] field
/// and every Code Review row (permalinks included) is written there
/// (GitHub #1023: a change to how the git facts are gathered).
///
/// Result (2026-10-22): passes; the boon-lay comparison is recorded on
/// GitHub #1019, the #1023 one on #1023.
///
/// [`GitFacts`]: kovan_common::review::engine::GitFacts
#[test]
fn engine_verdicts_can_be_dumped_for_a_before_after_comparison() {
    let r = Repo::new();
    let store = tempfile::tempdir().unwrap();
    let key = founder_key(&r, store.path());
    r.stamp(&key, "twice", "");
    let ev = super::super::evaluate_workspace(r.path()).unwrap();
    let lines = verdict_lines(&ev.evaluation, "crates/demo");
    assert!(lines.iter().any(|l| l.contains("::twice") && l.contains("fn=Valid")), "{lines:?}");
    if let (Some(root), Some(out)) = (std::env::var_os("KOVAN_STAMP_DUMP_ROOT"), std::env::var_os("KOVAN_STAMP_DUMP_OUT")) {
        let prefix = std::env::var("KOVAN_STAMP_DUMP_PREFIX").unwrap_or_default();
        let ev = super::super::evaluate_workspace(Path::new(&root)).unwrap();
        std::fs::write(out, verdict_lines(&ev.evaluation, &prefix).join("\n")).unwrap();
        // Every git fact and every row the Code Review shows (permalinks
        // included), for a change to how the facts are gathered (#1023).
        if let Some(facts) = std::env::var_os("KOVAN_STAMP_DUMP_FACTS") {
            let ws = super::super::load_workspace(Path::new(&root)).unwrap();
            let g = super::super::git_facts(Path::new(&root), &ws);
            let rows = super::super::stamp_states(Path::new(&root)).unwrap();
            std::fs::write(facts, format!("{g:#?}\n{rows:#?}\n")).unwrap();
        }
    }
}

/// Bob's key `id`, unlocked from the temporary keystore.
pub(crate) fn bob_key(store: &Path, id: &str) -> kovan_common::review::signing::keystore::UnlockedKey {
    Keystore::at(store).load(B, id).unwrap().unlock(PASS).unwrap()
}

/// Draft, sign as `by` with `key` and write a stamp of `qual`, dated
/// `date` (`None`: today, as the dialog does), then commit.
pub(crate) fn stamp_as(r: &Repo, key: &kovan_common::review::signing::keystore::UnlockedKey, by: &str, qual: &str, date: Option<&str>) {
    use super::super::tests::clean;
    use super::super::{draft_stamp, write_review, StampRequest};
    let req = StampRequest { function: format!("{LIB}::{qual}"), by: by.into(), checklist: clean(), ..StampRequest::default() };
    let mut d = draft_stamp(r.path(), &req).unwrap();
    if let Some(dt) = date {
        d.entry.review.date = dt.into();
    }
    key.sign_review(&mut d.entry).unwrap();
    write_review(r.path(), &d.entry, "").unwrap();
    r.commit(&format!("{by} stamps {qual}"));
}

/// Methodology (GitHub #1019 phase 2, the novice rule end to end, real git
/// facts and signatures enforced): the founder admits Bob through the panel
/// as a **novice** reviewer of `crates/demo/**` (admission plus a novice
/// event, one write). Bob stamps `twice`; then the founder stamps it too.
/// The founder promotes Bob from a later date (2099-01-01, written with
/// `apply_action`: the stamp dialog always dates today), and Bob stamps
/// `leaf` dated after it. Pass: Bob's stamp alone leaves `twice` at
/// "1 of 2: awaiting a trusted reviewer" (not reviewed); with the
/// founder's beside it `twice` is valid; Bob's `leaf` stamp dated after the
/// promotion is valid alone; the roster shows Bob as a novice, then
/// trusted, with each event in his standing history; a forged promotion
/// (signed bytes edited by hand) leaves the mark in force.
///
/// Result (2026-10-22): passes.
#[test]
fn a_novice_needs_a_second_sign_off_until_promoted() {
    let r = Repo::new();
    let store = tempfile::tempdir().unwrap();
    let fk = founder_key(&r, store.path());
    bob_registers(&r, store.path(), "b1");
    let mut p = UserPanel::new(r.path().into(), Keystore::at(store.path()), SharedKeySession::default());
    p.wait();
    choose(&mut p, BY, "k1");
    p.passphrase = PASS.into();
    assert!(p.submit(UserAction::Admit { reviewer: B.into(), role: Role::Reviewer, scope: vec!["crates/demo/**".into()], novice: true }));
    p.wait();
    assert!(p.error.is_none(), "{:?}", p.error);
    assert!(p.notices[0].contains("novice reviewer"), "{:?}", p.notices);
    r.commit("admit bob, novice");
    let row = p.roster().unwrap().reviewer(B).unwrap().clone();
    assert!(row.role_label.contains("novice everywhere"), "{}", row.role_label);
    assert!(row.standing.iter().any(|l| l.contains("marked novice (everywhere)")), "{:?}", row.standing);

    let bk = bob_key(store.path(), "b1");
    stamp_as(&r, &bk, B, "twice", None);
    let s = r.state("twice").unwrap();
    assert_eq!(s.state, Some(StateKind::AwaitingTrustedReviewer), "{}", s.reason);
    assert_eq!(StateKind::AwaitingTrustedReviewer.label(), "1 of 2: awaiting a trusted reviewer");
    assert!(!StateKind::AwaitingTrustedReviewer.counts());
    r.stamp(&fk, "twice", "");
    assert_eq!(r.state("twice").unwrap().state, Some(StateKind::Valid), "novice plus trusted");

    // Promotion from 2099-01-01; Bob's later stamp counts alone.
    let later = "2099-01-01";
    apply_action(r.path(), &fk, &UserAction::Promote { reviewer: B.into(), scope: vec![] }, later).unwrap();
    r.commit("promote bob");
    stamp_as(&r, &bk, B, "leaf", Some("2099-01-02"));
    assert_eq!(r.state("leaf").unwrap().state, Some(StateKind::Valid), "a promoted reviewer's later stamp");
    p.load();
    p.wait();
    let row = p.roster().unwrap().reviewer(B).unwrap().clone();
    assert!(row.novice.is_none() && row.role_label == "Reviewer", "{}", row.role_label);
    assert!(row.standing.iter().any(|l| l.starts_with("2099-01-01 promoted to trusted (everywhere)")), "{:?}", row.standing);

    // A forged promotion: copy the real one, widen its scope by hand.
    let text = r.read(ROOT_FILE);
    let forged = text.replacen("event = \"trusted\"\ndate = \"2099-01-01\"", "event = \"trusted\"\ndate = \"2099-01-01\"\nscope = [\"crates/demo/**\"]", 1);
    assert_ne!(forged, text);
    r.write(ROOT_FILE, &forged);
    let reg = registry(&r);
    assert!(reg.reviewer(B).unwrap().novice_at("2099-01-02", "crates/demo/src/lib.rs"), "the forged promotion has no effect");
    assert!(reg.warnings().iter().any(|w| matches!(w, kovan_common::review::signing::registry::Warning::RosterEvent { .. })));
    r.write(ROOT_FILE, &text);
}

/// Methodology (GitHub #1019 phase 2, continuity, through the panel): the
/// founder stamps `twice`; Bob is admitted as a reviewer; the founder
/// sets Bob's scope, then hands maintainer to Bob and steps down (one
/// write, two signed roster events); Bob then admits Carol. Pass: the
/// preconditions refuse a scope equal to the one in force, a handover to
/// oneself, and (after the handover) any roster action by the former
/// founder; the founder's earlier stamp of `twice` is still valid; the
/// roster shows Bob as Maintainer and the founder as "Reviewer · founder
/// (stepped down)"; Bob's admission of Carol is verified.
///
/// Result (2026-10-22): passes.
#[test]
fn scope_change_and_handover_through_the_panel() {
    let r = Repo::new();
    let store = tempfile::tempdir().unwrap();
    let fk = founder_key(&r, store.path());
    r.stamp(&fk, "twice", "");
    bob_registers(&r, store.path(), "b1");
    let mut p = UserPanel::new(r.path().into(), Keystore::at(store.path()), SharedKeySession::default());
    p.wait();
    choose(&mut p, BY, "k1");
    p.passphrase = PASS.into();
    assert!(p.submit(UserAction::Admit { reviewer: B.into(), role: Role::Reviewer, scope: vec!["crates/demo/**".into()], novice: false }));
    p.wait();
    let same = UserAction::ChangeScope { reviewer: B.into(), scope: vec!["crates/demo/**".into()] };
    assert!(p.problems(&same).iter().any(|x| x.contains("in force already")));
    let widen = UserAction::ChangeScope { reviewer: B.into(), scope: globs("crates/demo/**, crates/other/**") };
    assert!(p.submit(widen));
    p.wait();
    assert!(p.error.is_none(), "{:?}", p.error);
    assert_eq!(registry(&r).reviewer(B).unwrap().current_scope(), ["crates/demo/**", "crates/other/**"]);
    assert!(p.problems(&UserAction::HandOver { to: BY.into(), step_down: true }).iter().any(|x| x.contains("someone else")));
    assert!(p.submit(UserAction::HandOver { to: B.into(), step_down: true }));
    p.wait();
    assert!(p.error.is_none(), "{:?}", p.error);
    assert!(p.notices.last().unwrap().contains("stepped down"), "{:?}", p.notices);
    r.commit("handover");
    let ro = p.roster().unwrap().clone();
    assert_eq!(ro.reviewer(B).unwrap().role_label, "Maintainer");
    assert_eq!(ro.reviewer(BY).unwrap().role_label, "Reviewer \u{b7} founder (stepped down)");
    assert_eq!(r.state("twice").unwrap().state, Some(StateKind::Valid), "A's past stamps keep their standing");
    assert!(p.problems(&UserAction::MarkNovice { reviewer: B.into(), scope: vec![] }).iter().any(|x| x.contains("admitted maintainer")));
    // Carol registers; Bob admits her.
    let ks = Keystore::at(store.path());
    let (cf, _) = generate("github:carol", "c1", &today(), PASS).unwrap();
    ks.save(&cf).unwrap();
    register_key(r.path(), &cf, Some("Carol"), &today()).unwrap();
    r.commit("carol registers");
    p.load();
    p.wait();
    choose(&mut p, B, "b1");
    p.passphrase = PASS.into();
    assert!(p.submit(UserAction::Admit { reviewer: "github:carol".into(), role: Role::Reviewer, scope: vec!["crates/demo/**".into()], novice: false }));
    p.wait();
    assert!(p.error.is_none(), "{:?}", p.error);
    assert_eq!(
        registry(&r).reviewer("github:carol").unwrap().admission,
        Admission::Admitted { by: B.into(), key: "b1".into() }
    );
    assert!(p.problems(&UserAction::ChangeRole { reviewer: B.into(), role: Role::Maintainer }).iter().any(|x| x.contains("role already")));
    assert_eq!(UserAction::ChangeRole { reviewer: B.into(), role: Role::Reviewer }.label(), "Step down to reviewer");
}
