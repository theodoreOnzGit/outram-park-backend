//! Tests of the stamp dialog's state machine ([`super::flow`]) on the
//! throwaway git repository of [`super::tests`], with a temporary keystore
//! and a test passphrase. Nothing touches the real keystore, `~/.config/kovan`
//! or any `review.md` / `kovan_root.toml` of the real workspace.

use std::collections::BTreeMap;

use kovan_common::review::review_md::parse_review_md;
use kovan_common::review::signing::keystore::{generate, Keystore};
use kovan_common::review::state::StateKind;
use kovan_common::review::wizard::{Applicability, GateReason};

use super::flow::*;
use super::tests::{clean, founder_key, today, Repo, BY, DIR, LIB, PASS};
use super::*;

fn target(qual: &str) -> Target {
    Target {
        function: format!("{LIB}::{qual}"),
        name: qual.into(),
    }
}

fn context(answers: BTreeMap<String, String>) -> StampContext {
    StampContext {
        fn_id: "fn:00".into(),
        call_graph_id: format!("{LIB}::twice"),
        path: format!("{LIB}::twice"),
        review_md: format!("{DIR}/review.md"),
        applicability: Applicability::default(),
        answers,
        restamp: false,
    }
}

/// Replace the wizard's answers with `answers` (option and text), keeping
/// the context the flow prepared.
fn set_answers(f: &mut StampFlow, answers: &BTreeMap<String, String>) {
    let w = f.wizard.as_mut().expect("on the wizard");
    let ctx = StampContext {
        answers: answers.clone(),
        ..w.ctx.clone()
    };
    *w = WizardForm::new(ctx);
}

/// The setup form refuses an empty or malformed reviewer id (the
/// registry's own check), an empty passphrase and a confirmation that
/// differs; a complete form has no problem.
#[test]
fn key_setup_form_is_checked() {
    let mut f = KeySetupForm::default();
    assert_eq!(f.problems().len(), 2);
    f.reviewer = "not an id".into();
    f.passphrase = "abc".into();
    f.confirm = "abd".into();
    let p = f.problems();
    assert!(p[0].starts_with("Reviewer id:"), "{p:?}");
    assert!(p[1].contains("differ"), "{p:?}");
    f.reviewer = "github:tester".into();
    f.confirm = "abc".into();
    assert!(f.problems().is_empty(), "{:?}", f.problems());
}

/// The wizard form: prefilled answers parse into option and text, only
/// applicable questions are answered, a text option without text blocks,
/// a prompting answer offers needs fix with a note, and the derived rung
/// is 3 for the clean set.
#[test]
fn wizard_form_answers_and_gate() {
    let mut a = clean();
    a.insert("upstream_fidelity".into(), "matches".into()); // not a port
    a.insert(
        "error_handling".into(),
        "panics_justified: the table is compiled in".into(),
    );
    let mut w = WizardForm::new(context(a));
    assert_eq!(
        w.texts.get("error_handling").map(String::as_str),
        Some("the table is compiled in")
    );
    let ans = w.answers();
    assert!(!ans.contains_key("upstream_fidelity"));
    assert_eq!(
        ans["error_handling"],
        "panics_justified: the table is compiled in"
    );
    assert_eq!(ans.len(), w.questions().len());
    let g = w.gate();
    assert!(g.stampable(), "{:?}", g.blocked_by);
    assert_eq!(g.rung.as_u8(), 3);

    w.texts.insert("error_handling".into(), "x".into());
    assert!(matches!(
        w.gate().blocked_by.as_slice(),
        [GateReason::Invalid(_)]
    ));
    w.texts.insert("error_handling".into(), "ok now".into());
    w.options
        .insert("doc_matches_behaviour".into(), "partly".into());
    let g = w.gate();
    assert!(g.stampable() && g.prompts.len() == 1);
    assert!(!w.needs_fix_note().is_empty());
    w.options.remove("test_reach");
    assert!(w
        .gate()
        .blocked_by
        .contains(&GateReason::Unanswered("test_reach".into())));
}

/// Hints and plain words.
#[test]
fn refusal_hints_and_registration_words() {
    assert!(refusal_hint("a.rs has changes not committed: …").contains("Commit or stash"));
    assert!(
        refusal_hint("x is not in any kovan.toml (run kovan-cli index)").contains("Index fresh")
    );
    assert!(
        refusal_hint("a.rs::f: the index hash is not the function's hash at HEAD")
            .contains("Index fresh")
    );
    assert!(describe_registration(&KeyRegistration::Founder).contains("founding"));
    assert!(describe_registration(&KeyRegistration::AwaitingAdmission).contains("admission"));
    assert!(describe_registration(&KeyRegistration::KeyAdded {
        needs_endorsement: true
    })
    .contains("endorsement"));
}

