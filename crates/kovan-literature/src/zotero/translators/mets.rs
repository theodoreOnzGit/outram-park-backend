// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "METS.js" (translatorID
//   5c6895a1-b6a9-4939-9c65-43f8ae0ef096, lastUpdated 2021-07-13 20:44:34).
// Copyright (c) 2021 Abe Jellinek.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The METS translator (import). NOT YET PORTED (stub).

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::{ImportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "5c6895a1-b6a9-4939-9c65-43f8ae0ef096",
    label: "METS",
    creator: "Abe Jellinek",
    target: "xml",
    min_version: "3.0",
    priority: 50,
    translator_type: translator_type::IMPORT,
    config_options: &[("dataMode", HeaderValue::Str("xml/dom"))],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2021-07-13 20:44:34",
};

/// `detectImport`.
pub fn detect_import(_ctx: &mut ImportContext) -> bool {
    false
}

/// `doImport`.
pub fn do_import(_ctx: &mut ImportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator("not yet ported".into()))
}
