// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero utilities, https://github.com/zotero/utilities (commit
//   4051881d59c6, the translation-server's submodule for the references):
//   openurl.js `createContextObject` :35-195 (the string form, `asObj`
//   false); utilities_item.js `getFirstCreatorFromItemJSON` :640-656;
//   cachedTypes.js `CreatorTypes.getPrimaryIDForType` :123-127.
//   `encodeURIComponent` is ECMA-262 §19.2.6.5.
// Copyright (c) 2009 Center for History and New Media, George Mason
//   University, Fairfax, Virginia, USA; Corporation for Digital Scholarship.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! `ZU.createContextObject(item, version)`: an OpenURL ContextObject in
//! key-encoded-value form, for an export item (COinS export).
//!
//! Not ported: `parseContextObject` (the reverse direction, used only by
//! COinS' web translator, which is out of scope) and the `asObj` form,
//! which no ported translator asks for.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_uri_component_like_javascript() {
        assert_eq!(
            encode_uri_component("a b/é:~*'()!"),
            "a%20b%2F%C3%A9%3A~*'()!"
        );
    }
}
