# Crate Documentation

**Version:** 0.0.0

**Format Version:** 60

# Module `kovan_discovery`

# kovan-discovery

Repository indexing and file discovery for KOVAN. This is the layer beneath
semantics: before any language-native tooling runs, KOVAN needs to find the
files and grep their contents. It builds directly on two mature Rust
engines:

- [`ignore`] — the `.gitignore`-aware directory walker behind `fd`/ripgrep.
- [`grep_searcher`] + [`grep_regex`] — the ripgrep search engine.

Everything here is deterministic and offline: no index database, no
network access, no hidden state. Given the same file-system tree and the
same arguments, every function in this crate returns the same result on
every call, on every platform (Linux, Windows, macOS, Android/Termux) —
see "Determinism" on each function for the specific guarantee.

## What it provides

- [`discover`] / [`discover_kind`] — enumerate files under a root, honouring
  `.gitignore`, optionally filtered to a [`FileKind`] (source, Markdown,
  PDF, metadata). Results are always sorted, so callers get a stable order
  regardless of the host filesystem's raw directory-entry order.
- [`search_file`] — ripgrep-style regex search of a single file, returning
  line number, 1-based character column, and text for every match.
- [`search_repository`] — the two combined: discover every file of a
  [`FileKind`] under a root, then search each one, in one deterministic
  pass. `kovan-semantics`'s `rough_definition_scan` implements this same
  discover-then-search loop today (see that crate); reach for
  [`search_repository`] directly when you don't need a language-specific
  heuristic on top.

`kovan-semantics` starts from these primitives ("ripgrep first") before
escalating to language servers.

## Git-awareness ([`git`])

The [`git`] module is the history/provenance complement to the filesystem
walk above. Where [`discover`] answers "what files are on disk (honouring
`.gitignore`)", [`git`] answers "what does git *track*, and where did each
file/line come from" — repository discovery, current branch/`HEAD`,
tracked-file listing, per-path commit history + last-commit, per-line blame,
and working-tree cleanliness. It is built on the pure-Rust `gix`
(gitoxide) *library* (library-first), with a `gix`-CLI fallback backend
([`git::GitProvider`]); both stay deterministic and offline (local `.git`
only). See that module's docs for the library-first / binary-fallback
design and the Android story.

## What it does *not* do

No index is persisted anywhere — every call re-walks the filesystem and
re-searches the requested files from scratch. That is intentional: KOVAN's
"Deterministic First" design principle (see `docs/kovan.md`) prefers
recomputation over a cache that can silently go stale. If a caller needs
caching, that is its own responsibility, layered on top of this crate.

## Modules

## Module `git`

# Git-awareness (`git`)

