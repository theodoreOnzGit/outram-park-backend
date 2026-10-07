// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translate, https://github.com/zotero/translate
//   (commit e0fe482b8a07, the translation-server's submodule):
//   src/translation/translate.js `_makeSandboxItem` :1929-1968 (the
//   `Zotero.Item` class translators construct), `_makeSandboxCollection`
//   :1970-1981.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The item a translator builds or reads: upstream's sandbox `Zotero.Item`.
//!
//! In JavaScript this is a plain object: the class declares `itemType`,
//! `creators`, `notes`, `tags`, `seeAlso` and `attachments` (in that order),
//! and translators then assign any property they like (`item.title`,
//! `item.backupPublisher`, `item.uniqueFields`, ...). [`TranslatorItem`]
//! keeps the six declared members typed and every other property, in
//! insertion order, in [`TranslatorItem::props`] as a JSON value — insertion
//! order matters, because the framework's `itemToAPIJSON` walks the
//! properties in that order and the first of two fields mapping to the same
//! slot wins.

use super::js;
use serde::de::{Deserialize, Deserializer, MapAccess, Visitor};
use serde_json::{Map, Value};
use std::fmt;

/// An ordered JavaScript object: properties in insertion order, values as
/// JSON. Setting an existing property keeps its position; removing it and
/// setting it again moves it to the end, as in JavaScript.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct JsObject(Vec<(String, Value)>);

impl JsObject {
    /// An empty object.
    pub fn new() -> Self {
        JsObject(Vec::new())
    }

    /// The value of a property (`None` is `undefined`).
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// A mutable reference to the value of a property.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.0.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// The value of a property when it is a string.
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    /// JavaScript truthiness of a property.
    pub fn truthy(&self, key: &str) -> bool {
        js::truthy(self.get(key))
    }

    /// Whether the property exists (`key in obj`).
    pub fn contains(&self, key: &str) -> bool {
        self.0.iter().any(|(k, _)| k == key)
    }

    /// Set a property (`obj[key] = value`).
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<Value>) {
        let key = key.into();
        let value = value.into();
        match self.0.iter_mut().find(|(k, _)| *k == key) {
            Some((_, v)) => *v = value,
            None => self.0.push((key, value)),
        }
    }

    /// Remove a property (`delete obj[key]`), returning its value.
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let i = self.0.iter().position(|(k, _)| k == key)?;
        Some(self.0.remove(i).1)
    }

    /// The properties in order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// The property names in order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|(k, _)| k.as_str())
    }

    /// The number of properties.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there are no properties.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The properties, consuming the object.
    pub fn into_entries(self) -> Vec<(String, Value)> {
        self.0
    }

    /// From a JSON object (whose keys `serde_json` keeps sorted; use
    /// [`JsObject`]'s `Deserialize` impl to keep a document's own order).
    pub fn from_map(map: &Map<String, Value>) -> Self {
        JsObject(map.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
    }

    /// As a JSON object (key order is lost: `serde_json` sorts).
    pub fn to_value(&self) -> Value {
        Value::Object(self.0.iter().cloned().collect())
    }
}

impl FromIterator<(String, Value)> for JsObject {
    fn from_iter<T: IntoIterator<Item = (String, Value)>>(iter: T) -> Self {
        let mut o = JsObject::new();
        for (k, v) in iter {
            o.set(k, v);
        }
        o
    }
}

/// Deserialises a JSON object keeping its key order (what `JSON.parse`
/// does), with nested values as ordinary JSON.
impl<'de> Deserialize<'de> for JsObject {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = JsObject;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a JSON object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<JsObject, A::Error> {
                let mut o = JsObject::new();
                while let Some((k, v)) = m.next_entry::<String, Value>()? {
                    o.set(k, v);
                }
                Ok(o)
            }
        }
        d.deserialize_map(V)
    }
}

/// A creator as translators handle it: `{firstName, lastName, creatorType,
/// fieldMode}`; anything else a translator sets (e.g. `creatorTypeID` from
/// `itemFromCSLJSON`) is kept in `other`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TranslatorCreator {
    /// `firstName`.
    pub first_name: Option<String>,
    /// `lastName`.
    pub last_name: Option<String>,
    /// `creatorType`.
    pub creator_type: Option<String>,
    /// `fieldMode` (1 = single-field name).
    pub field_mode: Option<i64>,
    /// Other properties, in order.
    pub other: JsObject,
}

impl TranslatorCreator {
    /// A two-field creator.
    pub fn new(
        first_name: impl Into<String>,
        last_name: impl Into<String>,
        creator_type: impl Into<String>,
    ) -> Self {
        TranslatorCreator {
            first_name: Some(first_name.into()),
            last_name: Some(last_name.into()),
            creator_type: Some(creator_type.into()),
            ..Default::default()
        }
    }

    /// A single-field creator (`fieldMode: 1`).
    pub fn single(last_name: impl Into<String>, creator_type: impl Into<String>) -> Self {
        TranslatorCreator {
            last_name: Some(last_name.into()),
            creator_type: Some(creator_type.into()),
            field_mode: Some(1),
            ..Default::default()
        }
    }

