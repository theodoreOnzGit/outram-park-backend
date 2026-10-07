// Part of the kovan Zotero port (GitHub #747, #748).
//
// Upstream: Zotero utilities, https://github.com/zotero/utilities
//   (commit 4051881d59c6), utilities_item.js: `edtfToCSLDate` :43,
//   `cslDateToEDTF` :68, `dateExportsAsLiteral` :134, `itemToCSLJSON` :161,
//   `itemFromCSLJSON` :417, `parseParticles` :683 (itself taken from
//   citeproc-js, (c) 2009-2019 Frank Bennett, CPAL-1.0 or AGPL-3.0, used here
//   under the AGPL), `noteToTitle` :806, `extraToCSL` :844; and
//   utilities.js `unescapeHTML` :699.
// Copyright (c) 2021 Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! CSL-JSON <-> Zotero item, ported from `Zotero.Utilities.Item`.
//!
//! [`item_to_csl_json`] is `itemToCSLJSON` applied to the export format of an
//! item (the JSON [`ZoteroItem`] holds); [`item_from_csl_json`] is
//! `itemFromCSLJSON` filling a fresh `Zotero.Item` (the `isZoteroItem` path,
//! so base-mapped values land in the type-specific field, as `setField`
//! stores them). A CSL item is a JSON object, [`CslItem`].
//!
//! Differences from upstream, all deliberate:
//!
//! * dates go through [`super::date`] (en-US months, configurable "local"
//!   time zone; see that module);
//! * `noteToTitle` strips tags and decodes entities with a small decoder
//!   instead of a DOM (`textContent`); block-level whitespace can differ for
//!   unusual HTML;
//! * the CSL `id` is the item's `uri` (as for an export-format item) and is
//!   omitted when there is none; `itemFromCSLJSON` ignores the CSL `id`, as
//!   the `isZoteroItem` path does.

use super::date::{
    format_date, looks_like_edtf, lpad, parse_edtf, parse_era_year, str_to_date, str_to_iso,
    utc_sql_to_local_sql, is_iso_date, is_sql_date, iso_to_sql, DateOptions, EdtfDate, EdtfParts,
};
use super::item::{Creator, CreatorName, ZoteroItem};
use super::schema::{
    field_from_type_and_base, is_valid_creator_type, is_valid_for_type, item_types_for_csl_type,
    primary_creator_type,
};
use super::schema_generated::{
    CreatorType, Field, ItemType, CSL_DATE_FIELDS, CSL_NAMES, CSL_TEXT_FIELDS,
};
use regex::Regex;
use serde_json::{json, Map, Value};
use std::sync::OnceLock;

/// A CSL-JSON item.
pub type CslItem = Map<String, Value>;

/// Why a conversion failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CslError {
    /// `itemToCSLJSON`: the item type has no CSL type (`annotation`);
    /// upstream throws `Unexpected Zotero Item type "..."` (:176).
    UnexpectedItemType(ItemType),
    /// `itemFromCSLJSON`: no `type` (:424-427).
    MissingType,
}

impl std::fmt::Display for CslError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CslError::UnexpectedItemType(t) => {
                write!(f, "Unexpected Zotero Item type \"{}\"", t.as_str())
            }
            CslError::MissingType => write!(f, "No 'type' provided in CSL-JSON"),
        }
    }
}

impl std::error::Error for CslError {}

/// CSL date variables parsed as EDTF in Extra (:33).
const EXTRA_DATE_VARIABLES: [&str; 6] = [
    "issued",
    "event-date",
    "original-date",
    "publication-date",
    "available-date",
    "submitted",
];

/// Item types whose Place exports as `event-place` (:197-199).
const EVENT_PLACE_TYPES: [ItemType; 3] = [
    ItemType::AudioRecording,
    ItemType::Presentation,
    ItemType::VideoRecording,
];

fn re(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).expect("static regex compiles"))
}

/// `QUOTED_DATE_RE` (:40): the inner text of a date in straight or curly
/// double quotes.
fn quoted_date(s: &str) -> Option<String> {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, r#"^["\x{201c}](.+)["\x{201d}]$"#)
        .captures(s)
        .map(|c| c[1].to_owned())
}

/// `edtfToCSLDate` (:43).
fn edtf_to_csl_date(e: &EdtfDate) -> Value {
    let parts = |p: &EdtfParts| {
        let mut v = vec![json!(p.year)];
        if let Some(m) = p.month {
            v.push(json!(m + 1));
            if let Some(d) = p.day.filter(|d| *d != 0) {
                v.push(json!(d));
            }
        }
        Value::Array(v)
    };
    let mut date_parts = vec![parts(&e.begin)];
    if let Some(end) = &e.end {
        date_parts.push(parts(end));
    }
    let mut o = Map::new();
    o.insert("date-parts".into(), Value::Array(date_parts));
    if e.circa {
        o.insert("circa".into(), Value::Bool(true));
    }
    Value::Object(o)
}

/// JavaScript truthiness of a JSON value.
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

/// A date part as an integer, the way `cslDateToEDTF`'s `toInt` reads it.
fn to_int(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => {
            let t = s.strip_prefix('-').unwrap_or(s);
            if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) {
                s.parse().ok()
            } else {
                None
            }
        }
        _ => None,
    }
}

