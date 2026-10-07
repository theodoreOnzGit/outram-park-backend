// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "ADS Bibcode.js" (translatorID
//   09bd8037-a9bb-4f9a-b3b9-d18b2564b49e, lastUpdated 2025-04-29 03:02:00):
//   `getRealType` :40-52, `bibcodeRe` :55, `detectSearch` :57-59,
//   `doSearch` :61-65, `filterQuery` :67-82, `extractId` :84-87,
//   `makePdfUrl` :89-91, `isArXiv` :95-99, `scrape` :101-174.
// Copyright (c) 2021 Abe Jellinek.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The ADS Bibcode search translator: NASA ADS's RIS export for a
//! bibcode, imported by RIS and corrected (theses, proceedings, arXiv).
//!
//! As upstream, the export API is called with the anonymous access token
//! ADS's own bootstrap endpoint hands out; the token is fetched at lookup
//! time (only when the user runs a lookup) and never stored. The port's
//! tests use a kovan-authored fixture with a placeholder token; ADS was
//! never contacted to make it (DATA_POLICY.md).

use crate::zotero::framework::item::{JsObject, TranslatorItem};
use crate::zotero::framework::js;
use crate::zotero::framework::options::TranslatorMetadata;
use crate::zotero::framework::utilities::decode_uri_component;
use crate::zotero::search::http::RequestOptions;
use crate::zotero::search::{SearchContext, SearchError};
use crate::zotero::translators::Translator;
use serde_json::Value;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "09bd8037-a9bb-4f9a-b3b9-d18b2564b49e",
    label: "ADS Bibcode",
    creator: "Abe Jellinek",
    target: "",
    min_version: "6.0",
    priority: 100,
    translator_type: 8,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2025-04-29 03:02:00",
};

fn type_error(m: &str) -> SearchError {
    SearchError::Translator(format!("TypeError: {m}"))
}

/// `bibcodeRe` (:55): `/^\d{4}\D\S{13}[A-Z.:]$/`.
fn is_bibcode(s: &str) -> bool {
    let c: Vec<char> = s.chars().collect();
    c.len() == 19
        && c[..4].iter().all(char::is_ascii_digit)
        && !c[4].is_ascii_digit()
        && c[5..18].iter().all(|&ch| !js::is_space(ch))
        && (c[18].is_ascii_uppercase() || c[18] == '.' || c[18] == ':')
}

