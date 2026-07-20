//! Repository and symbol types shared across the KOVAN semantics layer.
//!
//! [`KovanSymbol`] is the normalised, cross-language representation of one
//! source symbol (a function, type, module, …). [`Language`] is the closed set
//! of source languages KOVAN understands; owning it here means the semantics
//! crate does not need a crate-local language identity type on the symbol.

/// A source language KOVAN understands and normalises symbols from.
///
/// This is the language *identity* only — it says nothing about which tool is
/// used to analyse it. `kovan-semantics` keeps a separate `LanguageAdapter`
/// enum for tool/extension selection (rust-analyzer vs. clangd, which file
/// extensions belong to the language, …) and converts into this shared
/// [`Language`] when it builds a [`KovanSymbol`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Language {
    /// Rust.
    Rust,
    /// C++ (and C headers scanned alongside it).
    Cpp,
    /// Python.
    Python,
    /// Fortran.
    Fortran,
}

impl Language {
    /// A short, stable, human-readable label (`"Rust"`, `"C++"`, `"Python"`,
    /// `"Fortran"`). Stable across releases — safe to write into generated
    /// Markdown or match against.
    pub fn as_str(self) -> &'static str {
        match self {
            Language::Rust => "Rust",
            Language::Cpp => "C++",
            Language::Python => "Python",
            Language::Fortran => "Fortran",
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A semantic symbol extracted from a source repository (a function, type,
/// module, …). Normalised across languages by `kovan-semantics`.
///
/// Carries enough to locate the symbol in its repository: the `qualified_name`
/// for identity, plus `file` + `line` for the exact definition site and
/// [`language`](KovanSymbol::language) for how it was parsed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KovanSymbol {
    /// Stable identifier for the symbol.
    pub id: String,
    /// Fully-qualified name/path as reported by the language tooling (e.g.
    /// `tuas_boussinesq_solver::heat_transfer_correlations::nusselt::dittus_boelter`).
    pub qualified_name: String,
    /// Symbol kind, free text for now (e.g. `"fn"`, `"struct"`, `"class"`).
    /// Kept as free text rather than an enum because it is sourced directly
    /// from heterogeneous language tooling (rust-analyzer, clangd, Pyright,
    /// fortls) whose vocabularies don't line up cleanly; normalise into an
    /// enum here only once `kovan-semantics` actually needs to match on it.
    pub kind: String,
    /// ID of the [`crate::KovanRepository`] this symbol belongs to.
    pub repository_id: String,
    /// Repository-relative path to the file the symbol is defined in (e.g.
    /// `src/nozzle.rs`). Empty string only if the extractor could not attribute
    /// a file (it always can for the ripgrep-first path).
    pub file: String,
    /// 1-based line number of the definition's keyword line within
    /// [`file`](KovanSymbol::file).
    pub line: u32,
    /// The source language this symbol was extracted from.
    pub language: Language,
}

/// A source-code repository KOVAN understands (e.g. TUAS, OpenFOAM, NJOY).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KovanRepository {
    /// Stable identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Primary language, free text for now (e.g. `"Rust"`, `"C++"`,
    /// `"Fortran"`). Kept a `String` rather than [`Language`] because a
    /// repository can be polyglot and the "primary" label is a curated,
    /// human-facing description, not a parser selection.
    pub language: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_display_and_str_agree() {
        assert_eq!(Language::Cpp.as_str(), "C++");
        assert_eq!(Language::Rust.to_string(), "Rust");
    }

    #[test]
    fn language_json_round_trips() {
        for l in [
            Language::Rust,
            Language::Cpp,
            Language::Python,
            Language::Fortran,
        ] {
            let json = serde_json::to_string(&l).expect("serialise");
            let back: Language = serde_json::from_str(&json).expect("deserialise");
            assert_eq!(l, back);
        }
    }

    #[test]
    fn symbol_json_and_toml_round_trip() {
        let symbol = KovanSymbol {
            id: "sym-1".into(),
            qualified_name: "tuas_boussinesq_solver::nusselt::dittus_boelter".into(),
            kind: "fn".into(),
            repository_id: "repo-tuas".into(),
            file: "src/nusselt.rs".into(),
            line: 42,
            language: Language::Rust,
        };
        let json = serde_json::to_string(&symbol).expect("serialise");
        assert_eq!(symbol, serde_json::from_str(&json).expect("deserialise"));
        let toml_str = toml::to_string(&symbol).expect("toml serialise");
        assert_eq!(symbol, toml::from_str(&toml_str).expect("toml deserialise"));
    }

    #[test]
    fn repository_json_and_toml_round_trip() {
        let repo = KovanRepository {
            id: "repo-tuas".into(),
            name: "tuas_boussinesq_solver".into(),
            language: "Rust".into(),
        };
        let json = serde_json::to_string(&repo).expect("serialise");
        assert_eq!(repo, serde_json::from_str(&json).expect("deserialise"));
        let toml_str = toml::to_string(&repo).expect("toml serialise");
        assert_eq!(repo, toml::from_str(&toml_str).expect("toml deserialise"));
    }
}
