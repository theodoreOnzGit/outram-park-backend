// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "PubMed.js" (translatorID
//   3d0231ce-fd4b-478c-b1d3-840389e5b68c, lastUpdated 2023-01-10 14:01:49).
// Copyright (c) 2015 Philipp Zumstein.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The PubMed search translator. NOT YET PORTED (stub).

use crate::zotero::framework::item::JsObject;
#[allow(unused_imports)]
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::search::{SearchContext, SearchError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "3d0231ce-fd4b-478c-b1d3-840389e5b68c",
    label: "PubMed",
    creator: "Philipp Zumstein",
    target: "^https?://([^/]+\\.)?(www|preview)\\.ncbi\\.nlm\\.nih\\.gov[^/]*/(m/)?(books|pubmed|labs/pubmed|myncbi|sites/pubmed|sites/entrez|entrez/query\\.fcgi\\?.*db=PubMed|myncbi/browse/collection/?|myncbi/collections/)|^https?://pubmed\\.ncbi\\.nlm\\.nih\\.gov/(\\d|\\?|searches/|clipboard|collections/)",
    min_version: "3.0",
    priority: 100,
    translator_type: 12,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2023-01-10 14:01:49",
};

/// `detectSearch`.
pub fn detect_search(_search: &JsObject) -> bool {
    false
}

/// `doSearch`.
pub fn do_search(_ctx: &mut SearchContext, _search: &JsObject) -> Result<(), SearchError> {
    Err(SearchError::Translator("PubMed: not ported".to_owned()))
}