/// `cslDateToEDTF` (:68): a CSL date with a range or circa flag as EDTF, or
/// `None`.
pub fn csl_date_to_edtf(d: &Map<String, Value>) -> Option<String> {
    if d.get("season").is_some_and(truthy) {
        return None;
    }
    let dp = d.get("date-parts")?.as_array()?;
    let first = dp.first()?.as_array().filter(|a| !a.is_empty())?;
    let end = dp
        .get(1)
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty());
    let circa = d.get("circa").is_some_and(truthy);
    if end.is_none() && !circa {
        return None;
    }
    let to_edtf = |parts: &[Value]| -> Option<String> {
        let year = to_int(&parts[0])?;
        if !(-9999..=9999).contains(&year) {
            return None;
        }
        let mut s = format!(
            "{}{}",
            if year < 0 { "-" } else { "" },
            lpad(&year.abs().to_string(), '0', 4)
        );
        if parts.len() > 1 {
            let month = to_int(&parts[1]).filter(|m| (1..=12).contains(m))?;
            s.push_str(&format!("-{}", lpad(&month.to_string(), '0', 2)));
            if parts.len() > 2 {
                let day = to_int(&parts[2]).filter(|d| (1..=31).contains(d))?;
                s.push_str(&format!("-{}", lpad(&day.to_string(), '0', 2)));
            }
        }
        Some(s)
    };
    let mut date = to_edtf(first)?;
    if let Some(end) = end {
        date.push('/');
        date.push_str(&to_edtf(end)?);
    }
    if circa {
        date.push('~');
    }
    parse_edtf(&date)?;
    Some(date)
}

/// `dateExportsAsLiteral` (:134).
pub fn date_exports_as_literal(s: &str, opts: &DateOptions) -> bool {
    let s = s.trim();
    quoted_date(s).is_none()
        && parse_edtf(s).is_none()
        && (looks_like_edtf(s) || str_to_date(s, opts).year.is_none())
}

/// The first ISBN of a field, `/^(?:97[89]-?)?(?:\d-?){9}[\dx](?!-)\b/i`
/// (:231), matched by a small backtracking search because the `regex` crate
/// has no look-ahead. Candidates are tried in the order JavaScript's
/// backtracking tries them.
fn first_isbn(s: &str) -> Option<&str> {
    let b = s.as_bytes();
    let is_word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    // After 9 digit units at `pos`: [\dx], then not '-', then \b.
    let tail = |pos: usize| -> Option<usize> {
        let c = *b.get(pos)?;
        if !(c.is_ascii_digit() || c == b'x' || c == b'X') {
            return None;
        }
        let next = b.get(pos + 1).copied();
        if next == Some(b'-') {
            return None;
        }
        // \b between a word char and next: next must be a non-word char or end.
        if next.is_some_and(is_word) {
            return None;
        }
        Some(pos + 1)
    };
    fn units<F: Fn(usize) -> Option<usize>>(
        b: &[u8],
        pos: usize,
        left: u32,
        tail: &F,
    ) -> Option<usize> {
        if left == 0 {
            return tail(pos);
        }
        if !b.get(pos)?.is_ascii_digit() {
            return None;
        }
        if b.get(pos + 1) == Some(&b'-') {
            if let Some(e) = units(b, pos + 2, left - 1, tail) {
                return Some(e);
            }
        }
        units(b, pos + 1, left - 1, tail)
    }
    let mut starts = Vec::new();
    if b.len() >= 3 && b[0] == b'9' && b[1] == b'7' && (b[2] == b'8' || b[2] == b'9') {
        if b.get(3) == Some(&b'-') {
            starts.push(4);
        }
        starts.push(3);
    }
    starts.push(0);
    for st in starts {
        if let Some(end) = units(b, st, 9, &tail) {
            return Some(&s[..end]);
        }
    }
    None
}

/// `Zotero.Utilities.unescapeHTML` (utilities.js:699) without a DOM: strip
/// tags, decode the common named and all numeric entities, collapse runs of
/// spaces.
pub fn unescape_html(s: &str) -> String {
    if !s.contains('<') && !s.contains('&') {
        return s.to_owned();
    }
    let mut text = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match (in_tag, c) {
            (false, '<') => in_tag = true,
            (true, '>') => in_tag = false,
            (false, c) => text.push(c),
            _ => {}
        }
    }
    static ENT: OnceLock<Regex> = OnceLock::new();
    let ent = re(&ENT, r"&(#[0-9]+|#[xX][0-9a-fA-F]+|[a-zA-Z]+);");
    let decoded = ent.replace_all(&text, |c: &regex::Captures| {
        let e = &c[1];
        let ch = if let Some(hex) = e.strip_prefix("#x").or_else(|| e.strip_prefix("#X")) {
            u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
        } else if let Some(dec) = e.strip_prefix('#') {
            dec.parse().ok().and_then(char::from_u32)
        } else {
            match e {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some('\u{a0}'),
                _ => None,
            }
        };
        ch.map(String::from).unwrap_or_else(|| c[0].to_owned())
    });
    static SP: OnceLock<Regex> = OnceLock::new();
    re(&SP, r" {2,}").replace_all(&decoded, " ").into_owned()
}

/// `noteToTitle` (:806): the first line (at most 120 characters) of a
/// note's HTML as plain text.
pub fn note_to_title(text: &str, stop_at_line_break: bool) -> String {
    const MAX: usize = 120;
    let orig = text;
    let mut t = text.trim().to_owned();
    static BLOCK: OnceLock<Regex> = OnceLock::new();
    t = re(&BLOCK, r"(</(h\d|p|div)+>)")
        .replace_all(&t, "$1\n")
        .into_owned();
    static BR: OnceLock<Regex> = OnceLock::new();
    t = re(&BR, r"<br\s*/?>")
        .replace_all(&t, if stop_at_line_break { "\n" } else { " " })
        .into_owned();
    t = unescape_html(&t);
    static OPEN: OnceLock<Regex> = OnceLock::new();
    if re(&OPEN, r"^<[^>\n]+[^/]>\n").is_match(orig) {
        t = t.trim().to_owned();
    }
    // JavaScript substring counts UTF-16 units; count chars (equal for BMP text).
    let mut t: String = t.chars().take(MAX).collect();
    if let Some(i) = t.find('\n') {
        t.truncate(i);
    }
    t
}

