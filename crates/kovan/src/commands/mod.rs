//! Subcommand implementations for the `kovan` CLI.
//!
//! One module per command (or command group), so `main.rs` stays a thin
//! `clap` dispatcher — the workspace's file-size-cap rule applies to this
//! crate too. Every module here follows the same shape: a `run` function (or
//! a small family of them) that takes already-parsed arguments and returns
//! `Result<(), String>` (or nothing, for infallible commands), printing
//! line-oriented, deterministic output to stdout so a coding agent can parse
//! it without a JSON layer.
//!
//! This module also holds the two `clap`-facing enums shared by more than one
//! command ([`KindArg`], [`LangArg`]) — thin mirrors of the library enums
//! [`kovan_discovery::FileKind`] and [`kovan_semantics::LanguageAdapter`].
//! They exist only because `clap::ValueEnum` is a foreign trait: it cannot be
//! implemented on a foreign enum from this crate, so each library enum gets a
//! local twin plus a `From` conversion, matched exhaustively both ways.

pub mod agent_docs_gen;
pub mod api_docs;
pub mod cost;
pub mod discover;
pub mod gen;
pub mod historian;
pub mod kloc;
pub mod lit;
pub mod methods;
pub mod outline;
pub mod scan;
pub mod search;
pub mod semq;
pub mod setup;
pub mod skill_gen;
pub mod slice;
pub mod symbols;
pub mod tokens;
pub mod workspace;

use clap::ValueEnum;

use kovan_discovery::FileKind;
use kovan_semantics::LanguageAdapter;

/// `clap`-facing mirror of [`FileKind`].
#[derive(Clone, Copy, ValueEnum)]
pub enum KindArg {
    Source,
    Markdown,
    Pdf,
    Metadata,
}

impl From<KindArg> for FileKind {
    fn from(k: KindArg) -> Self {
        match k {
            KindArg::Source => FileKind::Source,
            KindArg::Markdown => FileKind::Markdown,
            KindArg::Pdf => FileKind::Pdf,
            KindArg::Metadata => FileKind::Metadata,
        }
    }
}

/// `clap`-facing mirror of [`LanguageAdapter`].
#[derive(Clone, Copy, ValueEnum)]
pub enum LangArg {
    Rust,
    Cpp,
    Python,
    Fortran,
}

impl From<LangArg> for LanguageAdapter {
    fn from(l: LangArg) -> Self {
        match l {
            LangArg::Rust => LanguageAdapter::Rust,
            LangArg::Cpp => LanguageAdapter::Cpp,
            LangArg::Python => LanguageAdapter::Python,
            LangArg::Fortran => LanguageAdapter::Fortran,
        }
    }
}

impl LangArg {
    /// Human-readable language name, for a synthesised `KovanRepository.language`
    /// (`kovan-cli symbols`/`kovan-cli summary` build a repository record on the fly —
    /// there is no persisted `KovanRepository` catalogue yet).
    pub fn display_name(self) -> &'static str {
        match self {
            LangArg::Rust => "Rust",
            LangArg::Cpp => "C++",
            LangArg::Python => "Python",
            LangArg::Fortran => "Fortran",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_arg_covers_every_file_kind() {
        for k in [
            KindArg::Source,
            KindArg::Markdown,
            KindArg::Pdf,
            KindArg::Metadata,
        ] {
            let _: FileKind = k.into();
        }
    }

    #[test]
    fn lang_arg_covers_every_language_adapter() {
        for l in [
            LangArg::Rust,
            LangArg::Cpp,
            LangArg::Python,
            LangArg::Fortran,
        ] {
            let _: LanguageAdapter = l.into();
        }
    }

    #[test]
    fn lang_arg_display_names_are_distinct() {
        let names = [
            LangArg::Rust.display_name(),
            LangArg::Cpp.display_name(),
            LangArg::Python.display_name(),
            LangArg::Fortran.display_name(),
        ];
        for (i, a) in names.iter().enumerate() {
            for b in &names[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }
}