/// `filterQuery(items)` (:67-82) for one search item.
fn filter_query(search: &JsObject) -> Vec<String> {
    match search.get("adsBibcode") {
        Some(Value::String(b)) if !b.is_empty() => {
            let b = js::trim(b);
            if is_bibcode(b) {
                vec![b.to_owned()]
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

/// `detectSearch` (:57-59).
pub fn detect_search(search: &JsObject) -> bool {
    !filter_query(search).is_empty()
}

/// `doSearch` (:61-65).
pub fn do_search(ctx: &mut SearchContext, search: &JsObject) -> Result<(), SearchError> {
    let bibcodes = filter_query(search);
    if bibcodes.is_empty() {
        return Ok(());
    }
    scrape(ctx, &bibcodes)
}

/// `getRealType(bibStem, exportType)` (:40-52).
fn get_real_type(bib_stem: &str, export_type: &str) -> String {
    if bib_stem.starts_with("PhDT") || bib_stem.starts_with("MsT") {
        return "thesis".to_owned();
    }
    // `bibStem.substring(5, 9)`.
    let volume: String = bib_stem.chars().skip(5).take(4).collect();
    if volume == "conf" && export_type == "journalArticle" {
        return "book".to_owned();
    }
    export_type.to_owned()
}

/// `extractId(url)` (:84-87): `decodeURIComponent` of what follows
/// `/abs/`, up to the next `/`; `Ok(None)` is upstream's `null`.
fn extract_id(url: &str) -> Result<Option<String>, SearchError> {
    let Some(i) = url.find("/abs/") else {
        return Ok(None);
    };
    let rest = &url[i + 5..];
    let seg = rest.split('/').next().unwrap_or("");
    if seg.is_empty() {
        // `[^/]+` needs a character: try a later "/abs/".
        return extract_id(&url[i + 4..]);
    }
    decode_uri_component(seg)
        .map(Some)
        .ok_or_else(|| SearchError::Translator("URIError: URI malformed".to_owned()))
}

/// `scrape(ids)` (:101-174).
fn scrape(ctx: &mut SearchContext, ids: &[String]) -> Result<(), SearchError> {
    let bootstrap = ctx.request_json(
        "https://api.adsabs.harvard.edu/v1/accounts/bootstrap",
        &RequestOptions::default(),
    )?;
    let token = bootstrap
        .get("access_token")
        .filter(|v| js::truthy(Some(v)))
        .map(js::to_js_string)
        .ok_or_else(|| {
            SearchError::Translator("ADS Bibcode: cannot obtain access token".to_owned())
        })?;
    let body = serde_json::json!({ "bibcode": ids, "sort": ["no sort"] }).to_string();
    let auth = format!("Bearer {token}");
    let response = ctx.request_json(
        "https://api.adsabs.harvard.edu/v1/export/ris",
        &RequestOptions::post(
            body,
            &[
                ("Accept", "application/json"),
                ("Authorization", auth.as_str()),
                ("Content-Type", "application/json"),
            ],
        ),
    )?;
    let export = match &response {
        Value::Null => {
            return Err(type_error(
                "Cannot read properties of null (reading 'export')",
            ));
        }
        v => v
            .get("export")
            .map(js::to_js_string)
            .unwrap_or_else(|| "undefined".to_owned()),
    };
    for item in ctx.child_import(Translator::Ris, &export)? {
        let item = item_done_handler(ctx, item)?;
        ctx.complete(item)?;
    }
    Ok(())
}

/// The RIS `itemDone` handler (:117-170).
fn item_done_handler(
    ctx: &SearchContext,
    mut item: TranslatorItem,
) -> Result<TranslatorItem, SearchError> {
    let url = match item.get("url") {
        None | Some(Value::Null) => {
            return Err(type_error(
                "Cannot read properties of undefined (reading 'match')",
            ))
        }
        Some(v) => js::to_js_string(v),
    };
    let id = extract_id(&url)?
        .ok_or_else(|| type_error("Cannot read properties of null (reading 'slice')"))?;
    let bib_stem: String = id.chars().skip(4).collect();

    let ty = get_real_type(&bib_stem, &item.item_type);
    if ty != item.item_type {
        item.item_type = ty;
    }

    // isArXiv (:95-99).
    let doi_arxiv = item.truthy("DOI")
        && item
            .get_str("DOI")
            .is_some_and(|d| d.starts_with("10.48550/"));
    if doi_arxiv || bib_stem.starts_with("arXiv") {
        item.item_type = "preprint".to_owned();
        item.set("publisher", "arXiv");
        item.remove("pages");
        item.remove("publicationTitle");
        item.remove("journalAbbreviation");
    }

    let extra = if item.truthy("extra") {
        item.get_string("extra").unwrap_or_default()
    } else {
        String::new()
    };
    item.set("extra", format!("{extra}\nADS Bibcode: {id}"));

    if item.item_type == "thesis" {
        if bib_stem.starts_with("PhDT") {
            item.set("thesisType", "Ph.D. thesis");
        } else if bib_stem.starts_with("MsT") {
            item.set("thesisType", "Masters thesis");
        }
        item.remove("journalAbbreviation");
        item.remove("publicationTitle");
    }

    let mut pdf = JsObject::new();
    pdf.set(
        "url",
        format!("https://ui.adsabs.harvard.edu/link_gateway/{id}/ARTICLE"),
    );
    pdf.set("title", "Full Text PDF");
    pdf.set("mimeType", "application/pdf");
    item.attachments.push(pdf);

    // `item.journalAbbreviation == item.publicationTitle` (loose; undefined
    // equals undefined).
    let norm = |v: Option<&Value>| v.filter(|v| !v.is_null()).cloned();
    if norm(item.get("journalAbbreviation")) == norm(item.get("publicationTitle")) {
        item.remove("journalAbbreviation");
    }

    if item.truthy("date") {
        let d = item.get_string("date").unwrap_or_default();
        let iso = ctx.str_to_iso(&d).map_or(Value::Bool(false), Value::String);
        item.set("date", iso);
    }
    item.set("libraryCatalog", "NASA ADS");
    Ok(item)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bibcodes_and_types_like_upstream() {
        assert!(is_bibcode("2021PhDT.........5C"));
        assert!(is_bibcode("2022MSSP..16208070W"));
        assert!(!is_bibcode("2022MSSP..16208070w"));
        assert_eq!(get_real_type("PhDT.........5C", "journalArticle"), "thesis");
        assert_eq!(get_real_type("jsrs.conf.....B", "journalArticle"), "book");
        assert_eq!(
            extract_id("https://ui.adsabs.harvard.edu/abs/2021PhDT.........5C").unwrap(),
            Some("2021PhDT.........5C".to_owned())
        );
    }
}
