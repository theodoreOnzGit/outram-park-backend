// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): CFF.js (translatorID
//   e782b521-99ed-47c7-b021-62351a0a4f91, lastUpdated 2023-05-04 13:21:10):
//   header :1-11, `writeKeywords` :35-42, `writeDOI` :44-48,
//   `writeAuthors` :50-66, `doExport` :68-108.
// Copyright (c) 2023 Sebastian Karcher.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The CFF translator: export of datasets and software as a
//! `CITATION.cff` file (other item types are skipped).
//!
//! The helpers shared with CFF References ([`CffValue`], [`get_value`]) are
//! here.
//!
//! Not ported: nothing.
//!
//! **Maturity: AI draft (1).** Verified code-to-code against upstream run
//! in-process (`tests/zotero_translators.rs`, `cff_export_matches_upstream`).

use crate::zotero::framework::options::{translator_type, TranslatorMetadata};
use crate::zotero::framework::utilities::{clean_doi, field_is_valid_for_type, str_to_iso};
use crate::zotero::framework::{js, ExportContext, TranslateError, TranslatorItem};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "e782b521-99ed-47c7-b021-62351a0a4f91",
    label: "CFF",
    creator: "Sebastian Karcher",
    target: "cff",
    min_version: "5.0",
    priority: 100,
    translator_type: translator_type::EXPORT,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2023-05-04 13:21:10",
};

/// A value of the translators' `cff` object: `None` is falsy (`undefined`,
/// `false`, `null`, `""`, `0`), anything else is what `"" + value` writes.
pub(crate) type CffValue = Option<String>;

/// `item[key]` as a [`CffValue`].
pub(crate) fn get_value(item: &TranslatorItem, key: &str) -> CffValue {
    let v = item.get(key);
    if js::truthy(v) {
        v.map(js::to_js_string)
    } else {
        None
    }
}

/// `"" + item[key]` (`undefined` is "undefined").
pub(crate) fn concat_value(item: &TranslatorItem, key: &str) -> String {
    item.get(key)
        .map_or_else(|| "undefined".to_owned(), js::to_js_string)
}

/// `item.extra` as `RegExp#test` reads it (`undefined` is "undefined").
pub(crate) fn extra_for_test(item: &TranslatorItem) -> String {
    concat_value(item, "extra")
}

/// `ZU.cleanDOI(item.extra)` when the type has no DOI field and Extra
/// matches `doi_re` (CFF.js :92-94, CFF References.js :124-126). Returns
/// the new `item.DOI`, or the item's own when the branch is not taken.
pub(crate) fn doi_from_extra(
    item: &TranslatorItem,
    doi_re: &Regex,
) -> Result<CffValue, TranslateError> {
    if !field_is_valid_for_type("DOI", &item.item_type) && doi_re.is_match(&extra_for_test(item)) {
        // `cleanDOI` throws unless its argument is a string; Extra matched,
        // so it is defined.
        return Ok(match item.get("extra") {
            Some(Value::String(s)) => clean_doi(s),
            _ => {
                return Err(TranslateError::Translator(
                    "cleanDOI: argument must be a string".into(),
                ))
            }
        });
    }
    Ok(get_value(item, "DOI"))
}

/// `writeKeywords(tags)` (:35-42). (Its `replace(/\\n$/, "")` removes a
/// literal backslash and `n` at the end, which a string ending in a newline
/// never has.)
fn write_keywords(item: &TranslatorItem) -> CffValue {
    if item.tags.is_empty() {
        return None;
    }
    let mut k = String::from("\n");
    for t in &item.tags {
        k.push_str(&format!("  - {}\n", t.tag));
    }
    Some(k)
}

/// `writeDOI(itemDOI)` (:44-48).
fn write_doi(doi: &CffValue) -> CffValue {
    doi.as_ref()
        .map(|d| format!("\n  - type: doi\n    value: {d}\n"))
}

/// `writeAuthors(itemCreators)` (:50-66).
fn write_authors(item: &TranslatorItem) -> CffValue {
    let authors: Vec<_> = item
        .creators
        .iter()
        .filter(|c| matches!(c.creator_type.as_deref(), Some("author" | "programmer")))
        .collect();
    if authors.is_empty() {
        return None;
    }
    let mut s = String::from("\n");
    for a in authors {
        s.push_str(&format!(
            "  - family-names: {}\n",
            a.last_name.as_deref().unwrap_or("undefined")
        ));
        if let Some(f) = a.first_name.as_deref().filter(|f| !f.is_empty()) {
            s.push_str(&format!("    given-names: {f}\n"));
        }
    }
    Some(s)
}

/// `doExport` (:68-108).
pub fn do_export(ctx: &mut ExportContext) -> Result<(), TranslateError> {
    static DOI: OnceLock<Regex> = OnceLock::new();
    let doi_re = DOI.get_or_init(|| Regex::new("(?i)^doi:").unwrap());
    ctx.write("# This CITATION.cff file was generated with Zotero.\n");
    while let Some(item) = ctx.next_item() {
        if item.item_type != "dataset" && item.item_type != "computerProgram" {
            continue;
        }
        let ty = if item.item_type == "dataset" {
            "dataset"
        } else {
            "software"
        };
        let mut cff: Vec<(&str, CffValue)> = vec![
            (
                "title",
                Some(format!(" >-\n  {}", concat_value(&item, "title"))),
            ),
            ("abstract", get_value(&item, "abstractNote")),
            ("type", Some(ty.to_owned())),
            ("license", get_value(&item, "rights")),
            ("version", get_value(&item, "versionNumber")),
            ("url", get_value(&item, "url")),
            ("keywords", write_keywords(&item)),
            ("authors", write_authors(&item)),
        ];
        if js::truthy(item.get("date")) {
            let date = concat_value(&item, "date");
            cff.push(("date-released", str_to_iso(&date, &ctx.options.env.dates)));
        }
        let doi = doi_from_extra(&item, doi_re)?;
        cff.push(("identifiers", write_doi(&doi)));

        ctx.write(&format!(
            "\ncff-version: 1.2.0\nmessage: >-\n  If you use this {ty}, please cite it using the metadata from this file.\n"
        ));
        for (field, value) in cff {
            let Some(value) = value.filter(|v| !v.is_empty()) else {
                continue;
            };
            if field == "authors" || field == "keywords" {
                ctx.write(&format!("{field}: {value}"));
            } else {
                ctx.write(&format!("{field}: {value}\n"));
            }
        }
    }
    Ok(())
}
