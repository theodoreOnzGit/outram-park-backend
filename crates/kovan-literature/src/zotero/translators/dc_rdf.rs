// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Unqualified Dublin Core RDF.js" (translatorID
//   6e372642-ed9d-4934-b5d1-c11ac758ebb7, lastUpdated 2019-06-11 13:33:25):
//   `doExport` :39-153.
// Copyright (c) 2008 Simon Kornblith.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The Unqualified Dublin Core RDF export translator.

use crate::zotero::framework::js;
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::rdf::{RdfSandbox, Res};
use crate::zotero::framework::{ExportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "6e372642-ed9d-4934-b5d1-c11ac758ebb7",
    label: "Unqualified Dublin Core RDF",
    creator: "Simon Kornblith",
    target: "rdf",
    min_version: "1.0.0b3.r1",
    priority: 100,
    translator_type: translator_type::EXPORT,
    config_options: &[("dataMode", HeaderValue::Str("rdf/xml"))],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2019-06-11 13:33:25",
};

const DC: &str = "http://purl.org/dc/elements/1.1/";

/// `doExport()` (:39-153).
pub fn do_export(ctx: &mut ExportContext) -> Result<(), TranslateError> {
    let e = |m: String| TranslateError::Translator(m);
    let mut rdf = RdfSandbox::new(ctx.options.env.first_blank_node_id);
    rdf.add_namespace("dc", DC);
    while let Some(item) = ctx.next_item() {
        if item.item_type == "note" || item.item_type == "attachment" {
            continue;
        }
        let s = |k: &str| {
            item.get(k)
                .map(js::to_js_string)
                .unwrap_or_else(|| "undefined".to_owned())
        };
        let t = |k: &str| item.truthy(k);
        // `"urn:isbn:" + item.ISBN` (not URI-encoded here, unlike Zotero RDF).
        let resource: Res = if t("ISBN") {
            Res::Uri(format!("urn:isbn:{}", s("ISBN")))
        } else if t("url") {
            Res::Uri(s("url"))
        } else {
            Res::Node(rdf.new_resource())
        };
        let mut lit = |p: &str, v: &str| {
            rdf.add_literal(resource.clone(), format!("{DC}{p}"), v)
                .map_err(e)
        };
        if t("title") {
            lit("title", &s("title"))?;
        }
        lit("type", &item.item_type)?;
        for c in &item.creators {
            // `var creator = item.creators[j].lastName` (+ ", " + firstName).
            let mut creator = c
                .last_name
                .clone()
                .unwrap_or_else(|| "undefined".to_owned());
            if c.first_name.as_deref().is_some_and(|f| !f.is_empty()) {
                creator.push_str(", ");
                creator.push_str(c.first_name.as_deref().unwrap_or(""));
            }
            if c.creator_type.as_deref() == Some("author") {
                lit("creator", &creator)?;
            } else {
                lit("contributor", &creator)?;
            }
        }
        if t("source") {
            lit("source", &s("source"))?;
        }
        if t("accessionNumber") {
            lit("identifier", &s("accessionNumber"))?;
        }
        if t("rights") {
            lit("rights", &s("rights"))?;
        }
        if t("publisher") {
            lit("publisher", &s("publisher"))?;
        } else if t("distributor") {
            lit("publisher", &s("distributor"))?;
        } else if t("institution") {
            lit("publisher", &s("institution"))?;
        }
        if t("date") {
            lit("date", &s("date"))?;
        }
        if t("ISBN") {
            lit("identifier", &format!("ISBN {}", s("ISBN")))?;
        }
        if t("ISSN") {
            lit("identifier", &format!("ISSN {}", s("ISSN")))?;
        }
        if t("DOI") {
            lit("identifier", &format!("DOI {}", s("DOI")))?;
        }
        if t("callNumber") {
            lit("identifier", &s("callNumber"))?;
        }
        if t("archiveLocation") {
            lit("coverage", &s("archiveLocation"))?;
        }
        if t("medium") {
            lit("format", &s("medium"))?;
        }
    }
    let out = rdf.serialize().map_err(e)?;
    ctx.write(&out);
    Ok(())
}
