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
//! | Refer/BibIX | yes | yes | [`refer`] |
//! | RefWorks Tagged | yes | yes | [`refworks_tagged`] |
//! | Bookmarks | yes | yes | [`bookmarks`] |
//! | MEDLINE/nbib | yes | — | [`medline_nbib`] |
//! | OVID Tagged | yes | — | [`ovid_tagged`] |
//! | Web of Science Tagged | yes | — | [`wos_tagged`] |
//! | MAB2 | yes | — | [`mab2`] |
//! | Datacite JSON | yes | — | [`datacite_json`] |
//! | OpenAlex JSON | yes | — | [`openalex_json`] |
//! | CSV | — | yes | [`csv`] |
//! | COinS | — | yes | [`coins`] |
//! | Wikipedia Citation Templates | — | yes | [`wikipedia_citation_templates`] |
//! | Wikidata QuickStatements | — | yes | [`wikidata_quickstatements`] |
//! | CFF | — | yes | [`cff`] |
//! | CFF References | — | yes | [`cff_references`] |
//! | Simple Evernote Export | — | yes | [`evernote`] |
//!
//! Not ported from #749's list: Note HTML and Note Markdown (they parse the
//! note with a DOM, `DOMParser` and XPath; Note Markdown also bundles
//! turndown), which wait for the XML/DOM layer.

pub mod biblatex;
pub mod bibtex;
pub mod csl_json;
pub mod ris;
// Tagged-text, JSON and simple export translators (#749)
pub mod refer;
pub mod refworks_tagged;
pub mod bookmarks;
pub mod medline_nbib;
pub mod ovid_tagged;
pub mod wos_tagged;
pub mod mab2;
pub mod datacite_json;
pub mod openalex_json;
pub mod csv;
pub mod coins;
pub mod wikipedia_citation_templates;
pub mod wikidata_quickstatements;
pub mod cff;
pub mod cff_references;
pub mod evernote;

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
    // Tagged-text, JSON and simple export translators (#749)
    /// `ReferBibIX.js` (import and export).
    Refer,
    /// `RefWorks Tagged.js` (import and export).
    RefWorksTagged,
    /// `Bookmarks.js` (import and export).
    Bookmarks,
    /// `MEDLINEnbib.js` (import).
    MedlineNbib,
    /// `OVID Tagged.js` (import).
    OvidTagged,
    /// `Web of Science Tagged.js` (import).
    WosTagged,
    /// `MAB2.js` (import).
    Mab2,
    /// `Datacite JSON.js` (import).
    DataciteJson,
    /// `OpenAlex JSON.js` (import).
    OpenAlexJson,
    /// `CSV.js` (export).
    Csv,
    /// `COinS.js` (export).
    Coins,
    /// `Wikipedia Citation Templates.js` (export).
    WikipediaCitationTemplates,
    /// `Wikidata QuickStatements.js` (export).
    WikidataQuickStatements,
    /// `CFF.js` (export).
    Cff,
    /// `CFF References.js` (export).
    CffReferences,
    /// `Evernote.js` (export).
    Evernote,
}

impl Translator {
    /// Every ported translator. The import translators come in the order
    /// the translation-server tries them for import detection: by
    /// `priority`, ties in file-name order (its stable sort over the
    /// translators directory, which Node's `readdir` lists in byte order;
    /// translators.js:76-86). Export-only translators are not tried, so
    /// their place does not matter.
    pub const ALL: [Translator; 20] = [
        Translator::Bookmarks,
        Translator::CslJson,
        Translator::DataciteJson,
        Translator::Mab2,
        Translator::MedlineNbib,
        Translator::OvidTagged,
        Translator::OpenAlexJson,
        Translator::Ris,
        Translator::RefWorksTagged,
        Translator::Refer,
        Translator::WosTagged,
        Translator::BibLaTeX,
        Translator::BibTeX,
        // Export only (#749).
        Translator::Csv,
        Translator::Coins,
        Translator::WikipediaCitationTemplates,
        Translator::WikidataQuickStatements,
        Translator::Cff,
        Translator::CffReferences,
        Translator::Evernote,
    ];