/// `register_key` gives trust on first use when `[code_review] founder`
/// already names the key's reviewer (what "Index fresh" writes from the
/// keystore), and not when it names someone else.
#[test]
fn register_key_honours_a_declared_founder() {
    let r = Repo::new();
    r.write(
        ROOT_FILE,
        "schema_version = 1\n\n[code_review]\nfounder = \"github:tester\"\n",
    );
    let (kf, _) = generate(BY, "k1", &today(), PASS).unwrap();
    assert_eq!(
        register_key(r.path(), &kf, None, &today()).unwrap(),
        KeyRegistration::Founder
    );
    let root = kovan_common::review::root::ReviewRoot::parse(&r.read(ROOT_FILE)).unwrap();
    assert_eq!(root.reviewer(BY).unwrap().keys.len(), 1);

    r.write(
        ROOT_FILE,
        "schema_version = 1\n\n[code_review]\nfounder = \"github:someone\"\n",
    );
    assert_eq!(
        register_key(r.path(), &kf, None, &today()).unwrap(),
        KeyRegistration::AwaitingAdmission
    );
}

/// Methodology: the whole dialog on the fixture with an empty temporary
/// keystore: open on `twice`, set up a key (generate, save, register as
/// founder), get the wizard, answer, sign with a wrong passphrase (refused,
/// nothing written, answers kept), sign with the right one, refresh;
/// commit `review.md` as the maintainer would, refresh again. Then open it
/// again on the same function. Pass: SetupKey -> Wizard -> Done; the signed
/// checklist equals the answers; the state is unverified (not committed)
/// before the commit and valid at rung 3 after; the second opening is a
/// re-stamp prefilled with the previous answers.
///
/// Result (2026-10-10): passes.
#[test]
fn flow_sets_up_a_key_signs_and_turns_valid() {
    let r = Repo::new();
    let store = tempfile::tempdir().unwrap();
    let ks = Keystore::at(store.path());
    let mut f = StampFlow::new(
        r.path().to_path_buf(),
        ks.clone(),
        Purpose::Stamp,
        target("twice"),
    );
    f.wait();
    assert_eq!(f.step, Step::SetupKey);
    assert!(!f.generate_key(), "an empty form is refused");
    f.setup.reviewer = BY.into();
    f.setup.name = "Test Reviewer".into();
    f.setup.passphrase = PASS.into();
    f.setup.confirm = PASS.into();
    assert!(f.generate_key());
    assert!(f.setup.passphrase.is_empty() && f.setup.confirm.is_empty());
    f.wait();
    assert_eq!(f.step, Step::Wizard, "{:?} {:?}", f.error, f.notices);
    assert!(
        f.notices.iter().any(|n| n.contains("founding")),
        "{:?}",
        f.notices
    );
    assert_eq!(ks.list().unwrap().len(), 1);
    assert!(f.key().unwrap().registered);
    r.commit("register key");

    set_answers(&mut f, &clean());
    assert!(!f.can_sign(), "no passphrase yet");
    f.wizard.as_mut().unwrap().passphrase = "wrong passphrase".into();
    assert!(f.sign());
    f.wait();
    assert_eq!(f.step, Step::Wizard);
    assert!(f.error.as_deref().unwrap().contains("Wrong passphrase"));
    assert!(!r.path().join(DIR).join(REVIEW_MD).exists());
    assert_eq!(
        f.wizard.as_ref().unwrap().answers(),
        clean(),
        "answers kept"
    );

    f.wizard.as_mut().unwrap().passphrase = PASS.into();
    f.wizard.as_mut().unwrap().comments = "Checked by hand.".into();
    assert!(f.sign());
    f.wait();
    let Step::Done(o) = &f.step else {
        panic!("{:?} {:?}", f.step, f.error)
    };
    assert_eq!(
        o,
        &Outcome {
            what: Purpose::Stamp,
            review_md: format!("{DIR}/{REVIEW_MD}"),
            replaced: false
        }
    );
    // What commit-and-push may commit (#771): the review.md and the root
    // the key registration wrote.
    assert_eq!(
        f.wrote,
        [format!("{DIR}/{REVIEW_MD}"), ROOT_FILE.to_string()].into()
    );
    let md = r.read(&format!("{DIR}/{REVIEW_MD}"));
    let doc = parse_review_md(&md);
    let entry = doc.reviews().next().unwrap();
    assert_eq!(entry.review.checklist, clean(), "signed = answered");
    assert!(entry.review.signature.is_some() && md.contains("Checked by hand."));
    assert!(f.take_states().is_some());
    let s = f.target_state.clone().expect("a state for twice");
    assert_eq!(s.state, Some(StateKind::Unverified), "{}", s.reason);

    r.commit("stamp twice");
    f.refresh();
    f.wait();
    let s = f.target_state.clone().unwrap();
    assert_eq!(
        (s.state, s.rung),
        (Some(StateKind::Valid), 3),
        "{}",
        s.reason
    );

    let mut again = StampFlow::new(r.path().to_path_buf(), ks, Purpose::Stamp, target("twice"));
    again.wait();
    assert_eq!(again.step, Step::Wizard);
    let w = again.wizard.as_ref().unwrap();
    assert!(w.ctx.restamp);
    assert_eq!(w.answers(), clean());
}

