# Crate Documentation

**Version:** 0.0.0

**Format Version:** 60

# Module `kovan_semantics`

# kovan-semantics

A repository-understanding engine for the KOVAN knowledge layer. It turns a
source tree into normalised [`KovanSymbol`]s and human-readable Markdown
knowledge artifacts, **deterministically and offline** (KOVAN's Android-first
mandate — see `docs/kovan.md`, "# KOVAN Semantics").

## Two tiers, one philosophy

Per KOVAN's "Important Philosophy", this crate does **not** reimplement a
compiler or build a universal AST. It works in two tiers:

1. **Ripgrep-first (default, always on, Android-clean).** [`catalogue_symbols`]
   and [`extract`] locate definitions with the ripgrep engine (via
   [`kovan_discovery`]) and pull each symbol's name, kind, and location out
   with anchored regexes. No process spawn, no network, no system libraries.
   This is the floor every build — including `aarch64-linux-android` — gets.
2. **Language-server escalation (deferred, opt-in, non-Android).** The real
   semantic tools (`rust-analyzer`, `clangd`, Pyright, `fortls`) are the
   source of truth once wired. Their integration is scaffolded in
   [`adapters`] behind the off-by-default `language-servers` feature, itself
   source-gated to `cfg(not(target_os = "android"))`. Tree-sitter is a last
   resort only (KOVAN "Tree-Sitter Policy") and is **not** used here.

## Outputs

[`symbols_markdown`] → `symbols.md` and [`repository_summary_markdown`] →
`repository-summary.md`. `validation-links.md` / `dependency-graph.md` are
future work (they need the literature/graph layers).

## Example

```no_run
use kovan_semantics::{catalogue_symbols, symbols_markdown, LanguageAdapter};
use std::path::Path;

let repo = Path::new("crates/tampines-steam-tables");
// `catalogue_symbols` returns shared `KovanSymbol`s carrying file/line and
// language, which the Markdown renderers consume directly.
let symbols = catalogue_symbols(repo, LanguageAdapter::Rust).unwrap();
let md = symbols_markdown("TAMPINES", &symbols);
println!("{md}");
```

## Modules

## Module `adapters`

Language-server escalation scaffolding — the *deferred* high-fidelity path.

KOVAN's "Important Philosophy" (see `docs/kovan.md`, "# KOVAN Semantics") is
to consume semantic information from **mature language tooling** rather than
reimplement a compiler. The ripgrep-first scanner in [`crate::extract`] is the
deterministic, offline, Android-clean floor; this module is where KOVAN would
escalate to the real language servers when higher fidelity is required:

| Language | Server            | Primary target |
|----------|-------------------|----------------|
| Rust     | `rust-analyzer`   | TUAS, TAMPINES, BOON LAY |
| C++      | `clangd`          | OpenFOAM |
| Python   | `pyright`         | OpenMC |
| Fortran  | `fortls`          | NJOY |

## Why this is gated off by default

Driving these servers (especially the in-process `ra_ap_*` Rust-analyzer
crates and `libclang`) is heavyweight and **Android-hostile**. Per the
workspace Android rule, the whole module is compiled only under
`cfg(all(feature = "language-servers", not(target_os = "android")))`. The
DEFAULT build — and every Android build — sees an empty module and stays
ripgrep-only. Turn it on with `--features language-servers` on a desktop
target once the invocations below are implemented.

Nothing here is implemented yet: the invocation *contract* is scaffolded (each
adapter knows its binary and how it would be launched), the actual LSP
handshake is a `// TODO(kovan)`.

```rust
pub mod adapters { /* ... */ }
```

## Module `agent_docs`

Bundle this workspace's public-API documentation into a flat set of files
small enough to hand to an **external chat agent** with a fixed context
budget.

# The problem this solves

A coding agent running *inside* the repository can open any file it likes.
An agent reached through a web chat window cannot: it sees only what was
uploaded, and its context is finite. Two constraints follow, and both are
measured facts about this workspace rather than preferences:

1. **The upload dialog takes files, not folders.** So the output is *flat* —
   one file per crate, no subdirectories. [`write_bundle`] will not create
   one.
