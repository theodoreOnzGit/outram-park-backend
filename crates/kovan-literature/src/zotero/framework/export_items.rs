// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translation-server,
//   https://github.com/zotero/translation-server (commit 3a9d17614896):
//   src/exportEndpoint.js :65-80 (the endpoint's emulation of
//   itemsToExportFormat), src/translation/translate_item.js `ItemGetter`
//   :35-86; Zotero utilities, https://github.com/zotero/utilities (commit
//   1dd38e27edf8, the server's submodule; the same function at
//   4051881d59c6): utilities_item.js `itemToLegacyExportFormat` :996-1076,
//   cachedTypes.js `getBaseIDFromTypeAndField` :173-181; Zotero translate
//   (commit e0fe482b8a07) translate.js `Sandbox.Export.nextItem` :798-808.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The items an export translator reads (`Zotero.nextItem()`), built from
//! the Web API / export-format JSON a caller supplies, the way the
//! translation-server's `/export` endpoint builds them.

use super::item::{JsObject, TranslatorItem};
use super::js;
use kovan_common::zotero::date::iso_to_sql;
use kovan_common::zotero::schema_generated::{Field, ItemType};
use serde_json::{Map, Value};

/// `Zotero.ItemFields.getBaseIDFromTypeAndField` as the framework
/// implements it (cachedTypes.js:173-181): `Err(())` when the field name or
/// item type is unknown (upstream throws), `Ok(None)` when the field has no
/// base mapping in the type, else the base field.
fn base_for_type_and_field(item_type: &str, field: &str) -> Result<Option<Field>, ()> {
    let t = ItemType::from_name(item_type).ok_or(())?;
    let f = Field::from_name(field).ok_or(())?;
    Ok(t.schema()
        .fields
        .iter()
        .find(|x| x.field == f)
        .and_then(|x| x.base_field))
}

/// `itemToLegacyExportFormat` (utilities_item.js:996-1076).
///
/// The random `itemID` and `key` it assigns (`randomString(6)`) are replaced
/// by deterministic values: `itemID` is overwritten by the item getter's
/// counter anyway (translate_item.js:76), and `key` is set to `legacy_key`.
pub fn item_to_legacy_export_format(item: &mut TranslatorItem, legacy_key: &str) {
    let mut unique = Map::new();
    // `item.uniqueFields = {}` is added before the loop, so it sits after the
    // existing properties and before any base field the loop adds; the loop
    // visits it (not a field: getBaseIDFromTypeAndField throws, skipped).
    let entries: Vec<(String, Value)> = item
        .props
        .iter()
        .map(|(k, v)| (k.to_owned(), v.clone()))
        .collect();
    item.props.set("uniqueFields", Value::Object(Map::new()));
    for (field, val) in entries {
        let Ok(base) = base_for_type_and_field(&item.item_type, &field) else {
            continue;
        };
        match base {
            Some(b) if b.as_str() != field => {
                item.props.set(b.as_str(), val.clone());
                unique.insert(b.as_str().to_owned(), val);
            }
            _ => {
                unique.insert(field, val);
            }
        }
    }
    item.props.set("uniqueFields", Value::Object(unique));

    item.props.set("itemID", "");
    item.props.set("key", legacy_key);

    item.props.remove("version");
    if let Some(vn) = item.props.get("versionNumber").cloned() {
        if js::truthy(Some(&vn)) {
            item.props.set("version", vn.clone());
            if let Some(Value::Object(u)) = item.props.get_mut("uniqueFields") {
                u.insert("version".into(), vn);
            }
            item.props.remove("versionNumber");
        }
    }

    for c in &mut item.creators {
        if let Some(name) = c.other.get("name").cloned() {
            if js::truthy(Some(&name)) {
                c.field_mode = Some(1);
                c.last_name = Some(js::to_js_string(&name));
                c.other.remove("name");
            }
        }
    }

    // `item.sourceItemKey = item.parentItem` (undefined when there is no
    // parent; then nothing observable is set).
    if let Some(p) = item.props.get("parentItem").cloned() {
        item.props.set("sourceItemKey", p);
    }

    for t in &mut item.tags {
        if t.tag_type.is_none_or(|ty| ty == 0) {
            t.tag_type = Some(0);
        }
    }

    item.see_also = Vec::new();

    if let Some(ct) = item.props.get("contentType").cloned() {
        if js::truthy(Some(&ct)) {
            item.props.set("mimeType", ct.clone());
            if let Some(Value::Object(u)) = item.props.get_mut("uniqueFields") {
                u.insert("mimeType".into(), ct);
            }
        }
    }

    if let Some(n) = item.props.get("note").cloned() {
        if js::truthy(Some(&n)) {
            if let Some(Value::Object(u)) = item.props.get_mut("uniqueFields") {
                u.insert("note".into(), n);
            }
        }
    }
}

