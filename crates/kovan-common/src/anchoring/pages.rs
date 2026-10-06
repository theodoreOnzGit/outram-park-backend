// Ported from the Hypothesis client (TypeScript),
// `src/annotator/anchoring/pdf.ts` (`describe`, `anchor`/`anchorRange`,
// `anchorQuote`, `findPageByOffset`, `getPageOffset`, `createPageSelector`).
//   Upstream:  https://github.com/hypothesis/client
//   Commit:    b4d085a2f893aa6de3b61d8b8bc3ae4d0f24fc1a (shallow clone, 2026-10-07)
//   Copyright: (c) 2013-2019 Hypothes.is Project and contributors
//   Licence:   BSD-2-Clause (checked first-hand 2026-10-07; see NOTICE).
//              This Rust port is distributed under AGPL-3.0-only.
//
//! Describe and anchor a range of a paged text: the extracted text of a
//! PDF, one string per page. Upstream's PDF path without PDF.js: the "page
//! text" is the string kovan's extractor produced for that page.
//!
//! As upstream:
//! - The position selector counts from the start of page 0, over the page
//!   texts joined with nothing between them.
//! - Quote matching **ignores whitespace** ([`super::offsets::is_space`]):
//!   page text, quote, prefix and suffix are compared with their spaces
//!   stripped, because re-extraction (a different PDF library or version)
//!   often adds or drops spaces and line breaks.
//! - Pages are searched in order of distance from the page the position
//!   points at, and the search stops early on an exact quote with an exact
//!   prefix or suffix (or an exact quote and no context at all).
//! - The quote's prefix and suffix are taken from the quote's page only.
//!
//! Deviations, each deliberate:
//! - **Early-stop suffix check fixed.** Upstream compares
//!   `strippedText.slice(match.end, strippedSuffix.length)` (a length used
//!   as an end offset), so its exact-suffix test is almost always false.
//!   This port slices `[end, end + len)`, which is what the comment beside
//!   it describes. It can only make the search stop earlier on a match that
//!   is already exact in quote and suffix.
//! - **A position hint of 0 counts.** Upstream tests `if (positionHint)`,
//!   so a quote at the very start of the document is searched without a
//!   hint. Here `Some(0)` is a hint like any other.
//! - **Page selector as a hint (kovan's addition).** Upstream's text
//!   anchoring ignores the page selector. Here, when there is no position
//!   selector, a page selector orders the pages by distance from it, so a
//!   `page` + `quote` anchor searches its own page first.
//! - No session cache of quote matches and no placeholder for unrendered
//!   pages: both belong to the viewer, not the anchoring.

use super::match_quote::match_quote_chars;
use super::offsets::{is_not_space, strip_spaces, translate_offsets};
use super::selector::{PageSelector, Selector, TextPositionSelector};
use super::text::quote_of;
use super::{Anchor, AnchorStrategy, Anchoring, OrphanReason};

/// Selectors for `pages[page][start..end]` (`char` offsets within that
/// page): the document-wide position, the quote with context from that
/// page, and the page (label defaults to `page + 1`). Upstream pdf.ts
/// `describe`. `None` if `page` is out of range.
pub fn describe_in_pages<P: AsRef<str>>(
    pages: &[P],
    page: usize,
    start: usize,
    end: usize,
    label: Option<String>,
) -> Option<Vec<Selector>> {
    let text: Vec<char> = pages.get(page)?.as_ref().chars().collect();
    let end = end.min(text.len());
    let start = start.min(end);
    let offset: usize = pages[..page]
        .iter()
        .map(|p| p.as_ref().chars().count())
        .sum();
    Some(vec![
        Selector::TextPositionSelector(TextPositionSelector {
            start: offset + start,
            end: offset + end,
        }),
        Selector::TextQuoteSelector(quote_of(&text, start, end)),
        Selector::PageSelector(PageSelector {
            index: page,
            label: Some(label.unwrap_or_else(|| (page + 1).to_string())),
        }),
    ])
}

/// The page containing document offset `offset`, with the offset at which
/// that page starts (upstream `findPageByOffset`: the first page whose end
/// is at or past `offset`; past the end of the document, the last page).
fn find_page_by_offset(pages: &[Vec<char>], offset: i64) -> (usize, i64) {
    let mut page_start = 0i64;
    let mut page_end = 0i64;
    for (i, p) in pages.iter().enumerate() {
        page_start = page_end;
        page_end += p.len() as i64;
        if page_end >= offset {
            return (i, page_start);
        }
    }
    (pages.len() - 1, page_start)
}

