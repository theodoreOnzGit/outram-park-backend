// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "K10plus ISBN.js" (translatorID
//   de0eef58-cb39-4410-ada0-6b39f43383f9, lastUpdated 2024-10-09 14:20:54).
// Copyright (c) 2015 Philipp Zumstein.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The K10plus ISBN search translator. NOT YET PORTED (stub).

use crate::zotero::framework::item::JsObject;
#[allow(unused_imports)]
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::search::{SearchContext, SearchError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "de0eef58-cb39-4410-ada0-6b39f43383f9",
    label: "K10plus ISBN",
    creator: "Philipp Zumstein",
    target: "",
    min_version: "4.0",
    priority: 99,
    translator_type: 8,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2024-10-09 14:20:54",
};

/// `detectSearch`.
pub fn detect_search(_search: &JsObject) -> bool {
    false
}

/// `doSearch`.
pub fn do_search(_ctx: &mut SearchContext, _search: &JsObject) -> Result<(), SearchError> {
    Err(SearchError::Translator("K10plus ISBN: not ported".to_owned()))
}
