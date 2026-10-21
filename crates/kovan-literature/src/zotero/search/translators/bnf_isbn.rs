// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "BnF ISBN.js" (translatorID
//   f349954c-9957-4b5f-be24-1a8bb52f7fbd, lastUpdated 2021-07-30 21:23:00).
// Copyright (c) 2021 Abe Jellinek.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).
// Ported: `ns` :39-42, `detectSearch` :44-46, `doSearch` :48-87.

//! The BnF ISBN search translator: an SRU query to the Bibliothèque
//! nationale de France catalogue; each MarcXchange record is re-labelled as
//! MARCXML and imported by MARCXML.
//!
//! Kept from upstream: both XPaths inside the record loop start with `//`,
//! so they search the whole response, not the record (with several records,
//! every record is imported once per record).

use crate::zotero::framework::identifiers::clean_isbn;
use crate::zotero::framework::item::JsObject;
use crate::zotero::framework::js;
use crate::zotero::framework::options::TranslatorMetadata;
use crate::zotero::framework::xml::{XNode, XmlDocument};
use crate::zotero::framework::xpath::{xpath, xpath_text};
use crate::zotero::search::{SearchContext, SearchError};
use crate::zotero::translators::Translator;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "f349954c-9957-4b5f-be24-1a8bb52f7fbd",
    label: "BnF ISBN",
    creator: "Abe Jellinek",
    target: "",
    min_version: "4.0",
    priority: 98,
    translator_type: 8,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2021-07-30 21:23:00",
};

const NS: [(&str, &str); 2] = [
    ("srw", "http://www.loc.gov/zing/srw/"),
    ("mxc", "info:lc/xmlns/marcxchange-v2"),
];

/// `detectSearch` (:44-46): `!!item.ISBN`.
pub fn detect_search(search: &JsObject) -> bool {
    search.truthy("ISBN")
}

/// `doSearch` (:48-87).
pub fn do_search(ctx: &mut SearchContext, search: &JsObject) -> Result<(), SearchError> {
    let isbn = search.get("ISBN").map(js::to_js_string).unwrap_or_default();
    let isbn = clean_isbn(&isbn, false).unwrap_or_else(|| "false".to_owned());
    let url = format!(
        "https://catalogue.bnf.fr/api/SRU?version=1.2&operation=searchRetrieve&query=bib.isbn%20all%20%22{isbn}%22"
    );
    let xml_text = ctx.do_get(&url)?;
    let mut xml = XmlDocument::parse_from_string(&xml_text);
    let Some(root) = xml.document_element() else {
        // ZU.xpath(null, ...) throws.
        return Err(SearchError::Translator(
            "ZU.xpath: no document element".to_owned(),
        ));
    };
    let records = xpath(
        &xml,
        root,
        "/srw:searchRetrieveResponse/srw:records/srw:record",
        &NS,
    )?;
    for record in records {
        if xpath_text(&xml, record, "//srw:recordSchema", &NS, None)?.as_deref()
            != Some("marcxchange")
        {
            continue;
        }
        let marc_records = xpath(&xml, record, "//srw:recordData/mxc:record", &NS)?;
        for marc_record in marc_records {
            let XNode::Node(n) = marc_record else {
                continue;
            };
            xml.set_attribute(n, "xmlns:marc", "http://www.loc.gov/MARC21/slim");
            let marcxchange_text = xml.serialize(n);
            let marc_xml_text = marcxchange_text
                .replace("<mxc:", "<marc:")
                .replace("</mxc:", "</marc:");
            for item in ctx.child_import(Translator::MarcXml, &marc_xml_text)? {
                ctx.complete(item)?;
            }
        }
    }
    Ok(())
}
