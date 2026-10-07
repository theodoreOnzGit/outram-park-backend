// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "CSL JSON.js" (translatorID
//   bc03b4fe-436d-4a1f-ba59-de4d2d7a63f7, lastUpdated 2022-09-20 13:32:25):
//   `parseInput` :40-55, `detectImport` :57-85, `doImport`/`startImport`/
//   `importNext` :87-152, `doExport` :154-168.
// Copyright (c) 2022 Simon Kornblith and Sebastian Karcher.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The CSL JSON translator: import and export.

use crate::zotero::framework::csl::{item_from_csl_json, item_to_csl_json};
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::{ExportContext, ImportContext, TranslateError, TranslatorItem};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "bc03b4fe-436d-4a1f-ba59-de4d2d7a63f7",
    label: "CSL JSON",
    creator: "Simon Kornblith",
    target: "json",
    min_version: "4.0.27",
    priority: 100,
    translator_type: translator_type::IMPORT | translator_type::EXPORT,
    config_options: &[("async", HeaderValue::Bool(true))],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2022-09-20 13:32:25",
};

/// The CSL types `detectImport` accepts (:59-68).
const CSL_TYPES: [&str; 44] = [
    "article",
    "article-journal",
    "article-magazine",
    "article-newspaper",
    "bill",
    "book",
    "broadcast",
    "chapter",
    "classic",
    "collection",
    "dataset",
    "document",
    "entry",
    "entry-dictionary",
    "entry-encyclopedia",
    "event",
    "figure",
    "graphic",
    "hearing",
    "interview",
    "legal_case",
    "legislation",
    "manuscript",
    "map",
    "motion_picture",
    "musical_score",
    "pamphlet",
    "paper-conference",
    "patent",
    "performance",
    "personal_communication",
    "periodical",
    "post",
    "post-weblog",
    "regulation",
    "report",
    "review",
    "review-book",
    "song",
    "speech",
    "standard",
    "thesis",
    "treaty",
    "webpage",
];

/// `parseInput` (:40-55): read everything, `JSON.parse` it; `None` (upstream
/// `false`) when it does not parse.
fn parse_input(ctx: &mut ImportContext) -> Option<Value> {
    let mut json = String::new();
    while let Some(s) = ctx.read_chars(1_048_576) {
        json.push_str(&s);
    }
    serde_json::from_str(&json).ok()
}

/// `detectImport` (:57-85).
pub fn detect_import(ctx: &mut ImportContext) -> bool {
    let Some(parsed) = parse_input(ctx) else {
        return false;
    };
    // `if (!parsedData) return false; if (typeof parsedData !== "object") return false;`
    let items: Vec<Value> = match parsed {
        Value::Array(a) => a,
        v @ Value::Object(_) => vec![v],
        _ => return false,
    };
    items.iter().all(|item| {
        item.as_object().is_some_and(|o| {
            o.get("type")
                .filter(|t| crate::zotero::framework::js::truthy(Some(t)))
                .is_some_and(|t| {
                    CSL_TYPES.contains(&crate::zotero::framework::js::to_js_string(t).as_str())
                })
        })
    })
}

/// `doImport` (:87-152).
pub fn do_import(ctx: &mut ImportContext) -> Result<(), TranslateError> {
    let Some(parsed) = parse_input(ctx) else {
        // `if (!parsedData) resolve();` and then, upstream, the code carries
        // on with `[false]`, whose element is falsy, so nothing is imported.
        return Ok(());
    };
    let data: Vec<Value> = match parsed {
        Value::Array(a) => a,
        other => vec![other],
    };
    // `while (d = data.shift())`: stops at the first falsy element.
    for d in data {
        if !crate::zotero::framework::js::truthy(Some(&d)) {
            break;
        }
        let mut csl = match d {
            Value::Object(o) => o,
            // A non-object element: itemFromCSLJSON reads `.type` of a
            // primitive (undefined) and throws.
            _ => {
                return Err(TranslateError::Translator(
                    "No 'type' provided in CSL-JSON".into(),
                ))
            }
        };
        let mut item = TranslatorItem::new(String::new());
        // :133-135: default to 'article'.
        if !crate::zotero::framework::js::truthy(csl.get("type")) {
            csl.insert("type".into(), "article".into());
        }
        item_from_csl_json(&mut item, &csl, &ctx.options.env.dates)
            .map_err(TranslateError::Translator)?;
        ctx.item_done(item);
    }
    Ok(())
}

/// `doExport` (:154-168).
pub fn do_export(ctx: &mut ExportContext) -> Result<(), TranslateError> {
    static CITATION_KEY: OnceLock<Regex> = OnceLock::new();
    let re = CITATION_KEY.get_or_init(|| {
        Regex::new(&format!(
            r"(?i)(?:^|\n)citation key{ws}*:{ws}*({nws}+)(?:\n|$)",
            ws = crate::zotero::framework::js::WS,
            nws = crate::zotero::framework::js::NOT_WS
        ))
        .unwrap()
    });
    let mut data: Vec<Value> = Vec::new();
    let mut orders: Vec<Vec<String>> = Vec::new();
    while let Some(mut item) = ctx.next_item() {
        if let Some(extra) = item
            .get_str("extra")
            .filter(|e| !e.is_empty())
            .map(str::to_owned)
        {
            // `extra.replace(re, fn)` replaces the first match only, setting
            // `item.citationKey` and leaving "\n"; then `.trim()`.
            let mut key = None;
            let replaced = re
                .replacen(&extra, 1, |c: &regex::Captures| {
                    key = Some(c[1].to_owned());
                    "\n".to_owned()
                })
                .into_owned();
            if let Some(k) = key {
                item.set("citationKey", k);
            }
            item.set(
                "extra",
                crate::zotero::framework::js::trim(&replaced).to_owned(),
            );
        }
        let mut csl =
            item_to_csl_json(&item, &ctx.options.env.dates).map_err(TranslateError::Translator)?;
        if let Some(k) = item
            .get("citationKey")
            .filter(|k| crate::zotero::framework::js::truthy(Some(k)))
        {
            csl.insert("id".into(), k.clone());
        }
        let order = csl_key_order(&item, &csl);
        data.push(Value::Object(csl));
        orders.push(order);
    }
    let mut out = String::from("[");
    for (i, (v, order)) in data.iter().zip(&orders).enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("\n\t");
        write_json(&mut out, v, 1, Some(order));
    }
    out.push_str(if data.is_empty() { "]" } else { "\n]" });
    ctx.write(&out);
    Ok(())
}

