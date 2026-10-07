# Call graph: SCIP backend against the LSP backend (GitHub #757)

`kovan-cli call-graph` resolves every call-shaped token of every function
body to its definition. Until #757 it asked rust-analyzer's LSP, one
definition query per token (`--backend lsp`). `--backend scip` instead runs
`rust-analyzer scip` once over the whole workspace and answers the same
queries from that index in memory. This note records how the two compare
and what each costs.

## Methodology

**What is computed.** For one commit of this workspace, the call graph of
every member crate (48 crates, lib, examples and integration tests) is built
twice with the same binary:

1. `kovan-cli call-graph --backend lsp -o ws_lsp.json`, from a cold
   rust-analyzer (the keep-warm daemon stopped first);
2. `kovan-cli call-graph --backend scip -o ws_scip.json`, which runs
   `rust-analyzer scip` into `target/kovan-scip/index.scip` and builds from
   it.

Both runs take the function list, ids, signatures and every other field from
the same source scanner, so a function id names the same function in both
documents. Only where calls resolve differs.

**Instrument.** `kovan-cli call-graph-diff ws_lsp.json ws_scip.json`
(`kovan_common::call_graph::compare`). It matches edges by
`(caller id, callee id)`, compares their call-site lines and kinds, and
matches unresolved calls by `(function, line, kind)`. The LSP graph is the
reference (A), because it is the graph the review UI has used since #737.

**Why this instrument.** The question is whether SCIP can replace the LSP
as the source of call edges without losing any. So the first number is the
share of the reference's edges that SCIP reproduces (recall of A). Every
edge in only one graph is then listed and explained one by one. A
percentage alone would hide a systematic miss.

**Pass criterion (set before the run).** Every LSP-only and SCIP-only edge
is explained by a known difference between the backends; none is an
unexplained miss.

**Inputs.** Commit `c5788e79c2` (2026-10-07), rust-analyzer
`1.98.0 (88d9e12 2026-08-18)`, 16-core desktop with other agents building
at the same time (so the timings are indicative, not benchmarks).

## Results

Measured 2026-10-07. The LSP graph was built once by commit `c5788e79c2`.
The SCIP graph is from the final resolver (commit `d08cf4af66`, after
fixes 4 and 5 listed below), rebuilt over the **same source text** and
the **same index** (the follow-ups touch only `scip.rs` and `builder.rs`;
those two files were checked out at `c5788e79c2` for the rebuild, so every
line number matches the index).

### Wall time, whole workspace (48 crates, 43,232 functions)

| | LSP backend | SCIP backend |
|---|---|---|
| rust-analyzer work | 480,454 definition queries, cold start included | `rust-analyzer scip`: **221.8 s**, 348.6 MB index |
| building the graph | (included above) | decode 0.4 s; 480,454 in-memory lookups; ≈ 20 s with everything else |
| **total** | **1,953.2 s** (32.6 min) | **245.6 s** (4.1 min) |
| from an existing index | n/a | **20.3 to 22.2 s** |

On the three crates #757 measured first (kovan-common,
tampines-steam-tables, outram-park-digital-twin-engine; 6,155 functions):
LSP 172.7 s from cold; SCIP 1.9 s from the existing index. A fresh index
costs the whole ~222 s whatever the scope, because `rust-analyzer scip`
always indexes the whole workspace.

The LSP figure is about twice the 941 s #757 recorded earlier the same day,
because other agents were building on the same machine. Both runs here
shared those conditions. Peak memory was not measured.

### Edges

| | count |
|---|---|
| LSP edges | 101,391 |
| SCIP edges | 103,212 |
| in both | **101,018** |
| of those, with identical call-site lines | 100,955 |
| of those, with a different call kind | 0 |
| **recall of the LSP edges** | **99.66 %** |
| LSP only | 343 |
| SCIP only | 2,107 |

The 63 shared edges whose lines differ all gain sites in SCIP: the same
callee is also passed as a path elsewhere in the caller.

**The 2,107 SCIP-only edges** are calls the text scanner never sees:

- **1,711 operator calls** (new kind `operator`): `a + b`, `v[i]`,
  `x += y` on workspace types (`Field`, `Vector3`, `FvMatrix`, `Tensor`).
  A sample of 15 was read by hand, and all are real calls.
- **396 functions passed as paths, or passed bare on a line the scanner did
  not take as an argument** (kind `fn_value`): `.map(HeldOut::error_pcm)`,
  `t2a_ph as fn(f64, f64) -> f64`, and test tables passing `all, evaluate,`
  one per line. A sample of 15 was read, and all are real.

