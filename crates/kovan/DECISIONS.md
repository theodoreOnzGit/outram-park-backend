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
  It reuses `kovan_literature::digitiser::gui::run` (extracted from what was
  previously private code inside `kovan-literature`'s own
  `kovan-digitise-gui` binary) — the digitiser window is KOVAN's one
  established GUI surface, and this binary is a thin wrapper around the same
  library function `kovan-digitise-gui` now calls too. Gated behind this
  crate's non-default `gui` feature, which turns on `kovan-literature`'s
  `digitise-gui` feature in turn, so the default `kovan`/`kovan-tui` builds
  never pull egui/eframe.

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
