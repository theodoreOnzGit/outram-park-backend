# kovan-tui

The **human-facing** front end to KOVAN (binary: `kovan-tui`). A `ratatui`
terminal UI for browsing repositories, symbol catalogues, the numerical-method
codegen catalogue, and the literature archive — all backed by the same
deterministic, offline sibling crates the agent-facing `kovan-cli` wraps.
Agents should use `kovan-cli` instead; this crate is for a person at a
keyboard.

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

Five tabs, switched with `1`-`5` or `Tab`/`Shift+Tab`. `q`/`Esc` quits from any
tab (except while a text field is being edited, where `Esc` only cancels the
edit).

| # | Tab | Backing crate | What it does |
|---|-----|----------------|---------------|
| 1 | **Overview** | — | Static module map — the landing screen. |
| 2 | **Browser** | `kovan-discovery` | Walk a repository root, filter by `FileKind` (source/markdown/pdf/metadata/all), navigate the discovered files. |
| 3 | **Symbols** | `kovan-semantics` | Catalogue a repository's symbols with the ripgrep-first extractor; toggle between the raw symbol list and a live preview of the generated `symbols.md` Markdown artifact. |
| 4 | **Methods** | `kovan-codegen` | Browse the numerical-method catalogue by family (root finders / linear / nonlinear / ODE / PDE) and preview a method's generated Rust source. |
| 5 | **Literature** | `kovan-literature` | List PDFs / Markdown / BibTeX under a literature root and preview each: metadata extraction for PDFs, heading outline for Markdown, raw text for BibTeX. |

Every screen is a **read-only viewer** — it reads the filesystem through the
sibling crates and renders their deterministic output; it never writes to the
repositories or literature trees it browses (KOVAN's "not a repository
modification agent" non-goal, `docs/kovan.md` § "Non-Goals").

### Key bindings

Global (when no field is being edited):

- `1`-`5` / `Tab` / `Shift+Tab` — switch tabs.
- `q` / `Esc` — quit.

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
  re-scanning — `r` re-scans. `PageUp`/`PageDown` scroll the preview.

## Android

This crate is source-gated per the workspace's Android rule
(root `CLAUDE.md`, "Android portability"): `ratatui` (and its bundled
`crossterm`) are pulled only under `cfg(not(target_os = "android"))`, and the
whole `tui` module tree lives behind that same gate on `main.rs`'s single
`mod tui;` declaration, so no submodule needs to repeat it. On Android the
binary compiles to a two-line stub that redirects the user to the `kovan` CLI.

```bash
cargo check -p kovan-tui --target aarch64-linux-android          # library/stub path — clean
cargo check -p kovan-tui --tests --target aarch64-linux-android  # confirms the tempfile dev-dep is Android-clean too
```

## Testing

```bash
cargo test --release -p kovan-tui
```

44 unit tests, all headless (no real terminal spawned):

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

Beyond the unit suite, every screen was exercised in a live `tmux` terminal
session during development (typing a root path, scanning, toggling the
Symbols Markdown view, generating a Methods preview, previewing a Literature
entry, quitting) — see `DECISIONS.md` for what was checked.

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
    └── literature.rs          Literature tab (kovan-literature)
```

One module per screen, each under the workspace's 1000-line file-size cap
(root `CLAUDE.md`). `App` in `tui/mod.rs` owns one state struct per tab by
value (no `Arc`/lock) — see that module's doc comment for why the workspace's
`Arc<RwLock<T>>` shared-state rule doesn't apply to a single-threaded terminal
event loop, and `DECISIONS.md` for more.

See `DECISIONS.md` for screen/navigation design rationale, what was
deliberately left out, and open questions for human review.
