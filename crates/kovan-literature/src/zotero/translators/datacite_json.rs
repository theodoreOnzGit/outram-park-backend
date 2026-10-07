// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Datacite JSON.js" (translatorID
//   b5b5808b-1c61-473d-9a02-e1f5ba7b8eef, lastUpdated 2025-04-29 03:02:00):
//   header :1-12, `datasetType` :38-40, `parseInput` :43-58,
//   `detectImport` :60-66, `mappingTypes` :69-98, `doImport` :105-336.
// Copyright (c) 2019 Philipp Zumstein.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The Datacite JSON translator: import (a DataCite REST API JSON record).
//!
//! The record is read with JavaScript's semantics where they decide the
//! result: a property read on `undefined`/`null` throws (the import fails,
//! as upstream's does), `for...of` over a non-iterable throws, string
//! concatenation turns values into strings with `ToString`, `a + b` of two
//! numbers adds. These helpers ([`JsVal`] and friends) are shared with the
//! OpenAlex JSON port.
//!
//! Duplicate creators are removed as upstream does it: by
//! `JSON.stringify` of each creator object, so two creators that differ
//! only in key order (for instance `{lastName, firstName, creatorType}`
//! from DataCite's own name fields and `{firstName, lastName, creatorType}`
//! from `ZU.cleanAuthor`) are both kept.
//!
//! Not ported: nothing; `datasetType` is evaluated against the schema at run
//! time (`dataset` exists in the schema the port follows, so the
//! pre-6.0.26 `document` branch is inert, as upstream on that schema).
//!
//! **Maturity: AI draft (1).** Verified code-to-code against upstream
//! (`tests/zotero_translators.rs`, `datacite_json_*`).

use crate::zotero::framework::item::TranslatorCreator;
use crate::zotero::framework::options::{translator_type, TranslatorMetadata};
use crate::zotero::framework::utilities::{clean_author, field_is_valid_for_type};
use crate::zotero::framework::{js, ImportContext, TranslateError, TranslatorItem};
use crate::zotero::framework::{TranslatorNote, TranslatorTag};
use serde_json::{Map, Value};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "b5b5808b-1c61-473d-9a02-e1f5ba7b8eef",
    label: "Datacite JSON",
    creator: "Philipp Zumstein",
    target: "json",
    min_version: "3.0",
    priority: 100,
    translator_type: translator_type::IMPORT,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2025-04-29 03:02:00",
};

// ---------------------------------------------------------------------------
// JavaScript value semantics over JSON (shared with openalex_json).
// ---------------------------------------------------------------------------

/// A JavaScript value read from parsed JSON: `None` is `undefined`.
pub(crate) type JsVal = Option<Value>;

/// A `TypeError` as the translator would throw it.
pub(crate) fn type_error(what: &str) -> TranslateError {
    TranslateError::Translator(format!("TypeError: {what}"))
}

/// `v[key]`: throws on `undefined` and `null`; a string's index or
/// `length`, an array's index or `length`, an object's property; any other
/// primitive has no (own JSON) properties.
pub(crate) fn get(v: &JsVal, key: &str) -> Result<JsVal, TranslateError> {
    match v {
        None => Err(type_error(&format!(
            "Cannot read properties of undefined (reading '{key}')"
        ))),
        Some(Value::Null) => Err(type_error(&format!(
            "Cannot read properties of null (reading '{key}')"
        ))),
        Some(Value::Object(m)) => Ok(m.get(key).cloned()),
        Some(Value::Array(a)) => Ok(if key == "length" {
            Some(Value::from(a.len()))
        } else {
            index_key(key).and_then(|i| a.get(i).cloned())
        }),
        Some(Value::String(s)) => {
            let units: Vec<char> = s.chars().collect();
            Ok(if key == "length" {
                Some(Value::from(units.len()))
            } else {
                index_key(key).and_then(|i| units.get(i).map(|c| Value::String(c.to_string())))
            })
        }
        Some(_) => Ok(None),
    }
}

/// A canonical array index ("0", "1", ...).
fn index_key(key: &str) -> Option<usize> {
    if key == "0" || (!key.starts_with('0') && key.chars().all(|c| c.is_ascii_digit())) {
        key.parse().ok()
    } else {
        None
    }
}

