//! # kovan-semantics
//!
//! A repository-understanding engine for the KOVAN knowledge layer. It turns a
//! source tree into normalised [`KovanSymbol`]s and human-readable Markdown
//! knowledge artifacts, **deterministically and offline** (KOVAN's Android-first
//! mandate — see `docs/kovan.md`, "# KOVAN Semantics").
//!
//! ## Two tiers, one philosophy
//!
//! Per KOVAN's "Important Philosophy", this crate does **not** reimplement a
//! compiler or build a universal AST. It works in two tiers:
//!
//! 1. **Ripgrep-first (default, always on, Android-clean).** [`catalogue_symbols`]
//!    and [`extract`] locate definitions with the ripgrep engine (via
//!    [`kovan_discovery`]) and pull each symbol's name, kind, and location out
//!    with anchored regexes. No process spawn, no network, no system libraries.
//!    This is the floor every build — including `aarch64-linux-android` — gets.
//! 2. **Language-server escalation (deferred, opt-in, non-Android).** The real
//!    semantic tools (`rust-analyzer`, `clangd`, Pyright, `fortls`) are the
//!    source of truth once wired. Their integration is scaffolded in
//!    [`adapters`] behind the off-by-default `language-servers` feature, itself
//!    source-gated to `cfg(not(target_os = "android"))`. Tree-sitter is a last
//!    resort only (KOVAN "Tree-Sitter Policy") and is **not** used here.
//!
//! ## Outputs
//!
//! [`symbols_markdown`] → `symbols.md` and [`repository_summary_markdown`] →
//! `repository-summary.md`. `validation-links.md` / `dependency-graph.md` are
//! future work (they need the literature/graph layers).
//!
//! ## Example
//!
//! ```no_run
//! use kovan_semantics::{catalogue_symbols, symbols_markdown, LanguageAdapter};
//! use std::path::Path;
//!
//! let repo = Path::new("crates/tampines-steam-tables");
//! // `catalogue_symbols` returns shared `KovanSymbol`s carrying file/line and
//! // language, which the Markdown renderers consume directly.
//! let symbols = catalogue_symbols(repo, LanguageAdapter::Rust).unwrap();
//! let md = symbols_markdown("TAMPINES", &symbols);
//! println!("{md}");
//! ```

#![forbid(unsafe_code)]

use std::path::Path;

pub mod adapters;
pub mod extract;
mod outputs;

pub use extract::{catalogue_symbols_detailed, extract_from_text, ExtractedSymbol, SymbolKind};
pub use outputs::{repository_summary_markdown, symbols_markdown};

pub use kovan_common::{KovanRepository, KovanSymbol, KovanValidationCase, Language};
pub use kovan_discovery::{discover_kind, search_file, FileKind, SearchMatch};

/// A supported source language and the preferred semantic tool KOVAN drives for
/// it (see `docs/kovan.md`, "# KOVAN Semantics" → "Language Support"). Dispatch
/// is by enum (a closed set) rather than trait objects, per the workspace rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageAdapter {
    /// Rust — ripgrep-first, escalating to `rust-analyzer`. Primary targets:
    /// TUAS, TAMPINES, BOON LAY and the wider Outram Park ecosystem.
    Rust,
    /// C++ — ripgrep-first, escalating to `clangd` / `libclang`. Primary target:
    /// OpenFOAM.
    Cpp,
    /// Python — ripgrep-first, escalating to Pyright / Jedi. Primary target:
    /// OpenMC.
    Python,
    /// Fortran — ripgrep-first, escalating to `fortls`. Primary target: NJOY.
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

    /// The source-file extensions (lowercase, no dot) this language owns. Used
    /// to filter [`FileKind::Source`] discovery down to a single language.
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            LanguageAdapter::Rust => &["rs"],
            LanguageAdapter::Cpp => &["cpp", "cc", "cxx", "hpp", "hh", "h", "c"],
            LanguageAdapter::Python => &["py"],
            LanguageAdapter::Fortran => &["f90", "f", "for", "f95", "f03"],
        }
    }

    /// A naive ripgrep pattern that matches likely *definition* lines in this
    /// language. This is the cheap "does this file define anything?" pre-filter
    /// used by [`rough_definition_scan`]; the full extractor in [`extract`] is
    /// what actually names symbols. Intentionally approximate.
    pub fn rough_definition_pattern(self) -> &'static str {
        match self {
            LanguageAdapter::Rust => {
                r"^\s*(pub\s+)?(async\s+)?(unsafe\s+)?(fn|struct|enum|trait|mod|impl|type)\s"
            }
            LanguageAdapter::Cpp => {
                r"^\s*(namespace|class|struct|union)\s+\w+|^\s*[\w:<>,\*&\s]+\s+\w+\s*\([^;]*\)\s*\{"
            }
            LanguageAdapter::Python => r"^\s*(async\s+)?(def|class)\s+\w+",
            LanguageAdapter::Fortran => r"(?i)^\s*(module|subroutine|function|type)\s+\w+",
        }
    }

    /// The [`FileKind`] scanned for this language (always source files).
    pub fn file_kind(self) -> FileKind {
        FileKind::Source
    }
}

