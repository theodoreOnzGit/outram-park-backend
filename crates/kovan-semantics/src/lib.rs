//! # kovan-semantics
//!
//! A repository-understanding engine. It consumes semantic information from
//! **mature language tooling** and normalises it into the shared KOVAN model,
//! then emits human-readable Markdown knowledge artifacts.
//!
//! ## Philosophy
//!
//! KOVAN does **not** reimplement compilers. It drives the official semantic
//! tools for each language and represents *semantic facts* — it does not build
//! a giant universal AST. Tree-sitter is a last resort, used only where no
//! suitable semantic tooling exists.
//!
//! ## Outputs (target)
//!
//! `repository-summary.md`, `symbols.md`, `validation-links.md`,
//! `dependency-graph.md` — generated Markdown, not hidden databases.
//!
//! Placeholder stage: the adapters and generators below are stubs marked
//! `// TODO(kovan)`.

#![forbid(unsafe_code)]

use std::path::Path;

pub use kovan_common::{KovanRepository, KovanSymbol, KovanValidationCase};
pub use kovan_discovery::{discover_kind, search_file, FileKind, SearchMatch};

/// A supported source language and the preferred semantic tool KOVAN drives for
/// it. Dispatch is by enum (closed set) rather than trait objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageAdapter {
    /// Rust — via `rust-analyzer`. Primary target: TUAS, TAMPINES, BOON LAY and
    /// the wider Outram Park ecosystem.
    Rust,
    /// C++ — via `clangd` / `libclang`. Primary target: OpenFOAM.
    Cpp,
    /// Python — via Pyright / Jedi. Primary target: OpenMC.
    Python,
    /// Fortran — via `fortls`. Primary target: NJOY.
    Fortran,
}

impl LanguageAdapter {
    /// The command-line name of the preferred semantic tool for this language.
    pub fn preferred_tool(self) -> &'static str {
        match self {
            LanguageAdapter::Rust => "rust-analyzer",
            LanguageAdapter::Cpp => "clangd",
            LanguageAdapter::Python => "pyright",
            LanguageAdapter::Fortran => "fortls",
        }
    }
}

/// Errors produced by the semantics engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticsError {
    /// The requested operation is not implemented yet (placeholder stage).
    Unimplemented(&'static str),
    /// The underlying language tool was unavailable or failed.
    Tool(String),
}

impl std::fmt::Display for SemanticsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SemanticsError::Unimplemented(what) => write!(f, "not implemented yet: {what}"),
            SemanticsError::Tool(msg) => write!(f, "language tool error: {msg}"),
        }
    }
}

impl std::error::Error for SemanticsError {}

impl LanguageAdapter {
    /// A naive ripgrep pattern that matches likely *definitions* in this
    /// language. This is the "ripgrep first" heuristic used before the
    /// language server is available — it is intentionally approximate.
    pub fn rough_definition_pattern(self) -> &'static str {
        match self {
            LanguageAdapter::Rust => r"^\s*(pub\s+)?(async\s+)?fn\s+\w+",
            LanguageAdapter::Cpp => r"^\s*[\w:<>,\*&\s]+\s+\w+\s*\([^;]*\)\s*\{",
            LanguageAdapter::Python => r"^\s*def\s+\w+",
            LanguageAdapter::Fortran => r"(?i)^\s*(subroutine|function)\s+\w+",
        }
    }

    /// The [`FileKind`] scanned for this language (always source files).
    pub fn file_kind(self) -> FileKind {
        FileKind::Source
    }
}

/// A rough, ripgrep-first scan of a repository for probable definition sites of
/// the given language. Discovers source files with the `ignore` walker, then
/// greps each with the ripgrep engine. Returns `(path, match)` pairs.
///
/// This is deliberately approximate — it is the pre-language-server heuristic.
/// TODO(kovan): [`catalogue_symbols`] will replace it with real semantic output
/// from the language server.
pub fn rough_definition_scan(
    repo: &Path,
    adapter: LanguageAdapter,
) -> Result<Vec<(std::path::PathBuf, SearchMatch)>, SemanticsError> {
    let pattern = adapter.rough_definition_pattern();
    let mut hits = Vec::new();
    for file in discover_kind(repo, adapter.file_kind()) {
        match search_file(&file, pattern) {
            Ok(matches) => {
                for m in matches {
                    hits.push((file.clone(), m));
                }
            }
            Err(e) => return Err(SemanticsError::Tool(e.to_string())),
        }
    }
    Ok(hits)
}

/// Catalogue the symbols in a repository using the given language adapter's
/// language server (the source of truth).
///
/// TODO(kovan): drive the preferred tool and normalise its output into
/// [`KovanSymbol`]s. Until then use [`rough_definition_scan`] for a ripgrep-first
/// approximation.
pub fn catalogue_symbols(
    _repo: &Path,
    _adapter: LanguageAdapter,
) -> Result<Vec<KovanSymbol>, SemanticsError> {
    Err(SemanticsError::Unimplemented("catalogue_symbols"))
}

/// Generate a Markdown repository summary.
///
/// TODO(kovan): produce `repository-summary.md` content.
pub fn repository_summary_markdown(
    _repo: &KovanRepository,
) -> Result<String, SemanticsError> {
    Err(SemanticsError::Unimplemented("repository_summary_markdown"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapters_name_their_tools() {
        assert_eq!(LanguageAdapter::Rust.preferred_tool(), "rust-analyzer");
        assert_eq!(LanguageAdapter::Fortran.preferred_tool(), "fortls");
    }

    #[test]
    fn catalogue_is_stubbed() {
        assert!(matches!(
            catalogue_symbols(std::path::Path::new("."), LanguageAdapter::Rust),
            Err(SemanticsError::Unimplemented(_))
        ));
    }

    #[test]
    fn ripgrep_first_scan_finds_this_crates_functions() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let hits = rough_definition_scan(dir, LanguageAdapter::Rust).expect("scan ok");
        // This crate defines several `pub fn`s, so the ripgrep-first heuristic
        // must find at least one definition site.
        assert!(!hits.is_empty());
    }
}
