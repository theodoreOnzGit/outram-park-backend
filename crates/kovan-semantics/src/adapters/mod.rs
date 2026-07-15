//! Language-server escalation scaffolding — the *deferred* high-fidelity path.
//!
//! KOVAN's "Important Philosophy" (see `docs/kovan.md`, "# KOVAN Semantics") is
//! to consume semantic information from **mature language tooling** rather than
//! reimplement a compiler. The ripgrep-first scanner in [`crate::extract`] is the
//! deterministic, offline, Android-clean floor; this module is where KOVAN would
//! escalate to the real language servers when higher fidelity is required:
//!
//! | Language | Server            | Primary target |
//! |----------|-------------------|----------------|
//! | Rust     | `rust-analyzer`   | TUAS, TAMPINES, BOON LAY |
//! | C++      | `clangd`          | OpenFOAM |
//! | Python   | `pyright`         | OpenMC |
//! | Fortran  | `fortls`          | NJOY |
//!
//! ## Why this is gated off by default
//!
//! Driving these servers (especially the in-process `ra_ap_*` Rust-analyzer
//! crates and `libclang`) is heavyweight and **Android-hostile**. Per the
//! workspace Android rule, the whole module is compiled only under
//! `cfg(all(feature = "language-servers", not(target_os = "android")))`. The
//! DEFAULT build — and every Android build — sees an empty module and stays
//! ripgrep-only. Turn it on with `--features language-servers` on a desktop
//! target once the invocations below are implemented.
//!
//! Nothing here is implemented yet: the invocation *contract* is scaffolded (each
//! adapter knows its binary and how it would be launched), the actual LSP
//! handshake is a `// TODO(kovan)`.

use crate::LanguageAdapter;

impl LanguageAdapter {
    /// The executable name of the preferred language server for this language.
    ///
    /// Always available (it is just a string); actually *launching* the server
    /// lives behind the `language-servers` feature (see the module docs). Equal
    /// to [`LanguageAdapter::preferred_tool`] today, kept as a distinct method
    /// so the two can diverge (e.g. a wrapper binary) without an API break.
    pub fn server_binary(self) -> &'static str {
        self.preferred_tool()
    }
}

#[cfg(all(feature = "language-servers", not(target_os = "android")))]
pub use language_servers::{catalogue_symbols_via_server, LanguageServerInvocation};

#[cfg(all(feature = "language-servers", not(target_os = "android")))]
mod language_servers {
    use std::path::Path;

    use kovan_common::KovanSymbol;

    use crate::{LanguageAdapter, SemanticsError};

    /// How KOVAN *would* launch a language server for a repository: the binary
    /// plus its argument vector and the working directory (the repo root).
    ///
    /// This is a description only — building it does not spawn a process.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct LanguageServerInvocation {
        /// Executable name (e.g. `"rust-analyzer"`). Resolved on `PATH`.
        pub binary: &'static str,
        /// Argument vector the server is launched with.
        pub args: Vec<String>,
        /// Working directory — the repository root the server should analyse.
        pub cwd: std::path::PathBuf,
    }

    impl LanguageAdapter {
        /// Describe the language-server invocation for `repo`. Deterministic;
        /// performs no I/O and starts no process.
        pub fn language_server_invocation(self, repo: &Path) -> LanguageServerInvocation {
            // Each server speaks LSP over stdio when started bare; the concrete
            // argv/initialise handshake is filled in per server when this path
            // is implemented.
            let args = match self {
                // rust-analyzer runs as an LSP server over stdio by default.
                LanguageAdapter::Rust => Vec::new(),
                // clangd emits richer index data with a background index.
                LanguageAdapter::Cpp => vec!["--background-index".to_string()],
                // pyright ships a language server binary (`pyright-langserver`);
                // `pyright` here is the umbrella name — resolve at implementation.
                LanguageAdapter::Python => vec!["--stdio".to_string()],
                // fortls is stdio LSP by default.
                LanguageAdapter::Fortran => Vec::new(),
            };
            LanguageServerInvocation {
                binary: self.server_binary(),
                args,
                cwd: repo.to_path_buf(),
            }
        }
    }

    /// Catalogue symbols by driving the real language server (source of truth).
    ///
    /// TODO(kovan): implement the LSP client — spawn
    /// [`LanguageAdapter::language_server_invocation`], perform the
    /// `initialize` handshake, request `workspace/symbol` (and
    /// `textDocument/documentSymbol` per file), and normalise the results into
    /// [`KovanSymbol`]s. Until then this returns
    /// [`SemanticsError::Unimplemented`] and callers fall back to the
    /// ripgrep-first [`crate::catalogue_symbols`].
    pub fn catalogue_symbols_via_server(
        _repo: &Path,
        _adapter: LanguageAdapter,
    ) -> Result<Vec<KovanSymbol>, SemanticsError> {
        Err(SemanticsError::Unimplemented(
            "catalogue_symbols_via_server (language-servers feature)",
        ))
    }
}
