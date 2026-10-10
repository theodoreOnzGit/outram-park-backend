//! **Re-confirming an inherited-stale review** without the wizard (GitHub
//! #770; #740 decision 8 and the wizard notes; #739 D6).
//!
//! # What the issues decided
//!
//! - #740 decision 8: "**Inherited stale** (a callee changed, no test
//!   vouches): the callee's diff, the same unified style, with this function
//!   read-only alongside. The actions are **re-confirm** or **mark for
//!   re-review**."
//! - #740, wizard continued: "Re-confirming an inherited-stale stamp skips
//!   the wizard."
//! - #739 D6 (corrected 2026-10-07): "passing tests are necessary but
//!   insufficient. The caller always needs a human re-confirm, … and a
//!   failing reaching test blocks the re-confirm until it passes." That is
//!   the engine's `InheritedStale { blocked }`
//!   (`TestVerdict::allows_reconfirm`).
//!
//! # How a re-confirm is recorded: the conservative option
//!
//! Neither #739 nor #740 decides the record's form (checked 2026-10-10: no
//! re-confirm field or entry kind in `review.md`'s schema, no comment
//! naming one). So, as the brief for this work said to when undecided, a
//! re-confirm is **a new signed review by the same reviewer that replaces
//! their previous one, carrying the previous checklist, `no_concept`,
//! relations and separation attestation unchanged**, drafted at `HEAD` with
//! the callees' new hashes (that is what makes it valid again), and with
//! the comments noting the re-confirm ([`reconfirm_comments`]: the previous
//! comments, then one line naming the changed callees and the review it
//! re-confirms). The note is in the comments, which are **not** signed;
//! what is signed is everything a full stamp signs. **A decision for the
//! maintainer:** whether a re-confirm should be a distinct, signed record.
//!
//! # When it is refused
//!
//! Only this reviewer's own review can be re-confirmed, and only when the
//! engine judges it inherited stale **by a callee change** and not blocked;
//! bottom-up still holds (a callee without a valid stamp blocks it); the
//! previous answers must still pass the wizard's gate (a new question that
//! now applies, say), else the reviewer is sent to the full wizard; and the
//! same refusals as a stamp (no commit, uncommitted changes in the file,
//! index out of date). The function's hash is checked again when signing,
//! as for a stamp (#740 decision 7).
//!
//! Plain functions, no egui; they read git: call them off the UI thread.

use std::path::Path;

use kovan_common::review::draft::DraftError;
use kovan_common::review::engine::{InheritedCause, StampState};
use kovan_common::review::review_md::ReviewEntry;
use kovan_common::review::signing::keystore::UnlockedKey;
use kovan_common::review::wizard::ReviewWizard;

use super::review_mode::diff::{function_diff, FnDiff};
use super::review_mode::section::{function_sections, SectionKind};
use super::review_mode::{blockers_of, Snapshot};
use super::{
    applicability_for, call_graph_id, draft_stamp, evaluate_loaded, join, load_workspace, locate,
    write_review, StampRequest, WrittenEntry, REVIEW_MD,
};

/// One callee whose change made the review inherited stale.
#[derive(Debug, Clone, PartialEq)]
pub struct ChangedCallee {
    /// Its `fn:` id.
    pub id: String,
    /// Its name (`qual`).
    pub name: String,
    /// Its unified diff since the review's commit.
    pub diff: Result<FnDiff, String>,
}

/// What the re-confirm step shows and signs.
#[derive(Debug, Clone, PartialEq)]
pub struct ReconfirmContext {
    /// The re-confirming reviewer (their own review only).
    pub by: String,
    pub fn_id: String,
    pub call_graph_id: String,
    /// `file.rs::qual`.
    pub path: String,
    /// The folder's `review.md`, workspace-relative.
    pub review_md: String,
    /// The function's hash now (equal to the reviewed one: inherited stale
    /// means the function itself did not change).
    pub hash: String,
    /// The review being re-confirmed.
    pub previous: ReviewEntry,
    /// Its comments as written.
    pub previous_comments: String,
    /// The callees that changed, with their diffs.
    pub changed: Vec<ChangedCallee>,
}

