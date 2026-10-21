// SPDX-License-Identifier: AGPL-3.0-only
//
// Provenance (partial translation — see below for exactly which parts):
//   Upstream project: markdown-editor, "Live (GitHub-Flavored) Markdown Editor"
//                     https://github.com/jbt/markdown-editor
//   Source file:      index.js — `setOutput` (scroll the preview to the first
//                     element that changed) and `update` (take the document
//                     title from the first `<h1>`)
//   Commit:           58aa8bfe18c5a30af2c6fb08221b00b841688dd9 (2020-05-03)
//   Copyright:        (c) 2018 James Taylor
//   Licence:          ISC — "Permission to use, copy, modify, and/or distribute
//                     this software for any purpose with or without fee is
//                     hereby granted, provided that the above copyright notice
//                     and this permission notice appear in all copies." The
//                     full text is reproduced in crates/kovan/NOTICE.
//   Translated:       2026-09-28, JavaScript -> Rust, into kovan (AGPL-3.0-only;
//                     ISC is AGPL-compatible).
//
// NOT translated: upstream's parsing. markdown-editor renders through the
// third-party `markdown-it` 4.1.1 + `markdown-it-footnote` 1.0.0 (both MIT),
// which produce HTML for a browser. None of that is ported here: parsing is
// `pulldown-cmark` (already a kovan dependency) with its GFM options, and the
// block model below is this file's own, built for egui rather than the DOM.

//! The egui-free half of the kvim tab's GitHub-flavoured Markdown preview:
//! Markdown text -> a small block model with **source line ranges**.
//!
//! Keeping the source lines on every block is what makes the VS Code-style
//! scroll sync possible — the editor's top visible line picks a block, and
//! that block's painted position is where the preview scrolls
//! ([`sync_offset`]). It is also why this is a model of its own rather than
//! an off-the-shelf egui Markdown widget, which paints but does not report
//! where each source line ended up.
//!
//! # GFM coverage
//!
//! Tables, strikethrough, task lists, footnotes and GitHub alerts
//! (`> [!NOTE]`) come from `pulldown-cmark`'s own extensions ([`options`]).
//! GFM's **extended autolinks** (a bare `https://…` or `www.…` becomes a
//! link) are not something `pulldown-cmark` 0.12 implements, so
//! [`split_autolinks`] adds them here, on plain text only — never inside a
//! code span, a code block or an existing link.
//!
//! Not rendered: raw HTML (shown verbatim, monospaced, the way GitHub shows
//! what it would sanitise away) and images (shown as a labelled link — the
//! kovan GUI installs no image loaders, and a paper's Markdown is not the
//! place to fetch remote images from). Table column alignment is parsed but
//! not applied.

use std::ops::Range;

use pulldown_cmark::{BlockQuoteKind, CodeBlockKind, Event, LinkType, Options, Parser, Tag, TagEnd};

/// The `pulldown-cmark` extensions that make up GitHub-flavoured Markdown.
///
/// `ENABLE_GFM` is `pulldown-cmark`'s switch for GitHub's blockquote alerts
/// (`> [!NOTE]`, `> [!WARNING]`, …). Smart punctuation, heading attributes,
/// metadata blocks, maths and definition lists are deliberately **off**:
/// GitHub does none of them, and a preview that renders what GitHub will not
/// misleads the person writing for GitHub.
pub fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_GFM
}

/// How one run of inline text is styled.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SpanStyle {
    pub strong: bool,
    pub emphasis: bool,
    pub strikethrough: bool,
    /// An inline code span (or inline raw HTML, shown verbatim).
    pub code: bool,
    /// A footnote reference, `[^label]`.
    pub footnote_ref: bool,
}

/// One run of inline text with a single style, and a link target if it is
/// (part of) a link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub style: SpanStyle,
    pub link: Option<String>,
}

/// A GitHub alert (`> [!NOTE]` …) — the blockquote's kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

