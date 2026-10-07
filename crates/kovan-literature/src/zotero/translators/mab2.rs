// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): MAB2.js (translatorID
//   91acf493-0de7-4473-8b62-89fd141e6c74, lastUpdated 2014-05-20 17:57:47):
//   header :1-13, `detectImport` :15-21, cleaning functions :27-97,
//   `record` :99-321 (`importBinary` :110-149, `getField` :178-198,
//   `getFieldSubfields` :201-225, `_associateDBField` :228-257,
//   `_associateTags` :260-271, `translate` :274-319), `doImport` :323-352.
// Copyright: no notice upstream; translator by Simon Kornblith, adapted for
//   MAB2 by Leon Krauthausen (FUB) (header `creator`).
// Licence: AGPL-3.0 (no licence text upstream; treated as AGPLv3 as part of
//   Zotero, maintainer decision 2026-10-07, #747).

//! The MAB2 translator (Maschinelles Austauschformat für Bibliotheken, in
//! MARC-like binary records): import.
//!
//! Records are handled in UTF-16 code units, as upstream does: the directory
//! gives byte offsets, which upstream reconciles by padding each non-ASCII
//! code unit with NUL characters (one for U+0080..U+07FF, two above), and
//! `substr`/`length` count code units. The input is read in chunks of 4096
//! characters (`Zotero.read(4096)`), which the framework counts in `char`s
//! where upstream counts code units; the two differ only with astral-plane
//! characters, where a chunk boundary can fall in a different place.
//!
//! Not ported: `addField` (:153-175) and the `exports` object (:354-359),
//! which only another translator calling this one uses (no translator in
//! Zotero's repository does; MARC.js has its own `record`).
//!
//! **Maturity: AI draft (1).** Verified code-to-code against upstream
//! (`tests/zotero_translators.rs`, `mab2_import_matches_upstream`).

use crate::zotero::framework::options::{translator_type, TranslatorMetadata};
use crate::zotero::framework::utilities::clean_author;
use crate::zotero::framework::{
    js, ImportContext, TranslateError, TranslatorCreator, TranslatorItem, TranslatorTag,
};
use std::collections::HashMap;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "91acf493-0de7-4473-8b62-89fd141e6c74",
    label: "MAB2",
    creator: "Simon Kornblith. Adaptions for MAB2: Leon Krauthausen (FUB)",
    target: "mab2",
    min_version: "1.0.0b3.r1",
    priority: 100,
    translator_type: translator_type::IMPORT,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2014-05-20 17:57:47",
};

/// `fieldTerminator` (:23).
const FIELD_TERMINATOR: u16 = 0x1E;
/// `subfieldDelimiter` (:25).
const SUBFIELD_DELIMITER: u16 = 0x1F;

/// `detectImport` (:15-21): the first eight characters match
/// `/^[0-9]{3}[a-z ]{2}[a-z ]{3}$/`. (`Zotero.read(8)` at the end of the
/// input is `false`, tested as the string "false", which does not match.)
pub fn detect_import(ctx: &mut ImportContext) -> bool {
    let Some(read) = ctx.read_chars(8) else {
        return false;
    };
    let c: Vec<char> = read.chars().collect();
    c.len() == 8
        && c[..3].iter().all(char::is_ascii_digit)
        && c[3..].iter().all(|&x| x.is_ascii_lowercase() || x == ' ')
}

// ---------------------------------------------------------------------------
// JavaScript string and number semantics over UTF-16 code units.
// ---------------------------------------------------------------------------

type U16s = Vec<u16>;

fn utf16(s: &str) -> U16s {
    s.encode_utf16().collect()
}

fn from16(s: &[u16]) -> String {
    String::from_utf16_lossy(s)
}

/// `ToIntegerOrInfinity` of a number (NaN is 0).
fn to_int(n: f64) -> f64 {
    if n.is_nan() {
        0.0
    } else {
        n.trunc()
    }
}

