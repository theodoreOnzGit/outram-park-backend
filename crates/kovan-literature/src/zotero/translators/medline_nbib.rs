// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): MEDLINEnbib.js (translatorID
//   9ec64cfd-bea7-472a-9557-493c0c26b0fb, lastUpdated 2025-04-29 03:02:00):
//   header :1-15, `detectImport` :42-59, `fieldMap` :61-77, `inputTypeMap`
//   :81-94, `processTag` :96-197, `doImport` :199-242, `finalizeItem`
//   :244-312.
// Copyright (c) 2014-15 Sebastian Karcher.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The MEDLINE/nbib translator: import (PubMed's and ERIC's `.nbib`
//! tagged format).
//!
//! Upstream's `doImport` and `finalizeItem` are `async` but await nothing
//! but `item.complete()`; they are ported as ordinary functions.
//!
//! **Maturity: AI draft (1).** Verified code-to-code against Zotero
//! (`tests/zotero_translators.rs`, `medline_nbib_import_matches_upstream`).

use crate::zotero::framework::identifiers::{clean_isbn, clean_issn};
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::utilities::clean_author;
use crate::zotero::framework::{
    js, ImportContext, JsObject, TranslateError, TranslatorCreator, TranslatorItem, TranslatorTag,
};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "9ec64cfd-bea7-472a-9557-493c0c26b0fb",
    label: "MEDLINE/nbib",
    creator: "Sebastian Karcher",
    target: "txt",
    min_version: "4.0",
    priority: 100,
    translator_type: translator_type::IMPORT,
    config_options: &[("async", HeaderValue::Bool(true))],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2025-04-29 03:02:00",
};

fn re(cell: &'static OnceLock<Regex>, pattern: impl FnOnce() -> String) -> &'static Regex {
    cell.get_or_init(|| Regex::new(&pattern()).expect("static regex compiles"))
}

/// `line.replace(/^\s+/, "")`.
fn trim_lead(s: &str) -> &str {
    js::trim_start(s)
}

/// `detectImport` (:42-59). Upstream's `/^PMID( {1, 2})?- /` is not a
/// quantifier (`{1, 2}` with a space is literal text in JavaScript), so on
/// the first six characters it only matches `"PMID- "`.
pub fn detect_import(ctx: &mut ImportContext) -> bool {
    let mut i = 0;
    while let Some(line) = ctx.read_line() {
        let line = trim_lead(&line);
        if !line.is_empty() {
            if js::substr(line, 0, 6).starts_with("PMID- ") || line.contains("OWN - ERIC") {
                return true;
            }
            let before = i;
            i += 1;
            if before > 3 {
                return false;
            }
        }
    }
    false
}

/// `fieldMap` (:61-77).
fn field_map(tag: &str) -> Option<&'static str> {
    Some(match tag {
        "TI" => "title",
        "VI" => "volume",
        "IP" => "issue",
        "PL" => "place",
        "PB" => "publisher",
        "BTI" => "bookTitle",
        "JT" => "publicationTitle",
        "TA" => "journalAbbreviation",
        "PG" => "pages",
        "CI" => "rights",
        "ISBN" => "ISBN",
        "ISSN" => "ISSN",
        "LA" => "language",
        "EN" => "edition",
        "AB" => "abstractNote",
        _ => return None,
    })
}

/// `inputTypeMap` (:81-94).
fn input_type(value: &str) -> Option<&'static str> {
    Some(match value {
        "Book" | "Books" => "book",
        "Book Chapter" => "bookSection",
        "Case Reports" | "Case Report" | "Journal Article" => "journalArticle",
        "Newspaper Article" => "newspaperArticle",
        "Video-Audio Media" => "videoRecording",
        "Technical Report" => "report",
        "Legal Case" => "case",
        "Preprint" => "preprint",
        "Legislation" => "statute",
        _ => return None,
    })
}

/// The item being built, with upstream's `item.creatorsBackup` array kept
/// beside it (it is deleted before `complete()`).
struct Building {
    item: TranslatorItem,
    creators_backup: Vec<TranslatorCreator>,
}

impl Building {
    fn new() -> Self {
        Building {
            item: TranslatorItem::new(""),
            creators_backup: Vec::new(),
        }
    }
}

fn creator(value: &str, creator_type: &str) -> TranslatorCreator {
    let a = clean_author(value, creator_type, value.contains(','));
    TranslatorCreator {
        first_name: a.first_name,
        last_name: Some(a.last_name),
        creator_type: Some(a.creator_type),
        ..Default::default()
    }
}

/// `item[key]` as `"" + item[key]` would give it (`"undefined"` when unset).
fn js_str(item: &TranslatorItem, key: &str) -> String {
    item.get_string(key)
        .unwrap_or_else(|| "undefined".to_owned())
}

/// `item[to] = item[from]` (an unset `from` sets `undefined`, which
/// `_itemDone` drops).
fn copy_prop(item: &mut TranslatorItem, to: &str, from: &str) {
    let v = item.get(from).cloned().unwrap_or(Value::Null);
    item.set(to, v);
}

