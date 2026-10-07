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
//! | `local_library` | read a Zotero data folder (`zotero.sqlite` + `storage/`) and import it into kovan (#750). Native desktop only for now: not compiled for wasm32 or Android (being made pure Rust, #750). |

pub mod framework;
#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
pub mod local_library;
pub mod translators;
