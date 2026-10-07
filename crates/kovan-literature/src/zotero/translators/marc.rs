// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "MARC.js" (translatorID
//   a6ee60df-1ddc-4aae-bb25-45e0537be973, lastUpdated 2025-03-28 15:43:42).
// Copyright (c) Simon Kornblith, Sylvain Machefert.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The MARC translator (import). NOT YET PORTED (stub).

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::{ImportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "a6ee60df-1ddc-4aae-bb25-45e0537be973",
    label: "MARC",
    creator: "Simon Kornblith, Sylvain Machefert",
    target: "marc",
    min_version: "2.1.9",
    priority: 100,
    translator_type: translator_type::IMPORT,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2025-03-28 15:43:42",
};

/// `detectImport`.
pub fn detect_import(_ctx: &mut ImportContext) -> bool {
    false
}

/// `doImport`.
pub fn do_import(_ctx: &mut ImportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator("not yet ported".into()))
}
