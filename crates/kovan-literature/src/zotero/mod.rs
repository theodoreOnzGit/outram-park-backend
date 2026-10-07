// Part of the kovan Zotero port (GitHub #747). See this crate's NOTICE,
// "Upstream: Zotero"; each submodule carries its own attribution header.

//! The parts of the Zotero port that live in kovan-literature (epic #747).
//! The data model itself is `kovan_common::zotero` (#748).
//!
//! | Module | What |
//! |---|---|
//! | [`local_library`] | read a Zotero data folder (`zotero.sqlite` + `storage/`), or its database from bytes, and import it into kovan (#750). Pure Rust; every target, wasm32 and Android included (since 2026-10-07; before, native desktop only). |

pub mod local_library;
