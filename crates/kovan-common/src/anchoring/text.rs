// Ported from the Hypothesis client (TypeScript),
// `src/annotator/anchoring/html.ts` (`anchor`, `describe`) and
// `src/annotator/anchoring/types.ts` (`TextPositionAnchor`,
// `TextQuoteAnchor.fromRange` / `toPositionAnchor`).
//   Upstream:  https://github.com/hypothesis/client
//   Commit:    b4d085a2f893aa6de3b61d8b8bc3ae4d0f24fc1a (shallow clone, 2026-10-07)
//   Copyright: (c) 2013-2019 Hypothes.is Project and contributors
//   Licence:   BSD-2-Clause (checked first-hand 2026-10-07; see NOTICE).
//              This Rust port is distributed under AGPL-3.0-only.
//
//! Describe and anchor a range of one plain text: a note, a source file, a
//! page. Upstream's HTML path with the DOM taken out: the "root element's
//! text content" is simply the text.
//!
//! The line helpers at the end are kovan's own (not upstream): they convert
//! between `char` ranges and the 1-based inclusive line ranges review stamps
//! record, so a stamp's `lines` can be described and re-anchored.

use super::match_quote::match_quote_chars;
use super::selector::{Selector, TextPositionSelector, TextQuoteSelector};
use super::{Anchor, AnchorStrategy, Anchoring, OrphanReason};

/// Characters of context captured on each side of a quote (upstream's
/// fixed `contextLen`).
pub const CONTEXT_LEN: usize = 32;

/// The quote selector for `text[start..end]` (`char` offsets, clamped to
/// the text): the exact text and up to [`CONTEXT_LEN`] characters either
/// side. Upstream `TextQuoteAnchor.fromRange`; prefix and suffix are always
/// present, empty at the ends of the text, as upstream.
pub fn describe_quote(text: &str, start: usize, end: usize) -> TextQuoteSelector {
    let chars: Vec<char> = text.chars().collect();
    quote_of(&chars, start, end)
}

pub(crate) fn quote_of(chars: &[char], start: usize, end: usize) -> TextQuoteSelector {
    let end = end.min(chars.len());
    let start = start.min(end);
    let s = |a: usize, b: usize| chars[a..b].iter().collect::<String>();
    TextQuoteSelector {
        exact: s(start, end),
        prefix: Some(s(start.saturating_sub(CONTEXT_LEN), start)),
        suffix: Some(s(end, (end + CONTEXT_LEN).min(chars.len()))),
    }
}

/// The selectors for `text[start..end]`: position, then quote. Upstream
/// `describe` (html.ts) without the DOM-only `RangeSelector` and
/// `MediaTimeSelector`.
pub fn describe(text: &str, start: usize, end: usize) -> Vec<Selector> {
    let chars: Vec<char> = text.chars().collect();
    let end = end.min(chars.len());
    let start = start.min(end);
    vec![
        Selector::TextPositionSelector(TextPositionSelector { start, end }),
        Selector::TextQuoteSelector(quote_of(&chars, start, end)),
    ]
}

/// Anchor `selectors` in `text`, upstream's order (html.ts `anchor`):
///
/// 1. **Position**, accepted only if the text there equals the quote's
///    `exact` (when there is a quote; with no quote it is accepted
///    unverified, as upstream).
/// 2. **Quote**, via [`super::match_quote()`]: exact occurrences first, and
///    only if there are none the fuzzy search, both ranked by quote,
///    prefix, suffix and nearness to the position selector's `start`.
///
/// Page selectors are ignored here (see [`super::anchor_in_pages`]).
pub fn anchor(text: &str, selectors: &[Selector]) -> Anchoring {
    let chars: Vec<char> = text.chars().collect();
    let quote = Selector::quote(selectors);
    let position = Selector::position(selectors);

    if let Some(pos) = position {
        // `TextRange.fromOffsets` throws for offsets past the text.
        if pos.start <= pos.end && pos.end <= chars.len() {
            let here: String = chars[pos.start..pos.end].iter().collect();
            match quote {
                Some(q) if !q.exact.is_empty() => {
                    if here == q.exact {
                        return Anchoring::Anchored(Anchor {
                            page: None,
                            start: pos.start,
                            end: pos.end,
                            strategy: AnchorStrategy::Position,
                            score: 1.0,
                        });
                    }
                }
                _ => {
                    return Anchoring::Anchored(Anchor {
                        page: None,
                        start: pos.start,
                        end: pos.end,
                        strategy: AnchorStrategy::PositionUnverified,
                        score: 1.0,
                    });
                }
            }
        }
    }

    let Some(q) = quote else {
        return Anchoring::Orphaned(if position.is_some() {
            OrphanReason::NotFound
        } else {
            OrphanReason::NoUsableSelector
        });
    };
    let exact: Vec<char> = q.exact.chars().collect();
    let prefix: Option<Vec<char>> = q.prefix.as_ref().map(|s| s.chars().collect());
    let suffix: Option<Vec<char>> = q.suffix.as_ref().map(|s| s.chars().collect());
    let hint = position.map(|p| p.start as i64);
    match match_quote_chars(&chars, &exact, prefix.as_deref(), suffix.as_deref(), hint) {
        Some(m) => Anchoring::Anchored(Anchor {
            page: None,
            start: m.start,
            end: m.end,
            strategy: if m.errors == 0 {
                AnchorStrategy::ExactQuote
            } else {
                AnchorStrategy::FuzzyQuote
            },
            score: m.score,
        }),
        None => Anchoring::Orphaned(OrphanReason::NotFound),
    }
}

/// The `char` range of 1-based inclusive lines `first..=last` of `text`,
/// from the first character of `first` to the end of `last` (its newline
/// excluded). `None` if the lines do not exist. Kovan's, not upstream.
pub fn lines_to_range(text: &str, first: usize, last: usize) -> Option<(usize, usize)> {
    if first == 0 || last < first {
        return None;
    }
    let mut line = 1;
    let mut start = None;
    let mut line_start = 0;
    for (i, c) in text.chars().enumerate() {
        if line == first && start.is_none() {
            start = Some(line_start);
        }
        if c == '\n' {
            if line == last {
                return start.map(|s| (s, i));
            }
            line += 1;
            line_start = i + 1;
        }
    }
    let n = text.chars().count();
    if line == first && start.is_none() {
        start = Some(line_start);
    }
    if line == last {
        start.map(|s| (s, n))
    } else {
        None
    }
}

/// The 1-based inclusive lines that `text[start..end]` (`char` offsets)
/// touches. A range ending just after a newline does not count the next
/// line. Kovan's, not upstream.
pub fn range_to_lines(text: &str, start: usize, end: usize) -> (usize, usize) {
    let chars: Vec<char> = text.chars().collect();
    let start = start.min(chars.len());
    // Offset of the last character in the range (the start, if empty).
    let last_char = end.min(chars.len()).max(start + 1) - 1;
    let newlines = |upto: usize| {
        chars[..upto.min(chars.len())]
            .iter()
            .filter(|c| **c == '\n')
            .count()
    };
    (1 + newlines(start), 1 + newlines(last_char.max(start)))
}