/// `get` along a path.
pub(crate) fn path(v: &JsVal, keys: &[&str]) -> Result<JsVal, TranslateError> {
    let mut cur = v.clone();
    for k in keys {
        cur = get(&cur, k)?;
    }
    Ok(cur)
}

/// JavaScript truthiness.
pub(crate) fn truthy(v: &JsVal) -> bool {
    js::truthy(v.as_ref())
}

/// `String(v)` (`undefined` is "undefined").
pub(crate) fn to_str(v: &JsVal) -> String {
    match v {
        None => "undefined".to_owned(),
        Some(v) => js::to_js_string(v),
    }
}

/// `a || b`.
pub(crate) fn or(a: JsVal, b: JsVal) -> JsVal {
    if truthy(&a) {
        a
    } else {
        b
    }
}

/// `for (x of v)`: an array's elements or a string's characters; anything
/// else is not iterable and throws.
pub(crate) fn iter(v: &JsVal) -> Result<Vec<JsVal>, TranslateError> {
    match v {
        Some(Value::Array(a)) => Ok(a.iter().cloned().map(Some).collect()),
        Some(Value::String(s)) => Ok(s
            .chars()
            .map(|c| Some(Value::String(c.to_string())))
            .collect()),
        _ => Err(type_error("value is not iterable")),
    }
}

/// `v == s` for a string `s` (loose equality: an object compares by its
/// string form, a number numerically, `undefined`/`null` never equal).
pub(crate) fn eq_str(v: &JsVal, s: &str) -> bool {
    match v {
        Some(Value::String(x)) => x == s,
        Some(Value::Array(_)) | Some(Value::Object(_)) => {
            js::to_js_string(v.as_ref().unwrap()) == s
        }
        Some(Value::Number(n)) => {
            let t = js::trim(s);
            let parsed = if t.is_empty() {
                Some(0.0)
            } else {
                t.parse::<f64>().ok()
            };
            parsed.is_some_and(|p| n.as_f64() == Some(p))
        }
        Some(Value::Bool(b)) => {
            let t = js::trim(s);
            let parsed = if t.is_empty() {
                Some(0.0)
            } else {
                t.parse::<f64>().ok()
            };
            parsed == Some(if *b { 1.0 } else { 0.0 })
        }
        _ => false,
    }
}

/// `v.toLowerCase()`: throws unless `v` is a string.
pub(crate) fn to_lower(v: &JsVal) -> Result<String, TranslateError> {
    match v {
        Some(Value::String(s)) => Ok(s.to_lowercase()),
        _ => Err(type_error("toLowerCase is not a function")),
    }
}

/// `a + b`: numeric addition for two numbers, else string concatenation.
pub(crate) fn add(a: &JsVal, b: &JsVal) -> JsVal {
    if let (Some(Value::Number(x)), Some(Value::Number(y))) = (a, b) {
        if let (Some(x), Some(y)) = (x.as_f64(), y.as_f64()) {
            let s = x + y;
            return Some(if s.fract() == 0.0 && s.abs() < 9.0e15 {
                Value::from(s as i64)
            } else {
                serde_json::Number::from_f64(s).map_or(Value::Null, Value::Number)
            });
        }
    }
    Some(Value::String(to_str(a) + &to_str(b)))
}

/// `Array.prototype.join(sep)` (throws unless `v` is an array).
pub(crate) fn join(v: &JsVal, sep: &str) -> Result<String, TranslateError> {
    match v {
        Some(Value::Array(a)) => Ok(a
            .iter()
            .map(|x| match x {
                Value::Null => String::new(),
                other => js::to_js_string(other),
            })
            .collect::<Vec<_>>()
            .join(sep)),
        _ => Err(type_error("join is not a function")),
    }
}

/// `item[key] = v` (an `undefined` is kept as a property whose value is
/// dropped by `_itemDone`, so it holds its place in the property order).
pub(crate) fn set(item: &mut TranslatorItem, key: &str, v: JsVal) {
    item.set(key, v.unwrap_or(Value::Null));
}

