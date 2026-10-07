// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): RefWorks Tagged.js (translatorID
//   1a3506da-a303-4b0a-a1cd-f216e6138d86, lastUpdated 2016-06-21 08:45:20).
// Copyright: STUB, fill in from the upstream file's header.
// Licence: STUB, fill in from the upstream file's header.

//! The RefWorks Tagged translator: import and export. STUB (#749): not yet ported.

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
#[allow(unused_imports)]
use crate::zotero::framework::{ExportContext, ImportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "1a3506da-a303-4b0a-a1cd-f216e6138d86",
    label: "RefWorks Tagged",
    creator: "Simon Kornblith, Aurimas Vinckevicius, and Sebastian Karcher",
    target: "txt",
    min_version: "3.0.4",
    priority: 100,
    translator_type: translator_type::IMPORT | translator_type::EXPORT,
    config_options: &[],
    display_options: &[
        ("exportCharset", HeaderValue::Str("UTF-8")),
        ("exportNotes", HeaderValue::Bool(true)),
        ("exportFileData", HeaderValue::Bool(true)),
    ],
    hidden_prefs: &[],
    last_updated: "2016-06-21 08:45:20",
};

/// `detectImport`. STUB.
pub fn detect_import(_ctx: &mut ImportContext) -> bool {
    false
}

/// `doImport`. STUB.
pub fn do_import(_ctx: &mut ImportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator("not yet ported (#749)".into()))
}

/// `doExport`. STUB.
pub fn do_export(_ctx: &mut ExportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator("not yet ported (#749)".into()))
}
