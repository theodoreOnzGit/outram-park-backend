// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): Web of Science Tagged.js (translatorID
//   594ebe3c-90a0-4830-83bc-9502825a6810, lastUpdated 2025-08-18 17:06:42):
//   header :1-11, `ITEM_TYPES` :37-47, `FIELD_MAP` :49-85, `detectImport`
//   :89-104, `doImport` :106-118, `ItemMap` :127-399 (`scanLine` :143-206,
//   `save` :212-311, `reset` :316-320, `normalize` :326-398), `splitLine`
//   :401-427, `addCreator` :431-435, `wosToZoteroType` :437-453,
//   `stringToCreator` :455-462, `selectiveTitleCase` :467-490,
//   `indexOfAll` :494-498.
// Copyright (c) 2015-2021 Michael Berkowitz, Avram Lyon, and contributors.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The Web of Science Tagged translator: import (the "Plain text" /
//! "Other file format: tab-delimited" tagged export of Web of Science).
//!
//! Upstream's `ItemMap.records` is a JavaScript `Map` (insertion order;
//! `set` on an existing key keeps its place) from tag to an array of
//! lines; [`Records`] keeps that behaviour. Where upstream would throw a
//! `TypeError` on malformed input (an empty `DT`/`PT`/`PD`/`PI` value, a
//! dangling continuation line), the port returns
//! [`TranslateError::Translator`].
//!
//! **Maturity: AI draft (1).** Verified code-to-code against Zotero
//! (`tests/zotero_translators.rs`, `wos_tagged_import_matches_upstream`).

use crate::zotero::framework::options::{translator_type, TranslatorMetadata};
use crate::zotero::framework::title_case::capitalize_title;
use crate::zotero::framework::utilities::{clean_author, clean_doi, trim_internal};
use crate::zotero::framework::{
    js, ImportContext, TranslateError, TranslatorCreator, TranslatorItem, TranslatorTag,
};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "594ebe3c-90a0-4830-83bc-9502825a6810",
    label: "Web of Science Tagged",
    creator: "Michael Berkowitz, Avram Lyon, and contributors",
    target: "txt",
    min_version: "2.1",
    priority: 100,
    translator_type: translator_type::IMPORT,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2025-08-18 17:06:42",
};

/// JavaScript's `.`: any character but a line terminator.
const DOT: &str = r"[^\n\r\x{2028}\x{2029}]";

/// `ITEM_TYPES` (:37-47).
fn item_types(t: &str) -> Option<&'static str> {
    Some(match t {
        "J" => "journalArticle",
        "S" => "bookSection",
        "P" => "patent",
        "B" => "book",
        "PROCEEDINGS PAPER" => "conferencePaper",
        "DATA SET" => "dataset",
        _ => return None,
    })
}

/// `FIELD_MAP` (:49-85).
fn field_map(tag: &str) -> Option<&'static str> {
    Some(match tag {
        "AE" => "assignee",
        "AB" => "abstractNote",
        "AR" => "pages",
        "AW" => "url",
        "BN" => "ISBN",
        "BP" => "pages",
        "CE" => "edition",
        "CL" => "place",
        "CT" => "conferenceName",
        "DI" => "DOI",
        "FN" => "libraryCatalog",
        "IO" => "issuingAuthority",
        "IS" => "issue",
        "JI" => "journalAbbreviation",
        "LA" => "language",
        "PA" => "place",
        "PC" => "country",
        "PD" => "date",
        "PG" => "numPages",
        "PI" => "place",
        "PN" => "patentNumber",
        "PS" => "pages",
        "PU" => "publisher",
        "PV" => "place",
        "PY" => "date",
        "UR" => "url",
        "VL" => "volume",
        "VN" => "versionNumber",
        "SE" => "seriesTitle",
        "SO" => "publicationTitle",
        "TI" => "title",
        _ => return None,
    })
}

/// A line split by [`split_line`]: the tag (or two spaces for a
/// continuation) and the content (`None` is `undefined`).
type Line = (String, Option<String>);

