// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: ECMA-262 `encodeURI` (19.2.6.4) and `for-in` key order; Zotero
//   utilities, https://github.com/zotero/utilities (commit 4051881d59c6):
//   utilities_item.js `itemToLegacyExportFormat` :1179-1240 (the order its
//   `uniqueFields` object gets its keys), utilities.js `cleanISBN` :532-568
//   and `cleanISSN` :603-627; cachedTypes.js `getBaseIDFromTypeAndField`.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! JavaScript behaviour the RDF translators lean on that the framework does
//! not already have.

use crate::zotero::framework::{JsObject, TranslatorItem};
use kovan_common::zotero::schema_generated::{Field, ItemType};
use serde_json::Value;

/// `encodeURI(s)`: percent-encode (UTF-8, upper-case hex) everything but
/// `A-Z a-z 0-9 ; , / ? : @ & = + $ - _ . ! ~ * ' ( ) #`.
pub fn encode_uri(s: &str) -> String {
    const KEEP: &str = ";,/?:@&=+$-_.!~*'()#";
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || KEEP.contains(c) {
            out.push(c);
        } else {
            let mut buf = [0u8; 4];
            for b in c.encode_utf8(&mut buf).bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

/// The names a JavaScript array or plain object inherits (`[]["map"]` is a
/// function, so `used[x]` is truthy for these keys).
pub const INHERITED_NAMES: [&str; 47] = [
    "constructor",
    "__defineGetter__",
    "__defineSetter__",
    "hasOwnProperty",
    "__lookupGetter__",
    "__lookupSetter__",
    "isPrototypeOf",
    "propertyIsEnumerable",
    "toString",
    "valueOf",
    "__proto__",
    "toLocaleString",
    "at",
    "concat",
    "copyWithin",
    "fill",
    "find",
    "findIndex",
    "findLast",
    "findLastIndex",
    "lastIndexOf",
    "pop",
    "push",
    "reverse",
    "shift",
    "unshift",
    "slice",
    "sort",
    "splice",
    "includes",
    "indexOf",
    "join",
    "keys",
    "entries",
    "values",
    "forEach",
    "filter",
    "flat",
    "flatMap",
    "map",
    "every",
    "some",
    "reduce",
    "reduceRight",
    "toReversed",
    "toSorted",
    "toSpliced",
];

/// The JavaScript property key a value becomes (`obj[v]`): `undefined` is
/// `"undefined"`.
pub fn js_key(v: Option<&Value>) -> String {
    match v {
        None => "undefined".to_owned(),
        Some(v) => crate::zotero::framework::js::to_js_string(v),
    }
}

/// Whether `s` is an array index (`for-in` visits these first, in numeric
/// order).
pub fn is_array_index(s: &str) -> bool {
    !s.is_empty()
        && s.bytes().all(|b| b.is_ascii_digit())
        && (s == "0" || !s.starts_with('0'))
        && s.parse::<u64>().is_ok_and(|n| n < u32::MAX as u64)
}

/// Keys in JavaScript `for-in` order: array indices ascending, then the
/// rest in insertion order.
pub fn js_key_order<'a>(keys: impl IntoIterator<Item = &'a str>) -> Vec<&'a str> {
    let keys: Vec<&str> = keys.into_iter().collect();
    let mut idx: Vec<&str> = keys.iter().copied().filter(|k| is_array_index(k)).collect();
    idx.sort_by_key(|k| k.parse::<u64>().unwrap_or(0));
    idx.extend(keys.iter().copied().filter(|k| !is_array_index(k)));
    idx
}

/// `getBaseIDFromTypeAndField` as the legacy conversion uses it (the same
/// rule as the framework's `export_items`): `Err` for an unknown field or
/// type (upstream throws and the loop skips the property), `Ok(None)` when
/// the field has no base field in the type.
fn base_for_type_and_field(item_type: &str, field: &str) -> Result<Option<Field>, ()> {
    let t = ItemType::from_name(item_type).ok_or(())?;
    let f = Field::from_name(field).ok_or(())?;
    Ok(t.schema()
        .fields
        .iter()
        .find(|x| x.field == f)
        .and_then(|x| x.base_field))
}

