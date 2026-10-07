// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero utilities, https://github.com/zotero/utilities (commit
//   4051881d59c6, the translation-server's submodule for the references):
//   openurl.js `createContextObject` :35-195 (the string form, `asObj`
//   false) and `parseContextObject` :202-449 (#749, XML ContextObject);
//   utilities_item.js `getFirstCreatorFromItemJSON` :640-656;
//   cachedTypes.js `CreatorTypes.getPrimaryIDForType` :123-127.
//   `encodeURIComponent` is ECMA-262 §19.2.6.5.
// Copyright (c) 2009 Center for History and New Media, George Mason
//   University, Fairfax, Virginia, USA; Corporation for Digital Scholarship.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! `ZU.createContextObject(item, version)`: an OpenURL ContextObject in
//! key-encoded-value form, for an export item (COinS export).
//!
//! ~~Not ported: `parseContextObject` (the reverse direction, used only by
//! COinS' web translator, which is out of scope)~~ **CORRECTED 2026-10-07**
//! (#749): XML ContextObject (an import translator) calls it too;
//! [`parse_context_object`] ports it (openurl.js:202-449). Not ported: the
//! `asObj` form of `createContextObject`, which no ported translator asks
//! for.
//!
//! **Maturity: AI draft (1).**

use super::utilities::{get_creators_for_type, str_to_iso};
use super::{js, TranslatorCreator, TranslatorItem};
use kovan_common::zotero::date::DateOptions;
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

/// JavaScript `encodeURIComponent`: every UTF-8 byte of a character outside
/// `A-Z a-z 0-9 - _ . ! ~ * ' ( )` as `%XX` (upper-case hex).
pub fn encode_uri_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || "-_.!~*'()".contains(c) {
            out.push(c);
        } else {
            let mut buf = [0u8; 4];
            for b in c.encode_utf8(&mut buf).bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

/// The OpenURL version `createContextObject` writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenUrlVersion {
    /// "0.1".
    V0_1,
    /// "1.0".
    V1_0,
}

/// `Zotero.Utilities.Item.getFirstCreatorFromItemJSON(json)`
/// (utilities_item.js:640-656): the first creator of the item type's primary
/// type or `author`, else the first `editor`.
pub fn first_creator(item: &TranslatorItem) -> Option<&TranslatorCreator> {
    let primary = get_creators_for_type(&item.item_type).first().copied();
    item.creators
        .iter()
        .find(|c| {
            let t = c.creator_type.as_deref();
            let is_primary = match primary {
                Some(_) => t == primary,
                // No primary type: `getName(false)` is `false`, and
                // `creatorType == false` holds for "" and "0" (loose
                // equality converts both to 0).
                None => matches!(t, Some("") | Some("0")),
            };
            is_primary || t == Some("author")
        })
        .or_else(|| {
            item.creators
                .iter()
                .find(|c| c.creator_type.as_deref() == Some("editor"))
        })
}

