// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): RIS.js (translatorID
//   32d59d2d-b65a-4da4-b0a3-bdd3cfb979e7, lastUpdated 2026-01-05 18:52:35).
// Copyright (C) 2006-2023 Simon Kornblith, Aurimas Vinckevicus, Abe Jellinek.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The RIS translator: import and export. (Port in progress.)

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::{ExportContext, ImportContext, TranslateError};

/// The translator header. (`getCollections` is the string "true" upstream.)
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "32d59d2d-b65a-4da4-b0a3-bdd3cfb979e7",
    label: "RIS",
    creator: "Simon Kornblith and Aurimas Vinckevicius",
    target: "ris",
    min_version: "3.0.4",
    priority: 100,
    translator_type: translator_type::IMPORT | translator_type::EXPORT,
    config_options: &[
        ("async", HeaderValue::Bool(true)),
        ("getCollections", HeaderValue::Str("true")),
    ],
    display_options: &[
        ("exportCharset", HeaderValue::Str("UTF-8")),
        ("exportNotes", HeaderValue::Bool(true)),
        ("exportFileData", HeaderValue::Bool(false)),
    ],
    hidden_prefs: &[],
    last_updated: "2026-01-05 18:52:35",
};

/// `detectImport`.
pub fn detect_import(_ctx: &mut ImportContext) -> bool {
    false
}

/// `doImport`.
pub fn do_import(_ctx: &mut ImportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator(
        "RIS import: not yet ported".into(),
    ))
}

/// `doExport`.
pub fn do_export(_ctx: &mut ExportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator(
        "RIS export: not yet ported".into(),
    ))
}
