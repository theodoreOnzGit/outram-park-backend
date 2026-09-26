# kovan-common

**KOVAN** — **K**nowledge **O**riented **V**&V **A**nalysis for **N**uclear
science and engineering. This crate is part of KOVAN: the shared canonical
types every other `kovan-*` crate speaks in.

> ⚠️ **Research, education and V&V only.** Not for nuclear facility operation,
> reactor control, licensing, safety-critical decisions or emergency response.
> See the workspace `RESPONSIBLE_USE.md`.

A plain data crate: types, `serde` derives, a builder and a few convenience
constructors, with no pipeline logic. Cross-crate links (a symbol pointing at a
document, a benchmark pointing at a validation case) are carried as the string
IDs defined here rather than as crate-to-crate dependencies. These Rust structs
are the source of truth; BibTeX, TOML and Markdown metadata are generated from
them, never the other way round.

## What it provides

Everything is re-exported at the crate root (`kovan_common::KovanDocument`).

| Module | Types |
|---|---|
| `document` | `KovanDocument`, `KovanDocumentBuilder`, `Author`, `Visibility`, `DocumentType` |
| `symbol` | `KovanSymbol`, `KovanRepository`, `Language` |
| `knowledge` | `KovanCorrelation`, `KovanBenchmark`, `KovanValidationCase`, `GeneratedArtifact` |

Unlike its sibling pipeline crates, this one has no stub functions: `src/lib.rs`
states that every public type is implemented and round-trip tested through
`serde_json` and `toml`. Design choices (for example `#[serde(default)]` on the
`Vec` fields of `KovanDocument`, so older stored documents still parse) are
recorded in [`DECISIONS.md`](DECISIONS.md).

Dependencies: `serde`, `serde_json`, `toml`.

## Example

```bash
cargo run -p kovan-common --release --example build_document
```

The public API mirror is [`docs/kovan-common-api.md`](docs/kovan-common-api.md).

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