/// The items an export translator will read, in order: the endpoint's
/// preparation (exportEndpoint.js:65-80), then `ItemGetter.nextItem`
/// (translate_item.js:50-78) and `Sandbox.Export.nextItem`
/// (translate.js:798-808), for every item.
///
/// * `legacy`: the translator's `minVersion` is below 4.0.27
///   ([`super::TranslatorMetadata::legacy_export`]); dates go to SQL form
///   and items to the legacy format. Upstream's endpoint, in legacy mode,
///   writes the SQL form of `dateModified` into `dateAdded` (:76); ported as
///   is.
/// * `export_tags`: the `exportTags` display option, when the translator
///   declares it; `Some(false)` empties every item's tags.
pub fn prepare_export_items(
    items: &[JsObject],
    legacy: bool,
    export_tags: Option<bool>,
) -> Vec<TranslatorItem> {
    let mut out = Vec::with_capacity(items.len());
    for (n, obj) in items.iter().enumerate() {
        let mut obj = obj.clone();
        // exportEndpoint.js:67-71: there is no library, so the key is the URI.
        if !obj.truthy("uri") {
            let key = obj.remove("key").unwrap_or(Value::Null);
            obj.set("uri", key);
        }
        if legacy {
            for (from, to) in [
                ("dateAdded", "dateAdded"),
                ("dateModified", "dateAdded"),
                ("accessDate", "accessDate"),
            ] {
                if let Some(s) = obj.get_str(from).map(str::to_owned) {
                    if !s.is_empty() {
                        // isoToSQL of a non-ISO string: isoToDate gives
                        // false and dateToSQL(false) catches its own error
                        // and returns "" (date.js:167-200, 245-254).
                        let sql = iso_to_sql(&s).unwrap_or_default();
                        obj.set(to, sql);
                    }
                }
            }
        }
        let mut item = TranslatorItem::from_js_object(&obj);
        // translate_item.js:53-55
        if legacy {
            item_to_legacy_export_format(&mut item, &legacy_key(n));
        }
        // :56-61: attachments and notes default to [] (typed members always
        // exist). :64-74: single-field creators.
        for c in &mut item.creators {
            if let Some(name) = c.other.get("name").cloned() {
                if js::truthy(Some(&name)) {
                    c.last_name = Some(js::to_js_string(&name));
                    c.first_name = Some(String::new());
                    c.other.remove("name");
                    c.field_mode = Some(1);
                }
            }
        }
        // :76: `itemID` counts from 1.
        item.props.set("itemID", (n + 1) as u64);
        if export_tags == Some(false) {
            item.tags.clear();
        }
        out.push(item);
    }
    out
}

/// The deterministic stand-in for the random 6-character `key`
/// `itemToLegacyExportFormat` assigns: `"KVN"` and the item's 1-based index,
/// zero-padded to three digits.
pub fn legacy_key(index: usize) -> String {
    format!("KVN{:03}", index + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_format_maps_base_fields_and_creators() {
        let o: JsObject = serde_json::from_str(
            r#"{"key":"ABCD2345","version":3,"itemType":"thesis","title":"T",
                "university":"U","creators":[{"creatorType":"author","name":"Org"}],
                "tags":[{"tag":"x"}],"dateAdded":"2020-01-02T03:04:05Z",
                "dateModified":"2021-01-02T03:04:05Z"}"#,
        )
        .unwrap();
        let items = prepare_export_items(&[o], true, None);
        let i = &items[0];
        assert_eq!(i.get_str("uri"), Some("ABCD2345"));
        assert_eq!(i.get_str("publisher"), Some("U"));
        assert_eq!(i.get("uniqueFields").unwrap()["publisher"], "U");
        assert_eq!(i.get("version"), None);
        // The endpoint's dateModified-into-dateAdded quirk.
        assert_eq!(i.get_str("dateAdded"), Some("2021-01-02 03:04:05"));
        assert_eq!(i.creators[0].last_name.as_deref(), Some("Org"));
        assert_eq!(i.creators[0].field_mode, Some(1));
        assert_eq!(i.tags[0].tag_type, Some(0));
        assert_eq!(i.get("itemID"), Some(&serde_json::json!(1)));
    }
}
