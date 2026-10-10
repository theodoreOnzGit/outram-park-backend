//! End-to-end tests of review mode on the throwaway git repository of
//! `stamping::tests` (`Repo`: `twice` calls `leaf`), with a temporary
//! keystore and a test passphrase. Nothing touches the real keystore,
//! `~/.config/kovan` or any `review.md` / `kovan_root.toml` of the real
//! workspace.

use std::collections::BTreeMap;

use kovan_common::artifact::{render_block, ARTIFACT_LEVEL};
use kovan_common::review::review_md::{parse_review_md, Entry, EntryMeta, UpstreamEntry, UpstreamTable};
use kovan_common::review::signing::keystore::Keystore;
use kovan_common::review::state::StateKind;

use super::super::flow::{Purpose, StampFlow, Step, Target};
use super::super::tests::{clean, founder_key, today, Repo, BY, DIR, LIB, PASS};
use super::super::*;
use super::*;

fn cg(qual: &str) -> String {
    format!("{LIB}::{qual}")
}

/// What "Index fresh" does after a code commit, on the fixture: refresh
/// the folder's `kovan.toml` from the committed source (hashes now, ids
/// kept), mark it up to date, commit.
fn reindex(r: &Repo) {
    r.reindex();
}

/// Methodology: on the fixture, try to prepare `twice` before `leaf` has a
/// stamp, then stamp `leaf` (library: draft, sign, write, commit) and
/// prepare again. Pass: the first is refused with the bottom-up message
/// naming `leaf` and the flow's hint; the second gives the wizard's context
/// with the function's hash and no no-concept prefill (not a port).
///
/// Result (2026-10-10): passes.
#[test]
fn bottom_up_is_enforced_before_the_wizard() {
    let r = Repo::new();
    let store = tempfile::tempdir().unwrap();
    let key = founder_key(&r, store.path());
    let e = prepare_stamp(r.path(), &cg("twice"), BY).unwrap_err();
    assert!(e.starts_with("bottom-up") && e.contains("leaf"), "{e}");
    assert!(flow::refusal_hint(&e).contains("Bottom-up"));
    r.stamp(&key, "leaf", "");
    let ctx = prepare_stamp(r.path(), &cg("twice"), BY).unwrap();
    let ws = load_workspace(r.path()).unwrap();
    assert_eq!(ctx.hash, ws.find(&cg("twice")).unwrap().2.hash);
    assert_eq!(ctx.no_concept, None);
    assert_eq!(ctx.suggested_architecture, None);
}

/// Methodology: stamp `leaf` then `twice` (comments "Looked at it."),
/// commit; edit `twice`'s body and commit; then edit `leaf` and commit.
/// Build the review panel's view and the walk at each step. Pass:
/// (1) after the stamps `twice` is valid, its diff is unchanged, its
/// section's comments read back and the walk is leaf then twice, both
/// ticked; (2) after its own edit the diff shows one line removed and one
/// added, the state is directly stale; (3) after the callee edit the view
/// lists `leaf` as blocking and carries `leaf`'s diff since the review.
///
/// Result (2026-10-10): passes.
#[test]
fn function_view_diff_sections_blockers_and_walk() {
    let r = Repo::new();
    let store = tempfile::tempdir().unwrap();
    let key = founder_key(&r, store.path());
    r.stamp(&key, "leaf", "");
    r.stamp(&key, "twice", "Looked at it.");
    let snap = Snapshot::load(r.path()).unwrap();
    let v = function_view(r.path(), &snap, &cg("twice")).unwrap();
    assert_eq!(v.state, StateKind::Valid, "{}", v.reason);
    assert!(v.source.starts_with("/// Twice x, plus one.") && v.lines == [6, 9]);
    assert_eq!(v.review_md, format!("{DIR}/review.md"));
    let d = v.own_diff.clone().unwrap().unwrap();
    assert!(d.diff.unchanged());
    assert_eq!(v.sections.len(), 1);
    assert_eq!(v.sections[0].comments, "Looked at it.");
    assert!(
        v.blocked_by.is_empty() && v.callee_diffs.is_empty() && v.flags.is_empty(),
        "{:?}",
        v.flags
    );
    assert!(v.upstream_url.is_none() && !v.is_port);
    let p = plan_walk(&snap, &cg("twice")).unwrap();
    let order: Vec<&str> = p.items().map(|i| i.name.as_str()).collect();
    assert_eq!(order, vec!["leaf", "twice"]);
    assert_eq!(p.size().already_valid, 2);
    assert_eq!(walk::next(&p, &walk::Trail::default()), walk::Next::Done);

    r.edit("leaf(x) + 1.0", "leaf(x) + 2.0");
    r.commit("edit twice");
    let snap = Snapshot::load(r.path()).unwrap();
    let v = function_view(r.path(), &snap, &cg("twice")).unwrap();
    assert_eq!(v.state, StateKind::DirectlyStale, "{}", v.reason);
    let d = v.own_diff.unwrap().unwrap();
    assert_eq!(d.diff.counts(), (1, 1));
    assert!(d
        .diff
        .to_unified()
        .contains("-    leaf(x) + 1.0\n+    leaf(x) + 2.0\n"));
    let p = plan_walk(&snap, &cg("twice")).unwrap();
    let walk::Next::Review(i) = walk::next(&p, &walk::Trail::default()) else {
        panic!("twice is to review")
    };
    assert_eq!(i.name, "twice");

    r.edit("x * 2.0", "x * 3.0");
    r.commit("edit leaf");
    let snap = Snapshot::load(r.path()).unwrap();
    let v = function_view(r.path(), &snap, &cg("twice")).unwrap();
    assert_eq!(
        v.blocked_by
            .iter()
            .map(|(_, n)| n.as_str())
            .collect::<Vec<_>>(),
        vec!["leaf"]
    );
    assert_eq!(v.callee_diffs.len(), 1);
    let (name, cd) = &v.callee_diffs[0];
    assert_eq!(name, "leaf");
    assert_eq!(cd.as_ref().unwrap().diff.counts(), (1, 1));
    assert!(function_view(r.path(), &snap, &cg("nope")).is_err());
}

