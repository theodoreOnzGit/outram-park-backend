// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "K10plus ISBN.js" (translatorID
//   de0eef58-cb39-4410-ada0-6b39f43383f9, lastUpdated 2024-10-09 14:20:54).
// Copyright (c) 2015 Philipp Zumstein.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).
// Ported: `detectSearch` :37-39, `doSearch` :41-107 (with its MARCXML
//   `itemDone` handler).

//! The K10plus ISBN search translator: an SRU query to K10plus (the merged
//! GBV and SWB catalogue) for MARCXML, imported by MARCXML, with a table of
//! contents link, the queried ISBN first, no call number and no tags.

use crate::zotero::framework::identifiers::clean_isbn;
use crate::zotero::framework::item::{JsObject, TranslatorItem};
use crate::zotero::framework::js;
use crate::zotero::framework::options::TranslatorMetadata;
use crate::zotero::framework::xml::XmlDocument;
use crate::zotero::framework::xpath::xpath;
use crate::zotero::search::http::encode_uri_component;
use crate::zotero::search::{SearchContext, SearchError};
use crate::zotero::translators::Translator;
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

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

/// `detectSearch` (:37-39): `!!item.ISBN`.
pub fn detect_search(search: &JsObject) -> bool {
    search.truthy("ISBN")
}

/// `doSearch` (:41-107).
pub fn do_search(ctx: &mut SearchContext, search: &JsObject) -> Result<(), SearchError> {
    // `queryISBN` is `var`-scoped: undefined in the query branch, `false`
    // when cleanISBN finds nothing; the handler concatenates it either way.
    let mut query_isbn = "undefined".to_owned();
    let url = if search.truthy("ISBN") {
        let isbn = search.get("ISBN").map(js::to_js_string).unwrap_or_default();
        query_isbn = clean_isbn(&isbn, false).unwrap_or_else(|| "false".to_owned());
        format!("https://sru.k10plus.de/opac-de-627?version=1.1&operation=searchRetrieve&query=pica.isb={query_isbn}&maximumRecords=1")
    } else if search.truthy("query") {
        let q = search
            .get("query")
            .map(js::to_js_string)
            .unwrap_or_default();
        format!(
            "https://sru.k10plus.de/opac-de-627?version=1.1&operation=searchRetrieve&query={}&maximumRecords=50",
            encode_uri_component(&q)
        )
    } else {
        return Err(SearchError::Translator("Invalid URL: undefined".to_owned()));
    };
    let text = ctx.do_get(&url)?;
    for item in ctx.child_import(Translator::MarcXml, &text)? {
        let item = item_done_handler(item, &text, &query_isbn)?;
        ctx.complete(item)?;
    }
    Ok(())
}

/// The MARCXML `itemDone` handler (:55-101).
fn item_done_handler(
    mut item: TranslatorItem,
    text: &str,
    query_isbn: &str,
) -> Result<TranslatorItem, SearchError> {
    let xml = XmlDocument::parse_from_string(text);
    let ns = [("marc", "http://www.loc.gov/MARC21/slim")];
    let toc = xpath(
        &xml,
        xml.document(),
        r#"//marc:datafield[@tag="856"][ marc:subfield[text()="Inhaltsverzeichnis"] ]/marc:subfield[@code="u"]"#,
        &ns,
    )?;
    if let Some(&first) = toc.first() {
        let mut url = xml.text_content(first).unwrap_or_default();
        if let Some(rest) = url.strip_prefix("http://") {
            url = format!("https://{rest}");
        }
        let mut a = JsObject::new();
        a.set("url", url);
        a.set("title", "Table of Contents PDF");
        a.set("mimeType", "application/pdf");
        item.attachments = vec![a];
    }
    if item.truthy("place") {
        static R: OnceLock<Regex> = OnceLock::new();
        let re = R.get_or_init(|| {
            // /\[?u\.[\s ]?a\.\]?\s*$/ (JavaScript \s already has U+00A0).
            Regex::new(&format!(r"\[?u\.{ws}?a\.\]?{ws}*$", ws = js::WS)).expect("static regex")
        });
        let place = item.get_string("place").unwrap_or_default();
        item.set("place", re.replace(&place, "").into_owned());
    }
    item.set("callNumber", "");
    let isbn = item
        .get("ISBN")
        .map_or_else(|| "undefined".to_owned(), js::to_js_string);
    item.set("ISBN", Value::String(format!("{query_isbn} {isbn}")));
    item.tags.clear();
    Ok(item)
}
