// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "OpenAlex JSON.js" (translatorID
//   faa53754-fb55-4658-9094-ae8a7e0409a2, lastUpdated 2024-07-29 14:16:09):
//   header :1-11, `parseInput` :37-52, `detectImport` :54-64,
//   `PDFversionMap` :67-71, `mappingTypes` :74-95, `doImport` :97-108,
//   `parseIndividual` :110-193.
// Copyright (c) 2024 Sebastian Karcher.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The OpenAlex JSON translator: import (an OpenAlex work, or a page of
//! `results`).
//!
//! JavaScript value semantics come from the Datacite JSON port's helpers
//! ([`super::datacite_json`]). The PDF attachment upstream adds is kept on
//! the item; the translation-server's `itemToAPIJSON` drops attachments, so
//! it does not show in the Web API JSON.
//!
//! Not ported: nothing.
//!
//! **Maturity: AI draft (1).** Verified code-to-code against upstream
//! (`tests/zotero_translators.rs`, `openalex_json_*`).

use super::datacite_json::{
    eq_str, get, iter, or, parse_input, path, set, to_str, truthy, type_error, CreatorObj, JsVal,
};
use crate::zotero::framework::options::{translator_type, TranslatorMetadata};
use crate::zotero::framework::title_case::capitalize_title;
use crate::zotero::framework::utilities::clean_doi;
use crate::zotero::framework::{ImportContext, JsObject, TranslateError, TranslatorItem};
use crate::zotero::framework::TranslatorTag;
use serde_json::Value;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "faa53754-fb55-4658-9094-ae8a7e0409a2",
    label: "OpenAlex JSON",
    creator: "Sebastian Karcher",
    target: "json",
    min_version: "5.0",
    priority: 100,
    translator_type: translator_type::IMPORT,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2024-07-29 14:16:09",
};

/// `detectImport` (:54-64). A detector that throws (`results[0]` undefined)
/// detects nothing.
pub fn detect_import(ctx: &mut ImportContext) -> bool {
    let Some(parsed) = parse_input(ctx) else {
        return false;
    };
    detect(&Some(parsed)).unwrap_or(false)
}

fn detect(p: &JsVal) -> Result<bool, TranslateError> {
    if !truthy(p) {
        return Ok(false);
    }
    let ids = get(p, "ids")?;
    if truthy(&ids) && truthy(&get(&ids, "openalex")?) {
        return Ok(true);
    }
    let results = get(p, "results")?;
    if truthy(&results) {
        let ids = get(&get(&results, "0")?, "ids")?;
        if truthy(&ids) && truthy(&get(&ids, "openalex")?) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `mappingTypes[key]` (:74-95).
fn mapping_type(key: &str) -> Option<&'static str> {
    Some(match key {
        "article" => "journalArticle",
        "book" => "book",
        "book-chapter" => "bookSection",
        "dissertation" => "thesis",
        "other" => "document",
        "report" => "report",
        "paratext" => "document",
        "dataset" => "dataset",
        "reference-entry" => "encyclopediaArticle",
        "standard" => "standard",
        "editorial" => "journalArticle",
        "letter" => "journalArticle",
        "peer-review" => "document",
        "erratum" => "journalArticle",
        "grant" => "manuscript",
        "preprint" => "preprint",
        "review" => "journalArticle",
        "libguides" => "encyclopediaArticle",
        "supplementary-materials" => "journalArticle",
        "retraction" => "journalArticle",
        _ => return None,
    })
}

/// `PDFversionMap[key]` (:67-71).
fn pdf_version(key: &str) -> Option<&'static str> {
    Some(match key {
        "submittedVersion" => "Submitted Version PDF",
        "acceptedVersion" => "Accepted Version PDF",
        "publishedVersion" => "Full Text PDF",
        _ => return None,
    })
}

/// `doImport` (:97-108).
pub fn do_import(ctx: &mut ImportContext) -> Result<(), TranslateError> {
    let data: JsVal = Some(parse_input(ctx).unwrap_or(Value::Bool(false)));
    if truthy(&get(&data, "ids")?) {
        parse_individual(ctx, &data)?;
    } else {
        for result in iter(&get(&data, "results")?)? {
            parse_individual(ctx, &result)?;
        }
    }
    Ok(())
}

/// `x.toUpperCase()` (throws unless a string).
fn to_upper(v: &JsVal) -> Result<String, TranslateError> {
    match v {
        Some(Value::String(s)) => Ok(s.to_uppercase()),
        _ => Err(type_error("toUpperCase is not a function")),
    }
}

