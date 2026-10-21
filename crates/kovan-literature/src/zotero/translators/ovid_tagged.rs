// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): OVID Tagged.js (translatorID
//   59e7e93e-4ef0-4777-8388-d6eddb3261bf, lastUpdated 2025-03-03 21:50:09):
//   header :1-11, `detectImport` :40-57, `fieldMap` :59-79, `inputTypeMap`
//   :83-97, `processTag` :99-169, `doImport` :171-230, `finalizeItem`
//   :232-344.
// Copyright (c) 2014 Sebastian Karcher.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The OVID Tagged translator: import.
//!
//! JavaScript regular expressions are rewritten for the `regex` crate with
//! their JavaScript meaning: `\s` is [`js::WS`], `\d` `[0-9]`, `\w`
//! `[A-Za-z0-9_]`, `\b` `(?-u:\b)` and `.` anything but a line terminator
//! ([`DOT`]). Both engines pick the leftmost match and, at that position,
//! the same (leftmost-first) submatch.
//!
//! **Maturity: AI draft (1).** Verified code-to-code against Zotero
//! (`tests/zotero_translators.rs`, `ovid_tagged_import_matches_upstream`).

use crate::zotero::framework::identifiers::{clean_isbn, clean_issn};
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
    id: "59e7e93e-4ef0-4777-8388-d6eddb3261bf",
    label: "OVID Tagged",
    creator: "Sebastian Karcher",
    target: "txt",
    min_version: "4.0",
    priority: 100,
    translator_type: translator_type::IMPORT,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2025-03-03 21:50:09",
};

/// JavaScript's `.`: any character but a line terminator.
pub const DOT: &str = r"[^\n\r\x{2028}\x{2029}]";

/// A regex with `\s` (as `{WS}`) and `.` (as `{DOT}`) placeholders filled in.
fn re(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| {
        let p = pattern.replace("{WS}", js::WS).replace("{DOT}", DOT);
        Regex::new(&p).expect("static regex compiles")
    })
}

