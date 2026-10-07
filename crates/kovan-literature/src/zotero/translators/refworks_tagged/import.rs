// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): RefWorks Tagged.js `processTag` :405-525,
//   `applyValue` :527-588, `dateRWtoZotero` :590-618, `completeItem`
//   :620-708, `RW_format`/`getLine` :712-766, `getNewItem` :768-773,
//   `doImport` :775-807.
// Copyright: no notice upstream; translator by Simon Kornblith, Aurimas
//   Vinckevicius and Sebastian Karcher (header `creator`).
// Licence: AGPL-3.0 (no licence text upstream; treated as AGPLv3 as part of
//   Zotero, maintainer decision 2026-10-07, #747).

//! RefWorks Tagged import.

use super::get_fields;
use super::is_line_terminator;
use super::tables::{DEGENERATE_IMPORT_FIELD_MAP, FIELD_MAP, IMPORT_TYPE_MAP};
use crate::zotero::framework::html::unescape_html;
use crate::zotero::framework::utilities::{clean_doi, field_is_valid_for_type};
use crate::zotero::framework::{
    js, ImportContext, JsObject, TranslateError, TranslatorCreator, TranslatorItem, TranslatorNote,
    TranslatorTag,
};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

/// `Zotero.Date.getMonths().long` (en-US), as `ZU.formatDate` names months.
const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// `DEFAULT_IMPORT_TYPE` (:53).
const DEFAULT_IMPORT_TYPE: &str = "journalArticle";

