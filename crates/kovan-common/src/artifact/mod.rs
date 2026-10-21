//! The **kovan Markdown artifact** block scanner, shared by desktop kovan
//! (literature notes, lessons, walkthroughs) and by code review's
//! `review.md` (GitHub #743, #764).
//!
//! # What an artifact is
//!
//! > A **`#` Markdown heading** immediately followed by a **fenced `toml`
//! > block containing a `[kovan]` table**.
//!
//! The body runs from the end of that fence to the next `#` heading (or the
//! end of the document). `##` and deeper headings are prose inside the
//! artifact. A `toml` fence with no `[kovan]` table is an ordinary code
//! example, never an artifact and never a problem. Blank lines between the
//! heading and the fence are fine; a paragraph, quote, list or table between
//! them breaks the "immediately followed by" rule.
//!
//! # Why the scanner is generic
//!
//! Moved here from `kovan::artifact::parse_document` on 2026-10-07 (GitHub
//! #764; placement decided on #743, 2026-10-06: "the artifact, relation and
//! stamp types and the Markdown artifact parser" belong in kovan-common,
//! which builds for wasm). The literature payload type (`ArtifactToml`) still
//! lives in `kovan`, because it carries `kovan`'s classification and anchor
//! types; moving those is a separate step. So [`scan_blocks`] does the
//! Markdown half and hands each candidate block's TOML text to a reader
//! closure the caller supplies: `kovan::artifact::parse_document` passes its
//! literature reader, [`crate::review::review_md`] passes the code-review
//! reader. One scanner, so the two can never disagree about where an artifact
//! starts or what its body is.
//!
//! **Behaviour is unchanged by the move**, quirks included: a block's body is
//! only assigned while it is still empty, so an artifact whose body is blank
//! keeps absorbing text up to the next accepted block. `kovan`'s own artifact
//! tests (35 of them, the backwards-compatibility fixtures among them) run
//! through this scanner since the move.
//!
//! # Parsing is total
//!
//! A block the reader rejects is returned in [`Scan::problems`]; every other
//! block is still returned. One broken entry never hides the rest.

pub mod relation;

/// The Markdown heading depth that delimits one artifact from the next: a
/// single `#` (maintainer direction, GitHub #35, 2026-09-08).
pub const ARTIFACT_LEVEL: u8 = 1;

/// One block the reader accepted.
#[derive(Debug, Clone, PartialEq)]
pub struct ScannedBlock<T> {
    /// The heading text, trimmed, exactly as written otherwise.
    pub heading: String,
    /// Heading depth (always [`ARTIFACT_LEVEL`] today).
    pub level: u8,
    /// 1-based line of the heading.
    pub line: usize,
    /// What the reader made of the TOML block.
    pub payload: T,
    /// Everything after the fence up to the next `#` heading, trimmed.
    pub body: String,
}

/// The result of [`scan_blocks`]: accepted blocks in document order, and the
/// reader's errors for the rest.
#[derive(Debug, Clone, PartialEq)]
pub struct Scan<T, E> {
    pub blocks: Vec<ScannedBlock<T>>,
    pub problems: Vec<E>,
}

/// 1-based line number of `byte_offset` within `text`.
pub fn line_of(text: &str, byte_offset: usize) -> usize {
    text[..byte_offset.min(text.len())]
        .bytes()
        .filter(|b| *b == b'\n')
        .count()
        + 1
}

/// Whether `toml_text` parses as TOML and holds a top-level `kovan` key
/// (checked structurally, so a string value that mentions `[kovan]` is not
/// mistaken for one).
pub fn has_kovan_table(toml_text: &str) -> bool {
    toml::from_str::<toml::Value>(toml_text)
        .ok()
        .and_then(|v| v.get("kovan").cloned())
        .is_some()
}

/// Scan `markdown` for artifact blocks (module doc). For each `#` heading
/// immediately followed by a `toml` fence holding `[kovan]`, `read` is called
/// with the heading, its 1-based line and the fence's TOML text; `Ok` keeps
/// the block, `Err` records a problem and skips it.
///
/// Never fails as a whole and never panics.
pub fn scan_blocks<T, E>(
    markdown: &str,
    read: impl FnMut(&str, usize, &str) -> Result<T, E>,
) -> Scan<T, E> {
    scan_blocks_with(markdown, UnparseableFence::Ignore, read)
}

