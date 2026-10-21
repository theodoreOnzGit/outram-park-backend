//! **Acknowledging moves and recording deletions** from the need-you queue
//! (GitHub #771, #740 U5, #739 D14): the filesystem side of
//! [`kovan_common::review::review_md_relocate`]. Each call judges the
//! workspace afresh, writes the `review.md` files concerned, and returns
//! what it wrote so commit-and-push can include exactly those files.
//!
//! - **A move is acknowledged only when the engine allows it**: the
//!   function's state is moved, its reaching tests allow it
//!   ([`kovan_common::review::engine::TestVerdict::allows_reconfirm`]:
//!   every counting reaching test passed, or none reaches it), and it has
//!   a single candidate place (several identical copies need the
//!   maintainer to say which is the original). Anything else is skipped
//!   with the reason, never forced.
//! - **A cross-folder move moves the entries between `review.md` files**;
//!   the move is recorded at `HEAD`.
//! - **A deletion is recorded** only where the engine places its history
//!   row in the folder's own `review.md` (the folder still exists). A
//!   deleted folder's or crate's history goes to the crate's `kovan.toml`
//!   or `kovan_root.toml` (#739 D6), which this does not write yet: those
//!   rows are skipped with that reason.
//!
//! Every function reads the files fresh before each write, so a batch
//! whose functions share a `review.md` sees its own earlier writes.

use std::collections::BTreeSet;
use std::path::Path;

use kovan_common::review::engine::{HistoryPlacement, StampState};
use kovan_common::review::review_md::Entry;
use kovan_common::review::review_md_relocate::{acknowledge_move, record_deletion};
use kovan_common::review::signed_at::now_local;

use super::{evaluate_loaded, join, load_workspace, Workspace, REVIEW_MD};

/// What a batch did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RelocateReport {
    /// The functions done.
    pub done: Vec<String>,
    /// The functions skipped, each with why.
    pub skipped: Vec<(String, String)>,
    /// The files written, workspace-relative.
    pub written: BTreeSet<String>,
}

/// The folder whose `review.md` holds a readable entry of `id`.
fn folder_of(ws: &Workspace, id: &str) -> Option<String> {
    ws.reviews.iter().find_map(|(dir, m)| {
        m.doc
            .entries
            .iter()
            .any(|e| match &e.entry {
                Entry::Review(r) => r.function_id() == id,
                Entry::NeedsFix(n) => n.function_id() == id,
                Entry::Annotation(a) => a.function_id() == id,
                _ => false,
            })
            .then(|| dir.clone())
    })
}

fn parent(file: &str) -> &str {
    file.rsplit_once('/').map_or("", |(d, _)| d)
}

fn read(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap_or_default()
}

fn write(root: &Path, rel: &str, text: &str) -> Result<(), String> {
    let p = root.join(rel);
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    std::fs::write(&p, text).map_err(|e| format!("{}: {e}", p.display()))
}

/// Acknowledge the moves of `ids` (module doc). `Err` only when the
/// workspace cannot be read; a function that cannot be acknowledged is in
/// `skipped`.
pub fn acknowledge_moves(root: &Path, ids: &[String]) -> Result<RelocateReport, String> {
    let ws = load_workspace(root)?;
    if ws.head.is_empty() {
        return Err("no commit: a move is acknowledged at a commit".into());
    }
    let we = evaluate_loaded(root, &ws);
    let mut rep = RelocateReport::default();
    for id in ids {
        let skip = |rep: &mut RelocateReport, why: &str| {
            rep.skipped.push((id.clone(), why.to_string()));
        };
        let Some(fr) = we.evaluation.functions.get(id) else {
            skip(&mut rep, "not a current function");
            continue;
        };
        let moved = fr.reviews.iter().find_map(|r| match &r.state {
            StampState::Moved {
                to,
                tests,
                candidates,
                ..
            } => Some((to.clone(), tests.clone(), candidates.clone())),
            _ => None,
        });
        let Some((to, tests, candidates)) = moved else {
            skip(&mut rep, "no review of it is waiting as moved");
            continue;
        };
        if !candidates.is_empty() {
            skip(
                &mut rep,
                "identical code at several places: say which is the original",
            );
            continue;
        }
        if !tests.allows_reconfirm() {
            skip(
                &mut rep,
                &format!("its regression tests have not passed: {tests:?}"),
            );
            continue;
        }
        // The review's own id: the current function's, or the one the
        // engine matched back to it by hash (the index minted a new id at
        // the new place).
        let review_id = we
            .evaluation
            .id_matches
            .iter()
            .find(|m| m.current_id == *id)
            .map_or(id.as_str(), |m| m.review_function.as_str());
        let Some(from_dir) = folder_of(&ws, review_id) else {
            skip(&mut rep, "no review.md entry names it");
            continue;
        };
        let to_dir = parent(&to.file).to_string();
        let to_path = format!("{}::{}", to.file, to.qual);
        let from_md = join(&from_dir, REVIEW_MD);
        let to_md = join(&to_dir, REVIEW_MD);
        let cross = from_dir != to_dir;
        let to_text = cross.then(|| read(root, &to_md));
        match acknowledge_move(
            &read(root, &from_md),
            to_text.as_deref(),
            review_id,
            &to_path,
            &ws.head,
        ) {
            Ok(r) => {
                write(root, &from_md, &r.from_text)?;
                rep.written.insert(from_md.clone());
                if let Some(t) = r.to_text {
                    write(root, &to_md, &t)?;
                    rep.written.insert(to_md);
                }
                rep.done.push(id.clone());
            }
            Err(e) => skip(&mut rep, &e.to_string()),
        }
    }
    Ok(rep)
}

/// Record the deletions of `ids` (module doc).
pub fn record_deletions(root: &Path, ids: &[String]) -> Result<RelocateReport, String> {
    let ws = load_workspace(root)?;
    let we = evaluate_loaded(root, &ws);
    let now = now_local();
    let mut rep = RelocateReport::default();
    for id in ids {
        let row = we.evaluation.history.iter().find(|h| h.row.function == *id);
        let Some(h) = row else {
            rep.skipped.push((
                id.clone(),
                "the engine has no history row for it (not deleted)".into(),
            ));
            continue;
        };
        let HistoryPlacement::FolderReviewMd { dir } = &h.placement else {
            rep.skipped.push((
                id.clone(),
                "its folder or crate is gone: the history goes to kovan.toml or \
                 kovan_root.toml, which kovan does not write yet"
                    .into(),
            ));
            continue;
        };
        let holder = folder_of(&ws, id);
        if holder.as_deref() != Some(dir.as_str()) {
            rep.skipped.push((
                id.clone(),
                format!("its entries are not in {dir}/review.md"),
            ));
            continue;
        }
        let md = join(dir, REVIEW_MD);
        match record_deletion(&read(root, &md), id, &h.row, &now) {
            Ok(text) => {
                write(root, &md, &text)?;
                rep.written.insert(md);
                rep.done.push(id.clone());
            }
            Err(e) => rep.skipped.push((id.clone(), e.to_string())),
        }
    }
    Ok(rep)
}