/// `extraToCSL` (:844): rename `Field: value` lines of Extra to CSL
/// variable names.
pub fn extra_to_csl(extra: &str) -> String {
    static R: OnceLock<Regex> = OnceLock::new();
    let line = re(&R, r"(?m)^([A-Za-z \-]+)(:\s*.+)");
    line.replace_all(extra, |c: &regex::Captures| {
        let original = &c[1];
        let value = &c[2];
        let field = original.to_lowercase().replace(' ', "-");
        let out = match field.as_str() {
            "abstract"
            | "accessed"
            | "annote"
            | "archive"
            | "archive-place"
            | "author"
            | "authority"
            | "call-number"
            | "chapter-number"
            | "citation-label"
            | "citation-number"
            | "collection-editor"
            | "collection-number"
            | "collection-title"
            | "composer"
            | "container"
            | "container-author"
            | "container-title"
            | "container-title-short"
            | "dimensions"
            | "director"
            | "edition"
            | "editor"
            | "editorial-director"
            | "event"
            | "event-date"
            | "event-place"
            | "first-reference-note-number"
            | "genre"
            | "illustrator"
            | "interviewer"
            | "issue"
            | "issued"
            | "jurisdiction"
            | "keyword"
            | "language"
            | "locator"
            | "medium"
            | "note"
            | "number"
            | "number-of-pages"
            | "number-of-volumes"
            | "original-author"
            | "original-date"
            | "original-publisher"
            | "original-publisher-place"
            | "original-title"
            | "page"
            | "page-first"
            | "publisher"
            | "publisher-place"
            | "recipient"
            | "references"
            | "reviewed-author"
            | "reviewed-title"
            | "scale"
            | "section"
            | "source"
            | "status"
            | "submitted"
            | "title"
            | "title-short"
            | "translator"
            | "type"
            | "version"
            | "volume"
            | "year-suffix" => field,
            "doi" | "isbn" | "issn" | "pmcid" | "pmid" | "url" => field.to_uppercase(),
            "archive-location" => "archive_location".to_owned(),
            _ => {
                // "Publication Title" -> "publicationTitle" (first " X" only).
                let mut z = String::new();
                let mut done = false;
                let chars: Vec<char> = original.chars().collect();
                let mut i = 0;
                while i < chars.len() {
                    if !done
                        && chars[i] == ' '
                        && chars.get(i + 1).is_some_and(|c| c.is_ascii_uppercase())
                    {
                        done = true;
                        i += 1;
                        continue;
                    }
                    z.push(chars[i]);
                    i += 1;
                }
                let zc: Vec<char> = z.chars().collect();
                if zc.len() > 1 && zc[1].to_lowercase().eq(std::iter::once(zc[1])) {
                    let mut s: String = zc[0].to_lowercase().collect();
                    s.extend(&zc[1..]);
                    z = s;
                }
                match Field::from_name(&z).and_then(super::schema::csl_variable_for_field) {
                    Some(v) => v.to_owned(),
                    None => original.to_owned(),
                }
            }
        };
        format!("{out}{value}")
    })
    .into_owned()
}

