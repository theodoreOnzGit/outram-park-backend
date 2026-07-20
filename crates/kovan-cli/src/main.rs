//! # kovan (CLI)
//!
//! The **agent-facing** entry point to KOVAN. It exposes the knowledge-layer
//! operations as plain subcommands with line-oriented output, so a coding
//! agent (Claude Code and friends) can drive KOVAN deterministically and
//! parse the results. Humans get the richer `kovan-tui` instead.
//!
//! ```text
//! kovan discover --root . --kind source
//! kovan search   --path src/lib.rs --pattern "fn \w+"
//! kovan search   --root . --kind source --pattern "fn \w+"
//! kovan scan     --root . --lang rust
//! kovan methods
//! kovan symbols  . --lang rust
//! kovan symbols  . --lang rust --markdown
//! kovan summary  . --lang rust
//! kovan gen root newton-raphson
//! kovan lit import paper.pdf --json-out doc.json
//! kovan lit bibtex doc.json
//! kovan lit outline paper.pdf
//! kovan setup --dry-run
//! ```
//!
//! `discover`, `search`, `scan`, and `methods` wrap `kovan-discovery` and
//! `kovan-codegen`'s catalogue directly. `symbols`/`summary` wrap
//! `kovan-semantics`'s ripgrep-first extractor. `lit` wraps `kovan-literature`'s
//! PDF → Markdown → `KovanDocument` → BibTeX pipeline. `gen` wraps
//! `kovan-codegen::generate`; entries not yet backed by a template report
//! `CodegenError::Unimplemented` as a CLI error (see `kovan methods` for which
//! ones those are). `setup` is a standalone, explicit, online, desktop-scope
//! convenience — see `commands::setup` — that installs a curated list of
//! external CLI tools via `cargo install`; nothing else in this crate calls
//! it or depends on it running.
//!
//! See each `commands::*` submodule for the implementation of one subcommand
//! (or command group) at a time — this file is only the `clap` surface and
//! dispatcher.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod commands;

