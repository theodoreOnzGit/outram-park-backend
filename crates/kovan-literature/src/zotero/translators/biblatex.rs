// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): BibLaTeX.js (translatorID
//   b6e39b57-8942-4d11-8259-342c46ce395f, lastUpdated 2026-04-01 18:00:00).
// Copyright (C) 2019 Simon Kornblith, Richard Karnesky and Anders Johansson.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The BibLaTeX translator: export. (Port in progress.)

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::{ExportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "b6e39b57-8942-4d11-8259-342c46ce395f",
    label: "BibLaTeX",
    creator: "Simon Kornblith, Richard Karnesky and Anders Johansson",
    target: "bib",
    min_version: "2.1.9",
    priority: 100,
    translator_type: translator_type::EXPORT,
    config_options: &[("getCollections", HeaderValue::Bool(true))],
    display_options: &[
        ("exportCharset", HeaderValue::Str("UTF-8")),
        ("exportNotes", HeaderValue::Bool(false)),
        ("exportFileData", HeaderValue::Bool(false)),
        ("useJournalAbbreviation", HeaderValue::Bool(false)),
    ],
    hidden_prefs: &[],
    last_updated: "2026-04-01 18:00:00",
};

/// `doExport`.
pub fn do_export(_ctx: &mut ExportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator(
        "BibLaTeX export: not yet ported".into(),
    ))
}