/// `parseParticles` (:683): split name particles out of a CSL name object
/// with `family` and `given` set ("Jean de" + "la Fontaine" -> given "Jean",
/// dropping "de", non-dropping "la", family "Fontaine").
pub fn parse_particles(name: &mut Map<String, Value>) {
    static GIVEN: OnceLock<Regex> = OnceLock::new();
    static FAMILY: OnceLock<Regex> = OnceLock::new();
    let given_re = re(&GIVEN, r"^([^ ]+(?:\x{02bb} |\x{2019} | |' ) *)(.+)$");
    let family_re = re(&FAMILY, r"^([^ ]+(?:-|\x{02bb}|\x{2019}| |') *)(.+)$");
    let rev = |s: &str| s.chars().rev().collect::<String>();
    let clen = |s: &str| s.chars().count();
    let slice = |s: &str, a: usize, b: usize| {
        s.chars()
            .skip(a)
            .take(b.saturating_sub(a))
            .collect::<String>()
    };

    let split = |value: &str, first_name: bool| -> (String, Vec<String>) {
        let mut orig = value.to_owned();
        let mut name_value = if first_name {
            rev(value)
        } else {
            value.to_owned()
        };
        let rex = if first_name { given_re } else { family_re };
        let mut list: Vec<String> = Vec::new();
        while let Some(m) = rex.captures(&name_value) {
            let m1 = if first_name {
                rev(&m[1])
            } else {
                m[1].to_owned()
            };
            static FC: OnceLock<Regex> = OnceLock::new();
            let fc_re = re(&FC, r"^[-'\x{02bb}\x{2019}\s]*(.).*$");
            let first_char = fc_re.replace(&m1, "$1").into_owned();
            let has_particle = !first_char.is_empty() && first_char.to_uppercase() != first_char;
            if !has_particle {
                break;
            }
            let n = clen(&m1);
            let len = clen(&orig);
            if first_name {
                list.push(slice(&orig, len - n, len));
                orig = slice(&orig, 0, len - n);
            } else {
                list.push(slice(&orig, 0, n));
                orig = slice(&orig, n, len);
            }
            name_value = m[2].to_owned();
        }
        let out = if first_name {
            list.reverse();
            for i in 1..list.len() {
                if list[i].starts_with(' ') {
                    list[i - 1].push(' ');
                }
            }
            for p in list.iter_mut() {
                if let Some(s) = p.strip_prefix(' ') {
                    *p = s.to_owned();
                }
            }
            slice(&orig, 0, clen(&name_value))
        } else {
            let len = clen(&orig);
            let n = clen(&name_value);
            if n == 0 {
                orig.clone()
            } else {
                slice(&orig, len.saturating_sub(n), len)
            }
        };
        (out, list)
    };
    let trim_last = |s: &str| {
        let last = s.chars().last();
        let mut t = s.trim().to_owned();
        if last == Some(' ') && (t.ends_with('\'') || t.ends_with('\u{2019}')) {
            t.push(' ');
        }
        t
    };
    let get = |name: &Map<String, Value>, k: &str| {
        name.get(k).and_then(Value::as_str).unwrap_or("").to_owned()
    };

    let (family, list) = split(&get(name, "family"), false);
    name.insert("family".into(), family.into());
    let ndp = trim_last(&list.concat());
    if !ndp.is_empty() {
        name.insert("non-dropping-particle".into(), ndp.into());
    }
    // parseSuffix
    let given = get(name, "given");
    if !name.get("suffix").is_some_and(truthy) && !given.is_empty() {
        static SUF: OnceLock<Regex> = OnceLock::new();
        if let Some(m) = re(&SUF, r"(\s*,!*\s*)").find(&given) {
            let possible_suffix = given[m.end()..].to_owned();
            let possible_comma: String =
                m.as_str().chars().filter(|c| !c.is_whitespace()).collect();
            if possible_suffix.replace('.', "") == "et al"
                && !name.contains_key("dropping-particle")
            {
                name.insert("dropping-particle".into(), possible_suffix.into());
                name.insert("comma-dropping-particle".into(), ",".into());
            } else {
                if possible_comma.chars().count() == 2 {
                    name.insert("comma-suffix".into(), Value::Bool(true));
                }
                name.insert("suffix".into(), possible_suffix.into());
            }
            name.insert("given".into(), given[..m.start()].to_owned().into());
        }
    }
    let (given, list) = split(&get(name, "given"), true);
    name.insert("given".into(), given.into());
    let dp = list.concat().trim().to_owned();
    if !dp.is_empty() {
        name.insert("dropping-particle".into(), dp.into());
    }
}

/// The value `itemToCSLJSON` reads for a text mapping field (:209-224).
fn text_value(item: &ZoteroItem, field: Field) -> Option<&str> {
    if let Some(v) = item.fields.get(field.as_str()) {
        return Some(v.as_str());
    }
    // `versionNumber` is looked up as 'version', which is no field (:216).
    if field == Field::VersionNumber {
        return None;
    }
    let f = field_from_type_and_base(item.item_type, field)?;
    item.field(f)
}

/// `itemToCSLJSON` (:161) with [`DateOptions::default`].
pub fn item_to_csl_json(item: &ZoteroItem) -> Result<CslItem, CslError> {
    item_to_csl_json_with(item, &DateOptions::default())
}