/// JavaScript's `String.prototype.substring`: clamp both ends to the text
/// and swap them if reversed.
fn substring(text: &[char], a: i64, b: i64) -> String {
    let n = text.len() as i64;
    let (a, b) = (a.clamp(0, n) as usize, b.clamp(0, n) as usize);
    let (a, b) = if a > b { (b, a) } else { (a, b) };
    text[a..b].iter().collect()
}

/// Anchor `selectors` in a paged text (upstream pdf.ts `anchorRange` and
/// `anchorQuote`). A quote selector is required, as upstream: a position
/// alone cannot be checked against anything. The returned [`Anchor`] has
/// `page: Some(i)` and `char` offsets within that page's text.
pub fn anchor_in_pages<P: AsRef<str>>(pages: &[P], selectors: &[Selector]) -> Anchoring {
    let Some(quote) = Selector::quote(selectors) else {
        return Anchoring::Orphaned(OrphanReason::NoUsableSelector);
    };
    if pages.is_empty() {
        return Anchoring::Orphaned(OrphanReason::NotFound);
    }
    let pages: Vec<Vec<char>> = pages.iter().map(|p| p.as_ref().chars().collect()).collect();
    let position = Selector::position(selectors);

    // 1. Position, verified against the quote.
    if let Some(pos) = position {
        let (index, offset) = find_page_by_offset(&pages, pos.start as i64);
        let start = pos.start as i64 - offset;
        let end = pos.end as i64 - offset;
        if substring(&pages[index], start, end) == quote.exact {
            let n = pages[index].len() as i64;
            let (a, b) = (start.clamp(0, n) as usize, end.clamp(0, n) as usize);
            return Anchoring::Anchored(Anchor {
                page: Some(index),
                start: a.min(b),
                end: a.max(b),
                strategy: AnchorStrategy::Position,
                score: 1.0,
            });
        }
    }

    // 2. Quote, whitespace-insensitive, nearest pages first.
    let mut order: Vec<usize> = (0..pages.len()).collect();
    let mut expected: Option<(usize, Option<i64>)> = None;
    if let Some(pos) = position {
        let (index, offset) = find_page_by_offset(&pages, pos.start as i64);
        expected = Some((index, Some(pos.start as i64 - offset)));
    } else if let Some(page) = Selector::page(selectors) {
        expected = Some((page.index.min(pages.len() - 1), None));
    }
    if let Some((index, _)) = expected {
        order.sort_by_key(|&a| a.abs_diff(index));
    }

    let strip_opt = |s: &Option<String>| {
        s.as_ref()
            .map(|s| strip_spaces(&s.chars().collect::<Vec<_>>()))
    };
    let stripped_prefix = strip_opt(&quote.prefix);
    let stripped_suffix = strip_opt(&quote.suffix);
    let stripped_quote = strip_spaces(&quote.exact.chars().collect::<Vec<_>>());

    let mut best: Option<Anchor> = None;
    for page in order {
        let text = &pages[page];
        let stripped_text = strip_spaces(text);

        let stripped_hint = match expected {
            Some((exp, _)) if page < exp => Some(stripped_text.len() as i64),
            Some((exp, Some(off))) if page == exp => {
                Some(translate_offsets(text, &stripped_text, off, off, is_not_space).0 as i64)
            }
            Some((exp, None)) if page == exp => None,
            Some(_) => Some(0),
            None => None,
        };

        let Some(m) = match_quote_chars(
            &stripped_text,
            &stripped_quote,
            stripped_prefix.as_deref(),
            stripped_suffix.as_deref(),
            stripped_hint,
        ) else {
            continue;
        };

        if best.as_ref().is_none_or(|b| m.score > b.score) {
            let (start, end) = translate_offsets(
                &stripped_text,
                text,
                m.start as i64,
                m.end as i64,
                is_not_space,
            );
            let exact_quote = stripped_text[m.start..m.end] == stripped_quote[..];
            best = Some(Anchor {
                page: Some(page),
                start,
                end,
                strategy: if exact_quote {
                    AnchorStrategy::ExactQuote
                } else {
                    AnchorStrategy::FuzzyQuote
                },
                score: m.score,
            });

            // Stop early on an exact quote with an exact prefix or suffix.
            let exact_prefix = stripped_prefix
                .as_ref()
                .is_some_and(|p| stripped_text[m.start.saturating_sub(p.len())..m.start] == p[..]);
            let exact_suffix = stripped_suffix.as_ref().is_some_and(|s| {
                stripped_text[m.end..(m.end + s.len()).min(stripped_text.len())] == s[..]
            });
            let has_context = stripped_prefix.is_some() || stripped_suffix.is_some();
            if exact_quote && (exact_prefix || exact_suffix || !has_context) {
                break;
            }
        }
    }
    match best {
        Some(a) => Anchoring::Anchored(a),
        None => Anchoring::Orphaned(OrphanReason::NotFound),
    }
}
