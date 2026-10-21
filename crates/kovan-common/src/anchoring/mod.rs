// Port of the Hypothesis client's annotation anchoring and of the
// approx-string-match library it uses (GitHub #754, 2026-10-07).
//   Hypothesis client: https://github.com/hypothesis/client, commit
//     b4d085a2f893aa6de3b61d8b8bc3ae4d0f24fc1a, (c) 2013-2019 Hypothes.is
//     Project and contributors, BSD-2-Clause.
//   approx-string-match: https://github.com/robertknight/approx-string-match-js,
//     v2.0.0, commit fe814eba4d6b6daf88d38179331a14d156a0b5a0, (c) 2020
//     Robert Knight, MIT.
//   Both licences were read first-hand (LICENSE and package.json) and are
//   reproduced in this crate's NOTICE. This port is AGPL-3.0-only.
//
//! # Robust annotation anchoring (GitHub #754)
//!
//! Attach a note to a span of text so that it **survives edits to that
//! text**, the way Hypothesis keeps web and PDF annotations attached. An
//! annotation stores several [`Selector`]s in the W3C Web Annotation shape:
//!
//! - [`TextQuoteSelector`]: the exact text plus 32 characters either side;
//! - [`TextPositionSelector`]: `char` offsets;
//! - [`PageSelector`]: the page, for paged text (Hypothesis's structural
//!   selector for PDFs).
//!
//! **Describing** ([`describe`], [`describe_in_pages`]) makes them from a
//! text and a range. **Anchoring** ([`anchor`], [`anchor_in_pages`]) finds
//! the range again in a possibly-changed text, trying, in upstream's order:
//!
//! 1. the position, accepted only if the text there still equals the quote
//!    ([`AnchorStrategy::Position`]);
//! 2. exact occurrences of the quote, ranked by context and nearness to the
//!    old position ([`AnchorStrategy::ExactQuote`]);
//! 3. the fuzzy (Myers bit-parallel edit-distance) search, at most
//!    `min(256, quote/2)` edits, ranked the same way
//!    ([`AnchorStrategy::FuzzyQuote`]).
//!
//! If none applies the annotation is [`Anchoring::Orphaned`]. Every match
//! carries upstream's score in [0, 1] (50 quote, 20 prefix, 20 suffix,
//! 2 position, normalised; see [`match_quote`](mod@match_quote)). Upstream applies no
//! score threshold and neither does this port: callers that want one (for
//! example "flag for review below 0.75") apply it to [`Anchor::score`].
//!
//! Paged text ([`anchor_in_pages`]) compares quotes **ignoring whitespace**,
//! as upstream's PDF path does, since re-extraction moves spaces and line
//! breaks. Plain text ([`anchor`]) does not, as upstream's HTML path.
//!
//! Units: all offsets are Unicode scalar values (`char`s), per the W3C
//! model; Hypothesis itself counts UTF-16 code units ([`selector`](crate::anchoring::selector) doc).
//!
//! Pure `std` + `serde`; no I/O. Nothing in kovan uses it yet: wiring it
//! into review stamps, `[[relation]]` anchors or PDF highlights is a
//! separate maintainer decision (#754).

pub mod approx_match;
pub mod match_quote;
pub mod offsets;
pub mod pages;
pub mod selector;
pub mod text;

#[cfg(test)]
mod tests;

pub use match_quote::{match_quote, QuoteContext, QuoteMatch};
pub use pages::{anchor_in_pages, describe_in_pages};
pub use selector::{PageSelector, Selector, TextPositionSelector, TextQuoteSelector};
pub use text::{anchor, describe, describe_quote, lines_to_range, range_to_lines};

/// How an anchor was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorStrategy {
    /// The position selector, with the text there equal to the quote.
    Position,
    /// The position selector, with no quote to check it against.
    PositionUnverified,
    /// An exact occurrence of the quote (for paged text: exact once
    /// whitespace is ignored).
    ExactQuote,
    /// An approximate occurrence of the quote.
    FuzzyQuote,
}

/// Where an annotation anchored.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Anchor {
    /// The page, for [`anchor_in_pages`]; `None` for [`anchor`].
    pub page: Option<usize>,
    /// Start (`char` offset; within the page for paged text).
    pub start: usize,
    /// End, exclusive.
    pub end: usize,
    /// Which step found it.
    pub strategy: AnchorStrategy,
    /// Upstream's match score in [0, 1]; 1.0 for a verified position.
    pub score: f64,
}

/// Why an annotation could not be anchored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrphanReason {
    /// No selector this module can anchor (for paged text: no quote).
    NoUsableSelector,
    /// The selectors were usable but nothing in the text matches them.
    NotFound,
}

/// The outcome of anchoring.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Anchoring {
    /// Found, with how and how well.
    Anchored(Anchor),
    /// Could not be anchored; the annotation is orphaned.
    Orphaned(OrphanReason),
}

impl Anchoring {
    /// The anchor, if any.
    pub fn anchor(&self) -> Option<&Anchor> {
        match self {
            Anchoring::Anchored(a) => Some(a),
            Anchoring::Orphaned(_) => None,
        }
    }
}
