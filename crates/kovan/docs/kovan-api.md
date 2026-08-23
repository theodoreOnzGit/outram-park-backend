# Crate Documentation

**Version:** 0.0.0

**Format Version:** 61

# Module `kovan`

# kovan (library)

Shared modules behind KOVAN's three front ends, which live as separate
binaries in this same crate — exactly three, per the final interface spec
on GitHub issue #30 (2026-08-21):

- **`kovan`** (human-facing GUI, [`digitiser::gui`]) — a thin wrapper
  around the egui digitiser window, KOVAN's one GUI surface. Desktop-only:
  the `gui` feature (default everywhere except Android) target-gates its
  egui/eframe dependencies off Android, and [`digitiser::gui::run`]
  branches internally to a redirect message there.
- **`kovan-cli`** (agent-facing CLI, [`commands`]) — deterministic,
  line-oriented subcommands for a coding agent, including `digitise`
  (the automatic-only path over [`digitiser::frontend::AutoArgs`]).
- **`kovan-tui`** (human-facing terminal UI, [`tui`]) — a `ratatui`
  browser over the same sibling `kovan-*` crates, plus interactive
  literature ingestion (Ingest tab) and interactive graph digitisation
  (Digitiser tab, over the same [`digitiser`] engine `kovan-cli digitise`
  uses). Genuinely Android/Termux-usable, not just buildable — see
  `src/bin/kovan-tui.rs`.

This crate was consolidated from the former separate `kovan-cli` and
`kovan-tui` crates on 2026-08-21 so one crate carries all three interfaces
over the same knowledge-layer libraries — see `DECISIONS.md` for the
merge rationale and each front end's original design history. The
[`digitiser`] module joined the same day, moved here from
`kovan-literature` so it can depend on `kopitiam-pdf` (this crate's own
AGPL-3.0-only relicense, see `NOTICE`) without dragging
`kovan-literature` — used well beyond the GUI — into that relicense too.
The binaries were briefly five (`kovan`, `kovan-tui`, `kovan-gui`,
`kovan-digitise`, `kovan-digitise-tui`) before collapsing to the three
above later the same day, per GitHub issue #30's final spec — see
`NOTICE` and `src/tui/digitiser.rs`.

## Modules

## Module `commands`

Subcommand implementations for the `kovan` CLI.

One module per command (or command group), so `main.rs` stays a thin
`clap` dispatcher — the workspace's file-size-cap rule applies to this
crate too. Every module here follows the same shape: a `run` function (or
a small family of them) that takes already-parsed arguments and returns
`Result<(), String>` (or nothing, for infallible commands), printing
line-oriented, deterministic output to stdout so a coding agent can parse
it without a JSON layer.

This module also holds the two `clap`-facing enums shared by more than one
command ([`KindArg`], [`LangArg`]) — thin mirrors of the library enums
[`kovan_discovery::FileKind`] and [`kovan_semantics::LanguageAdapter`].
They exist only because `clap::ValueEnum` is a foreign trait: it cannot be
implemented on a foreign enum from this crate, so each library enum gets a
local twin plus a `From` conversion, matched exhaustively both ways.

```rust
pub mod commands { /* ... */ }
```

### Modules

## Module `agent_docs_gen`

`kovan-cli agent-docs-gen` — bundle the workspace's API documentation into a
flat set of files for an external chat agent with a fixed context budget.

The bundling logic lives in
[`kovan_semantics::agent_docs`](kovan_semantics::agent_docs); this module is
the `clap` surface, the console report, and the one opt-in path that shells
out to regenerate missing mirrors.

# Why the output is flat

The bundle exists to be uploaded to a web chat window, and those upload
dialogs take **files but not folders**. One file per crate, no
subdirectories, ever.

# Regeneration is opt-in and is the one non-offline path