2. **The corpus is far larger than the budget.** The thirteen
   `crates/<crate>/docs/api.md` mirrors totalled 5,154,447 bytes when this
   module was written — roughly 1.29 M estimated tokens against a typical
   200 k window, and the largest single crate exceeds that window on its
   own. Copying everything is not a design that can work.

# The shape of the answer

Every bundle carries two things unconditionally:

- **`AGENTS.md`** — the workspace's coding rules, written for a remote agent
  (see [`agents_md`]). Hardcoded, not derived from the repository's own
  `CLAUDE.md`, which is mostly harness policy irrelevant to a chat agent.
- **`_INDEX.md`** — a condensed signature index covering **every** crate that
  has a mirror, so the agent has a map of the whole workspace even when it
  has been given the full text of only a few crates (see [`condense`]).

and then the **verbatim** `api.md` of each crate the caller selected. The
index is what stops the agent inventing APIs for crates it was not given;
the verbatim files are what let it write correct code for the ones it was.

# Determinism

Re-running over unchanged inputs produces byte-identical output. Crates are
ordered by directory name, counts are accumulated in [`BTreeMap`]s, and
nothing here writes a timestamp, a hostname, or an absolute path into a
generated file. `agent_docs::tests::the_bundle_is_byte_identical_on_a_rerun`
is the gate on that, because a generator that quietly stops being
reproducible still looks like it works.

[`BTreeMap`]: std::collections::BTreeMap

```rust
pub mod agent_docs { /* ... */ }
```

### Modules

## Module `agents_md`

The hardcoded `AGENTS.md` uploaded alongside the API documentation.

# Why this is hardcoded rather than derived from `CLAUDE.md`

The workspace's own `CLAUDE.md` is written for an agent running *inside* the
repository. Most of it — the issue tracker, the git hooks, the token-usage
trailers, the push policy, the working-hours guardrail — is harness policy a
chat agent can neither follow nor act on, and it would consume a large slice
of the very context budget this bundle exists to protect.

What is reproduced here is the subset that changes **the code the agent
writes**: the Rust design rules, the `uom` convention, the documentation
standard, the V&V standard, and the scope limits on what this software may
be used for.

# Keeping it honest

[`agents_markdown`] takes the finished [`BundleReport`] so the document can
state **what the agent was not given**. An agent that is not told a crate
exists will invent its API rather than ask, so the omissions are part of the
instructions, not an afterthought.

```rust
pub mod agents_md { /* ... */ }
```

### Functions

#### Function `agents_markdown`

Render the `AGENTS.md` that ships with a bundle.

Appends to [`RULES`] a manifest of what the agent actually received and —
more importantly — what it did not, drawn from `report`.

```rust
pub fn agents_markdown(report: &super::BundleReport) -> String { /* ... */ }
```

## Module `condense`

Condense a crate's `docs/api.md` down to a signature index.

# What this is for

The bundle can carry only a few crates' documentation in full. Without
something covering the rest, an external agent asked about
`tampines-steam-tables` when it was handed only `outram-foam-basic-lib` has
no way to know the former exists — and an agent that does not know a type
exists will confidently invent one. The index is the cheap map that prevents
that: every crate, every module, every public signature, one line of prose
each, and nothing else.

# What is dropped, and why that is acceptable here

Dropped: doc-comment bodies beyond the first line, examples, status tables,
licence and trademark preambles, and the `# Crate Documentation` /
`**Format Version:**` header rustdoc-md emits. Kept: headings, the module
path, and every line inside a fenced block that declares a public item.

This is **lossy in a way the reader cannot see from the output**, which is
why [`condensed_index_markdown`] writes a banner at the top of the file
saying so and pointing at the full `api.md`. An index that looks like
complete documentation is worse than no index.

# A structural limit, inherited from rustdoc-md

rustdoc-md emits **flat headings** — a submodule gets the same heading level
as its parent — so the module tree cannot be recovered from heading depth.
This is recorded in `kovan-cli`'s `commands::api_docs` module as a knowingly
accepted trade-off. The condenser therefore carries the module path from the
`# Module \`x\`` heading text and never infers nesting from `#` count.

```rust
pub mod condense { /* ... */ }
```

### Functions

#### Function `condensed_index_markdown`

Render `_INDEX.md`: the **roster** of every crate in the workspace and what
each one would cost to request.

# Why this is a roster and not the whole index