    /// From a JSON creator object (`name` is kept in `other`; the export
    /// item getter turns it into `lastName`).
    pub fn from_value(v: &Value) -> Self {
        let mut c = TranslatorCreator::default();
        if let Value::Object(m) = v {
            for (k, val) in m {
                match k.as_str() {
                    "firstName" => c.first_name = Some(js::to_js_string(val)),
                    "lastName" => c.last_name = Some(js::to_js_string(val)),
                    "creatorType" => c.creator_type = Some(js::to_js_string(val)),
                    "fieldMode" => c.field_mode = val.as_i64(),
                    _ => c.other.set(k.clone(), val.clone()),
                }
            }
        }
        c
    }

    /// As a JSON object.
    pub fn to_value(&self) -> Value {
        let mut m = Map::new();
        if let Some(s) = &self.first_name {
            m.insert("firstName".into(), s.clone().into());
        }
        if let Some(s) = &self.last_name {
            m.insert("lastName".into(), s.clone().into());
        }
        if let Some(s) = &self.creator_type {
            m.insert("creatorType".into(), s.clone().into());
        }
        if let Some(f) = self.field_mode {
            m.insert("fieldMode".into(), f.into());
        }
        for (k, v) in self.other.iter() {
            m.insert(k.to_owned(), v.clone());
        }
        Value::Object(m)
    }
}

/// A tag: what a translator pushes (a string, or `{tag, type}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslatorTag {
    /// The tag text.
    pub tag: String,
    /// `type`: 0 manual, 1 automatic; `None` when not given.
    pub tag_type: Option<i64>,
}

impl TranslatorTag {
    /// A tag with no type (a translator pushing a plain string).
    pub fn new(tag: impl Into<String>) -> Self {
        TranslatorTag {
            tag: tag.into(),
            tag_type: None,
        }
    }

    /// From a JSON tag (a string, or an object with `tag` or `name`).
    pub fn from_value(v: &Value) -> Option<Self> {
        match v {
            Value::String(s) => Some(TranslatorTag::new(s.clone())),
            Value::Object(m) => {
                let tag = m
                    .get("tag")
                    .filter(|t| js::truthy(Some(t)))
                    .or_else(|| m.get("name"))
                    .map(js::to_js_string)?;
                Some(TranslatorTag {
                    tag,
                    tag_type: m.get("type").and_then(Value::as_i64),
                })
            }
            Value::Null => None,
            other => Some(TranslatorTag::new(js::to_js_string(other))),
        }
    }
}

/// A note: `note` is its HTML; on export, the rest of the child note's JSON
/// (`key`, `tags`, ...) is in `props`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TranslatorNote {
    /// The note's HTML.
    pub note: String,
    /// Other properties, in order.
    pub props: JsObject,
}

impl TranslatorNote {
    /// A note with only text.
    pub fn new(note: impl Into<String>) -> Self {
        TranslatorNote {
            note: note.into(),
            props: JsObject::new(),
        }
    }
}

/// A translator's item (upstream's sandbox `Zotero.Item`; module docs).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TranslatorItem {
    /// `itemType` (a string: translators may set a type the schema lacks;
    /// `itemToAPIJSON` turns an unknown one into `webpage`).
    pub item_type: String,
    /// `creators`.
    pub creators: Vec<TranslatorCreator>,
    /// `notes`.
    pub notes: Vec<TranslatorNote>,
    /// `tags`.
    pub tags: Vec<TranslatorTag>,
    /// `seeAlso`.
    pub see_also: Vec<Value>,
    /// `attachments`, each a plain object (`{path, mimeType, title, url, ...}`).
    pub attachments: Vec<JsObject>,
    /// Every other property, in insertion order.
    pub props: JsObject,
}

impl TranslatorItem {
    /// `new Zotero.Item(itemType)`.
    pub fn new(item_type: impl Into<String>) -> Self {
        TranslatorItem {
            item_type: item_type.into(),
            ..Default::default()
        }
    }