**The 343 LSP-only edges** are of five kinds. None is a call SCIP resolves
wrongly. Each is a place where rust-analyzer's go-to-definition does
something extra that an index of references does not record:

| LSP only | edges | why |
|---|---|---|
| blanket-impl jumps: `.into()` → `From::from`, `.parse()` → `FromStr::from_str`, `.try_into()` → `TryFrom::try_from` | 154 | go-to-definition follows std's blanket impl to the user's impl; SCIP records the reference to std's `Into::into` (external) |
| `.to_string()` → the type's `Display::fmt` | 103 | the same blanket-impl jump |
| functions defined inside a `macro_rules!` body (dhoby-ghaut `monte_carlo_web/rungs.rs`) | 56 | the LSP points into the macro's text; SCIP has no definition for macro-generated items, so the call is `UNRESOLVED(other)` |
| files pulled in with `include!` (kovan-codegen's `templates/*.rs`) | 20 | the included file is not an index document |
| other | 10 | 5 calls from a test into a binary's file included with `#[path]` (indexed only once, as the binary); 5 calls through the extension trait `SurfaceKindExt` in outram-mc-libs, which SCIP records as the trait declaration (`UNRESOLVED(trait)`) |

### Unresolved calls (matched by function, line and kind)

| kind | LSP | SCIP | both |
|---|---|---|---|
| closure | 10,731 | 10,730 | 10,730 |
| macro | 178 | 178 | 178 |
| no-definition | 2,240 | 2,247 | 2,240 |
| other | 4,431 | 4,344 | 4,316 |
| trait | 881 | 882 | 881 |

`other` differs mostly by the derived-`Display` side of the `.to_string()`
jump (the LSP marks it `other`, SCIP drops it as a std call). SCIP's own 28
`other` cover the macro-generated and `include!`d items above.

### Reach (a downstream check)

Non-test functions reached by at least one test: 19,855 (LSP), 19,955
(SCIP). By at least one example: 6,126, against 6,202. The operator and
path edges extend reach; nothing reachable before became unreachable.

### Interpretation

**Pass.** Every difference is explained and none is a SCIP miss of a call
the source writes. The comparison also found **five defects**. All are fixed
in this change:

1. **(LSP backend, existing)** A call through a function-typed parameter
   was a false recursive edge to the enclosing function. Both backends now
   give `UNRESOLVED(closure)`.
2. **(LSP backend, existing)** The unresolved calls of integration-test
   functions were collected and then dropped (`c.targets` only). They are
   kept now.
3. **(SCIP, first draft)** A symbol in both the library and an example
   (rust-analyzer's symbols name the package, not the target) took the
   example's copy even when written `my_crate::prelude::run`. A path through
   the package's own library name now picks the library (bins, examples,
   tests and benches excluded).
4. **(SCIP, first draft)** A file indexed twice kept one copy and lost the
   other copy's definitions. The copies are merged now.
5. **(SCIP, first draft)** Helpers of one name nested in different functions
   of one file share a symbol, and the first was always taken (66 wrong
   edges). A definition at the queried position now answers with itself.
   A same-file collision prefers a definition visible from the caller
   (not nested in another function), then one nested in the caller, then
   the nearest line.

The SCIP backend therefore reproduces the LSP graph's resolved calls, adds
about 2 % more, and takes 4 minutes instead of 33 for the whole workspace,
or 20 seconds when an index exists. Its fixed cost is the index (≈ 222 s
even for one crate), so the LSP backend stays the cheaper choice for a
quick look at one or two crates.

**Which number to quote:** 99.66 % recall of the LSP edges, with every one
of the 343 misses explained above. The 97.5 % Jaccard figure counts SCIP's
extra real calls against it, so it is not the measure of agreement.

### How to reproduce

```bash
cargo build --release -p knowledge-oriented-vv-analysis-for-nuclear-sciences-kovan --no-default-features --bin kovan-cli
kc=target/release/kovan-cli
$kc lsp-daemon-stop --root .
$kc call-graph --workspace . --backend lsp  -o ws_lsp.json     # ~16-33 min
$kc call-graph --workspace . --backend scip -o ws_scip.json    # ~4 min; index in target/kovan-scip/
$kc call-graph-diff ws_lsp.json ws_scip.json --list 20
```

The regression tests are `crates/kovan/tests/call_graph_scip.rs` (both
backends on a throwaway workspace, with one case per category above), the
unit tests in `crates/kovan/src/scip.rs`, and
`crates/kovan-common/src/call_graph/compare.rs`.
