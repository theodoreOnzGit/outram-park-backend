# kovan

Exactly three binaries in one crate, per the final interface spec on GitHub
issue #30 (2026-08-21): `kovan` (human-facing GUI — the graph digitiser
window), `kovan-cli` (agent-facing CLI), `kovan-tui` (human-facing terminal
UI). `kovan-cli`/`kovan-tui` wrap the same deterministic, offline sibling
`kovan-*` crates — `kovan-discovery`, `kovan-literature`, `kovan-semantics`,
`kovan-codegen`, `kovan-metrics`; the graph digitiser is this crate's own
code (`src/digitiser/`), reachable from all three binaries: `kovan` (GUI
review), `kovan-cli digitise` (fully automatic), and `kovan-tui`'s Digitiser
tab (automatic pass + terminal review).

Consolidated 2026-08-21 from the former separate `kovan-cli` and `kovan-tui`
crates — see `DECISIONS.md` for the merge rationale and each front end's
original design history. The graph digitiser joined the same day, moved
here from `kovan-literature` — see the License section below and `NOTICE`.
The binaries were briefly five (`kovan`, `kovan-tui`, `kovan-gui`,
`kovan-digitise`, `kovan-digitise-tui`) before collapsing to the three above
later the same day, per issue #30's final spec: `kovan-gui` was renamed to
plain `kovan`, the old `kovan` (CLI) was renamed to `kovan-cli` and gained a
`digitise` subcommand absorbing `kovan-digitise`, and `kovan-tui` gained a
Digitiser tab absorbing `kovan-digitise-tui`'s interactive review screen.

See [`docs/kovan.md`](../../docs/kovan.md) for KOVAN's overall design
principles (deterministic-first, local-first, Android-first) and mission.

> Unverified until validated — see the workspace root `RESPONSIBLE_USE.md`.
> This crate is at the "Prototype" / "Unit Tested" V&V stage: wired to real
> library functionality and covered by unit + end-to-end tests, but not yet
> exercised against a real literature/repository corpus by a human reviewer.

## License — AGPL-3.0-only (differs from the workspace default)

