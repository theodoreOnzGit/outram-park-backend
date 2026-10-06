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

## Tables are digitised by hand in a Calc-style grid; OCR removed (2026-09-28, GH #353–#357)

**Maintainer, 2026-09-28:** "i want to do table digitisation without OCR...
the tesseract thing can be quite annoying"; "i want libreoffice like
interface... libreoffice calc cells on the left panel, pdf viewer of table on
right hand side, and i slowly use the select text to copy/paste into the
libreoffice calc cells... then once done, i want to save the cells to csv".

**Why OCR went.** `kopitiam-ocr` 0.1.0 rejected every installed tesseract
model (#288), so the entry above ("reads on arrival, and finds its own model")
never produced a single cell. Born-digital PDFs already carry exact text, so
recognition was the wrong tool for them in the first place.

**What replaced it.**

- `digitiser::table_grid` — the grid model, no `egui`: cursor and Shift
  selection, LibreOffice Calc's tab-separated paste and copy, grow-on-edge,
  insert/delete row and column, bounded undo, CSV through the `csv` crate
  (RFC 4180 quoting, no `#` lines — provenance lives in `[extraction]`).
- The reader's `SelectGranularity::Char` (`select_chars_in_rect`): in the
  table view a drag selects characters, rebuilds rows from glyph baselines,
  turns gaps wider than 1 em into tabs, and copies at once. So dragging across
  a table row and pressing Ctrl+V fills a row of cells.
- The table view is the grid on the left and the same reader on the right.
  Saving writes a `digitised_table` artifact with
  `[extraction] method = "pdf_native"`, `digitised_by` and `digitised_at`,
  with the body in the digitised graph's series schema
  (`### start of data series` / `### Series: <table name>` / fenced CSV /
  `### end of series`). ~~Export writes a plain `.csv`.~~ **CHANGED same
  day**: there is no CSV export ("i don't want to export to csv, i want to
  save artifact"); **Save artifact** sits at the top of the grid panel, and
  Ctrl+S does the same.
- Selections are highlighted as the theme's text-selection colour over
  exactly the text that is copied: one block per selected line, or per cell
  (run of text between column gaps) for a character selection, updated live
  while dragging.
- **No "mark reviewed" step** (maintainer: "the workflow is fully human").
  Every value is typed or pasted by a person from the PDF, so there is no
  machine output to gate; `digitised_by` records "every value entered by
  hand" instead. Re-opening a saved table loads
  its CSV back into the grid instead of re-cropping.

**LibreOffice is not vendored** — it is C++ and would break the pure-Rust and
Android build. Only its conventions are copied (keys, paste, CSV quoting).

**Removed:** `digitiser/table_ocr.rs`, `DigitiserError::Ocr`, the
`kopitiam-ocr` dependency, the JSON table export. Scanned pages, which have no
text layer, are typed into the grid while reading the page.

## The kvim tab is source + live GFM preview, read-only until Edit is confirmed (2026-09-28)

**Ask (maintainer, 2026-09-28).** "make kvim editor tab a kvim editor +
markdown renderer. kvim editor is read-only by default. User has to click
edit, where a popup box warns you could break the schema ... markdown is gh
flavoured", layout "similar to vscode", and "you can vendor and translate
this code: https://github.com/jbt/markdown-editor.git".

**What was built.**

- `app::kvim_tab` — the tab: kvim source on the left, preview on the right in
  a resizable split, with `Source | Preview`, `Source` and `Preview` layouts.
  `EditLock` (`Locked -> ConfirmPending -> Unlocked`, `lock()` from anywhere)
  gates the editor; while locked the buffer is drawn by
  `KvimEditorState::ui_locked`, the read-only path that forwards no key,
  clipboard or mouse edit. **Edit** opens an `egui::Modal` that lists, from
  `artifact.rs`'s schema, what a hand edit can break (the `[kovan]` TOML
  fence under a `#` heading, unique ids, `connections`/`[[relation]]`
  two-way references, `csv` payload fences, the paper header); only
  "I understand the risks — edit" unlocks. **Done editing** re-locks.
- **The confirmation is per document open**: activating a paper or opening
  an external file calls `KvimTab::document_opened`, which re-locks. Saving,
  and the background resync from other flows, do not.
- `app::gfm_preview` — `pulldown-cmark` 0.12 (already a dependency) with
  tables, strikethrough, task lists, footnotes and GitHub alerts, into a block
  model that keeps each block's source lines; GFM bare-URL autolinks are added
  on plain text because `pulldown-cmark` does not do them. The source lines
  drive the VS Code-style scroll sync: the editor's top visible line picks the
  preview offset.
- Save is untouched: the same Save button, locked or not.

**Why not `egui_commonmark`.** Checked first: 0.25.0 targets egui 0.36 and is
MIT OR Apache-2.0, with the GFM extensions. But it cannot report where each
source line was painted, so scroll sync could only be proportional, and it
brings `pulldown-cmark` 0.13 beside the workspace's 0.12. The maintainer then
directed a translation of jbt/markdown-editor.

**What was translated from jbt/markdown-editor** (ISC, commit `58aa8bf`,
full record in `NOTICE`): `setOutput`'s scroll-to-first-changed-element (kept,
but only when that block is off screen), `update`'s title-from-first-h1,
`render_tasklist`'s disabled checkboxes, and the `#in`/`#out` split plus
`toggleReadMode`. Its parser (`markdown-it`, MIT) was **not** ported —
`pulldown-cmark` replaces it.

**Not done.** Preview-to-editor scroll sync (only editor -> preview), table
column alignment, images (shown as links — no image loaders are installed).

### Same day, after the maintainer tried it

