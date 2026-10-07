// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Library of Congress ISBN.js" (translatorID
//   c070e5a2-4bfd-44bb-9b3c-4be20c50d0d9, lastUpdated 2023-08-25 05:06:07):
//   `detectSearch` :14-20, `doSearch` :23-43.
// Copyright (c) Sebastian Karcher.
// Licence: AGPL-3.0 (no licence text upstream; treated as AGPLv3 as part
//   of Zotero, maintainer decision 2026-10-07, #747).

//! The Library of Congress ISBN search translator: an SRU query to the
//! Library of Congress catalogue for MARCXML, imported by MARCXML.

use crate::zotero::framework::item::JsObject;
use crate::zotero::framework::options::TranslatorMetadata;
use crate::zotero::search::http::encode_uri_component;
use crate::zotero::search::{SearchContext, SearchError};
use crate::zotero::framework::identifiers::clean_isbn;
use crate::zotero::framework::js;
use crate::zotero::translators::Translator;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "c070e5a2-4bfd-44bb-9b3c-4be20c50d0d9",
    label: "Library of Congress ISBN",
    creator: "Sebastian Karcher",
    target: "",
    min_version: "3.0.9",
    priority: 97,
    translator_type: 8,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2023-08-25 05:06:07",
};

/// `detectSearch` (:14-20): `!!item.ISBN`.
pub fn detect_search(search: &JsObject) -> bool {
    search.truthy("ISBN")
}

/// `doSearch` (:23-43).
pub fn do_search(ctx: &mut SearchContext, search: &JsObject) -> Result<(), SearchError> {
    let url = if search.truthy("ISBN") {
        let isbn = search.get("ISBN").map(js::to_js_string).unwrap_or_default();
        // `"..." + ZU.cleanISBN(item.ISBN)`: `false` concatenates as "false".
        let clean = clean_isbn(&isbn, false).unwrap_or_else(|| "false".to_owned());
        format!("https://lx2.loc.gov/sru/lcdb?operation=searchRetrieve&version=1.1&query=bath.ISBN=^{clean}&maximumRecords=1")
    } else if search.truthy("query") {
        let q = search.get("query").map(js::to_js_string).unwrap_or_default();
        format!(
            "https://lx2.loc.gov/sru/lcdb?operation=searchRetrieve&version=1.1&query={}&maximumRecords=50",
            encode_uri_component(&q)
        )
    } else {
        // `ZU.doGet(undefined, ...)`: resolveURL(undefined) throws.
        return Err(SearchError::Translator("Invalid URL: undefined".to_owned()));
    };
    let text = ctx.do_get(&url)?;
    for item in ctx.child_import(Translator::MarcXml, &text)? {
        ctx.complete(item)?;
    }
    Ok(())
}