/// The order in which upstream's `itemToCSLJSON` inserts a CSL item's keys
/// (utilities_item.js:161-406), which `JSON.stringify` preserves: `id`,
/// `type`; text variables in schema order; name variables in the order of
/// the first creator using each; date variables from fields in schema
/// order; date variables set only from Extra, in Extra's line order; a
/// note's `title`. (kovan-common returns a `serde_json::Map`, which sorts
/// keys, so the order is rebuilt here.)
fn csl_key_order(item: &TranslatorItem, csl: &serde_json::Map<String, Value>) -> Vec<String> {
    use kovan_common::zotero::schema_generated::{CSL_DATE_FIELDS, CSL_TEXT_FIELDS};
    let mut order: Vec<String> = vec!["id".into(), "type".into()];
    let push = |k: &str, order: &mut Vec<String>| {
        if csl.contains_key(k) && !order.iter().any(|o| o == k) {
            order.push(k.to_owned());
        }
    };
    for (var, _) in CSL_TEXT_FIELDS.iter() {
        push(var, &mut order);
    }
    let z = crate::zotero::framework::csl::export_item_to_zotero_item(item).ok();
    if let Some(z) = &z {
        let primary = kovan_common::zotero::schema::primary_creator_type(z.item_type);
        for c in &z.creators {
            let var = c
                .creator_type
                .csl_name()
                .or((Some(c.creator_type) == primary).then_some("author"));
            if let Some(var) = var {
                push(var, &mut order);
            }
        }
        for (var, field) in CSL_DATE_FIELDS.iter() {
            let direct = z.field(*field).filter(|v| !v.is_empty()).is_some();
            let via = crate::zotero::framework::api_json::type_field_for_base(z.item_type, *field)
                .and_then(|f| z.field(f))
                .filter(|v| !v.is_empty())
                .is_some();
            if direct || via {
                push(var, &mut order);
            }
        }
        if let Some(extra) = z.field(kovan_common::zotero::Field::Extra) {
            for line in kovan_common::zotero::csl::extra_to_csl(extra).split('\n') {
                if let Some((var, _)) = line.split_once(':') {
                    push(var.trim(), &mut order);
                }
            }
        }
    }
    let rest: Vec<String> = csl.keys().filter(|k| !order.contains(k)).cloned().collect();
    order.extend(rest);
    order
}

/// `JSON.stringify(value, null, "\t")`: tab indentation, `[]`/`{}` for
/// empty containers; keys in serde's order, except the top object's when
/// `order` is given.
pub fn json_stringify_tab(v: &Value) -> String {
    let mut out = String::new();
    write_json(&mut out, v, 0, None);
    out
}

fn write_json(out: &mut String, v: &Value, depth: usize, order: Option<&Vec<String>>) {
    match v {
        Value::Array(a) if !a.is_empty() => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push('\n');
                out.push_str(&"\t".repeat(depth + 1));
                write_json(out, x, depth + 1, None);
            }
            out.push('\n');
            out.push_str(&"\t".repeat(depth));
            out.push(']');
        }
        Value::Object(o) if !o.is_empty() => {
            out.push('{');
            let keys: Vec<&String> = match order {
                Some(ord) => ord.iter().filter(|k| o.contains_key(*k)).collect(),
                None => {
                    // Name and date objects, in upstream's insertion order:
                    // a name gets family and given, then parseParticles
                    // (utilities_item.js:775-794) adds non-dropping-particle,
                    // then parseSuffix's comma-suffix and suffix (or, for
                    // "et al", dropping-particle and comma-dropping-particle),
                    // then dropping-particle; a date gets date-parts, then
                    // season or circa.
                    let et_al = o.contains_key("comma-dropping-particle");
                    let nested: [&str; 11] = [
                        "family",
                        "given",
                        "non-dropping-particle",
                        "comma-suffix",
                        "suffix",
                        if et_al {
                            "dropping-particle"
                        } else {
                            "comma-dropping-particle"
                        },
                        if et_al {
                            "comma-dropping-particle"
                        } else {
                            "dropping-particle"
                        },
                        "date-parts",
                        "season",
                        "circa",
                        "literal",
                    ];
                    let mut ks: Vec<&String> = o.keys().collect();
                    ks.sort_by_key(|k| nested.iter().position(|n| n == k).unwrap_or(nested.len()));
                    ks
                }
            };
            for (i, k) in keys.into_iter().enumerate() {
                let x = &o[k];
                if i > 0 {
                    out.push(',');
                }
                out.push('\n');
                out.push_str(&"\t".repeat(depth + 1));
                out.push_str(&serde_json::to_string(k).unwrap());
                out.push_str(": ");
                write_json(out, x, depth + 1, None);
            }
            out.push('\n');
            out.push_str(&"\t".repeat(depth));
            out.push('}');
        }
        other => out.push_str(&serde_json::to_string(other).unwrap()),
    }
}
