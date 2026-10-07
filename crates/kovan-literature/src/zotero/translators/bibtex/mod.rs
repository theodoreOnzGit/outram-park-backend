// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): BibTeX.js (translatorID
//   9cb70025-a888-4a29-a210-93ec52da40d4, lastUpdated 2026-08-03 15:56:33).
// Copyright (C) 2019 CHNM, Simon Kornblith, Richard Karnesky and Emiliano
//   heyns.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The BibTeX translator: import and export. (Port in progress.)

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::{ExportContext, ImportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "9cb70025-a888-4a29-a210-93ec52da40d4",
    label: "BibTeX",
    creator: "Simon Kornblith, Richard Karnesky and Emiliano heyns",
    target: "bib",
    min_version: "2.1.9",
    priority: 200,
    translator_type: translator_type::IMPORT | translator_type::EXPORT,
    config_options: &[
        ("async", HeaderValue::Bool(true)),
        ("getCollections", HeaderValue::Bool(true)),
    ],
    display_options: &[
        ("exportCharset", HeaderValue::Str("UTF-8")),
        ("exportNotes", HeaderValue::Bool(true)),
        ("exportFileData", HeaderValue::Bool(false)),
        ("useJournalAbbreviation", HeaderValue::Bool(false)),
    ],
    hidden_prefs: &[],
    last_updated: "2026-08-03 15:56:33",
};

/// `detectImport`.
pub fn detect_import(_ctx: &mut ImportContext) -> bool {
    false
}

/// `doImport`.
pub fn do_import(_ctx: &mut ImportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator(
        "BibTeX import: not yet ported".into(),
    ))
}

/// `doExport`.
pub fn do_export(_ctx: &mut ExportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator(
        "BibTeX export: not yet ported".into(),
    ))
}