impl AlertKind {
    fn from_cmark(kind: BlockQuoteKind) -> Self {
        match kind {
            BlockQuoteKind::Note => Self::Note,
            BlockQuoteKind::Tip => Self::Tip,
            BlockQuoteKind::Important => Self::Important,
            BlockQuoteKind::Warning => Self::Warning,
            BlockQuoteKind::Caution => Self::Caution,
        }
    }

    /// The label GitHub prints at the top of the alert.
    pub fn label(self) -> &'static str {
        match self {
            Self::Note => "Note",
            Self::Tip => "Tip",
            Self::Important => "Important",
            Self::Warning => "Warning",
            Self::Caution => "Caution",
        }
    }
}

/// One list item: its task-list state (`Some(checked)` for `- [ ]`/`- [x]`)
/// and its content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    pub task: Option<bool>,
    pub blocks: Vec<Block>,
}

/// What a block is.
///
/// Container blocks hold `Vec<Block>` — the one place this model is
/// recursive, which is what a Markdown document is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockKind {
    Heading {
        level: u8,
        spans: Vec<Span>,
    },
    Paragraph(Vec<Span>),
    CodeBlock {
        lang: String,
        code: String,
    },
    Quote {
        alert: Option<AlertKind>,
        blocks: Vec<Block>,
    },
    /// `start` is `Some(n)` for an ordered list starting at `n`.
    List {
        start: Option<u64>,
        items: Vec<ListItem>,
    },
    Table {
        head: Vec<Vec<Span>>,
        rows: Vec<Vec<Vec<Span>>>,
    },
    Rule,
    /// Raw HTML, kept verbatim.
    Html(String),
    FootnoteDefinition {
        label: String,
        blocks: Vec<Block>,
    },
}

/// One block and the 0-based, end-exclusive source lines it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub kind: BlockKind,
    pub lines: Range<usize>,
}

/// Byte offset -> 0-based line, over a precomputed table of line starts.
struct LineIndex {
    starts: Vec<usize>,
}

impl LineIndex {
    fn new(src: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(src.match_indices('\n').map(|(i, _)| i + 1));
        Self { starts }
    }

    fn line_of(&self, byte: usize) -> usize {
        match self.starts.binary_search(&byte) {
            Ok(line) => line,
            Err(next) => next.saturating_sub(1),
        }
    }

    /// The lines a byte range covers, end-exclusive and never empty.
    fn lines(&self, range: &Range<usize>) -> Range<usize> {
        let first = self.line_of(range.start);
        let last = self.line_of(range.end.saturating_sub(1).max(range.start));
        first..last + 1
    }
}

/// An open container while the event stream is walked.
enum Frame {
    Root(Vec<Block>),
    Quote {
        alert: Option<AlertKind>,
        blocks: Vec<Block>,
        range: Range<usize>,
    },
    List {
        start: Option<u64>,
        items: Vec<ListItem>,
        range: Range<usize>,
    },
    Item {
        task: Option<bool>,
        blocks: Vec<Block>,
    },
    Footnote {
        label: String,
        blocks: Vec<Block>,
        range: Range<usize>,
    },
    Table {
        head: Vec<Vec<Span>>,
        rows: Vec<Vec<Vec<Span>>>,
        row: Vec<Vec<Span>>,
        range: Range<usize>,
    },
}

/// Where the inline text being collected will go when it ends.
enum InlineTarget {
    Paragraph,
    /// A tight list item's text, which `pulldown-cmark` emits with no
    /// `Paragraph` around it.
    ImplicitParagraph,
    Heading(u8),
    TableCell,
}

struct Inline {
    target: InlineTarget,
    spans: Vec<Span>,
    range: Range<usize>,
}

/// The walk's whole state.
struct Builder {
    lines: LineIndex,
    stack: Vec<Frame>,
    inline: Option<Inline>,
    strong: usize,
    emphasis: usize,
    strike: usize,
    links: Vec<String>,
    /// `Some` while inside a fenced/indented code block: (lang, code, range).
    code: Option<(String, String, Range<usize>)>,
    /// `Some` while inside an HTML block.
    html: Option<(String, Range<usize>)>,
}

