// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Citavi 5 XML.js" (translatorID
//   e7243cef-a709-4a46-ba46-1b1318051bec, lastUpdated 2025-01-04 01:03:00).
// Copyright (c) Philipp Zumstein, Tomasz Najdek.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The Citavi 5 XML translator (import). NOT YET PORTED (stub).

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::{ImportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "e7243cef-a709-4a46-ba46-1b1318051bec",
    label: "Citavi 5 XML",
    creator: "Philipp Zumstein, Tomasz Najdek",
    target: "xml",
    min_version: "3.0",
    priority: 100,
    translator_type: translator_type::IMPORT,
    config_options: &[("dataMode", HeaderValue::Str("xml/dom")), ("async", HeaderValue::Bool(true))],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2025-01-04 01:03:00",
};

/// `detectImport`.
pub fn detect_import(_ctx: &mut ImportContext) -> bool {
    false
}

/// `doImport`.
pub fn do_import(_ctx: &mut ImportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator("not yet ported".into()))
}
