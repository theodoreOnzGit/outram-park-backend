// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translate, https://github.com/zotero/translate
//   (commit e0fe482b8a07): src/translation/translate.js — the import
//   sandbox (`Zotero.read`, `Zotero.Item#complete` -> `_itemDone` :83,
//   `Zotero.Collection#complete` -> `_collectionDone` :767, `getOption`
//   :245, `getHiddenPref` :260), the export sandbox (`Zotero.nextItem`
//   :798, `Zotero.write`), and `translate()`'s completion: items saved in
//   order (`_saveItems` :1587, `complete` :1449); translation-server
//   (commit 3a9d17614896) importEndpoint.js and exportEndpoint.js.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! What a translator sees while it runs: [`ImportContext`] and
//! [`ExportContext`], the Rust form of the `Zotero` object in upstream's
//! sandbox.

use super::api_json::{item_to_api_json, KeyGenerator};
use super::export_items::prepare_export_items;
use super::io::{ExportOutput, ImportInput};
use super::item::{JsObject, TranslatorCollection, TranslatorItem};
use super::item_done::item_done;
use super::options::{TranslateOptions, TranslatorMetadata};
use kovan_common::zotero::{ZoteroItem, ZoteroJsonError};
use serde_json::Value;
use std::collections::VecDeque;

/// Why a translation failed (upstream: the translator threw, or the
/// framework refused).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranslateError {
    /// The translator does not do this direction.
    Unsupported {
        /// The translator's label.
        translator: &'static str,
        /// "import" or "export".
        direction: &'static str,
    },
    /// The translator raised an error (the message upstream would throw).
    Translator(String),
    /// The export input was not a JSON array of item objects (the
    /// translation-server answers 400 "Input must be an array of items as
    /// JSON").
    BadExportInput(String),
}

impl std::fmt::Display for TranslateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TranslateError::Unsupported {
                translator,
                direction,
            } => {
                write!(f, "{translator} does not {direction}")
            }
            TranslateError::Translator(m) => write!(f, "translator error: {m}"),
            TranslateError::BadExportInput(m) => write!(f, "bad export input: {m}"),
        }
    }
}

impl std::error::Error for TranslateError {}

/// The `Zotero` object of an import translation.
#[derive(Debug, Clone)]
pub struct ImportContext {
    /// The input (`Zotero.read`).
    pub input: ImportInput,
    /// The options.
    pub options: TranslateOptions,
    /// The running translator's header.
    pub meta: &'static TranslatorMetadata,
    items: Vec<TranslatorItem>,
    collections: Vec<TranslatorCollection>,
}

impl ImportContext {
    /// A context over `input`.
    pub fn new(input: &str, meta: &'static TranslatorMetadata, options: TranslateOptions) -> Self {
        ImportContext {
            input: ImportInput::new(input),
            options,
            meta,
            items: Vec::new(),
            collections: Vec::new(),
        }
    }

    /// `Zotero.read()`: the next line.
    pub fn read_line(&mut self) -> Option<String> {
        self.input.read_line()
    }

    /// `Zotero.read(n)`: the next `n` characters.
    pub fn read_chars(&mut self, n: usize) -> Option<String> {
        self.input.read_chars(n)
    }

    /// `Zotero.getOption(name)`.
    pub fn get_option(&self, name: &str) -> Option<&Value> {
        self.options.get_option(name)
    }

    /// `Zotero.getHiddenPref(name)`.
    pub fn get_hidden_pref(&self, name: &str) -> Option<Value> {
        self.options.get_hidden_pref(self.meta, name)
    }

    /// `Zotero.parentTranslator` is set.
    pub fn in_child_translator(&self) -> bool {
        self.options.env.parent_translator.is_some()
    }

    /// `item.complete()`: the framework's `_itemDone`, then the item saver.
    pub fn item_done(&mut self, item: TranslatorItem) {
        let child = self.in_child_translator();
        self.items.push(item_done(item, child));
    }

    /// `collection.complete()`.
    pub fn collection_done(&mut self, collection: TranslatorCollection) {
        self.collections.push(collection);
    }

    /// The items completed so far.
    pub fn items(&self) -> &[TranslatorItem] {
        &self.items
    }

    /// Finish: the import's result.
    pub fn finish(self) -> ImportResult {
        ImportResult {
            items: self.items,
            collections: self.collections,
            first_key_index: self.options.first_key_index,
        }
    }
}

/// What an import produced.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportResult {
    /// The items as the framework saved them (after `_itemDone`), in
    /// translator format.
    pub items: Vec<TranslatorItem>,
    /// The collections the translator completed (the translation-server
    /// drops these).
    pub collections: Vec<TranslatorCollection>,
    /// The index of the first key [`ImportResult::api_json`] assigns.
    pub first_key_index: u64,
}

impl ImportResult {
    /// The items as Zotero Web API JSON, as the translation-server's
    /// `/import` returns them (`itemToAPIJSON` on each item; child notes
    /// follow their parent). `now_iso` replaces an `accessDate` of
    /// `CURRENT_TIMESTAMP`.
    pub fn api_json_at(&self, now_iso: &str) -> Vec<Value> {
        let mut keys = KeyGenerator::new(self.first_key_index);
        self.items
            .iter()
            .flat_map(|i| item_to_api_json(i, &mut keys, now_iso))
            .collect()
    }

