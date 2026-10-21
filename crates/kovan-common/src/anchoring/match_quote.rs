// Ported from the Hypothesis client (TypeScript),
// `src/annotator/anchoring/match-quote.ts`.
//   Upstream:  https://github.com/hypothesis/client
//   Commit:    b4d085a2f893aa6de3b61d8b8bc3ae4d0f24fc1a (shallow clone, 2026-10-07)
//   Copyright: (c) 2013-2019 Hypothes.is Project and contributors
//   Licence:   BSD-2-Clause (LICENSE file and package.json checked first-hand,
//              2026-10-07). The notice is reproduced in this crate's NOTICE.
//              This Rust port is distributed under AGPL-3.0-only as part of
//              kovan-common.
//
//! Find the best approximate match of a quote in a text, scored by how well
//! the quote, its prefix and suffix, and its expected position agree
//! (Hypothesis's `matchQuote`, GitHub #754).
//!
//! The weights and the error budget are upstream's, unchanged:
//!
//! | Term | Weight | Score in [0, 1] |
//! |---|---|---|
//! | quote | 50 | `1 - errors / quote.len()` |
//! | prefix | 20 | similarity of the text before the match to `prefix` |
//! | suffix | 20 | similarity of the text after the match to `suffix` |
//! | position | 2 | `1 - |start - hint| / text.len()` (a tie-breaker) |
//!
//! The total is divided by 92. The candidates are only the matches with the
//! fewest errors, and at most `min(256, quote.len() / 2)` errors are allowed;
//! beyond that there is no match. Upstream has **no score threshold**: the
//! best candidate is returned whatever its score. That is kept; the score is
//! returned so a caller can apply its own.

use super::approx_match::{search_f64, Match};

/// The best match of a quote, in `char` offsets of the searched text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuoteMatch {
    /// Start offset of the match.
    pub start: usize,
    /// End offset (exclusive) of the match.
    pub end: usize,
    /// Edits between the quote and the matched text (0 for an exact match).
    pub errors: usize,
    /// Upstream's normalised score in [0, 1]; 1.0 is a perfect match of
    /// the quote and of both context strings at the expected position.
    /// (The position term can go below 0 when the hint lies beyond the end
    /// of the text, as upstream.)
    pub score: f64,
}

/// What the quote was expected to sit in (upstream's `Context`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QuoteContext {
    /// Expected text just before the quote.
    pub prefix: Option<String>,
    /// Expected text just after the quote.
    pub suffix: Option<String>,
    /// Expected start offset (`char`) of the quote in the text.
    pub hint: Option<usize>,
}

/// Exact matches first (overlapping, as upstream's `indexOf` loop), and
/// only when there are none the approximate search. Upstream `search`.
fn search(text: &[char], s: &[char], max_errors: f64) -> Vec<Match> {
    let mut exact = Vec::new();
    if !s.is_empty() && s.len() <= text.len() {
        for start in 0..=(text.len() - s.len()) {
            if text[start..start + s.len()] == *s {
                exact.push(Match {
                    start,
                    end: start + s.len(),
                    errors: 0,
                });
            }
        }
    }
    if !exact.is_empty() {
        return exact;
    }
    search_f64(text, s, max_errors)
}

/// Similarity in [0, 1] of `text` to `s` (upstream `textMatchScore`).
fn text_match_score(text: &[char], s: &[char]) -> f64 {
    if s.is_empty() || text.is_empty() {
        return 0.0;
    }
    let matches = search(text, s, s.len() as f64);
    1.0 - matches[0].errors as f64 / s.len() as f64
}

/// [`match_quote`] on `char` slices; the form the anchoring code uses.
/// `hint` is signed because the PDF path can produce a hint from a position
/// outside the page (upstream passes such hints through unchanged).
pub(crate) fn match_quote_chars(
    text: &[char],
    quote: &[char],
    prefix: Option<&[char]>,
    suffix: Option<&[char]>,
    hint: Option<i64>,
) -> Option<QuoteMatch> {
    if quote.is_empty() {
        return None;
    }
    // Recall/precision/cost trade-off; expected cost O(max_errors/32 * n).
    let max_errors = 256f64.min(quote.len() as f64 / 2.0);
    let matches = search(text, quote, max_errors);
    if matches.is_empty() {
        return None;
    }

    const QUOTE_WEIGHT: f64 = 50.0;
    const PREFIX_WEIGHT: f64 = 20.0;
    const SUFFIX_WEIGHT: f64 = 20.0;
    const POS_WEIGHT: f64 = 2.0;

    let score_match = |mt: &Match| -> f64 {
        let quote_score = 1.0 - mt.errors as f64 / quote.len() as f64;
        // An empty prefix/suffix is "falsy" upstream and scores 1.0.
        let prefix_score = match prefix {
            Some(p) if !p.is_empty() => {
                text_match_score(&text[mt.start.saturating_sub(p.len())..mt.start], p)
            }
            _ => 1.0,
        };
        let suffix_score = match suffix {
            Some(s) if !s.is_empty() => {
                let end = (mt.end + s.len()).min(text.len());
                text_match_score(&text[mt.end..end], s)
            }
            _ => 1.0,
        };
        let pos_score = match hint {
            Some(h) => 1.0 - (mt.start as i64 - h).unsigned_abs() as f64 / text.len() as f64,
            None => 1.0,
        };
        let raw = QUOTE_WEIGHT * quote_score
            + PREFIX_WEIGHT * prefix_score
            + SUFFIX_WEIGHT * suffix_score
            + POS_WEIGHT * pos_score;
        raw / (QUOTE_WEIGHT + PREFIX_WEIGHT + SUFFIX_WEIGHT + POS_WEIGHT)
    };

    // Highest score wins; on a tie the earliest match (a stable sort, as
    // JavaScript's `Array.prototype.sort`).
    let mut best: Option<QuoteMatch> = None;
    for mt in &matches {
        let score = score_match(mt);
        if best.is_none_or(|b| score > b.score) {
            best = Some(QuoteMatch {
                start: mt.start,
                end: mt.end,
                errors: mt.errors,
                score,
            });
        }
    }
    best
}

/// The best approximate match of `quote` in `text`, or `None` if the quote
/// is empty or nothing is within upstream's error budget. Offsets are
/// `char` indices. Upstream `matchQuote`.
pub fn match_quote(text: &str, quote: &str, context: &QuoteContext) -> Option<QuoteMatch> {
    let t: Vec<char> = text.chars().collect();
    let q: Vec<char> = quote.chars().collect();
    let p: Option<Vec<char>> = context.prefix.as_ref().map(|s| s.chars().collect());
    let s: Option<Vec<char>> = context.suffix.as_ref().map(|s| s.chars().collect());
    match_quote_chars(
        &t,
        &q,
        p.as_deref(),
        s.as_deref(),
        context.hint.map(|h| h as i64),
    )
}
