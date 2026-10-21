// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Crossref REST.js" (translatorID
//   0a61e167-de9a-4f93-a68a-628b48855909, lastUpdated 2025-08-03 05:38:26):
//   `removeUnsupportedMarkup` :50-73, `decodeEntities` :75-83,
//   `fixAuthorCapitalization` :85-99, `parseCreators` :101-134,
//   `parseDate` :136-154, `processCrossref` :156-355, `detectSearch`
//   :357-359, `doSearch` :361-398.
// Copyright (c) 2018 Martynas Bagdonas.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The Crossref REST search translator: `api.crossref.org/works?filter=doi:`
//! mapped to an item. Its `detectSearch` is `false` upstream, so identifier
//! lookup never picks it; it runs only when called by translator ID
//! ([`crate::zotero::search::LookupSession::forced`]).
//!
//! Item properties are created in upstream's assignment order, with an
//! assignment of `undefined`/`null` kept as a `null` placeholder (dropped by
//! `_itemDone` as upstream drops it), because `itemToAPIJSON` reads the
//! properties in order and base-field mapping (publisher/institution/
//! university) depends on it.

use crate::zotero::framework::item::{JsObject, TranslatorCreator, TranslatorItem, TranslatorNote};
use crate::zotero::framework::js;
use crate::zotero::framework::options::TranslatorMetadata;
use crate::zotero::framework::utilities::{capitalize_name, clean_doi};
use crate::zotero::search::http::{encode_uri_component, HttpFailure};
use crate::zotero::search::{SearchContext, SearchError};
use regex::{Captures, Regex};
use serde_json::Value;
use std::sync::OnceLock;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "0a61e167-de9a-4f93-a68a-628b48855909",
    label: "Crossref REST",
    creator: "Martynas Bagdonas",
    target: "",
    min_version: "5.0.0",
    priority: 90,
    translator_type: 8,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2025-08-03 05:38:26",
};

fn re(cell: &'static OnceLock<Regex>, p: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(p).expect("static regex"))
}

/// `removeUnsupportedMarkup` (:50-73).
fn remove_unsupported_markup(text: &str) -> String {
    static CDATA: OnceLock<Regex> = OnceLock::new();
    static MARKUP: OnceLock<Regex> = OnceLock::new();
    let text = re(&CDATA, r"<!\[CDATA\[((?s:.)*?)\]\]>").replace_all(text, "$1");
    re(&MARKUP, r"<(/?)([A-Za-z0-9_]+)[^<>]*>")
        .replace_all(&text, |c: &Captures| {
            let close = !c[1].is_empty();
            let name = c[2].to_lowercase();
            if ["i", "b", "sub", "sup", "span", "sc"].contains(&name.as_str()) {
                format!("{}{}>", if close { "</" } else { "<" }, name)
            } else if name == "scp" {
                if close {
                    "</span>".to_owned()
                } else {
                    "<span style=\"font-variant:small-caps;\">".to_owned()
                }
            } else {
                String::new()
            }
        })
        .into_owned()
}

/// `decodeEntities` (:75-83).
fn decode_entities(n: &str) -> String {
    static E: OnceLock<Regex> = OnceLock::new();
    let n = n.replace('\n', "");
    re(&E, "(&quot;|&lt;|&gt;|&amp;)")
        .replace_all(&n, |c: &Captures| match &c[1] {
            "&quot;" => "\"",
            "&lt;" => "<",
            "&gt;" => ">",
            _ => "&",
        })
        .into_owned()
}

/// `fixAuthorCapitalization` (:85-99): `ZU.capitalizeName` exists in the
/// server's utilities; a non-string comes back as it is.
fn fix_author_capitalization(v: Option<&Value>) -> Option<String> {
    match v {
        Some(Value::String(s)) => Some(capitalize_name(s)),
        Some(Value::Null) | None => None,
        Some(other) => Some(js::to_js_string(other)),
    }
}

