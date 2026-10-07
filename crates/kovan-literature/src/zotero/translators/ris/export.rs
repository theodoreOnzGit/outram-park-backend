// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): RIS.js `exportOrder` :1920-2010, `newLineChar`
//   :2012, `addTag` :2017-2028, `doExport` :2030-2179.
// Copyright (C) 2006-2023 Simon Kornblith, Aurimas Vinckevicus, Abe Jellinek.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! RIS export.

use super::mapper::TagMapper;
use super::tables::{EXPORT_ORDER_BILL, EXPORT_ORDER_DEFAULT, EXPORT_TYPE_MAP, FIELD_MAP};
use crate::zotero::framework::{js, ExportContext, JsObject, TranslateError, TranslatorItem};
use kovan_common::zotero::date::str_to_date;
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

/// `DEFAULT_EXPORT_TYPE` (:80).
const DEFAULT_EXPORT_TYPE: &str = "GEN";

/// `newLineChar` (:2012).
const NEW_LINE: &str = "\r\n";

/// `addTag(tag, value)` (:2017-2028). `None` entries are `undefined`, which
/// stop the tag (`return`, not `continue`); other values are written as
/// `(value + '').trim()` unless empty.
fn add_tag(ctx: &mut ExportContext, tag: &str, value: &[Option<Value>]) {
    for v in value {
        let Some(v) = v else { return };
        let s = js::to_js_string(v);
        let s = js::trim(&s);
        if s.is_empty() {
            continue;
        }
        ctx.write(&format!("{tag}  - {s}{NEW_LINE}"));
    }
}

/// `item[field]` as JavaScript would read it for `strToDate`: a string or a
/// number is parsed (trimInternal'ed upstream); anything else gives an empty
/// date.
fn date_input(item: &TranslatorItem, field: &str) -> String {
    match item.get(field) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => js::number_to_string(n),
        _ => String::new(),
    }
}

/// `('000' + s).substr(-4)` and friends: the last `n` characters.
fn last_chars(s: &str, n: usize) -> String {
    let c: Vec<char> = s.chars().collect();
    c[c.len().saturating_sub(n)..].iter().collect()
}

/// `doExport` (:2030-2179).
pub fn do_export(ctx: &mut ExportContext) -> Result<(), TranslateError> {
    let fields = TagMapper::new(vec![FIELD_MAP], None, false);
    while let Some(item) = ctx.next_item() {
        if item.item_type == "note" || item.item_type == "attachment" {
            continue;
        }
        let ty = EXPORT_TYPE_MAP
            .iter()
            .find(|(k, _)| *k == item.item_type)
            .map_or(DEFAULT_EXPORT_TYPE, |(_, v)| v);
        add_tag(ctx, "TY", &[Some(Value::String(ty.to_owned()))]);

        // Attachments by MIME type (:2054-2071).
        let (mut pdf, mut html, mut other): (Vec<&JsObject>, Vec<&JsObject>, Vec<&JsObject>) =
            (Vec::new(), Vec::new(), Vec::new());
        for a in &item.attachments {
            match a.get_str("mimeType") {
                Some("application/pdf") => pdf.push(a),
                Some("text/html") => html.push(a),
                _ => other.push(a),
            }
        }

        let order = if item.item_type == "bill" {
            EXPORT_ORDER_BILL
        } else {
            EXPORT_ORDER_DEFAULT
        };
        for &tag0 in order {
            let mut tag = tag0;
            let Some(field) = fields.get_field(&item.item_type, tag) else {
                continue;
            };
            let field: Vec<&str> = field.split('/').collect();
            // `undefined` is an empty list here; `[undefined]` stops addTag.
            let mut value: Vec<Option<Value>> = vec![None];
            match field[0] {
                "creators" => {
                    let mut names = Vec::new();
                    for c in &item.creators {
                        if c.creator_type.as_deref() == field.get(1).copied() {
                            let mut name = vec![c.last_name.clone().unwrap_or_default()];
                            if let Some(f) = c.first_name.as_deref().filter(|f| !f.is_empty()) {
                                name.push(f.to_owned());
                            }
                            names.push(Some(Value::String(name.join(", "))));
                        }
                    }
                    if !names.is_empty() {
                        value = names;
                    }
                }
                "notes" => {
                    if ctx.options.option_truthy("exportNotes") {
                        static NL: OnceLock<Regex> = OnceLock::new();
                        let nl = NL.get_or_init(|| Regex::new(r"\r\n?|\n").unwrap());
                        value = item
                            .notes
                            .iter()
                            .map(|n| {
                                Some(Value::String(nl.replace_all(&n.note, "\r\n").into_owned()))
                            })
                            .collect();
                    }
                }
                "tags" => {
                    value = item
                        .tags
                        .iter()
                        .map(|t| Some(Value::String(t.tag.clone())))
                        .collect();
                }
                "attachments" => {
                    let att = match field.get(1).copied() {
                        Some("PDF") => &pdf,
                        Some("HTML") => &html,
                        _ => &other,
                    };
                    // No `saveFile` (file data is not exported): the URL.
                    value = att.iter().map(|a| a.get("url").cloned()).collect();
                }
                "pages" => {
                    if tag == "SP" && item.truthy("pages") {
                        static RANGE: OnceLock<Regex> = OnceLock::new();
                        let range = RANGE.get_or_init(|| {
                            Regex::new(&format!(
                                r"(.+?)[\x{{2D}}\x{{AD}}\x{{2010}}-\x{{2015}}\x{{2212}}\x{{2E3A}}\x{{2E3B}}{}]+(.+)",
                                &js::WS[1..js::WS.len() - 1]
                            ))
                            .unwrap()
                        });
                        let pages = item.get_string("pages").unwrap_or_default();
                        let trimmed = js::trim(&pages).to_owned();
                        match range.captures(&trimmed) {
                            Some(m) => {
                                add_tag(ctx, tag, &[Some(Value::String(m[1].to_owned()))]);
                                tag = "EP";
                                value = vec![Some(Value::String(m[2].to_owned()))];
                            }
                            None => value = vec![item.get("pages").cloned()],
                        }
                    }
                }
                f => value = vec![item.get(f).cloned()],
            }

            // :2146-2172
            match tag {
                "PY" => {
                    let d = str_to_date(&date_input(&item, field[0]), &ctx.options.env.dates);
                    value = match d.year.filter(|y| !y.is_empty()) {
                        Some(y) => vec![Some(Value::String(last_chars(&format!("000{y}"), 4)))],
                        None => vec![item.get(field[0]).cloned()],
                    };
                }
                "Y2" | "DA" => {
                    let d = str_to_date(&date_input(&item, field[0]), &ctx.options.env.dates);
                    value = match d.year.filter(|y| !y.is_empty()) {
                        Some(y) => {
                            let year = last_chars(&format!("000{y}"), 4);
                            let month = d
                                .month
                                .map_or(String::new(), |m| last_chars(&format!("0{}", m + 1), 2));
                            let day = d
                                .day
                                .filter(|d| *d != 0)
                                .map_or(String::new(), |d| last_chars(&format!("0{d}"), 2));
                            let part = d.part.unwrap_or_default();
                            vec![Some(Value::String(format!("{year}/{month}/{day}/{part}")))]
                        }
                        None => vec![item.get(field[0]).cloned()],
                    };
                }
                _ => {}
            }
            add_tag(ctx, tag, &value);
        }
        ctx.write(&format!("ER  - {NEW_LINE}{NEW_LINE}"));
    }
    Ok(())
}
