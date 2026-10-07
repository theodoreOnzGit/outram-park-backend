// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42); translator selection as the translation-server
//   does it (https://github.com/zotero/translation-server, commit
//   3a9d17614896: src/translators.js :76-86, src/importEndpoint.js :31-40,
//   src/formats.js).
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! Zotero's import/export translators, one module each, behind the
//! [`Translator`] enum (no trait objects: a new translator is a new variant
//! and a new module).
//!
//! | Translator | Import | Export | Module |
//! |---|---|---|---|
//! | BibTeX | yes | yes | [`bibtex`] |
//! | BibLaTeX | — | yes | [`biblatex`] |
//! | RIS | yes | yes | [`ris`] |
//! | CSL JSON | yes | yes | [`csl_json`] |

pub mod biblatex;
pub mod bibtex;
pub mod csl_json;
pub mod ris;

use super::framework::options::TranslatorMetadata;
use super::framework::{
    export_input_from_zotero_items, parse_export_input, ExportContext, ImportContext, ImportResult,
    JsObject, TranslateError, TranslateOptions,
};
use kovan_common::zotero::ZoteroItem;

/// A ported Zotero translator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Translator {
    /// `BibTeX.js` (import and export).
    BibTeX,
    /// `BibLaTeX.js` (export).
    BibLaTeX,
    /// `RIS.js` (import and export).
    Ris,
    /// `CSL JSON.js` (import and export).
    CslJson,
}

impl Translator {
    /// Every ported translator, in the order the translation-server tries
    /// them for import detection: by `priority`, ties in file-name order
    /// (its stable sort over the translators directory, translators.js:76-86).
    pub const ALL: [Translator; 4] = [
        Translator::CslJson,
        Translator::Ris,
        Translator::BibLaTeX,
        Translator::BibTeX,
    ];

    /// The translator's header.
    pub fn metadata(self) -> &'static TranslatorMetadata {
        match self {
            Translator::BibTeX => &bibtex::METADATA,
            Translator::BibLaTeX => &biblatex::METADATA,
            Translator::Ris => &ris::METADATA,
            Translator::CslJson => &csl_json::METADATA,
        }
    }

    /// The translation-server's format name (`/export?format=`,
    /// formats.js).
    pub fn format_name(self) -> &'static str {
        match self {
            Translator::BibTeX => "bibtex",
            Translator::BibLaTeX => "biblatex",
            Translator::Ris => "ris",
            Translator::CslJson => "csljson",
        }
    }

    /// The translator with this format name.
    pub fn from_format_name(name: &str) -> Option<Translator> {
        Translator::ALL
            .into_iter()
            .find(|t| t.format_name() == name)
    }

    /// The translator with this `translatorID`.
    pub fn from_id(id: &str) -> Option<Translator> {
        Translator::ALL.into_iter().find(|t| t.metadata().id == id)
    }

    /// The options the translator runs with by default (its header's
    /// display options, the translation-server environment).
    pub fn default_options(self) -> TranslateOptions {
        TranslateOptions::for_translator(self.metadata())
    }

    /// `detectImport` on `input` (false for an export-only translator).
    pub fn detect_import(self, input: &str) -> bool {
        if !self.metadata().can_import() {
            return false;
        }
        let mut ctx = ImportContext::new(input, self.metadata(), self.default_options());
        match self {
            Translator::BibTeX => bibtex::detect_import(&mut ctx),
            Translator::Ris => ris::detect_import(&mut ctx),
            Translator::CslJson => csl_json::detect_import(&mut ctx),
            Translator::BibLaTeX => false,
        }
    }

    /// Import `input` (`doImport`), returning the saved items.
    pub fn import(
        self,
        input: &str,
        options: &TranslateOptions,
    ) -> Result<ImportResult, TranslateError> {
        let meta = self.metadata();
        if !meta.can_import() {
            return Err(TranslateError::Unsupported {
                translator: meta.label,
                direction: "import",
            });
        }
        let mut ctx = ImportContext::new(input, meta, options.clone());
        match self {
            Translator::BibTeX => bibtex::do_import(&mut ctx)?,
            Translator::Ris => ris::do_import(&mut ctx)?,
            Translator::CslJson => csl_json::do_import(&mut ctx)?,
            Translator::BibLaTeX => unreachable!("checked can_import"),
        }
        Ok(ctx.finish())
    }

    /// Export items given as JSON objects in their own key order (Web API
    /// or export format; see [`parse_export_input`]).
    pub fn export(
        self,
        items: &[JsObject],
        options: &TranslateOptions,
    ) -> Result<String, TranslateError> {
        let meta = self.metadata();
        if !meta.can_export() {
            return Err(TranslateError::Unsupported {
                translator: meta.label,
                direction: "export",
            });
        }
        let mut ctx = ExportContext::new(items, meta, options.clone());
        match self {
            Translator::BibTeX => bibtex::do_export(&mut ctx)?,
            Translator::BibLaTeX => biblatex::do_export(&mut ctx)?,
            Translator::Ris => ris::do_export(&mut ctx)?,
            Translator::CslJson => csl_json::do_export(&mut ctx)?,
        }
        Ok(ctx.finish())
    }

    /// Export a JSON array of items (what the translation-server's
    /// `/export` takes).
    pub fn export_json(
        self,
        json: &str,
        options: &TranslateOptions,
    ) -> Result<String, TranslateError> {
        self.export(&parse_export_input(json)?, options)
    }

    /// Export kovan-common items.
    pub fn export_zotero_items(
        self,
        items: &[ZoteroItem],
        options: &TranslateOptions,
    ) -> Result<String, TranslateError> {
        self.export(&export_input_from_zotero_items(items), options)
    }
}

/// The import translators whose `detectImport` accepts `input`, in the
/// order the translation-server tries them; its `/import` uses the first.
/// (Upstream tries all of Zotero's import translators; this covers the
/// ported ones only.)
pub fn detect_import(input: &str) -> Vec<Translator> {
    Translator::ALL
        .into_iter()
        .filter(|t| t.detect_import(input))
        .collect()
}