fn re(cell: &'static OnceLock<Regex>, pattern: impl FnOnce() -> String) -> &'static Regex {
    cell.get_or_init(|| Regex::new(&pattern()).expect("static regex compiles"))
}

/// An item under construction with the translator's scratch properties
/// (`unknownFields`, `unsupportedFields`, the `backup*` values), which
/// upstream keeps on the item and sets to `undefined` before completing it.
struct Work {
    item: TranslatorItem,
    unknown_fields: Vec<String>,
    unsupported_fields: Vec<String>,
    backup_publication_title: Option<String>,
    backup_num_pages: Option<String>,
    backup_end_page: Option<String>,
    /// `{field, value}`: the field's name (the one-element array upstream
    /// uses as a property key) and the date.
    backup_date: Option<(String, String)>,
}

/// `getNewItem(type)` (:768-773).
fn get_new_item(item_type: &str) -> Work {
    Work {
        item: TranslatorItem::new(item_type),
        unknown_fields: Vec::new(),
        unsupported_fields: Vec::new(),
        backup_publication_title: None,
        backup_num_pages: None,
        backup_end_page: None,
        backup_date: None,
    }
}

/// A value as `processTag` holds it.
enum Val {
    Undefined,
    Str(String),
    Creator(TranslatorCreator),
    Tags(Vec<String>),
    /// `{note}`; `None` is `{note: undefined}`.
    Note(Option<String>),
    Attachment(JsObject),
}

impl Val {
    fn truthy(&self) -> bool {
        match self {
            Val::Undefined => false,
            Val::Str(s) => !s.is_empty(),
            _ => true,
        }
    }
}

/// `RW_format` (:712) applied to a line: `[raw, tag, value]`.
fn rw_format(line: &str) -> Option<[String; 3]> {
    let c: Vec<char> = line.chars().collect();
    if c.len() < 3
        || !c[0].is_ascii_uppercase()
        || !(c[1].is_ascii_uppercase() || c[1].is_ascii_digit())
        || c[2] != ' '
    {
        return None;
    }
    // `(?:(.*))?$`: the rest, which `.` must cover to the end.
    if c[3..].iter().any(|&ch| is_line_terminator(ch)) {
        return None;
    }
    Some([
        line.to_owned(),
        c[..2].iter().collect(),
        c[3..].iter().collect(),
    ])
}

/// `getLine` (:713-766), with `getLine.buffer`.
struct LineReader {
    buffer: Option<String>,
}

impl LineReader {
    fn get_line(&mut self, ctx: &mut ImportContext) -> Option<[String; 3]> {
        let mut entry: Option<[String; 3]> = None;
        let mut last_line_length = 0;
        if let Some(b) = self.buffer.take() {
            let e = rw_format(&b).expect("the buffer holds a matching line");
            last_line_length = js::len(&e[2]);
            entry = Some(e);
        }
        while let Some(next_line) = ctx.read_line() {
            let temp = rw_format(&next_line);
            match (temp, entry.as_mut()) {
                (Some(t), Some(_)) => {
                    self.buffer = Some(t[0].clone());
                    return entry;
                }
                (Some(t), None) => {
                    last_line_length = js::len(&t[2]);
                    entry = Some(t);
                }
                (None, Some(e)) => {
                    let mut next = next_line;
                    if (e[1] == "AB" || e[1] == "NO") && last_line_length < 60 {
                        next = format!("\r\n{next}");
                    }
                    if e[1] == "K1" {
                        next = format!("\r\n{next}");
                    }
                    if !e[2].ends_with(' ') {
                        next = format!(" {next}");
                    }
                    e[0].push_str(&next);
                    e[2].push_str(&next);
                }
                (None, None) => {}
            }
        }
        entry
    }
}

/// `doImport` (:775-807).
pub fn do_import(ctx: &mut ImportContext) -> Result<(), TranslateError> {
    let mut lines = LineReader { buffer: None };
    let mut entry;
    loop {
        entry = lines.get_line(ctx);
        match &entry {
            Some(e) if e[1] != "RT" => continue,
            _ => break,
        }
    }
    let mut item: Option<Work> = None;
    while let Some(e) = entry {
        if e[1] == "RT" {
            if let Some(w) = item.take() {
                complete_item(ctx, w);
            }
            let key = js::trim(&e[2]);
            let ty = IMPORT_TYPE_MAP
                .iter()
                .find(|(k, _)| *k == key)
                .map_or(DEFAULT_IMPORT_TYPE, |(_, v)| v);
            item = Some(get_new_item(ty));
        } else if let Some(w) = item.as_mut() {
            process_tag(ctx, w, &e);
        }
        entry = lines.get_line(ctx);
    }
    if let Some(w) = item {
        complete_item(ctx, w);
    }
    Ok(())
}

/// JavaScript `str.split(regex)` for a regex that never matches empty.
fn js_split(re: &Regex, s: &str) -> Vec<String> {
    re.split(s).map(str::to_owned).collect()
}

/// `processTag(item, entry)` (:405-525).
fn process_tag(ctx: &ImportContext, w: &mut Work, entry: &[String; 3]) {
    let tag = entry[1].as_str();
    let mut value = js::trim(&entry[2]).to_owned();
    let raw_line = &entry[0];

    let z = get_fields(
        &[FIELD_MAP, DEGENERATE_IMPORT_FIELD_MAP],
        &w.item.item_type,
        tag,
    )
    .first()
    .copied()
    .unwrap_or("unknown");
    if value.is_empty() {
        return;
    }
    let z_field: Vec<&str> = z.split('/').collect();

    if tag != "NO" && tag != "AB" {
        value = unescape_html(&value);
    }

    let mut v = Val::Str(value);
    let mut process_fields = true;
    match tag {
        "NO" => {
            let Val::Str(s) = &v else { unreachable!() };
            if w.item.get_str("title") == Some(s.as_str()) {
                v = Val::Undefined;
            } else {
                static TAG: OnceLock<Regex> = OnceLock::new();
                if !re(&TAG, || "<[^>]+>".to_owned()).is_match(s) {
                    let html = s
                        .replace("\n\n", "</p><p>")
                        .replace('\n', "<br/>")
                        .replace('\t', "&nbsp;&nbsp;&nbsp;&nbsp;")
                        .replace("  ", "&nbsp;&nbsp;");
                    v = Val::Str(format!("<p>{html}</p>"));
                }
            }
        }
        "OP" => {
            let Val::Str(s) = &v else { unreachable!() };
            match w.item.get_string("pages").filter(|p| !p.is_empty()) {
                Some(p) => {
                    if !p.contains('-') {
                        w.item.set("pages", format!("{p}-{s}"));
                    } else {
                        w.backup_num_pages = Some(s.clone());
                    }
                }
                None => w.backup_end_page = Some(s.clone()),
            }
            v = Val::Undefined;
        }
        "YR" => {
            let Val::Str(s) = &v else { unreachable!() };
            // `field: zField` is the array; as a property key it is its
            // elements joined with ",".
            w.backup_date = Some((z_field.join(","), date_rw_to_zotero(s)));
            v = Val::Undefined;
            process_fields = false;
        }
        _ => {}
    }

    if process_fields {
        match z_field[0] {
            "backupPublicationTitle" => {
                w.backup_publication_title = match &v {
                    Val::Str(s) => Some(s.clone()),
                    _ => None,
                };
                v = Val::Undefined;
            }
            "creators" => {
                if let Val::Str(s) = &v {
                    static COMMA: OnceLock<Regex> = OnceLock::new();
                    let comma = re(&COMMA, || format!("{ws}*,{ws}*", ws = js::WS));
                    let parts = js_split(comma, s);
                    v = Val::Creator(TranslatorCreator {
                        last_name: parts.first().cloned(),
                        first_name: parts.get(1).cloned(),
                        creator_type: z_field.get(1).map(|t| (*t).to_owned()),
                        ..Default::default()
                    });
                }
            }
            "date" | "accessDate" | "filingDate" | "issueDate" | "dateEnacted" | "dateDecided" => {
                if let Val::Str(s) = &v {
                    v = Val::Str(date_rw_to_zotero(s));
                }
            }
            "tags" => {
                if let Val::Str(s) = &v {
                    static SEP: OnceLock<Regex> = OnceLock::new();
                    let sep = re(&SEP, || {
                        format!(r"{ws}*(?:[\r\n]+{ws}*)+|{ws}*(?:;{ws}*)+", ws = js::WS)
                    });
                    let mut t = js_split(sep, s);
                    if t.first().is_some_and(String::is_empty) {
                        t.remove(0);
                    }
                    if t.last().is_some_and(String::is_empty) {
                        t.pop();
                    }
                    v = if t.is_empty() {
                        Val::Undefined
                    } else {
                        Val::Tags(t)
                    };
                }
            }
            "notes" => {
                let note = match &v {
                    Val::Str(s) => Some(s.clone()),
                    _ => None,
                };
                let note = match z_field.get(1) {
                    // `zField[1] + ': ' + value.note`
                    Some(t) => Some(format!(
                        "{t}: {}",
                        note.unwrap_or_else(|| "undefined".to_owned())
                    )),
                    None => note,
                };
                v = Val::Note(note);
            }
            "attachments" => {
                if let Val::Str(s) = &v {
                    static DOMAIN: OnceLock<Regex> = OnceLock::new();
                    let d = re(&DOMAIN, || r"(?i)^https?://([^/]+)".to_owned());
                    let domain = d
                        .captures(s)
                        .map_or(String::new(), |c| format!("{} ", &c[1]));
                    let mut a = JsObject::new();
                    a.set("path", s.clone());
                    a.set("title", format!("{domain}Link"));
                    a.set("mimeType", "text/html");
                    v = Val::Attachment(a);
                }
            }
            "unsupported" => {
                if let (Val::Str(s), Some(t)) = (&v, z_field.get(1)) {
                    v = Val::Str(format!("{t}: {s}"));
                }
            }
            _ => {}
        }
    }

    apply_value(ctx, w, z_field[0], v, raw_line);
}

/// `applyValue(item, zField, value, rawLine)` (:527-588).
fn apply_value(ctx: &ImportContext, w: &mut Work, z_field: &str, value: Val, raw_line: &str) {
    if !value.truthy() {
        return;
    }
    let child = ctx.in_child_translator();
    if z_field.is_empty() || z_field == "unknown" {
        if !child {
            w.unknown_fields.push(raw_line.to_owned());
        }
        return;
    }
    if z_field == "unsupported" {
        if !child {
            if let Val::Str(s) = &value {
                w.unsupported_fields.push(s.clone());
            }
        }
        return;
    }
    if !matches!(z_field, "creators" | "tags" | "notes" | "attachments")
        && !field_is_valid_for_type(z_field, &w.item.item_type)
        && !child
    {
        w.unknown_fields.push(raw_line.to_owned());
        return;
    }
    match (z_field, value) {
        ("notes", Val::Note(n)) => w
            .item
            .notes
            .push(TranslatorNote::new(n.unwrap_or_default())),
        ("attachments", Val::Attachment(a)) => w.item.attachments.push(a),
        ("creators", Val::Creator(c)) => w.item.creators.push(c),
        ("tags", Val::Tags(t)) => w.item.tags.extend(t.into_iter().map(TranslatorTag::new)),
        // A string under one of the array fields: `[value]` concatenated.
        ("notes", Val::Str(s)) => w.item.notes.push(TranslatorNote::new(s)),
        ("tags", Val::Str(s)) => w.item.tags.push(TranslatorTag::new(s)),
        ("extra", Val::Str(s)) => {
            let extra = match w.item.get_string("extra").filter(|e| !e.is_empty()) {
                Some(e) => format!("{e}; {s}"),
                None => s,
            };
            w.item.set("extra", extra);
        }
        (f, Val::Str(s)) => {
            if w.item.truthy(f) {
                let cur = w.item.get_string(f).unwrap_or_default();
                if !child && cur != s {
                    w.item.notes.push(TranslatorNote::new(raw_line));
                }
            } else {
                w.item.set(f, s);
            }
        }
        _ => {}
    }
}

/// `parseInt(s, 10)`: `None` is NaN.
fn parse_int(s: &str) -> Option<i64> {
    let t = js::trim_start(s);
    let (neg, rest) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let d: String = rest.chars().take_while(char::is_ascii_digit).collect();
    d.parse::<i64>().ok().map(|n| if neg { -n } else { n })
}

/// `risDate.split(/\s*\/\s*(?:0*(?=\d))?/)`: the separator is optional
/// whitespace, a slash, optional whitespace, then the leading zeros of a
/// number (all but the last digit when the number is all zeros).
fn split_rw_date(s: &str) -> Vec<String> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut piece_start = 0;
    let mut q = 0;
    while q < c.len() {
        // A match must start at q: whitespace, then '/'.
        let mut k = q;
        while k < c.len() && js::is_space(c[k]) {
            k += 1;
        }
        if k < c.len() && c[k] == '/' {
            k += 1;
            while k < c.len() && js::is_space(c[k]) {
                k += 1;
            }
            let mut z = 0;
            while k + z < c.len() && c[k + z] == '0' {
                z += 1;
            }
            if k + z < c.len() && c[k + z].is_ascii_digit() {
                k += z;
            } else if z > 0 {
                k += z - 1;
            }
            out.push(c[piece_start..q].iter().collect());
            piece_start = k;
            q = k;
            continue;
        }
        q += 1;
    }
    out.push(c[piece_start..].iter().collect());
    out
}

