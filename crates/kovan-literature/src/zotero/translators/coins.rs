// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): COinS.js (translatorID
//   05d07af9-105a-4572-99f6-a8e231c0daef, lastUpdated 2021-06-01 17:38:46):
//   header :1-12, `doExport` :249-260.
// Copyright (c) 2021 Simon Kornblith and Abe Jellinek.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The COinS translator: export (one `<span class='Z3988'>` per item, its
//! `title` an OpenURL 1.0 ContextObject).
//!
//! Not ported: the web half (`detectWeb`, `doWeb` and its helpers,
//! :38-247), which reads COinS spans from a web page and looks items up
//! through search translators; kovan has no web translation.
//!
//! **Maturity: AI draft (1).** Verified code-to-code against the Zotero
//! translation-server (`tests/zotero_translators.rs`, `coins_*`).

use crate::zotero::framework::openurl::{create_context_object, OpenUrlVersion};
use crate::zotero::framework::options::{translator_type, TranslatorMetadata};
use crate::zotero::framework::utilities::html_special_chars;
use crate::zotero::framework::{ExportContext, TranslateError};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "05d07af9-105a-4572-99f6-a8e231c0daef",
    label: "COinS",
    creator: "Simon Kornblith",
    target: "",
    min_version: "2.1",
    priority: 310,
    translator_type: translator_type::EXPORT | translator_type::WEB,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2021-06-01 17:38:46",
};

/// `doExport` (:249-260).
pub fn do_export(ctx: &mut ExportContext) -> Result<(), TranslateError> {
    while let Some(item) = ctx.next_item() {
        let co = create_context_object(&item, OpenUrlVersion::V1_0, &ctx.options.env.dates);
        if !co.is_empty() {
            ctx.write(&format!(
                "<span class='Z3988' title='{}'></span>\n",
                html_special_chars(&co)
            ));
        }
    }
    Ok(())
}
