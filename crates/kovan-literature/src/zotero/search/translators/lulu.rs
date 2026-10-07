// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Lulu.js" (translatorID
//   9a0ecbda-c0e9-4a19-84a9-fc8e7c845afa, lastUpdated 2024-10-24 19:22:51):
//   `getSearchResults` :15-17, `makeItem` :57-104, `detectSearch` :106-120,
//   `doSearch` :122-147.
// Copyright (c) Aurimas Vinckevicius.
// Licence: AGPL-3.0 (no licence text upstream; treated as AGPLv3 as part
//   of Zotero, maintainer decision 2026-10-07, #747).

//! The Lulu search translator (ISBN -> lulu.com product page, scraped).
//! **Disabled upstream**: `detectSearch` returns `false` ("no longer
//! working"), so no identifier lookup reaches it; `doSearch` is ported for
//! completeness and verified on a kovan-authored page
//! (`fixtures/synthetic_lulu.json`).

use crate::zotero::framework::html_dom::class_list_contains;
use crate::zotero::framework::identifiers::clean_isbn;
use crate::zotero::framework::item::{JsObject, TranslatorCreator, TranslatorItem};
use crate::zotero::framework::js;
use crate::zotero::framework::options::TranslatorMetadata;
use crate::zotero::framework::title_case::capitalize_title;
use crate::zotero::framework::utilities::{clean_author, trim_internal};
use crate::zotero::framework::xml::{NodeId, XNode, XmlDocument};
use crate::zotero::framework::xpath::{xpath, xpath_text};
use crate::zotero::search::http::{resolve_url, url_href};
use crate::zotero::search::{SearchContext, SearchError};
use regex::Regex;
use std::sync::OnceLock;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "9a0ecbda-c0e9-4a19-84a9-fc8e7c845afa",
    label: "Lulu",
    creator: "Aurimas Vinckevicius",
    target: "^https?://www\\.lulu\\.com/shop/",
    min_version: "3.0",
    priority: 101,
    translator_type: 12,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2024-10-24 19:22:51",
};

/// `detectSearch` (:106-120): `if (true) return false;`.
pub fn detect_search(_search: &JsObject) -> bool {
    false
}

/// `getSearchResults` (:15-17).
fn get_search_results(doc: &XmlDocument) -> Result<Vec<XNode>, SearchError> {
    Ok(xpath(
        doc,
        doc.document(),
        r#"//div[@class="middle-column"]/div[@class="products"]/div//a[@class="title" and @href]"#,
        &[],
    )?)
}

/// `getElementsByClassName(name)[0]` under `root`.
fn first_by_class(doc: &XmlDocument, root: NodeId, name: &str) -> Option<NodeId> {
    doc.descendants(root)
        .into_iter()
        .find(|&n| doc.is_element(n) && class_list_contains(doc, n, name))
}

fn xt(doc: &XmlDocument, n: impl Into<XNode>, expr: &str) -> Result<Option<String>, SearchError> {
    Ok(xpath_text(doc, n, expr, &[], None)?)
}

