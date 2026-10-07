// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Camara Brasileira do Livro ISBN.js" (translatorID
//   cdb5c893-ab69-4e96-9b5c-f4456d49ddd8, lastUpdated 2023-09-26 16:11:18).
// Copyright (c) 2023 Abe Jellinek.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The Câmara Brasileira do Livro ISBN search translator. NOT YET PORTED (stub).

use crate::zotero::framework::item::JsObject;
#[allow(unused_imports)]
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::search::{SearchContext, SearchError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "cdb5c893-ab69-4e96-9b5c-f4456d49ddd8",
    label: "Câmara Brasileira do Livro ISBN",
    creator: "Abe Jellinek",
    target: "",
    min_version: "5.0",
    priority: 98,
    translator_type: 8,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2023-09-26 16:11:18",
};

/// `detectSearch`.
pub fn detect_search(_search: &JsObject) -> bool {
    false
}

/// `doSearch`.
pub fn do_search(_ctx: &mut SearchContext, _search: &JsObject) -> Result<(), SearchError> {
    Err(SearchError::Translator("Câmara Brasileira do Livro ISBN: not ported".to_owned()))
}
