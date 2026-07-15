# kovan-cli — design decisions

Recorded 2026-07-15, as the KOVAN library crates (`kovan-common`,
`kovan-discovery`, `kovan-literature`, `kovan-semantics`, `kovan-codegen`)
were fleshed out from placeholder to real functionality by other agents in
parallel. This crate's job was to wire that new functionality through to the
`kovan` CLI, agent-facing (`docs/kovan.md`).

## Baseline

Before touching anything: `cargo build --release -p kovan-common
-p kovan-discovery -p kovan-literature -p kovan-semantics -p kovan-codegen
-p kovan-cli -p kovan-tui` was green (previously-cached "Finished in 0.10s").
`cargo check --workspace --lib --tests` also passed with only pre-existing,
unrelated warnings in other workspace crates (`boon-lay`,
`tampines-steam-tables`). One transient failure was observed mid-session:
`kovan-tui` briefly failed with `E0583: file not found for module 'tui'`
because a concurrent agent was actively writing `crates/kovan-tui/src/tui/`
(git showed the directory as untracked, then a `mod.rs` appeared moments
later). This resolved itself on retry and was not touched — `kovan-cli` does
not depend on `kovan-tui`, and per the task instructions the fix belongs to
whichever agent owns that crate, not to this one.

## Command shape

Followed the existing style in the placeholder `main.rs`: a single `clap`
`Parser`/`Subcommand` derive tree, flat subcommands for single-purpose
operations (`discover`, `scan`, `methods`, `symbols`, `summary`), nested
subcommand groups where a family of related operations shares an underlying
crate (`lit import|bibtex|outline`, `gen root|linear|nonlinear|ode|pde`).

- **`discover` / `scan` / `methods`** — kept as-is, only extending `methods`
  to also list the new `Method::Pde(PdeScheme)` family (`pde-schemes:`).
- **`search`** — extended rather than duplicated. The old command only
  supported `--path <file> --pattern <re>` (single-file). Added `--root`/
  `--kind` as an alternative mode that calls the new
  `kovan_discovery::search_repository`, with `--path` winning if both are
  given. This keeps the existing invocation working byte-for-byte (mod one
  intentional change below) rather than adding a separate `search-repo`
  command, since both modes answer the same question ("where does this regex
  match?") and a coding agent should not have to remember two verb names for
  it.
  - **Intentional behaviour change:** single-file mode's output line changed
    from `path:line: text` to `path:line:column: text`, now that
    `kovan_discovery::SearchMatch` carries a `column` field. This is a
    strictly additive, ripgrep-consistent format (`rg`'s own default output
    is `path:line:column:text`), so it was judged safe to change rather than
    keep the old 2-field format for backward compatibility — no other code
    in this workspace parses `kovan search`'s stdout today (the CLI is new
    enough that there is no consumer to break).