Read-only git awareness for KOVAN, built on the pure-Rust
[`gix`](https://docs.rs/gix) (gitoxide) *library*. Where [`crate::discover`]
answers "what files are on disk (honouring `.gitignore`)", this module
answers the complementary questions a knowledge layer needs about a
repository's *history and provenance*:

- **Where is the repo?** — [`GixBackend::discover`] / [`GixBackend::open`]
  (walks up to the enclosing `.git`, or opens one exactly).
- **What is checked out?** — current branch / `HEAD` commit
  ([`GitProvider::head`]).
- **What does git actually track?** — [`GitProvider::tracked_files`], read
  from the git index. This is the git-truth complement to the filesystem
  walk in [`crate::discover`]: `discover` lists what is *present and not
  ignored*; `tracked_files` lists what git is *actually versioning*.
- **Provenance of a file / symbol** — commit history for a path
  ([`GitProvider::history_for_path`]), the last commit that touched it
  ([`GitProvider::last_commit_for_path`]), and per-line blame
  ([`GitProvider::blame_line`]) so a [`kovan_common::KovanSymbol`] or
  [`kovan_common::KovanDocument`] can be tied to the commit that introduced
  its definition line.
- **Is the working tree clean?** — [`GitProvider::is_worktree_dirty`].

Everything here is **deterministic and offline**: `gix` reads the local
`.git` directory only — no network, no remote fetch. Given the same
repository state, every function returns the same result on every call.

## Library-first, binary-fallback

Per KOVAN's "use the library first, fall back on the binary" principle, the
backend is an **enum** ([`GitProvider`]) — not a trait object — with two
variants:

- [`GitProvider::Library`] wrapping [`GixBackend`] — the **default and
  primary** path. Pure Rust, so it builds and runs on Android, and it
  covers *every* operation this module exposes natively.
- [`GitProvider::Binary`] wrapping [`GixCliBackend`] — the **fallback**,
  which shells out to the `gix` command-line tool (the one
  `kovan setup` installs). It is selected only when the library path cannot
  open the repository (see [`GitProvider::open_preferring_library`]).

[`GitBackend`] is a compile-time *contract* implemented by both backends so
the compiler checks they stay in step; it is **never** used as `dyn Trait`.
Dispatch is the exhaustive `match` in [`GitProvider`]'s methods.

### Which operations fall back to the binary?

In this implementation, **none at the per-operation level**: the `gix`
*library* cleanly covers all of `head` / `tracked_files` / `history` /
`last_commit_for_path` / `blame_line` / `is_worktree_dirty`, so the primary
path never needs to defer a specific call. The binary backend is the
*structural* fallback for the case where the library cannot even open the
repository (an unusual on-disk layout, a `gix`-version the library build
doesn't understand). The `gix` CLI's higher-level porcelain is still
experimental and its output is not a stable contract, so the binary
backend's structured queries deliberately return [`GitError::Unsupported`]
rather than parse unstable text — it guarantees only availability probing
([`GixCliBackend::is_available`]). The library is authoritative.

### Android

The library path is pure Rust and Android-clean. The binary backend cannot
run (no `gix` binary on Android), so every [`GixCliBackend`] operation
returns [`GitError::BinaryUnavailableOnTarget`] when compiled for
`target_os = "android"`. Because the default constructors return the
library backend, `cargo check --target aarch64-linux-android` stays clean
and the Android code path never depends on the binary.

```rust
pub mod git { /* ... */ }
```

### Types

#### Struct `CommitInfo`

One commit, reduced to the provenance facts a knowledge layer records.

All fields are owned `String`/`i64` (no borrows into the repository), so a
`CommitInfo` outlives the [`gix::Repository`] it was read from and can be
stored directly on a [`kovan_common::KovanSymbol`] provenance record.

```rust
pub struct CommitInfo {
    pub id: String,
    pub short_id: String,
    pub summary: String,
    pub author_name: String,
    pub author_email: String,
    pub committed_unix_seconds: i64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `id` | `String` | Full 40-character hex commit id (the SHA-1 object id). |
| `short_id` | `String` | The first 12 characters of [`id`](CommitInfo::id) — the customary<br>abbreviated commit hash, for human-facing display. |
| `summary` | `String` | Commit summary: the first line of the commit message, trimmed. Empty<br>only if the message itself is empty. |
| `author_name` | `String` | Author name (the person who wrote the change), as recorded in the<br>commit. May be empty if the commit omits it. |
| `author_email` | `String` | Author email, as recorded in the commit. May be empty. |
| `committed_unix_seconds` | `i64` | Committer timestamp, in whole seconds since the Unix epoch (UTC). This<br>is the commit's own date — the point in history the commit represents —<br>not the author date. Used for newest-first ordering of history. |

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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> CommitInfo { /* ... */ }
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
    fn eq(self: &Self, other: &CommitInfo) -> bool { /* ... */ }
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
#### Struct `HeadInfo`

The state of `HEAD`: which branch is checked out and the commit it resolves
to.

```rust
pub struct HeadInfo {
    pub branch: Option<String>,
    pub detached: bool,
    pub commit: CommitInfo,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `branch` | `Option<String>` | Short branch name `HEAD` points at (e.g. `"main"`), or `None` when<br>`HEAD` is *detached* (checked out directly at a commit, not via a<br>branch). |
| `detached` | `bool` | `true` when `HEAD` is detached (no branch). Equivalent to<br>`branch.is_none()`, exposed as its own flag for readability at call<br>sites. |
| `commit` | `CommitInfo` | The commit `HEAD` currently resolves to. |

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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> HeadInfo { /* ... */ }
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
    fn eq(self: &Self, other: &HeadInfo) -> bool { /* ... */ }
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
#### Enum `BackendKind`

Which backend a [`GitProvider`] is dispatching to.

```rust
pub enum BackendKind {
    Library,
    Binary,
}
```

##### Variants

###### `Library`

The pure-Rust `gix` **library** backend (primary; Android-clean).

###### `Binary`

The `gix` **command-line** fallback backend.

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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> BackendKind { /* ... */ }
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

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &BackendKind) -> bool { /* ... */ }
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
#### Enum `GitError`

Errors from the git-awareness layer.

```rust
pub enum GitError {
    NotARepository(std::path::PathBuf),
    Open(String),
    UnbornHead,
    Backend(String),
    Unsupported(&'static str),
    BinaryUnavailableOnTarget,
    BinaryFailed(String),
}
```

##### Variants

###### `NotARepository`

No git repository was found at (or above) the given path.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `std::path::PathBuf` |  |

###### `Open`

Opening/discovering the repository failed (I/O, corrupt `.git`, …). The
string is the underlying `gix` error rendered for a human.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `UnbornHead`

`HEAD` has no commit yet — a freshly `git init`ed repository with
nothing committed. History/blame/last-commit queries need at least one
commit.

###### `Backend`

A `gix`-library operation failed after the repository opened
(object lookup, index read, diff, …). The string is the underlying
error rendered for a human.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Unsupported`

The requested operation is not supported by the **binary** backend.
The `gix` CLI's porcelain for this query is not a stable contract, so
the binary path declines rather than parse unstable output — use the
library backend, which covers it natively. The `&str` names the op.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&'static str` |  |

###### `BinaryUnavailableOnTarget`

The **binary** backend is unavailable on this build target — there is no
`gix` executable on Android. Returned by every [`GixCliBackend`]
operation under `target_os = "android"`.

###### `BinaryFailed`

The **binary** backend could not run the `gix` executable (not on
`PATH`, or it exited non-zero). The string carries the detail.

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

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, f: &mut std::fmt::Formatter<''_>) -> std::fmt::Result { /* ... */ }
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
#### Struct `GixBackend`

The **primary** git backend, built on the pure-Rust `gix` library.

Owns an opened [`gix::Repository`] by value (no lifetimes, per the workspace
design rules). Construct with [`GixBackend::discover`] (walk up to the
enclosing repo) or [`GixBackend::open`] (open a repo at an exact path).

```rust
pub struct GixBackend {
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
  pub fn discover(start: &Path) -> Result<Self, GitError> { /* ... */ }
  ```
  Discover the git repository at or above `start`, opening it. This is the

- ```rust
  pub fn open(path: &Path) -> Result<Self, GitError> { /* ... */ }
  ```
  Open the git repository located *exactly* at `path` (does not walk up).

- ```rust
  pub fn repository(self: &Self) -> &gix::Repository { /* ... */ }
  ```
  Borrow the underlying opened [`gix::Repository`] for callers that need a

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

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **GitBackend**
  - ```rust
    fn repo_root(self: &Self) -> &Path { /* ... */ }
    ```

  - ```rust
    fn head(self: &Self) -> Result<HeadInfo, GitError> { /* ... */ }
    ```

  - ```rust
    fn tracked_files(self: &Self) -> Result<Vec<PathBuf>, GitError> { /* ... */ }
    ```

  - ```rust
    fn history(self: &Self, max: usize) -> Result<Vec<CommitInfo>, GitError> { /* ... */ }
    ```

  - ```rust
    fn history_for_path(self: &Self, rela_path: &Path, max: usize) -> Result<Vec<CommitInfo>, GitError> { /* ... */ }
    ```

  - ```rust
    fn last_commit_for_path(self: &Self, rela_path: &Path) -> Result<Option<CommitInfo>, GitError> { /* ... */ }
    ```

  - ```rust
    fn blame_line(self: &Self, rela_path: &Path, line: u32) -> Result<Option<CommitInfo>, GitError> { /* ... */ }
    ```

  - ```rust
    fn is_worktree_dirty(self: &Self) -> Result<bool, GitError> { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

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
#### Struct `GixCliBackend`

The **fallback** git backend, which shells out to the `gix` command-line
tool (installed by `kovan setup`).

This exists to satisfy KOVAN's "library first, fall back on the binary"
principle when the [`GixBackend`] library path cannot open a repository. The
`gix` CLI's structured porcelain is still experimental and its text output
is not a stable contract, so the structured queries here deliberately return
[`GitError::Unsupported`] rather than parse unstable output — the library
backend is authoritative for those. What the binary backend *does*
guarantee is availability detection ([`GixCliBackend::is_available`]).

On `target_os = "android"` there is no `gix` executable, so every operation
returns [`GitError::BinaryUnavailableOnTarget`].

```rust
pub struct GixCliBackend {
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
  pub fn new(root: &Path) -> Self { /* ... */ }
  ```
  A binary backend rooted at `root`, invoking the default `gix` program

- ```rust
  pub fn with_program</* synthetic */ impl Into<String>: Into<String>>(root: &Path, program: impl Into<String>) -> Self { /* ... */ }
  ```
  A binary backend that invokes `program` instead of the default `gix`

- ```rust
  pub fn is_available(self: &Self) -> bool { /* ... */ }
  ```
  Whether the `gix` executable is runnable — probes `gix --version`.

- ```rust
  pub fn version(self: &Self) -> Result<String, GitError> { /* ... */ }
  ```
  The `gix --version` string, or an error if the binary can't be run.

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

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **GitBackend**
  - ```rust
    fn repo_root(self: &Self) -> &Path { /* ... */ }
    ```

  - ```rust
    fn head(self: &Self) -> Result<HeadInfo, GitError> { /* ... */ }
    ```

  - ```rust
    fn tracked_files(self: &Self) -> Result<Vec<PathBuf>, GitError> { /* ... */ }
    ```

  - ```rust
    fn history(self: &Self, _max: usize) -> Result<Vec<CommitInfo>, GitError> { /* ... */ }
    ```

  - ```rust
    fn history_for_path(self: &Self, _rela_path: &Path, _max: usize) -> Result<Vec<CommitInfo>, GitError> { /* ... */ }
    ```

  - ```rust
    fn last_commit_for_path(self: &Self, _rela_path: &Path) -> Result<Option<CommitInfo>, GitError> { /* ... */ }
    ```

  - ```rust
    fn blame_line(self: &Self, _rela_path: &Path, _line: u32) -> Result<Option<CommitInfo>, GitError> { /* ... */ }
    ```

  - ```rust
    fn is_worktree_dirty(self: &Self) -> Result<bool, GitError> { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

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
#### Enum `GitProvider`

**Attributes:**

- `Other("#[allow(clippy::large_enum_variant)]")`

Git-awareness entry point: an **enum** over the two backends, dispatched by
exhaustive `match` (never `dyn Trait`).

The [`GitProvider::Library`] (pure-Rust `gix`) variant is the default and
primary path; [`GitProvider::Binary`] (the `gix` CLI) is the fallback. Use
[`GitProvider::open`] for the usual "just use the library" case, or
[`GitProvider::open_preferring_library`] to opt into the fallback when the
library cannot open the repo.

```rust
pub enum GitProvider {
    Library(GixBackend),
    Binary(GixCliBackend),
}
```

##### Variants

###### `Library`

The pure-Rust `gix` library backend (primary, Android-clean).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `GixBackend` |  |

###### `Binary`

The `gix` CLI fallback backend.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `GixCliBackend` |  |

##### Implementations

###### Methods

- ```rust
  pub fn open(start: &Path) -> Result<Self, GitError> { /* ... */ }
  ```
  Discover and open the repository at or above `start` using the

- ```rust
  pub fn open_preferring_library(start: &Path) -> Result<Self, GitError> { /* ... */ }
  ```
  Try the **library** backend first; if it cannot open the repository,

- ```rust
  pub fn binary(root: &Path) -> Self { /* ... */ }
  ```
  Construct a provider that explicitly uses the **binary** backend rooted

- ```rust
  pub fn backend_kind(self: &Self) -> BackendKind { /* ... */ }
  ```
  Which backend this provider dispatches to.

- ```rust
  pub fn repo_root(self: &Self) -> &Path { /* ... */ }
  ```
  The repository root — see [`GitBackend::repo_root`].

- ```rust
  pub fn head(self: &Self) -> Result<HeadInfo, GitError> { /* ... */ }
  ```
  Current branch / `HEAD` commit — see [`GitBackend::head`].

- ```rust
  pub fn tracked_files(self: &Self) -> Result<Vec<PathBuf>, GitError> { /* ... */ }
  ```
  Every path git tracks — see [`GitBackend::tracked_files`].

- ```rust
  pub fn history(self: &Self, max: usize) -> Result<Vec<CommitInfo>, GitError> { /* ... */ }
  ```
  Up to `max` commits, newest first — see [`GitBackend::history`].

- ```rust
  pub fn history_for_path(self: &Self, rela_path: &Path, max: usize) -> Result<Vec<CommitInfo>, GitError> { /* ... */ }
  ```
  Up to `max` commits that changed `rela_path`, newest first — see

- ```rust
  pub fn last_commit_for_path(self: &Self, rela_path: &Path) -> Result<Option<CommitInfo>, GitError> { /* ... */ }
  ```
  The most recent commit that changed `rela_path` — see

- ```rust
  pub fn blame_line(self: &Self, rela_path: &Path, line: u32) -> Result<Option<CommitInfo>, GitError> { /* ... */ }
  ```
  The commit introducing 1-based `line` of `rela_path` — see

- ```rust
  pub fn is_worktree_dirty(self: &Self) -> Result<bool, GitError> { /* ... */ }
  ```
  Whether the working tree is dirty — see [`GitBackend::is_worktree_dirty`].

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
### Traits

#### Trait `GitBackend`

Compile-time contract every git backend satisfies.

This trait exists so the compiler verifies [`GixBackend`] and
[`GixCliBackend`] expose the same read-only surface. It is **never** used as
a trait object (`dyn GitBackend`): dispatch is the exhaustive `match` inside
[`GitProvider`]. Downstream code should call the methods on [`GitProvider`],
not import this trait — it is the internal contract, not the public entry
point.

Paths passed to the per-path methods are **repository-relative** (e.g.
`src/lib.rs`), matching [`kovan_common::KovanSymbol::file`].

```rust
pub trait GitBackend {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `repo_root`: The absolute path to the repository's working-tree root (or the `.git`
- `head`: The current branch and the commit `HEAD` resolves to.
- `tracked_files`: Every path git currently tracks (reads the index), sorted, as
- `history`: Up to `max` commits of the whole repository, newest first.
- `history_for_path`: Up to `max` commits that changed `rela_path`, newest first.
- `last_commit_for_path`: The most recent commit that changed `rela_path`, or `None` if the path
- `blame_line`: The commit that introduced the current content of 1-based `line` in
- `is_worktree_dirty`: Whether the working tree differs from the index/`HEAD` for *tracked*

##### Implementations

This trait is implemented for the following types:

- `GixBackend`
- `GixCliBackend`

## Types

### Enum `FileKind`

A category of file KOVAN cares about, with its associated extensions.

This is a closed, compile-time-known set (per the workspace's "enums over
trait objects" rule) — adding a new kind is a one-line match-exhaustiveness
error at every call site that needs updating, not a runtime surprise.

```rust
pub enum FileKind {
    Source,
    Markdown,
    Pdf,
    Metadata,
}
```

#### Variants

##### `Source`

Source code (Rust, C++, Python, Fortran, …).

##### `Markdown`

Markdown documents.

##### `Pdf`

PDF literature.

##### `Metadata`

Metadata / configuration (TOML, JSON, YAML).

#### Implementations

##### Methods

- ```rust
  pub fn extensions(self: Self) -> &'static [&'static str] { /* ... */ }
  ```
  The lowercase file extensions associated with this kind.

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
    fn clone(self: &Self) -> FileKind { /* ... */ }
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

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FileKind) -> bool { /* ... */ }
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
### Struct `SearchMatch`

A single search hit within a file.

Line and column are both 1-based, matching the convention used by editors,
compilers, and `ripgrep`'s own CLI output (`path:line:column: text`).

```rust
pub struct SearchMatch {
    pub line: u64,
    pub column: usize,
    pub text: String,
}
```

#### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `line` | `u64` | 1-based line number. |
| `column` | `usize` | 1-based character (not byte) column of the start of the match on that<br>line. Computed by counting `char`s before the match, so it stays<br>correct for multi-byte UTF-8 text (e.g. a match after a non-ASCII<br>identifier or comment). Defaults to `1` in the unexpected case where<br>the regex that already matched this line cannot be re-located within<br>it (see [`search_file`] for when that can happen). |
| `text` | `String` | The matching line, trailing newline trimmed. |

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
    fn clone(self: &Self) -> SearchMatch { /* ... */ }
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
    fn eq(self: &Self, other: &SearchMatch) -> bool { /* ... */ }
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
### Enum `DiscoveryError`

Errors produced by discovery / search.

```rust
pub enum DiscoveryError {
    BadPattern(String),
    Io(std::io::Error),
}
```

#### Variants

##### `BadPattern`

The regex pattern was invalid.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

##### `Io`

An I/O error occurred while searching (includes the file not existing,
a permissions failure, or the file not being valid UTF-8 — this crate
searches decoded text, so a binary or non-UTF-8 file surfaces as an
I/O-flavoured decoding error rather than a match list).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `std::io::Error` |  |

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
    fn fmt(self: &Self, f: &mut std::fmt::Formatter<''_>) -> std::fmt::Result { /* ... */ }
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

### Function `discover`

Discover all files under `root`, honouring `.gitignore` (and `.ignore`,
and global git excludes — anything the `ignore` crate's default walker
respects, the same rules `fd` and `ripgrep` use). If `exts` is non-empty,
only files whose (lowercased) extension is listed are returned; an empty
slice returns every non-ignored file.

Hidden files/directories (dotfiles, `.git/`) are skipped by default — this
is the `ignore` crate's standard behaviour, matching `fd`/`ripgrep`'s
defaults. Symlinks are not followed.

# `.gitignore` handling

`.gitignore` rules are honoured **even when `root` is not inside a git
repository** — a bare `.gitignore` in a plain directory (a downloaded
tarball, a vendored source tree, a literature staging directory) is still
respected, because that is what `.gitignore` is *supposed* to do. This is a
deliberate choice: the `ignore` crate's default (`require_git = true`) would
instead treat `.gitignore` as inert outside a real `.git` repo, silently
breaking this function's "honours `.gitignore`" contract for non-repo trees.
We call `.require_git(false)` so the contract holds everywhere. `.ignore`
files and global git excludes are honoured either way, and for a directory
that *is* inside a git repo the behaviour is unchanged.

# Determinism

The returned `Vec` is always sorted by path
([`Ord`] on [`PathBuf`], i.e. lexicographic by path component), regardless
of the order the underlying filesystem's directory entries were returned
in. Two calls against the same tree — on the same machine or a different
one — always produce the same order.

# Error handling

This function does not return a `Result`: a root that does not exist, or
a subdirectory that cannot be read (permissions), is silently skipped
rather than surfaced as an error, matching `fd`'s default behaviour of
best-effort traversal. A non-existent or fully inaccessible `root` simply
yields an empty `Vec`. Callers that need to distinguish "no matching
files" from "root does not exist" should check `root.exists()` themselves
before calling.

```rust
pub fn discover(root: &std::path::Path, exts: &[&str]) -> Vec<std::path::PathBuf> { /* ... */ }
```

### Function `discover_kind`

Discover all files under `root` of a given [`FileKind`]. Equivalent to
calling [`discover`] with that kind's [`FileKind::extensions`].

See [`discover`] for the `.gitignore` and determinism (sorted-output)
guarantees, which apply identically here.

```rust
pub fn discover_kind(root: &std::path::Path, kind: FileKind) -> Vec<std::path::PathBuf> { /* ... */ }
```

### Function `search_file`

Search a single file for `pattern` (a regular expression), returning every
matching line with its line number and column. Uses the ripgrep engine
(`grep-searcher` + `grep-regex`), so the pattern accepts the same syntax
`rg` does (Rust's `regex` crate syntax).

# Determinism

Matches are returned in ascending line order (the order the file is read
top to bottom); re-running against an unchanged file always yields an
identical result.

# Errors

- [`DiscoveryError::BadPattern`] if `pattern` fails to compile as a regex.
- [`DiscoveryError::Io`] if `path` cannot be opened/read, or is not valid
  UTF-8 (this function searches decoded text; binary files are rejected
  rather than silently mis-decoded).

# Column computation

[`SearchMatch::column`] is derived by re-locating the pattern within the
matched line via [`grep_matcher::Matcher::find`]. This is a second, local
search over an already-matched line (not a full extra file pass), so it is
cheap. In the practically-unreachable case where that re-location fails —
the searcher matched a line but the matcher then reports no match on that
same line — the column falls back to `1` rather than panicking.

```rust
pub fn search_file(path: &std::path::Path, pattern: &str) -> Result<Vec<SearchMatch>, DiscoveryError> { /* ... */ }
```

### Function `search_repository`

Discover every file of `kind` under `root`, then [`search_file`] each one
for `pattern`. The combined "find files, then grep them" primitive —
`kovan-semantics`'s `rough_definition_scan` implements this same
discover-then-search loop by hand today; reach for this directly if you
just need "grep this whole repository for X" without a language-specific
heuristic on top.

# Determinism

Files are visited in the sorted order [`discover_kind`] returns, and each
file's matches are appended in ascending line order, so the overall
`Vec<(PathBuf, SearchMatch)>` is fully deterministic for a given tree.

# Errors

Stops at (and returns) the first [`DiscoveryError`] raised by
[`search_file`] on any discovered file — a [`DiscoveryError::BadPattern`]
on the first file means every subsequent file has the same bad pattern, so
failing fast avoids repeating the same compile error. A per-file I/O error
(e.g. one non-UTF-8 file among many source files) aborts the whole scan
rather than silently skipping that file; callers that want best-effort
behaviour should call [`discover_kind`] and [`search_file`] themselves and
decide how to handle a single file's error.

```rust
pub fn search_repository(root: &std::path::Path, kind: FileKind, pattern: &str) -> Result<Vec<(std::path::PathBuf, SearchMatch)>, DiscoveryError> { /* ... */ }
```