/// `parseIndividual` (:110-193).
fn parse_individual(ctx: &mut ImportContext, data: &JsVal) -> Result<(), TranslateError> {
    let oa_type = get(data, "type")?;
    let ty = mapping_type(&to_str(&oa_type)).unwrap_or("document");
    let mut item = TranslatorItem::new(ty);
    let title = get(data, "title")?;
    set(&mut item, "title", title.clone());
    // Fix all-caps titles (:117-119).
    let upper = to_upper(&title)?;
    if let Some(Value::String(t)) = &title {
        if *t == upper {
            item.set("title", capitalize_title(t, true));
        }
    }
    set(&mut item, "date", get(data, "publication_date")?);
    set(&mut item, "language", get(data, "language")?);
    let doi = get(data, "doi")?;
    if truthy(&doi) {
        let Some(Value::String(d)) = &doi else {
            return Err(TranslateError::Translator(
                "cleanDOI: argument must be a string".into(),
            ));
        };
        set(&mut item, "DOI", clean_doi(d).map(Value::from));
    }
    let primary = get(data, "primary_location")?;
    let source = get(&primary, "source")?;
    if truthy(&source) {
        let source_name = get(&source, "display_name")?;
        if item.item_type == "thesis" || item.item_type == "dataset" {
            set(&mut item, "publisher", source_name);
        } else if item.item_type == "book" {
            set(
                &mut item,
                "publisher",
                get(&source, "host_organization_name")?,
            );
        } else {
            set(&mut item, "publicationTitle", source_name);
            set(
                &mut item,
                "publisher",
                get(&source, "host_organization_name")?,
            );
        }
        let issn = get(&source, "issn")?;
        match &issn {
            Some(Value::String(_)) => set(&mut item, "ISSN", issn.clone()),
            Some(Value::Array(a)) => set(&mut item, "ISSN", a.first().cloned()),
            _ => {}
        }
        let source_type = get(&source, "type")?;
        if eq_str(&source_type, "journal") {
            item.item_type = "journalArticle".into();
        } else if eq_str(&source_type, "conference") {
            item.item_type = "conferencePaper".into();
        }
        if eq_str(&get(&primary, "version")?, "submittedVersion") {
            item.item_type = "preprint".into();
        }
    }

    // Pages (:149-158).
    let biblio = get(data, "biblio")?;
    set(&mut item, "issue", get(&biblio, "issue")?);
    set(&mut item, "volume", get(&biblio, "volume")?);
    let first = get(&biblio, "first_page")?;
    let last = get(&biblio, "last_page")?;
    if truthy(&first) && truthy(&last) && !loose_eq(&first, &last) {
        set(
            &mut item,
            "pages",
            Some(format!("{}-{}", to_str(&first), to_str(&last)).into()),
        );
    } else if truthy(&first) {
        set(&mut item, "pages", first);
    }

    // Authors (:161-169).
    let authors = get(data, "authorships")?;
    let author_list = iter(&authors)?;
    for a in &author_list {
        let name = path(a, &["author", "display_name"])?;
        let c = CreatorObj::clean(&name, "author", false)?;
        item.creators.push(c.to_creator());
    }
    // `!item.publisher & authors.length` is a bitwise AND.
    let no_publisher = i64::from(!truthy(&get_item(&item, "publisher")));
    let len = match get(&authors, "length")? {
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0),
        _ => 0,
    };
    if item.item_type == "thesis" && (no_publisher & len) != 0 {
        set(
            &mut item,
            "university",
            get(&get(&authors, "0")?, "raw_affiliation_string")?,
        );
    }

    // The open-access PDF (:171-177).
    let best = get(data, "best_oa_location")?;
    if truthy(&best) && truthy(&get(&best, "pdf_url")?) {
        let mut version: JsVal = Some("Submitted Version PDF".into());
        let v = get(&best, "version")?;
        if truthy(&v) {
            version = pdf_version(&to_str(&v)).map(Value::from);
        }
        let mut a = JsObject::new();
        a.set("url", get(&best, "pdf_url")?.unwrap_or(Value::Null));
        a.set("title", version.unwrap_or(Value::Null));
        a.set("mimeType", "application/pdf");
        item.attachments.push(a);
    }
    for tag in iter(&get(data, "keywords")?)? {
        let t = or(get(&tag, "display_name")?, get(&tag, "keyword")?);
        if let Some(t) = t.as_ref().and_then(TranslatorTag::from_value) {
            item.tags.push(t);
        }
    }
    let openalex = path(data, &["ids", "openalex"])?;
    item.set("extra", format!("OpenAlex: {}", to_str(&openalex)));
    ctx.item_done(item);
    Ok(())
}

fn get_item(item: &TranslatorItem, key: &str) -> JsVal {
    super::datacite_json::item_get(item, key)
}

/// `a == b` (loose) for the JSON values page numbers hold.
fn loose_eq(a: &JsVal, b: &JsVal) -> bool {
    match (a, b) {
        (Some(Value::String(s)), _) => eq_str(b, s),
        (_, Some(Value::String(s))) => eq_str(a, s),
        (Some(Value::Number(x)), Some(Value::Number(y))) => x.as_f64() == y.as_f64(),
        (x, y) => x == y,
    }
}
