// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "CFF References.js" (translatorID
//   99A6641F-A8C2-4923-9BBB-0DA87F1E5187, lastUpdated 2024-05-17 20:02:13):
//   header :1-11, `referencesSpacing` :37-38, `writeArray` :40-48,
//   `writeDOI` :50-54, `writeCreators` :56-75, `doExport` :77-165.
// Copyright (c) 2024 Sebastian Karcher, Dave Bunten.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The CFF References translator: export of any items as the `references`
//! list of a `CITATION.cff` file.
//!
//! Ported as upstream behaves, including two upstream quirks the reference
//! output shows: `writeCreators` pushes every creator whatever role is asked
//! for (both branches of its `if` push), so `authors`, `editors`,
//! `recipients` and `translators` each list all creators; and `pmcid` is
//! the whole `match` array written with `Array#toString` ("pmcid: X,X").
//!
//! Not ported: nothing.
//!
//! **Maturity: AI draft (1).** Verified code-to-code against upstream run
//! in-process (`tests/zotero_translators.rs`,
//! `cff_references_export_matches_upstream`).

use super::cff::{concat_value, doi_from_extra, get_value, CffValue};
use crate::zotero::framework::options::{translator_type, TranslatorMetadata};
use crate::zotero::framework::utilities::{get_creators_for_type, str_to_iso};
use crate::zotero::framework::{js, ExportContext, TranslateError, TranslatorItem};
use regex::Regex;
use std::sync::OnceLock;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "99A6641F-A8C2-4923-9BBB-0DA87F1E5187",
    label: "CFF References",
    creator: "Sebastian Karcher, Dave Bunten",
    target: "cff",
    min_version: "5.0",
    priority: 100,
    translator_type: translator_type::EXPORT,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2024-05-17 20:02:13",
};

/// `referencesSpacing` (:38).
const SP: &str = "    ";

/// `writeArray(array)` (:40-48) over the array's elements (`None` falsy).
fn write_array(array: &[CffValue]) -> CffValue {
    if array.is_empty() {
        return None;
    }
    let mut out = String::from("\n");
    for e in array.iter().flatten() {
        if e.is_empty() {
            continue;
        }
        out.push_str(&format!("{SP}  - {e}\n"));
    }
    let out = out.strip_suffix('\n').unwrap_or(&out).to_owned();
    Some(out)
}

/// `writeDOI(itemDOI)` (:50-54).
fn write_doi(doi: &CffValue) -> CffValue {
    doi.as_ref()
        .map(|d| format!("\n{SP}  - type: doi\n{SP}    value: {d}"))
}

/// `writeCreators(itemCreators, creatorType)` (:56-75): every creator, in
/// order (module docs).
fn write_creators(item: &TranslatorItem) -> CffValue {
    if item.creators.is_empty() {
        return None;
    }
    let mut s = String::from("\n");
    for a in &item.creators {
        s.push_str(&format!(
            "{SP}  - family-names: {}\n",
            a.last_name.as_deref().unwrap_or("undefined")
        ));
        if let Some(f) = a.first_name.as_deref().filter(|f| !f.is_empty()) {
            s.push_str(&format!("{SP}    given-names: {f}\n"));
        }
    }
    Some(s.strip_suffix('\n').unwrap_or(&s).to_owned())
}

