// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "arXiv.org.js" (translatorID
//   ecddda2e-4fc6-4aea-9f17-ef3b56d7377a, lastUpdated 2026-05-19 15:28:10).
// Copyright (c) 2019 Sean Takats and Michael Berkowitz.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The arXiv.org search translator. NOT YET PORTED (stub).

use crate::zotero::framework::item::JsObject;
#[allow(unused_imports)]
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::search::{SearchContext, SearchError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "ecddda2e-4fc6-4aea-9f17-ef3b56d7377a",
    label: "arXiv.org",
    creator: "Sean Takats and Michael Berkowitz",
    target: "^https?://([^\\.]+\\.)?(arxiv\\.org|xxx\\.lanl\\.gov)/(search|find|catchup|list/\\w|abs/|pdf/)",
    min_version: "6.0",
    priority: 100,
    translator_type: 12,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2026-05-19 15:28:10",
};

/// `detectSearch`.
pub fn detect_search(_search: &JsObject) -> bool {
    false
}

/// `doSearch`.
pub fn do_search(_ctx: &mut SearchContext, _search: &JsObject) -> Result<(), SearchError> {
    Err(SearchError::Translator("arXiv.org: not ported".to_owned()))
}
