//! A **deleted folder's** review history for the crate root `kovan.toml`
//! (#739 D6: "Whole folder deleted: its history goes into the crate's
//! `kovan.toml` ... rebuildable from git, `git log --diff-filter=D` on
//! `review.md` files").
//!
//! The caller lists the `review.md` files git deleted under the crate (the
//! newest deletion of each path, skipping folders that have a `review.md`
//! again) and reads each one's last content (`git show <commit>^:<path>`);
//! [`deleted_folder`] turns that text into the history row. Pure.

use std::collections::BTreeMap;

use crate::review::index::DeletedFolder;
use crate::review::review_md::{parse_review_md, DeletedFunction, Entry};

/// The history of folder `dir`, whose `review.md` was deleted in
/// `deleted_commit` and last read `old_review_md`: one row per reviewed
/// function (its last review's commit and every reviewer), plus the rows
/// its own `deleted_functions` table already held.
pub fn deleted_folder(dir: &str, deleted_commit: &str, old_review_md: &str) -> DeletedFolder {
    let doc = parse_review_md(old_review_md);
    // function -> (path, last review commit by date, reviewers)
    let mut rows: BTreeMap<String, (String, (String, String), Vec<String>)> = BTreeMap::new();
    let mut functions = Vec::new();
    for e in &doc.entries {
        match &e.entry {
            Entry::Review(r) => {
                let id = r.function_id();
                let path = r.path().unwrap_or_default();
                let when = (r.review.date.clone(), r.review.commit.clone());
                let row = rows
                    .entry(id)
                    .or_insert_with(|| (path.clone(), when.clone(), Vec::new()));
                if when > row.1 {
                    row.1 = when;
                    row.0 = path;
                }
                row.2.push(r.review.by.clone());
            }
            Entry::DeletedFunctions(d) => functions.extend(d.deleted.iter().cloned()),
            _ => {}
        }
    }
    for (function, (path, (_, commit), mut reviewers)) in rows {
        reviewers.sort();
        reviewers.dedup();
        functions.push(DeletedFunction {
            function,
            path,
            deleted_commit: Some(deleted_commit.to_string()),
            branch: None,
            last_review_commit: commit,
            reviewers,
        });
    }
    functions.sort();
    DeletedFolder {
        dir: dir.to_string(),
        deleted_commit: Some(deleted_commit.to_string()),
        functions,
    }
}

/// From `git log --diff-filter=D --name-only --format=%x00%H` output (a
/// `\0` then the commit on its own line, then the deleted paths; newest
/// first): the newest deletion commit of each `review.md` under `crate_dir`,
/// as (folder, commit), skipping folders in `still_reviewed`.
pub fn deleted_review_mds(
    log: &str,
    crate_dir: &str,
    still_reviewed: &std::collections::BTreeSet<String>,
) -> Vec<(String, String)> {
    let mut out: BTreeMap<String, String> = BTreeMap::new();
    let mut commit = String::new();
    for line in log.lines() {
        if let Some(c) = line.strip_prefix('\0') {
            commit = c.trim().to_string();
            continue;
        }
        let p = line.trim();
        let Some(dir) = p.strip_suffix("/review.md") else { continue };
        let inside = crate_dir.is_empty() || dir == crate_dir || dir.starts_with(&format!("{crate_dir}/"));
        if inside && !still_reviewed.contains(dir) && !commit.is_empty() {
            out.entry(dir.to_string()).or_insert_with(|| commit.clone());
        }
    }
    out.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const C1: &str = "1111111111111111111111111111111111111111";
    const C2: &str = "2222222222222222222222222222222222222222";

    fn review(by: &str, date: &str, commit: &str, path: &str) -> String {
        format!(
            "# Review {by}\n\n```toml\n[kovan]\nid = \"review-{by}-{date}\"\nkind = \"review\"\ncreated = \"{date}\"\nmodified = \"{date}\"\ntarget = \"fn:00000000000000ab\"\n\n[review]\npath = \"{path}\"\nby = \"github:{by}\"\nrung = 3\ndate = \"{date}\"\ncommit = \"{commit}\"\nhash = \"sha256:{}\"\ndoc_hash = \"sha256:{}\"\n```\n\n",
            "a".repeat(64),
            "b".repeat(64)
        )
    }

    /// Methodology: a deleted folder's last `review.md` holding two reviews
    /// of one function (different reviewers and dates) becomes one history
    /// row with the later review's commit and both reviewers; the git log
    /// parser keeps the newest deletion per folder, only inside the crate,
    /// and skips a folder that has a `review.md` again.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn a_deleted_review_md_becomes_one_history_row_per_function() {
        let md = format!(
            "{}{}",
            review("alice", "2026-10-01", C1, "crates/x/src/old/a.rs::f"),
            review("bob", "2026-10-05", C2, "crates/x/src/old/a.rs::f")
        );
        let d = deleted_folder("crates/x/src/old", C2, &md);
        assert_eq!(d.functions.len(), 1);
        let f = &d.functions[0];
        assert_eq!(f.function, "fn:00000000000000ab");
        assert_eq!(f.last_review_commit, C2);
        assert_eq!(f.reviewers, vec!["github:alice", "github:bob"]);
        assert_eq!(f.path, "crates/x/src/old/a.rs::f");

        let log = format!(
            "\0{C2}\n\ncrates/x/src/old/review.md\ncrates/y/src/review.md\n\0{C1}\n\ncrates/x/src/old/review.md\ncrates/x/src/back/review.md\n"
        );
        let again: std::collections::BTreeSet<String> = ["crates/x/src/back".to_string()].into();
        assert_eq!(
            deleted_review_mds(&log, "crates/x", &again),
            vec![("crates/x/src/old".to_string(), C2.to_string())]
        );
    }
}
