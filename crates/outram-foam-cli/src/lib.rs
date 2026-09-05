//! # outram-foam-cli
//!
//! OpenFOAM-style **command-line utilities** as terminal binaries. Each tool is
//! its own binary named exactly like upstream OpenFOAM (`blockMesh`,
//! `pimpleFoam`, `gen-foam`, …), so a user drops into a case directory and runs
//! the tool by name, mirroring the OpenFOAM workflow.
//!
//! > **Independent OUTRAM PARK fork, not the official OpenFOAM.** Not affiliated
//! > with or endorsed by OpenCFD Ltd. / the OpenFOAM Foundation / ESI Group; the
//! > tool names identify the upstream utilities re-implemented here. See
//! > `TRADEMARKS.md`. **Unverified until validated** — not for safety-critical use.
//!
//! ## Shared CLI conventions ([`CaseArgs`])
//!
//! Every tool accepts the common OpenFOAM options: `-case <dir>` (default `.`)
//! selects the case directory; standard `--help`/`--version` via `clap`. The
//! tool then reads the case (`system/`, `constant/polyMesh`, time dirs) through
//! [`outram_foam_basic_lib::io`], runs, and writes its output back into the case.
//!
//! ## Wiring status
//!
//! This crate is the thin CLI layer; the actual work lives in the library
//! crates ([`outram_foam_mesh`] for meshing, `outram-foam-appbuilder-lib` for
//! the solvers). See each binary's `--help` and the `op-` beads for what is
//! live vs stubbed.

use std::ffi::OsString;
use std::path::PathBuf;

use clap::Parser;

/// Common OpenFOAM-style command-line options shared by every tool binary.
///
/// Mirrors the upstream convention: a tool is run from (or pointed at) a **case
/// directory** — the folder containing `system/`, `constant/`, and time
/// directories.
#[derive(Debug, Clone, Parser)]
pub struct CaseArgs {
    /// Case directory to operate on (the OpenFOAM `-case` option). Defaults to
    /// the current working directory.
    #[arg(long = "case", short = 'c', default_value = ".")]
    pub case: PathBuf,
}

impl CaseArgs {
    /// The resolved case directory. Errors if it does not exist / is not a dir.
    pub fn case_dir(&self) -> Result<PathBuf, CliError> {
        if self.case.is_dir() {
            Ok(self.case.clone())
        } else {
            Err(CliError::CaseNotFound(self.case.clone()))
        }
    }
}

/// Rewrite an argv so **OpenFOAM single-dash long options** (`-case`, `-help`)
/// are accepted as if they were `clap`'s double-dash form (`--case`, `--help`).
///
/// OpenFOAM utilities use single-dash multi-letter options; `clap` uses
/// double-dash. This promotes any `-<word>` (single dash + two-or-more
/// alphanumeric/`-` chars) to `--<word>`, while leaving genuine short flags
/// (`-c`), bare values (`/tmp`), and negative numbers (`-5.0`) untouched — so
/// `blockMesh -case cavity` works exactly like upstream. Exposed for testing.
pub fn openfoam_argv<I, S>(args: I) -> Vec<OsString>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    args.into_iter()
        .map(|a| {
            let os: OsString = a.into();
            match os.to_str() {
                Some(s)
                    if s.starts_with('-')
                        && !s.starts_with("--")
                        && s.len() > 2
                        && s[1..]
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '-') =>
                {
                    OsString::from(format!("-{s}"))
                }
                _ => os,
            }
        })
        .collect()
}

/// Parse [`CaseArgs`] from the process arguments using the OpenFOAM single-dash
/// option convention (see [`openfoam_argv`]). The entry point every tool binary
/// uses in place of `clap`'s `parse()`.
pub fn openfoam_args() -> CaseArgs {
    CaseArgs::parse_from(openfoam_argv(std::env::args_os()))
}

/// Errors surfaced by the CLI tools (wrapping the library layers).
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// The `-case` directory does not exist or is not a directory.
    #[error("case directory not found: {0}")]
    CaseNotFound(PathBuf),
    /// A case-I/O error (dict/polyMesh/field read or write).
    #[error("I/O error: {0}")]
    Io(String),
    /// The tool ran but the underlying solver / mesher reported an error.
    #[error("tool error: {0}")]
    Tool(String),
    /// The tool is scaffolded but its case-wiring is not yet implemented.
    #[error("{0} is not yet wired to the case interface (scaffold)")]
    NotWired(&'static str),
}
