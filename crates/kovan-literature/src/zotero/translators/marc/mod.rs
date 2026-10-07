// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "MARC.js" (translatorID
//   a6ee60df-1ddc-4aae-bb25-45e0537be973, lastUpdated 2025-03-28 15:43:42):
//   `detectImport` :38-45, `doImport` :838-860, `exports` :862-867.
// Copyright (c) 2020 Simon Kornblith, Sylvain Machefert.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The MARC translator (import): binary MARC 21 and UNIMARC records.
//!
//! | Module | Upstream |
//! |---|---|
//! | [`record`] | the record model (`importBinary`, `addField`, `getField`, `extractSubfields`) and the cleaning functions; what MARCXML uses through `getTranslatorObject` |
//! | [`translate`] | `record.prototype.translate` and its `_associate*` helpers |
//!
//! **Maturity: AI draft (1).**

pub mod record;
pub mod translate;

pub use record::{Record, FIELD_TERMINATOR, RECORD_TERMINATOR, SUBFIELD_DELIMITER};

use crate::zotero::framework::options::{translator_type, TranslatorMetadata};
use crate::zotero::framework::{ImportContext, TranslateError, TranslatorItem};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "a6ee60df-1ddc-4aae-bb25-45e0537be973",
    label: "MARC",
    creator: "Simon Kornblith, Sylvain Machefert",
    target: "marc",
    min_version: "2.1.9",
    priority: 100,
    translator_type: translator_type::IMPORT,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2025-03-28 15:43:42",
};

/// `detectImport` (:38-45): the first 8 characters match
/// `/^[0-9]{5}[a-z ]{3}$/`.
pub fn detect_import(ctx: &mut ImportContext) -> bool {
    let Some(read) = ctx.read_chars(8) else {
        return false;
    };
    let c: Vec<char> = read.chars().collect();
    c.len() == 8
        && c[..5].iter().all(char::is_ascii_digit)
        && c[5..].iter().all(|&x| x == ' ' || x.is_ascii_lowercase())
}

/// `doImport` (:838-860): the input read 4096 characters at a time, split
/// on the record terminator; every complete record is imported (text after
/// the last terminator is held over and, at the end, dropped).
pub fn do_import(ctx: &mut ImportContext) -> Result<(), TranslateError> {
    let mut hold_over = String::new();
    while let Some(text) = ctx.read_chars(4096).filter(|t| !t.is_empty()) {
        let mut records: Vec<String> = text.split(RECORD_TERMINATOR).map(str::to_owned).collect();
        if records.len() > 1 {
            records[0] = format!("{hold_over}{}", records[0]);
            hold_over = records.pop().unwrap_or_default();
            for r in records {
                let mut item = TranslatorItem::new("");
                let mut rec = Record::new();
                rec.import_binary(&r);
                rec.translate(&mut item)?;
                ctx.item_done(item);
            }
        } else {
            hold_over.push_str(&text);
        }
    }
    Ok(())
}