/// `itemToCSLJSON` (:161): a Zotero item (export format) as a CSL item.
///
/// # Errors
/// [`CslError::UnexpectedItemType`] for an `annotation`.
pub fn item_to_csl_json_with(item: &ZoteroItem, opts: &DateOptions) -> Result<CslItem, CslError> {
    let it = item.item_type;
    let csl_type = it.csl_type().ok_or(CslError::UnexpectedItemType(it))?;
    let mut csl = CslItem::new();
    if let Some(uri) = &item.uri {
        csl.insert("id".into(), uri.clone().into());
    }
    csl.insert("type".into(), csl_type.into());

    // Text variables.
    for (variable, fields) in CSL_TEXT_FIELDS.iter() {
        if *variable == "shortTitle" {
            continue;
        }
        let mut fields: &[Field] = fields;
        if EVENT_PLACE_TYPES.contains(&it) {
            if *variable == "event-place" {
                fields = &[Field::Place];
            } else if *variable == "publisher-place" {
                continue;
            }
        }
        for field in fields {
            let Some(value) = text_value(item, *field).filter(|v| !v.is_empty()) else {
                continue;
            };
            let mut value = value.to_owned();
            if *field == Field::Isbn {
                if let Some(isbn) = first_isbn(&value) {
                    value = isbn.to_owned();
                }
            } else if *field == Field::Extra {
                value = extra_to_csl(&value);
            }
            // Strip enclosing quotes (:239).
            let n = value.chars().count();
            if value.starts_with('"')
                && n > 1
                && value[1..].find('"').map(|i| i + 1) == Some(value.len() - 1)
            {
                value = value[1..value.len() - 1].to_owned();
            }
            csl.insert((*variable).to_owned(), value.into());
            break;
        }
    }

    // Name variables (upstream tests `zoteroItem.type`, which an
    // export-format item never has, so this always runs; :249).
    let primary = primary_creator_type(it);
    for creator in &item.creators {
        let mut csl_var = creator.creator_type.csl_name();
        if csl_var.is_none() && Some(creator.creator_type) == primary {
            csl_var = Some("author");
        }
        let Some(csl_var) = csl_var else { continue };
        let name_obj = match &creator.name {
            CreatorName::SingleField { name } => json!({ "literal": name }),
            CreatorName::TwoField {
                first_name,
                last_name,
            } if first_name.is_empty() && last_name.is_empty() => {
                // Neither literal nor family/given: upstream pushes `undefined`.
                Value::Null
            }
            CreatorName::TwoField {
                first_name,
                last_name,
            } => {
                let mut o = Map::new();
                o.insert("family".into(), last_name.clone().into());
                o.insert("given".into(), first_name.clone().into());
                if !last_name.is_empty() && !first_name.is_empty() {
                    let lc: Vec<char> = last_name.chars().collect();
                    if lc.len() > 1 && lc[0] == '"' && lc[lc.len() - 1] == '"' {
                        o.insert(
                            "family".into(),
                            lc[1..lc.len() - 1].iter().collect::<String>().into(),
                        );
                    } else {
                        parse_particles(&mut o);
                    }
                }
                Value::Object(o)
            }
        };
        match csl.get_mut(csl_var) {
            Some(Value::Array(a)) => a.push(name_obj),
            _ => {
                csl.insert(csl_var.to_owned(), Value::Array(vec![name_obj]));
            }
        }
    }

    // Date variables.
    for (variable, mapping) in CSL_DATE_FIELDS.iter() {
        let mut date = item.field(*mapping).map(str::to_owned);
        if date.as_deref().is_none_or(str::is_empty) {
            date = field_from_type_and_base(it, *mapping)
                .and_then(|f| item.field(f))
                .map(str::to_owned);
        }
        let Some(mut date) = date.filter(|d| !d.is_empty()) else {
            continue;
        };
        let is_access = *mapping == Field::AccessDate;
        if is_access && !is_sql_date(&date) {
            if is_iso_date(&date) {
                date = iso_to_sql(&date).unwrap_or_default();
            }
            date = utc_sql_to_local_sql(&date, opts);
        }
        let quoted = if is_access {
            None
        } else {
            quoted_date(date.trim())
        };
        let edtf = if is_access || quoted.is_some() {
            None
        } else {
            parse_edtf(&date)
        };
        let value = if let Some(q) = quoted {
            json!({ "literal": q })
        } else if let Some(e) = edtf {
            edtf_to_csl_date(&e)
        } else if !is_access && looks_like_edtf(&date) {
            json!({ "literal": date })
        } else {
            let d = str_to_date(&date, opts);
            match &d.year {
                Some(year) => {
                    let mut parts = vec![match parse_era_year(year) {
                        Some(n) => json!(n),
                        None => json!(year),
                    }];
                    if let Some(m) = d.month {
                        parts.push(json!(m + 1));
                        if let Some(day) = d.day {
                            parts.push(json!(day));
                        }
                    }
                    let mut o = Map::new();
                    o.insert("date-parts".into(), json!([parts]));
                    if let (Some(part), None) = (&d.part, d.month) {
                        o.insert("season".into(), part.clone().into());
                    }
                    Value::Object(o)
                }
                None => json!({ "literal": date }),
            }
        };
        csl.insert((*variable).to_owned(), value);
    }

    // EDTF dates in Extra's date variables (:367-401).
    if let Some(note) = csl.get("note").and_then(Value::as_str).map(str::to_owned) {
        static LINE: OnceLock<Regex> = OnceLock::new();
        let line_re = re(&LINE, r"^([-_a-z]+|[A-Z]+):\s*([^}]+)$");
        let mut lines: Vec<String> = note.split('\n').map(str::to_owned).collect();
        for (i, line) in lines.iter_mut().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let Some(m) = line_re.captures(line) else {
                if i == 0 {
                    continue;
                }
                break;
            };
            let var = m[1].to_owned();
            if !EXTRA_DATE_VARIABLES.contains(&var.as_str()) {
                continue;
            }
            let value = m[2].trim().to_owned();
            if let Some(q) = quoted_date(&value) {
                csl.insert(var, json!({ "literal": q }));
                *line = String::new();
                continue;
            }
            if let Some(e) = parse_edtf(&value) {
                csl.insert(var, edtf_to_csl_date(&e));
                *line = String::new();
            }
        }
        csl.insert("note".into(), lines.join("\n").into());
    }

    if it == ItemType::Note {
        if let Some(n) = item.note.as_deref().filter(|n| !n.is_empty()) {
            csl.insert("title".into(), note_to_title(n, false).into());
        }
    }
    Ok(csl)
}

/// `itemFromCSLJSON` (:417) with [`DateOptions::default`].
pub fn item_from_csl_json(csl: &CslItem) -> Result<ZoteroItem, CslError> {
    item_from_csl_json_with(csl, &DateOptions::default())
}

/// The item type `itemFromCSLJSON` picks for a CSL item (:432-468).
pub fn item_type_for_csl(csl: &CslItem) -> Option<ItemType> {
    let ty = csl.get("type")?.as_str()?;
    let has = |k: &str| csl.get(k).is_some_and(truthy);
    Some(
        if ty == "bill" && (has("publisher") || has("number-of-volumes")) {
            ItemType::Hearing
        } else if ty == "broadcast"
            && [
                "archive",
                "archive_location",
                "container-title",
                "event-place",
                "publisher",
                "publisher-place",
                "source",
            ]
            .iter()
            .any(|k| has(k))
        {
            if has("author") && has("director") {
                ItemType::RadioBroadcast
            } else {
                ItemType::TvBroadcast
            }
        } else if ty == "book" && has("version") {
            ItemType::ComputerProgram
        } else if ty == "song" && has("number") {
            ItemType::Podcast
        } else if ty == "motion_picture"
            && (has("collection-title") || has("volume") || has("number-of-volumes") || has("ISBN"))
        {
            ItemType::VideoRecording
        } else if let Some(types) = item_types_for_csl_type(ty) {
            types[0]
        } else {
            ItemType::Document
        },
    )
}

