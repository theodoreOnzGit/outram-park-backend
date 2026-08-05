# kovan-tui

The **human-facing** front end to KOVAN (binary: `kovan-tui`). A `ratatui`
terminal UI for browsing repositories, symbol catalogues, the numerical-method
codegen catalogue, and the literature archive — and for **ingesting literature**
(import a PDF, review and correct its extracted metadata, save the generated
artifacts). All of it is backed by the same deterministic, offline sibling
crates the agent-facing `kovan-cli` wraps. Agents should use `kovan-cli`
instead; this crate is for a person at a keyboard.

See [`docs/kovan.md`](../../docs/kovan.md) for KOVAN's overall design
principles (deterministic-first, local-first, Android-first) and mission.

> Unverified until validated — see the workspace root `RESPONSIBLE_USE.md`.
> This crate is at the "Prototype" / "Unit Tested" V&V stage: wired to real
> library functionality (no mocked data) and covered by unit tests plus a
> manual interactive smoke test of every screen, but not yet exercised by a
> human reviewer against a real, non-synthetic literature/repository corpus.

## Install / run

```bash
cargo build --release -p kovan-tui
./target/release/kovan-tui
```

Desktop only (Linux/Windows/macOS). On Android the binary compiles but prints
a message pointing you at the `kovan` CLI instead — see "Android" below.

## Screens

Six tabs, switched with `1`-`6` or `Tab`/`Shift+Tab`. `q`/`Esc` quits from any
tab (except while a text field is being edited, where `Esc` only cancels the
edit, and except while the Ingest tab has an import in flight — see below).

| # | Tab | Backing crate | What it does |
|---|-----|----------------|---------------|
| 1 | **Overview** | — | Static module map — the landing screen. |
| 2 | **Browser** | `kovan-discovery` | Walk a repository root, filter by `FileKind` (source/markdown/pdf/metadata/all), navigate the discovered files. |
| 3 | **Symbols** | `kovan-semantics` | Catalogue a repository's symbols with the ripgrep-first extractor; toggle between the raw symbol list and a live preview of the generated `symbols.md` Markdown artifact. |
| 4 | **Methods** | `kovan-codegen` | Browse the numerical-method catalogue by family (root finders / linear / nonlinear / ODE / PDE) and preview a method's generated Rust source. |
| 5 | **Literature** | `kovan-literature` | List PDFs / Markdown / BibTeX under a literature root and preview each: metadata extraction for PDFs, heading outline for Markdown, raw text for BibTeX. |
| 6 | **Ingest** | `kovan-literature` | Import a PDF interactively: pick it from a directory listing, watch extraction run on a worker thread, **review and correct the extracted metadata**, then save the Markdown / `KovanDocument` JSON / BibTeX. |

