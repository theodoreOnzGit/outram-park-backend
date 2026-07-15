//! # kovan (CLI)
//!
//! The **agent-facing** entry point to KOVAN. It exposes the knowledge-layer
//! operations as plain subcommands with line-oriented output, so a coding agent
//! (Claude Code and friends) can drive KOVAN deterministically and parse the
//! results. Humans get the richer [`kovan-tui`] instead.
//!
//! ```text
//! kovan discover --root . --kind source
//! kovan search   --path src/lib.rs --pattern "fn \w+"
//! kovan scan     --root . --lang rust
//! kovan methods
//! ```
//!
//! Placeholder stage: `discover`, `search`, and `scan` are wired to real
//! functionality (fd + ripgrep engines); literature/codegen commands print the
//! catalogue and note what is still a stub.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use kovan_codegen::{LinearSolver, Method, NonlinearSolver, OdeSolver, RootFinder};
use kovan_discovery::{discover_kind, search_file, FileKind};
use kovan_semantics::{rough_definition_scan, LanguageAdapter};

/// KOVAN — deterministic knowledge tooling for the Outram Park ecosystem.
#[derive(Parser)]
#[command(name = "kovan", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Discover files under a root, honouring .gitignore.
    Discover {
        /// Root directory to walk.
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// Restrict to a file kind.
        #[arg(long, value_enum)]
        kind: Option<KindArg>,
    },
    /// Search a single file with a regular expression (ripgrep engine).
    Search {
        /// File to search.
        #[arg(long)]
        path: PathBuf,
        /// Regular expression.
        #[arg(long)]
        pattern: String,
    },
    /// Ripgrep-first scan of a repository for probable definitions.
    Scan {
        /// Root directory of the repository.
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// Source language.
        #[arg(long, value_enum)]
        lang: LangArg,
    },
    /// List the numerical-method codegen catalogue.
    Methods,
}

#[derive(Clone, Copy, ValueEnum)]
enum KindArg {
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

#[derive(Clone, Copy, ValueEnum)]
enum LangArg {
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

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    match run(cli.command) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("kovan: error: {msg}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> Result<(), String> {
    match command {
        Command::Discover { root, kind } => {
            let kind = kind.map(FileKind::from);
            let files = match kind {
                Some(k) => discover_kind(&root, k),
                None => kovan_discovery::discover(&root, &[]),
            };
            for f in files {
                println!("{}", f.display());
            }
            Ok(())
        }
        Command::Search { path, pattern } => {
            let hits = search_file(&path, &pattern).map_err(|e| e.to_string())?;
            for h in hits {
                println!("{}:{}: {}", path.display(), h.line, h.text);
            }
            Ok(())
        }
        Command::Scan { root, lang } => {
            let hits =
                rough_definition_scan(&root, lang.into()).map_err(|e| e.to_string())?;
            for (path, m) in hits {
                println!("{}:{}: {}", path.display(), m.line, m.text);
            }
            Ok(())
        }
        Command::Methods => {
            println!("root-finders:");
            for m in [
                RootFinder::Bisection,
                RootFinder::RegulaFalsi,
                RootFinder::Illinois,
                RootFinder::Pegasus,
                RootFinder::Secant,
                RootFinder::NewtonRaphson,
                RootFinder::Brent,
            ] {
                println!("  {:?} (codegen: {})", m, stub_status(Method::Root(m)));
            }
            println!("linear-solvers:");
            for m in [
                LinearSolver::Jacobi,
                LinearSolver::GaussSeidel,
                LinearSolver::Sor,
                LinearSolver::ConjugateGradient,
                LinearSolver::BiCgStab,
                LinearSolver::Gmres,
                LinearSolver::Lu,
                LinearSolver::Qr,
                LinearSolver::Cholesky,
            ] {
                println!("  {:?} (codegen: {})", m, stub_status(Method::Linear(m)));
            }
            println!("nonlinear-solvers:");
            for m in [
                NonlinearSolver::Newton,
                NonlinearSolver::QuasiNewton,
                NonlinearSolver::Broyden,
                NonlinearSolver::TrustRegion,
            ] {
                println!("  {:?} (codegen: {})", m, stub_status(Method::Nonlinear(m)));
            }
            println!("ode-solvers:");
            for m in [
                OdeSolver::Euler,
                OdeSolver::Rk2,
                OdeSolver::Rk4,
                OdeSolver::DormandPrince,
                OdeSolver::BackwardEuler,
                OdeSolver::CrankNicolson,
            ] {
                println!("  {:?} (codegen: {})", m, stub_status(Method::Ode(m)));
            }
            Ok(())
        }
    }
}

/// Report whether codegen for a method is implemented yet (all stubbed today).
fn stub_status(method: Method) -> &'static str {
    match kovan_codegen::generate(method) {
        Ok(_) => "ready",
        Err(_) => "not-implemented",
    }
}
