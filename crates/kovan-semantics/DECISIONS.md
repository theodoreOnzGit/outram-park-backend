# kovan-semantics — design decisions & status

Records the choices made while fleshing out the four `// TODO(kovan)` stubs in
`kovan-semantics`, per the KOVAN spec (`docs/kovan.md`, "# KOVAN Semantics") and
the workspace + crate `CLAUDE.md` rules. Read alongside the module docs.

## What is real vs stubbed vs deferred

| Capability | Status |
|---|---|
| Ripgrep-first symbol extraction (Rust / C++ / Python / Fortran) | **Real.** `src/extract/*.rs`, deterministic, offline, Android-clean. |
| Normalisation into shared `KovanSymbol` | **Real.** `ExtractedSymbol::into_kovan_symbol` + `catalogue_symbols`. |
| Markdown outputs `symbols.md`, `repository-summary.md` | **Real.** `src/outputs.rs`. |
| `catalogue_symbols_detailed` (location-carrying records) | **Real.** Feeds the Markdown; `catalogue_symbols` maps it to `KovanSymbol`. |
| `rough_definition_scan` (cheap ripgrep pre-filter) | **Real** (was already present; kept + broadened patterns). |
| Language-server adapters (rust-analyzer / clangd / Pyright / fortls) | **Scaffolded, deferred.** `src/adapters/mod.rs`, behind `language-servers` feature + `cfg(not(target_os="android"))`. Invocation *contract* is described; the LSP client itself is `// TODO(kovan)` returning `Unimplemented`. |
| `validation-links.md`, `dependency-graph.md`, module/validation graphs | **Not started.** Need the literature/graph layers; out of scope here. |
| Tree-sitter | **Not used** and not added (KOVAN "Tree-Sitter Policy" — last resort only). |

## Regex-extraction approach

`catalogue_symbols_detailed` = discover source files with
`kovan_discovery::discover_kind` (the `.gitignore`-aware `fd`/ripgrep walker) →
filter to the adapter's extensions → read each file → run a per-language,
line-oriented scanner. Each scanner uses anchored `regex` patterns to name the
symbol and a light **scope stack** to build a best-effort qualified name.

- **Rust** — `fn/struct/enum/trait/type/mod/impl`. Scope stack over `mod` and
  `impl` blocks tracked by textual brace depth ⇒ `module::Type::method`. A trait
  impl records `Trait for Type` but qualifies members by the concrete type.
- **C++** — functions, `class/struct/union`, `namespace`. Scope stack over
  namespaces and records by brace depth (same-line `{` or next-line `{`, the two
  dominant styles). Functions split into a *definition* regex (`… name(args) {`)
  and a *prototype* regex (`ret name(args);`, return type mandatory) with a
  control-keyword blocklist.
- **Python** — `def`/`class`. Scope via **indentation** (the one place Python is
  unambiguous) ⇒ dotted `Class.method`, `outer.inner`.
- **Fortran** — `module/subroutine/function/type`, case-insensitive. Scope via
  matched `end <kw>` keywords ⇒ `module::subroutine`.

### Known limits, per language (documented in code too)

- **Shared:** brace/keyword counting is textual — braces inside string/char
  literals or block comments (`/* */`) are counted, so exotic formatting can
  mis-nest a *qualified* name. The bare `name` and `kind` are always correct.
  Multi-line signatures are read from their first (keyword) line.
- **C++ (weakest, by design):** the prototype path cannot distinguish a function
  declaration `T f(args);` from a variable declaration with a parenthesised
  initialiser `const bool x(y.z());` — C++'s "most vexing parse", which needs
  real semantics. Empirically this produced 4 such false positives in GeN-Foam's
  `GeN-Foam.C` (all `const bool name(...)` locals). Macro-hidden definitions
  (heavy in OpenFOAM) are invisible. This is exactly what the `clangd`
  escalation path exists to supersede. Definitions ending in `{` are reliable.
- **Fortran:** free-form friendly; fixed-form column rules and `&` line
  continuations are not modelled. `module procedure/subroutine/function` and
  `type(kind) :: var` declarations are correctly *excluded* (post-filter, since
  the `regex` crate has no look-around — see below).
- **Python:** decorators skipped (the `def`/`class` line is the location);
  dynamically created defs (`type()`, `exec`) are invisible. Indent width is raw
  leading-whitespace length (a tab counts as one column).

### Empirical spot-check (real code, not fabricated)

Ran the extractor against in-repo fixtures (read-only) on 2026-07-15:

- `njoy-outram-park-fork/…/NJOY2016/src/acecm.f90` → 15 symbols: `module acecm`
  + 10 subroutines + 4 functions, all correctly qualified `acecm::…`. Correct.
