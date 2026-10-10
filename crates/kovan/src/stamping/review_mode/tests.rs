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
    use kovan_common::code_index::refresh::refresh_folder;
    use kovan_common::review::index::FolderIndex;
    let head = r.git(&["rev-parse", "HEAD"]).trim().to_string();
    let toml = format!("{DIR}/kovan.toml");
    let old = FolderIndex::parse(&r.read(&toml)).unwrap();
    let files = BTreeMap::from([("lib.rs".to_string(), r.read(LIB))]);
    let (mut idx, _) = refresh_folder("demo", DIR, Some(&old), &files, None, &[], &head);
    for m in idx.modules.values_mut() {
        for f in &mut m.functions {
            f.index_out_of_date = false;
        }
    }
    r.write(&toml, &idx.to_toml().unwrap());
    r.commit("reindex");
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