/// The keys of a legacy item's `uniqueFields` in the order upstream's object
/// holds them (insertion order), which `for (var p in item.uniqueFields)`
/// visits.
///
/// The framework keeps `uniqueFields` as a JSON object, whose keys
/// `serde_json` sorts, so the order is rebuilt by replaying the legacy
/// conversion's visit (`itemToLegacyExportFormat`, utilities_item.js
/// :1179-1240): it walks the item's properties in order and inserts each
/// valid field, or its base field when the type maps it to one (a key
/// inserted a second time keeps its first place); then `version`,
/// `mimeType` and `note` when it set them.
///
/// `props` should be the item as the caller gave it, before the conversion
/// (the export input object): the conversion removes `versionNumber` from the
/// item, so the converted item no longer shows where it was. Given the
/// converted item instead, the order is right except for a removed property.
pub fn unique_fields_order(
    item_type: &str,
    props: &JsObject,
    uf: &serde_json::Map<String, Value>,
) -> Vec<String> {
    let mut order: Vec<String> = Vec::new();
    let push = |k: &str, order: &mut Vec<String>| {
        if uf.contains_key(k) && !order.iter().any(|o| o == k) {
            order.push(k.to_owned());
        }
    };
    for (k, _) in props.iter() {
        match base_for_type_and_field(item_type, k) {
            Err(()) => {}
            Ok(Some(b)) if b.as_str() != k => push(b.as_str(), &mut order),
            Ok(_) => push(k, &mut order),
        }
    }
    for k in ["version", "mimeType", "note"] {
        push(k, &mut order);
    }
    // Anything else (not produced by the conversion): sorted, at the end.
    for k in uf.keys() {
        push(k, &mut order);
    }
    order
}

/// [`unique_fields_order`] of a [`TranslatorItem`], replayed on `original`
/// (the export input object it was made from) when given.
pub fn item_unique_fields_order(item: &TranslatorItem, original: Option<&JsObject>) -> Vec<String> {
    let Some(Value::Object(uf)) = item.props.get("uniqueFields") else {
        return Vec::new();
    };
    unique_fields_order(&item.item_type, original.unwrap_or(&item.props), uf)
}