    /// [`ImportResult::api_json_at`] with the current UTC time (on targets
    /// without a clock, `wasm32-unknown-unknown`, the Unix epoch).
    pub fn api_json(&self) -> Vec<Value> {
        self.api_json_at(&now_iso())
    }

    /// The items as kovan-common [`ZoteroItem`]s (read from
    /// [`ImportResult::api_json`]).
    pub fn zotero_items(&self) -> Result<Vec<ZoteroItem>, ZoteroJsonError> {
        self.api_json()
            .iter()
            .map(ZoteroItem::from_json_value)
            .collect()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    epoch_to_iso(secs)
}

#[cfg(target_arch = "wasm32")]
fn now_iso() -> String {
    epoch_to_iso(0)
}

/// Seconds since the epoch as `YYYY-MM-DDThh:mm:ssZ` (`Zotero.Date.dateToISO`).
fn epoch_to_iso(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Civil-from-days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// The `Zotero` object of an export translation.
#[derive(Debug, Clone)]
pub struct ExportContext {
    items: VecDeque<TranslatorItem>,
    /// The collections `Zotero.nextCollection()` hands out (#749; the
    /// translation-server's item getter has none).
    collections: VecDeque<JsObject>,
    /// The output (`Zotero.write`).
    pub output: ExportOutput,
    /// The options.
    pub options: TranslateOptions,
    /// The running translator's header.
    pub meta: &'static TranslatorMetadata,
}

impl ExportContext {
    /// A context over items in Web API / export JSON (each a JSON object in
    /// its own key order), prepared as the translation-server prepares them
    /// ([`prepare_export_items`]).
    pub fn new(
        items: &[JsObject],
        meta: &'static TranslatorMetadata,
        options: TranslateOptions,
    ) -> Self {
        let export_tags = options.get_option("exportTags").and_then(Value::as_bool);
        let prepared = prepare_export_items(items, meta.legacy_export(), export_tags);
        ExportContext {
            items: prepared.into(),
            collections: VecDeque::new(),
            output: ExportOutput::new(),
            options,
            meta,
        }
    }

    /// `Zotero.nextItem()`.
    pub fn next_item(&mut self) -> Option<TranslatorItem> {
        self.items.pop_front()
    }

    /// Give the export the collections [`ExportContext::next_collection`]
    /// hands out, in Zotero's export format (what Zotero desktop's
    /// `ItemGetter.nextCollection` returns: `{id, name, descendents |
    /// children: [{type: "item", id} | {type: "collection", id, name,
    /// children}]}`). The translation-server has none (its `nextCollection`
    /// returns false); Zotero desktop exporting a library gives every
    /// collection (#749).
    pub fn set_collections(&mut self, collections: Vec<JsObject>) {
        self.collections = collections.into();
    }

    /// `Zotero.nextCollection()` (translate.js:839-845): the next collection,
    /// `None` when none is left. (Upstream throws when the translator does
    /// not declare the `getCollections` config option; every caller here
    /// declares it.)
    pub fn next_collection(&mut self) -> Option<JsObject> {
        self.collections.pop_front()
    }

    /// `Zotero.write(data)`.
    pub fn write(&mut self, data: &str) {
        self.output.write(data);
    }

    /// `Zotero.getOption(name)`.
    pub fn get_option(&self, name: &str) -> Option<&Value> {
        self.options.get_option(name)
    }

    /// `Zotero.getHiddenPref(name)`.
    pub fn get_hidden_pref(&self, name: &str) -> Option<Value> {
        self.options.get_hidden_pref(self.meta, name)
    }

    /// The text written.
    pub fn finish(self) -> String {
        self.output.into_string()
    }
}

/// Parse export input: a JSON array of item objects, each kept in its own
/// key order (as `JSON.parse` does). The translation-server requires a
/// non-empty array whose first element has an `itemType`
/// (exportEndpoint.js:55-57).
pub fn parse_export_input(json: &str) -> Result<Vec<JsObject>, TranslateError> {
    let items: Vec<JsObject> =
        serde_json::from_str(json).map_err(|e| TranslateError::BadExportInput(e.to_string()))?;
    if items.first().is_none_or(|i| !i.truthy("itemType")) {
        return Err(TranslateError::BadExportInput(
            "Input must be an array of items as JSON".to_owned(),
        ));
    }
    Ok(items)
}

/// Export input from kovan-common items (their JSON, key order sorted).
pub fn export_input_from_zotero_items(items: &[ZoteroItem]) -> Vec<JsObject> {
    items
        .iter()
        .map(|i| match i.to_json_value() {
            Value::Object(m) => JsObject::from_map(&m),
            _ => JsObject::new(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_from_epoch() {
        assert_eq!(epoch_to_iso(0), "1970-01-01T00:00:00Z");
        assert_eq!(epoch_to_iso(951_782_400), "2000-02-29T00:00:00Z");
    }
}
