# Crate Documentation

**Version:** 0.1.0

**Format Version:** 60

# Module `outram_foam_cli`

# outram-foam-cli

OpenFOAM-style **command-line utilities** as terminal binaries. Each tool is
its own binary named exactly like upstream OpenFOAM (`blockMesh`,
`pimpleFoam`, `gen-foam`, …), so a user drops into a case directory and runs
the tool by name, mirroring the OpenFOAM workflow.

> **Independent OUTRAM PARK fork, not the official OpenFOAM.** Not affiliated
> with or endorsed by OpenCFD Ltd. / the OpenFOAM Foundation / ESI Group; the
> tool names identify the upstream utilities re-implemented here. See
> `TRADEMARKS.md`. **Unverified until validated** — not for safety-critical use.

## Shared CLI conventions ([`CaseArgs`])

Every tool accepts the common OpenFOAM options: `-case <dir>` (default `.`)
selects the case directory; standard `--help`/`--version` via `clap`. The
tool then reads the case (`system/`, `constant/polyMesh`, time dirs) through
[`outram_foam_basic_lib::io`], runs, and writes its output back into the case.

## Wiring status

This crate is the thin CLI layer; the actual work lives in the library
crates ([`outram_foam_mesh`] for meshing, `outram-foam-appbuilder-lib` for
the solvers). See each binary's `--help` and the `op-` beads for what is
live vs stubbed.

## Types

### Struct `CaseArgs`

Common OpenFOAM-style command-line options shared by every tool binary.

Mirrors the upstream convention: a tool is run from (or pointed at) a **case
directory** — the folder containing `system/`, `constant/`, and time
directories.

```rust
pub struct CaseArgs {
    pub case: std::path::PathBuf,
}
```

#### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `case` | `std::path::PathBuf` | Case directory to operate on (the OpenFOAM `-case` option). Defaults to<br>the current working directory. |

#### Implementations

##### Methods

- ```rust
  pub fn case_dir(self: &Self) -> Result<PathBuf, CliError> { /* ... */ }
  ```
  The resolved case directory. Errors if it does not exist / is not a dir.

##### Trait Implementations

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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> CaseArgs { /* ... */ }
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

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Parser**
- **RefUnwindSafe**
- **Same**
- **Send**
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
### Enum `CliError`

Errors surfaced by the CLI tools (wrapping the library layers).

```rust
pub enum CliError {
    CaseNotFound(std::path::PathBuf),
    Io(String),
    Tool(String),
    NotWired(&'static str),
}
```

#### Variants

##### `CaseNotFound`

The `-case` directory does not exist or is not a directory.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `std::path::PathBuf` |  |

##### `Io`

A case-I/O error (dict/polyMesh/field read or write).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

##### `Tool`

The tool ran but the underlying solver / mesher reported an error.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

##### `NotWired`

The tool is scaffolded but its case-wiring is not yet implemented.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&'static str` |  |

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

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **Error**
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

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
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

### Function `openfoam_argv`

Rewrite an argv so **OpenFOAM single-dash long options** (`-case`, `-help`)
are accepted as if they were `clap`'s double-dash form (`--case`, `--help`).

OpenFOAM utilities use single-dash multi-letter options; `clap` uses
double-dash. This promotes any `-<word>` (single dash + two-or-more
alphanumeric/`-` chars) to `--<word>`, while leaving genuine short flags
(`-c`), bare values (`/tmp`), and negative numbers (`-5.0`) untouched — so
`blockMesh -case cavity` works exactly like upstream. Exposed for testing.

```rust
pub fn openfoam_argv<I, S>(args: I) -> Vec<std::ffi::OsString>
where
    I: IntoIterator<Item = S>,
    S: Into<std::ffi::OsString> { /* ... */ }
```

### Function `openfoam_args`

Parse [`CaseArgs`] from the process arguments using the OpenFOAM single-dash
option convention (see [`openfoam_argv`]). The entry point every tool binary
uses in place of `clap`'s `parse()`.

```rust
pub fn openfoam_args() -> CaseArgs { /* ... */ }
```

