//! **One function's section of `review.md`** (GitHub #770; #740 U3: "the
//! sidebar edits only the reviewed function's section of `review.md`, the
//! same way artifacts and annotations are edited in kovan literature, using
//! the same schema").
//!
//! ```text
//!  review.md ── parse_review_md ──> entries targeting fn:<id>
//!                                    (review, needs_fix, annotation)
//!                    │ heading_span (the literature artifact rule)
//!                    v
//!   FunctionSection { line range, the editable comments }   ── bands in the sidebar
//!                    │ the reviewer edits the comments only
//!                    v
//!   splice_comments(md, heading line, new text) ──> new review.md text
//!       (TOML block bytes kept, the generated sign-off kept, read-back checked)
//! ```
//!
//! **Only the comments are editable.** The `[kovan]` TOML is machine-owned
//! and signed (#739 decision 6), and the last `## Sign-off` of a review is
//! generated from the stamp (#740 decision 4); both are kept byte for byte.
//! A comment line that is a level-1 heading would end the entry, so it is
//! refused, as `review_md_write` refuses it.
//!
//! **Read-back check (Leak Before Break).** The spliced file is parsed
//! again: every entry must read back equal (only the edited entry's body
//! may differ) and the unreadable count must not change. Otherwise nothing
//! is returned and the reason is shown.

use kovan_common::artifact::{heading_span, ARTIFACT_LEVEL};
use kovan_common::review::review_md::{parse_review_md, sign_off, Entry};

/// What kind of entry a section is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    Review,
    NeedsFix,
    Annotation,
}

impl SectionKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Review => "review",
            Self::NeedsFix => "needs fix",
            Self::Annotation => "highlight",
        }
    }
}

/// One entry of the function in `review.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSection {
    pub kind: SectionKind,
    pub heading: String,
    /// 1-based line of the `#` heading.
    pub line: usize,
    /// 0-based, end-exclusive line range of the whole entry (the band).
    pub span: std::ops::Range<usize>,
    /// Who wrote it.
    pub by: String,
    /// The editable part: the body without a review's generated sign-off.
    pub comments: String,
}

/// The body of an entry without the generated sign-off of a review.
fn comments_of(entry: &Entry, body: &str) -> String {
    match entry {
        Entry::Review(r) => {
            let so = sign_off(&r.review);
            body.trim_end()
                .strip_suffix(so.as_str())
                .unwrap_or(body)
                .trim()
                .to_string()
        }
        _ => body.trim().to_string(),
    }
}

/// The entries of `review.md` text `md` that target function `fn_id`, in
/// file order (module doc).
pub fn function_sections(md: &str, fn_id: &str) -> Vec<FunctionSection> {
    let doc = parse_review_md(md);
    doc.entries
        .iter()
        .filter_map(|e| {
            let (kind, by, target) = match &e.entry {
                Entry::Review(r) => (SectionKind::Review, r.review.by.clone(), r.function_id()),
                Entry::NeedsFix(n) => (
                    SectionKind::NeedsFix,
                    n.needs_fix.by.clone(),
                    n.function_id(),
                ),
                Entry::Annotation(a) => (
                    SectionKind::Annotation,
                    a.annotation.by.clone(),
                    a.function_id(),
                ),
                _ => return None,
            };
            (target == fn_id).then(|| FunctionSection {
                kind,
                heading: e.heading.clone(),
                line: e.line,
                span: heading_span(md, e.line, ARTIFACT_LEVEL),
                by,
                comments: comments_of(&e.entry, &e.body),
            })
        })
        .collect()
}

/// The 0-based line ranges of the sections (the sidebar's bands).
pub fn bands(sections: &[FunctionSection]) -> Vec<std::ops::Range<usize>> {
    sections.iter().map(|s| s.span.clone()).collect()
}

/// Why comments could not be spliced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpliceError {
    /// No entry heading on that line (the file changed under the editor).
    NoEntry(usize),
    /// A comment line is a level-1 heading.
    HeadingInComments(String),
    /// The result does not read back (module doc).
    NotReadBack(String),
}

impl std::fmt::Display for SpliceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoEntry(l) => write!(
                f,
                "no review.md entry starts on line {l} any more (the file changed): reload"
            ),
            Self::HeadingInComments(l) => write!(
                f,
                "comment line {l:?} is a level-1 heading and would end the entry; use ## or deeper"
            ),
            Self::NotReadBack(m) => write!(f, "review.md would not read back: {m}; nothing saved"),
        }
    }
}