/// Methodology: the folder's `review.md` gets a confirmed `[upstream]`
/// entry (a port of a GitHub repository, `lib.rs` mapped to an upstream
/// file at a line), committed. Pass: preparing `leaf` pre-fills the
/// no-concept reason "upstream control flow / solver structure"; the view
/// links the upstream file at the commit and line.
///
/// Result (2026-10-10): passes.
#[test]
fn a_port_prefills_no_concept_and_links_upstream() {
    let r = Repo::new();
    let store = tempfile::tempdir().unwrap();
    founder_key(&r, store.path());
    let sha = "0123456789abcdef0123456789abcdef01234567";
    let up = UpstreamEntry {
        kovan: EntryMeta {
            id: "upstream-demo".into(),
            kind: "upstream".into(),
            origin: Some("human".into()),
            created: "2026-10-10T10:00:00+08:00".into(),
            modified: "2026-10-10T10:00:00+08:00".into(),
            target: None,
        },
        upstream: UpstreamTable {
            is_port: true,
            repository: Some("https://github.com/njoy/NJOY2016".into()),
            commit: Some(sha.into()),
            tag: None,
            files: BTreeMap::from([("lib.rs".to_string(), "src/broadr.f90:12-40".to_string())]),
            routines: BTreeMap::new(),
            confirmed_by: BY.into(),
            date: today(),
        },
    };
    let toml = Entry::Upstream(up).to_toml().unwrap();
    r.write(
        &format!("{DIR}/review.md"),
        &render_block(ARTIFACT_LEVEL, "Upstream", &toml, ""),
    );
    r.commit("confirm upstream");
    let ctx = prepare_stamp(r.path(), &cg("leaf"), BY).unwrap();
    assert!(ctx.applicability.is_port);
    assert_eq!(ctx.no_concept.as_deref(), Some(prefill::UPSTREAM_STRUCTURE));
    let snap = Snapshot::load(r.path()).unwrap();
    let v = function_view(r.path(), &snap, &cg("leaf")).unwrap();
    assert!(v.is_port);
    assert_eq!(
        v.upstream_url.as_deref(),
        Some(
            format!("https://github.com/njoy/NJOY2016/blob/{sha}/src/broadr.f90#L12-L40").as_str()
        )
    );
}

