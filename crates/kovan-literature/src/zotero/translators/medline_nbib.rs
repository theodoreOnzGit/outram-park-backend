// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): MEDLINEnbib.js (translatorID
//   9ec64cfd-bea7-472a-9557-493c0c26b0fb, lastUpdated 2025-04-29 03:02:00).
// Copyright: STUB, fill in from the upstream file's header.
// Licence: STUB, fill in from the upstream file's header.

//! The MEDLINE/nbib translator: import. STUB (#749): not yet ported.

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
#[allow(unused_imports)]
use crate::zotero::framework::{ExportContext, ImportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "9ec64cfd-bea7-472a-9557-493c0c26b0fb",
    label: "MEDLINE/nbib",
    creator: "Sebastian Karcher",
    target: "txt",
    min_version: "4.0",
    priority: 100,
    translator_type: translator_type::IMPORT,
    config_options: &[("async", HeaderValue::Bool(true))],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2025-04-29 03:02:00",
};

/// `detectImport`. STUB.
pub fn detect_import(_ctx: &mut ImportContext) -> bool {
    false
}

/// `doImport`. STUB.
pub fn do_import(_ctx: &mut ImportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator("not yet ported (#749)".into()))
}
