# kovan — design decisions

## Crate consolidation (2026-08-21)

Merged the formerly separate `kovan-cli` and `kovan-tui` crates into this one
`kovan` crate, at the maintainer's explicit request, so a single crate hosts
all three of KOVAN's front ends:

- `kovan` (binary) — the agent-facing CLI, unchanged in behaviour (same
  subcommands, same output). Was `kovan-cli`'s `src/main.rs`; now
  `src/bin/kovan.rs`, with its `commands/` module tree moved to this crate's
  `src/commands/` and re-exported via `pub mod commands;` in `src/lib.rs` so
  a `[[bin]]`-only file can `use kovan::commands;`.
- `kovan-tui` (binary) — the human-facing terminal UI, unchanged in
  behaviour. Was `kovan-tui`'s `src/main.rs` + `src/tui/`; now
  `src/bin/kovan-tui.rs` + this crate's `src/tui/`, re-exported the same way
  behind `#[cfg(not(target_os = "android"))] pub mod tui;`.
- `kovan-gui` (binary, **new**) — added to satisfy the ask that this crate
  carry a GUI interface too, without building a second GUI implementation.
  It reused `kovan_literature::digitiser::gui::run` (extracted from what was
  previously private code inside `kovan-literature`'s own
  `kovan-digitise-gui` binary) — the digitiser window is KOVAN's one
  established GUI surface, and this binary was a thin wrapper around the same
  library function `kovan-digitise-gui` also called. Gated behind this
  crate's non-default `gui` feature, which turned on `kovan-literature`'s
  `digitise-gui` feature in turn, so the default `kovan`/`kovan-tui` builds
  never pulled egui/eframe.

  **Superseded later the same day.** Once this crate was relicensed to
  AGPL-3.0-only to take `kopitiam-pdf` as a dependency (GitHub issue #30's
  PDF-native digitiser work — see this crate's `NOTICE`), the digitiser
  itself — not just `kovan-gui`'s wrapper around it — moved into this crate
  from `kovan-literature`: `src/digitiser/` and all three binaries
  (`kovan-gui`, `kovan-digitise`, `kovan-digitise-tui`). `kovan-literature`
  must stay GPL-3.0-only (it's used well beyond the GUI), so the digitiser
  could only become PDF-native from somewhere allowed to depend on
  `kopitiam-pdf` — which is `kovan` alone. `kovan-literature`'s own
  `kovan-digitise-gui` binary (which did exactly what `kovan-gui` already
  did) was retired rather than carried forward as a second name for the same
  thing. This paragraph documents that as an append, not a rewrite — the
  bullet above is accurate history for the state it describes.

  **Superseded again, still the same day: five binaries collapsed to
  three.** After the digitiser-move paragraph above landed, the maintainer
  posted the final interface spec on GitHub issue #30: "have 3 binaries —
  kovan / kovan-cli / kovan-tui", explicitly because they wanted `kovan`
  usable on Android. The five binaries at that point (`kovan`, `kovan-tui`,
  `kovan-gui`, `kovan-digitise`, `kovan-digitise-tui`) became exactly three:

  - `kovan-gui.rs` renamed to `kovan.rs`, taking over the plain `kovan`
    binary name — `cargo run -p kovan --bin kovan --features gui` now opens
    the GUI, where it used to run the CLI.
  - The old `kovan.rs` (CLI) renamed to `kovan-cli.rs`. It gained a
    `digitise` subcommand that flattens `AutoArgs` exactly as the retired
    `kovan-digitise.rs` binary's `Cli` struct did; `kovan-digitise.rs` was
    deleted, its `main` becoming `kovan-cli.rs`'s `run_digitise` helper.
  - `kovan-digitise-tui.rs` was deleted and its `App` (fields: `raster`,
    `dataset`, `operator`, `json_path`, `csv_path`, `selected`, `dirty`,
    `message`; methods: `select`/`nudge`/`delete_selected`/
    `duplicate_selected`/`save`) became `src/tui/digitiser.rs`'s
    `ReviewState`, unchanged in mechanics. The new file adds a `Setup`
    phase (a small text-field form building `AutoArgs` by hand rather than
    via `clap`) ahead of it, and a `Running` phase (worker thread + `mpsc`
    channel, mirroring the [`tui::ingest`] tab's pattern exactly — see that
    module's own docs for why a thread is the right tool for a
    potentially-slow library call). `Tab` gained a seventh variant,
    `Digitiser`, bound to digit `7`.
  - **This forced a second, unplanned consequence: `kovan-tui`'s Android
    gate came off entirely.** `ratatui` had already been made an
    unconditional (non-Android-gated) dependency earlier the same day
    specifically to keep the standalone `kovan-digitise-tui` binary's
    Android-functional review screen working (see the `Cargo.toml` comment
    on the `ratatui` line). Once the Digitiser tab moved *inside*
    `kovan::tui` — and `pub mod tui;` in `src/lib.rs` was still
    `#[cfg(not(target_os = "android"))]` at that point — the tab's Android
    functionality would have been silently lost again: `kovan-tui`'s
    `main()` still stubbed to a CLI-redirect message on Android, so the
    whole seven-tab TUI, digitiser tab included, would never run there.
    Checking why the module was gated at all turned up that the gate had
    become vestigial: it existed only because `ratatui` used to be
    Android-gated too, and that was no longer true. Removed
    `#[cfg(not(target_os = "android"))]` from `pub mod tui;` and the
    matching `#[cfg(target_os = "android")]` stub `main()` in
    `src/bin/kovan-tui.rs`, leaving one ungated `fn main() { kovan::tui::run() }`.
    Verified clean with `cargo check -p kovan --all-targets --target
    aarch64-linux-android` before and after (both 0 errors) — the module
    tree itself needed no other change to compile there; nothing inside
    Browser/Symbols/Methods/Literature/Ingest turned out to be
    Android-hostile either. `kovan-tui` is now genuinely
    Android/Termux-*usable*, not merely Android-*buildable* — which is
    exactly what the maintainer's stated reason for this restructuring
    ("I want kovan usable on android") asked for, even though the request
    only named the binary count, not the TUI's own Android gate.
  - `tests/cli_e2e.rs` was updated to spawn `CARGO_BIN_EXE_kovan-cli`
    instead of `CARGO_BIN_EXE_kovan` (Cargo's bin-name env vars keep the
    hyphen verbatim — confirmed by the tests passing unmodified otherwise),
    since plain `kovan` is now the GUI and cannot run headlessly.
  - Filed as kopi-beans `op-ygmc` (child of the GUI epic `op-9c2e`) before
    starting, and claimed for the duration of the work.

Nothing in the workspace depended on `kovan-cli` or `kovan-tui` as libraries
(`grep` for `kovan-cli`/`kovan-tui` in `[workspace.dependencies]` returned
nothing before the merge), so this was a pure rename/reorganisation with no
downstream API to migrate. Root `Cargo.toml` workspace members, the root
`CLAUDE.md` crate table, and every doc/script referencing the old crate names
were updated in the same change.

The two crates' original decision logs are preserved verbatim below, for the
rationale behind the CLI/TUI command and screen designs themselves (unchanged
by the merge).

---

# kovan-cli — design decisions (historical, pre-merge)

> Preserved verbatim from the former `crates/kovan-cli/DECISIONS.md`
> ahead of the 2026-08-21 consolidation into this crate. References to
> `kovan-cli` below describe that crate as it existed before the merge.


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

---

# kovan-tui — design decisions (historical, pre-merge)

> Preserved verbatim from the former `crates/kovan-tui/DECISIONS.md`
> ahead of the 2026-08-21 consolidation into this crate. References to
> `kovan-tui` below describe that crate as it existed before the merge.


This is the record of growing `kovan-tui` from its static-overview placeholder
(one screen, no interaction beyond `q`/`Esc`) into five interactive, real-data
screens over the five sibling `kovan-*` libraries (2026-07-15). Nothing in
`kovan-common`, `kovan-discovery`, `kovan-semantics`, `kovan-literature`, or
`kovan-codegen` was touched — this pass is `kovan-tui` only, per the task
brief.

## What changed

- **Resolved the crate's one `// TODO(kovan)`** (the placeholder-stage note in
  `main.rs`'s module doc, "The real browser panes are TODO(kovan)") by
  building five tabs: Overview (kept, refactored into `tui/overview.rs`),
  Browser (`kovan-discovery`), Symbols (`kovan-semantics`), Methods
  (`kovan-codegen`), Literature (`kovan-literature`).
- **Restructured `src/main.rs`** from one large inline `mod tui { ... }` block
  into `main.rs` (Android/desktop entry-point split only) plus a `src/tui/`
  module directory: `mod.rs` (App state + dispatch + terminal loop),
  `text_input.rs`, and one file per tab. Every file is well under the
  workspace's 1000-line cap (`root CLAUDE.md`) — see the README's Layout
  table for line counts (largest is `literature.rs` at 465 lines).
- **State is enum-dispatched, not trait-object-dispatched**, per the
  workspace's "no `dyn Trait`" rule: `Tab` is a closed 5-variant enum, and
  `App::handle_key`/`draw` `match` on it directly to each tab's own
  `handle_key`/`draw` function. Same pattern for `KindFilter` (Browser),
  `Family` (Methods), and `LitKind` (Literature) — each is a small closed
  enum with a hand-rolled cyclic `step(delta)` rather than pulling in an enum
  iteration crate.
- **No `Box<T>`, no lifetimes, no `Arc<RwLock<T>>`.** `App` owns one state
  struct per tab by value. The workspace's `Arc<RwLock<T>>` rule (root
  `CLAUDE.md`, "Shared state") is written for the simulation-timestep
  shared-mutable-state pattern (multiple threads computing over the same
  fields, then synchronising); a `ratatui` terminal event loop is
  single-threaded and synchronous — `terminal.draw` and `event::read` and
  `app.handle_key` all run on the one thread, one after another, with nothing
  ever borrowed across threads. Introducing a lock here would add ceremony
  with no corresponding safety benefit, which the workspace's "human
  interface layer" principle explicitly warns against ("do not add
  complexity … if it raises the mental context load for a human reader").
  Flagged as an explicit judgement call in case a future change (e.g.
  background/async scanning so a large repository scan doesn't block the
  draw loop) reintroduces real cross-thread sharing — at that point the rule
  would apply and `Arc<RwLock<T>>` would be the right tool.
- **Every tab is a read-only viewer.** None of the five screens write to a
  repository or to the literature storage tree (`open/`/`proprietary/`/
  `generated/`) they browse — matching KOVAN's "not a repository modification
  agent" non-goal (`docs/kovan.md` § "Non-Goals"). The Literature tab reads
  existing `.md`/`.bib`/`.pdf` files but never calls `kovan_literature`'s
  writer-shaped functions (there are none exposed as writers today — the
  crate itself only generates strings, e.g. `to_bibtex`, `pdf_to_markdown`;
  writing them to disk is left to the CLI/caller, and `kovan-tui` doesn't do
  that either).
- **Key-handling is a pure reducer, testable without a terminal.** Every tab's
  `handle_key(&mut self, key: KeyEvent, ..)` only mutates its own state struct
  (plus, for three tabs, a shared `editing: &mut bool` flag owned by `App`) —
  no I/O beyond the explicit "run the scan now" actions
  (`run_discovery`/`run_catalogue`/`generate_selected`/`run_scan`), which
  synchronously call the sibling crate and are exactly what a human pressing
  Enter expects to happen. This is what makes the 44-test unit suite possible
  without spinning up a real terminal: construct a state struct, call
  `handle_key` with synthetic `KeyEvent`s (`KeyEvent::new(code, modifiers)`,
  from `ratatui::crossterm::event`), assert on the resulting struct fields.
- **Render tests use `ratatui::backend::TestBackend`**, not a real terminal:
  `Terminal::new(TestBackend::new(w, h))`, `terminal.draw(|f| draw(f, ..))`,
  then either assert the draw call didn't panic (covers every
  family/tab/view-mode combination cheaply) or inspect
  `terminal.backend().buffer().content()` (a `&[Cell]`, joined via
  `Cell::symbol()`) for expected substrings. Fully headless and Android-clean
  in principle (though the whole `tui` module — tests included — is gated off
  Android anyway, since `ratatui` itself is desktop-only).
