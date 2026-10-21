// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "MARC.js" (translatorID
//   a6ee60df-1ddc-4aae-bb25-45e0537be973, lastUpdated 2025-03-28 15:43:42):
//   the record model `record` :129-293 (`importBinary` :140-182, `addField`
//   :185-207, `getField` :210-230, `extractSubfields` :233-259,
//   `getFieldSubfields` :262-271) and the cleaning functions :59-127.
// Copyright (c) 2020 Simon Kornblith, Sylvain Machefert.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! MARC's record model (`marc.record` in upstream, which MARCXML gets with
//! `getTranslatorObject`), with JavaScript's string semantics.
//!
//! The record content is kept in UTF-16 code units, as upstream's string
//! is: `importBinary` pads every non-ASCII code unit with NULs so that the
//! directory's byte offsets line up (a code unit above U+07FF gets two NULs,
//! one above U+007F one; `charCodeAt` never exceeds U+FFFF, so an astral
//! character's two surrogates get two NULs each), and the NULs are removed
//! when a field is read. Numbers from `parseInt` are `f64` so that `NaN`
//! flows through `substr` as it does in JavaScript.

use crate::zotero::framework::js;
use crate::zotero::framework::xpath::js_number_to_string;
use regex::Regex;
use std::sync::OnceLock;

/// `fieldTerminator`.
pub const FIELD_TERMINATOR: char = '\x1E';
/// `recordTerminator`.
pub const RECORD_TERMINATOR: char = '\x1D';
/// `subfieldDelimiter`.
pub const SUBFIELD_DELIMITER: char = '\x1F';

pub(super) fn re(
    cell: &'static OnceLock<Regex>,
    pattern: impl FnOnce() -> String,
) -> &'static Regex {
    cell.get_or_init(|| Regex::new(&pattern()).expect("static regex compiles"))
}

/// JavaScript `parseInt(s)` (radix 10): leading whitespace, a sign, digits;
/// `NaN` when there are none.
pub fn js_parse_int(s: &str) -> f64 {
    let t = js::trim_start(s);
    let (neg, rest) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let d: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if d.is_empty() {
        return f64::NAN;
    }
    let v: f64 = d.parse().unwrap_or(f64::NAN);
    if neg {
        -v
    } else {
        v
    }
}

/// `String.prototype.substr(start, length)` over UTF-16 code units (`len`
/// `None` is `undefined`).
pub fn substr16(v: &[u16], start: f64, len: Option<f64>) -> Vec<u16> {
    let size = v.len() as f64;
    let mut s = if start.is_nan() { 0.0 } else { start.trunc() };
    if s < 0.0 {
        s = (size + s).max(0.0);
    }
    s = s.min(size);
    let l = match len {
        None => size - s,
        Some(l) if l.is_nan() => 0.0,
        Some(l) => l.trunc().max(0.0).min(size - s),
    };
    if l <= 0.0 {
        return Vec::new();
    }
    v[s as usize..(s + l) as usize].to_vec()
}

/// `substr` on a string.
pub fn substr(s: &str, start: f64, len: Option<f64>) -> String {
    let v: Vec<u16> = s.encode_utf16().collect();
    String::from_utf16_lossy(&substr16(&v, start, len))
}

/// JavaScript `length` of a string.
pub fn len16(s: &str) -> f64 {
    s.encode_utf16().count() as f64
}

/// The subfields of one field (`extractSubfields`' object), in insertion
/// order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Subfields(pub Vec<(String, String)>);

impl Subfields {
    /// `subfields[code]` (`None` is undefined).
    pub fn get(&self, code: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(k, _)| k == code)
            .map(|(_, v)| v.as_str())
    }

    /// JavaScript truthiness of `subfields[code]`.
    pub fn truthy(&self, code: &str) -> bool {
        self.get(code).is_some_and(|v| !v.is_empty())
    }

    /// `subfields[code] = value`.
    pub fn set(&mut self, code: &str, value: String) {
        match self.0.iter_mut().find(|(k, _)| k == code) {
            Some(x) => x.1 = value,
            None => self.0.push((code.to_owned(), value)),
        }
    }
}

/// A MARC record (`new marc.record()`, :129-137).
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    /// `directory`: tag (as JavaScript writes the number as a property
    /// name) -> `[position, length]` pairs.
    directory: Vec<(String, Vec<(f64, f64)>)>,
    /// `leader` (`None` is null, which MARCXML can set).
    pub leader: Option<String>,
    content: Vec<u16>,
    /// `indicatorLength`.
    pub indicator_length: f64,
    /// `subfieldCodeLength`.
    pub subfield_code_length: f64,
}

