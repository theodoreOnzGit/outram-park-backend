//! **Re-anchoring a module's reviews after a squash or rebase** (GitHub
//! #838; maintainer, 2026-10-12: "if squash commits occur, then stamps need
//! to be re-anchored to a new commit", "by modules, rather than by
//! functions").
//!
//! # The problem
//!
//! A stamp records the commit it certifies. A squash merge or a rebase
//! replaces that commit, so it is no longer in the branch's history and the
//! engine can no longer show the stamp was added after the code it
//! certifies: the review is
//! [`UnverifiedReason::CertifiedCommitNotInHistory`]. The function itself
//! did not change, so a full review is not needed, but the lost evidence
//! cannot be restored without the reviewer.
//!
//! # What a re-anchor is
//!
//! As for a re-confirm ([`super::reconfirm`], whose record form this
//! follows): **a new signed review by the same reviewer that replaces their
//! previous one**, carrying the previous checklist, `no_concept`, relations,
//! authorship and separation attestation unchanged, drafted at `HEAD`. The
//! comments keep the previous ones and add one line naming the commit the
//! review was anchored to before ([`reanchor_comments`]; not signed, and
//! the replaced entry stays in git). It is committed after `HEAD`, so the
//! engine's "added after the certified commit" rule holds unchanged.
//!
//! ```text
//!   prepare_reanchor_module(root, "crates/x/src/m.rs", by)
//!        │  every review by `by` in that file whose certified commit is
//!        │  not in HEAD's history: ready, or refused with the reason
//!        v
//!   sign_reanchor_module(root, &module, &key)   one unlocked key, one
//!        │  sign-off for the module; every draft is checked again, all
//!        │  are signed, then all are written
//!        v
//!   RelocateReport { done, skipped, written }
//! ```
//!
//! # When a function is refused (it needs a full review, or a re-index)
//!
//! - its code, its doc comment, a callee's hash or `Cargo.lock` differs
//!   from what the previous review recorded;
//! - the previous review's signature does not verify against
//!   `kovan_root.toml` (a re-anchor carries a sound review over; it never
//!   repairs one);
//! - its `kovan.toml` entry is out of date, or its file has uncommitted
//!   changes;
//! - the previous answers no longer pass the review wizard.
//!
//! **Since 2026-10-22 (#838, #1018)** a review whose code changed is not
//! offered here at all: with its anchor gone and its code changed, the
//! engine shows it as directly stale. And a review whose code is unchanged
//! usually needs no re-anchor either: `kovan-cli review carry-forward`
//! writes an unsigned, verifiable record that moves its key to the squash
//! or rebased commit ([`super::carry_forward`]), with no new signature. A
//! re-anchor is still the way to put a fresh signature on `HEAD`.
//!
//! The whole module is refused in a **shallow clone**: there the commit is
//! missing because history was not fetched, and the remedy is a full fetch,
//! never a new signature.
//!
//! Bottom-up is not asked for again: nothing a re-anchored review covers
//! changed (callee hashes are compared), and after a squash every callee's
//! stamp is in the same state, so the rule would block the whole batch.
//!
//! The rung is derived again at `HEAD`, as for any stamp. It can differ
//! from the previous one when the squash changed what git says about the
//! reaching tests; the comment line then names both.
//!
//! Plain functions, no egui; they read git: call them off the UI thread.
//! Nothing here signs without a key unlocked by its human owner.

use std::collections::BTreeMap;
use std::path::Path;

use kovan_common::review::engine::{StampState, UnverifiedReason};
use kovan_common::review::review_md::ReviewEntry;
use kovan_common::review::signing::keystore::UnlockedKey;
use kovan_common::review::signing::registry::Registry;
use kovan_common::review::signing::verify_review;
use kovan_common::review::wizard::ReviewWizard;

use super::relocate::RelocateReport;
use super::review_mode::section::{function_sections, SectionKind};
use super::{
    applicability_for, call_graph_id, cargo_lock_hash, draft_stamp_loaded, evaluate_loaded, git,
    join, load_workspace, write_review, StampRequest, Workspace, REVIEW_MD,
};

/// One review that can be re-anchored.
#[derive(Debug, Clone, PartialEq)]
pub struct ReanchorItem {
    pub fn_id: String,
    pub call_graph_id: String,
    /// `file.rs::qual`.
    pub path: String,
    /// The function's hash now (equal to the reviewed one).
    pub hash: String,
    /// The review being re-anchored.
    pub previous: ReviewEntry,
    /// Its comments as written.
    pub previous_comments: String,
}

