---
name: kovan-cli
description: Use when you need token-frugal repository understanding, literature/PDF cataloguing, numerical-method code generation, or graph digitisation in the OUTRAM PARK workspace, driven non-interactively from a script or coding agent.
---

# kovan-cli skill

## What kovan-cli is

`kovan-cli` is the agent-facing front end to KOVAN, this workspace's own
deterministic knowledge layer: repository discovery/search/scan, a
ripgrep-first symbol/outline extractor, a PDF literature pipeline, numerical
method code generation, per-commit token accounting, and a fully automatic
graph digitiser. Every subcommand is non-interactive, offline, and
deterministic — plain flags in, line-oriented (or `--json`) output out.

## CRITICAL agent guidance (read first)

- **Only run `kovan-cli` from an agent.** The other two binaries in this
  crate are INTERACTIVE and will HANG a non-interactive session: `kovan` is
  the GUI (an `eframe` window — the process never returns without a display
  and a human closing it), and `kovan-tui` is a full-screen `ratatui`
  terminal UI (it owns the terminal and blocks on keyboard input). Neither
  has a script-friendly mode. If a task looks like it needs the digitiser or
  PDF reader, use `kovan-cli digitise` (fully automatic) instead of reaching
  for the GUI.
- **Prefer the `cost -> outline -> slice` loop over reading a whole file
  blind.** Check what a file would cost first, read only its declarations if
  that's enough, and slice out just the lines you actually need otherwise.
- **`lit`/`digitise` are the literature and graph-digitisation paths** — see
  their recipes below rather than reading a PDF's raw bytes or eyeballing a
  figure.

## Recipes

Estimate a file's token cost before deciding whether to read it whole:

```bash
kovan-cli cost src/big_module.rs
kovan-cli cost src/big_module.rs --by-line   # which lines dominate the cost
```

Get a declarations-only skeleton of one file instead of reading it whole:

```bash
kovan-cli outline src/big_module.rs --lang rust
```

Read only a line range once the outline says where the part you need is:

```bash
kovan-cli slice src/big_module.rs 120 180
```

Rust-analyzer-backed semantic queries (need `rust-analyzer` on PATH; slower
than the three above, since they spawn and index a real language server —
expect the first call in a workspace to take up to a couple of minutes):

```bash
kovan-cli def foo --file src/lib.rs      # where it's defined, plus its signature
kovan-cli sig foo --file src/lib.rs      # just the signature
kovan-cli refs foo --file src/lib.rs     # every reference site, as coordinates
```

Discover files under a root, honouring `.gitignore`:

```bash
kovan-cli discover --root . --kind source
```

Regex search a single file or a whole repository:

```bash
kovan-cli search --path src/lib.rs --pattern 'fn \w+'
kovan-cli search --root . --kind source --pattern 'fn \w+'
```

Catalogue a repository's symbols, or render the full Markdown artifact:

```bash
kovan-cli symbols . --lang rust
kovan-cli symbols . --lang rust --markdown --out symbols.md
kovan-cli summary . --lang rust --out repository-summary.md
```

Import a PDF into the literature archive and get its BibTeX entry:

```bash
kovan-cli lit import paper.pdf --json-out doc.json --markdown-out doc.md
kovan-cli lit bibtex doc.json
kovan-cli lit outline paper.pdf
```

Generate numerical-method source from the codegen catalogue:

```bash
kovan-cli methods
kovan-cli gen root newton-raphson
```

Digitise a plot fully automatically (never do this by eyeballing a figure):

```bash
kovan-cli digitise --image fig7.png --x-scale log --x-range 1,1e6 \
    --y-scale log --y-range 0.1,10 --figure "Fig. 7" --json fig7.json
```

The emitted dataset is always `UNREVIEWED` — a human marks it reviewed in
`kovan-tui`'s Digitiser tab, never from an agent session.

## Command reference

| Command | Agent-safe | Notes |
|---|---|---|
| `cost` | yes | Token-cost estimate (`kopitiam-tokenizer`-backed) |
| `outline` | yes | Declarations-only skeleton, ripgrep-first |
| `slice` | yes | Print one line range |
| `def` / `sig` / `refs` | yes, but needs `rust-analyzer` | Semantic queries (definition/signature/references) |
| `discover` / `search` / `scan` | yes | Repository discovery/search |
| `symbols` / `summary` | yes | Symbol catalogue / Markdown artifact |
| `lit` | yes | PDF import / BibTeX / literature outline |
| `gen` / `methods` | yes | Numerical-method code generation |
| `digitise` | yes | Fully automatic graph digitiser |
| `tokens` / `historian` | yes | Per-commit API-token accounting |
| `agent-docs-gen` / `api-docs` / `kloc` | yes | Documentation/accounting generators |
| `setup` | yes, but online | Installs external tools via `cargo install` |
| `kovan` (GUI) | **NO — hangs** | Interactive `eframe` window |
| `kovan-tui` | **NO — hangs** | Interactive full-screen terminal UI |
