// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translate, https://github.com/zotero/translate
//   (commit e0fe482b8a07): src/translation/translate.js
//   `Zotero.Translate.Sandbox.Base._itemDone` :83-237, `_cleanTitle`
//   :1551-1556, `_cleanTags` :1563-1581.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! What the framework does to an item when a translator calls
//! `item.complete()`.

use super::item::{JsObject, TranslatorItem, TranslatorTag};
use super::js;
use kovan_common::zotero::date::{is_sql_date_time, sql_to_iso8601};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

/// Property names `_itemDone` allows to hold objects (:97-106). The typed
/// members of [`TranslatorItem`] are always objects; among `props` only
/// `relations` and `complete` can be.
const ALLOWED_OBJECTS: [&str; 7] = [
    "complete",
    "attachments",
    "creators",
    "tags",
    "notes",
    "relations",
    "seeAlso",
];

/// `_cleanTitle` (:1551-1556): drop a trailing ": a novel" from a book or
/// book section title.
pub fn clean_title(title: &str, item_type: &str) -> String {
    if item_type == "book" || item_type == "bookSection" {
        static R: OnceLock<Regex> = OnceLock::new();
        let re = R.get_or_init(|| {
            Regex::new(&format!(r"(?i){ws}*:{ws}*a novel{ws}*$", ws = js::WS)).unwrap()
        });
        return re.replace(title, "").into_owned();
    }
    title.to_owned()
}

/// `_cleanTags` (:1563-1581) on typed tags: drop empty ones, and a type
/// that is not truthy (`if(tag.type) newTag.type = tag.type`).
pub fn clean_tags(tags: &[TranslatorTag]) -> Vec<TranslatorTag> {
    tags.iter()
        .filter(|t| !t.tag.is_empty())
        .map(|t| TranslatorTag {
            tag: t.tag.clone(),
            tag_type: t.tag_type.filter(|ty| *ty != 0),
        })
        .collect()
}

/// `_cleanTags` on a JSON array (attachments' and notes' `tags`).
pub fn clean_tags_value(tags: &Value) -> Value {
    let Some(a) = tags.as_array() else {
        // `_cleanTags(tags)` on a non-array with no length gives [].
        return Value::Array(Vec::new());
    };
    let mut out = Vec::new();
    for t in a {
        if !js::truthy(Some(t)) {
            continue;
        }
        match t {
            Value::Object(m) => {
                let s = m
                    .get("tag")
                    .filter(|x| js::truthy(Some(x)))
                    .or_else(|| m.get("name").filter(|x| js::truthy(Some(x))));
                if let Some(s) = s {
                    let mut o = serde_json::Map::new();
                    o.insert("tag".into(), s.clone());
                    if let Some(ty) = m.get("type").filter(|x| js::truthy(Some(x))) {
                        o.insert("type".into(), ty.clone());
                    }
                    out.push(Value::Object(o));
                }
            }
            other => out.push(serde_json::json!({ "tag": js::to_js_string(other) })),
        }
    }
    Value::Array(out)
}

/// `_itemDone`'s normalisation of an item's properties (:108-131): drop
/// falsy values (keeping 0), trim strings, turn objects under names that
/// should not hold objects into strings and non-objects under names that
/// should into one-element arrays.
fn normalise_props(item: &mut TranslatorItem) {
    // `itemType` is the first property; a falsy one is dropped, a string trimmed.
    item.item_type = js::trim(&item.item_type).to_owned();
    let mut out = JsObject::new();
    for (k, v) in std::mem::take(&mut item.props).into_entries() {
        if !js::truthy(Some(&v)) && !matches!(&v, Value::Number(n) if n.as_f64() == Some(0.0)) {
            continue;
        }
        let is_object = matches!(v, Value::Array(_) | Value::Object(_));
        let should_be_object = ALLOWED_OBJECTS.contains(&k.as_str());
        let v = if is_object && !should_be_object {
            Value::String(js::to_js_string(&v))
        } else if should_be_object && !is_object {
            Value::Array(vec![v])
        } else if let Value::String(s) = &v {
            Value::String(js::trim(s).to_owned())
        } else {
            v
        };
        out.set(k, v);
    }
    item.props = out;
}

