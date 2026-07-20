//! Ripgrep-first symbol extraction — the deterministic, offline, Android-clean
//! core of `kovan-semantics`.
//!
//! Per KOVAN's "Important Philosophy" (see `docs/kovan.md`, "# KOVAN Semantics"),
//! this layer does **not** reimplement a compiler or build a universal AST. It
//! locates likely definition sites with the ripgrep engine (via
//! [`kovan_discovery`]) and pulls the symbol *name*, *kind*, and *location* out
//! of those lines with a small set of anchored regexes. The result is a set of
//! [`ExtractedSymbol`]s normalised into the shared [`KovanSymbol`] model.
//!
//! This is intentionally approximate — it is the pre-language-server heuristic.
//! When higher fidelity is needed the crate escalates to the real language
//! servers (see [`crate::adapters`]), which stay behind the off-by-default,
//! non-Android `language-servers` feature. The extractor here is what every
//! Android build gets, and it never shells out or touches the network.
//!
//! ## Per-language scanners
//!
//! One submodule per language, each a line-oriented scanner that also tracks a
//! light enclosing-scope stack so members get a qualified name:
//!
//! - `rust`    — `fn` / `struct` / `enum` / `trait` / `type` / `mod` / `impl`
//!   (scopes: `mod` and `impl` blocks, brace-depth tracked).
//! - `cpp`     — functions / `class` / `struct` / `union` / `namespace`
//!   (scopes: `namespace` and class blocks, brace-depth tracked).
//! - `python`  — `def` / `class` (scopes: indentation).
//! - `fortran` — `module` / `subroutine` / `function` / `type` (scopes:
//!   keyword `end` matching, case-insensitive).
//!
//! ## Known limits (all languages)
//!
//! Brace/keyword counting is textual: braces inside string/char literals or
//! comments are counted, so deeply macro-generated or unusual formatting can
//! mis-nest a qualified name. Names are always correct; qualification is
//! best-effort. Multi-line signatures are read from their first (keyword) line.
//! These are acceptable for a knowledge-map heuristic and are exactly what the
//! language-server escalation path exists to supersede.

mod cpp;
mod fortran;
mod python;
mod rust;

use std::path::{Path, PathBuf};

use kovan_common::{KovanSymbol, Language};

use crate::{LanguageAdapter, SemanticsError};

/// The kind of a source symbol, normalised across the four supported languages.
///
/// Stored on [`ExtractedSymbol`] and rendered into [`KovanSymbol::kind`] via
/// [`SymbolKind::as_str`]. The set is closed (enum dispatch, per the workspace
/// rules) — add a variant here and every match site must handle it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    /// A free function or method: Rust `fn`, C++ function, Python `def`,
    /// Fortran `function`.
    Function,
    /// A Fortran `subroutine` (a function with no return value).
    Subroutine,
    /// A Rust `struct` or a C++ `struct`.
    Struct,
    /// A Rust `enum`.
    Enum,
    /// A Rust `trait`.
    Trait,
    /// A Rust `impl` block (associated to a type, optionally a trait).
    Impl,
    /// A Rust type alias, or a Fortran derived `type` definition.
    Type,
    /// A C++ `union`.
    Union,
    /// A C++ or Python `class`.
    Class,
    /// A C++ `namespace`.
    Namespace,
    /// A Rust `mod`, a Fortran `module`, or a Python module (file).
    Module,
}

impl SymbolKind {
    /// The short, stable string written into [`KovanSymbol::kind`]. Matches the
    /// source keyword where one exists (`"fn"`, `"struct"`, `"class"`, …).
    pub fn as_str(self) -> &'static str {
        match self {
            SymbolKind::Function => "fn",
            SymbolKind::Subroutine => "subroutine",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Trait => "trait",
            SymbolKind::Impl => "impl",
            SymbolKind::Type => "type",
            SymbolKind::Union => "union",
            SymbolKind::Class => "class",
            SymbolKind::Namespace => "namespace",
            SymbolKind::Module => "module",
        }
    }
}

/// One symbol found by the ripgrep-first scanner, with its source location.
///
/// This is the extractor's **file-local** intermediate record: the scanners
/// produce it per source file, with a typed [`SymbolKind`] and no repository
/// context. [`ExtractedSymbol::into_kovan_symbol`] attaches the repository ID
/// and normalises it into the shared [`KovanSymbol`] — which, since the
/// `kovan-common` v2 pass, carries the file/line/language directly, so the
/// Markdown outputs and every downstream crate now work off `KovanSymbol`
/// alone. `ExtractedSymbol` remains as the scanner-stage record because a
/// `KovanSymbol` additionally needs the `id`/`repository_id` that only the
/// catalogue step (which knows the repo) can supply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedSymbol {
    /// The bare identifier as written in source (e.g. `mass_flux`).
    pub name: String,
    /// Best-effort qualified path: enclosing scopes joined to `name`
    /// (`module::Type::method`, `Namespace::Class::method`, `pkg.Class.method`).
    /// Falls back to `name` when no scope is detected.
    pub qualified_name: String,
    /// The normalised symbol kind.
    pub kind: SymbolKind,
    /// The language whose scanner produced this symbol.
    pub language: LanguageAdapter,
    /// Path to the file the symbol was found in. Made repository-relative by
    /// [`catalogue_symbols_detailed`] when possible.
    pub file: PathBuf,
    /// 1-based line number of the definition's keyword line.
    pub line: u64,
}

