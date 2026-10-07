// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "LIBRIS ISBN.js" (translatorID
//   5f506a9a-8076-4e1e-950c-f55d32003aae, lastUpdated 2024-04-23 18:44:30).
// Copyright (c) 2023 Sebastian Berlin.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).
// Ported: `detectSearch` :37-45, `doSearch` :47-67.

//! The LIBRIS ISBN search translator: Swedish ISBNs (group 91) looked up
//! in LIBRIS xsearch as MARCXML, the first `collection > record` imported
//! by MARCXML, without the "Bok" tag.

use super::nlp_isbn::collection_record;
use crate::zotero::framework::identifiers::clean_isbn;
use crate::zotero::framework::item::JsObject;
use crate::zotero::framework::options::TranslatorMetadata;
use crate::zotero::framework::xml::XmlDocument;
use crate::zotero::search::http::RequestOptions;
use crate::zotero::search::{SearchContext, SearchError};
use crate::zotero::translators::Translator;
use serde_json::Value;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "5f506a9a-8076-4e1e-950c-f55d32003aae",
    label: "LIBRIS ISBN",
    creator: "Sebastian Berlin",
    target: "",
    min_version: "5.0",
    priority: 98,
    translator_type: 8,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2024-04-23 18:44:30",
};

/// `detectSearch` (:37-45): a string ISBN whose cleaned form matches
/// `/^(97[8-9])?91/` (`cleanISBN` giving `false` tests as "false").
pub fn detect_search(search: &JsObject) -> bool {
    let Some(Value::String(isbn)) = search.get("ISBN") else {
        return false;
    };
    let Some(isbn) = clean_isbn(isbn, false) else {
        return false;
    };
    isbn.starts_with("91") || isbn.starts_with("97891") || isbn.starts_with("97991")
}

/// `doSearch` (:47-67).
pub fn do_search(ctx: &mut SearchContext, search: &JsObject) -> Result<(), SearchError> {
    let isbn = search
        .get("ISBN")
        .map(crate::zotero::framework::js::to_js_string)
        .unwrap_or_default();
    let isbn = clean_isbn(&isbn, false).unwrap_or_else(|| "false".to_owned());
    let url = format!("http://libris.kb.se/xsearch?query=ISBN:{isbn}");
    let xml_text = ctx.request_text(&url, &RequestOptions::default())?;
    let doc = XmlDocument::parse_from_string(&xml_text);
    let Some(record) = collection_record(&doc) else {
        return Ok(());
    };
    let marc_xml = doc.serialize(record);
    for mut item in ctx.child_import(Translator::MarcXml, &marc_xml)? {
        // `item.tags.filter(tag => (tag.tag || tag) !== 'Bok')`
        item.tags.retain(|t| t.tag != "Bok");
        ctx.complete(item)?;
    }
    Ok(())
}