/// `doExport` (:77-165).
pub fn do_export(ctx: &mut ExportContext) -> Result<(), TranslateError> {
    static PMCID_TEST: OnceLock<Regex> = OnceLock::new();
    static PMCID: OnceLock<Regex> = OnceLock::new();
    static DOI: OnceLock<Regex> = OnceLock::new();
    let pmcid_test = PMCID_TEST.get_or_init(|| Regex::new("(?i)^pmcid:").unwrap());
    let pmcid_re = PMCID.get_or_init(|| {
        Regex::new(&format!(
            "pmcid:{ws}*({nws}+)",
            ws = js::WS,
            nws = js::NOT_WS
        ))
        .unwrap()
    });
    // `/^doi:/im`: with the m flag JavaScript's `^` also follows \r, U+2028
    // and U+2029, not only \n.
    let doi_re = DOI.get_or_init(|| Regex::new(r"(?i)(?:^|[\n\r\x{2028}\x{2029}])doi:").unwrap());

    ctx.write("# This CITATION.cff reference content was generated from Zotero.\n");
    ctx.write("references:\n");
    while let Some(item) = ctx.next_item() {
        let g = |k: &str| get_value(&item, k);
        let mut cff: Vec<(&str, CffValue)> = vec![
            (
                "title",
                Some(format!(">-\n{SP}  {}\n", concat_value(&item, "title"))),
            ),
            ("abstract", g("abstractNote")),
            ("type", Some(item.item_type.clone())),
            ("license", g("rights")),
            ("version", g("versionNumber")),
            ("collection_title", g("proceedingsTitle")),
            ("conference", g("conferenceName")),
            ("copyright", g("rights")),
            ("database", g("libraryCatalog")),
            ("date_accessed", g("accessDate")),
            ("edition", g("edition")),
            ("editors_series", g("series")),
            ("format", g("format")),
            ("institution", g("institution")),
            ("isbn", g("ISBN")),
            ("issn", g("ISSN")),
            ("issue", g("issue")),
            ("issue_date", g("issueDate")),
            ("journal", g("journalAbbreviation")),
            ("languages", write_array(&[g("language")])),
            ("location", g("archiveLocation")),
            ("medium", g("medium")),
            ("number", g("number")),
            ("number_volumes", g("numberOfVolumes")),
            ("pages", g("pages")),
        ];
        // pmcid (:117-120): the `match` array, or null.
        if let Some(extra) = g("extra") {
            if pmcid_test.is_match(&extra) {
                let m = pmcid_re
                    .captures(&extra)
                    .map(|c| format!("{},{}", &c[0], &c[1]));
                cff.push(("pmcid", m));
            }
        }
        cff.extend([
            ("publisher", g("publisher")),
            ("repository", g("repository")),
            ("section", g("section")),
            ("thesis_type", g("thesisType")),
            ("volume", g("volume")),
            ("url", g("url")),
        ]);
        // `tag.tag || tag`: an empty tag would be the object itself.
        let tags: Vec<CffValue> = item
            .tags
            .iter()
            .map(|t| {
                Some(if t.tag.is_empty() {
                    "[object Object]".to_owned()
                } else {
                    t.tag.clone()
                })
            })
            .collect();
        cff.push(("keywords", write_array(&tags)));
        if ["letter", "email", "instantMessage"].contains(&item.item_type.as_str()) {
            cff.push(("senders", write_creators(&item)));
        } else {
            // `ZU.getCreatorsForType(item.itemType)[0]` is evaluated and
            // ignored (module docs).
            let _ = get_creators_for_type(&item.item_type);
            cff.push(("authors", write_creators(&item)));
        }
        cff.push(("editors", write_creators(&item)));
        cff.push(("recipients", write_creators(&item)));
        cff.push(("translators", write_creators(&item)));

        if js::truthy(item.get("date")) {
            let date = concat_value(&item, "date");
            let iso = str_to_iso(&date, &ctx.options.env.dates);
            if item.item_type == "dataset" || item.item_type == "computerProgram" {
                cff.push(("date-released", iso.clone()));
            }
            cff.push(("date-published", iso));
        }
        let doi = doi_from_extra(&item, doi_re)?;
        cff.push(("identifiers", write_doi(&doi)));

        ctx.write("  - ");
        for (field, value) in cff {
            let Some(value) = value.filter(|v| !v.is_empty()) else {
                continue;
            };
            if field == "title" {
                ctx.write(&format!("{field}: {value}"));
            } else if field == "abstract" {
                ctx.write(&format!("{SP}{field}: |{}\n", indent_abstract(&value)));
            } else {
                ctx.write(&format!("{SP}{}: {value}\n", field.replacen('_', "-", 1)));
            }
        }
    }
    Ok(())
}

/// `abstract.replace(/^|\n/g, "\n" + referencesSpacing + "  ")`: the empty
/// match at 0 inserts the prefix, and the global search then resumes one
/// character on, so a newline at index 0 is kept as it is; every later
/// newline is replaced.
fn indent_abstract(s: &str) -> String {
    let rep = format!("\n{SP}  ");
    let mut out = rep.clone();
    let mut chars = s.chars();
    if let Some(c) = chars.next() {
        out.push(c);
    }
    out.push_str(&chars.as_str().replace('\n', &rep));
    out
}