/// A CSL value as the string `setField` would store (numbers become
/// strings, item.js:779; other JSON types are not text).
fn csl_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// `itemFromCSLJSON` (:417): a CSL item as a new Zotero item.
///
/// # Errors
/// [`CslError::MissingType`] when the CSL item has no `type`.
pub fn item_from_csl_json_with(
    csl_in: &CslItem,
    opts: &DateOptions,
) -> Result<ZoteroItem, CslError> {
    let mut csl = csl_in.clone();
    if !csl.get("type").is_some_and(truthy) {
        return Err(CslError::MissingType);
    }
    let it = item_type_for_csl(&csl).ok_or(CslError::MissingType)?;
    let mut item = ZoteroItem::new(it);

    if EVENT_PLACE_TYPES.contains(&it) {
        if let Some(ep) = csl.remove("event-place") {
            csl.insert("publisher-place".into(), ep);
        }
    }

    // Text fields.
    for (variable, mappings) in CSL_TEXT_FIELDS.iter() {
        let Some(value) = csl.get(*variable) else {
            continue;
        };
        for field in mappings.iter() {
            let mut target = *field;
            if target.is_base_field() {
                if let Some(t) = field_from_type_and_base(it, target) {
                    target = t;
                }
            }
            if is_valid_for_type(target, it) {
                match csl_text(value) {
                    Some(s) => item.set_field(target, s),
                    None => item.set_field(target, ""),
                }
                break;
            }
        }
    }

    // Creators.
    let mut done: Vec<&str> = Vec::new();
    for (creator_type, csl_var) in CSL_NAMES.iter() {
        if done.contains(csl_var) {
            continue;
        }
        done.push(csl_var);
        let Some(names) = csl.get(*csl_var).and_then(Value::as_array) else {
            continue;
        };
        let ct: Option<CreatorType> = if is_valid_creator_type(*creator_type, it) {
            Some(*creator_type)
        } else {
            primary_creator_type(it)
        };
        let Some(ct) = ct else { continue };
        for n in names {
            let Some(o) = n.as_object() else { continue };
            let s = |k: &str| o.get(k).and_then(csl_text).unwrap_or_default();
            let (family, given, literal) = (s("family"), s("given"), s("literal"));
            if !family.is_empty() || !given.is_empty() {
                item.creators.push(Creator::person(ct, given, family));
            } else if !literal.is_empty() {
                item.creators.push(Creator::single(ct, literal));
            }
        }
    }

    // Dates.
    for (variable, mapping) in CSL_DATE_FIELDS.iter() {
        let Some(cdate) = csl.get(*variable) else {
            continue;
        };
        let mut target = *mapping;
        if target.is_base_field() {
            if let Some(t) = field_from_type_and_base(it, target) {
                target = t;
            }
        }
        if !is_valid_for_type(target, it) {
            continue;
        }
        let empty = Map::new();
        let cd = cdate.as_object().unwrap_or(&empty);
        let literal = cd.get("literal").filter(|v| truthy(v)).and_then(csl_text);
        let raw = cd.get("raw").filter(|v| truthy(v)).and_then(csl_text);
        let is_accessed = *variable == "accessed";
        let edtf = if !is_accessed && literal.is_none() && raw.is_none() {
            csl_date_to_edtf(cd)
        } else {
            None
        };
        let mut date = String::new();
        if literal.is_some() || raw.is_some() {
            date = literal.clone().or(raw).unwrap_or_default();
            if is_accessed {
                date = str_to_iso(&date, opts).unwrap_or_default();
            } else if literal.is_some() && !date_exports_as_literal(&date, opts) {
                date = format!("\"{date}\"");
            }
        } else if let Some(e) = edtf {
            date = e;
        } else {
            let first = cd
                .get("date-parts")
                .and_then(Value::as_array)
                .and_then(|a| a.first())
                .and_then(Value::as_array);
            let pick = |i: usize| first.and_then(|p| p.get(i)).filter(|v| truthy(v)).cloned();
            let year = pick(0).or_else(|| cd.get("year").cloned());
            let month = pick(1).or_else(|| cd.get("month").cloned());
            let day = pick(2).or_else(|| cd.get("day").cloned());
            let as_str = |v: &Value| csl_text(v).unwrap_or_default();
            if let Some(year) = year.filter(truthy) {
                if is_accessed {
                    date = lpad(&as_str(&year), '0', 4);
                    if let Some(m) = month.filter(truthy) {
                        date.push('-');
                        date.push_str(&lpad(&as_str(&m), '0', 2));
                        if let Some(d) = day.filter(truthy) {
                            date.push('-');
                            date.push_str(&lpad(&as_str(&d), '0', 2));
                        }
                    }
                } else {
                    let month0 = month
                        .filter(truthy)
                        .and_then(|m| to_int(&m))
                        .and_then(|m| u32::try_from(m - 1).ok());
                    let day = day
                        .filter(truthy)
                        .and_then(|d| to_int(&d))
                        .and_then(|d| u32::try_from(d).ok());
                    date = format_date(None, Some(&as_str(&year)), month0, day);
                    if let Some(season) = cd.get("season").filter(|v| truthy(v)).and_then(csl_text)
                    {
                        date = format!("{season} {date}");
                    }
                }
            }
        }
        item.set_field(target, date);
    }
    Ok(item)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> DateOptions {
        DateOptions {
            current_year: 2026,
            ..DateOptions::default()
        }
    }

    fn book_with_date(date: &str) -> Value {
        let mut item = ZoteroItem::new(ItemType::Book);
        item.set_field(Field::Date, date);
        item_to_csl_json_with(&item, &opts())
            .unwrap()
            .get("issued")
            .cloned()
            .unwrap()
    }

    fn date_of(csl: Value) -> String {
        let csl: CslItem = serde_json::from_value(csl).unwrap();
        item_from_csl_json_with(&csl, &opts())
            .unwrap()
            .field(Field::Date)
            .unwrap_or("")
            .to_owned()
    }

    /// utilities test/tests/utilities_itemTest.js:3-31.
    #[test]
    fn from_csl_preserves_edtf_ranges_and_circa() {
        assert_eq!(
            date_of(json!({"type": "book", "issued": {"date-parts": [[2021], [2026]]}})),
            "2021/2026"
        );
        assert_eq!(
            date_of(json!({"type": "book", "issued": {"date-parts": [[-429]], "circa": true}})),
            "-0429~"
        );
        assert_eq!(
            date_of(json!({"type": "book", "issued": {"date-parts": [[2021, 5], [2021, 6]]}})),
            "2021-05/2021-06"
        );
        assert_eq!(
            date_of(json!({"type": "book", "issued": {"date-parts": [[-1], [0]]}})),
            "-0001/0000"
        );
        assert_eq!(
            date_of(
                json!({"type": "book", "issued": {"date-parts": [[2021]], "season": "Spring", "circa": true}})
            ),
            "Spring 2021"
        );
        assert_eq!(
            date_of(json!({"type": "book", "issued": {"date-parts": [[2026], [2021]]}})),
            "2026"
        );
    }

    /// utilities_itemTest.js:33-42.
    #[test]
    fn from_csl_quotes_literal_dates_that_would_parse() {
        assert_eq!(
            date_of(json!({"type": "book", "issued": {"literal": "1637 and 1662"}})),
            "\"1637 and 1662\""
        );
        assert_eq!(
            date_of(json!({"type": "book", "issued": {"literal": "n.d."}})),
            "n.d."
        );
    }

    /// utilities_itemTest.js:191-249.
    #[test]
    fn to_csl_dates() {
        assert_eq!(
            book_with_date("2021/2026"),
            json!({"date-parts": [[2021], [2026]]})
        );
        assert_eq!(
            book_with_date("-429?"),
            json!({"date-parts": [[-429]], "circa": true})
        );
        assert_eq!(
            book_with_date("~429 BCE"),
            json!({"date-parts": [[-429]], "circa": true})
        );
        assert_eq!(
            book_with_date("1000-900 BCE"),
            json!({"date-parts": [[-1000], [-900]]})
        );
        assert_eq!(
            book_with_date("2021-22"),
            json!({"date-parts": [[2021], [2022]]})
        );
        for lit in ["-500/-750", "500-750 BCE", "1996-1995", "2026/2021"] {
            assert_eq!(book_with_date(lit), json!({ "literal": lit }), "{lit}");
        }
        assert_eq!(
            book_with_date("January 10, 200 BCE"),
            json!({"date-parts": [[-200, 1, 10]]})
        );
        assert_eq!(
            book_with_date("Summer 429 BCE"),
            json!({"date-parts": [[-429]], "season": "Summer"})
        );
        assert_eq!(
            book_with_date("May 13, 2021"),
            json!({"date-parts": [["2021", 5, 13]]})
        );
        assert_eq!(
            book_with_date("\"1637 and 1662\""),
            json!({"literal": "1637 and 1662"})
        );
        assert_eq!(
            book_with_date("\u{201c}1637 and 1662\u{201d}"),
            json!({"literal": "1637 and 1662"})
        );
        assert_eq!(book_with_date("\"2021\""), json!({"literal": "2021"}));
    }

    /// utilities_itemTest.js:241-265: date variables in Extra.
    #[test]
    fn to_csl_extra_dates() {
        let mut item = ZoteroItem::new(ItemType::Book);
        item.set_field(Field::Extra, "original-date: \u{201c}1637 and 1662\u{201d}");
        let c = item_to_csl_json_with(&item, &opts()).unwrap();
        assert_eq!(c["original-date"], json!({"literal": "1637 and 1662"}));
        assert!(!c["note"].as_str().unwrap().contains("original-date"));

        let mut item = ZoteroItem::new(ItemType::Book);
        item.set_field(Field::Date, "1999");
        item.set_field(Field::Extra, "issued: 2021/2026\noriginal-date: 429 BCE");
        let c = item_to_csl_json_with(&item, &opts()).unwrap();
        assert_eq!(c["issued"], json!({"date-parts": [[2021], [2026]]}));
        assert_eq!(c["original-date"], json!({"date-parts": [[-429]]}));
        assert!(!c["note"].as_str().unwrap().contains("issued"));

        let mut item = ZoteroItem::new(ItemType::Book);
        item.set_field(Field::Extra, "issued: Spring 1995");
        let c = item_to_csl_json_with(&item, &opts()).unwrap();
        assert!(c.get("issued").is_none());
        assert!(c["note"].as_str().unwrap().contains("issued: Spring 1995"));
    }

    /// utilities_itemTest.js:266-277 and 87-110: standalone attachment and note.
    #[test]
    fn standalone_attachment_and_note() {
        let mut a = ZoteroItem::new(ItemType::Attachment);
        a.set_field(Field::Title, "Empty");
        a.set_field(Field::AccessDate, "2001-02-03 12:13:14");
        a.set_field(Field::Url, "http://example.com");
        a.note = Some("Note".into());
        let c = item_to_csl_json_with(&a, &opts()).unwrap();
        assert_eq!(c["type"], "document");
        assert_eq!(c["title"], "Empty");
        assert_eq!(c["accessed"], json!({"date-parts": [["2001", 2, 3]]}));
        let back = item_from_csl_json_with(&c, &opts()).unwrap();
        assert_eq!(back.field(Field::Title), Some("Empty"));

        let mut n = ZoteroItem::new(ItemType::Note);
        n.note = Some("Some note longer than 50 characters, which will become the title.".into());
        let c = item_to_csl_json_with(&n, &opts()).unwrap();
        assert_eq!(c["type"], "document");
        assert_eq!(c["title"], note_to_title(n.note.as_deref().unwrap(), false));
        let back = item_from_csl_json_with(&c, &opts()).unwrap();
        assert_eq!(back.field(Field::Title), c["title"].as_str());
    }

    /// utilities_itemTest.js:278-284: annotation (no CSL type) is refused.
    #[test]
    fn annotation_is_refused() {
        let a = ZoteroItem::new(ItemType::Annotation);
        assert_eq!(
            item_to_csl_json_with(&a, &opts()),
            Err(CslError::UnexpectedItemType(ItemType::Annotation))
        );
        assert_eq!(
            CslError::UnexpectedItemType(ItemType::Annotation).to_string(),
            "Unexpected Zotero Item type \"annotation\""
        );
    }

    /// utilities_itemTest.js:286-361: name particles.
    #[test]
    fn particles_in_creator_names() {
        let mut item = ZoteroItem::new(ItemType::JournalArticle);
        item.creators = vec![
            Creator::person(CreatorType::Author, "John", "Smith"),
            Creator::person(CreatorType::Author, "Jean de", "la Fontaine"),
            Creator::person(CreatorType::Author, "Vincent", "van Gogh"),
            Creator::person(CreatorType::Author, "Alexander von", "Humboldt"),
            Creator::single(CreatorType::Author, "Jean de la Fontaine"),
            Creator::person(CreatorType::Author, "Jean de", "\"la Fontaine\""),
        ];
        let c = item_to_csl_json_with(&item, &opts()).unwrap();
        let a = c["author"].as_array().unwrap();
        assert_eq!(a[0], json!({"given": "John", "family": "Smith"}));
        assert_eq!(
            a[1],
            json!({"given": "Jean", "dropping-particle": "de", "non-dropping-particle": "la", "family": "Fontaine"})
        );
        assert_eq!(
            a[2],
            json!({"given": "Vincent", "non-dropping-particle": "van", "family": "Gogh"})
        );
        assert_eq!(
            a[3],
            json!({"given": "Alexander", "dropping-particle": "von", "family": "Humboldt"})
        );
        assert_eq!(a[4], json!({"literal": "Jean de la Fontaine"}));
        assert_eq!(a[5], json!({"given": "Jean de", "family": "la Fontaine"}));
    }

    /// utilities_itemTest.js:363-382: UTC access date to local time.
    #[test]
    fn access_date_to_local_time() {
        let mut item = ZoteroItem::new(ItemType::Webpage);
        // 2019-01-09 00:00:00 in UTC+08:00 is 2019-01-08 16:00:00 UTC.
        item.set_field(Field::AccessDate, "2019-01-08T16:00:00Z");
        let sgt = DateOptions {
            utc_offset_minutes: 480,
            ..opts()
        };
        let c = item_to_csl_json_with(&item, &sgt).unwrap();
        assert_eq!(c["accessed"], json!({"date-parts": [["2019", 1, 9]]}));
        let est = DateOptions {
            utc_offset_minutes: -300,
            ..opts()
        };
        item.set_field(Field::AccessDate, "2019-01-10T04:59:59Z");
        let c = item_to_csl_json_with(&item, &est).unwrap();
        assert_eq!(c["accessed"], json!({"date-parts": [["2019", 1, 9]]}));
    }

    /// utilities_itemTest.js:384-391 and 118-142: Presentation Place <-> event-place.
    #[test]
    fn presentation_event_place() {
        let mut item = ZoteroItem::new(ItemType::Presentation);
        item.set_field(Field::Place, "New York");
        let c = item_to_csl_json_with(&item, &opts()).unwrap();
        assert_eq!(c["event-place"], "New York");
        assert!(c.get("publisher-place").is_none());
        let back = item_from_csl_json_with(&c, &opts()).unwrap();
        assert_eq!(back.field(Field::Place), Some("New York"));
        let mut c2 = c.clone();
        c2.remove("event-place");
        let back = item_from_csl_json_with(&c2, &opts()).unwrap();
        assert_eq!(back.field(Field::Place), None);
    }

    /// utilities_itemTest.js:394-414: noteToTitle.
    #[test]
    fn note_to_title_cases() {
        assert_eq!(note_to_title("<h1>Foo</h1><p>Bar</p>", true), "Foo");
        assert_eq!(
            note_to_title("<blockquote>\n<p>Foo</p>\n</blockquote>\n<p>Bar</p>", false),
            "Foo"
        );
        assert_eq!(
            note_to_title(
                "<h1>Annotations<br/>(2/18/2022, 3:49:43 AM)</h1><p>Foo</p>",
                true
            ),
            "Annotations"
        );
    }

    #[test]
    fn isbn_first_only() {
        assert_eq!(
            first_isbn("978-1-234-56789-7 9781234567897"),
            Some("978-1-234-56789-7")
        );
        assert_eq!(first_isbn("0-306-40615-2"), Some("0-306-40615-2"));
        assert_eq!(first_isbn("not an isbn"), None);
    }

    #[test]
    fn extra_to_csl_renames_fields() {
        assert_eq!(
            extra_to_csl("Original Date: 1900\nDOI: x"),
            "original-date: 1900\nDOI: x"
        );
        assert_eq!(extra_to_csl("Publication Title: J"), "container-title: J");
        assert_eq!(
            extra_to_csl("Archive Location: shelf"),
            "archive_location: shelf"
        );
        assert_eq!(extra_to_csl("Something Else: y"), "Something Else: y");
    }
}