fn is_dash(c: char) -> bool {
    matches!(
        c,
        '\u{2D}' | '\u{AD}' | '\u{2010}'..='\u{2015}' | '\u{2043}' | '\u{2212}'
    )
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn boundary(chars: &[char], i: usize) -> bool {
    let before = i > 0 && is_word(chars[i - 1]);
    let after = i < chars.len() && is_word(chars[i]);
    before != after
}

fn is_js_space(c: char) -> bool {
    crate::zotero::framework::js::is_space(c)
}

/// `(?:\d\s*){n}` then `last` at `p`: the end of the match.
fn digits_then(
    chars: &[char],
    mut i: usize,
    n: usize,
    last: impl Fn(char) -> bool,
) -> Option<usize> {
    for _ in 0..n {
        if !chars.get(i).is_some_and(|c| c.is_ascii_digit()) {
            return None;
        }
        i += 1;
        while i < chars.len() && is_js_space(chars[i]) {
            i += 1;
        }
    }
    match chars.get(i) {
        Some(&c) if last(c) => Some(i + 1),
        _ => None,
    }
}

fn digit(c: u8) -> u32 {
    (c - b'0') as u32
}

/// `Zotero.Utilities.cleanISBN(isbnStr)` (validating). Ported after
/// kovan-semantics' `zotero::text::clean_isbn` (the same upstream function;
/// kovan-literature cannot depend on that crate, which is not wasm-clean).
pub fn clean_isbn(isbn_str: &str) -> Option<String> {
    let chars: Vec<char> = isbn_str
        .to_uppercase()
        .chars()
        .filter(|c| !is_dash(*c))
        .collect();
    for p in 0..chars.len() {
        if !boundary(&chars, p) {
            continue;
        }
        // 97[89]\s*(?:\d\s*){9}\d  |  (?:\d\s*){9}[\dX]
        let thirteen = (|| {
            if chars.get(p) != Some(&'9') || chars.get(p + 1) != Some(&'7') {
                return None;
            }
            if !matches!(chars.get(p + 2), Some('8' | '9')) {
                return None;
            }
            let mut i = p + 3;
            while i < chars.len() && is_js_space(chars[i]) {
                i += 1;
            }
            digits_then(&chars, i, 9, |c| c.is_ascii_digit())
        })()
        .filter(|&e| boundary(&chars, e));
        let end = thirteen.or_else(|| {
            digits_then(&chars, p, 9, |c| c.is_ascii_digit() || c == 'X')
                .filter(|&e| boundary(&chars, e))
        });
        let Some(end) = end else { continue };
        let isbn: String = chars[p..end].iter().filter(|c| !is_js_space(**c)).collect();
        let b = isbn.as_bytes();
        let ok = if b.len() == 10 {
            let mut sum: u32 = (0..9).map(|i| digit(b[i]) * (10 - i as u32)).sum();
            sum += if b[9] == b'X' { 10 } else { digit(b[9]) };
            sum % 11 == 0
        } else {
            let mut sum: u32 = (0..12)
                .map(|i| digit(b[i]) * if i % 2 == 1 { 3 } else { 1 })
                .sum();
            sum += digit(b[12]);
            sum % 10 == 0
        };
        if ok {
            return Some(isbn);
        }
    }
    None
}

/// `Zotero.Utilities.cleanISSN(issnStr)`: the first `\b(?:\d\s*){7}[\dX]\b`
/// with a valid check digit, as `NNNN-NNNN`.
pub fn clean_issn(issn_str: &str) -> Option<String> {
    let chars: Vec<char> = issn_str
        .to_uppercase()
        .chars()
        .filter(|c| !is_dash(*c))
        .collect();
    for p in 0..chars.len() {
        if !boundary(&chars, p) {
            continue;
        }
        let Some(end) = digits_then(&chars, p, 7, |c| c.is_ascii_digit() || c == 'X')
            .filter(|&e| boundary(&chars, e))
        else {
            continue;
        };
        let issn: String = chars[p..end].iter().filter(|c| !is_js_space(**c)).collect();
        let b = issn.as_bytes();
        let mut sum: u32 = (0..7).map(|i| digit(b[i]) * (8 - i as u32)).sum();
        sum += if b[7] == b'X' { 10 } else { digit(b[7]) };
        if sum % 11 == 0 {
            return Some(format!("{}-{}", &issn[..4], &issn[4..]));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_uri_keeps_reserved() {
        assert_eq!(encode_uri("978-1 2"), "978-1%202");
        assert_eq!(encode_uri("a/b?c=d#e"), "a/b?c=d#e");
        assert_eq!(encode_uri("é"), "%C3%A9");
        assert_eq!(encode_uri("[x]"), "%5Bx%5D");
    }

    #[test]
    fn isbn_and_issn() {
        assert_eq!(
            clean_isbn("978-1-234-56789-7").as_deref(),
            Some("9781234567897")
        );
        assert_eq!(clean_isbn("0134685997").as_deref(), Some("0134685997"));
        assert_eq!(clean_isbn("0134685998"), None);
        assert_eq!(clean_issn("ISSN 1234-5679").as_deref(), Some("1234-5679"));
        assert_eq!(clean_issn("1234-5678"), None);
    }

    #[test]
    fn key_order() {
        assert_eq!(js_key_order(["b", "2", "a", "1"]), ["1", "2", "b", "a"]);
    }

    #[test]
    fn unique_fields_order_follows_the_conversion() {
        let o: JsObject = serde_json::from_str(
            r#"{"title":"T","university":"U","date":"2000","uniqueFields":{"date":"2000","publisher":"U","title":"T"},"publisher":"U"}"#,
        )
        .unwrap();
        let Some(Value::Object(uf)) = o.get("uniqueFields") else {
            panic!()
        };
        assert_eq!(
            unique_fields_order("thesis", &o, uf),
            ["title", "publisher", "date"]
        );
    }
}
