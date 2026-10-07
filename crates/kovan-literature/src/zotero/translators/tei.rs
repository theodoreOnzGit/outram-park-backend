// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "TEI.js" (translatorID
//   032ae9b7-ab90-9205-a479-baf81f49184a, lastUpdated 2026-05-20 17:56:18).
// Copyright (c) Stefan Majewski.
// Licence: AGPL-3.0 (upstream: GPL-3.0-or-later upstream, combinable with AGPL-3.0).

//! The TEI translator (export). NOT YET PORTED (stub).

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::{ExportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "032ae9b7-ab90-9205-a479-baf81f49184a",
    label: "TEI",
    creator: "Stefan Majewski",
    target: "xml",
    min_version: "4.0.27",
    priority: 25,
    translator_type: translator_type::EXPORT,
    config_options: &[("dataMode", HeaderValue::Str("xml/dom")), ("getCollections", HeaderValue::Str("true"))],
    display_options: &[("exportNotes", HeaderValue::Bool(false)), ("Export Tags", HeaderValue::Bool(false)), ("Generate XML IDs", HeaderValue::Bool(true)), ("Full TEI Document", HeaderValue::Bool(false)), ("Export Collections", HeaderValue::Bool(false))],
    hidden_prefs: &[],
    last_updated: "2026-05-20 17:56:18",
};

/// `doExport`.
pub fn do_export(_ctx: &mut ExportContext) -> Result<(), TranslateError> {
    Err(TranslateError::Translator("not yet ported".into()))
}
