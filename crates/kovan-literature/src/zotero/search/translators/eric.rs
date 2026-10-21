// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "ERIC.js" (translatorID
//   e4660e05-a935-43ec-8eec-df0347362e4c, lastUpdated 2024-07-05 11:56:39):
//   `getType` :44-66, `cleanInput` :161-176, `detectSearch` :178-180,
//   `doSearch` :182-229. The web half (`detectWeb`, `findType`,
//   `getSearchResults`, `doWeb`, `scrape`, which runs Embedded Metadata) is
//   not ported.
// Copyright (c) 2013 Sebastian Karcher.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The ERIC search translator: the ERIC API (`api.ies.ed.gov/eric`) by ERIC
//! number. No identifier that `extractIdentifiers` finds sets
//! `ericNumber`, so it runs only when called with a search item that has
//! one.

use crate::zotero::framework::html::unescape_html;
use crate::zotero::framework::identifiers::{clean_isbn, clean_issn};
use crate::zotero::framework::item::{JsObject, TranslatorCreator, TranslatorItem, TranslatorTag};
use crate::zotero::framework::js;
use crate::zotero::framework::options::TranslatorMetadata;
use crate::zotero::framework::utilities::{clean_author, clean_doi, decode_uri_component};
use crate::zotero::search::http::RequestOptions;
use crate::zotero::search::{SearchContext, SearchError};
use serde_json::Value;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "e4660e05-a935-43ec-8eec-df0347362e4c",
    label: "ERIC",
    creator: "Sebastian Karcher",
    target: "^https?://(www\\.)?eric\\.ed\\.gov/",
    min_version: "3.0",
    priority: 100,
    translator_type: 12,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2024-07-05 11:56:39",
};

/// `getType` (:44-66).
fn get_type(eric_id: &str, publication_type: &str) -> &'static str {
    if eric_id.starts_with("ED") {
        if !publication_type.is_empty() {
            if publication_type.contains("Books") {
                "book"
            } else if publication_type.contains("Dissertations/Theses") {
                "thesis"
            } else {
                "report"
            }
        } else {
            "report"
        }
    } else {
        "journalArticle"
    }
}

/// `cleanInput` (:161-176) for a search item: the first `E[DJ]\d+` in
/// `ericNumber` (a string), or `None` (upstream `false`).
fn clean_input(search: &JsObject) -> Option<String> {
    let s = search.get_str("ericNumber")?;
    let c: Vec<char> = s.chars().collect();
    (0..c.len()).find_map(|i| {
        if c[i] != 'E' || !matches!(c.get(i + 1), Some('D' | 'J')) {
            return None;
        }
        let mut e = i + 2;
        while e < c.len() && c[e].is_ascii_digit() {
            e += 1;
        }
        (e > i + 2).then(|| c[i..e].iter().collect())
    })
}

/// `detectSearch` (:178-180).
pub fn detect_search(search: &JsObject) -> bool {
    clean_input(search).is_some()
}

fn type_error(what: &str) -> SearchError {
    SearchError::Translator(format!(
        "TypeError: Cannot read properties of undefined ({what})"
    ))
}

/// `a.join(sep)` of a JSON array (`null`/`undefined` elements as "").
fn join(v: &Value, sep: &str) -> Option<String> {
    v.as_array().map(|a| {
        a.iter()
            .map(|x| {
                if x.is_null() {
                    String::new()
                } else {
                    js::to_js_string(x)
                }
            })
            .collect::<Vec<_>>()
            .join(sep)
    })
}

fn truthy(v: Option<&Value>) -> bool {
    js::truthy(v)
}

/// `str.match(/(v\d+)?\s*(n\d+)?\s*(p[\d-]+)?/)`: always matches at 0;
/// the three optional groups.
fn source_id_groups(s: &str) -> [Option<String>; 3] {
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    let group = |i: &mut usize, lead: char, more: fn(char) -> bool| -> Option<String> {
        if c.get(*i) == Some(&lead) && c.get(*i + 1).is_some_and(|&x| more(x)) {
            let start = *i;
            *i += 1;
            while c.get(*i).is_some_and(|&x| more(x)) {
                *i += 1;
            }
            Some(c[start..*i].iter().collect())
        } else {
            None
        }
    };
    let skip_ws = |i: &mut usize| {
        while c.get(*i).is_some_and(|&x| js::is_space(x)) {
            *i += 1;
        }
    };
    let v = group(&mut i, 'v', |x| x.is_ascii_digit());
    skip_ws(&mut i);
    let n = group(&mut i, 'n', |x| x.is_ascii_digit());
    skip_ws(&mut i);
    let p = group(&mut i, 'p', |x| x.is_ascii_digit() || x == '-');
    [v, n, p]
}

/// `x && x.substring(1)`.
fn rest(g: Option<String>) -> Value {
    g.map_or(Value::Null, |s| Value::String(s.chars().skip(1).collect()))
}

