// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): Wikipedia Citation Templates.js (translatorID
//   3f50aaac-7acc-4350-acd0-59cb77faf620, lastUpdated 2023-11-05 21:29:00).
// Copyright: STUB, fill in from the upstream file's header.
// Licence: STUB, fill in from the upstream file's header.

//! The Wikipedia Citation Templates translator: export. STUB (#749): not yet ported.

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
#[allow(unused_imports)]
use crate::zotero::framework::{ExportContext, ImportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "3f50aaac-7acc-4350-acd0-59cb77faf620",
    label: "Wikipedia Citation Templates",
    creator: "Simon Kornblith",
    target: "txt",
    min_version: "1.0.0b4.r1",
    priority: 100,
    translator_type: translator_type::EXPORT,
    config_options: &[],
    display_options: &[("exportCharset", HeaderValue::Str("UTF-8"))],
    hidden_prefs: &[],
    last_updated: "2023-11-05 21:29:00",
};

/// `doExport`. STUB.
pub fn do_export(_ctx: &mut ExportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator("not yet ported (#749)".into()))
}
