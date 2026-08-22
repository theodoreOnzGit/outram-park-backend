//! # kovan-cli
//!
//! The **agent-facing** entry point to KOVAN. It exposes the knowledge-layer
//! operations as plain subcommands with line-oriented output, so a coding
//! agent (Claude Code and friends) can drive KOVAN deterministically and
//! parse the results. Humans get the richer `kovan-tui` instead, and the
//! GUI is `kovan` (see `src/bin/kovan.rs`).
//!
//! **Renamed from plain `kovan` to `kovan-cli` on 2026-08-21**, per the final
//! interface spec on GitHub issue #30: exactly three binaries — `kovan`
//! (GUI), `kovan-cli` (this one), `kovan-tui` (terminal UI). The `digitise`
//! subcommand below absorbed the standalone `kovan-digitise` binary the same
//! day, for the same reason.
//!
//! ```text
//! kovan-cli discover --root . --kind source
//! kovan-cli search   --path src/lib.rs --pattern "fn \w+"
//! kovan-cli search   --root . --kind source --pattern "fn \w+"
//! kovan-cli scan     --root . --lang rust
//! kovan-cli methods
//! kovan-cli symbols  . --lang rust
//! kovan-cli symbols  . --lang rust --markdown
//! kovan-cli summary  . --lang rust
//! kovan-cli gen root newton-raphson
//! kovan-cli lit import paper.pdf --json-out doc.json
//! kovan-cli lit bibtex doc.json
//! kovan-cli lit outline paper.pdf
//! kovan-cli setup --dry-run
//! kovan-cli digitise --image fig7.png --x-scale log --x-range 1,1e6 \
//!     --y-scale log --y-range 0.1,10 --figure "Fig. 7" --json fig7.json
//! kovan-cli cost src/lib.rs --by-line
//! kovan-cli outline src/lib.rs --lang rust
//! kovan-cli slice src/lib.rs 10 40
//! kovan-cli skill-gen --out kovan_skill.md
//! ```
//!
//! `discover`, `search`, `scan`, and `methods` wrap `kovan-discovery` and
//! `kovan-codegen`'s catalogue directly. `symbols`/`summary` wrap
//! `kovan-semantics`'s ripgrep-first extractor. `lit` wraps `kovan-literature`'s
//! PDF → Markdown → `KovanDocument` → BibTeX pipeline. `gen` wraps
//! `kovan-codegen::generate`; entries not yet backed by a template report
//! `CodegenError::Unimplemented` as a CLI error (see `kovan-cli methods` for
//! which ones those are). `digitise` wraps
//! [`kovan::digitiser::frontend::AutoArgs`] — the fully automatic graph
//! digitiser pipeline (image in, provenance-carrying data points out, always
//! `UNREVIEWED`); human verification is `kovan-tui`'s Digitiser tab's job.
//! `setup` is a standalone, explicit, online, desktop-scope convenience — see
//! `commands::setup` — that installs a curated list of external CLI tools via
//! `cargo install`; nothing else in this crate calls it or depends on it
//! running. `cost`/`outline`/`slice`/`skill-gen` are GitHub issue #32's
//! token-savings commands — `cost` wraps `kopitiam-tokenizer` directly
//! (a real dependency, not a port — see `commands::cost`), `outline` reuses
//! `kovan-semantics`'s ripgrep-first extractor on one file, `slice` is a
//! plain line-range read, and `skill-gen` writes a Claude Code Skill-format
//! Markdown file describing all of the above for an agent to read.
//!
//! See each `commands::*` submodule for the implementation of one subcommand
//! (or command group) at a time — this file is only the `clap` surface and
//! dispatcher.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use kovan::commands;
use commands::gen::GenCommand;
use commands::lit::LitCommand;
use commands::tokens::TokensCommand;
use commands::{KindArg, LangArg};
use kovan::digitiser::frontend::AutoArgs;