/// `dateRWtoZotero(risDate)` (:590-618).
fn date_rw_to_zotero(ris_date: &str) -> String {
    let parts = split_rw_date(ris_date);
    if parts.len() == 1 {
        return ris_date.to_owned();
    }
    let mut value: Vec<Option<String>> = parts.into_iter().map(Some).collect();
    while value.len() < 4 {
        value.push(None);
    }
    let mut i = 0;
    while i < 3 {
        let ok = value[i]
            .as_deref()
            .is_some_and(|v| !v.is_empty() && parse_int(v).is_some_and(|n| n != 0));
        if !ok {
            break;
        }
        i += 1;
    }
    for v in value.iter_mut().take(3).skip(i) {
        *v = None;
    }
    // `value[1] = parseInt(value[1]); if (value[1]) value[1]--;`
    let month: Option<i64> = value[1].as_deref().and_then(parse_int).map(|m| m - 1);
    let month = month.and_then(|m| u32::try_from(m).ok());
    let day = value[2].clone();
    let part = value[3].clone();
    // `ZU.formatDate({year, month, day, part})` (date.js:567-598) with the
    // string year, day and part dateRWtoZotero passes.
    let mut s = String::new();
    if let Some(p) = part.as_deref().filter(|p| !p.is_empty()) {
        s.push_str(p);
        s.push(' ');
    }
    if let Some(name) = month.and_then(|m| MONTHS.get(m as usize)) {
        s.push_str(name);
        match day.as_deref().filter(|d| !d.is_empty()) {
            Some(d) => s.push_str(&format!(" {d}, ")),
            None => s.push(' '),
        }
    }
    if let Some(y) = value[0].as_deref().filter(|y| !y.is_empty()) {
        s.push_str(y);
    }
    s
}

