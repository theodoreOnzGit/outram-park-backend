// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "DSpace Intermediate Metadata.js" (translatorID
//   2c05e2d1-a533-448f-aa20-e919584864cb, lastUpdated 2022-12-24 19:29:02).
// Copyright (c) 2022 Sebastian Karcher.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The DSpace Intermediate Metadata translator (import). NOT YET PORTED (stub).

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::{ImportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "2c05e2d1-a533-448f-aa20-e919584864cb",
    label: "DSpace Intermediate Metadata",
    creator: "Sebastian Karcher",
    target: "xml",
    min_version: "5.0",
    priority: 100,
    translator_type: translator_type::IMPORT,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2022-12-24 19:29:02",
};

/// `detectImport`.
pub fn detect_import(_ctx: &mut ImportContext) -> bool {
    false
}

/// `doImport`.
pub fn do_import(_ctx: &mut ImportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator("not yet ported".into()))
}
