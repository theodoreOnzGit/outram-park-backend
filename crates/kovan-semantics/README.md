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
