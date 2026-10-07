// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): Wikidata QuickStatements.js (translatorID
//   51e5355d-9974-484f-80b9-f84d2b55782e, lastUpdated 2025-08-11 00:00:00).
// Copyright: STUB, fill in from the upstream file's header.
// Licence: STUB, fill in from the upstream file's header.

//! The Wikidata QuickStatements translator: export. STUB (#749): not yet ported.

use crate::zotero::framework::options::{translator_type, TranslatorMetadata};
#[allow(unused_imports)]
use crate::zotero::framework::{ExportContext, ImportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "51e5355d-9974-484f-80b9-f84d2b55782e",
    label: "Wikidata QuickStatements",
    creator: "Philipp Zumstein with contributors",
    target: "txt",
    min_version: "3.0",
    priority: 100,
    translator_type: translator_type::EXPORT,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2025-08-11 00:00:00",
};

/// `doExport`. STUB.
pub fn do_export(_ctx: &mut ExportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator("not yet ported (#749)".into()))
}
