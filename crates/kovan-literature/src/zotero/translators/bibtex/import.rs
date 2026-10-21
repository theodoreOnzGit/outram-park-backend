// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): BibTeX.js — `dateFieldsToDate` :155-175,
//   `processField` :322-509, `splitUnprotected` :517-570,
//   `parseFilePathRecord` :572-609, `getFieldValue` :611-663,
//   `unescapeBibTeX` :665-718, `jabrefSplit` :720-737, `jabrefCollect`
//   :739-750, `processComment` :752-862, `beginRecord` :864-1006,
//   `doImport`/`readString` :1008-1070, `mapTeXmarkup` and helpers
//   :1115-1135, `decodeFilePathComponent` :1206-1209.
// Copyright (C) 2019 CHNM, Simon Kornblith, Richard Karnesky and Emiliano
//   heyns.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! BibTeX import (`doImport`).
//!
//! The parser reads one character at a time (`Zotero.read(1)`), as upstream
//! does. Where upstream's control flow depends on JavaScript coercions the
//! port reproduces them: at the end of input `Zotero.read(1)` is `false`,
//! which upstream's tests read as the string "false" (so `keyRe.test(false)`
//! is true); a `,` inside an `@string` record dereferences an undefined item
//! and throws; a JabRef "intersection" group calls the undefined
//! `jabrefMap` and throws. Each such throw fails the import, as upstream's
//! `reject(e)` does.

use super::text::{parse_file_path_record, split_unprotected, unescape_bibtex};
use super::{EXTRA_IDENTIFIERS, FIELD_MAP, MONTHS};
use crate::zotero::framework::js;
use crate::zotero::framework::utilities::{
    clean_author, clean_doi, field_is_valid_for_type, text2html, trim_internal,
};
use crate::zotero::framework::{
    CollectionChild, ImportContext, JsObject, TranslateError, TranslatorCollection,
    TranslatorCreator, TranslatorItem, TranslatorNote, TranslatorTag,
};
use crate::zotero::framework::utilities::str_to_iso;
use kovan_common::zotero::date::format_date;
use regex::Regex;
use std::sync::OnceLock;

fn re(cell: &'static OnceLock<Regex>, pattern: impl FnOnce() -> String) -> &'static Regex {
    cell.get_or_init(|| Regex::new(&pattern()).expect("static regex compiles"))
}

/// `inputFieldMap` (:206-217).
const INPUT_FIELD_MAP: [(&str, &str); 9] = [
    ("booktitle", "publicationTitle"),
    ("school", "publisher"),
    ("publisher", "publisher"),
    ("issue", "issue"),
    ("journaltitle", "publicationTitle"),
    ("shortjournal", "journalAbbreviation"),
    ("eventtitle", "conferenceName"),
    ("pagetotal", "numPages"),
    ("version", "version"),
];

/// `bibtex2zoteroTypeMap` (:237-270).
pub(super) const BIBTEX_TO_ZOTERO_TYPE: [(&str, &str); 30] = [
    ("book", "book"),
    ("inbook", "bookSection"),
    ("incollection", "bookSection"),
    ("article", "journalArticle"),
    ("patent", "patent"),
    ("phdthesis", "thesis"),
    ("unpublished", "manuscript"),
    ("inproceedings", "conferencePaper"),
    ("conference", "conferencePaper"),
    ("techreport", "report"),
    ("booklet", "book"),
    ("manual", "book"),
    ("mastersthesis", "thesis"),
    ("misc", "document"),
    ("proceedings", "book"),
    ("online", "webpage"),
    ("electronic", "webpage"),
    ("thesis", "thesis"),
    ("letter", "letter"),
    ("movie", "film"),
    ("artwork", "artwork"),
    ("report", "report"),
    ("legislation", "bill"),
    ("jurisdiction", "case"),
    ("audio", "audioRecording"),
    ("video", "videoRecording"),
    ("software", "computerProgram"),
    ("inreference", "encyclopediaArticle"),
    ("collection", "book"),
    ("mvbook", "book"),
];

/// `eprintIds` (:144-153).
const EPRINT_IDS: [(&str, &str); 5] = [
    ("arxiv", "arXiv"),
    ("jstor", "JSTOR"),
    ("pubmed", "PMID"),
    ("hdl", "HDL"),
    ("googlebooks", "GoogleBooksID"),
];

fn lookup<'a>(table: &'a [(&'a str, &'a str)], key: &str) -> Option<&'a str> {
    table.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