The original design put a condensed signature index of every crate into this
one file. Measured on the real workspace on 2026-08-14, that file came to
**535,580 bytes — about 134 k estimated tokens**, or two thirds of a 200 k
budget consumed before a single crate's real documentation was uploaded. Its
bulk was evenly spread (headings 25%, signatures 44%, descriptions 29%), so
no single trim rescued it.

The content was not dropped; it was **split per crate** into
`<crate>.index.md` (see [`crate_index_markdown`]), which makes the bundle a
ladder the reader can climb one crate at a time:

| Tier | File | Typical size | Upload when |
|---|---|---|---|
| Roster | `_INDEX.md` | ~3 KB | always |
| Index | `<crate>.index.md` | ~40 KB | you need to know what a crate contains |
| Full | `<crate>.api.md` | ~400 KB | you are writing against that crate |

Uploading every `.index.md` reproduces the old single file exactly, so
nothing is lost by the split — it only becomes optional.

Output is deterministic: `entries` is consumed in the order given (which
[`inventory`](super::inventory) sorts by directory name) and nothing clock-
or machine-dependent is written.

```rust
pub fn condensed_index_markdown(entries: &[super::CrateEntry]) -> String { /* ... */ }
```

#### Function `crate_index_markdown`

Render one crate's `<crate>.index.md` — the middle rung of the ladder.

Reads that crate's `docs/api.md` and condenses it with
[`condense_api_markdown`]. Returns `Ok(None)` when the crate has no mirror.

```rust
pub fn crate_index_markdown(workspace_root: &std::path::Path, entry: &super::CrateEntry) -> io::Result<Option<String>> { /* ... */ }
```

#### Function `condense_api_markdown`

Reduce one `api.md` body to headings, module paths, public signatures, and a
single line of description per item.

