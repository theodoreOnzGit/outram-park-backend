// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "PubMed.js" (translatorID
//   3d0231ce-fd4b-478c-b1d3-840389e5b68c, lastUpdated 2023-01-10 14:01:49):
//   the search half only: `lookupPMIDs` :43-58, `getPMID` :247-259,
//   `detectSearch` :261-275, `doSearch` :277-287. In the sandbox
//   `Zotero.Utilities.HTTP` is `Zotero.Utilities` (translate commit
//   dd524aea9a55, translate.js :1872), so `Zotero.Utilities.HTTP.doGet` is
//   `ZU.doGet`. The web half is not ported: kovan runs only identifier
//   lookup.
// Copyright (c) 2015 Philipp Zumstein.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The PubMed search translator: NCBI E-utilities `efetch` for the PMIDs,
//! imported by PubMed XML.

use crate::zotero::framework::item::JsObject;
use crate::zotero::framework::js;
use crate::zotero::framework::options::TranslatorMetadata;
use crate::zotero::framework::utilities::decode_uri_component;
use crate::zotero::search::{SearchContext, SearchError};
use crate::zotero::translators::Translator;
use serde_json::Value;

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

/// `getPMID(co)` (:247-259): the PMID in a context object's
/// `rft_id=info:pmid/...`; `Ok(None)` is upstream's `false`, `Err` its
/// `decodeURIComponent` URIError.
fn get_pmid(co: &str) -> Result<Option<String>, SearchError> {
    for part in co.split('&') {
        if let Some(v) = part.strip_prefix("rft_id=") {
            let value = decode_uri_component(v)
                .ok_or_else(|| SearchError::Translator("URIError: URI malformed".to_owned()))?;
            if let Some(pmid) = value.strip_prefix("info:pmid/") {
                return Ok(Some(pmid.to_owned()));
            }
        }
    }
    Ok(None)
}

/// `item.PMID && (typeof item.PMID == 'string' || item.PMID.length > 0)`.
fn pmid_value(search: &JsObject) -> Option<&Value> {
    let v = search.get("PMID").filter(|v| js::truthy(Some(v)))?;
    match v {
        Value::String(_) => Some(v),
        Value::Array(a) if !a.is_empty() => Some(v),
        _ => None,
    }
}

/// `detectSearch` (:261-275). A `getPMID` that throws makes detection
/// fail, which upstream treats as not detected.
pub fn detect_search(search: &JsObject) -> bool {
    if search.truthy("contextObject") {
        let co = search
            .get("contextObject")
            .map(js::to_js_string)
            .unwrap_or_default();
        match get_pmid(&co) {
            Ok(Some(p)) if !p.is_empty() => return true,
            Ok(_) => {}
            Err(_) => return false,
        }
    }
    pmid_value(search).is_some()
}

/// `doSearch` (:277-287).
pub fn do_search(ctx: &mut SearchContext, search: &JsObject) -> Result<(), SearchError> {
    let mut pmid: Option<String> = None;
    if search.truthy("contextObject") {
        let co = search
            .get("contextObject")
            .map(js::to_js_string)
            .unwrap_or_default();
        pmid = get_pmid(&co)?.filter(|p| !p.is_empty());
    }
    let ids: Vec<String> = match pmid {
        Some(p) => vec![p],
        None => match search.get("PMID") {
            Some(Value::Array(a)) => a.iter().map(js::to_js_string).collect(),
            Some(v) => vec![js::to_js_string(v)],
            None => {
                return Err(SearchError::Translator(
                    "TypeError: Cannot read properties of undefined (reading 'join')".to_owned(),
                ))
            }
        },
    };
    lookup_pmids(ctx, &ids)
}

/// `lookupPMIDs(ids)` (:43-58).
fn lookup_pmids(ctx: &mut SearchContext, ids: &[String]) -> Result<(), SearchError> {
    let uri = format!(
        "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/efetch.fcgi?db=PubMed&tool=Zotero&retmode=xml&rettype=citation&id={}",
        ids.join(",")
    );
    let text = ctx.do_get(&uri)?;
    if !text.contains("PubmedArticle") && !text.contains("PubmedBookArticle") {
        return Err(SearchError::Translator(
            "No Pubmed Data found - Most likely eutils is temporarily down".to_owned(),
        ));
    }
    for item in ctx.child_import(Translator::PubMedXml, &text)? {
        ctx.complete(item)?;
    }
    Ok(())
}