/// Methodology: enter review mode on the folder, stamp `leaf`, save edited
/// comments into its section, add a highlight on its body line; then
/// Cancel. Pass: the save changes only the comments (the entry still reads
/// back, the signature kept); a save over a changed file is refused; the
/// highlight is anchored on the line chosen; Cancel puts `review.md` back
/// as it was on entering (here: absent), and a second Cancel does nothing.
///
/// Result (2026-10-10): passes.
#[test]
fn sections_highlights_and_cancel() {
    let r = Repo::new();
    let store = tempfile::tempdir().unwrap();
    let key = founder_key(&r, store.path());
    let md = format!("{DIR}/review.md");
    let session = Session::enter(r.path(), &md);
    assert_eq!(session.entered, None);
    r.stamp(&key, "leaf", "First.");
    assert!(session.changed(r.path()));
    let snap = Snapshot::load(r.path()).unwrap();
    let v = function_view(r.path(), &snap, &cg("leaf")).unwrap();
    let s = &v.sections[0];
    let text = save_section(
        r.path(),
        &md,
        &v.review_md_text,
        s.line,
        "## Comments\n\nSecond.",
    )
    .unwrap();
    let doc = parse_review_md(&text);
    let entry = doc.reviews().next().unwrap();
    assert!(entry.review.signature.is_some());
    assert!(text.contains("Second.") && !text.contains("First."));
    assert!(save_section(r.path(), &md, &v.review_md_text, s.line, "x")
        .unwrap_err()
        .contains("changed since"));

    add_highlight(r.path(), &cg("leaf"), BY, 3, 3, "Why 2.0?").unwrap();
    let snap = Snapshot::load(r.path()).unwrap();
    let v = function_view(r.path(), &snap, &cg("leaf")).unwrap();
    assert_eq!(v.highlights.len(), 1);
    assert_eq!(v.highlights[0].lines, Some((3, 3)));
    assert_eq!(v.highlights[0].note, "Why 2.0?");
    assert_eq!(v.sections.len(), 2, "the review and the highlight");

    assert!(session.discard(r.path()).unwrap());
    assert!(!r.path().join(&md).exists());
    assert!(!session.discard(r.path()).unwrap());
}

/// Methodology: through the stamp dialog's flow, with `leaf` stamped:
/// open Stamp on `twice`, answer, choose no-concept Other with one
/// character (sign refused), then with text and the suggested
/// architecture absent; commit an edit to `twice` before signing. Pass:
/// the sign is refused because the function changed while reviewed
/// (nothing written, answers kept); preparing again and signing writes
/// the stamp with the no-concept reason in the wizard's answer form and the
/// sidebar's comments seed.
///
/// Result (2026-10-10): passes.
#[test]
fn flow_refuses_a_changed_hash_and_signs_no_concept() {
    let r = Repo::new();
    let store = tempfile::tempdir().unwrap();
    let key = founder_key(&r, store.path());
    r.stamp(&key, "leaf", "");
    let target = Target {
        function: cg("twice"),
        name: "twice".into(),
    };
    let mut f = StampFlow::new(
        r.path().to_path_buf(),
        Keystore::at(store.path()),
        Purpose::Stamp,
        target,
    );
    f.comments_seed = "From the sidebar.".into();
    f.wait();
    assert_eq!(f.step, Step::Wizard, "{:?}", f.step);
    {
        let w = f.wizard.as_mut().unwrap();
        assert_eq!(w.comments, "From the sidebar.");
        let ctx = w.ctx.clone();
        *w = flow::WizardForm::new(StampContext {
            answers: clean(),
            ..ctx
        });
        w.comments = "From the sidebar.".into();
        w.passphrase = PASS.into();
        w.no_concept.choice = Some(prefill::OTHER.into());
        w.no_concept.other = "x".into();
    }
    assert!(!f.can_sign(), "Other needs 2 characters");
    f.wizard.as_mut().unwrap().no_concept.other = "test plumbing".into();
    assert!(f.can_sign());
    r.edit("leaf(x) + 1.0", "leaf(x) + 4.0");
    r.commit("changed under the reviewer");
    reindex(&r);
    assert!(f.sign());
    f.wait();
    assert_eq!(f.step, Step::Wizard);
    let err = f.error.clone().unwrap();
    assert!(err.contains("changed while you were reviewing"), "{err}");
    let before = parse_review_md(&r.read(&format!("{DIR}/review.md")));
    assert_eq!(
        before.reviews().count(),
        1,
        "nothing written: only leaf's stamp"
    );

    f.prepare();
    f.wait();
    assert_eq!(f.step, Step::Wizard);
    {
        let w = f.wizard.as_mut().unwrap();
        let ctx = w.ctx.clone();
        *w = flow::WizardForm::new(StampContext {
            answers: clean(),
            ..ctx
        });
        w.comments = "From the sidebar.".into();
        w.passphrase = PASS.into();
        w.no_concept.choice = Some(prefill::OTHER.into());
        w.no_concept.other = "test plumbing".into();
    }
    assert!(f.sign());
    f.wait();
    assert!(
        matches!(f.step, Step::Done(_)),
        "{:?} {:?}",
        f.step,
        f.error
    );
    let doc = parse_review_md(&r.read(&format!("{DIR}/review.md")));
    let twice = doc
        .reviews()
        .find(|e| e.path().unwrap().ends_with("::twice"))
        .unwrap();
    assert_eq!(
        twice.review.no_concept.as_deref(),
        Some("other: test plumbing")
    );
    assert!(r
        .read(&format!("{DIR}/review.md"))
        .contains("From the sidebar."));
}

