# kovan-semantics

**KOVAN** — **K**nowledge **O**riented **V**&V **A**nalysis for **N**uclear
science and engineering. This crate is part of KOVAN: it turns a source tree
into a catalogue of symbols and Markdown summaries.

> ⚠️ **Research, education and V&V only.** Not for nuclear facility operation,
> reactor control, licensing, safety-critical decisions or emergency response.
> See the workspace `RESPONSIBLE_USE.md`.

A repository-understanding engine that runs **deterministically and offline**.
It does not reimplement a compiler or build a universal AST. The default tier
finds definition lines with `kovan-discovery`'s ripgrep engine and pulls out
each symbol's name, kind and location with anchored regexes, for Rust, C++,
Python and Fortran. Results are normalised into `kovan-common`'s `KovanSymbol`.
No process is spawned and no network is used, and the tier builds for
`aarch64-linux-android`.

## What it provides

| Capability | Status (per `src/lib.rs` and [`DECISIONS.md`](DECISIONS.md)) |
|---|---|
| Ripgrep-first symbol extraction (`catalogue_symbols`, `catalogue_symbols_detailed`, `extract`) | Implemented. The regex scanners have documented limits per language; C++ is the weakest (the "most vexing parse" gives false positives, macro-hidden definitions are invisible). |
| Markdown outputs: `symbols_markdown` → `symbols.md`, `repository_summary_markdown` → `repository-summary.md` | Implemented |
| `ontology`: a typed graph of engineering *concepts* (`ConceptGraph`, `Relation`), separate from code symbols | Implemented |
| `agent_docs`: bundles the workspace's `docs/<crate>-api.md` mirrors into a flat file set (`AGENTS.md`, `_INDEX.md`, per-crate files) for an external chat agent with a fixed context budget | Implemented |
| Language-server escalation (`rust-analyzer`, `clangd`, Pyright, `fortls`) in `adapters` | **Scaffolded only.** Behind the off-by-default `language-servers` feature and gated off Android; the client is a `// TODO(kovan)` returning `Unimplemented`. |
| `validation-links.md`, `dependency-graph.md` | Not started |

Tree-sitter is not used.

## Example

```bash
cargo run -p kovan-semantics --release --example adapters
```

From the CLI: `kovan-cli symbols . --lang rust`, `kovan-cli summary . --lang rust`
and `kovan-cli agent-docs-gen`. The public API mirror is
[`docs/kovan-semantics-api.md`](docs/kovan-semantics-api.md).

## Zotero: duplicates, relations, merge (`zotero` module, #751)

A port of Zotero's duplicate detection (`duplicates.js`), relations model
(`relations.js`, `dataObject.js`, `uri.js`) and item merge
(`mergeItems.mjs`, which `Zotero.Items.merge` calls), run over
`kovan_common::zotero::ZoteroLibrary` in memory, plus a mapping of Zotero
relations, collections and tags onto kovan's `[[relation]]` and concept
schema, produced as data for a later step to write (no kovan schema
changes).

| Function | What |
|---|---|
| `zotero::find_duplicates` | duplicate sets by ISBN, DOI, normalised title + creators + year |
| `zotero::merge_items` | merge items into a master, as a pure function returning the new library |
| `zotero::merge_pane_order`, `zotero::field_alternatives` | the duplicates pane's choice of master and its per-field alternatives |
| `zotero::relations::*` | predicates, item/collection URIs, add/remove/set, subjects of a URI, `updateUser`, `purge`, linked items |
| `zotero::map_library` | relations, collections, tags -> `paper:`/`collection:` drafts, with every loss recorded |

Upstream Zotero cannot run here and the local translation server does not
expose these functions, so the reference is upstream's own tests, ported in
`tests/zotero_upstream.rs` (77 cases, all passing; see
[`docs/zotero-port.md`](docs/zotero-port.md) for the list, the evidence
model for attachment files, and what is not ported). AI draft, not yet
human-reviewed.

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

~~GPL-3.0.~~ **AGPL-3.0-only since 2026-10-07**, like all of kovan (see [`NOTICE`](NOTICE)). Part of the [OUTRAM PARK](../../README.md) workspace.
