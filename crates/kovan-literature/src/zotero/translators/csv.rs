// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): CSV.js (translatorID
//   25f4c5e2-d790-4daa-a667-797619c7e2f2, lastUpdated 2022-06-28 19:45:59).
// Copyright: STUB, fill in from the upstream file's header.
// Licence: STUB, fill in from the upstream file's header.

//! The CSV translator: export. STUB (#749): not yet ported.

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
#[allow(unused_imports)]
use crate::zotero::framework::{ExportContext, ImportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "25f4c5e2-d790-4daa-a667-797619c7e2f2",
    label: "CSV",
    creator: "Philipp Zumstein and Aurimas Vinckevicius",
    target: "csv",
    min_version: "4.0.26",
    priority: 100,
    translator_type: translator_type::EXPORT,
    config_options: &[],
    display_options: &[
        ("exportCharset", HeaderValue::Str("UTF-8xBOM")),
        ("exportNotes", HeaderValue::Bool(false)),
    ],
    hidden_prefs: &[],
    last_updated: "2022-06-28 19:45:59",
};

/// `doExport`. STUB.
pub fn do_export(_ctx: &mut ExportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator("not yet ported (#749)".into()))
}
