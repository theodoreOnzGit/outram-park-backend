// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): BibTeX.js (translatorID
//   9cb70025-a888-4a29-a210-93ec52da40d4, lastUpdated 2026-08-03 15:56:33):
//   header :1-22, `detectImport` :42-82, `fieldMap` :89-107,
//   `extraIdentifiers`/`revExtraIds` :119-141, `months` :277-278.
// Copyright (C) 2019 CHNM, Simon Kornblith, Richard Karnesky and Emiliano
//   heyns.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The BibTeX translator: import and export.
//!
//! | File | Upstream |
//! |---|---|
//! | [`import`] | `doImport` and everything it calls (parser, `processField`, `unescapeBibTeX`, JabRef groups) |
//! | [`export`] | `doExport` (`writeField`, escaping, case protection, creators, Extra identifiers) |
//! | [`text`] | `splitUnprotected`, `unescapeBibTeX`, `mapTeXmarkup`, file records |
//! | [`common`] | Extra parsing, citation keys, file paths (shared with BibLaTeX) |
//! | [`mapping_table`], [`reverse_mapping_table`] | the two LaTeX <-> Unicode tables, generated from the JS |
//!
//! Not ported: `setKeywordSplitOnSpace` and `setKeywordDelimRe`, the
//! `exports` other translators call through `loadTranslator` (no ported
//! translator calls them; the defaults they would change are used).

pub mod common;
pub mod export;
pub mod import;
#[rustfmt::skip]
pub mod mapping_table;
#[rustfmt::skip]
pub mod reverse_mapping_table;
pub mod text;

pub use export::do_export;
pub use import::do_import;

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::ImportContext;
use regex::Regex;
use std::sync::OnceLock;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "9cb70025-a888-4a29-a210-93ec52da40d4",
    label: "BibTeX",
    creator: "Simon Kornblith, Richard Karnesky and Emiliano heyns",
    target: "bib",
    min_version: "2.1.9",
    priority: 200,
    translator_type: translator_type::IMPORT | translator_type::EXPORT,
    config_options: &[
        ("async", HeaderValue::Bool(true)),
        ("getCollections", HeaderValue::Bool(true)),
    ],
    display_options: &[
        ("exportCharset", HeaderValue::Str("UTF-8")),
        ("exportNotes", HeaderValue::Bool(true)),
        ("exportFileData", HeaderValue::Bool(false)),
        ("useJournalAbbreviation", HeaderValue::Bool(false)),
    ],
    hidden_prefs: &[],
    last_updated: "2026-08-03 15:56:33",
};

/// `fieldMap` (:89-107): BibTeX field -> Zotero field, in property order.
pub(crate) const FIELD_MAP: [(&str, &str); 17] = [
    ("address", "place"),
    ("chapter", "section"),
    ("edition", "edition"),
    ("type", "type"),
    ("series", "series"),
    ("title", "title"),
    ("volume", "volume"),
    ("copyright", "rights"),
    ("isbn", "ISBN"),
    ("issn", "ISSN"),
    ("shorttitle", "shortTitle"),
    ("url", "url"),
    ("doi", "DOI"),
    ("abstract", "abstractNote"),
    ("nationality", "country"),
    ("language", "language"),
    ("assignee", "assignee"),
];

/// `extraIdentifiers` (:119-135): BibTeX field -> Extra label.
pub(crate) const EXTRA_IDENTIFIERS: [(&str, &str); 5] = [
    ("lccn", "LCCN"),
    ("mrnumber", "MR"),
    ("zmnumber", "Zbl"),
    ("pmid", "PMID"),
    ("pmcid", "PMCID"),
];

/// `revExtraIds` (:138-141): Extra label -> BibTeX field.
pub(crate) const REV_EXTRA_IDS: [(&str, &str); 6] = [
    ("DOI", "doi"),
    ("LCCN", "lccn"),
    ("MR", "mrnumber"),
    ("Zbl", "zmnumber"),
    ("PMID", "pmid"),
    ("PMCID", "pmcid"),
];

/// `months` (:277-278).
pub(crate) const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

/// `detectImport` (:42-82): a line (or the end of a 4096-character read)
/// whose non-space text starts with `@type{` or `@type(`; `%` comments to
/// the end of the line; at most 1 MiB scanned. Falls off the end
/// (`undefined`, falsy) when nothing matches.
pub fn detect_import(ctx: &mut ImportContext) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(&format!(
            r"^{}*@[a-zA-Z]+[\(\{{]",
            crate::zotero::framework::js::WS
        ))
        .unwrap()
    });
    let max_chars = 1_048_576usize;
    let mut in_comment = false;
    let mut block = String::new();
    let mut chars_read = 0usize;
    loop {
        // `(buffer = Zotero.read(4096)) && charsRead < maxChars`
        let Some(buffer) = ctx.read_chars(4096).filter(|b| !b.is_empty()) else {
            return false;
        };
        if chars_read >= max_chars {
            return false;
        }
        let buf: Vec<char> = buffer.chars().collect();
        chars_read += buf.len();
        for (i, &chr) in buf.iter().enumerate() {
            if in_comment && chr != '\r' && chr != '\n' {
                continue;
            }
            in_comment = false;
            if chr == '%' {
                block.clear();
                in_comment = true;
            } else if (chr == '\n' || chr == '\r' || i == buf.len() - 1) && !block.is_empty() {
                if re.is_match(&block) {
                    return true;
                }
                block.clear();
            } else if !" \n\r\t".contains(chr) {
                block.push(chr);
            }
        }
    }
}
