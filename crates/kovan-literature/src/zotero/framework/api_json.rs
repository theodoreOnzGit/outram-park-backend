// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero utilities, https://github.com/zotero/utilities (commit
//   1dd38e27edf8, the translation-server's submodule; the function is the
//   same at 4051881d59c6): utilities_item.js `itemToAPIJSON` :850-988;
//   cachedTypes.js `getFieldIDFromTypeAndBase` :154-171; utilities.js
//   `generateObjectKey` :1734-1736, `allowedKeyChars` :1729.
//   Zotero translation-server, https://github.com/zotero/translation-server
//   (commit 3a9d17614896): src/importEndpoint.js :42-47.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! Translator items to Zotero Web API JSON (`itemToAPIJSON`), which is what
//! the translation-server's `/import` returns and what
//! [`kovan_common::zotero::ZoteroItem`] reads.

use super::item::TranslatorItem;
use super::js;
use kovan_common::zotero::schema_generated::{CreatorType, Field, ItemType};
use kovan_common::zotero::schema::is_valid_for_type;
use serde_json::{json, Map, Value};

/// `Zotero.Utilities.allowedKeyChars`.
pub const ALLOWED_KEY_CHARS: &str = "23456789ABCDEFGHIJKLMNPQRSTUVWXYZ";

/// Where `itemToAPIJSON`'s item keys come from.
///
/// Upstream draws each key at random (`generateObjectKey`). This port is
/// deterministic: the n-th key is `"KVN"` followed by n written in base 33
/// over [`ALLOWED_KEY_CHARS`] (five digits), so the first is `KVN22222`.
/// The reference harness (`scripts/zotero-reference.mjs`, `normKey`)
/// rewrites upstream's random keys to the same sequence, in order of first
/// appearance, which is the only normalisation applied to upstream output.
/// A caller merging imports into one library should start each import at a
/// fresh index ([`super::TranslateOptions::first_key_index`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyGenerator {
    next: u64,
}

impl KeyGenerator {
    /// Start at key number `first`.
    pub fn new(first: u64) -> Self {
        KeyGenerator { next: first }
    }

    /// The n-th key.
    pub fn key(n: u64) -> String {
        let chars: Vec<char> = ALLOWED_KEY_CHARS.chars().collect();
        let mut n = n;
        let mut digits = Vec::with_capacity(5);
        for _ in 0..5 {
            digits.push(chars[(n % 33) as usize]);
            n /= 33;
        }
        digits.reverse();
        format!("KVN{}", digits.into_iter().collect::<String>())
    }

    /// The next key.
    pub fn next_key(&mut self) -> String {
        let k = Self::key(self.next);
        self.next += 1;
        k
    }
}

/// `Zotero.ItemFields.getFieldIDFromTypeAndBase` as the translation
/// framework implements it (cachedTypes.js:154-171): the type-specific field
/// of `item_type` that maps to the base field `base`, or `None`. Unlike the
/// Zotero client's version (kovan-common's
/// [`field_from_type_and_base`](kovan_common::zotero::schema::field_from_type_and_base)),
/// a base field the type uses directly does not map to itself, and a field
/// that is not a base field gives `None`.
pub fn type_field_for_base(item_type: ItemType, base: Field) -> Option<Field> {
    item_type
        .schema()
        .fields
        .iter()
        .find(|f| f.base_field == Some(base))
        .map(|f| f.field)
}