`kovan` is licensed **AGPL-3.0-only**, not the workspace's usual
GPL-3.0-only — the one deliberate exception in this repository. It depends
on [`kopitiam-pdf`](https://github.com/theodoreOnzGit/kopitiam) (also
AGPL-3.0-only), the pure-Rust PDF page rasterizer this crate's PDF-reader
work (GitHub issue #30) is built on. See [`NOTICE`](./NOTICE) for the full
relicense record, `kopitiam-pdf`'s provenance, and — importantly — the
workspace-boundary rule it sets: no other crate in this repository may take
`kovan` as a dependency without re-examining that boundary first.

## Install / run

```bash
# CLI (agent-facing)
cargo install --path crates/kovan --bin kovan-cli
kovan-cli --help

# or without installing to ~/.cargo/bin:
cargo build --release -p kovan --bin kovan-cli
./target/release/kovan-cli --help

# TUI (human-facing)
cargo build --release -p kovan --bin kovan-tui
./target/release/kovan-tui

# GUI (the graph digitiser window — see "Graph digitiser" below)
cargo build --release -p kovan --bin kovan --features gui
./target/release/kovan
```

Optionally, bring your shell up to a useful baseline for working in this
repository:

```bash
kovan-cli setup             # installs any of a curated tool list (rg, fd, bat, ...) missing from PATH
kovan-cli setup --dry-run   # report what would be installed, without installing anything
```

`kovan-cli setup` is an explicit, online, desktop-scope convenience — see
"`setup`" below. It never runs automatically and has no bearing on the rest
of this crate's offline/Android-clean operation (see "Android").

## `kovan-cli` — CLI commands

```text
kovan-cli discover --root . --kind source
kovan-cli search   --path src/lib.rs --pattern "fn \w+"
kovan-cli search   --root . --kind source --pattern "fn \w+"
kovan-cli scan     --root . --lang rust
kovan-cli methods
kovan-cli symbols  . --lang rust
kovan-cli symbols  . --lang rust --markdown
kovan-cli summary  . --lang rust
kovan-cli gen root newton-raphson
kovan-cli lit import paper.pdf --json-out doc.json
kovan-cli lit bibtex doc.json
kovan-cli lit outline paper.pdf
kovan-cli setup --dry-run
kovan-cli digitise --image fig7.png --x-scale log --x-range 1,1e6 \
    --y-scale log --y-range 0.1,10 --figure "Fig. 7" --json fig7.json
```

Every command's own `--help` documents its flags; the summary below is the
map of what wraps what.

### `discover` / `search` / `scan` — `kovan-discovery` / `kovan-semantics`

- `discover --root <dir> [--kind source|markdown|pdf|metadata]` — walk `root`
  honouring `.gitignore`, print one matching path per line (sorted,
  deterministic).
- `search` — regex search, two modes:
  - `--path <file> --pattern <re>` — search a single file.
  - `--root <dir> [--kind <k>] --pattern <re>` (root defaults to `.`, kind to
    `source`) — search every file of that kind under `root`
    ([`kovan_discovery::search_repository`]).
  - `--path` wins if both are given. Both print ripgrep-style
    `path:line:column: text`.
- `scan --root <dir> --lang rust|cpp|python|fortran` — the cheap
  ripgrep-first "probable definition line" pre-filter
  ([`kovan_semantics::rough_definition_scan`]); prints `path:line: text`.

### `symbols` / `summary` — `kovan-semantics`

- `symbols <root> --lang <lang> [--markdown] [--out <path>] [--name <name>]`
  — catalogue the repository's symbols
  ([`kovan_semantics::catalogue_symbols_detailed`]). Default output is
  line-oriented (`path:line: kind qualified_name`); `--markdown` (or passing
  `--out`) renders the full `symbols.md` artifact
  (`docs/kovan.md`, "Outputs") instead.
- `summary <root> --lang <lang> [--id <id>] [--name <name>] [--out <path>]`
  — render `repository-summary.md`. There is no persisted `KovanRepository`
  catalogue yet, so the repository record is synthesised from `root`'s
  directory name (or `--id`/`--name`) and `--lang`.

Both commands are as fast/approximate as the underlying ripgrep-first
extractor — see `kovan-semantics`'s crate docs for known limits (textual
brace/keyword tracking, no macro expansion).

### `gen` — `kovan-codegen`