impl Default for Record {
    fn default() -> Self {
        Record::new()
    }
}

fn key(tag: f64) -> String {
    js_number_to_string(tag)
}

impl Record {
    /// `new record()`.
    pub fn new() -> Self {
        Record {
            directory: Vec::new(),
            leader: Some(String::new()),
            content: Vec::new(),
            indicator_length: 2.0,
            subfield_code_length: 2.0,
        }
    }

    fn dir_push(&mut self, tag: f64, loc: (f64, f64)) {
        let k = key(tag);
        match self.directory.iter_mut().find(|(t, _)| *t == k) {
            Some((_, v)) => v.push(loc),
            None => self.directory.push((k, vec![loc])),
        }
    }

    /// `importBinary(record)` (:140-182).
    pub fn import_binary(&mut self, record: &str) {
        let rec: Vec<u16> = record.encode_utf16().collect();
        let ft = FIELD_TERMINATOR as u16;
        let idx = rec.iter().position(|&c| c == ft).map_or(-1.0, |i| i as f64);
        let directory = substr16(&rec, 0.0, Some(idx));
        let leader = substr16(&directory, 0.0, Some(24.0));
        let directory = substr16(&directory, 24.0, None);
        let leader_s = String::from_utf16_lossy(&leader);
        self.indicator_length = js_parse_int(&substr(&leader_s, 10.0, Some(1.0)));
        self.subfield_code_length = js_parse_int(&substr(&leader_s, 11.0, Some(1.0)));
        let base_address = js_parse_int(&substr(&leader_s, 12.0, Some(5.0)));
        self.leader = Some(leader_s);
        let content_tmp = substr16(&rec, base_address, None);
        self.content = Vec::with_capacity(content_tmp.len());
        for &u in &content_tmp {
            self.content.push(u);
            if u > 0x07FF {
                self.content.extend([0, 0]);
            } else if u > 0x007F {
                self.content.push(0);
            }
        }
        let dir_s = String::from_utf16_lossy(&directory);
        let n = directory.len();
        let mut i = 0;
        while i < n {
            let at = |o: usize, l: f64| {
                js_parse_int(&String::from_utf16_lossy(&substr16(
                    &directory,
                    (i + o) as f64,
                    Some(l),
                )))
            };
            let _ = &dir_s;
            let tag = at(0, 3.0);
            let field_length = at(3, 4.0);
            let field_position = at(7, 5.0);
            self.dir_push(tag, (field_position, field_length));
            i += 12;
        }
    }

    /// `addField(field, indicator, value)` (:185-207); `field` as the
    /// string JavaScript's `parseInt` receives.
    pub fn add_field(&mut self, field: &str, indicator: &str, value: &str) {
        let tag = js_parse_int(field);
        let il = self.indicator_length;
        let mut indicator = indicator.to_owned();
        if len16(&indicator) > il {
            indicator = substr(&indicator, 0.0, Some(il));
        } else if len16(&indicator) != il {
            // Zotero.Utilities.lpad(indicator, " ", length)
            while len16(&indicator) < il {
                indicator = format!(" {indicator}");
            }
        }
        let value = format!("{indicator}{value}{FIELD_TERMINATOR}");
        let v: Vec<u16> = value.encode_utf16().collect();
        self.dir_push(tag, (self.content.len() as f64, v.len() as f64));
        self.content.extend(v);
    }

    /// `getField(field)` (:210-230): `[indicator, value]` of every field
    /// with this tag, NULs removed from the value.
    pub fn get_field(&self, field: &str) -> Vec<(String, String)> {
        let k = key(js_parse_int(field));
        let Some((_, locs)) = self.directory.iter().find(|(t, _)| *t == k) else {
            return Vec::new();
        };
        let il = self.indicator_length;
        locs.iter()
            .map(|&(pos, len)| {
                let ind = String::from_utf16_lossy(&substr16(&self.content, pos, Some(il)));
                let mut val = substr16(&self.content, pos + il, Some(len - il - 1.0));
                val.retain(|&u| u != 0);
                (ind, String::from_utf16_lossy(&val))
            })
            .collect()
    }

