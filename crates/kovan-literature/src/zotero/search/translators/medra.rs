// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "mEDRA.js" (translatorID
//   d9b57cd5-5a9c-4946-8616-3bdf8edfcbb5, lastUpdated 2025-07-14 17:55:30):
//   `scrapeMasterTable` :15-24, `scrapeTable` :26-71, `map`/`creatorMap`
//   :73-101, `scrapeMeta` :103-146, `itemTypeMap`/`mapItemType` :155-180,
//   `doWeb` :182-282, `sanitizeQueries` :284-300, `detectSearch` :302-313,
//   `doSearch` :315-323.
// Copyright (c) Aurimas Vinckevicius.
// Licence: AGPL-3.0 (no licence text upstream; treated as AGPLv3 as part
//   of Zotero, maintainer decision 2026-10-07, #747).

//! The mEDRA search translator (DOI -> the mEDRA DOI resolution page,
//! scraped). **Disabled upstream**: `detectSearch` returns `false` (TEMP,
//! "This translator is broken"), so no identifier lookup reaches it;
//! `doSearch` is ported for completeness and verified on a kovan-authored
//! page (`fixtures/synthetic_medra.json`).

use crate::zotero::framework::html::unescape_html;
use crate::zotero::framework::html_dom::{html_node_name, next_element_sibling};
use crate::zotero::framework::identifiers::clean_isbn;
use crate::zotero::framework::item::{JsObject, TranslatorCreator, TranslatorItem};
use crate::zotero::framework::js;
use crate::zotero::framework::options::TranslatorMetadata;
use crate::zotero::framework::title_case::capitalize_title;
use crate::zotero::framework::utilities::{clean_author, clean_doi, trim_internal};
use crate::zotero::framework::xml::{NodeId, XmlDocument};
use crate::zotero::search::http::encode_uri_component;
use crate::zotero::search::{SearchContext, SearchError};
use regex::Regex;
use std::sync::OnceLock;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "d9b57cd5-5a9c-4946-8616-3bdf8edfcbb5",
    label: "mEDRA",
    creator: "Aurimas Vinckevicius",
    target: "^https?://www\\.medra\\.org/servlet/view\\?",
    min_version: "3.0",
    priority: 105,
    translator_type: 12,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2025-07-14 17:55:30",
};

/// A value in `meta`: a string, or the `creators` array of
/// `{lastName, creatorType}`.
#[derive(Debug, Clone, PartialEq)]
enum MetaVal {
    Str(String),
    Creators(Vec<(String, String)>),
}

/// `meta`, a JavaScript object: keys in insertion order; `delete` leaves a
/// hole so a `for...in` in progress skips it.
#[derive(Debug, Clone, Default)]
struct Meta(Vec<(String, Option<MetaVal>)>);

impl Meta {
    fn get(&self, k: &str) -> Option<&MetaVal> {
        self.0
            .iter()
            .find(|(key, _)| key == k)
            .and_then(|(_, v)| v.as_ref())
    }
    fn get_str(&self, k: &str) -> Option<&str> {
        match self.get(k) {
            Some(MetaVal::Str(s)) => Some(s),
            _ => None,
        }
    }
    fn set(&mut self, k: &str, v: MetaVal) {
        match self
            .0
            .iter_mut()
            .find(|(key, val)| key == k && val.is_some())
        {
            Some(e) => e.1 = Some(v),
            None => self.0.push((k.to_owned(), Some(v))),
        }
    }
    fn delete(&mut self, k: &str) -> Option<MetaVal> {
        self.0
            .iter_mut()
            .find(|(key, val)| key == k && val.is_some())
            .and_then(|e| e.1.take())
    }
    fn is_empty(&self) -> bool {
        self.0.iter().all(|(_, v)| v.is_none())
    }
}

fn type_error(m: &str) -> SearchError {
    SearchError::Translator(format!("TypeError: {m}"))
}

fn first_element_child(doc: &XmlDocument, n: NodeId) -> Option<NodeId> {
    doc.element_children(n).into_iter().next()
}

/// `scrapeMasterTable` (:15-24).
fn scrape_master_table(doc: &XmlDocument) -> Result<Option<Meta>, SearchError> {
    let Some(td) = doc.get_element_by_id("contenuto") else {
        return Ok(None);
    };
    let mut meta = Meta::default();
    scrape_table(doc, first_element_child(doc, td), &mut meta)?;
    if meta.is_empty() {
        return Ok(None);
    }
    Ok(Some(meta))
}

