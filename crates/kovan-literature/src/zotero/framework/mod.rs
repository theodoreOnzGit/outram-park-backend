// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translate, https://github.com/zotero/translate (commit
//   e0fe482b8a07), Zotero utilities, https://github.com/zotero/utilities
//   (commits 4051881d59c6 and 1dd38e27edf8), and the Zotero
//   translation-server, https://github.com/zotero/translation-server
//   (commit 3a9d17614896); each file names the routines it ports.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The translation framework: the parts of Zotero's translate API that
//! import and export translators call, and what the framework does around
//! them.
//!
//! ```text
//! import:  text --ImportInput--> translator --item.complete()--> _itemDone
//!               (Zotero.read)                                   (item_done)
//!          --> ImportResult.items (translator format)
//!          --> itemToAPIJSON (api_json) --> Web API JSON --> ZoteroItem
//!
//! export:  Web API JSON --endpoint prep + ItemGetter (export_items)-->
//!          translator (Zotero.nextItem / Zotero.write) --> text
//! ```
//!
//! | Module | What | Upstream |
//! |---|---|---|
//! | [`item`] | [`TranslatorItem`], the sandbox `Zotero.Item`; [`JsObject`], an ordered JS object | translate.js `_makeSandboxItem` |
//! | [`io`] | `Zotero.read` (lines or n characters), `Zotero.write` | translate.js `IO.String` |
//! | [`options`] | translator headers, `getOption`, `getHiddenPref`, the environment | translate.js, translation-server |
//! | [`item_done`] | `item.complete()`: `_itemDone`, `_cleanTags`, `_cleanTitle` | translate.js |
//! | [`api_json`] | `itemToAPIJSON`, the deterministic key sequence, child-note folding | utilities_item.js, importEndpoint.js |
//! | [`export_items`] | the export endpoint's item preparation, `itemToLegacyExportFormat`, `ItemGetter` | exportEndpoint.js, translate_item.js, utilities_item.js |
//! | [`context`] | [`ImportContext`], [`ExportContext`], [`ImportResult`] | translate.js sandbox |
//! | [`utilities`] | `ZU.cleanAuthor`, `cleanDOI`, `trimInternal`, `text2html`, `removeDiacritics`, ... | utilities.js |
//! | [`html`] | `ZU.unescapeHTML` (HTML5 parse, then `textContent`) | utilities.js + the WHATWG parser |
//! | [`csl`] | `ZU.itemFromCSLJSON` into a translator item, `ZU.itemToCSLJSON` of an export item | utilities_item.js |
//! | [`js`] | JavaScript string semantics (whitespace, `ToString`, truthiness) | ECMA-262 |
//! | [`identifiers`] | `ZU.cleanISBN`, `ZU.cleanISSN` (#749) | utilities.js |
//! | [`title_case`] | `ZU.capitalizeTitle` as the translation-server runs it (#749) | utilities.js, utilities_translate.js |
//! | [`openurl`] | `ZU.createContextObject` (OpenURL 1.0 KEV, for COinS; #749), `ZU.parseContextObject` (XML ContextObject) | openurl.js |
//! | [`xml`] | the XML DOM the XML translators see (`Zotero.getXML`, `DOMParser`, node accessors, mutation, `getElementsByTagName(NS)`, `querySelectorAll`) (#749) | translate.js `parseDOMXML`, jsdom (WHATWG DOM) |
//! | [`xml_parse`] | XML parsing as jsdom drives saxes (well-formedness errors, entities, namespaces) | jsdom xml.js, saxes 6.0.0 |
//! | [`xml_serialize`] | `XMLSerializer.serializeToString`, XML `innerHTML` | w3c-xmlserializer 5.0.0 |
//! | [`xpath`] | `ZU.xpath`, `ZU.xpathText` over wicked-good-xpath's semantics, quirks included | wicked-good-xpath 1.3.1-z002, utilities.js |
//! | [`child`] | child translators (`Zotero.loadTranslator`, METS running MODS/MARCXML) | translate.js |
//! | [`html_dom`] | HTML documents for the note exporters: parsing (html5ever), `outerHTML`, selectors, `element.style` | jsdom, parse5 8.0.0 |
//! | [`rdf`] | `Zotero.RDF`: an RDF store, RDF/XML parser and serializer (#749) | translate's src/rdf/ |
//!
//! **Adding a translator** is one module under `translators/` plus a
//! variant of [`super::translators::Translator`]: an `import` function over
//! an [`ImportContext`] and/or an `export` function over an
//! [`ExportContext`], and `detect_import` where upstream has
//! `detectImport`.
//!
//! **Which upstream this follows.** The translators are ported from
//! translators commit 3d1c78530f42, which is what the reference
//! translation-server ran. ~~The framework follows the translation-server's
//! own submodules (translate e0fe482b8a07, utilities 1dd38e27edf8) wherever
//! they decide output, except that dates and CSL-JSON come from
//! kovan-common, which ports the newer utilities 4051881d59c6 (EDTF dates,
//! ranges, quoted literal dates). Where the two utilities versions disagree
//! the reference comparisons record the difference (see the tests in
//! `tests/zotero_translators.rs`).~~ **CORRECTED 2026-10-07** (#749): the
//! reference server now runs the submodule commits Zotero desktop
//! 9cbba8c4d281 pins (translate dd524aea9a55, utilities 4051881d59c6,
//! zotero-schema b86c79b56479; `reference/manifest.json`), the versions
//! kovan-common ports, so there is no version skew left to record (see the
//! methodology in `tests/zotero_translators.rs`). File headers that cite
//! e0fe482b8a07 / 1dd38e27edf8 name the code read when they were written.
//!
//! **Maturity: AI draft (1).** Not yet human-reviewed.

pub mod api_json;
pub mod context;
pub mod csl;
pub mod export_items;
pub mod html;
// #749 additions.
pub mod identifiers;
pub mod openurl;
pub mod title_case;
#[rustfmt::skip]
pub mod html_entities;
pub mod io;
pub mod item;
pub mod item_done;
pub mod js;
pub mod options;
// RDF data mode for the RDF translators (#749).
pub mod rdf;
pub mod utilities;
// XML translators (#749): DOM, parser, serializer, XPath, child translators.
pub mod child;
pub mod html_dom;
pub mod xml;
pub mod xml_parse;
pub mod xml_serialize;
pub mod xpath;

pub use api_json::{fold_child_notes, item_to_api_json, KeyGenerator};
pub use context::{
    export_input_from_zotero_items, parse_export_input, ExportContext, ImportContext, ImportResult,
    TranslateError,
};
pub use io::{ExportOutput, ImportInput};
pub use item::{
    CollectionChild, JsObject, TranslatorCollection, TranslatorCreator, TranslatorItem,
    TranslatorNote, TranslatorTag,
};
pub use options::{HeaderValue, TranslateOptions, TranslationEnv, TranslatorMetadata};
