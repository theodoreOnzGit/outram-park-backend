// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): RIS.js `processTag` :1281-1491, `applyValue`
//   :1493-1561, `dateRIStoZotero` :1563-1696, `completeItem` :1698-1796,
//   `getNewItem` :1799-1804, `doImport`/`startImport`/`importNext`
//   :1806-1912.
// Copyright (C) 2006-2023 Simon Kornblith, Aurimas Vinckevicus, Abe Jellinek.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! RIS import.

use super::mapper::TagMapper;
use super::reader::{citavi_clean, end_note_clean, RisReader, TagValue};
use super::tables::{DEGENERATE_IMPORT_FIELD_MAP, FIELD_MAP, IMPORT_TYPE_MAP};
use crate::zotero::framework::html::unescape_html;
use crate::zotero::framework::utilities::{clean_doi, decode_uri_component, field_is_valid_for_type};
use crate::zotero::framework::{
    js, ImportContext, JsObject, TranslateError, TranslatorCreator, TranslatorItem, TranslatorNote,
    TranslatorTag,
};
use kovan_common::zotero::date::str_to_date;
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

/// `DEFAULT_IMPORT_TYPE` (:81).
const DEFAULT_IMPORT_TYPE: &str = "journalArticle";

fn re(cell: &'static OnceLock<Regex>, pattern: impl FnOnce() -> String) -> &'static Regex {
    cell.get_or_init(|| Regex::new(&pattern()).expect("static regex compiles"))
}

/// The properties `getNewItem` and `processTag` keep on the item only until
/// `completeItem` sets them to `undefined` (so `_itemDone` drops them):
/// `unknownFields`, `unsupportedFields` and the `backup*` values.
#[derive(Debug, Clone, Default)]
struct Scratch {
    unknown_fields: Vec<String>,
    unsupported_fields: Vec<String>,
    backup_publication_title: Option<String>,
    backup_num_pages: Option<String>,
    backup_end_page: Option<String>,
    /// `{field, value}`.
    backup_date: Option<(String, String)>,
    backup_access_date: Option<(String, String)>,
}

/// A value on its way through `processTag` and `applyValue`.
#[derive(Debug, Clone)]
enum RisValue {
    /// `undefined` / `false`.
    None,
    Str(String),
    Creator(TranslatorCreator),
    Tags(Vec<String>),
    Note(String),
}

impl RisValue {
    fn truthy(&self) -> bool {
        match self {
            RisValue::None => false,
            RisValue::Str(s) => !s.is_empty(),
            _ => true,
        }
    }
}

/// The settings `startImport` reads (:1823-1847).
struct Settings {
    fields: TagMapper,
    ignore_unknown: bool,
    parent: bool,
}

/// `doImport` (:1806-1912).
pub fn do_import(ctx: &mut ImportContext) -> Result<(), TranslateError> {
    let mut ignore_unknown = true;
    if let Some(p) = ctx.get_hidden_pref("RIS.import.ignoreUnknown") {
        // `if (pref != undefined) ignoreUnknown = pref;` (null too is skipped)
        if !p.is_null() {
            ignore_unknown = js::truthy(Some(&p));
        }
    }
    let keep_id = ctx.get_hidden_pref("RIS.import.keepID") == Some(Value::Bool(true));
    let settings = Settings {
        fields: TagMapper::new(
            vec![FIELD_MAP, DEGENERATE_IMPORT_FIELD_MAP],
            Some(DEGENERATE_IMPORT_FIELD_MAP),
            keep_id,
        ),
        ignore_unknown,
        parent: ctx.in_child_translator(),
    };
    let mut reader = RisReader::default();
    while let Some(mut entry) = reader.next_entry(ctx) {
        let mut item_type: Option<String> = None;
        if let Some(ty) = entry.first("TY") {
            let ris_type = js::trim(&ty.value).to_uppercase();
            item_type = IMPORT_TYPE_MAP
                .iter()
                .find(|(k, _)| *k == ris_type)
                .map(|(_, v)| (*v).to_owned());
        }
        let item_type = item_type.unwrap_or_else(|| DEFAULT_IMPORT_TYPE.to_owned());
        let mut item = TranslatorItem::new(item_type.clone());
        let mut scratch = Scratch::default();
        reader.pro_cite_clean(&mut entry, &item_type, &settings.fields)?;
        end_note_clean(&mut entry)?;
        citavi_clean(&mut entry)?;

        let mut deferred: Vec<TagValue> = Vec::new();
        for i in 0..entry.len() {
            let tv = entry.at(i).clone();
            if tv.tag == "TY" || tv.tag == "ER" {
                continue;
            }
            if !process_tag(ctx, &settings, &mut item, &mut scratch, &tv, false) {
                deferred.push(tv);
            }
        }
        for tv in &deferred {
            process_tag(ctx, &settings, &mut item, &mut scratch, tv, true);
        }
        complete_item(ctx, &settings, item, scratch);
    }
    Ok(())
}

/// `item[field]` is truthy.
fn has(item: &TranslatorItem, field: &str) -> bool {
    item.truthy(field)
}

/// `processTag(item, tagValue, risEntry, allowDeprecated)` (:1281-1491):
/// `false` when the tag was not processed (empty, or deprecated and not
/// allowed).
fn process_tag(
    ctx: &ImportContext,
    s: &Settings,
    item: &mut TranslatorItem,
    scratch: &mut Scratch,
    tv: &TagValue,
    allow_deprecated: bool,
) -> bool {
    let tag = tv.tag.as_str();
    let raw_value = js::trim(&tv.value).to_owned();
    if raw_value.is_empty() {
        return false;
    }
    if !allow_deprecated && s.fields.is_deprecated(tag) {
        return false;
    }
    let z = s
        .fields
        .get_field(&item.item_type, tag)
        .unwrap_or("unknown");
    let mut z_field: Vec<String> = z.split('/').map(str::to_owned).collect();

    let mut value = if tag != "N1" && tag != "RN" && tag != "AB" {
        RisValue::Str(unescape_html(&raw_value))
    } else {
        RisValue::Str(raw_value)
    };

    let mut process_fields = true;
    match tag {
        "N1" | "RN" => {
            let v = match &value {
                RisValue::Str(v) => v.clone(),
                _ => unreachable!(),
            };
            if item.get_str("title") == Some(v.as_str()) {
                value = RisValue::None;
                process_fields = false;
            } else {
                static TAG: OnceLock<Regex> = OnceLock::new();
                if !re(&TAG, || "<[^>]+>".to_owned()).is_match(&v) {
                    value = RisValue::Str(format!(
                        "<p>{}</p>",
                        v.replace("\n\n", "</p><p>")
                            .replace('\n', "<br/>")
                            .replace('\t', "&nbsp;&nbsp;&nbsp;&nbsp;")
                            .replace("  ", "&nbsp;&nbsp;")
                    ));
                }
            }
        }
        "EP" => {
            let v = str_of(&value);
            if let Some(pages) = item
                .get_str("pages")
                .filter(|p| !p.is_empty())
                .map(str::to_owned)
            {
                if !pages.contains('-') {
                    item.set("pages", format!("{pages}-{v}"));
                } else {
                    scratch.backup_num_pages = Some(v);
                }
            } else {
                scratch.backup_end_page = Some(v);
            }
            value = RisValue::None;
        }
        "M1" => {
            if z_field[0] == "accessDate" {
                let v = str_of(&value);
                scratch.backup_access_date =
                    Some((z_field[0].clone(), date_ris_to_zotero(ctx, &v, &z_field[0])));
                value = RisValue::None;
                process_fields = false;
            }
        }
        "M3" => {
            if z_field[0] == "DOI" {
                match clean_doi(&str_of(&value)) {
                    Some(d) => value = RisValue::Str(d),
                    None => z_field[0] = "unknown".to_owned(),
                }
            }
        }
        "VL" => {
            if z_field[0] == "accessDate" {
                if scratch.backup_access_date.is_none() {
                    let v = str_of(&value);
                    scratch.backup_access_date =
                        Some((z_field[0].clone(), date_ris_to_zotero(ctx, &v, &z_field[0])));
                }
                value = RisValue::None;
                process_fields = false;
            }
        }
        "PY" => {
            let v = str_of(&value);
            scratch.backup_date =
                Some((z_field[0].clone(), date_ris_to_zotero(ctx, &v, &z_field[0])));
            value = RisValue::None;
            process_fields = false;
        }
        "UR" => {
            let v = str_of(&value);
            if v.contains("PM:") {
                value = RisValue::Str(format!("PMID: {}", js::substr(&v, 3, usize::MAX)));
                z_field = vec!["extra".to_owned()];
            }
        }
        _ => {}
    }

    if process_fields {
        match z_field[0].as_str() {
            "__ignore" => value = RisValue::None,
            "backupPublicationTitle" => {
                scratch.backup_publication_title = Some(str_of(&value));
                value = RisValue::None;
            }
            "creators" => {
                let v = str_of(&value);
                static COMMA: OnceLock<Regex> = OnceLock::new();
                let comma = re(&COMMA, || format!("{ws}*,{ws}*", ws = js::WS));
                let l_name = comma.split(&v).next().unwrap_or("").to_owned();
                static LEAD: OnceLock<Regex> = OnceLock::new();
                let rest = js::substr(&v, js::len(&l_name), usize::MAX);
                let f_name = re(&LEAD, || format!("^{ws}*,{ws}*", ws = js::WS))
                    .replace(&rest, "")
                    .into_owned();
                let mut c = TranslatorCreator {
                    last_name: Some(l_name),
                    creator_type: z_field.get(1).cloned(),
                    ..Default::default()
                };
                if f_name.is_empty() {
                    c.field_mode = Some(1);
                } else {
                    c.first_name = Some(f_name);
                }
                value = RisValue::Creator(c);
            }
            "date" | "accessDate" | "filingDate" | "issueDate" | "dateEnacted" | "dateDecided" => {
                let v = str_of(&value);
                value = RisValue::Str(date_ris_to_zotero(ctx, &v, &z_field[0]));
            }
            "tags" => {
                static SPLIT: OnceLock<Regex> = OnceLock::new();
                let split = re(&SPLIT, || {
                    format!(
                        r"{ws}*(?:[\r\n]+{ws}*)+(?:%K{ws}+)?|{ws}*(?:;{ws}*)+",
                        ws = js::WS
                    )
                });
                let v = str_of(&value);
                let mut parts: Vec<String> = split.split(&v).map(str::to_owned).collect();
                if parts.first().is_some_and(String::is_empty) {
                    parts.remove(0);
                }
                if parts.last().is_some_and(String::is_empty) {
                    parts.pop();
                }
                value = if parts.is_empty() {
                    RisValue::None
                } else {
                    RisValue::Tags(parts)
                };
            }
            "notes" => {
                let mut note = str_of(&value);
                if let Some(t) = z_field.get(1) {
                    note = format!("{t}: {note}");
                }
                value = RisValue::Note(note);
            }
            "attachments" => {
                let v = str_of(&value);
                static INTERNAL: OnceLock<Regex> = OnceLock::new();
                static TITLE: OnceLock<Regex> = OnceLock::new();
                let mut mime_type: Option<&str> = None;
                for line in v.split('\n') {
                    let url =
                        re(&INTERNAL, || "(?i)^internal-pdf://".to_owned()).replace(line, "PDF/");
                    let url = js::trim(&url).to_owned();
                    if url.is_empty() {
                        continue;
                    }
                    let mut title =
                        match re(&TITLE, || r"([^/\\]+)(?:\.[A-Za-z0-9_]{1,8})$".to_owned())
                            .captures(&url)
                        {
                            Some(m) => {
                                decode_uri_component(&m[1]).unwrap_or_else(|| m[1].to_owned())
                            }
                            None => "Attachment".to_owned(),
                        };
                    if z_field.get(1).map(String::as_str) == Some("HTML") {
                        title = "Full Text (HTML)".to_owned();
                        mime_type = Some("text/html");
                    }
                    let mut a = JsObject::new();
                    a.set("title", title);
                    a.set("path", url);
                    if let Some(m) = mime_type {
                        a.set("mimeType", m);
                    }
                    item.attachments.push(a);
                }
                value = RisValue::None;
            }
            "unsupported" => {
                if let Some(label) = z_field.get(1) {
                    value = RisValue::Str(format!("{label}: {}", str_of(&value)));
                }
            }
            _ => {}
        }
    }
    apply_value(s, item, scratch, &z_field[0], value, &tv.raw);
    true
}

/// The string in a value (processTag only calls this where upstream's value
/// is a string).
fn str_of(v: &RisValue) -> String {
    match v {
        RisValue::Str(s) => s.clone(),
        _ => String::new(),
    }
}

/// `applyValue(item, zField, value, rawLine)` (:1493-1561).
fn apply_value(
    s: &Settings,
    item: &mut TranslatorItem,
    scratch: &mut Scratch,
    z_field: &str,
    value: RisValue,
    raw_line: &str,
) {
    if !value.truthy() {
        return;
    }
    if z_field.is_empty() || z_field == "unknown" {
        if !s.ignore_unknown && !s.parent {
            scratch.unknown_fields.push(raw_line.to_owned());
        }
        return;
    }
    if z_field == "unsupported" {
        if !s.ignore_unknown && !s.parent {
            scratch.unsupported_fields.push(str_of(&value));
        }
        return;
    }
    if !s.parent
        && !matches!(
            z_field,
            "creators" | "tags" | "notes" | "attachments" | "DOI"
        )
        && !field_is_valid_for_type(z_field, &item.item_type)
        && !s.ignore_unknown
    {
        scratch.unknown_fields.push(raw_line.to_owned());
        return;
    }
    match z_field {
        "notes" | "attachments" | "creators" | "tags" => match value {
            RisValue::Creator(c) if z_field == "creators" => item.creators.push(c),
            RisValue::Tags(t) if z_field == "tags" => {
                item.tags.extend(t.into_iter().map(TranslatorTag::new))
            }
            RisValue::Note(n) if z_field == "notes" => item.notes.push(TranslatorNote::new(n)),
            // A string under one of these names (not produced by the maps).
            RisValue::Str(v) => match z_field {
                "tags" => item.tags.push(TranslatorTag::new(v)),
                "notes" => item.notes.push(TranslatorNote::new(v)),
                _ => {}
            },
            _ => {}
        },
        "extra" => {
            let v = str_of(&value);
            match item
                .get_str("extra")
                .filter(|e| !e.is_empty())
                .map(str::to_owned)
            {
                Some(e) => item.set("extra", format!("{e}\n{v}")),
                None => item.set("extra", v),
            }
        }
        "DOI" => {
            let v = clean_doi(&str_of(&value));
            item.set("DOI", v.map_or(Value::Null, Value::String));
        }
        _ => {
            let v = str_of(&value);
            if has(item, z_field) {
                let existing = item.get_string(z_field).unwrap_or_default();
                if !s.ignore_unknown && !s.parent && existing != v {
                    scratch.unsupported_fields.push(format!("{z_field}: {v}"));
                }
            } else {
                item.set(z_field, v);
            }
        }
    }
}

/// `dateRIStoZotero(risDate, zField)` (:1563-1696).
pub fn date_ris_to_zotero(ctx: &ImportContext, ris_date: &str, z_field: &str) -> String {
    static FULL: OnceLock<Regex> = OnceLock::new();
    static Y4: OnceLock<Regex> = OnceLock::new();
    static DAY: OnceLock<Regex> = OnceLock::new();
    static ALPHA: OnceLock<Regex> = OnceLock::new();
    let full = re(&FULL, || {
        format!(
            r"^([0-9]+)(?:/([0-9]{{0,2}})(?:/([0-9]{{0,2}})(?:(?:/|{ws})([^/]*))?)?)?$",
            ws = js::WS
        )
    });
    // `date` as JavaScript holds it: undefined entries are None.
    let mut date: Vec<Option<String>>;
    let mut time_check: Option<String> = None;
    if let Some(m) = full.captures(ris_date) {
        date = vec![
            Some(m[1].to_owned()),
            m.get(2).map(|x| x.as_str().to_owned()),
            m.get(3).map(|x| x.as_str().to_owned()),
        ];
        time_check = m.get(4).map(|x| x.as_str().to_owned());
    } else {
        let y = re(&Y4, || r"(?-u:\b)[0-9]{4}(?-u:\b)".to_owned()).find(ris_date);
        let d = re(&DAY, || r"(?-u:\b)(?:[1-3][0-9]|[1-9])(?-u:\b)".to_owned()).find(ris_date);
        let m = re(&ALPHA, || "[A-Za-z]+".to_owned()).find(ris_date);
        if let (None, Some(m)) = (y, m) {
            return format!(
                "0000 {}{}",
                m.as_str(),
                d.map_or(String::new(), |d| format!(" {}", d.as_str()))
            );
        }
        if z_field != "accessDate" {
            return ris_date.to_owned();
        }
        let parsed = str_to_date(ris_date, &ctx.options.env.dates);
        let Some(year) = parsed.year.filter(|y| !y.is_empty()) else {
            return ris_date.to_owned();
        };
        // `'' + (parsedDate.month + 1)`, `'' + parsedDate.day`: NaN and
        // "undefined" when absent.
        date = vec![
            Some(year),
            Some(
                parsed
                    .month
                    .map_or("NaN".to_owned(), |m| (m + 1).to_string()),
            ),
            Some(parsed.day.map_or("undefined".to_owned(), |d| d.to_string())),
        ];
    }

    // :1604-1611: drop leading zeros; an empty part ends the date.
    for i in 0..3 {
        if i >= date.len() {
            break;
        }
        if let Some(p) = date[i].as_mut() {
            *p = p.trim_start_matches('0').to_owned();
        }
        if date[i].as_deref().is_none_or(str::is_empty) {
            date.truncate(i);
            break;
        }
    }
    let part = |i: usize| date.get(i).cloned().flatten();

    if z_field == "accessDate" {
        let Some(year) = part(0) else {
            return ris_date.to_owned();
        };
        let mut month = part(1);
        if let Some(m) = &month {
            // parseInt then `- 1`, kept as a string.
            month = Some(match parse_int(m) {
                Some(n) if n != 0 => (n - 1).to_string(),
                _ => "0".to_owned(),
            });
        }
        let month = month
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| "0".to_owned());
        let day = part(2)
            .filter(|d| !d.is_empty())
            .unwrap_or_else(|| "1".to_owned());

        let mut time: Option<(String, String, String)> = None;
        if let Some(tc) = time_check.filter(|t| !t.is_empty()) {
            static TIME: OnceLock<Regex> = OnceLock::new();
            let tre = re(&TIME, || {
                format!(
                    r"(?i)(?-u:\b)([0-2]?[1-9]):([0-9]{{2}})(?::([0-9]{{2}})){ws}*(am|pm)?",
                    ws = js::WS
                )
            });
            if let Some(t) = tre.captures(&tc) {
                let mut h = t[1].to_owned();
                let mi = t[2].to_owned();
                let sec = t.get(3).map_or("0".to_owned(), |x| x.as_str().to_owned());
                if let Some(ap) = t.get(4) {
                    let hour = parse_int(&h).unwrap_or(0);
                    let ap = ap.as_str().to_lowercase();
                    if ap == "pm" && hour < 12 {
                        h = (hour + 12).to_string();
                    } else if ap == "am" && hour == 12 {
                        h = "0".to_owned();
                    }
                }
                time = Some((h, mi, sec));
            }
        }
        return access_date_string(ctx, &year, &month, &day, time.as_ref());
    }

    if date.is_empty() {
        return String::new();
    }
    let mut out = pad_start(&part(0).unwrap_or_default(), 4);
    if let Some(m) = part(1).filter(|m| !m.is_empty()) {
        out.push('-');
        out.push_str(&pad_start(&m, 2));
        if let Some(d) = part(2).filter(|d| !d.is_empty()) {
            out.push('-');
            out.push_str(&pad_start(&d, 2));
        }
    }
    out
}