/// Check that `by` may re-confirm `function` now and gather what the step
/// shows (module doc). Reads git and runs the engine.
pub fn prepare_reconfirm(
    root: &Path,
    function: &str,
    by: &str,
) -> Result<ReconfirmContext, String> {
    let ws = load_workspace(root)?;
    let (idx, file, f) = locate(root, &ws, function)?;
    let path = format!("{}::{}", idx.file_path(file), f.qual);
    if f.index_out_of_date {
        return Err(DraftError::IndexOutOfDate { path }.to_string());
    }
    let (idx, file, f) = (idx.clone(), file.to_string(), f.clone());
    let we = evaluate_loaded(root, &ws);
    let snap = Snapshot { ws, we };
    let mine = snap
        .we
        .evaluation
        .functions
        .get(&f.id)
        .and_then(|r| r.reviews.iter().find(|r| r.by == by))
        .ok_or_else(|| {
            format!("{by} has no review of {path} to re-confirm: review it with Stamp")
        })?;
    let changed = match &mine.state {
        StampState::InheritedStale { blocked: true, .. } => {
            return Err(format!(
                "re-confirm is blocked: a test reaching {path} failed, so it is enabled only \
                 once the reaching tests pass (#739 D6)"
            ))
        }
        StampState::InheritedStale {
            cause: InheritedCause::Callees(c),
            ..
        } => c.clone(),
        other => {
            return Err(format!(
                "re-confirm applies only to a review made stale by a callee change; {by}'s \
                 review of {path} is {}: review it with Stamp",
                other.kind().label()
            ))
        }
    };
    let blocked = blockers_of(&snap, &f.id);
    if !blocked.is_empty() {
        let names: Vec<&str> = blocked.iter().map(|(_, n)| n.as_str()).collect();
        return Err(format!(
            "bottom-up: {path} calls {} without a valid stamp; review {} first",
            names.join(", "),
            if names.len() == 1 { "it" } else { "them" }
        ));
    }
    let folder = snap.ws.reviews.get(&idx.dir);
    let previous = folder
        .and_then(|m| {
            m.doc
                .reviews()
                .find(|r| r.function_id() == f.id && r.review.by == by)
        })
        .cloned()
        .ok_or_else(|| format!("{by}'s review of {path} is not in its review.md"))?;
    let (mut applicability, _) = applicability_for(root, &snap.ws, &idx, &f);
    applicability.physical_interface |= previous.review.checklist.contains_key("units_documented");
    let gate = ReviewWizard::embedded().stamp_gate(&previous.review.checklist, applicability);
    if !gate.stampable() {
        return Err(format!(
            "your previous answers no longer pass the review wizard ({} blocking: a question \
             now applies or an answer is no longer accepted): review it with Stamp",
            gate.blocked_by.len()
        ));
    }
    let changed = changed
        .into_iter()
        .map(|c| {
            let diff = match snap.ws.find(&c) {
                Some((ci, cf, cfn)) => {
                    let cfile = ci.file_path(cf);
                    let cpath = format!("{cfile}::{}", cfn.qual);
                    function_diff(root, &cfile, &cfn.qual, &cpath, &previous.review.commit)
                        .map_err(|e| e.to_string())
                }
                None => Err(format!("{c}: no longer in the index (deleted?)")),
            };
            ChangedCallee {
                name: snap.name(&c),
                id: c,
                diff,
            }
        })
        .collect();
    let previous_comments = folder
        .map(|m| function_sections(&m.text, &f.id))
        .unwrap_or_default()
        .into_iter()
        .find(|s| s.kind == SectionKind::Review && s.by == by)
        .map(|s| s.comments)
        .unwrap_or_default();
    Ok(ReconfirmContext {
        by: by.to_string(),
        call_graph_id: call_graph_id(&idx, &file, &f),
        review_md: join(&idx.dir, REVIEW_MD),
        fn_id: f.id.clone(),
        hash: f.hash.clone(),
        path,
        previous,
        previous_comments,
        changed,
    })
}

/// The stamp request a re-confirm drafts: the previous review's answers,
/// `no_concept`, relations and attestation, unchanged (module doc).
pub fn reconfirm_request(rc: &ReconfirmContext) -> StampRequest {
    let b = &rc.previous.review;
    StampRequest {
        function: rc.fn_id.clone(),
        by: rc.by.clone(),
        checklist: b.checklist.clone(),
        authorship: None,
        no_concept: b.no_concept.clone(),
        relations: rc.previous.relations.clone(),
        separation_attestation: b.separation_attestation.clone(),
    }
}

/// The comments written under the re-confirmed review: the previous
/// comments, then the re-confirm note (module doc). Not signed.
pub fn reconfirm_comments(rc: &ReconfirmContext, date: &str) -> String {
    let names: Vec<&str> = rc.changed.iter().map(|c| c.name.as_str()).collect();
    let note = format!(
        "Re-confirmed on {date} without the wizard (inherited stale: {} changed; their diffs \
         were shown). Re-confirms the review of {} at commit {}; the checklist is carried over \
         unchanged.",
        names.join(", "),
        rc.previous.review.date,
        rc.previous
            .review
            .commit
            .chars()
            .take(10)
            .collect::<String>()
    );
    let prev = rc.previous_comments.trim();
    if prev.is_empty() {
        note
    } else {
        format!("{prev}\n\n{note}")
    }
}

/// Draft the re-confirmation at `HEAD`, sign it with `key` (unlocked by its
/// human owner) and write it, replacing the previous review. Refused when
/// the function changed since [`prepare_reconfirm`] (#740 decision 7) or
/// the draft's checklist is not the previous one.
pub fn sign_reconfirm(
    root: &Path,
    rc: &ReconfirmContext,
    key: &UnlockedKey,
) -> Result<WrittenEntry, String> {
    let mut d = draft_stamp(root, &reconfirm_request(rc))?;
    if d.entry.review.hash != rc.hash {
        return Err(
            "the function changed while you were re-confirming it (a pull, a checkout or a quick \
             fix): the re-confirm is refused"
                .into(),
        );
    }
    if d.entry.review.checklist != rc.previous.review.checklist {
        return Err("the drafted checklist is not the previous one".into());
    }
    key.sign_review(&mut d.entry).map_err(|e| e.to_string())?;
    let comments = reconfirm_comments(rc, &d.entry.review.date);
    write_review(root, &d.entry, &comments)
}
