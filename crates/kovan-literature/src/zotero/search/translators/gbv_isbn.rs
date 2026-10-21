// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Gemeinsamer Bibliotheksverbund ISBN.js"
//   (translatorID b7259652-640b-43f1-94e5-18e2f2268463, lastUpdated
//   2021-06-30 03:15:48): `detectSearch` :19-21, `doSearch` :23 (the whole
//   file).
// Copyright (c) Philipp Zumstein.
// Licence: AGPL-3.0 (no licence text upstream; treated as AGPLv3 as part
//   of Zotero, maintainer decision 2026-10-07, #747).

//! The Gemeinsamer Bibliotheksverbund ISBN search translator: upstream's
//! placeholder after the translator was renamed K10plus ISBN; it detects
//! nothing and does nothing.

use crate::zotero::framework::item::JsObject;
use crate::zotero::framework::options::TranslatorMetadata;
use crate::zotero::search::{SearchContext, SearchError};

/// The translator header. Upstream's `target` and `maxVersion` are `null`.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "b7259652-640b-43f1-94e5-18e2f2268463",
    label: "Gemeinsamer Bibliotheksverbund ISBN",
    creator: "Philipp Zumstein",
    target: "",
    min_version: "4.0",
    priority: 99,
    translator_type: 8,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2021-06-30 03:15:48",
};

/// `detectSearch` (:19-21): always false.
pub fn detect_search(_search: &JsObject) -> bool {
    false
}

/// `doSearch` (:23): nothing.
pub fn do_search(_ctx: &mut SearchContext, _search: &JsObject) -> Result<(), SearchError> {
    Ok(())
}
