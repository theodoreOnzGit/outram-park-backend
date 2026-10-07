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
//! | MODS | yes | yes | [`mods`] (#749, XML) |
//! | Endnote XML | yes | yes | [`endnote_xml`] (#749, XML) |
//! | TEI | — | yes | [`tei`] (#749, XML) |
//! | Crossref Unixref XML | yes | — | [`crossref_unixref_xml`] (#749, XML) |
//! | MARCXML | yes | — | [`marcxml`] (#749, XML; runs [`marc`]'s record model) |
//! | MARC | yes | — | [`marc`] (#749, binary/line MARC) |
//! | PubMed XML | yes | — | [`pubmed_xml`] (#749, XML) |
//! | METS | yes | — | [`mets`] (#749, XML; runs MODS or MARCXML as child translators) |
//! | Primo Normalized XML | yes | — | [`primo_normalized_xml`] (#749, XML) |
//! | DSpace Intermediate Metadata | yes | — | [`dspace_intermediate_metadata`] (#749, XML) |
//! | Citavi 5 XML | yes | — | [`citavi5_xml`] (#749, XML) |
//! | XML ContextObject | yes | — | [`xml_contextobject`] (#749, XML; OpenURL) |

pub mod biblatex;
pub mod bibtex;
pub mod csl_json;
pub mod ris;
// XML translators (#749)
pub mod citavi5_xml;
pub mod crossref_unixref_xml;
pub mod dspace_intermediate_metadata;
pub mod endnote_xml;
pub mod marc;
pub mod marcxml;
pub mod mets;
pub mod mods;
pub mod primo_normalized_xml;
pub mod pubmed_xml;
pub mod tei;
pub mod xml_contextobject;

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
    // XML translators (#749)
    /// `MODS.js` (import and export).
    Mods,
    /// `Endnote XML.js` (import and export).
    EndnoteXml,
    /// `TEI.js` (export).
    Tei,
    /// `Crossref Unixref XML.js` (import).
    CrossrefUnixrefXml,
    /// `MARCXML.js` (import).
    MarcXml,
    /// `MARC.js` (import).
    Marc,
    /// `PubMed XML.js` (import).
    PubMedXml,
    /// `METS.js` (import).
    Mets,
    /// `Primo Normalized XML.js` (import).
    PrimoNormalizedXml,
    /// `DSpace Intermediate Metadata.js` (import).
    DSpaceIntermediateMetadata,
    /// `Citavi 5 XML.js` (import).
    Citavi5Xml,
    /// `XML ContextObject.js` (import).
    XmlContextObject,
}

impl Translator {
    /// Every ported translator, in the order the translation-server tries
    /// them for import detection: by `priority`, ties in file-name order
    /// (its stable sort over `fs.readdir` of the translators directory,
    /// which lists names in byte order, translators.js:76-104).
    ///
    /// ~~`[CslJson, Ris, BibLaTeX, BibTeX]`~~ **CORRECTED 2026-10-07**
    /// (#749): BibLaTeX.js sorts before CSL JSON.js (checked with
    /// `fs.readdirSync` on vendor/translators). It is export-only, so the old
    /// order never changed a detection.
    pub const ALL: [Translator; 16] = [
        // priority 25
        Translator::Tei,
        // priority 50
        Translator::Mets,
        Translator::Mods,
        // priority 100
        Translator::BibLaTeX,
        Translator::CslJson,
        Translator::Citavi5Xml,
        Translator::CrossrefUnixrefXml,
        Translator::DSpaceIntermediateMetadata,
        Translator::EndnoteXml,
        Translator::Marc,
        Translator::MarcXml,
        Translator::PrimoNormalizedXml,
        Translator::PubMedXml,
        Translator::Ris,
        Translator::XmlContextObject,
        // priority 200
        Translator::BibTeX,
    ];