/// `s.substr(start, length)` (Annex B), `length` `None` for undefined.
fn substr(s: &[u16], start: f64, length: Option<f64>) -> U16s {
    let size = s.len() as f64;
    let mut st = to_int(start);
    if st == f64::NEG_INFINITY {
        st = 0.0;
    } else if st < 0.0 {
        st = (size + st).max(0.0);
    }
    st = st.min(size);
    let len = match length {
        None => size,
        Some(l) => to_int(l),
    };
    let end = (st + len.max(0.0)).min(size);
    if end <= st {
        return Vec::new();
    }
    s[st as usize..end as usize].to_vec()
}

/// `parseInt(s, 10)` (NaN when there are no digits).
fn parse_int(s: &[u16]) -> f64 {
    let st = from16(s);
    let t = js::trim_start(&st);
    let (neg, rest) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let d: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if d.is_empty() {
        return f64::NAN;
    }
    let n: f64 = d.parse().unwrap_or(f64::NAN);
    if neg {
        -n
    } else {
        n
    }
}

/// A number as a property key (`String(n)`): integers without a decimal
/// point, NaN as "NaN".
fn key_of(n: f64) -> String {
    if n.is_nan() {
        "NaN".to_owned()
    } else if n.fract() == 0.0 && n.abs() < 1e21 {
        format!("{n:.0}")
    } else {
        n.to_string()
    }
}

// ---------------------------------------------------------------------------
// Cleaning functions (:27-97).
// ---------------------------------------------------------------------------

/// `[\s\.\,\/\:;]`
fn is_clean_char(c: char) -> bool {
    js::is_space(c) || matches!(c, '.' | ',' | '/' | ':' | ';')
}

/// Remove every run of two or more `ch` (`/<<+/g`, `/>>+/g`).
fn remove_runs(s: &str, ch: char) -> String {
    let mut out = String::new();
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < c.len() {
        if c[i] == ch && c.get(i + 1) == Some(&ch) {
            while i < c.len() && c[i] == ch {
                i += 1;
            }
            continue;
        }
        out.push(c[i]);
        i += 1;
    }
    out
}

/// `clean(value)` (:30-46).
fn clean(value: &str) -> String {
    let v = value.trim_start_matches(is_clean_char);
    let v = v.trim_end_matches(is_clean_char);
    let v = remove_runs(v, '<');
    let v = remove_runs(&v, '>');
    // `/ +/g` -> ' '
    let mut out = String::new();
    for c in v.chars() {
        if c == ' ' && out.ends_with(' ') {
            continue;
        }
        out.push(c);
    }
    let u = utf16(&out);
    let (c1, c2) = (u.first().copied(), u.last().copied());
    let br = |a: char, b: char| c1 == Some(a as u16) && c2 == Some(b as u16);
    if br('[', ']') || br('(', ')') {
        return from16(&substr(&u, 1.0, Some(u.len() as f64 - 2.0)));
    }
    out
}

/// `cleanTag(value)` (:48-52): `value.slice(0, value.indexOf('|'))`, which
/// drops the last character when there is no `|`.
fn clean_tag(value: &str) -> String {
    let u = utf16(value);
    let end = match u.iter().position(|&c| c == '|' as u16) {
        Some(i) => i,
        None => u.len().saturating_sub(1),
    };
    from16(&u[..end])
}

/// `pullNumber(text)` (:55-61): the first run of digits.
fn pull_number(text: &str) -> Option<String> {
    let start = text.find(|c: char| c.is_ascii_digit())?;
    let rest = &text[start..];
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    Some(rest[..end].to_owned())
}

/// `pullISBN(text)` (:64-70): the first run of `[0-9X\-]`.
fn pull_isbn(text: &str) -> Option<String> {
    let ok = |c: char| c.is_ascii_digit() || c == 'X' || c == '-';
    let start = text.find(ok)?;
    let rest = &text[start..];
    let end = rest.find(|c: char| !ok(c)).unwrap_or(rest.len());
    Some(rest[..end].to_owned())
}