- **`TextInput` (`tui/text_input.rs`)** is a deliberately minimal shared
  single-line buffer (`push_char`/`backspace`/`value`/`new`, plus
  `clear`/`set` used by test fixtures and carrying an explained
  `#[allow(dead_code)]` since no tab's `handle_key` calls them today — a user
  clears a field by hand with repeated `Backspace`). No cursor-in-the-middle
  editing, no selection: KOVAN's TUI is a viewer/navigator, not a text
  editor, and every current caller only ever needs "type a path, press
  Enter."
- **Cargo.toml**: added `tempfile.workspace = true` as a plain
  `[dev-dependencies]` entry (not target-gated) — it's pure-Rust and
  Android-friendly (same precedent as `kovan-discovery`'s use, and confirmed
  again here: `cargo check -p kovan-tui --tests --target aarch64-linux-android`
  is clean), and in any case it's only reachable from `#[cfg(test)]` code
  inside `src/tui/`, itself behind `main.rs`'s Android gate on `mod tui;` —
  it can never reach an Android *library* build even in principle.
- **`kovan-tui/README.md`** — this crate had no README before; added one
  (screens table, key-binding reference, Android section, testing
  methodology, layout). Follows the `kovan-cli/README.md` structure/tone for
  consistency across the two front-end crates.

## Design choices, spelled out

- **Tab switching is `1`-`5` (direct) plus `Tab`/`Shift+Tab` (cyclic)**, not
  `Left`/`Right` at the top level — `Left`/`Right` are reserved, per tab, for
  cycling that tab's own filter/family/language (Browser's `KindFilter`,
  Symbols' `LanguageAdapter`, Methods' `Family`, Literature's `LitKind`).
  Overloading `Left`/`Right` for both "switch tab" and "cycle filter" would be
  ambiguous depending on which tab is active; keeping tab-switching on
  digits/`Tab` avoids that entirely.
- **Editing mode is a single `bool` owned by `App`, not per-tab.** Only one
  tab is ever active at a time, so only one text field can be under edit at
  once; a single flag is simpler than N per-tab flags and there is no
  scenario where two tabs edit simultaneously. When `editing` is true, the
  global quit/tab-switch keys are suppressed in `App::handle_key` *before*
  dispatch, so a user typing `"q"` or a digit into a path never accidentally
  quits or switches tabs mid-edit (`editing_suppresses_global_quit_and_tab_switch_keys`
  test in `tui/mod.rs`).
- **The Methods tab has no text field and thus no `editing` interaction at
  all** — `MethodsState::handle_key` takes a plain `KeyEvent`, not the
  `editing: &mut bool` the other three tabs take. This is a deliberate API
  asymmetry (documented on the function itself) rather than a fake unused
  parameter every other tab carries.
- **Literature's `Enter` previews rather than re-scans** (unlike
  Browser/Symbols, where a plain `Enter` re-runs the scan). Once a Literature
  list is populated, "show me what this entry is" is the more useful default
  action than "scan again with the same root" — re-scanning is still one key
  away (`r`). This is called out explicitly in both the tab's own doc comment
  and the README's key-binding table so the asymmetry isn't a silent surprise.
- **Symbols' Markdown-view scroll steals `Up`/`Down` from list navigation**
  while that view is active (`markdown_view_scroll_does_not_move_the_list_selection`
  test) — there is nothing to select in a text preview, so repurposing the
  same two keys for scrolling avoids adding a second pair of bindings for
  what is, from the user's perspective, still "the thing that moves within
  the pane I'm looking at."
- **Methods' family list uses `Debug`-derived labels** (`format!("{x:?}")` on
  each `RootFinder`/`LinearSolver`/… variant) rather than a hand-written label
  table like `Family::label`/`KindFilter::label`/`LitKind::label` use for
  their own (much smaller, hand-curated) enums. `kovan-codegen`'s method enums
  are numerous (9 linear solvers alone) and already `#[derive(Debug)]`, and
  their variant names (`Bisection`, `ConjugateGradient`, `DormandPrince`, …)
  are exactly what a user typing `kovan gen linear conjugate-gradient` (the
  CLI's kebab-case form) would recognise in PascalCase — writing a parallel
  label table would be pure duplication with a real risk of drifting out of
  sync as `kovan-codegen`'s catalogue grows (it already has, since this
  session started: `Method::Pde` / `PdeScheme` exist and are wired in here).
- **Literature root defaults to `"crates/kovan-literature"`**, i.e. it assumes
  `kovan-tui` is launched from the repository root — matching how a workspace
  member binary is normally run (`cargo run -p kovan-tui`, or the built binary
  invoked from the repo root). Browser/Symbols default to `"."` instead
  (any directory is a valid thing to browse/catalogue, with no workspace-
  specific assumption). This asymmetry is intentional, not an oversight.