- **Save artifact closes the table view** and returns to the PDF reader
  ("save artifact for table should close the table digitiser and return to
  pdf viewer"). A failed save stays put, with the error shown.
- **Selected cells contrast with their neighbours**: the theme's selection
  colour at high opacity, the block outlined as one shape, the active cell
  in a heavy border of the strongest text colour, and the selected columns'
  letters and rows' numbers lit up, as in Calc. Dragging across cells selects
  a block.
- **Pages can be turned 90° and saved** ("there should be a way to rotate and
  save individual pages of pdf in case they are in the 90 degree
  orientation"). `crate::page_rotation` sets the page's `/Rotate` (honouring
  an inherited value) through `kopitiam_pdf`'s incremental update, so the
  original bytes survive as a prefix of the saved file. The reader toolbar
  has ⟲/⟳ 90° and, once turned, **Save rotation**; the turn shows at once and
  is written only on Save. Because `/Rotate` is applied in the page
  transform for both rendering and structured text, selection still finds
  the right glyphs on a turned page. That was tested rather than assumed
  (`the_renderer_and_the_text_layer_both_follow_the_turn`). The table view
  gets this for free, since its right half is the same reader. Plain images
  turn for viewing only. Boxes already saved on a turned page were drawn in
  the old orientation and will not line up until it is turned back.
- **A setup box first, like the graph wizard** ("the first popup box asks you
  what table is this. Then it brings you straight to the digitiser"). A new
  "Read table" region opens a modal, "Which table is this?", showing the page
  and paper, with a required "Table, as printed" field. Enter or Start goes
  to the grid; Cancel, Esc or clicking outside returns to the PDF. The graph
  wizard's axis-range and label stages have no table counterpart, so it is
  one question. A re-opened saved table skips it: it already has a name.
- **Cancel digitisation** ("table digitiser should also have a cancel
  digitisation option, which brings us back to pdf reader"). A plain button
  left of the Save artifact button in the grid's header returns to the PDF
  reader without saving. If the grid holds cells that differ from what was
  last saved or loaded (an edit in progress counts), a "Discard this table?"
  box asks first, with "Discard and go back" / "Keep editing"; Esc or
  clicking outside keeps editing. An empty or unchanged grid leaves at once.
  Leaving resets the grid, name, region and status, so re-entering the tab
  does not show a stale half-done table. The decision is
  `request_cancel`/`answer_discard`, unit-tested without a window.
- **Format to standard form (E)** in the grid toolbar (after Undo/Redo):
  "User selects cells, and clicks a format to standard form button, which
  then puts in e notation for highlighted cells". It calls
  `TableGrid::reformat_standard_form` on the selection: whole-cell standard
  form (`2.1×10^6`, `8.2×107`) becomes E notation (`2.1e6`) in one undo
  step, and the status line reports how many cells changed. Cells with
  units (`1.0X10^5 m^2`) are left alone. It asks first ("The wizard then
  asks in a popup box, are you sure? then displays the superscripted text
  before, and e form text after"): a "Reformat N cell(s) to E notation?"
  box lists each cell (B3, ...) with its text before, superscripts drawn
  raised, and its E form after; Reformat applies, Cancel/Esc/click outside
  changes nothing. With nothing convertible highlighted there is no box,
  only a status line. The grid's own cells now draw `^` superscripts raised
  too; the stored text keeps the `^`.
- **Resizable column widths and row heights** ("pls allow me to change
  widths and heights of the cells too"), Calc's way: drag the right edge of
  a column letter or the bottom edge of a row number (resize cursor over a
  ±4 px grab zone); double-click a column edge for Calc's optimal width
  (widest cell text as drawn, superscripts included, plus padding) and a
  row edge to fit its tallest cell. Widths clamp to 24–1200 px, heights to
  one text line–400 px. Sizes are GUI state in `TableDigitiserState`
  (`col_widths`/`row_heights`), never saved into the CSV or artifact; the
  toolbar's +/− row/col insert/delete the matching size so a resized line
  moves with its cells, paste growth pads with defaults, and a new region
  (`load_crop`) or `reset_table` returns to defaults. Tall rows centre their
  text vertically; wide text still clips; the cell editor takes the cell's
  size. The arithmetic (`clamp_size`, `fit_size`, `fit_len`, `sync_sizes`,
  `edge_hit`) is egui-free and unit-tested.
- **2026-09-28 — The view follows the keyboard cursor everywhere in kovan.**
  Maintainer: "for anything in kovan, when the cursor moves beyond the scroll
  area, like for the kvim text editor, or the csv, the scrollbar shld
  follow". Rule: scroll only on a frame where the cursor/selection **changed**
  (so the mouse wheel is never fought), by the minimum distance
  (`scroll_to_rect(rect, None)`, no recentring). The table digitiser grid now
  does this for the active cell (`follow_moved` over `(cursor, selection)`,
  so a paste that selects a block also follows); pinned by a headless test
  that arrows 60 rows down a 291-pt viewport and checks the whole cell is in
  view at the bottom edge, with a no-move control that does not scroll. kvim
  already followed its caret (2026-09-24, `last_cursor`); it gained an
  end-to-end `G`/`gg` test on the real `ScrollArea` offset. Checked and left:
  the Ctrl+P literature finder (already `scroll_to_me` on Up/Down); the
  literature list, wiki, bibliography, mindmap, plot setup, CSV preview, git
  view and GFM preview (no keyboard cursor; the preview syncs to kvim's top
  line); the TUI (ratatui `List` + `ListState` scrolls to the selection
  itself; its `Paragraph` panes have no cursor). `pdf_reader.rs` not audited
  (another session's).
- **2026-09-28 — Save Repository takes an optional user commit note; the
  generated subject never changes.** Maintainer: "is there a way i can put in
  a commit message into kovan, so that it appends to the save kovan
  repository?" The Save Repository tab now has a multiline "what did you do?
  (optional)" box. Its text becomes the **first body paragraph** of every
  commit the save makes — the Kovan repository's, the private submodule's
  (both `gix`) and the open corpus's (system `git`, single `-m` argument,
  `--cleanup=verbatim`, no shell) — under the **unchanged** subject
  (`Save Kovan repository` / `Save Kovan repository: open corpus`), with the
  generated Added/Edited/Removed list after it. Subject kept rather than
  `Save Kovan repository: <first line>` so history stays uniformly greppable,
  the open-corpus subject cannot collide, and no note can make an over-long
  or multi-line subject; the cost is that the one-line History list does not
  show the note. Trailing whitespace per line and surrounding blank lines
  are trimmed; `#` lines and interior blank lines are kept (with `-m`, Git
  keeps `#` lines anyway; verbatim also stops it collapsing blank lines, so
  both code paths store the same body). A blank note is byte-for-byte the
  old message (tested). The box is cleared only after a save that
  committed; kept on failure and on nothing-to-save. Logic:
  `repository::compose_commit_message`, `save_repository_with_message`,
  `advanced_git::save_with_message`. Neither `kovan-cli` (code tooling, no
  save command) nor the TUI has a save/commit command, so no `-m` flag was
  added.
- **2026-09-28 — Save Repository pushes by default: the proprietary and open
  corpora to their own remotes, then the Kovan repository.** Maintainer:
  "kovan should be able to push pdfs to the proprietary repos by default",
  then "and open source". ~~Saving never pushes~~ (never written down as a
  decision; it was simply how `save_repository` behaved since `op-9vo6.19`)
  **CHANGED 2026-09-28**: after any successful save (including one with
  nothing new, so earlier unpushed saves go up), `save_push::push_after_save`
  pushes, in order, the proprietary corpus, the open corpus, then the Kovan
  repository — the parent only if neither corpus push failed or was refused,
  so it never publishes a gitlink to a corpus commit its remote lacks (a
  corpus merely *skipped*, e.g. not downloaded or with no remote configured,
  does not block it). The standard corpus is never pushed. **Default ON;
  opt-out** is the "Push after save" checkbox under the note box, persisted
  as `[save] push_after_save = false` in the library's `kovan_root.toml`
  (per library, so it travels with it; the table is omitted while at the
  default; written by `KovanRoot::set_push_after_save`, which re-reads the
  file first so a stale in-memory root cannot revert other settings).
  **Safety rules**, each pinned by a test in `src/save_push/tests.rs`
  (temp repos, local bare remotes): never forced (refspec
  `refs/heads/B:refs/heads/B`, no `+`, no `--force`), so a remote that moved
  on fails as "pull first" with the local commit kept; never from a detached
  HEAD — the commit is put on the tracked branch (`.gitmodules` `branch =`,
  else `refs/remotes/origin/HEAD`, else `ls-remote --symref`) only if that
  branch's local and remote-tracking tips are ancestors (a fast-forward),
  otherwise refused with nothing moved; every push URL (`remote get-url
  --push --all`, so a separate `pushurl` is checked too) of the proprietary
  corpus must be the private remote (`[private_submodule] remote` and
  `[corpora] proprietary_remote`, which must agree) and must not be the open
  or standard-corpus remote, and the open corpus's must be `open_remote` and
  must not be a proprietary one — URL spellings (`https://`, `ssh://`,
  `git@host:`) are normalised before comparing; nothing is ever pushed to an
  `outram-park-backend` URL. Network via system `git` with
  `GIT_TERMINAL_PROMPT=0` (credential helpers still run; a missing
  credential fails fast with Git's own words). Each repository's result
  (pushed / nothing to push / not pushed and why / refused / failed) is
  listed under the Save button, red if any failed or was refused; the note
  box is still cleared only by a save that committed, whatever the push did.
  API: `advanced_git::save_and_push(root, note, push)`,
  `advanced_git::push_after_save_setting`, `save_push::{push_after_save,
  PushReport, PushOutcome}`.
- **2026-09-28 — A Save now writes the index; it had been leaving staged
  deletions behind.** Found diagnosing the maintainer's real proprietary
  submodule, which showed four saved PDFs as `D ` (staged deletion) plus
  `??` and `yuanzhong2002fission.pdf` as `MM`. Cause: Save commits with
  `gix` from a tree built off the worktree and never wrote `.git/index`, so
  the index stayed at whatever it last was (there: exactly commit `ddcd155`,
  2026-09-24) while five further saves moved `HEAD`; one plain `git commit`
  would have deleted the PDFs. Reproduced in a temp repo (the regression
  test showed `MM papers/c.pdf`, `D  papers/d.pdf`, `?? papers/d.pdf`) and
  fixed in `repository::sync_index`: after each commit the index is rebuilt
  from the committed tree, keeping stat data for unchanged entries. Pinned by
  `after_a_save_git_status_is_clean_in_every_repository_it_committed`. The
  same submodule's **detached HEAD** was not made by Save itself: it came
  from `git submodule update --init` in setup on 2026-09-22 15:58, ten
  minutes before `corpus_repos::attach_to_branch` existed (16:08), and Save
  then committed onto it; the push above now puts such commits on their
  branch.
- **2026-09-28 — "Edit digitisation" prefills the wizard and restores the
  saved curves; a successful graph save returns to the PDF reader.**
  Maintainer: "next time we have an edit digitisation, please pre-fill the
  values in the wizard with existing values." Re-opening a saved
  `digitised_graph` used to show the three-stage wizard empty and the
  digitiser with no points. Now (`src/app/saved_digitisation.rs`, the exact
  inverse of `DigitisedDataset::extraction` + `save_into_project`): the
  figure (artifact heading), page, document title, both axis ranges and log
  flags (parsed back from `[extraction].x_axis`/`y_axis`), and both labels
  (extraction, else the CSV header unless it is the blank-label `x`/`y`
  fallback) are prefilled; any field that does not parse is left blank and
  named in a note shown above every wizard stage. On "Start digitising" the
  curves come back from the CSV (`### Series:` blocks or the single fence):
  earlier ones banked by name, the last one live. **Rectangle records carry
  their reference pixels** (`px 107.2 = …`), so the reference lines go back
  on them and each point to `saved_cal.pixel_at(x, y)`; values are kept
  exactly (an untouched re-save writes a byte-identical body, pinned by a
  test), and if a range was corrected in the wizard each value is re-read
  from its saved pixel through the new calibration. **Limitations, stated in
  the wizard note:** notes, rotation and deskew are not in the artifact, so
  a figure that was turned must be turned again (points landing outside the
  crop raise an error saying so); a **parallelogram record has no corners**,
  so its points are held until the operator drags the corners back and
  presses "Restore saved points" — pixels are never invented; per-point
  origin/uncertainty are not in the CSV, so restored points are hand-placed
  by the saved `digitised_by` with ±0.5 px uncertainty re-derived. Re-saving
  still replaces the same artifact (`replace_id`). Tables already behaved
  this way (Edit table skips the setup box and reloads the CSV into the grid,
  `table_digitiser.rs::load_crop`/`resolve_reload`) — confirmed, unchanged.
  Found by the round-trip test and fixed: saving without banking the last
  curve dropped the name typed in the series box (it was written as
  `series-N`). Also (maintainer, same day: "save csv into project markdown
  should also move us into the pdf reader"): `save_into_project` now returns
  whether it wrote, and the button's `save_into_project_then_read` switches
  to the PDF reader on success only — failures stay in the digitiser with
  the error, as the table digitiser's Save does.
- **2026-09-28 — A re-digitise re-save rewrites `[extraction]`, not just the
  body.** Maintainer, in real use: PANAMA Figs. 6 and 7, re-digitised via Edit
  digitisation with the y top corrected to 10^0, still read `px 88.03 = 10` /
  `px 35.40 = 10` and `digitised_at = 2026-09-24`. That happened because both replace
  branches of `classify::save_digitised_csv` (`replace_id`, and the
  same-heading overwrite) went through `replace_artifact_body`, which clones
  the old TOML and only bumps `modified`, so the new `Extraction` the caller
  passed was thrown away. Now both go through the new
  `classify::replace_digitisation(session, id, body, Option<Extraction>)`:
  a supplied extraction replaces `[extraction]` wholesale with
  `digitised_at` = the re-save time. `[kovan]` id/kind/created, `[source]`,
  classification, relation and connections are kept and `modified` is
  bumped. **`[kovan].reviewed` is cleared when the body changed** (the
  review vouched for numbers that no longer exist) and kept when it did not.
  This applies to graphs and tables alike: the table save already passed an
  extraction, and `table_digitiser.rs` is unchanged. `replace_artifact_body`
  still keeps metadata verbatim for the inline prose editor. `[source]` is
  still not updated by the same-heading overwrite of a fresh crop
  (unchanged, out of scope). **Existing artifacts are not repaired
  retroactively, and a plain re-save will NOT repair Figs. 6/7.** Edit
  digitisation prefills from the saved (stale) `y_axis` and places the
  restored points through it. Keeping the stale range just writes it again.
  Correcting it in the wizard re-reads every value from pixels worked out
  through the stale calibration, which would corrupt the correct data. The
  calibration those CSVs were actually made with is recorded nowhere. Repair:
  first hand-edit each artifact's `y_axis` in the paper's Markdown to the
  calibration actually used (e.g. `px 88.03 = 1` if only the value was
  corrected and the line was not moved). Then Edit digitisation → Start
  digitising (ranges untouched) → Save CSV into project markdown, which
  keeps the values exactly and stamps a fresh `digitised_at`. If the
  reference line was also moved, the true pixel is unknown and the figure
  should be re-digitised from scratch.
- **2026-09-28 — Select a banked series to edit it (graph digitiser).**
  Maintainer: "I want to be able to select data in previous banks, like
  through a dropdown menu, or as clickable buttons." Before this a banked
  curve could only be dropped (last one), never re-opened. Now the series
  panel shows one button per series in saved order, `name (points)`, with
  the live one highlighted (a ComboBox once there are more than 6). Clicking
  a banked series **swaps** it with the live one (`src/app/series_select.rs`,
  `select_banked_series`): the live curve is banked under the name-box name
  if it has points (refused if it is unnamed or its name is taken, as "Bank &
  start next" refuses), an empty live curve is discarded rather than banked
  as an empty series, and the chosen curve becomes live with its name in the
  box, so every existing edit path (add, drag, erase, right-drag erase)
  applies to it unchanged. **Order:** a new `live_position` keeps the live
  curve's place in the saved order, so `all_series()` and the saved artifact
  never reorder when the operator hops, and banking a curve picked from the
  middle puts it back there. **Names:** the name box now wins over the live
  dataset's own `series` field on save (`named_series_csv`), so renaming a
  restored or re-selected curve is what gets saved (previously a restored
  live curve kept its old name on save despite a rename). Banked series are
  drawn faintly on the figure under the live one; hovering a series button
  draws that curve bold. No per-series delete was added. Tests in
  `series_select.rs`, including a save round trip through a paper session.
- **2026-09-28 — Edit digitisation: the saved values are the data; a range
  change that would recompute them needs explicit confirmation.** Maintainer
  report ("kovan edits to current digitisation may be buggy"): PANAMA-I
  Fig. 7, header hand-edited `px 35.40 = 10` → `= 1`, then Edit digitisation
  → Start digitising → Save, and every value changed. **Investigated, not a
  re-read by the edit path.** Hypotheses and results: (a1) a hand-edited
  header makes an *untouched* wizard re-read values — refuted: the prefill and
  the restore parse the same header, so `saved_calibration == calibration()`
  and values are kept byte-for-byte (pinned:
  `an_untouched_edit_after_a_hand_edited_header_keeps_every_value`, which
  passed before any change); (a2) a changed wizard range re-read the restored
  points — refuted for Fig. 7, because that keeps every x value (the x axis
  was unchanged) and **no** 488f28f x value equals a 59f1b92 x value; (b) the
  edit path drops series/points — refuted: nothing filters by name, count or
  position (pinned: `four_series_and_a_lone_early_point_survive_...`); the
  "Level of Heavy Metal Contamination" series and the 99.8 h point were
  already absent from 59f1b92, a full re-trace made before restore existed.
  **What the data show:** the 488f28f curves are new placements, on the same
  pixel grid as the original 162939f trace, read through the hand-edited
  header: against 162939f compressed by 6/7 in log about 1e-6, the mean
  residual is 0.000 decade (max 0.02–0.07 over 7–83 points per series).
  Against 59f1b92, as saved or compressed, it is 0.4–0.9 decade. The
  restored 59f1b92 markers would have been drawn where the header puts them,
  which is off the curves, because 59f1b92 was measured through a different
  calibration (`px 365.69 = 1e-6, px 34.05 = 1`, stale-header bug). The
  "hand-edit the header, then re-save" repair in the bullet above assumes the
  lines were not moved, and they were, so it did not apply to Fig. 7.
  **Changed:** (1) the prefilled wizard records the saved ranges
  (`PlotSetup::saved_ranges`). A range or log flag that differs *numerically*
  (`1e-3` for `0.001` is no change) shows a red warning naming how many saved
  points will be recomputed. The warning offers "Revert to saved ranges" and
  blocks Next / Start digitising until "recompute the saved points (N)" is
  ticked. `finish_plot_setup` refuses too. (2) A failed restore no longer
  throws the saved points away (`finish_plot_setup` took `pending_restore`
  and ignored the `false`); they stay pending behind "Restore saved points".
  (3) The prefill note says values are kept while ranges are untouched and
  that markers off the curves mean the header does not describe the values.
  Tests: `src/app/edit_digitisation_tests.rs` (5).
- **2026-09-30 — Standard-corpus documents are ingested literature;
  duplicate PDFs are refused.** Maintainer: "kovan doesn't recognise
  literature in the standard corpus as ingested. it should". Kovan listed
  WASH-1400 as "not ingested yet", so it was ingested again as
  `papers/2008/2008muffletwond`, byte-identical (SHA-256
  `029dfd5bffa8a430...`) to `kovan-standard-open-corpus/nrc/ML15334A199.pdf`.
  **Root cause:** "ingested" meant only "some paper's `kovan.toml` records
  this PDF" (`paper_owning_pdf`, the literature list's `owners` map).
  Nothing joined the compiled metadata (`corpus::LITERATURE`) to the pulled
  corpus files, so no standard-corpus document was ever ingested; the reader
  offered to ingest each one, and the ingest duplicate check covered only a
  citekey collision (its doc called the content check "future work").
  **Decisions.** (1) An entry is **ingested** when its `corpus_file` is
  present in any checkout of the corpus repository
  (`standard_corpus::StandardCorpus`: the folder's
  `literature/standard-corpus/`, the folder's open corpus when it is the same
  repository, the shared application-data clone; a future second repository
  is one more checkout). Absent, it is **known, not downloaded**, shown with
  its source URL, never hidden. The literature list's standard group is built
  from `LITERATURE` and matches on id, title, authors and topics. (2) **A
  corpus document's notes live in an ordinary paper keyed by the corpus
  id**, filed the first time it is opened in a Kovan folder
  (`standard_corpus::ensure_paper`): `papers/<year>/<corpus-id>/kovan.toml`
  with `[source] corpus = "<corpus-id>"`, `access = "open"` and the corpus
  topics; `<corpus-id>.md` for annotations and digitisations; a
  `bibliography.bib` entry generated from the compiled metadata (an existing
  entry with that key is kept). The PDF is never copied; `[source].pdf` is
  recorded only when the corpus file is inside the folder, since `corpus`
  finds it anywhere. Rejected: a separate note format (every paper-aware view
  works on a `PaperSession`), and filing papers for every entry when a folder
  opens (files the user never asked for). A paper that already records the
  corpus PDF is reused; a different paper already using the corpus id is
  refused, not merged. (3) **Duplicate guard** (`ingest::find_existing`): by
  path, then by SHA-256 against every downloaded corpus file and every
  paper's PDF, hashing only files of the same byte length, cached in
  `.kovan/pdf-sha256.json` (`fingerprint::HashCache`). A match is refused
  (`IngestError::Duplicate`) and the GUI opens the existing entry, saying
  which. The same **file name** with different content is only a warning in
  the ingest form (`IngestPreview::name_clash`): it may be another revision.
  (4) A corpus citation's "Open" on the Mindmap and the Wiki is enabled and
  opens the document the same way. Tests use synthetic PDFs in temporary
  folders: `standard_corpus` (5), `ingest` (4 new), `fingerprint` (1),
  `app::literature_list::standard_corpus_documents_are_listed_as_ingested`.
- **2026-09-30 — Several standard, open and proprietary repositories per
  tier (GitHub issue #458).** Maintainer: "kovan should be able to take on
  multiple standard, multiple open and multiple propreitrary github repos in
  their corpus"; repositories are to be split by topic as they near GitHub's
  recommended ~1 GB (#454). **Decisions.** (1) **Configuration:** a
  `[repos]` table in `kovan_root.toml` with `[[repos.standard]]`,
  `[[repos.open]]` and `[[repos.proprietary]]` entries, each `name`
  (unique across all tiers), `path` (relative to the folder), optional
  `remote`, `branch`, `default` and, standard only, `writable`; plus
  `known_public = [...]` and `builtin_standard` (default true).
  `src/corpus_tiers.rs` resolves it into one ordered list
  (`KovanRoot::corpus_repos`). (2) **Backward compatible without a
  migration:** the older settings are each tier's implicit first
  repository — standard `kovan-standard` (the built-in list,
  `BUILTIN_STANDARD_REPOS`, today `CORPUS_REPOSITORY_URL`) at `[paths]
  standard_corpus`; open `open` at `open_sources` from `[corpora]
  open_remote`; proprietary `proprietary` at `restricted_sources` from
  `proprietary_remote`, else `[private_submodule] remote`. Entries are
  appended; one with the same `path` (or, for standard, the same remote as a
  built-in) replaces the implicit one in place, which is how an existing
  repository is renamed or made the default. A file without `[repos]` writes
  back unchanged (tested). Rejected: rewriting existing files into
  `[[repos]]` form (touches a user's hand-edited file for no gain), and
  making `[repos]` replace the older settings (a silent drop of a repository
  the user already has). (3) **Validation at open and on write**
  (`corpus_tiers::validate`, `RootError::InvalidRepos`): names safe and
  unique, paths relative without `..`, no two repositories of one tier at
  one path, and **no proprietary repository sharing or nested in any other
  repository's path**, since a proprietary PDF inside an open checkout
  would be published by its push. A standard and an open repository may
  share a checkout (pulled and committed once). (4) **Every consumer
  iterates:** `StandardCorpus::for_root` searches every standard, then every
  open repository, so a corpus entry's `corpus_file` is found in whichever
  standard repository holds it; the literature list shows every repository
  of each tier (item labels prefixed with the repository name when a tier
  has several); setup (`ensure_library_corpora`, new
  `CorporaSetup::others`), pull (`pull_corpora`, one `CorpusPull` per
  repository with its `name`), Save (`repository::commit_corpus_repos`:
  every writable checked-out repository committed independently, before the
  Kovan repository; every proprietary directory kept out of the Kovan tree
  by `is_excluded`) and push (`save_push`, one `RepoPush` per repository).
  New `[[repos]]` mounts are added to `.gitignore`
  (`KovanRoot::ensure_repo_paths_ignored`). **Behaviour change, stated:** a
  checked-out writable repository that is not a registered submodule (for
  example a locally initialised open corpus with no remote) is now committed
  by Save; before, only registered submodules and a ready private submodule
  were. (5) **Duplicate guard across everything:** `ingest::find_existing`
  now also hashes every PDF in every repository of every tier
  (`ExistingEntry::RepoFile`), still only files of the incoming length; the
  incoming file itself, already in a repository, is an ingest in place and
  not a duplicate of itself. (6) **Ingest target:** `IngestChoice::target`
  (`RepoRef { tier, name }`), `None` meaning the default repository
  (`default = true`, else the first) of the tier the access implies;
  `resolve_target` refuses a restricted document outside the proprietary
  tier and any read-only standard repository (`IngestError::Target`). The
  Ingest form has a Repository dropdown fed by `ingest::target_choices`
  (restricted: proprietary repositories only; open: open, writable standard,
  proprietary). The repository is recorded as `[source] repo = "<name>"`
  (`SourceRef::repo`). (7) **Push safety, per repository, all earlier rules
  kept** (never forced, detached HEAD only by fast-forward, every push URL
  must be the repository's own configured remote). **A proprietary
  repository is refused** when its remote is any standard or open
  repository's remote, a built-in standard remote, a `known_public` entry
  or an `outram-park-backend` URL (offline,
  `corpus_tiers::public_remote_reason`), or when it is an HTTPS or GitHub
  `git@` remote that `git ls-remote` reads with no credential of the user's
  in reach (`anonymously_readable`: credential helpers cleared, an empty
  `HOME`, no global or system Git config, run from an empty directory, so
  neither a per-URL helper such as `gh auth setup-git`'s nor `~/.netrc` can
  answer; network, skipped for local paths and with
  `KOVAN_SKIP_VISIBILITY_PROBE`). An unreachable probe counts as
  "not shown public", since the push would fail anyway. Standard
  repositories are never committed or pushed unless `writable = true`.
  (8) **Size:** the push report carries a warning for every checkout of
  0.9 GiB or more (files, excluding `.git`), suggesting another repository
  in the same tier (`corpus_tiers::size_warning`, `PushReport::warnings`).
  **The maintainer's layout** (`~/Documents/local-kovan-repo`, read, not
  edited) needs no change; splitting later looks like:

  ```toml
  [[repos.open]]
  name = "reactor-literature"          # renames the implicit open repository
  path = "literature/open-corpus"

  [[repos.open]]
  name = "open-htgr"
  remote = "https://github.com/theodoreOnzGit/<new-open-repo>.git"
  path = "literature/open-htgr"
  default = true

  [[repos.proprietary]]
  name = "proprietary-books"
  remote = "https://github.com/theodoreOnzGit/<new-private-repo>.git"
  path = "literature/proprietary-books"
  ```

  Not done: a GUI editor for `[repos]` (hand-edit the file; the setup
  dialog still configures only the first open and proprietary remote), and
  a check that a non-GitHub SSH remote is private (only the offline list
  applies there). Tests: `corpus_tiers::tests` (6, parsing, resolution,
  validation, offline public-remote guard, size) and
  `corpus_tiers::multi_repo_tests` (9, two repositories in every tier as
  submodules of local bare remotes: discovery, ingest into a chosen
  repository, cross-repository duplicates, per-repository save and push,
  writable standard, the proprietary-remote guard, pull, size warning, the
  maintainer's single-repository file and the refused nested layout).


## The standard mind map is the concept tree, levels 1–3 (2026-10-06, GH #724, #727)

**Maintainer direction:** "Levels 1–3 should be in the standard kovan mindmap
(replacing the existing one) and standard corpus. Kovan should not need the
user's literature repository to populate it. The standard corpus suffices."
And: "the mindmapping should be same aesthetics, but now populated with this
tree."

**What changed.**

- `kovan-literature/src/concept_tree.rs` (new, wasm-clean): parses
  `concept_skeleton.toml` and `concept_proposals.toml` once (`OnceLock`) into
  a typed `ConceptTree` of levels 1–3. Level 3 is the `approved` and
  `deferred` concepts (deferred flagged); `proposed` concepts and the
  `[[implementation]]` seeds are not part of it. API: `concept_tree()`,
  `roots`, `children`, `node`, `parent`, `cross_links`, `cross_linked_from`,
  `documents`, `document`, `documents_cited_by`, `nodes_citing`.
- `corpus.rs`: ~~`TOPICS` (44 rows), `ROOT_TOPIC = "nuclear-engineering"`~~
  replaced by `topics()`, built from the tree, under a **virtual root**
  `ROOT_TOPIC = "iaea_milestones"`, titled "Nuclear knowledge (IAEA
  Milestones)". Its children are the 19 issues in IAEA order. Topic paths
  are the concept paths with **no root prefix**, so a classification names
  them directly; `CorpusTopic::parent_path` (and
  `runtime_graph::parent_of`, used by Up and the breadcrumb) supplies the
  root as a level-1 issue's parent. The id cannot collide: every concept
  path starts with a numbered `NN-` segment, and the underscore is outside
  Kovan's slug alphabet, so no user topic can take it either.
- Ontology links kept only where a node *is* the concept: neutron
  transport (`Neutronics::Transport`), natural-convection cooling
  (`ThermalHydraulics::NaturalCirculation`), liquid-fuelled cores (MSR)
  (`Reactor::Msr`). Diffusion, HTGR and FHR have no node that is that
  concept and lost their links.
- Literature: the 14 existing entries re-filed under concept paths (the
  list is in the commit and the hand-off report); 13 more added, one per
  tree `[[document]]` not already present (11 standard-tier with
  `corpus_file`, 2 private-tier IAEA documents citation-only). A new field,
  `concept_document`, ties an entry to its tree document, and every node
  citing that document files the entry (`filed_under`), so a node's sources
  are among its citations. Only metadata is compiled in; PDFs stay in the
  `reactor-literature` repository.
- Cross-links are curated connections (`curated_connections`,
  `ConnectionOrigin::KovanCorpus`), drawn as the existing light-blue link
  cards with the subtitle "cross-link (built-in)" and no "Remove" entry;
  shown with or without a folder.
- Map style unchanged: same star, cards, colours, pan/zoom and menus.

**User libraries.** The `Library` namespace is untouched. Old
`topics/nuclear-engineering/...` folders were mirrors of corpus topics and
were hidden behind the corpus cards; now that those paths are not corpus
topics, they draw as the user's own (light-green) topics, with their papers
and classifications as before (`runtime_graph` test
`old_nuclear_engineering_folders_stay_user_topics`). Mirrors under the new
concept paths behave as the old ones did.

**Not done.** The "show empty nodes" toggle UI (the model exposes
`corpus::has_literature`, counting a node's own sources, which makes nearly
every node non-empty, and `has_classified_literature`, hand filings only;
the maintainer has not chosen which "empty" means); source hyperlinks on
cards (#729); public URLs for the four `nureg-…` files supplied without an
ADAMS accession number; the Code Review tab and level 4.

## The standard corpus lives in a folder the user chooses, refreshed at every start; a larger centre card; numbered issues (2026-10-06)

**Maintainer direction (2026-10-06).** *"when kovan opens, i need to see the
standard corpus automatically loaded"*; *"the standard corpus should always
refresh on open"*; *"the user also needs to specify an empty folder (or
existing one) for the public corpus, so that kovan knows where to dump the
pdfs"*; *"the central node needs to be bigger in font size, and the 19
milestones need to have their number in the mindmap"*.

**What was wrong.** The standard corpus was cloned once into the
application-data folder (`~/.local/share/kovan/standard-corpus`) and never
updated, so every document added to the corpus later stayed "not
downloaded". The maintainer's clone was several commits behind and lacked all
of the 2026-10-06 NRC, DOE, EC and CFR documents.

**Decisions.**
- At start, Kovan asks where to keep the standard corpus (window
  "Standard corpus folder", `app/corpus_folder.rs`) until a folder is chosen.
  The folder may be new, empty, or an existing clone of the corpus
  (`corpus_repos::check_standard_corpus_folder`); anything else is refused
  with the reason. The choice is remembered in the config folder
  (`standard_corpus.toml`). The old application-data folder is the suggested
  value, so an earlier clone is adopted. "Not now" closes the window for the
  session only. ~~The first-run setup dialog opens after it, never on
  top.~~ **CHANGED the same day** (maintainer: *"during startup i should only
  be prompted for the standard corpus folder and use kovan as is. The local
  corpus will be loaded through the usual setup button"*): this window is the
  only thing asked at start. The first-run setup dialog no longer opens by
  itself; the user's own folder is opened from Home or "⚙ Setup".
- The PDF reader's literature panel and the Ctrl+P finder work with no Kovan
  folder open, listing the standard corpus alone, and list every PDF in
  Kovan's own clone as well as the folder's (`LiteratureList::build(None,
  …)`). The list is rebuilt when a background job (the refresh) finishes.
- In the mind map's right-click citation list, a standard-corpus document
  opens with one click (it has no other action); a library paper keeps its
  sub-menu.
- At every start, the chosen folder is cloned or fast-forwarded in the
  background (`corpus_repos::update_standard_corpus`, sharing
  `save_push::follow_branch` with the folder's Pull). A clone with local
  changes is left alone and reported. A Kovan folder's own corpus
  repositories are still pulled only by Pull.
- The web version does not use any of this: a browser cannot write a folder
  or run git. The agreed web design fetches each PDF from the GitHub raw URL
  on demand (#729).
- The mind map's centre card is 260 × 72 with a title 1.35 times larger,
  wrapping onto two lines. Ring layout, bounds, the Up button and connectors
  use its real size (test `the_centre_card_has_room_at_every_ring_size`).
- Level-1 titles carry their IAEA issue number, "2. Nuclear safety", from the
  `NN-` path segment (`corpus::numbered_title`). Deeper nodes are unnumbered.

## The code map: drawn from the Cargo.toml tags, in the app and on the site (2026-10-06, GH #734)

**Maintainer direction** (#729, #734): kovan draws a deterministic map of
outram-park-backend from the `[package.metadata.kovan]` tags (#733). Root on
top; each row-4 app in its own box; topic boxes as columns through rows 3
and 2, highest fidelity left; a fidelity range spans its columns (raffles
the whole Risk box); utilities base for rows 1-0; the kovan family in a
full-height knowledge-management box to the right; maturity on every card,
maturity 0 greyed; required dependencies faint, lit for a selection. Later
the same day: publish it on the Pages site as well, as an MVP.

**What was built.**
- `src/code_map/` (plain serde + std, no GUI): `CodeMap::from_cargo_metadata`
  (crates sorted by name, required internal edges only, every malformed tag
  reported), `placement_problems`, `layout::layout` and `layout::check`,
  `svg::render`. The integration test `tests/code_map_tags.rs` now reads the
  tags through this parser and checks the real workspace's layout too.
- `kovan-cli code-map [--workspace] [--format json|svg] [-o]`.
- The desktop **Code Map** view (`src/app/code_map_view.rs`): cargo metadata
  on a background thread, folder and file pickers, the mind map's card style,
  canvas and zoom controls, a collapsible details panel.
- The Pages site: `docs/site/code-map/index.html` (inlines the SVG, pan, zoom
  buttons, details from the JSON, `#<crate>` links) and a preview on the main
  menu; `scripts/build-pages.sh` generates both files, never committed.

**Choices made here that the maintainer has not ruled on.**
- **At most three crates of one fidelity side by side** in a row
  (`layout::MAX_TIES`); more wrap to a lane below. Without it the six
  `outram-foam-*` crates at fidelity 3 widened the map further. The map is
  still about seven times wider than tall (6779 x 958 world units with 47
  crates), so Fit on a phone shows a strip; links and the selection zoom in.
- **Columns** in a topic box are the levels a crate sits at plus both ends of
  every range; a column only a range reaches (the Risk box's F4) is half a
  card wide.
- **Edges** use the mind map's edge-to-edge curve
  (`mindmap_view::connector_sized`, made public). Edges between crates in the
  same row run horizontally through the cards between them; no routing.
- **Long names** are shrunk to 11 px, then cut with an ellipsis
  (`code_map::fit_label`); the full name is in the tooltip and the panel.

**rust-analyzer is not used** anywhere in this; phones only render the
static SVG and JSON.

## The call graph: crate → module → function, with source, as JSON (2026-10-06, GH #737)

**Maintainer direction** (#735, #737): `kovan-cli` precomputes the call graph
at three levels and each function's source, deterministically, for the
code-review UI; web-kovan reads it statically, desktop kovan can compute it
live. Function paths must be `code-walk`'s, because review stamps (#739) key
on them.

**What was built.**
- `src/call_graph/` (plain serde + std, no GUI, no I/O): the model
  (`CallGraphDoc`: crates → targets → modules → functions; `calls` with every
  call-site line; `module_calls` and `crate_calls` with site and pair counts;
  `outside`; `totals`), `CallGraphDoc::assemble` (sorts everything,
  `BTreeMap`s only), `function_ids`, and `modules` (the Rust reference's
  `mod` file lookup, `#[path]`, inline modules, `#[cfg(test)]` ranges).
- `src/commands/call_graph.rs` and `kovan-cli call-graph [--workspace]
  [--crates a,b] [-o]`.
- **Reused, not rewritten:** `code_walk::source::FileIndex` finds every `fn`
  (qualified name, signature, doc sentence, body range);
  `code_walk::builder::Workspace::expand` resolves each body through
  rust-analyzer via the keep-warm `lsp_daemon` and classifies what it cannot
  follow as `UNRESOLVED(<kind>)`; `code_map::run_cargo_metadata` and
  `CodeMap::from_cargo_metadata` supply the targets and the maturity tags.
  The only change to code-walk: `Walk` now also keeps every call site
  (`sites`), since its graph keeps the first per pair, and `index`/`node`
  became `pub(crate)`.

**Choices made here that the maintainer has not ruled on.**
- **A module is a source file.** Inline `mod x { }` blocks are not separate
  modules; their functions belong to the file. A `#[cfg(test)]` module file
  is a module marked `test`; functions in test code are kept and marked
  `test`, so the UI can hide them.
- **Targets:** the lib and every example. Bins, integration tests and
  benches are not included.
- **Ambiguous ids:** where `file.rs::Type::name` is not unique in its file
  (two trait impls both defining `fmt`), each gets `#k` in source order and
  `ambiguous: true`. 16 of 3632 functions in the run below. `code-walk
  --from` cannot name these either; #739 should decide what a stamp on one
  keys on.
- **Maturity** is the crate tag's, and for a library module the deepest
  `maturity_modules` entry covering it. Per-function maturity waits for
  stamps (#739).
- **`start_line`** includes the `///` doc comment and attributes above the
  `fn`, so `source` is what a stamp's hash covers (#735: code and `///`
  docs). No hash is computed here; that is #739's.

**Measured (2026-10-06, 16-core desktop, rust-analyzer 1.98.0)** on the
#742 case, `--crates outram-park-digital-twin-engine,boon-lay` (lib + 4
examples, lib + 5 examples): 276 modules, 3632 functions (1253 test),
7288 calls (10536 sites; 6 function values), 1728 unresolved (1464 closure,
216 other, 38 trait, 10 no-definition), 290 outside functions, 27560 calls
into std or dependencies dropped, 37641 definition queries. 127 s with a
cold rust-analyzer in a fresh worktree (about 60 s indexing), 50 s warm;
10.5 MB of JSON; two runs byte-identical (`cmp`). The `htgr_sim_v1` example
analyses like a library: 26 function pairs (28 sites) from it into
boon-lay, including
`examples/htgr_sim_v1/physics/fission_product_release.rs::TrisoAtopsReleaseChannel::update`
→ `crates/boon-lay/src/triso_atops_fork/activities/live_pools.rs::step` (line
876) and `::new_htr10` → `nuclide_model/nuclide_database.rs::supported_nuclides`
(line 770). #742 calls the type `FissionProductRelease`; in the code it is
`TrisoAtopsReleaseChannel`.

**Known gaps (code-walk's, now visible in bulk).** `UNRESOLVED(other)` is
213 of 216 times a derived method (`Default::default()`, `clone()` on a `#[derive]`
type), which rust-analyzer resolves to the derive attribute. A function
value written as a path (`.map(other_crate::leaf)`) is not detected at all,
only a bare name is (found writing `tests/call_graph_rust_analyzer.rs`).
Trait calls stop at `UNRESOLVED(trait)`. Not done: the whole-workspace run
(not measured), wiring into `scripts/build-pages.sh`, and the `kovan_skill.md`
entry.


## Human review stamps: `review/stamps.toml`, hashed with `syn`, voided from git (2026-10-06, GH #739)

**Maintainer direction** (#735 comments, 2026-10-06): stamps live in
`review/stamps.toml`, lightweight TOML; each links to the exact file, lines
and commit; the hash covers the function's code **and its `///` doc
comments**, with `//` comments and formatting normalised away; voiding is
deterministic from git, `kovan-cli stamps-check [--diff <range>]` says
exactly which stamps a change voids and never edits the file; stamping
happens only in desktop kovan (#740). Later the same day (#743): a stamp may
target a Markdown **artifact** instead of a function.

**What was built** (`src/review_stamps/`, `src/commands/stamps.rs`).
- `parse`: the function is found and tokenised by `syn` (`full`) with
  `proc-macro2`'s `span-locations` for lines, not by a brace counter. All
  three crates (`syn`, `proc-macro2`, `quote`) were already in `Cargo.lock`
  through the proc-macro stack; they were added to the root
  `[workspace.dependencies]`, so no new crate entered the tree. The path rule
  is `code-walk`'s (`FileIndex::find`), and `split_spec` is reused from it.
- **The normalisation** (pinned by `parse` tests): doc text = the outer
  literal `#[doc]` attributes (`///`, `/** */`), split into lines, blank
  lines as paragraph breaks, words single-spaced; code = every other token of
  the item, identifiers and literals as written, punctuation one character
  at a time with joint/alone spacing dropped. Hash =
  `sha256("kovan-review-stamp-v1\ndoc\n" + doc + "\ncode\n" + code)`.
  Reformatting, `//` edits, re-wrapping a doc paragraph and moving the
  function keep it; any token, doc word or paragraph break changes it.
- `check`: VALID / VOID with the reason, found by re-locating the function at
  the stamped commit and comparing code and doc separately; a void stamp
  followed by a valid stamp of the same function is STALE (superseded), not a
  failure. `--diff A..B` takes the stamps whose function lines a
  zero-context `git diff --no-renames` touches at A or at B, judged at B.
- `stamp_function` (desktop kovan's entry point) stamps the code as
  committed at `HEAD` and refuses a function whose file is dirty; `kovan-cli
  stamp` refuses without `--i-am-the-reviewer`. **AI agents never stamp.**
- `levels::derived_levels`: for a crate, each rung-3/4 claim in its tag is
  SUPPORTED when every non-test library function in its scope has a valid
  stamp at that rung or above. Report only (`kovan-cli stamps-levels`).

**Choices made here that the maintainer has not ruled on.**
- Re-review appends a new stamp; the old void one stays as history and is
  reported STALE. CI fails only on void stamps nothing supersedes.
- `artifact = "<file.md>#<id>"` is accepted in place of `function` (exactly
  one required), and reported UNCHECKED: artifact hashing and the
  `## Review: …` sign-off exclusion (#743) are not implemented.
- A function nested inside another's body is part of that function and
  cannot be stamped alone (code-walk's scanner does list nested `fn`s).
- `derived_levels` excludes `#[cfg(test)]` modules and `#[test]` functions,
  examples and binaries. Only functions are stampable: `const`s, statics and
  types are not counted, so a module of constants alone cannot be supported
  yet, and says so. A `#[path]` module is reported as a problem, never
  silently counted as supported.
- `review/stamps.toml` is **not** created in the repository yet; the first
  stamp creates it from the documented header (`review_stamps::TEMPLATE`).
