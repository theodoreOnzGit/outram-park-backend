// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "ADS Bibcode.js" (translatorID
//   09bd8037-a9bb-4f9a-b3b9-d18b2564b49e, lastUpdated 2025-04-29 03:02:00).
// Copyright (c) 2021 Abe Jellinek.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The ADS Bibcode search translator. NOT YET PORTED (stub).

use crate::zotero::framework::item::JsObject;
#[allow(unused_imports)]
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::search::{SearchContext, SearchError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "09bd8037-a9bb-4f9a-b3b9-d18b2564b49e",
    label: "ADS Bibcode",
    creator: "Abe Jellinek",
    target: "",
    min_version: "6.0",
    priority: 100,
    translator_type: 8,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2025-04-29 03:02:00",
};

/// `detectSearch`.
pub fn detect_search(_search: &JsObject) -> bool {
    false
}

/// `doSearch`.
pub fn do_search(_ctx: &mut SearchContext, _search: &JsObject) -> Result<(), SearchError> {
    Err(SearchError::Translator("ADS Bibcode: not ported".to_owned()))
}