/// `item[key]` as a JS value.
pub(crate) fn item_get(item: &TranslatorItem, key: &str) -> JsVal {
    match item.get(key) {
        Some(Value::Null) | None => None,
        Some(v) => Some(v.clone()),
    }
}

/// Read all input and `JSON.parse` it (`parseInput`): `None` (upstream
/// `false`) when it does not parse.
pub(crate) fn parse_input(ctx: &mut ImportContext) -> Option<Value> {
    let mut json = String::new();
    while let Some(s) = ctx.read_chars(1_048_576) {
        json.push_str(&s);
    }
    serde_json::from_str(&json).ok()
}

/// A creator object as the translator builds it: its properties in order
/// (`None` values are `undefined`).
#[derive(Debug, Clone)]
pub(crate) struct CreatorObj(pub Vec<(&'static str, JsVal)>);

impl CreatorObj {
    /// `{lastName, firstName, creatorType}`.
    pub fn last_first(last: JsVal, first: JsVal, ty: &str) -> Self {
        CreatorObj(vec![
            ("lastName", last),
            ("firstName", first),
            ("creatorType", Some(ty.into())),
        ])
    }

    /// `{lastName, creatorType, fieldMode: 1}`.
    pub fn single(last: JsVal, ty: &str) -> Self {
        CreatorObj(vec![
            ("lastName", last),
            ("creatorType", Some(ty.into())),
            ("fieldMode", Some(1.into())),
        ])
    }

    /// `ZU.cleanAuthor(name, type, useComma)`: `{firstName, lastName,
    /// creatorType}` (throws unless `name` is a string).
    pub fn clean(name: &JsVal, ty: &str, use_comma: bool) -> Result<Self, TranslateError> {
        let Some(Value::String(name)) = name else {
            return Err(type_error("author.replace is not a function"));
        };
        let a = clean_author(name, ty, use_comma);
        Ok(CreatorObj(vec![
            ("firstName", a.first_name.map(Value::from)),
            ("lastName", Some(a.last_name.into())),
            ("creatorType", Some(a.creator_type.into())),
        ]))
    }

    /// `JSON.stringify(creator)`.
    pub fn stringify(&self) -> String {
        let mut s = String::from("{");
        let mut first = true;
        for (k, v) in &self.0 {
            let Some(v) = v else { continue };
            if !first {
                s.push(',');
            }
            first = false;
            s.push_str(&serde_json::to_string(k).unwrap());
            s.push(':');
            s.push_str(&serde_json::to_string(v).unwrap());
        }
        s.push('}');
        s
    }

    /// `JSON.parse(JSON.stringify(creator))` as a translator creator.
    pub fn to_creator(&self) -> TranslatorCreator {
        let mut m = Map::new();
        for (k, v) in &self.0 {
            if let Some(v) = v {
                m.insert((*k).to_owned(), v.clone());
            }
        }
        TranslatorCreator::from_value(&Value::Object(m))
    }
}

/// `new Set(creators.map(JSON.stringify))` then `JSON.parse` back: the
/// first of each identical serialisation, in order.
pub(crate) fn unique_creators(creators: &[CreatorObj]) -> Vec<TranslatorCreator> {
    let mut seen: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for c in creators {
        let s = c.stringify();
        if !seen.contains(&s) {
            seen.push(s);
            out.push(c.to_creator());
        }
    }
    out
}

// ---------------------------------------------------------------------------
// The translator.
// ---------------------------------------------------------------------------

/// `datasetType` (:38-40).
fn dataset_type() -> &'static str {
    if field_is_valid_for_type("title", "dataset") {
        "dataset"
    } else {
        "document"
    }
}

/// `mappingTypes[key]` (:69-102, with the pre-6.0.26 `dataset` override).
fn mapping_type(key: &str) -> Option<&'static str> {
    Some(match key {
        "article" => "preprint",
        "book" => "book",
        "chapter" => "bookSection",
        "article-journal" => "journalArticle",
        "article-magazine" => "magazineArticle",
        "article-newspaper" => "newspaperArticle",
        "thesis" => "thesis",
        "entry-encyclopedia" => "encyclopediaArticle",
        "entry-dictionary" => "dictionaryEntry",
        "paper-conference" => "conferencePaper",
        "personal_communication" => "letter",
        "manuscript" => "manuscript",
        "interview" => "interview",
        "motion_picture" => "film",
        "graphic" => "artwork",
        "webpage" => "webpage",
        "report" => "report",
        "bill" => "bill",
        "legal_case" => "case",
        "patent" => "patent",
        "legislation" => "statute",
        "map" => "map",
        "post-weblog" => "blogPost",
        "post" => "forumPost",
        "song" => "audioRecording",
        "speech" => "presentation",
        "broadcast" => "radioBroadcast",
        "dataset" => dataset_type(),
        _ => return None,
    })
}

