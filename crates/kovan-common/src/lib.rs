//! # kovan-common
//!
//! Shared canonical types for the KOVAN knowledge layer. Every other KOVAN
//! crate depends on this one and speaks in these types; cross-crate links
//! (a symbol referencing a document, a benchmark referencing a validation
//! case) are expressed as the string IDs defined here rather than as direct
//! crate-to-crate dependencies.
//!
//! **Source-of-truth rule:** these Rust structs are authoritative. BibTeX,
//! TOML, and Markdown metadata are *generated* from them and must never be
//! treated as the canonical record.
//!
//! ## What belongs here
//!
//! Types that more than one KOVAN crate needs: documents, symbols,
//! repositories, correlations, benchmarks, validation cases, generated-code
//! provenance, and the small enums/records they contain. Do **not** put
//! pipeline logic (PDF parsing, semantic extraction, code generation) here —
//! that lives in the respective feature crate. The one exception is
//! [`zotero`] (2026-10-07, GitHub #748): Zotero's item model comes with the
//! conversions that define it (schema validation, CSL-JSON both ways, dates),
//! placed here by maintainer direction so every kovan crate can read and
//! write Zotero libraries.
//!
//! ## Module map
//!
//! - [`document`] — [`KovanDocument`] + [`KovanDocumentBuilder`], [`Author`],
//!   [`Visibility`], [`DocumentType`].
//! - [`symbol`] — [`KovanSymbol`], [`KovanRepository`], the [`Language`] enum.
//! - [`knowledge`] — [`KovanCorrelation`], [`KovanBenchmark`],
//!   [`KovanValidationCase`], [`GeneratedArtifact`].
//! - [`code_map`], [`call_graph`], [`geometry`], [`mindmap_view`] — the
//!   pure data and layout behind the code map and the Code Review UI, moved
//!   out of `kovan` on 2026-10-06 so the wasm web view (`kovan-web`, GitHub
//!   #736) can use them. Plain `serde` + `std`; `kovan` re-exports each one
//!   under its old path.
//! - [`zotero`] — Zotero's item model, schema, CSL-JSON conversion and the
//!   [`KovanDocument`] mapping (GitHub #748), ported from Zotero (AGPL-3.0).
//!
//! Everything is re-exported at the crate root, so downstream crates can keep
//! importing `kovan_common::KovanDocument` directly.
//!
//! ## Maturity
//!
//! Unlike the other `kovan-*` crates, this one is **not** a placeholder stage
//! with stub logic — it is a plain data crate (types + serde derives + a
//! builder + convenience constructors) and there is nothing left here to stub
//! out. Every public type is fully implemented, documented, and round-trip
//! tested (`serde_json` and `toml`). The pipeline crates that build on top of
//! these types (~~`kovan-literature`,~~ `kovan-semantics`, `kovan-codegen`) still
//! carry their own `// TODO(kovan)` markers for unimplemented behaviour; that
//! is expected and tracked separately in each of those crates.
//! **CORRECTED 2026-09-25** — `kovan-literature/src` no longer contains any
//! `TODO(kovan)` marker (checked with `grep -rn 'TODO(kovan)'`); its
//! `DECISIONS.md` records the five stubs as fleshed out. `kovan-semantics`
//! (`src/adapters/`) and `kovan-codegen` (`src/macros_support.rs`) still do.

#![forbid(unsafe_code)]

pub mod document;
pub mod knowledge;
pub mod symbol;

/// The workspace call graph's data model, crate -> module -> function
/// (GitHub #737), and its per-crate split for the web (#736). Moved here
/// from `kovan::call_graph` on 2026-10-06; `kovan` re-exports it.
pub mod call_graph;
/// The code map of a Cargo workspace (GitHub #734): model, layout and SVG.
/// Moved here from `kovan::code_map` on 2026-10-06; `kovan` re-exports it
/// and keeps the `cargo metadata` call.
pub mod code_map;
/// World-space [`geometry::Point`] and [`geometry::Bounds`].
pub mod geometry;
/// The fuzzy scorer of kovan's finders, moved here from `kovan::fuzzy` on
/// 2026-10-06 for web-kovan's search bar.
pub mod fuzzy;
/// The star (ring) layout and the scrollable-canvas arithmetic of kovan's
/// map views. Moved here from `kovan::mindmap_view` on 2026-10-06.
pub mod mindmap_view;
// Zotero's data model (GitHub #748); documented by its own `//!` block (an
// outer `///` here would make rustdoc resolve that block's links from the
// crate root).
pub mod zotero;

pub use document::{Author, DocumentType, KovanDocument, KovanDocumentBuilder, Visibility};
pub use knowledge::{GeneratedArtifact, KovanBenchmark, KovanCorrelation, KovanValidationCase};
pub use symbol::{KovanRepository, KovanSymbol, Language};
