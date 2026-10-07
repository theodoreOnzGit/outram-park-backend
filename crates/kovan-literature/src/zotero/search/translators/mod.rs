// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42); each module names its file.
// Copyright (c) Corporation for Digital Scholarship and the translators'
//   authors (see each module).
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later; five files carry no
//   licence text and are treated as AGPLv3, #747).

//! The search translators, one module each. Each has `METADATA`,
//! `detect_search(&JsObject) -> bool` and
//! `do_search(&mut SearchContext, &JsObject) -> Result<(), SearchError>`;
//! [`super::SearchTranslator`] dispatches to them.

pub mod ads_bibcode;
pub mod arxiv;
pub mod bnf_isbn;
pub mod camara_isbn;
pub mod crossref_rest;
pub mod doi_content_negotiation;
pub mod eidr;
pub mod eric;
pub mod gbv_isbn;
pub mod k10plus_isbn;
pub mod libris_isbn;
pub mod loc_isbn;
pub mod lulu;
pub mod medra;
pub mod nlp_isbn;
pub mod open_worldcat;
pub mod openalex;
pub mod pubmed;
pub mod who;
