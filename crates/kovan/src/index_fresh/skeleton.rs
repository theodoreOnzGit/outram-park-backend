//! **`review.md` skeletons** (GitHub #780; the maintainer's "readme.md",
//! confirmed 2026-10-07 to mean `review.md`). A separate, clearly delimited
//! step of the fresh index: each indexed folder with no `review.md` gets a
//! minimal one; an existing one is never written.
//!
//! The skeleton is one entry in the shared artifact format, written by the
//! existing writer ([`render_review_md`]): a `note` entry (`[kovan] kind =
//! "note"`), which review reads as [`Entry::Other`] and ignores, so the
//! file counts as "no review yet" exactly like a missing one. Reviews are
//! added later by the review wizard.
//!
//! An existing `review.md` with unreadable entries is reported for the
//! existing redo flow (an unreadable entry already counts as no review);
//! it is not touched.

use std::path::Path;

use kovan_common::review::review_md::{parse_review_md, render_review_md, Entry, ParsedEntry};

use super::root_file::write_new;

/// The file name.
pub const REVIEW_MD: &str = "review.md";

/// The skeleton `review.md` of workspace-relative folder `dir` (`""` for
/// the root), created at `created` (an ISO 8601 timestamp).
pub fn review_skeleton(dir: &str, created: &str) -> Result<String, String> {
    let shown = if dir.is_empty() {
        "(workspace root)"
    } else {
        dir
    };
    let slug: String = dir
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let id = format!("review-record-{}", slug.trim_matches('-'));
    let toml = format!(
        "[kovan]\nid = \"{}\"\nkind = \"note\"\norigin = \"kovan\"\ncreated = \"{created}\"\nmodified = \"{created}\"\n",
        id.trim_end_matches('-')
    );
    let entry = ParsedEntry {
        heading: format!("Review record: {shown}"),
        line: 1,
        entry: Entry::Other {
            kind: "note".into(),
            toml,
        },
        body:
            "The review record of this folder's code, created empty by kovan's \"Index fresh\".\n\
               Reviews, needs-fix notes and annotations are added by kovan's review wizard.\n\
               This file is human-owned: kovan never overwrites it."
                .into(),
    };
    render_review_md(&[entry]).map_err(|e| e.to_string())
}

fn review_md_of(dir: &str) -> String {
    if dir.is_empty() {
        REVIEW_MD.to_string()
    } else {
        format!("{dir}/{REVIEW_MD}")
    }
}

/// The `review.md` paths (workspace-relative) the step would create:
/// those of `folders` that have none.
pub fn missing_review_mds(root: &Path, folders: &[String]) -> Vec<String> {
    folders
        .iter()
        .map(|d| review_md_of(d))
        .filter(|p| !root.join(p).exists())
        .collect()
}

/// What the skeleton step did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkeletonReport {
    /// Created, workspace-relative.
    pub created: Vec<String>,
    /// Already present, left as they were.
    pub kept: Vec<String>,
    /// Present with unreadable entries: (path, how many). For the redo flow.
    pub unreadable: Vec<(String, usize)>,
}

/// Create a skeleton in each of `folders` that has no `review.md` (module
/// doc). Never overwrites: a file that appears meanwhile is kept.
pub fn create_missing_skeletons(
    root: &Path,
    folders: &[String],
    created: &str,
) -> Result<SkeletonReport, String> {
    let mut report = SkeletonReport::default();
    for dir in folders {
        let rel = review_md_of(dir);
        let path = root.join(&rel);
        if path.exists() {
            if let Ok(text) = std::fs::read_to_string(&path) {
                let n = parse_review_md(&text).unreadable.len();
                if n > 0 {
                    report.unreadable.push((rel.clone(), n));
                }
            }
            report.kept.push(rel);
            continue;
        }
        if !root.join(dir).is_dir() {
            continue;
        }
        write_new(&path, &review_skeleton(dir, created)?)?;
        report.created.push(rel);
    }
    Ok(report)
}