/// `a && b`: `b` when `a` is truthy, else `a`.
fn and(a: Option<&Value>, b: impl FnOnce(&Value) -> Value) -> Value {
    match a {
        Some(v) if js::truthy(Some(v)) => b(v),
        Some(v) => v.clone(),
        None => Value::Null,
    }
}

fn val(v: Option<&Value>) -> Value {
    v.cloned().unwrap_or(Value::Null)
}

/// `x[0]` of an array-valued property.
fn first(v: Option<&Value>) -> Option<&Value> {
    v.and_then(|a| a.get(0))
}

fn truthy(v: Option<&Value>) -> bool {
    js::truthy(v)
}

/// `parseCreators` (:101-134).
fn parse_creators(result: &Value, item: &mut TranslatorItem, overrides: &[(&str, &str)]) {
    for ty in ["author", "editor", "chair", "translator"] {
        let Some(list) = result.get(ty).filter(|v| truthy(Some(v))) else {
            continue;
        };
        let creator_type = overrides
            .iter()
            .find(|(k, _)| *k == ty)
            .map(|(_, v)| (*v).to_owned())
            .unwrap_or_else(|| {
                if ty == "author" || ty == "editor" || ty == "translator" {
                    ty.to_owned()
                } else {
                    "contributor".to_owned()
                }
            });
        if creator_type.is_empty() {
            continue;
        }
        for creator in list.as_array().map(Vec::as_slice).unwrap_or(&[]) {
            let mut c = TranslatorCreator {
                creator_type: Some(creator_type.clone()),
                ..Default::default()
            };
            if truthy(creator.get("name")) {
                c.field_mode = Some(1);
                c.last_name = creator.get("name").map(js::to_js_string);
            } else {
                c.first_name = fix_author_capitalization(creator.get("given"));
                c.last_name = fix_author_capitalization(creator.get("family"));
                if c.first_name.as_deref().is_none_or(str::is_empty) {
                    c.field_mode = Some(1);
                }
            }
            item.creators.push(c);
        }
    }
}

/// `x.toString().padStart(2, '0')`.
fn pad2(v: &Value) -> String {
    let s = js::to_js_string(v);
    if s.chars().count() < 2 {
        format!("{}{s}", "0".repeat(2 - s.chars().count()))
    } else {
        s
    }
}

/// `parseDate` (:136-154).
fn parse_date(date: Option<&Value>) -> Option<String> {
    let parts = date?
        .get("date-parts")?
        .get(0)
        .filter(|p| truthy(Some(p)))?;
    let get = |i: usize| parts.get(i).filter(|v| truthy(Some(v)));
    let year = get(0)?;
    Some(match (get(1), get(2)) {
        (Some(m), Some(d)) => format!("{}-{}-{}", js::to_js_string(year), pad2(m), pad2(d)),
        (Some(m), None) => format!("{}/{}", pad2(m), js::to_js_string(year)),
        (None, _) => js::to_js_string(year),
    })
}

fn opt_str(s: Option<String>) -> Value {
    s.map_or(Value::Null, Value::String)
}

fn type_error(what: &str) -> SearchError {
    SearchError::Translator(format!(
        "TypeError: Cannot read properties of undefined ({what})"
    ))
}

