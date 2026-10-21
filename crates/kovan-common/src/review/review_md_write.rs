//! **Writing one entry into `review.md`** without disturbing anything else
//! (GitHub #770, #762): the stamp dialog's "save". Pure text in, text out.
//!
//! [`super::review_md::render_review_md`] re-renders a whole file and drops
//! unreadable entries (`review.md` is never auto-fixed), so it cannot be
//! used to save into a file that holds one. These functions **splice**:
//! the new entry's block replaces exactly the line span of the entry it
//! supersedes ([`crate::artifact::heading_span`]) or is appended at the end;
//! every other byte, readable or not, is kept as it was.
//!
//! # Which entry a new one replaces
//!
//! - **A review** replaces this reviewer's standing review of the same
//!   function (same `fn:` target, same `by`): "at most one standing review
//!   per reviewer per function" (#764 data model, 2026-10-07; two make both
//!   unreadable, [`super::review_md::parse_review_md`]), and a re-review
//!   replaces, the old one living on in git history (maintainer,
//!   2026-10-06, first for the since-removed `review/stamps.toml`). Another reviewer's review of the
//!   same function is kept: many maintainers may each stand behind it.
//! - **A needs-fix** replaces the entry with the same `[kovan] id` (to
//!   resolve it) and is otherwise appended: one reviewer may raise several.
//!
//! An **unreadable** entry is never replaced (it counts as no review, so a
//! new stamp is simply added beside it: "just redo", #764).
//!
//! # Read-back check
//!
//! The result is parsed again before it is returned: the written entry must
//! read back as a readable entry equal to the one given, and the file must
//! have exactly as many unreadable entries as before. Otherwise nothing is
//! returned ([`WriteError::NotReadBack`]): for example when the file already
//! holds two standing reviews by this reviewer, a third would be demoted
//! with them, and the maintainer resolves that by hand.

use crate::artifact::{heading_span, render_block, ARTIFACT_LEVEL};

use super::review_md::{
    parse_review_md, sign_off, Entry, NeedsFixEntry, ReviewDocument, ReviewEntry, ReviewMdError,
};

/// Why an entry could not be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteError {
    /// TOML serialisation failed.
    Emit(ReviewMdError),
    /// The comments hold a `# ` heading line, which would end the entry.
    HeadingInComments(String),
    /// The spliced file does not read back as intended (module doc); the
    /// message says what the parser reported.
    NotReadBack(String),
}

impl std::fmt::Display for WriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Emit(e) => write!(f, "{e}"),
            Self::HeadingInComments(l) => {
                write!(f, "comment line {l:?} is a level-1 heading and would end the entry; use ## or deeper")
            }
            Self::NotReadBack(m) => write!(
                f,
                "review.md would not read back with the new entry: {m}; nothing written"
            ),
        }
    }
}

impl std::error::Error for WriteError {}

/// The new file text, and whether an existing entry was replaced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    pub text: String,
    pub replaced: bool,
}

/// Byte offset of the start of every line of `md`, plus `md.len()`.
fn line_starts(md: &str) -> Vec<usize> {
    let mut v = vec![0];
    let mut at = 0;
    for l in md.split_inclusive('\n') {
        at += l.len();
        v.push(at);
    }
    v
}

/// `md` with the entry whose heading is on 1-based `line` replaced by
/// `block`, or `block` appended when `line` is `None`.
pub(crate) fn splice(md: &str, line: Option<usize>, block: &str) -> String {
    let Some(line) = line else {
        if md.trim().is_empty() {
            return block.to_string();
        }
        let mut out = md.to_string();
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
        out.push_str(block);
        return out;
    };
    let span = heading_span(md, line, ARTIFACT_LEVEL);
    let starts = line_starts(md);
    let a = starts[span.start.min(starts.len() - 1)];
    let b = starts
        .get(span.end)
        .copied()
        .unwrap_or(md.len())
        .min(md.len());
    let mut out = md[..a].to_string();
    out.push_str(block);
    if b < md.len() {
        out.push('\n');
    }
    out.push_str(&md[b..]);
    out
}

