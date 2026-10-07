// Part of the kovan Zotero port (GitHub #747).
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later). See this crate's NOTICE.

//! Zotero in kovan-literature (epic #747). The item model is
//! `kovan_common::zotero`; this module holds what reads and writes files.
//!
//! * [`framework`]: the translation framework (Zotero's translate API) the
//!   translators run in (#749).
//! * [`translators`]: the import/export translators (#749), verified
//!   code-to-code against a running Zotero translation-server (#752; see
//!   `tests/zotero_translators.rs` and `scripts/zotero-reference.sh`).

pub mod framework;
pub mod translators;
