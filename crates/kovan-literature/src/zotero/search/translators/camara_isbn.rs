// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Camara Brasileira do Livro ISBN.js" (translatorID
//   cdb5c893-ab69-4e96-9b5c-f4456d49ddd8, lastUpdated 2023-09-26 16:11:18).
// Copyright (c) 2023 Abe Jellinek.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).
// Ported: `detectSearch` :37-40, `doSearch` :42-77, `translateResult`
//   :79-178, `fixCase` :180-185, `cleanData` :187-206.

//! The Câmara Brasileira do Livro ISBN search translator: Brazilian ISBNs
//! (groups 65 and 85) looked up in the CBL's ISBN search index (an Azure
//! Cognitive Search endpoint), each result mapped to a book.
//!
//! **Deviation from upstream (DATA_POLICY.md, #756).** Upstream sends the
//! index's query key as an `api-key` header written into the translator.
//! The workspace's data policy forbids API keys in the repository, so the
//! port does not carry it: the key is read from the hidden preference
//! `CamaraBrasileiraDoLivro.apiKey` ([`SearchContext::get_hidden_pref`]),
//! which the user sets. Unset, the translator fails with a message saying
//! so, and an identifier search goes on to the next ISBN translator.
//! Everything else (URL, body, other headers, the mapping) is upstream's.
//!
//! Not reproduced: `cleanData` in `detectSearch` also overwrites the
//! shared search item's `ISBN` with its cleaned form (upstream passes the
//! same object to every translator); the port's detection does not mutate
//! the search item. An identifier search's ISBN is already clean, so the
//! two agree on every input `extractIdentifiers` produces.

use crate::zotero::framework::identifiers::{clean_isbn, to_isbn13};
use crate::zotero::framework::item::{JsObject, TranslatorCreator, TranslatorItem, TranslatorTag};
use crate::zotero::framework::js;
use crate::zotero::framework::options::TranslatorMetadata;
use crate::zotero::framework::title_case::capitalize_title;
use crate::zotero::framework::utilities::{clean_author, remove_diacritics};
use crate::zotero::search::http::RequestOptions;
use crate::zotero::search::{SearchContext, SearchError};
use regex::{Captures, Regex};
use serde_json::Value;
use std::sync::OnceLock;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "cdb5c893-ab69-4e96-9b5c-f4456d49ddd8",
    label: "Câmara Brasileira do Livro ISBN",
    creator: "Abe Jellinek",
    target: "",
    min_version: "5.0",
    priority: 98,
    translator_type: 8,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2023-09-26 16:11:18",
};

/// The hidden preference holding the index's query key.
pub const API_KEY_PREF: &str = "CamaraBrasileiraDoLivro.apiKey";

/// `cleanData` (:187-206) on one search item: its cleaned ISBN, when it is
/// a Brazilian one.
fn clean_data(search: &JsObject) -> Vec<String> {
    let Some(v) = search.get("ISBN").filter(|v| js::truthy(Some(v))) else {
        return Vec::new();
    };
    let Some(isbn) = clean_isbn(&js::to_js_string(v), false) else {
        return Vec::new();
    };
    if ["97865", "65", "97885", "85"]
        .iter()
        .any(|p| isbn.starts_with(p))
    {
        vec![isbn]
    } else {
        Vec::new()
    }
}

/// `detectSearch` (:37-40).
pub fn detect_search(search: &JsObject) -> bool {
    !clean_data(search).is_empty()
}

/// `doSearch` (:42-77).
pub fn do_search(ctx: &mut SearchContext, search: &JsObject) -> Result<(), SearchError> {
    for isbn in clean_data(search) {
        let mut q = isbn.clone();
        if isbn.len() == 10 {
            q.push_str(" OR ");
            q.push_str(
                &to_isbn13(&isbn).ok_or_else(|| {
                    SearchError::Translator(format!("ISBN not found in \"{isbn}\""))
                })?,
            );
        }
        // JSON.stringify of the body object, keys in upstream's order.
        let body = format!(
            r#"{{"count":true,"facets":[],"filter":"","orderby":null,"queryType":"full","search":{},"searchFields":"FormattedKey,RowKey","searchMode":"any","select":"*","skip":0,"top":1}}"#,
            Value::String(q)
        );
        let key = match ctx.get_hidden_pref(API_KEY_PREF) {
            Some(Value::String(k)) if !k.is_empty() => k,
            _ => {
                return Err(SearchError::Translator(format!(
                    "Câmara Brasileira do Livro ISBN: no api-key configured; set the hidden preference {API_KEY_PREF} (kovan does not ship upstream's key)"
                )))
            }
        };
        let response = ctx.request_json(
            "https://isbn-search-br.search.windows.net/indexes/isbn-index/docs/search?api-version=2016-09-01",
            &RequestOptions::post(
                body,
                &[
                    ("Content-Type", "application/json; charset=UTF-8"),
                    ("api-key", key.as_str()),
                    ("Origin", "https://www.cblservicos.org.br"),
                    ("Referer", "https://www.cblservicos.org.br/"),
                ],
            ),
        )?;
        let results = response
            .get("value")
            .and_then(Value::as_array)
            .ok_or_else(|| type_error("response.value is not iterable"))?;
        for result in results {
            translate_result(ctx, result)?;
        }
    }
    Ok(())
}