/// `cleanAuthor` as a creator object.
fn creator(author: &str, ty: &str, use_comma: bool) -> TranslatorCreator {
    let a = clean_author(author, ty, use_comma);
    TranslatorCreator {
        first_name: a.first_name,
        last_name: Some(a.last_name),
        creator_type: Some(a.creator_type),
        ..Default::default()
    }
}

/// `authorMab(author, authType, useComma)` (:84-91): `[Hrsg.]`, `[Mitarb.]`
/// and `[Übers.]` (first occurrence each) become editor, contributor and
/// translator; no type is "author".
fn author_mab(author: &str, auth_type: Option<&str>, use_comma: bool) -> TranslatorCreator {
    let t = auth_type.filter(|t| !t.is_empty()).unwrap_or("author");
    let t = t.replacen("[Hrsg.]", "editor", 1);
    let t = t.replacen("[Mitarb.]", "contributor", 1);
    let t = t.replacen("[Übers.]", "translator", 1);
    creator(author, &t, use_comma)
}

// ---------------------------------------------------------------------------
// The record (:99-321).
// ---------------------------------------------------------------------------

/// A field's subfields (`returnFields[i]`): code -> value. Upstream builds a
/// plain object and only looks values up, so order does not matter.
type Subfields = HashMap<String, String>;

/// `record` (:99-107).
struct Record {
    /// `directory`: tag (as a property key) -> `[position, length]`s.
    directory: HashMap<String, Vec<(f64, f64)>>,
    leader: U16s,
    content: U16s,
    indicator_length: f64,
    subfield_code_length: f64,
}

/// What `_associateDBField` passes values through (`execMe`).
#[derive(Clone, Copy)]
enum Exec {
    None,
    AuthorMab,
    CorpAuthor,
    PullNumber,
    PullIsbn,
}

impl Record {
    /// `importBinary(record)` (:110-149).
    fn import_binary(record: &[u16]) -> Record {
        let ft = record.iter().position(|&c| c == FIELD_TERMINATOR);
        // `substr(0, indexOf(...))`: a length of -1 gives "".
        let directory = match ft {
            Some(i) => record[..i].to_vec(),
            None => Vec::new(),
        };
        let leader = substr(&directory, 0.0, Some(24.0));
        let directory = substr(&directory, 24.0, None);
        let at = |i: usize| leader.get(i).map_or(Vec::new(), |c| vec![*c]);
        let indicator_length = parse_int(&at(10));
        let subfield_code_length = parse_int(&at(11));
        let base_address = parse_int(&substr(&leader, 12.0, Some(5.0)));
        let content_tmp = substr(record, base_address, None);
        let mut content = Vec::with_capacity(content_tmp.len());
        for &c in &content_tmp {
            content.push(c);
            if c > 0x07FF {
                content.extend([0, 0]);
            } else if c > 0x007F {
                content.push(0);
            }
        }
        let mut dir: HashMap<String, Vec<(f64, f64)>> = HashMap::new();
        let mut i = 0;
        while i < directory.len() {
            let tag = parse_int(&substr(&directory, i as f64, Some(3.0)));
            let len = parse_int(&substr(&directory, i as f64 + 3.0, Some(4.0)));
            let pos = parse_int(&substr(&directory, i as f64 + 7.0, Some(5.0)));
            dir.entry(key_of(tag)).or_default().push((pos, len));
            i += 12;
        }
        Record {
            directory: dir,
            leader,
            content,
            indicator_length,
            subfield_code_length,
        }
    }

    /// `getField(field)` (:178-198): `[indicator, value]` of each occurrence.
    fn get_field(&self, field: &str) -> Vec<(U16s, U16s)> {
        let key = key_of(parse_int(&utf16(field)));
        let Some(locs) = self.directory.get(&key) else {
            return Vec::new();
        };
        locs.iter()
            .map(|&(pos, len)| {
                let ind = substr(&self.content, pos, Some(self.indicator_length));
                let val: U16s = substr(
                    &self.content,
                    pos + self.indicator_length,
                    Some(len - self.indicator_length - 1.0),
                )
                .into_iter()
                .filter(|&c| c != 0)
                .collect();
                (ind, val)
            })
            .collect()
    }