/// Errors produced by the semantics engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticsError {
    /// The requested operation is not implemented yet (e.g. the language-server
    /// escalation path is scaffolded but not wired).
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

/// A rough, ripgrep-first scan of a repository for probable definition *lines*
/// of the given language. Discovers source files with the `.gitignore`-aware
/// walker, then greps each with the ripgrep engine. Returns `(path, match)`
/// pairs (line number + text) — it does not name symbols.
///
/// This is the cheap heuristic; [`catalogue_symbols`] is the real extractor that
/// resolves names, kinds, and qualified paths.
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

/// Catalogue the symbols in a repository as normalised [`KovanSymbol`]s, using
/// the ripgrep-first extractor for `adapter`'s language.
///
/// Deterministic and offline. This is the default (Android-clean) path; each
/// symbol's stable ID embeds its repository-relative `file:line` and qualified
/// name. For the richer, location-carrying records use
/// [`catalogue_symbols_detailed`]; for the (deferred) language-server path see
/// [`adapters`].
///
/// `repository_id` is stamped onto every [`KovanSymbol`] so the results link
/// back to their [`KovanRepository`].
pub fn catalogue_symbols(
    repo: &Path,
    adapter: LanguageAdapter,
) -> Result<Vec<KovanSymbol>, SemanticsError> {
    let detailed = catalogue_symbols_detailed(repo, adapter)?;
    let repo_id = repo
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repository");
    Ok(detailed
        .into_iter()
        .map(|s| s.into_kovan_symbol(repo_id))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapters_name_their_tools() {
        assert_eq!(LanguageAdapter::Rust.preferred_tool(), "rust-analyzer");
        assert_eq!(LanguageAdapter::Fortran.preferred_tool(), "fortls");
        assert_eq!(LanguageAdapter::Cpp.server_binary(), "clangd");
    }

    #[test]
    fn catalogue_finds_this_crates_own_symbols() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let syms = catalogue_symbols(dir, LanguageAdapter::Rust).expect("scan ok");
        // This crate defines `LanguageAdapter`, `catalogue_symbols`, etc.
        assert!(syms
            .iter()
            .any(|s| s.qualified_name.contains("catalogue_symbols")));
        assert!(syms.iter().any(|s| s.kind == "enum"));
    }

    #[test]
    fn ripgrep_first_scan_finds_this_crates_functions() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let hits = rough_definition_scan(dir, LanguageAdapter::Rust).expect("scan ok");
        assert!(!hits.is_empty());
    }

    #[test]
    fn markdown_outputs_render_from_a_real_scan() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let symbols = catalogue_symbols(dir, LanguageAdapter::Rust).expect("scan ok");
        let md = symbols_markdown("kovan-semantics", &symbols);
        assert!(md.contains("# Symbols — kovan-semantics"));
        assert!(md.contains("catalogue_symbols"));
    }
}