/// Methodology: with a registered founder key, open Stamp on `twice` while
/// its file has an uncommitted edit, then revert and Try again, then take
/// "Mark as Needs fix instead?" and write the needs-fix. Pass: the
/// refusal is shown verbatim with the commit-or-stash hint, the retry
/// reaches the wizard, and the needs-fix is written (unsigned, as the
/// library defines it) and shows as open after the refresh.
///
/// Result (2026-10-10): passes.
#[test]
fn flow_refuses_a_dirty_file_then_writes_needs_fix() {
    let r = Repo::new();
    let store = tempfile::tempdir().unwrap();
    founder_key(&r, store.path());
    r.edit("leaf(x) + 1.0", "leaf(x) + 5.0");
    let mut f = StampFlow::new(
        r.path().to_path_buf(),
        Keystore::at(store.path()),
        Purpose::Stamp,
        target("twice"),
    );
    f.wait();
    let Step::Refused { message, hint } = &f.step else {
        panic!("{:?}", f.step)
    };
    assert!(message.contains("not committed"), "{message}");
    assert!(hint.contains("Commit or stash"));
    r.git(&["checkout", "--", LIB]);
    f.prepare();
    f.wait();
    assert_eq!(f.step, Step::Wizard);

    let mut a = clean();
    a.insert("doc_matches_behaviour".into(), "partly".into());
    set_answers(&mut f, &a);
    assert_eq!(f.wizard.as_ref().unwrap().gate().prompts.len(), 1);
    f.switch_to_needs_fix();
    assert_eq!(
        (f.step.clone(), f.purpose),
        (Step::NeedsFixForm, Purpose::NeedsFix)
    );
    assert!(!f.note.is_empty());
    f.note = "the doc says twice but it adds one".into();
    assert!(f.write_needs_fix());
    f.wait();
    assert!(matches!(&f.step, Step::Done(o) if o.what == Purpose::NeedsFix));
    let s = f.target_state.clone().unwrap();
    assert_eq!(s.state, Some(StateKind::NeedsFixOpen), "{}", s.reason);
}

/// No `kovan_root.toml`: the dialog stops at NoRoot before any key is
/// made. Several keys: the picker; an unregistered pick asks to register,
/// registering an unknown reviewer gives "awaiting admission" and the flow
/// goes on with the same key to the wizard.
#[test]
fn flow_without_root_and_with_several_keys() {
    let r = Repo::new();
    std::fs::remove_file(r.path().join(ROOT_FILE)).unwrap();
    let store = tempfile::tempdir().unwrap();
    let ks = Keystore::at(store.path());
    let mut f = StampFlow::new(
        r.path().to_path_buf(),
        ks.clone(),
        Purpose::Stamp,
        target("twice"),
    );
    f.wait();
    assert_eq!(f.step, Step::NoRoot);
    assert!(ks.list().unwrap().is_empty());

    let r = Repo::new();
    founder_key(&r, store.path());
    let (kf, _) = generate("github:second", "s1", &today(), PASS).unwrap();
    ks.save(&kf).unwrap();
    let mut f = StampFlow::new(r.path().to_path_buf(), ks, Purpose::Stamp, target("twice"));
    f.wait();
    assert_eq!(f.step, Step::PickKey);
    f.chosen = f
        .keys
        .iter()
        .position(|k| k.reviewer == "github:second")
        .unwrap();
    f.use_key();
    assert_eq!(f.step, Step::Register);
    assert!(f.register_chosen());
    f.wait();
    assert_eq!(f.step, Step::Wizard, "{:?}", f.error);
    assert_eq!(f.key().unwrap().reviewer, "github:second");
    assert!(
        f.notices.iter().any(|n| n.contains("admission")),
        "{:?}",
        f.notices
    );
}