    /// `getFieldSubfields(tag)` (:201-225).
    fn get_field_subfields(&self, tag: &str) -> Vec<Subfields> {
        self.get_field(tag)
            .into_iter()
            .map(|(_, value)| {
                let mut r = Subfields::new();
                let parts: Vec<&[u16]> = value.split(|&c| c == SUBFIELD_DELIMITER).collect();
                if parts.len() == 1 {
                    r.insert("?".to_owned(), from16(&value));
                } else {
                    for p in parts {
                        if p.is_empty() {
                            continue;
                        }
                        let idx = from16(&substr(p, 0.0, Some(self.subfield_code_length - 1.0)));
                        if r.get(&idx).is_none_or(String::is_empty) {
                            let v = from16(&substr(p, self.subfield_code_length - 1.0, None));
                            r.insert(idx, v);
                        }
                    }
                }
                r
            })
            .collect()
    }

    /// `_associateDBField(item, fieldNo, part, fieldName, execMe, arg1,
    /// arg2)` (:228-257). `auth_type` is `arg1` for `authorMab`.
    fn associate_db_field(
        &self,
        item: &mut TranslatorItem,
        field_no: &str,
        part: &str,
        field_name: &str,
        exec: Exec,
        auth_type: Option<&str>,
    ) {
        for f in self.get_field_subfields(field_no) {
            let mut value: Option<String> = None;
            for p in part.chars() {
                if let Some(v) = f.get(&p.to_string()).filter(|v| !v.is_empty()) {
                    value = Some(match value {
                        Some(cur) => format!("{cur} {v}"),
                        None => v.clone(),
                    });
                }
            }
            let Some(v) = value else { continue };
            let v = clean(&v);
            if field_name == "creator" {
                let c = match exec {
                    Exec::AuthorMab => author_mab(&v, auth_type, true),
                    Exec::CorpAuthor => TranslatorCreator {
                        last_name: Some(v),
                        field_mode: Some(1),
                        ..Default::default()
                    },
                    _ => TranslatorCreator {
                        last_name: Some(v),
                        ..Default::default()
                    },
                };
                item.creators.push(c);
            } else {
                let v = match exec {
                    Exec::PullNumber => pull_number(&v),
                    Exec::PullIsbn => pull_isbn(&v),
                    _ => Some(v),
                };
                // `item[fieldName] = value; return;` (undefined from a pull
                // function leaves the field unset).
                match v {
                    Some(v) => item.set(field_name, v),
                    None => {
                        item.remove(field_name);
                    }
                }
                return;
            }
        }
    }

    /// `_associateTags(item, fieldNo, part)` (:260-271).
    fn associate_tags(&self, item: &mut TranslatorItem, field_no: &str, part: &str) {
        for f in self.get_field_subfields(field_no) {
            for p in part.chars() {
                if let Some(v) = f.get(&p.to_string()).filter(|v| !v.is_empty()) {
                    item.tags.push(TranslatorTag::new(clean_tag(v)));
                }
            }
        }
    }