/// `processTag` (:96-197).
fn process_tag(b: &mut Building, tag: &str, value: &str) {
    static OID: OnceLock<Regex> = OnceLock::new();
    static AU: OnceLock<Regex> = OnceLock::new();
    static DOI: OnceLock<Regex> = OnceLock::new();
    static PII: OnceLock<Regex> = OnceLock::new();
    let value = js::trim(value).to_owned();
    let item = &mut b.item;
    if let Some(f) = field_map(tag) {
        item.set(f, value);
    } else if tag == "PT" {
        if let Some(t) = input_type(&value) {
            item.item_type = t.to_owned();
        } else if value.contains("Dissertation") {
            item.item_type = "thesis".to_owned();
        } else if value.contains("Report") {
            item.set("itemTypeBackup", "report");
        }
    } else if tag == "FAU" || tag == "FED" {
        let ty = if tag == "FAU" { "author" } else { "editor" };
        item.creators.push(creator(&value, ty));
    } else if tag == "AU" || tag == "ED" {
        let ty = if tag == "AU" { "author" } else { "editor" };
        let au = re(&AU, || format!("{}([A-Z]+)$", js::WS));
        let value = au.replace(&value, ", $1").into_owned();
        b.creators_backup.push(creator(&value, ty));
    } else if tag == "OID" && re(&OID, || "E[JD][0-9]+".into()).is_match(&value) {
        item.set("extra", format!("ERIC Number: {value}"));
    } else if tag == "PMID" {
        item.set("extra", format!("PMID: {value}"));
    } else if tag == "PMC" {
        let extra = js_str(item, "extra");
        item.set("extra", format!("{extra} \nPMCID: {value}"));
    } else if tag == "IS" {
        if let Some(issn) = clean_issn(&value) {
            if !item.truthy("ISSN") {
                item.set("ISSN", issn);
            } else {
                let cur = js_str(item, "ISSN");
                item.set("ISSN", format!("{cur} {issn}"));
            }
        } else if let Some(isbn) = clean_isbn(&value, false) {
            if !item.truthy("ISBN") {
                item.set("ISBN", isbn);
            } else {
                let cur = js_str(item, "ISBN");
                item.set("ISBN", format!("{cur} {isbn}"));
            }
        }
    } else if tag == "AID" {
        if value.contains("[doi]") {
            let doi = re(&DOI, || format!("{}*\\[doi\\]", js::WS));
            item.set("DOI", doi.replace(&value, "").into_owned());
        }
    } else if tag == "DP" {
        item.set("date", value);
    } else if tag == "SO" {
        item.set("citation", value);
    } else if tag == "LID" {
        if value.starts_with("http") {
            let mut a = JsObject::new();
            a.set("url", value);
            a.set("title", "Catalog Link");
            a.set("snapshot", false);
            item.attachments.push(a);
        } else if value.contains("[pii]") {
            let pii = re(&PII, || format!("{}*\\[pii\\]", js::WS));
            item.set("pagesBackup", pii.replace(&value, "").into_owned());
        }
    } else if tag == "MH" || tag == "OT" || tag == "KW" {
        item.tags.push(TranslatorTag::new(value));
    }
}

/// `/^[A-Z0-9]+\s*-/`.
fn is_tag_line(line: &str) -> bool {
    static TAG: OnceLock<Regex> = OnceLock::new();
    re(&TAG, || format!("^[A-Z0-9]+{}*-", js::WS)).is_match(line)
}

/// `line.match(/^[A-Z0-9]+/)[0]`.
fn tag_of(line: &str) -> String {
    line.chars()
        .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        .collect()
}

/// `line.substr(line.indexOf("-") + 1)`.
fn after_dash(line: &str) -> String {
    match line.find('-') {
        Some(i) => line[i + 1..].to_owned(),
        None => line.to_owned(),
    }
}

/// `doImport` (:199-242).
pub fn do_import(ctx: &mut ImportContext) -> Result<(), TranslateError> {
    // First valid line is the type.
    let first = loop {
        match ctx.read_line() {
            None => {
                return Err(TranslateError::Translator(
                    "TypeError: line.match is not a function".into(),
                ))
            }
            Some(l) if is_tag_line(&l) => break l,
            Some(_) => {}
        }
    };
    let mut b = Building::new();
    let mut tag: Option<String> = Some(tag_of(&first));
    let mut data = after_dash(&first);
    while let Some(line) = ctx.read_line() {
        if line.is_empty() {
            if let Some(t) = tag.take() {
                process_tag(&mut b, &t, &data);
                data = String::new();
                finalize_item(ctx, std::mem::replace(&mut b, Building::new()));
            }
        } else if is_tag_line(&line) {
            if let Some(t) = &tag {
                process_tag(&mut b, t, &data);
            }
            tag = Some(tag_of(&line));
            data = js::trim(&after_dash(&line)).to_owned();
        } else if tag.is_some() {
            data.push(' ');
            data.push_str(trim_lead(&line));
        }
    }
    if let Some(t) = tag {
        process_tag(&mut b, &t, &data);
        finalize_item(ctx, b);
    }
    Ok(())
}