/// One `_extraFields` record: `{field, value}` or `{raw}`.
#[derive(Debug, Clone)]
pub(super) struct ExtraField {
    pub field: Option<String>,
    pub value: Option<String>,
    pub raw: Option<String>,
}

/// `extraFieldsToString` (:193-204).
pub(super) fn extra_fields_to_string(extra: &[ExtraField]) -> String {
    let mut s = String::new();
    for e in extra {
        s.push('\n');
        match e.raw.as_deref().filter(|r| !r.is_empty()) {
            None => {
                s.push_str(e.field.as_deref().unwrap_or("undefined"));
                s.push_str(": ");
                s.push_str(e.value.as_deref().unwrap_or("undefined"));
            }
            Some(r) => s.push_str(r),
        }
    }
    if s.is_empty() {
        s
    } else {
        s[1..].to_owned()
    }
}

/// An item being built, with the translator's private `_extraFields`,
/// `_eprint` and `_eprinttype` (deleted before `complete()`).
struct Building {
    item: TranslatorItem,
    extra_fields: Vec<ExtraField>,
}

/// The translator's globals for one import.
struct State {
    /// `strings` (:298): `@string` macros, by lower-cased name.
    strings: Vec<(String, String)>,
    /// `jabref.format` (:280-283).
    jabref_format: Option<i64>,
    /// `jabref.root`, in insertion order.
    jabref_root: Vec<TranslatorCollection>,
    /// `keywordSplitOnSpace` (:304): `!!Zotero.parentTranslator`.
    keyword_split_on_space: bool,
}

/// `Zotero.read(1)`: `None` is JavaScript `false`.
fn read1(ctx: &mut ImportContext) -> Option<char> {
    ctx.read_chars(1).and_then(|s| s.chars().next())
}

/// `" \n\r\t".includes(read)`; `includes(false)` looks for "false": false.
fn is_ws4(r: Option<char>) -> bool {
    matches!(r, Some(' ' | '\n' | '\r' | '\t'))
}

/// `dateFieldsToDate(year, month, day)` (:155-175).
fn date_fields_to_date(
    year: Option<&str>,
    month: Option<&str>,
    day: Option<&str>,
    ctx: &ImportContext,
) -> Option<String> {
    let year = year.filter(|y| !y.is_empty())?;
    let mut date = year.to_owned();
    if let Some(m) = month.filter(|m| !m.is_empty()) {
        if m.contains(&date) {
            date = m.to_owned();
        } else {
            date.push('-');
            date.push_str(m);
        }
        if let Some(d) = day.filter(|d| !d.is_empty()) {
            date.push('-');
            date.push_str(d);
        }
    }
    str_to_iso(&date, &ctx.options.env.dates)
}

/// `getFieldValue(read)` (:611-663).
fn get_field_value(read: Option<char>, ctx: &mut ImportContext) -> String {
    let mut value = String::new();
    if read == Some('{') {
        let mut open = 1;
        let mut next_literal = false;
        while let Some(r) = read1(ctx) {
            if next_literal {
                value.push(r);
                next_literal = false;
                continue;
            }
            if r == '\\' {
                value.push(r);
                next_literal = true;
                continue;
            }
            if r == '{' {
                open += 1;
                value.push('{');
            } else if r == '}' {
                open -= 1;
                if open == 0 {
                    break;
                }
                value.push('}');
            } else {
                value.push(r);
            }
        }
    } else if read == Some('"') {
        let mut open = 0;
        while let Some(r) = read1(ctx) {
            let prev_bs = value.ends_with('\\');
            if r == '{' && !prev_bs {
                open += 1;
                value.push('{');
            } else if r == '}' && !prev_bs {
                open -= 1;
                value.push('}');
            } else if r == '"' && open == 0 {
                break;
            } else {
                value.push(r);
            }
        }
    }
    value
}