`--regenerate-missing` calls [`super::api_docs`], which needs a nightly
toolchain (rustdoc's JSON output is nightly-only) and the `rustdoc-md`
binary, and compiles the crate. That is explicitly outside
KOVAN's offline/deterministic charter, so it is **off by default** and never
runs on its own — the same treatment [`super::setup`] gives its online
`cargo install` path. The default invocation reads files and writes files,
nothing more.

```rust
pub mod agent_docs_gen { /* ... */ }
```

### Functions

#### Function `run`

Run `kovan-cli agent-docs-gen`.

`workspace_root` is the directory containing `crates/`; `out_dir` is where
the flat bundle is written (cleared of `*.md` first). `selected` names the
crate directories whose `<crate>-api.md` is copied in full — every crate with a
mirror still appears in the condensed `_INDEX.md` regardless.

Prints a per-file table with byte sizes and estimated tokens, a running
total against `budget_tokens`, and the crates that were omitted. Returns an
error only for genuine IO failures; being over budget is reported loudly but
is not an error, because the estimate is not precise enough to justify
refusing to write.

```rust
pub fn run(workspace_root: &std::path::Path, out_dir: &std::path::Path, selected: &[String], budget_tokens: Option<u64>, regenerate_missing: bool, list_only: bool) -> io::Result<()> { /* ... */ }
```

#### Function `resolve_out_dir`

Where the bundle is written, in order of preference.

Delegates to [`super::workspace::output_dir`]: an explicit `--out` wins,
then the workspace (`<workspace>/agent-docs`, which the repository's
`.gitignore` already covers), then `~/Documents/agent-docs`, then
`~/agent-docs`.

```rust
pub fn resolve_out_dir(explicit: Option<&std::path::Path>) -> io::Result<(std::path::PathBuf, String)> { /* ... */ }
```

## Module `api_docs`

`kovan-cli api-docs` — regenerate a crate's `docs/<crate>-api.md`, the committed
markdown mirror of its public API.

A Rust port of `scripts/gen_api_docs.py`, which it **replaces** (retired
2026-08-14). The pipeline is unchanged:

```text
cargo +nightly doc --output-format json  ->  target/doc/<crate>.json
                       rustdoc-md        ->  crates/<crate>/docs/<crate>-api.md
```

# Filename: `<crate>-api.md`, not `api.md`

Named after its own crate directory (2026-08-17 onward) rather than the bare
`api.md` every crate used to write. Two problems that name had: a reader with
several of these files open in an editor or a bundle sees N identically-named
tabs, and nothing stopped a crate from acquiring a *second*, differently-named
mirror by accident — `njoy-outram-park-fork` had carried both `docs/api.md`
and `docs/njoy-api.md`, byte-identical, since 2026-08-14, doubling that
crate's published package for no reason anyone meant. `<crate>-api.md` is
self-describing out of context and only one name is ever right for a given
crate directory, so a second copy cannot arise without the mismatch being
visible in the filename itself.

# Why the Python went

The chain used to be `kovan` (Rust) → `python3` → `cargo` + `rustdoc-md`: a
Rust binary spawning an interpreter in order to spawn Rust tooling. That
inverts the direction epic `op-yz7b` already set when it deleted
`docs/historian/*.py` and `token_usage.py` in favour of `kovan-metrics`, so
the toolchain would need no Python interpreter. The reason recorded in
`.githooks/kovan-bin.sh` is concrete rather than aesthetic: on Windows,
`python3` routinely resolves to a Microsoft Store alias stub that prints an
advert and exits, which silently turned the token hooks into no-ops.

# Why nightly, and why that is not alarming

**`rustdoc-md` is an ordinary stable binary** that reads a JSON file. The
nightly requirement belongs one step upstream, to rustdoc's
`--output-format json`, which is still gated behind `-Z unstable-options`.
Verified 2026-08-14: `cargo +stable doc --output-format json` fails with
*"unexpected argument `--output-format` found"*.

It is **build tooling only**. Nothing shipped needs nightly; the workspace
builds, tests and publishes on stable, and nightly is touched only when a
mirror is regenerated.

The alternative was tried and rejected before this workspace's first commit:
scraping rustdoc's HTML with pandoc produced a *truncated* enum-variant list,
because rustdoc hides long variant lists behind a JavaScript "Show N
variants" widget meant for browsers. The JSON is the same structured AST
rustdoc renders from, so item lists come out complete and correctly typed.

# Why this lives in `kovan` (the CLI) and not `kovan-semantics`

It spawns `cargo`, so it is desktop-scope and neither offline nor
deterministic. `kovan-semantics` must stay Android-clean and offline by
charter. This is the same split [`super::setup`] already uses for its
`cargo install` path.

```rust
pub mod api_docs { /* ... */ }
```

### Types

#### Enum `Scope`

Which crates a run covers.

```rust
pub enum Scope {
    Existing,
    All,
}
```

##### Variants

###### `Existing`

Refresh only crates that already have a `docs/<crate>-api.md`.

The default for `--all`, and what "regenerate the suite" normally means:
bring the committed mirrors back in step with the code, without
deciding on anyone's behalf that 23 more crates should acquire one.

###### `All`

Every crate under `crates/`, creating mirrors that do not yet exist.

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Scope { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Scope) -> bool { /* ... */ }
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
### Functions

#### Function `generate`

Regenerate `crates/<crate_dir>/docs/<crate_dir>-api.md`.

`workspace_root` is the directory containing `crates/`. `crate_dir` is the
**directory name** under `crates/` (e.g. `outram-foam-basic-lib`), which may
differ from the `[package] name` in its manifest. `private` adds
`--document-private-items`, for auditing internals rather than the published
surface.

Returns the path written. Every failure names the missing prerequisite and
the command that fixes it, rather than reporting a bare non-zero exit: a
missing toolchain is a task, not a diagnosis (see the "API-doc toolchain"
hard rule in the workspace `CLAUDE.md`).

```rust
pub fn generate(workspace_root: &std::path::Path, crate_dir: &str, private: bool) -> io::Result<std::path::PathBuf> { /* ... */ }
```

#### Function `run`

Run `kovan-cli api-docs`.

`crate_dir` names a single crate; `all` regenerates the whole suite instead,
and `include_missing` widens that to crates with no mirror yet. Exactly one
of `crate_dir` and `all` is expected — the CLI enforces that.

```rust
pub fn run(workspace_root: &std::path::Path, crate_dir: Option<&str>, all: bool, include_missing: bool, private: bool) -> io::Result<()> { /* ... */ }
```

## Module `cost`

`kovan-cli cost` — estimate how many tokens a file would cost an agent to
read whole (GitHub issue #32: "kovan-cli should import the token savings
features of kopitiam-cli, by wiring the api in directly").

Wired directly to [`kopitiam_tokenizer::estimate_tokens`] /
[`kopitiam_tokenizer::estimate_tokens_by_line`] — a dependency-free,
per-Unicode-script character-weighted estimate (see that crate's own
module doc for the accuracy model: roughly ±25-30% against a real
GPT-2/Qwen-family BPE tokenizer on ordinary text). This is materially
better than [`kovan_semantics::agent_docs::estimated_tokens`]'s `bytes/4`
heuristic, which is left as-is — it only feeds `agent-docs-gen --budget`'s
own accounting and is not this command's concern.

Deliberately named `cost`, not `tokens` — that name is already
[`super::tokens`]'s per-commit API-usage accounting (`kovan-metrics`), an
unrelated concept this command must not collide with.

```rust
pub mod cost { /* ... */ }
```

### Functions

#### Function `run`

Read `path` and print its estimated token cost — the whole-file total by
default, or a per-line breakdown with `by_line`.

# Errors

A message if `path` cannot be read (missing, a directory, or not valid
UTF-8 — the estimator operates on `&str`, so a binary file is reported
rather than guessed at).

```rust
pub fn run(path: std::path::PathBuf, by_line: bool) -> Result<(), String> { /* ... */ }
```

## Module `discover`

`kovan-cli discover` — enumerate files under a root, honouring `.gitignore`
(via [`kovan_discovery::discover`] / [`kovan_discovery::discover_kind`]).

```rust
pub mod discover { /* ... */ }
```

### Functions

#### Function `run`

Print one discovered path per line, sorted (see [`kovan_discovery::discover`]
for the determinism guarantee). `kind` restricts the result to a single
[`super::KindArg`]; `None` returns every non-ignored file.

```rust
pub fn run(root: std::path::PathBuf, kind: Option<super::KindArg>) { /* ... */ }
```

## Module `gen`

`kovan-cli gen` — deterministic numerical-method code generation
(`kovan-codegen`). One nested subcommand per method family, mirroring
`kovan-cli methods`'s catalogue grouping.

`clap::ValueEnum` is a foreign trait, so it cannot be derived directly on
`kovan-codegen`'s own catalogue enums from this crate; each family gets a
local `clap`-facing mirror plus a `From` conversion (same pattern as
[`super::KindArg`]/[`super::LangArg`]), matched exhaustively both ways.

```rust
pub mod gen { /* ... */ }
```

### Types

#### Enum `GenCommand`

`kovan-cli gen <family> <method> [--out <path>]`.

```rust
pub enum GenCommand {
    Root {
        method: RootFinderArg,
        out: Option<std::path::PathBuf>,
    },
    Linear {
        method: LinearSolverArg,
        out: Option<std::path::PathBuf>,
    },
    Nonlinear {
        method: NonlinearSolverArg,
        out: Option<std::path::PathBuf>,
    },
    Ode {
        method: OdeSolverArg,
        out: Option<std::path::PathBuf>,
    },
    Pde {
        method: PdeSchemeArg,
        out: Option<std::path::PathBuf>,
    },
}
```

##### Variants

###### `Root`

Generate a scalar root finder.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `method` | `RootFinderArg` |  |
| `out` | `Option<std::path::PathBuf>` | Write the generated Rust source here instead of stdout. |

###### `Linear`

Generate a linear-system solver.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `method` | `LinearSolverArg` |  |
| `out` | `Option<std::path::PathBuf>` |  |

###### `Nonlinear`

Generate a nonlinear (Newton-family) solver.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `method` | `NonlinearSolverArg` |  |
| `out` | `Option<std::path::PathBuf>` |  |

###### `Ode`

Generate an ODE initial-value-problem integrator.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `method` | `OdeSolverArg` |  |
| `out` | `Option<std::path::PathBuf>` |  |

###### `Pde`

Generate a PDE spatial-discretisation scheme.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `method` | `PdeSchemeArg` |  |
| `out` | `Option<std::path::PathBuf>` |  |

##### Implementations

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

- **CastableFrom**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **FromArgMatches**
  - ```rust
    fn from_arg_matches(__clap_arg_matches: &clap::ArgMatches) -> ::std::result::Result<Self, clap::Error> { /* ... */ }
    ```

  - ```rust
    fn from_arg_matches_mut(__clap_arg_matches: &mut clap::ArgMatches) -> ::std::result::Result<Self, clap::Error> { /* ... */ }
    ```

  - ```rust
    fn update_from_arg_matches(self: &mut Self, __clap_arg_matches: &clap::ArgMatches) -> ::std::result::Result<(), clap::Error> { /* ... */ }
    ```

  - ```rust
    fn update_from_arg_matches_mut<''b>(self: &mut Self, __clap_arg_matches: &mut clap::ArgMatches) -> ::std::result::Result<(), clap::Error> { /* ... */ }
    ```

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

- **Subcommand**
  - ```rust
    fn augment_subcommands<''b>(__clap_app: clap::Command) -> clap::Command { /* ... */ }
    ```

  - ```rust
    fn augment_subcommands_for_update<''b>(__clap_app: clap::Command) -> clap::Command { /* ... */ }
    ```

  - ```rust
    fn has_subcommand(__clap_name: &str) -> bool { /* ... */ }
    ```

- **Sync**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
#### Enum `RootFinderArg`

```rust
pub enum RootFinderArg {
    Bisection,
    RegulaFalsi,
    Illinois,
    Pegasus,
    Secant,
    NewtonRaphson,
    Brent,
}
```

##### Variants

###### `Bisection`

###### `RegulaFalsi`

###### `Illinois`

###### `Pegasus`

###### `Secant`

###### `NewtonRaphson`

###### `Brent`

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> RootFinderArg { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(m: RootFinderArg) -> Self { /* ... */ }
    ```

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **ValueEnum**
  - ```rust
    fn value_variants<''a>() -> &'a [Self] { /* ... */ }
    ```

  - ```rust
    fn to_possible_value<''a>(self: &Self) -> ::std::option::Option<clap::builder::PossibleValue> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
#### Enum `LinearSolverArg`

```rust
pub enum LinearSolverArg {
    Jacobi,
    GaussSeidel,
    Sor,
    ConjugateGradient,
    BiCgStab,
    Gmres,
    Lu,
    Qr,
    Cholesky,
}
```

##### Variants

###### `Jacobi`

###### `GaussSeidel`

###### `Sor`

###### `ConjugateGradient`

###### `BiCgStab`

###### `Gmres`

###### `Lu`

###### `Qr`

###### `Cholesky`

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> LinearSolverArg { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(m: LinearSolverArg) -> Self { /* ... */ }
    ```

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **ValueEnum**
  - ```rust
    fn value_variants<''a>() -> &'a [Self] { /* ... */ }
    ```

  - ```rust
    fn to_possible_value<''a>(self: &Self) -> ::std::option::Option<clap::builder::PossibleValue> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
#### Enum `NonlinearSolverArg`

```rust
pub enum NonlinearSolverArg {
    Newton,
    QuasiNewton,
    Broyden,
    TrustRegion,
}
```

##### Variants

###### `Newton`

###### `QuasiNewton`

###### `Broyden`

###### `TrustRegion`

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> NonlinearSolverArg { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(m: NonlinearSolverArg) -> Self { /* ... */ }
    ```

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **ValueEnum**
  - ```rust
    fn value_variants<''a>() -> &'a [Self] { /* ... */ }
    ```

  - ```rust
    fn to_possible_value<''a>(self: &Self) -> ::std::option::Option<clap::builder::PossibleValue> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
#### Enum `OdeSolverArg`

```rust
pub enum OdeSolverArg {
    Euler,
    Rk2,
    Rk4,
    DormandPrince,
    BackwardEuler,
    CrankNicolson,
}
```

##### Variants

###### `Euler`

###### `Rk2`

###### `Rk4`

###### `DormandPrince`

###### `BackwardEuler`

###### `CrankNicolson`

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> OdeSolverArg { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(m: OdeSolverArg) -> Self { /* ... */ }
    ```

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **ValueEnum**
  - ```rust
    fn value_variants<''a>() -> &'a [Self] { /* ... */ }
    ```

  - ```rust
    fn to_possible_value<''a>(self: &Self) -> ::std::option::Option<clap::builder::PossibleValue> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
#### Enum `PdeSchemeArg`

```rust
pub enum PdeSchemeArg {
    Poisson1dFiniteDifference,
    Diffusion1dFiniteVolume,
    BoundaryConditionScaffold,
}
```

##### Variants

###### `Poisson1dFiniteDifference`

###### `Diffusion1dFiniteVolume`

###### `BoundaryConditionScaffold`

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> PdeSchemeArg { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(m: PdeSchemeArg) -> Self { /* ... */ }
    ```

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **ValueEnum**
  - ```rust
    fn value_variants<''a>() -> &'a [Self] { /* ... */ }
    ```

  - ```rust
    fn to_possible_value<''a>(self: &Self) -> ::std::option::Option<clap::builder::PossibleValue> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
### Functions

#### Function `run`

Dispatch a parsed [`GenCommand`]: resolve it to a [`Method`], generate,
then either print the source or write it to `--out`.

```rust
pub fn run(command: GenCommand) -> Result<(), String> { /* ... */ }
```

## Module `historian`

`kovan-cli historian` — the pre-merge-to-`main` accounting report.

A thin frontend over [`kovan_metrics::historian`]. Generates the markdown
report that accompanies a `develop` → `main` merge: tokens spent and
lines/KLOC written across the released window, with a per-crate breakdown
and a per-commit ledger.

With no `--from`, the window is everything on `--branch` not yet on
`--base` — exactly what the pending merge would deliver.

```rust
pub mod historian { /* ... */ }
```

### Functions

#### Function `run`

Generate a historian report.

```rust
pub fn run(from: Option<String>, to: Option<String>, branch: String, base: String, outfile: Option<std::path::PathBuf>) -> Result<(), String> { /* ... */ }
```

## Module `kloc`

`kovan-cli kloc` — the paper's productivity accounting.

The measurement lives in
[`kovan_metrics::kloc`](kovan_metrics::kloc); this module is the `clap`
surface plus the one thing that crate deliberately does not do: **clone
repositories from GitHub**.

# Why cloning is here and not in `kovan-metrics`

`kovan-metrics` is offline by charter. Fetching from the network belongs in
the CLI layer, opt-in behind a flag, exactly as [`super::setup`] and
[`super::api_docs`] handle their own non-offline paths.

```rust
pub mod kloc { /* ... */ }
```

### Functions

#### Function `run`

Run `kovan-cli kloc`.

```rust
pub fn run(out_dir: std::path::PathBuf, clone: bool, from_github: bool, fetch: bool, check: bool, no_figure: bool) -> io::Result<()> { /* ... */ }
```

#### Function `default_out_dir`

Default output directory.

```rust
pub fn default_out_dir(root: &std::path::Path) -> std::path::PathBuf { /* ... */ }
```

## Module `lit`

`kovan-cli lit` — the literature pipeline (`kovan-literature`): PDF import,
BibTeX generation, and Markdown heading outlines.

Implements the canonical workflow from `docs/kovan.md`, "Literature
Workflow": `PDF → Markdown → KovanDocument → BibTeX`. The Rust
[`KovanDocument`] is authoritative; `lit bibtex` only ever *renders* from
it, never the reverse.

```rust
pub mod lit { /* ... */ }
```

### Types

#### Enum `LitCommand`

`kovan-cli lit <subcommand>`.

```rust
pub enum LitCommand {
    Import {
        pdf: std::path::PathBuf,
        json_out: Option<std::path::PathBuf>,
        markdown_out: Option<std::path::PathBuf>,
    },
    Bibtex {
        input: std::path::PathBuf,
    },
    Outline {
        pdf: std::path::PathBuf,
    },
}
```

##### Variants

###### `Import`

Import a PDF: extract metadata + generate the Markdown body into a
`KovanDocument`, and print a line-oriented summary.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `pdf` | `std::path::PathBuf` | Source PDF. |
| `json_out` | `Option<std::path::PathBuf>` | Also write the full document as pretty JSON to this path (the<br>canonical on-disk form — re-readable by `lit bibtex`). |
| `markdown_out` | `Option<std::path::PathBuf>` | Also write just the generated Markdown body to this path. |

###### `Bibtex`

Emit a BibTeX entry — from a source PDF (metadata is extracted first)
or from a previously-saved `KovanDocument` JSON file (`.json`
extension, e.g. from `lit import --json-out`).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `input` | `std::path::PathBuf` | Source PDF, or a `.json` `KovanDocument` (dispatched by extension). |

###### `Outline`

Print the Markdown heading outline of a PDF, one heading per line
(`"#"` repeated `level` times, a space, then the heading text — mirrors
the Markdown itself).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `pdf` | `std::path::PathBuf` | Source PDF. |

##### Implementations

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

- **CastableFrom**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **FromArgMatches**
  - ```rust
    fn from_arg_matches(__clap_arg_matches: &clap::ArgMatches) -> ::std::result::Result<Self, clap::Error> { /* ... */ }
    ```

  - ```rust
    fn from_arg_matches_mut(__clap_arg_matches: &mut clap::ArgMatches) -> ::std::result::Result<Self, clap::Error> { /* ... */ }
    ```

  - ```rust
    fn update_from_arg_matches(self: &mut Self, __clap_arg_matches: &clap::ArgMatches) -> ::std::result::Result<(), clap::Error> { /* ... */ }
    ```

  - ```rust
    fn update_from_arg_matches_mut<''b>(self: &mut Self, __clap_arg_matches: &mut clap::ArgMatches) -> ::std::result::Result<(), clap::Error> { /* ... */ }
    ```

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

- **Subcommand**
  - ```rust
    fn augment_subcommands<''b>(__clap_app: clap::Command) -> clap::Command { /* ... */ }
    ```

  - ```rust
    fn augment_subcommands_for_update<''b>(__clap_app: clap::Command) -> clap::Command { /* ... */ }
    ```

  - ```rust
    fn has_subcommand(__clap_name: &str) -> bool { /* ... */ }
    ```

- **Sync**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
### Functions

#### Function `run`

Dispatch a parsed [`LitCommand`].

```rust
pub fn run(command: LitCommand) -> Result<(), String> { /* ... */ }
```

## Module `lsp_daemon`

Keep-warm rust-analyzer daemon for `kovan-cli def`/`sig`/`refs` (op-fdph,
GitHub issue #32's follow-up).

# Why

[`super::semq::run_def`]/`run_sig`/`run_refs`'s fallback path
(`super::semq::connect`) spawns a **fresh** `rust-analyzer` and waits out
its full index on *every* invocation, then shuts it down — correct for a
single one-shot query, wasteful across a session that asks several. This
module fixes that by keeping one indexed session alive in a small
background daemon process, so only the *first* query in a workspace pays
the indexing cost; every later one (from any `kovan-cli` invocation, i.e.
any process) answers immediately.

# Shape

One daemon process per workspace root, holding one
[`kopitiam_semantic::AsyncRustAnalyzerSession`] (the primitive
`kopitiam-semantic` already ships for exactly this: non-blocking readiness
polling, built for a long-lived host rather than a one-shot call — see its
own module doc). [`serve`] runs the daemon in the foreground (spawned
detached by [`query`]'s lazy-start path) and keeps running **until
explicitly stopped** (`kovan-cli lsp-daemon stop --root <root>`, wired to
[`stop`]) — no idle timeout, by maintainer direction (2026-08-22): once
warm, it stays warm for the rest of the session rather than risking a cold
restart mid-work. Precedent for the lazy-spawn half of this shape already
lives in this workspace: `kopi-beans`' own `bn daemon run`.

Wire protocol: one [`Request`]/[`Response`] pair, as one line of JSON
each, per connection — a client opens a connection, writes one line,
reads one line, and closes. No framing beyond the newline, no
multiplexing: `kovan-cli` invocations are short-lived processes issuing
one query each, so a persistent multi-request connection buys nothing.

# Platform scope

Unix-domain-socket-based ([`std::os::unix::net`], no extra dependency),
so this only actually runs on `cfg(unix)` — which covers Linux, macOS,
*and* Android/Termux (Android's `target_family` is `"unix"`), matching
this crate's Android-clean rule with no extra gating needed. **Windows has
no daemon**: [`query`] unconditionally returns `None` there (checked at
compile time, not silently degraded), and every caller already treats
`None` as "fall back to `super::semq::connect`'s spawn-per-call path" —
so Windows keeps working, just without the warm-daemon speedup. A named
pipe implementation is future work, not attempted here (this workspace
has no Windows CI to validate it against).

```rust
pub mod lsp_daemon { /* ... */ }
```

### Re-exports

#### Re-export `serve`

```rust
pub use unix_impl::serve;
```

#### Re-export `stop`

```rust
pub use unix_impl::stop;
```

## Module `methods`

`kovan-cli methods` — list the `kovan-codegen` numerical-method catalogue and
report, per entry, whether it is backed by a generated template yet
(`ready`) or only catalogued (`not-implemented`).

```rust
pub mod methods { /* ... */ }
```

### Functions

#### Function `run`

Print the full catalogue, one method per line, grouped by family.

```rust
pub fn run() { /* ... */ }
```

## Module `outline`

`kovan-cli outline <file>` — a declarations-only skeleton of one file, so
an agent can decide whether it needs the whole thing before reading it
(GitHub issue #32's token-savings ask).

Reuses [`kovan_semantics::LanguageAdapter::rough_definition_pattern`] and
[`kovan_discovery::search_file`] directly — the same ripgrep-first
extractor `kovan-cli symbols`/`summary` already run repository-wide, just
applied to one file instead of walking a tree. No new dependency: this is
the ripgrep tier only, matching what [`kovan_semantics`] actually has
today; a richer, rust-analyzer-backed outline (mirroring kopitiam's own
`outline`, kopitiam-semantic-backed) is a deferred follow-on (see
`op-l3uz`), not this command's job.

```rust
pub mod outline { /* ... */ }
```

### Functions

#### Function `run`

Print `<line>: <declaration line>` for every rough-definition match in
`path`, in file order.

# Errors

A message if the file cannot be searched (missing, a directory, not valid
UTF-8, or an internal regex failure — see [`kovan_discovery::search_file`]).

```rust
pub fn run(path: std::path::PathBuf, lang: kovan_semantics::LanguageAdapter) -> Result<(), String> { /* ... */ }
```

## Module `project`

`kovan-cli project` — the "kovan folder" project format (op-63u0's
design, `docs/kovan-folder-format.md`): rescanning a project and
(re)writing its `kovan.toml` index (op-b1y5).

Wraps `crate::project` directly — this module is the `clap` surface
and line-oriented output only; the scan/regenerate/write logic lives in
the library so a future GUI action (a "regenerate now" button, or a
markdown-save hook) can call it without going through the CLI.

```rust
pub mod project { /* ... */ }
```

### Types

#### Enum `ProjectCommand`

`kovan-cli project <subcommand>`.

```rust
pub enum ProjectCommand {
    Regen {
        root: std::path::PathBuf,
    },
}
```

##### Variants

###### `Regen`

Rescan a "kovan folder" project and rewrite its `kovan.toml` index.

Prints one line per document found, then a summary. `kovan.toml` is
generated — see the module docs — so this always fully replaces it,
never merges.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `root` | `std::path::PathBuf` | The project root (containing `kovan.toml`, one `.bib` file,<br>`pdf/`, `markdown/`). |

##### Implementations

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

- **CastableFrom**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **FromArgMatches**
  - ```rust
    fn from_arg_matches(__clap_arg_matches: &clap::ArgMatches) -> ::std::result::Result<Self, clap::Error> { /* ... */ }
    ```

  - ```rust
    fn from_arg_matches_mut(__clap_arg_matches: &mut clap::ArgMatches) -> ::std::result::Result<Self, clap::Error> { /* ... */ }
    ```

  - ```rust
    fn update_from_arg_matches(self: &mut Self, __clap_arg_matches: &clap::ArgMatches) -> ::std::result::Result<(), clap::Error> { /* ... */ }
    ```

  - ```rust
    fn update_from_arg_matches_mut<''b>(self: &mut Self, __clap_arg_matches: &mut clap::ArgMatches) -> ::std::result::Result<(), clap::Error> { /* ... */ }
    ```

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

- **Subcommand**
  - ```rust
    fn augment_subcommands<''b>(__clap_app: clap::Command) -> clap::Command { /* ... */ }
    ```

  - ```rust
    fn augment_subcommands_for_update<''b>(__clap_app: clap::Command) -> clap::Command { /* ... */ }
    ```

  - ```rust
    fn has_subcommand(__clap_name: &str) -> bool { /* ... */ }
    ```

- **Sync**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
### Functions

#### Function `run`

Dispatch a `kovan-cli project` subcommand.

```rust
pub fn run(cmd: ProjectCommand) -> Result<(), String> { /* ... */ }
```

## Module `scan`

`kovan-cli scan` — a ripgrep-first scan of a repository for probable
definition lines of a given language (see
[`kovan_semantics::rough_definition_scan`]). This is the cheap heuristic
pre-filter; [`super::symbols`] is the real named-symbol catalogue.

```rust
pub mod scan { /* ... */ }
```

### Functions

#### Function `run`

Print `path:line: text` for every probable-definition line found.

```rust
pub fn run(root: std::path::PathBuf, lang: super::LangArg) -> Result<(), String> { /* ... */ }
```

## Module `search`

`kovan-cli search` — regex search over a single file or a whole repository.

Two modes, chosen by which arguments are given:

- `--path <file> --pattern <re>` — single-file search
  ([`kovan_discovery::search_file`]).
- `--root <dir> [--kind <k>] --pattern <re>` (root defaults to `.`, kind
  defaults to `source`) — repository-wide search
  ([`kovan_discovery::search_repository`]).

`--path` wins if both are given. Both modes print ripgrep-style
`path:line:column: text` lines (1-based line and column).

```rust
pub mod search { /* ... */ }
```

### Functions

#### Function `run`

Run the search in whichever mode `path`/`root` select. See the module docs
for the dispatch rule.

```rust
pub fn run(path: Option<std::path::PathBuf>, root: Option<std::path::PathBuf>, kind: Option<super::KindArg>, pattern: &str) -> Result<(), String> { /* ... */ }
```

## Module `semq`

`kovan-cli def`/`refs`/`sig` — rust-analyzer-backed semantic queries
(GitHub issue #32's follow-up: "wire in rust analyzer capability from
kopitiam into kovan cli").

Wired directly to [`kopitiam_semantic::RustAnalyzerSession`], which spawns
the external `rust-analyzer` binary and talks LSP over stdio — this
dependency is lightweight at compile time (no `ra_ap_*` crate linked) but
needs `rust-analyzer` on `PATH` at runtime (`rustup component add
rust-analyzer`) to answer anything.

Every query here is **name-based**: [`locate_declaration`] finds `symbol`'s
declaration line in `--file` by text (not `document_symbols` — see its own
doc comment for why), and that identifier's position is the query anchor.
[`extract_signature`]/[`looks_like_signature`] are close ports of
kopitiam's own `semq.rs` functions of the same name.

**Keeping rust-analyzer warm across invocations** (op-fdph): each of
`run_def`/`run_sig`/`run_refs` tries [`super::lsp_daemon::query`] first —
a background daemon holding one long-lived, already-indexed session — and
only falls back to [`connect`]'s spawn-index-shutdown-per-call path if no
daemon is reachable (including on non-Unix targets, where the daemon
doesn't exist at all). See `commands::lsp_daemon`'s module doc for the
daemon design.

**Deferred, not implemented here** (see `op-l3uz`): `callers`/`callees`
(call-hierarchy composition over `references` + `document_symbols`) and
`impls` (trait `impl`-site filtering). Both are real, more involved
features on top of the same session — this module ships the three
highest-value, simplest-to-verify queries first.

```rust
pub mod semq { /* ... */ }
```

### Functions

#### Function `run_def`

`kovan-cli def <symbol> --file <file>` — definition location plus its
signature (from `hover` at the same identifier — the declaration `--file`
matched *is* the definition site).

```rust
pub fn run_def(symbol: String, file: std::path::PathBuf, root: std::path::PathBuf) -> Result<(), String> { /* ... */ }
```

#### Function `run_sig`

`kovan-cli sig <symbol> --file <file>` — the signature alone.

```rust
pub fn run_sig(symbol: String, file: std::path::PathBuf, root: std::path::PathBuf) -> Result<(), String> { /* ... */ }
```

#### Function `run_refs`

`kovan-cli refs <symbol> --file <file>` — every reference site, as
`file:line:character` coordinates (0-based, matching every
`kopitiam_semantic` query — no +1 display conversion, same convention
kopitiam's own `refs` uses).

```rust
pub fn run_refs(symbol: String, file: std::path::PathBuf, root: std::path::PathBuf) -> Result<(), String> { /* ... */ }
```

## Module `setup`

`kovan-cli setup` — install a curated set of useful external CLI tools via
`cargo install`, skipping any whose binary is already on `PATH`.

This is an **explicit, online, desktop-scope convenience**: it is never
run automatically by any other `kovan` subcommand, and it has no bearing
on the rest of this crate's offline / Android-clean operation (see the
crate's Android note in `README.md`). It exists purely so a human or an
agent working in this repository can bring their shell up to a useful
baseline (`rg`, `fd`, `bat`, …) in one command instead of five.

The tool list ([`TOOLS`]) is intentionally hard-coded and small — extend
it by adding one more [`ToolSpec`] entry, nothing else needs to change.
The PATH-detection / "would this be installed" decision
([`decide`]) is a pure function so it is unit-testable without touching
the real `PATH` or spawning `cargo`; only [`run`] performs I/O.

```rust
pub mod setup { /* ... */ }
```

### Types

#### Struct `ToolSpec`

One entry in the curated external-tool list: the crate `cargo install`
pulls, the binary name that install provides (checked on `PATH`), and a
one-line explanation of what the tool is for.

```rust
pub struct ToolSpec {
    pub crate_name: &'static str,
    pub binary_name: &'static str,
    pub description: &'static str,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `crate_name` | `&'static str` | crates.io crate name, as passed to `cargo install <crate_name>`. |
| `binary_name` | `&'static str` | Binary name the crate installs — this is what gets probed on `PATH`. |
| `description` | `&'static str` | Human-readable one-line description of the tool's purpose. |

##### Implementations

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

- **CastableFrom**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

- **Sync**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
#### Enum `Action`

What `kovan-cli setup` should do with one tool, given whether its binary is
already on `PATH`. Pure decision, no I/O — see [`decide`].

```rust
pub enum Action {
    Skip,
    Install,
}
```

##### Variants

###### `Skip`

Binary already present and `--force` was not given: do nothing.

###### `Install`

Binary missing (or `--force` was given): run `cargo install`.

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Action { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Action) -> bool { /* ... */ }
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
### Functions

#### Function `decide`

Decide the [`Action`] for a tool from whether its binary was found on
`PATH` (`present`) and whether `--force` was given. Pure function —
takes the already-computed PATH-presence bool rather than probing
itself, so it is testable without touching the real filesystem/`PATH`.

```rust
pub fn decide(present: bool, force: bool) -> Action { /* ... */ }
```

#### Function `run`

`kovan-cli setup [--dry-run] [--force]` — the CLI entry point. Walks
[`TOOLS`], skipping tools already on `PATH` (unless `--force`),
installing the rest via `cargo install <crate>` (unless `--dry-run`, which
only reports what would happen). `cargo`/network/install failures are
caught per-tool and reported — one failing tool never stops the rest.
Returns `Err` only if at least one *requested* install genuinely failed
(never for tools that were skipped or dry-run reported).

```rust
pub fn run(dry_run: bool, force: bool) -> Result<(), String> { /* ... */ }
```

### Constants and Statics

#### Constant `TOOLS`

The curated, hard-coded list of external Rust CLI tools `kovan-cli setup`
knows how to install. Add an entry here to extend the list — everything
else (`--dry-run` reporting, PATH detection, install loop) is generic
over this array.

```rust
pub const TOOLS: &[ToolSpec] = _;
```

## Module `skill_gen`

`kovan-cli skill-gen` — write a Claude Code Skill-format Markdown file
documenting `kovan-cli`'s commands for an AI agent (GitHub issue #32:
"we shld be able to have kovan-cli generate skill.md also for AI agents
to read").

A native subcommand rather than a companion script, per this workspace's
"build it into kovan, don't reach for a script" direction (the same
reasoning `kovan agent-docs-gen`/`kovan api-docs` already follow) —
mirrors the *shape* of kopitiam's own `scripts/gen-kopitiam-skill.sh` ->
`kopitiam_skill.md` (frontmatter, a "CRITICAL agent guidance" section, then
recipes) without depending on that script or its output.

The content below is a hand-maintained template, not introspected from the
`clap::Command` tree — keep it in sync by hand when a subcommand's shape
changes materially (a mechanical `--help`-driven generator is a
reasonable follow-on, not implemented here).

```rust
pub mod skill_gen { /* ... */ }
```

### Functions

#### Function `run`

Write [`SKILL_MD`] to `out` (or [`DEFAULT_OUT`] if `None`).

# Errors

A message if the file cannot be written.

```rust
pub fn run(out: Option<std::path::PathBuf>) -> Result<(), String> { /* ... */ }
```

### Constants and Statics

#### Constant `DEFAULT_OUT`

Default output path when `--out` is not given.

```rust
pub const DEFAULT_OUT: &str = "kovan_skill.md";
```

## Module `slice`

`kovan-cli slice <file> <start> <end>` — print one line range instead of
the whole file, the third leg of the token-frugal `cost -> outline ->
slice` loop (GitHub issue #32).

No dependency at all: this is a plain line-indexed read, deliberately kept
that simple rather than reusing any parsing/search machinery it doesn't
need.

```rust
pub mod slice { /* ... */ }
```

### Functions

#### Function `run`

Print lines `start..=end` (1-based, inclusive) of `path`, each prefixed
with its line number. Out-of-range bounds are clamped to the file's
actual line count rather than erroring — asking for more than a short
file has is a common, harmless case (an agent guessing at a range).

# Errors

A message if `path` cannot be read, or if `start > end`.

```rust
pub fn run(path: std::path::PathBuf, start: usize, end: usize) -> Result<(), String> { /* ... */ }
```

## Module `symbols`

`kovan-cli symbols` and `kovan-cli summary` — repository symbol cataloguing via
`kovan-semantics`'s ripgrep-first extractor
([`kovan_semantics::catalogue_symbols_detailed`]), rendered either as
agent-facing line-oriented text or as the documented Markdown artifacts
(`docs/kovan.md`, "# KOVAN Semantics" → "Outputs": `symbols.md`,
`repository-summary.md`).

```rust
pub mod symbols { /* ... */ }
```

### Functions

#### Function `run_symbols`

`kovan-cli symbols <root> --lang <lang> [--markdown] [--out <path>] [--name <name>]`.

Default output is line-oriented: `path:line: kind qualified_name`, one
symbol per line (sorted by discovery order — see
[`catalogue_symbols`]). `--markdown` (or passing `--out`) renders
the full `symbols.md` artifact instead.

```rust
pub fn run_symbols(root: std::path::PathBuf, lang: super::LangArg, markdown: bool, out: Option<std::path::PathBuf>, name: Option<String>) -> Result<(), String> { /* ... */ }
```

#### Function `run_summary`

`kovan-cli summary <root> --lang <lang> [--id <id>] [--name <name>] [--out <path>]`
— renders `repository-summary.md`. There is no persisted `KovanRepository`
catalogue yet, so the repository record is synthesised from `root`'s
directory name (or `--id`/`--name`) and `lang`.

```rust
pub fn run_summary(root: std::path::PathBuf, lang: super::LangArg, id: Option<String>, name: Option<String>, out: Option<std::path::PathBuf>) -> Result<(), String> { /* ... */ }
```

## Module `tokens`

`kovan-cli tokens` — per-commit API-token accounting.

A thin frontend over [`kovan_metrics::tokens`]. The write-side subcommands
are invoked by the git hooks (`.githooks/prepare-commit-msg` and
`post-commit`); `query` is the human/agent entry point for asking what a
window of history cost.

**Exit-code contract.** Every subcommand here exits `0`, including on
internal failure. The hooks must never block a commit because accounting
failed — a missing trailer is recoverable, a blocked commit is not.

```rust
pub mod tokens { /* ... */ }
```

### Types

#### Enum `TokensCommand`

Subcommands of `kovan-cli tokens`.

```rust
pub enum TokensCommand {
    Trailer {
        msgfile: std::path::PathBuf,
    },
    Record,
    Report,
    Init,
    Show,
    Query {
        from: Option<String>,
        to: Option<String>,
        branch: String,
        per_commit: bool,
        json: bool,
    },
}
```

##### Variants

###### `Trailer`

Append the `API-Usage-*` trailers to a commit message file
(`prepare-commit-msg`). Idempotent — safe on amend and rebase.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `msgfile` | `std::path::PathBuf` | Path to the commit message file git is preparing. |

###### `Record`

Advance the baseline and regenerate the ledger (`post-commit`).

###### `Report`

Regenerate `docs/token-usage.md` from the commit trailers.

###### `Init`

Stamp the baseline at the current cumulative reading (installer).

###### `Show`

Print the live cumulative reading and the delta since the last commit.

###### `Query`

Sum the usage recorded in commit trailers over a date window.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `from` | `Option<String>` | Window start, `DDMMYY` (day-month-year, 2-digit year). |
| `to` | `Option<String>` | Window end, `DDMMYY`. |
| `branch` | `String` | Branch to report on. |
| `per_commit` | `bool` | Include a per-commit breakdown. |
| `json` | `bool` | Emit JSON instead of the human-facing summary. |

##### Implementations

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

- **CastableFrom**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **FromArgMatches**
  - ```rust
    fn from_arg_matches(__clap_arg_matches: &clap::ArgMatches) -> ::std::result::Result<Self, clap::Error> { /* ... */ }
    ```

  - ```rust
    fn from_arg_matches_mut(__clap_arg_matches: &mut clap::ArgMatches) -> ::std::result::Result<Self, clap::Error> { /* ... */ }
    ```

  - ```rust
    fn update_from_arg_matches(self: &mut Self, __clap_arg_matches: &clap::ArgMatches) -> ::std::result::Result<(), clap::Error> { /* ... */ }
    ```

  - ```rust
    fn update_from_arg_matches_mut<''b>(self: &mut Self, __clap_arg_matches: &mut clap::ArgMatches) -> ::std::result::Result<(), clap::Error> { /* ... */ }
    ```

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

- **Subcommand**
  - ```rust
    fn augment_subcommands<''b>(__clap_app: clap::Command) -> clap::Command { /* ... */ }
    ```

  - ```rust
    fn augment_subcommands_for_update<''b>(__clap_app: clap::Command) -> clap::Command { /* ... */ }
    ```

  - ```rust
    fn has_subcommand(__clap_name: &str) -> bool { /* ... */ }
    ```

- **Sync**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
### Functions

#### Function `run`

Dispatch a `kovan-cli tokens` subcommand.

Only `query` can return an error (a malformed `DDMMYY`), because it is the
interactive path. The hook-facing subcommands always succeed.

```rust
pub fn run(command: TokensCommand) -> Result<(), String> { /* ... */ }
```

## Module `workspace`

Finding the OUTRAM PARK workspace, so `kovan` can be run from anywhere.

# Why discovery rather than a `--root .` default

A bare `--root .` default is only correct when the command happens to be run
from the workspace root. Run it one directory deeper — or from the home
directory, which is where a shell usually starts — and it fails with "no
`crates/`", which reads as a broken tool rather than a wrong working
directory.

# The order, and why it is this order

1. **An explicit `--root`.** If the caller named a path, that is the answer,
   and a wrong one is an error rather than something quietly overridden by a
   search. Predictability beats helpfulness here.
2. **The current directory, walking up.** Being *inside* the workspace is the
   strongest possible signal about which workspace is meant, and walking up
   means it works from any subdirectory.
3. **The home directory**, then **`Documents/`** and `Documents/research/`,
   which is where this workspace actually lives on the maintainer's machines.

If none matches, the error names every path tried and tells the caller to
pass `--root`. A discovery that fails silently, or picks something plausible
but wrong, would be worse than not discovering at all — this command
*writes* into the tree it finds.

# What counts as the workspace

A directory holding both `crates/` and a `Cargo.toml` declaring
`[workspace]`. Checking for the marker rather than the *name* means a clone
under any directory name is found, and an unrelated directory that happens to
be called `outram-park-backend` is not mistaken for one.

```rust
pub mod workspace { /* ... */ }
```

### Functions

#### Function `is_workspace_root`

Does `path` look like a Cargo workspace root with a `crates/` directory?

Both markers are required. `crates/` alone would match a random directory;
`[workspace]` alone would match any workspace, including one this command has
no business writing into.

```rust
pub fn is_workspace_root(path: &std::path::Path) -> bool { /* ... */ }
```

#### Function `resolve`

Resolve the workspace root.

`explicit` is the caller's `--root`, which wins outright when given. See the
module docs for the search order used otherwise.

Returns the root and a short phrase describing how it was found, so the
command can say which tree it is about to write into — discovery that does
not announce its result is discovery you cannot trust.

```rust
pub fn resolve(explicit: Option<&std::path::Path>) -> io::Result<(std::path::PathBuf, String)> { /* ... */ }
```

#### Function `output_dir`

Choose where a generated directory such as `agent-docs/` should live.

# The order

1. **An explicit `--out`.** The caller's path wins outright.
2. **Inside the workspace**, if one can be found — `<workspace>/agent-docs`.
   This is the normal case and keeps the bundle beside the code it describes,
   where the repository's `.gitignore` already covers it.
3. **`$HOME/<name>`**, then **`$HOME/Documents/<name>`**, for running the
   command with no workspace to hand.

Returns the directory and a phrase describing the choice, so the command can
say where it wrote — a generator that silently picks a location is one whose
output you then have to hunt for.

# A caution worth stating

Outside the workspace there is no `.gitignore` protecting the result. The
bundle is several megabytes of generated copies; if it is placed inside some
*other* repository, that repository will see it as new files to commit.

```rust
pub fn output_dir(explicit: Option<&std::path::Path>, name: &str) -> io::Result<(std::path::PathBuf, String)> { /* ... */ }
```

### Types

#### Enum `KindArg`

`clap`-facing mirror of [`FileKind`].

```rust
pub enum KindArg {
    Source,
    Markdown,
    Pdf,
    Metadata,
}
```

##### Variants

###### `Source`

###### `Markdown`

###### `Pdf`

###### `Metadata`

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> KindArg { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(k: KindArg) -> Self { /* ... */ }
    ```

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **ValueEnum**
  - ```rust
    fn value_variants<''a>() -> &'a [Self] { /* ... */ }
    ```

  - ```rust
    fn to_possible_value<''a>(self: &Self) -> ::std::option::Option<clap::builder::PossibleValue> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
#### Enum `LangArg`

`clap`-facing mirror of [`LanguageAdapter`].

```rust
pub enum LangArg {
    Rust,
    Cpp,
    Python,
    Fortran,
}
```

##### Variants

###### `Rust`

###### `Cpp`

###### `Python`

###### `Fortran`

##### Implementations

###### Methods

- ```rust
  pub fn display_name(self: Self) -> &'static str { /* ... */ }
  ```
  Human-readable language name, for a synthesised `KovanRepository.language`

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> LangArg { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(l: LangArg) -> Self { /* ... */ }
    ```

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **ValueEnum**
  - ```rust
    fn value_variants<''a>() -> &'a [Self] { /* ... */ }
    ```

  - ```rust
    fn to_possible_value<''a>(self: &Self) -> ::std::option::Option<clap::builder::PossibleValue> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
## Module `digitiser`

# Graph digitiser — extract `(x, y)` data points from plot images

Several validation targets in this project exist **only as figures in
papers** (HTR-10 safety-demonstration transients, MSRE reactivity-insertion
curves, the Tobias decay-heat plots). This module turns a raster image of a
published plot into numeric data points *with the provenance record that
makes them usable as validation evidence* (`DATA_POLICY.md`: digitisation
is a processing step and must be documented as one).

## What belongs in this module

- [`raster`] — loading a plot image into an owned RGB buffer (pure-Rust
  decoding via the `image` crate; PNG and JPEG).
- [`calibration`] — mapping pixel coordinates to data coordinates, with
  **linear and logarithmic axes independently per axis**. Log axes are
  calibrated in log10 space, never by linear pixel interpolation.
- [`detect`] — automatic detection of the plot frame (axis box) from dark
  line runs. Deterministic; no ML, no OCR (unlike [`table_ocr`] below,
  whose OCR use is a deliberate, separately-decided exception — see its
  own module doc).
- [`trace`] — automatic curve tracing by column scan, with enum-dispatched
  strategies ([`trace::TraceStrategy`]) and colour selectors
  ([`trace::CurveSelector`]).
- [`dataset`] — the output types. [`dataset::DigitisedDataset`] is
  deliberately impossible to construct or export without its
  [`calibration::PlotCalibration`] and [`dataset::FigureSource`] attached.
- [`auto`] — the one-shot automatic pipeline shared by all front ends.
- [`synthetic`] — deterministic rendering of known curves to images, used
  as self-consistency test fixtures (and later to cross-check the
  maintainer-supplied golden oracle, bead `op-amfh`).
- [`frontend`] — the shared `clap` argument surface used by `kovan-cli
  digitise` (the automatic-only path) and `kovan-tui`'s Digitiser tab
  (automatic pass, then interactive review). Compiled unconditionally:
  `clap` is already a hard dependency of this crate's own `kovan-cli`, so
  — unlike when this module lived in `kovan-literature`, where `clap` was
  optional — there is nothing left to gate.
- [`table_ocr`] — table digitisation (op-hnhp): OCR text recognition
  over a cropped table region via `kopitiam_ocr` (op-9bvi's engine
  decision), split into cells by a whitespace-run heuristic, with the
  same [`dataset::ReviewStatus`] human-review gate the plot digitiser
  uses. Compiled unconditionally — like `frontend`, it needs no GUI, so
  `kovan-cli`/`kovan-tui` could drive it too even though only the GUI
  does today.
- [`gui`] *(behind this crate's `gui` feature, default except on
  Android)* — the egui app powering the `kovan` binary, exposed as a
  library function (`gui::run`). Its `desktop` submodule also carries
  GitHub issue #30's file picker (`egui-file-dialog`, op-689u), Gruvbox
  theming (op-t5sq), and integrated PDF reader panel (op-95x6, over
  `kopitiam_pdf::mupdf` — see the next bullet).

## What does not belong here

- OCR / reading printed tick labels. KOVAN is deterministic and offline
  (no ML), so **numeric axis values must be supplied by the caller** (they
  are stated in the figure's caption/axes and are facts, not guesses); the
  pixel geometry is what gets automated. (GitHub issue #30 has since asked
  for OCR specifically for *table* digitisation, which is new ground for
  this crate and needs an explicit decision — tracked as bead `op-9bvi`,
  not yet made.)
- Network access of any kind.
- PDF *parsing* (text/metadata extraction) — that stays
  `kovan_literature::extract_metadata`'s job. This module's own PDF
  involvement is display-only: `gui`'s private `desktop::pdf_reader`
  submodule opens a PDF with `kopitiam_pdf::mupdf::PdfDocument` and
  rasterizes the current page
  with `kopitiam_pdf::mupdf::rasterize_page` (op-6ez3's rendering-engine
  decision) so it can be shown as a `kovan` GUI panel. It does not (yet)
  feed a rasterized page into the digitiser as a plot-image source —
  that's the draw-box-then-digitise interaction, a separate bead
  (op-p17q) this panel is built to support but does not itself implement.

## Units and `uom`

Digitised axes carry whatever units the source figure printed — often
non-SI, arbitrary, or normalised (e.g. "% of operating power",
"MeV/fission·s"). The engine therefore works in plain `f64` *document
units* and records the axis label text verbatim in
[`dataset::DigitisedDataset::x_label`]/`y_label`; converting into `uom`
quantities is the consumer's job, at the point where the unit is actually
interpreted. Forcing `uom` here would require inventing dimensions for
axes the engine cannot know.

## Verification status (honest limits)

The engine is verified by **synthetic self-consistency tests only**
(`tests/digitiser_synthetic.rs`): known curves are rendered to images at
known pixel positions, digitised, and compared against the analytic
values, for linear-linear, log-linear and log-log axes. Measured accuracy
figures live in that test file's doc comments. **No accuracy claim is made
against real published figures** — the hand-digitised golden oracle
(Tobias decay-heat points, bead `op-amfh`) does not exist yet. When it
lands, compare with [`synthetic`]-style tolerance checks against
[`dataset::DigitisedDataset`] output over the real scans.

```rust
pub mod digitiser { /* ... */ }
```

### Modules

## Module `auto`

One-shot automatic digitisation — the pipeline every front end shares.

Belongs here: [`AxisValueSpec`], [`AutoDigitiseConfig`], [`AxisPixelRefs`]
and [`auto_digitise`], which chain frame detection → calibration → trace →
dataset in one deterministic call. The CLI runs exactly this and nothing
more; the TUI/GUI run it as their "automatic pass first" and then let a
human correct the result.

Does not belong here: the individual algorithms (see [`super::detect`],
[`super::calibration`], [`super::trace`]) or any interactivity.

```rust
pub mod auto { /* ... */ }
```

### Types

#### Enum `AxisPixelRefs`

How the numeric axis values are anchored to pixels for one axis. Closed
set, enum-dispatched.

Tick-label OCR is deliberately out of scope (see the [`super`] module
doc), so the *values* always come from the caller; what varies is whether
the *pixels* they attach to come from automatic frame detection or are
given explicitly.

```rust
pub enum AxisPixelRefs {
    FrameEdges {
        min_value: f64,
        max_value: f64,
    },
    Explicit {
        r1: super::calibration::AxisRef,
        r2: super::calibration::AxisRef,
    },
}
```

##### Variants

###### `FrameEdges`

Anchor the values to the detected frame edges: `min_value` at the
frame's left (x axis) / bottom (y axis), `max_value` at its right /
top. The fully automatic path — correct whenever the figure's axis
extremes are labelled, which is the common case.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `min_value` | `f64` | Data value at the left/bottom frame edge. |
| `max_value` | `f64` | Data value at the right/top frame edge. |

###### `Explicit`

Two explicit pixel↔value pairs, e.g. read off gridline intersections.
Use when the curve is cropped oddly or the frame edges are unlabelled.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `r1` | `super::calibration::AxisRef` | First reference (pixel coordinate along this axis + its value). |
| `r2` | `super::calibration::AxisRef` | Second reference. |

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> AxisPixelRefs { /* ... */ }
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

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AxisPixelRefs) -> bool { /* ... */ }
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SerializableAny**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
#### Struct `AxisValueSpec`

Full specification of one axis: scale plus pixel anchoring.

```rust
pub struct AxisValueSpec {
    pub scale: super::calibration::AxisScale,
    pub refs: AxisPixelRefs,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `scale` | `super::calibration::AxisScale` | Linear or logarithmic. |
| `refs` | `AxisPixelRefs` | Where the values sit in pixel space. |

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> AxisValueSpec { /* ... */ }
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

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AxisValueSpec) -> bool { /* ... */ }
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SerializableAny**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
#### Struct `AutoDigitiseConfig`

Everything the automatic pipeline needs besides the image and the
provenance strings.

```rust
pub struct AutoDigitiseConfig {
    pub x: AxisValueSpec,
    pub y: AxisValueSpec,
    pub detect: super::detect::DetectConfig,
    pub trace: super::trace::TraceConfig,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `AxisValueSpec` | x-axis specification. |
| `y` | `AxisValueSpec` | y-axis specification. |
| `detect` | `super::detect::DetectConfig` | Frame-detection tuning. |
| `trace` | `super::trace::TraceConfig` | Curve-trace tuning. |

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> AutoDigitiseConfig { /* ... */ }
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

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AutoDigitiseConfig) -> bool { /* ... */ }
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SerializableAny**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
### Functions

#### Function `auto_digitise`

Run the full automatic pipeline: detect (or derive) the frame, build the
calibration, trace the curve, and package a [`DigitisedDataset`] with the
complete provenance record. Deterministic: same raster + config +
provenance strings → identical dataset.

Frame detection is skipped only when **both** axes use
[`AxisPixelRefs::Explicit`] *and* automatic detection fails — in that case
the trace region falls back to the rectangle spanned by the explicit
reference pixels. When either axis anchors to
[`AxisPixelRefs::FrameEdges`], detection must succeed.

`digitised_by`/`digitised_at` are recorded verbatim; pass
[`super::dataset::utc_now_iso8601`] for `digitised_at` unless a
reproducible stamp is required. The returned dataset is always
[`super::dataset::ReviewStatus::Unreviewed`].

# Errors

Any [`DigitiserError`] from detection, calibration, or tracing.

```rust
pub fn auto_digitise</* synthetic */ impl Into<String>: Into<String>, /* synthetic */ impl Into<String>: Into<String>, /* synthetic */ impl Into<String>: Into<String>, /* synthetic */ impl Into<String>: Into<String>>(raster: &super::raster::PlotRaster, config: &AutoDigitiseConfig, source: super::dataset::FigureSource, x_label: impl Into<String>, y_label: impl Into<String>, digitised_by: impl Into<String>, digitised_at: impl Into<String>) -> Result<super::dataset::DigitisedDataset, super::DigitiserError> { /* ... */ }
```

## Module `calibration`

Axis calibration — mapping pixel coordinates to data coordinates.

Belongs here: [`AxisScale`], [`AxisRef`], [`AxisCalibration`],
[`PlotCalibration`], and the pixel ↔ data-value maps. Logarithmic axes are
interpolated in **log10 space** — the pixel position of a value on a log
axis is affine in `log10(value)`, not in the value itself, and getting
this wrong is the classic digitisation error this module exists to avoid.

Does not belong here: image handling ([`super::raster`]), curve extraction
([`super::trace`]), output formats ([`super::dataset`]).

```rust
pub mod calibration { /* ... */ }
```

### Types

#### Enum `AxisScale`

Whether an axis is linear or logarithmic. Closed set — enum-dispatched per
the workspace Rust design rules.

```rust
pub enum AxisScale {
    Linear,
    Logarithmic,
}
```

##### Variants

###### `Linear`

Value is an affine function of pixel position.

###### `Logarithmic`

`log10(value)` is an affine function of pixel position (decade-ruled
axis). Reference values must be strictly positive.

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> AxisScale { /* ... */ }
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

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Display**
  - ```rust
    fn fmt(self: &Self, f: &mut std::fmt::Formatter<''_>) -> std::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AxisScale) -> bool { /* ... */ }
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SerializableAny**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
- **ToCompactString**
  - ```rust
    fn try_to_compact_string(self: &Self) -> Result<CompactString, ToCompactStringError> { /* ... */ }
    ```

- **ToLine**
  - ```rust
    fn to_line(self: &Self) -> Line<''_> { /* ... */ }
    ```

- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **ToSmolStr**
  - ```rust
    fn to_smolstr(self: &Self) -> SmolStr { /* ... */ }
    ```

- **ToSpan**
  - ```rust
    fn to_span(self: &Self) -> Span<''_> { /* ... */ }
    ```

- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **ToText**
  - ```rust
    fn to_text(self: &Self) -> Text<''_> { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
#### Struct `AxisRef`

One axis reference point: a pixel coordinate along the axis direction
(column index for the x axis, row index for the y axis) paired with the
data value the figure assigns to that pixel.

`pixel` is an `f64` because reference points may be placed with sub-pixel
precision (e.g. the centre of a 2-px-thick axis line). `value` is in
*document units* — whatever the source figure's axis label says (see the
module doc of [`super`] for why `uom` is not used here).

```rust
pub struct AxisRef {
    pub pixel: f64,
    pub value: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pixel` | `f64` | Pixel coordinate along this axis (x axis → column, y axis → row;<br>image rows increase downward). |
| `value` | `f64` | Data value at that pixel, in the figure's own units. |

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> AxisRef { /* ... */ }
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

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AxisRef) -> bool { /* ... */ }
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SerializableAny**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
#### Struct `AxisCalibration`

Calibration of a single axis from two reference points.

Construct with [`AxisCalibration::new`], which validates the references;
the fields stay public so a deserialised calibration can be inspected, but
prefer the constructor for anything built at runtime.

```rust
pub struct AxisCalibration {
    pub scale: AxisScale,
    pub r1: AxisRef,
    pub r2: AxisRef,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `scale` | `AxisScale` | Linear or logarithmic interpolation between the reference points. |
| `r1` | `AxisRef` | First reference point. |
| `r2` | `AxisRef` | Second reference point. Must differ from `r1` in both pixel and value. |

##### Implementations

###### Methods

- ```rust
  pub fn new(scale: AxisScale, r1: AxisRef, r2: AxisRef) -> Result<Self, DigitiserError> { /* ... */ }
  ```
  Build a validated axis calibration.

- ```rust
  pub fn value_at(self: &Self, pixel: f64) -> f64 { /* ... */ }
  ```
  Data value at pixel coordinate `pixel`, in the figure's own units.

- ```rust
  pub fn pixel_at(self: &Self, value: f64) -> Option<f64> { /* ... */ }
  ```
  Pixel coordinate at which `value` sits on this axis — the inverse of

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> AxisCalibration { /* ... */ }
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

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AxisCalibration) -> bool { /* ... */ }
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SerializableAny**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
#### Struct `PlotCalibration`

Full two-axis calibration of a plot: an [`AxisCalibration`] for x (pixel
columns) and one for y (pixel rows; rows increase *downward*, which the
two-point form handles with no special casing — the bottom-of-plot
reference simply has the larger row index).

```rust
pub struct PlotCalibration {
    pub x: AxisCalibration,
    pub y: AxisCalibration,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `AxisCalibration` | Horizontal axis (pixel columns → data x). |
| `y` | `AxisCalibration` | Vertical axis (pixel rows → data y). |

##### Implementations

###### Methods

- ```rust
  pub fn point_at(self: &Self, x_px: f64, y_px: f64) -> (f64, f64) { /* ... */ }
  ```
  Map an image pixel `(column, row)` to data coordinates `(x, y)`.

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> PlotCalibration { /* ... */ }
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

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PlotCalibration) -> bool { /* ... */ }
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SerializableAny**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
## Module `dataset`

Digitised datasets — the output type, with mandatory provenance.

Belongs here: [`FigureSource`], [`PointOrigin`], [`ReviewStatus`],
[`DigitisedPoint`], [`TraceRecord`], [`DigitisedDataset`], and their
JSON/CSV export. The design rule (from `DATA_POLICY.md`: digitisation is
a processing step and must be documented as one) is that **a dataset
cannot exist, be serialised, or be exported without its calibration and
source record** — [`DigitisedDataset`]'s calibration and source are plain
required fields, there is no points-only constructor, and both exporters
read them from the struct itself.

Does not belong here: pixel scanning ([`super::trace`]), calibration math
([`super::calibration`]), or interactive editing (the TUI/GUI binaries own
that, and record their edits *into* these types).

```rust
pub mod dataset { /* ... */ }
```

### Types

#### Struct `FigureSource`

Where the digitised figure came from — the document-level half of the
provenance record.

`document_id`/`document_title` should reference the figure's
[`crate::KovanDocument`] (its `id` and `title`) when the source has been
catalogued into the KOVAN literature archive; they stay `None` for a
not-yet-catalogued source, in which case `image_path` at least pins the
file that was digitised.

```rust
pub struct FigureSource {
    pub document_id: Option<String>,
    pub document_title: Option<String>,
    pub figure: String,
    pub page: Option<u32>,
    pub image_path: Option<String>,
    pub image_sha256: Option<String>,
    pub notes: Option<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `document_id` | `Option<String>` | [`crate::KovanDocument::id`] of the catalogued source document, if<br>catalogued. |
| `document_title` | `Option<String>` | [`crate::KovanDocument::title`] (or a free-text citation) of the<br>source document. |
| `figure` | `String` | Figure designation as printed, e.g. `"Fig. 7"` or `"Figure 3(b)"`.<br>Required — a digitisation that cannot say which figure it read is not<br>usable as evidence. |
| `page` | `Option<u32>` | Page number the figure appears on, if known. |
| `image_path` | `Option<String>` | Path of the image file that was digitised (as given by the caller). |
| `image_sha256` | `Option<String>` | Lowercase-hex SHA-256 of the image file's bytes, so the exact raster<br>this dataset was read from can be re-identified. Filled automatically<br>when the raster was loaded from a file. |
| `notes` | `Option<String>` | Free-text notes (e.g. "curve labelled '235U thermal'", crop applied,<br>known scan skew). |

##### Implementations

###### Methods

- ```rust
  pub fn new</* synthetic */ impl Into<String>: Into<String>>(figure: impl Into<String>) -> Result<Self, DigitiserError> { /* ... */ }
  ```
  Minimal source record: just the figure designation. Fill the optional

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FigureSource { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FigureSource) -> bool { /* ... */ }
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SerializableAny**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
#### Enum `PointOrigin`

How a single point came to be — automatic, hand-placed, or hand-corrected.
Closed set, enum-dispatched; recorded per point so a reviewer can see
exactly which values a human touched.

```rust
pub enum PointOrigin {
    AutoTraced,
    HandPlaced {
        by: String,
    },
    HandCorrected {
        by: String,
    },
}
```

##### Variants

###### `AutoTraced`

Emitted by the automatic tracer, untouched by a human.

###### `HandPlaced`

Placed by a human (TUI/GUI editing), never produced by the tracer.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `by` | `String` | Who placed it (operator name as given to the front end). |

###### `HandCorrected`

Auto-traced, then moved by a human.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `by` | `String` | Who corrected it. |

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> PointOrigin { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PointOrigin) -> bool { /* ... */ }
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SerializableAny**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
#### Enum `ReviewInterface`

Which front end a human review happened in. Closed set.

```rust
pub enum ReviewInterface {
    Tui,
    Gui,
    External,
}
```

##### Variants

###### `Tui`

`kovan-tui`'s Digitiser tab (ratatui).

###### `Gui`

`kovan`, the GUI (egui).

###### `External`

Reviewed outside the shipped front ends (e.g. plotted and inspected by
hand); the reviewer takes responsibility for the method.

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ReviewInterface { /* ... */ }
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

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ReviewInterface) -> bool { /* ... */ }
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SerializableAny**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
#### Enum `ReviewStatus`

Whether a human has verified this dataset. The automatic CLI always emits
[`ReviewStatus::Unreviewed`]; only the hybrid front ends (or an external
reviewer) may record a review, and the record says who, when, and where —
**confirmation is recorded, never assumed**.

```rust
pub enum ReviewStatus {
    Unreviewed,
    Reviewed {
        by: String,
        at: String,
        interface: ReviewInterface,
    },
}
```

##### Variants

###### `Unreviewed`

No human has checked the points against the figure.

###### `Reviewed`

A human inspected the points overlaid on the figure and accepted them.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `by` | `String` | Reviewer name. |
| `at` | `String` | UTC timestamp, ISO 8601 (`YYYY-MM-DDTHH:MM:SSZ`). |
| `interface` | `ReviewInterface` | Front end the review happened in. |

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ReviewStatus { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ReviewStatus) -> bool { /* ... */ }
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SerializableAny**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
#### Struct `DigitisedPoint`

One digitised data point, in the figure's own units, with its reading
uncertainty and per-point origin.

Uncertainties are stored as separate `minus`/`plus` magnitudes (both
`>= 0`) because on a logarithmic axis the pixel reading error maps to an
**asymmetric, value-dependent** interval — collapsing it to one symmetric
number would misstate exactly the case (log-log decay-heat curves) this
tool exists for.

```rust
pub struct DigitisedPoint {
    pub x: f64,
    pub y: f64,
    pub x_minus: f64,
    pub x_plus: f64,
    pub y_minus: f64,
    pub y_plus: f64,
    pub x_px: Option<f64>,
    pub y_px: Option<f64>,
    pub origin: PointOrigin,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `f64` | Data x value, in the figure's x-axis units. |
| `y` | `f64` | Data y value, in the figure's y-axis units. |
| `x_minus` | `f64` | Magnitude of the downward x reading uncertainty: the value could be as<br>low as `x - x_minus`. |
| `x_plus` | `f64` | Magnitude of the upward x reading uncertainty. |
| `y_minus` | `f64` | Magnitude of the downward y reading uncertainty. |
| `y_plus` | `f64` | Magnitude of the upward y reading uncertainty. |
| `x_px` | `Option<f64>` | Pixel column this point sits at (kept so the TUI/GUI can re-overlay<br>the point on the image; `None` only for hand-placed points created in<br>data space). |
| `y_px` | `Option<f64>` | Pixel row this point sits at. |
| `origin` | `PointOrigin` | How the point came to be. |

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> DigitisedPoint { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DigitisedPoint) -> bool { /* ... */ }
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SerializableAny**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
#### Struct `TraceRecord`

Record of the automatic pass that produced the auto-traced points: the
exact configuration, so the run can be reproduced bit-for-bit.

```rust
pub struct TraceRecord {
    pub engine: String,
    pub config: super::trace::TraceConfig,
    pub frame: super::detect::PixelRect,
    pub frame_auto_detected: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `engine` | `String` | Engine identifier and version, e.g.<br>`"kovan graph digitiser 0.0.0"`. |
| `config` | `super::trace::TraceConfig` | The full trace configuration used. |
| `frame` | `super::detect::PixelRect` | The pixel frame the trace ran inside. |
| `frame_auto_detected` | `bool` | `true` when the frame came from automatic detection,<br>`false` when the caller supplied it. |

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> TraceRecord { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TraceRecord) -> bool { /* ... */ }
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SerializableAny**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
#### Struct `DigitisedDataset`

A complete digitised dataset: points **plus** the calibration, source,
operator, and review records that make them usable as validation evidence.

There is deliberately no way to build or export one without calibration
and source — they are required fields of the only constructors
([`DigitisedDataset::from_pixel_trace`] and deserialisation of a
previously exported record), and both exporters embed them.

```rust
pub struct DigitisedDataset {
    pub schema_version: u32,
    pub source: FigureSource,
    pub calibration: super::calibration::PlotCalibration,
    pub x_label: String,
    pub y_label: String,
    pub digitised_by: String,
    pub digitised_at: String,
    pub trace: Option<TraceRecord>,
    pub review: ReviewStatus,
    pub points: Vec<DigitisedPoint>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `schema_version` | `u32` | Schema version of this record ([`DATASET_SCHEMA_VERSION`]). |
| `source` | `FigureSource` | Which document and figure the points were read from. |
| `calibration` | `super::calibration::PlotCalibration` | The axis calibration every point was computed with (reference points,<br>linear/log per axis). |
| `x_label` | `String` | x-axis label as printed on the figure, units included, e.g.<br>`"Time after fission burst (s)"`. |
| `y_label` | `String` | y-axis label as printed on the figure, units included. |
| `digitised_by` | `String` | Who ran the digitisation (a person, or e.g.<br>`"kovan-cli digitise (automatic)"` for the unattended CLI). |
| `digitised_at` | `String` | UTC timestamp of the digitisation, ISO 8601. |
| `trace` | `Option<TraceRecord>` | The automatic pass that produced the auto-traced points; `None` for a<br>dataset built entirely by hand in a front end. |
| `review` | `ReviewStatus` | Human verification state. Starts [`ReviewStatus::Unreviewed`]. |
| `points` | `Vec<DigitisedPoint>` | The points, in increasing-x order as traced. |

##### Implementations

###### Methods

- ```rust
  pub fn from_pixel_trace</* synthetic */ impl Into<String>: Into<String>, /* synthetic */ impl Into<String>: Into<String>, /* synthetic */ impl Into<String>: Into<String>, /* synthetic */ impl Into<String>: Into<String>>(source: FigureSource, calibration: PlotCalibration, x_label: impl Into<String>, y_label: impl Into<String>, digitised_by: impl Into<String>, digitised_at: impl Into<String>, trace_record: TraceRecord, trace_points: &[PixelTracePoint]) -> Self { /* ... */ }
  ```
  Convert a pixel-space trace into a data-space dataset.

- ```rust
  pub fn to_json_string(self: &Self) -> String { /* ... */ }
  ```
  Serialise to pretty-printed JSON — the canonical on-disk form; feed it

- ```rust
  pub fn from_json_str(json: &str) -> Result<Self, DigitiserError> { /* ... */ }
  ```
  Parse a dataset previously written by [`DigitisedDataset::to_json_string`].

- ```rust
  pub fn write_json(self: &Self, path: &Path) -> Result<(), DigitiserError> { /* ... */ }
  ```
  Write the JSON form to `path`.

- ```rust
  pub fn read_json(path: &Path) -> Result<Self, DigitiserError> { /* ... */ }
  ```
  Read a JSON dataset from `path`.

- ```rust
  pub fn to_csv_string(self: &Self) -> String { /* ... */ }
  ```
  Serialise to CSV with the **full provenance record embedded** as `#`

- ```rust
  pub fn write_csv(self: &Self, path: &Path) -> Result<(), DigitiserError> { /* ... */ }
  ```
  Write the CSV form to `path`.

- ```rust
  pub fn record_review</* synthetic */ impl Into<String>: Into<String>, /* synthetic */ impl Into<String>: Into<String>>(self: &mut Self, by: impl Into<String>, at: impl Into<String>, interface: ReviewInterface) { /* ... */ }
  ```
  Record a human review — called by the hybrid front ends after the

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> DigitisedDataset { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DigitisedDataset) -> bool { /* ... */ }
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SerializableAny**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
### Functions

#### Function `uncertainty_interval`

Map a `± half_pixels` pixel reading error at `pixel` through an axis
calibration, returning `(minus, plus)` magnitudes in data units (both
`>= 0`).

On a linear axis the two magnitudes are equal; on a logarithmic axis they
are asymmetric and grow with the value — which is why they are computed by
evaluating the calibration at `pixel ± half_pixels` rather than by a
constant scale factor.

```rust
pub fn uncertainty_interval(axis: &super::calibration::AxisCalibration, pixel: f64, half_pixels: f64) -> (f64, f64) { /* ... */ }
```

#### Function `utc_now_iso8601`

Current UTC time as an ISO 8601 string (`YYYY-MM-DDTHH:MM:SSZ`), from the
system clock and pure `std` (no chrono dependency). Used by the binaries
to stamp `digitised_at` / review times; pass an explicit string instead
when reproducible output is needed (the CLI's `--timestamp` flag).

```rust
pub fn utc_now_iso8601() -> String { /* ... */ }
```

### Constants and Statics

#### Constant `DATASET_SCHEMA_VERSION`

Version stamp written into every serialised dataset so future readers can
tell what they are looking at. Bump on breaking schema changes.

```rust
pub const DATASET_SCHEMA_VERSION: u32 = 1;
```

## Module `detect`

Automatic plot-frame detection — finding the axis box in pixel space.

Belongs here: [`PixelRect`], [`DetectConfig`], and
[`detect_plot_frame`], which locates the rectangle bounded by the plot's
axis lines by scanning for long dark horizontal/vertical pixel runs.
Deterministic; no ML, no OCR — it finds *where* the axes are, never what
their tick labels say (the caller supplies the numeric axis values, see
the [`super`] module doc).

Does not belong here: calibration values ([`super::calibration`]) or curve
pixels ([`super::trace`]).

```rust
pub mod detect { /* ... */ }
```

### Types

#### Struct `PixelRect`

An axis-aligned pixel rectangle, inclusive on all four edges.

Rows increase downward, so `top < bottom` numerically while `top` is the
visually upper edge.

```rust
pub struct PixelRect {
    pub left: u32,
    pub right: u32,
    pub top: u32,
    pub bottom: u32,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `left` | `u32` | Leftmost column (inclusive). |
| `right` | `u32` | Rightmost column (inclusive). Always `> left`. |
| `top` | `u32` | Topmost row (inclusive; visually the upper edge). |
| `bottom` | `u32` | Bottommost row (inclusive; visually the lower edge). Always `> top`. |

##### Implementations

###### Methods

- ```rust
  pub fn width(self: &Self) -> u32 { /* ... */ }
  ```
  Width in pixels (inclusive of both edges).

- ```rust
  pub fn height(self: &Self) -> u32 { /* ... */ }
  ```
  Height in pixels (inclusive of both edges).

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> PixelRect { /* ... */ }
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

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PixelRect) -> bool { /* ... */ }
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SerializableAny**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
#### Struct `DetectConfig`

Tuning knobs for [`detect_plot_frame`]. [`DetectConfig::default`] suits
typical black-on-white published figures.

```rust
pub struct DetectConfig {
    pub dark_threshold: u8,
    pub min_line_fraction: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `dark_threshold` | `u8` | A pixel with Rec. 709 luminance strictly below this counts as "dark"<br>(axis-line ink). Default 128 — the midpoint, tolerant of grey<br>anti-aliasing and scan noise. |
| `min_line_fraction` | `f64` | A row/column is an axis-line candidate when its longest contiguous<br>dark run covers at least this fraction of the image's<br>width/height. Default 0.4 — axis lines span most of a cropped figure;<br>curve segments and tick marks do not. |

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> DetectConfig { /* ... */ }
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

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **NoneValue**
  - ```rust
    fn null_value() -> T { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DetectConfig) -> bool { /* ... */ }
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

- **Read**
- **ReadPrimitive**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SerializableAny**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
### Functions

#### Function `detect_plot_frame`

Detect the plot frame (axis box) of a black-on-white figure.

**Method (deterministic).** Every row's and column's longest contiguous
dark run is measured. Rows/columns whose run covers at least
[`DetectConfig::min_line_fraction`] of the image dimension are axis-line
candidates. With two or more candidate rows *and* columns (a fully boxed
plot), the frame is their outermost members. With exactly one of either
(an L-shaped plot: one x axis, one y axis), the missing top/right edges
are taken from the dark extent of the detected axis lines themselves.

# Errors

[`DigitiserError::Detection`] when no candidate row or column exists, or
the resulting rectangle is degenerate (under 10 px in either direction) —
in that case supply explicit pixel reference points instead (see
[`super::auto::AxisPixelRefs`]).

```rust
pub fn detect_plot_frame(raster: &super::raster::PlotRaster, config: &DetectConfig) -> Result<PixelRect, super::DigitiserError> { /* ... */ }
```

## Module `frontend`

Shared command-line surface for the digitiser front ends.

Belongs here: [`AutoArgs`] — the `clap` argument set that fully describes
one automatic digitisation run — and [`AutoArgs::run`], which executes it.
`kovan-cli digitise` (`src/bin/kovan-cli.rs`) parses these flags directly
via `#[command(flatten)]`; `kovan-tui`'s Digitiser tab
(`src/tui/digitiser.rs`) builds the same struct programmatically from its
Setup form, so a TUI session's automatic pass can always be re-run
headlessly by pasting the equivalent flags onto `kovan-cli digitise`.

Does not belong here: any interactivity (the TUI tab owns that) or the
pipeline itself ([`super::auto`]).

Compiled unconditionally, no feature gate — `clap` is already a hard
dependency of this crate's own `kovan-cli`, unlike when this module lived
in `kovan-literature` (moved 2026-08-21, see this crate's `NOTICE`), where
`clap` was optional and this module was gated behind `digitise-cli` /
`digitise-tui`.

```rust
pub mod frontend { /* ... */ }
```

### Types

#### Struct `AutoArgs`

Arguments for one automatic digitisation pass.

Axis values are supplied by the caller (read from the figure's printed
labels — tick-label OCR is deliberately out of scope, see the
[`super`] module doc); pixel geometry is automatic unless explicit
`--x-ref`/`--y-ref` pairs are given.

```rust
pub struct AutoArgs {
    pub image: String,
    pub x_scale: String,
    pub y_scale: String,
    pub x_range: Option<String>,
    pub y_range: Option<String>,
    pub x_ref: Vec<String>,
    pub y_ref: Vec<String>,
    pub figure: String,
    pub document_id: Option<String>,
    pub document_title: Option<String>,
    pub page: Option<u32>,
    pub notes: Option<String>,
    pub x_label: String,
    pub y_label: String,
    pub operator: String,
    pub timestamp: Option<String>,
    pub strategy: String,
    pub step: u32,
    pub threshold: u8,
    pub curve_rgb: Option<String>,
    pub curve_tolerance: u16,
    pub inset: u32,
    pub max_column_fill: f64,
    pub dark_threshold: u8,
    pub min_line_fraction: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `image` | `String` | Path to the plot image (PNG or JPEG). |
| `x_scale` | `String` | x-axis scale: `linear` or `log`. |
| `y_scale` | `String` | y-axis scale: `linear` or `log`. |
| `x_range` | `Option<String>` | Data values at the detected frame's left and right edges, as<br>`min,max` (e.g. `--x-range 1,1e6`). Mutually exclusive with `--x-ref`. |
| `y_range` | `Option<String>` | Data values at the detected frame's bottom and top edges, as<br>`min,max`. Mutually exclusive with `--y-ref`. |
| `x_ref` | `Vec<String>` | Explicit x reference point as `pixel=value`; give exactly twice<br>(e.g. `--x-ref 57=1 --x-ref 462=1000`). Overrides `--x-range`. |
| `y_ref` | `Vec<String>` | Explicit y reference point as `pixel=value` (pixel row, growing<br>downward); give exactly twice. Overrides `--y-range`. |
| `figure` | `String` | Figure designation as printed, e.g. `"Fig. 7"`. Required provenance. |
| `document_id` | `Option<String>` | `KovanDocument` id of the catalogued source, if any. |
| `document_title` | `Option<String>` | Source document title / free-text citation. |
| `page` | `Option<u32>` | Page the figure appears on. |
| `notes` | `Option<String>` | Free-text provenance notes (crop, curve label, known skew…). |
| `x_label` | `String` | x-axis label as printed (units included). |
| `y_label` | `String` | y-axis label as printed (units included). |
| `operator` | `String` | Operator recorded as `digitised_by`. |
| `timestamp` | `Option<String>` | Override the `digitised_at` timestamp (ISO 8601) for byte-reproducible<br>output; defaults to the current UTC time. |
| `strategy` | `String` | Trace strategy: `continuity` (default), `largest-run`, or `centroid`. |
| `step` | `u32` | Sample every Nth pixel column. |
| `threshold` | `u8` | Curve-ink luminance threshold (0–255); ignored with `--curve-rgb`. |
| `curve_rgb` | `Option<String>` | Trace a specific curve colour, as `r,g,b` (0–255 each). |
| `curve_tolerance` | `u16` | RGB distance tolerance for `--curve-rgb`. |
| `inset` | `u32` | Pixels to shrink the frame inward before tracing. |
| `max_column_fill` | `f64` | Skip columns whose ink fill exceeds this fraction (vertical gridlines). |
| `dark_threshold` | `u8` | Frame detection: luminance below this is axis ink. |
| `min_line_fraction` | `f64` | Frame detection: min dark-run fraction of the image dimension. |

##### Implementations

###### Methods

- ```rust
  pub fn run(self: &Self) -> Result<(PlotRaster, DigitisedDataset), DigitiserError> { /* ... */ }
  ```
  Load the image and run the automatic pipeline, returning the raster

- ```rust
  pub fn pipeline_config(self: &Self) -> Result<AutoDigitiseConfig, DigitiserError> { /* ... */ }
  ```
  Build the [`AutoDigitiseConfig`] these arguments describe.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Args**
  - ```rust
    fn group_id() -> Option<clap::Id> { /* ... */ }
    ```

  - ```rust
    fn augment_args<''b>(__clap_app: clap::Command) -> clap::Command { /* ... */ }
    ```

  - ```rust
    fn augment_args_for_update<''b>(__clap_app: clap::Command) -> clap::Command { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> AutoArgs { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **CommandFactory**
  - ```rust
    fn command<''b>() -> clap::Command { /* ... */ }
    ```

  - ```rust
    fn command_for_update<''b>() -> clap::Command { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **FromArgMatches**
  - ```rust
    fn from_arg_matches(__clap_arg_matches: &clap::ArgMatches) -> ::std::result::Result<Self, clap::Error> { /* ... */ }
    ```

  - ```rust
    fn from_arg_matches_mut(__clap_arg_matches: &mut clap::ArgMatches) -> ::std::result::Result<Self, clap::Error> { /* ... */ }
    ```

  - ```rust
    fn update_from_arg_matches(self: &mut Self, __clap_arg_matches: &clap::ArgMatches) -> ::std::result::Result<(), clap::Error> { /* ... */ }
    ```

  - ```rust
    fn update_from_arg_matches_mut(self: &mut Self, __clap_arg_matches: &mut clap::ArgMatches) -> ::std::result::Result<(), clap::Error> { /* ... */ }
    ```

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Parser**
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
### Functions

#### Function `parse_scale`

Parse `linear` / `log` (also accepts `lin` / `logarithmic`).

```rust
pub fn parse_scale(s: &str) -> Result<super::calibration::AxisScale, super::DigitiserError> { /* ... */ }
```

#### Function `parse_strategy`

Parse a trace strategy name.

```rust
pub fn parse_strategy(s: &str) -> Result<super::trace::TraceStrategy, super::DigitiserError> { /* ... */ }
```

## Module `gui`

**Attributes:**

- `Other("#[attr = CfgTrace([NameValue { name: \"feature\", value: Some(\"gui\"), span: crates/kovan/src/digitiser/mod.rs:100:7: 100:22 (#0) }])]")`

Egui-based hybrid digitiser GUI (graphreader-style), exposed as a library
function so more than one binary can open the same window.

**Automatic pass first, then human verification — recorded, not
assumed.** The interaction model follows graphreader.com: load a plot
image; click two reference points per axis and type their values; choose
linear or log per axis; auto-trace the curve; then drag / add / delete
individual points with the mouse; finally mark the dataset reviewed and
export. Every hand edit is recorded per point (`HandPlaced` /
`HandCorrected` with the operator name), any edit after a review resets
the status to `UNREVIEWED`, and the export always carries the full
calibration + provenance record.

Desktop-only by policy: this module only compiles under this crate's
default `gui` feature (default everywhere except Android — see this
crate's `Cargo.toml`), and its egui/eframe dependencies are target-gated
off Android; [`run`] itself branches internally so its one caller — the
`kovan` binary — gets Android-safe behaviour for free.

(Was `digitise-gui`, called from a now-retired `kovan-digitise-gui`
binary, before the digitiser moved from `kovan-literature` into this
crate 2026-08-21 — see this crate's `NOTICE`. The wrapper binary was named
`kovan-gui` at that point too, then renamed to plain `kovan` the same day
per GitHub issue #30's final 3-binary spec — `kovan` (GUI), `kovan-cli`
(agent CLI), `kovan-tui` (terminal UI).)

```rust
pub mod gui { /* ... */ }
```

### Functions

#### Function `run`

**Attributes:**

- `Other("#[attr = CfgTrace([Not(NameValue { name: \"target_os\", value: Some(\"android\"), span: crates/kovan/src/digitiser/gui/mod.rs:41:11: 41:32 (#0) }, crates/kovan/src/digitiser/gui/mod.rs:41:10: 41:33 (#0))])]")`

Open the digitiser window, optionally pre-loading `image_arg` as the plot
image. Blocks until the window is closed.

```rust
pub fn run(image_arg: Option<String>) -> Result<(), String> { /* ... */ }
```

## Module `raster`

Plot image loading — an owned RGB pixel buffer decoded with pure Rust.

Belongs here: [`PlotRaster`] (the in-memory image the whole digitiser
works on) and its constructors. Decoding uses the `image` crate's
pure-Rust PNG/JPEG decoders — no C toolchain, no system libraries, so the
engine builds natively on Termux/Android.

Does not belong here: axis geometry ([`super::detect`]), curve pixels
([`super::trace`]), or any pixel *interpretation* beyond luminance.

```rust
pub mod raster { /* ... */ }
```

### Types

#### Struct `PlotRaster`

An owned, row-major RGB8 plot image.

The public API deliberately does not expose `image`-crate types, so a
caller only needs this struct and plain integers to work with the
digitiser (workspace "human interface layer" rule).

```rust
pub struct PlotRaster {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn from_path(path: &Path) -> Result<Self, DigitiserError> { /* ... */ }
  ```
  Decode a plot image from a file on disk (PNG or JPEG).

- ```rust
  pub fn from_bytes(bytes: &[u8]) -> Result<Self, DigitiserError> { /* ... */ }
  ```
  Decode a plot image from in-memory encoded bytes (PNG or JPEG).

- ```rust
  pub fn from_rgb_fn</* synthetic */ impl Fn(u32, u32) -> [u8; 3]: Fn(u32, u32) -> [u8; 3]>(width: u32, height: u32, f: impl Fn(u32, u32) -> [u8; 3]) -> Self { /* ... */ }
  ```
  Build a raster from a pixel generator function — used by

- ```rust
  pub fn width(self: &Self) -> u32 { /* ... */ }
  ```
  Image width in pixels (number of columns).

- ```rust
  pub fn height(self: &Self) -> u32 { /* ... */ }
  ```
  Image height in pixels (number of rows).

- ```rust
  pub fn rgb(self: &Self, x: u32, y: u32) -> [u8; 3] { /* ... */ }
  ```
  RGB triple at column `x`, row `y` (row 0 is the top of the image).

- ```rust
  pub fn luminance(self: &Self, x: u32, y: u32) -> u8 { /* ... */ }
  ```
  Rec. 709 luminance of the pixel at `(x, y)`, 0 (black) – 255 (white).

- ```rust
  pub fn source_sha256(self: &Self) -> Option<&str> { /* ... */ }
  ```
  Lowercase-hex SHA-256 of the encoded source bytes, when this raster

- ```rust
  pub fn to_png_bytes(self: &Self) -> Result<Vec<u8>, DigitiserError> { /* ... */ }
  ```
  Encode this raster as PNG bytes (pure Rust). Used to write synthetic

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> PlotRaster { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PlotRaster) -> bool { /* ... */ }
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
## Module `synthetic`

Synthetic plot rendering — deterministic ground-truth fixtures.

Belongs here: [`SyntheticPlotSpec`] and [`render_synthetic_plot`], which
draw a *known analytic curve* into a [`PlotRaster`] at known pixel
positions and return the exact [`PlotCalibration`] used. The
self-consistency tests (`tests/digitiser_synthetic.rs`) digitise these
images and compare the recovered points against the analytic function —
the only ground truth available until the maintainer-supplied golden
oracle (bead `op-amfh`) lands. Keeping the renderer public also lets that
future oracle comparison reuse the same tolerance machinery.

Does not belong here: any digitising. This module only *makes* images.

```rust
pub mod synthetic { /* ... */ }
```

### Types

#### Struct `SyntheticPlotSpec`

Description of a synthetic plot: image size, frame placement, axis ranges
and scales, and the curve to draw.

The curve is a plain function pointer (`fn(f64) -> f64`), not a closure
trait object, per the workspace no-trait-objects rule; every fixture curve
is a free function anyway.

```rust
pub struct SyntheticPlotSpec {
    pub width: u32,
    pub height: u32,
    pub frame: super::detect::PixelRect,
    pub x_scale: super::calibration::AxisScale,
    pub x_min: f64,
    pub x_max: f64,
    pub y_scale: super::calibration::AxisScale,
    pub y_min: f64,
    pub y_max: f64,
    pub curve: fn(f64) -> f64,
    pub curve_half_thickness: u32,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `width` | `u32` | Total image width in pixels. |
| `height` | `u32` | Total image height in pixels. |
| `frame` | `super::detect::PixelRect` | Where the axis frame is drawn. Must fit inside the image with at<br>least 1 px margin. |
| `x_scale` | `super::calibration::AxisScale` | x-axis scale and the data values at the frame's left and right edges. |
| `x_min` | `f64` | Data x at `frame.left`. |
| `x_max` | `f64` | Data x at `frame.right`. |
| `y_scale` | `super::calibration::AxisScale` | y-axis scale and the data values at the frame's bottom and top edges. |
| `y_min` | `f64` | Data y at `frame.bottom` (rows grow downward, so the bottom edge is<br>the *smaller* y for a conventional plot). |
| `y_max` | `f64` | Data y at `frame.top`. |
| `curve` | `fn(f64) -> f64` | The curve to draw: `y = curve(x)` in data units. |
| `curve_half_thickness` | `u32` | Half-thickness of the drawn curve in pixels (the drawn band spans<br>`centre ± half`, so thickness is `2*half + 1`). 1 gives a 3-px line,<br>typical of published figures. |

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SyntheticPlotSpec { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
### Functions

#### Function `render_synthetic_plot`

Render the spec to an image, returning the raster **and the exact
calibration** implied by the frame/ranges (which is also the ground-truth
calibration a digitising test should use).

**Method (deterministic).** White background; 1-px black frame on the
spec's rectangle; then for every pixel column strictly inside the frame,
`x = cal.x.value_at(col)` and the curve pixel row is
`cal.y.pixel_at(curve(x))`. A vertical band of `2*half+1` px is inked at
the rounded row, and consecutive columns are connected by filling the row
interval between them, so steep curves have no gaps. Curve values that
fall outside the frame (or are non-finite / non-positive on a log axis)
are simply not drawn for that column.

# Errors

[`DigitiserError::Calibration`] if the axis ranges are invalid for their
scale (via [`AxisCalibration::new`]), or the frame does not fit in the
image.

```rust
pub fn render_synthetic_plot(spec: &SyntheticPlotSpec) -> Result<(super::raster::PlotRaster, super::calibration::PlotCalibration), super::DigitiserError> { /* ... */ }
```

## Module `table_ocr`

Table digitiser — OCR text recognition over a cropped table region,
human-reviewed before export (op-hnhp — GitHub issue #30: "draw box,
right click, digitise with OCR, check values then export csv or
copy/paste").

## Engine decision (op-9bvi)

[`kopitiam_ocr`] — a pure-Rust translation of Tesseract's LSTM
recognizer (see this crate's `NOTICE` for the full provenance/licensing
record; AGPL-3.0-only, same crate-local dependency shape as
`kopitiam-pdf`). This is deliberately the **one place** in this crate's
digitiser that reaches for anything ML-shaped — the plot digitiser's own
"no tick-label OCR" rule is unchanged and still applies to axis values,
which a human still supplies. Table *cell text* is different ground,
opened explicitly by this decision, and gated the same way the plot
digitiser already gates automatic output: [`RecognizedTable`] always
starts [`ReviewStatus::Unreviewed`][crate::digitiser::dataset::ReviewStatus],
and nothing in this module marks it reviewed — only a human front end
calling [`RecognizedTable::record_review`] can.

## What this module does *not* do

- **Table structure / column detection.** [`recognize_table`] finds
  *text lines* ([`kopitiam_ocr::find_text_lines`]) and splits each line
  into cells by a simple heuristic — a run of two or more spaces is a
  column boundary (see `split_into_cells`, private below). This is deterministic and
  ML-free, matching the workspace's offline-first posture, but it is
  **not** real table/border/column detection: a table whose columns
  aren't whitespace-separated in the OCR'd text will not split cleanly,
  and the operator is expected to catch and fix that during the
  mandatory review step, same as the plot digitiser's auto-trace errors
  are expected to be caught and hand-corrected.
- **Model download.** The `.traineddata` model file must already be on
  disk; the operator supplies its path. `kopitiam`'s own OCR pipeline
  downloads models on demand into a cache — that download machinery is
  not ported here (out of scope for this pass; a natural follow-up if a
  model-path text field turns out to be too much friction in practice).

```rust
pub mod table_ocr { /* ... */ }
```

### Types

#### Struct `RecognizedTable`

A recognized table: OCR'd rows of cell text, with the same
provenance-and-review discipline the plot digitiser's
[`super::dataset::DigitisedDataset`] enforces (`DATA_POLICY.md`:
digitisation is a processing step and must be documented as one).

```rust
pub struct RecognizedTable {
    pub schema_version: u32,
    pub source_image_sha256: Option<String>,
    pub source_note: Option<String>,
    pub engine: String,
    pub recognized_by: String,
    pub recognized_at: String,
    pub review: super::dataset::ReviewStatus,
    pub rows: Vec<Vec<String>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `schema_version` | `u32` |  |
| `source_image_sha256` | `Option<String>` | SHA-256 of the source crop's pixel data, if known — the same<br>provenance convention [`super::raster::PlotRaster::source_sha256`]<br>uses for the plot digitiser. |
| `source_note` | `Option<String>` | Free-text note on where the crop came from (e.g. a PDF path and page<br>number) — filled in by the caller, not derived here. |
| `engine` | `String` | Engine + model identification, e.g. `"kopitiam-ocr 0.1.0 (model:<br>/path/to/eng.traineddata)"` — recorded so a reviewer can tell which<br>model produced a given recognition. |
| `recognized_by` | `String` | Who/when ran the automatic pass (distinct from `review`, which<br>records who *checked* the result). |
| `recognized_at` | `String` |  |
| `review` | `super::dataset::ReviewStatus` |  |
| `rows` | `Vec<Vec<String>>` | One row per recognized text line, one cell per whitespace-split<br>segment (see the module doc's "table structure" limitation). |

##### Implementations

###### Methods

- ```rust
  pub fn record_review</* synthetic */ impl Into<String>: Into<String>, /* synthetic */ impl Into<String>: Into<String>>(self: &mut Self, by: impl Into<String>, at: impl Into<String>, interface: ReviewInterface) { /* ... */ }
  ```
  Mark this table reviewed — the plot digitiser's `record_review`

- ```rust
  pub fn to_json_string(self: &Self) -> String { /* ... */ }
  ```

- ```rust
  pub fn write_json(self: &Self, path: &Path) -> Result<(), DigitiserError> { /* ... */ }
  ```

- ```rust
  pub fn to_csv_string(self: &Self) -> String { /* ... */ }
  ```
  Serialise to CSV with the provenance record embedded as `#` comment

- ```rust
  pub fn write_csv(self: &Self, path: &Path) -> Result<(), DigitiserError> { /* ... */ }
  ```

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> RecognizedTable { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &RecognizedTable) -> bool { /* ... */ }
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SerializableAny**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
### Functions

#### Function `recognize_table`

Run the automatic OCR pass over `image` using the `.traineddata` model
at `model_path`: grayscale → Otsu binarize → find text lines → recognize
each line → split into cells (module doc's whitespace-run heuristic).
Always returns rows marked [`ReviewStatus::Unreviewed`] — nothing in
this function, or its callers in this crate, may mark a table reviewed.

# Errors

[`DigitiserError::Ocr`] if the model file can't be read/parsed, or a
line fails to recognize.

```rust
pub fn recognize_table</* synthetic */ impl Into<String>: Into<String>>(model_path: &std::path::Path, image: &kopitiam_ocr::RgbImage, operator: impl Into<String>) -> Result<RecognizedTable, super::DigitiserError> { /* ... */ }
```

### Constants and Statics

#### Constant `TABLE_SCHEMA_VERSION`

Current `RecognizedTable` schema version.

```rust
pub const TABLE_SCHEMA_VERSION: u32 = 1;
```

## Module `trace`

Automatic curve tracing — extracting curve pixel positions by column scan.

Belongs here: [`CurveSelector`] (which pixels count as curve ink),
[`TraceStrategy`] (which vertical run to keep when a column has several),
[`TraceConfig`], [`PixelTracePoint`], and [`trace_curve`]. All strategy
dispatch is by enum `match` — no trait objects, per the workspace Rust
design rules. The trace is deterministic: the same raster and config
always produce the same points.

Does not belong here: converting pixels to data values (that is
[`super::calibration`], applied in [`super::dataset`]) and axis-box
finding ([`super::detect`]).

```rust
pub mod trace { /* ... */ }
```

### Types

#### Enum `CurveSelector`

Which pixels count as "curve ink". Closed set, enum-dispatched.

```rust
pub enum CurveSelector {
    DarkestBand {
        max_luminance: u8,
    },
    Rgb {
        rgb: [u8; 3],
        tolerance: u16,
    },
}
```

##### Variants

###### `DarkestBand`

Any pixel with Rec. 709 luminance strictly below `max_luminance` is
curve ink. The right default for black-on-white published figures.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `max_luminance` | `u8` | Luminance cut, 0–255. 128 tolerates anti-aliasing and scan grey. |

###### `Rgb`

Pixels within `tolerance` of a target colour (Euclidean RGB distance,
0–441). Use for a coloured curve that must be separated from black
gridlines or from other curves.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `rgb` | `[u8; 3]` | Target curve colour as `[r, g, b]`. |
| `tolerance` | `u16` | Maximum Euclidean RGB distance from `rgb` that still counts. |

##### Implementations

###### Methods

- ```rust
  pub fn matches(self: &Self, raster: &PlotRaster, x: u32, y: u32) -> bool { /* ... */ }
  ```
  Does the pixel at `(x, y)` count as curve ink under this selector?

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CurveSelector { /* ... */ }
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

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CurveSelector) -> bool { /* ... */ }
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SerializableAny**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
#### Enum `TraceStrategy`

When a scanned column holds several disjoint vertical runs of curve ink
(curve + gridline, or two curves), which one is the curve? Closed set,
enum-dispatched. Ties always resolve to the topmost run (deterministic).

```rust
pub enum TraceStrategy {
    ColumnCentroid,
    LargestRun,
    ContinuityNearest,
}
```

##### Variants

###### `ColumnCentroid`

Centroid of *all* matching pixels in the column. Cheapest; correct
only when the column contains nothing but the one curve.

###### `LargestRun`

Centroid of the longest contiguous run. Robust against thin
horizontal gridlines crossing the column.

###### `ContinuityNearest`

Centroid of the run nearest (vertically) to the previous column's
accepted point; the first accepted column uses the longest run. Tracks
one curve through crossings with other curves or gridlines. The
default.

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> TraceStrategy { /* ... */ }
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

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TraceStrategy) -> bool { /* ... */ }
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SerializableAny**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
#### Struct `TraceConfig`

Tuning for [`trace_curve`]. [`TraceConfig::default`] suits a clean
black-on-white single-curve figure.

```rust
pub struct TraceConfig {
    pub selector: CurveSelector,
    pub strategy: TraceStrategy,
    pub column_step: u32,
    pub inset: u32,
    pub max_column_fill: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `selector` | `CurveSelector` | What counts as curve ink. Default: luminance < 128. |
| `strategy` | `TraceStrategy` | Run-choice strategy. Default: [`TraceStrategy::ContinuityNearest`]. |
| `column_step` | `u32` | Sample every `column_step`-th pixel column (≥ 1). Default 1. |
| `inset` | `u32` | Pixels to shrink the frame inward on every side before scanning, so<br>the frame lines and their anti-aliasing halo are not traced as curve.<br>Default 3. |
| `max_column_fill` | `f64` | Skip a column when the matched fraction of its scanned height exceeds<br>this (it is a vertical gridline or axis, not curve). Default 0.6. |

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> TraceConfig { /* ... */ }
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

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **NoneValue**
  - ```rust
    fn null_value() -> T { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TraceConfig) -> bool { /* ... */ }
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

- **Read**
- **ReadPrimitive**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SerializableAny**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
#### Struct `PixelTracePoint`

One traced curve sample, still in pixel coordinates.

```rust
pub struct PixelTracePoint {
    pub x_px: f64,
    pub y_px: f64,
    pub thickness_px: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x_px` | `f64` | Column index of the sample (whole pixel, stored as `f64` so hand<br>corrections can be sub-pixel). |
| `y_px` | `f64` | Centroid row of the accepted ink run in this column. |
| `thickness_px` | `f64` | Vertical extent (pixel count) of the accepted run — the local curve<br>line thickness, which [`super::dataset`] turns into the per-point<br>reading uncertainty. |

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> PixelTracePoint { /* ... */ }
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

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PixelTracePoint) -> bool { /* ... */ }
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SerializableAny**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
### Functions

#### Function `trace_curve`

Trace the curve inside `frame`, one sample per scanned column.

**Method (deterministic).** For each sampled column inside the frame
(shrunk by [`TraceConfig::inset`]), the contiguous vertical runs of pixels
matching [`TraceConfig::selector`] are collected. Columns whose matched
fraction exceeds [`TraceConfig::max_column_fill`] are skipped as vertical
gridlines. One run is accepted per remaining column according to
[`TraceConfig::strategy`], and its centroid row becomes the sample.
Columns with no matching pixels yield no sample (gaps are permitted —
dashed curves still trace).

Returns the samples in strictly increasing `x_px` order; possibly empty
(e.g. an empty plot region) — emptiness is the *caller's* signal to warn,
not an error, because a legitimately empty sub-range can occur when
tracing a figure region-by-region.

# Errors

[`DigitiserError::Trace`] if `frame` (after inset) leaves no columns or
rows to scan, or `column_step == 0`.

```rust
pub fn trace_curve(raster: &super::raster::PlotRaster, frame: &super::detect::PixelRect, config: &TraceConfig) -> Result<Vec<PixelTracePoint>, super::DigitiserError> { /* ... */ }
```

### Types

#### Enum `DigitiserError`

Errors produced by the graph digitiser.

Enum-dispatched per the workspace Rust design rules (no trait objects).
Every variant carries a human-readable message describing what failed.

```rust
pub enum DigitiserError {
    Image(String),
    Calibration(String),
    Detection(String),
    Trace(String),
    Io(String),
    Ocr(String),
}
```

##### Variants

###### `Image`

The image file could not be read or decoded (bad path, unsupported
format, corrupt data).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Calibration`

Axis calibration is invalid — coincident reference pixels, coincident
reference values, or non-positive values on a logarithmic axis.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Detection`

The plot frame (axis box) could not be detected automatically.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Trace`

Curve tracing failed (e.g. no curve pixels found inside the frame).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Io`

A dataset file could not be read, written, or parsed.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Ocr`

Table OCR (`table_ocr` — op-hnhp) failed: the `.traineddata` model
could not be loaded, or line recognition itself failed.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> DigitiserError { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DigitiserError) -> bool { /* ... */ }
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
- **ToCompactString**
  - ```rust
    fn try_to_compact_string(self: &Self) -> Result<CompactString, ToCompactStringError> { /* ... */ }
    ```

- **ToLine**
  - ```rust
    fn to_line(self: &Self) -> Line<''_> { /* ... */ }
    ```

- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **ToSmolStr**
  - ```rust
    fn to_smolstr(self: &Self) -> SmolStr { /* ... */ }
    ```

- **ToSpan**
  - ```rust
    fn to_span(self: &Self) -> Span<''_> { /* ... */ }
    ```

- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **ToText**
  - ```rust
    fn to_text(self: &Self) -> Text<''_> { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
## Module `project`

The "kovan folder" project format — `kovan.toml` generation and
maintenance (op-b1y5), implementing the design in
`docs/kovan-folder-format.md` (op-63u0). See that document for the full
rationale; this module implements §3 (schema), §4.1 (section markers)
and §5 (regeneration algorithm) of it.

## What belongs here

- [`ProjectIndex`]/[`DocumentEntry`]/[`SectionRanges`] — the `kovan.toml`
  schema, `serde`-derived for `toml` (de)serialisation.
- [`scan_markdown_sections`] — the marker scanner (design doc §5, steps
  1–3): finds `<!-- kovan:section NAME -->` lines and computes each
  section's inclusive 1-indexed line range.
- [`regenerate`]/[`write_index`]/[`regenerate_and_write`] — rescan a
  project folder and (re)write its `kovan.toml`, atomically.

## What does not belong here (design doc §3/§7 — read before extending)

**`kovan.toml` is generated, never hand-authored, and never read back as
an input to anything but a locate-by-line-number lookup** — this module
must never grow a "merge my hand edits back in" path; every regeneration
fully replaces the file from a fresh scan (design doc §5 step 4),
deliberately, so a stale or hand-edited copy can never silently survive.

**Join key (closed 2026-08-23, op-vi1n):** the design doc says
`document.id` must equal the `.bib` entry's cite key. [`regenerate`] now
parses the project's one `.bib` file with
[`kovan_literature::parse_bib_entries`] and drives the document list from
its cite keys — a document exists only when a `.bib` entry's cite key
*and* a matching `pdf/<key>.pdf` *and* a matching `markdown/<key>.md` all
exist; `id` is that cite key. This is the join the design doc specifies,
not a lookalike: it requires the PDF and markdown files to actually be
*named* after the cite key (the natural outcome of an ingest flow that
names its outputs after the document it processed), which is the only
association this module has any way to make without a further,
not-yet-designed "which file goes with which bib entry" mapping. A `.bib`
entry whose PDF/markdown pair isn't present yet (not fully ingested), or
a `pdf/<stem>.pdf`+`markdown/<stem>.md` pair whose stem matches no cite
key, is silently skipped — both are normal, expected in-progress states,
not errors. (Previously this module joined by shared filename stem alone,
with no reference to the `.bib` file at all — see git history / op-b1y5
if that v1 shape is needed for comparison.)

```rust
pub mod project { /* ... */ }
```

### Types

#### Enum `ProjectError`

Errors from scanning a project folder or reading/writing `kovan.toml`.

```rust
pub enum ProjectError {
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    Toml(String),
    UnknownSection {
        markdown: std::path::PathBuf,
        name: String,
        line: usize,
    },
    DuplicateSection {
        markdown: std::path::PathBuf,
        name: String,
    },
    AmbiguousOrMissingBibFile {
        root: std::path::PathBuf,
        found: Vec<std::path::PathBuf>,
    },
    Bib {
        path: std::path::PathBuf,
        source: kovan_literature::BibParseError,
    },
    StaleSectionRange {
        markdown: std::path::PathBuf,
        name: String,
    },
}
```

##### Variants

###### `Io`

An I/O failure reading/writing a file, with the path it happened on.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `path` | `std::path::PathBuf` |  |
| `source` | `std::io::Error` |  |

###### `Toml`

`toml` (de)serialisation failed.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `UnknownSection`

A `<!-- kovan:section NAME -->` marker named something outside
[`SECTION_ORDER`] — design doc §5 step 1: "an unknown name is a
parse error, not a silently-ignored line."

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `markdown` | `std::path::PathBuf` |  |
| `name` | `String` |  |
| `line` | `usize` |  |

###### `DuplicateSection`

The same section marker appeared twice in one file.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `markdown` | `std::path::PathBuf` |  |
| `name` | `String` |  |

###### `AmbiguousOrMissingBibFile`

The project root has no `.bib` file, or more than one — design doc
§1 says exactly one (the user-named main bibliography file).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `root` | `std::path::PathBuf` |  |
| `found` | `Vec<std::path::PathBuf>` |  |

###### `Bib`

The project's `.bib` file could not be parsed
([`kovan_literature::BibParseError`]).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `path` | `std::path::PathBuf` |  |
| `source` | `kovan_literature::BibParseError` |  |

###### `StaleSectionRange`

[`write_section`]'s caller-supplied range no longer matches a fresh
scan — the file changed on disk since it was read for editing.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `markdown` | `std::path::PathBuf` |  |
| `name` | `String` |  |

##### Implementations

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

- **CastableFrom**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, f: &mut std::fmt::Formatter<''_>) -> std::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Error**
- **ErrorExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

- **Sync**
- **ToCompactString**
  - ```rust
    fn try_to_compact_string(self: &Self) -> Result<CompactString, ToCompactStringError> { /* ... */ }
    ```

- **ToLine**
  - ```rust
    fn to_line(self: &Self) -> Line<''_> { /* ... */ }
    ```

- **ToSmolStr**
  - ```rust
    fn to_smolstr(self: &Self) -> SmolStr { /* ... */ }
    ```

- **ToSpan**
  - ```rust
    fn to_span(self: &Self) -> Span<''_> { /* ... */ }
    ```

- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **ToText**
  - ```rust
    fn to_text(self: &Self) -> Text<''_> { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
#### Struct `SectionRanges`

One document's line-range pointer into its markdown file, per standard
section (design doc §3) — `None` for a section whose marker is absent
(no digitised tables/plots yet, an author summary not written yet, …),
never a fabricated `[0, 0]` (this workspace's data-honesty convention).

```rust
pub struct SectionRanges {
    pub ai_summary: Option<[usize; 2]>,
    pub author_summary: Option<[usize; 2]>,
    pub full_text: Option<[usize; 2]>,
    pub table_csvs: Option<[usize; 2]>,
    pub graph_csvs: Option<[usize; 2]>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ai_summary` | `Option<[usize; 2]>` |  |
| `author_summary` | `Option<[usize; 2]>` |  |
| `full_text` | `Option<[usize; 2]>` |  |
| `table_csvs` | `Option<[usize; 2]>` |  |
| `graph_csvs` | `Option<[usize; 2]>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn get(self: &Self, name: &str) -> Option<[usize; 2]> { /* ... */ }
  ```
  The range for `name` (one of [`SECTION_ORDER`]), if that section's

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SectionRanges { /* ... */ }
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
    fn default() -> SectionRanges { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **NoneValue**
  - ```rust
    fn null_value() -> T { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SectionRanges) -> bool { /* ... */ }
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

- **Read**
- **ReadPrimitive**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SerializableAny**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
#### Struct `DocumentEntry`

One document's entry in `kovan.toml` (design doc §3).

```rust
pub struct DocumentEntry {
    pub id: String,
    pub pdf: String,
    pub markdown: String,
    pub sections: SectionRanges,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `id` | `String` | Join key across the `.bib`/PDF/markdown files — the `.bib` entry's<br>cite key (see this module's doc comment). |
| `pdf` | `String` | Path to the PDF, relative to `kovan.toml`'s own directory. |
| `markdown` | `String` | Path to the markdown file, relative to `kovan.toml`'s own directory. |
| `sections` | `SectionRanges` | Line-range pointers into `markdown`, one per standard section. |

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> DocumentEntry { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DocumentEntry) -> bool { /* ... */ }
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SerializableAny**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
#### Struct `ProjectIndex`

The `kovan.toml` index — see this module's doc comment: generated only,
never hand-authored.

```rust
pub struct ProjectIndex {
    pub schema_version: u32,
    pub bib_file: String,
    pub documents: Vec<DocumentEntry>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `schema_version` | `u32` |  |
| `bib_file` | `String` | The project's one bibliography file, relative to `kovan.toml`'s own<br>directory (design doc §1: "a main bibliography file wherein the<br>user can choose to name it whatever"). |
| `documents` | `Vec<DocumentEntry>` |  |

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ProjectIndex { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ProjectIndex) -> bool { /* ... */ }
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SerializableAny**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
#### Struct `SectionContent`

One section's content, split into its read-only heading (design doc
§4.2: the marker line plus the heading line immediately following it —
structure, never shown as editable text) and its editable body (every
line after the heading through the end of the section's range).

```rust
pub struct SectionContent {
    pub marker_line: String,
    pub heading_line: String,
    pub body: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `marker_line` | `String` | The `<!-- kovan:section NAME -->` marker line, verbatim. |
| `heading_line` | `String` | The line immediately after the marker (normally the `##` heading). |
| `body` | `String` | Everything from the line after `heading_line` through the end of the<br>section's range — the part a GUI editor (op-wr08) may change. |

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SectionContent { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SectionContent) -> bool { /* ... */ }
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

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
### Functions

#### Function `scan_markdown_sections`

Scan `markdown_text` for `<!-- kovan:section NAME -->` marker lines and
compute each present section's inclusive, 1-indexed line range: from the
marker line itself through the line before the next marker (or end of
file for the last section) — design doc §5 steps 1–3.

`markdown_path` is used only to attribute errors to a file; it need not
exist on disk (callers with an in-memory string, e.g. tests, can pass any
path).

```rust
pub fn scan_markdown_sections(markdown_path: &std::path::Path, markdown_text: &str) -> Result<SectionRanges, ProjectError> { /* ... */ }
```

#### Function `regenerate`

Rescan `root` (a "kovan folder" — design doc §1: `kovan.toml`, one
`.bib` file, `pdf/`, `markdown/`) and build a fresh [`ProjectIndex`] —
design doc §5's regeneration algorithm, minus the write (see
[`write_index`]/[`regenerate_and_write`]).

Joins by cite key (design doc §3): for each entry in the project's
`.bib` file whose cite key has a matching `pdf/<key>.pdf` *and*
`markdown/<key>.md`, emits one [`DocumentEntry`] with `id` set to that
cite key. A `.bib` entry missing its PDF or markdown counterpart is
silently skipped — not every reference has been fully ingested yet, and
that is a normal, expected state, not an error. A `pdf`/`markdown`
filename-stem pair with no matching cite key is likewise skipped (see
this module's doc comment for why that's the only association available).

```rust
pub fn regenerate(root: &std::path::Path) -> Result<ProjectIndex, ProjectError> { /* ... */ }
```

#### Function `write_index`

Serialise `index` and write it to `root/kovan.toml` atomically (temp
file + rename — design doc §5 step 5), preceded by
`GENERATED_HEADER` (the "do not edit by hand" comment block).

```rust
pub fn write_index(root: &std::path::Path, index: &ProjectIndex) -> Result<(), ProjectError> { /* ... */ }
```

#### Function `regenerate_and_write`

Convenience: [`regenerate`] then [`write_index`] — what `kovan-cli
project regen` and a future markdown-write-triggered call both run.

```rust
pub fn regenerate_and_write(root: &std::path::Path) -> Result<ProjectIndex, ProjectError> { /* ... */ }
```

#### Function `read_section`

Read one document's section content for editing (op-wr08's GUI editor
surface) — the marker+heading (read-only) and the body (editable), split
out of `markdown_path` at `range` (a `[start, end]` pair as recorded in
`kovan.toml`, 1-indexed inclusive, `start` = the marker's own line).

```rust
pub fn read_section(markdown_path: &std::path::Path, range: [usize; 2]) -> Result<SectionContent, ProjectError> { /* ... */ }
```

#### Function `write_section`

Write a new `body` back into `section_name` of `markdown_rel` (relative
to `root`), then regenerate `kovan.toml` (design doc §5: any markdown
write triggers regeneration).

`expected_range` must match the section's *current* range — re-scanned
fresh from disk before writing — or the write is rejected with
[`ProjectError::StaleSectionRange`] rather than silently overwriting
whatever is actually there now (design doc §4.2's conflict rule: the
file may have changed since the editor opened it, e.g. a fresh
digitisation appended a CSV subsection).

```rust
pub fn write_section(root: &std::path::Path, markdown_rel: &str, section_name: &str, expected_range: [usize; 2], new_body: &str) -> Result<ProjectIndex, ProjectError> { /* ... */ }
```

### Constants and Statics

#### Constant `PROJECT_SCHEMA_VERSION`

Current `kovan.toml` schema version (design doc §3).

```rust
pub const PROJECT_SCHEMA_VERSION: u32 = 1;
```

#### Constant `SECTION_ORDER`

The five standard markdown sections, in their fixed order (design doc
§4.1) — every generated markdown file contains every marker in this
order, even when a section's body is empty.

```rust
pub const SECTION_ORDER: [&str; 5] = _;
```

## Module `tui`

The desktop TUI application: terminal setup/teardown, the top-level
[`App`] state machine, and screen dispatch. This whole module tree is
compiled only when `main.rs` includes it (behind
`cfg(not(target_os = "android"))`), so nothing below needs to repeat that
gate.

# Navigation

Seven tabs ([`Tab`]), switched with `1`-`7` or `Tab`/`Shift+Tab` whenever
no text field is being edited. Each tab that reads the filesystem
(Browser, Symbols, Literature, Ingest) owns a small text field for its
root path, entered with `e` and confirmed with `Enter`/cancelled with
`Esc` — see [`App::editing`]; the Digitiser tab's Setup form uses the same
`e`/`Enter`/`Esc` convention across several fields. `q`/`Esc` quits from
any tab, except while editing (where `Esc` only cancels the edit) and
except when the Ingest or Digitiser tab has work in flight (see
[`App::handle_key`]).

# State ownership

[`App`] owns one state struct per tab by value — no `Arc`/lock anywhere.
The workspace's `Arc<RwLock<T>>` rule (root `CLAUDE.md`, "Shared state")
governs state shared **across threads** in a simulation timestep loop. The
draw loop itself is single-threaded, and the one background worker (PDF
extraction, [`ingest`]) shares no state at all: it owns its input, sends one
result down an `mpsc` channel, and exits. So plain ownership remains the
correct, simpler tool here. See `DECISIONS.md`.

# The loop is polled, not blocking

[`draw_loop`] waits on input with `event::poll` and calls [`App::tick`] each
time round, so a running extraction can animate and deliver its result while
the user does nothing. The poll interval is short only while work is in
flight; otherwise it is long, so an idle TUI stays effectively asleep.

```rust
pub mod tui { /* ... */ }
```

### Types

#### Enum `Tab`

The seven human-facing screens.

```rust
pub enum Tab {
    Overview,
    Browser,
    Symbols,
    Methods,
    Literature,
    Ingest,
    Digitiser,
}
```

##### Variants

###### `Overview`

###### `Browser`

###### `Symbols`

###### `Methods`

###### `Literature`

###### `Ingest`

Interactive literature ingestion — writes Markdown/JSON/BibTeX.

###### `Digitiser`

Interactive graph digitiser — writes the digitised dataset JSON/CSV.
Absorbed the standalone `kovan-digitise-tui` binary on 2026-08-21
(GitHub issue #30's 3-binary consolidation; see [`digitiser`]).

##### Implementations

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Tab { /* ... */ }
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

- **Default**
  - ```rust
    fn default() -> Tab { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **NoneValue**
  - ```rust
    fn null_value() -> T { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Tab) -> bool { /* ... */ }
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

- **Read**
- **ReadPrimitive**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
- **WithSubscriber**
#### Struct `App`

Top-level application state: the active tab, whether a text field is being
edited, and one state struct per tab (see the module docs on why these are
owned by value with no lock).

```rust
pub struct App {
    pub tab: Tab,
    pub editing: bool,
    pub should_quit: bool,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `tab` | `Tab` |  |
| `editing` | `bool` | `true` while the active tab's text-input field (repository/literature<br>root) is capturing keystrokes instead of navigation keys. |
| `should_quit` | `bool` |  |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn handle_key(self: &mut Self, key: KeyEvent) { /* ... */ }
  ```
  Route one key event to the global handlers (quit, tab switch) or, if

- ```rust
  pub fn tick(self: &mut Self) -> bool { /* ... */ }
  ```
  Advance any background work by one draw-loop iteration.

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

- **CastableFrom**
- **Default**
  - ```rust
    fn default() -> App { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **NoneValue**
  - ```rust
    fn null_value() -> T { /* ... */ }
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

- **Read**
- **ReadPrimitive**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

- **Sync**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WithSubscriber**
### Functions

#### Function `run`

Set up the terminal, run the draw/input loop, and restore on exit (or on a
draw/read error — `ratatui::restore()` always runs, and `ratatui::init()`
installs a panic hook that restores first, so neither a panic nor an I/O
error can leave the user's terminal in raw mode).

```rust
pub fn run() -> std::io::Result<()> { /* ... */ }
```