/// The entry body: the reviewer's comments, then the generated sign-off
/// (for a review).
fn checked_comments(comments: &str) -> Result<String, WriteError> {
    if let Some(l) = comments
        .lines()
        .find(|l| l.starts_with("# ") || l.trim() == "#")
    {
        return Err(WriteError::HeadingInComments(l.to_string()));
    }
    Ok(comments.trim().to_string())
}

/// What went wrong reading `heading` back, for the error message.
fn readback_problem(doc: &ReviewDocument, heading: &str) -> String {
    doc.unreadable
        .iter()
        .find(|u| u.heading == heading)
        .map(|u| u.message.clone())
        .unwrap_or_else(|| "the entry is missing or differs after parsing".into())
}

/// Write `entry` into `md` (module doc): replace this reviewer's standing
/// review of the same function, else append. `comments` is the Markdown
/// under the heading (may be empty); the sign-off line is generated.
pub fn upsert_review(md: &str, entry: &ReviewEntry, comments: &str) -> Result<Written, WriteError> {
    let comments = checked_comments(comments)?;
    let before = parse_review_md(md);
    let fid = entry.function_id();
    let line = before.entries.iter().find_map(|e| match &e.entry {
        Entry::Review(r) if r.function_id() == fid && r.review.by == entry.review.by => {
            Some(e.line)
        }
        _ => None,
    });
    let heading = super::draft::review_heading(
        entry
            .path()
            .as_deref()
            .and_then(|p| p.split_once(".rs::").map(|(_, q)| q))
            .unwrap_or(&fid),
        &entry.review.by,
    );
    let toml = Entry::Review(entry.clone())
        .to_toml()
        .map_err(WriteError::Emit)?;
    let body = if comments.is_empty() {
        sign_off(&entry.review)
    } else {
        format!("{comments}\n\n{}", sign_off(&entry.review))
    };
    let text = splice(
        md,
        line,
        &render_block(ARTIFACT_LEVEL, &heading, &toml, &body),
    );
    let after = parse_review_md(&text);
    let back = after.reviews().any(|r| r == entry);
    if !back || after.unreadable.len() != before.unreadable.len() {
        return Err(WriteError::NotReadBack(readback_problem(&after, &heading)));
    }
    Ok(Written {
        text,
        replaced: line.is_some(),
    })
}

/// Write a needs-fix `entry` into `md` (module doc): replace the entry with
/// the same `[kovan] id`, else append.
pub fn upsert_needs_fix(
    md: &str,
    entry: &NeedsFixEntry,
    comments: &str,
) -> Result<Written, WriteError> {
    let comments = checked_comments(comments)?;
    let before = parse_review_md(md);
    let line = before.entries.iter().find_map(|e| match &e.entry {
        Entry::NeedsFix(n) if n.kovan.id == entry.kovan.id => Some(e.line),
        _ => None,
    });
    let qual = entry
        .path()
        .and_then(|p| p.split_once(".rs::").map(|(_, q)| q.to_string()))
        .unwrap_or_else(|| entry.function_id());
    let heading = super::draft::needs_fix_heading(&qual, &entry.needs_fix.by);
    let toml = Entry::NeedsFix(entry.clone())
        .to_toml()
        .map_err(WriteError::Emit)?;
    let text = splice(
        md,
        line,
        &render_block(ARTIFACT_LEVEL, &heading, &toml, &comments),
    );
    let after = parse_review_md(&text);
    let back = after.needs_fixes().any(|n| n == entry);
    if !back || after.unreadable.len() != before.unreadable.len() {
        return Err(WriteError::NotReadBack(readback_problem(&after, &heading)));
    }
    Ok(Written {
        text,
        replaced: line.is_some(),
    })
}

#[cfg(test)]
#[path = "review_md_write_tests.rs"]
mod tests;