/// `processField(item, field, value, rawValue)` (:322-509).
fn process_field(
    b: &mut Building,
    field: &str,
    value: &str,
    raw_value: &str,
    st: &State,
    ctx: &ImportContext,
) {
    static SUBT_END: OnceLock<Regex> = OnceLock::new();
    static SUBT_START: OnceLock<Regex> = OnceLock::new();
    static AND: OnceLock<Regex> = OnceLock::new();
    static COMMA: OnceLock<Regex> = OnceLock::new();
    static SPACES: OnceLock<Regex> = OnceLock::new();
    static DELIM: OnceLock<Regex> = OnceLock::new();
    static WSPLIT: OnceLock<Regex> = OnceLock::new();
    static MENDELEY_BS: OnceLock<Regex> = OnceLock::new();
    static MENDELEY_ESC: OnceLock<Regex> = OnceLock::new();
    let ws = js::WS;
    let child = ctx.in_child_translator();
    let item = &mut b.item;
    if js::trim(value).is_empty() {
        return;
    }
    if let Some(zf) = lookup(&FIELD_MAP, field) {
        // map DOIs + Label to Extra for unsupported item types
        if field == "doi" && !field_is_valid_for_type("DOI", &item.item_type) {
            if let Some(doi) = clean_doi(value) {
                b.extra_fields.push(ExtraField {
                    field: Some("DOI".into()),
                    value: Some(doi),
                    raw: None,
                });
            }
        }
        if field == "url" {
            item.set("url", raw_value);
        } else {
            item.set(zf, value);
        }
    } else if let Some(zf) = lookup(&INPUT_FIELD_MAP, field) {
        item.set(zf, value);
    } else if field == "subtitle" {
        let title = item.get_string("title").unwrap_or_default();
        let mut title = js::trim(&title).to_owned();
        let value = js::trim(value);
        let end = re(&SUBT_END, || "[-\u{2013}\u{2014}:!?.;]$".into());
        let start = re(&SUBT_START, || "^[-\u{2013}\u{2014}:.;\u{A1}\u{BF}]".into());
        if !end.is_match(&title) && !start.is_match(value) {
            title.push_str(": ");
        } else if !title.is_empty() {
            title.push(' ');
        }
        title.push_str(value);
        item.set("title", title);
    } else if field == "journal" {
        if item.truthy("publicationTitle") {
            item.set("journalAbbreviation", value);
        } else {
            item.set("publicationTitle", value);
        }
    } else if field == "fjournal" {
        if item.truthy("publicationTitle") {
            let pt = item.get("publicationTitle").cloned().unwrap();
            item.set("journalAbbreviation", pt);
        }
        item.set("publicationTitle", value);
    } else if field == "author" || field == "editor" || field == "translator" {
        let and = re(&AND, || format!("(?i){ws}+and{ws}+"));
        let comma = re(&COMMA, || format!("{ws}*,{ws}*"));
        let spaces = re(&SPACES, || " +".into());
        for name in split_unprotected(js::trim(raw_value), and) {
            if name.is_empty() {
                continue;
            }
            let mut pieces = split_unprotected(&name, comma);
            let creator = if pieces.len() > 1 {
                let mut first = pieces.pop().unwrap();
                let last = unescape_bibtex(&pieces.remove(0), child);
                if !pieces.is_empty() {
                    first = format!("{first}, {}", pieces.join(", "));
                }
                TranslatorCreator {
                    first_name: Some(unescape_bibtex(&first, child)),
                    last_name: Some(last),
                    creator_type: Some(field.into()),
                    ..Default::default()
                }
            } else if split_unprotected(&name, spaces).len() > 1 {
                let a = clean_author(&unescape_bibtex(&name, child), field, false);
                TranslatorCreator {
                    first_name: a.first_name,
                    last_name: Some(a.last_name),
                    creator_type: Some(a.creator_type),
                    ..Default::default()
                }
            } else {
                TranslatorCreator::single(unescape_bibtex(&name, child), field)
            };
            item.creators.push(creator);
        }
    } else if field == "institution" || field == "organization" {
        item.set("backupPublisher", value);
    } else if field == "location" {
        item.set("backupLocation", value);
    } else if field == "number" {
        let target = match item.item_type.as_str() {
            "report" => "reportNumber",
            "book" | "bookSection" => "seriesNumber",
            "patent" => "patentNumber",
            _ => "issue",
        };
        item.set(target, value);
    } else if field == "day" {
        item.set("day", value);
    } else if field == "month" {
        let lower = value.to_lowercase();
        let v = match MONTHS.iter().position(|m| *m == lower) {
            Some(i) => format_date(None, None, Some(i as u32), None),
            None => value.to_owned(),
        };
        item.set("month", v);
    } else if field == "year" {
        item.set("year", value);
    } else if field == "date" {
        item.set("date", value);
    } else if field == "pages" {
        if matches!(item.item_type.as_str(), "book" | "thesis" | "manuscript") {
            item.set("numPages", value);
        } else {
            item.set("pages", value.replace("--", "-"));
        }
    } else if field == "note" {
        let t = js::trim(value);
        let is_extra_id = EXTRA_IDENTIFIERS
            .iter()
            .any(|(_, label)| t.starts_with(label));
        if is_extra_id {
            b.extra_fields.push(ExtraField {
                field: None,
                value: None,
                raw: Some(t.to_owned()),
            });
        } else {
            item.notes
                .push(TranslatorNote::new(text2html(value, false)));
        }
    } else if field == "howpublished" {
        if js::len(value) >= 7 {
            let s = js::substr(value, 0, 7);
            if s == "http://" || s == "https:/" || s == "mailto:" {
                item.set("url", value);
            } else {
                b.extra_fields.push(ExtraField {
                    field: Some("Published".into()),
                    value: Some(value.into()),
                    raw: None,
                });
            }
        }
    } else if field == "lastchecked" || field == "urldate" {
        item.set("accessDate", value);
    } else if field == "keywords" || field == "keyword" {
        let delim = re(&DELIM, || format!("{ws}*[,;]{ws}*"));
        let mut tags: Vec<String> = delim.split(value).map(str::to_owned).collect();
        if tags.len() == 1 && st.keyword_split_on_space {
            tags = re(&WSPLIT, || format!("{ws}+"))
                .split(value)
                .map(str::to_owned)
                .collect();
        }
        item.tags = tags.into_iter().map(TranslatorTag::new).collect();
    } else if matches!(field, "comment" | "annote" | "review" | "notes") {
        item.notes
            .push(TranslatorNote::new(text2html(value, false)));
    } else if field == "pdf" || field == "path" {
        let mut a = JsObject::new();
        a.set("path", value);
        a.set("mimeType", "application/pdf");
        item.attachments.push(a);
    } else if field == "sentelink" {
        let mut a = JsObject::new();
        a.set("path", value.split(',').next().unwrap_or(""));
        a.set("mimeType", "application/pdf");
        item.attachments.push(a);
    } else if field == "file" {
        let raw = re(&MENDELEY_BS, || r"\$\\backslash\$".into()).replace_all(raw_value, r"\");
        let raw = re(&MENDELEY_ESC, || {
            r"([^\\](?:\\\\)*)\\([^\n\r\x{2028}\x{2029}])\{\}".into()
        })
        .replace_all(&raw, "${1}${2}");
        let raw: Vec<char> = raw.chars().collect();
        let mut start = 0;
        let mut i = 0;
        while i < raw.len() {
            if raw[i] == '\\' {
                i += 2;
                continue;
            }
            if raw[i] == ';' {
                if let Some(a) = parse_file_path_record(&raw[start..i]) {
                    item.attachments.push(a);
                }
                start = i + 1;
            }
            i += 1;
        }
        if let Some(a) = parse_file_path_record(&raw[start.min(raw.len())..]) {
            item.attachments.push(a);
        }
    } else if field == "eprint" || field == "eprinttype" {
        item.set(
            if field == "eprint" {
                "_eprint"
            } else {
                "_eprinttype"
            },
            value,
        );
        let (Some(eprint), Some(eprinttype)) =
            (item.get_string("_eprint"), item.get_string("_eprinttype"))
        else {
            return;
        };
        if eprint.is_empty() || eprinttype.is_empty() {
            return;
        }
        let Some(label) = lookup(&EPRINT_IDS, &js::trim(&eprinttype).to_lowercase()) else {
            return;
        };
        b.extra_fields.push(ExtraField {
            field: Some(label.into()),
            value: Some(js::trim(&eprint).into()),
            raw: None,
        });
        item.remove("_eprinttype");
        item.remove("_eprint");
    } else if let Some(label) = lookup(&EXTRA_IDENTIFIERS, field) {
        b.extra_fields.push(ExtraField {
            field: Some(label.into()),
            value: Some(js::trim(value).into()),
            raw: None,
        });
    }
}

/// `jabrefSplit(str, sep)` (:720-737).
fn jabref_split(s: &str, sep: char) -> Vec<String> {
    let mut chars: std::collections::VecDeque<char> = s.chars().collect();
    let mut result: Vec<String> = Vec::new();
    while !chars.is_empty() {
        if result.is_empty() {
            result.push(String::new());
        }
        if chars[0] == sep {
            chars.pop_front();
            result.push(String::new());
        } else {
            if chars[0] == '\\' {
                chars.pop_front();
            }
            // `result[...] += str.shift()`: undefined after a trailing
            // backslash concatenates as "undefined".
            match chars.pop_front() {
                Some(c) => result.last_mut().unwrap().push(c),
                None => result.last_mut().unwrap().push_str("undefined"),
            }
        }
    }
    result
}

/// The collection at `path` under the root collection `root`, created as
/// needed (:806-847), returning (the collection, its parent's item children
/// if it has a parent).
fn locate<'a>(
    root: &'a mut TranslatorCollection,
    path: &[String],
) -> (&'a mut TranslatorCollection, Option<Vec<CollectionChild>>) {
    let mut collection = root;
    let mut parent_items: Option<Vec<CollectionChild>> = None;
    for name in path {
        let pos = collection
            .children
            .iter()
            .position(|c| matches!(c, CollectionChild::Collection(x) if x.name == *name));
        let idx = match pos {
            Some(i) => i,
            None => {
                collection
                    .children
                    .push(CollectionChild::Collection(TranslatorCollection {
                        name: name.clone(),
                        children: Vec::new(),
                    }));
                collection.children.len() - 1
            }
        };
        // parentCollection = collection (before descending); its items are
        // collected after the loop, from the last parent.
        parent_items = Some(
            collection
                .children
                .iter()
                .filter(|c| matches!(c, CollectionChild::Item { .. }))
                .cloned()
                .collect(),
        );
        collection = match &mut collection.children[idx] {
            CollectionChild::Collection(c) => c,
            CollectionChild::Item { .. } => unreachable!(),
        };
    }
    (collection, parent_items)
}

