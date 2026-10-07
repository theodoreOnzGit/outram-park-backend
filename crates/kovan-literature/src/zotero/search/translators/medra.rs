// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "mEDRA.js" (translatorID
//   d9b57cd5-5a9c-4946-8616-3bdf8edfcbb5, lastUpdated 2025-07-14 17:55:30).
// Copyright (c) Aurimas Vinckevicius.
// Licence: AGPL-3.0 (no licence text upstream; treated as AGPLv3 as part of Zotero, maintainer decision 2026-10-07, #747).

//! The mEDRA search translator. NOT YET PORTED (stub).

use crate::zotero::framework::item::JsObject;
#[allow(unused_imports)]
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::search::{SearchContext, SearchError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "d9b57cd5-5a9c-4946-8616-3bdf8edfcbb5",
    label: "mEDRA",
    creator: "Aurimas Vinckevicius",
    target: "^https?://www\\.medra\\.org/servlet/view\\?",
    min_version: "3.0",
    priority: 105,
    translator_type: 12,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2025-07-14 17:55:30",
};

/// `detectSearch`.
pub fn detect_search(_search: &JsObject) -> bool {
    false
}

/// `doSearch`.
pub fn do_search(_ctx: &mut SearchContext, _search: &JsObject) -> Result<(), SearchError> {
    Err(SearchError::Translator("mEDRA: not ported".to_owned()))
}