- `outram-foam-appbuilder-lib/…/GeN-Foam/GeN-Foam.C` → 4 symbols, all the
  most-vexing-parse false positives noted above (this `main()` file has no
  top-level defs). Illustrates the C++ precision limit.
- `outram-park-fork-coolprop/dev/gen_fluid.py` → 13 functions incl. nested
  `build_friction_theory.arr` / `.num`. Correct.

## Language-server scaffolding + gating decision

- New cargo feature **`language-servers`**, `default = []` (off). The whole
  `adapters::language_servers` submodule is additionally source-gated to
  `cfg(all(feature = "language-servers", not(target_os = "android")))`, so the
  default build *and every Android build* compile it away. Verified:
  `cargo check --target aarch64-linux-android` is clean both with and without
  the feature.
- `LanguageAdapter::server_binary()` is always available (just a string).
  `language_server_invocation()` (feature-gated) describes the argv/cwd each
  server *would* launch with, without spawning anything.
  `catalogue_symbols_via_server()` (feature-gated) is the `// TODO(kovan)` entry
  point that will host the LSP client (`initialize` → `workspace/symbol` →
  normalise into `KovanSymbol`). It returns `Unimplemented` today.
- The heavy in-process `ra_ap_*` / `libclang` integrations are **not** added
  (Android-hostile, deferred). No new heavyweight deps entered `Cargo.toml`.

## Dependency change (needs a glance in review)

Added `regex = "1"` to the **root** `[workspace.dependencies]` and consumed it
via `regex.workspace = true`. Rationale: pure-Rust, Android-friendly, no system
libs, and far more reviewable than hand-rolled string parsing or the
`grep-matcher` capture API. `grep-regex` already pulls `regex-automata`/
`regex-syntax` transitively, so the added weight is minimal. Kept off the
critical KOVAN "avoid" list (no SQLite/Tantivy/vector/LLM).

Note: the `regex` crate has **no look-around** (`(?!…)`). Two patterns that
originally used negative look-ahead (Fortran `module` / derived-`type`
exclusions) were rewritten to post-filter the captured name instead.

## Subagents

**Not used.** The four extractors share one `ExtractedSymbol`/`SymbolKind`
contract and a common scope-stack idiom; writing them directly kept the API and
the brace/scope conventions coherent and avoided merge drift across a small,
tightly-coupled surface. Noted here per the deliverable checklist.

## Needs from `kovan-common` (for human review — not applied; that crate is off-limits here)

1. **`KovanSymbol` should carry source location + language.** Today it is
   `{ id, qualified_name, kind, repository_id }` — no file/line/language. The
   extractor's richer `ExtractedSymbol` (in `kovan-semantics`) carries
   `file: PathBuf`, `line: u64`, `language`. Proposed additions to
   `KovanSymbol`: `file: String` (repo-relative), `line: u32`, and either a
   `language: String` or a shared `Language` enum. This would let
   `catalogue_symbols` return locations directly and let `symbols.md` be
   generated from `KovanSymbol` alone (currently it needs `ExtractedSymbol`).
2. **Consider a canonical `KovanModule`** type (the spec's "Shared Semantic
   Model" lists it) — `kovan-semantics` already models modules only as a
   `SymbolKind::Module` symbol.
3. **`kovan_common::KovanSymbol::kind` is free-text `String`.** Fine for now;
   `kovan-semantics` owns the closed `SymbolKind` enum and stringifies via
   `as_str()`. If common ever adopts an enum, reuse this one.

No changes requested to `kovan-discovery`; its `discover_kind` + `search_file`
were sufficient. (Minor future nicety: a `discover_kind` variant filtered to a
specific extension set would save the in-crate extension re-filter, but it is
not needed.)

## Open questions for a human

- Should the C++ prototype path be **off by default** (definitions-only, higher
  precision) given the most-vexing-parse noise on OpenFOAM? Currently on because
  header files are mostly prototypes and are worth cataloguing.
- Symbol IDs currently embed the repo-relative `file:line`, so they are stable
  across re-scans but **move when code moves**. If KOVAN wants IDs stable across
  edits, a content/name-based scheme is needed (needs the common-type decision).
- Intended follow-up beads (KOVAN epic `op-5v5` is JSONL-only / not in local
  Dolt, so **not** filed there per instructions — recorded here instead):
  - *op-5v5.sem-lsp*: implement the LSP client behind `language-servers`
    (rust-analyzer first) and normalise `workspace/symbol` into `KovanSymbol`.
  - *op-5v5.sem-loc*: add location/language fields to `kovan_common::KovanSymbol`
    (see "Needs from kovan-common"), then simplify `catalogue_symbols`.
  - *op-5v5.sem-graph*: `dependency-graph.md` / module + validation graphs
    (needs `petgraph` + the literature layer).
  - *op-5v5.sem-cpp*: reduce C++ most-vexing-parse false positives (initialiser
    heuristics) or gate the prototype path.