/// `String.prototype.padStart(n, '0')`.
fn pad_start(s: &str, n: usize) -> String {
    let l = js::len(s);
    if l >= n {
        s.to_owned()
    } else {
        format!("{}{s}", "0".repeat(n - l))
    }
}

/// `parseInt(s)` for the digit strings this code passes (`None` is NaN).
fn parse_int(s: &str) -> Option<i64> {
    let t = js::trim_start(s);
    let (neg, rest) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let d: String = rest.chars().take_while(char::is_ascii_digit).collect();
    d.parse::<i64>().ok().map(|n| if neg { -n } else { n })
}

/// Days since 1970-01-01 of a proleptic Gregorian date (month 1-12).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// (year, month 1-12, day) of days since 1970-01-01.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// :1651-1677: `d = new Date()`; with a time, `setUTCFullYear(y, m, d)` and
/// `setUTCHours(h, mi, s)`; without, `setFullYear(y, m, d)` in local time
/// (keeping the current local time of day); then the UTC parts. Month and
/// day overflow roll over as JavaScript's `Date` does; a part that is not a
/// number makes the date invalid, printed as upstream's `pad` prints NaN.
///
/// "Now" is [`super::now_unix_secs`] and local time is UTC shifted by
/// `DateOptions::utc_offset_minutes`: without a time of day, the result
/// depends on the clock (a local time of day before the UTC offset moves
/// the UTC date back a day), as upstream's does.
fn access_date_string(
    ctx: &ImportContext,
    year: &str,
    month0: &str,
    day: &str,
    time: Option<&(String, String, String)>,
) -> String {
    let num = |s: &str| -> Option<i64> { s.parse::<i64>().ok() };
    let pad = |n: Option<i64>, width: usize| -> String {
        let s = format!("000{}", n.map_or("NaN".to_owned(), |v| v.to_string()));
        let c: Vec<char> = s.chars().collect();
        c[c.len().saturating_sub(width)..].iter().collect()
    };
    let (Some(y), Some(m), Some(d)) = (num(year), num(month0), num(day)) else {
        return format!("{}-{}-{}", pad(None, 4), pad(None, 2), pad(None, 2))
            + if time.is_some() { " aN:aN:aN" } else { "" };
    };
    // Normalise month overflow the way Date.UTC does.
    let (y, m) = (y + m.div_euclid(12), m.rem_euclid(12));
    let days = days_from_civil(y, m + 1, 1) + (d - 1);
    let offset = i64::from(ctx.options.env.dates.utc_offset_minutes) * 60;
    let secs = match time {
        Some((h, mi, s)) => {
            let (h, mi, s) = (
                num(h).unwrap_or(0),
                num(mi).unwrap_or(0),
                num(s).unwrap_or(0),
            );
            days * 86_400 + h * 3600 + mi * 60 + s
        }
        None => {
            let now_local = super::now_unix_secs(ctx) + offset;
            let tod = now_local.rem_euclid(86_400);
            days * 86_400 + tod - offset
        }
    };
    let (yy, mm, dd) = civil_from_days(secs.div_euclid(86_400));
    let mut out = format!(
        "{}-{}-{}",
        pad(Some(yy), 4),
        pad(Some(mm), 2),
        pad(Some(dd), 2)
    );
    if time.is_some() {
        let r = secs.rem_euclid(86_400);
        out.push_str(&format!(
            " {}:{}:{}",
            pad(Some(r / 3600), 2),
            pad(Some((r % 3600) / 60), 2),
            pad(Some(r % 60), 2)
        ));
    }
    out
}