- **`.bib` discovery** goes through `kovan_discovery::discover(root, &["bib"])`
  directly rather than `discover_kind`/`FileKind`, because `FileKind` doesn't
  have a BibTeX category (its four variants are Source/Markdown/Pdf/Metadata
  — see `kovan-discovery`'s crate docs) and `discover` already accepts an
  arbitrary extension list, so this needed no change to `kovan-discovery`
  (out of scope for this pass per the task brief).

## What was deliberately left out (not gold-plated)

- **No async / background scanning.** `run_discovery`/`run_catalogue`/
  `run_scan`/`generate_selected` all run synchronously on the draw-loop
  thread; a very large repository scan would visibly block the UI for its
  duration. Every sibling-crate call here is already what the `kovan-cli`
  agent-facing commands do synchronously too, and KOVAN's own design
  principles favour simple, deterministic, inspectable operation over
  responsiveness machinery — but this is worth a second look if a real user
  points the Browser/Symbols tab at something the size of, say, the whole
  workspace or a vendored OpenFOAM checkout. Flagged, not built, since
  nothing in the brief asked for it and adding a worker-thread + channel (or
  `Arc<RwLock<T>>`-shared result) would be exactly the complexity the
  "human interface layer" principle warns against introducing speculatively.
- **No PDF-asset preview** (`kovan_literature::extract_assets`). The
  Literature tab previews metadata/outline/raw-text only; a fourth preview
  mode for "list the JPEG/JPEG2000 images this PDF embeds" would be a
  reasonable future addition but wasn't requested and the storage tree is
  currently empty of real PDFs to test it against meaningfully (see
  Verification below).
- **No cursor-in-the-middle text editing, no clipboard, no undo** in
  `TextInput` — see its own doc comment. Every current field only needs
  append/backspace.
- **No colour theming beyond the existing `Modifier::REVERSED` highlight**
  the placeholder screen already used — kept consistent across all five
  tabs rather than inventing a colour palette, since KOVAN's design
  principles say nothing about visual branding and the workspace has no
  existing TUI style guide to match.

## Open questions for human review

1. **Is synchronous (blocking) scanning acceptable long-term?** See "What was
   deliberately left out" above. Fine for this workspace's own crates (a few
   hundred files, sub-second `catalogue_symbols_detailed` calls observed
   during manual testing — e.g. cataloguing `kovan-tui`'s own ~2100-line
   source tree found 144 symbols instantly); worth re-evaluating if KOVAN is
   ever pointed at something OpenFOAM-sized.
2. **The Literature tab's storage tree is currently near-empty** (only
   `.gitkeep` placeholders under `open/{papers,reports,standards,benchmarks}/`
   and `generated/{markdown,bibtex,assets}/`, per `kovan-literature`'s
   `docs/kovan.md`-specified layout) — the only real content found during
   manual testing was `kovan-literature/DECISIONS.md` itself (a Markdown
   file, correctly discovered and its outline correctly previewed). The PDF
   metadata-extraction preview path (`extract_metadata`) was exercised by
   `kovan-tui`'s own unit tests (against synthetic PDFs built the same way
   `kovan-literature`'s and `kovan-cli`'s own test fixtures are, per the
   workspace's data-provenance rule — no real/proprietary PDF was used
   anywhere) but not against a real paper/report during the manual `tmux`
   walkthrough, since none exists in the tree yet. Worth a second pass once
   real literature is imported.
3. **No beads filed.** Per this agent's brief, KOVAN epic `op-5v5` is
   JSONL-only / not in local Dolt, and I was told not to create children
   under it or try to fix the sync. Recording the two follow-ups above here
   instead, per the brief's instruction, for whenever that sync issue is
   resolved.
4. **Concurrent sibling-crate work.** `kovan-cli` gained several new
   subcommands (`symbols`, `summary`, `gen`, `lit`) during this session,
   evidently from a different concurrent agent — this pass did not add
   equivalent new *tabs* for those beyond what's already covered (Symbols tab
   ≈ `symbols`/`summary`; Methods tab ≈ `gen`/`methods`; Literature tab ≈
   `lit`), since the five-screen scope was set by this task's brief. Worth
   comparing the two front ends' feature parity in a follow-up if `kovan-cli`
   keeps growing.

## Verification performed (this pass, 2026-07-15, all commands run for real)

- `cargo build --release -p kovan-tui` — clean, no warnings.
- `cargo check -p kovan-tui --target aarch64-linux-android` — clean (Android
  stub path).
- `cargo check -p kovan-tui --tests --target aarch64-linux-android` — clean
  (confirms the new `tempfile` dev-dependency is Android-buildable too).
- `cargo test --release -p kovan-tui` — **44/44 unit tests pass**.
- `cargo fmt -p kovan-tui -- --check` — clean (after running `cargo fmt`
  once).
- `cargo clippy --release -p kovan-tui --all-targets -- -D warnings` — clean
  (six `field_reassign_with_default` findings in test fixtures fixed with
  struct-update syntax; nothing else flagged).
- `RUSTDOCFLAGS="-D warnings" cargo doc -p kovan-tui --no-deps --release` —
  clean, no broken-doc-link or missing-doc warnings.
- `pandoc -f gfm+tex_math_dollars -t html --mathml README.md > /dev/null` —
  exit 0, no warnings (this README uses no math, but the workspace mandate
  applies the same check to every README regardless).
- **Manual interactive smoke test**, real compiled binary in a `tmux`
  session (`./target/release/kovan-tui`, 120×35): confirmed the Overview tab
  renders the tagline and module list; switched to Browser, edited the root
  to `crates/kovan-tui/src`, scanned, confirmed all 8 real source files
  listed; switched to Symbols, scanned the same root, confirmed 144 real
  symbols catalogued (self-referentially — the TUI catalogued its own
  source), toggled the Markdown view and confirmed the `symbols.md`-shaped
  output rendered with a real kind-count table; switched to Methods,
  generated Bisection and confirmed the real generated Rust source (with its
  full rustdoc) appeared in the preview pane; switched to Literature, scanned
  `crates/kovan-literature`, found the one real Markdown file present
  (`DECISIONS.md`), previewed it and confirmed a real heading outline
  rendered; returned to Overview and confirmed `q` exits cleanly (tmux
  session terminated, pane no longer existed on the next capture attempt).

---

# Ingestion pass — decisions, assumptions, open questions (2026-08-05)

Second pass on this crate: adding **interactive literature ingestion** (tab 6,
`src/tui/ingest/`) so a maintainer can import a PDF from inside the TUI instead
of typing `kovan lit import`. Scope was `crates/kovan-tui/` only — no sibling
`kovan-*` crate was modified.

## What changed

- **New Ingest tab** (`src/tui/ingest/`, three files): a four-phase state
  machine (`Picking → Running → Review → saved`, with `Failed` off both), a PDF
  picker over `kovan_discovery::discover_kind(root, FileKind::Pdf)`, a worker
  thread running `kovan_literature::extract_metadata`, an editable metadata
  review form, and saving of Markdown / `KovanDocument` JSON / BibTeX.
- **The library calls are exactly the CLI's** — `extract_metadata` and
  `to_bibtex`, matching `kovan-cli/src/commands/lit.rs`. Nothing about the
  pipeline is reimplemented here.
- **Literature tab gained `i`** — hands the selected PDF to the Ingest tab
  (`LiteratureState::take_ingest_request`, drained by `App::handle_key`) so the
  read-only viewer stays read-only and all writing lives in one screen.
- **The draw loop is now polled** (`event::poll` + `App::tick`) instead of
  blocking on `event::read`, so a running extraction animates and delivers its
  result with no key press. Poll interval is 100 ms only while work is in
  flight, 1000 ms otherwise.
- **`q`/`Esc` are refused on the Ingest tab while work is in flight** — a
  running extraction or an unsaved review must be dismissed with `x` first.
  Losing hand-corrected metadata to a reflexive `q` is a bad trade.
- **`serde_json`** added under the existing non-Android dependency table (it is
  only reachable from the Android-gated `tui` tree).

## Why the review step is the point of this feature

`kovan_literature::extract_metadata` documents itself as best-effort, and it is.
Importing the real 1977 Argonne benchmark-problem report during this pass
reproduced the reported failure exactly: `title: ANL-7416 Supplement 2`
(correct), `year: 2004` (the scanner's digitisation date), `authors: []` (the
real corporate author is "Argonne Code Center"), giving `slug: 2004anl7416`.
That record would render a wrong `@misc` BibTeX entry and, from there, a wrong
citation — a provenance error under `RESEARCH_INTEGRITY_AND_PROVENANCE.md`.

So the tab never saves what the extractor produced without showing it first. It
also flags what is *typically* wrong (empty authors; a year later than years
found in the document's own front matter; `Other` document type; a title shaped
like a report number) — advisories only, never auto-correction, because silently
"fixing" metadata would be the same integrity problem wearing a different hat.

## Design choices, spelled out

- **A worker thread + `std::sync::mpsc` channel, not `Arc<RwLock<T>>` and not an
  async runtime.** The previous pass flagged that background work would be the
  moment to revisit the no-lock decision; having built it, a lock is still not
  the right tool. Nothing is *shared*: the worker owns its `PathBuf`, sends one
  `Result<KovanDocument, String>`, and exits. That is the produce-once pipeline
  the root `CLAUDE.md` shared-state rule explicitly contrasts with simulation
  state, and it needs no runtime — `std::thread` plus one channel is the whole
  mechanism.