/// The page-range expansion loop of `finalizeItem` (:253-272): returns
/// `fullPageRange` (the last one built), if any. Positions are in chars.
fn expand_page_ranges(pages: &str) -> Option<String> {
    let c: Vec<char> = pages.chars().collect();
    let digits_from = |p: usize| {
        let mut e = p;
        while e < c.len() && c[e].is_ascii_digit() {
            e += 1;
        }
        e
    };
    // `/(\d+)-(\d+)/g.exec` from `last`: (index, start digits end, end).
    let exec = |last: usize| -> Option<(usize, usize, usize)> {
        let mut p = last;
        while p < c.len() {
            let e1 = digits_from(p);
            if e1 > p && e1 < c.len() && c[e1] == '-' {
                let e2 = digits_from(e1 + 1);
                if e2 > e1 + 1 {
                    return Some((p, e1, e2));
                }
            }
            p += 1;
        }
        None
    };
    let mut last_index: usize = 0;
    let mut full = None;
    while let Some((index, dash, end)) = exec(last_index) {
        last_index = end;
        let start: String = c[index..dash].iter().collect();
        let mut end_s: String = c[dash + 1..end].iter().collect();
        let diff = start.len() as isize - end_s.len() as isize;
        if diff > 0 {
            end_s = format!("{}{}", &start[..diff as usize], end_s);
            let new_range = format!("{start}-{end_s}");
            let before: String = c[..index].iter().collect();
            let after: String = c[end..].iter().collect();
            full = Some(format!("{before}{new_range}{after}"));
            last_index += new_range.len() - (end - index);
        }
    }
    full
}

/// `finalizeItem` (:244-312).
fn finalize_item(ctx: &mut ImportContext, b: Building) {
    static UNI_TEST: OnceLock<Regex> = OnceLock::new();
    static UNI: OnceLock<Regex> = OnceLock::new();
    static EISSN: OnceLock<Regex> = OnceLock::new();
    let Building {
        mut item,
        creators_backup,
    } = b;
    if item.creators.is_empty() && !creators_backup.is_empty() {
        item.creators = creators_backup;
    }
    if item.truthy("pages") {
        let pages = js_str(&item, "pages");
        if let Some(full) = expand_page_ranges(&pages) {
            item.set("pages", full);
        }
    } else if item.truthy("pagesBackup") {
        copy_prop(&mut item, "pages", "pagesBackup");
    }
    item.remove("pagesBackup");

    // Duplicate ISSNs.
    let issn = item.get_string("ISSN");
    if item.truthy("ISSN") && issn.as_deref().is_some_and(|s| s.contains(' ')) {
        let mut seen: Vec<String> = Vec::new();
        for part in issn.unwrap().split(js::is_space) {
            if !seen.iter().any(|s| s == part) {
                seen.push(part.to_owned());
            }
        }
        item.set("ISSN", seen.join(" "));
    } else if item.truthy("ISSN") {
        let s = js_str(&item, "ISSN");
        let s = re(&EISSN, || "E?ISSN-".into()).replace(&s, "").into_owned();
        item.set(
            "ISSN",
            clean_issn(&s).map_or(Value::Bool(false), Value::from),
        );
    }
    if item.truthy("ISBN") {
        let s = js_str(&item, "ISBN").replacen("ISBN-", "", 1);
        item.set(
            "ISBN",
            clean_isbn(&s, false).map_or(Value::Bool(false), Value::from),
        );
    }

    if item.item_type == "book" {
        copy_prop(&mut item, "publisher", "publicationTitle");
        item.remove("publicationTitle");
    } else if item.item_type == "thesis" {
        if item.truthy("citation") {
            let citation = js_str(&item, "citation");
            let test = re(&UNI_TEST, || ",[^,]+ University".into());
            if test.is_match(&citation) {
                let uni = re(&UNI, || format!(",{}*([^,]+ University)", js::WS));
                let u = uni.captures(&citation).unwrap()[1].to_owned();
                item.set("university", u);
            }
        }
        copy_prop(&mut item, "archive", "publicationTitle");
        item.remove("publicationTitle");
    } else if item.item_type.is_empty() {
        let eric_ej = item.truthy("extra") && js_str(&item, "extra").contains("ERIC Number: EJ");
        if item.truthy("itemTypeBackup") && !eric_ej {
            item.item_type = js_str(&item, "itemTypeBackup");
            copy_prop(&mut item, "institution", "publicationTitle");
            item.remove("publicationTitle");
        } else {
            item.item_type = "journalArticle".to_owned();
        }
    }

    item.remove("citation");
    item.remove("itemTypeBackup");
    if item.item_type == "book" {
        copy_prop(&mut item, "title", "bookTitle");
    }
    ctx.item_done(item);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_ranges_expand() {
        assert_eq!(expand_page_ranges("1234-56").as_deref(), Some("1234-1256"));
        assert_eq!(expand_page_ranges("12-34"), None);
    }
}