The transform is a line-oriented state machine rather than a Markdown parse:
the input is machine-generated by one tool with a stable shape, so a parser
would buy nothing and cost a dependency. It is exact about fenced blocks
(tracking ``` openers and closers) because that is the one place a naive
filter would go wrong — prose containing the word `pub` must not be mistaken
for a signature.

```rust
pub fn condense_api_markdown(body: &str) -> String { /* ... */ }
```

### Types

#### Struct `CrateEntry`

One workspace member and the documentation files found for it.

Produced by [`inventory`]. The paths are **relative to the workspace root**,
never absolute, so that a bundle generated on one machine is byte-identical
to one generated on another.

```rust
pub struct CrateEntry {
    pub directory: String,
    pub package: String,
    pub api_md: Option<std::path::PathBuf>,
    pub api_bytes: u64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `directory` | `String` | Directory name under `crates/`, e.g. `outram-foam-basic-lib`. This is<br>the identifier `kovan api-docs` takes, and the one used to name<br>the crate's file in the bundle. |
| `package` | `String` | The `[package] name` from the crate's `Cargo.toml`, e.g.<br>`outram_foam_basic_lib` may differ from the directory name. |
| `api_md` | `Option<std::path::PathBuf>` | Path to `docs/api.md`, relative to the workspace root, if it exists. |
| `api_bytes` | `u64` | Size of `docs/api.md` in bytes, or `0` when absent. |

##### Implementations

###### Methods

- ```rust
  pub fn has_api_docs(self: &Self) -> bool { /* ... */ }
  ```
  Whether this crate has a rustdoc mirror to contribute.

- ```rust
  pub fn bundle_filename(self: &Self) -> String { /* ... */ }
  ```
  The flat filename this crate's **full** documentation takes in the

- ```rust
  pub fn index_filename(self: &Self) -> String { /* ... */ }
  ```
  The flat filename this crate's **condensed index** takes, e.g.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> CrateEntry { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CrateEntry) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `BundleReport`

What [`write_bundle`] produced, for the caller to report to the user.

Carries the sizes so a CLI can print a per-file table and a running total
against the budget. Every token figure here is an **estimate** — see
[`estimated_tokens`].

```rust
pub struct BundleReport {
    pub files: std::collections::BTreeMap<String, u64>,
    pub included: Vec<String>,
    pub indexed: Vec<String>,
    pub missing_api_docs: Vec<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `files` | `std::collections::BTreeMap<String, u64>` | Bundle filename → size in bytes, ordered by filename. |
| `included` | `Vec<String>` | Crates whose full `api.md` was copied, by directory name. |
| `indexed` | `Vec<String>` | Crates that got a condensed `<crate>.index.md`, by directory name. |
| `missing_api_docs` | `Vec<String>` | Crates that have **no** `docs/api.md` and so appear nowhere in the<br>bundle, by directory name. Named in `AGENTS.md` so the agent is told<br>what it has not been shown. |

##### Implementations

###### Methods

- ```rust
  pub fn total_bytes(self: &Self) -> u64 { /* ... */ }
  ```
  Total bytes across every file written to disk.

- ```rust
  pub fn core_bytes(self: &Self) -> u64 { /* ... */ }
  ```
  Bytes of the **core upload set** — `AGENTS.md`, `_INDEX.md`, and the full

- ```rust
  pub fn core_estimated_tokens(self: &Self) -> u64 { /* ... */ }
  ```
  Estimated tokens for the core upload set.

- ```rust
  pub fn optional_files(self: &Self) -> Vec<(String, u64)> { /* ... */ }
  ```
  The optional per-crate index files and their sizes, smallest first, so a

- ```rust
  pub fn total_estimated_tokens(self: &Self) -> u64 { /* ... */ }
  ```
  Total **estimated** tokens across every file written.

- ```rust
  pub fn exceeds_budget(self: &Self, budget_tokens: u64) -> bool { /* ... */ }
  ```
  Whether the **core upload set** exceeds `budget_tokens`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> BundleReport { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> BundleReport { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &BundleReport) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `estimated_tokens`

Estimated model tokens for `bytes` of text, rounding up.

See [`BYTES_PER_ESTIMATED_TOKEN`] for what this is and is not. Every caller
that surfaces the result must label it an *estimate*; describing it as a
token count would be a claim this cannot support.

```rust
pub fn estimated_tokens(bytes: u64) -> u64 { /* ... */ }
```

#### Function `inventory`

Walk `crates/` and record every member with its documentation files.

Returns entries **sorted by directory name**, which is what makes every
downstream artifact reproducible. Directories without a `Cargo.toml` are
skipped silently: they are not crates.

`workspace_root` is the directory containing `crates/`. Errors from reading
individual `Cargo.toml` files are propagated rather than swallowed — a crate
that cannot be read is a fact the caller needs, not one to paper over.

```rust
pub fn inventory(workspace_root: &std::path::Path) -> io::Result<Vec<CrateEntry>> { /* ... */ }
```

#### Function `write_bundle`

Write the flat bundle into `out_dir`, replacing anything already there.

`selected` names the crate directories whose `api.md` is copied verbatim;
crates outside it still appear in `_INDEX.md`. A name in `selected` that
matches no crate, or matches one with no mirror, is simply not copied — the
caller is expected to have validated the selection and to report on it.

# Why the directory is cleared first

The bundle is *uploaded*, so a stale file left behind from a previous run is
not merely untidy — it is a crate the agent will be told about that the
maintainer thought they had dropped. Clearing makes the directory's contents
exactly the current selection, always.

Only the bundle's own file types are removed (`*.md`), so pointing this at a
directory holding something else cannot destroy it wholesale.

```rust
pub fn write_bundle(workspace_root: &std::path::Path, out_dir: &std::path::Path, entries: &[CrateEntry], selected: &[String]) -> io::Result<BundleReport> { /* ... */ }
```

### Constants and Statics

#### Constant `BYTES_PER_ESTIMATED_TOKEN`

Bytes of source text assumed to correspond to one model token.

**This is a convention, not a measurement.** KOVAN has no tokenizer and must
not gain one — it is offline and deterministic by charter, and every real
tokenizer is a model-specific data file. Four bytes per token is the usual
rule of thumb for English prose and code.

It is **optimistic for this corpus.** Generated API markdown is dense in
punctuation, `snake_case` identifiers and fully-qualified paths, all of which
tokenize worse than prose, so a real count will typically come out *above*
this estimate. Treat any budget computed from it as soft, and prefer leaving
headroom over filling it exactly.

```rust
pub const BYTES_PER_ESTIMATED_TOKEN: u64 = 4;
```

### Re-exports

#### Re-export `agents_markdown`

```rust
pub use agents_md::agents_markdown;
```

#### Re-export `condensed_index_markdown`

```rust
pub use condense::condensed_index_markdown;
```

#### Re-export `crate_index_markdown`

```rust
pub use condense::crate_index_markdown;
```

## Module `extract`

Ripgrep-first symbol extraction — the deterministic, offline, Android-clean
core of `kovan-semantics`.

Per KOVAN's "Important Philosophy" (see `docs/kovan.md`, "# KOVAN Semantics"),
this layer does **not** reimplement a compiler or build a universal AST. It
locates likely definition sites with the ripgrep engine (via
[`kovan_discovery`]) and pulls the symbol *name*, *kind*, and *location* out
of those lines with a small set of anchored regexes. The result is a set of
[`ExtractedSymbol`]s normalised into the shared [`KovanSymbol`] model.

This is intentionally approximate — it is the pre-language-server heuristic.
When higher fidelity is needed the crate escalates to the real language
servers (see [`crate::adapters`]), which stay behind the off-by-default,
non-Android `language-servers` feature. The extractor here is what every
Android build gets, and it never shells out or touches the network.

## Per-language scanners

One submodule per language, each a line-oriented scanner that also tracks a
light enclosing-scope stack so members get a qualified name:

- `rust`    — `fn` / `struct` / `enum` / `trait` / `type` / `mod` / `impl`
  (scopes: `mod` and `impl` blocks, brace-depth tracked).
- `cpp`     — functions / `class` / `struct` / `union` / `namespace`
  (scopes: `namespace` and class blocks, brace-depth tracked).
- `python`  — `def` / `class` (scopes: indentation).
- `fortran` — `module` / `subroutine` / `function` / `type` (scopes:
  keyword `end` matching, case-insensitive).

## Known limits (all languages)

Brace/keyword counting is textual: braces inside string/char literals or
comments are counted, so deeply macro-generated or unusual formatting can
mis-nest a qualified name. Names are always correct; qualification is
best-effort. Multi-line signatures are read from their first (keyword) line.
These are acceptable for a knowledge-map heuristic and are exactly what the
language-server escalation path exists to supersede.

```rust
pub mod extract { /* ... */ }
```

### Types

#### Enum `SymbolKind`

The kind of a source symbol, normalised across the four supported languages.

Stored on [`ExtractedSymbol`] and rendered into [`KovanSymbol::kind`] via
[`SymbolKind::as_str`]. The set is closed (enum dispatch, per the workspace
rules) — add a variant here and every match site must handle it.

```rust
pub enum SymbolKind {
    Function,
    Subroutine,
    Struct,
    Enum,
    Trait,
    Impl,
    Type,
    Union,
    Class,
    Namespace,
    Module,
}
```

##### Variants

###### `Function`

A free function or method: Rust `fn`, C++ function, Python `def`,
Fortran `function`.

###### `Subroutine`

A Fortran `subroutine` (a function with no return value).

###### `Struct`

A Rust `struct` or a C++ `struct`.

###### `Enum`

A Rust `enum`.

###### `Trait`

A Rust `trait`.

###### `Impl`

A Rust `impl` block (associated to a type, optionally a trait).

###### `Type`

A Rust type alias, or a Fortran derived `type` definition.

###### `Union`

A C++ `union`.

###### `Class`

A C++ or Python `class`.

###### `Namespace`

A C++ `namespace`.

###### `Module`

A Rust `mod`, a Fortran `module`, or a Python module (file).

##### Implementations

###### Methods

- ```rust
  pub fn as_str(self: Self) -> &'static str { /* ... */ }
  ```
  The short, stable string written into [`KovanSymbol::kind`]. Matches the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> SymbolKind { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SymbolKind) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `ExtractedSymbol`

One symbol found by the ripgrep-first scanner, with its source location.

This is the extractor's **file-local** intermediate record: the scanners
produce it per source file, with a typed [`SymbolKind`] and no repository
context. [`ExtractedSymbol::into_kovan_symbol`] attaches the repository ID
and normalises it into the shared [`KovanSymbol`] — which, since the
`kovan-common` v2 pass, carries the file/line/language directly, so the
Markdown outputs and every downstream crate now work off `KovanSymbol`
alone. `ExtractedSymbol` remains as the scanner-stage record because a
`KovanSymbol` additionally needs the `id`/`repository_id` that only the
catalogue step (which knows the repo) can supply.

```rust
pub struct ExtractedSymbol {
    pub name: String,
    pub qualified_name: String,
    pub kind: SymbolKind,
    pub language: crate::LanguageAdapter,
    pub file: std::path::PathBuf,
    pub line: u64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | The bare identifier as written in source (e.g. `mass_flux`). |
| `qualified_name` | `String` | Best-effort qualified path: enclosing scopes joined to `name`<br>(`module::Type::method`, `Namespace::Class::method`, `pkg.Class.method`).<br>Falls back to `name` when no scope is detected. |
| `kind` | `SymbolKind` | The normalised symbol kind. |
| `language` | `crate::LanguageAdapter` | The language whose scanner produced this symbol. |
| `file` | `std::path::PathBuf` | Path to the file the symbol was found in. Made repository-relative by<br>[`catalogue_symbols_detailed`] when possible. |
| `line` | `u64` | 1-based line number of the definition's keyword line. |

##### Implementations

###### Methods

- ```rust
  pub fn into_kovan_symbol(self: Self, repository_id: &str) -> KovanSymbol { /* ... */ }
  ```
  Normalise into the shared [`KovanSymbol`], attaching the given repository

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ExtractedSymbol { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ExtractedSymbol) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `extract_from_text`

Extract every symbol from a single file's `text` using the scanner for
`adapter`. Pure function of its inputs (deterministic); does no I/O.

`file` is recorded on each returned [`ExtractedSymbol`] but is not read — the
caller supplies the already-read `text`. This keeps the scanners unit-testable
against synthetic snippets.

```rust
pub fn extract_from_text(adapter: crate::LanguageAdapter, file: &std::path::Path, text: &str) -> Vec<ExtractedSymbol> { /* ... */ }
```

#### Function `catalogue_symbols_detailed`

Catalogue every symbol in `repo` for the language of `adapter`, returning the
rich, location-carrying [`ExtractedSymbol`]s.

Pipeline (all deterministic + offline):
1. discover source files with [`kovan_discovery::discover_kind`] (the
   `.gitignore`-aware `fd`/ripgrep walker);
2. keep only files whose extension belongs to `adapter`'s language;
3. read each file and run its regex scanner ([`extract_from_text`]).

Unreadable or non-UTF-8 files are skipped (not an error), so the catalogue is
a best-effort superset — missing a file never fails the whole scan. File
paths on the results are made relative to `repo` when possible.

```rust
pub fn catalogue_symbols_detailed(repo: &std::path::Path, adapter: crate::LanguageAdapter) -> Result<Vec<ExtractedSymbol>, crate::SemanticsError> { /* ... */ }
```

## Types

### Enum `LanguageAdapter`

A supported source language and the preferred semantic tool KOVAN drives for
it (see `docs/kovan.md`, "# KOVAN Semantics" → "Language Support"). Dispatch
is by enum (a closed set) rather than trait objects, per the workspace rules.

```rust
pub enum LanguageAdapter {
    Rust,
    Cpp,
    Python,
    Fortran,
}
```

#### Variants

##### `Rust`

Rust — ripgrep-first, escalating to `rust-analyzer`. Primary targets:
TUAS, TAMPINES, BOON LAY and the wider Outram Park ecosystem.

##### `Cpp`

C++ — ripgrep-first, escalating to `clangd` / `libclang`. Primary target:
OpenFOAM.

##### `Python`

Python — ripgrep-first, escalating to Pyright / Jedi. Primary target:
OpenMC.

##### `Fortran`

Fortran — ripgrep-first, escalating to `fortls`. Primary target: NJOY.

#### Implementations

##### Methods

- ```rust
  pub fn server_binary(self: Self) -> &'static str { /* ... */ }
  ```
  The executable name of the preferred language server for this language.

- ```rust
  pub fn preferred_tool(self: Self) -> &'static str { /* ... */ }
  ```
  The command-line name of the preferred semantic tool for this language.

- ```rust
  pub fn extensions(self: Self) -> &'static [&'static str] { /* ... */ }
  ```
  The source-file extensions (lowercase, no dot) this language owns. Used

- ```rust
  pub fn rough_definition_pattern(self: Self) -> &'static str { /* ... */ }
  ```
  A naive ripgrep pattern that matches likely *definition* lines in this

- ```rust
  pub fn file_kind(self: Self) -> FileKind { /* ... */ }
  ```
  The [`FileKind`] scanned for this language (always source files).

##### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> LanguageAdapter { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(adapter: LanguageAdapter) -> Self { /* ... */ }
    ```
    Map the tool-selecting [`LanguageAdapter`] to the shared language

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &LanguageAdapter) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Enum `SemanticsError`

Errors produced by the semantics engine.

```rust
pub enum SemanticsError {
    Unimplemented(&'static str),
    Tool(String),
}
```

#### Variants

##### `Unimplemented`

The requested operation is not implemented yet (e.g. the language-server
escalation path is scaffolded but not wired).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&'static str` |  |

##### `Tool`

The underlying language tool was unavailable or failed.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

#### Implementations

##### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> SemanticsError { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, f: &mut std::fmt::Formatter<''_>) -> std::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Error**
- **ErrorExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SemanticsError) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Functions

### Function `rough_definition_scan`

A rough, ripgrep-first scan of a repository for probable definition *lines*
of the given language. Discovers source files with the `.gitignore`-aware
walker, then greps each with the ripgrep engine. Returns `(path, match)`
pairs (line number + text) — it does not name symbols.

This is the cheap heuristic; [`catalogue_symbols`] is the real extractor that
resolves names, kinds, and qualified paths.

```rust
pub fn rough_definition_scan(repo: &std::path::Path, adapter: LanguageAdapter) -> Result<Vec<(std::path::PathBuf, SearchMatch)>, SemanticsError> { /* ... */ }
```

### Function `catalogue_symbols`

Catalogue the symbols in a repository as normalised [`KovanSymbol`]s, using
the ripgrep-first extractor for `adapter`'s language.

Deterministic and offline. This is the default (Android-clean) path; each
symbol's stable ID embeds its repository-relative `file:line` and qualified
name. For the richer, location-carrying records use
[`catalogue_symbols_detailed`]; for the (deferred) language-server path see
[`adapters`].

`repository_id` is stamped onto every [`KovanSymbol`] so the results link
back to their [`KovanRepository`].

```rust
pub fn catalogue_symbols(repo: &std::path::Path, adapter: LanguageAdapter) -> Result<Vec<KovanSymbol>, SemanticsError> { /* ... */ }
```

## Re-exports

### Re-export `agents_markdown`

```rust
pub use agent_docs::agents_markdown;
```

### Re-export `condensed_index_markdown`

```rust
pub use agent_docs::condensed_index_markdown;
```

### Re-export `estimated_tokens`

```rust
pub use agent_docs::estimated_tokens;
```

### Re-export `inventory`

```rust
pub use agent_docs::inventory;
```

### Re-export `write_bundle`

```rust
pub use agent_docs::write_bundle;
```

### Re-export `BundleReport`

```rust
pub use agent_docs::BundleReport;
```

### Re-export `CrateEntry`

```rust
pub use agent_docs::CrateEntry;
```

### Re-export `catalogue_symbols_detailed`

```rust
pub use extract::catalogue_symbols_detailed;
```

### Re-export `extract_from_text`

```rust
pub use extract::extract_from_text;
```

### Re-export `ExtractedSymbol`

```rust
pub use extract::ExtractedSymbol;
```

### Re-export `SymbolKind`

```rust
pub use extract::SymbolKind;
```

### Re-export `repository_summary_markdown`

```rust
pub use outputs::repository_summary_markdown;
```

### Re-export `symbols_markdown`

```rust
pub use outputs::symbols_markdown;
```

### Re-export `KovanRepository`

```rust
pub use kovan_common::KovanRepository;
```

### Re-export `KovanSymbol`

```rust
pub use kovan_common::KovanSymbol;
```

### Re-export `KovanValidationCase`

```rust
pub use kovan_common::KovanValidationCase;
```

### Re-export `Language`

```rust
pub use kovan_common::Language;
```

### Re-export `discover_kind`

```rust
pub use kovan_discovery::discover_kind;
```

### Re-export `search_file`

```rust
pub use kovan_discovery::search_file;
```

### Re-export `FileKind`

```rust
pub use kovan_discovery::FileKind;
```

### Re-export `SearchMatch`

```rust
pub use kovan_discovery::SearchMatch;
```

