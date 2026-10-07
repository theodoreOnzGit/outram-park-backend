// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): CSV.js (translatorID
//   25f4c5e2-d790-4daa-a667-797619c7e2f2, lastUpdated 2022-06-28 19:45:59):
//   header :1-15, settings :45-50, `exportedFields` :53-145,
//   `creatorBaseTypes` :148-160, `doExport` :162-178, `escapeValue`
//   :180-187, `writeColumnHeaders` :189-214, `getValue` :216-291.
// Copyright (c) 2014 Philipp Zumstein, Aurimas Vinckevicius.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The CSV translator: export (one row per regular item, a fixed column
//! set, a byte-order mark first).
//!
//! The item is the legacy export format (minVersion 4.0.26 < 4.0.27), which
//! the framework prepares. Where upstream throws (an item with no `uri`, or
//! one not ending in `[A-Z0-9]+`: `item.uri.match(...)[1]`), the port
//! returns [`TranslateError::Translator`] and nothing is written, as the
//! endpoint answers 500.
//!
//! **Maturity: AI draft (1).** Verified code-to-code against the Zotero
//! translation-server (`tests/zotero_translators.rs`, `csv_*`).

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::utilities::str_to_iso;
use crate::zotero::framework::{js, ExportContext, TranslateError, TranslatorItem};
use kovan_common::zotero::date::str_to_date;
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "25f4c5e2-d790-4daa-a667-797619c7e2f2",
    label: "CSV",
    creator: "Philipp Zumstein and Aurimas Vinckevicius",
    target: "csv",
    min_version: "4.0.26",
    priority: 100,
    translator_type: translator_type::EXPORT,
    config_options: &[],
    display_options: &[
        ("exportCharset", HeaderValue::Str("UTF-8xBOM")),
        ("exportNotes", HeaderValue::Bool(false)),
    ],
    hidden_prefs: &[],
    last_updated: "2022-06-28 19:45:59",
};

const RECORD_DELIMITER: &str = "\n";
const FIELD_DELIMITER: &str = ",";
const WRAPPER: &str = "\"";
const VALUE_SEPARATOR: &str = "; ";

/// `exportedFields` (:53-145), in export order.
const EXPORTED_FIELDS: [&str; 87] = [
    "key",
    "itemType",
    "publicationYear",
    "creators/author",
    "title",
    "publicationTitle",
    "ISBN",
    "ISSN",
    "DOI",
    "url",
    "abstractNote",
    "date",
    "dateAdded",
    "dateModified",
    "accessDate",
    "pages",
    "numPages",
    "issue",
    "volume",
    "numberOfVolumes",
    "journalAbbreviation",
    "shortTitle",
    "series",
    "seriesNumber",
    "seriesText",
    "seriesTitle",
    "publisher",
    "place",
    "language",
    "rights",
    "type",
    "archive",
    "archiveLocation",
    "libraryCatalog",
    "callNumber",
    "extra",
    "notes",
    "attachments/path",
    "attachments/url",
    "tags/own",
    "tags/automatic",
    "creators/editor",
    "creators/seriesEditor",
    "creators/translator",
    "creators/contributor",
    "creators/attorneyAgent",
    "creators/bookAuthor",
    "creators/castMember",
    "creators/commenter",
    "creators/composer",
    "creators/cosponsor",
    "creators/counsel",
    "creators/interviewer",
    "creators/producer",
    "creators/recipient",
    "creators/reviewedAuthor",
    "creators/scriptwriter",
    "creators/wordsBy",
    "creators/guest",
    "number",
    "edition",
    "runningTime",
    "scale",
    "medium",
    "artworkSize",
    "filingDate",
    "applicationNumber",
    "assignee",
    "issuingAuthority",
    "country",
    "meetingName",
    "conferenceName",
    "court",
    "references",
    "reporter",
    "legalStatus",
    "priorityNumbers",
    "programmingLanguage",
    "version",
    "system",
    "code",
    "codeNumber",
    "section",
    "session",
    "committee",
    "history",
    "legislativeBody",
];

/// `creatorBaseTypes` (:148-160).
fn creator_base_type(t: &str) -> Option<&'static str> {
    match t {
        "interviewee" | "director" | "artist" | "sponsor" | "contributor" | "inventor"
        | "cartographer" | "performer" | "presenter" | "podcaster" | "programmer" => Some("author"),
        _ => None,
    }
}

/// `escapeValue(str)` (:181-187): runs of CR/LF become a space, `"` is
/// doubled.
fn escape_value(s: &str) -> String {
    static NL: OnceLock<Regex> = OnceLock::new();
    let nl = NL.get_or_init(|| Regex::new("[\r\n]+").unwrap());
    nl.replace_all(s, " ").replace('"', "\"\"")
}

/// `writeColumnHeaders()` (:189-214).
fn column_headers() -> String {
    static CAMEL: OnceLock<Regex> = OnceLock::new();
    let camel = CAMEL.get_or_init(|| Regex::new("([a-z])([A-Z])").unwrap());
    let mut line = String::new();
    for (i, field) in EXPORTED_FIELDS.iter().enumerate() {
        if i > 0 {
            line.push_str(FIELD_DELIMITER);
        }
        line.push_str(WRAPPER);
        let parts: Vec<&str> = field.split('/').collect();
        let label = match parts[0] {
            "creators" => parts[1],
            "tags" => {
                if parts[1] == "own" {
                    "Manual Tags"
                } else {
                    "Automatic Tags"
                }
            }
            "attachments" => {
                if parts[1] == "url" {
                    "Link Attachments"
                } else {
                    "File Attachments"
                }
            }
            other => other,
        };
        // `label[0].toUpperCase() + label.substr(1)`
        let mut chars = label.chars();
        let first = chars.next().map(|c| c.to_uppercase().to_string());
        let label = first.unwrap_or_default() + chars.as_str();
        let label = camel.replace_all(&label, "$1 $2");
        line.push_str(&escape_value(&label));
        line.push_str(WRAPPER);
    }
    line
}