fn type_error(m: &str) -> SearchError {
    SearchError::Translator(format!("TypeError: {m}"))
}

/// `fixCase` (:180-185): an all-upper-case string, title-cased.
fn fix_case(s: Option<&Value>) -> Option<Value> {
    match s {
        Some(Value::String(t)) if !t.is_empty() && *t == t.to_uppercase() => {
            Some(Value::String(capitalize_title(t, true)))
        }
        other => other.cloned(),
    }
}

/// `Zotero.Utilities.capitalizeName` (utilities.js:199-216).
fn capitalize_name(s: &str) -> String {
    static R: OnceLock<Regex> = OnceLock::new();
    let r = R.get_or_init(|| Regex::new(r"(^|[^\p{L}])\p{L}").expect("static regex"));
    s.split(' ')
        .map(|part| {
            if part.to_uppercase() == part || part.to_lowercase() == part {
                r.replace_all(&part.to_lowercase(), |c: &Captures| c[0].to_uppercase())
                    .into_owned()
            } else {
                part.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// JavaScript `a == '1'` for a JSON value (loose equality with a string).
fn loose_eq_str(v: Option<&Value>, s: &str) -> bool {
    match v {
        Some(Value::String(t)) => t == s,
        Some(Value::Number(n)) => s.parse::<f64>().ok() == n.as_f64(),
        Some(Value::Bool(b)) => s.parse::<f64>().ok() == Some(if *b { 1.0 } else { 0.0 }),
        _ => false,
    }
}

fn set_opt(item: &mut TranslatorItem, k: &str, v: Option<Value>) {
    match v {
        Some(v) => item.set(k, v),
        None => {
            item.remove(k);
        }
    }
}

/// `translateResult` (:79-178).
fn translate_result(ctx: &mut SearchContext, result: &Value) -> Result<(), SearchError> {
    let g = |k: &str| result.get(k);
    let mut item = TranslatorItem::new("book");
    let mut title = g("Title").cloned();
    let subtitle = g("Subtitle").filter(|v| js::truthy(Some(v)));
    if let Some(sub) = subtitle {
        let t = match &title {
            Some(Value::String(t)) => t.clone(),
            _ => {
                return Err(type_error(
                    "Cannot read properties of undefined (reading 'includes')",
                ))
            }
        };
        let sub = js::to_js_string(sub);
        if !t.contains(':') && !sub.contains(':') {
            title = Some(Value::String(format!("{t}: {sub}")));
        }
    }
    set_opt(&mut item, "title", fix_case(title.as_ref()));
    set_opt(&mut item, "abstractNote", g("Sinopse").cloned());
    set_opt(&mut item, "series", fix_case(g("Colection")));
    let edition = g("Edicao");
    if loose_eq_str(edition, "1") {
        item.set("edition", "");
    } else {
        set_opt(&mut item, "edition", edition.cloned());
    }
    let cidade = g("Cidade")
        .filter(|v| js::truthy(Some(v)))
        .map_or(String::new(), js::to_js_string);
    let uf = g("UF")
        .filter(|v| js::truthy(Some(v)))
        .map_or(String::new(), |u| format!(", {}", js::to_js_string(u)));
    item.set("place", format!("{cidade}{uf}"));
    set_opt(&mut item, "publisher", fix_case(g("Imprint")));
    // ZU.strToISO(result.Date): `false` (dropped) when it finds no year.
    let date = g("Date")
        .filter(|v| !v.is_null())
        .and_then(|d| ctx.str_to_iso(&js::to_js_string(d)));
    set_opt(&mut item, "date", date.map(Value::String));
    let pages = g("Paginas");
    if loose_eq_str(pages, "0") {
        item.set("numPages", "");
    } else {
        set_opt(&mut item, "numPages", pages.cloned());
    }
    let lang = g("IdiomasObra")
        .filter(|v| js::truthy(Some(v)))
        .and_then(|a| a.get(0))
        .filter(|v| js::truthy(Some(v)))
        .cloned()
        .unwrap_or_else(|| Value::String("pt-BR".to_owned()));
    if lang.as_str() == Some("português (Brasil)") {
        item.set("language", "pt-BR");
    } else {
        item.set("language", lang);
    }
    let isbn = g("FormattedKey")
        .map(js::to_js_string)
        .and_then(|k| clean_isbn(&k, false));
    set_opt(&mut item, "ISBN", isbn.map(Value::String));

    let authors = g("Authors")
        .and_then(Value::as_array)
        .ok_or_else(|| type_error("result.Authors is undefined"))?;
    let profissoes = g("Profissoes").and_then(Value::as_array);
    for (i, author) in authors.iter().enumerate() {
        let mut author = js::to_js_string(author);
        if author == author.to_uppercase() {
            author = capitalize_name(&author);
        }
        let creator_type = match profissoes.filter(|p| p.len() == authors.len()) {
            Some(p) => match p[i].as_str() {
                Some("Coordenador" | "Autor" | "Roteirista") => "author",
                Some("Revisor" | "Organizador" | "Editor") => "editor",
                Some("Tradutor") => "translator",
                Some("Ilustrador" | "Projeto Gráfico") => "illustrator",
                _ => {
                    if i == 0 {
                        "author"
                    } else {
                        "contributor"
                    }
                }
            },
            None if i > 0 => "contributor",
            None => "author",
        };
        let c = clean_author(&author, creator_type, author.contains(','));
        let mut creator = TranslatorCreator {
            first_name: c.first_name.clone(),
            last_name: Some(c.last_name.clone()),
            creator_type: Some(c.creator_type.clone()),
            ..Default::default()
        };
        if c.first_name.as_deref().is_none_or(str::is_empty) {
            creator.field_mode = Some(1);
        }
        if let (Some(first), last) = (c.first_name.as_deref(), c.last_name.as_str()) {
            const SUFFIXES: [&str; 6] =
                ["filho", "junior", "neto", "sobrinho", "segundo", "terceiro"];
            if !first.is_empty()
                && !last.is_empty()
                && SUFFIXES.contains(&remove_diacritics(&last.to_lowercase(), false).as_str())
            {
                // firstName.split(/\s+/)
                let parts = split_ws(first);
                let lastp = parts.last().cloned().unwrap_or_default();
                creator.last_name = Some(format!("{lastp} {last}"));
                creator.first_name = Some(parts[..parts.len() - 1].join(" "));
            }
        }
        item.creators.push(creator);
    }
    if let Some(s) = g("Subject").filter(|v| js::truthy(Some(v))) {
        item.tags.push(TranslatorTag::new(js::to_js_string(s)));
    }
    let keywords = g("PalavrasChave")
        .and_then(Value::as_array)
        .ok_or_else(|| type_error("result.PalavrasChave is not iterable"))?;
    for tag in keywords {
        item.tags.push(TranslatorTag::new(js::to_js_string(tag)));
    }
    ctx.complete(item)
}

/// `s.split(/\s+/)` (JavaScript whitespace; leading/trailing whitespace
/// gives empty pieces, as in JavaScript).
fn split_ws(s: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut in_ws = false;
    for c in s.chars() {
        if js::is_space(c) {
            if !in_ws {
                out.push(String::new());
                in_ws = true;
            }
        } else {
            in_ws = false;
            if let Some(last) = out.last_mut() {
                last.push(c);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zotero::search::{HttpCache, SearchOptions};

    fn isbn(s: &str) -> JsObject {
        let mut o = JsObject::new();
        o.set("ISBN", s);
        o
    }

    #[test]
    fn detects_brazilian_isbns_only() {
        assert!(detect_search(&isbn("8532511015")));
        assert!(detect_search(&isbn("978-65-995947-5-5")));
        assert!(!detect_search(&isbn("9780521779241")));
        assert!(!detect_search(&JsObject::new()));
    }

    #[test]
    fn without_a_configured_key_it_fails_without_a_request() {
        let mut ctx = SearchContext::new(HttpCache::new(), SearchOptions::default());
        let e = do_search(&mut ctx, &isbn("8532511015")).unwrap_err();
        assert!(
            matches!(&e, SearchError::Translator(m) if m.contains(API_KEY_PREF)),
            "{e}"
        );
        assert!(ctx.requests().is_empty());
    }
}