/// `splitLine` (:401-427).
fn split_line(line: &str) -> Option<Line> {
    static M: OnceLock<Regex> = OnceLock::new();
    let line: String = js::trim_end(line)
        .chars()
        .filter(|&c| c != '\u{FEFF}')
        .collect();
    if line.is_empty() {
        return None;
    }
    let m = M.get_or_init(|| Regex::new(&format!("^( {{2}}|[A-Z][A-Z0-9])( {DOT}+)?$")).unwrap());
    match m.captures(&line) {
        None => Some(("  ".to_owned(), Some(line))),
        Some(c) => Some((
            c[1].to_owned(),
            c.get(2).map(|g| js::trim(g.as_str()).to_owned()),
        )),
    }
}

/// `detectImport` (:89-104): a `PT` or `DT` tag among the first ten
/// non-empty lines.
pub fn detect_import(ctx: &mut ImportContext) -> bool {
    let mut i = 0;
    loop {
        let Some(line) = ctx.read_line() else {
            return false;
        };
        if i >= 10 {
            return false;
        }
        if let Some((head, _)) = split_line(&line) {
            i += 1;
            if head == "PT" || head == "DT" {
                return true;
            }
        }
    }
}

/// `ItemMap.records`: a JavaScript `Map` from tag to lines.
#[derive(Debug, Default)]
struct Records(Vec<(String, Vec<String>)>);

impl Records {
    fn get(&self, k: &str) -> Option<&Vec<String>> {
        self.0.iter().find(|(x, _)| x == k).map(|(_, v)| v)
    }
    fn get_mut(&mut self, k: &str) -> Option<&mut Vec<String>> {
        self.0.iter_mut().find(|(x, _)| x == k).map(|(_, v)| v)
    }
    fn has(&self, k: &str) -> bool {
        self.get(k).is_some()
    }
    fn set(&mut self, k: &str, v: Vec<String>) {
        match self.get_mut(k) {
            Some(x) => *x = v,
            None => self.0.push((k.to_owned(), v)),
        }
    }
    fn delete(&mut self, k: &str) {
        self.0.retain(|(x, _)| x != k);
    }
}

/// `ItemMap` (:127-399).
struct ItemMap {
    records: Records,
    current_key: Option<String>,
    terminate: bool,
}

fn type_error(m: &str) -> TranslateError {
    TranslateError::Translator(format!("TypeError: {m}"))
}

impl ItemMap {
    /// `scanLine` (:143-206).
    fn scan_line(&mut self, ctx: &mut ImportContext, line: &str) -> Result<(), TranslateError> {
        let Some((head, content)) = split_line(line) else {
            return Ok(());
        };
        if head == "  " {
            let Some(key) = &self.current_key else {
                return Err(TranslateError::Translator(format!(
                    "Dangling line continuation; the rest of line is {}",
                    content.as_deref().unwrap_or("undefined")
                )));
            };
            let key = key.clone();
            let Some(arr) = self.records.get_mut(&key) else {
                return Err(TranslateError::Translator(format!(
                    "Unexpected uninitialized field at {key} found while handling line continuation"
                )));
            };
            if let Some(c) = content.filter(|c| !c.is_empty()) {
                arr.push(c);
            }
            return Ok(());
        }
        if head == "ER" {
            self.save(ctx)?;
            self.reset();
            return Ok(());
        }
        if head == "EF" {
            self.terminate = true;
            return Ok(());
        }
        let v = match content {
            Some(c) if !c.is_empty() => vec![c],
            _ => Vec::new(),
        };
        self.records.set(&head, v);
        self.current_key = Some(head);
        Ok(())
    }

    /// `reset` (:316-320).
    fn reset(&mut self) {
        self.records = Records::default();
        self.current_key = None;
        self.terminate = false;
    }