    /// `translate(item)` (:274-319).
    fn translate(&self, item: &mut TranslatorItem) {
        item.item_type = if self.leader.is_empty() {
            "book".to_owned()
        } else {
            match self
                .leader
                .get(6)
                .map(|&c| char::from_u32(c.into()).unwrap_or('\0'))
            {
                Some('g') => "film",
                Some('k' | 'e' | 'f') => "artwork",
                Some('t') => "manuscript",
                _ => "book",
            }
            .to_owned()
        };

        for i in 100..=196 {
            let tag = i.to_string();
            let subs = self.get_field_subfields(&tag);
            if let Some(first) = subs.first() {
                let author_field = if first.get("a").is_some_and(|v| !v.is_empty()) {
                    "a"
                } else {
                    "p"
                };
                let auth_type = first.get("b").cloned();
                self.associate_db_field(
                    item,
                    &tag,
                    author_field,
                    "creator",
                    Exec::AuthorMab,
                    auth_type.as_deref(),
                );
            }
        }

        if !item.truthy("language") {
            self.associate_db_field(item, "037b", "a", "language", Exec::None, None);
        }
        self.associate_db_field(item, "200", "a", "creator", Exec::CorpAuthor, None);
        if !item.truthy("title") {
            self.associate_db_field(item, "331", "a", "title", Exec::None, None);
        }
        self.associate_db_field(item, "304", "a", "extra", Exec::None, None);
        if let Some(f335) = self.get_field_subfields("335").first() {
            let title = item
                .get_string("title")
                .unwrap_or_else(|| "undefined".to_owned());
            let sub = f335
                .get("a")
                .cloned()
                .unwrap_or_else(|| "undefined".to_owned());
            item.set("title", format!("{title}: {sub}"));
        }
        // `if (!item.x) this._associateDBField(...)` (only_if_unset) and the
        // unconditional extra fields, in upstream's order (:302-315).
        let pairs: [(&str, &str, &str, Exec, bool); 13] = [
            ("403", "a", "edition", Exec::None, true),
            ("410", "a", "place", Exec::None, true),
            ("412", "a", "publisher", Exec::None, true),
            ("1300", "a", "title", Exec::None, true),
            ("425", "a", "date", Exec::PullNumber, true),
            ("433", "a", "pages", Exec::PullNumber, true),
            ("451", "a", "series", Exec::None, true),
            ("501", "a", "extra", Exec::None, false),
            ("519", "a", "extra", Exec::None, false),
            ("523", "a", "edition", Exec::None, true),
            ("540", "a", "ISBN", Exec::PullIsbn, true),
            ("595", "a", "date", Exec::PullNumber, true),
            ("655e", "u", "url", Exec::None, true),
        ];
        for (no, part, name, exec, only_if_unset) in pairs {
            if only_if_unset && item.truthy(name) {
                continue;
            }
            self.associate_db_field(item, no, part, name, exec, None);
        }

        for no in [
            "902", "907", "912", "917", "922", "927", "932", "937", "942",
        ] {
            self.associate_tags(item, no, "acfgpkstz");
        }
    }
}

/// `doImport` (:323-352): records end at `\x1D`; the text after the last
/// one is never imported.
pub fn do_import(ctx: &mut ImportContext) -> Result<(), TranslateError> {
    let mut hold_over: U16s = Vec::new();
    while let Some(text) = ctx.read_chars(4096) {
        if text.is_empty() {
            break;
        }
        let text = utf16(&text);
        let mut records: Vec<U16s> = text.split(|&c| c == 0x1D).map(<[u16]>::to_vec).collect();
        if records.len() > 1 {
            let mut first = std::mem::take(&mut hold_over);
            first.extend_from_slice(&records[0]);
            records[0] = first;
            hold_over = records.pop().unwrap_or_default();
            for r in records {
                let mut item = TranslatorItem::default();
                Record::import_binary(&r).translate(&mut item);
                ctx.item_done(item);
            }
        } else {
            hold_over.extend_from_slice(&text);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleaning_like_upstream() {
        assert_eq!(
            clean("<<Die>> Kernreaktortechnik ;"),
            "Die Kernreaktortechnik"
        );
        assert_eq!(clean("(Ein Film)"), "Ein Film");
        assert_eq!(clean_tag("Geschichte"), "Geschicht");
        assert_eq!(clean_tag("Kernreaktor|(DE-588)"), "Kernreaktor");
        assert_eq!(pull_number("c1999"), Some("1999".into()));
        assert_eq!(
            pull_isbn("ISBN 3-540-12345-X kart."),
            Some("3-540-12345-X".into())
        );
    }
}