    /// The translator's header.
    pub fn metadata(self) -> &'static TranslatorMetadata {
        match self {
            Translator::BibTeX => &bibtex::METADATA,
            Translator::BibLaTeX => &biblatex::METADATA,
            Translator::Ris => &ris::METADATA,
            Translator::CslJson => &csl_json::METADATA,
            // XML translators (#749)
            Translator::Mods => &mods::METADATA,
            Translator::EndnoteXml => &endnote_xml::METADATA,
            Translator::Tei => &tei::METADATA,
            Translator::CrossrefUnixrefXml => &crossref_unixref_xml::METADATA,
            Translator::MarcXml => &marcxml::METADATA,
            Translator::Marc => &marc::METADATA,
            Translator::PubMedXml => &pubmed_xml::METADATA,
            Translator::Mets => &mets::METADATA,
            Translator::PrimoNormalizedXml => &primo_normalized_xml::METADATA,
            Translator::DSpaceIntermediateMetadata => &dspace_intermediate_metadata::METADATA,
            Translator::Citavi5Xml => &citavi5_xml::METADATA,
            Translator::XmlContextObject => &xml_contextobject::METADATA,
        }
    }

    /// The translation-server's format name (`/export?format=`,
    /// formats.js). An import-only translator has none there; it gets a
    /// kovan name (its file name in snake case), which is also the name of
    /// its reference file (`tests/data/zotero/reference/import/<name>.json`).
    pub fn format_name(self) -> &'static str {
        match self {
            Translator::BibTeX => "bibtex",
            Translator::BibLaTeX => "biblatex",
            Translator::Ris => "ris",
            Translator::CslJson => "csljson",
            // XML translators (#749)
            Translator::Mods => "mods",
            Translator::EndnoteXml => "endnote_xml",
            Translator::Tei => "tei",
            Translator::CrossrefUnixrefXml => "crossref_unixref_xml",
            Translator::MarcXml => "marcxml",
            Translator::Marc => "marc",
            Translator::PubMedXml => "pubmed_xml",
            Translator::Mets => "mets",
            Translator::PrimoNormalizedXml => "primo_normalized_xml",
            Translator::DSpaceIntermediateMetadata => "dspace_intermediate_metadata",
            Translator::Citavi5Xml => "citavi5_xml",
            Translator::XmlContextObject => "xml_contextobject",
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
            // XML translators (#749)
            Translator::Mods => mods::detect_import(&mut ctx),
            Translator::EndnoteXml => endnote_xml::detect_import(&mut ctx),
            Translator::CrossrefUnixrefXml => crossref_unixref_xml::detect_import(&mut ctx),
            Translator::MarcXml => marcxml::detect_import(&mut ctx),
            Translator::Marc => marc::detect_import(&mut ctx),
            Translator::PubMedXml => pubmed_xml::detect_import(&mut ctx),
            Translator::Mets => mets::detect_import(&mut ctx),
            Translator::PrimoNormalizedXml => primo_normalized_xml::detect_import(&mut ctx),
            Translator::DSpaceIntermediateMetadata => {
                dspace_intermediate_metadata::detect_import(&mut ctx)
            }
            Translator::Citavi5Xml => citavi5_xml::detect_import(&mut ctx),
            Translator::XmlContextObject => xml_contextobject::detect_import(&mut ctx),
            Translator::Tei => false,
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
            // XML translators (#749)
            Translator::Mods => mods::do_import(&mut ctx)?,
            Translator::EndnoteXml => endnote_xml::do_import(&mut ctx)?,
            Translator::CrossrefUnixrefXml => crossref_unixref_xml::do_import(&mut ctx)?,
            Translator::MarcXml => marcxml::do_import(&mut ctx)?,
            Translator::Marc => marc::do_import(&mut ctx)?,
            Translator::PubMedXml => pubmed_xml::do_import(&mut ctx)?,
            Translator::Mets => mets::do_import(&mut ctx)?,
            Translator::PrimoNormalizedXml => primo_normalized_xml::do_import(&mut ctx)?,
            Translator::DSpaceIntermediateMetadata => {
                dspace_intermediate_metadata::do_import(&mut ctx)?
            }
            Translator::Citavi5Xml => citavi5_xml::do_import(&mut ctx)?,
            Translator::XmlContextObject => xml_contextobject::do_import(&mut ctx)?,
            Translator::Tei => unreachable!("checked can_import"),
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
            // XML translators (#749)
            Translator::Mods => mods::do_export(&mut ctx)?,
            Translator::EndnoteXml => endnote_xml::do_export(&mut ctx)?,
            Translator::Tei => tei::do_export(&mut ctx)?,
            Translator::CrossrefUnixrefXml
            | Translator::MarcXml
            | Translator::Marc
            | Translator::PubMedXml
            | Translator::Mets
            | Translator::PrimoNormalizedXml
            | Translator::DSpaceIntermediateMetadata
            | Translator::Citavi5Xml
            | Translator::XmlContextObject => unreachable!("checked can_export"),
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
