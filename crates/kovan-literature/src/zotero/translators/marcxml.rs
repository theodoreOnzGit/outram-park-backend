// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "MARCXML.js" (translatorID
//   edd87d07-9194-42f8-b2ad-997c4c7deefd, lastUpdated 2025-03-28 15:11:06).
// Copyright (c) Sebastian Karcher.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The MARCXML translator (import). NOT YET PORTED (stub).

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::{ImportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "edd87d07-9194-42f8-b2ad-997c4c7deefd",
    label: "MARCXML",
    creator: "Sebastian Karcher",
    target: "xml",
    min_version: "3.0",
    priority: 100,
    translator_type: translator_type::IMPORT,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2025-03-28 15:11:06",
};

/// `detectImport`.
pub fn detect_import(_ctx: &mut ImportContext) -> bool {
    false
}

/// `doImport`.
pub fn do_import(_ctx: &mut ImportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator("not yet ported".into()))
}
