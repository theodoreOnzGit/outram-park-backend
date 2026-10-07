// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "PubMed XML.js" (translatorID
//   fcf41bed-0cbc-3704-85c7-8062a0068a7a, lastUpdated 2026-05-21 14:52:55).
// Copyright (c) Simon Kornblith, Michael Berkowitz, Avram Lyon, and Rintze Zelle.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The PubMed XML translator (import). NOT YET PORTED (stub).

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::{ImportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "fcf41bed-0cbc-3704-85c7-8062a0068a7a",
    label: "PubMed XML",
    creator: "Simon Kornblith, Michael Berkowitz, Avram Lyon, and Rintze Zelle",
    target: "xml",
    min_version: "2.1.9",
    priority: 100,
    translator_type: translator_type::IMPORT,
    config_options: &[("dataMode", HeaderValue::Str("xml/dom"))],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2026-05-21 14:52:55",
};

/// `detectImport`.
pub fn detect_import(_ctx: &mut ImportContext) -> bool {
    false
}

/// `doImport`.
pub fn do_import(_ctx: &mut ImportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator("not yet ported".into()))
}
