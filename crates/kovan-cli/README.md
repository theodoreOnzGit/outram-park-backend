# kovan-cli

The **agent-facing** front end to KOVAN (binary: `kovan`). It exposes the
knowledge-layer operations from the sibling `kovan-*` crates as plain
subcommands with deterministic, line-oriented output, so a coding agent
(Claude Code and friends) can drive KOVAN and parse the results without a
JSON layer in between. Humans get the richer `kovan-tui` instead.

See [`docs/kovan.md`](../../docs/kovan.md) for KOVAN's overall design
principles (deterministic-first, local-first, Android-first) and mission.

> Unverified until validated — see the workspace root `RESPONSIBLE_USE.md`.
> This crate is at the "Prototype" / "Unit Tested" V&V stage: wired to real
> library functionality and covered by unit + end-to-end tests, but not yet
> exercised against a real literature/repository corpus by a human reviewer.

## Install / run

```bash
cargo install --path crates/kovan-cli
kovan --help
```

(or, without installing to `~/.cargo/bin`: `cargo build --release -p kovan-cli`
and run `./target/release/kovan --help`.)

Optionally, bring your shell up to a useful baseline for working in this
repository:

```bash
kovan setup             # installs any of a curated tool list (rg, fd, bat, ...) missing from PATH
kovan setup --dry-run   # report what would be installed, without installing anything
```

`kovan setup` is an explicit, online, desktop-scope convenience — see
"`setup`" below. It never runs automatically and has no bearing on the rest
of this crate's offline/Android-clean operation (see "Android").

## Commands

```text
kovan discover --root . --kind source
kovan search   --path src/lib.rs --pattern "fn \w+"
kovan search   --root . --kind source --pattern "fn \w+"
kovan scan     --root . --lang rust
kovan methods
kovan symbols  . --lang rust
kovan symbols  . --lang rust --markdown
kovan summary  . --lang rust
kovan gen root newton-raphson
kovan lit import paper.pdf --json-out doc.json
kovan lit bibtex doc.json
kovan lit outline paper.pdf
kovan setup --dry-run
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
family (mirrors `kovan methods`'s grouping):

```text
kovan gen root       <bisection|regula-falsi|illinois|pegasus|secant|newton-raphson|brent>
kovan gen linear     <jacobi|gauss-seidel|sor|conjugate-gradient|bi-cg-stab|gmres|lu|qr|cholesky>
kovan gen nonlinear  <newton|quasi-newton|broyden|trust-region>
kovan gen ode        <euler|rk2|rk4|dormand-prince|backward-euler|crank-nicolson>
kovan gen pde        <poisson1d-finite-difference|diffusion1d-finite-volume|boundary-condition-scaffold>
```

Prints the generated Rust source to stdout, or writes it to `--out <path>`.
Catalogue entries not yet backed by a template (see `kovan methods` for which
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

- `--dry-run` — report which tools are already present vs. would be
  installed; installs nothing.
- `--force` — reinstall even if the binary is already on `PATH`.
- A missing `cargo`, a network failure, or a non-zero `cargo install` exit
  are all caught per-tool and reported (`[FAILED] <tool> — <reason>`) rather
  than panicking; one failing tool never stops the rest. The command exits
  non-zero only if at least one requested install genuinely failed.

**`setup` is explicit, online, and desktop-scope** — no other `kovan`
subcommand calls it, it is never run automatically, and it does not affect
the rest of this crate's offline/Android-clean core operation (below). On
Android it detects PATH presence normally but no-ops the actual install
(there is no meaningful `cargo install`-a-dev-tool host on-device).

## Determinism & offline guarantees

Every subcommand **except `setup`** is deterministic and fully offline,
inheriting the guarantees of the library crate it wraps (see each `kovan-*`
crate's own `README.md`/crate docs for the specifics: `kovan-discovery`'s
sorted-output contract, `kovan-literature`'s byte-for-byte PDF pipeline,
`kovan-codegen`'s byte-identical generation). The CLI itself adds no
additional non-determinism — it only formats and prints what the library
call returned. `setup` is the one deliberate exception: it reads the live
`PATH` and, unless `--dry-run`, reaches the network via `cargo install`.

## Android

This crate is non-GUI and Android-buildable:

```bash
cargo check -p kovan-cli --target aarch64-linux-android
```

## Testing

```bash
cargo test --release -p kovan-cli
```

- `src/main.rs` unit tests — `clap` argument-parsing coverage for every
  subcommand (via `Cli::try_parse_from`), plus small pure-function tests in
  each `commands::*` module.
- `tests/cli_e2e.rs` — black-box end-to-end tests that spawn the compiled
  `kovan` binary against synthetic, throwaway fixtures (a tempdir Rust
  "repository", and a minimal synthetic PDF built with `lopdf` — mirroring
  `kovan-literature`'s own private test-PDF helper so no real, possibly
  proprietary PDF ever ships as a fixture) and assert on stdout/stderr/exit
  code.

## Layout

```text
src/
├── main.rs              clap surface + dispatcher only
└── commands/
    ├── mod.rs            shared clap-facing enums (KindArg, LangArg)
    ├── discover.rs        `kovan discover`
    ├── search.rs           `kovan search` (single-file + repository modes)
    ├── scan.rs              `kovan scan`
    ├── methods.rs            `kovan methods`
    ├── symbols.rs             `kovan symbols` / `kovan summary`
    ├── gen.rs                  `kovan gen <family> <method>`
    ├── lit.rs                   `kovan lit import|bibtex|outline`
    └── setup.rs                  `kovan setup` (curated external-tool installer)
```

One module per command (or command group), so `main.rs` stays a thin `clap`
dispatcher — the workspace's file-size-cap rule (`CLAUDE.md`) applies to this
crate too; every file here is well under the 1000-line cap.

See `DECISIONS.md` for command-design rationale, what was left out, and any
friction against the sibling libraries' APIs.