/// What to do with a `toml` fence right after a `#` heading whose text is
/// not TOML at all (so it cannot be known whether it holds `[kovan]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnparseableFence {
    /// Treat it as an ordinary code example (the literature rule, §13).
    Ignore,
    /// Hand it to the reader, which reports it. `review.md` uses this: a
    /// broken review entry must show as unreadable, never vanish
    /// (maintainer, #739, 2026-10-07).
    Report,
}

/// [`scan_blocks`] with the choice of what to do with an unparseable fence.
pub fn scan_blocks_with<T, E>(
    markdown: &str,
    unparseable: UnparseableFence,
    mut read: impl FnMut(&str, usize, &str) -> Result<T, E>,
) -> Scan<T, E> {
    use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

    let mut out = Scan {
        blocks: Vec::new(),
        problems: Vec::new(),
    };

    // (heading text, level, byte offset of the heading)
    let mut pending: Option<(String, u8, usize)> = None;
    let mut heading_text = String::new();
    let mut in_heading = false;
    let mut heading_start = 0usize;
    let mut heading_level = 1u8;

    // Set while inside a fenced `toml` block that directly follows a heading.
    let mut toml_buf: Option<String> = None;
    let mut fence_end = 0usize;

    let parser = Parser::new_ext(
        markdown,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH,
    );
    for (event, range) in parser.into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                // Only a level-1 heading closes the previous artifact.
                if level as u8 == ARTIFACT_LEVEL {
                    if let Some(a) = out.blocks.last_mut() {
                        if a.body.is_empty() && fence_end > 0 && fence_end <= range.start {
                            a.body = markdown[fence_end..range.start].trim().to_string();
                        }
                    }
                }
                in_heading = true;
                heading_text.clear();
                heading_start = range.start;
                heading_level = level as u8;
            }
            Event::Text(t) | Event::Code(t) if in_heading => heading_text.push_str(&t),
            Event::End(TagEnd::Heading(_)) => {
                in_heading = false;
                pending = (heading_level == ARTIFACT_LEVEL).then(|| {
                    (
                        heading_text.trim().to_string(),
                        heading_level,
                        heading_start,
                    )
                });
            }
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang)))
                if pending.is_some() && lang.split(',').next().unwrap_or("").trim() == "toml" =>
            {
                toml_buf = Some(String::new());
            }
            Event::Text(t) if toml_buf.is_some() => {
                if let Some(buf) = toml_buf.as_mut() {
                    buf.push_str(&t);
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                if let (Some(buf), Some((heading, level, offset))) =
                    (toml_buf.take(), pending.take())
                {
                    let candidate = has_kovan_table(&buf)
                        || (unparseable == UnparseableFence::Report
                            && toml::from_str::<toml::Value>(&buf).is_err());
                    if candidate {
                        let line = line_of(markdown, offset);
                        match read(&heading, line, &buf) {
                            Ok(payload) => {
                                fence_end = range.end;
                                out.blocks.push(ScannedBlock {
                                    heading,
                                    level,
                                    line,
                                    payload,
                                    body: String::new(),
                                });
                            }
                            Err(e) => out.problems.push(e),
                        }
                    }
                }
            }
            // Any other block between a heading and a fence breaks the
            // "immediately followed by" rule.
            Event::Start(Tag::Paragraph)
            | Event::Start(Tag::BlockQuote(_))
            | Event::Start(Tag::List(_))
            | Event::Start(Tag::Table(_))
                if toml_buf.is_none() =>
            {
                pending = None;
            }
            _ => {}
        }
    }

    // The final artifact's body runs to the end of the document.
    if let Some(a) = out.blocks.last_mut() {
        if a.body.is_empty() && fence_end > 0 && fence_end <= markdown.len() {
            a.body = markdown[fence_end..].trim().to_string();
        }
    }
    out
}

