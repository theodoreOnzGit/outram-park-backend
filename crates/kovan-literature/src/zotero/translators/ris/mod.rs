// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): RIS.js (translatorID
//   32d59d2d-b65a-4da4-b0a3-bdd3cfb979e7, lastUpdated 2026-01-05 18:52:35):
//   header :1-21, `detectImport` :47-62; the rest in this module's files.
// Copyright (C) 2006-2023 Simon Kornblith, Aurimas Vinckevicus, Abe Jellinek.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The RIS translator: import and export.
//!
//! | File | Upstream |
//! |---|---|
//! | [`tables`] | type maps, `fieldMap`, `degenerateImportFieldMap`, `exportOrder`, ProCite maps (generated from RIS.js) |
//! | [`mapper`] | `TagMapper` |
//! | [`reader`] | `RISReader`, `TagCleaner`, `ProCiteCleaner`, `EndNoteCleaner`, `CitaviCleaner` |
//! | [`import`] | `processTag`, `applyValue`, `dateRIStoZotero`, `completeItem`, `importNext` |
//! | [`export`] | `addTag`, `doExport` |
//!
//! Not ported: `exportedOptions` (:68-73: `itemType`, `defaultItemType`,
//! `typeMap`, `fieldMap`), which only another translator calling this one
//! through `loadTranslator` can set; they are all `false` for a direct
//! import or export, and the code paths that read them are ported for that
//! value. Also not ported: `saveFile` for attachments on export
//! (`exportFileData`), which the translation-server does not support.
//!
//! **Maturity: AI draft (1).** Verified code-to-code against the Zotero
//! translation-server (`tests/zotero_translators.rs`, `ris_*`).

pub mod export;
pub mod import;
pub mod mapper;
pub mod reader;
#[rustfmt::skip]
pub mod tables;

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::{ExportContext, ImportContext, TranslateError};
use regex::Regex;
use std::sync::OnceLock;

/// The translator header. (`getCollections` is the string "true" upstream.)
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "32d59d2d-b65a-4da4-b0a3-bdd3cfb979e7",
    label: "RIS",
    creator: "Simon Kornblith and Aurimas Vinckevicius",
    target: "ris",
    min_version: "3.0.4",
    priority: 100,
    translator_type: translator_type::IMPORT | translator_type::EXPORT,
    config_options: &[
        ("async", HeaderValue::Bool(true)),
        ("getCollections", HeaderValue::Str("true")),
    ],
    display_options: &[
        ("exportCharset", HeaderValue::Str("UTF-8")),
        ("exportNotes", HeaderValue::Bool(true)),
        ("exportFileData", HeaderValue::Bool(false)),
    ],
    hidden_prefs: &[],
    last_updated: "2026-01-05 18:52:35",
};

/// `detectImport` (:47-62): a `TY  - ` line among the first five
/// non-blank lines.
pub fn detect_import(ctx: &mut ImportContext) -> bool {
    static LEAD: OnceLock<Regex> = OnceLock::new();
    static TY: OnceLock<Regex> = OnceLock::new();
    let lead = LEAD
        .get_or_init(|| Regex::new(&format!("^{}+", crate::zotero::framework::js::WS)).unwrap());
    let ty = TY.get_or_init(|| Regex::new("^TY {1,2}- ").unwrap());
    let mut i = 0;
    while let Some(line) = ctx.read_line() {
        let line = lead.replace(&line, "");
        if !line.is_empty() {
            if ty.is_match(&crate::zotero::framework::js::substr(&line, 0, 6)) {
                return true;
            }
            let before = i;
            i += 1;
            if before > 3 {
                return false;
            }
        }
    }
    false
}

/// `doImport` (:1806-1912).
pub fn do_import(ctx: &mut ImportContext) -> Result<(), TranslateError> {
    import::do_import(ctx)
}

/// `doExport` (:2030-2179).
pub fn do_export(ctx: &mut ExportContext) -> Result<(), TranslateError> {
    export::do_export(ctx)
}

/// "Now" for `new Date()` (seconds since the epoch):
/// [`TranslationEnv::now_unix_secs`](crate::zotero::framework::TranslationEnv),
/// else the system clock (the epoch on `wasm32`, which has none).
pub(crate) fn now_unix_secs(ctx: &ImportContext) -> i64 {
    if let Some(n) = ctx.options.env.now_unix_secs {
        return n;
    }
    system_now()
}

#[cfg(not(target_arch = "wasm32"))]
fn system_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

#[cfg(target_arch = "wasm32")]
fn system_now() -> i64 {
    0
}
