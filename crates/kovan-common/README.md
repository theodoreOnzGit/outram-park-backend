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
| `code_map` | the workspace code map: `CodeMap`, `layout`, `svg` (moved from `kovan`, 2026-10-06) |
| `call_graph` | the call graph `CallGraphDoc` (schema 2), `split` for the web, `merge` (moved from `kovan`, 2026-10-06) |
| `mindmap_view`, `geometry`, `fuzzy` | star layout and canvas arithmetic, `Point`/`Bounds`, the finders' scorer (moved from `kovan`, 2026-10-06) |
| `anchoring` | robust annotation anchoring ported from Hypothesis (#754): W3C `TextQuoteSelector` / `TextPositionSelector` and Hypothesis's `PageSelector`; `describe`/`anchor` for plain text, `describe_in_pages`/`anchor_in_pages` for extracted PDF pages; position, then exact quote, then Myers fuzzy quote match |

~~A plain data crate … with no pipeline logic~~ **UPDATED 2026-10-06** (#736):
the last four modules are pure logic (layout, assembly, scoring), moved here
from `kovan` so the wasm Code Review UI (`kovan-web`) can use them; `kovan`
re-exports each under its old path. Still no I/O and no GUI.

Unlike its sibling pipeline crates, this one has no stub functions: `src/lib.rs`
states that every public type is implemented and round-trip tested through
`serde_json` and `toml`. Design choices (for example `#[serde(default)]` on the
`Vec` fields of `KovanDocument`, so older stored documents still parse) are
recorded in [`DECISIONS.md`](DECISIONS.md).

Dependencies: `serde`, `serde_json`, `toml`.

### Anchoring (GitHub #754, 2026-10-07)

`anchoring` keeps an annotation attached to its text after the text is
edited, the way Hypothesis does. It is a port of the Hypothesis client's
anchoring (`match-quote.ts`, `html.ts`, `pdf.ts`, `types.ts`,
`util/normalize.ts`; BSD-2-Clause) and of the `approx-string-match`
library it uses (Myers' bit-parallel edit-distance search; MIT). Commits
and licences are in [`NOTICE`](NOTICE).

- **Describe**: a text and a `char` range give a position selector and a
  quote selector (the exact text plus 32 characters either side); paged
  text adds a page selector.
- **Anchor**: the position is tried first and accepted only if the text
  there still equals the quote; then exact occurrences of the quote; then
  the fuzzy search (at most `min(256, quote/2)` edits). Candidates are
  ranked by upstream's score: 50 quote, 20 prefix, 20 suffix, 2 nearness
  to the old position, normalised to [0, 1]. The result names the
  strategy and the score, or is `Orphaned`. As upstream, there is no score
  threshold; a caller can apply one.
- **Paged text** ignores whitespace when matching, as upstream's PDF
  path, because re-extraction moves spaces and line breaks.
- Offsets are Unicode scalar values (`char`s), as the W3C model
  specifies; Hypothesis counts UTF-16 code units.
- Not ported: DOM-only selectors (`RangeSelector`, `MediaTimeSelector`,
  `EPUBContentSelector`, `ShapeSelector`) and the NFKD normalisation in
  `translateOffsets`. Deviations are listed in each file's module doc.

V&V: upstream's own tests are ported with the same inputs and expected
outputs (`src/anchoring/tests/upstream_*.rs`), and
`src/anchoring/tests/kovan_uses.rs` checks kovan's three uses (a PDF
quote after re-extraction, a note quote after edits around and inside it,
a function's lines after insertions and light edits), with predictions
written before the run and the measured scores. Two score predictions were
refuted (an insertion of k characters in the context costs about 2k edits
in upstream's fixed-length window); the anchors themselves were all
correct. Nothing in kovan uses this module yet: wiring it into review
stamps, `[[relation]]` anchors or PDF highlights is a maintainer decision.

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

~~GPL-3.0.~~ **AGPL-3.0-only since 2026-10-07**, like all of kovan (see [`NOTICE`](NOTICE)). Part of the [OUTRAM PARK](../../README.md) workspace.
