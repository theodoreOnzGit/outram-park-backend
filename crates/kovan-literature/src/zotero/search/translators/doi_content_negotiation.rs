// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "DOI Content Negotiation.js" (translatorID
//   b28d0d42-8549-4c6d-83fc-8382874a5cb9, lastUpdated 2025-07-27 04:51:26):
//   `detectSearch` :37-39, `filterQuery` :42-60, `doSearch` :62-66,
//   `processDOI` :68-156.
// Copyright (c) 2019 Sebastian Karcher.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The DOI Content Negotiation search translator: `https://doi.org/<DOI>`
//! asked for DataCite JSON, Crossref Unixref XML or CSL JSON, imported by
//! the matching import translator.
//!
//! Upstream's Crossref-outage branch (:69-101) is `if (false)` and is not
//! ported.

use crate::zotero::framework::item::{JsObject, TranslatorItem};
use crate::zotero::framework::options::TranslatorMetadata;
use crate::zotero::framework::utilities::{clean_doi, decode_uri_component};
use crate::zotero::search::http::{encode_uri_component, RequestOptions};
use crate::zotero::search::{SearchContext, SearchError};
use crate::zotero::translators::Translator;
use serde_json::Value;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "b28d0d42-8549-4c6d-83fc-8382874a5cb9",
    label: "DOI Content Negotiation",
    creator: "Sebastian Karcher",
    target: "",
    min_version: "5.0",
    priority: 100,
    translator_type: 8,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2025-07-27 04:51:26",
};

/// `filterQuery` (:42-60) for a search item (an object; upstream also
/// takes strings and arrays, which no caller here passes).
fn filter_query(search: &JsObject) -> Vec<String> {
    match search.get("DOI") {
        Some(v) if crate::zotero::framework::js::truthy(Some(v)) => {
            clean_doi(&crate::zotero::framework::js::to_js_string(v))
                .into_iter()
                .collect()
        }
        _ => Vec::new(),
    }
}

/// `detectSearch` (:37-39).
pub fn detect_search(search: &JsObject) -> bool {
    !filter_query(search).is_empty()
}

/// `doSearch` (:62-66).
pub fn do_search(ctx: &mut SearchContext, search: &JsObject) -> Result<(), SearchError> {
    for doi in filter_query(search) {
        process_doi(ctx, &doi)?;
    }
    Ok(())
}

const ACCEPT: &str = "application/vnd.datacite.datacite+json, application/vnd.crossref.unixref+xml, application/vnd.citationstyles.csl+json";

/// `processDOI` (:68-156).
fn process_doi(ctx: &mut SearchContext, doi: &str) -> Result<(), SearchError> {
    let response = ctx.request_text(
        &format!("https://doi.org/{}", encode_uri_component(doi)),
        &RequestOptions::headers(&[("Accept", ACCEPT)]),
    )?;
    if response.is_empty() {
        return Ok(());
    }
    if response.contains("<crossref") {
        for mut item in ctx.child_import(Translator::CrossrefUnixrefXml, &response)? {
            item.set("libraryCatalog", "DOI.org (Crossref)");
            ctx.complete(item)?;
        }
    } else if response.contains("http://datacite.org/schema")
        || response.contains("\"agency\": \"DataCite\"")
        || response.contains("\"providerId\": ")
    {
        for mut item in ctx.child_import(Translator::DataciteJson, &response)? {
            item.set("libraryCatalog", "DOI.org (Datacite)");
            ctx.complete(item)?;
        }
    } else {
        for mut item in ctx.child_import(Translator::CslJson, &response)? {
            item.set("libraryCatalog", "DOI.org (CSL JSON)");
            fix_control_characters(&mut item)?;
            ctx.complete(item)?;
        }
    }
    Ok(())
}

/// The CSL JSON `itemDone` handler's repair (:137-149): a string field
/// holding a character in U+007F..U+009F has every character outside
/// `[0-9A-Za-z ]` replaced by `"%" + charCode.toString(16)` and is then
/// `decodeURIComponent`-ed (which throws, failing the translation, when
/// that is not valid percent-encoded UTF-8).
fn fix_control_characters(item: &mut TranslatorItem) -> Result<(), SearchError> {
    let fix = |s: &str| -> Result<Option<String>, SearchError> {
        if !s.chars().any(|c| ('\u{7F}'..='\u{9F}').contains(&c)) {
            return Ok(None);
        }
        let mut escaped = String::new();
        for u in s.encode_utf16() {
            let c = char::from_u32(u32::from(u)).filter(|c| c.is_ascii_alphanumeric() || *c == ' ');
            match c {
                Some(c) => escaped.push(c),
                None => escaped.push_str(&format!("%{u:x}")),
            }
        }
        decode_uri_component(&escaped)
            .map(Some)
            .ok_or_else(|| SearchError::Translator("URIError: URI malformed".to_owned()))
    };
    if let Some(t) = fix(&item.item_type)? {
        item.item_type = t;
    }
    let keys: Vec<String> = item.props.keys().map(str::to_owned).collect();
    for k in keys {
        if let Some(Value::String(s)) = item.props.get(&k) {
            if let Some(fixed) = fix(s)? {
                item.props.set(k, fixed);
            }
        }
    }
    Ok(())
}