/// KOVAN — deterministic knowledge tooling for the Outram Park ecosystem.
#[derive(Parser)]
#[command(name = "kovan-cli", version, about)]
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
    /// Bundle the workspace's API docs into a flat, upload-ready set of files
    /// for an external chat agent with a fixed context budget.
    ///
    /// Always writes `AGENTS.md` (the workspace's coding rules) and `_INDEX.md`
    /// (a condensed signature index of every documented crate); `--crates` adds
    /// the verbatim `<crate>-api.md` of the crates named. Output is flat because upload
    /// dialogs take files but not folders.
    AgentDocsGen {
        /// Workspace root (the directory containing `crates/`). Discovered
        /// automatically when omitted: the current directory or an ancestor,
        /// then `~`, `~/Documents`, `~/Documents/research`.
        #[arg(long)]
        root: Option<PathBuf>,
        /// Crate directories whose full `<crate>-api.md` to include, comma-separated.
        #[arg(long, value_delimiter = ',')]
        crates: Vec<String>,
        /// Where to write the bundle (default: `<root>/agent-docs`).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Context budget in ESTIMATED tokens (default 200000).
        #[arg(long)]
        budget: Option<u64>,
        /// Generate `docs/<crate>-api.md` for selected crates that lack one. Needs a
        /// nightly toolchain and `rustdoc-md`; not offline and not
        /// deterministic, so it never runs unless asked for.
        #[arg(long)]
        regenerate_missing: bool,
        /// Print the crate inventory and per-crate token estimates, and write
        /// nothing. Run this first to choose a selection.
        #[arg(long)]
        list: bool,
    },
    /// Regenerate a crate's `docs/<crate>-api.md` -- the committed markdown mirror of
    /// its public API -- via nightly rustdoc JSON piped through `rustdoc-md`.
    ///
    /// Replaces the retired `scripts/gen_api_docs.py`. Needs a nightly
    /// toolchain and `rustdoc-md` on PATH; both are mandatory workspace tooling.
    ApiDocs {
        /// Crate directory name under `crates/`, e.g. `outram-foam-basic-lib`.
        /// Omit when using `--all`.
        krate: Option<String>,
        /// Regenerate every crate that already has a `docs/<crate>-api.md`, instead of
        /// one named crate.
        #[arg(long)]
        all: bool,
        /// With `--all`, also generate mirrors for crates that have none yet.
        #[arg(long, requires = "all")]
        include_missing: bool,
        /// Workspace root (the directory containing `crates/`). Discovered
        /// automatically when omitted.
        #[arg(long)]
        root: Option<PathBuf>,
        /// Include private items (`--document-private-items`).
        #[arg(long)]
        private: bool,
    },
    /// Reproduce the paper's productivity accounting: pre-agentic baseline,
    /// agentic output, CSVs, LaTeX tables and the SVG figure.
    ///
    /// Replaces the retired `scripts/kloc_accounting.py`. Measures committed
    /// state at named refs, never a working directory.
    Kloc {
        /// Workspace root (used to site the default output directory).
        /// Discovered automatically when omitted.
        #[arg(long)]
        root: Option<PathBuf>,
        /// Where to write the artifacts (default: `<root>/docs/kloc-accounting`).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Clone any repository not found locally into the vendor directory.
        #[arg(long)]
        clone: bool,
        /// Ignore local checkouts and measure only the vendor clones. This is
        /// the reproduction path: it needs nothing but git and network access.
        #[arg(long)]
        from_github: bool,
        /// Fetch the vendor clones before measuring.
        #[arg(long)]
        fetch: bool,
        /// Compare the measurements against the manuscript's published figures.
        #[arg(long)]
        check: bool,
        /// Skip the SVG figure.
        #[arg(long)]
        no_figure: bool,
    },
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
    /// Per-commit API-token accounting (`kovan-metrics`). The write-side
    /// subcommands are driven by the git hooks and never fail a commit.
    #[command(subcommand)]
    Tokens(TokensCommand),
    /// Pre-merge-to-`main` accounting report: tokens spent and lines/KLOC
    /// written across a window of history (`kovan-metrics`).
    Historian {
        /// Window start, `DDMMYY` (day-month-year, 2-digit year). Omit for
        /// "everything on --branch not yet on --base".
        #[arg(long = "from")]
        from: Option<String>,
        /// Window end, `DDMMYY` (default: today, when --from is given).
        #[arg(long = "to")]
        to: Option<String>,
        /// Branch to report on.
        #[arg(long, default_value = "develop")]
        branch: String,
        /// Base branch for the default "not yet in base" window.
        #[arg(long, default_value = "main")]
        base: String,
        /// Explicit output path (default:
        /// `docs/historian/historian_<from>_to_<to>.md`).
        #[arg(long)]
        outfile: Option<PathBuf>,
    },
    /// Fully automatic graph digitiser: plot image in, provenance-carrying
    /// data points out. Absorbed from the former standalone `kovan-digitise`
    /// binary on 2026-08-21 (GitHub issue #30's 3-binary consolidation).
    ///
    /// The emitted dataset is always marked `UNREVIEWED` — human
    /// verification is `kovan-tui`'s Digitiser tab's job.
    Digitise {
        #[command(flatten)]
        auto: AutoArgs,
        /// Write the dataset as JSON to this path.
        #[arg(long)]
        json: Option<String>,
        /// Write the dataset as CSV (provenance embedded as `#` header lines).
        #[arg(long)]
        csv: Option<String>,
        /// Print a one-line summary to stderr instead of staying silent.
        #[arg(long)]
        verbose: bool,
    },
    /// Estimate a file's token cost (GitHub issue #32) — a dependency-free,
    /// per-Unicode-script BPE approximation (`kopitiam-tokenizer`), read
    /// before deciding whether to read the whole file.
    Cost {
        /// File to estimate.
        path: PathBuf,
        /// Also print a per-line breakdown.
        #[arg(long)]
        by_line: bool,
    },
    /// Declarations-only skeleton of one file (GitHub issue #32) —
    /// ripgrep-first, reusing `kovan-semantics`'s repository-wide extractor
    /// on a single file.
    Outline {
        /// File to outline.
        path: PathBuf,
        /// Source language.
        #[arg(long, value_enum)]
        lang: LangArg,
    },
    /// Print one line range of a file instead of the whole thing (GitHub
    /// issue #32) — the third leg of the `cost -> outline -> slice` loop.
    Slice {
        /// File to slice.
        path: PathBuf,
        /// First line (1-based, inclusive).
        start: usize,
        /// Last line (1-based, inclusive).
        end: usize,
    },
    /// Write a Claude Code Skill-format Markdown file documenting
    /// `kovan-cli` for an AI agent (GitHub issue #32).
    SkillGen {
        /// Output path (default: `kovan_skill.md`).
        #[arg(long)]
        out: Option<PathBuf>,
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
        Command::AgentDocsGen {
            root,
            crates,
            out,
            budget,
            regenerate_missing,
            list,
        } => {
            let (root, root_how) =
                commands::workspace::resolve(root.as_deref()).map_err(|error| error.to_string())?;
            println!("workspace {} ({root_how})", root.display());
            let (out_dir, how) = commands::agent_docs_gen::resolve_out_dir(out.as_deref())
                .map_err(|error| error.to_string())?;
            println!("writing to {} ({how})", out_dir.display());
            commands::agent_docs_gen::run(
                &root,
                &out_dir,
                &crates,
                budget,
                regenerate_missing,
                list,
            )
            .map_err(|error| error.to_string())
        }
        Command::ApiDocs {
            krate,
            all,
            include_missing,
            root,
            private,
        } => {
            let (root, how) =
                commands::workspace::resolve(root.as_deref()).map_err(|error| error.to_string())?;
            println!("workspace {} ({how})", root.display());
            commands::api_docs::run(&root, krate.as_deref(), all, include_missing, private)
                .map_err(|error| error.to_string())
        }
        Command::Kloc {
            root,
            out,
            clone,
            from_github,
            fetch,
            check,
            no_figure,
        } => {
            let (root, how) =
                commands::workspace::resolve(root.as_deref()).map_err(|error| error.to_string())?;
            println!("workspace {} ({how})", root.display());
            let out_dir = out.unwrap_or_else(|| commands::kloc::default_out_dir(&root));
            commands::kloc::run(out_dir, clone, from_github, fetch, check, no_figure)
                .map_err(|error| error.to_string())
        }
        Command::Setup { dry_run, force } => commands::setup::run(dry_run, force),
        Command::Tokens(cmd) => commands::tokens::run(cmd),
        Command::Historian {
            from,
            to,
            branch,
            base,
            outfile,
        } => commands::historian::run(from, to, branch, base, outfile),
        Command::Digitise {
            auto,
            json,
            csv,
            verbose,
        } => run_digitise(auto, json, csv, verbose),
        Command::Cost { path, by_line } => commands::cost::run(path, by_line),
        Command::Outline { path, lang } => commands::outline::run(path, lang.into()),
        Command::Slice { path, start, end } => commands::slice::run(path, start, end),
        Command::SkillGen { out } => commands::skill_gen::run(out),
    }
}