/// `itemToAPIJSON` (utilities_item.js:850-988): one translator item (after
/// `_itemDone`) as Web API JSON items, the item first and then one child
/// `note` item per note.
///
/// `now_iso` is what an `accessDate` of `"CURRENT_TIMESTAMP"` becomes
/// (upstream: the current time, :973-975).
pub fn item_to_api_json(
    item: &TranslatorItem,
    keys: &mut KeyGenerator,
    now_iso: &str,
) -> Vec<Value> {
    let key = keys.next_key();
    let mut new_item = Map::new();
    new_item.insert("key".into(), key.clone().into());
    new_item.insert("version".into(), 0.into());
    let mut children: Vec<Value> = Vec::new();

    // :857-862: an unknown type becomes `webpage`.
    let (type_name, item_type) = match ItemType::from_name(&item.item_type) {
        Some(t) => (item.item_type.clone(), t),
        None => ("webpage".to_owned(), ItemType::Webpage),
    };

    // `for (var field in item)`: the class members in declaration order
    // (itemType, creators, notes, tags, seeAlso, attachments), then the
    // translator's properties. seeAlso and attachments are skipped (:868).
    new_item.insert("itemType".into(), type_name.into());

    let mut creators = Vec::new();
    for c in &item.creators {
        let first = c.first_name.as_deref().unwrap_or("");
        let last = c.last_name.as_deref().unwrap_or("");
        if first.is_empty() && last.is_empty() {
            continue;
        }
        let mut nc = Map::new();
        if first.is_empty() || c.field_mode == Some(1) {
            // `name: creator.lastName` (undefined when only firstName is set
            // and fieldMode is 1; JSON.stringify then drops it).
            if let Some(l) = &c.last_name {
                nc.insert("name".into(), l.clone().into());
            }
        } else {
            nc.insert("firstName".into(), first.into());
            nc.insert(
                "lastName".into(),
                c.last_name.clone().map_or(Value::Null, Value::from),
            );
            if c.last_name.is_none() {
                // `lastName: undefined` is dropped by JSON.stringify.
                nc.remove("lastName");
            }
        }
        let ct = c
            .creator_type
            .as_deref()
            .filter(|t| !t.is_empty() && CreatorType::from_name(t).is_some())
            .unwrap_or("author");
        nc.insert("creatorType".into(), ct.into());
        creators.push(Value::Object(nc));
    }
    new_item.insert("creators".into(), Value::Array(creators));

    for n in &item.notes {
        if n.note.is_empty() {
            continue;
        }
        children.push(json!({
            "itemType": "note",
            "parentItem": key,
            "note": n.note,
        }));
    }

    let tags: Vec<Value> = item
        .tags
        .iter()
        .filter(|t| !t.tag.is_empty())
        .map(|t| json!({ "tag": t.tag, "type": 1 }))
        .collect();
    new_item.insert("tags".into(), Value::Array(tags));

    for (name, val) in item.props.iter() {
        if matches!(name, "complete" | "itemID" | "attachments" | "seeAlso") {
            continue;
        }
        let Some(field) = Field::from_name(name) else {
            continue; // unknown field (:983)
        };
        let val = match val {
            Value::String(s) => s.clone(),
            other if js::truthy(Some(other)) || other.as_f64() == Some(0.0) => {
                js::to_js_string(other)
            }
            _ => continue,
        };
        if let Some(type_field) = type_field_for_base(item_type, field) {
            let fname = type_field.as_str();
            if fname != name && !js::truthy(new_item.get(fname)) {
                new_item.insert(fname.into(), val.into());
            }
            continue;
        }
        if is_valid_for_type(field, item_type) {
            let val = if field == Field::AccessDate && val == "CURRENT_TIMESTAMP" {
                now_iso.to_owned()
            } else {
                val
            };
            new_item.insert(name.into(), val.into());
        }
    }

    let mut out = vec![Value::Object(new_item)];
    out.extend(children);
    out
}

/// Fold child notes into their parents' `notes` arrays, as Zotero's export
/// format holds them (`itemToExportFormat`): each item whose `itemType` is
/// `note` and whose `parentItem` names an earlier item in the list moves into
/// that item's `notes`. The reference harness does the same before posting
/// import output to `/export` (`foldChildNotes` in
/// `scripts/zotero-reference.mjs`).
pub fn fold_child_notes(items: &[Value]) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    let mut index_of_key: Vec<(String, usize)> = Vec::new();
    for it in items {
        let parent = it.get("parentItem").and_then(Value::as_str);
        if it.get("itemType").and_then(Value::as_str) == Some("note") {
            if let Some(p) = parent {
                if let Some((_, i)) = index_of_key.iter().find(|(k, _)| k == p) {
                    let target = &mut out[*i];
                    if !target.get("notes").is_some_and(Value::is_array) {
                        target["notes"] = Value::Array(Vec::new());
                    }
                    target["notes"].as_array_mut().unwrap().push(it.clone());
                    continue;
                }
            }
        }
        if let Some(k) = it.get("key").and_then(Value::as_str) {
            index_of_key.push((k.to_owned(), out.len()));
        }
        out.push(it.clone());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zotero::framework::item::{TranslatorCreator, TranslatorNote};

    #[test]
    fn key_sequence() {
        assert_eq!(KeyGenerator::key(0), "KVN22222");
        assert_eq!(KeyGenerator::key(1), "KVN22223");
        assert_eq!(KeyGenerator::key(33), "KVN22232");
    }

    #[test]
    fn base_fields_map_and_invalid_fields_drop() {
        let mut i = TranslatorItem::new("thesis");
        i.set("publisher", "U");
        i.set("university", "V");
        i.set("publicationTitle", "dropped");
        i.set("bogus", "dropped");
        i.creators
            .push(TranslatorCreator::new("A", "B", "nonsense"));
        i.notes.push(TranslatorNote::new("<p>n</p>"));
        let mut k = KeyGenerator::new(0);
        let v = item_to_api_json(&i, &mut k, "");
        // publisher maps to university first; the later `university` property
        // then overwrites it (it is not a base field, and valid for thesis).
        assert_eq!(v[0]["university"], "V");
        assert!(v[0].get("publisher").is_none());
        assert!(v[0].get("publicationTitle").is_none());
        assert_eq!(v[0]["creators"][0]["creatorType"], "author");
        assert_eq!(v[1]["parentItem"], "KVN22222");
        assert!(v[1].get("key").is_none());
    }
}
