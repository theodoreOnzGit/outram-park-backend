// Part of the kovan Zotero port (GitHub #747, #748).
//
// Upstream: Zotero, https://github.com/zotero/zotero (commit 9cbba8c4d281)
// and its schema, https://github.com/zotero/zotero-schema (commit b86c79b56479).
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (Zotero). zotero-schema: no licence text upstream; treated
// as AGPLv3 as part of Zotero, maintainer decision 2026-10-07, #747.
//
// This file holds no ported logic: it is the order-preserving JSON reader the
// generator needs, because Zotero iterates schema.json's objects in document
// order (JavaScript object key order) and that order is observable in the
// ported code (e.g. `itemFromCSLJSON` lets a later CSL variable overwrite an
// earlier one mapped to the same field; utilities_item.js:488).

//! An order-preserving JSON value, read through `serde_json`.
//!
//! `serde_json::Value` sorts object keys (its `Map` is a `BTreeMap` unless the
//! `preserve_order` feature is on, and turning that feature on would change
//! every crate in the workspace build through feature unification). This
//! type keeps object members as a `Vec` in document order instead.

use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

/// A JSON value whose objects keep their members in document order.
#[derive(Debug, Clone, PartialEq)]
pub enum OrderedJson {
    /// `null`.
    Null,
    /// `true` / `false`.
    Bool(bool),
    /// Any JSON number.
    Number(serde_json::Number),
    /// A string.
    String(String),
    /// An array.
    Array(Vec<OrderedJson>),
    /// An object, members in document order.
    Object(Vec<(String, OrderedJson)>),
}

impl OrderedJson {
    /// Parse JSON text, keeping object member order.
    pub fn parse(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }

    /// The member `key` of an object, or `None` (also for a non-object).
    pub fn get(&self, key: &str) -> Option<&OrderedJson> {
        match self {
            OrderedJson::Object(members) => members.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The members of an object, in document order.
    pub fn as_object(&self) -> Option<&[(String, OrderedJson)]> {
        match self {
            OrderedJson::Object(members) => Some(members),
            _ => None,
        }
    }

    /// The elements of an array.
    pub fn as_array(&self) -> Option<&[OrderedJson]> {
        match self {
            OrderedJson::Array(items) => Some(items),
            _ => None,
        }
    }

    /// The string, for a string value.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            OrderedJson::String(s) => Some(s),
            _ => None,
        }
    }

    /// The boolean, for a boolean value.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            OrderedJson::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The value as an unsigned integer, for a non-negative integer number.
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            OrderedJson::Number(n) => n.as_u64(),
            _ => None,
        }
    }
}

struct OrderedJsonVisitor;

impl<'de> Visitor<'de> for OrderedJsonVisitor {
    type Value = OrderedJson;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_unit<E>(self) -> Result<OrderedJson, E> {
        Ok(OrderedJson::Null)
    }

    fn visit_none<E>(self) -> Result<OrderedJson, E> {
        Ok(OrderedJson::Null)
    }

    fn visit_bool<E>(self, v: bool) -> Result<OrderedJson, E> {
        Ok(OrderedJson::Bool(v))
    }

    fn visit_i64<E>(self, v: i64) -> Result<OrderedJson, E> {
        Ok(OrderedJson::Number(v.into()))
    }

    fn visit_u64<E>(self, v: u64) -> Result<OrderedJson, E> {
        Ok(OrderedJson::Number(v.into()))
    }

    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<OrderedJson, E> {
        serde_json::Number::from_f64(v)
            .map(OrderedJson::Number)
            .ok_or_else(|| E::custom("non-finite number"))
    }

    fn visit_str<E>(self, v: &str) -> Result<OrderedJson, E> {
        Ok(OrderedJson::String(v.to_owned()))
    }

    fn visit_string<E>(self, v: String) -> Result<OrderedJson, E> {
        Ok(OrderedJson::String(v))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<OrderedJson, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element::<OrderedJson>()? {
            items.push(item);
        }
        Ok(OrderedJson::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<OrderedJson, A::Error> {
        let mut members: Vec<(String, OrderedJson)> = Vec::new();
        while let Some((key, value)) = map.next_entry::<String, OrderedJson>()? {
            // JavaScript's JSON.parse keeps the LAST duplicate, at the position
            // of the FIRST occurrence.
            if let Some(slot) = members.iter_mut().find(|(k, _)| *k == key) {
                slot.1 = value;
            } else {
                members.push((key, value));
            }
        }
        Ok(OrderedJson::Object(members))
    }
}

impl<'de> Deserialize<'de> for OrderedJson {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(OrderedJsonVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_member_order_is_document_order() {
        let v = OrderedJson::parse(r#"{"b": 1, "a": [true, null], "c": {"z": "x", "y": 2}}"#)
            .expect("parse");
        let keys: Vec<&str> = v
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        assert_eq!(keys, ["b", "a", "c"]);
        let inner: Vec<&str> = v
            .get("c")
            .unwrap()
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        assert_eq!(inner, ["z", "y"]);
    }

    #[test]
    fn duplicate_key_keeps_last_value_at_first_position() {
        let v = OrderedJson::parse(r#"{"a": 1, "b": 2, "a": 3}"#).expect("parse");
        let members = v.as_object().unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(members[0].0, "a");
        assert_eq!(members[0].1.as_u64(), Some(3));
    }
}