/// `processComment()` (:752-862).
fn process_comment(ctx: &mut ImportContext, st: &mut State) -> Result<(), TranslateError> {
    static META: OnceLock<Regex> = OnceLock::new();
    let mut comment = String::new();
    while let Some(r) = read1(ctx) {
        if r == '}' {
            break;
        }
        comment.push(r);
    }
    if comment == "jabref-meta: groupsversion:3;" {
        st.jabref_format = Some(3);
        return Ok(());
    }
    if !comment.starts_with("jabref-meta: groupstree:") {
        return Ok(());
    }
    if st.jabref_format != Some(3) {
        return Ok(());
    }
    let comment: String = comment["jabref-meta: groupstree:".len()..]
        .chars()
        .filter(|c| *c != '\r' && *c != '\n')
        .collect();
    let meta = re(&META, || {
        r"^([0-9]) ([^:]*):([^\n\r\x{2028}\x{2029}]*)".into()
    });
    let mut collection_path: Vec<String> = Vec::new();
    let mut records: std::collections::VecDeque<String> = jabref_split(&comment, ';').into();
    while let Some(record) = records.pop_front() {
        let mut keys: std::collections::VecDeque<String> = jabref_split(&record, ';').into();
        if keys.len() < 2 {
            continue;
        }
        let id = keys.pop_front().unwrap();
        let Some(m) = meta.captures(&id) else {
            return Ok(()); // "fatal: unexpected non-match"
        };
        let level: usize = m[1].parse().unwrap();
        let rtype = m[2].to_owned();
        let name = m[3].to_owned();
        let intersection = keys.pop_front();
        if level == 0 {
            continue;
        }
        if rtype != "ExplicitGroup" {
            return Ok(());
        }
        collection_path.truncate(level - 1);
        collection_path.push(name);

        let root_idx = match st
            .jabref_root
            .iter()
            .position(|c| c.name == collection_path[0])
        {
            Some(i) => i,
            None => {
                st.jabref_root.push(TranslatorCollection {
                    name: collection_path[0].clone(),
                    children: Vec::new(),
                });
                st.jabref_root.len() - 1
            }
        };
        let (collection, parent_items) =
            locate(&mut st.jabref_root[root_idx], &collection_path[1..]);
        if intersection.as_deref() == Some("2") {
            if let Some(p) = &parent_items {
                collection.children = p.clone();
            }
        }
        while let Some(key) = keys.pop_front() {
            if !key.is_empty() {
                collection.children.push(CollectionChild::Item { id: key });
            }
        }
        if parent_items.is_some() && intersection.as_deref() == Some("1") {
            // `jabrefMap` is not defined in BibTeX.js: ReferenceError.
            return Err(TranslateError::Translator(
                "ReferenceError: jabrefMap is not defined".into(),
            ));
        }
    }
    Ok(())
}