/// `scrapeTable` (:26-71).
fn scrape_table(
    doc: &XmlDocument,
    node: Option<NodeId>,
    meta: &mut Meta,
) -> Result<(), SearchError> {
    let Some(mut node) = node else {
        return Ok(());
    };
    let mut section: Option<&'static str> = None;
    loop {
        let full = html_node_name(doc, node);
        let tag = full.rsplit(':').next().unwrap_or("").to_owned();
        if tag == "SPAN" {
            let heading = trim_internal(&doc.text(node)).to_lowercase();
            section = match heading.as_str() {
                "doi resolution data:"
                | "serial article data:"
                | "content item data:"
                | "metadata:" => Some("top"),
                "serial publication data:"
                | "journal issue data:"
                | "monographic publication data:" => Some("container"),
                _ => None,
            };
        } else if tag == "TABLE" {
            if !doc.get_elements_by_tag_name(node, "span").is_empty() {
                let body = first_element_child(doc, node)
                    .ok_or_else(|| type_error("firstElementChild is null"))?;
                let mut tr = first_element_child(doc, body);
                while let Some(t) = tr {
                    if let Some(td) = first_element_child(doc, t) {
                        scrape_table(doc, first_element_child(doc, td), meta)?;
                    }
                    tr = next_element_sibling(doc, t);
                }
            } else {
                let body = first_element_child(doc, node)
                    .ok_or_else(|| type_error("firstElementChild is null"))?;
                scrape_meta(doc, first_element_child(doc, body), section, meta)?;
            }
        }
        match next_element_sibling(doc, node) {
            Some(n) => node = n,
            None => return Ok(()),
        }
    }
}

/// `map.all`.
fn map_all(label: &str) -> Option<&'static str> {
    Some(match label {
        "doi" => "DOI",
        "url" => "url",
        "publisher" => "publisher",
        "country of publication" => "place",
        "issn" => "ISSN",
        "product form" => "itemType",
        "journal issue number" => "issue",
        "language of text" => "language",
        "first page" => "firstPage",
        "last page" => "lastPage",
        "copyright year" => "cDate",
        "copyright owner" => "cOwner",
        "descriptive text" => "abstractNote",
        _ => return None,
    })
}

/// `scrapeMeta` (:103-146).
fn scrape_meta(
    doc: &XmlDocument,
    tr: Option<NodeId>,
    section: Option<&'static str>,
    meta: &mut Meta,
) -> Result<(), SearchError> {
    let Some(mut tr) = tr else {
        return Ok(());
    };
    static ROLE: OnceLock<Regex> = OnceLock::new();
    static SLASH: OnceLock<Regex> = OnceLock::new();
    let role_re = ROLE.get_or_init(|| Regex::new(r"\(([^(]+?)\)").expect("static regex"));
    let slash = SLASH
        .get_or_init(|| Regex::new(&format!(r"{ws}*/{ws}*", ws = js::WS)).expect("static regex"));
    loop {
        let label_el = first_element_child(doc, tr).ok_or_else(|| type_error("label is null"))?;
        let value_el = next_element_sibling(doc, label_el)
            .ok_or_else(|| type_error("nextElementSibling is null"))?;
        let value = trim_internal(&doc.text(value_el));
        let label = trim_internal(&doc.text(label_el)).to_lowercase();
        if !label.is_empty() && !value.is_empty() {
            let z = match map_all(&label) {
                Some(z) => Some(z),
                None => match section {
                    Some("container") => (label == "full title").then_some("publicationTitle"),
                    Some(_) => (label == "full title").then_some("title"),
                    None => return Err(type_error("map[section] is undefined")),
                },
            };
            if let Some(z) = z {
                meta.set(z, MetaVal::Str(value));
            } else if label.starts_with("by ") {
                let role = role_re
                    .captures(&label)
                    .map(|c| js::trim(&c[1]).to_owned())
                    .filter(|r| r == "author");
                if let Some(role) = role {
                    let mut list = match meta.get("creators") {
                        Some(MetaVal::Creators(c)) => c.clone(),
                        _ => Vec::new(),
                    };
                    list.push((value, role));
                    meta.set("creators", MetaVal::Creators(list));
                }
            } else if label.starts_with("journal issue date")
                || label.starts_with("publication date")
            {
                meta.set(
                    "date",
                    MetaVal::Str(slash.replace_all(&value, "-").into_owned()),
                );
            } else if label.starts_with("other product identifier") {
                if let Some(isbn) = clean_isbn(&value, false) {
                    meta.set("ISBN", MetaVal::Str(isbn));
                }
            }
        }
        match next_element_sibling(doc, tr) {
            Some(n) => tr = n,
            None => return Ok(()),
        }
    }
}

/// `mapItemType` (:162-180).
fn map_item_type(meta: &mut Meta) -> &'static str {
    let value = meta.delete("itemType");
    if let Some(MetaVal::Str(v)) = value {
        static T: OnceLock<Regex> = OnceLock::new();
        let re = T.get_or_init(|| {
            Regex::new(&format!(r"\({ws}*([A-Z]{{2}}){ws}*\)", ws = js::WS)).expect("static regex")
        });
        if let Some(c) = re.captures(&v) {
            match &c[1] {
                "DH" | "JB" | "JD" => return "journalArticle",
                "BA" => return "bookSection",
                _ => {}
            }
        }
    }
    "journalArticle"
}

fn ends_with_escape(s: &str) -> bool {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"&#[0-9]{2,4}$").expect("static regex"))
        .is_match(s)
}

fn to_creator(a: crate::zotero::framework::utilities::CleanedAuthor) -> TranslatorCreator {
    TranslatorCreator {
        first_name: a.first_name,
        last_name: Some(a.last_name),
        creator_type: Some(a.creator_type),
        ..Default::default()
    }
}

