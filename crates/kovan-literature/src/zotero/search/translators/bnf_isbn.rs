// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "BnF ISBN.js" (translatorID
//   f349954c-9957-4b5f-be24-1a8bb52f7fbd, lastUpdated 2021-07-30 21:23:00).
// Copyright (c) 2021 Abe Jellinek.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The BnF ISBN search translator. NOT YET PORTED (stub).

use crate::zotero::framework::item::JsObject;
#[allow(unused_imports)]
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::search::{SearchContext, SearchError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "f349954c-9957-4b5f-be24-1a8bb52f7fbd",
    label: "BnF ISBN",
    creator: "Abe Jellinek",
    target: "",
    min_version: "4.0",
    priority: 98,
    translator_type: 8,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2021-07-30 21:23:00",
};

/// `detectSearch`.
pub fn detect_search(_search: &JsObject) -> bool {
    false
}

/// `doSearch`.
pub fn do_search(_ctx: &mut SearchContext, _search: &JsObject) -> Result<(), SearchError> {
    Err(SearchError::Translator("BnF ISBN: not ported".to_owned()))
}
