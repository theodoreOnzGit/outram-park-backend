// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "LIBRIS ISBN.js" (translatorID
//   5f506a9a-8076-4e1e-950c-f55d32003aae, lastUpdated 2024-04-23 18:44:30).
// Copyright (c) 2023 Sebastian Berlin.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The LIBRIS ISBN search translator. NOT YET PORTED (stub).

use crate::zotero::framework::item::JsObject;
#[allow(unused_imports)]
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::search::{SearchContext, SearchError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "5f506a9a-8076-4e1e-950c-f55d32003aae",
    label: "LIBRIS ISBN",
    creator: "Sebastian Berlin",
    target: "",
    min_version: "5.0",
    priority: 98,
    translator_type: 8,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2024-04-23 18:44:30",
};

/// `detectSearch`.
pub fn detect_search(_search: &JsObject) -> bool {
    false
}

/// `doSearch`.
pub fn do_search(_ctx: &mut SearchContext, _search: &JsObject) -> Result<(), SearchError> {
    Err(SearchError::Translator("LIBRIS ISBN: not ported".to_owned()))
}