impl Builder {
    fn style(&self) -> SpanStyle {
        SpanStyle {
            strong: self.strong > 0,
            emphasis: self.emphasis > 0,
            strikethrough: self.strike > 0,
            code: false,
            footnote_ref: false,
        }
    }

    fn push_block(&mut self, kind: BlockKind, range: &Range<usize>) {
        let block = Block {
            kind,
            lines: self.lines.lines(range),
        };
        match self.stack.last_mut() {
            Some(Frame::Root(blocks))
            | Some(Frame::Quote { blocks, .. })
            | Some(Frame::Item { blocks, .. })
            | Some(Frame::Footnote { blocks, .. }) => blocks.push(block),
            // A block directly inside a list or table cannot happen in a
            // well-formed event stream; drop it rather than panic.
            Some(Frame::List { .. }) | Some(Frame::Table { .. }) | None => {}
        }
    }

    /// Inline content arriving with nothing open to receive it is a tight
    /// list item's text: open an implicit paragraph for it.
    fn ensure_inline(&mut self, range: &Range<usize>) {
        if self.inline.is_none() {
            self.inline = Some(Inline {
                target: InlineTarget::ImplicitParagraph,
                spans: Vec::new(),
                range: range.clone(),
            });
        }
    }

    /// Close an implicit paragraph before any block-level event.
    fn flush_implicit(&mut self) {
        if matches!(
            self.inline,
            Some(Inline {
                target: InlineTarget::ImplicitParagraph,
                ..
            })
        ) {
            let inline = self.inline.take().expect("checked above");
            self.push_block(BlockKind::Paragraph(inline.spans), &inline.range);
        }
    }

    fn push_span(&mut self, text: &str, style: SpanStyle, range: &Range<usize>) {
        self.ensure_inline(range);
        let link = self.links.last().cloned();
        let inline = self.inline.as_mut().expect("ensured above");
        inline.range.end = inline.range.end.max(range.end);
        if link.is_none() && !style.code {
            for (piece, auto) in split_autolinks(text) {
                inline.spans.push(Span {
                    text: piece,
                    style,
                    link: auto,
                });
            }
        } else {
            inline.spans.push(Span {
                text: text.to_string(),
                style,
                link,
            });
        }
    }