    /// A property's value.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.props.get(key)
    }

    /// A property's value when it is a string.
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.props.get_str(key)
    }

    /// A property as JavaScript would concatenate it (`"" + item[key]`), or
    /// `None` when undefined.
    pub fn get_string(&self, key: &str) -> Option<String> {
        self.props.get(key).map(js::to_js_string)
    }

    /// JavaScript truthiness of a property.
    pub fn truthy(&self, key: &str) -> bool {
        self.props.truthy(key)
    }

    /// `item[key] = value`.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<Value>) {
        self.props.set(key, value)
    }

    /// `delete item[key]`.
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        self.props.remove(key)
    }

    /// `Zotero.Item#setExtra(field, value)` (translate.js:1952-1962): replace
    /// the `field: ` line of Extra, or append one.
    pub fn set_extra(&mut self, field: &str, value: &str) {
        let extra = self.get_string("extra").unwrap_or_default();
        let mut lines: Vec<String> = extra.split('\n').map(str::to_owned).collect();
        let prefix = format!("{field}: ");
        let line = format!("{field}: {value}");
        match lines.iter().position(|l| l.starts_with(&prefix)) {
            Some(i) => lines[i] = line,
            None => lines.push(line),
        }
        self.set("extra", lines.join("\n"));
    }

    /// From a JSON object in its key order (an item of the translator export
    /// format or the Web API format).
    pub fn from_js_object(o: &JsObject) -> Self {
        let mut item = TranslatorItem::default();
        for (k, v) in o.iter() {
            match k {
                "itemType" => item.item_type = js::to_js_string(v),
                "creators" => {
                    item.creators = v
                        .as_array()
                        .map(|a| a.iter().map(TranslatorCreator::from_value).collect())
                        .unwrap_or_default()
                }
                "tags" => {
                    item.tags = v
                        .as_array()
                        .map(|a| a.iter().filter_map(TranslatorTag::from_value).collect())
                        .unwrap_or_default()
                }
                "notes" => {
                    item.notes = v
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .map(|n| match n {
                                    Value::Object(m) => {
                                        let mut props = JsObject::from_map(m);
                                        let note = props
                                            .remove("note")
                                            .map(|x| js::to_js_string(&x))
                                            .unwrap_or_default();
                                        TranslatorNote { note, props }
                                    }
                                    other => TranslatorNote::new(js::to_js_string(other)),
                                })
                                .collect()
                        })
                        .unwrap_or_default()
                }
                "attachments" => {
                    item.attachments = v
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .map(|x| match x {
                                    Value::Object(m) => JsObject::from_map(m),
                                    _ => JsObject::new(),
                                })
                                .collect()
                        })
                        .unwrap_or_default()
                }
                "seeAlso" => item.see_also = v.as_array().cloned().unwrap_or_default(),
                _ => item.props.set(k, v.clone()),
            }
        }
        item
    }

    /// As a JSON object in translator format (the shape of upstream's test
    /// cases' `items`).
    pub fn to_value(&self) -> Value {
        let mut m = Map::new();
        m.insert("itemType".into(), self.item_type.clone().into());
        m.insert(
            "creators".into(),
            Value::Array(
                self.creators
                    .iter()
                    .map(TranslatorCreator::to_value)
                    .collect(),
            ),
        );
        m.insert(
            "notes".into(),
            Value::Array(
                self.notes
                    .iter()
                    .map(|n| {
                        let mut o = n.props.to_value();
                        o["note"] = n.note.clone().into();
                        o
                    })
                    .collect(),
            ),
        );
        m.insert(
            "tags".into(),
            Value::Array(
                self.tags
                    .iter()
                    .map(|t| {
                        let mut o = Map::new();
                        o.insert("tag".into(), t.tag.clone().into());
                        if let Some(ty) = t.tag_type {
                            o.insert("type".into(), ty.into());
                        }
                        Value::Object(o)
                    })
                    .collect(),
            ),
        );
        m.insert("seeAlso".into(), Value::Array(self.see_also.clone()));
        m.insert(
            "attachments".into(),
            Value::Array(self.attachments.iter().map(JsObject::to_value).collect()),
        );
        for (k, v) in self.props.iter() {
            m.insert(k.to_owned(), v.clone());
        }
        Value::Object(m)
    }
}

/// One member of a [`TranslatorCollection`].
#[derive(Debug, Clone, PartialEq)]
pub enum CollectionChild {
    /// `{type: 'item', id}`: an item, by the id the translator gave it.
    Item {
        /// The item's id (a BibTeX citation key, for instance).
        id: String,
    },
    /// A subcollection.
    Collection(TranslatorCollection),
}

/// A collection a translator builds (`new Zotero.Collection()`,
/// translate.js:1970-1981). The translation-server discards collections
/// (its `ItemSaver.saveCollection` is a no-op); they are returned here so a
/// caller can keep them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TranslatorCollection {
    /// `name`.
    pub name: String,
    /// `children`, in order.
    pub children: Vec<CollectionChild>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn js_object_keeps_order_and_js_set_semantics() {
        let o: JsObject = serde_json::from_str(r#"{"z": 1, "a": 2, "m": 3}"#).unwrap();
        assert_eq!(o.keys().collect::<Vec<_>>(), ["z", "a", "m"]);
        let mut o = o;
        o.set("z", 5);
        assert_eq!(o.keys().collect::<Vec<_>>(), ["z", "a", "m"]);
        o.remove("z");
        o.set("z", 6);
        assert_eq!(o.keys().collect::<Vec<_>>(), ["a", "m", "z"]);
        assert_eq!(o.get("z"), Some(&json!(6)));
    }

    #[test]
    fn set_extra_replaces_or_appends() {
        let mut i = TranslatorItem::new("book");
        i.set_extra("DOI", "10.1/x");
        // String(undefined || '') is '', split gives [''], so the first line is empty.
        assert_eq!(i.get_str("extra"), Some("\nDOI: 10.1/x"));
        i.set_extra("DOI", "10.1/y");
        assert_eq!(i.get_str("extra"), Some("\nDOI: 10.1/y"));
    }
}