/// `detectImport` (:60-66). A detector that throws (a non-string
/// `schemaVersion`) detects nothing.
pub fn detect_import(ctx: &mut ImportContext) -> bool {
    let Some(parsed) = parse_input(ctx) else {
        return false;
    };
    let parsed = Some(parsed);
    if !truthy(&parsed) {
        return false;
    }
    let schema = get(&parsed, "schemaVersion").unwrap_or(None);
    if truthy(&schema) {
        match &schema {
            Some(Value::String(s)) => {
                if s.starts_with("http://datacite.org/schema/") {
                    return true;
                }
            }
            _ => return false, // startsWith is not a function: throws
        }
    }
    let agency = get(&parsed, "agency").unwrap_or(None);
    // `/datacite/i`: JavaScript's non-Unicode case folding never maps a
    // non-ASCII character to an ASCII one, so ASCII folding is exact.
    to_str(&agency).to_ascii_lowercase().contains("datacite")
}

/// One `contributors` or `relatedItems[].contributor` entry (:162-185,
/// :278-296): `role`, then the three name forms.
fn push_contributor(
    creators: &mut Vec<CreatorObj>,
    c: &JsVal,
    role: &str,
    use_comma: bool,
) -> Result<(), TranslateError> {
    let family = get(c, "familyName")?;
    let given = get(c, "givenName")?;
    if truthy(&family) && truthy(&given) {
        creators.push(CreatorObj::last_first(family, given, role));
    } else if eq_str(&get(c, "nameType")?, "Personal") {
        creators.push(CreatorObj::clean(&get(c, "name")?, role, use_comma)?);
    } else {
        creators.push(CreatorObj::single(get(c, "name")?, role));
    }
    Ok(())
}

