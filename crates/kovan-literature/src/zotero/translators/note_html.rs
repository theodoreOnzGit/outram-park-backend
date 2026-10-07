// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Note HTML.js" (translatorID
//   897a81c2-9f60-4bec-ae6b-85a5030b8be5, lastUpdated 2024-07-10 15:30:00).
// Copyright (c) 2021 Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The Note HTML translator (export). NOT YET PORTED (stub).

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::{ExportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "897a81c2-9f60-4bec-ae6b-85a5030b8be5",
    label: "Note HTML",
    creator: "Martynas Bagdonas",
    target: "html",
    min_version: "5.0.97",
    priority: 50,
    translator_type: translator_type::EXPORT,
    config_options: &[("noteTranslator", HeaderValue::Bool(true))],
    display_options: &[("includeAppLinks", HeaderValue::Bool(false))],
    hidden_prefs: &[],
    last_updated: "2024-07-10 15:30:00",
};

/// `doExport`.
pub fn do_export(_ctx: &mut ExportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator("not yet ported".into()))
}
