// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "XML ContextObject.js" (translatorID
//   24d9f058-3eb3-4d70-b78f-1ba1aef2128d, lastUpdated 2015-05-20 00:05:55).
// Copyright (c) Avram Lyon and Simon Kornblith.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The XML ContextObject translator (import). NOT YET PORTED (stub).

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::{ImportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "24d9f058-3eb3-4d70-b78f-1ba1aef2128d",
    label: "XML ContextObject",
    creator: "Avram Lyon and Simon Kornblith",
    target: "ctx",
    min_version: "3.0",
    priority: 100,
    translator_type: translator_type::IMPORT,
    config_options: &[("dataMode", HeaderValue::Str("xml/dom"))],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2015-05-20 00:05:55",
};

/// `detectImport`.
pub fn detect_import(_ctx: &mut ImportContext) -> bool {
    false
}

/// `doImport`.
pub fn do_import(_ctx: &mut ImportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator("not yet ported".into()))
}