/// `doImport` (:105-336).
pub fn do_import(ctx: &mut ImportContext) -> Result<(), TranslateError> {
    // `parseInput()` is `false` when the text is not JSON.
    let data: JsVal = Some(parse_input(ctx).unwrap_or(Value::Bool(false)));
    let types = get(&data, "types")?;

    let mut ty = "journalArticle";
    let citeproc = get(&types, "citeproc")?;
    if truthy(&citeproc) {
        if let Some(t) = mapping_type(&to_str(&citeproc)) {
            ty = t;
        }
    }
    let schema_org = get(&types, "schemaOrg")?;
    if truthy(&schema_org)
        && [
            "softwaresourcecode",
            "softwareapplication",
            "mobileapplication",
            "videogame",
            "webapplication",
        ]
        .contains(&to_lower(&schema_org)?.as_str())
    {
        ty = "computerProgram";
    }
    if eq_str(&get(&types, "resourceTypeGeneral")?, "BookChapter") {
        ty = "bookSection";
    }

    let mut item = TranslatorItem::new(ty);
    if eq_str(&get(&types, "citeproc")?, "dataset") && dataset_type() == "document" {
        item.set("extra", "Type: dataset");
    }

    // Titles (:126-140).
    let mut title = String::new();
    let mut alternate: JsVal = Some("".into());
    for t in iter(&get(&data, "titles")?)? {
        let tt = get(&t, "title")?;
        if !truthy(&tt) {
            continue;
        }
        let title_type = get(&t, "titleType")?;
        if !truthy(&title_type) {
            title = to_str(&tt) + &title;
        } else if to_lower(&title_type)? == "subtitle" {
            title = title + ": " + &to_str(&tt);
        } else if !truthy(&alternate) {
            alternate = tt;
        }
    }
    set(
        &mut item,
        "title",
        if title.is_empty() {
            alternate
        } else {
            Some(title.into())
        },
    );

    // Creators (:142-188).
    let mut creators: Vec<CreatorObj> = Vec::new();
    let data_creators = get(&data, "creators")?;
    if truthy(&data_creators) {
        for c in iter(&data_creators)? {
            push_contributor(&mut creators, &c, "author", true)?;
        }
    }
    let contributors = get(&data, "contributors")?;
    if truthy(&contributors) {
        for c in iter(&contributors)? {
            let mut role = "contributor";
            let ct = get(&c, "contributorType")?;
            if truthy(&ct) {
                match to_lower(&ct)?.as_str() {
                    "editor" => role = "editor",
                    "producer" => role = "producer",
                    _ => {}
                }
            }
            push_contributor(&mut creators, &c, role, false)?;
        }
    }

    // Publisher (:189-194): `typeof null` is "object" too.
    let publisher = get(&data, "publisher")?;
    match &publisher {
        Some(Value::Object(_)) | Some(Value::Array(_)) | Some(Value::Null) => {
            set(&mut item, "publisher", get(&publisher, "name")?);
        }
        _ => set(&mut item, "publisher", publisher.clone()),
    }

    // Dates (:195-201).
    let dates_v = get(&data, "dates")?;
    if truthy(&dates_v) {
        let mut dates: Vec<(String, JsVal)> = Vec::new();
        for d in iter(&dates_v)? {
            let k = to_str(&get(&d, "dateType")?);
            let v = get(&d, "date")?;
            match dates.iter_mut().find(|(x, _)| *x == k) {
                Some(e) => e.1 = v,
                None => dates.push((k, v)),
            }
        }
        let pick = |k: &str| {
            dates
                .iter()
                .find(|(x, _)| x == k)
                .and_then(|(_, v)| v.clone())
        };
        let mut date = pick("Issued");
        for k in ["Updated", "Available", "Accepted", "Submitted", "Created"] {
            date = or(date, pick(k));
        }
        date = or(date, get(&data, "publicationYear")?);
        set(&mut item, "date", date);
    }

    set(&mut item, "DOI", get(&data, "doi")?);
    set(&mut item, "url", get(&data, "url")?);
    set(&mut item, "language", get(&data, "language")?);
    let subjects = get(&data, "subjects")?;
    if truthy(&subjects) {
        for s in iter(&subjects)? {
            if let Some(t) = get(&s, "subject")?
                .as_ref()
                .and_then(TranslatorTag::from_value)
            {
                item.tags.push(t);
            }
        }
    }
    let formats = get(&data, "formats")?;
    if truthy(&formats) {
        // `join()` with the default separator.
        item.set("medium", join(&formats, ",")?);
    }
    let sizes = get(&data, "sizes")?;
    if truthy(&sizes) {
        // `item.pages = item.artworkSize = ...`: the inner assignment runs
        // first, so artworkSize is created first.
        let s = join(&sizes, ", ")?;
        item.set("artworkSize", s.clone());
        item.set("pages", s);
    }
    set(&mut item, "version", get(&data, "version")?);
    let rights = get(&data, "rightsList")?;
    if truthy(&rights) {
        let Some(Value::Array(list)) = &rights else {
            return Err(type_error("rightsList.map is not a function"));
        };
        let mut parts = Vec::new();
        for x in list {
            let r = get(&Some(x.clone()), "rights")?;
            parts.push(match r {
                None | Some(Value::Null) => String::new(),
                Some(v) => js::to_js_string(&v),
            });
        }
        item.set("rights", parts.join(", "));
    }

    // Descriptions (:222-236).
    let mut description_note = String::new();
    let descriptions = get(&data, "descriptions")?;
    if truthy(&descriptions) {
        for d in iter(&descriptions)? {
            let dt = get(&d, "descriptionType")?;
            if eq_str(&dt, "Abstract") {
                set(&mut item, "abstractNote", get(&d, "description")?);
            } else {
                description_note += &format!(
                    "<h2>{}</h2>\n{}",
                    to_str(&dt),
                    to_str(&get(&d, "description")?)
                );
            }
        }
    }
    if !description_note.is_empty() {
        item.notes.push(TranslatorNote::new(description_note));
    }

    // Container (:237-256).
    let container = get(&data, "container")?;
    if truthy(&container) {
        if eq_str(&get(&container, "type")?, "Series") {
            set(&mut item, "publicationTitle", get(&container, "title")?);
            set(&mut item, "volume", get(&container, "volume")?);
            let pages = add(
                &or(get(&container, "firstPage")?, Some("".into())),
                &or(get(&container, "lastPage")?, Some("".into())),
            );
            if !truthy(&item_get(&item, "pages")) && pages != Some("".into()) {
                set(&mut item, "pages", pages);
            }
        }
        let identifier = get(&container, "identifier")?;
        let identifier_type = get(&container, "identifierType")?;
        if truthy(&identifier) && truthy(&identifier_type) {
            if eq_str(&identifier_type, "ISSN") {
                set(&mut item, "ISSN", identifier.clone());
            }
            if eq_str(&identifier_type, "ISBN") {
                set(&mut item, "ISBN", identifier);
            }
        }
    }

    // Related items (:257-305).
    let related = get(&data, "relatedItems")?;
    if truthy(&related) {
        for c in iter(&related)? {
            if !eq_str(&get(&c, "relationType")?, "IsPublishedIn") {
                continue;
            }
            set(&mut item, "volume", get(&c, "volume")?);
            let titles = get(&c, "titles")?;
            if truthy(&titles) {
                match &titles {
                    Some(Value::Array(a)) if !a.is_empty() => {
                        set(
                            &mut item,
                            "publicationTitle",
                            get(&Some(a[0].clone()), "title")?,
                        );
                    }
                    _ => set(&mut item, "publicationTitle", get(&titles, "title")?),
                }
            }
            let rid = get(&c, "relatedItemIdentifier")?;
            if truthy(&rid) {
                let rid_type = get(&rid, "relatedItemIdentifierType")?;
                if eq_str(&rid_type, "ISSN") {
                    set(&mut item, "ISSN", get(&rid, "relatedItemIdentifier")?);
                } else if eq_str(&rid_type, "ISBN") {
                    set(&mut item, "ISBN", get(&rid, "relatedItemIdentifier")?);
                }
            }
            set(&mut item, "issue", get(&c, "issue")?);
            let year = get(&c, "publicationYear")?;
            if truthy(&year) {
                set(&mut item, "date", year);
            }
            let first = get(&c, "firstPage")?;
            let last = get(&c, "lastPage")?;
            if truthy(&first) && truthy(&last) {
                set(
                    &mut item,
                    "pages",
                    Some(format!("{}-{}", to_str(&first), to_str(&last)).into()),
                );
            } else {
                set(
                    &mut item,
                    "pages",
                    add(&or(first, Some("".into())), &or(last, Some("".into()))),
                );
            }
            set(&mut item, "edition", get(&c, "edition")?);
            let contributor = get(&c, "contributor")?;
            if let Some(Value::Array(list)) = &contributor {
                for x in list {
                    let x = Some(x.clone());
                    let role = if eq_str(&get(&x, "contributorType")?, "Editor") {
                        "editor"
                    } else {
                        "contributor"
                    };
                    push_contributor(&mut creators, &x, role, true)?;
                }
            }
            break;
        }
    }

    // Related identifiers (:306-315).
    let rel_ids = get(&data, "relatedIdentifiers")?;
    if truthy(&rel_ids) {
        for r in iter(&rel_ids)? {
            if !truthy(&item_get(&item, "ISSN"))
                && eq_str(&get(&r, "relatedIdentifierType")?, "ISSN")
            {
                set(&mut item, "ISSN", get(&r, "relatedIdentifier")?);
            }
            if !truthy(&item_get(&item, "ISBN"))
                && eq_str(&get(&r, "relatedIdentifierType")?, "ISBN")
            {
                set(&mut item, "ISBN", get(&r, "relatedIdentifier")?);
            }
        }
    }

    // Remove duplicate creators (:317-319).
    item.creators = unique_creators(&creators);
    ctx.item_done(item);
    Ok(())
}