    /// `extractSubfields(fieldStr, tag)` (:233-259).
    pub fn extract_subfields(&self, field_str: &str) -> Subfields {
        let mut out = Subfields::default();
        let parts: Vec<&str> = field_str.split(SUBFIELD_DELIMITER).collect();
        if parts.len() == 1 {
            out.set("?", field_str.to_owned());
            return out;
        }
        let scl = self.subfield_code_length;
        for p in parts {
            if p.is_empty() {
                continue;
            }
            let idx = substr(p, 0.0, Some(scl - 1.0));
            let rest = substr(p, scl - 1.0, None);
            match out.get(&idx).map(str::to_owned) {
                None => out.set(&idx, rest),
                Some(prev) => out.set(&idx, format!("{prev} {rest}")),
            }
        }
        out
    }

    /// `getFieldSubfields(tag)` (:262-271).
    pub fn get_field_subfields(&self, tag: &str) -> Vec<Subfields> {
        self.get_field(tag)
            .iter()
            .map(|(_, v)| self.extract_subfields(v))
            .collect()
    }
}

/// `clean(value)` (:59-77): `None` (undefined) gives `None` (null).
pub fn clean(value: Option<&str>) -> Option<String> {
    static LEAD: OnceLock<Regex> = OnceLock::new();
    static TRAIL: OnceLock<Regex> = OnceLock::new();
    static SPACES: OnceLock<Regex> = OnceLock::new();
    let value = value?;
    let ws = &js::WS[1..js::WS.len() - 1];
    let v = re(&LEAD, || format!(r"^[{ws}.,/:;]+")).replace(value, "");
    let v = re(&TRAIL, || format!(r"[{ws}.,/:;]+$")).replace(&v, "");
    let v = re(&SPACES, || " +".into())
        .replace_all(&v, " ")
        .into_owned();
    let n = len16(&v);
    let c1 = substr(&v, 0.0, Some(1.0));
    let c2 = substr(&v, n - 1.0, None);
    if (c1 == "[" && c2 == "]") || (c1 == "(" && c2 == ")") {
        return Some(substr(&v, 1.0, Some(n - 2.0)));
    }
    Some(v)
}

/// `pullNumber(text)` (:80-87).
pub fn pull_number(text: &str) -> String {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, || "[0-9]+".into())
        .find(text)
        .map_or(String::new(), |m| m.as_str().to_owned())
}

/// `pullISBN(text)` (:90-97).
pub fn pull_isbn(text: &str) -> String {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, || "[0-9X-]+".into())
        .find(text)
        .map_or(String::new(), |m| m.as_str().to_owned())
}

/// `glueTogether(part1, part2, delimiter)` (:106-123), with JavaScript
/// truthiness (`None` and "" are falsy).
pub fn glue_together(
    part1: Option<String>,
    part2: Option<String>,
    delimiter: Option<&str>,
) -> Option<String> {
    static PUNCT: OnceLock<Regex> = OnceLock::new();
    let t = |p: &Option<String>| p.as_deref().is_some_and(|s| !s.is_empty());
    if !t(&part1) && !t(&part2) {
        return None;
    }
    if !t(&part2) {
        return part1;
    }
    if !t(&part1) {
        return part2;
    }
    let (p1, p2) = (part1.unwrap_or_default(), part2.unwrap_or_default());
    let Some(d) = delimiter.filter(|d| !d.is_empty()) else {
        return Some(format!("{p1} {p2}"));
    };
    if re(&PUNCT, || format!(r"[?:,.!;]{}*$", js::WS)).is_match(&p1) {
        return Some(format!("{p1} {p2}"));
    }
    Some(format!("{p1}{d}{p2}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_string_helpers() {
        assert!(js_parse_int("x").is_nan());
        assert_eq!(js_parse_int(" 012a"), 12.0);
        assert_eq!(substr("abc", f64::NAN, Some(2.0)), "ab");
        assert_eq!(substr("abc", -1.0, None), "c");
        assert_eq!(substr("abc", 1.0, Some(f64::NAN)), "");
        assert_eq!(clean(Some(" [Foo  bar] ;")).as_deref(), Some("Foo bar"));
        assert_eq!(
            glue_together(Some("A?".into()), Some("B".into()), Some(": ")).as_deref(),
            Some("A? B")
        );
    }

    #[test]
    fn add_and_read_fields() {
        let mut r = Record::new();
        r.add_field("245", "10", "\x1FaTitle\x1Fbsub");
        r.add_field("245", "1", "\x1Faé");
        let f = r.get_field_subfields("245");
        assert_eq!(f[0].get("a"), Some("Title"));
        assert_eq!(f[0].get("b"), Some("sub"));
        assert_eq!(r.get_field("245")[1].0, " 1");
        assert_eq!(f[1].get("a"), Some("é"));
    }
}
