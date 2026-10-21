// Selector shapes follow the W3C Web Annotation Data Model
// (https://www.w3.org/TR/annotation-model/, §4.2.4 Text Quote Selector and
// §4.2.5 Text Position Selector) and, for `PageSelector`, the Hypothesis
// client's `src/types/api.ts` (commit b4d085a2f893aa6de3b61d8b8bc3ae4d0f24fc1a,
// (c) 2013-2019 Hypothes.is Project and contributors, BSD-2-Clause; see
// NOTICE). This file is distributed under AGPL-3.0-only.
//
//! The selectors an annotation carries, in the W3C JSON shape
//! (`{"type": "TextQuoteSelector", "exact": …, "prefix": …, "suffix": …}`).
//!
//! **Taken:** `TextQuoteSelector`, `TextPositionSelector` (W3C) and
//! `PageSelector` (Hypothesis's structural selector for paged documents:
//! 0-based `index` plus an optional printed `label`).
//!
//! **Left out**, as they describe a DOM or media and not plain text, lines
//! of code or pages of extracted text: `RangeSelector` (XPath into a DOM),
//! `EPUBContentSelector`, `MediaTimeSelector` and `ShapeSelector`. A
//! selector of a type this module does not know deserialises as
//! [`Selector::Unsupported`] and is ignored by the anchoring, as upstream
//! ignores types it has no anchor for; it cannot be serialised back.
//!
//! **Offsets are Unicode scalar values (`char`s)**, as the W3C model
//! specifies ("the selection of the text MUST be in terms of unicode code
//! points"), not UTF-16 code units as Hypothesis stores them. The two agree
//! on any text without characters outside the Basic Multilingual Plane.

use serde::{Deserialize, Serialize};

/// W3C `TextQuoteSelector`: the exact text, with some text before and after
/// it to tell repeated occurrences apart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextQuoteSelector {
    /// The selected text, exactly.
    pub exact: String,
    /// Text immediately before `exact` (Hypothesis takes 32 characters).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    /// Text immediately after `exact` (32 characters).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,
}

/// W3C `TextPositionSelector`: `[start, end)` in `char`s from the start of
/// the text (for paged text, from the start of the first page).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextPositionSelector {
    /// Offset of the first selected character.
    pub start: usize,
    /// Offset one past the last selected character.
    pub end: usize,
}

/// Hypothesis's `PageSelector`: the page of a paged document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageSelector {
    /// 0-based index of the page in the document's page sequence.
    pub index: usize,
    /// The printed page number, or the 1-based page number when the pages
    /// carry none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// One selector, tagged by `type` as in the W3C JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Selector {
    TextQuoteSelector(TextQuoteSelector),
    TextPositionSelector(TextPositionSelector),
    PageSelector(PageSelector),
    /// Any other `type` (module doc). Ignored when anchoring; serialising it
    /// is an error, because what it held was not kept.
    #[serde(other, skip_serializing)]
    Unsupported,
}

impl Selector {
    /// The first quote selector in `selectors`.
    pub fn quote(selectors: &[Selector]) -> Option<&TextQuoteSelector> {
        selectors.iter().find_map(|s| match s {
            Selector::TextQuoteSelector(q) => Some(q),
            _ => None,
        })
    }

    /// The first position selector in `selectors`.
    pub fn position(selectors: &[Selector]) -> Option<&TextPositionSelector> {
        selectors.iter().find_map(|s| match s {
            Selector::TextPositionSelector(p) => Some(p),
            _ => None,
        })
    }

    /// The first page selector in `selectors`.
    pub fn page(selectors: &[Selector]) -> Option<&PageSelector> {
        selectors.iter().find_map(|s| match s {
            Selector::PageSelector(p) => Some(p),
            _ => None,
        })
    }
}