/// What a module's re-anchor shows and signs.
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleReanchor {
    /// The re-anchoring reviewer (their own reviews only).
    pub by: String,
    /// The module: a source file, workspace-relative.
    pub module: String,
    /// The commit the reviews will certify (`HEAD` when prepared).
    pub head: String,
    pub ready: Vec<ReanchorItem>,
    /// `file.rs::qual` of each review that cannot be re-anchored, with why
    /// (module doc).
    pub refused: Vec<(String, String)>,
}

/// The first 10 characters of a commit id.
fn short(commit: &str) -> String {
    commit.chars().take(10).collect()
}

/// Whether the clone is shallow (`git rev-parse --is-shallow-repository`).
fn is_shallow(root: &Path) -> bool {
    git::git(root, &["rev-parse", "--is-shallow-repository"]).is_ok_and(|o| o.trim() == "true")
}

/// The modules (source files, workspace-relative) holding a review by `by`
/// whose certified commit is not in `HEAD`'s history, with how many. Runs
/// the engine.
pub fn reanchor_modules(root: &Path, by: &str) -> Result<BTreeMap<String, usize>, String> {
    let ws = load_workspace(root)?;
    let we = evaluate_loaded(root, &ws);
    let mut out = BTreeMap::new();
    for idx in &ws.indexes {
        for (file, f) in idx.functions() {
            if lost_anchor(&we.evaluation.functions.get(&f.id), by) {
                *out.entry(idx.file_path(file)).or_insert(0) += 1;
            }
        }
    }
    Ok(out)
}

/// Whether `by`'s review in `report` lost its anchor commit.
fn lost_anchor(
    report: &Option<&kovan_common::review::engine::FunctionReport>,
    by: &str,
) -> bool {
    report.is_some_and(|r| {
        r.reviews.iter().any(|r| {
            r.by == by
                && r.state == StampState::Unverified(UnverifiedReason::CertifiedCommitNotInHistory)
        })
    })
}

/// Why `previous` cannot be carried over to the function as it is now
/// (module doc), or `None` when it can.
fn refusal(
    root: &Path,
    ws: &Workspace,
    registry: &Registry,
    lock: Option<&str>,
    hashes: &BTreeMap<String, String>,
    function: &str,
    previous: &ReviewEntry,
) -> Option<String> {
    let (idx, _, f) = match super::locate(root, ws, function) {
        Ok(found) => found,
        Err(e) => return Some(e),
    };
    let b = &previous.review;
    if f.index_out_of_date {
        return Some("its kovan.toml entry is out of date: run Index fresh first".into());
    }
    if !verify_review(previous, registry).is_verified() {
        return Some(
            "the previous review's signature does not verify against kovan_root.toml: review \
             it with Stamp"
                .into(),
        );
    }
    if f.hash != b.hash {
        return Some("its code changed since the review: review it with Stamp".into());
    }
    if f.doc_hash != b.doc_hash {
        return Some("its doc comment changed since the review: review it with Stamp".into());
    }
    // Callees, and the types and constants the review recorded (#843; a
    // review written before items were indexed records none).
    let now: BTreeMap<String, String> = f
        .depends_on()
        .filter(|c| f.callees.contains(*c) || b.callees.contains_key(*c))
        .filter_map(|c| hashes.get(c).map(|h| (c.clone(), h.clone())))
        .collect();
    if now != b.callees {
        return Some("a callee changed since the review: review it with Stamp".into());
    }
    if b.cargo_lock.as_deref() != lock {
        return Some("Cargo.lock changed since the review: review it with Stamp".into());
    }
    let (mut applicability, _) = applicability_for(root, ws, idx, f);
    applicability.physical_interface |= b.checklist.contains_key("units_documented");
    let gate = ReviewWizard::embedded().stamp_gate(&b.checklist, applicability);
    if !gate.stampable() {
        return Some(format!(
            "the previous answers no longer pass the review wizard ({} blocking): review it \
             with Stamp",
            gate.blocked_by.len()
        ));
    }
    None
}

