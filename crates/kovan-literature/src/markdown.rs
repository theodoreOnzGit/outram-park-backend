//! Markdown generation and structural analysis.
//!
//! This module holds the deterministic text → Markdown conversion used by the
//! PDF pipeline plus small helpers for inspecting the result (heading outline,
//! page-limit splitting). It has no I/O and no external state, so every
//! function here is a pure, offline transformation — implementing the
//! "Markdown generation" step of `docs/kovan.md`, "PDF Processing".
//!
//! What belongs here: string → string / string → structure transforms over
//! Markdown or over the flat text an extractor produces. What does not: PDF
//! parsing (see [`crate::pdf_import`]) or metadata heuristics (see
//! [`crate::metadata`]).

/// One heading in a Markdown document: its level (1–6) and text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    /// Heading level, 1 (`#`) through 6 (`######`).
    pub level: u8,
    /// Plain-text heading content.
    pub text: String,
}

/// The marker inserted between source pages by [`text_to_markdown`]. Also the
/// boundary [`split_markdown_by_page_limit`] splits on. A blank-line-delimited
/// Markdown thematic break, so it renders as a horizontal rule and survives a
/// round-trip through any CommonMark parser.
pub const PAGE_SEPARATOR: &str = "\n\n---\n\n";

/// Parse a Markdown body and return its heading outline, using `pulldown-cmark`
/// (the one Markdown parser KOVAN starts with, `docs/kovan.md`). Deterministic;
/// useful for building a table of contents or for splitting long documents.
///
/// Input: any CommonMark string. Output: headings in document order.
pub fn markdown_outline(markdown: &str) -> Vec<Heading> {
    use pulldown_cmark::{Event, Parser, Tag, TagEnd};

    let mut outline = Vec::new();
    let mut current: Option<u8> = None;
    let mut buf = String::new();
    for event in Parser::new(markdown) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current = Some(heading_level_to_u8(level));
                buf.clear();
            }
            Event::Text(t) | Event::Code(t) if current.is_some() => buf.push_str(&t),
            Event::End(TagEnd::Heading(_)) => {
                if let Some(level) = current.take() {
                    outline.push(Heading {
                        level,
                        text: buf.trim().to_string(),
                    });
                }
            }
            _ => {}
        }
    }
    outline
}

fn heading_level_to_u8(level: pulldown_cmark::HeadingLevel) -> u8 {
    use pulldown_cmark::HeadingLevel::*;
    match level {
        H1 => 1,
        H2 => 2,
        H3 => 3,
        H4 => 4,
        H5 => 5,
        H6 => 6,
    }
}

/// Convert the flat text an extractor produces into structured Markdown.
///
/// This is a deterministic, offline transform. Its steps:
///
/// 1. Split on the form-feed (`U+000C`) page breaks emitted by `pdf-extract`.
/// 2. Within each page, group runs of non-blank lines into paragraphs (a blank
///    line ends a paragraph), joining wrapped lines with a space and undoing
///    end-of-line hyphenation (`combus-\ntion` → `combustion`).
/// 3. Promote lines that are *high-confidence* headings to Markdown `##`+:
///    numbered section headers (`1 Introduction`, `2.1 Methods`) and a fixed
///    set of well-known section keywords (`Abstract`, `References`, …). Nesting
///    depth sets the level. Everything else stays paragraph text — the
///    converter deliberately does **not** guess at other structure, to avoid
///    fabricating headings the source does not have.
/// 4. Join pages with [`PAGE_SEPARATOR`].
///
/// Returns a single Markdown body. Use [`split_markdown_by_page_limit`] to break
/// it into `≤ MAX_MARKDOWN_PAGES` chunks.
pub fn text_to_markdown(raw: &str) -> String {
    let pages: Vec<String> = raw
        .split('\u{000C}')
        .map(page_to_markdown)
        .filter(|p| !p.trim().is_empty())
        .collect();
    pages.join(PAGE_SEPARATOR)
}

/// Split a Markdown body produced by [`text_to_markdown`] into chunks that each
/// contain at most `max_pages` source pages, so no generated document exceeds
/// the `docs/kovan.md` `≤ 30`-page target (see [`crate::MAX_MARKDOWN_PAGES`]).
///
/// Splitting happens on the [`PAGE_SEPARATOR`] boundaries the converter
/// inserted. A body with `≤ max_pages` pages returns as a single chunk. A
/// `max_pages` of 0 is treated as 1.
pub fn split_markdown_by_page_limit(markdown: &str, max_pages: u32) -> Vec<String> {
    let per = max_pages.max(1) as usize;
    let pages: Vec<&str> = markdown.split(PAGE_SEPARATOR).collect();
    pages
        .chunks(per)
        .map(|chunk| chunk.join(PAGE_SEPARATOR))
        .collect()
}

/// Convert one page of flat text into Markdown blocks.
fn page_to_markdown(page: &str) -> String {
    let mut out = String::new();
    let mut para = String::new();

    let flush = |out: &mut String, para: &mut String| {
        let trimmed = para.trim();
        if !trimmed.is_empty() {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(trimmed);
        }
        para.clear();
    };

    for line in page.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() {
            flush(&mut out, &mut para);
            continue;
        }
        if let Some(level) = heading_level_for(line) {
            flush(&mut out, &mut para);
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(&"#".repeat(level as usize));
            out.push(' ');
            out.push_str(line.trim());
        } else {
            append_wrapped(&mut para, line.trim());
        }
    }
    flush(&mut out, &mut para);
    out
}