/// `makeItem(doc, url)` (:57-104).
fn make_item(
    ctx: &SearchContext,
    doc: &XmlDocument,
    url: &str,
) -> Result<TranslatorItem, SearchError> {
    let mut item = TranslatorItem::new("book");
    // ZU.trimInternal(null) would throw.
    let h2 = xt(
        doc,
        doc.document(),
        r#"//div[@class="product-information"]/h2[1]"#,
    )?
    .ok_or_else(|| SearchError::Translator("TypeError: trimInternal of null".to_owned()))?;
    item.set("title", capitalize_title(&trim_internal(&h2), true));

    static HONOR: OnceLock<Regex> = OnceLock::new();
    let honor = HONOR.get_or_init(|| {
        let ws = js::WS;
        Regex::new(&format!(
            r"(?i)^(?:Dr|Prof)\.?{ws}|{ws}(?:M.?A|Ph\.?D|B\.?S|B\.?A|M\.?D(?:\.?{ws}Ph\.?D)?)\.?$"
        ))
        .expect("static regex")
    });
    let authors = xpath(
        doc,
        doc.document(),
        r#"//div[@class="product-information"]//span[@class="authors"]/a/span"#,
        &[],
    )?;
    for a in authors {
        let name = honor
            .replace_all(&trim_internal(&doc.text(a)), "")
            .into_owned();
        let c = clean_author(&capitalize_title(&name, true), "author", false);
        item.creators.push(TranslatorCreator {
            first_name: c.first_name,
            last_name: Some(c.last_name),
            creator_type: Some(c.creator_type),
            ..Default::default()
        });
    }

    let description = first_by_class(doc, doc.document(), "description")
        .ok_or_else(|| SearchError::Translator("TypeError: description is undefined".to_owned()))?;
    let description: Option<String> =
        if first_by_class(doc, description, "expandable-text").is_some() {
            let a = xt(doc, description, "./span/text()[1]")?;
            let b = xt(
                doc,
                description,
                r#"./span/span[@class="more-text"]/text()[1]"#,
            )?;
            Some(format!(
                "{} {}",
                a.unwrap_or_else(|| "null".to_owned()),
                b.unwrap_or_else(|| "null".to_owned())
            ))
        } else {
            let t = doc.text(description);
            (trim_internal(&t) != "No description supplied").then_some(t)
        };
    if let Some(d) = description.filter(|d| !d.is_empty()) {
        // .trim().replace(/ +/, ' '): the first run of spaces only.
        static SP: OnceLock<Regex> = OnceLock::new();
        let sp = SP.get_or_init(|| Regex::new(" +").expect("static regex"));
        item.set("abstractNote", sp.replace(js::trim(&d), " ").into_owned());
    }

    let details = first_by_class(doc, doc.document(), "product-details").ok_or_else(|| {
        SearchError::Translator("TypeError: productDetails is undefined".to_owned())
    })?;
    let dd = |cls: &str| -> Result<String, SearchError> {
        Ok(xt(doc, details, &format!(r#"./dd[@class="{cls}"]"#))?.unwrap_or_default())
    };
    match clean_isbn(&dd("isbn")?, true) {
        Some(i) => item.set("ISBN", i),
        None => item.set("ISBN", false),
    }
    item.set("publisher", trim_internal(&dd("publisher")?));
    item.set("rights", trim_internal(&dd("copyright-info")?));
    item.set("language", trim_internal(&dd("language")?));
    match ctx.str_to_iso(&dd("publication-date")?) {
        Some(d) => item.set("date", d),
        None => item.set("date", false),
    }
    item.set("numPages", trim_internal(&dd("pages")?));

    let mut att = JsObject::new();
    att.set("title", "Lulu Link");
    att.set("url", url);
    att.set("mimeType", "text/html");
    att.set("snapshot", false);
    item.attachments.push(att);
    Ok(item)
}

/// `doSearch` (:122-147) for one search item.
pub fn do_search(ctx: &mut SearchContext, search: &JsObject) -> Result<(), SearchError> {
    if !search.truthy("ISBN") {
        // `items[i].complete` is undefined on a search object.
        return Ok(());
    }
    let raw = search.get("ISBN").map(js::to_js_string).unwrap_or_default();
    let Some(isbn) = clean_isbn(&raw, false) else {
        return Ok(());
    };
    let (doc, url) = ctx.process_document(&format!(
        "https://www.lulu.com/shop/search.ep?keyWords={isbn}"
    ))?;
    let results = get_search_results(&doc)?;
    let Some(first) = results.first() else {
        return Ok(());
    };
    // `results[0].href`: the attribute resolved against the document URL.
    let href = doc
        .get_attribute(first.node(), "href")
        .unwrap_or("")
        .to_owned();
    let target = url_href(&resolve_url(&url, &href)).unwrap_or(href);
    let (doc2, url2) = ctx.process_document(&target)?;
    let new_item = make_item(ctx, &doc2, &url2)?;
    if new_item.get_str("ISBN") == Some(isbn.as_str()) {
        ctx.complete(new_item)?;
    }
    Ok(())
}