/// Port of `_itemDone` (:83-237) for a top-level import translation whose
/// items are saved (the translation-server passes `libraryID: 1`): returns
/// the item as the framework hands it to the item saver.
///
/// Omitted, because nothing observable depends on them: the random `id`
/// (:186; `itemToAPIJSON` drops it), the deprecation debug messages, and the
/// connector-only conversion of `attachment.document` (:165-169), which no
/// import translator sets.
pub fn item_done(mut item: TranslatorItem, in_child_translator: bool) -> TranslatorItem {
    normalise_props(&mut item);

    // :133-141
    if let Some(Value::String(title)) = item.props.get("title").cloned() {
        let cleaned = clean_title(&title, &item.item_type);
        item.props.set("title", cleaned.clone());
        if item.props.get_str("shortTitle") == Some(cleaned.as_str()) {
            item.props.remove("shortTitle");
        }
    }

    // :144-152: drop creators with neither name.
    item.creators.retain(|c| {
        c.first_name.as_deref().is_some_and(|s| !s.is_empty())
            || c.last_name.as_deref().is_some_and(|s| !s.is_empty())
    });

    // :155-157
    if !in_child_translator {
        item.tags = clean_tags(&item.tags);
    }

    // :159-176
    if !in_child_translator {
        for a in &mut item.attachments {
            if let Some(t) = a.get("tags").cloned() {
                a.set("tags", clean_tags_value(&t));
            }
        }
    }

    // :180-183: a child translator hands the item back as it is.
    if in_child_translator {
        return item;
    }

    // :188-203: notes (a falsy note is dropped; strings are already
    // `{note}` objects in the typed model).
    for n in &mut item.notes {
        if let Some(t) = n.props.get("tags").cloned() {
            n.props.set("tags", clean_tags_value(&t));
        }
    }

    // :205-208
    if let Some(v) = item.props.get("version").cloned() {
        if js::truthy(Some(&v)) {
            item.props.set("versionNumber", v);
        }
    }

    // :210-215
    if let Some(ad) = item.props.get_str("accessDate").map(str::to_owned) {
        if is_sql_date_time(&ad) {
            if let Some(iso) = sql_to_iso8601(&ad) {
                item.props.set("accessDate", iso);
            }
        }
    }

    item
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zotero::framework::item::TranslatorCreator;
    use serde_json::json;

    #[test]
    fn item_done_normalises_like_upstream() {
        let mut i = TranslatorItem::new("book");
        i.set("title", "  Moby Dick: A Novel ");
        i.set("shortTitle", "Moby Dick");
        i.set("volume", 0);
        i.set("pages", "");
        i.set("extra", json!(["a", "b"]));
        i.set("relations", "x");
        i.creators.push(TranslatorCreator::new("", "", "author"));
        i.creators.push(TranslatorCreator::single("Org", "author"));
        i.tags.push(TranslatorTag::new(""));
        i.tags.push(TranslatorTag {
            tag: "t".into(),
            tag_type: Some(0),
        });
        let o = item_done(i, false);
        assert_eq!(o.get_str("title"), Some("Moby Dick"));
        assert_eq!(o.get("shortTitle"), None);
        assert_eq!(o.get("volume"), Some(&json!(0)));
        assert_eq!(o.get("pages"), None);
        assert_eq!(o.get_str("extra"), Some("a,b"));
        assert_eq!(o.get("relations"), Some(&json!(["x"])));
        assert_eq!(o.creators.len(), 1);
        assert_eq!(o.tags, vec![TranslatorTag::new("t")]);
    }
}
