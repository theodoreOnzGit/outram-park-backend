// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Endnote XML.js" (translatorID
//   eb7059a4-35ec-4961-a915-3cf58eb9784b, lastUpdated 2021-07-14 20:41:42).
// Copyright (c) Sebastian Karcher.
// Licence: AGPL-3.0 (upstream: no licence text upstream; treated as AGPLv3 as part of Zotero, maintainer decision 2026-10-07, #747).

//! The Endnote XML translator (import and export). NOT YET PORTED (stub).

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::{ExportContext, ImportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "eb7059a4-35ec-4961-a915-3cf58eb9784b",
    label: "Endnote XML",
    creator: "Sebastian Karcher",
    target: "xml",
    min_version: "4.0",
    priority: 100,
    translator_type: translator_type::IMPORT | translator_type::EXPORT,
    config_options: &[("async", HeaderValue::Bool(true)), ("getCollections", HeaderValue::Bool(true))],
    display_options: &[("exportNotes", HeaderValue::Bool(true)), ("exportFileData", HeaderValue::Bool(false))],
    hidden_prefs: &[],
    last_updated: "2021-07-14 20:41:42",
};

/// `detectImport`.
pub fn detect_import(_ctx: &mut ImportContext) -> bool {
    false
}

/// `doImport`.
pub fn do_import(_ctx: &mut ImportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator("not yet ported".into()))
}

/// `doExport`.
pub fn do_export(_ctx: &mut ExportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator("not yet ported".into()))
}