/// Append a wrapped line to a paragraph buffer, undoing soft hyphenation.
fn append_wrapped(para: &mut String, line: &str) {
    if para.is_empty() {
        para.push_str(line);
        return;
    }
    if para.ends_with('-') {
        // End-of-line hyphen: drop it and join without a space.
        para.pop();
        para.push_str(line);
    } else {
        para.push(' ');
        para.push_str(line);
    }
}

/// Section keywords that, on a line of their own, are treated as `##` headings.
const SECTION_KEYWORDS: &[&str] = &[
    "abstract",
    "introduction",
    "background",
    "methods",
    "methodology",
    "materials and methods",
    "theory",
    "results",
    "discussion",
    "results and discussion",
    "conclusion",
    "conclusions",
    "summary",
    "nomenclature",
    "acknowledgements",
    "acknowledgments",
    "references",
    "bibliography",
    "appendix",
];

/// Decide whether a line is a high-confidence heading and, if so, its level.
///
/// Two accepted forms (both capped at 6):
/// - A known section keyword on its own line → level 2.
/// - A numbered section header (`1`, `1.2`, `2.3.1`, optional trailing dot)
///   followed by a short, capitalised title → level `1 + depth`.
fn heading_level_for(line: &str) -> Option<u8> {
    let t = line.trim();
    if t.is_empty() || t.len() > 80 {
        return None;
    }

    // Keyword sections (allow an optional trailing colon).
    let key = t.trim_end_matches(':').to_ascii_lowercase();
    if SECTION_KEYWORDS.contains(&key.as_str()) {
        return Some(2);
    }

    // Numbered sections: "<number> <Title>".
    let mut it = t.splitn(2, char::is_whitespace);
    let first = it.next().unwrap_or("");
    let rest = it.next().unwrap_or("").trim();
    if rest.is_empty() || rest.len() > 70 {
        return None;
    }
    // The title must start with a letter and not end like a sentence.
    let starts_alpha = rest.chars().next().is_some_and(|c| c.is_alphabetic());
    let sentence_like = rest.ends_with('.') || rest.split_whitespace().count() > 10;
    if !starts_alpha || sentence_like {
        return None;
    }
    let depth = section_number_depth(first)?;
    Some(((depth + 1).min(6)) as u8)
}

/// If `tok` is a section number like `1`, `1.2`, `2.3.1` (optional trailing
/// dot), return its dotted depth (`1`, `2`, `3` respectively); else `None`.
fn section_number_depth(tok: &str) -> Option<usize> {
    let tok = tok.trim_end_matches('.');
    if tok.is_empty() {
        return None;
    }
    let mut depth = 0;
    for seg in tok.split('.') {
        if seg.is_empty() || !seg.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        depth += 1;
    }
    Some(depth)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_outline_extracts_headings() {
        let md = "# Title\n\nsome text\n\n## Section A\n\n### Sub\n";
        let outline = markdown_outline(md);
        assert_eq!(outline.len(), 3);
        assert_eq!(
            outline[0],
            Heading {
                level: 1,
                text: "Title".into()
            }
        );
        assert_eq!(outline[1].level, 2);
        assert_eq!(
            outline[2],
            Heading {
                level: 3,
                text: "Sub".into()
            }
        );
    }

    #[test]
    fn text_to_markdown_groups_paragraphs_and_dehyphenates() {
        let raw = "This is a wrapped\nparagraph with a combus-\ntion word.\n\nSecond paragraph.\n";
        let md = text_to_markdown(raw);
        assert_eq!(
            md,
            "This is a wrapped paragraph with a combustion word.\n\nSecond paragraph."
        );
    }

    #[test]
    fn text_to_markdown_promotes_numbered_and_keyword_headings() {
        let raw = "Abstract\n\nWe present a model.\n\n1 Introduction\n\nReactors are hot.\n\n2.1 Governing Equations\n\nConservation holds.\n";
        let md = text_to_markdown(raw);
        let outline = markdown_outline(&md);
        assert_eq!(
            outline[0],
            Heading {
                level: 2,
                text: "Abstract".into()
            }
        );
        assert_eq!(
            outline[1],
            Heading {
                level: 2,
                text: "1 Introduction".into()
            }
        );
        assert_eq!(
            outline[2],
            Heading {
                level: 3,
                text: "2.1 Governing Equations".into()
            }
        );
    }

    #[test]
    fn text_to_markdown_does_not_invent_headings() {
        // A normal sentence line must stay a paragraph, not become a heading.
        let raw = "The reactor reached criticality at noon.\n";
        let md = text_to_markdown(raw);
        assert!(markdown_outline(&md).is_empty());
    }

    #[test]
    fn page_separator_joins_pages() {
        let raw = "Page one text.\u{000C}Page two text.";
        let md = text_to_markdown(raw);
        assert_eq!(md, "Page one text.\n\n---\n\nPage two text.");
    }

    #[test]
    fn split_by_page_limit_chunks_pages() {
        // Four pages, limit 2 → two chunks of two pages each.
        let raw = "P1\u{000C}P2\u{000C}P3\u{000C}P4";
        let md = text_to_markdown(raw);
        let chunks = split_markdown_by_page_limit(&md, 2);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], "P1\n\n---\n\nP2");
        assert_eq!(chunks[1], "P3\n\n---\n\nP4");
    }

    #[test]
    fn split_below_limit_is_single_chunk() {
        let md = text_to_markdown("only one page");
        assert_eq!(split_markdown_by_page_limit(&md, 30).len(), 1);
    }
}