    /// The translator's header.
    pub fn metadata(self) -> &'static TranslatorMetadata {
        match self {
            Translator::BibTeX => &bibtex::METADATA,
            Translator::BibLaTeX => &biblatex::METADATA,
            Translator::Ris => &ris::METADATA,
            Translator::CslJson => &csl_json::METADATA,
            Translator::Refer => &refer::METADATA,
            Translator::RefWorksTagged => &refworks_tagged::METADATA,
            Translator::Bookmarks => &bookmarks::METADATA,
            Translator::MedlineNbib => &medline_nbib::METADATA,
            Translator::OvidTagged => &ovid_tagged::METADATA,
            Translator::WosTagged => &wos_tagged::METADATA,
            Translator::Mab2 => &mab2::METADATA,
            Translator::DataciteJson => &datacite_json::METADATA,
            Translator::OpenAlexJson => &openalex_json::METADATA,
            Translator::Csv => &csv::METADATA,
            Translator::Coins => &coins::METADATA,
            Translator::WikipediaCitationTemplates => &wikipedia_citation_templates::METADATA,
            Translator::WikidataQuickStatements => &wikidata_quickstatements::METADATA,
            Translator::Cff => &cff::METADATA,
            Translator::CffReferences => &cff_references::METADATA,
            Translator::Evernote => &evernote::METADATA,
        }
    }

    /// The translation-server's format name (`/export?format=`,
    /// formats.js). A translator the server has no name for (an import-only
    /// translator; Wikidata QuickStatements, CFF, CFF References) has a
    /// kovan name, its file name in snake case, which is also the name of
    /// its reference files under `tests/data/zotero/reference/`.
    pub fn format_name(self) -> &'static str {
        match self {
            Translator::BibTeX => "bibtex",
            Translator::BibLaTeX => "biblatex",
            Translator::Ris => "ris",
            Translator::CslJson => "csljson",
            Translator::Refer => "refer",
            Translator::RefWorksTagged => "refworks_tagged",
            Translator::Bookmarks => "bookmarks",
            Translator::MedlineNbib => "medline_nbib",
            Translator::OvidTagged => "ovid_tagged",
            Translator::WosTagged => "wos_tagged",
            Translator::Mab2 => "mab2",
            Translator::DataciteJson => "datacite_json",
            Translator::OpenAlexJson => "openalex_json",
            Translator::Csv => "csv",
            Translator::Coins => "coins",
            Translator::WikipediaCitationTemplates => "wikipedia",
            Translator::WikidataQuickStatements => "wikidata_quickstatements",
            Translator::Cff => "cff",
            Translator::CffReferences => "cff_references",
            Translator::Evernote => "evernote",
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
            Translator::Refer => refer::detect_import(&mut ctx),
            Translator::RefWorksTagged => refworks_tagged::detect_import(&mut ctx),
            Translator::Bookmarks => bookmarks::detect_import(&mut ctx),
            Translator::MedlineNbib => medline_nbib::detect_import(&mut ctx),
            Translator::OvidTagged => ovid_tagged::detect_import(&mut ctx),
            Translator::WosTagged => wos_tagged::detect_import(&mut ctx),
            Translator::Mab2 => mab2::detect_import(&mut ctx),
            Translator::DataciteJson => datacite_json::detect_import(&mut ctx),
            Translator::OpenAlexJson => openalex_json::detect_import(&mut ctx),
            Translator::BibLaTeX
            | Translator::Csv
            | Translator::Coins
            | Translator::WikipediaCitationTemplates
            | Translator::WikidataQuickStatements
            | Translator::Cff
            | Translator::CffReferences
            | Translator::Evernote => false,
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
            Translator::Refer => refer::do_import(&mut ctx)?,
            Translator::RefWorksTagged => refworks_tagged::do_import(&mut ctx)?,
            Translator::Bookmarks => bookmarks::do_import(&mut ctx)?,
            Translator::MedlineNbib => medline_nbib::do_import(&mut ctx)?,
            Translator::OvidTagged => ovid_tagged::do_import(&mut ctx)?,
            Translator::WosTagged => wos_tagged::do_import(&mut ctx)?,
            Translator::Mab2 => mab2::do_import(&mut ctx)?,
            Translator::DataciteJson => datacite_json::do_import(&mut ctx)?,
            Translator::OpenAlexJson => openalex_json::do_import(&mut ctx)?,
            Translator::BibLaTeX
            | Translator::Csv
            | Translator::Coins
            | Translator::WikipediaCitationTemplates
            | Translator::WikidataQuickStatements
            | Translator::Cff
            | Translator::CffReferences
            | Translator::Evernote => unreachable!("checked can_import"),
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
            Translator::Refer => refer::do_export(&mut ctx)?,
            Translator::RefWorksTagged => refworks_tagged::do_export(&mut ctx)?,
            Translator::Bookmarks => bookmarks::do_export(&mut ctx)?,
            Translator::Csv => csv::do_export(&mut ctx)?,
            Translator::Coins => coins::do_export(&mut ctx)?,
            Translator::WikipediaCitationTemplates => {
                wikipedia_citation_templates::do_export(&mut ctx)?
            }
            Translator::WikidataQuickStatements => wikidata_quickstatements::do_export(&mut ctx)?,
            Translator::Cff => cff::do_export(&mut ctx)?,
            Translator::CffReferences => cff_references::do_export(&mut ctx)?,
            Translator::Evernote => evernote::do_export(&mut ctx)?,
            Translator::MedlineNbib => unreachable!("checked can_export"),
            Translator::OvidTagged => unreachable!("checked can_export"),
            Translator::WosTagged => unreachable!("checked can_export"),
            Translator::Mab2 => unreachable!("checked can_export"),
            Translator::DataciteJson => unreachable!("checked can_export"),
            Translator::OpenAlexJson => unreachable!("checked can_export"),
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
