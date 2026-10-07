// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): COinS.js (translatorID
//   05d07af9-105a-4572-99f6-a8e231c0daef, lastUpdated 2021-06-01 17:38:46).
// Copyright: STUB, fill in from the upstream file's header.
// Licence: STUB, fill in from the upstream file's header.

//! The COinS translator: export. STUB (#749): not yet ported.

use crate::zotero::framework::options::{translator_type, TranslatorMetadata};
#[allow(unused_imports)]
use crate::zotero::framework::{ExportContext, ImportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "05d07af9-105a-4572-99f6-a8e231c0daef",
    label: "COinS",
    creator: "Simon Kornblith",
    target: "",
    min_version: "2.1",
    priority: 310,
    translator_type: translator_type::EXPORT | translator_type::WEB,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2021-06-01 17:38:46",
};

/// `doExport`. STUB.
pub fn do_export(_ctx: &mut ExportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator("not yet ported (#749)".into()))
}