/// Run the automatic digitiser pipeline and write/print its output. Mirrors
/// the former standalone `kovan-digitise` binary's `main` exactly.
fn run_digitise(
    auto: AutoArgs,
    json: Option<String>,
    csv: Option<String>,
    verbose: bool,
) -> Result<(), String> {
    let (_raster, dataset) = auto.run().map_err(|e| e.to_string())?;
    if verbose {
        let frame = dataset
            .trace
            .as_ref()
            .map(|t| format!("{:?} (auto: {})", t.frame, t.frame_auto_detected))
            .unwrap_or_else(|| "none".to_string());
        eprintln!(
            "kovan-cli digitise: {} points traced, frame {frame}, review status: UNREVIEWED",
            dataset.points.len()
        );
    }
    if dataset.points.is_empty() {
        eprintln!(
            "kovan-cli digitise: warning: no curve points found — check --threshold/--curve-rgb \
             and that the image really contains a curve inside the axis frame"
        );
    }
    let mut wrote = false;
    if let Some(p) = &json {
        dataset
            .write_json(std::path::Path::new(p))
            .map_err(|e| e.to_string())?;
        wrote = true;
    }
    if let Some(p) = &csv {
        dataset
            .write_csv(std::path::Path::new(p))
            .map_err(|e| e.to_string())?;
        wrote = true;
    }
    if !wrote {
        // No output file requested: JSON on stdout, scriptable.
        println!("{}", dataset.to_json_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Cli {
        let mut full = vec!["kovan-cli"];
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
        assert!(Cli::try_parse_from(["kovan-cli", "search", "--path", "x"]).is_err());
    }

    #[test]
    fn methods_takes_no_arguments() {
        assert!(matches!(parse(&["methods"]).command, Command::Methods));
    }

    #[test]
    fn symbols_requires_lang() {
        assert!(Cli::try_parse_from(["kovan-cli", "symbols", "."]).is_err());
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
        assert!(Cli::try_parse_from(["kovan-cli", "gen", "root"]).is_err());
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