`gen <family> <method> [--out <path>]`, one nested subcommand per method
family (mirrors `kovan-cli methods`'s grouping):

```text
kovan-cli gen root       <bisection|regula-falsi|illinois|pegasus|secant|newton-raphson|brent>
kovan-cli gen linear     <jacobi|gauss-seidel|sor|conjugate-gradient|bi-cg-stab|gmres|lu|qr|cholesky>
kovan-cli gen nonlinear  <newton|quasi-newton|broyden|trust-region>
kovan-cli gen ode        <euler|rk2|rk4|dormand-prince|backward-euler|crank-nicolson>
kovan-cli gen pde        <poisson1d-finite-difference|diffusion1d-finite-volume|boundary-condition-scaffold>
```

Prints the generated Rust source to stdout, or writes it to `--out <path>`.
Catalogue entries not yet backed by a template (see `kovan-cli methods` for which
ones) fail with a `CodegenError::Unimplemented` message on stderr and a
non-zero exit code — this is expected, not a bug in the CLI.

### `methods` — the full `kovan-codegen` catalogue

Lists every method in every family with a `ready`/`not-implemented` tag
(whether [`kovan_codegen::generate`] actually emits source for it yet).

### `lit` — `kovan-literature`

Implements the canonical workflow from `docs/kovan.md`, "Literature
Workflow": `PDF → Markdown → KovanDocument → BibTeX`. The Rust
`KovanDocument` struct is authoritative; `lit bibtex` only ever *renders*
from it, never the reverse.

- `lit import <pdf> [--json-out <path>] [--markdown-out <path>]` — extract
  metadata and generate the Markdown body
  ([`kovan_literature::extract_metadata`]), print a line-oriented summary
  (`id`, `slug`, `visibility`, `document_type`, `title`, `authors`, `year`,
  `doi`, `keywords`, `markdown_chars`, `markdown_lines`). `--json-out` writes
  the full `KovanDocument` as pretty JSON (the canonical on-disk form,
  re-readable by `lit bibtex`); `--markdown-out` writes just the generated
  Markdown body.
- `lit bibtex <input>` — emit a BibTeX entry. If `<input>` ends in `.json`,
  it is read back as a `KovanDocument` (e.g. one written by
  `lit import --json-out`); otherwise it is treated as a source PDF and its
  metadata is extracted first. Prints the entry to stdout.
- `lit outline <pdf>` — print the Markdown heading outline of a PDF, one
  heading per line (`#`-repeated-by-level, a space, the heading text). Can be
  empty for a PDF with no high-confidence headings — that is a correct,
  documented result of `kovan-literature`'s deliberately conservative heading
  detection (see its crate docs), not a CLI bug.

### `setup` — curated external CLI tools (`commands::setup`)

`setup [--dry-run] [--force]` installs a small, hard-coded, easily-extended
list of useful external Rust CLI tools via `cargo install`, skipping any
whose binary is already on `PATH`:

| crate (`cargo install <crate>`) | binary | purpose |
|---|---|---|
| `eza` | `eza` | modern `ls` replacement (colour, git status, tree view) |
| `ripgrep` | `rg` | fast recursive regex search — what `kovan-discovery`/`kovan-semantics` shell out to |
| `fd-find` | `fd` | fast, user-friendly `find` replacement |
| `bat` | `bat` | `cat` with syntax highlighting and git-diff markers |
| `tokei` | `tokei` | fast source-code line counter / per-language breakdown |
| `gitoxide` | `gix` | pure-Rust git implementation CLI (cross-platform, no system libgit2) |

- `--dry-run` — report which tools are already present vs. would be
  installed; installs nothing.
- `--force` — reinstall even if the binary is already on `PATH`.
- A missing `cargo`, a network failure, or a non-zero `cargo install` exit
  are all caught per-tool and reported (`[FAILED] <tool> — <reason>`) rather
  than panicking; one failing tool never stops the rest. The command exits
  non-zero only if at least one requested install genuinely failed.

**`setup` is explicit, online, and desktop-scope** — no other `kovan-cli`
subcommand calls it, it is never run automatically, and it does not affect
the rest of this crate's offline/Android-clean core operation (below). On
Android it detects PATH presence normally but no-ops the actual install
(there is no meaningful `cargo install`-a-dev-tool host on-device).

### Determinism & offline guarantees

Every `kovan-cli` subcommand **except `setup`** is deterministic and fully
offline, inheriting the guarantees of the library crate it wraps (see each
`kovan-*` crate's own `README.md`/crate docs for the specifics:
`kovan-discovery`'s sorted-output contract, `kovan-literature`'s
byte-for-byte PDF pipeline, `kovan-codegen`'s byte-identical generation). The
CLI itself adds no additional non-determinism — it only formats and prints
what the library call returned. `setup` is the one deliberate exception: it
reads the live `PATH` and, unless `--dry-run`, reaches the network via
`cargo install`.

## `kovan-tui` — screens

Seven tabs, switched with `1`-`7` or `Tab`/`Shift+Tab`. `q`/`Esc` quits from
any tab (except while a text field is being edited, where `Esc` only cancels
the edit, and except while the Ingest or Digitiser tab has work in flight —
see below). **Genuinely Android/Termux-usable, not just buildable** — see
"Android" below.

| # | Tab | Backing crate | What it does |
|---|-----|----------------|---------------|
| 1 | **Overview** | — | Static module map — the landing screen. |
| 2 | **Browser** | `kovan-discovery` | Walk a repository root, filter by `FileKind` (source/markdown/pdf/metadata/all), navigate the discovered files. |
| 3 | **Symbols** | `kovan-semantics` | Catalogue a repository's symbols with the ripgrep-first extractor; toggle between the raw symbol list and a live preview of the generated `symbols.md` Markdown artifact. |
| 4 | **Methods** | `kovan-codegen` | Browse the numerical-method catalogue by family (root finders / linear / nonlinear / ODE / PDE) and preview a method's generated Rust source. |
| 5 | **Literature** | `kovan-literature` | List PDFs / Markdown / BibTeX under a literature root and preview each: metadata extraction for PDFs, heading outline for Markdown, raw text for BibTeX. |
| 6 | **Ingest** | `kovan-literature` | Import a PDF interactively: pick it from a directory listing, watch extraction run on a worker thread, **review and correct the extracted metadata**, then save the Markdown / `KovanDocument` JSON / BibTeX. |
| 7 | **Digitiser** | this crate's own `digitiser` | Digitise a plot image interactively: fill in a Setup form (image path, axis scales/ranges, figure/labels), watch the automatic trace run on a worker thread, **review/correct the traced points**, then save the dataset JSON/CSV. Absorbed the standalone `kovan-digitise-tui` binary on 2026-08-21. |

Tabs 1-5 are **read-only viewers** — they read the filesystem through the
sibling crates and render their deterministic output; they never write to the
repositories or literature trees they browse (KOVAN's "not a repository
modification agent" non-goal, `docs/kovan.md` § "Non-Goals"). **Ingest and
Digitiser are the two screens that write**, and only on an explicit save key
(`s` for Ingest, `S`/`s` for Digitiser) to output paths the user can see and
edit; neither modifies its source (PDF / plot image) or touches a repository
being browsed. See the former `kovan-tui` `README.md` content preserved via
`DECISIONS.md`'s pre-merge history for the full Ingest workflow write-up
(pick → wait → review/correct → save) and the real-report example
(`ANL-7416 Supplement 2`) motivating the review step; see "Graph digitiser"
below for the Digitiser tab's engine and the same pattern applied there.

### Key bindings

Global (when no field is being edited):

- `1`-`7` / `Tab` / `Shift+Tab` — switch tabs.
- `q` / `Esc` — quit. On the Ingest or Digitiser tab this is refused while
  work is running or an unsaved review is on screen; press `x` to discard
  first.

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
- **Digitiser**, Setup: `Up`/`Down`/`Tab`/`Shift+Tab` move between fields, `e`
  edit the focused field, `Enter` starts the automatic pass (needs at least
  image path, figure, x/y range filled in).
- **Digitiser**, running: `x` abandon.
- **Digitiser**, reviewing: `Tab`/`Left`/`Right` select a point, `Up`/`Down`
  nudge it in y, `h`/`l` nudge in x (hold `Shift` for a 5px step), `d`
  delete, `a` duplicate the selected point as hand-placed, `v` mark
  reviewed, `e` edit the JSON save path, `S`/`s` save, `x` discard.

## Graph digitiser — `kovan` / `kovan-cli digitise` / `kovan-tui`'s Digitiser tab

**Moved into this crate from `kovan-literature` on 2026-08-21** (engine at
`src/digitiser/`) — see `NOTICE` for why: only the digitiser needs
`kopitiam-pdf` (GitHub issue #30's PDF-native work), which is why this
crate alone is relicensed AGPL-3.0-only, and `kovan-literature` — used well
beyond the GUI — must not be dragged into that. See the workspace
`CLAUDE.md` "Graph digitisation: dogfood kovan-digitise" for the
mandated-tool context; that section's `-p kovan-literature` invocations now
read `-p kovan`.

Extract `(x, y)` data points from a plot image with a full calibration +
provenance record (`DigitisedDataset`): load an image, calibrate the axes
(linear or log, independently per axis), auto-trace or hand-place points,
review/correct, export. Three front ends over one engine, all in this same
crate — collapsed from five standalone binaries to three later the same day
(2026-08-21), per GitHub issue #30's final interface spec:

- **`kovan-cli digitise`** — fully automatic, scriptable subcommand (the
  agent path). Absorbed the former standalone `kovan-digitise` binary; no
  feature needed (`clap` is already a hard dependency of `kovan-cli`).
- **`kovan-tui`'s Digitiser tab** — automatic pass, then a `ratatui`
  terminal review screen (`src/tui/digitiser.rs`). Absorbed the former
  standalone `kovan-digitise-tui` binary's review mechanics unchanged
  (nudge/delete/duplicate/mark-reviewed/save); only the phase machine and
  key/draw dispatch were adapted to fit alongside `kovan-tui`'s other tabs.
- **`kovan`** — automatic pass, then an egui review window
  (graphreader-style: drag/add/delete points by mouse). This is the
  renamed-from-`kovan-gui` binary itself — built by default on desktop (its
  `gui` feature is a default feature) but never on Android (see "Android"
  below for why):

  ```bash
  cargo run --release -p kovan --bin kovan --features gui [image-path]
  ```

  Used to have a same-behaviour twin, `kovan-digitise-gui`, back when
  `kovan-literature` owned this engine — retired in the move rather than
  carried forward as a second name for the same binary.

See [`kovan::digitiser`] module docs for the interaction model,
calibration/provenance guarantees, and known limits (no tick-label OCR —
axis reference values must be supplied by the caller).

## Android

**`kovan-cli` and `kovan-tui` are both non-GUI and genuinely
Android/Termux-usable, not just buildable** — every screen and subcommand,
including the Digitiser tab, actually runs there:

```bash
cargo check -p kovan --bin kovan-cli --target aarch64-linux-android
cargo check -p kovan --bin kovan-tui --target aarch64-linux-android
cargo check -p kovan --all-targets --target aarch64-linux-android
```

`ratatui` (and its bundled `crossterm`) is an **unconditional** dependency of
this crate — not target-gated off Android. Until 2026-08-21 it *was*
Android-gated, and `#[cfg(not(target_os = "android"))] pub mod tui;` in
`src/lib.rs` was a hard technical necessity as a result (the `tui` module
couldn't reference `ratatui` types on a target that didn't have `ratatui`
as a dependency at all) — `kovan-tui`'s binary compiled to a desktop-only
redirect stub on Android. `ratatui` was made unconditional the same day so
the former `kovan-digitise-tui` binary's Android-functional review screen
would keep working once it moved into `kovan-tui` as the Digitiser tab —
and that same change removed the *technical* reason for gating `pub mod
tui;` at all. The gate was dropped in the same change: `kovan-tui` is now a
single, ungated `main()` with no Android stub, and the whole seven-tab TUI
— not just the digitiser — is Android-buildable and Android-runnable.
Confirmed 2026-08-21: `cargo check -p kovan --all-targets --target
aarch64-linux-android` is clean. This directly serves GitHub issue #30's
stated reason for wanting exactly `kovan`/`kovan-cli`/`kovan-tui`: Android
usability.

`kovan` (the GUI) is desktop-only by design (egui/eframe are Android-hostile)
and sits behind the `gui` feature — a **default** feature everywhere except
Android, where its `eframe`/`egui` dependencies are additionally
target-gated off, so building this crate with its default feature set —
what a plain `cargo build -p kovan` or an Android/Termux build does — never
pulls egui/eframe into the dependency graph there. `kovan` itself has no
Android build (the redirect message it *would* print lives inside
[`kovan::digitiser::gui::run`], since that function is used on Android by
nothing — no binary in this crate calls it there).

## Testing

```bash
cargo test --release -p kovan
```

- `src/bin/kovan-cli.rs` unit tests — `clap` argument-parsing coverage for
  every subcommand (via `Cli::try_parse_from`), plus small pure-function
  tests in each `commands::*` module.
- `tests/cli_e2e.rs` — black-box end-to-end tests that spawn the compiled
  `kovan-cli` binary against synthetic, throwaway fixtures (a tempdir Rust
  "repository", and a minimal synthetic PDF built with `lopdf` — mirroring
  `kovan-literature`'s own private test-PDF helper so no real, possibly
  proprietary PDF ever ships as a fixture) and assert on stdout/stderr/exit
  code. Targets `kovan-cli` specifically — `kovan` is the GUI binary and
  needs a display, so it cannot run headlessly here.
- `src/tui/**` unit tests — state-update ("reducer") tests that construct a
  tab's state struct and call its `handle_key(key, ..)` directly, plus
  `ratatui::backend::TestBackend` render tests, plus fixture-backed tests
  driving the real sibling-crate calls against throwaway trees (no mocking).

## Layout

```text
src/
├── lib.rs                library re-exports shared by the three binaries
├── commands/             kovan-cli command implementations
│   ├── mod.rs              shared clap-facing enums (KindArg, LangArg)
│   ├── discover.rs         `kovan-cli discover`
│   ├── search.rs           `kovan-cli search` (single-file + repository modes)
│   ├── scan.rs             `kovan-cli scan`
│   ├── methods.rs          `kovan-cli methods`
│   ├── symbols.rs          `kovan-cli symbols` / `kovan-cli summary`
│   ├── gen.rs              `kovan-cli gen <family> <method>`
│   ├── lit.rs              `kovan-cli lit import|bibtex|outline`
│   └── setup.rs            `kovan-cli setup` (curated external-tool installer)
├── digitiser/             graph digitiser engine, shared by all three binaries
│   ├── mod.rs              `DigitiserError`, module map
│   ├── raster.rs           image loading (`PlotRaster`)
│   ├── calibration.rs      pixel <-> data coordinate mapping (linear/log per axis)
│   ├── detect.rs           automatic plot-frame detection
│   ├── trace.rs            automatic curve tracing (strategies, colour selectors)
│   ├── dataset.rs          `DigitisedDataset` and its provenance types
│   ├── auto.rs             the one-shot automatic pipeline
│   ├── frontend.rs         shared `AutoArgs` clap surface (`kovan-cli digitise`, `kovan-tui`'s Digitiser tab)
│   ├── synthetic.rs        deterministic test-fixture rendering
│   └── gui.rs              *(behind the `gui` feature)* the `kovan` binary's egui app
├── tui/                  kovan-tui screens (Android/Termux-usable, see "Android")
│   ├── mod.rs              App state, key-event dispatch, terminal setup/teardown
│   ├── text_input.rs       tiny shared single-line text buffer (root-path fields)
│   ├── overview.rs         Overview tab
│   ├── browser.rs          Browser tab (kovan-discovery)
│   ├── symbols.rs          Symbols tab (kovan-semantics)
│   ├── methods.rs          Methods tab (kovan-codegen)
│   ├── literature.rs       Literature tab (kovan-literature)
│   ├── ingest/             Ingest tab (kovan-literature)
│   │   ├── mod.rs            phase state machine, PDF picker, worker thread
│   │   ├── review.rs         metadata review form, slug/id regeneration, saving
│   │   └── draw.rs           rendering for each phase
│   └── digitiser.rs        Digitiser tab: Setup form, worker thread, review screen (over `../digitiser/`)
└── bin/
    ├── kovan.rs            GUI: thin wrapper over `digitiser::gui::run`
    ├── kovan-cli.rs        CLI: clap surface + dispatcher (incl. `digitise`)
    └── kovan-tui.rs        TUI: entry point (`kovan::tui::run()`)
```

One module per command/screen, so each binary stays a thin dispatcher — the
workspace's file-size-cap rule (`CLAUDE.md`) applies to this crate too; every
file here is well under the 1000-line cap.

See `DECISIONS.md` for command/screen design rationale, what was left out,
and the 2026-08-21 crate-consolidation rationale.
