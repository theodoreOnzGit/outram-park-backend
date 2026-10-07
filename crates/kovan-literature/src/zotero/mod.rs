// Part of the kovan Zotero port (GitHub #747). See this crate's NOTICE,
// "Upstream: Zotero"; each submodule carries its own attribution header.

//! The parts of the Zotero port that live in kovan-literature (epic #747).
//! The data model itself is `kovan_common::zotero` (#748).
//!
//! | Module | What |
//! |---|---|
//! | [`local_library`] | read a Zotero data folder (`zotero.sqlite` + `storage/`) and import it into kovan (#750). Native desktop only: not compiled for wasm32 or Android. |

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
pub mod local_library;
