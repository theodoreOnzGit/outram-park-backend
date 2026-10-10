//! **Highlights in the reviewed source** (GitHub #770; #740 decision 3:
//! "Select code in the source view, then Annotate. Each highlight is its
//! own annotation artifact in `review.md` … anchored with Hypothesis
//! selectors (#754: exact quote with context, plus position). They
//! re-anchor after edits, and are marked orphaned rather than misplaced.")
//!
//! ```text
//!  lines first..=last of the function's source (doc comment included)
//!      │ kovan_common::anchoring::{lines_to_range, describe}
//!      v
//!  [annotation] selector = [position, quote]  ── appended to review.md
//!                                               as its own `#` entry
//!  later: anchoring::anchor(function source now, selectors)
//!      ├─ Anchored ──> the lines it covers now
//!      └─ Orphaned ──> shown as orphaned, never placed by guess
//! ```
//!
//! Selectors are relative to **the function's own source** (doc comment
//! included), not the file, so a highlight survives edits elsewhere in
//! the file by its position alone, and edits inside the function by its
//! quote.
//!
//! An annotation is not a stamp: it is not signed (the schema has no
//! signature for it, `review_md::AnnotationBody`). Writing one never
//! commits (#740 decision 6).

use kovan_common::anchoring::{anchor, describe, lines_to_range, range_to_lines, Anchoring};
use kovan_common::artifact::{render_block, ARTIFACT_LEVEL};
use kovan_common::review::draft::review_entry_id;
use kovan_common::review::index::FunctionIndex;
use kovan_common::review::review_md::{
    parse_review_md, AnnotationBody, AnnotationEntry, Entry, EntryMeta, ReviewDocument,
};
use kovan_common::review::types::{check_commit, reviewer_id_kind};

/// A highlight as shown.
#[derive(Debug, Clone, PartialEq)]
pub struct Highlight {
    /// The `[kovan] id`.
    pub id: String,
    /// 1-based line of its heading in `review.md` (click: jump to the note).
    pub line: usize,
    pub by: String,
    /// The note (the entry's body).
    pub note: String,
    /// 1-based inclusive lines of the function's source it covers now;
    /// `None`: orphaned.
    pub lines: Option<(usize, usize)>,
    /// Found by the fuzzy quote search (the code there changed): shown as
    /// re-anchored, to be looked at.
    pub fuzzy: bool,
}

/// Every highlight of function `fn_id` in `doc`, anchored in `source` (the
/// function's source now) (module doc).
pub fn highlights(doc: &ReviewDocument, fn_id: &str, source: &str) -> Vec<Highlight> {
    doc.entries
        .iter()
        .filter_map(|e| match &e.entry {
            Entry::Annotation(a) if a.function_id() == fn_id => {
                let found = anchor(source, &a.annotation.selector);
                let (lines, fuzzy) = match found {
                    Anchoring::Anchored(x) => (
                        Some(range_to_lines(source, x.start, x.end)),
                        x.strategy == kovan_common::anchoring::AnchorStrategy::FuzzyQuote,
                    ),
                    Anchoring::Orphaned(_) => (None, false),
                };
                Some(Highlight {
                    id: a.kovan.id.clone(),
                    line: e.line,
                    by: a.annotation.by.clone(),
                    note: e.body.trim().to_string(),
                    lines,
                    fuzzy,
                })
            }
            _ => None,
        })
        .collect()
}

/// The heading of a highlight entry.
pub fn highlight_heading(qual: &str, by: &str) -> String {
    format!("Highlight: {qual} ({by})")
}

/// Draft the annotation entry for lines `first..=last` of `source` (the
/// function's source at `commit`), by `by`, at time `now` (RFC 3339).
#[allow(clippy::too_many_arguments)]
pub fn draft_highlight(
    f: &FunctionIndex,
    file: &str,
    by: &str,
    commit: &str,
    now: &str,
    source: &str,
    first: usize,
    last: usize,
) -> Result<AnnotationEntry, String> {
    reviewer_id_kind(by).map_err(|e| e.to_string())?;
    check_commit("annotation.commit", commit).map_err(|e| e.to_string())?;
    let (start, end) = lines_to_range(source, first, last)
        .ok_or_else(|| format!("lines {first}-{last} are not in the function"))?;
    let stamp: String = now.chars().filter(char::is_ascii_digit).collect();
    let base = review_entry_id(&f.name, by, &f.id);
    Ok(AnnotationEntry {
        kovan: EntryMeta {
            id: format!(
                "highlight{}-{stamp}-l{first}",
                base.trim_start_matches("review")
            ),
            kind: "annotation".into(),
            origin: Some("human".into()),
            created: now.into(),
            modified: now.into(),
            target: Some(f.id.clone()),
        },
        annotation: AnnotationBody {
            path: Some(format!("{file}::{}", f.qual)),
            by: by.into(),
            commit: commit.into(),
            selector: describe(source, start, end),
            needs_fix: None,
        },
    })
}

