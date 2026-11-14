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

pub mod cfmesh;
pub mod set_fields;

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

/// Which OpenFOAM file layout a tool writes (gh:#1363 row 7).
///
/// [`WriteFlavour::Default`] is this project's own form (banner naming
/// OUTRAM PARK, shortest-round-trip numbers). [`WriteFlavour::Esi`] writes the
/// layout of ESI OpenFOAM v2506 (`-flavour esi`): its banner, `arch` and
/// `note` header entries, `writePrecision` `%g` numbers, `inGroups`. See
/// [`outram_foam_basic_lib::io::esi`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WriteFlavour {
    /// The project's own writers (unchanged behaviour).
    #[default]
    Default,
    /// ESI v2506's byte layout.
    Esi,
}

impl WriteFlavour {
    /// Parse the `-flavour` word: `default`/`dev`, or `esi`.
    ///
    /// # Errors
    /// [`CliError::Tool`] for any other word.
    pub fn parse(word: &str) -> Result<Self, CliError> {
        match word {
            "default" | "dev" => Ok(Self::Default),
            "esi" => Ok(Self::Esi),
            other => Err(CliError::Tool(format!(
                "-flavour {other}: expected default, dev or esi"
            ))),
        }
    }
}

/// `-case` plus `-flavour default|esi` for the solver binaries that write
/// fields (gh:#1363 row 7).
#[derive(Debug, Clone, Parser)]
pub struct FlavourArgs {
    /// `-case` directory.
    #[command(flatten)]
    pub case: CaseArgs,
    /// Layout of the written field files: `default` or `esi` (v2506 bytes).
    #[arg(long = "flavour", default_value = "default")]
    pub flavour: String,
}

/// Parse [`FlavourArgs`] from the process arguments (single-dash options).
pub fn flavour_args() -> FlavourArgs {
    FlavourArgs::parse_from(openfoam_argv(std::env::args_os()))
}

/// Write a finished run's fields in the chosen layout; ESI uses the case's
/// `writePrecision`.
///
/// # Errors
/// [`CliError::Tool`] if the solver's writer fails.
pub fn write_run_fields(
    run: &outram_foam_appbuilder_lib::case_runner::CaseRun,
    case: &std::path::Path,
    flavour: WriteFlavour,
) -> Result<outram_foam_appbuilder_lib::case_runner::CaseSummary, CliError> {
    match flavour {
        WriteFlavour::Default => run.write_fields(),
        WriteFlavour::Esi => {
            let opts = outram_foam_basic_lib::io::esi::EsiWriteOptions::default()
                .with_precision(write_precision(case));
            run.write_fields_esi(&opts)
        }
    }
    .map_err(|e| CliError::Tool(e.to_string()))
}

/// `writePrecision` of `<case>/system/controlDict`, or ESI's default 6 when
/// the file or the entry is absent.
pub fn write_precision(case: &std::path::Path) -> usize {
    let file = outram_foam_basic_lib::io::dict::FoamFile::read(case.join("system/controlDict"));
    match file.ok().and_then(|f| f.dict.get_scalar("writePrecision")) {
        Some(p) if p >= 1.0 => p as usize,
        _ => 6,
    }
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