/// Methodology (#770, #740 decision 8): stamp `leaf` and `twice`, edit
/// `leaf` and commit (re-indexed): `twice` is inherited stale. Re-confirm is
/// refused while `leaf` has no valid stamp (bottom-up) and for `leaf`
/// itself (directly stale, not a callee change). Re-stamp `leaf`, then
/// re-confirm `twice` through the dialog's state machine (purpose
/// Reconfirm: key, prepare on a worker, the re-confirm step, a wrong then
/// the right passphrase), commit. Pass: the step lists `leaf` with its
/// diff (+1 -1); a wrong passphrase writes nothing and stays on the step;
/// the re-confirm replaces the review (one entry), keeps the checklist,
/// the previous comments and adds the re-confirm note; `twice` is valid
/// at rung 3 again; a second re-confirm is refused (it is valid now).
///
/// Result (2026-10-10): passes.
#[test]
fn inherited_stale_is_reconfirmed_without_the_wizard() {
    let r = Repo::new();
    let store = tempfile::tempdir().unwrap();
    let key = founder_key(&r, store.path());
    r.stamp(&key, "leaf", "");
    r.stamp(&key, "twice", "Checked against the doc.");
    r.edit("x * 2.0", "x * 3.0");
    r.commit("edit leaf");
    reindex(&r);
    assert_eq!(
        r.state("twice").unwrap().state,
        Some(StateKind::InheritedStale)
    );
    let e = reconfirm::prepare_reconfirm(r.path(), &cg("twice"), BY).unwrap_err();
    assert!(e.starts_with("bottom-up") && e.contains("leaf"), "{e}");
    let e = reconfirm::prepare_reconfirm(r.path(), &cg("leaf"), BY).unwrap_err();
    assert!(e.contains("only to a review made stale by a callee change"), "{e}");
    let e = reconfirm::prepare_reconfirm(r.path(), &cg("twice"), "github:nobody").unwrap_err();
    assert!(e.contains("no review"), "{e}");
    r.stamp(&key, "leaf", "Re-reviewed after the change.");

    let mut f = StampFlow::new(
        r.path().to_path_buf(),
        Keystore::at(store.path()),
        Purpose::Reconfirm,
        Target {
            function: cg("twice"),
            name: "twice".into(),
        },
    );
    f.wait();
    assert_eq!(f.step, Step::Reconfirm, "{:?}", f.step);
    let rc = f.reconfirm.clone().unwrap();
    assert_eq!(rc.changed.len(), 1);
    assert_eq!(rc.changed[0].name, "leaf");
    assert_eq!(rc.changed[0].diff.as_ref().unwrap().diff.counts(), (1, 1));
    assert_eq!(rc.previous_comments, "Checked against the doc.");
    assert!(!f.can_reconfirm(), "no passphrase yet");
    f.reconfirm_passphrase = "wrong passphrase".into();
    assert!(f.sign_reconfirm());
    f.wait();
    assert_eq!(f.step, Step::Reconfirm);
    assert!(f.error.as_deref().unwrap().contains("Wrong passphrase"));
    let md = r.read(&format!("{DIR}/review.md"));
    assert!(!md.contains("Re-confirmed on"), "nothing written");

    f.reconfirm_passphrase = PASS.into();
    assert!(f.sign_reconfirm());
    f.wait();
    let Step::Done(o) = &f.step else {
        panic!("{:?} {:?}", f.step, f.error)
    };
    assert_eq!((o.what, o.replaced), (Purpose::Reconfirm, true));
    assert!(f.reconfirm_passphrase.is_empty());
    r.commit("re-confirm twice");

    let md = r.read(&format!("{DIR}/review.md"));
    let doc = parse_review_md(&md);
    let twices: Vec<_> = doc
        .reviews()
        .filter(|e| e.path().unwrap().ends_with("::twice"))
        .collect();
    assert_eq!(twices.len(), 1, "replaced, not appended");
    assert_eq!(twices[0].review.checklist, clean());
    assert!(md.contains("Checked against the doc."), "{md}");
    assert!(md.contains("Re-confirmed on") && md.contains("leaf changed"), "{md}");
    let s = r.state("twice").unwrap();
    assert_eq!((s.state, s.rung), (Some(StateKind::Valid), 3), "{}", s.reason);
    let e = reconfirm::prepare_reconfirm(r.path(), &cg("twice"), BY).unwrap_err();
    assert!(e.contains("valid"), "{e}");

    // "Mark for re-review" leaves for the full wizard as a Stamp.
    r.edit("x * 3.0", "x * 5.0");
    r.commit("edit leaf again");
    reindex(&r);
    r.stamp(&key, "leaf", "");
    let mut f = StampFlow::new(
        r.path().to_path_buf(),
        Keystore::at(store.path()),
        Purpose::Reconfirm,
        Target {
            function: cg("twice"),
            name: "twice".into(),
        },
    );
    f.wait();
    assert_eq!(f.step, Step::Reconfirm, "{:?}", f.step);
    f.switch_to_review();
    f.wait();
    assert_eq!((f.purpose, &f.step), (Purpose::Stamp, &Step::Wizard), "{:?}", f.error);
    assert!(f.reconfirm.is_none());
}