/// `Zotero.Utilities.createContextObject(item, version)` (openurl.js:35-195)
/// on an export item, in string form. `dates` are the options of
/// `Zotero.Date.strToISO`.
pub fn create_context_object(
    item: &TranslatorItem,
    version: OpenUrlVersion,
    dates: &DateOptions,
) -> String {
    let v1 = version == OpenUrlVersion::V1_0;
    let mut entries: Vec<String> = Vec::new();
    // `_mapTag(data, tag, dontAddPrefix)` (:38-49); `data` as JavaScript
    // would see it (`None` is undefined).
    let mut map = |data: Option<Value>, tag: &str, dont_add_prefix: bool| {
        if !js::truthy(data.as_ref()) {
            return;
        }
        let tag = if v1 && !dont_add_prefix {
            format!("rft.{tag}")
        } else {
            tag.to_owned()
        };
        entries.push(format!(
            "{tag}={}",
            encode_uri_component(&js::to_js_string(data.as_ref().unwrap()))
        ));
    };
    let get = |k: &str| item.get(k).cloned();
    let s = |x: &str| Some(Value::String(x.to_owned()));
    // String concatenation `"x" + item.f`.
    let cat = |prefix: &str, k: &str| {
        let v = item.get(k).map_or("undefined".to_owned(), js::to_js_string);
        Some(Value::String(format!("{prefix}{v}")))
    };

    // PMID in Extra (:56-58): `pmidRe.exec(item.extra)`, `extra` as a string.
    static PMID: OnceLock<Regex> = OnceLock::new();
    let pmid_re =
        PMID.get_or_init(|| Regex::new(&format!(r"(?:\n|^)PMID:{}*([0-9]+)", js::WS)).unwrap());
    let extra = item
        .get("extra")
        .map_or("undefined".to_owned(), js::to_js_string);
    let pmid = pmid_re
        .captures(&extra)
        .map(|c| c.get(1).unwrap().as_str().to_owned());

    if !v1 {
        map(s("Zotero:2"), "sid", true);
        if item.truthy("DOI") {
            map(cat("doi:", "DOI"), "id", true);
        }
        if item.truthy("ISBN") {
            map(get("ISBN"), "isbn", true);
        }
        if let Some(p) = &pmid {
            map(s(&format!("pmid:{p}")), "id", true);
        }
    } else {
        map(s("Z39.88-2004"), "url_ver", true);
        map(s("Z39.88-2004"), "ctx_ver", true);
        map(s("info:sid/zotero.org:2"), "rfr_id", true);
        if item.truthy("DOI") {
            map(cat("info:doi/", "DOI"), "rft_id", true);
        }
        if item.truthy("ISBN") {
            map(cat("urn:isbn:", "ISBN"), "rft_id", true);
        }
        if let Some(p) = &pmid {
            map(s(&format!("info:pmid/{p}")), "rft_id", true);
        }
    }

    let t = item.item_type.as_str();
    let title_tag = |v1_tag: &'static str| if v1 { v1_tag } else { "title" };
    if t == "journalArticle" {
        if v1 {
            map(s("info:ofi/fmt:kev:mtx:journal"), "rft_val_fmt", true);
        }
        map(s("article"), "genre", false);
        map(get("title"), "atitle", false);
        map(get("publicationTitle"), title_tag("jtitle"), false);
        map(get("journalAbbreviation"), "stitle", false);
        map(get("volume"), "volume", false);
        map(get("issue"), "issue", false);
    } else if matches!(t, "book" | "bookSection" | "conferencePaper" | "report") {
        if v1 {
            map(s("info:ofi/fmt:kev:mtx:book"), "rft_val_fmt", true);
        }
        if t == "book" {
            map(s("book"), "genre", false);
            map(get("title"), title_tag("btitle"), false);
        } else if t == "conferencePaper" {
            map(s("proceeding"), "genre", false);
            map(get("title"), "atitle", false);
            map(get("proceedingsTitle"), title_tag("btitle"), false);
        } else if t == "report" {
            map(s("report"), "genre", false);
            map(get("seriesTitle"), "series", false);
            map(get("title"), title_tag("btitle"), false);
        } else {
            map(s("bookitem"), "genre", false);
            map(get("title"), "atitle", false);
            map(get("publicationTitle"), title_tag("btitle"), false);
        }
        map(get("place"), "place", false);
        map(get("publisher"), "publisher", false);
        map(get("edition"), "edition", false);
        map(get("series"), "series", false);
    } else if t == "thesis" && v1 {
        map(s("info:ofi/fmt:kev:mtx:dissertation"), "rft_val_fmt", true);
        map(get("title"), "title", false);
        map(get("publisher"), "inst", false);
        map(get("type"), "degree", false);
    } else if t == "patent" && v1 {
        map(s("info:ofi/fmt:kev:mtx:patent"), "rft_val_fmt", true);
        map(get("title"), "title", false);
        map(get("assignee"), "assignee", false);
        map(get("patentNumber"), "number", false);
        if item.truthy("issueDate") {
            map(
                iso_of(item.get("issueDate"), dates).map(Value::String),
                "date",
                false,
            );
        }
    } else {
        map(s("info:ofi/fmt:kev:mtx:dc"), "rft_val_fmt", true);
        map(s(t), "type", false);
        map(get("title"), "title", false);
        map(get("publicationTitle"), "source", false);
        map(get("rights"), "rights", false);
        map(get("publisher"), "publisher", false);
        map(get("abstractNote"), "description", false);
        if item.truthy("DOI") {
            map(cat("urn:doi:", "DOI"), "identifier", false);
        } else if item.truthy("url") {
            map(get("url"), "identifier", false);
        }
    }

    if !item.creators.is_empty() {
        let opt = |o: &Option<String>| o.clone().map(Value::String);
        // `firstCreator` may be `false`, whose properties are undefined.
        let first = first_creator(item);
        if t == "patent" {
            map(first.and_then(|c| opt(&c.first_name)), "invfirst", false);
            map(first.and_then(|c| opt(&c.last_name)), "invlast", false);
        } else {
            // `isInstitution` is never set on an export item's creator.
            map(first.and_then(|c| opt(&c.first_name)), "aufirst", false);
            map(first.and_then(|c| opt(&c.last_name)), "aulast", false);
        }
        for c in &item.creators {
            let first = match c.first_name.as_deref() {
                Some(f) if !f.is_empty() => format!("{f} "),
                _ => String::new(),
            };
            let last = c.last_name.as_deref().unwrap_or("undefined");
            map(
                s(&format!("{first}{last}")),
                if t == "patent" { "inventor" } else { "au" },
                false,
            );
        }
    }

    if item.truthy("date") {
        map(
            iso_of(item.get("date"), dates).map(Value::String),
            if t == "patent" { "appldate" } else { "date" },
            false,
        );
    }
    if item.truthy("pages") {
        map(get("pages"), "pages", false);
        let pages = item.get("pages").map(js::to_js_string).unwrap_or_default();
        let parts: Vec<&str> = pages.split(['-', '–']).collect();
        if parts.len() > 1 {
            map(s(parts[0]), "spage", false);
            map(s(parts[1]), "epage", false);
        }
    }
    map(get("numPages"), "tpages", false);
    map(get("ISBN"), "isbn", false);
    map(get("ISSN"), "issn", false);
    map(get("language"), "language", false);
    entries.join("&")
}

