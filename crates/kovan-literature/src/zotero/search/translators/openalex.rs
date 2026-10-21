// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "OpenAlex.js" (translatorID
//   432d79fe-79e1-4791-b3e1-baf700710163, lastUpdated 2024-07-29 14:16:50):
//   `detectSearch` :49-51, `doSearch` :53-55, `scrape` :92-103. The web
//   half (`detectWeb`, `getSearchResults`, `doWeb`) is not ported.
// Copyright (c) 2024 Sebastian Karcher.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The OpenAlex search translator: the OpenAlex works API filtered by
//! OpenAlex ID, imported by OpenAlex JSON. No identifier that
//! `extractIdentifiers` finds sets `openAlex`, so it runs only when called
//! with a search item that has one.

use crate::zotero::framework::item::JsObject;
use crate::zotero::framework::js;
use crate::zotero::framework::options::TranslatorMetadata;
use crate::zotero::search::http::RequestOptions;
use crate::zotero::search::{SearchContext, SearchError};
use crate::zotero::translators::Translator;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "432d79fe-79e1-4791-b3e1-baf700710163",
    label: "OpenAlex",
    creator: "Sebastian Karcher",
    target: "^https://openalex\\.org/works",
    min_version: "5.0",
    priority: 100,
    translator_type: 12,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2024-07-29 14:16:50",
};

/// `detectSearch` (:49-51): `!!item.openAlex`.
pub fn detect_search(search: &JsObject) -> bool {
    search.truthy("openAlex")
}

/// `doSearch` (:53-55): `scrape([item.openAlex])`.
pub fn do_search(ctx: &mut SearchContext, search: &JsObject) -> Result<(), SearchError> {
    // `[x].join("|")` of one element: "" for undefined/null, else String(x).
    let id = match search.get("openAlex") {
        None | Some(serde_json::Value::Null) => String::new(),
        Some(v) => js::to_js_string(v),
    };
    scrape(ctx, &[id])
}

/// `scrape` (:92-103): the works API, then OpenAlex JSON (its `itemDone`
/// handler only completes the item).
fn scrape(ctx: &mut SearchContext, ids: &[String]) -> Result<(), SearchError> {
    let url = format!(
        "https://api.openalex.org/works?filter=openalex:{}",
        ids.join("|")
    );
    let json = ctx.request_text(&url, &RequestOptions::default())?;
    for item in ctx.child_import(Translator::OpenAlexJson, &json)? {
        ctx.complete(item)?;
    }
    Ok(())
}