    /// `normalize` (:326-398); returns the Zotero item type (upstream stores
    /// it under `DT`, which `save` pops straight away).
    fn normalize(&mut self) -> Result<String, TranslateError> {
        let r = &mut self.records;
        let mut zotero_type = wos_to_zotero_type(r.get("DT"))?;
        if zotero_type.is_none() {
            zotero_type = wos_to_zotero_type(r.get("PT"))?;
        }
        let zotero_type = zotero_type.unwrap_or("journalArticle").to_owned();
        r.delete("PT");
        // `r.set("DT", zoteroType)` keeps DT's place; `save` deletes it.
        r.set("DT", Vec::new());

        if r.has("AF") && r.has("AU") {
            r.delete("AU");
        }

        if r.has("PS") {
            r.delete("BP");
            r.delete("EP");
        } else if let Some(begin) = r.get("BP").cloned() {
            let suffix = match r.get("EP").and_then(|e| e.first()) {
                Some(e) if !e.is_empty() => format!("-{e}"),
                _ => String::new(),
            };
            let b0 = begin.first().map_or("undefined", String::as_str);
            r.set("PS", vec![format!("{b0}{suffix}")]);
            r.delete("BP");
            r.delete("EP");
        }

        if r.has("AR") && (r.has("PS") || r.has("BP")) {
            r.delete("AR");
        }

        if let Some(py) = r.get("PY").cloned() {
            match r.get("PD").cloned() {
                None => r.set("PD", py),
                Some(pd) => {
                    let year = py.first().map_or("undefined", String::as_str);
                    let Some(pd) = pd.first() else {
                        return Err(type_error(
                            "Cannot read properties of undefined (reading 'includes')",
                        ));
                    };
                    if !pd.contains(year) {
                        r.set("PD", vec![format!("{pd} {year}")]);
                    }
                }
            }
            r.delete("PY");
        }

        if let Some(pi) = r.get("PI").cloned() {
            let Some(city) = pi.first() else {
                return Err(type_error(
                    "Cannot read properties of undefined (reading 'replace')",
                ));
            };
            r.set("PI", vec![capitalize_title(city, true)]);
            r.delete("PA");
        }

        let issn: Vec<String> = r
            .get("SN")
            .into_iter()
            .flatten()
            .chain(r.get("EI").into_iter().flatten())
            .filter(|s| !s.is_empty())
            .cloned()
            .collect();
        if !issn.is_empty() {
            r.set("SN", issn);
            r.delete("EI");
        }
        Ok(zotero_type)
    }

    /// `save` (:212-311).
    fn save(&mut self, ctx: &mut ImportContext) -> Result<(), TranslateError> {
        let ty = self.normalize()?;
        self.records.delete("DT");
        let mut item = TranslatorItem::new(ty.clone());
        let mut extra: Vec<String> = Vec::new();
        let mut tag_missed = 0;
        for (wos_tag, arr) in &self.records.0 {
            let mut s = trim_internal(&arr.join(" "));
            match wos_tag.as_str() {
                "AF" | "AU" => add_creator(
                    &mut item,
                    arr,
                    if ty == "patent" { "inventor" } else { "author" },
                ),
                "BE" | "ED" => add_creator(&mut item, arr, "editor"),
                "TR" => add_creator(&mut item, arr, "translator"),
                "AA" => add_creator(&mut item, arr, "contributor"),
                "BD" | "DE" | "ID" | "IP" | "MC" | "MQ" | "OR" => {
                    for t in s.split("; ") {
                        item.tags.push(TranslatorTag::new(t));
                    }
                }
                "NO" => extra.push(format!("Comments, Corrections, Erratum: {s}")),
                "NT" => extra.push(format!("Notes: {s}")),
                "UT" => {
                    if !js::trim(&s).is_empty() {
                        extra.push(format!("Web of Science ID: {s}"));
                    }
                }
                "SN" => {
                    let j = arr.join(", ");
                    item.set("ISSN", if j.is_empty() { Value::Null } else { j.into() });
                }
                "SE" | "SO" | "TI" => {
                    s = selective_title_case(&s, false);
                    item.set(field_map(wos_tag).unwrap(), s);
                }
                "DI" => {
                    item.set("DOI", clean_doi(&s).map_or(Value::Null, Value::from));
                }
                other => {
                    if matches!(other, "AE" | "CT" | "PU") {
                        s = selective_title_case(&s, true);
                    }
                    match field_map(other) {
                        None => tag_missed += 1,
                        Some(f) => item.set(f, s),
                    }
                }
            }
        }
        if tag_missed == self.records.0.len() {
            return Ok(());
        }
        if !extra.is_empty() {
            item.set("extra", extra.join("\n"));
        }
        ctx.item_done(item);
        Ok(())
    }
}

