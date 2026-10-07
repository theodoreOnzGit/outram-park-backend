// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translate, https://github.com/zotero/translate (commit
//   dd524aea9a55, the submodule Zotero desktop 9cbba8c4d281 pins):
//   src/rdf/ (term.js, identity.js, rdfparser.js, serialize.js, uri.js:
//   the AJAW / Tabulator RDF library, W3C Software Notice and License) and
//   src/translation/translate.js (`Zotero.RDF`, the RDF data mode of
//   `IO.String`). Each file names what it ports.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA;
//   (c) 2005-2006 World Wide Web Consortium (MIT, ERCIM, Keio).
// Licence: AGPL-3.0 (upstream translate: AGPL-3.0-or-later; the rdf/
//   library: W3C Software Notice and License, GPL-compatible; NOTICE has
//   its text).

//! RDF for the translators whose `dataMode` is `rdf/xml` (Zotero RDF, RDF,
//! Bibliontology RDF, Unqualified Dublin Core RDF).
//!
//! ```text
//! import:  text --dom (framework::xml)--> DOM --parser (rdfparser.js)--> Store
//!          translator <--Zotero.RDF (sandbox)--> Store
//! export:  translator --Zotero.RDF--> Store --serializer (serialize.js)--> text
//! ```
//!
//! | Module | What | Upstream |
//! |---|---|---|
//! | [`term`] | terms (symbol, blank node, literal, collection) and their order | term.js |
//! | [`store`] | `IndexedFormula`: statements, indexes, `owl:sameAs` smushing | identity.js |
//! | [`dom`] | the XML DOM the parser walks (a mutable copy of the framework XML layer's parse) | translate.js `parseDOMXML` + jsdom |
//! | [`parser`] | RDF/XML to triples | rdfparser.js |
//! | [`serializer`] | triples to RDF/XML, byte for byte | serialize.js `statementsToXML` |
//! | [`uri`] | `Util.uri.join` | uri.js |
//! | [`sandbox`] | `Zotero.RDF`: `getTargets`, `getStatementsMatching`, `addStatement`, ... | translate.js `_RDFSandbox` |
//!
//! **Why a port and not an RDF crate.** The translators' output depends on
//! this library's exact behaviour: statement order (insertion order, which
//! decides the order of items, tags and creators on import, and of elements
//! on export), which blank nodes nest, the serializer's line packing, and
//! the parser's quirks. A general RDF crate would be correct RDF and a
//! different answer.
//!
//! **Blank node ids** come from a per-store counter starting at 0; upstream's
//! is global to the server process, so its ids differ run to run. Only their
//! order is meaningful, and the reference comparisons renumber `rdf:nodeID`s
//! and `_:n` names by first appearance on both sides (`tests/zotero_translators.rs`).
//!
//! **Maturity: AI draft (1).** Not yet human-reviewed.

pub mod dom;
pub mod parser;
pub mod sandbox;
pub mod serializer;
pub mod store;
pub mod term;
pub mod uri;

pub use sandbox::{RdfSandbox, RdfValue, Res, Triple};
pub use term::{Node, Term, RDF_NS};