    fn start(&mut self, tag: Tag, range: Range<usize>) {
        let block_level = !matches!(
            tag,
            Tag::Emphasis | Tag::Strong | Tag::Strikethrough | Tag::Link { .. } | Tag::Image { .. }
        );
        if block_level {
            self.flush_implicit();
        }
        match tag {
            Tag::Paragraph => {
                self.inline = Some(Inline {
                    target: InlineTarget::Paragraph,
                    spans: Vec::new(),
                    range,
                })
            }
            Tag::Heading { level, .. } => {
                self.inline = Some(Inline {
                    target: InlineTarget::Heading(level as u8),
                    spans: Vec::new(),
                    range,
                })
            }
            Tag::BlockQuote(kind) => self.stack.push(Frame::Quote {
                alert: kind.map(AlertKind::from_cmark),
                blocks: Vec::new(),
                range,
            }),
            Tag::CodeBlock(kind) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(info) => {
                        info.split_whitespace().next().unwrap_or("").to_string()
                    }
                    CodeBlockKind::Indented => String::new(),
                };
                self.code = Some((lang, String::new(), range));
            }
            Tag::HtmlBlock => self.html = Some((String::new(), range)),
            Tag::List(start) => self.stack.push(Frame::List {
                start,
                items: Vec::new(),
                range,
            }),
            Tag::Item => self.stack.push(Frame::Item {
                task: None,
                blocks: Vec::new(),
            }),
            Tag::FootnoteDefinition(label) => self.stack.push(Frame::Footnote {
                label: label.to_string(),
                blocks: Vec::new(),
                range,
            }),
            Tag::Table(_) => self.stack.push(Frame::Table {
                head: Vec::new(),
                rows: Vec::new(),
                row: Vec::new(),
                range,
            }),
            Tag::TableHead | Tag::TableRow => {
                if let Some(Frame::Table { row, .. }) = self.stack.last_mut() {
                    row.clear();
                }
            }
            Tag::TableCell => {
                self.inline = Some(Inline {
                    target: InlineTarget::TableCell,
                    spans: Vec::new(),
                    range,
                })
            }
            Tag::Emphasis => self.emphasis += 1,
            Tag::Strong => self.strong += 1,
            Tag::Strikethrough => self.strike += 1,
            Tag::Link {
                link_type,
                dest_url,
                ..
            } => {
                let url = if link_type == LinkType::Email && !dest_url.starts_with("mailto:") {
                    format!("mailto:{dest_url}")
                } else {
                    dest_url.to_string()
                };
                self.links.push(url);
            }
            Tag::Image { dest_url, .. } => {
                // Shown as a link to the image, labelled with its alt text
                // (which arrives as the Text events inside).
                self.push_span("\u{1F5BC} ", self.style(), &range);
                self.links.push(dest_url.to_string());
            }
            // Not enabled by `options()`; harmless if they ever arrive.
            Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::MetadataBlock(_) => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        let block_level = !matches!(
            tag,
            TagEnd::Emphasis
                | TagEnd::Strong
                | TagEnd::Strikethrough
                | TagEnd::Link
                | TagEnd::Image
        );
        if block_level {
            self.flush_implicit();
        }
        match tag {
            TagEnd::Paragraph | TagEnd::Heading(_) => {
                if let Some(inline) = self.inline.take() {
                    let kind = match inline.target {
                        InlineTarget::Heading(level) => BlockKind::Heading {
                            level,
                            spans: inline.spans,
                        },
                        _ => BlockKind::Paragraph(inline.spans),
                    };
                    self.push_block(kind, &inline.range);
                }
            }
            TagEnd::TableCell => {
                if let Some(inline) = self.inline.take() {
                    if let Some(Frame::Table { row, .. }) = self.stack.last_mut() {
                        row.push(inline.spans);
                    }
                }
            }
            TagEnd::TableHead => {
                if let Some(Frame::Table { head, row, .. }) = self.stack.last_mut() {
                    *head = std::mem::take(row);
                }
            }
            TagEnd::TableRow => {
                if let Some(Frame::Table { rows, row, .. }) = self.stack.last_mut() {
                    rows.push(std::mem::take(row));
                }
            }
            TagEnd::Table => {
                if let Some(Frame::Table {
                    head, rows, range, ..
                }) = self.stack.pop()
                {
                    self.push_block(BlockKind::Table { head, rows }, &range);
                }
            }
            TagEnd::BlockQuote(_) => {
                if let Some(Frame::Quote {
                    alert,
                    blocks,
                    range,
                }) = self.stack.pop()
                {
                    self.push_block(BlockKind::Quote { alert, blocks }, &range);
                }
            }
            TagEnd::CodeBlock => {
                if let Some((lang, code, range)) = self.code.take() {
                    self.push_block(BlockKind::CodeBlock { lang, code }, &range);
                }
            }
            TagEnd::HtmlBlock => {
                if let Some((html, range)) = self.html.take() {
                    self.push_block(BlockKind::Html(html), &range);
                }
            }
            TagEnd::Item => {
                if let Some(Frame::Item { task, blocks }) = self.stack.pop() {
                    if let Some(Frame::List { items, .. }) = self.stack.last_mut() {
                        items.push(ListItem { task, blocks });
                    }
                }
            }
            TagEnd::List(_) => {
                if let Some(Frame::List {
                    start,
                    items,
                    range,
                }) = self.stack.pop()
                {
                    self.push_block(BlockKind::List { start, items }, &range);
                }
            }
            TagEnd::FootnoteDefinition => {
                if let Some(Frame::Footnote {
                    label,
                    blocks,
                    range,
                }) = self.stack.pop()
                {
                    self.push_block(BlockKind::FootnoteDefinition { label, blocks }, &range);
                }
            }
            TagEnd::Emphasis => self.emphasis = self.emphasis.saturating_sub(1),
            TagEnd::Strong => self.strong = self.strong.saturating_sub(1),
            TagEnd::Strikethrough => self.strike = self.strike.saturating_sub(1),
            TagEnd::Link | TagEnd::Image => {
                self.links.pop();
            }
            TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition
            | TagEnd::MetadataBlock(_) => {}
        }
    }
}