/// How `beginRecord` ended.
enum Begin {
    /// `return item.complete()`: a promise, so `readString` restarts.
    Completed,
    /// `return;` (undefined).
    Nothing,
}

fn lookup_string<'a>(strings: &'a [(String, String)], key: &str) -> Option<&'a str> {
    strings
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

/// `beginRecord(type, closeChar)` (:864-1006).
fn begin_record(
    rtype: &str,
    close: char,
    ctx: &mut ImportContext,
    st: &mut State,
) -> Result<Begin, TranslateError> {
    let child = ctx.in_child_translator();
    let rtype = trim_internal(&rtype.to_lowercase());
    let mut item: Option<Building> = None;
    if rtype != "string" && rtype != "preamble" {
        let Some(zt) = lookup(&BIBTEX_TO_ZOTERO_TYPE, &rtype) else {
            return Ok(Begin::Nothing);
        };
        item = Some(Building {
            item: TranslatorItem::new(zt),
            extra_fields: Vec::new(),
        });
    } else if rtype == "preamble" {
        return Ok(Begin::Nothing);
    }
    if let Some(b) = item.as_mut() {
        if rtype == "mastersthesis" {
            b.item.set("type", "Master's Thesis");
        }
        if rtype == "phdthesis" {
            b.item.set("type", "PhD Thesis");
        }
    }
    let mut field = String::new();
    let mut dont_read = false;
    let mut read: Option<char> = None;
    let key_re = |r: Option<char>| match r {
        // keyRe.test(false) tests "false".
        None => true,
        Some(c) => c.is_ascii_alphanumeric() || c == '-',
    };
    loop {
        if !dont_read {
            read = read1(ctx);
            if read.is_none() {
                break;
            }
        }
        dont_read = false;
        if read == Some('=') {
            let mut values: Vec<String> = Vec::new();
            let mut raws: Vec<String> = Vec::new();
            loop {
                read = read1(ctx);
                while is_ws4(read) {
                    read = read1(ctx);
                }
                let (value, raw_value);
                if key_re(read) {
                    let mut v = match read {
                        Some(c) => c.to_string(),
                        None => "false".to_owned(),
                    };
                    loop {
                        read = read1(ctx);
                        match read {
                            Some(c)
                                if c.is_ascii_alphanumeric()
                                    || c == '-'
                                    || c == ':'
                                    || c == '_' =>
                            {
                                v.push(c)
                            }
                            _ => break,
                        }
                    }
                    dont_read = true;
                    if let Some(s) =
                        lookup_string(&st.strings, &v.to_lowercase()).filter(|s| !s.is_empty())
                    {
                        v = s.to_owned();
                    }
                    raw_value = v.clone();
                    value = v;
                } else {
                    raw_value = get_field_value(read, ctx);
                    value = unescape_bibtex(&raw_value, child);
                }
                values.push(value);
                raws.push(raw_value);
                while is_ws4(read) {
                    read = read1(ctx);
                }
                if read != Some('#') {
                    break;
                }
            }
            let value = values.concat();
            let raw_value = raws.concat();
            let lf = field.to_lowercase();
            if let Some(b) = item.as_mut() {
                process_field(b, &lf, &value, &raw_value, st, ctx);
            } else if rtype == "string" {
                match st.strings.iter_mut().find(|(k, _)| *k == lf) {
                    Some((_, v)) => *v = value,
                    None => st.strings.push((lf, value)),
                }
            }
            field.clear();
        } else if read == Some(',') {
            let Some(b) = item.as_mut() else {
                return Err(TranslateError::Translator(
                    "TypeError: Cannot read properties of undefined (reading 'itemID')".into(),
                ));
            };
            if b.item.get("itemID").is_none() {
                b.item.set("itemID", field.clone());
            }
            field.clear();
        } else if read == Some(close) {
            let Some(mut b) = item else {
                return Ok(Begin::Nothing);
            };
            let it = &mut b.item;
            if let Some(loc) = it
                .get("backupLocation")
                .cloned()
                .filter(|v| js::truthy(Some(v)))
            {
                if it.item_type == "conferencePaper" {
                    b.extra_fields.push(ExtraField {
                        field: Some("event-place".into()),
                        value: Some(js::to_js_string(&loc)),
                        raw: None,
                    });
                } else if !it.truthy("place") {
                    it.set("place", loc);
                }
                it.remove("backupLocation");
            }
            if !it.truthy("date") {
                let d = date_fields_to_date(
                    it.get_str("year"),
                    it.get_str("month"),
                    it.get_str("day"),
                    ctx,
                );
                // `item.date = false` when there is none: falsy, dropped
                // by _itemDone.
                it.set("date", d.map_or(serde_json::Value::Bool(false), Into::into));
            }
            it.remove("year");
            it.remove("month");
            it.remove("day");
            let extra = extra_fields_to_string(&b.extra_fields);
            it.set("extra", extra);
            it.remove("_eprint");
            it.remove("_eprinttype");
            if !it.truthy("publisher") && it.truthy("backupPublisher") {
                let bp = it.get("backupPublisher").cloned().unwrap();
                it.set("publisher", bp);
                it.remove("backupPublisher");
            }
            ctx.item_done(b.item);
            return Ok(Begin::Completed);
        } else if !is_ws4(read) {
            match read {
                Some(c) => field.push(c),
                None => field.push_str("false"),
            }
        }
    }
    Ok(Begin::Nothing)
}

