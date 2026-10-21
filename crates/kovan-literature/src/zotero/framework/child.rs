// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translate, https://github.com/zotero/translate (commit
//   dd524aea9a55): src/translation/translate.js — `Zotero.loadTranslator`
//   (a child `Zotero.Translate.Import` whose `_parentTranslator` is the
//   caller), `setTranslator` :920-944 (an array is accepted, and an import
//   runs its first entry only: the try-the-next fallback is
//   `Zotero.Translate.Search#complete` :2795-2815, not Import's),
//   `_itemDone` in a child :178-183 (the item goes to the parent's
//   `itemDone` handler instead of being saved), and the child's
//   `Zotero.parentTranslator` :1877-1879.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! Child translators: a translator running another one
//! (`Zotero.loadTranslator("import")`, `setString`, `setHandler("itemDone",
//! ...)`, `translate()`), as METS runs MODS and MARCXML.
//!
//! The child gets a context of its own over the string, with
//! `parent_translator` set, so its `item.complete()` applies the child form
//! of `_itemDone` (normalised, not saved). The parent receives those items
//! in completion order (its `itemDone` handler) and completes them itself.

use super::context::{ImportContext, TranslateError};
use super::item::TranslatorItem;
use super::options::{TranslateOptions, TranslatorMetadata};

/// A child import context over `input` for the translator `meta`, called
/// from `parent` (the environment is the parent's; `Zotero.parentTranslator`
/// is the parent's translatorID).
pub fn child_import_context(
    parent: &ImportContext,
    input: &str,
    meta: &'static TranslatorMetadata,
) -> ImportContext {
    let mut options = TranslateOptions::for_translator(meta);
    options.env = parent.options.env.clone();
    options.env.parent_translator = Some(parent.meta.id.to_owned());
    ImportContext::new(input, meta, options)
}

/// Run a child import (`run` is the child translator's `doImport` over the
/// child context) and return the items it completed, in order, ready for
/// the parent's `itemDone` handler. An error thrown by the child is the
/// parent's error.
pub fn run_child_import(
    parent: &ImportContext,
    input: &str,
    meta: &'static TranslatorMetadata,
    run: fn(&mut ImportContext) -> Result<(), TranslateError>,
) -> Result<Vec<TranslatorItem>, TranslateError> {
    let mut child = child_import_context(parent, input, meta);
    run(&mut child)?;
    Ok(child.finish().items)
}