/// Parse `src` as GitHub-flavoured Markdown into top-level [`Block`]s, each
/// carrying the source lines it came from. Total: any input gives a model.
pub fn parse(src: &str) -> Vec<Block> {
    let mut b = Builder {
        lines: LineIndex::new(src),
        stack: vec![Frame::Root(Vec::new())],
        inline: None,
        strong: 0,
        emphasis: 0,
        strike: 0,
        links: Vec::new(),
        code: None,
        html: None,
    };
    for (event, range) in Parser::new_ext(src, options()).into_offset_iter() {
        match event {
            Event::Start(tag) => b.start(tag, range),
            Event::End(tag) => b.end(tag),
            Event::Text(text) => {
                if let Some((_, code, _)) = b.code.as_mut() {
                    code.push_str(&text);
                } else {
                    let style = b.style();
                    b.push_span(&text, style, &range);
                }
            }
            Event::Code(text) => {
                let style = SpanStyle {
                    code: true,
                    ..b.style()
                };
                b.push_span(&text, style, &range);
            }
            Event::Html(text) => {
                if let Some((html, _)) = b.html.as_mut() {
                    html.push_str(&text);
                } else {
                    b.flush_implicit();
                    b.push_block(BlockKind::Html(text.to_string()), &range);
                }
            }
            Event::InlineHtml(text) => {
                let style = SpanStyle {
                    code: true,
                    ..b.style()
                };
                b.push_span(&text, style, &range);
            }
            Event::FootnoteReference(label) => {
                let style = SpanStyle {
                    footnote_ref: true,
                    ..b.style()
                };
                b.push_span(&format!("[{label}]"), style, &range);
            }
            Event::SoftBreak => {
                let style = b.style();
                b.push_span(" ", style, &range);
            }
            Event::HardBreak => {
                let style = b.style();
                b.push_span("\n", style, &range);
            }
            Event::Rule => {
                b.flush_implicit();
                b.push_block(BlockKind::Rule, &range);
            }
            Event::TaskListMarker(checked) => {
                if let Some(Frame::Item { task, .. }) = b.stack.last_mut() {
                    *task = Some(checked);
                }
            }
            // Maths is not enabled (GitHub's `$…$` is not CommonMark).
            Event::InlineMath(text) | Event::DisplayMath(text) => {
                let style = b.style();
                b.push_span(&text, style, &range);
            }
        }
    }
    b.flush_implicit();
    // Anything left open by a truncated stream is dropped; the root is kept.
    match b.stack.into_iter().next() {
        Some(Frame::Root(blocks)) => blocks,
        _ => Vec::new(),
    }
}

/// GFM's extended autolinks on one run of plain text: split it into pieces,
/// each `Some(url)` if it is a bare `http://`, `https://` or `www.` link.
///
/// The URL runs to the next whitespace or `<`; trailing `. , : ; ! ? ' "`
/// and an unbalanced `)` are left outside it, per the GFM spec's
/// "extended autolink path validation". `www.` links get `http://`
/// prepended, as GitHub does.
pub fn split_autolinks(text: &str) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    let mut plain_start = 0;
    let mut i = 0;
    let bytes = text.as_bytes();
    while i < text.len() {
        let rest = &text[i..];
        let at_boundary = i == 0
            || !text[..i]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric());
        let prefix = ["https://", "http://", "www."]
            .into_iter()
            .find(|p| rest.len() > p.len() && rest.starts_with(p));
        if let (true, Some(prefix)) = (at_boundary, prefix) {
            let mut end = rest
                .find(|c: char| c.is_whitespace() || c == '<')
                .unwrap_or(rest.len());
            loop {
                let candidate = &rest[..end];
                let Some(last) = candidate.chars().next_back() else {
                    break;
                };
                let unbalanced_paren =
                    last == ')' && candidate.matches(')').count() > candidate.matches('(').count();
                if matches!(last, '.' | ',' | ':' | ';' | '!' | '?' | '\'' | '"')
                    || unbalanced_paren
                {
                    end -= last.len_utf8();
                } else {
                    break;
                }
            }
            if end > prefix.len() {
                if plain_start < i {
                    out.push((text[plain_start..i].to_string(), None));
                }
                let shown = &rest[..end];
                let url = if prefix == "www." {
                    format!("http://{shown}")
                } else {
                    shown.to_string()
                };
                out.push((shown.to_string(), Some(url)));
                i += end;
                plain_start = i;
                continue;
            }
        }
        // Advance one char (UTF-8 safe).
        i += 1;
        while i < text.len() && (bytes[i] & 0b1100_0000) == 0b1000_0000 {
            i += 1;
        }
    }
    if plain_start < text.len() || out.is_empty() {
        out.push((text[plain_start..].to_string(), None));
    }
    out
}

