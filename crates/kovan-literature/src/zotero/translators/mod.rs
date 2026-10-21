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
//! | Note HTML | — | yes | [`note_html`] (#749, DOM) |
//! | Note Markdown | — | yes | [`note_markdown`] (#749, DOM; turndown) |
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
//! ~~Not ported from #749's list: Note HTML and Note Markdown (they parse the
//! note with a DOM, `DOMParser` and XPath; Note Markdown also bundles
//! turndown), which wait for the XML/DOM layer.~~ **CORRECTED 2026-10-07**:
//! both ported on the XML/DOM layer (`framework::xml`, `framework::xpath`).
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
pub mod note_html;
pub mod note_markdown;
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
    /// `Note HTML.js` (export).
    NoteHtml,
    /// `Note Markdown.js` (export).
    NoteMarkdown,
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
    /// Every ported translator. The import translators come in the order
    /// the translation-server tries them for import detection: by
    /// `priority`, ties in file-name order (its stable sort over the
    /// translators directory, which Node's `readdir` lists in byte order;
    /// translators.js:76-104; checked with `fs.readdirSync` on
    /// vendor/translators, 2026-10-07). Export-only translators are not
    /// tried, so their place does not matter.
    ///
    /// The RDF translators (#749): Bibliontology RDF has priority 50 and
    /// RDF.js 100 ("RDF.js" sorts between "PubMed XML.js" and "RIS.js");
    /// Zotero RDF and Unqualified Dublin Core RDF are export-only.
    pub const ALL: [Translator; 38] = [
        // Import, priority 50.
        Translator::BibliontologyRdf,
        Translator::Mets,
        Translator::Mods,
        // Import, priority 100.
        Translator::Bookmarks,
        Translator::CslJson,
        Translator::Citavi5Xml,
        Translator::CrossrefUnixrefXml,
        Translator::DSpaceIntermediateMetadata,
        Translator::DataciteJson,
        Translator::EndnoteXml,
        Translator::Mab2,
        Translator::Marc,
        Translator::MarcXml,
        Translator::MedlineNbib,
        Translator::OvidTagged,
        Translator::OpenAlexJson,
        Translator::PrimoNormalizedXml,
        Translator::PubMedXml,
        Translator::Rdf,
        Translator::Ris,
        Translator::RefWorksTagged,
        Translator::Refer,
        Translator::WosTagged,
        Translator::XmlContextObject,
        // Import, priority 200.
        Translator::BibTeX,
        // Export only.
        Translator::BibLaTeX,
        Translator::Tei,
        Translator::Csv,
        Translator::Coins,
        Translator::WikipediaCitationTemplates,
        Translator::WikidataQuickStatements,
        Translator::Cff,
        Translator::CffReferences,
        Translator::Evernote,
        Translator::NoteHtml,
        Translator::NoteMarkdown,
        Translator::ZoteroRdf,
        Translator::DcRdf,
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
            Translator::NoteHtml => &note_html::METADATA,
            Translator::NoteMarkdown => &note_markdown::METADATA,
            Translator::ZoteroRdf => &zotero_rdf::METADATA,
            Translator::Rdf => &rdf::METADATA,
            Translator::BibliontologyRdf => &bibliontology::METADATA,
            Translator::DcRdf => &dc_rdf::METADATA,
        }
    }

    /// The translation-server's format name (`/export?format=`,
    /// formats.js). An import-only translator has none there; it gets a
    /// kovan name (its file name in snake case), which is also the name of
    /// its reference file (`tests/data/zotero/reference/import/<name>.json`).
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
            Translator::NoteHtml => "note_html",
            Translator::NoteMarkdown => "note_markdown",
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
            | Translator::Evernote
            | Translator::NoteHtml
            | Translator::NoteMarkdown => false,
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
            | Translator::Evernote
            | Translator::NoteHtml
            | Translator::NoteMarkdown => unreachable!("checked can_import"),
            Translator::Rdf => rdf::do_import(&mut ctx)?,
            Translator::BibliontologyRdf => bibliontology::do_import(&mut ctx)?,
            Translator::ZoteroRdf | Translator::DcRdf => {
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
            Translator::NoteHtml => note_html::do_export(&mut ctx)?,
            Translator::NoteMarkdown => note_markdown::do_export(&mut ctx)?,
            Translator::MedlineNbib => unreachable!("checked can_export"),
            Translator::OvidTagged => unreachable!("checked can_export"),
            Translator::WosTagged => unreachable!("checked can_export"),
            Translator::Mab2 => unreachable!("checked can_export"),
            Translator::DataciteJson => unreachable!("checked can_export"),
            Translator::OpenAlexJson => unreachable!("checked can_export"),
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
