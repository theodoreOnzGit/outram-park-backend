// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "ERIC.js" (translatorID
//   e4660e05-a935-43ec-8eec-df0347362e4c, lastUpdated 2024-07-05 11:56:39).
// Copyright (c) Sebastian Karcher.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The ERIC search translator. NOT YET PORTED (stub).

use crate::zotero::framework::item::JsObject;
#[allow(unused_imports)]
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::search::{SearchContext, SearchError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "e4660e05-a935-43ec-8eec-df0347362e4c",
    label: "ERIC",
    creator: "Sebastian Karcher",
    target: "^https?://(www\\.)?eric\\.ed\\.gov/",
    min_version: "3.0",
    priority: 100,
    translator_type: 12,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2024-07-05 11:56:39",
};

/// `detectSearch`.
pub fn detect_search(_search: &JsObject) -> bool {
    false
}

/// `doSearch`.
pub fn do_search(_ctx: &mut SearchContext, _search: &JsObject) -> Result<(), SearchError> {
    Err(SearchError::Translator("ERIC: not ported".to_owned()))
}