/// Translated from upstream `update()`: the first level-1 heading's text,
/// which upstream used as the window title. Here it titles the preview.
pub fn document_title(blocks: &[Block]) -> Option<String> {
    blocks.iter().find_map(|b| match &b.kind {
        BlockKind::Heading { level: 1, spans } => {
            let text: String = spans.iter().map(|s| s.text.as_str()).collect();
            let text = text.trim().to_string();
            (!text.is_empty()).then_some(text)
        }
        _ => None,
    })
}

/// Translated from upstream `setOutput()`: after a re-render, upstream walked
/// the old and new DOM side by side and scrolled the preview to the **first
/// element that differs**, so an edit is always visible in the preview. This
/// is the same walk over top-level blocks (compared by content, not by line
/// — inserting a line above shifts every later block's lines without
/// changing them). `None` when nothing changed.
pub fn first_changed_block(old: &[Block], new: &[Block]) -> Option<usize> {
    let common = old.len().min(new.len());
    (0..common)
        .find(|&i| old[i].kind != new[i].kind)
        .or((old.len() != new.len()).then_some(common))
}

/// A top-level block's painted extent in the preview, recorded by the view
/// each frame: its source lines and its top/bottom relative to the start of
/// the preview's content.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockPos {
    pub lines: Range<usize>,
    pub top: f32,
    pub bottom: f32,
}

/// The preview scroll offset that shows source line `line` at the top — the
/// VS Code "editor scroll -> preview scroll" mapping.
///
/// Inside a block, the offset is interpolated by how far through the block's
/// lines `line` is; on a blank line between blocks it is the next block's
/// top; past the last block it is the last block's bottom. `None` with no
/// recorded blocks (nothing painted yet).
pub fn sync_offset(blocks: &[BlockPos], line: usize) -> Option<f32> {
    let last = blocks.iter().max_by_key(|b| b.lines.end)?;
    if let Some(b) = blocks.iter().find(|b| b.lines.contains(&line)) {
        let span = (b.lines.end - b.lines.start).max(1) as f32;
        let frac = (line - b.lines.start) as f32 / span;
        return Some(b.top + frac * (b.bottom - b.top));
    }
    Some(
        blocks
            .iter()
            .filter(|b| b.lines.start > line)
            .min_by_key(|b| b.lines.start)
            .map_or(last.bottom, |b| b.top),
    )
}

/// What the preview shows above a fenced `toml` block that is an artifact's
/// `[kovan]` metadata (GH issue #743): its kind, an "AI" badge when
/// `origin = "ai"`, and one card per `[relation]`/`[[relation]]` record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactStrip {
    /// The kind's wire name, e.g. `walk_step`.
    pub kind: &'static str,
    /// Whether `origin = "ai"`.
    pub ai: bool,
    /// One `(relation kind label, target text, navigable)` per record. A
    /// `code:` target is not navigable yet (it arrives with web-kovan).
    pub links: Vec<(String, String, bool)>,
}

