// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): ReferBibIX.js (translatorID
//   881f60f2-0802-411a-9228-ce5f47b64c7d, lastUpdated 2023-10-27 09:03:42).
// Copyright: STUB, fill in from the upstream file's header.
// Licence: STUB, fill in from the upstream file's header.

//! The Refer/BibIX translator: import and export. STUB (#749): not yet ported.

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
#[allow(unused_imports)]
use crate::zotero::framework::{ExportContext, ImportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "881f60f2-0802-411a-9228-ce5f47b64c7d",
    label: "Refer/BibIX",
    creator: "Simon Kornblith",
    target: "txt",
    min_version: "2.1",
    priority: 100,
    translator_type: translator_type::IMPORT | translator_type::EXPORT,
    config_options: &[],
    display_options: &[("exportCharset", HeaderValue::Str("UTF-8"))],
    hidden_prefs: &[],
    last_updated: "2023-10-27 09:03:42",
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