/// `md` with the comments of the entry whose heading is on 1-based `line`
/// replaced by `comments` (module doc).
pub fn splice_comments(md: &str, line: usize, comments: &str) -> Result<String, SpliceError> {
    if let Some(l) = comments
        .lines()
        .find(|l| l.starts_with("# ") || l.trim() == "#")
    {
        return Err(SpliceError::HeadingInComments(l.to_string()));
    }
    let before = parse_review_md(md);
    let Some(target) = before.entries.iter().find(|e| e.line == line) else {
        return Err(SpliceError::NoEntry(line));
    };
    let span = heading_span(md, line, ARTIFACT_LEVEL);
    let lines: Vec<&str> = md.lines().collect();
    // The end of the entry's TOML fence: the second ``` line in the span.
    let mut fences = 0;
    let mut toml_end = None;
    for (i, l) in lines.iter().enumerate().take(span.end).skip(span.start) {
        if l.trim_start().starts_with("```") {
            fences += 1;
            if fences == 2 {
                toml_end = Some(i);
                break;
            }
        }
    }
    let Some(toml_end) = toml_end else {
        return Err(SpliceError::NoEntry(line));
    };
    let comments = comments.trim();
    let body = match &target.entry {
        Entry::Review(r) if comments.is_empty() => sign_off(&r.review),
        Entry::Review(r) => format!("{comments}\n\n{}", sign_off(&r.review)),
        _ => comments.to_string(),
    };
    let mut out: Vec<String> = lines[..=toml_end].iter().map(|s| s.to_string()).collect();
    if !body.is_empty() {
        out.push(String::new());
        out.push(body);
    }
    if span.end < lines.len() {
        out.push(String::new());
        out.extend(lines[span.end..].iter().map(|s| s.to_string()));
    }
    let mut text = out.join("\n");
    if md.ends_with('\n') {
        text.push('\n');
    }
    let after = parse_review_md(&text);
    let same_entries = after.entries.len() == before.entries.len()
        && after
            .entries
            .iter()
            .zip(&before.entries)
            .all(|(a, b)| a.entry == b.entry && a.heading == b.heading);
    if !same_entries || after.unreadable.len() != before.unreadable.len() {
        return Err(SpliceError::NotReadBack(
            "an entry changed or became unreadable".into(),
        ));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FID: &str = "fn:0123456789abcdef";
    const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

    fn hash(c: char) -> String {
        format!("sha256:{}", c.to_string().repeat(64))
    }

    /// A needs-fix entry, an unrelated note and another function's
    /// needs-fix, as `review.md` holds them.
    fn md() -> String {
        let nf = |id: &str, target: &str, note: &str, body: &str| {
            format!(
                "# Needs fix: f (github:tester)\n\n```toml\n[kovan]\nid = \"{id}\"\nkind = \"needs_fix\"\norigin = \"human\"\ncreated = \"2026-10-10T10:00:00+08:00\"\nmodified = \"2026-10-10T10:00:00+08:00\"\ntarget = \"{target}\"\n\n[needs_fix]\npath = \"crates/a/src/lib.rs::f\"\nby = \"github:tester\"\ndate = \"2026-10-10\"\ncommit = \"{SHA}\"\nhash = \"{}\"\nnote = \"{note}\"\nstatus = \"open\"\n```\n{body}",
                hash('a')
            )
        };
        format!(
            "{}\n# A note\n\n```toml\n[kovan]\nid = \"n1\"\nkind = \"note\"\ncreated = \"c\"\nmodified = \"m\"\n```\n\nKeep   me.\n\n{}",
            nf("fix-1", FID, "guard missing", "\nWhere: line 3.\n"),
            nf("fix-2", "fn:fedcba9876543210", "other", "")
        )
    }

    /// Methodology: a file with two needs-fix entries (one for this
    /// function) and a note. Pass: only this function's entry is a
    /// section, with its comments and its band.
    #[test]
    fn sections_are_this_functions_entries_only() {
        let m = md();
        let s = function_sections(&m, FID);
        assert_eq!(s.len(), 1, "{s:?}");
        assert_eq!(s[0].kind, SectionKind::NeedsFix);
        assert_eq!(s[0].comments, "Where: line 3.");
        assert_eq!(s[0].line, 1);
        assert_eq!(bands(&s), vec![0..23]);
        assert!(function_sections(&m, "fn:0000000000000000").is_empty());
    }

    /// Methodology: replace the comments of this function's entry. Pass:
    /// the TOML, the other entries and every other byte are unchanged; the
    /// new comments read back; a level-1 heading and a stale line are
    /// refused.
    #[test]
    fn splice_changes_only_the_comments() {
        let m = md();
        let t = splice_comments(&m, 1, "## Comments\n\nGuard p < 0.").unwrap();
        let s = function_sections(&t, FID);
        assert_eq!(s[0].comments, "## Comments\n\nGuard p < 0.");
        let toml_end = m.find("```\n").unwrap();
        assert_eq!(t[..toml_end], m[..toml_end], "TOML block kept");
        assert!(t.contains("\nKeep   me.\n"), "other bytes kept");
        assert_eq!(t.matches("# Needs fix:").count(), 2);
        assert!(matches!(
            splice_comments(&m, 1, "# not allowed"),
            Err(SpliceError::HeadingInComments(_))
        ));
        assert_eq!(splice_comments(&m, 2, "x"), Err(SpliceError::NoEntry(2)));
        // Emptied, then refilled: still the same entries.
        let e = splice_comments(&m, 1, "").unwrap();
        assert_eq!(function_sections(&e, FID)[0].comments, "");
    }
}