/// One artifact block as Markdown: `{#…} {heading}`, a blank line, the
/// ```` ```toml ```` fence holding `toml_text`, and the body when non-empty.
/// `level` is clamped to 1..=6. The inverse of [`scan_blocks`].
pub fn render_block(level: u8, heading: &str, toml_text: &str, body: &str) -> String {
    let level = level.clamp(1, 6);
    let hashes = "#".repeat(level as usize);
    let mut toml_text = toml_text.to_string();
    if !toml_text.ends_with('\n') {
        toml_text.push('\n');
    }
    let mut out = format!("{hashes} {heading}\n\n```toml\n{toml_text}```\n");
    let body = body.trim();
    if !body.is_empty() {
        out.push('\n');
        out.push_str(body);
        out.push('\n');
    }
    out
}

/// The 0-based, end-exclusive line range from the heading on 1-based `line`
/// up to the next heading of depth `<= level` outside a fence, or the end of
/// the document. Every `#` heading delimits a block, readable or not.
///
/// Fence tracking matters: a `#` at the start of a line inside a ```` ```csv
/// ```` or ```` ```toml ```` block is data or a TOML comment, not a heading
/// (GitHub #35, 2026-09-08).
pub fn heading_span(md: &str, line: usize, level: u8) -> std::ops::Range<usize> {
    let start = line.saturating_sub(1);
    let lines: Vec<&str> = md.lines().collect();
    let mut end = lines.len();
    let mut in_fence = false;
    for (i, line) in lines.iter().enumerate().skip(start + 1) {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let hashes = line.chars().take_while(|c| *c == '#').count();
        if hashes >= 1 && hashes <= level as usize && line.chars().nth(hashes) == Some(' ') {
            end = i;
            break;
        }
    }
    start..end
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_id(_h: &str, _l: usize, t: &str) -> Result<String, String> {
        let v: toml::Value = toml::from_str(t).map_err(|e| e.to_string())?;
        v.get("kovan")
            .and_then(|k| k.get("id"))
            .and_then(|i| i.as_str())
            .map(str::to_string)
            .ok_or_else(|| "no id".to_string())
    }

    /// Methodology: a synthetic note in the literature shape (header
    /// artifact, an ordinary TOML fence, a `##` subsection, a malformed
    /// block, a CSV fence whose first line starts with `#`) scanned with a
    /// reader that only wants `[kovan] id`. Checks bodies, lines, the
    /// ordinary-fence rule and per-block problems.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn scans_blocks_bodies_and_problems() {
        let md = "# a\n\n```toml\n[kovan]\nid = \"a\"\n```\n\nBody A.\n\n## sub\n\n```toml\nx = 1\n```\n\n# b\n\n```toml\n[kovan]\nkind = \"x\"\n```\n\n# c\n\n```toml\n[kovan]\nid = \"c\"\n```\n\n```csv\n# not a heading\n1,2\n```\n";
        let s = scan_blocks(md, read_id);
        let ids: Vec<&str> = s.blocks.iter().map(|b| b.payload.as_str()).collect();
        assert_eq!(ids, ["a", "c"]);
        assert_eq!(s.blocks[0].line, 1);
        assert!(s.blocks[0].body.starts_with("Body A.\n\n## sub"));
        assert!(s.blocks[1].body.contains("# not a heading"));
        assert_eq!(s.problems, vec!["no id".to_string()]);
        let span = heading_span(md, s.blocks[1].line, ARTIFACT_LEVEL);
        assert_eq!(span.end, md.lines().count());
    }

    /// Methodology: what [`render_block`] writes is read back by
    /// [`scan_blocks`] with the same heading, payload and body.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn render_block_round_trips() {
        let md = render_block(1, "Title", "[kovan]\nid = \"t\"", "Some prose.");
        let s = scan_blocks(&md, read_id);
        assert_eq!(s.blocks.len(), 1);
        assert_eq!(s.blocks[0].heading, "Title");
        assert_eq!(s.blocks[0].payload, "t");
        assert_eq!(s.blocks[0].body, "Some prose.");
    }
}