/// `completeItem(item)` (:620-708).
fn complete_item(ctx: &mut ImportContext, mut w: Work) {
    let child = ctx.in_child_translator();
    let item = &mut w.item;
    if let Some(b) = w.backup_publication_title.take().filter(|b| !b.is_empty()) {
        if !item.truthy("publicationTitle") {
            item.set("publicationTitle", b);
        }
    }
    if let Some(b) = w.backup_num_pages.take().filter(|b| !b.is_empty()) {
        if !item.truthy("numPages") {
            item.set("numPages", b);
        }
    }
    if let Some(b) = w.backup_end_page.take().filter(|b| !b.is_empty()) {
        if !item.truthy("pages") {
            item.set("pages", b);
        } else {
            let p = item.get_string("pages").unwrap_or_default();
            if !p.contains('-') {
                item.set("pages", format!("{p}-{b}"));
            } else if !item.truthy("numPages") {
                item.set("numPages", b);
            }
        }
    }
    if let Some((field, value)) = w.backup_date.take() {
        if !item.truthy(&field) {
            item.set(field, value);
        } else {
            static Y4: OnceLock<Regex> = OnceLock::new();
            let cur = item.get_string(&field).unwrap_or_default();
            if !re(&Y4, || "[0-9]{4}".to_owned()).is_match(&cur) {
                item.set(field, format!("{cur} {value}"));
            }
        }
    }
    if item.truthy("DOI") {
        let doi = item.get_string("DOI").unwrap_or_default();
        match clean_doi(&doi) {
            Some(d) => item.set("DOI", d),
            None => item.set("DOI", Value::Null),
        }
    }
    if item.truthy("journalAbbreviation") && !item.truthy("publicationTitle") {
        let j = item.get("journalAbbreviation").cloned().unwrap();
        item.set("publicationTitle", j);
    }
    if item.truthy("shortTitle") && !item.truthy("title") {
        let t = item.get("shortTitle").cloned().unwrap();
        item.set("title", t);
    }
    if item.tags.len() == 1 {
        static SEP: OnceLock<Regex> = OnceLock::new();
        let sep = re(&SEP, || format!("{ws}*(?:,{ws}*)+", ws = js::WS));
        let mut t = js_split(sep, &item.tags[0].tag);
        if t.first().is_some_and(String::is_empty) {
            t.remove(0);
        }
        if t.last().is_some_and(String::is_empty) {
            t.pop();
        }
        item.tags = t.into_iter().map(TranslatorTag::new).collect();
    }
    if child {
        item.remove("accessDate");
    }
    if !child {
        let mut note = String::new();
        for f in w.unsupported_fields.iter().chain(w.unknown_fields.iter()) {
            note.push_str(f);
            note.push_str("<br/>");
        }
        if !note.is_empty() {
            let note =
                format!("The following values have no corresponding Zotero field:<br/>{note}");
            let mut n = TranslatorNote::new(js::trim(&note));
            n.props.set("tags", serde_json::json!(["_RW import"]));
            item.notes.push(n);
        }
    }
    ctx.item_done(w.item);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rw_dates() {
        assert_eq!(split_rw_date("2001/05/03"), ["2001", "5", "3"]);
        assert_eq!(split_rw_date("2001 / 00"), ["2001", "0"]);
        assert_eq!(date_rw_to_zotero("2001/05/03"), "May 3, 2001");
        assert_eq!(date_rw_to_zotero("2001/0/0/Spring"), "Spring 2001");
        assert_eq!(date_rw_to_zotero("2001/12"), "December 2001");
        assert_eq!(date_rw_to_zotero("May 2001"), "May 2001");
    }
}
