// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "National Library of Poland ISBN.js" (translatorID
//   aa7f310e-10d3-4209-91dc-88301e7070c6, lastUpdated 2023-06-15 02:45:16).
// Copyright (c) 2023 Maciej Nux Jaros.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).
// Ported: `detectSearch` :37-48, `doSearch` :68-93.

//! The National Library of Poland ISBN search translator: Polish ISBNs
//! (group 83) looked up in the Biblioteka Narodowa's data API as MARCXML,
//! the first `collection > record` imported by MARCXML.

use crate::zotero::framework::identifiers::clean_isbn;
use crate::zotero::framework::item::JsObject;
use crate::zotero::framework::options::TranslatorMetadata;
use crate::zotero::framework::xml::{NodeId, XmlDocument};
use crate::zotero::search::{SearchContext, SearchError};
use crate::zotero::translators::Translator;
use serde_json::Value;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "aa7f310e-10d3-4209-91dc-88301e7070c6",
    label: "National Library of Poland ISBN",
    creator: "Maciej Nux Jaros",
    target: "",
    min_version: "4.0",
    priority: 98,
    translator_type: 8,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2023-06-15 02:45:16",
};

/// `detectSearch` (:37-48): a string ISBN whose digits (spaces and hyphens
/// removed) start with 97883, 97983 or 83.
pub fn detect_search(search: &JsObject) -> bool {
    let Some(Value::String(isbn)) = search.get("ISBN") else {
        return false;
    };
    let isbn: String = isbn.chars().filter(|&c| c != ' ' && c != '-').collect();
    isbn.starts_with("97883") || isbn.starts_with("97983") || isbn.starts_with("83")
}

/// `doSearch` (:68-93).
pub fn do_search(ctx: &mut SearchContext, search: &JsObject) -> Result<(), SearchError> {
    let isbn = search
        .get("ISBN")
        .map(crate::zotero::framework::js::to_js_string)
        .unwrap_or_default();
    let isbn = clean_isbn(&isbn, false).unwrap_or_else(|| "false".to_owned());
    let url = format!("https://data.bn.org.pl/api/institutions/bibs.marcxml?isbnIssn={isbn}");
    let xml_text = ctx.do_get(&url)?;
    let doc = XmlDocument::parse_from_string(&xml_text);
    let Some(record) = collection_record(&doc) else {
        return Ok(());
    };
    let marc_xml = doc.serialize(record);
    for item in ctx.child_import(Translator::MarcXml, &marc_xml)? {
        ctx.complete(item)?;
    }
    Ok(())
}

/// `doc.querySelector('collection > record')`: the first element (tree
/// order) with local name `record` whose parent is an element with local
/// name `collection` (type selectors in an XML document match the local
/// name in any namespace). The framework's `query_selector` has descendant
/// combinators only, hence this helper (also used by LIBRIS ISBN).
pub(crate) fn collection_record(doc: &XmlDocument) -> Option<NodeId> {
    doc.descendants(doc.document()).into_iter().find(|&n| {
        doc.element(n).is_some_and(|e| e.local == "record")
            && doc
                .parent(n)
                .and_then(|p| doc.element(p))
                .is_some_and(|p| p.local == "collection")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_polish_isbns_like_upstream() {
        let s = |v: &str| {
            let mut o = JsObject::new();
            o.set("ISBN", v);
            o
        };
        // The cases in upstream's commented-out test() (:50-66).
        for no in ["123", "978"] {
            assert!(!detect_search(&s(no)));
        }
        for yes in [
            "97883",
            "83",
            "978-83-578",
            "83-123456",
            " 978-83-578",
            " -83-123456",
        ] {
            assert!(detect_search(&s(yes)), "{yes}");
        }
        assert!(!detect_search(&JsObject::new()));
    }
}