/// Methodology (#770, #740 decision 11; #739 decision 21): through the
/// stamp dialog's state machine, link a concept with the finder while
/// stamping `leaf`. Pass: the finder finds the concept by its title;
/// choosing it clears the no-concept reason and a reason clears it again
/// (the two exclude each other); with neither, Sign is refused; the review
/// written carries `[[relation]] kind = "implements"` to the concept; the
/// re-stamp's wizard starts with the concept linked; the review panel's
/// view lists the concept with no formula ("no formula recorded").
///
/// Result (2026-10-10): passes.
#[test]
fn a_concept_is_linked_from_the_wizard() {
    let r = Repo::new();
    let store = tempfile::tempdir().unwrap();
    founder_key(&r, store.path());
    let concept = super::super::concepts::standard_concepts()
        .into_iter()
        .find(|c| c.level == 3)
        .unwrap();
    let open = |r: &Repo| {
        let mut f = StampFlow::new(
            r.path().to_path_buf(),
            Keystore::at(store.path()),
            Purpose::Stamp,
            Target {
                function: cg("leaf"),
                name: "leaf".into(),
            },
        );
        f.wait();
        assert_eq!(f.step, Step::Wizard, "{:?}", f.step);
        f
    };
    let mut f = open(&r);
    {
        let w = f.wizard.as_mut().unwrap();
        assert_eq!(w.concept, None);
        let ctx = w.ctx.clone();
        *w = flow::WizardForm::new(StampContext {
            answers: clean(),
            ..ctx
        });
        w.passphrase = PASS.into();
        assert!(w.concept_problem().is_some(), "neither a concept nor a reason");
        w.concept_query = concept.title.clone();
        let found: Vec<String> = w.concept_matches().iter().map(|c| c.id.clone()).collect();
        assert!(found.contains(&concept.id), "{found:?}");
        w.choose_reason(Some("plumbing"));
        w.choose_concept(&concept.id);
        assert_eq!(w.no_concept.choice, None, "a concept clears the reason");
        w.choose_reason(Some("plumbing"));
        assert_eq!(w.concept, None, "a reason unlinks the concept");
        w.choose_reason(None);
    }
    assert!(!f.can_sign());
    f.wizard.as_mut().unwrap().choose_concept(&concept.id);
    assert!(f.can_sign());
    assert!(f.sign());
    f.wait();
    assert!(matches!(f.step, Step::Done(_)), "{:?} {:?}", f.step, f.error);
    r.commit("stamp leaf with a concept");

    let md = r.read(&format!("{DIR}/review.md"));
    let doc = parse_review_md(&md);
    let leaf = doc.reviews().next().unwrap();
    assert_eq!(
        super::super::concepts::linked_concepts(&leaf.relations),
        vec![concept.id.clone()]
    );
    assert_eq!(leaf.review.no_concept, None);
    assert_eq!(r.state("leaf").unwrap().state, Some(StateKind::Valid));
    let ws = load_workspace(r.path()).unwrap();
    let areas = super::super::concepts::concept_areas(&ws);
    assert_eq!(
        areas.get(&leaf.function_id()).map(|a| a.iter().cloned().collect::<Vec<_>>()),
        Some(vec![concept.id.clone()])
    );

    let f = open(&r);
    assert_eq!(f.wizard.as_ref().unwrap().concept.as_deref(), Some(concept.id.as_str()));
    let snap = Snapshot::load(r.path()).unwrap();
    let v = function_view(r.path(), &snap, &cg("leaf")).unwrap();
    assert_eq!(v.concepts.len(), 1);
    assert_eq!((v.concepts[0].id.as_str(), v.concepts[0].title.as_str()), (concept.id.as_str(), concept.title.as_str()));
    assert!(v.concepts[0].formulas.is_empty(), "no formula recorded anywhere");
    assert_eq!(v.reconfirm, None);
}
