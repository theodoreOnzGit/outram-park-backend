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
//! | Zotero RDF | — | yes | [`zotero_rdf`] |
//! | RDF | yes | — | [`rdf`] |
//! | Bibliontology RDF | yes | yes | [`bibliontology`] |
//! | Unqualified Dublin Core RDF | — | yes | [`dc_rdf`] |
//!
//! The four RDF translators (#749) run on the framework's RDF data mode
//! ([`super::framework::rdf`]); [`rdf_support`] holds the JavaScript
//! behaviour they share.

pub mod biblatex;
pub mod bibtex;
pub mod csl_json;
pub mod ris;
// RDF translators (#749)
pub mod bibliontology;
pub mod dc_rdf;
pub mod rdf;
pub mod rdf_creator_types;
pub mod rdf_support;
pub mod zotero_rdf;

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
    // RDF translators (#749)
    /// `Zotero RDF.js` (export).
    ZoteroRdf,
    /// `RDF.js` (import).
    Rdf,
    /// `Bibliontology RDF.js` (import and export).
    BibliontologyRdf,
    /// `Unqualified Dublin Core RDF.js` (export).
    DcRdf,
}

impl Translator {
    /// Every ported translator, in the order the translation-server tries
    /// them for import detection: by `priority`, ties in file-name order
    /// (its stable sort over the translators directory, translators.js:76-86).
    ///
    /// The RDF translators (#749): Zotero RDF has priority 25, Bibliontology
    /// RDF 50, RDF and Unqualified Dublin Core RDF 100 ("RDF.js" sorts
    /// between "CSL JSON.js" and "RIS.js").
    pub const ALL: [Translator; 8] = [
        Translator::ZoteroRdf,
        Translator::BibliontologyRdf,
        Translator::CslJson,
        Translator::Rdf,
        Translator::Ris,
        Translator::DcRdf,
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
            Translator::ZoteroRdf => &zotero_rdf::METADATA,
            Translator::Rdf => &rdf::METADATA,
            Translator::BibliontologyRdf => &bibliontology::METADATA,
            Translator::DcRdf => &dc_rdf::METADATA,
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
            Translator::ZoteroRdf => "rdf_zotero",
            // RDF.js has no export format; this names its import fixtures.
            Translator::Rdf => "rdf",
            Translator::BibliontologyRdf => "rdf_bibliontology",
            Translator::DcRdf => "rdf_dc",
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
            Translator::Rdf => rdf::detect_import(&mut ctx),
            Translator::BibliontologyRdf => bibliontology::detect_import(&mut ctx),
            Translator::ZoteroRdf | Translator::DcRdf => false,
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
            Translator::Rdf => rdf::do_import(&mut ctx)?,
            Translator::BibliontologyRdf => bibliontology::do_import(&mut ctx)?,
            Translator::BibLaTeX | Translator::ZoteroRdf | Translator::DcRdf => {
                unreachable!("checked can_import")
            }
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
        self.export_with_collections(items, &[], options)
    }

    /// Export items and collections (#749): the collections are what
    /// `Zotero.nextCollection()` hands out, in Zotero's export format (see
    /// [`ExportContext::set_collections`]); only Zotero RDF reads them.
    /// [`Translator::export`] passes none, as the translation-server does.
    pub fn export_with_collections(
        self,
        items: &[JsObject],
        collections: &[JsObject],
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
        ctx.set_collections(collections.to_vec());
        match self {
            Translator::BibTeX => bibtex::do_export(&mut ctx)?,
            Translator::BibLaTeX => biblatex::do_export(&mut ctx)?,
            Translator::Ris => ris::do_export(&mut ctx)?,
            Translator::CslJson => csl_json::do_export(&mut ctx)?,
            Translator::ZoteroRdf => zotero_rdf::do_export(&mut ctx, items)?,
            Translator::BibliontologyRdf => bibliontology::do_export(&mut ctx, items)?,
            Translator::DcRdf => dc_rdf::do_export(&mut ctx)?,
            Translator::Rdf => unreachable!("checked can_export"),
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
