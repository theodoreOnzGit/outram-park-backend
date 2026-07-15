//! # kovan-discovery
//!
//! Repository indexing and file discovery for KOVAN. This is the layer beneath
//! semantics: before any language-native tooling runs, KOVAN needs to find the
//! files and grep their contents. It builds directly on two mature Rust
//! engines:
//!
//! - [`ignore`] — the `.gitignore`-aware directory walker behind `fd`/ripgrep.
//! - [`grep_searcher`] + [`grep_regex`] — the ripgrep search engine.
//!
//! Everything here is deterministic and offline: no index database, no network.
//!
//! ## What it provides
//!
//! - [`discover`] / [`discover_kind`] — enumerate files under a root, honouring
//!   `.gitignore`, optionally filtered to a [`FileKind`] (source, Markdown,
//!   PDF, metadata).
//! - [`search_file`] — ripgrep-style regex search of a single file, returning
//!   line numbers and text.
//!
//! `kovan-semantics` starts from these primitives ("ripgrep first") before
//! escalating to language servers.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use grep_regex::RegexMatcher;
use grep_searcher::sinks::UTF8;
use grep_searcher::Searcher;
use ignore::WalkBuilder;

/// A category of file KOVAN cares about, with its associated extensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// Source code (Rust, C++, Python, Fortran, …).
    Source,
    /// Markdown documents.
    Markdown,
    /// PDF literature.
    Pdf,
    /// Metadata / configuration (TOML, JSON, YAML).
    Metadata,
}

impl FileKind {
    /// The lowercase file extensions associated with this kind.
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            FileKind::Source => &[
                "rs", "cpp", "cc", "cxx", "hpp", "hh", "h", "c", "py", "f90",
                "f", "for", "f95", "f03",
            ],
            FileKind::Markdown => &["md", "markdown"],
            FileKind::Pdf => &["pdf"],
            FileKind::Metadata => &["toml", "json", "yaml", "yml"],
        }
    }
}

/// A single search hit within a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    /// 1-based line number.
    pub line: u64,
    /// The matching line, trailing newline trimmed.
    pub text: String,
}

/// Errors produced by discovery / search.
#[derive(Debug)]
pub enum DiscoveryError {
    /// The regex pattern was invalid.
    BadPattern(String),
    /// An I/O error occurred while searching.
    Io(std::io::Error),
}

impl std::fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiscoveryError::BadPattern(p) => write!(f, "invalid search pattern: {p}"),
            DiscoveryError::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for DiscoveryError {}

/// Discover all files under `root`, honouring `.gitignore`. If `exts` is
/// non-empty, only files whose (lowercased) extension is listed are returned.
pub fn discover(root: &Path, exts: &[&str]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in WalkBuilder::new(root).build().flatten() {
        let is_file = entry.file_type().is_some_and(|ft| ft.is_file());
        if !is_file {
            continue;
        }
        let path = entry.path();
        let keep = exts.is_empty()
            || path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase())
                .is_some_and(|e| exts.contains(&e.as_str()));
        if keep {
            out.push(path.to_path_buf());
        }
    }
    out
}

/// Discover all files under `root` of a given [`FileKind`].
pub fn discover_kind(root: &Path, kind: FileKind) -> Vec<PathBuf> {
    discover(root, kind.extensions())
}

/// Search a single file for `pattern` (a regular expression), returning every
/// matching line. Uses the ripgrep engine.
pub fn search_file(path: &Path, pattern: &str) -> Result<Vec<SearchMatch>, DiscoveryError> {
    let matcher =
        RegexMatcher::new(pattern).map_err(|e| DiscoveryError::BadPattern(e.to_string()))?;
    let mut matches = Vec::new();
    Searcher::new()
        .search_path(
            &matcher,
            path,
            UTF8(|lnum, line| {
                matches.push(SearchMatch {
                    line: lnum,
                    text: line.trim_end().to_string(),
                });
                Ok(true)
            }),
        )
        .map_err(DiscoveryError::Io)?;
    Ok(matches)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn crate_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
    }

    #[test]
    fn discovers_own_source() {
        let rs = discover_kind(&crate_dir(), FileKind::Source);
        assert!(rs.iter().any(|p| p.ends_with("src/lib.rs")));
    }

    #[test]
    fn searches_own_source() {
        let lib = crate_dir().join("src/lib.rs");
        let hits = search_file(&lib, r"kovan-discovery").expect("search ok");
        assert!(!hits.is_empty());
    }

    #[test]
    fn bad_pattern_errors() {
        let lib = crate_dir().join("src/lib.rs");
        assert!(matches!(
            search_file(&lib, "("),
            Err(DiscoveryError::BadPattern(_))
        ));
    }
}
