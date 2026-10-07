// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "National Library of Poland ISBN.js" (translatorID
//   aa7f310e-10d3-4209-91dc-88301e7070c6, lastUpdated 2023-06-15 02:45:16).
// Copyright (c) 2023 Maciej Nux Jaros.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The National Library of Poland ISBN search translator. NOT YET PORTED (stub).

use crate::zotero::framework::item::JsObject;
#[allow(unused_imports)]
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::search::{SearchContext, SearchError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "aa7f310e-10d3-4209-91dc-88301e7070c6",
    label: "National Library of Poland ISBN",
    creator: "Maciej Nux Jaros",
    target: "",
    min_version: "4.0",
    priority: 98,
    translator_type: 8,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2023-06-15 02:45:16",
};

/// `detectSearch`.
pub fn detect_search(_search: &JsObject) -> bool {
    false
}

/// `doSearch`.
pub fn do_search(_ctx: &mut SearchContext, _search: &JsObject) -> Result<(), SearchError> {
    Err(SearchError::Translator("National Library of Poland ISBN: not ported".to_owned()))
}
