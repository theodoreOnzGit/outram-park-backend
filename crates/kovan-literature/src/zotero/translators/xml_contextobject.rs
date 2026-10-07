// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "XML ContextObject.js" (translatorID
//   24d9f058-3eb3-4d70-b78f-1ba1aef2128d, lastUpdated 2015-05-20 00:05:55):
//   `detectImport` :58-68, `doImport` :71-89, `contextObjectXMLToCOinS`
//   :95-147, `mapXMLtoKEV` :162-169.
// Copyright (c) 2010 Avram Lyon, 2013 Sebastian Karcher.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The XML ContextObject translator (import): OpenURL ContextObjects in
//! XML, turned into key-encoded-value strings (COinS titles) and read with
//! `ZU.parseContextObject`
//! ([`crate::zotero::framework::openurl::parse_context_object`]).

use crate::zotero::framework::openurl::{encode_uri_component, parse_context_object};
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::xml::{get_xml, XmlDocument};
use crate::zotero::framework::xpath::{xpath, xpath_text};
use crate::zotero::framework::{ImportContext, TranslateError, TranslatorItem};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "24d9f058-3eb3-4d70-b78f-1ba1aef2128d",
    label: "XML ContextObject",
    creator: "Avram Lyon and Simon Kornblith",
    target: "ctx",
    min_version: "3.0",
    priority: 100,
    translator_type: translator_type::IMPORT,
    config_options: &[("dataMode", HeaderValue::Str("xml/dom"))],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2015-05-20 00:05:55",
};

/// `detectImport` (:58-68): the ctx namespace in one of the first 100
/// lines.
pub fn detect_import(ctx: &mut ImportContext) -> bool {
    let mut i = 0;
    while i < 100 {
        let Some(line) = ctx.read_line() else {
            break;
        };
        if line.contains("info:ofi/fmt:xml:xsd:ctx") {
            return true;
        }
        i += 1;
    }
    false
}

/// `doImport` (:71-89).
pub fn do_import(ctx: &mut ImportContext) -> Result<(), TranslateError> {
    let doc = get_xml(ctx)?;
    for span in context_object_xml_to_coins(&doc)? {
        let mut item = TranslatorItem::new("");
        if parse_context_object(&span, &mut item)? {
            // :81-83
            if item.truthy("journalAbbreviation") && !item.truthy("publicationTitle") {
                let j = item.get("journalAbbreviation").cloned().unwrap_or_default();
                item.set("publicationTitle", j);
            }
            ctx.item_done(item);
        }
    }
    Ok(())
}

const NS: &[(&str, &str)] = &[
    ("xsi", "http://www.w3.org/2001/XMLSchema-instance"),
    ("ctx", "info:ofi/fmt:xml:xsd:ctx"),
    ("rft", "info:ofi/fmt:xml:xsd:journal"),
];

/// `mapXMLtoKEV` (:162-169).
fn xml_to_kev(f: &str) -> Option<&'static str> {
    Some(match f {
        "info:ofi/fmt:xml:xsd:book" => "info:ofi/fmt:kev:mtx:book",
        "info:ofi/fmt:xml:xsd:oai_dc" => "info:ofi/fmt:kev:mtx:dc",
        "info:ofi/fmt:xml:xsd:dissertation" => "info:ofi/fmt:kev:mtx:dissertation",
        "info:ofi/fmt:xml:xsd:journal" => "info:ofi/fmt:kev:mtx:journal",
        "info:ofi/fmt:xml:xsd:patent" => "info:ofi/fmt:kev:mtx:patent",
        "info:ofi/fmt:xml:xsd:sch_svc" => "info:ofi/fmt:kev:mtx:sch_svc",
        _ => return None,
    })
}

/// `contextObjectXMLToCOinS` (:95-147): one KEV string per context object.
fn context_object_xml_to_coins(doc: &XmlDocument) -> Result<Vec<String>, TranslateError> {
    let objects = xpath(doc, doc.document(), "//ctx:context-object", NS)?;
    let mut titles = Vec::new();
    for o in objects {
        let mut pieces = Vec::new();
        // encodeURIComponent(null) is "null".
        let version = xpath_text(doc, o, "./@version", NS, None)?.unwrap_or_else(|| "null".into());
        pieces.push(format!("ctx_ver={}", encode_uri_component(&version)));
        let format = xpath_text(doc, o, ".//ctx:format", NS, None)?;
        let format = format
            .as_deref()
            .and_then(xml_to_kev)
            .unwrap_or("info:ofi/fmt:kev:mtx:journal");
        pieces.push(format!("rft_val_fmt={}", encode_uri_component(format)));
        for f in xpath(doc, o, ".//ctx:metadata/*/*", NS)? {
            let name = doc.node_name(f).replacen(':', ".", 1);
            let value = encode_uri_component(&doc.text(f));
            pieces.push(format!("{name}={value}"));
        }
        titles.push(pieces.join("&"));
    }
    Ok(titles)
}