/// `doImport` (:106-118).
pub fn do_import(ctx: &mut ImportContext) -> Result<(), TranslateError> {
    let mut map = ItemMap {
        records: Records::default(),
        current_key: None,
        terminate: false,
    };
    while !map.terminate {
        let Some(line) = ctx.read_line() else {
            break;
        };
        map.scan_line(ctx, &line)?;
    }
    map.save(ctx)
}

/// `addCreator` (:431-435).
fn add_creator(item: &mut TranslatorItem, authors: &[String], creator_type: &str) {
    for a in authors {
        item.creators.push(string_to_creator(a, creator_type));
    }
}

/// `wosToZoteroType` (:437-453) on a tag's lines.
fn wos_to_zotero_type(
    wos_type: Option<&Vec<String>>,
) -> Result<Option<&'static str>, TranslateError> {
    let Some(v) = wos_type else {
        return Ok(None);
    };
    let Some(first) = v.first() else {
        return Err(type_error(
            "Cannot read properties of undefined (reading 'toUpperCase')",
        ));
    };
    Ok(item_types(&first.to_uppercase()))
}

/// `stringToCreator` (:455-462).
fn string_to_creator(author: &str, creator_type: &str) -> TranslatorCreator {
    static PAREN: OnceLock<Regex> = OnceLock::new();
    let paren = PAREN.get_or_init(|| Regex::new(&format!(r"\({DOT}*\)")).unwrap());
    let mut author = paren.replace_all(author, "").into_owned();
    if !author.contains(',') {
        author = author.replacen(' ', ", ", 1);
    }
    let a = clean_author(&author, creator_type, true);
    TranslatorCreator {
        first_name: a.first_name,
        last_name: Some(a.last_name),
        creator_type: Some(a.creator_type),
        ..Default::default()
    }
}

/// `selectiveTitleCase` (:467-490).
fn selective_title_case(string: &str, force: bool) -> String {
    const WORD_FORMS: [(&str, &str); 13] = [
        ("IOP", "IoP"),
        ("PEERJ", "PeerJ"),
        ("PLOS", "PLoS"),
        ("ACM", "ACM"),
        ("AIP", "AIP"),
        ("BMC", "BMC"),
        ("BMJ", "BMJ"),
        ("CRC", "CRC"),
        ("IEEE", "IEEE"),
        ("JAMA", "JAMA"),
        ("MDPI", "MDPI"),
        ("SAGE", "SAGE"),
        ("USA", "USA"),
    ];
    let clean = trim_internal(string);
    let words: Vec<&str> = clean.split(' ').collect();
    let mut locations: Vec<(usize, &str)> = Vec::new();
    for (word, form) in WORD_FORMS {
        for (i, w) in words.iter().enumerate() {
            if *w == word {
                match locations.iter_mut().find(|(j, _)| *j == i) {
                    Some(l) => l.1 = form,
                    None => locations.push((i, form)),
                }
            }
        }
    }
    let cap = capitalize_title(&clean, force);
    let mut out: Vec<String> = cap.split(' ').map(str::to_owned).collect();
    for (i, form) in locations {
        // Assigning past the end of a JavaScript array leaves holes, which
        // `join` writes as "".
        if i >= out.len() {
            out.resize(i + 1, String::new());
        }
        out[i] = form.to_owned();
    }
    out.join(" ")
}