- **Elapsed time and a spinner, never a percentage.** `kovan-literature` exposes
  no progress callback, and a fabricated progress bar is a small lie of exactly
  the kind KOVAN exists to avoid. Measured for the record (release build,
  developer desktop): 12 MB / 447 pages → 0.3 s; 1.4 MB / 103 pages → 0.1 s.
  Faster than the brief assumed — the worker thread is still right, because the
  call is unbounded in principle and much slower in a debug build or on a phone.
- **`catch_unwind` around the library call.** PDF parsing runs over untrusted
  third-party bytes; a panic there must become a `Failed` phase, not a dead
  process with the terminal left in raw mode. A worker that dies without sending
  is caught too (`TryRecvError::Disconnected`). The terminal is cleared on every
  transition out of `Running`, because a panic message printed by the default
  hook can smear the frame.
- **Abandon, not cancel.** `extract_metadata` has no cancellation token, so `x`
  drops the receiver and the detached worker's result is discarded. The UI says
  exactly that rather than implying the work stopped.
- **The slug/id are re-derived from the corrected values** (`ingest/metadata.rs`),
  so fixing the year really does change the citation key
  (`2004anl7416` → `argonnecodecenter1977anl7416`), and the default output paths
  follow it. See the API gap below.
- **Output paths default to the storage layout** via
  `kovan_literature::storage::generated_dir_for`, so Markdown and BibTeX land in
  `generated/{markdown,bibtex}/{open,proprietary}/` with the visibility the
  extraction inferred from the source path. The JSON record has **no** directory
  defined in `docs/kovan.md`; it defaults beside the Markdown, flagged in the
  code as this crate's choice rather than a layout rule. Hand-editing any path
  pins all three so later slug changes stop moving the user's files.
- **Corporate authors are first-class.** The author line parses `;`-separated
  entries, splitting `Family, Given` on a comma; an entry without a comma is a
  corporate author (`family` set, `given` empty), which is the convention
  `kovan_common::Author` documents. Typing `Argonne Code Center` therefore yields
  one organisation, not three people.
- **Document type is cycled, not typed** (Left/Right) — a closed enum cannot be
  misspelled, and the type drives the BibTeX entry type.

## Missing from `kovan-literature`'s API (worked around, not patched)

- **No public identifier derivation.** `make_slug`/`make_id` are private to
  `kovan-literature`'s `metadata.rs` and run exactly once, inside
  `extract_metadata`. There is no `derive_identifiers(&mut KovanDocument)` or
  equivalent, so a corrected document cannot ask the library to re-derive its
  own slug/id. Both functions are **mirrored** in `ingest/metadata.rs`, kept
  byte-for-byte compatible and pinned by a test that reproduces the library's
  own `doe2021test` expectation. This is real duplication and will drift if the
  upstream algorithm changes — the proper fix is a public function there.
- **No progress reporting.** `extract_metadata` (and `pdf_to_markdown`) are
  all-or-nothing calls with no callback and no page counter, so the UI can only
  show elapsed time. A `page_done: usize` callback (or an iterator over pages)
  would let a real progress bar exist without inventing numbers.
- **No cancellation.** Nothing in the API takes a stop flag, hence "abandon"
  rather than "cancel".
- **No metadata-confidence signal.** The crate knows *which* source each field
  came from (Info dictionary, cover page, text fallback) but discards that in
  the returned `KovanDocument`, so the TUI re-derives its advisories from the
  finished record. Exposing provenance per field (`title_source`, `year_source`)
  would make the review screen sharper and is arguably what a best-effort
  extractor owes its reviewer.

## What was deliberately left out

- **No editing of DOI, keywords, abstract, journal locators, visibility or
  document body.** The review form covers the fields that drive the citation and
  the identifiers (and the ones observed to be wrong). The rest can be edited in
  the saved JSON, which is the canonical record.
- **No re-opening of a previously saved JSON for a second review pass.** Worth
  adding if metadata correction becomes iterative.
- **No BibTeX append/merge into an existing `.bib` file** — each save writes one
  entry to its own path, matching `kovan lit bibtex`'s one-entry output.
- **No batch/queue ingestion.** One document at a time, deliberately: the whole
  value of this screen is a human looking at each record.
- **No cursor-in-the-middle text editing** — `TextInput` is still
  append/backspace only, so replacing a long default path means holding
  `Backspace`. Mildly annoying with the ~70-character default output paths; the
  first real fix should probably be a "clear field" key rather than a full
  editor.

## Verification performed (this pass, 2026-08-05, all commands run for real)

- `cargo check -p kovan-tui --bins --tests` — clean.
- `cargo test --release -p kovan-tui` — **98/98 unit tests pass** (44 before
  this pass; the new ones cover the picker, the worker/channel path end to end,
  metadata correction and slug regeneration, saving and its failure modes, and
  rendering of every phase).
- `cargo clippy --release -p kovan-tui --all-targets -- -D warnings` — clean.
  One `#[allow(clippy::large_enum_variant)]` on `IngestPhase` with the reason
  recorded in its doc comment (the suggested fix is `Box`, which the workspace
  forbids).
- `RUSTDOCFLAGS="-D warnings" cargo doc -p kovan-tui --no-deps --release` — clean.
- `cargo check -p kovan-tui --all-targets --target aarch64-linux-android` —
  clean (the full `--all-targets` form the root `CLAUDE.md` mandates, not the
  `--lib`-only proxy).
- `pandoc -f gfm+tex_math_dollars -t html --mathml README.md > /dev/null` —
  exit 0, no warnings.
- **Manual interactive test against a real document**, compiled release binary
  in `tmux` (200×50): imported the real 12 MB / 447-page Argonne report from a
  local (gitignored, uncommitted) working directory. Extraction returned in
  0.3 s with 447 pages and 557 707 characters of Markdown; the review screen
  reproduced the reported failure (`ANL-7416 Supplement 2` / `2004` / no
  authors / `2004anl7416`) and raised all four advisories, including "earlier
  years in the text: 1963, 1968, 1971, 1972, 1973, 1977". Corrected the author
  to `Argonne Code Center`, the year to `1977`, the type to `Report` and the
  institution to `Argonne National Laboratory`; the slug updated live to
  `argonnecodecenter1977anl7416` and the pane showed "(extractor said
  '2004anl7416')". Redirected all three output paths to a scratch directory and
  saved: `@techreport{argonnecodecenter1977anl7416, author = {Argonne Code
  Center}, title = {ANL-7416 Supplement 2}, year = {1977}, institution =
  {Argonne National Laboratory}}`, a 608 KB JSON record round-tripping the
  corrections, and a 567 KB Markdown body. Also verified the Literature tab's
  `i` hand-off (which surfaced the same failure class on a second report:
  `NEACRP-L-330`, year `2007`), that `q` is refused with an unsaved review on
  screen, and that `x` then `q` exits cleanly with the terminal restored. No
  file was written anywhere inside the repository, and no PDF was added to any
  test fixture (both test documents are local, gitignored material).

## Open questions for human review

1. **Should the mirrored slug/id derivation live in `kovan-literature`
   instead?** It is the one piece of genuine duplication introduced here, and
   the only reason it exists is that the library's derivation is private. A
   public `derive_identifiers` would let this crate delete ~40 lines and remove
   a drift risk. Out of scope for this pass (kovan-tui only).
2. **Should corrections be recorded in the saved record?** Right now a saved
   `KovanDocument` does not say which fields a human changed — only the live UI
   shows the `*` markers. A `corrected_fields: Vec<String>` (or a note in
   `tags`) would make provenance auditable after the fact, which is arguably
   what the integrity policy wants; it needs a `kovan-common` field, so it was
   not done here.
3. **Default output paths assume the repository root as cwd** (they start
   `crates/kovan-literature/generated/…`), matching the Literature tab's
   existing assumption. Running the binary from elsewhere silently produces a
   relative path under the wrong directory — visible in the form before saving,
   but a `--archive-root` flag or a persisted setting would be better.
4. **No beads filed** — this agent's brief was kovan-tui only and beads are the
   maintainer's to open/close; the three items above are the candidates.

## GitHub issue #30 workbench, first slice: PDF engine decision, file picker, theming, PDF reader (2026-08-21)

