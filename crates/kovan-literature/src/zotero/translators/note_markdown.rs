// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Note Markdown.js" (translatorID
//   1412e9e2-51e1-42ec-aa35-e036a895534b, lastUpdated 2024-07-10 16:00:00).
// Copyright (c) 2021 Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The Note Markdown translator (export). NOT YET PORTED (stub).

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::{ExportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "1412e9e2-51e1-42ec-aa35-e036a895534b",
    label: "Note Markdown",
    creator: "Martynas Bagdonas",
    target: "md",
    min_version: "5.0.97",
    priority: 50,
    translator_type: translator_type::EXPORT,
    config_options: &[("noteTranslator", HeaderValue::Bool(true))],
    display_options: &[("includeAppLinks", HeaderValue::Bool(true))],
    hidden_prefs: &[],
    last_updated: "2024-07-10 16:00:00",
};

/// `doExport`.
pub fn do_export(_ctx: &mut ExportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator("not yet ported".into()))
}