/// `detectImport` (:40-57).
pub fn detect_import(ctx: &mut ImportContext) -> bool {
    static VN: OnceLock<Regex> = OnceLock::new();
    let vn = re(&VN, "^VN{WS}{1,2}- Ovid Technologies");
    let mut i = 0;
    while let Some(line) = ctx.read_line() {
        let line = js::trim_start(&line);
        if !line.is_empty() {
            if vn.is_match(line) {
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

/// `fieldMap` (:59-79).
fn field_map(tag: &str) -> Option<&'static str> {
    Some(match tag {
        "TI" => "title",
        "VI" => "volume",
        "IP" => "issue",
        "PL" => "place",
        "PB" => "publisher",
        "BT" => "bookTitle",
        "JT" => "publicationTitle",
        "TA" => "journalAbbreviation",
        "PG" => "pages",
        "PN" => "patentNumber",
        "RO" => "rights",
        "DG" => "issueDate",
        "IB" => "ISBN",
        "IS" => "ISSN",
        "LG" => "language",
        "EN" => "edition",
        "DB" => "libraryCatalog",
        "AB" => "abstractNote",
        "AN" => "callNumber",
        _ => return None,
    })
}

/// `inputTypeMap` (:83-97).
fn input_type(value: &str) -> Option<&'static str> {
    Some(match value {
        "Book" => "book",
        "Book Chapter" | "Book chapter" | "Chapter" => "bookSection",
        "Dissertation" | "Dissertation Abstract" => "thesis",
        "Journal Article" => "journalArticle",
        "Newspaper Article" => "newspaperArticle",
        "Video-Audio Media" => "videoRecording",
        "Technical Report" => "report",
        "Legal Case" => "case",
        "Legislation" => "statute",
        "Patent" => "patent",
        _ => return None,
    })
}

/// The item being built and upstream's `item.creatorsBackup`.
struct Building {
    item: TranslatorItem,
    creators_backup: Vec<TranslatorCreator>,
}

impl Building {
    fn new(item_id: Option<String>) -> Self {
        let mut item = TranslatorItem::new("");
        if let Some(id) = item_id {
            item.set("itemID", id);
        }
        Building {
            item,
            creators_backup: Vec::new(),
        }
    }
}

fn creator(value: &str, creator_type: &str, use_comma: bool) -> TranslatorCreator {
    let a = clean_author(value, creator_type, use_comma);
    TranslatorCreator {
        first_name: a.first_name,
        last_name: Some(a.last_name),
        creator_type: Some(a.creator_type),
        ..Default::default()
    }
}

/// `"" + item[key]`.
fn js_str(item: &TranslatorItem, key: &str) -> String {
    item.get_string(key)
        .unwrap_or_else(|| "undefined".to_owned())
}

/// `processTag` (:99-169).
fn process_tag(b: &mut Building, tag: &str, value: &str) {
    static R1: OnceLock<Regex> = OnceLock::new();
    static R2: OnceLock<Regex> = OnceLock::new();
    static R3: OnceLock<Regex> = OnceLock::new();
    static R4: OnceLock<Regex> = OnceLock::new();
    static KW: OnceLock<Regex> = OnceLock::new();
    let value = js::trim(value).to_owned();
    let item = &mut b.item;
    if tag == "DB" && value == "Books@Ovid" {
        item.item_type = "book".to_owned();
    }
    if let Some(f) = field_map(tag) {
        item.set(f, value);
    } else if tag == "PT" || tag == "DT" {
        if let Some(t) = input_type(&value) {
            item.item_type = t.to_owned();
        }
    } else if tag == "FA" || tag == "FED" {
        let ty = if tag == "FA" { "author" } else { "editor" };
        item.creators.push(creator(&value, ty, value.contains(',')));
    } else if tag == "AU" || tag == "ED" {
        let ty = if tag == "AU" { "author" } else { "editor" };
        let r1 = re(&R1, "[0-9,+*{WS}]+$");
        let r2 = re(&R2, r" Ph\.?D\.?{DOT}*");
        let r3 = re(&R3, r"\[{DOT}+");
        let r4 = re(&R4, r"((?-u:\b)(?i-u:MD|[BM]Sc|[BM]A|MPH|MB)(,{WS}*)?)+$");
        for name in value.split(';') {
            let name = r1.replace(name, "");
            let name = r2.replace(&name, "");
            let name = r3.replace(&name, "");
            let name = r4.replace_all(&name, "");
            b.creators_backup
                .push(creator(&name, ty, value.contains(',')));
        }
    } else if tag == "UI" {
        item.set("PMID", format!("PMID: {value}"));
    } else if tag == "DI" || tag == "DO" {
        if value.contains("10.") {
            item.set("DOI", value);
        }
    } else if tag == "YR" {
        item.set("date", value);
    } else if tag == "IN" {
        item.set("institution", value);
    } else if tag == "SO" {
        item.set("citation", value);
    } else if tag == "PU" {
        item.set("publishing", value);
    } else if tag == "KW" {
        for t in re(&KW, ";{WS}*").split(&value) {
            item.tags.push(TranslatorTag::new(t));
        }
    }
}

/// `/^[A-Z0-9]+\s*-/`.
fn is_tag_line(line: &str) -> bool {
    static TAG: OnceLock<Regex> = OnceLock::new();
    re(&TAG, "^[A-Z0-9]+{WS}*-").is_match(line)
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

/// `line.match(/^<\s*(\d+)\.\s*>\s*$/)`: the item number.
fn check_id(line: &str) -> Option<String> {
    static ID: OnceLock<Regex> = OnceLock::new();
    re(&ID, r"^<{WS}*([0-9]+)\.{WS}*>{WS}*$")
        .captures(line)
        .map(|c| c[1].to_owned())
}

/// `doImport` (:171-230).
pub fn do_import(ctx: &mut ImportContext) -> Result<(), TranslateError> {
    let mut potential_item_id: Option<String> = None;
    let first = loop {
        let Some(line) = ctx.read_line() else {
            // `false.replace(...)` throws.
            return Err(TranslateError::Translator(
                "TypeError: line.replace is not a function".into(),
            ));
        };
        let line = js::trim_start(&line).to_owned();
        if let Some(id) = check_id(&line) {
            potential_item_id = Some(id);
        }
        if is_tag_line(&line) {
            break line;
        }
    };
    let mut b = Building::new(potential_item_id.take());
    let mut tag = tag_of(&first);
    let mut data = after_dash(&first);
    while let Some(line) = ctx.read_line() {
        let line = js::trim_start(&line).to_owned();
        if let Some(id) = check_id(&line) {
            if potential_item_id.is_none() {
                potential_item_id = Some(id);
            }
        }
        if is_tag_line(&line) {
            process_tag(&mut b, &tag, &data);
            tag = tag_of(&line);
            if tag == "VN" {
                let done = std::mem::replace(&mut b, Building::new(potential_item_id.take()));
                finalize_item(ctx, done)?;
            }
            data = after_dash(&line);
        } else {
            data.push(' ');
            data.push_str(&line);
        }
    }
    process_tag(&mut b, &tag, &data);
    finalize_item(ctx, b)
}

/// `item[key].match(re)[n]` as an owned string.
fn cap(re: &Regex, s: &str, n: usize) -> Option<String> {
    re.captures(s)
        .and_then(|c| c.get(n).map(|m| m.as_str().to_owned()))
}

/// `finalizeItem` (:232-344).
fn finalize_item(ctx: &mut ImportContext, b: Building) -> Result<(), TranslateError> {
    static T1: OnceLock<Regex> = OnceLock::new();
    static T2: OnceLock<Regex> = OnceLock::new();
    static MONTH: OnceLock<Regex> = OnceLock::new();
    static YEAR: OnceLock<Regex> = OnceLock::new();
    static VOLISS: OnceLock<Regex> = OnceLock::new();
    static VOL: OnceLock<Regex> = OnceLock::new();
    static YVOL: OnceLock<Regex> = OnceLock::new();
    static ISS: OnceLock<Regex> = OnceLock::new();
    static PG_T: OnceLock<Regex> = OnceLock::new();
    static PG: OnceLock<Regex> = OnceLock::new();
    static PP: OnceLock<Regex> = OnceLock::new();
    static JOURNAL: OnceLock<Regex> = OnceLock::new();
    static PT_SPLIT: OnceLock<Regex> = OnceLock::new();
    static ED_T: OnceLock<Regex> = OnceLock::new();
    static ED_G: OnceLock<Regex> = OnceLock::new();
    static ED_1: OnceLock<Regex> = OnceLock::new();
    static ED_2: OnceLock<Regex> = OnceLock::new();
    static BT_T: OnceLock<Regex> = OnceLock::new();
    static BT: OnceLock<Regex> = OnceLock::new();
    static RANGE: OnceLock<Regex> = OnceLock::new();
    static PUB_T: OnceLock<Regex> = OnceLock::new();
    static PLACE: OnceLock<Regex> = OnceLock::new();
    static PUB_R: OnceLock<Regex> = OnceLock::new();
    static INST: OnceLock<Regex> = OnceLock::new();
    static CALL: OnceLock<Regex> = OnceLock::new();

    let Building {
        mut item,
        creators_backup,
    } = b;
    if item.creators.is_empty() && !creators_backup.is_empty() {
        item.creators = creators_backup;
    }
    if item.item_type.is_empty() {
        item.item_type = "journalArticle".to_owned();
    }
    // `item.title.replace(...)`: throws when there is no title.
    let Some(title) = item.get_string("title") else {
        return Err(TranslateError::Translator(
            "TypeError: Cannot read properties of undefined (reading 'replace')".into(),
        ));
    };
    let t = re(
        &T1,
        r"(\.{WS}*)?(\[(Article|Report|Miscellaneous|References)\])?([.{WS}]*)?$",
    )
    .replace(&title, "");
    let t = re(&T2, r#"^{WS}*"({DOT}+)"{WS}*$"#).replace(&t, "$1");
    item.set("title", t.into_owned());

    let month = re(
        &MONTH,
        r"(?:[-/]?(?:Jan(?:uary)?|Feb(?:ruary)?|Mar(?:ch)?|Apr(?:il)?|May|Jun(?:e)?|Jul(?:y)?|Aug(?:ust)?|Sep(?:tember)?|Oct(?:ober)?|Nov(?:ember)?|Dec(?:ember)?))+(?-u:\b)",
    );
    let pg_t = re(&PG_T, r":{WS}*[0-9]+-[0-9]+");
    let pg = re(&PG, r":{WS}*([0-9]+-[0-9]+)");
    let pp = re(&PP, r"pp\.{WS}*([0-9]+-[0-9]+)");

    let mut value = item.get("citation").cloned();
    if !js::truthy(value.as_ref()) && item.item_type == "bookSection" {
        value = item.get("bookTitle").cloned();
    }
    let value_str = value
        .as_ref()
        .filter(|v| js::truthy(Some(v)))
        .map(js::to_js_string);

    if let (true, Some(value)) = (item.item_type == "journalArticle", value_str.as_deref()) {
        let year = re(&YEAR, "[0-9]{4}");
        if let Some(y) = year.find(value) {
            if !item.truthy("date") {
                item.set("date", y.as_str());
            }
        }
        if let Some(m) = month.find(value) {
            let d = js_str(&item, "date");
            item.set("date", format!("{d} {}", m.as_str()));
        }
        let voliss = re(&VOLISS, r"([0-9]+)\(([0-9]+(?:-[0-9]+)?)\)");
        if let Some(c) = voliss.captures(value) {
            item.set("volume", c[1].to_owned());
            item.set("issue", c[2].to_owned());
        }
        if let Some(v) = cap(re(&VOL, r"vol\.{WS}*([0-9]+)"), value, 1) {
            item.set("volume", v);
        }
        if !item.truthy("volume") {
            if let Some(v) = cap(re(&YVOL, r"[0-9]{4};([0-9]+):"), value, 1) {
                item.set("volume", v);
            }
        }
        if let Some(v) = cap(
            re(&ISS, r"vol\.{WS}*[0-9]+{WS}*,{WS}*no\.{WS}*([0-9]+)"),
            value,
            1,
        ) {
            item.set("issue", v);
        }
        if pg_t.is_match(value) {
            if let Some(p) = cap(pg, value, 1) {
                item.set("pages", p);
            }
        }
        if let Some(p) = cap(pp, value, 1) {
            item.set("pages", p);
        }
        let journal = re(&JOURNAL, r"^{WS}*[J|j]ournal[-{WS}A-Za-z0-9_&:]+");
        let pt = if let Some(m) = journal.find(value) {
            m.as_str().to_owned()
        } else {
            let split = re(&PT_SPLIT, r"(\.|;|(,{WS}*vol\.))");
            let first = split.find(value).map_or(value, |m| &value[..m.start()]);
            trim_internal(first)
        };
        let pt = month
            .find(&pt)
            .map_or(pt.clone(), |m| pt[..m.start()].to_owned());
        item.set("publicationTitle", pt);
    }
    if let (true, Some(value)) = (item.item_type == "bookSection", value_str.as_deref()) {
        if !item.truthy("pages") {
            if pg_t.is_match(value) {
                if let Some(p) = cap(pg, value, 1) {
                    item.set("pages", p);
                }
            }
            if let Some(p) = cap(pp, value, 1) {
                item.set("pages", p);
            }
        }
        if re(&ED_T, r"({DOT}+?)\[Ed(itor|\.|\])").is_match(value) {
            for m in re(&ED_G, r"{DOT}+?\[Ed(itor|\.|\])").find_iter(value) {
                let e = re(&ED_1, r"\[Ed(itor|\.|\]){DOT}*$").replace(m.as_str(), "");
                let e = re(&ED_2, r"{DOT}*?\][,{WS}]*").replace(&e, "");
                item.creators.push(creator(&e, "editor", true));
            }
        }
        if re(&BT_T, r"{DOT}+\[Ed(?:\.|itor)?\][.{WS}]*([^.]+)").is_match(value) {
            let bt = re(
                &BT,
                r"{DOT}+\[Ed(?:\.|itor)?\][.{WS}]*(?:\([0-9]{4}\)\.)?([^.]+)",
            );
            if let Some(t) = cap(bt, value, 1) {
                item.set("bookTitle", t);
            }
        }
    }
    // Fix all-caps authors.
    for c in &mut item.creators {
        if let Some(last) = &c.last_name {
            if !last.is_empty() && *last == last.to_uppercase() {
                c.last_name = Some(capitalize_title(&last.to_lowercase(), true));
            }
        }
    }
    if item.truthy("pages") {
        let pages = js_str(&item, "pages");
        if let Some(c) = re(&RANGE, r"([0-9]+)-([0-9]+)").captures(&pages) {
            let start = &c[1];
            let end = &c[2];
            let diff = start.len() as isize - end.len() as isize;
            if diff > 0 {
                let m = c.get(0).unwrap();
                let new_range = format!("{start}-{}{end}", &start[..diff as usize]);
                let full = format!("{}{new_range}{}", &pages[..m.start()], &pages[m.end()..]);
                item.set("pages", full);
            }
        }
    }
    if (item.item_type == "book" || item.item_type == "bookSection") && !item.truthy("publisher") {
        let p = item.get("publishing").cloned().unwrap_or(Value::Null);
        item.set("publisher", p);
    }
    // `item.publisher && !item.pace` (sic: always true).
    if item.truthy("publisher") {
        let publisher = js_str(&item, "publisher");
        if re(&PUB_T, ",{DOT}").is_match(&publisher) {
            if let Some(place) = cap(re(&PLACE, ",({DOT}+?)$"), &publisher, 1) {
                item.set("place", place);
            }
            let p = re(&PUB_R, ",{DOT}+?$").replace(&publisher, "").into_owned();
            item.set("publisher", p);
        }
    }
    if item.item_type == "thesis" && item.truthy("institution") {
        let inst = js_str(&item, "institution");
        let p = re(&INST, "^{DOT}+:{WS}*").replace(&inst, "").into_owned();
        item.set("publisher", p);
        item.remove("institution");
    }
    if item.truthy("ISBN") {
        let s = js_str(&item, "ISBN");
        item.set(
            "ISBN",
            clean_isbn(&s, false).map_or(Value::Bool(false), Value::from),
        );
    }
    if item.truthy("ISSN") {
        let s = js_str(&item, "ISSN");
        item.set(
            "ISSN",
            clean_issn(&s).map_or(Value::Bool(false), Value::from),
        );
    }
    if item.truthy("DOI") {
        let s = js_str(&item, "DOI");
        item.set("DOI", clean_doi(&s).map_or(Value::Null, Value::from));
    }
    if item.truthy("callNumber") {
        let s = js_str(&item, "callNumber");
        let s = re(&CALL, "[.{WS}]+$").replace(&s, "").into_owned();
        item.set("callNumber", s);
    }
    if item.truthy("libraryCatalog")
        && js_str(&item, "libraryCatalog").contains("MEDLINE")
        && item.truthy("PMID")
    {
        let p = item.get("PMID").cloned().unwrap_or(Value::Null);
        item.set("extra", p);
        item.remove("PMID");
    }
    item.remove("publishing");
    item.remove("citation");
    if !ctx.in_child_translator() {
        item.remove("itemID");
    }
    ctx.item_done(item);
    Ok(())
}
