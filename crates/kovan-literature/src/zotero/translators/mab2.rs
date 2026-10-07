// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): MAB2.js (translatorID
//   91acf493-0de7-4473-8b62-89fd141e6c74, lastUpdated 2014-05-20 17:57:47).
// Copyright: STUB, fill in from the upstream file's header.
// Licence: STUB, fill in from the upstream file's header.

//! The MAB2 translator: import. STUB (#749): not yet ported.

use crate::zotero::framework::options::{translator_type, TranslatorMetadata};
#[allow(unused_imports)]
use crate::zotero::framework::{ExportContext, ImportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "91acf493-0de7-4473-8b62-89fd141e6c74",
    label: "MAB2",
    creator: "Simon Kornblith. Adaptions for MAB2: Leon Krauthausen (FUB)",
    target: "mab2",
    min_version: "1.0.0b3.r1",
    priority: 100,
    translator_type: translator_type::IMPORT,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2014-05-20 17:57:47",
};

/// `detectImport`. STUB.
pub fn detect_import(_ctx: &mut ImportContext) -> bool {
    false
}

/// `doImport`. STUB.
pub fn do_import(_ctx: &mut ImportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator("not yet ported (#749)".into()))
}