/// Gather the reviews by `by` in `module` (a source file, workspace-relative)
/// whose certified commit is not in `HEAD`'s history, sorted into ready and
/// refused (module doc). Refused as a whole in a shallow clone, with no
/// commit, or when the module has no such review. Reads git and runs the
/// engine.
pub fn prepare_reanchor_module(
    root: &Path,
    module: &str,
    by: &str,
) -> Result<ModuleReanchor, String> {
    if is_shallow(root) {
        return Err(
            "this clone is shallow, so the commits the reviews certify were never fetched: \
             fetch the full history (git fetch --unshallow) instead of re-anchoring"
                .into(),
        );
    }
    let ws = load_workspace(root)?;
    if ws.head.is_empty() {
        return Err("no commit: a stamp certifies a commit".into());
    }
    let we = evaluate_loaded(root, &ws);
    let registry = Registry::build(&ws.review_root);
    let lock = cargo_lock_hash(root);
    let hashes = ws.hashes();
    let (mut ready, mut refused) = (Vec::new(), Vec::new());
    for idx in &ws.indexes {
        for (file, f) in idx.functions() {
            if idx.file_path(file) != module
                || !lost_anchor(&we.evaluation.functions.get(&f.id), by)
            {
                continue;
            }
            let path = format!("{module}::{}", f.qual);
            let folder = ws.reviews.get(&idx.dir);
            let Some(previous) = folder.and_then(|m| {
                m.doc
                    .reviews()
                    .find(|r| r.function_id() == f.id && r.review.by == by)
                    .cloned()
            }) else {
                refused.push((path, format!("{by}'s review is not in its review.md")));
                continue;
            };
            match refusal(root, &ws, &registry, lock.as_deref(), &hashes, &f.id, &previous) {
                Some(why) => refused.push((path, why)),
                None => ready.push(ReanchorItem {
                    fn_id: f.id.clone(),
                    call_graph_id: call_graph_id(idx, file, f),
                    path,
                    hash: f.hash.clone(),
                    previous_comments: folder
                        .map(|m| function_sections(&m.text, &f.id))
                        .unwrap_or_default()
                        .into_iter()
                        .find(|s| s.kind == SectionKind::Review && s.by == by)
                        .map(|s| s.comments)
                        .unwrap_or_default(),
                    previous,
                }),
            }
        }
    }
    if ready.is_empty() && refused.is_empty() {
        return Err(format!(
            "{by} has no review in {module} whose certified commit is missing from this \
             branch's history: nothing to re-anchor"
        ));
    }
    Ok(ModuleReanchor {
        by: by.to_string(),
        module: module.to_string(),
        head: ws.head,
        ready,
        refused,
    })
}

/// The stamp request a re-anchor drafts: the previous review's answers,
/// authorship, `no_concept`, relations and attestation, unchanged.
pub fn reanchor_request(item: &ReanchorItem, by: &str) -> StampRequest {
    let b = &item.previous.review;
    StampRequest {
        function: item.fn_id.clone(),
        by: by.to_string(),
        checklist: b.checklist.clone(),
        authorship: b.authorship.clone(),
        no_concept: b.no_concept.clone(),
        relations: item.previous.relations.clone(),
        separation_attestation: b.separation_attestation.clone(),
    }
}

/// The comments written under a re-anchored review: the previous comments,
/// then the re-anchor note (module doc). Not signed.
pub fn reanchor_comments(item: &ReanchorItem, date: &str, head: &str, rung: u8) -> String {
    let b = &item.previous.review;
    let rung_note = if rung == b.rung {
        String::new()
    } else {
        format!(" The rung derived at the new commit is {rung}; it was {}.", b.rung)
    };
    let note = format!(
        "Re-anchored on {date} to commit {} without the wizard: commit {}, which the review of \
         {} certified, is no longer in the branch's history (a squash or rebase). The code, \
         doc comment, callees and Cargo.lock hash as reviewed; the checklist is carried over \
         unchanged.{rung_note}",
        short(head),
        short(&b.commit),
        b.date,
    );
    let prev = item.previous_comments.trim();
    if prev.is_empty() {
        note
    } else {
        format!("{prev}\n\n{note}")
    }
}

