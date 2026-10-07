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
//! kovan-cli zotero formats
//! kovan-cli zotero import ~/Zotero --to ~/my-kovan-folder --dry-run
//! kovan-cli zotero import library.bib --to ~/my-kovan-folder
//! kovan-cli zotero export --format ris --from ~/my-kovan-folder -o library.ris
//! kovan-cli zotero duplicates ~/my-kovan-folder
//! kovan-cli zotero search ~/my-kovan-folder "pebble bed"
//! kovan-cli setup --dry-run
//! kovan-cli digitise --image fig7.png --x-scale log --x-range 1,1e6 \
//!     --y-scale log --y-range 0.1,10 --figure "Fig. 7" --json fig7.json
//! kovan-cli cost src/lib.rs --by-line
//! kovan-cli outline src/lib.rs --lang rust
//! kovan-cli slice src/lib.rs 10 40
//! kovan-cli skill-gen --out kovan_skill.md
//! kovan-cli def foo --file src/lib.rs
//! kovan-cli sig foo --file src/lib.rs
//! kovan-cli refs foo --file src/lib.rs
//! kovan-cli code-walk --from crates/x/examples/demo.rs::main --to crates/x/src/geom.rs::Sphere::distance
//! kovan-cli code-walk --from crates/x/src/keff.rs::transport_history --depth 2 --format json
//! kovan-cli code-walk-check crates/x/docs/lessons --update
//! kovan-cli stamps-check --diff HEAD~1..HEAD
//! kovan-cli stamps-levels tampines-steam-tables
//! kovan-cli lsp-daemon-stop --root .
//! kovan-cli project regen /path/to/my-kovan-folder
//! ```
//!
//! `discover`, `search`, `scan`, and `methods` wrap `kovan-discovery` and
//! `kovan-codegen`'s catalogue directly. `symbols`/`summary` wrap
//! `kovan-semantics`'s ripgrep-first extractor. `lit` wraps `kovan-literature`'s
//! PDF → Markdown → `KovanDocument` → BibTeX pipeline. `zotero` wraps
//! [`kovan::zotero`] — Zotero import into a Kovan folder and export to every
//! ported Zotero translator (GitHub #752, see `commands::zotero`). `gen` wraps
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
//! `def`/`sig`/`refs` are that same issue's follow-up — rust-analyzer-backed
//! semantic queries wired directly to `kopitiam-semantic`'s
//! `RustAnalyzerSession` (see `commands::semq`); they need `rust-analyzer` on
//! `PATH`. ~~`callers`/`callees`/`impls` are deliberately not implemented yet
//! (`op-l3uz`).~~ **CORRECTED 2026-10-04**: callees now exist as `code-walk`
//! (GitHub issue #523; concept paths and exhaustive call trees, see
//! `commands::code_walk`), with `code-walk-check` as the lessons' staleness
//! gate; `callers` and `impls` are still not implemented. `project regen` wraps [`kovan::project::regenerate_and_write`]
//! — the "kovan folder" `kovan.toml` index generator (GitHub issue #30's
//! project-folder format, op-63u0's design, op-b1y5's implementation); the
//! file is always fully regenerated, never hand-merged — see that module's
//! docs for why.
//!
//! See each `commands::*` submodule for the implementation of one subcommand
//! (or command group) at a time — this file is only the `clap` surface and
//! dispatcher.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use kovan::commands;
use commands::gen::GenCommand;
use commands::ci::CiCommand;
use commands::lit::LitCommand;
use commands::project::ProjectCommand;
use commands::tokens::TokensCommand;
use commands::zotero::ZoteroCommand;
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
    /// Which workspace crates a diff affects, closed over reverse
    /// dependencies — the CI crate selector. Prints `-p a -p b`, or nothing
    /// at all when the whole workspace is selected, so a caller can splice
    /// the output onto a `cargo` command unconditionally.
    Affected {
        /// The ref to diff against, e.g. `origin/develop` or a merge SHA.
        #[arg(long)]
        base: String,
        /// Workspace root (default: discovered).
        #[arg(long)]
        root: Option<PathBuf>,
        /// `cargo-args` (default) or `list`.
        #[arg(long, default_value = "cargo-args")]
        format: String,
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
    /// The push CI's compile gate and test selection: `top-crates`, `smoke`,
    /// `known-failures` (GitHub #414, #416).
    #[command(subcommand)]
    Ci(CiCommand),
    /// Literature pipeline: PDF import, BibTeX, Markdown outline
    /// (`kovan-literature`).
    #[command(subcommand)]
    Lit(LitCommand),
    /// Zotero: import a Zotero library or file into a Kovan folder, export
    /// to every Zotero format, list formats, duplicates, quick search
    /// (GitHub #752).
    #[command(subcommand)]
    Zotero(ZoteroCommand),
    /// The "kovan folder" project format (op-63u0's design): rescan a
    /// project and rewrite its `kovan.toml` index.
    #[command(subcommand)]
    Project(ProjectCommand),
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
    /// Rust-analyzer-backed: where a symbol is defined, plus its signature
    /// (GitHub issue #32's follow-up — wires `kopitiam-semantic` in
    /// directly). Needs `rust-analyzer` on PATH.
    Def {
        #[command(flatten)]
        locator: Locator,
    },
    /// Rust-analyzer-backed: the signature alone.
    Sig {
        #[command(flatten)]
        locator: Locator,
    },
    /// Rust-analyzer-backed: every reference site, as
    /// `file:line:character` coordinates.
    Refs {
        #[command(flatten)]
        locator: Locator,
    },
    /// Rust-analyzer-backed call walk (GitHub issue #523): the shortest
    /// chain(s) of workspace calls from `--from` to `--to`, or, without
    /// `--to`, everything `--from` reaches within `--depth` hops. Functions
    /// are `path/to/file.rs::name` or `path/to/file.rs::Type::name`. Calls
    /// that cannot be followed (trait methods, closures, workspace macros)
    /// are marked UNRESOLVED(<kind>), never guessed. Needs `rust-analyzer`.
    CodeWalk {
        /// Entry point, e.g. `crates/x/examples/demo.rs::main`.
        #[arg(long)]
        from: String,
        /// Concept function; omit for the exhaustive tree.
        #[arg(long)]
        to: Option<String>,
        /// Max hops searched (path mode, default 12) or shown (tree mode,
        /// default 3).
        #[arg(long)]
        depth: Option<usize>,
        #[arg(long, value_enum, default_value_t = commands::code_walk::Format::Markdown)]
        format: commands::code_walk::Format,
        /// Most shortest chains to report.
        #[arg(long, default_value_t = 8)]
        max_paths: usize,
        /// A hop filled in by hand, `<from> -> <to> | <note>` (repeatable).
        #[arg(long)]
        hand: Vec<String>,
        /// Directory containing the workspace `Cargo.toml`.
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// Repository the permalinks point into (`/blob/@@COMMIT@@/...`).
        #[arg(long, default_value = commands::code_walk::render::DEFAULT_REPO_URL)]
        repo_url: String,
    },
    /// The workspace's code map (GitHub #734): every crate placed by its
    /// `[package.metadata.kovan]` tag, with its required dependencies, from
    /// `cargo metadata` (no rust-analyzer). `--format json` writes the data,
    /// `--format svg` the drawn map; the same Cargo.tomls give byte-identical
    /// output.
    CodeMap {
        /// Workspace root; found from the current directory when omitted.
        #[arg(long)]
        workspace: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = commands::code_map::Format::Json)]
        format: commands::code_map::Format,
        /// Output file; stdout when omitted.
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// The workspace call graph (GitHub #737): crate -> module/example ->
    /// function, every resolved call with its call-site lines, unresolved
    /// calls per function, module and crate aggregates, and each function's
    /// source, as deterministic JSON for the code-review UI. Calls are
    /// resolved by rust-analyzer as in `code-walk`; nothing is guessed.
    /// The whole workspace is slow (one definition query per call-shaped
    /// token); `--crates` restricts the scope.
    CallGraph {
        /// Workspace root; found from the current directory when omitted.
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// Comma-separated crate names; every member when omitted.
        #[arg(long)]
        crates: Option<String>,
        /// Output file; stdout when omitted.
        #[arg(short, long, conflicts_with = "split_dir")]
        out: Option<PathBuf>,
        /// Write one `<crate>.json` per crate (no source text) and an
        /// `index.json` (with review stamp states) into this directory, for
        /// web-kovan (GitHub #736), instead of one document.
        #[arg(long)]
        split_dir: Option<PathBuf>,
        /// Merge these call-graph documents (one per crate, written by
        /// earlier runs) instead of building: the incremental site build
        /// (#745). The result is the bytes one run over all of them gives.
        #[arg(long, num_args = 1.., conflicts_with = "crates")]
        merge: Vec<PathBuf>,
    },
    /// One line `<crate> <key>` per workspace member: the cache key of its
    /// call-graph data in the incremental site build (#745). SHA-256 of the
    /// files `cargo package --list` ships, the keys of its workspace
    /// dependencies (transitively), the rust-analyzer version and the
    /// schema versions.
    CallGraphKeys {
        /// Workspace root; found from the current directory when omitted.
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// Comma-separated crate names; every member when omitted.
        #[arg(long)]
        crates: Option<String>,
    },
    /// Regenerates every `<!-- code-walk: ... -->` block in the Markdown
    /// under the given paths and fails if one is stale, a concept path no
    /// longer connects, or a hand-filled hop names a function that no longer
    /// exists (GitHub issue #523; the lessons' staleness gate). `--update`
    /// writes the regenerated walks instead of failing on a difference.
    CodeWalkCheck {
        /// Markdown files or directories to scan.
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        #[arg(long)]
        update: bool,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, default_value = commands::code_walk::render::DEFAULT_REPO_URL)]
        repo_url: String,
    },
    /// Re-hashes the functions recorded in `review/stamps.toml` (GitHub #739)
    /// and prints which human review stamps are VOID and why (code changed,
    /// doc comment changed, function not found), with the permalink to the
    /// stamped code. `--diff A..B` limits it to stamps whose function lines
    /// that range touches, judged at B. Exits non-zero if a stamp in scope is
    /// void. Never edits the file.
    StampsCheck {
        /// Workspace root; found from the current directory when omitted.
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// Revision range, e.g. `HEAD~1..HEAD` or `origin/develop...HEAD`.
        #[arg(long)]
        diff: Option<String>,
        #[arg(long, default_value = commands::code_walk::render::DEFAULT_REPO_URL)]
        repo_url: String,
    },
    /// Which human-rung maturity claims (3, 4) in a crate's
    /// `[package.metadata.kovan]` tag are supported by valid review stamps
    /// (GitHub #739, #735). Report only; never edits a Cargo.toml.
    StampsLevels {
        /// The member crate's name.
        crate_name: String,
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
    /// Creates a human review stamp in `review/stamps.toml` (GitHub #739).
    /// AI AGENTS MUST NEVER RUN THIS: a stamp records a human review and is
    /// the maintainer's alone. Refuses without `--i-am-the-reviewer`, and
    /// refuses a function whose file has uncommitted changes.
    Stamp {
        /// `path/to/file.rs::name` or `path/to/file.rs::Type::name`.
        function: String,
        /// 3 human reviewed, 4 human V&V.
        #[arg(long)]
        rung: u8,
        #[arg(long)]
        reviewer: String,
        #[arg(long, default_value = "")]
        note: String,
        /// Confirms a human reviewer is running this.
        #[arg(long)]
        i_am_the_reviewer: bool,
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
    /// Internal: runs the keep-warm rust-analyzer daemon in the foreground
    /// for one workspace root (op-fdph). Spawned automatically and detached
    /// by `def`/`sig`/`refs`'s client-side logic the first time one of them
    /// is asked about a given root — not meant to be invoked directly by a
    /// human or agent. Unix-only (Linux/macOS/Android); a no-op error on
    /// other platforms, which always use the spawn-per-call path instead.
    /// Runs until stopped with `kovan-cli lsp-daemon-stop` — no idle timeout.
    #[command(hide = true)]
    LspDaemonServe {
        #[arg(long)]
        root: PathBuf,
    },
    /// Stops the keep-warm rust-analyzer daemon (op-fdph) for one workspace
    /// root, if one is running. Not an error if none is — that's the
    /// already-stopped state.
    LspDaemonStop {
        /// Directory containing the workspace `Cargo.toml`.
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
}

/// Shared arguments for the name-based semantic queries (`def`/`sig`/`refs`)
/// — the symbol name plus the file whose `documentSymbol` tree declares it
/// and the workspace root to start rust-analyzer in.
#[derive(clap::Args)]
struct Locator {
    /// The symbol name to resolve (bare or `::`-qualified).
    symbol: String,
    /// The file whose `documentSymbol` tree declares `symbol`.
    #[arg(long)]
    file: PathBuf,
    /// Directory containing the workspace `Cargo.toml`.
    #[arg(long, default_value = ".")]
    root: PathBuf,
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
        Command::Affected { base, root, format } => {
            // Reuse the existing discovery so this works from any
            // subdirectory, same as every other command here.
            let (root, how) = commands::workspace::resolve(root.as_deref())
                .map_err(|e| e.to_string())?;
            eprintln!("workspace: {} ({how})", root.display());
            commands::workspace::fetch_literature(&root);
            let format = match format.as_str() {
                "list" => commands::affected::Format::List,
                _ => commands::affected::Format::CargoArgs,
            };
            commands::affected::run(&root, &base, format).map_err(|e| e.to_string())
        }
        Command::Methods => {
            commands::methods::run();
            Ok(())
        }
        Command::Ci(cmd) => commands::ci::run(cmd),
        Command::Lit(cmd) => commands::lit::run(cmd),
        Command::Zotero(cmd) => commands::zotero::run(cmd),
        Command::Project(cmd) => commands::project::run(cmd),
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
            commands::workspace::fetch_literature(&root);
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
            commands::workspace::fetch_literature(&root);
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
            commands::workspace::fetch_literature(&root);
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
        Command::Def { locator } => {
            commands::semq::run_def(locator.symbol, locator.file, locator.root)
        }
        Command::Sig { locator } => {
            commands::semq::run_sig(locator.symbol, locator.file, locator.root)
        }
        Command::Refs { locator } => {
            commands::semq::run_refs(locator.symbol, locator.file, locator.root)
        }
        Command::CodeWalk {
            from,
            to,
            depth,
            format,
            max_paths,
            hand,
            root,
            repo_url,
        } => commands::code_walk::run(from, to, depth, format, max_paths, hand, root, repo_url),
        Command::CodeMap { workspace, format, out } => {
            let (root, _) = commands::workspace::resolve(workspace.as_deref())
                .map_err(|error| error.to_string())?;
            commands::code_map::run(&root, format, out)
        }
        Command::CallGraph { workspace, crates, out, split_dir, merge } => {
            let (root, _) = commands::workspace::resolve(workspace.as_deref())
                .map_err(|error| error.to_string())?;
            let crates = crates.as_deref().map(commands::call_graph::parse_crates);
            if !merge.is_empty() {
                return commands::call_graph::run_merge(&root, &merge, out, split_dir);
            }
            match split_dir {
                Some(dir) => commands::call_graph::run_split(&root, crates, &dir),
                None => commands::call_graph::run(&root, crates, out),
            }
        }
        Command::CallGraphKeys { workspace, crates } => {
            let (root, _) = commands::workspace::resolve(workspace.as_deref())
                .map_err(|error| error.to_string())?;
            commands::call_graph::run_keys(&root, crates.as_deref().map(commands::call_graph::parse_crates))
        }
        Command::CodeWalkCheck {
            paths,
            update,
            root,
            repo_url,
        } => commands::code_walk::run_check(paths, update, root, repo_url),
        Command::StampsCheck {
            workspace,
            diff,
            repo_url,
        } => {
            let (root, _) = commands::workspace::resolve(workspace.as_deref())
                .map_err(|error| error.to_string())?;
            commands::stamps::run_check(&root, diff, &repo_url)
        }
        Command::StampsLevels {
            crate_name,
            workspace,
        } => {
            let (root, _) = commands::workspace::resolve(workspace.as_deref())
                .map_err(|error| error.to_string())?;
            commands::stamps::run_levels(&root, &crate_name)
        }
        Command::Stamp {
            function,
            rung,
            reviewer,
            note,
            i_am_the_reviewer,
            workspace,
        } => {
            let (root, _) = commands::workspace::resolve(workspace.as_deref())
                .map_err(|error| error.to_string())?;
            commands::stamps::run_stamp(&root, &function, rung, &reviewer, &note, i_am_the_reviewer)
        }
        Command::LspDaemonServe { root } => commands::lsp_daemon::serve(root),
        Command::LspDaemonStop { root } => commands::lsp_daemon::stop(root),
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
    fn zotero_import_and_export_parse() {
        let cli = parse(&[
            "zotero",
            "import",
            "lib.ris",
            "--to",
            "k",
            "--dry-run",
            "--topic",
            "a/b",
            "--topic",
            "c",
        ]);
        match cli.command {
            Command::Zotero(ZoteroCommand::Import {
                source,
                to,
                dry_run,
                topics,
                format,
                ..
            }) => {
                assert_eq!(source, PathBuf::from("lib.ris"));
                assert_eq!(to, PathBuf::from("k"));
                assert!(dry_run);
                assert_eq!(topics, vec!["a/b".to_string(), "c".to_string()]);
                assert_eq!(format, None);
            }
            _ => panic!("wrong variant"),
        }
        let cli = parse(&[
            "zotero", "export", "--format", "ris", "--from", "a", "b", "-o", "x",
        ]);
        match cli.command {
            Command::Zotero(ZoteroCommand::Export { from, out, .. }) => {
                assert_eq!(from, vec![PathBuf::from("a"), PathBuf::from("b")]);
                assert_eq!(out, Some(PathBuf::from("x")));
            }
            _ => panic!("wrong variant"),
        }
        assert!(Cli::try_parse_from(["kovan-cli", "zotero", "import", "x"]).is_err());
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