Tabs 1-5 are **read-only viewers** — they read the filesystem through the
sibling crates and render their deterministic output; they never write to the
repositories or literature trees they browse (KOVAN's "not a repository
modification agent" non-goal, `docs/kovan.md` § "Non-Goals"). The **Ingest tab
is the one screen that writes**, and only on an explicit `s` (save) to output
paths the user can see and edit. It never modifies the source PDF, and it never
touches a repository being browsed.

### Ingesting literature (tab 6)

Equivalent to `kovan lit import <PDF> [--json-out <p>] [--markdown-out <p>]`,
calling the same library functions — with a review step in the middle.

1. **Pick** — type a directory (`e`) and an optional filename filter (`f`),
   press `r` to scan; PDFs are found with the same `.gitignore`-aware walk the
   Browser tab uses. `Enter` imports the selected file. (Or press `i` on a PDF
   in the Literature tab, which hands it straight to this tab.)
2. **Wait** — extraction runs on a worker thread so the UI never freezes. On a
   developer desktop (release build) a 12 MB / 447-page scanned report extracts
   in about 0.3 s, but the cost is a property of the file and the machine and is
   unbounded in principle, so the UI does not assume it is quick. The screen
   shows the file, its size, elapsed seconds and a spinner. It does
   **not** show a percentage: `kovan-literature` reports no intermediate
   progress, and an invented progress bar would be a lie. `x` abandons the wait
   (the library call itself cannot be interrupted; its result is discarded).
3. **Review and correct** — title, authors, year, document type and institution
   are editable, with a `*` marking every field changed from what the extractor
   produced. The right-hand pane shows the record that would be written,
   including the regenerated slug/id and the exact BibTeX.
4. **Save** — `s` writes whichever of the three output paths are non-empty,
   creating parent directories. Every success and failure is listed in the save
   report; nothing panics and nothing is written when the form is invalid.

**Why the review step exists.** `kovan_literature::extract_metadata` is
best-effort by design. Importing a real 1977 Argonne report
(`ANL-7416 Supplement 2`, 12 MB, 447 pages) reproduces the problem exactly:
the title is right, but the year comes out as `2004` (the *digitisation* date
recorded by the scanner), the author list is empty (the real corporate author
is "Argonne Code Center"), and the slug is therefore `2004anl7416`. Left alone,
that becomes a wrong BibTeX entry and then a wrong citation — a provenance
error under the workspace's `RESEARCH_INTEGRITY_AND_PROVENANCE.md`, not a
cosmetic one. The tab flags all of it up front:

```text
- authors: extraction found none (it never guesses from body text) — type them,
  e.g. a corporate author as 'Argonne Code Center'
- year 2004 may be a digitisation/scan date — earlier years in the text:
  1963, 1968, 1971, 1972, 1973, 1977
- type: 'Other' renders as BibTeX @misc — set Report/Paper/Benchmark if it is one
- title 'ANL-7416 Supplement 2' looks like a report number, not a title
```

Advisories never change data on their own. Correcting the year and author
re-derives the identifiers (`2004anl7416` → `argonnecodecenter1977anl7416`)
before anything is saved, and the default output paths follow the corrected
slug.

### Key bindings

Global (when no field is being edited):

- `1`-`6` / `Tab` / `Shift+Tab` — switch tabs.
- `q` / `Esc` — quit. On the Ingest tab this is refused while an extraction is
  running or an unsaved review is on screen; press `x` to discard first, so a
  reflexive `q` cannot throw away hand-corrected metadata.

Browser / Symbols / Literature (each owns one root-path text field):

- `e` — start editing the root path.
- While editing: type to append, `Backspace` to delete, `Enter` to confirm
  (and run the scan), `Esc` to cancel (keeps the previous value).
- `Left` / `Right` — cycle the kind/language filter.
- `Up` / `Down` — move the list selection.

Screen-specific:

- **Browser**: `Enter` (not editing) re-runs the scan with the current
  root/filter.
- **Symbols**: `Enter` (not editing) re-runs the catalogue; `m` toggles
  between the symbol list and the `symbols.md` Markdown preview (`Up`/`Down`
  scroll the Markdown preview instead of the list while it's showing).
- **Methods**: `Left`/`Right` cycle the method family, `Up`/`Down` move the
  method selection, `Enter` generates and previews the selected method's
  source, `PageUp`/`PageDown` scroll the preview.
- **Literature**: `Enter` (not editing) previews the selected entry instead of
  re-scanning — `r` re-scans. `PageUp`/`PageDown` scroll the preview. `i`
  imports the selected PDF on the Ingest tab.
- **Ingest**, picking a file: `e` edit the directory, `f` edit the filename
  filter, `r` rescan, `Up`/`Down` select, `Enter` import.
- **Ingest**, importing: `x` abandon the wait.
- **Ingest**, reviewing: `Up`/`Down` move between fields, `e` (or `Enter`) edit
  the focused field, `Left`/`Right` cycle the document type (that row only),
  `s` save, `x` discard, `PageUp`/`PageDown` scroll the record pane.

## Android

This crate is source-gated per the workspace's Android rule
(root `CLAUDE.md`, "Android portability"): `ratatui` (and its bundled
`crossterm`) are pulled only under `cfg(not(target_os = "android"))`, and the
whole `tui` module tree lives behind that same gate on `main.rs`'s single
`mod tui;` declaration, so no submodule needs to repeat it. On Android the
binary compiles to a two-line stub that redirects the user to the `kovan` CLI.

`serde_json` (used by the Ingest tab to write the canonical `KovanDocument`
JSON) is declared in the same non-Android dependency table for the same reason:
it is only reachable from the gated `tui` module tree, so the Android stub build
stays dependency-free.

```bash
cargo check -p kovan-tui --target aarch64-linux-android              # library/stub path — clean
cargo check -p kovan-tui --all-targets --target aarch64-linux-android  # examples/tests/benches too — clean
```

## Testing

```bash
cargo test --release -p kovan-tui
```

98 unit tests, all headless (no real terminal spawned):

- **State-update ("reducer") tests** — construct a tab's state struct and call
  its `handle_key(key, ..)` directly, asserting on the resulting state (list
  selection, scanned entries, edit-mode transitions, wraparound at the ends of
  a cyclic filter). This is the bulk of the coverage and needs no rendering at
  all.
- **Render tests** — `ratatui::backend::TestBackend` + `Terminal::draw`,
  asserting the rendered cell buffer contains expected text (or simply that
  `draw` does not panic across every tab/family/view-mode combination).
- Fixture-backed tests (`tempfile::TempDir`) drive the real sibling-crate calls
  (`kovan_discovery::discover_kind`, `kovan_semantics::catalogue_symbols_detailed`,
  `kovan_literature::markdown_outline`/`extract_metadata`) against throwaway
  trees — no mocking, so a regression in a sibling crate's behaviour would
  surface here too.
- **Ingestion tests** drive the whole worker path without a real PDF: a
  `.pdf`-named file with junk contents makes the worker thread report a real
  `kovan-literature` error, which the test collects through the same
  `tick()`/channel code the draw loop uses. The metadata-correction logic
  (slug/id regeneration, author parsing, advisories, saving) is tested against
  a `KovanDocument` built in-process — no real or proprietary PDF is used as a
  fixture anywhere.

Beyond the unit suite, every screen was exercised in a live `tmux` terminal
session during development (typing a root path, scanning, toggling the
Symbols Markdown view, generating a Methods preview, previewing a Literature
entry, importing and correcting a real 447-page report, quitting) — see
`DECISIONS.md` for what was checked.

## Layout

```text
src/
├── main.rs              Android/desktop entry-point split; `mod tui;` gate
└── tui/
    ├── mod.rs            App state, key-event dispatch, terminal setup/teardown, top-level draw
    ├── text_input.rs     tiny shared single-line text buffer (root-path fields)
    ├── overview.rs        Overview tab
    ├── browser.rs          Browser tab (kovan-discovery)
    ├── symbols.rs           Symbols tab (kovan-semantics)
    ├── methods.rs            Methods tab (kovan-codegen)
    ├── literature.rs          Literature tab (kovan-literature)
    └── ingest/                 Ingest tab (kovan-literature)
        ├── mod.rs               phase state machine, PDF picker, worker thread
        ├── review.rs             metadata review form, slug/id regeneration, saving
        └── draw.rs                rendering for each phase
```

One module per screen, each under the workspace's 1000-line file-size cap
(root `CLAUDE.md`). `App` in `tui/mod.rs` owns one state struct per tab by
value (no `Arc`/lock) — see that module's doc comment for why the workspace's
`Arc<RwLock<T>>` shared-state rule doesn't apply to a single-threaded terminal
event loop, and `DECISIONS.md` for more.

See `DECISIONS.md` for screen/navigation design rationale, what was
deliberately left out, and open questions for human review.