/// Draft every ready review of `m` at `HEAD`, sign each with `key` (unlocked
/// by its human owner) and write them, replacing the previous reviews. A
/// function that changed since [`prepare_reanchor_module`] is skipped with
/// the reason; nothing is written unless every other draft was signed.
/// Refused as a whole when `HEAD` moved since the module was prepared.
pub fn sign_reanchor_module(
    root: &Path,
    m: &ModuleReanchor,
    key: &UnlockedKey,
) -> Result<RelocateReport, String> {
    let ws = load_workspace(root)?;
    if ws.head != m.head {
        return Err(format!(
            "HEAD moved from {} to {} while you were re-anchoring {}: prepare it again",
            short(&m.head),
            short(&ws.head),
            m.module
        ));
    }
    let mut report = RelocateReport::default();
    let mut signed = Vec::new();
    for item in &m.ready {
        let mut d = match draft_stamp_loaded(root, &ws, &reanchor_request(item, &m.by)) {
            Ok(d) => d,
            Err(e) => {
                report.skipped.push((item.path.clone(), e));
                continue;
            }
        };
        let (new, old) = (&d.entry.review, &item.previous.review);
        if new.hash != item.hash
            || new.doc_hash != old.doc_hash
            || new.callees != old.callees
            || new.cargo_lock != old.cargo_lock
            || new.checklist != old.checklist
        {
            report.skipped.push((
                item.path.clone(),
                "it changed while you were re-anchoring it (a pull, a checkout or a quick fix)"
                    .into(),
            ));
            continue;
        }
        key.sign_review(&mut d.entry).map_err(|e| e.to_string())?;
        signed.push((item, d));
    }
    for (item, d) in signed {
        let b = &d.entry.review;
        let comments = reanchor_comments(item, &b.date, &m.head, b.rung);
        write_review(root, &d.entry, &comments)?;
        report.done.push(item.path.clone());
        report.written.insert(join(
            item.path
                .rsplit_once('/')
                .map_or("", |(dir, _)| dir),
            REVIEW_MD,
        ));
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use kovan_common::review::review_md::parse_review_md;
    use kovan_common::review::state::StateKind;

    use super::super::tests::{clean, founder_key, Repo, BY, DIR, LIB};
    use super::*;

    /// Squash every commit after the fixture's first into one, as a squash
    /// merge does: the old commits stay in the object store but are no
    /// longer ancestors of `HEAD`.
    fn squash(r: &Repo) {
        let first = r.git(&["rev-list", "--max-parents=0", "HEAD"]);
        r.git(&["reset", "-q", "--soft", first.trim()]);
        r.git(&["commit", "-q", "-m", "squashed"]);
    }

    /// Methodology: on the stamping fixture (a throwaway git repository;
    /// `twice` calls `leaf`), register a founder key and stamp `leaf`,
    /// `twice` and `other`, each in its own commit: all valid. Squash the
    /// history into one commit, so the commits the three stamps certify
    /// leave `HEAD`'s history. Then edit `other`'s body, commit and
    /// re-index. Prepare the module's re-anchor, sign it with the one key,
    /// commit, and judge again with signatures enforced.
    ///
    /// Pass: after the squash all three are unverified with the
    /// not-in-history reason (never `NotAfterCertifiedCommit`); the module
    /// lists `leaf` and `twice` as ready; ~~and refuses `other` (its code
    /// changed)~~ **CORRECTED 2026-10-22** (#838, #1018): `other`, whose
    /// anchor is gone and whose code changed, is now directly stale (no
    /// carry-forward can exist for it), so it is not offered for re-anchor
    /// at all; a reviewer with no review there, and a module with nothing
    /// to re-anchor, are refused; after signing, `leaf` and `twice` are
    /// valid at rung 3 again with their checklist and comments carried
    /// over and each review replaced (not appended), `other` stays
    /// ~~unverified~~ directly stale, and a second prepare finds ~~only
    /// `other`~~ nothing to re-anchor.
    ///
    /// Result (2026-10-12): passes. Re-run 2026-10-22 with the corrected
    /// expectations: passes.
    #[test]
    fn a_squashed_module_is_reanchored_in_one_sign_off() {
        let r = Repo::new();
        let store = tempfile::tempdir().unwrap();
        let key = founder_key(&r, store.path());
        r.stamp(&key, "leaf", "");
        r.stamp(&key, "twice", "Checked against the doc.");
        r.stamp(&key, "other", "");
        for q in ["leaf", "twice", "other"] {
            assert_eq!(r.state(q).unwrap().state, Some(StateKind::Valid), "{q}");
        }
        let old_commit = {
            let doc = parse_review_md(&r.read(&format!("{DIR}/review.md")));
            let e = doc.reviews().find(|e| e.path().unwrap().ends_with("::twice")).unwrap();
            e.review.commit.clone()
        };

        squash(&r);
        for q in ["leaf", "twice", "other"] {
            let s = r.state(q).unwrap();
            assert_eq!(s.state, Some(StateKind::Unverified), "{q}");
            assert!(s.reason.contains("not in this branch's history"), "{q}: {}", s.reason);
        }
        assert_eq!(
            reanchor_modules(r.path(), BY).unwrap(),
            BTreeMap::from([(LIB.to_string(), 3)])
        );

        r.edit("    1\n", "    2\n");
        r.commit("edit other");
        r.reindex();

        let e = prepare_reanchor_module(r.path(), LIB, "github:nobody").unwrap_err();
        assert!(e.contains("nothing to re-anchor"), "{e}");
        let e = prepare_reanchor_module(r.path(), "crates/demo/src/absent.rs", BY).unwrap_err();
        assert!(e.contains("nothing to re-anchor"), "{e}");

        let m = prepare_reanchor_module(r.path(), LIB, BY).unwrap();
        let ready: Vec<&str> = m.ready.iter().map(|i| i.path.as_str()).collect();
        assert_eq!(ready, [format!("{LIB}::leaf"), format!("{LIB}::twice")]);
        assert!(m.refused.is_empty(), "{:?}", m.refused);
        assert_eq!(r.state("other").unwrap().state, Some(StateKind::DirectlyStale));

        let rep = sign_reanchor_module(r.path(), &m, &key).unwrap();
        assert_eq!(rep.done, [format!("{LIB}::leaf"), format!("{LIB}::twice")]);
        assert!(rep.skipped.is_empty(), "{:?}", rep.skipped);
        assert_eq!(
            rep.written.iter().collect::<Vec<_>>(),
            [&format!("{DIR}/review.md")]
        );
        r.commit("re-anchor lib.rs");

        let md = r.read(&format!("{DIR}/review.md"));
        let doc = parse_review_md(&md);
        let twices: Vec<_> = doc
            .reviews()
            .filter(|e| e.path().unwrap().ends_with("::twice"))
            .collect();
        assert_eq!(twices.len(), 1, "replaced, not appended");
        assert_eq!(twices[0].review.checklist, clean());
        assert_eq!(twices[0].review.commit, m.head);
        assert!(md.contains("Checked against the doc."), "{md}");
        assert!(
            md.contains("Re-anchored on") && md.contains(&short(&old_commit)),
            "{md}"
        );
        for q in ["leaf", "twice"] {
            let s = r.state(q).unwrap();
            assert_eq!((s.state, s.rung), (Some(StateKind::Valid), 3), "{q}: {}", s.reason);
        }
        assert_eq!(r.state("other").unwrap().state, Some(StateKind::DirectlyStale));
        let again = prepare_reanchor_module(r.path(), LIB, BY).unwrap_err();
        assert!(again.contains("nothing to re-anchor"), "{again}");
    }

    /// Methodology: stamp `leaf` on the fixture, then clone the repository
    /// with `--depth 1`, as a CI checkout does. The clone holds the stamp
    /// but not the commit it certifies.
    ///
    /// Pass: in the clone the stamp is unverified with the not-in-history
    /// reason, and a re-anchor is refused naming the shallow clone (the
    /// remedy is a full fetch, never a new signature). `HEAD` moving after
    /// a prepare is refused too.
    ///
    /// Result (2026-10-12): passes.
    #[test]
    fn a_shallow_clone_is_never_reanchored() {
        let r = Repo::new();
        let store = tempfile::tempdir().unwrap();
        let key = founder_key(&r, store.path());
        r.stamp(&key, "leaf", "");
        let clone = tempfile::tempdir().unwrap();
        let url = format!("file://{}", r.path().display());
        r.git(&["clone", "-q", "--depth", "1", &url, clone.path().to_str().unwrap()]);
        let states = super::super::stamp_states(clone.path()).unwrap();
        let s = states.iter().find(|s| s.function.ends_with("::leaf")).unwrap();
        assert_eq!(s.state, Some(StateKind::Unverified));
        assert!(s.reason.contains("not in this branch's history"), "{}", s.reason);
        let e = prepare_reanchor_module(clone.path(), LIB, BY).unwrap_err();
        assert!(e.contains("shallow"), "{e}");

        squash(&r);
        let m = prepare_reanchor_module(r.path(), LIB, BY).unwrap();
        r.write("note.txt", "x\n");
        r.commit("another commit");
        let e = sign_reanchor_module(r.path(), &m, &key).unwrap_err();
        assert!(e.contains("HEAD moved"), "{e}");
    }
}
