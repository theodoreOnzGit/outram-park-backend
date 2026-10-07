// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Lulu.js" (translatorID
//   9a0ecbda-c0e9-4a19-84a9-fc8e7c845afa, lastUpdated 2024-10-24 19:22:51).
// Copyright (c) Aurimas Vinckevicius.
// Licence: AGPL-3.0 (no licence text upstream; treated as AGPLv3 as part of Zotero, maintainer decision 2026-10-07, #747).

//! The Lulu search translator. NOT YET PORTED (stub).

use crate::zotero::framework::item::JsObject;
#[allow(unused_imports)]
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::search::{SearchContext, SearchError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "9a0ecbda-c0e9-4a19-84a9-fc8e7c845afa",
    label: "Lulu",
    creator: "Aurimas Vinckevicius",
    target: "^https?://www\\.lulu\\.com/shop/",
    min_version: "3.0",
    priority: 101,
    translator_type: 12,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2024-10-24 19:22:51",
};

/// `detectSearch`.
pub fn detect_search(_search: &JsObject) -> bool {
    false
}

/// `doSearch`.
pub fn do_search(_ctx: &mut SearchContext, _search: &JsObject) -> Result<(), SearchError> {
    Err(SearchError::Translator("Lulu: not ported".to_owned()))
}