/// A JavaScript value in `[...].join("; ")`: undefined and null are "".
fn join_part(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(v) => js::to_js_string(v),
    }
}

/// `item[field]`, where `itemType` is a typed member of the port's item.
fn prop(item: &TranslatorItem, field: &str) -> Option<Value> {
    if field == "itemType" {
        return Some(Value::String(item.item_type.clone()));
    }
    item.get(field).cloned()
}

/// `getValue(item, field)` (:216-291).
fn get_value(
    ctx: &ExportContext,
    item: &TranslatorItem,
    field: &str,
    export_notes: bool,
) -> Result<String, TranslateError> {
    let split: Vec<&str> = field.split('/').collect();
    let mut value = WRAPPER.to_owned();
    let dates = &ctx.options.env.dates;
    match split[0] {
        "key" => {
            // `item.uri.match(/([A-Z0-9]+)$/)[1]`: throws when `uri` is not a
            // string or does not match.
            static KEY: OnceLock<Regex> = OnceLock::new();
            let re = KEY.get_or_init(|| Regex::new("([A-Z0-9]+)$").unwrap());
            let uri = match item.get("uri") {
                Some(Value::String(s)) => s,
                _ => {
                    return Err(TranslateError::Translator(
                        "item.uri is undefined".to_owned(),
                    ))
                }
            };
            let m = re.captures(uri).ok_or_else(|| {
                TranslateError::Translator("item.uri.match(...) is null".to_owned())
            })?;
            value.push_str(&m[1]);
        }
        "publicationYear" => {
            if item.truthy("date") {
                let d = str_to_date(&item.get_string("date").unwrap_or_default(), dates);
                if let Some(y) = d.year.filter(|y| !y.is_empty()) {
                    value.push_str(&escape_value(&y));
                }
            }
        }
        "creators" => {
            let mut creators = Vec::new();
            for c in &item.creators {
                let base = c.creator_type.as_deref().and_then(creator_base_type);
                if c.creator_type.as_deref() != Some(split[1]) && base != Some(split[1]) {
                    continue;
                }
                let last = c.last_name.as_deref().unwrap_or("undefined");
                let first = match c.first_name.as_deref() {
                    Some(f) if !f.is_empty() => format!(", {f}"),
                    _ => String::new(),
                };
                creators.push(format!("{last}{first}"));
            }
            value.push_str(&escape_value(&creators.join(VALUE_SEPARATOR)));
        }
        "tags" => {
            let tag_type = if split[1] == "automatic" { 1 } else { 0 };
            let tags: Vec<&str> = item
                .tags
                .iter()
                .filter(|t| t.tag_type.unwrap_or(0) == tag_type)
                .map(|t| t.tag.as_str())
                .collect();
            value.push_str(&escape_value(&tags.join(VALUE_SEPARATOR)));
        }
        "attachments" => {
            let mut paths = Vec::new();
            for a in &item.attachments {
                if split[1] == "path" {
                    paths.push(join_part(a.get("localPath")));
                } else if split[1] == "url" && !a.truthy("localPath") {
                    paths.push(join_part(a.get("url")));
                }
            }
            value.push_str(&escape_value(&paths.join(VALUE_SEPARATOR)));
        }
        "notes" => {
            if export_notes {
                let notes: Vec<&str> = item.notes.iter().map(|n| n.note.as_str()).collect();
                value.push_str(&escape_value(&notes.join(VALUE_SEPARATOR)));
            }
        }
        "date" => {
            if item.truthy("date") {
                let date = item.get_string("date").unwrap_or_default();
                match str_to_iso(&date, dates) {
                    Some(iso) => value.push_str(&iso),
                    None => value.push_str(&date),
                }
            }
        }
        _ => {
            let own = prop(item, field).filter(|v| js::truthy(Some(v)));
            let v = own.or_else(|| match item.get("uniqueFields") {
                Some(Value::Object(u)) => u.get(field).cloned().filter(|v| js::truthy(Some(v))),
                _ => None,
            });
            if let Some(v) = v {
                value.push_str(&escape_value(&js::to_js_string(&v)));
            }
        }
    }
    value.push_str(WRAPPER);
    Ok(value)
}

/// `doExport` (:163-178).
pub fn do_export(ctx: &mut ExportContext) -> Result<(), TranslateError> {
    let export_notes = ctx.options.option_truthy("exportNotes");
    ctx.write("\u{FEFF}");
    ctx.write(&column_headers());
    while let Some(item) = ctx.next_item() {
        if item.item_type == "note" || item.item_type == "attachment" {
            continue;
        }
        let mut line = String::new();
        for (i, field) in EXPORTED_FIELDS.iter().enumerate() {
            line.push_str(if i > 0 {
                FIELD_DELIMITER
            } else {
                RECORD_DELIMITER
            });
            line.push_str(&get_value(ctx, &item, field, export_notes)?);
        }
        ctx.write(&line);
    }
    Ok(())
}
