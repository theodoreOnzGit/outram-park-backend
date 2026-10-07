// Part of the kovan Zotero port (GitHub #747).
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later). See this crate's NOTICE;
// each submodule carries its own attribution header.

//! Zotero in kovan-literature (epic #747). The item model is
//! `kovan_common::zotero` (#748); this module holds what reads and writes
//! files.
//!
//! | Module | What |
//! |---|---|
//! | [`framework`] | the translation framework (Zotero's translate API) the translators run in (#749) |
//! | [`translators`] | the import/export translators (#749), verified code-to-code against a running Zotero translation-server (#752; see `tests/zotero_translators.rs` and `scripts/zotero-reference.sh`) |
//! | [`search`] | identifier lookup, "Add Item by Identifier" (#756): `extractIdentifiers` and the search translators, network-free (the caller fetches; see the module docs), verified code-to-code on recorded responses (`tests/zotero_search.rs`, `scripts/zotero-search-reference.mjs`) |
//! | [`local_library`] | read a Zotero data folder (`zotero.sqlite` + `storage/`), or its database from bytes, and import it into kovan (#750). Pure Rust; every target, wasm32 and Android included (since 2026-10-07; before, native desktop only). |

pub mod framework;
pub mod local_library;
pub mod search;
pub mod translators;
