// Part of the kovan Zotero port (GitHub #747, #750).
//
// Upstream: Zotero, https://github.com/zotero/zotero (commit 9cbba8c4d281):
//   chrome/content/zotero/xpcom/data/search.js (`Zotero.Search#toJSON` :893,
//   `fromJSON` :848) and data/dataObject.js (`_postToJSON` :1565).
// Copyright (c) 2009 Center for History and New Media, George Mason
// University; (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! A Zotero saved search, in the JSON shape of `Zotero.Search#toJSON`
//! (search.js:893): `{key, version, name, conditions: [{condition, operator,
//! value}], deleted?}`.
//!
//! The conditions are **kept as stored**, not evaluated: running a saved
//! search needs Zotero's search engine (search.js `_buildQuery`, ~1500 lines
//! of SQL generation) and is not part of this port. A condition's
//! `condition` string carries its mode after a slash (`"title/any"`), exactly
//! as `toJSON` writes it.
//!
//! Added 2026-10-07 for the Zotero database reader (#750); purely additive to
//! the #748 model.

use super::item::ZoteroJsonError;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// One condition of a saved search, as `toJSON` writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchCondition {
    /// `condition[/mode]`, e.g. `title`, `collection`, `fulltextContent/phraseBinary`.
    pub condition: String,
    /// The operator, e.g. `is`, `contains`, `isNot`; `None` when stored as
    /// NULL (upstream writes it through as `null`).
    pub operator: Option<String>,
    /// The value; `""` when none (`toJSON` writes `value || ""`).
    pub value: String,
}

/// A Zotero saved search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoteroSearch {
    /// The 8-character object key.
    pub key: Option<String>,
    /// The object version.
    pub version: Option<u64>,
    /// The search's name.
    pub name: String,
    /// The conditions, in order.
    pub conditions: Vec<SearchCondition>,
    /// `deleted` (in the trash); written only when true, as `_postToJSON` does.
    pub deleted: Option<bool>,
    /// Every other property, verbatim.
    pub other: BTreeMap<String, Value>,
}

impl ZoteroSearch {
    /// A search with this name and no conditions.
    pub fn new(name: impl Into<String>) -> Self {
        ZoteroSearch {
            key: None,
            version: None,
            name: name.into(),
            conditions: Vec::new(),
            deleted: None,
            other: BTreeMap::new(),
        }
    }

    /// Parse search JSON (lenient: unknown properties go to `other`).
    pub fn from_json_value(v: &Value) -> Result<Self, ZoteroJsonError> {
        let o = v.as_object().ok_or(ZoteroJsonError::NotAnObject)?;
        let bad = |p: &str| ZoteroJsonError::BadProperty(p.to_owned());
        let name = o
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| bad("name"))?;
        let mut s = ZoteroSearch::new(name);
        for (k, val) in o {
            match k.as_str() {
                "name" => {}
                "key" => s.key = val.as_str().map(str::to_owned),
                "version" => s.version = val.as_u64(),
                "deleted" => s.deleted = val.as_bool(),
                "conditions" => {
                    let arr = val.as_array().ok_or_else(|| bad("conditions"))?;
                    for c in arr {
                        let c = c.as_object().ok_or_else(|| bad("conditions[]"))?;
                        let condition = c
                            .get("condition")
                            .and_then(Value::as_str)
                            .ok_or_else(|| bad("conditions[].condition"))?
                            .to_owned();
                        let operator = c.get("operator").and_then(Value::as_str).map(str::to_owned);
                        let value = match c.get("value") {
                            Some(Value::String(s)) => s.clone(),
                            Some(Value::Number(n)) => n.to_string(),
                            _ => String::new(),
                        };
                        s.conditions.push(SearchCondition {
                            condition,
                            operator,
                            value,
                        });
                    }
                }
                _ => {
                    s.other.insert(k.clone(), val.clone());
                }
            }
        }
        Ok(s)
    }

    /// Write search JSON as `toJSON` does.
    pub fn to_json_value(&self) -> Value {
        let mut o = Map::new();
        if let Some(k) = &self.key {
            o.insert("key".into(), k.clone().into());
        }
        if let Some(v) = self.version {
            o.insert("version".into(), v.into());
        }
        o.insert("name".into(), self.name.clone().into());
        o.insert(
            "conditions".into(),
            Value::Array(
                self.conditions
                    .iter()
                    .map(|c| {
                        let mut m = Map::new();
                        m.insert("condition".into(), c.condition.clone().into());
                        m.insert(
                            "operator".into(),
                            c.operator.clone().map(Value::String).unwrap_or(Value::Null),
                        );
                        m.insert("value".into(), c.value.clone().into());
                        Value::Object(m)
                    })
                    .collect(),
            ),
        );
        if self.deleted == Some(true) {
            o.insert("deleted".into(), true.into());
        }
        for (k, v) in &self.other {
            o.insert(k.clone(), v.clone());
        }
        Value::Object(o)
    }
}

impl serde::Serialize for ZoteroSearch {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.to_json_value().serialize(s)
    }
}

impl<'de> serde::Deserialize<'de> for ZoteroSearch {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Value::deserialize(d)?;
        ZoteroSearch::from_json_value(&v).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The JSON of a saved search round-trips, and the shape is
    /// search.js:893's (`operator` and `value` always present).
    #[test]
    fn search_json_round_trip() {
        let v: Value = serde_json::from_str(
            r#"{"key":"ABCD2345","version":3,"name":"Reactors",
                "conditions":[{"condition":"title","operator":"contains","value":"HTR"},
                              {"condition":"joinMode","operator":"any","value":""}]}"#,
        )
        .unwrap();
        let s = ZoteroSearch::from_json_value(&v).unwrap();
        assert_eq!(s.conditions.len(), 2);
        assert_eq!(s.to_json_value(), v);
    }
}