/// `md` with the highlight appended as its own entry, the note as its
/// body; read back before it is returned (the entry must parse equal, the
/// unreadable count must not change).
pub fn append_highlight(md: &str, entry: &AnnotationEntry, note: &str) -> Result<String, String> {
    if note.lines().any(|l| l.starts_with("# ") || l.trim() == "#") {
        return Err("a note line is a level-1 heading; use ## or deeper".into());
    }
    let toml = Entry::Annotation(entry.clone())
        .to_toml()
        .map_err(|e| e.to_string())?;
    let qual = entry
        .path()
        .and_then(|p| p.split_once(".rs::").map(|(_, q)| q.to_string()))
        .unwrap_or_default();
    let block = render_block(
        ARTIFACT_LEVEL,
        &highlight_heading(&qual, &entry.annotation.by),
        &toml,
        note.trim(),
    );
    let mut text = md.to_string();
    if !text.trim().is_empty() {
        if !text.ends_with('\n') {
            text.push('\n');
        }
        text.push('\n');
    } else {
        text.clear();
    }
    text.push_str(&block);
    let before = parse_review_md(md);
    let after = parse_review_md(&text);
    let back = after
        .entries
        .iter()
        .any(|e| matches!(&e.entry, Entry::Annotation(a) if a == entry));
    if !back || after.unreadable.len() != before.unreadable.len() {
        return Err("review.md would not read back with the highlight; nothing written".into());
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

    fn f() -> FunctionIndex {
        FunctionIndex {
            id: "fn:0123456789abcdef".into(),
            name: "flash".into(),
            qual: "Steam::flash".into(),
            item: Default::default(),
            lines: [3, 8],
            hash: format!("sha256:{}", "a".repeat(64)),
            doc_hash: format!("sha256:{}", "b".repeat(64)),
            callees: Vec::new(),
            reached_by: Vec::new(),
            test: false,
            index_out_of_date: false,
            physical_interface: false,
        }
    }

    /// Methodology: highlight lines 2-3 of a function, append it to a
    /// `review.md` holding an unreadable entry, then re-anchor in (a) the
    /// same source, (b) the source with a line inserted above, (c) the
    /// source with the highlighted code edited slightly, (d) the source
    /// with it deleted. Pass: the entry reads back and the unreadable
    /// entry's bytes are kept; (a) lines 2-3, (b) lines 3-4, (c) found
    /// fuzzily, (d) orphaned, never placed by guess.
    #[test]
    fn highlight_appends_and_reanchors() {
        let src = "/// Flash.\nfn flash(p: f64) -> f64 {\n    let q = p.max(0.0);\n    q * 2.0\n}";
        let e = draft_highlight(
            &f(),
            "crates/t/src/steam.rs",
            "github:tester",
            SHA,
            "2026-10-10T10:00:00+08:00",
            src,
            2,
            3,
        )
        .unwrap();
        let broken = "# Broken\n\n```toml\n[kovan]\nid = \"x\"\nkind = \"review\"\ncreated = \"c\"\nmodified = \"m\"\n```\n\nKeep   these.\n";
        let md = append_highlight(broken, &e, "## Note\n\nWhy clamp at zero?").unwrap();
        assert!(md.starts_with(broken));
        assert!(md.contains("# Highlight: Steam::flash (github:tester)"));
        let doc = parse_review_md(&md);
        let h = highlights(&doc, "fn:0123456789abcdef", src);
        assert_eq!(h.len(), 1);
        assert_eq!((h[0].lines, h[0].fuzzy), (Some((2, 3)), false));
        assert_eq!(h[0].note, "## Note\n\nWhy clamp at zero?");
        let moved = format!("// new line\n{src}");
        assert_eq!(
            highlights(&doc, "fn:0123456789abcdef", &moved)[0].lines,
            Some((3, 4))
        );
        let edited = src.replace("p.max(0.0)", "p.max(0.01)");
        let h = highlights(&doc, "fn:0123456789abcdef", &edited);
        assert!(h[0].lines.is_some() && h[0].fuzzy, "{h:?}");
        let gone = "/// Flash.\nfn other() {}";
        assert_eq!(highlights(&doc, "fn:0123456789abcdef", gone)[0].lines, None);
        assert!(highlights(&doc, "fn:ffffffffffffffff", src).is_empty());
        assert!(append_highlight("", &e, "# bad").is_err());
        assert!(draft_highlight(&f(), "a.rs", "github:tester", SHA, "t", src, 9, 9).is_err());
        assert!(draft_highlight(&f(), "a.rs", "nobody", SHA, "t", src, 1, 1).is_err());
    }
}