/// `processCrossref` (:156-355).
fn process_crossref(ctx: &mut SearchContext, text: &str) -> Result<(), SearchError> {
    let json: Value = serde_json::from_str(text)
        .map_err(|e| SearchError::Http(HttpFailure::Malformed(format!("JSON: {e}"))))?;
    let items = json
        .get("message")
        .ok_or_else(|| type_error("reading 'items'"))?
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| type_error("result is not iterable"))?;
    // Declared outside the loop upstream: an override persists to later
    // results.
    let mut overrides: Vec<(&str, &str)> = Vec::new();
    for result in items {
        let ty = result.get("type").and_then(Value::as_str).unwrap_or("");
        let has = |k: &str| truthy(result.get(k));
        let mut item;
        if [
            "journal",
            "journal-article",
            "journal-volume",
            "journal-issue",
        ]
        .contains(&ty)
        {
            item = TranslatorItem::new("journalArticle");
        } else if ["report", "report-series", "report-component"].contains(&ty) {
            item = TranslatorItem::new("report");
        } else if [
            "book",
            "book-series",
            "book-set",
            "book-track",
            "monograph",
            "reference-book",
            "edited-book",
        ]
        .contains(&ty)
        {
            item = TranslatorItem::new("book");
        } else if [
            "book-chapter",
            "book-part",
            "book-section",
            "reference-entry",
        ]
        .contains(&ty)
        {
            item = TranslatorItem::new("bookSection");
            overrides = vec![("author", "bookAuthor")];
        } else if ty == "other" && has("ISBN") && has("container-title") {
            item = TranslatorItem::new("bookSection");
            let ct = result.get("container-title");
            let len = ct.and_then(Value::as_array).map_or(0, Vec::len);
            if len >= 2 {
                item.set("seriesTitle", val(ct.and_then(|a| a.get(0))));
                item.set("bookTitle", val(ct.and_then(|a| a.get(1))));
            } else {
                item.set("bookTitle", val(first(ct)));
            }
            overrides = vec![("author", "bookAuthor")];
        } else if ty == "standard" {
            item = TranslatorItem::new("standard");
        } else if ["dataset", "database"].contains(&ty) {
            item = TranslatorItem::new("dataset");
        } else if ["proceedings", "proceedings-article", "proceedings-series"].contains(&ty) {
            item = TranslatorItem::new("conferencePaper");
        } else if ty == "dissertation" {
            item = TranslatorItem::new("thesis");
            item.set("date", opt_str(parse_date(result.get("approved"))));
            let degree = result.get("degree");
            let thesis_type = and(degree, |d| {
                and(d.get(0), |d0| {
                    static P: OnceLock<Regex> = OnceLock::new();
                    Value::String(
                        re(&P, r"\([^\n\r\u{2028}\u{2029}]+\)")
                            .replace(&js::to_js_string(d0), "")
                            .into_owned(),
                    )
                })
            });
            item.set("thesisType", thesis_type);
        } else if ty == "posted-content" {
            if result.get("subtype").and_then(Value::as_str) == Some("preprint") {
                item = TranslatorItem::new("preprint");
                item.set("repository", val(result.get("group-title")));
            } else {
                item = TranslatorItem::new("blogPost");
                let inst = result.get("institution");
                if truthy(inst)
                    && inst
                        .and_then(Value::as_array)
                        .is_some_and(|a| !a.is_empty())
                {
                    let name = first(inst).and_then(|i| i.get("name"));
                    item.set("blogTitle", and(name, |n| n.clone()));
                }
            }
        } else if ty == "peer-review" {
            item = TranslatorItem::new("manuscript");
            item.set("type", "peer review");
            if !has("author") {
                item.creators
                    .push(TranslatorCreator::single("Anonymous Reviewer", "author"));
            }
            let review_of = result.get("relation").and_then(|r| r.get("is-review-of"));
            if has("relation")
                && truthy(review_of)
                && review_of
                    .and_then(Value::as_array)
                    .is_some_and(|a| !a.is_empty())
            {
                let r0 = first(review_of);
                let id_type = r0.and_then(|r| r.get("id-type")).and_then(Value::as_str);
                let id = r0
                    .and_then(|r| r.get("id"))
                    .map(js::to_js_string)
                    .unwrap_or_else(|| "undefined".to_owned());
                let identifier = match id_type {
                    Some("doi") => {
                        format!("<a href=\"https://doi.org/{id}\">https://doi.org/{id}</a>")
                    }
                    Some("url") => format!("<a href=\"{id}\">{id}</a>"),
                    _ => id,
                };
                item.notes
                    .push(TranslatorNote::new(format!("Review of {identifier}")));
            }
        } else {
            item = TranslatorItem::new("document");
        }

        parse_creators(result, &mut item, &overrides);

        if has("description") {
            item.notes.push(TranslatorNote::new(js::to_js_string(&val(
                result.get("description")
            ))));
        }

        item.set(
            "abstractNote",
            and(result.get("abstract"), |a| {
                Value::String(remove_unsupported_markup(&js::to_js_string(a)))
            }),
        );
        let pages = match result.get("page") {
            Some(p) if truthy(Some(p)) => p.clone(),
            _ => val(result.get("article-number")),
        };
        item.set("pages", pages);
        let join = |v: &Value| {
            Value::String(
                v.as_array()
                    .map(|a| {
                        a.iter()
                            .map(js::to_js_string)
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default(),
            )
        };
        item.set("ISBN", and(result.get("ISBN"), join));
        item.set("ISSN", and(result.get("ISSN"), join));
        item.set("issue", val(result.get("issue")));
        item.set("volume", val(result.get("volume")));
        item.set("language", val(result.get("language")));
        item.set("edition", val(result.get("edition-number")));
        // `item.university = item.institution = item.publisher = ...`:
        // created publisher, institution, university.
        let publisher = val(result.get("publisher"));
        item.set("publisher", publisher.clone());
        item.set("institution", publisher.clone());
        item.set("university", publisher);

        let ct = result.get("container-title");
        let ct0 = first(ct);
        if truthy(ct) && truthy(ct0) {
            let field = match item.item_type.as_str() {
                "journalArticle" => "publicationTitle",
                "conferencePaper" => "proceedingsTitle",
                "book" => "series",
                "bookSection" => "bookTitle",
                _ => "seriesTitle",
            };
            item.set(field, val(ct0));
        }

        let event = result.get("event");
        item.set("conferenceName", and(event, |e| val(e.get("name"))));

        let sct = result.get("short-container-title");
        if truthy(sct) {
            if !truthy(ct) && ct.is_none_or(Value::is_null) {
                return Err(type_error("reading '0'"));
            }
            if first(sct) != ct0 && !(first(sct).is_none() && ct0.is_none()) {
                item.set("journalAbbreviation", val(first(sct)));
            }
        }

        let inst = result.get("institution");
        let inst0 = first(inst).filter(|v| truthy(Some(v)));
        if truthy(event) && truthy(event.and_then(|e| e.get("location"))) {
            item.set("place", val(event.and_then(|e| e.get("location"))));
        } else if truthy(inst) && inst0.is_some() && truthy(inst0.and_then(|i| i.get("place"))) {
            let place = inst0
                .and_then(|i| i.get("place"))
                .map(join)
                .unwrap_or(Value::Null);
            item.set("place", place);
        } else {
            item.set("place", val(result.get("publisher-location")));
        }

        // `item.institution = item.university = result.institution &&
        // result.institution[0] && result.institution[0].name`.
        let inst_name = and(inst, |i| and(i.get(0), |i0| val(i0.get("name"))));
        item.set("university", inst_name.clone());
        item.set("institution", inst_name);

        if let Some(d) = parse_date(result.get("published-print")) {
            item.set("date", d);
        } else if let Some(d) = parse_date(result.get("issued")) {
            item.set("date", d);
        }

        item.set("DOI", val(result.get("DOI")));
        let resource = result.get("resource");
        item.set(
            "url",
            and(resource, |r| and(r.get("primary"), |p| val(p.get("URL")))),
        );
        let license = result.get("license");
        item.set(
            "rights",
            and(license, |l| and(l.get(0), |l0| val(l0.get("URL")))),
        );

        let title = result.get("title");
        if truthy(title) && truthy(first(title)) {
            let mut t = js::to_js_string(&val(first(title)));
            let sub = result.get("subtitle");
            if truthy(sub) && truthy(first(sub)) {
                let s0 = js::to_js_string(&val(first(sub)));
                if !t.to_lowercase().contains(&s0.to_lowercase()) {
                    if t.encode_utf16().last() != Some(u16::from(b':')) {
                        t.push(':');
                    }
                    t.push(' ');
                    t.push_str(&s0);
                }
            }
            item.set("title", remove_unsupported_markup(&t));
        }
        if !item.truthy("title") {
            item.set("title", "[No title found]");
        }

        // :334-351: repair mis-decoded UTF-8, then decode entities, on every
        // string property (itemType included: `for (let field in item)`).
        item.item_type = repair(&item.item_type);
        let keys: Vec<String> = item.props.keys().map(str::to_owned).collect();
        for k in keys {
            if let Some(Value::String(s)) = item.props.get(&k) {
                let fixed = repair(s);
                item.props.set(k, fixed);
            }
        }
        item.set("libraryCatalog", "Crossref");
        ctx.complete(item)?;
    }
    Ok(())
}

/// One field of the loop at :334-351.
fn repair(s: &str) -> String {
    let mut s = s.to_owned();
    if s.chars().any(|c| ('\u{7F}'..='\u{9F}').contains(&c)) {
        // decodeURIComponent(escape(s)): escape() of a character above
        // U+00FF gives %uXXXX, which decodeURIComponent rejects; otherwise the
        // characters are the bytes, decoded as UTF-8 (rejecting invalid).
        let latin1: Option<Vec<u8>> = s.chars().map(|c| u8::try_from(u32::from(c)).ok()).collect();
        s = match latin1.and_then(|b| String::from_utf8(b).ok()) {
            Some(d) => d,
            None => s
                .chars()
                .filter(|c| {
                    !(('\u{0}'..='\u{1F}').contains(c) || ('\u{7F}'..='\u{9F}').contains(c))
                })
                .collect(),
        };
    }
    decode_entities(&s)
}

/// `detectSearch` (:357-359).
pub fn detect_search(_search: &JsObject) -> bool {
    false
}

/// `doSearch` (:361-398).
pub fn do_search(ctx: &mut SearchContext, search: &JsObject) -> Result<(), SearchError> {
    let mut query = if search.truthy("DOI") {
        match search.get("DOI") {
            Some(Value::Array(a)) => format!(
                "?filter=doi:{}",
                a.iter()
                    .filter_map(|x| clean_doi(&js::to_js_string(x)))
                    .collect::<Vec<_>>()
                    .join(",doi:")
            ),
            Some(v) => format!(
                "?filter=doi:{}",
                clean_doi(&js::to_js_string(v)).unwrap_or_else(|| "null".to_owned())
            ),
            None => unreachable!("truthy"),
        }
    } else if search.truthy("query") {
        format!(
            "?query.bibliographic={}",
            encode_uri_component(
                &search
                    .get("query")
                    .map(js::to_js_string)
                    .unwrap_or_default()
            )
        )
    } else {
        return Ok(());
    };
    if let Some(email) = ctx
        .get_hidden_pref("CrossrefREST.email")
        .filter(|v| js::truthy(Some(v)))
    {
        query.push_str("&mailto=");
        query.push_str(&js::to_js_string(&email));
    }
    let text = ctx.do_get(&format!("https://api.crossref.org/works/{query}"))?;
    process_crossref(ctx, &text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markup_and_entities() {
        assert_eq!(
            remove_unsupported_markup(
                "<jats:p>A <jats:italic>x</jats:italic> <scp>Y</scp><![CDATA[z]]></jats:p>"
            ),
            "A x <span style=\"font-variant:small-caps;\">Y</span>z"
        );
        assert_eq!(decode_entities("a&amp;b\n&lt;"), "a&b<");
        assert_eq!(repair("\u{e2}\u{80}\u{93}"), "\u{2013}");
    }
}
