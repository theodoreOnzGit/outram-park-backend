// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): OpenAlex JSON.js (translatorID
//   faa53754-fb55-4658-9094-ae8a7e0409a2, lastUpdated 2024-07-29 14:16:09).
// Copyright: STUB, fill in from the upstream file's header.
// Licence: STUB, fill in from the upstream file's header.

//! The OpenAlex JSON translator: import. STUB (#749): not yet ported.

use crate::zotero::framework::options::{translator_type, TranslatorMetadata};
#[allow(unused_imports)]
use crate::zotero::framework::{ExportContext, ImportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "faa53754-fb55-4658-9094-ae8a7e0409a2",
    label: "OpenAlex JSON",
    creator: "Sebastian Karcher",
    target: "json",
    min_version: "5.0",
    priority: 100,
    translator_type: translator_type::IMPORT,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2024-07-29 14:16:09",
};

/// `detectImport`. STUB.
pub fn detect_import(_ctx: &mut ImportContext) -> bool {
    false
}

/// `doImport`. STUB.
pub fn do_import(_ctx: &mut ImportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator("not yet ported (#749)".into()))
}