/// `completeItem(item)` (:1698-1796).
fn complete_item(
    ctx: &mut ImportContext,
    s: &Settings,
    mut item: TranslatorItem,
    scratch: Scratch,
) {
    if let Some(bpt) = scratch.backup_publication_title.filter(|v| !v.is_empty()) {
        if !has(&item, "publicationTitle") {
            item.set("publicationTitle", bpt);
        }
    }
    if let Some(bnp) = scratch.backup_num_pages.filter(|v| !v.is_empty()) {
        if !has(&item, "numPages") {
            item.set("numPages", bnp);
        }
    }
    if let Some(bep) = scratch.backup_end_page.filter(|v| !v.is_empty()) {
        if !has(&item, "pages") {
            item.set("pages", bep);
        } else {
            let pages = item.get_string("pages").unwrap_or_default();
            if !pages.contains('-') {
                item.set("pages", format!("{pages}-{bep}"));
            } else if !has(&item, "numPages") {
                item.set("numPages", bep);
            }
        }
    }
    if let Some((field, value)) = scratch.backup_date {
        if !has(&item, &field) {
            item.set(field, value);
        } else {
            static ZEROS: OnceLock<Regex> = OnceLock::new();
            let cur = item.get_string(&field).unwrap_or_default();
            let new = re(&ZEROS, || r"(?-u:\b)0000(?-u:\b)".to_owned())
                .replace(&cur, regex::NoExpand(&value))
                .into_owned();
            item.set(field, new);
        }
    }
    if let Some((field, value)) = scratch.backup_access_date {
        if !has(&item, &field) {
            item.set(field, value);
        }
    }
    if has(&item, "DOI") {
        if let Some(d) = clean_doi(&item.get_string("DOI").unwrap_or_default()) {
            item.set("DOI", d);
        }
    }
    if has(&item, "journalAbbreviation") && !has(&item, "publicationTitle") {
        let ja = item.get("journalAbbreviation").cloned().unwrap();
        item.set("publicationTitle", ja);
    }
    if has(&item, "shortTitle") && !has(&item, "title") {
        let st = item.get("shortTitle").cloned().unwrap();
        item.set("title", st);
    }
    if item.tags.len() == 1 {
        static SPLIT: OnceLock<Regex> = OnceLock::new();
        let split = re(&SPLIT, || format!("{ws}*(?:,{ws}*)+", ws = js::WS));
        let one = item.tags[0].tag.clone();
        let mut parts: Vec<String> = split.split(&one).map(str::to_owned).collect();
        if parts.first().is_some_and(String::is_empty) {
            parts.remove(0);
        }
        if parts.last().is_some_and(String::is_empty) {
            parts.pop();
        }
        item.tags = parts.into_iter().map(TranslatorTag::new).collect();
    }
    if s.parent {
        item.remove("accessDate");
    }
    if !s.parent {
        let mut note = String::new();
        for f in scratch
            .unsupported_fields
            .iter()
            .chain(&scratch.unknown_fields)
        {
            note.push_str(f);
            note.push_str("<br/>");
        }
        if !note.is_empty() {
            note = format!("The following values have no corresponding Zotero field:<br/>{note}");
            let mut n = TranslatorNote::new(js::trim(&note).to_owned());
            n.props.set("tags", serde_json::json!(["_RIS import"]));
            item.notes.push(n);
        }
    }
    ctx.item_done(item);
}
