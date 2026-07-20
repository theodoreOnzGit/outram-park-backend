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
//! that lives in the respective feature crate.
//!
//! ## Module map
//!
//! - [`document`] — [`KovanDocument`] + [`KovanDocumentBuilder`], [`Author`],
//!   [`Visibility`], [`DocumentType`].
//! - [`symbol`] — [`KovanSymbol`], [`KovanRepository`], the [`Language`] enum.
//! - [`knowledge`] — [`KovanCorrelation`], [`KovanBenchmark`],
//!   [`KovanValidationCase`], [`GeneratedArtifact`].
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
//! these types (`kovan-literature`, `kovan-semantics`, `kovan-codegen`) still
//! carry their own `// TODO(kovan)` markers for unimplemented behaviour; that
//! is expected and tracked separately in each of those crates.

#![forbid(unsafe_code)]

pub mod document;
pub mod knowledge;
pub mod symbol;

pub use document::{Author, DocumentType, KovanDocument, KovanDocumentBuilder, Visibility};
pub use knowledge::{GeneratedArtifact, KovanBenchmark, KovanCorrelation, KovanValidationCase};
pub use symbol::{KovanRepository, KovanSymbol, Language};