Started implementing the `op-9c2e` epic ("kovan GUI: PDF-native literature
workbench"). Of its 13 child issues, four had no unmet dependency and were
picked up in this pass: `op-6ez3` (DECISION: PDF page rendering engine),
`op-t5sq` (Gruvbox theming), `op-689u` (file picker), and `op-95x6`
(integrated PDF reader view, unblocked once `op-6ez3` was resolved).
Deliberately **not** touched this pass: `op-63u0` (DESIGN: the "kovan
folder" project format) and `op-9bvi` (DECISION: OCR tooling/policy for
table digitisation) — both are genuinely maintainer-scope decisions (the
OCR one explicitly conflicts with this crate's documented "no ML" digitiser
rule and needs an exception granted, not assumed), so everything chained
off them (`op-hnhp`, `op-b1y5`, `op-9vml`, `op-wr08`, `op-x3wl`, `op-5sdc`,
`op-p17q`) stays unimplemented. See `bn show op-9c2e` for the full graph.

**`op-6ez3` — PDF page rendering engine: decided as `kopitiam_pdf::mupdf`.**
This was largely already decided by the `kopitiam-pdf` dependency landing
earlier the same day (see this crate's `NOTICE` and the `Cargo.toml`
comment on that dependency) — that dependency's whole stated purpose was
this decision. What this pass did was the missing half: actually call it.
`kopitiam_pdf::mupdf::PdfDocument::open(bytes)` parses a PDF's xref/page
tree; `.page_count()` reports pages; `kopitiam_pdf::mupdf::rasterize_page(&doc,
page_index, dpi)` returns a `Pixmap { w, h, n, alpha, stride, samples }` —
confirmed via `rasterize_page`'s own doc comment ("a fresh white DeviceRGB
Pixmap") that `n == 3`, no alpha, so the reader converts with
`ColorImage::from_rgb` (an `alpha` branch to `from_rgba_unmultiplied` is
kept for robustness but is not expected to trigger against this version).
No alternative engine was seriously evaluated — `kopitiam-pdf` is this
crate's maintainer's own pure-Rust MuPDF port, already a dependency for
exactly this purpose, Android-Termux-clean, and reusing it needs no new
licence/FFI/build-toolchain surface. Rejecting it in favour of, say, an
`-sys` binding to real MuPDF or `pdfium` would have reopened questions
(C toolchain, Android story, licence) this dependency already closed.

**`op-t5sq` — Gruvbox theming: ported, not re-derived.** New file
`src/digitiser/gui/desktop/theme.rs`, copied from
`tampines-steam-tables-gui/theme.rs`'s `GuiTheme` enum + `gruvbox_visuals`
+ the `GRUVBOX_*` hex constants, field-for-field identical. The original
file's other half — `figure_palette`/`live_ink_colour`, which style that
crate's *exported* PNG/PDF/SVG figures — was **not** ported: `kovan` has no
equivalent exported-figure concept, so porting it would have been dead code
kept "in case," which the workspace's anti-scaffolding rule forbids. A
top-bar dropdown (`ComboBox::from_id_salt("gui-theme")`) selects between
the two variants; `DigitiseApp::theme.apply(ctx)` runs once per frame in
`eframe::App::ui` — cheap enough not to gate behind a change-detection
check.

**`op-689u` — file picker: reused `egui-file-dialog`, already a workspace
dependency.** `tampines-steam-tables-gui` already depends on
`egui-file-dialog 0.13` (`[workspace.dependencies]`) and uses exactly the
`FileDialog::new() → .pick_file()/.pick_directory() → .update(ctx) →
.take_picked()` flow this pass needed — grepped for it
(`docs`/`CLAUDE.md`'s "search before building" hard rule) before writing
anything, per that crate's own `app.rs`. One `FileDialog` instance lives on
`DigitiseApp`, shared by both "open a file" actions (the digitiser's
`Browse…` button and the PDF reader's `Open PDF…` button) via a small
`FileDialogTarget { Image, Pdf }` enum recording which action asked for the
pick, read back in `handle_picked_file` once `take_picked()` resolves. Two
named extension filters (`Images`: png/jpg/jpeg, `PDF`: pdf) are registered
up front rather than swapped per-action, since `egui-file-dialog`'s filter
methods are builder-style (consume `self`, meant for construction) and a
user picking the "wrong" file type through an unfiltered-by-default picker
is a minor inconvenience, not a correctness issue. Declared as a new
`[target.'cfg(not(target_os = "android"))'.dependencies]` entry alongside
`eframe`/`egui`, gated into the `gui` feature the same way
(`dep:egui-file-dialog`).

**`op-95x6` — PDF reader: new `desktop::pdf_reader` submodule.** One page
is rasterized and cached at a time (`PdfReaderState::texture_page` tracks
which page the cached texture belongs to) rather than pre-rendering the
whole document, so opening a large PDF stays cheap and only visited pages
cost render time — matching how the digitiser's own image loading is
already lazy (texture uploaded on first draw after `load_image`). Fixed
`RENDER_DPI = 150.0` for the rasterization; the zoom slider scales the
*displayed* texture size rather than triggering a re-rasterize per zoom
level, so zooming in past 100% shows raster blur — flagged as a known limit
in the module doc rather than solved, since re-rasterizing per zoom step
was not asked for and adds real complexity (when to re-rasterize, at what
granularity) for a workbench feature nothing downstream depends on yet.

**File-size-cap-driven restructuring: `gui.rs` became `gui/mod.rs` +
`gui/desktop/{mod.rs,theme.rs,pdf_reader.rs}`.** The single `gui.rs` file
(745 lines before this pass) would have crossed this crate's own
"well under the 1000-line cap" convention (see this crate's `README.md`
"Layout") once the new panels landed inline. Split via `git mv gui.rs
gui/mod.rs`, then mechanically extracted the private `mod desktop { ... }`
block's body into `gui/desktop/mod.rs` (dedented, no content changes) with
`#[cfg(not(target_os = "android"))] mod desktop;` replacing the inline
block in `gui/mod.rs` — same Android-gating semantics, just file-based
instead of inline. `theme.rs` and `pdf_reader.rs` are private submodules of
`desktop`, so they inherit its Android gate for free without repeating
`#[cfg(not(target_os = "android"))]` on each file.

**Verification.** `cargo check -p kovan --all-targets` and `--target
aarch64-linux-android` both clean throughout (checked after the file split,
after each new module, and after the final wiring); `cargo tree -p kovan
--target aarch64-linux-android -e features` confirms zero
`eframe`/`egui`/`egui-file-dialog` on that target, matching the existing
`gui` default-feature pattern. `cargo test --release -p kovan --lib --tests`
green (unaffected — no test coverage was added for the new GUI code itself,
see "Known gaps" below). Manual smoke test: built `kovan` in release,
launched under `xvfb-run` (`libxkbcommon-x11-0` installed to get past an
initial missing-library panic) — got as far as opening a native window and
initialising `winit`'s event loop before failing at
`WGPU error: Failed to create surface for any enabled backend`, which is
this sandbox having no GPU/DRI surface for `wgpu` to bind to, not a defect
in the code reachable from a static check. **No interactive click-through
of the new panels was possible in this environment** — the maintainer
should smoke-test file-picking, PDF paging/zoom, and theme switching on a
real desktop before trusting this beyond "compiles and the logic reads
correctly."

**Known gaps, left for the maintainer or a follow-up pass:**

1. **No automated tests for the new GUI code.** `DigitiseApp`'s existing
   logic (calibration, auto-trace, point editing) was already untested at
   the unit level — this pass did not change that policy, just didn't
   improve it either. `PdfReaderState`'s page-open/rasterize path and the
   file-dialog routing are equally untested. A `PdfDocument`/`Pixmap`-level
   test (open a small synthetic PDF fixture, rasterize page 0, assert
   dimensions) would be the cheapest first slice, mirroring
   `tests/cli_e2e.rs`'s synthetic-PDF-via-`lopdf` fixture pattern.
2. **No re-rasterization on zoom** (see the `op-95x6` paragraph above) —
   raster blur above 100% zoom is accepted, not fixed.
3. **The PDF reader is display-only** — it does not yet feed a page (or a
   cropped region of one) into the digitiser as a plot-image source. That
   wiring is `op-p17q`, unimplemented, and is what would make this reader
   actually replace "screenshot then digitise" per the issue's original
   ask; today a user must still export/screenshot a page to digitise it.
4. **File filters are registered but not verified against
   `egui-file-dialog`'s actual runtime behaviour** (no interactive test was
   possible — see "Verification" above) — if the picker's default filter
   doesn't behave as the API docs describe, that would only surface on a
   real desktop run.

## A conflicted pull asks "sure anot?", and can be forced (2026-09-23, GH issue #279)

**Maintainer, 2026-09-23:** "there are merge conflicts when i pull from there. I
want it such to be when merge conflicts arise during pull, kovan gives a popup
box saying (you may have unsaved changes, u sure u want to pull anot?) then two
boxes say (yes, can) or (no, i manage myself)" — about the local Kovan folder
(`local-kovan-repo`). Clarified in the same exchange: **"yes, can" is a forced
pull, overriding local changes.**

### What Git actually prints — measured, not assumed

Before writing the classifier, each failure mode was reproduced against a real
`git` driving a local bare remote (2026-09-23). The result that shaped the
design:

| case | Git says | stream |
|---|---|---|
| uncommitted edit to a file the merge touches | `error: Your local changes to the following files would be overwritten by merge:` | stderr |
| both sides committed the same lines | `CONFLICT (content): Merge conflict in <file>` / `Automatic merge failed` | **stdout** |
| pulled again, merge unresolved | `error: Pulling is not possible because you have unmerged files.` | stderr |
| both sides committed, no `pull.rebase` set | `fatal: Need to specify how to reconcile divergent branches.` | stderr |

The second row is why this was not a pure GUI change. `advanced_git::run_git_in`
kept **stderr only** on a non-zero exit, so the true merge-conflict case — the
one the issue is about — rendered in the tab as `From <url>\n * branch main ->
FETCH_HEAD`, **a message that does not mention a conflict at all**. That is
fixed here (`git_output_in` keeps both streams; `pull_in` classifies on the
pair), and it would have been missed entirely by reasoning about what `git pull`
"obviously" prints.

The divergent-branches row is included in `is_conflict` deliberately, even
though Git frames it as missing configuration rather than a conflict: the folder
*has* diverged, and the forced pull is exactly what resolves it.

### The two answers

- **"yes, can"** → `advanced_git::force_pull_in`: abort whatever merge or rebase
  the failed pull left behind, `git fetch <remote> <branch>`,
  `git reset --hard FETCH_HEAD`, `git clean -fd`. `FETCH_HEAD` rather than
  `<remote>/<branch>` so it also works in a folder with no remote-tracking ref.
  **Untracked files are wiped too** — the maintainer's explicit choice when
  asked, the folder being meant to end up an exact mirror of the remote. Ignored
  files and submodule contents survive (`clean` without `-x`, without `-ff`).
- **"no, i manage myself"** → `advanced_git::abort_in_progress_in`: restore the
  pre-pull state, also the maintainer's choice when asked. The alternative —
  leaving Git's half-applied merge in place for the user to resolve by hand — was
  rejected as leaving a folder in a state whose owner "should not need Git
  vocabulary" (op-wqaw) cannot get out of, and whose next Pull click fails
  confusingly until they do.

`abort_in_progress_in` checks `git rev-parse --git-path MERGE_HEAD` /
`rebase-merge` / `rebase-apply` rather than running `git merge --abort` and
swallowing the failure, because that exits 128 with "There is no merge to abort"
in the common case where Git refused the pull outright and changed nothing.

Only `RemoteError::Conflict` raises the prompt. A bad URL, an unknown branch or
a refused credential stays a plain `Failed` and a red message: destroying the
folder is not the answer to a typo, and a forced pull must never be offered as
if it were a generic retry.

### Verification

`cargo test --release -p kovan --lib --tests` — 13 tests over the two touched
modules, all passing (2026-09-23):

- `a_conflict_is_told_apart_from_an_ordinary_pull_failure` — the six real
  messages captured above classify as conflicts; a missing repository, an
  unknown ref, a failed authentication, `Already up to date.` and an empty
  string do not.
- `an_uncommitted_edit_makes_pull_report_a_conflict` — real bare remote, real
  `git pull`; the `Conflict` carries Git's own reason, and nothing on disk moved.
- `declining_the_prompt_restores_the_folder_to_its_pre_pull_state` — after a
  merge left in progress, "no" returns `HEAD`, the file contents and
  `git status` to exactly their pre-pull values.
- `the_forced_pull_makes_the_folder_match_the_remote_exactly` — the unpushed
  commit, the uncommitted edit, the untracked file and the untracked directory
  are all gone; the log is the remote's, not a merge; a second forced pull on an
  already-matching folder is a no-op rather than an error.
- Three view-level tests, one of them driving `egui::Context::run_ui` headless
  (no window, no GPU): the prompt is raised with the right repository and no red
  message, an ordinary failure raises no prompt, and drawing the prompt across
  two frames without pressing either button neither pulls nor cancels.

**Not verified:** no interactive click-through of the dialog itself — the
buttons' handlers are covered only through `force_pull_in`/`abort_in_progress_in`
being tested directly. The maintainer should confirm the wording and the button
order on a real desktop before trusting the destructive branch.

## The kvim editor gets the system clipboard; the ingest dialogs get centred (2026-09-23, GH issues #280, #281)

**Maintainer, 2026-09-23,** ingesting the NJOY manual and wanting to record its
licence: *"i cannot ctrl-shift-v to paste inside the summary text box"*, and
*"when i ingest njoy manual, the popup boxes need to be in the centre"*.
Offered the choice of replacing kvim with a plain `egui::TextEdit`, the
maintainer kept kvim: *"just use kvim, but make sure my pasting and stuff
works"*. So the adapter was fixed, not swapped.

### It was never a missing key mapping

`egui-winit` recognises the clipboard chords **itself** and returns before
emitting any key event:

```rust
if is_cut_command(..)   { events.push(egui::Event::Cut);         return; }
if is_copy_command(..)  { events.push(egui::Event::Copy);        return; }
if is_paste_command(..) { events.push(egui::Event::Paste(text)); return; }
```

`app/kvim_editor.rs`'s `map_event` handled `Event::Text` and `Event::Key` only,
so all three fell through its `_ => None` arm and the *focused* editor left them
unconsumed — every kvim surface in the app, not just the summary box. Reading
that function first is what turned "add a Ctrl+V binding" into "handle the three
events egui already hands us"; the chord was arriving perfectly well.

Also read there, and worth keeping: `is_paste_command` is `modifiers.command &&
key == V` and never inspects shift, so **Ctrl+Shift+V was already producing a
`Event::Paste`**. Nothing platform-side was missing.

### What the three gestures now do

- **Paste** — inserts **at** the cursor, replacing a Visual selection if one is
  up. Deliberately not Vim's `p`, which puts *after* the cursor grapheme: the
  GUI gesture lands where the caret is. `p` itself is unchanged.
- **Copy** — the Visual selection, or, with nothing selected, the whole cursor
  line including its newline.
- **Cut** — copy, then delete: `d` over a selection, `dd` on a line.

The deletions go through kvim's own `d`/`dd` rather than a second
selection-deleting implementation here, so charwise, linewise and blockwise all
behave as the engine defines them, the edit is undoable, and the text lands in
the unnamed register as it always would. `selection_text` is the one place that
*does* have to distinguish the three granularities, because reading them is not
something `Editor` exposes — and conflating them is the "classic mistake"
`Editor::selection`'s own doc warns about.

### Centring (#281)

`setup.rs` already anchored `CENTER_CENTER` and `literature_list.rs`
`CENTER_TOP`; the six remaining dialogs — "Ingest this PDF?", "Ingest
Literature", "Sort <citekey>", "Add connection…", "Connections", "Delete
annotation" — now anchor `CENTER_CENTER` too. All are `collapsible(false)`
prompts rather than panels, so this makes the app consistent rather than
special-casing the ingest path.

### Verification

`cargo test --release -p kovan --lib --tests` green. Nine new tests on the
clipboard, all on the pure state (no window, no GPU): the multi-line licence
paste that prompted this, paste over a selection, paste from Insert mode, an
empty paste being a no-op, charwise copy including the grapheme under the cursor,
linewise copy taking whole lines with their newlines, copy/cut of the whole line
with nothing selected, cut returning what it removed, and `clipboard_action`
claiming the three events while leaving `Event::Text` and a genuine `<C-v>` key
alone.

**Not verified:** no interactive paste was performed — this environment has no
display, so the egui→arboard→X11/Wayland leg is covered by reading egui-winit's
source, not by running it. The maintainer should confirm Ctrl+Shift+V into the
summary box on the desktop.

## Click-to-insert: an unhurried click was a drag, and an opened block started in Normal (2026-09-23, GH issue #282)

**Maintainer, 2026-09-23:** "when i click the page context editor in kvim, i
expect to go into insert mode. It doesn't do that."

Two causes, both in the click-to-insert behaviour §27 promises, and one of them
not specific to kvim at all.

### egui calls an unhurried click a drag

`app/kvim_editor.rs`'s `text_area` senses `click_and_drag` and branches
`drag_started → dragged → drag_stopped → clicked`, with only the last arm
entering Insert mode. Read in `egui-0.36.1/src/input_state/mod.rs`, a press stops
being a click as soon as it passes **either** default threshold:

| `egui::Options` | default | what trips it |
|---|---|---|
| `max_click_dist` | 6.0 px | a trackpad wobble |
| `max_click_duration` | 0.8 s | resting on the button |

`is_decidedly_dragging()` then holds, `clicked()` never fires, and the press
lands in the `drag_started` arm — which enters **Visual** mode. So a deliberate
click left the editor in Visual, where typing runs Vim commands; a quick, still
click worked, which is exactly why the failure reads as intermittent rather than
total.

**Fix:** `end_drag` — a finished drag whose selection is empty (anchor == head)
was a click, so leave Visual and enter Insert. A drag that did select something
is untouched, so drag-to-select still works.

**The same threshold broke the read-only preview**, which is the other half of
the page-context panel: it opens a block on `clicked()`, so an unhurried click on
a banded block opened nothing at all. A press and release on the *same line* is
now a click there too; a drag across lines still opens nothing.

### An opened block started in Normal mode with no focus

`PdfReaderState::open_artifact` calls `block_editor.load_text`, and `load_text`
builds a fresh `Editor::new()` — Normal mode, no keyboard focus. The user has
already clicked (on a card, or a banded preview line) to get there, so
`begin_insert` now opens it ready to type and claims focus on the next frame,
per GH issue #35's own "a single click to bring me into insert mode, not double
click".

### Verification

`cargo test --release -p kovan --lib --tests` green (622 tests). Four new:

- `a_drag_that_selected_nothing_is_a_click_and_enters_insert_mode`, and its
  control `a_drag_that_selected_text_stays_in_visual_mode`.
- `begin_insert_opens_ready_to_type_and_claims_focus_once`, plus an assertion
  added to `open_artifact_on_a_text_block_loads_the_inline_editor` that the
  block editor lands in `INSERT`.
- `a_slow_press_on_the_preview_opens_the_same_line_a_quick_click_does` — driven
  headless through `egui::Context::run_ui` with synthetic pointer events and a
  1.5 s hold, asserted *relative* to a quick click at the same position so it
  does not depend on font metrics or panel layout. **Checked capable of
  failing:** backing the preview fix out turns it red, and it goes green again
  when restored.

**Not verified:** no interactive click on a real desktop — the mouse path is
exercised only through egui's own input state here.

**Left alone deliberately:** the page-context preview stays read-only. Editing a
schema-sensitive block as raw text is how the fenced TOML gets broken; clicking a
banded block to open it in the editor is the edit path, as the preview's caption
says.

## The mindmap's purple "up one level" card, and full paths in the finder (2026-09-23, GH issues #283, #284)

### The parent card (#283)

**Maintainer, 2026-09-23:** "i want a purple node with an up button that allows
user to go up one level, this will be connected to central node in dotted line
with an up button within a box on the dotted line. double clicking brings us to
that level."

Going up already existed as `runtime_graph::up_one_level` behind the breadcrumb's
`⬆ Up` button; this adds the representation in the star itself. The card shows
the parent concept's title, or *the top* from a top-level concept, and is drawn
only when `up_one_level` is `Some` — which is exactly when there is a centre
card.

**Purple is not a concept colour.** `color_for`'s palette means ownership (dark
green corpus, light green user, lilac project); this card is not a concept at
all, it is where you came from, so it sits off that palette — darker and more
saturated than the projects' lilac so the two do not read as the same family.

**The ring turns half a step when the card is shown.** `star_positions` puts
ring card 0 exactly straight up, which is where the dotted connector and the Up
button go, so the card would cover both. `star_layout_with_parent` rotates by
`pi / n`, putting the *gap* between two ring cards at the top for every `n`.

Two things the layout tests found, which guessing would have got wrong:

1. **The first version of the rotation test asserted "every radius is
   unchanged". It failed at `n = 10`** — `cards_collide` compares axis-aligned
   boxes and so is not rotation-invariant (a card is 170 x 46: two side by side
   need far more room than two stacked), so a turned ring can trip the growth
   loop where the straight one did not. The layout was right and the assumption
   was wrong; the doc on `star_layout_with_parent` now records it, and the test
   asserts what actually matters — nothing in the corridor, nothing overlapping.
2. **The Up button at the midpoint of the line was found underneath a fan card**
   at `n = 3`. It is now clamped past the furthest card's radius: every card has
   `|y| <= furthest`, so clearing that radius clears every card at every angle,
   which a midpoint cannot promise. With no ring at all it is still the midpoint.

### Full paths in the finder (#284)

**Maintainer, same day:** "i put njoy there and added 2016 and 2021, it shows up
in fuzzy finder as 2016 and 2021. without context, i cannot tell what it is."

`autocomplete.rs` labelled topic and project candidates with
`CollectionEntry::name` — the last path segment. The path was already in
`insert_text`, and `matches_query` already ranked on both, so only the *display*
threw the context away. Labels are now the full path, in `wiki_candidates` (the
kvim `[[` popup) and in `library_candidates` (the PDF reader's connection
picker). A built-in corpus topic labels with its path too, and its human title
moves into the detail text rather than being dropped.

`collection_picker::rank`'s consumers — the Wiki's sort dialog, the mindmap's
"Move…" — already printed `c.path` and are unchanged.

### Verification

`cargo test --release -p kovan --lib --tests`: 626 tests, all passing. New:
four in `mindmap_view` (the parent card clears every card including expanded
fans, the Up button box clears every card, the corridor is empty and nothing
overlaps after the rotation, `has_parent = false` is exactly `star_layout`), and
one in `autocomplete` pinning the full-path labels through both finders,
including that searching the leaf `2016` still finds `njoy/2016` and still shows
the whole path.

**Not verified:** the parent card's drawing and its double-click were not
exercised — the geometry is tested, the painting is not. Wanted on a real
desktop: that the purple reads against both themes, and that the Up button is
comfortably clickable at Fit zoom on a large star.

## Mind-map links: hyperlinks between concepts, linked artifacts, and no restart to see them (2026-09-23, GH issues #285, #286)

**Maintainer, 2026-09-23:** "in the add subtopic right click on the mindmap, i
also want to add hyperlink. For example, NJOY is a nuclear data processing code,
so it should like link to the NJOY entry within the code corpus. hyperlinks
should be light blue boxes with dark blue underlined text, just like hyperlinks
in markdown or wikipedia"; "hyperlink addition ui should be the same
fuzzyfinder"; "also i can't see artifacts i linked to the mindmap yet"; and,
correcting that last one, "i do see these artifacts load when i restart kovan,
but i don't want to have to restart kovan to see those hyperlinks. once i click
save annotations, i should be able to see them on the mindmap as well."

### Why a new store, and not `relation::add_connection`

`add_connection` refuses a collection outright — `SourceNotArtifact`, "a
collection cannot own a relation, having no file of its own" — and a
concept→concept hyperlink is precisely that excluded case. Asked, the maintainer
chose the new file over relaxing the old one, which is also their own 2026-09-22
decision getting its first user: `crates/kovan/src/connections.rs`, writing
`<root>/mindmap/connections.toml`, one entry per link, **written once with the
reverse derived** (`for_node` answers from either end).

**Two stores for now, deliberately.** Relations owned by an artifact stay in
`mindmap.md`; only the ownerless links go in the new file. Folding the first
into the second is the migration that decision implies and is *not* done here.

### ~~One card type for both (#285 and #286)~~ — CORRECTED the same day

~~To a reader a hyperlink and a linked annotation are the same thing — "this
points somewhere else" — so both draw as the same light-blue card with dark-blue
underlined text, on the ring beside the sub-concepts.~~

**CORRECTED 2026-09-23**, maintainer: *"differentiate between artifacts and
hyperlinks. artifacts should live in the right click when i right click a box,
hyperlinks can live as linked boxes"*. The two are not the same thing to the
person reading the map:

- **A hyperlink is a place**, so it is a light-blue card with dark-blue
  underlined text on the ring, double-clicked to travel, with "Remove
  hyperlink" on its own right-click menu.
- **A linked artifact is a detail of a concept**, so it is an entry on that
  concept's right-click menu — `LinkCache::linked_artifacts`, one line per
  relation, opening the paper it belongs to. Nothing is drawn for it.

The correction also keeps the star readable: a topic with a dozen annotations
connected to it would otherwise bury its sub-concepts under a dozen boxes,
while the menu holds any number without touching the layout. Only literature
ends count as artifacts; a concept on the far side of a relation is not offered
here.

**The mirror trap, and how it was nearly missed.** `relation`'s endpoints are
the older untyped `graph::NodeId` strings, so they are read into typed ids
before comparing — and canonicalised on **both** sides, for two different
reasons: canonicalising the relation's end is what puts the card on the
**corpus** node the map shows for a mirrored path, and canonicalising the
current node is what puts it there when the user is standing on the mirror
itself. The first version of the test asserted only one direction and passed
with the other half of the fix deleted; both directions are now asserted and
each half was checked capable of failing by removing it. This is the same trap
`runtime_graph::canonical_concept` was written for after the Up button landed on
the light-green mirror of a corpus topic.

### Seeing it without a restart (#286)

The shared `WorkspaceKnowledge` was rebuilt only on opening a root, on setup and
after an ingest or a reclassify. Saving an annotation or adding a connection
wrote to disk and changed nothing on screen until the next launch —
`RepoSaveFingerprint` already noticed such a save once per frame (it exists to
re-run `git status`), so it now also calls `refresh_knowledge`, and it watches
`mindmap.md` and `mindmap/connections.toml` as well, because a connection is
written to those rather than to the active paper. Watching files rather than
threading a signal out of every save site keeps every writing path covered from
one place, and catches an external edit too.

### Verification

`cargo test --release -p kovan --lib --tests`: 638 tests, all passing. New:
seven in `connections` (round trip, both ends, the readable TOML shape, no
duplicate in either direction, no self-link, removal from either direction, a
malformed row skipped, an unreadable file being "none" rather than a failure),
four in `mindmap` (a hyperlink card from both ends, a relation offered on the
menu **and not drawn as a card**, the mirror case in both directions, both
candidate id syntaxes), and one in `app` pinning that the save fingerprint
notices either mind-map file.

**Not verified:** none of the drawing or the dialogs were exercised — no display
here. Wanted on a real desktop: that the light blue and dark blue read well in
both themes (the fill is a fixed colour, not a themed one), that the underline
sits right at Fit zoom, and that "Add hyperlink…" lands where the maintainer
expects beside "Add subtopic…".

## The table digitiser reads on arrival, and finds its own model (2026-09-23, GH issue #287)

**Maintainer, 2026-09-23:** "the table digitiser, i don't want to deal with
selecting an OCR model, i should be able to just see the table and csv
extracted"; "when i click read table, the OCR should already be run"; "and csv
extracted"; "then i can edit the csv table, and save the artifact".

`table_ocr`'s own module doc had predicted this exactly — model download was left
out as "a natural follow-up **if a model-path text field turns out to be too much
friction in practice**". It did, and that paragraph is now struck through in
place rather than quietly rewritten.

### What changed

- **`discover_models`** searches, in order: `$KOVAN_TESSDATA` (which may name a
  file, so it can point at one model), `$TESSDATA_PREFIX` and `<it>/tessdata`,
  Kovan's own application-data folder, then the usual system tessdata
  directories. Within a directory `eng.traineddata` is preferred; **across**
  directories order wins, so the override stays an override rather than being
  beaten by a system English model.
- **`osd.traineddata` is never chosen.** It sits beside the real models and is a
  `.traineddata`, but it carries no LSTM recognizer, so picking it converts "no
  model installed" into an obscure load error.
- **`load_crop` recognises immediately** — no button, no path field. The tab
  shows which model read it, offers "Read again", and shows the **CSV** beside
  the editable grid, read-only, because the cells are the editable copy and two
  editable views of one table would have to answer which of them wins.

### A model being installed does not mean it can be used

Measured on the maintainer's machine while building this: `tesseract 5.5.3-1` is
installed, `/usr/share/tessdata/` holds only `afr.traineddata` and
`osd.traineddata`, and **`find / -name eng.traineddata` is empty**. Worse, the
one usable-looking model is rejected outright by the recognizer:

```
/usr/share/tessdata/afr.traineddata: format error:
network outputs 96 != recoder code_range + 1 = 97 (CTC-null invariant)
```

So the first design — "discover a model, use it" — would have reported an
obscure format error where the real answer is "install the English model". The
digitiser therefore **walks every candidate** and uses the first that loads,
collecting the rejections, and when none works it says so with the files it
tried and the package to install (`tesseract-data-eng`, verified to be the right
Arch package name). Falling back to another language is still worth doing where
it loads: these are Latin-script LSTM models and a table is mostly digits, and
the status line names the model so odd words can be told from a bad crop.

~~**Unverified, and filed as GH issue #288:** whether `eng.traineddata` loads at
all.~~ **RESOLVED the same day** — the maintainer installed
`tesseract-data-eng 2:4.1.0-5` (23 MB on disk, 9 MB download) and it fails
identically:

| model | `num_outputs` | `code_range` | the check demands |
|---|---|---|---|
| `eng.traineddata` | 111 | 111 | 112 |
| `afr.traineddata` | 96 | 96 | 97 |

Both satisfy `num_outputs == code_range`; `kopitiam-ocr` 0.1.0 computes
`expected = code_range + 1`. Two unrelated languages, each exactly one short in
the same direction, so **no `tesseract-data` 4.1.0 model loads at all** — a
defect in the engine, not a missing or broken model. `kopitiam-ocr` is a
separate repository, so the fix cannot land here; #288 records the one line to
compare against Tesseract's own `LoadCharsets`.

The failure message was corrected once that was known: with models present but
unloadable it no longer says "install `tesseract-data-eng`", which would send
the user after a file they already have.

### Verification

`cargo test --release -p kovan --lib --tests`: 642 tests, all passing. Four new:
English preferred and `osd` never chosen, directory order beating language with
a bare file accepted as a model, the documented search order actually containing
the system location, and — machine-independently — that a loaded crop is read at
once, never leaving the old "set the model path, then Run OCR" state, with any
failure naming both what it tried and how to fix it.

**Not verified:** no successful OCR pass has been run anywhere in this work,
because no model on this machine loads (see above). The recognition path itself
is unchanged from the button-driven version that came before, but "Read table
now lands on a filled-in table" has not been *seen* — only that it is attempted
and that the failure is honest. That waits on #288.

## kvim owns the keyboard; the graph digitiser is drawn, not auto-traced (2026-09-23, GH issues #289, #290)

### The editor never left Insert mode (#289)

**Maintainer, 2026-09-23:** "When in the text edit mode, i want kvim to act like
vim keys, until the thing is saved. means ctrl+shift+v to paste, u to undo etc."

Neither the engine nor the key mapping was at fault. Driven directly,
`kopitiam-neovim` does the right thing — `i`, `X`, `Esc`, `u` restores the
buffer — and `map_event` maps Escape. The loss is in **egui**:
`egui-0.36.1/src/memory/mod.rs:570` reads the focused widget's `EventFilter`
and, for keys the filter does not claim, **Escape sets `focused_widget = None`**
while Tab and the arrows move focus by direction. `EventFilter::default()`
claims none of them, and the kvim text area is a plain `ui.interact`.

So Escape **never reached the engine**: the editor stayed in Insert for ever,
`u` typed a `u`, and — once focus was gone — the keys after it went to the
*app*, where `j`/`k`/`n` are the PDF reader's page-turn shortcuts. That is a
much worse failure than the report suggested, and it was invisible from the
Rust side of the adapter, which looked correct.

`Memory::set_focus_lock_filter` with all four claimed, while the editor has
focus. It applies from the second frame of focus (`had_focus_last_frame`),
egui's own constraint and the one its `TextEdit` lives with.

### Auto-trace replaced by a drawn stroke (#290)

**Maintainer, same day:** "for graph digitiser, we won't do auto-trace anymore.
It will be manual, were i draw a line and it will be snapped to the curve after
i let go of the mouse. the points will be placed 2 pixels apart."

`trace::snap_stroke` resamples the drawn polyline by **arc length** (so the
points are 2 px apart *along the stroke*, which on a steep segment is finer in x
than any column scan), snaps each sample vertically onto the nearest ink run
within a radius, and reads the run's centroid and thickness exactly as
`trace_curve` does — so the per-point uncertainty `dataset` derives from line
thickness keeps working unchanged.

Two things the tests caught, both real:

1. **A clipped run gives a wrong centroid and a wrong thickness.** The search
   window truncates the ink wherever the drawn point sat near its edge; the
   first version read the truncated run and put a point at y = 49 on a band
   centred at 50. The run is now grown back to the ink's real extent before
   anything is read off it — which matters twice over, because thickness feeds
   uncertainty.
2. **"Drew off the curve" and "drew near a gridline" are different things.** A
   test asserting that straying off the band produced nothing failed because the
   fixture had a gridline at row 20 and the snap correctly found it. The
   behaviour is right and is now pinned deliberately by
   `a_stroke_drawn_along_a_gridline_snaps_to_the_gridline`: the snap takes the
   ink under the stroke, gridline included. A `max_thickness_px` cap keeps an
   axis or a bar from being read as a curve.

**The GUI's auto-trace is gone** — button, strategy picker, column-step slider,
`DigitiseApp::auto_trace` and the `step`/`strategy` fields, rather than left
sitting unreachable. `trace_curve` and `auto.rs` stay for `kovan-cli digitise`,
a different surface that was not part of the decision.

**Open, not done:** `PointOrigin` has three variants and a snapped stroke is
honestly none of them — a human drew it, a machine placed it on the ink. Points
are recorded `HandPlaced`, since the gesture is the human's; a fourth variant
would be the fuller record and is a serialized-schema change.

### Verification

`cargo test --release -p kovan --lib --tests`: 651 tests, all passing. New:
two headless egui tests for #289 (Escape-then-`u` undoes; the arrows move the
cursor rather than focus), each **checked capable of failing** by removing the
focus lock — without it the editor still reports `INSERT` after Escape — and
seven for #290 covering the snap, the 2 px spacing (and that it is a setting),
dropped samples, deduplication, the gridline behaviour and the thickness cap.

**Not verified:** no drawn stroke has been made with a real mouse, and no
rendering has been seen — no display here. The gesture handling
(`dragged`/`drag_stopped` into `snap_drawn_stroke`) is covered only by the
library function underneath it.
