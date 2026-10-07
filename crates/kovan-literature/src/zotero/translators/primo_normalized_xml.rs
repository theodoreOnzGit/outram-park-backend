// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Primo Normalized XML.js" (translatorID
//   efd737c9-a227-4113-866e-d57fbc0684ca, lastUpdated 2026-09-01 18:46:48).
// Copyright (c) Philipp Zumstein.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The Primo Normalized XML translator (import). NOT YET PORTED (stub).

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::{ImportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "efd737c9-a227-4113-866e-d57fbc0684ca",
    label: "Primo Normalized XML",
    creator: "Philipp Zumstein",
    target: "xml",
    min_version: "3.0",
    priority: 100,
    translator_type: translator_type::IMPORT,
    config_options: &[("dataMode", HeaderValue::Str("xml/dom"))],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2026-09-01 18:46:48",
};

/// `detectImport`.
pub fn detect_import(_ctx: &mut ImportContext) -> bool {
    false
}

/// `doImport`.
pub fn do_import(_ctx: &mut ImportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator("not yet ported".into()))
}
