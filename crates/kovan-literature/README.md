# kovan-literature

**KOVAN** — **K**nowledge **O**riented **V**&V **A**nalysis for **N**uclear
science and engineering. This crate is part of KOVAN: its literature archive
and the PDF-to-BibTeX pipeline.

> ⚠️ **Research, education and V&V only.** Not for nuclear facility operation,
> reactor control, licensing, safety-critical decisions or emergency response.
> See the workspace `RESPONSIBLE_USE.md`.

The library turns a source PDF into the canonical `KovanDocument` (from
`kovan-common`) and derives Markdown, BibTeX and extracted image assets from
it:

```text
PDF → Markdown → KovanDocument → BibTeX → generated knowledge artifacts
```

The `KovanDocument` is authoritative; BibTeX and generated Markdown are always
derived from it. Every function is deterministic and runs fully offline, with
no network, cloud or OCR service. Text extraction uses the pure-Rust
`pdf-extract` crate and the PDF object model uses `lopdf`. Both build for
`aarch64-linux-android`.

## What is implemented and what is best-effort

Per `src/lib.rs`:

| Function | Status |
|---|---|
| `pdf_to_markdown`, `markdown_outline`, `to_bibtex` | Implemented and tested |
| `extract_metadata` | Best-effort: the PDF Info dictionary first, then conservative text scanning. Unknown fields are left empty rather than guessed. |
| `extract_assets` | Extracts embedded images already stored as standalone files (JPEG via `DCTDecode`, JPEG-2000 via `JPXDecode`). Images under other filters are reported as skipped, not re-encoded. |

The graph digitiser is **not** in this crate. It moved to `kovan` on
2026-08-21 (`kovan::digitiser`) so this crate could stay GPL-3.0-only.

## The archive

This crate is also the on-disk literature archive. **It holds no PDFs.** Since
2026-09-22 the open PDFs are in the `reactor-literature/` Git submodule
(fetch it with `git submodule update --init crates/kovan-literature/reactor-literature`),
and proprietary documents are kept outside this public repository. The crate
keeps each open document's metadata JSON and extracted Markdown under `open/`,
generated Markdown and BibTeX under `generated/`, and datasets extracted from
documents, with their provenance, under `derived/`.

- [`CATALOGUE.md`](CATALOGUE.md) is the record of every document: what it
  is, its access tier and why, and where its files live.
- [`CLAUDE.md`](CLAUDE.md) holds the rules for adding or moving a document.
  The access tier comes from the document's own licence, never from where it
  is hosted (workspace `DATA_POLICY.md`).

Only `generated/bibtex/open/` ships in the published crate. The PDFs,
Markdown, `derived/` data and the submodule are repository-only (see the
`exclude` list in `Cargo.toml`).

## Example

```bash
cargo run -p kovan-literature --release --example bibtex
```

From the CLI: `kovan-cli lit`. The public API mirror is
[`docs/kovan-literature-api.md`](docs/kovan-literature-api.md).

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

## License

GPL-3.0. Part of the [OUTRAM PARK](../../README.md) workspace.
