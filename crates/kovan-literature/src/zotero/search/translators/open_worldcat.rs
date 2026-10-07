// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Open WorldCat.js" (translatorID
//   c73a4a8c-3ef1-4ec8-8229-7531ee384cc4, lastUpdated 2026-08-28 17:44:19).
// Copyright (c) 2022 Simon Kornblith, Sebastian Karcher, and Abe Jellinek.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The Open WorldCat search translator. NOT YET PORTED (stub).

use crate::zotero::framework::item::JsObject;
#[allow(unused_imports)]
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::search::{SearchContext, SearchError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "c73a4a8c-3ef1-4ec8-8229-7531ee384cc4",
    label: "Open WorldCat",
    creator: "Simon Kornblith, Sebastian Karcher, Abe Jellinek",
    target: "^https?://([^/]+\\.)?worldcat\\.org/",
    min_version: "5.0",
    priority: 100,
    translator_type: 12,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2026-08-28 17:44:19",
};

/// `detectSearch`.
pub fn detect_search(_search: &JsObject) -> bool {
    false
}

/// `doSearch`.
pub fn do_search(_ctx: &mut SearchContext, _search: &JsObject) -> Result<(), SearchError> {
    Err(SearchError::Translator("Open WorldCat: not ported".to_owned()))
}
