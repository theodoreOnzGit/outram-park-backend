// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "WHO.js" (translatorID
//   cd587058-6125-4b33-a876-8c6aae48b5e8, lastUpdated 2022-12-06 12:21:28).
// Copyright (c) 2018-2021 Mario Trojan and Abe Jellinek.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The WHO search translator. NOT YET PORTED (stub).

use crate::zotero::framework::item::JsObject;
#[allow(unused_imports)]
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::search::{SearchContext, SearchError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "cd587058-6125-4b33-a876-8c6aae48b5e8",
    label: "WHO",
    creator: "Mario Trojan, Philipp Zumstein, and Abe Jellinek",
    target: "^https?://apps\\.who\\.int/iris/",
    min_version: "3.0",
    priority: 96,
    translator_type: 12,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2022-12-06 12:21:28",
};

/// `detectSearch`.
pub fn detect_search(_search: &JsObject) -> bool {
    false
}

/// `doSearch`.
pub fn do_search(_ctx: &mut SearchContext, _search: &JsObject) -> Result<(), SearchError> {
    Err(SearchError::Translator("WHO: not ported".to_owned()))
}
