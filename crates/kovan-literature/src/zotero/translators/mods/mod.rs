// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "MODS.js" (translatorID
//   0e2235e7-babf-413c-9acf-f27cce5f059c, lastUpdated 2022-10-31 14:01:52).
// Copyright (c) 2019-2021 Simon Kornblith, Richard Karnesky, and Abe Jellinek.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The MODS translator (import and export). NOT YET PORTED (stub).

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::{ExportContext, ImportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "0e2235e7-babf-413c-9acf-f27cce5f059c",
    label: "MODS",
    creator: "Simon Kornblith, Richard Karnesky, and Abe Jellinek",
    target: "xml",
    min_version: "2.1.9",
    priority: 50,
    translator_type: translator_type::IMPORT | translator_type::EXPORT,
    config_options: &[("dataMode", HeaderValue::Str("xml/dom"))],
    display_options: &[("exportNotes", HeaderValue::Bool(true))],
    hidden_prefs: &[],
    last_updated: "2022-10-31 14:01:52",
};

/// `detectImport`.
pub fn detect_import(_ctx: &mut ImportContext) -> bool {
    false
}

/// `doImport`.
pub fn do_import(_ctx: &mut ImportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator("not yet ported".into()))
}

/// `doExport`.
pub fn do_export(_ctx: &mut ExportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator("not yet ported".into()))
}
