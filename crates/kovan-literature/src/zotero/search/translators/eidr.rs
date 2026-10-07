// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "EIDR.js" (translatorID
//   79c3d292-0afc-42a1-bd86-7e706fc35aa5, lastUpdated 2017-06-03 11:41:00).
// Copyright (c) Aurimas Vinckevicius.
// Licence: AGPL-3.0 (no licence text upstream; treated as AGPLv3 as part of Zotero, maintainer decision 2026-10-07, #747).

//! The EIDR search translator. NOT YET PORTED (stub).

use crate::zotero::framework::item::JsObject;
#[allow(unused_imports)]
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::search::{SearchContext, SearchError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "79c3d292-0afc-42a1-bd86-7e706fc35aa5",
    label: "EIDR",
    creator: "Aurimas Vinckevicius",
    target: "",
    min_version: "1.0",
    priority: 80,
    translator_type: 8,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2017-06-03 11:41:00",
};

/// `detectSearch`.
pub fn detect_search(_search: &JsObject) -> bool {
    false
}

/// `doSearch`.
pub fn do_search(_ctx: &mut SearchContext, _search: &JsObject) -> Result<(), SearchError> {
    Err(SearchError::Translator("EIDR: not ported".to_owned()))
}