impl ExtractedSymbol {
    /// Normalise into the shared [`KovanSymbol`], attaching the given repository
    /// ID. The symbol ID is deterministic: `"{repository_id}::{file}:{line}::
    /// {qualified_name}"`, so the same scan of the same tree always yields the
    /// same IDs (KOVAN's determinism mandate, `docs/kovan.md`).
    pub fn into_kovan_symbol(self, repository_id: &str) -> KovanSymbol {
        let id = format!(
            "{repository_id}::{}:{}::{}",
            self.file.display(),
            self.line,
            self.qualified_name
        );
        KovanSymbol {
            id,
            qualified_name: self.qualified_name,
            kind: self.kind.as_str().to_string(),
            repository_id: repository_id.to_string(),
            file: self.file.display().to_string(),
            // Line numbers comfortably fit in u32; the `as` truncation is
            // unreachable for any real source file.
            line: self.line as u32,
            language: self.language.into(),
        }
    }
}

impl From<LanguageAdapter> for Language {
    /// Map the tool-selecting [`LanguageAdapter`] to the shared language
    /// identity [`Language`]. The two are 1:1 on the language; `LanguageAdapter`
    /// additionally encodes which language server / file extensions to use.
    fn from(adapter: LanguageAdapter) -> Self {
        match adapter {
            LanguageAdapter::Rust => Language::Rust,
            LanguageAdapter::Cpp => Language::Cpp,
            LanguageAdapter::Python => Language::Python,
            LanguageAdapter::Fortran => Language::Fortran,
        }
    }
}

/// Extract every symbol from a single file's `text` using the scanner for
/// `adapter`. Pure function of its inputs (deterministic); does no I/O.
///
/// `file` is recorded on each returned [`ExtractedSymbol`] but is not read — the
/// caller supplies the already-read `text`. This keeps the scanners unit-testable
/// against synthetic snippets.
pub fn extract_from_text(
    adapter: LanguageAdapter,
    file: &Path,
    text: &str,
) -> Vec<ExtractedSymbol> {
    match adapter {
        LanguageAdapter::Rust => rust::extract(file, text),
        LanguageAdapter::Cpp => cpp::extract(file, text),
        LanguageAdapter::Python => python::extract(file, text),
        LanguageAdapter::Fortran => fortran::extract(file, text),
    }
}

/// Catalogue every symbol in `repo` for the language of `adapter`, returning the
/// rich, location-carrying [`ExtractedSymbol`]s.
///
/// Pipeline (all deterministic + offline):
/// 1. discover source files with [`kovan_discovery::discover_kind`] (the
///    `.gitignore`-aware `fd`/ripgrep walker);
/// 2. keep only files whose extension belongs to `adapter`'s language;
/// 3. read each file and run its regex scanner ([`extract_from_text`]).
///
/// Unreadable or non-UTF-8 files are skipped (not an error), so the catalogue is
/// a best-effort superset — missing a file never fails the whole scan. File
/// paths on the results are made relative to `repo` when possible.
pub fn catalogue_symbols_detailed(
    repo: &Path,
    adapter: LanguageAdapter,
) -> Result<Vec<ExtractedSymbol>, SemanticsError> {
    let exts = adapter.extensions();
    let mut out = Vec::new();
    for file in kovan_discovery::discover_kind(repo, adapter.file_kind()) {
        let matches_lang = file
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .is_some_and(|e| exts.contains(&e.as_str()));
        if !matches_lang {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue; // binary / non-UTF-8 / unreadable → skip deterministically
        };
        let rel = file.strip_prefix(repo).unwrap_or(&file).to_path_buf();
        for mut sym in extract_from_text(adapter, &rel, &text) {
            sym.file = rel.clone();
            out.push(sym);
        }
    }
    Ok(out)
}

/// A tiny helper shared by the brace-tracking scanners (Rust, C++): the net
/// change in brace depth on a line, ignoring nothing (textual count). Returns
/// `(opens, closes)`.
pub(crate) fn brace_delta(line: &str) -> (usize, usize) {
    let opens = line.bytes().filter(|&b| b == b'{').count();
    let closes = line.bytes().filter(|&b| b == b'}').count();
    (opens, closes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn into_kovan_symbol_is_deterministic() {
        let sym = ExtractedSymbol {
            name: "mass_flux".into(),
            qualified_name: "nozzle::mass_flux".into(),
            kind: SymbolKind::Function,
            language: LanguageAdapter::Rust,
            file: PathBuf::from("src/nozzle.rs"),
            line: 42,
        };
        let ks = sym.clone().into_kovan_symbol("tampines");
        assert_eq!(ks.kind, "fn");
        assert_eq!(ks.qualified_name, "nozzle::mass_flux");
        assert_eq!(ks.repository_id, "tampines");
        // Location + language carried through into the shared type.
        assert_eq!(ks.file, "src/nozzle.rs");
        assert_eq!(ks.line, 42);
        assert_eq!(ks.language, Language::Rust);
        // Same input → identical id.
        assert_eq!(ks.id, sym.into_kovan_symbol("tampines").id);
    }

    #[test]
    fn dispatch_routes_to_the_right_scanner() {
        let rs = extract_from_text(
            LanguageAdapter::Rust,
            Path::new("a.rs"),
            "pub fn foo() {}\n",
        );
        assert!(rs
            .iter()
            .any(|s| s.name == "foo" && s.kind == SymbolKind::Function));

        let py = extract_from_text(
            LanguageAdapter::Python,
            Path::new("a.py"),
            "def foo():\n    pass\n",
        );
        assert!(py
            .iter()
            .any(|s| s.name == "foo" && s.kind == SymbolKind::Function));
    }
}