- **`symbols` / `summary`** — new, one command each rather than folding
  `summary` into `symbols --summary`. Different output artifact
  (`symbols.md` vs. `repository-summary.md`), different required inputs
  (`summary` needs a synthesised `KovanRepository` record — id/name/language
  — that `symbols` doesn't), and `docs/kovan.md`'s own "Outputs" section
  already names them as two separate deliverables. Splitting keeps each
  command's flag set free of the other's.
- **`gen`** — one nested subcommand per method family with a `clap::ValueEnum`
  per family for the method, rather than a single `gen <family> <method>`
  with a free-text method string. This gets `--help`/tab-completion listing
  of valid methods per family for free and rejects typos at parse time
  instead of at `kovan_codegen::generate`'s `CodegenError::Unimplemented`
  (which is reserved for "catalogued but no template yet", not "not a real
  method name").
- **`lit`** — three subcommands (`import`, `bibtex`, `outline`), matching the
  task's explicit ask for `import`/`bibtex` plus one more to exercise
  `markdown_outline` (the third piece of the literature crate's public API
  the task named). `import` prints a line-oriented `key: value` summary
  (not JSON) as the default view, with `--json-out`/`--markdown-out` for the
  full record — consistent with every other command's "line-oriented by
  default, richer artifact opt-in" shape.

## `clap::ValueEnum` mirrors, not derived on the library enums

`kovan-discovery::FileKind`, `kovan-semantics::LanguageAdapter`, and all five
`kovan-codegen` catalogue enums (`RootFinder`, `LinearSolver`,
`NonlinearSolver`, `OdeSolver`, `PdeScheme`) are plain `Debug + Clone + Copy
+ PartialEq + Eq` enums with no `clap` dependency — correctly so, since those
crates must stay CLI-agnostic. `clap::ValueEnum` is a foreign trait, so it
cannot be implemented on those foreign enums from `kovan-cli` (Rust's
orphan rule). Every one of them therefore gets a local 1:1 mirror
(`KindArg`, `LangArg`, `RootFinderArg`, …) plus a `From` conversion, matched
exhaustively both ways — the same pattern the placeholder `main.rs` already
used for `KindArg`/`LangArg`, just extended to the `kovan-codegen` catalogue.
This is boilerplate but mechanical and compiler-checked: an added catalogue
variant is a non-exhaustive-match compile error at the `From` impl, not a
silent CLI gap.

## What was exposed vs. left out

Exposed everything named in the task brief:
`pdf_import`/`markdown`/`metadata::extract_metadata`/`bibtex::to_bibtex`
(`lit import`/`bibtex`/`outline`); `catalogue_symbols_detailed` +
`outputs::{symbols_markdown, repository_summary_markdown}` (`symbols`/
`summary`); `generate(Method)` incl. `Method::Pde` (`gen`, `methods`);
`search_repository` + `SearchMatch.column` (`search --root`, and column now
in every search output).

Left out, deliberately:

- **`kovan_literature::extract_assets`** (embedded-image extraction) — no
  CLI command. The task brief named `pdf_import`, `markdown`,
  `metadata::extract_metadata`, `bibtex::to_bibtex` specifically, not asset
  extraction, and there was no obvious deterministic, line-oriented output
  shape for "wrote N image files" that adds enough value over just running
  `extract_assets` from a script — flagged here rather than silently
  skipped. Candidate for a future `lit assets <pdf> --out-dir <dir>` if an
  agent workflow needs it.
- **`kovan_semantics::adapters`** (the deferred language-server escalation
  scaffolding) — intentionally not wired. It is off-by-default,
  non-Android, and (per that crate's own docs) not yet backed by a working
  `rust-analyzer`/`clangd`/Pyright/`fortls` integration; wiring a CLI flag to
  an unimplemented path would just relocate the "not implemented" error
  without adding anything a user couldn't already get from
  `kovan_semantics::SemanticsError::Unimplemented`.
- **`kovan_common`** — no direct CLI surface (e.g. no `kovan doc show
  <id>`). There is no persistence layer yet (`docs/kovan.md`'s "Deterministic
  First": generated Markdown over a hidden database), so a `KovanDocument`
  only exists as the JSON file `lit import --json-out` writes; there is
  nothing yet to "look up by id" beyond reading that file directly.
- **A `--json` output mode on `discover`/`search`/`scan`/`symbols`** — the
  task asked for line-oriented, deterministic output specifically ("Keep
  output parseable by coding agents"); ripgrep-style `path:line:col: text`
  and `key: value` are already trivially parseable without a JSON encode/
  decode round trip, and `docs/kovan.md`'s "Deterministic First" principle
  explicitly ranks plain text above databases/structured stores as the
  preferred medium. `lit import --json-out` is the one place JSON was added,
  because that path's job is specifically to produce the *canonical*
  on-disk `KovanDocument` record (re-consumed by `lit bibtex <file>.json`),
  not an ad hoc report.

## Library-API friction encountered

None blocking. Two small observations, not required fixes:

- `kovan_semantics::outputs` module is private (`mod outputs;` in
  `kovan-semantics/src/lib.rs`, with `pub use outputs::{repository_summary_
  markdown, symbols_markdown};` re-exporting only the two functions). This
  crate imports the two functions from the crate root
  (`kovan_semantics::{repository_summary_markdown, symbols_markdown}`), not
  from `kovan_semantics::outputs::*` — worth knowing if a future command
  needs anything else from that module, since it isn't reachable by path.
- `ExtractedSymbol`'s doc comment already notes (see `kovan-semantics`'s own
  `DECISIONS.md`) that `KovanSymbol` doesn't yet carry file/line — `kovan
  symbols`'s line-oriented mode therefore prints location from
  `ExtractedSymbol` (the richer, location-carrying record), not from
  `KovanSymbol`. If `kovan-common::KovanSymbol` grows a location field later,
  this command should be revisited to use the plain `catalogue_symbols` path
  instead.

## Beads

`op-5v5` (the KOVAN epic) is JSONL-only / not in local Dolt per the task
brief — no beads were created or modified. Follow-up work worth a bead when
the epic is reachable again:

- `lit assets` CLI command (see "What was exposed vs. left out" above).
- A `kovan-tui` cross-check once that crate's browser/symbols/methods/
  literature screens (seen mid-write during this session) land, to confirm
  its output/flag conventions stay consistent with this CLI's.
- Revisit `kovan symbols`'s KovanSymbol/ExtractedSymbol split if/when
  `KovanSymbol` gains a location field (see "Library-API friction" above).

## Testing approach

Two layers, per the task's "CLI arg parsing + a couple of end-to-end command
runs on synthetic/temp inputs" ask:

- **Unit tests** (`src/main.rs` + each `commands/*.rs`) — `clap` parsing via
  `Cli::try_parse_from`, covering every subcommand's shape (required flags,
  defaults, mode dispatch for `search`), plus small pure-function tests
  (`default_repo_name`, `load_document`, the `KindArg`/`LangArg`
  `From`-exhaustiveness, one `gen` mapping/generation check per family).
- **End-to-end tests** (`tests/cli_e2e.rs`) — spawn the compiled `kovan`
  binary (`CARGO_BIN_EXE_kovan`) against a synthetic tempdir "repository"
  fixture and a synthetic PDF built with `lopdf` (mirroring
  `kovan-literature`'s own private, non-exported `test_pdf.rs` helper, so no
  real — and possibly proprietary — PDF ever ships as a fixture, and no
  `DATA_POLICY.md` concern arises). Covers `discover`/`search` (both
  modes)/`scan`/`methods`/`symbols` (both output modes)/`summary`
  (file-write mode)/`gen` (success + unimplemented-error case)/`lit`
  (import → json-out → bibtex round trip, direct-PDF bibtex, outline,
  missing-file error).

23 unit tests + 16 end-to-end tests, all passing under `cargo test --release
-p kovan-cli`.

## Verification run (2026-07-15)

- `cargo build --release -p kovan-common -p kovan-discovery
  -p kovan-literature -p kovan-semantics -p kovan-codegen -p kovan-cli
  -p kovan-tui` — clean (the one transient `kovan-tui` failure noted above
  was a concurrent-write race, not a real break; a retry a few seconds later
  built clean).
- `cargo check --workspace --lib --tests` — clean (workspace-wide, confirms
  no other crate depends on/breaks from `kovan-cli`'s new `Cargo.toml`
  dependencies).
- `cargo test --release -p kovan-cli` — 23 + 16 = 39 passed, 0 failed.
- `cargo fmt -p kovan-cli -- --check` — clean.
- `cargo clippy --release -p kovan-cli --all-targets -- -D warnings` —
  clean (one `clippy::cmp_owned` finding fixed during development: a test
  compared a `PathBuf` against an owned `PathBuf::from(".")`; switched to a
  `match` + `assert_eq!` instead of `matches!` with an inline comparison).
- `cargo doc -p kovan-cli --no-deps` — clean, no warnings.
- `cargo check -p kovan-cli --target aarch64-linux-android` — clean (the CLI
  is non-GUI and stays Android-buildable; no new dependency here is
  Android-hostile — `lopdf` is already used, and is pure-Rust, by
  `kovan-literature`, and is dev-only here).
