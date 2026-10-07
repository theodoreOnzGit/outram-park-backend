// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Crossref REST.js" (translatorID
//   0a61e167-de9a-4f93-a68a-628b48855909, lastUpdated 2025-08-03 05:38:26).
// Copyright (c) 2018.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The Crossref REST search translator. NOT YET PORTED (stub).

use crate::zotero::framework::item::JsObject;
#[allow(unused_imports)]
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::search::{SearchContext, SearchError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "0a61e167-de9a-4f93-a68a-628b48855909",
    label: "Crossref REST",
    creator: "Martynas Bagdonas",
    target: "",
    min_version: "5.0.0",
    priority: 90,
    translator_type: 8,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2025-08-03 05:38:26",
};

/// `detectSearch`.
pub fn detect_search(_search: &JsObject) -> bool {
    false
}

/// `doSearch`.
pub fn do_search(_ctx: &mut SearchContext, _search: &JsObject) -> Result<(), SearchError> {
    Err(SearchError::Translator("Crossref REST: not ported".to_owned()))
}