/// `/[a-zA-Z0-9-_]/` (:1050): in a JavaScript character class `9-_` is the
/// range U+0039..U+005F, so this is a-z plus U+0030..U+005F.
fn type_char(c: char) -> bool {
    c.is_ascii_lowercase() || ('0'..='_').contains(&c)
}

/// `doImport` (:1008-1022) and `readString` (:1024-1070).
pub fn do_import(ctx: &mut ImportContext) -> Result<(), TranslateError> {
    let mut st = State {
        strings: Vec::new(),
        jabref_format: None,
        jabref_root: Vec::new(),
        keyword_split_on_space: ctx.in_child_translator(),
    };
    // `type` is local to each readString call; a completed item restarts
    // readString (item.complete() returns a promise), resetting it.
    let mut rtype: Option<String> = None;
    while let Some(read) = read1(ctx) {
        if read == '@' {
            rtype = Some(String::new());
        } else if let Some(t) = rtype.as_mut() {
            if t == "comment" {
                process_comment(ctx, &mut st)?;
                rtype = None;
            } else if read == '{' || read == '(' {
                let close = if read == '{' { '}' } else { ')' };
                let t = t.clone();
                if let Begin::Completed = begin_record(&t, close, ctx, &mut st)? {
                    rtype = None;
                }
            } else if type_char(read) {
                t.push(read);
            }
        }
    }
    for c in std::mem::take(&mut st.jabref_root) {
        ctx.collection_done(c);
    }
    Ok(())
}
