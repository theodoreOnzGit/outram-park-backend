# The code index: `kovan.toml` and `kovan_links.json` (GitHub #767)

> ⚠️ **Research, education and V&V only.** Not for nuclear facility
> operation, reactor control, licensing, safety-critical decisions or
> emergency response. See the workspace `RESPONSIBLE_USE.md`.

`kovan-cli index` builds the data the code review (#739, #740) runs on:

| File | Where | What | Owner |
|---|---|---|---|
| `kovan.toml` (`kind = "code_folder"`) | every folder with indexed `.rs` files | per file, per function: stable id, location, hashes, callees, reaching tests, physical interface; the folder's `[test_run]`, cached `[upstream]` and `[[review]]` list; the crate root's deleted-folder history | machine (regenerated, never hand-edited), **committed** |
| `kovan_links.json` | each crate's folder | every identifier occurrence that links to a definition: go-to-definition and find-references without rust-analyzer (#745) | machine, ships in the crates.io package |
| `review.md` | the folder | reviews, needs-fix, highlights, upstream confirmation | **human**: the index only reads it |

The code is `kovan_common::code_index` (pure, wasm-clean) and
`kovan::commands::index` (the files and processes).

## Running it

```text
kovan-cli index                      # rust-analyzer scip (3-5 min), then everything
kovan-cli index --scip <file>        # reuse an index written earlier
kovan-cli index --crates a,b         # restrict (cross-crate callees then need the whole run)
kovan-cli index --check              # write nothing; fail if any file would change (CI)
kovan-cli index --draft-upstream     # also print proposed [upstream] review.md entries
kovan-cli index --pin-rust-analyzer  # pin the installed version in kovan_root.toml
kovan-cli index --refresh            # NO rust-analyzer: hashes of edited files now,
                                     # what cannot be recomputed marked "index out of date"
```

**rust-analyzer is needed only to regenerate the index**, never to use it
(maintainer, #767): desktop kovan, web-kovan and `kovan-cli` read the
committed files. Without rust-analyzer (and without `--scip`) `kovan-cli
index` stops with a typed error before reading or writing anything, and
points to `--refresh`.

**rust-analyzer version.** `kovan_root.toml` `[code_review] rust_analyzer`
pins it (#739 D4). A different installed version warns and the index is
regenerated with it; a `--scip` file written by another version than the
installed one is regenerated. The link file records the version that
wrote it (`generator`).

## Pipeline (full run)

```text
 rust-analyzer scip ──> index.scip (target/, never kept in git)
        │
        ├──[links::build]──────────────> <crate>/kovan_links.json
        └──> call graph (#757) ─┐
 source .rs ──[syn hasher]──────┤
 review.md (read only) ─────────┼─[folders::build]──> <folder>/kovan.toml
 previous kovan.toml ───────────┤   ids, carried [test_run]
 git log --diff-filter=D ───────┘   crate root: [[deleted_folder]]
```

Everything is computed in memory before the first file is written.

## Stable ids

`fn:<16 hex>`, the join key with `review.md` (`[kovan] target`). In order
(`kovan_common::code_index::ids`):

1. a `review.md` entry's id goes to the function its `path` names today;
2. else the previous `kovan.toml`'s id for the same path;
3. else a `review.md` id (an entry's, or a key of a review's
   `[review.callees]`) goes to the one function with the hash it recorded
   (moved or renamed without an edit; several candidates: ambiguous,
   nothing taken, reported);
4. else the previous `kovan.toml`'s id by hash;
5. else minted, `kovan_common::review::id::mint_fn_id(path, hash, commit)`.

So the ids a review depends on come back from `review.md` alone if every
`kovan.toml` is lost (code unchanged). A `review.md` speaks only for its own
folder: an entry whose path is in another folder (a copied fixture) is
ignored.

## Self-healing

A folder's `kovan.toml` is rewritten **without asking** when it is missing,
malformed, conflict-marked, stale or hand-edited; an orphan (written for a
folder that no longer has indexed code: its `dir` names that folder) is
removed. Every write and removal is listed with its reason (Leak Before
Break). Never touched: a literature `kovan.toml` (`kind = "paper"`, …) and a
code-folder `kovan.toml` written for another folder (a test fixture).

**`[test_run]` is carried over** (test evidence lives in `kovan.toml`,
maintainer on #766): when a file is unreadable its `[test_run]` is
recovered from `git show HEAD:<path>`; when that fails too it is left out,
which reads as **pending**, never as passed.

## Test evidence

`kovan-cli test` writes a **counted** run (full suite, clean tree, every
binary finished) into every code-folder `kovan.toml`'s `[test_run]`,
restricted to the tests that folder uses; libtest names are mapped to test
ids with #766's rule over the `kovan.toml` files themselves, so no
rust-analyzer is needed. Every run's raw record goes to
`target/kovan/test_evidence_last.toml`. ~~`kovan_test_evidence.toml`~~ is
no longer written.

## `kovan.toml` format additions (additive, schema 1)

- `index_out_of_date` (per function): written by `--refresh` for a new or
  edited function whose callees and reaching tests could not be recomputed.
- `physical_interface` (per function): a uom quantity, a `uom::` path or a
  workspace alias of one (`pub type HeatFlux = HeatFluxDensity;`) appears
  in the signature (`kovan_common::code_index::physical`; token test,
  limits in its doc).
- `test_ids` (folder, **on disk only**): every test id once; `reached_by`
  and `[test_run]` lists hold `"#<index>"` into it. `FolderIndex::parse`
  expands them, so in memory the lists hold full ids. On
  `njoy-outram-park-fork/src/reconr` the full ids had made 1.54 MB of a
  1.59 MB file.

Test ids are the call graph's path ids of test functions
(`crates/x/tests/t.rs::name`), as #766's evidence names tests, not `fn:`
ids. `commit` is not written: an index that records the commit it was built
at changes on every commit.

A real excerpt (`crates/changi/src/flexpart/kovan.toml`, 2026-10-07):

```toml
schema_version = 1
kind = "code_folder"
crate = "changi"
dir = "crates/changi/src/flexpart"
test_ids = [
    "crates/changi/src/activity/chi_over_q.rs::a_fixed_stability_class_overrides_the_wind_derived_one",
    …
]

[module."thermo.rs"]
path = "crate::flexpart::thermo"

[[module."thermo.rs".function]]
id = "fn:5906055d16b77c5c"
name = "saturation_vapour_pressure"
qual = "saturation_vapour_pressure"
lines = [22, 55]                 # written one number per line
hash = "sha256:1e9515aa…"
doc_hash = "sha256:979525d4…"
callees = ["fn:bafb50166fd05428"]
physical_interface = true

[[module."thermo.rs".function]]
id = "fn:bafb50166fd05428"
name = "ew_kelvin"
…
reached_by = ["#57", "#59", …]
```

## `kovan_links.json` format (schema 1)

Documented in full in `kovan_common::code_index::links`. In short:
`files[]` (crate-relative path, `sha256` of the text it was built from, and
the occurrences as flat `[line delta, column, length, definition]`
quadruples), `ext[]` (other files holding a referenced definition) and
`defs[]` (flat `[file, line, column]` triples). Lines are 0-based, columns
in `encoding` units (UTF-8 bytes from rust-analyzer). Local variables are
included; definition sites and references into std or dependencies are not.
A reader compares a file's `hash` with the text it has and shows "links out
of date" for an edited file. `LinkIndex::{definition_at, references,
is_current}` answer the three questions; web-kovan will consume it (#745).
JSON, not a binary layout, so web-kovan reads it with `serde_json`; it is
all integers, so the gzipped crates.io package carries it cheaply.

## Measured (2026-10-07, rust-analyzer 1.98.0, 16-core desktop)

`kovan-cli index --crates njoy-outram-park-fork,outram-mc-libs,changi`:
`rust-analyzer scip` over the whole workspace 291.9 s (4,402 documents,
3,089,056 occurrences, 103,516 symbols); from that index the whole run
(links, call graph of 7,354 functions, hashes, `kovan.toml`) **5 s**.

| crate | `kovan_links.json` raw | gzip -9 | `kovan.toml` files | raw | gzip -9 |
|---|---|---|---|---|---|
| njoy-outram-park-fork | 2.28 MB | **0.58 MB** | 48 | 3.06 MB | 0.56 MB |
| outram-mc-libs | 1.70 MB | 0.43 MB | 23 | 1.68 MB | 0.40 MB |
| changi | 0.56 MB | 0.15 MB | 7 | 0.47 MB | 0.11 MB |

The njoy link file meets the ≲1 MB gz target. Before the `test_ids`
table, njoy's `kovan.toml` files were 11.3 MB raw (0.83 MB gz).

Same index, second run: `--check` passes (byte-identical). `--refresh
--check` on the unchanged tree: 77 of 78 files identical; the one
difference is three trait-method declarations with multi-line signatures
in `outram-mc-libs/src/geometry/crossing/mod.rs` that the call graph's
source scanner does not list (so the full run leaves them out) and the
`syn` hasher does (so a refresh adds them, marked out of date). Tracked
as a call-graph scanner gap, #781.

Also reported by that run: 25 functions with no hash (nested in another
`fn` or made by a macro; their calls count for the enclosing function) and
183 callees outside the three crates (left out of `callees` in a scoped
run). `physical_interface` was set on 4 of 3,404 njoy functions, 0 of 2,977
outram-mc-libs and 58 of 948 changi functions.

## Known limits

- Functions the call graph does not list (bins, benches, `build.rs`,
  nested `fn`s, some multi-line trait declarations) are not in `kovan.toml`.
- `reached_by` is a lower bound (resolved calls only: #746).
- A scoped run does not know the ids of out-of-scope callees unless a
  claim or the previous index names them; run the whole workspace.
- `--refresh` learns quantity aliases from the folder only; unchanged
  functions keep the full run's answer.
- Reach is computed at the current commit only. The reach at an older
  commit is the committed `kovan.toml` at that commit (`git show
  <commit>:<folder>/kovan.toml`, `reached_by`), since the files are
  committed.