/// The `creators` case of `doWeb` (:225-266).
fn fix_creators(mut value: Vec<(String, String)>) -> Vec<TranslatorCreator> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < value.len() {
        let (last, role) = value[i].clone();
        if ends_with_escape(&last) {
            let mut name = format!("{last};");
            let j = i + 1;
            while j < value.len() {
                let next = value[j].0.clone();
                let first: String = next
                    .encode_utf16()
                    .take(1)
                    .map(|u| char::from_u32(u32::from(u)).unwrap_or('\u{FFFD}'))
                    .collect();
                if first.to_uppercase() == first && name.contains(',') && next.contains(',') {
                    break;
                }
                name.push_str(&next);
                value.remove(j);
                if !ends_with_escape(&name) {
                    break;
                }
                name.push(';');
            }
            let name = unescape_html(&name);
            out.push(to_creator(clean_author(&name, &role, true)));
        } else {
            out.push(to_creator(clean_author(&last, &role, last.contains(','))));
        }
        i += 1;
    }
    out
}

/// `doWeb(doc, url)` (:182-282).
fn do_web(ctx: &mut SearchContext, doc: &XmlDocument) -> Result<(), SearchError> {
    let Some(mut meta) = scrape_master_table(doc)? else {
        return Ok(());
    };
    let item_type = map_item_type(&mut meta);
    let mut item = TranslatorItem::new(item_type);
    static LANG: OnceLock<Regex> = OnceLock::new();
    static PLACE: OnceLock<Regex> = OnceLock::new();
    static COLON: OnceLock<Regex> = OnceLock::new();
    let lang_re = LANG.get_or_init(|| {
        Regex::new(&format!(r"\({ws}*([A-Za-z0-9_]{{3}}){ws}*\)", ws = js::WS))
            .expect("static regex")
    });
    let place_re =
        PLACE.get_or_init(|| Regex::new(&format!(r"{}*\(.*", js::WS)).expect("static regex"));
    let colon_re =
        COLON.get_or_init(|| Regex::new(&format!(r"{}+:", js::WS)).expect("static regex"));
    let mut idx = 0;
    while idx < meta.0.len() {
        let (key, val) = meta.0[idx].clone();
        idx += 1;
        let Some(val) = val else { continue };
        let mut label = key.clone();
        let mut value = val;
        match label.as_str() {
            "language" => {
                if let MetaVal::Str(s) = &value {
                    if let Some(c) = lang_re.captures(s) {
                        value = MetaVal::Str(js::trim(&c[1]).to_owned());
                    }
                }
            }
            "place" => {
                if let MetaVal::Str(s) = &value {
                    value = MetaVal::Str(place_re.replace(s, "").into_owned());
                }
            }
            "cDate" | "cOwner" => {
                let date = meta
                    .get_str("cDate")
                    .map(|d| format!("{d} "))
                    .unwrap_or_default();
                let owner = meta.get_str("cOwner").unwrap_or("").to_owned();
                value = MetaVal::Str(format!("©{date}{owner}"));
                meta.delete("cOwner");
                meta.delete("cDate");
                label = "rights".to_owned();
            }
            "firstPage" => {
                if let MetaVal::Str(s) = &value {
                    let mut s = s.clone();
                    if let Some(lp) = meta.get_str("lastPage") {
                        s.push('–');
                        s.push_str(lp);
                    }
                    value = MetaVal::Str(s);
                }
                label = "pages".to_owned();
            }
            "lastPage" => continue,
            "creators" => {
                if let MetaVal::Creators(c) = value {
                    item.creators = fix_creators(c);
                }
                continue;
            }
            _ => {}
        }
        if let MetaVal::Str(mut s) = value {
            if label == "title" || label == "publicationTitle" {
                if s.to_uppercase() == s {
                    s = capitalize_title(&s, true);
                }
                s = colon_re.replace_all(&s, ":").into_owned();
            }
            item.set(label, s);
        }
    }
    ctx.complete(item)
}

/// `sanitizeQueries` (:284-300) for one search object.
fn sanitize_queries(search: &JsObject) -> Vec<String> {
    match search.get("DOI") {
        Some(v) if js::truthy(Some(v)) => clean_doi(&js::to_js_string(v)).into_iter().collect(),
        _ => Vec::new(),
    }
}

/// `detectSearch` (:302-313): `return false;` (TEMP, disabled upstream).
pub fn detect_search(_search: &JsObject) -> bool {
    false
}

/// `doSearch` (:315-323): each DOI's mEDRA page through `doWeb`, in turn.
pub fn do_search(ctx: &mut SearchContext, search: &JsObject) -> Result<(), SearchError> {
    for doi in sanitize_queries(search) {
        let url = format!(
            "https://www.medra.org/servlet/view?lang=en&doi={}",
            encode_uri_component(&doi)
        );
        let (doc, _url) = ctx.process_document(&url)?;
        do_web(ctx, &doc)?;
    }
    Ok(())
}
