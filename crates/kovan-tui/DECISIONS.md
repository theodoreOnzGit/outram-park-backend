# kovan-tui — decisions, assumptions, open questions

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