use commands::gen::GenCommand;
use commands::lit::LitCommand;
use commands::{KindArg, LangArg};

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
    /// Regex search — a single file (`--path`) or a whole repository
    /// (`--root`/`--kind`; root defaults to `.`, kind to `source`). `--path`
    /// wins if both are given.
    Search {
        /// File to search (single-file mode).
        #[arg(long)]
        path: Option<PathBuf>,
        /// Root directory to search (repository mode).
        #[arg(long)]
        root: Option<PathBuf>,
        /// File kind to search, in repository mode (default: source).
        #[arg(long, value_enum)]
        kind: Option<KindArg>,
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
    /// Literature pipeline: PDF import, BibTeX, Markdown outline
    /// (`kovan-literature`).
    #[command(subcommand)]
    Lit(LitCommand),
    /// Catalogue a repository's symbols (`kovan-semantics`, ripgrep-first).
    Symbols {
        /// Root directory of the repository.
        #[arg(default_value = ".")]
        root: PathBuf,
        /// Source language.
        #[arg(long, value_enum)]
        lang: LangArg,
        /// Render the full `symbols.md` artifact instead of line-oriented output.
        #[arg(long)]
        markdown: bool,
        /// Write the `symbols.md` artifact here (implies `--markdown`).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Repository display name for the Markdown heading (default: the
        /// root directory's name).
        #[arg(long)]
        name: Option<String>,
    },
    /// Render `repository-summary.md` for a repository (`kovan-semantics`).
    Summary {
        /// Root directory of the repository.
        #[arg(default_value = ".")]
        root: PathBuf,
        /// Source language.
        #[arg(long, value_enum)]
        lang: LangArg,
        /// Repository ID (default: the display name, lowercased and
        /// space-hyphenated).
        #[arg(long)]
        id: Option<String>,
        /// Repository display name (default: the root directory's name).
        #[arg(long)]
        name: Option<String>,
        /// Write `repository-summary.md` here instead of stdout.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Generate numerical-method Rust source (`kovan-codegen`).
    #[command(subcommand)]
    Gen(GenCommand),
    /// Install a curated list of useful external CLI tools via `cargo
    /// install`, skipping any already on PATH. Explicit, online,
    /// desktop-scope convenience — never run automatically, and has no
    /// bearing on the rest of `kovan`'s offline/Android-clean operation
    /// (see `commands::setup`).
    Setup {
        /// Report what would be installed without installing anything.
        #[arg(long)]
        dry_run: bool,
        /// Reinstall even if the tool's binary is already on PATH.
        #[arg(long)]
        force: bool,
    },
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
            commands::discover::run(root, kind);
            Ok(())
        }
        Command::Search {
            path,
            root,
            kind,
            pattern,
        } => commands::search::run(path, root, kind, &pattern),
        Command::Scan { root, lang } => commands::scan::run(root, lang),
        Command::Methods => {
            commands::methods::run();
            Ok(())
        }
        Command::Lit(cmd) => commands::lit::run(cmd),
        Command::Symbols {
            root,
            lang,
            markdown,
            out,
            name,
        } => commands::symbols::run_symbols(root, lang, markdown, out, name),
        Command::Summary {
            root,
            lang,
            id,
            name,
            out,
        } => commands::symbols::run_summary(root, lang, id, name, out),
        Command::Gen(cmd) => commands::gen::run(cmd),
        Command::Setup { dry_run, force } => commands::setup::run(dry_run, force),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Cli {
        let mut full = vec!["kovan"];
        full.extend_from_slice(args);
        Cli::try_parse_from(full).expect("args should parse")
    }

    #[test]
    fn discover_parses_with_defaults() {
        let cli = parse(&["discover"]);
        match cli.command {
            Command::Discover { root, kind } => {
                assert_eq!(root, PathBuf::from("."));
                assert!(kind.is_none());
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn search_single_file_mode_parses() {
        let cli = parse(&["search", "--path", "src/lib.rs", "--pattern", "fn \\w+"]);
        match cli.command {
            Command::Search {
                path,
                root,
                pattern,
                ..
            } => {
                assert_eq!(path, Some(PathBuf::from("src/lib.rs")));
                assert!(root.is_none());
                assert_eq!(pattern, "fn \\w+");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn search_repository_mode_parses() {
        let cli = parse(&[
            "search",
            "--root",
            ".",
            "--kind",
            "source",
            "--pattern",
            "x",
        ]);
        match cli.command {
            Command::Search {
                path, root, kind, ..
            } => {
                assert!(path.is_none());
                assert_eq!(root, Some(PathBuf::from(".")));
                assert!(kind.is_some());
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn search_requires_pattern() {
        assert!(Cli::try_parse_from(["kovan", "search", "--path", "x"]).is_err());
    }

    #[test]
    fn methods_takes_no_arguments() {
        assert!(matches!(parse(&["methods"]).command, Command::Methods));
    }

    #[test]
    fn symbols_requires_lang() {
        assert!(Cli::try_parse_from(["kovan", "symbols", "."]).is_err());
    }

    #[test]
    fn symbols_parses_with_root_and_flags() {
        let cli = parse(&["symbols", "some/repo", "--lang", "rust", "--markdown"]);
        match cli.command {
            Command::Symbols { root, markdown, .. } => {
                assert_eq!(root, PathBuf::from("some/repo"));
                assert!(markdown);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn summary_defaults_root_to_dot() {
        let cli = parse(&["summary", "--lang", "cpp"]);
        match cli.command {
            Command::Summary { root, .. } => assert_eq!(root, PathBuf::from(".")),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn lit_import_parses() {
        let cli = parse(&["lit", "import", "paper.pdf", "--json-out", "doc.json"]);
        match cli.command {
            Command::Lit(LitCommand::Import { pdf, json_out, .. }) => {
                assert_eq!(pdf, PathBuf::from("paper.pdf"));
                assert_eq!(json_out, Some(PathBuf::from("doc.json")));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn lit_bibtex_parses() {
        let cli = parse(&["lit", "bibtex", "doc.json"]);
        assert!(matches!(
            cli.command,
            Command::Lit(LitCommand::Bibtex { .. })
        ));
    }

    #[test]
    fn gen_root_parses_method_and_out() {
        let cli = parse(&["gen", "root", "newton-raphson", "--out", "nr.rs"]);
        match cli.command {
            Command::Gen(GenCommand::Root { out, .. }) => {
                assert_eq!(out, Some(PathBuf::from("nr.rs")));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn gen_pde_parses() {
        let cli = parse(&["gen", "pde", "poisson1d-finite-difference"]);
        assert!(matches!(cli.command, Command::Gen(GenCommand::Pde { .. })));
    }

    #[test]
    fn gen_requires_a_method() {
        assert!(Cli::try_parse_from(["kovan", "gen", "root"]).is_err());
    }

    #[test]
    fn setup_parses_with_defaults() {
        let cli = parse(&["setup"]);
        match cli.command {
            Command::Setup { dry_run, force } => {
                assert!(!dry_run);
                assert!(!force);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn setup_parses_dry_run_and_force() {
        let cli = parse(&["setup", "--dry-run", "--force"]);
        match cli.command {
            Command::Setup { dry_run, force } => {
                assert!(dry_run);
                assert!(force);
            }
            _ => panic!("wrong variant"),
        }
    }
}
