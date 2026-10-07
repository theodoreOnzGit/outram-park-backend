// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): Evernote.js (translatorID
//   18dd188a-9afc-4cd6-8775-1980c3ce0fbf, lastUpdated 2019-10-11 07:30:00).
// Copyright: STUB, fill in from the upstream file's header.
// Licence: STUB, fill in from the upstream file's header.

//! The Simple Evernote Export translator: export. STUB (#749): not yet ported.

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
#[allow(unused_imports)]
use crate::zotero::framework::{ExportContext, ImportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "18dd188a-9afc-4cd6-8775-1980c3ce0fbf",
    label: "Simple Evernote Export",
    creator: "Volodymir Skipa",
    target: "enex",
    min_version: "2.1.9",
    priority: 50,
    translator_type: translator_type::EXPORT,
    config_options: &[],
    display_options: &[("exportNotes", HeaderValue::Bool(true))],
    hidden_prefs: &[],
    last_updated: "2019-10-11 07:30:00",
};

/// `doExport`. STUB.
pub fn do_export(_ctx: &mut ExportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator("not yet ported (#749)".into()))
}