/// `Zotero.Date.strToISO(value)` (date.js:600-614) for a field value.
fn iso_of(v: Option<&Value>, dates: &DateOptions) -> Option<String> {
    let s = match v? {
        Value::String(s) => s.clone(),
        other => js::to_js_string(other),
    };
    str_to_iso(&s, dates)
}

/// `Zotero.OpenURL.parseContextObject(co, item)` (openurl.js:202-449,
/// utilities 4051881d59c6; `ZU.parseContextObject` in a translator), added
/// for XML ContextObject (#749): fill `item` from a key-encoded-value
/// ContextObject. `Ok(false)` where upstream returns false (no recognised
/// `rft_val_fmt`); `Err` where upstream throws (`decodeURIComponent`'s
/// URIError, a key without `=`, the duplicate check reading `.length` of an
/// undefined first name).
pub fn parse_context_object(
    co: &str,
    item: &mut TranslatorItem,
) -> Result<bool, super::TranslateError> {
    use super::utilities::{clean_author, decode_uri_component, item_type_exists};
    use super::TranslateError;
    let co_parts: Vec<&str> = co.split('&').collect();
    let has = |p: &str| co_parts.contains(&p);
    // :210-239 type
    for part in &co_parts {
        if js::substr(part, 0, 12) == "rft_val_fmt=" {
            let format = decode_uri_component(&part.chars().skip(12).collect::<String>())
                .ok_or_else(|| TranslateError::Translator("URIError: URI malformed".into()))?;
            let t = match format.as_str() {
                "info:ofi/fmt:kev:mtx:journal" => "journalArticle",
                "info:ofi/fmt:kev:mtx:book" => {
                    if has("rft.genre=bookitem") {
                        "bookSection"
                    } else if has("rft.genre=conference") || has("rft.genre=proceeding") {
                        "conferencePaper"
                    } else if has("rft.genre=report") {
                        "report"
                    } else if has("rft.genre=document") {
                        "document"
                    } else {
                        "book"
                    }
                }
                "info:ofi/fmt:kev:mtx:dissertation" => "thesis",
                "info:ofi/fmt:kev:mtx:patent" => "patent",
                "info:ofi/fmt:kev:mtx:dc" => "webpage",
                _ => continue,
            };
            item.item_type = t.to_owned();
            break;
        }
    }
    if item.item_type.is_empty() {
        return Ok(false);
    }

    let mut pages_key = "";
    // complexAu entries: (creator, offset).
    let mut complex_au: Vec<(TranslatorCreator, usize)> = Vec::new();
    static PLUS: OnceLock<Regex> = OnceLock::new();
    let plus = PLUS.get_or_init(|| Regex::new(r"\+|%2[bB]").expect("regex"));
    let set = |item: &mut TranslatorItem, k: &str, v: &str| item.set(k, v.to_owned());
    for part in &co_parts {
        let mut kv = part.split('=');
        let key = kv.next().unwrap_or("");
        let raw = kv.next().ok_or_else(|| {
            TranslateError::Translator(
                "TypeError: Cannot read properties of undefined (reading 'replace')".into(),
            )
        })?;
        let value = decode_uri_component(&plus.replace_all(raw, " "))
            .ok_or_else(|| TranslateError::Translator("URIError: URI malformed".into()))?;
        if value.is_empty() {
            continue;
        }
        let v = value.as_str();
        let ty = item.item_type.clone();
        let in_parts = matches!(ty.as_str(), "journalArticle" | "bookSection" | "conferencePaper");
        match key {
            "rft_id" => {
                let first8 = js::substr(v, 0, 8).to_lowercase();
                if first8 == "info:doi" {
                    set(item, "DOI", &v.chars().skip(9).collect::<String>());
                } else if first8 == "urn:isbn" {
                    set(item, "ISBN", &v.chars().skip(9).collect::<String>());
                } else if v.starts_with("http://") || v.starts_with("https://") {
                    set(item, "url", v);
                    set(item, "accessDate", "");
                }
            }
            "rft.btitle" => {
                if ty == "book" || ty == "report" {
                    set(item, "title", v);
                } else if ty == "bookSection" || ty == "conferencePaper" {
                    set(item, "publicationTitle", v);
                }
            }
            "rft.atitle" if in_parts => set(item, "title", v),
            "rft.jtitle" if ty == "journalArticle" => set(item, "publicationTitle", v),
            "rft.stitle" if ty == "journalArticle" => set(item, "journalAbbreviation", v),
            "rft.title" => {
                if in_parts {
                    set(item, "publicationTitle", v);
                } else {
                    set(item, "title", v);
                }
            }
            "rft.date" => {
                if ty == "patent" {
                    set(item, "issueDate", v);
                } else {
                    set(item, "date", v);
                }
            }
            "rft.volume" => set(item, "volume", v),
            "rft.issue" => set(item, "issue", v),
            "rft.pages" => {
                pages_key = "rft.pages";
                set(item, "pages", v);
            }
            "rft.spage" | "rft.epage" => {
                if pages_key != "rft.pages" {
                    let other = if key == "rft.spage" { "rft.epage" } else { "rft.spage" };
                    if pages_key == other {
                        let cur = item.get_string("pages").unwrap_or_else(|| "undefined".into());
                        if v != cur {
                            let p = if key == "rft.spage" {
                                format!("{v}-{cur}")
                            } else {
                                format!("{cur}-{v}")
                            };
                            set(item, "pages", &p);
                        }
                    } else {
                        set(item, "pages", v);
                    }
                    pages_key = if key == "rft.spage" { "rft.spage" } else { "rft.epage" };
                }
            }
            "rft.issn" => set(item, "ISSN", v),
            "rft.eissn" if !item.truthy("ISSN") => set(item, "ISSN", v),
            "rft.aulast" | "rft.invlast" => {
                // `lastCreator.institutional` is never set: always false.
                match complex_au.last_mut() {
                    Some((c, _)) if c.last_name.as_deref().is_none_or(str::is_empty) => {
                        c.last_name = Some(v.to_owned())
                    }
                    _ => complex_au.push((
                        TranslatorCreator {
                            last_name: Some(v.to_owned()),
                            creator_type: Some(
                                if key == "rft.aulast" { "author" } else { "inventor" }.into(),
                            ),
                            ..Default::default()
                        },
                        item.creators.len(),
                    )),
                }
            }
            "rft.aufirst" | "rft.invfirst" => match complex_au.last_mut() {
                Some((c, _)) if c.first_name.as_deref().is_none_or(str::is_empty) => {
                    c.first_name = Some(v.to_owned())
                }
                _ => complex_au.push((
                    TranslatorCreator {
                        first_name: Some(v.to_owned()),
                        creator_type: Some(
                            if key == "rft.aufirst" { "author" } else { "inventor" }.into(),
                        ),
                        ..Default::default()
                    },
                    item.creators.len(),
                )),
            },
            "rft.au" | "rft.creator" | "rft.contributor" | "rft.inventor" => {
                let t = match key {
                    "rft.contributor" => "contributor",
                    "rft.inventor" => "inventor",
                    _ => "author",
                };
                let a = clean_author(v, t, v.contains(','));
                item.creators.push(TranslatorCreator {
                    first_name: a.first_name,
                    last_name: Some(a.last_name),
                    creator_type: Some(a.creator_type),
                    ..Default::default()
                });
            }
            "rft.aucorp" => {
                let mut c = TranslatorCreator {
                    last_name: Some(v.to_owned()),
                    ..Default::default()
                };
                c.other.set("isInstitution", true);
                // No offset property: `offset` is undefined, and
                // `undefined + inserted` is NaN, which splice reads as 0.
                complex_au.push((c, usize::MAX));
            }
            "rft.isbn" if !item.truthy("ISBN") => set(item, "ISBN", v),
            "rft.pub" | "rft.publisher" => set(item, "publisher", v),
            "rft.place" => set(item, "place", v),
            "rft.tpages" => set(item, "numPages", v),
            "rft.edition" => set(item, "edition", v),
            "rft.series" => {
                if ty == "report" {
                    set(item, "seriesTitle", v);
                } else {
                    set(item, "series", v);
                }
            }
            _ if ty == "thesis" => {
                if key == "rft.inst" {
                    set(item, "publisher", v);
                } else if key == "rft.degree" {
                    set(item, "type", v);
                }
            }
            _ if ty == "patent" => match key {
                "rft.assignee" => set(item, "assignee", v),
                "rft.number" => set(item, "patentNumber", v),
                "rft.appldate" => set(item, "date", v),
                _ => {}
            },
            "rft.identifier" => {
                if js::len(v) > 8 {
                    if js::substr(v, 0, 5) == "ISBN " {
                        set(item, "ISBN", &v.chars().skip(5).collect::<String>());
                    } else if js::substr(v, 0, 5) == "ISSN " {
                        set(item, "ISSN", &v.chars().skip(5).collect::<String>());
                    } else if js::substr(v, 0, 8) == "urn:doi:" {
                        set(item, "DOI", &v.chars().skip(4).collect::<String>());
                    } else if v.starts_with("http://") || v.starts_with("https://") {
                        set(item, "url", v);
                    }
                }
            }
            "rft.description" => set(item, "abstractNote", v),
            "rft.rights" => set(item, "rights", v),
            "rft.language" => set(item, "language", v),
            "rft.subject" => item.tags.push(super::TranslatorTag::new(v)),
            "rft.type" => {
                if item_type_exists(v) {
                    item.item_type = v.to_owned();
                }
            }
            "rft.source" => set(item, "publicationTitle", v),
            _ => {}
        }
    }

    // :420-446: merge complex authors, keeping plain ones that match.
    let mut inserted = 0usize;
    for (c, offset) in complex_au {
        let mut push = true;
        for p in &item.creators {
            if p.last_name == c.last_name && p.first_name.as_deref().is_some_and(|f| !f.is_empty()) {
                let pf = p.first_name.as_deref().unwrap_or("");
                let Some(cf) = c.first_name.as_deref() else {
                    return Err(TranslateError::Translator(
                        "TypeError: Cannot read properties of undefined (reading 'length')".into(),
                    ));
                };
                if js::len(pf) >= js::len(cf) && js::substr(pf, 0, js::len(cf)) == cf {
                    push = false;
                    break;
                }
            }
        }
        if push {
            let at = if offset == usize::MAX { 0 } else { (offset + inserted).min(item.creators.len()) };
            item.creators.insert(at, c);
            inserted += 1;
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_context_object_pages_creators_and_plus() {
        let mut item = TranslatorItem::new("");
        let co = "ctx_ver=Z39.88-2004&rft_val_fmt=info%3Aofi%2Ffmt%3Akev%3Amtx%3Ajournal&rft.atitle=A%2BB&rft.epage=9&rft.spage=1&rft.au=J%20D%20Watson&rft.aulast=Crick&rft.aufirst=F";
        assert!(parse_context_object(co, &mut item).unwrap());
        assert_eq!(item.item_type, "journalArticle");
        // `%2B` reads as a space (openurl.js:252).
        assert_eq!(item.get_str("title"), Some("A B"));
        assert_eq!(item.get_str("pages"), Some("1-9"));
        assert_eq!(item.creators.len(), 2);
        assert_eq!(item.creators[1].last_name.as_deref(), Some("Crick"));
        let mut other = TranslatorItem::new("");
        assert!(!parse_context_object("rft_val_fmt=x", &mut other).unwrap());
    }

    #[test]
    fn encode_uri_component_like_javascript() {
        assert_eq!(
            encode_uri_component("a b/é:~*'()!"),
            "a%20b%2F%C3%A9%3A~*'()!"
        );
    }
}