/// [`ArtifactStrip`] for a code block, or `None` when it is not `toml`, has
/// no `[kovan]` table, or does not parse as one (the raw block still shows).
pub fn artifact_strip(lang: &str, code: &str) -> Option<ArtifactStrip> {
    if lang.split(',').next().unwrap_or("").trim() != "toml" || !code.contains("[kovan]") {
        return None;
    }
    let toml: crate::artifact::ArtifactToml = toml::from_str(code).ok()?;
    let links = toml
        .relation
        .as_ref()
        .map_or(&[][..], crate::relation::Relations::records)
        .iter()
        .map(|r| {
            let mut text = r.target.clone();
            if let Some(p) = r.page {
                text.push_str(&format!(" p. {p}"));
            }
            let navigable = !crate::relation::CodeTarget::is_code(&r.target);
            (r.kind.label().to_string(), text, navigable)
        })
        .collect();
    Some(ArtifactStrip {
        kind: toml.kovan.kind.as_str(),
        ai: toml.kovan.origin() == crate::artifact::Origin::Ai,
        links,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kovan_toml_block_gets_an_artifact_strip() {
        let code = "[kovan]\nid = \"s1\"\nkind = \"walk_step\"\ncreated = \"t\"\nmodified = \"t\"\norigin = \"ai\"\n\n[[relation]]\ntarget = \"code:crates/x/src/a.rs::A::f@L12\"\nkind = \"implements\"\n\n[[relation]]\ntarget = \"paper:smith2020\"\nkind = \"supports\"\npage = 4\n";
        let strip = artifact_strip("toml", code).expect("strip");
        assert_eq!(strip.kind, "walk_step");
        assert!(strip.ai);
        assert_eq!(strip.links.len(), 2);
        assert!(!strip.links[0].2, "a code: target is not navigable yet");
        assert_eq!(strip.links[1].1, "paper:smith2020 p. 4");
        assert!(strip.links[1].2);
        // An ordinary TOML example and a non-toml block get none.
        assert_eq!(artifact_strip("toml", "[package]\nname = \"x\"\n"), None);
        assert_eq!(artifact_strip("rust", code), None);
    }

    fn plain(spans: &[Span]) -> String {
        spans.iter().map(|s| s.text.as_str()).collect()
    }

    #[test]
    fn a_gfm_table_parses_into_head_and_rows() {
        let blocks = parse("| a | b |\n|---|--:|\n| 1 | 2 |\n| 3 | 4 |\n");
        assert_eq!(blocks.len(), 1);
        let BlockKind::Table { head, rows } = &blocks[0].kind else {
            panic!("not a table: {:?}", blocks[0].kind);
        };
        assert_eq!(
            head.iter().map(|c| plain(c)).collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(rows.len(), 2);
        assert_eq!(plain(&rows[1][1]), "4");
        assert_eq!(blocks[0].lines, 0..4);
    }

    #[test]
    fn strikethrough_marks_the_span() {
        let blocks = parse("keep ~~gone~~ keep\n");
        let BlockKind::Paragraph(spans) = &blocks[0].kind else {
            panic!("not a paragraph");
        };
        let struck: Vec<_> = spans.iter().filter(|s| s.style.strikethrough).collect();
        assert_eq!(struck.len(), 1);
        assert_eq!(struck[0].text, "gone");
    }

    #[test]
    fn task_list_items_carry_their_checked_state() {
        let blocks = parse("- [ ] todo\n- [x] done\n- plain\n");
        let BlockKind::List { start, items } = &blocks[0].kind else {
            panic!("not a list");
        };
        assert_eq!(*start, None);
        assert_eq!(
            items.iter().map(|i| i.task).collect::<Vec<_>>(),
            [Some(false), Some(true), None]
        );
        // Tight-list text arrives without a Paragraph tag; it must not be lost.
        let BlockKind::Paragraph(spans) = &items[1].blocks[0].kind else {
            panic!("item text missing");
        };
        assert_eq!(plain(spans).trim(), "done");
    }

    #[test]
    fn fenced_code_keeps_its_language_and_is_never_autolinked() {
        let blocks = parse("```toml\n[kovan]\nurl = \"https://x.org\"\n```\n");
        assert_eq!(
            blocks[0].kind,
            BlockKind::CodeBlock {
                lang: "toml".into(),
                code: "[kovan]\nurl = \"https://x.org\"\n".into()
            }
        );
        assert_eq!(blocks[0].lines, 0..4);
    }

    #[test]
    fn bare_urls_become_links_but_code_spans_do_not() {
        let blocks = parse("see https://example.org/a. and `https://no.pe` or www.x.com\n");
        let BlockKind::Paragraph(spans) = &blocks[0].kind else {
            panic!("not a paragraph");
        };
        let links: Vec<_> = spans.iter().filter_map(|s| s.link.clone()).collect();
        assert_eq!(links, ["https://example.org/a", "http://www.x.com"]);
        assert!(spans.iter().any(|s| s.style.code && s.link.is_none()));
    }

    #[test]
    fn autolink_split_trims_trailing_punctuation_and_unbalanced_parens() {
        assert_eq!(
            split_autolinks("(see https://a.org/x)"),
            vec![
                ("(see ".to_string(), None),
                (
                    "https://a.org/x".to_string(),
                    Some("https://a.org/x".to_string())
                ),
                (")".to_string(), None),
            ]
        );
        assert_eq!(
            split_autolinks("no link"),
            vec![("no link".to_string(), None)]
        );
        assert_eq!(
            split_autolinks("xhttps://a.org"),
            vec![("xhttps://a.org".to_string(), None)]
        );
    }

    #[test]
    fn footnotes_and_alerts_parse() {
        let blocks = parse("Text[^1].\n\n[^1]: The note.\n\n> [!WARNING]\n> Careful.\n");
        assert!(blocks.iter().any(|b| matches!(
            &b.kind,
            BlockKind::FootnoteDefinition { label, .. } if label == "1"
        )));
        assert!(blocks.iter().any(|b| matches!(
            b.kind,
            BlockKind::Quote {
                alert: Some(AlertKind::Warning),
                ..
            }
        )));
        let BlockKind::Paragraph(spans) = &blocks[0].kind else {
            panic!()
        };
        assert!(
            spans
                .iter()
                .any(|s| s.style.footnote_ref && s.text == "[1]")
        );
    }

    #[test]
    fn blocks_record_their_source_lines() {
        let blocks = parse("# Title\n\npara one\nstill one\n\n## Next\n");
        let lines: Vec<_> = blocks.iter().map(|b| b.lines.clone()).collect();
        assert_eq!(lines, [0..1, 2..4, 5..6]);
        assert_eq!(document_title(&blocks).as_deref(), Some("Title"));
    }

    #[test]
    fn first_changed_block_finds_the_edit_like_upstream_set_output() {
        let old = parse("# A\n\nx\n\ny\n");
        let edited = parse("# A\n\nx!\n\ny\n");
        assert_eq!(first_changed_block(&old, &edited), Some(1));
        assert_eq!(first_changed_block(&old, &old), None);
        // A line inserted above shifts later lines but changes no content.
        let shifted = parse("\n\n# A\n\nx\n\ny\n");
        assert_eq!(first_changed_block(&old, &shifted), None);
        let appended = parse("# A\n\nx\n\ny\n\nz\n");
        assert_eq!(first_changed_block(&old, &appended), Some(3));
    }

    #[test]
    fn sync_offset_interpolates_within_and_snaps_between_blocks() {
        let pos = vec![
            BlockPos {
                lines: 0..1,
                top: 0.0,
                bottom: 30.0,
            },
            BlockPos {
                lines: 2..6,
                top: 40.0,
                bottom: 120.0,
            },
        ];
        assert_eq!(sync_offset(&pos, 0), Some(0.0));
        assert_eq!(sync_offset(&pos, 1), Some(40.0), "blank line -> next block");
        assert_eq!(sync_offset(&pos, 4), Some(80.0), "halfway through block 2");
        assert_eq!(sync_offset(&pos, 99), Some(120.0), "past the end");
        assert_eq!(sync_offset(&[], 3), None);
    }
}
