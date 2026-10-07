// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: ECMA-262 `encodeURI` (19.2.6.4) and `for-in` key order; Zotero
//   utilities, https://github.com/zotero/utilities (commit 4051881d59c6):
//   utilities_item.js `itemToLegacyExportFormat` :1179-1240 (the order its
//   `uniqueFields` object gets its keys); cachedTypes.js
//   `getBaseIDFromTypeAndField`.
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