/// `doSearch` (:182-229).
pub fn do_search(ctx: &mut SearchContext, search: &JsObject) -> Result<(), SearchError> {
    // `search = cleanInput(search)`: a non-matching item makes
    // `search.ericNumber` a TypeError (false has no property; reads undefined).
    let eric_number = clean_input(search).unwrap_or_else(|| "undefined".to_owned());
    let body = ctx.request_json(
        &format!("https://api.ies.ed.gov/eric/?search=id:{eric_number}&format=json&fields=*"),
        &RequestOptions::default(),
    )?;
    let response = body
        .get("response")
        .filter(|v| !v.is_null())
        .ok_or_else(|| type_error("reading 'docs'"))?;
    let docs = response.get("docs");
    let docs_len = docs.and_then(Value::as_array).map(Vec::len);
    if !truthy(docs) || docs_len == Some(0) {
        return Err(SearchError::Translator(
            "ERIC search returned no results".to_owned(),
        ));
    }
    let doc = docs
        .and_then(|d| d.get(0))
        .filter(|d| !d.is_null())
        .ok_or_else(|| type_error("reading 'id'"))?;
    let get = |k: &str| doc.get(k).filter(|v| !v.is_null());
    let id = get("id")
        .map(js::to_js_string)
        .ok_or_else(|| type_error("reading 'startsWith'"))?;
    let pub_type = get("publicationtype")
        .and_then(|v| join(v, "; "))
        .ok_or_else(|| type_error("reading 'join'"))?;
    let mut item = TranslatorItem::new(get_type(&id, &pub_type));
    item.set(
        "title",
        get("title").map_or(Value::Null, |t| {
            Value::String(unescape_html(&js::to_js_string(t)))
        }),
    );
    let authors = get("author")
        .and_then(Value::as_array)
        .ok_or_else(|| type_error("reading 'map'"))?;
    for name in authors {
        let a = clean_author(&js::to_js_string(name), "author", true);
        item.creators.push(TranslatorCreator {
            first_name: a.first_name,
            last_name: Some(a.last_name),
            creator_type: Some(a.creator_type),
            ..Default::default()
        });
    }
    let abs = match get("description") {
        Some(d) if truthy(Some(d)) => Some(d.clone()),
        _ => get("desc").cloned(),
    };
    item.set("abstractNote", abs.clone().unwrap_or(Value::Null));
    if truthy(abs.as_ref()) {
        let a = js::to_js_string(abs.as_ref().unwrap_or(&Value::Null));
        item.set("abstractNote", unescape_html(&a));
    }
    item.set(
        "ISBN",
        match get("isbn") {
            Some(v) if truthy(Some(v)) => join(v, " ")
                .and_then(|s| clean_isbn(&s, false))
                .map_or(Value::Null, Value::String),
            _ => Value::Null,
        },
    );
    item.set(
        "ISSN",
        match get("issn") {
            Some(v) if truthy(Some(v)) => join(v, " ")
                .and_then(|s| clean_issn(&s.replace("ISSN-", "")))
                .map_or(Value::Null, Value::String),
            _ => Value::Null,
        },
    );
    let doi = match get("url") {
        Some(u) if truthy(Some(u)) => {
            let decoded = decode_uri_component(&js::to_js_string(u))
                .ok_or_else(|| SearchError::Translator("URIError: URI malformed".to_owned()))?;
            clean_doi(&decoded).map_or(Value::Null, Value::String)
        }
        _ => Value::Null,
    };
    item.set("DOI", doi);
    item.set(
        "language",
        match get("language") {
            Some(l) if truthy(Some(l)) => l.get(0).cloned().unwrap_or(Value::Null),
            _ => Value::Null,
        },
    );
    let date_src = match get("publicationdate") {
        Some(d) if truthy(Some(d)) => Some(d.clone()),
        _ => get("publicationdateyear").cloned(),
    };
    // strToDate takes only strings and numbers.
    let date = match &date_src {
        Some(v @ (Value::String(_) | Value::Number(_))) => ctx.str_to_iso(&js::to_js_string(v)),
        _ => None,
    };
    item.set("date", date.map_or(Value::Null, Value::String));
    item.set(
        "publisher",
        match get("publisher") {
            Some(p) if truthy(Some(p)) => {
                let s = js::to_js_string(p);
                Value::String(s.split(". ").next().unwrap_or("").to_owned())
            }
            _ => Value::Null,
        },
    );
    item.set("numPages", get("pagecount").cloned().unwrap_or(Value::Null));
    if item.item_type == "report" {
        item.set(
            "institution",
            get("institution").cloned().unwrap_or(Value::Null),
        );
    } else {
        let title = match get("source") {
            Some(s) if truthy(Some(s)) => s.clone(),
            _ => get("institution").cloned().unwrap_or(Value::Null),
        };
        item.set("publicationTitle", title);
        if let Some(sid) = get("sourceid").filter(|v| truthy(Some(v))) {
            let [v, n, p] = source_id_groups(&js::to_js_string(sid));
            item.set("volume", rest(v));
            item.set("issue", rest(n));
            item.set("pages", rest(p));
        }
    }
    item.set("extra", format!("ERIC Number: {id}"));
    let subjects = get("subject")
        .and_then(Value::as_array)
        .ok_or_else(|| type_error("reading 'map'"))?;
    item.tags = subjects
        .iter()
        .map(|s| TranslatorTag::new(js::to_js_string(s)))
        .collect();
    if truthy(get("e_fulltextauth")) {
        let mut a = JsObject::new();
        a.set("title", "Full Text PDF");
        a.set("mimeType", "application/pdf");
        a.set(
            "url",
            format!("https://files.eric.ed.gov/fulltext/{id}.pdf"),
        );
        item.attachments.push(a);
        item.set("url", format!("https://eric.ed.gov/?id={id}"));
    }
    ctx.complete(item)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_id_like_upstream() {
        assert_eq!(
            source_id_groups("v35 n4 p19-26 Mar 2015"),
            [Some("v35".into()), Some("n4".into()), Some("p19-26".into())]
        );
        assert_eq!(source_id_groups("Paper presented"), [None, None, None]);
        let mut s = JsObject::new();
        s.set("ericNumber", "see ED616685.");
        assert_eq!(clean_input(&s).as_deref(), Some("ED616685"));
    }
}
