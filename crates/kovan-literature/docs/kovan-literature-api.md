# Crate Documentation

**Version:** 0.0.1

**Format Version:** 60

# Module `kovan_literature`

# kovan-literature

The nuclear-engineering knowledge archive. It turns source PDFs into the
canonical [`KovanDocument`] and generates derived artifacts (Markdown,
BibTeX, extracted assets).

## Canonical workflow

```text
PDF → Markdown → KovanDocument → BibTeX → generated knowledge artifacts
```

Implements the pipeline described in `docs/kovan.md` sections
"Literature Workflow", "Canonical Representation" and "PDF Processing".
The [`KovanDocument`] struct is authoritative; BibTeX and generated Markdown
are always derived from it, never the other way round.

## Determinism & offline guarantees

Every function here is **deterministic** (same input bytes → same output
bytes) and runs **fully offline** — no network, no cloud, no OCR service.
PDF text extraction uses the pure-Rust [`pdf_extract`] crate; the low-level
object model (metadata, assets) uses pure-Rust [`lopdf`]. Both build for
Android (`aarch64-linux-android`), matching KOVAN's Android-first mandate
(`docs/kovan.md`, "Android First").

## Storage layout

Content lives on disk next to this crate (`docs/kovan.md`, "Storage Layout"):

- `open/{papers,reports,standards,benchmarks,theses}/` — redistributable
  content, may be committed.
- `proprietary/{…}/` — user-owned content; **gitignored**, never committed.
- `generated/{markdown,bibtex,assets}/{open,proprietary}/` — reproducible
  outputs, split by [`Visibility`] so the proprietary half can be kept out of
  both git and the published crate. See [`storage::generated_dir_for`].

Three distribution tiers follow from that split: generated **open BibTeX** is
committed *and* published to crates.io; ~~open PDFs and~~ generated open Markdown
~~are~~ is committed but **not** published (licence scope and size); everything
proprietary is neither.
**CORRECTED 2026-09-25** — no PDF is tracked under this crate any more
(`git ls-files crates/kovan-literature` lists none outside the submodule).
Since 2026-09-22 open PDFs live in the `reactor-literature/` Git submodule
and `open/` holds only metadata JSON and some Markdown; see `CLAUDE.md` and
`CATALOGUE.md`.

## What is real vs. best-effort

- [`pdf_to_markdown`], [`markdown_outline`], [`to_bibtex`] — fully
  implemented and tested.
- [`extract_metadata`] — best-effort heuristics (PDF Info dictionary first,
  then conservative text scanning). Unknown fields are left `None`/empty
  rather than guessed.
- [`extract_assets`] — extracts embedded raster images whose codec is already
  a standalone file format (JPEG via `DCTDecode`, JPEG-2000 via `JPXDecode`).
  Images stored under other filters are reported-skipped, not re-encoded.
- [`zotero`] — a port of Zotero's translation framework and its BibTeX,
  BibLaTeX, RIS and CSL JSON translators (GitHub #749), verified
  code-to-code against a running Zotero translation-server (#752,
  `tests/zotero_translators.rs`). AI draft, not yet human-reviewed.
  Independent of [`to_bibtex`], kovan's own BibTeX writer.

**The graph digitiser moved to the `kovan` crate on 2026-08-21** (was
`[crate::digitiser]`, now `kovan::digitiser`; binaries ~~`kovan-digitise`,
`kovan-digitise-tui`, `kovan-gui`~~ **CORRECTED 2026-09-25** — `kovan`
(GUI), `kovan-cli digitise` and `kovan-tui`, the only three `[[bin]]`
targets in `crates/kovan/Cargo.toml`) — see that crate's `NOTICE`. It moved
so it can depend on `kopitiam-pdf` (AGPL-3.0-only, GitHub issue #30's
PDF-native digitising) without pulling this crate — used well beyond the
GUI — into that relicense. This crate ~~stays GPL-3.0-only and~~ carries no
digitiser code, no `image`/`eframe`/`egui`/`ratatui` dependency, and no
`digitise-*` feature. **CORRECTED 2026-10-07** — this crate is
AGPL-3.0-only since b142a065d2 (the whole kovan family was relicensed for
the Zotero port; `Cargo.toml` and `NOTICE`), so "stays GPL-3.0-only" no
longer holds.

## Zotero

[`zotero::local_library`] reads a Zotero data folder (`zotero.sqlite` +
`storage/`) into the `kovan_common::zotero` model and imports it as
[`KovanDocument`]s (GitHub #750). ~~Native desktop targets only: it is
compiled out on wasm32 and Android (SQLite is C; see `Cargo.toml`).~~
**CORRECTED 2026-10-07**: the reader is pure Rust (turso_core reads the
SQLite file format and WAL from memory) and compiles on every target,
wasm32 and Android included; `read_database_files` reads a database from
bytes, with no file system.

## Modules

## Module `concept_tree`

The concept tree, levels 1–3, as typed data (GitHub #724, #727).

What belongs here: reading the two hand-maintained TOML files beside this
module into one [`ConceptTree`] that anything else (Kovan's mind map, the
Code Review tab, the hosted page) can walk without parsing TOML itself:

- `concept_skeleton.toml`: the `[[document]]` list, and the `[[node]]`s of
  level 1 (the 19 IAEA Milestones issues, `NN-name`, `NN` = NG-G-3.1
  Rev. 1 §3.NN) and level 2 (regulatory review categories, unnumbered);
- `concept_proposals.toml`: the level-3 `[[concept]]`s. Only `approved`
  and `deferred` concepts are part of the standard tree; a `deferred` one
  is kept (and flagged, so a map can grey it) because the maintainer
  approved its place and is waiting only on its sources. `proposed`
  concepts are not in the tree. The `[[implementation]]` (level 4) seeds in
  the same file are **not** read here: level 4 is generated from code tags
  (#729) and is not part of the standard tree.

Both files are compiled in with `include_str!` and parsed once, on first
use ([`concept_tree`]), so the tree needs no file system, no corpus
checkout and no network; this module is wasm-clean
(`cargo check --release -p kovan-literature --target wasm32-unknown-unknown`).

What does not belong here: drawing, the literature filed under a node
(Kovan's `corpus.rs` decides that), and validation beyond what parsing
needs. The structural rules (19 L1 issues in order, parents exist,
sources name declared documents, cross-links resolve) are enforced by
`tests/concept_skeleton.rs` and `tests/concept_proposals.rs`, which read
the data through this module.

```rust
pub mod concept_tree { /* ... */ }
```

### Types

#### Enum `DocumentTier`

Where a document's file is held.

```rust
pub enum DocumentTier {
    Standard,
    Private,
}
```

##### Variants

###### `Standard`

In the public standard corpus (`reactor-literature/` repository,
folder `kovan-standard-open-corpus/`); `file` is relative to that
repository's root.

###### `Private`

In the maintainer's private corpus (may not be redistributed, e.g. the
IAEA documents). Cited only; no tool may require the file.

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
    fn clone(self: &Self) -> DocumentTier { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &DocumentTier) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `ConceptDocument`

One `[[document]]` of the skeleton: a source the tree's nodes cite.

```rust
pub struct ConceptDocument {
    pub id: String,
    pub title: String,
    pub publisher: String,
    pub date: String,
    pub tier: DocumentTier,
    pub file: String,
    pub licence: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `id` | `String` | Unique id, e.g. `nureg-0800-toc-rev6`. Sources refer to it. |
| `title` | `String` |  |
| `publisher` | `String` |  |
| `date` | `String` | As the document prints it (`"March 2007"`, `"2015"`). |
| `tier` | `DocumentTier` |  |
| `file` | `String` | The file, relative to its tier's repository root. |
| `licence` | `String` | The licence basis, as recorded in the skeleton. |

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
    fn clone(self: &Self) -> ConceptDocument { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &ConceptDocument) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `ConceptSource`

A node's citation of a document, at section (and sometimes page) level.

```rust
pub struct ConceptSource {
    pub document: String,
    pub section: String,
    pub page: Option<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `document` | `String` | A [`ConceptDocument::id`]. |
| `section` | `String` | The document's own section numbering, verbatim. |
| `page` | `Option<String>` | The printed page, where the document gives one. |

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
    fn clone(self: &Self) -> ConceptSource { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &ConceptSource) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Enum `ConceptOrigin`

Where a level-3 concept comes from (`origin` in the proposals file).
Levels 1 and 2 carry no origin field: their sources say where they come
from, and their [`ConceptNode::origin`] is `None`.

```rust
pub enum ConceptOrigin {
    Nrc,
    Iaea,
    OutramPark,
}
```

##### Variants

###### `Nrc`

A subsection an NRC (or NRC-family) document names itself.

###### `Iaea`

A subsection of the IAEA Milestones text.

###### `OutramPark`

A concept outram-park's code needs that the cited text only implies;
[`ConceptNode::why`] says why.

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
    fn clone(self: &Self) -> ConceptOrigin { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &ConceptOrigin) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Enum `ConceptStatus`

Whether a node is settled.

```rust
pub enum ConceptStatus {
    Approved,
    Deferred,
}
```

##### Variants

###### `Approved`

Approved by the maintainer (every level-1 and level-2 node is).

###### `Deferred`

Placed by the maintainer, awaiting sources. Shown, but greyed.

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
    fn clone(self: &Self) -> ConceptStatus { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &ConceptStatus) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `ConceptNode`

One node of the tree, at any level.

```rust
pub struct ConceptNode {
    pub path: String,
    pub title: String,
    pub level: usize,
    pub sources: Vec<ConceptSource>,
    pub cross_links: Vec<String>,
    pub origin: Option<ConceptOrigin>,
    pub status: ConceptStatus,
    pub why: Option<String>,
    pub note: Option<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `path` | `String` | Slash-separated, parent first, no root prefix:<br>`02-nuclear-safety/nuclear-design/neutron-transport`. |
| `title` | `String` |  |
| `level` | `usize` | 1 (IAEA issue), 2 (review category), 3 or deeper (concept). |
| `sources` | `Vec<ConceptSource>` |  |
| `cross_links` | `Vec<String>` | Other nodes this one also belongs to (paths; the node's home is its<br>own path). Shown dotted on a map. |
| `origin` | `Option<ConceptOrigin>` | `None` for levels 1–2. |
| `status` | `ConceptStatus` |  |
| `why` | `Option<String>` | Why an `outram-park` concept exists. |
| `note` | `Option<String>` | A judgement call recorded for the maintainer. |

##### Implementations

###### Methods

- ```rust
  pub fn parent_path(self: &Self) -> Option<&str> { /* ... */ }
  ```
  The parent's path, or `None` for a level-1 issue.

- ```rust
  pub fn slug(self: &Self) -> &str { /* ... */ }
  ```
  The last path segment.

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
    fn clone(self: &Self) -> ConceptNode { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &ConceptNode) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `ConceptTree`

Levels 1–3 and the documents they cite. Obtain it with [`concept_tree`].

```rust
pub struct ConceptTree {
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
  pub fn parse(skeleton: &str, proposals: &str) -> Result<Self, ConceptTreeError> { /* ... */ }
  ```
  Build a tree from the two files' text. [`concept_tree`] does this

- ```rust
  pub fn nodes(self: &Self) -> &[ConceptNode] { /* ... */ }
  ```
  Every node, levels 1–3, in depth-first tree order: each level-1

- ```rust
  pub fn documents(self: &Self) -> &[ConceptDocument] { /* ... */ }
  ```
  Every declared document, in file order.

- ```rust
  pub fn document(self: &Self, id: &str) -> Option<&ConceptDocument> { /* ... */ }
  ```
  The document with id `id`.

- ```rust
  pub fn node(self: &Self, path: &str) -> Option<&ConceptNode> { /* ... */ }
  ```
  The node at `path`.

- ```rust
  pub fn roots(self: &Self) -> impl Iterator<Item = &ConceptNode> { /* ... */ }
  ```
  The level-1 issues, in IAEA order.

- ```rust
  pub fn children(self: &Self, path: &str) -> impl Iterator<Item = &ConceptNode> { /* ... */ }
  ```
  The direct children of the node at `path` (`""` gives the roots), in

- ```rust
  pub fn parent(self: &Self, path: &str) -> Option<&ConceptNode> { /* ... */ }
  ```
  The parent of the node at `path` (`None` for a level-1 issue or an

- ```rust
  pub fn cross_links(self: &Self, path: &str) -> impl Iterator<Item = &ConceptNode> { /* ... */ }
  ```
  The nodes the node at `path` cross-links to. A link naming no node in

- ```rust
  pub fn cross_linked_from(self: &Self, path: &str) -> impl Iterator<Item = &ConceptNode> { /* ... */ }
  ```
  The nodes that cross-link *to* `path` (the reverse direction of

- ```rust
  pub fn documents_cited_by(self: &Self, path: &str) -> Vec<&ConceptDocument> { /* ... */ }
  ```
  The documents the node at `path` cites, each once, in the order its

- ```rust
  pub fn nodes_citing(self: &Self, id: &str) -> impl Iterator<Item = &ConceptNode> { /* ... */ }
  ```
  The nodes that cite document `id` in their sources, in tree order.

- ```rust
  pub fn is_within(path: &str, ancestor: &str) -> bool { /* ... */ }
  ```
  Whether `path` is `ancestor` or lies below it.

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
- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `ConceptTreeError`

Why the TOML could not be read into a tree.

```rust
pub struct ConceptTreeError(pub String);
```

##### Fields

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
    fn clone(self: &Self) -> ConceptTreeError { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
- **Error**
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
    fn eq(self: &Self, other: &ConceptTreeError) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
### Functions

#### Function `concept_tree`

The compiled-in tree, parsed on first use.

# Panics

If the compiled-in TOML does not parse, which `tests/concept_skeleton.rs`
rules out before anything ships.

```rust
pub fn concept_tree() -> &'static ConceptTree { /* ... */ }
```

### Constants and Statics

#### Constant `SKELETON_TOML`

The skeleton file (levels 1–2 and the documents), compiled in.

```rust
pub const SKELETON_TOML: &str = "# Kovan\'s concept-tree skeleton (OUTRAM PARK GitHub #724, #726, #727).\n# Lives in kovan-literature, beside the corpus it links to (the reactor-literature\n# submodule is mounted in this crate); wasm-clean, so the Code Review tab can be\n# built for the browser as well as the desktop.\n#\n# Two fixed levels, shared by the Literature and Code Review tabs:\n#   L1: the 19 infrastructure issues of IAEA NG-G-3.1 (Rev. 1), numbered so\n#       `NN-` is the document\'s section 3.NN (decided 2026-10-06);\n#   L2: categories from the open NRC guides (NUREG-0800, NUREG-1537 Part 1,\n#       RG 1.232, NUREG-1520, NUREG-1555, NUREG-0654/FEMA-REP-1) and ORNL\'s\n#       MSR gap analysis, plain-word segments with no number.\n# The owner\'s concepts hang below L2 as folders, as today.\n#\n# Every source names a [[document]] by `id`; each document names its file in\n# a corpus repository (`tier = \"standard\"`: reactor-literature; `\"private\"`:\n# the owner\'s private corpus). This is the INITIAL linking, at document and\n# section level (2026-10-06); finer artifact links (page anchors, quotes) are\n# the owner\'s, added later. Section numbers are the documents\' own.\n# `cross_links` names other nodes a node also belongs to (shown as a dotted\n# link; the node\'s home is its path).\n# A node is shown greyed (and hidden by the \'show empty nodes\' toggle) when\n# nothing is classified under it; that is computed, not stored here.\n\n[[document]]\nid = \"iaea-ng-g-3.1-rev1\"\ntitle = \"IAEA Nuclear Energy Series No. NG-G-3.1 (Rev. 1), Milestones in the Development of a National Infrastructure for Nuclear Power\"\npublisher = \"IAEA, Vienna\"\ndate = \"2015\"\ntier = \"private\"\nfile = \"reports/iaea2015ngg31rev1-milestones.pdf\"\nlicence = \"(c) IAEA; cited by section and page only\"\n\n[[document]]\nid = \"nureg-0800-toc-rev6\"\ntitle = \"NUREG-0800, Standard Review Plan for the Review of Safety Analysis Reports for Nuclear Power Plants, Table of Contents, Revision 6\"\npublisher = \"U.S. NRC\"\ndate = \"March 2007\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/nrc/ML070810350.pdf\"\nlicence = \"U.S. Government work\"\n\n[[document]]\nid = \"nureg-0800-4.2-rev3\"\ntitle = \"NUREG-0800, Standard Review Plan, Section 4.2, Revision 3, Fuel System Design\"\npublisher = \"U.S. NRC\"\ndate = \"March 2007\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/nrc/ML070740002.pdf\"\nlicence = \"U.S. Government work\"\n\n[[document]]\nid = \"nureg-1537-part1\"\ntitle = \"NUREG-1537, Part 1, Guidelines for Preparing and Reviewing Applications for the Licensing of Non-Power Reactors: Format and Content\"\npublisher = \"U.S. NRC, Office of Nuclear Reactor Regulation\"\ndate = \"February 1996\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/nrc/nureg-1537-part1-1996.pdf\"\nlicence = \"U.S. Government work\"\n\n[[document]]\nid = \"rg-1.232-rev0\"\ntitle = \"Regulatory Guide 1.232, Revision 0, Guidance for Developing Principal Design Criteria for Non-Light-Water Reactors\"\npublisher = \"U.S. NRC\"\ndate = \"April 2018\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/nrc/ML17325A611.pdf\"\nlicence = \"U.S. Government work\"\n\n[[document]]\nid = \"ornl-tm-2018-976\"\ntitle = \"ORNL/TM-2018/976, Regulatory Gap Analysis of Select NUREG-0800 Chapters for Applicability to Molten Salt Reactors (Belles, Flanagan)\"\npublisher = \"Oak Ridge National Laboratory for U.S. DOE\"\ndate = \"October 2018\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/us-doe/ornl-tm-2018-976-msr-nureg0800-gap-analysis.pdf\"\nlicence = \"approved for public release, distribution unlimited\"\n\n[[document]]\nid = \"nureg-1520-rev2\"\ntitle = \"NUREG-1520, Revision 2, Standard Review Plan for Fuel Cycle Facilities License Applications\"\npublisher = \"U.S. NRC, Office of Nuclear Material Safety and Safeguards\"\ndate = \"2015\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/nrc/nureg-1520-rev2-2015.pdf\"\nlicence = \"U.S. Government work\"\n\n[[document]]\nid = \"nureg-1555\"\ntitle = \"NUREG-1555, Standard Review Plans for Environmental Reviews for Nuclear Power Plants\"\npublisher = \"U.S. NRC, Office of Nuclear Reactor Regulation\"\ndate = \"October 1999\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/nrc/nureg-1555-1999.pdf\"\nlicence = \"U.S. Government work\"\n\n[[document]]\nid = \"nureg-0654-rev2\"\ntitle = \"NUREG-0654/FEMA-REP-1, Revision 2, Criteria for Preparation and Evaluation of Radiological Emergency Response Plans and Preparedness in Support of Nuclear Power Plants\"\npublisher = \"U.S. NRC and FEMA\"\ndate = \"December 2019\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/nrc/nureg-0654-fema-rep-1-rev2-2019.pdf\"\nlicence = \"U.S. Government work\"\n\n[[document]]\nid = \"jrc-eur-28712\"\ntitle = \"K. Kugeler, H. Nabielek, D. Buckthorpe, The High Temperature Gas-cooled Reactor: Safety considerations of the (V)HTR-Modul, EUR 28712 EN\"\npublisher = \"European Commission, Joint Research Centre (Publications Office of the European Union)\"\ndate = \"2017\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/eu-jrc/kjna28712enn.pdf\"\nlicence = \"reuse authorised provided the source is acknowledged (Decision 2011/833/EU)\"\n\n[[document]]\nid = \"10cfr50\"\ntitle = \"10 CFR Part 50, Domestic Licensing of Production and Utilization Facilities (eCFR, as of 2 Oct 2026)\"\npublisher = \"U.S. Government (Office of the Federal Register; NRC regulations, 10 CFR Chapter I)\"\ndate = \"2 October 2026 (eCFR)\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/cfr/10cfr50-ecfr-2026-10-02.pdf\"\nlicence = \"U.S. Government work (17 U.S.C. 105); eCFR: authoritative but unofficial\"\n\n[[document]]\nid = \"10cfr52\"\ntitle = \"10 CFR Part 52, Licenses, Certifications, and Approvals for Nuclear Power Plants (eCFR, as of 2 Oct 2026)\"\npublisher = \"U.S. Government (Office of the Federal Register; NRC regulations, 10 CFR Chapter I)\"\ndate = \"2 October 2026 (eCFR)\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/cfr/10cfr52-ecfr-2026-10-02.pdf\"\nlicence = \"U.S. Government work (17 U.S.C. 105); eCFR: authoritative but unofficial\"\n\n[[document]]\nid = \"10cfr53\"\ntitle = \"10 CFR Part 53, Risk-Informed, Technology-Inclusive Regulatory Framework for Commercial Nuclear Plants (eCFR, as of 2 Oct 2026; final rule 91 FR 15794, Mar. 30, 2026)\"\npublisher = \"U.S. Government (Office of the Federal Register; NRC regulations, 10 CFR Chapter I)\"\ndate = \"2 October 2026 (eCFR)\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/cfr/10cfr53-ecfr-2026-10-02.pdf\"\nlicence = \"U.S. Government work (17 U.S.C. 105); eCFR: authoritative but unofficial\"\n\n[[document]]\nid = \"iaea-nutec-plastics\"\ntitle = \"IAEA, NUTEC Plastics: Scaling up solutions and partnerships for global impact\"\npublisher = \"IAEA\"\ndate = \"July 2026\"\ntier = \"private\"\nfile = \"reports/iaea2026-nutec-plastics-scaling-up.pdf\"\nlicence = \"no reuse licence found; cited only\"\n\n[[document]]\nid = \"wash-1400\"\ntitle = \"WASH-1400 (NUREG-75/014), Reactor Safety Study: An Assessment of Accident Risks in U.S. Commercial Nuclear Power Plants\"\npublisher = \"U.S. NRC\"\ndate = \"October 1975\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/nrc/ML15334A199.pdf\"\nlicence = \"U.S. Government work\"\n\n[[document]]\nid = \"nureg-br-0167\"\ntitle = \"NUREG/BR-0167, Software Quality Assurance Program and Guidelines\"\npublisher = \"U.S. NRC, Division of Information Support Services, Office of Information Resources Management\"\ndate = \"February 1993\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/nrc/nureg-br-0167-1993-sqa-program-and-guidelines.pdf\"\nlicence = \"U.S. Government work\"\n\n[[document]]\nid = \"doe-g-414.1-4\"\ntitle = \"DOE G 414.1-4, Safety Software Guide for Use with 10 CFR 830 Subpart A, Quality Assurance Requirements, and DOE O 414.1C, Quality Assurance\"\npublisher = \"U.S. Department of Energy, Office of Environment, Safety and Health\"\ndate = \"June 2005 (certified November 2010)\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/us-doe/doe-g-414-1-4-2005-safety-software-guide.pdf\"\nlicence = \"U.S. Government work (DOE guide); public domain per DOE\'s web policies, acknowledge DOE\"\n\n[[document]]\nid = \"doe-std-1172-2003\"\ntitle = \"DOE-STD-1172-2003, Safety Software Quality Assurance Functional Area Qualification Standard (superseded by DOE-STD-1172-2011)\"\npublisher = \"U.S. Department of Energy\"\ndate = \"December 2003\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/us-doe/doe-std-1172-2003-safety-software-qa-faqs.pdf\"\nlicence = \"DOE standard, Distribution Statement A: approved for public release, distribution unlimited\"\n\n[[node]]\npath = \"01-national-position\"\ntitle = \"National position\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.1\", page = \"10\" }]\n\n[[node]]\npath = \"01-national-position/need-for-power\"\ntitle = \"Need for power\"\nsources = [{ document = \"nureg-1555\", section = \"Ch. 8\" }]\n\n[[node]]\npath = \"02-nuclear-safety\"\ntitle = \"Nuclear safety\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.2\", page = \"14\" }]\n\n[[node]]\npath = \"02-nuclear-safety/facility-description\"\ntitle = \"Facility description\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 1\" }, { document = \"nureg-1537-part1\", section = \"Ch. 1\" }, { document = \"rg-1.232-rev0\", section = \"Sec. I, Overall requirements\" }]\n\n[[node]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components\"\ntitle = \"Design of structures, systems and components\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 3\" }, { document = \"nureg-1537-part1\", section = \"Ch. 3\" }, { document = \"10cfr53\", section = \"\u{a7} 53.415 Protection against external hazards\" }]\n\n[[node]]\npath = \"02-nuclear-safety/fuel-system-design\"\ntitle = \"Fuel system design\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"4.2\" }, { document = \"nureg-0800-4.2-rev3\", section = \"whole section\" }, { document = \"nureg-1537-part1\", section = \"4.2.1\" }, { document = \"rg-1.232-rev0\", section = \"Sec. II, Multiple barriers (MHTGR-DC 10, 16 functional containment)\" }, { document = \"ornl-tm-2018-976\", section = \"3.1.1\" }]\n\n[[node]]\npath = \"02-nuclear-safety/nuclear-design\"\ntitle = \"Nuclear design and core physics\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"4.3\" }, { document = \"nureg-1537-part1\", section = \"4.5 (4.5.2 core physics parameters)\" }, { document = \"rg-1.232-rev0\", section = \"Sec. III, Reactivity control\" }, { document = \"ornl-tm-2018-976\", section = \"3.1.2\" }]\n\n[[node]]\npath = \"02-nuclear-safety/moderator-and-reflector\"\ntitle = \"Moderator and reflector\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.2.3\" }]\n\n[[node]]\npath = \"02-nuclear-safety/thermal-hydraulic-design\"\ntitle = \"Thermal-hydraulic design\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"4.4\" }, { document = \"nureg-1537-part1\", section = \"4.6\" }, { document = \"rg-1.232-rev0\", section = \"Sec. IV, Fluid systems\" }, { document = \"ornl-tm-2018-976\", section = \"3.1.3\" }]\n\n[[node]]\npath = \"02-nuclear-safety/control-rods-and-drives\"\ntitle = \"Control rods and drives\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"4.5, 4.6\" }, { document = \"nureg-1537-part1\", section = \"4.2.2\" }, { document = \"ornl-tm-2018-976\", section = \"3.1.4, 3.1.6\" }]\n\n[[node]]\npath = \"02-nuclear-safety/reactor-coolant-system\"\ntitle = \"Reactor coolant system\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 5\" }, { document = \"nureg-1537-part1\", section = \"Ch. 5\" }, { document = \"rg-1.232-rev0\", section = \"Sec. IV\" }, { document = \"ornl-tm-2018-976\", section = \"3.2 (proposed 5.4.20 fuel salt drain tank, 5.4.21 freeze valve boundary)\" }]\n\n[[node]]\npath = \"02-nuclear-safety/msr-coolant-loop\"\ntitle = \"MSR coolant loop\"\nsources = [{ document = \"ornl-tm-2018-976\", section = \"3.2.4 (proposed SRP Section 5.5)\" }]\n\n[[node]]\npath = \"02-nuclear-safety/engineered-safety-features\"\ntitle = \"Engineered safety features\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 6\" }, { document = \"nureg-1537-part1\", section = \"Ch. 6\" }, { document = \"ornl-tm-2018-976\", section = \"3.3\" }]\n\n[[node]]\npath = \"02-nuclear-safety/containment\"\ntitle = \"Containment (incl. functional containment)\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"6.2\" }, { document = \"rg-1.232-rev0\", section = \"Sec. V, Reactor containment\" }, { document = \"ornl-tm-2018-976\", section = \"3.3.2\" }]\n\n[[node]]\npath = \"02-nuclear-safety/instrumentation-and-control\"\ntitle = \"Instrumentation and control\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 7\" }, { document = \"nureg-1537-part1\", section = \"Ch. 7\" }]\n\n[[node]]\npath = \"02-nuclear-safety/auxiliary-systems\"\ntitle = \"Auxiliary systems\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 9\" }, { document = \"nureg-1537-part1\", section = \"Ch. 9\" }, { document = \"ornl-tm-2018-976\", section = \"3.4 (proposed 9.2.X drain tank cooling)\" }]\n\n[[node]]\npath = \"02-nuclear-safety/steam-and-power-conversion\"\ntitle = \"Steam and power conversion system\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 10\" }]\n\n[[node]]\npath = \"02-nuclear-safety/experimental-facilities-and-utilization\"\ntitle = \"Experimental facilities and utilization\"\nsources = [{ document = \"nureg-1537-part1\", section = \"Ch. 10\" }]\n\n[[node]]\npath = \"02-nuclear-safety/conduct-of-operations\"\ntitle = \"Conduct of operations\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 13\" }, { document = \"nureg-1537-part1\", section = \"Ch. 12\" }]\n\n[[node]]\npath = \"02-nuclear-safety/initial-test-program\"\ntitle = \"Initial test program and ITAAC\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 14\" }]\n\n[[node]]\npath = \"02-nuclear-safety/accident-analysis\"\ntitle = \"Accident analysis\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 15 (15.0.2 review of transient and accident analysis methods)\" }, { document = \"nureg-1537-part1\", section = \"Ch. 13\" }]\n\n[[node]]\npath = \"02-nuclear-safety/technical-specifications\"\ntitle = \"Technical specifications\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 16\" }, { document = \"nureg-1537-part1\", section = \"Ch. 14\" }, { document = \"10cfr50\", section = \"\u{a7} 50.36 Technical specifications\" }, { document = \"10cfr53\", section = \"\u{a7} 53.710 Maintaining capabilities and availability of structures, systems, and components, (a)\" }]\n\n[[node]]\npath = \"02-nuclear-safety/human-factors-engineering\"\ntitle = \"Human factors engineering\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 18\" }, { document = \"10cfr53\", section = \"\u{a7} 53.730 Defining, fulfilling, and maintaining the role of personnel in ensuring safe operations, (a) Human factors engineering design requirements\" }, { document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (n)(1)\" }]\n\n[[node]]\npath = \"02-nuclear-safety/severe-accidents\"\ntitle = \"Severe accidents\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 19\" }]\n\n[[node]]\npath = \"02-nuclear-safety/source-terms\"\ntitle = \"Source terms\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"11.1 Source Terms; 15.0.1 Radiological Consequence Analyses Using Alternate Source Terms\" }, { document = \"nureg-1555\", section = \"Ch. 7 (postulated accidents)\" }, { document = \"ornl-tm-2018-976\", section = \"3.5.1 (coolant source terms, MSR)\" }]\ncross_links = [\"13-environmental-protection\"]\n\n[[node]]\npath = \"03-management\"\ntitle = \"Management\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.3\", page = \"17\" }]\n\n[[node]]\npath = \"03-management/organization-and-administration\"\ntitle = \"Organization and administration\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"Ch. 2\" }]\n\n[[node]]\npath = \"03-management/management-measures\"\ntitle = \"Management measures\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"Ch. 11\" }]\n\n[[node]]\npath = \"04-funding-and-financing\"\ntitle = \"Funding and financing\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.4\", page = \"20\" }]\n\n[[node]]\npath = \"04-funding-and-financing/financial-qualifications\"\ntitle = \"Financial qualifications\"\nsources = [{ document = \"nureg-1537-part1\", section = \"Ch. 15\" }, { document = \"10cfr50\", section = \"\u{a7} 50.33 Contents of applications; general information, (f)\" }, { document = \"10cfr50\", section = \"Appendix C, A Guide for the Financial Data and Related Information Required To Establish Financial Qualifications for Construction Permits and Combined Licenses\" }, { document = \"10cfr50\", section = \"\u{a7} 50.76 Licensee\'s change of status; financial qualifications\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1670 Financial qualifications\" }]\n\n[[node]]\npath = \"05-legal-framework\"\ntitle = \"Legal framework\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.5\", page = \"25\" }]\n\n[[node]]\npath = \"05-legal-framework/nuclear-legislation\"\ntitle = \"Nuclear legislation (safety, security, safeguards and civil liability)\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.5\", page = \"25\" }]\n\n[[node]]\npath = \"05-legal-framework/international-legal-instruments\"\ntitle = \"International legal instruments (conventions adopted under IAEA auspices)\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.5, Box 1\", page = \"26\" }]\n\n[[node]]\npath = \"05-legal-framework/independent-regulatory-body\"\ntitle = \"Independent regulatory body (separation of regulatory and promotional functions)\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.5\", page = \"25\" }]\ncross_links = [\"07-regulatory-framework\"]\n\n[[node]]\npath = \"05-legal-framework/civil-liability-for-nuclear-damage\"\ntitle = \"Civil liability for nuclear damage (Vienna, Paris, CSC; U.S. financial protection)\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.5, Box 1\", page = \"26\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1710-53.1730 (financial protection under part 140)\" }]\ncross_links = [\"04-funding-and-financing/financial-qualifications/financial-protection-and-accident-insurance\"]\n\n[[node]]\npath = \"05-legal-framework/licensee-obligations\"\ntitle = \"Licensee obligations (deliberate misconduct, employee protection, completeness and accuracy, violations, license transfers, exemptions)\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.5, 50.7, 50.9, 50.110, 50.80, 50.12\" }, { document = \"10cfr52\", section = \"\u{a7} 52.4-52.7\" }, { document = \"10cfr53\", section = \"\u{a7} 53.050-53.080, 53.1570\" }]\n\n[[node]]\npath = \"06-safeguards\"\ntitle = \"Safeguards\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.6\", page = \"28\" }]\n\n[[node]]\npath = \"06-safeguards/material-control-and-accounting\"\ntitle = \"Material control and accounting\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"Ch. 12\" }, { document = \"10cfr50\", section = \"\u{a7} 50.78 Facility information and verification\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1650 Facility information and verification\" }]\n\n[[node]]\npath = \"07-regulatory-framework\"\ntitle = \"Regulatory framework\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.7\", page = \"30\" }]\n\n[[node]]\npath = \"07-regulatory-framework/reactor-licensing\"\ntitle = \"Reactor licensing\"\nsources = [{ document = \"10cfr50\", section = \"Part 50\" }, { document = \"10cfr52\", section = \"Part 52\" }, { document = \"10cfr53\", section = \"Part 53\" }]\n\n[[node]]\npath = \"07-regulatory-framework/quality-assurance\"\ntitle = \"Quality assurance (incl. software QA)\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 17\" }]\n\n[[node]]\npath = \"07-regulatory-framework/other-license-considerations\"\ntitle = \"Other license considerations\"\nsources = [{ document = \"nureg-1537-part1\", section = \"Ch. 16\" }]\n\n[[node]]\npath = \"08-radiation-protection\"\ntitle = \"Radiation protection\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.8\", page = \"35\" }]\n\n[[node]]\npath = \"08-radiation-protection/radiation-protection\"\ntitle = \"Radiation protection\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 12\" }, { document = \"nureg-1537-part1\", section = \"Ch. 11\" }, { document = \"nureg-1520-rev2\", section = \"Ch. 4\" }]\n\n[[node]]\npath = \"09-electrical-grid\"\ntitle = \"Electrical grid\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.9\", page = \"36\" }]\n\n[[node]]\npath = \"09-electrical-grid/electric-power\"\ntitle = \"Electric power\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 8\" }, { document = \"nureg-1537-part1\", section = \"Ch. 8\" }]\n\n[[node]]\npath = \"10-human-resource-development\"\ntitle = \"Human resource development\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.10\", page = \"38\" }]\n\n[[node]]\npath = \"10-human-resource-development/knowledge-management-and-education\"\ntitle = \"Knowledge management and education\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"\u{a7}3.10, p. 38\" }]\n\n[[node]]\npath = \"11-stakeholder-involvement\"\ntitle = \"Stakeholder involvement\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.11\", page = \"42\" }]\n\n[[node]]\npath = \"11-stakeholder-involvement/public-information-and-communication\"\ntitle = \"Public information and communication (surveys, information tools, benefits and risks)\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.11.1\", page = \"43\" }]\n\n[[node]]\npath = \"11-stakeholder-involvement/stakeholder-involvement-programmes\"\ntitle = \"Stakeholder involvement programmes (government, owner/operator, regulatory body; neighbouring countries)\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.11.1-3.11.2\", page = \"43\" }]\n\n[[node]]\npath = \"11-stakeholder-involvement/public-participation-in-licensing\"\ntitle = \"Public participation in licensing (public inspection of applications, notice for comment, hearings, ACRS)\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.39, 50.58, 50.91\" }, { document = \"10cfr52\", section = \"\u{a7} 52.21, 52.85, 52.163\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1121, 53.1155\" }]\ncross_links = [\"07-regulatory-framework/reactor-licensing\"]\n\n[[node]]\npath = \"12-site-and-supporting-facilities\"\ntitle = \"Site and supporting facilities\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.12\", page = \"45\" }]\n\n[[node]]\npath = \"12-site-and-supporting-facilities/site-characteristics\"\ntitle = \"Site characteristics\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 2\" }, { document = \"nureg-1537-part1\", section = \"Ch. 2\" }, { document = \"10cfr53\", section = \"\u{a7} 53.500 General siting and siting assessment\" }, { document = \"10cfr53\", section = \"\u{a7} 53.520 Site characteristics\" }, { document = \"10cfr50\", section = \"Appendix Q, Pre-Application Early Review of Site Suitability Issues\" }]\n\n[[node]]\npath = \"13-environmental-protection\"\ntitle = \"Environmental protection\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.13\", page = \"48\" }]\n\n[[node]]\npath = \"13-environmental-protection/environmental-impact-statement\"\ntitle = \"Environmental impact statement\"\nsources = [{ document = \"nureg-1555\", section = \"Ch. 1\" }]\n\n[[node]]\npath = \"13-environmental-protection/environmental-description\"\ntitle = \"Environmental description\"\nsources = [{ document = \"nureg-1555\", section = \"Ch. 2\" }]\n\n[[node]]\npath = \"13-environmental-protection/meteorology-and-air-quality\"\ntitle = \"Meteorology and air quality (atmospheric dispersion)\"\nsources = [{ document = \"nureg-1555\", section = \"2.7\" }, { document = \"nureg-0800-toc-rev6\", section = \"2.3 (2.3.4 short-term, 2.3.5 long-term dispersion)\" }, { document = \"nureg-1537-part1\", section = \"2.3\" }]\n\n[[node]]\npath = \"13-environmental-protection/plant-description\"\ntitle = \"Plant description\"\nsources = [{ document = \"nureg-1555\", section = \"Ch. 3\" }]\n\n[[node]]\npath = \"13-environmental-protection/construction-impacts\"\ntitle = \"Environmental impacts of construction\"\nsources = [{ document = \"nureg-1555\", section = \"Ch. 4\" }]\n\n[[node]]\npath = \"13-environmental-protection/station-operation-impacts\"\ntitle = \"Environmental impacts of station operation\"\nsources = [{ document = \"nureg-1555\", section = \"Ch. 5\" }]\n\n[[node]]\npath = \"13-environmental-protection/radiological-impacts-of-normal-operation\"\ntitle = \"Radiological impacts of normal operation\"\nsources = [{ document = \"nureg-1555\", section = \"5.4\" }]\n\n[[node]]\npath = \"13-environmental-protection/environmental-monitoring\"\ntitle = \"Environmental measurements and monitoring programs\"\nsources = [{ document = \"nureg-1555\", section = \"Ch. 6\" }, { document = \"10cfr50\", section = \"\u{a7} 50.36b Environmental conditions\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1112 Environmental conditions\" }]\n\n[[node]]\npath = \"13-environmental-protection/postulated-accident-impacts\"\ntitle = \"Environmental impacts of postulated accidents\"\nsources = [{ document = \"nureg-1555\", section = \"Ch. 7\" }]\n\n[[node]]\npath = \"13-environmental-protection/alternatives\"\ntitle = \"Alternatives to the proposed action\"\nsources = [{ document = \"nureg-1555\", section = \"Ch. 9\" }]\n\n[[node]]\npath = \"13-environmental-protection/environmental-consequences\"\ntitle = \"Environmental consequences of the proposed action\"\nsources = [{ document = \"nureg-1555\", section = \"Ch. 10\" }]\n\n[[node]]\npath = \"13-environmental-protection/facility-environmental-protection\"\ntitle = \"Environmental protection (fuel cycle facilities)\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"Ch. 9\" }]\n\n[[node]]\npath = \"14-emergency-planning\"\ntitle = \"Emergency planning\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.14\", page = \"50\" }]\n\n[[node]]\npath = \"14-emergency-planning/assignment-of-responsibility\"\ntitle = \"Assignment of responsibility\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard A\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(1)\" }]\n\n[[node]]\npath = \"14-emergency-planning/emergency-response-organization\"\ntitle = \"Emergency response organization\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard B\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(2)\" }]\n\n[[node]]\npath = \"14-emergency-planning/emergency-response-support-and-resources\"\ntitle = \"Emergency response support and resources\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard C\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(3)\" }]\n\n[[node]]\npath = \"14-emergency-planning/emergency-classification-system\"\ntitle = \"Emergency classification system\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard D\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(4)\" }]\n\n[[node]]\npath = \"14-emergency-planning/notification-methods-and-procedures\"\ntitle = \"Notification methods and procedures\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard E\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(5)\" }]\n\n[[node]]\npath = \"14-emergency-planning/emergency-communications\"\ntitle = \"Emergency communications\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard F\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(6)\" }]\n\n[[node]]\npath = \"14-emergency-planning/public-education-and-information\"\ntitle = \"Public education and information\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard G\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(7)\" }]\n\n[[node]]\npath = \"14-emergency-planning/emergency-facilities-and-equipment\"\ntitle = \"Emergency facilities and equipment\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard H\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(8)\" }]\n\n[[node]]\npath = \"14-emergency-planning/accident-assessment\"\ntitle = \"Accident assessment\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard I\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(9)\" }]\n\n[[node]]\npath = \"14-emergency-planning/protective-response\"\ntitle = \"Protective response\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard J\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(10)\" }]\n\n[[node]]\npath = \"14-emergency-planning/radiological-exposure-control\"\ntitle = \"Radiological exposure control\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard K\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(11)\" }]\n\n[[node]]\npath = \"14-emergency-planning/medical-and-public-health-support\"\ntitle = \"Medical and public health support\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard L\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(12)\" }]\n\n[[node]]\npath = \"14-emergency-planning/recovery-reentry-and-post-accident-operations\"\ntitle = \"Recovery, reentry, and post-accident operations\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard M\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(13)\" }]\n\n[[node]]\npath = \"14-emergency-planning/exercises-and-drills\"\ntitle = \"Exercises and drills\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard N\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(14)\" }]\n\n[[node]]\npath = \"14-emergency-planning/radiological-emergency-response-training\"\ntitle = \"Radiological emergency response training\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard O\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(15)\" }]\n\n[[node]]\npath = \"14-emergency-planning/emergency-plan-development-and-review\"\ntitle = \"Responsibility for the planning effort\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard P\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(16)\" }, { document = \"10cfr50\", section = \"Appendix E, Emergency Planning and Preparedness for Production and Utilization Facilities\" }, { document = \"10cfr53\", section = \"\u{a7} 53.855 Emergency preparedness\" }]\n\n[[node]]\npath = \"14-emergency-planning/emergency-management\"\ntitle = \"Emergency management (fuel cycle facilities)\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"Ch. 8\" }]\n\n[[node]]\npath = \"15-nuclear-security\"\ntitle = \"Nuclear security\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.15\", page = \"52\" }]\n\n[[node]]\npath = \"15-nuclear-security/cybersecurity\"\ntitle = \"Cybersecurity and information security\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.860 Security programs, (d) Cybersecurity; (e) Information security\" }, { document = \"10cfr50\", section = \"\u{a7} 50.34 Contents of applications; technical information, (c)(2)\" }]\n\n[[node]]\npath = \"15-nuclear-security/physical-protection\"\ntitle = \"Physical protection\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"Ch. 13\" }, { document = \"10cfr53\", section = \"\u{a7} 53.860 Security programs, (a) Physical protection program\" }, { document = \"10cfr50\", section = \"\u{a7} 50.34 Contents of applications; technical information, (c) Physical security plan\" }]\n\n[[node]]\npath = \"16-nuclear-fuel-cycle\"\ntitle = \"Nuclear fuel cycle\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.16\", page = \"54\" }]\n\n# Maintainer 2026-10-06: waste disposal sits in the fuel cycle (back end);\n# topics await 10 CFR Parts 61/63 or IAEA literature.\n[[node]]\npath = \"16-nuclear-fuel-cycle/waste-disposal\"\ntitle = \"Waste disposal (near-surface and geological)\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.16\", page = \"54\" }]\ncross_links = [\"17-radioactive-waste-management\"]\n\n# Maintainer 2026-10-06: kaki-bukit (CYCLUS port) lives here.\n[[node]]\npath = \"16-nuclear-fuel-cycle/fuel-cycle-scenarios\"\ntitle = \"Fuel cycle scenarios and material flows\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.16\", page = \"54\" }]\n\n[[node]]\npath = \"16-nuclear-fuel-cycle/fuel-cycle-facility-general-information\"\ntitle = \"General information (fuel cycle facilities)\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"Ch. 1\" }]\n\n[[node]]\npath = \"16-nuclear-fuel-cycle/integrated-safety-analysis\"\ntitle = \"Integrated safety analysis\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"Ch. 3\" }]\n\n[[node]]\npath = \"16-nuclear-fuel-cycle/nuclear-criticality-safety\"\ntitle = \"Nuclear criticality safety\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"Ch. 5\" }]\n\n[[node]]\npath = \"16-nuclear-fuel-cycle/chemical-process-safety\"\ntitle = \"Chemical process safety\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"Ch. 6\" }]\n\n[[node]]\npath = \"16-nuclear-fuel-cycle/fire-safety\"\ntitle = \"Fire safety\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"Ch. 7\" }]\n\n[[node]]\npath = \"16-nuclear-fuel-cycle/heu-to-leu-conversion\"\ntitle = \"HEU to LEU conversion\"\nsources = [{ document = \"nureg-1537-part1\", section = \"Ch. 18\" }, { document = \"10cfr50\", section = \"\u{a7} 50.64 Limitations on the use of highly enriched uranium (HEU) in domestic non-power reactors\" }]\n\n[[node]]\npath = \"16-nuclear-fuel-cycle/fuel-depletion\"\ntitle = \"Fuel depletion and burnup\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.16\", page = \"54\" }]\n\n[[node]]\npath = \"17-radioactive-waste-management\"\ntitle = \"Radioactive waste management\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.17\", page = \"56\" }]\n\n[[node]]\npath = \"17-radioactive-waste-management/radioactive-waste-management\"\ntitle = \"Radioactive waste management\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 11\" }, { document = \"nureg-1537-part1\", section = \"Ch. 11\" }, { document = \"ornl-tm-2018-976\", section = \"3.5\" }, { document = \"10cfr50\", section = \"\u{a7} 50.34a Design objectives for equipment to control releases of radioactive material in effluents\u{2014}nuclear power reactors\" }, { document = \"10cfr50\", section = \"Appendix F, Policy Relating to the Siting of Fuel Reprocessing Plants and Related Waste Management Facilities\" }]\n\n[[node]]\npath = \"17-radioactive-waste-management/decommissioning\"\ntitle = \"Decommissioning\"\nsources = [{ document = \"nureg-1537-part1\", section = \"Ch. 17\" }, { document = \"nureg-1520-rev2\", section = \"Ch. 10\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1000 Scope and purpose (Subpart G, Decommissioning Requirements)\" }]\n\n[[node]]\npath = \"18-industrial-involvement\"\ntitle = \"Industrial involvement\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.18\", page = \"58\" }]\n\n# Maintainer 2026-10-06: process heat, chemical processes (DWSIM, provisional).\n# World knowledge to add: IAEA NUTEC Plastics (nuclear techniques against plastic\n# pollution) -- literature to be supplied.\n[[node]]\npath = \"18-industrial-involvement/process-heat-and-industrial-applications\"\ntitle = \"Process heat and industrial applications\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.18\", page = \"58\" }]\n\n[[node]]\npath = \"19-procurement\"\ntitle = \"Procurement\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.19\", page = \"60\" }]\n\n[[node]]\npath = \"19-procurement/procurement-capability-and-policy\"\ntitle = \"Procurement capability and policy\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.19.1-3.19.3\", page = \"60\" }]\n\n[[node]]\npath = \"19-procurement/supplier-quality-and-specifications\"\ntitle = \"Supplier quality and specifications\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.19\", page = \"60\" }, { document = \"10cfr50\", section = \"Appendix B, IV. Procurement Document Control; VII. Control of Purchased Material, Equipment, and Services\" }]\ncross_links = [\"07-regulatory-framework/quality-assurance\"]\n\n[[node]]\npath = \"19-procurement/reporting-of-defects-and-noncompliance\"\ntitle = \"Reporting of defects and noncompliance\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.55(e)\" }, { document = \"10cfr53\", section = \"\u{a7} 53.605 Reporting of defects and noncompliance\" }]\ncross_links = [\"18-industrial-involvement\"]\n\n[[node]]\npath = \"19-procurement/emergency-procurement\"\ntitle = \"Emergency procurement (urgent supply; pre-positioned emergency equipment)\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.19.3\", page = \"61\" }]\ncross_links = [\"14-emergency-planning\"]\n";
```

#### Constant `PROPOSALS_TOML`

The concepts file (level 3, plus the level-4 seeds this module ignores),
compiled in.

```rust
pub const PROPOSALS_TOML: &str = "# Kovan concept-tree PROPOSALS: level 3 (concepts) and level 4 (outram-park\n# implementation leaves). OUTRAM PARK GitHub #724, #726, #727, #729.\n#\n# STATUS: every entry here is `status = \"proposed\"`. NOTHING in this file is\n# part of the tree until the maintainer approves it, line by line. Approved\n# concepts move into `concept_skeleton.toml` (or a public-concepts file beside\n# it); rejected ones are deleted here. The readable review copy is\n# `docs/concept-proposals.md`, regenerated from this file by\n# `KOVAN_REGENERATE_PROPOSALS=1 cargo test --release -p kovan-literature --test concept_proposals`\n# (the test fails while the two disagree).\n#\n# [[concept]] (L3; may nest under another proposed concept)\n#   path    full path, under an L2 node of the skeleton (or under another\n#           proposed concept); plain lower-case hyphenated segments\n#   title   the source document\'s own words, verbatim or lightly shortened\n#   origin  \"nrc\"         = a subsection the cited document names itself;\n#           \"outram-park\" = a concept the outram-park code needs that the\n#                           cited text names only implicitly (`why` says why)\n#   sources [{ document, section }], document = a skeleton [[document]] id;\n#           sections read from the document text (2026-10-06), never from memory\n#   note    optional: a judgement call the maintainer should look at\n#\n# [[implementation]] (L4, outram-park only; world leaves are the maintainer\'s)\n#   concept the full path of a proposed concept (or a skeleton node)\n#   crate   a workspace member directory under crates/\n#   module  a file or directory under crates/<crate>/src/, without `.rs`\n#   kind    \"port\" (with `upstream`) or \"new-work\" (`upstream` = what it is\n#           built from, when that is a published method rather than code)\n# L4 will be REGENERATED by kovan from `//! kovan-concept: <L3 path>` lines in\n# each module\'s own doc comment (maintainer, 2026-10-06); these entries are the\n# SEED set of those tags. The test `tests/concept_proposals.rs` checks every\n# crate/module still exists on develop, so a refactor that moves a module\n# fails until this file is updated.\n\n# ============================================================================\n# 02-nuclear-safety / design-of-structures-systems-and-components\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/wind-and-tornado-loadings\"\ntitle = \"Wind and tornado loadings (meteorological damage)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"3.3.1 Wind Loading; 3.3.2 Tornado Loads\" }, { document = \"nureg-1537-part1\", section = \"3.2 Meteorological Damage\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/flood-protection\"\ntitle = \"Flood protection (water damage)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"3.4.1 Internal Flood Protection for Onsite Equipment Failures; 3.4.2 Analysis Procedures\" }, { document = \"nureg-1537-part1\", section = \"3.3 Water Damage\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/missile-protection\"\ntitle = \"Protection against missiles\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"3.5.1.1-3.5.3 (internally generated, turbine, tornado and site-proximity missiles; aircraft hazards; barrier design)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/protection-against-piping-failures\"\ntitle = \"Protection against postulated piping failures\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"3.6.1-3.6.3 (incl. 3.6.3 Leak-Before-Break Evaluation Procedures)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/seismic-design\"\ntitle = \"Seismic design (parameters, system and subsystem analysis, instrumentation)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"3.7.1-3.7.4\" }, { document = \"nureg-1537-part1\", section = \"3.4 Seismic Damage\" }, { document = \"10cfr50\", section = \"Appendix S, Earthquake Engineering Criteria for Nuclear Power Plants\" }, { document = \"10cfr53\", section = \"\u{a7} 53.480 Earthquake engineering\" }, { document = \"10cfr53\", section = \"\u{a7} 53.720 Response to seismic events\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/seismic-category-i-structures\"\ntitle = \"Containment and other Seismic Category I structures and foundations\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"3.8.1-3.8.5\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/mechanical-systems-and-components\"\ntitle = \"Mechanical systems and components\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"3.9.1 Special Topics for Mechanical Components; 3.9.2-3.9.8; 3.12; 3.13\" }, { document = \"nureg-1537-part1\", section = \"3.5 Systems and Components\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/equipment-qualification\"\ntitle = \"Seismic, dynamic and environmental qualification of equipment\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"3.10; 3.11\" }, { document = \"10cfr50\", section = \"\u{a7} 50.49 Environmental qualification of electric equipment important to safety for nuclear power plants\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / fuel-system-design   (SRP 4.2 Rev. 3 review areas)\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/fuel-system-damage\"\ntitle = \"Fuel system damage\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"II.1.A Fuel System Damage\" }, { document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 10 Reactor design\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/fuel-system-damage/oxidation-hydriding-and-crud\"\ntitle = \"Oxidation, hydriding, and the buildup of corrosion products (crud)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"II.1.A.iv\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/fuel-system-damage/rod-internal-gas-pressure\"\ntitle = \"Fuel and burnable poison rod internal gas pressures\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"II.1.A.vi; II.3.C.vii Fuel Rod Pressure\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/fuel-rod-failure\"\ntitle = \"Fuel rod failure\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"II.1.B Fuel Rod Failure (i hydriding ... viii mechanical fracturing)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/fuel-rod-failure/pellet-cladding-interaction\"\ntitle = \"Pellet/cladding interaction\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"II.1.B.vi\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/fuel-coolability\"\ntitle = \"Fuel coolability\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"II.1.C Fuel Coolability (cladding embrittlement, violent expulsion, melting, ballooning, structural deformation)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/fuel-performance-analytical-predictions\"\ntitle = \"Analytical predictions of fuel performance\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"II.3.C Analytical Predictions\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/fuel-performance-analytical-predictions/fuel-temperatures-and-stored-energy\"\ntitle = \"Fuel temperatures (stored energy)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"II.3.C.i\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/testing-inspection-and-surveillance\"\ntitle = \"Testing, inspection, and surveillance plans\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"I.4; II.4 (new fuel, online monitoring, postirradiation surveillance)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/reactivity-initiated-accident-criteria\"\ntitle = \"Acceptance criteria for reactivity-initiated accidents\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"Appendix B, Interim Acceptance Criteria and Guidance for the Reactivity Initiated Accidents\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/fuel-assembly-structural-response\"\ntitle = \"Fuel assembly structural response to externally applied forces\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"Appendix A\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/fuel-salt-chemistry\"\ntitle = \"Fuel salt chemistry (MSR)\"\norigin = \"nrc\"\nsources = [{ document = \"ornl-tm-2018-976\", section = \"3.1.1 Section 4.2, Fuel System Design\" }]\nnote = \"ORNL, not NRC; origin \'nrc\' here means \'named by the cited regulatory text\'.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel\"\ntitle = \"TRISO coated-particle fuel as the primary fission-product barrier\"\norigin = \"nrc\"\nsources = [{ document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 10 Reactor design (rationale: TRISO is the primary fission product barrier; SARRDLs)\" }]\ncross_links = [\"02-nuclear-safety/containment/functional-containment\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-failure\"\ntitle = \"TRISO coated-particle failure (pressure-vessel failure under accident conditions)\"\norigin = \"outram-park\"\nsources = [{ document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 10 rationale\" }]\nwhy = \"boon-lay computes particle failure fractions; the RG names TRISO retention, not the failure mechanisms.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release\"\ntitle = \"Fission-product diffusion and release from TRISO particles\"\norigin = \"outram-park\"\nsources = [{ document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 10 rationale; MHTGR-DC 16\" }]\nwhy = \"boon-lay\'s TRISO-ATOPS fork is the release model behind every outram-park HTGR source term.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-conduction\"\ntitle = \"Heat conduction through the TRISO particle layers\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"II.3.C.i (fuel and cladding temperature distribution)\" }]\nwhy = \"the innermost of the pebble-bed conduction scales; SRP 4.2\'s temperature distribution in TRISO geometry.\"\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / nuclear-design\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/core-configuration-and-criticality\"\ntitle = \"Normal operating conditions: core configurations and criticality physics\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.1 Normal Operating Conditions\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/core-configurations/pebble-bed/pebble-bed-packing\"\ntitle = \"Pebble-bed packing (core geometry of a pebble-bed reactor)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.1 (core geometry and configurations)\" }]\nwhy = \"the packing fraction and pebble positions are the core configuration of HTR-10 (gh #216); DEM and packing algorithms produce it.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/power-distribution\"\ntitle = \"Power distribution (axial and radial neutron flux densities, peaking)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.2 Reactor Core Physics Parameters\" }, { document = \"ornl-tm-2018-976\", section = \"3.1.2 (SRP 4.3 \'focuses on the core power distribution and reactivity coefficients\')\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/reactivity-coefficients\"\ntitle = \"Coefficients of reactivity (fuel and moderator temperature, void, power)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.2\" }, { document = \"ornl-tm-2018-976\", section = \"3.1.2\" }, { document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 11 Reactor inherent protection\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/kinetics-parameters\"\ntitle = \"Neutron lifetime and effective delayed neutron fraction\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.2\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/reactor-kinetic-behaviour\"\ntitle = \"Kinetic behaviour of the reactor for steady-state and transient operation\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 Nuclear Design (introduction); 4.5.1 (analyses of the reactor kinetic behavior)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/reactivity-worths\"\ntitle = \"Reactivity worths of fuel, reflector, experimental components and control rods\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.1\" }, { document = \"ornl-tm-2018-976\", section = \"3.1.2 (control rods, control rod patterns, and reactivity worths)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/burnup-and-poison-reactivity-effects\"\ntitle = \"Changes in core reactivity with fuel burnup, plutonium buildup, and poisons\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.1\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/excess-reactivity-and-shutdown-margin\"\ntitle = \"Operating limits: excess reactivity and shutdown margin\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.3 Operating Limits\" }, { document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 26 Reactivity control systems; MHTGR-DC 28 Reactivity limits\" }, { document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (g)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/suppression-of-power-oscillations\"\ntitle = \"Suppression of reactor power oscillations\"\norigin = \"nrc\"\nsources = [{ document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 12\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/accuracy-of-analytical-methods\"\ntitle = \"Estimates of the accuracy of the analytical methods (uncertainty and sensitivity)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 Nuclear Design (introduction)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/core-configurations/liquid-fuelled/delayed-neutron-precursor-drift\"\ntitle = \"Delayed-neutron precursor drift in circulating fuel (MSR)\"\norigin = \"outram-park\"\nsources = [{ document = \"ornl-tm-2018-976\", section = \"3.1.2 (MSRs may use ... intrinsic nuclear phenomena, such as flow, to provide reactor control)\" }]\nwhy = \"the MSRE reactivity effect; moltres and GeN-Foam\'s precursor_drift model it.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/neutron-transport\"\ntitle = \"Neutron transport methods\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 Nuclear Design (introduction: a detailed description of the analytical methods ... computer codes)\" }]\nwhy = \"the NRC asks for the analytical methods but does not name them; outram-park\'s transport solvers live here.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/neutron-transport/surface-tracking\"\ntitle = \"Surface tracking through constructive solid geometry\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 (introduction)\" }]\nwhy = \"OpenMC\'s tracking method; the CSG description and its navigation kernel.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/neutron-transport/delta-tracking\"\ntitle = \"Delta (Woodcock) tracking\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 (introduction)\" }]\nwhy = \"NEW WORK in outram-mc (OpenMC has none); the tracking that makes doubly heterogeneous pebble beds affordable.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/neutron-transport/delta-tracking/majorant\"\ntitle = \"The majorant cross section\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 (introduction)\" }]\nwhy = \"the bound every delta-tracking flight is sampled on; a correctness invariant of its own.\"\nnote = \"Nested under delta-tracking rather than a sibling of it, since it exists only for delta tracking.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/neutron-transport/doubly-heterogeneous-geometry\"\ntitle = \"Doubly heterogeneous geometry (TRISO particles in pebbles, stochastic media)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.1 (interacting effects of fuel, neutron moderators and reflectors)\" }]\nwhy = \"pebble-bed specialisation of outram-mc: explicit, RPT and stochastic (CLS/SCLS) treatments.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/neutron-transport/continuous-energy-collision-physics\"\ntitle = \"Continuous-energy collision physics (scattering, fission)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 (introduction)\" }]\nwhy = \"the per-collision kernels of Monte Carlo transport, ported from OpenMC.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/neutron-transport/eigenvalue-and-fixed-source-calculations\"\ntitle = \"k-eigenvalue (power iteration) and fixed-source calculations\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.1 (calculated core reactivities for all core configurations)\" }]\nwhy = \"the run modes that produce k_eff and source-driven flux.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/neutron-transport/variance-reduction\"\ntitle = \"Variance reduction (weight windows, uniform fission sites)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 (estimates of the accuracy of the analytical methods)\" }]\nwhy = \"Monte Carlo efficiency techniques ported from OpenMC.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/neutron-transport/monte-carlo-tallies\"\ntitle = \"Monte Carlo tallies (structured and unstructured mesh, filters, triggers)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.2 (axial and radial distributions of neutron flux densities)\" }]\nwhy = \"how fluxes and reaction rates are estimated from histories.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/neutron-transport/deterministic-neutronics\"\ntitle = \"Deterministic neutronics (multigroup diffusion, SP3, discrete ordinates, nodal)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 (introduction)\" }]\nwhy = \"the GeN-Foam, Moltres and BEDOK solver family.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/neutron-transport/multigroup-cross-section-condensation\"\ntitle = \"Multigroup cross sections condensed from Monte Carlo\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 (methods of obtaining parameters such as cross sections)\" }]\nwhy = \"the bridge from stochastic to deterministic transport (nee_soon).\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/nuclear-data-processing\"\ntitle = \"Nuclear data processing (evaluated data to transport libraries)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 (introduction: methods of obtaining parameters such as cross sections)\" }]\nwhy = \"the NJOY2016 port; the NRC names cross sections as a method input only.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/resonance-reconstruction\"\ntitle = \"Resonance reconstruction of pointwise cross sections\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 (introduction)\" }]\nwhy = \"NJOY RECONR and the SAMM R-matrix kernel.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/doppler-broadening\"\ntitle = \"Doppler broadening of cross sections\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.2 (fuel temperature coefficient)\" }]\nwhy = \"the temperature dependence behind the fuel temperature coefficient: NJOY BROADR and windowed multipole.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/unresolved-resonance-probability-tables\"\ntitle = \"Unresolved-resonance self-shielding and probability tables\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 (introduction)\" }]\nwhy = \"NJOY UNRESR/PURR; on by default in outram-mc since 2026-09-20 (root CLAUDE.md).\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/thermal-scattering\"\ntitle = \"Thermal neutron scattering, S(alpha, beta)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.2.3 Neutron Moderator and Reflector; 4.5\" }]\nwhy = \"bound-atom scattering in graphite and other moderators: NJOY LEAPR/THERMR and its use in transport.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/ace-library-generation\"\ntitle = \"Continuous-energy (ACE) library generation\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 (introduction)\" }]\nwhy = \"NJOY ACER, the hand-off from processing to Monte Carlo transport.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/multigroup-data-generation\"\ntitle = \"Multigroup cross sections and transfer matrices from evaluated data\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 (introduction)\" }]\nwhy = \"NJOY GROUPR/GAMINR.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/nuclear-data-covariances\"\ntitle = \"Nuclear-data covariances\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 (estimates of the accuracy of the analytical methods)\" }]\nwhy = \"NJOY ERRORR/COVR: the data half of the accuracy estimate.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/heating-and-damage\"\ntitle = \"Heating (KERMA) and damage cross sections\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6 (thermal power density distribution)\" }]\nwhy = \"NJOY HEATR: where deposited power comes from.\"\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / moderator-and-reflector   (NUREG-1537 4.2.3)\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/moderator-and-reflector/moderator-and-reflector-nuclear-design\"\ntitle = \"Nuclear design of the moderator and reflector\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.2.3 (the nuclear design of the moderator and reflector should be discussed in Section 4.5)\" }]\ncross_links = [\"02-nuclear-safety/nuclear-design/nuclear-data-processing/thermal-scattering\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/moderator-and-reflector/cooling-radiation-damage-and-encapsulation\"\ntitle = \"Provisions for cooling, radiation damage, and failure of encapsulation\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.2.3\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / thermal-hydraulic-design   (SRP 4.4, NUREG-1537 4.6)\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/core-coolant-hydraulics\"\ntitle = \"Coolant hydraulic characteristics of the core (flow rates, pressures, frictional and buoyant forces)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6 Thermal-Hydraulic Design, first item\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/power-density-and-heat-flux-distribution\"\ntitle = \"Thermal power density distribution and heat fluxes into the coolant\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6, second item\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/heat-transfer-to-coolant\"\ntitle = \"Transfer of heat to the coolant (thermal-hydraulic methodology)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6, third item\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/fuel-heat-removal-limits\"\ntitle = \"Fuel heat-removal limits (onset of nucleate boiling, departure from nucleate boiling, flow instability)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6, fifth item\" }, { document = \"nureg-0800-4.2-rev3\", section = \"I (SRP 4.4 provides DNBR and CPR criteria)\" }]\nnote = \"LWR mechanisms (ONB, DNB, CHF, CPR): not applicable to MSR technology (ORNL/TM-2018/976 3.1.3); see gas-generation-and-entrainment.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-stability\"\ntitle = \"Susceptibility to thermal-hydraulic instability\"\norigin = \"nrc\"\nsources = [{ document = \"ornl-tm-2018-976\", section = \"3.1.3 (quoting SRP 4.4: \'(4) is not susceptible to thermal-hydraulic instability\')\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/natural-convection-cooling\"\ntitle = \"Natural-convection cooling (forced-to-natural transition; decay-heat removal)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6, first and sixth items\" }, { document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 34 Passive residual heat removal\" }]\nnote = \"The brief proposed \'natural-circulation\' as outram-park origin; NUREG-1537 4.6 names it, so it is marked nrc.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/shutdown-decay-heat\"\ntitle = \"Shutdown decay heat\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6, fourth item (operating conditions should include ... shutdown decay heat)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/pulsing-reactor-analysis\"\ntitle = \"Pulse analysis (feedback coefficients and thermal-hydraulic evolution during a pulse)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6, last item\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics\"\ntitle = \"Computational fluid dynamics (finite volume)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6 (a detailed description of the analytical methods used in the thermal-hydraulic design)\" }]\nwhy = \"the OpenFOAM port (outram-foam-*); the NRC asks for methods without naming them.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics/mesh-generation\"\ntitle = \"Mesh generation for finite-volume and finite-element solvers\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6 (introduction)\" }]\nwhy = \"blockMesh/snappyHexMesh/cfMesh ports and the neutral unstructured mesh (#492).\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/system-thermal-hydraulics\"\ntitle = \"System (1-D network) thermal hydraulics\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6 (introduction)\" }, { document = \"ornl-tm-2018-976\", section = \"3.1.3 (focus on acceptable analytical methods)\" }]\nwhy = \"TUAS and TAMPINES control-volume networks; the peer-reviewed TUAS work sits here.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/two-phase-flow\"\ntitle = \"Two-phase flow and boiling\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6, fifth item\" }]\nwhy = \"two-fluid, drift-flux and homogeneous models behind the boiling limits.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/coolant-thermophysical-properties\"\ntitle = \"Coolant and structure thermophysical properties\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6, third item (uncertainties in thermal-hydraulic ... parameters)\" }]\nwhy = \"IAPWS-IF97, CoolProp\'s Helmholtz EOS, and TUAS\'s salt and solid property library.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/pebble-bed-thermal-hydraulics\"\ntitle = \"Pebble-bed thermal hydraulics (packed-bed pressure drop, effective conductivity)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6, first and third items\" }]\nwhy = \"KTA 3102 correlations, ZBS conductivity and contact conduction in the bed.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/neutronics-thermal-hydraulics-coupling\"\ntitle = \"Neutronics and thermal-hydraulics coupling\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6, second item (heat fluxes derived from the fuel loading and neutron flux characteristics)\" }]\nwhy = \"multiphysics coupling in GeN-Foam, nee_soon and BEDOK.\"\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / control-rods-and-drives\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/control-rods-and-drives/control-rods\"\ntitle = \"Control rods\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.2.2 Control Rods\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/control-rods-and-drives/control-rod-drive-structural-materials\"\ntitle = \"Control rod drive structural materials\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"4.5.1\" }, { document = \"ornl-tm-2018-976\", section = \"3.1.4\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/control-rods-and-drives/functional-design-of-control-rod-drive-system\"\ntitle = \"Functional design of control rod drive system\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"4.6\" }, { document = \"ornl-tm-2018-976\", section = \"3.1.6\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / reactor-coolant-system\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/reactor-coolant-system/reactor-coolant-pressure-boundary\"\ntitle = \"Reactor coolant (helium) pressure boundary\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"5.2.1.1-5.2.5\" }, { document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 14, 15, 30-32\" }, { document = \"ornl-tm-2018-976\", section = \"3.2.1\" }]\ncross_links = [\"02-nuclear-safety/accident-analysis/air-and-moisture-ingress\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/reactor-coolant-system/reactor-vessel-integrity\"\ntitle = \"Reactor vessel materials and integrity\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"5.3.1-5.3.3\" }, { document = \"ornl-tm-2018-976\", section = \"3.2.2\" }, { document = \"10cfr50\", section = \"\u{a7} 50.60 Acceptance criteria for fracture prevention measures for lightwater nuclear power reactors for normal operation\" }, { document = \"10cfr50\", section = \"\u{a7} 50.66 Requirements for thermal annealing of the reactor pressure vessel\" }, { document = \"10cfr50\", section = \"Appendix G, Fracture Toughness Requirements\" }, { document = \"10cfr50\", section = \"Appendix H, Reactor Vessel Material Surveillance Program Requirements\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/reactor-coolant-system/residual-heat-removal\"\ntitle = \"Residual heat removal (passive, in the MHTGR criteria)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"5.4.7 Residual Heat Removal (RHR) System\" }, { document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 34, 36, 37\" }, { document = \"ornl-tm-2018-976\", section = \"3.2.3\" }, { document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (h)\" }]\ncross_links = [\"02-nuclear-safety/thermal-hydraulic-design/natural-convection-cooling\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/reactor-coolant-system/steam-generators\"\ntitle = \"Steam generators\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"5.4.2.1 Steam Generator Materials; 5.4.2.2 Steam Generator Program\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/reactor-coolant-system/primary-heat-exchangers\"\ntitle = \"Primary heat exchanger\"\norigin = \"nrc\"\nsources = [{ document = \"ornl-tm-2018-976\", section = \"3.2.3 (proposed MSR subsection)\" }, { document = \"nureg-1537-part1\", section = \"5.2 Primary Coolant System; 5.3 Secondary Coolant System\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/reactor-coolant-system/coolant-pumps-and-circulators\"\ntitle = \"Reactor coolant pumps (fuel salt pump; gas circulator)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"5.4.1.1 Pump Flywheel Integrity (PWR)\" }, { document = \"ornl-tm-2018-976\", section = \"3.2.3 (fuel salt pump)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/reactor-coolant-system/primary-coolant-cleanup-and-makeup\"\ntitle = \"Primary coolant cleanup and makeup water systems\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"5.4; 5.5\" }, { document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 33 Reactor coolant makeup\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/reactor-coolant-system/nitrogen-16-control\"\ntitle = \"Nitrogen-16 control system\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"5.6\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/reactor-coolant-system/fuel-salt-drain-tank-and-freeze-valve\"\ntitle = \"Fuel salt drain tank and freeze valve boundary (MSR)\"\norigin = \"nrc\"\nsources = [{ document = \"ornl-tm-2018-976\", section = \"3.2.3 (proposed 5.4.20, 5.4.21)\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / msr-coolant-loop   (ORNL 3.2.4, proposed SRP 5.5)\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/msr-coolant-loop/coolant-loop-materials-and-chemistry\"\ntitle = \"Coolant loop materials and chemistry\"\norigin = \"nrc\"\nsources = [{ document = \"ornl-tm-2018-976\", section = \"3.2.4 (coolant loop materials; coolant loop chemistry)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/msr-coolant-loop/coolant-loop-cleanup-sampling-and-makeup\"\ntitle = \"Coolant loop cleanup, sampling and makeup systems\"\norigin = \"nrc\"\nsources = [{ document = \"ornl-tm-2018-976\", section = \"3.2.4\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/msr-coolant-loop/coolant-loop-pump-and-heat-exchangers\"\ntitle = \"Coolant loop pump and coolant heat exchangers\"\norigin = \"nrc\"\nsources = [{ document = \"ornl-tm-2018-976\", section = \"3.2.4\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/msr-coolant-loop/coolant-loop-drain-tank-and-isolation\"\ntitle = \"Coolant loop drain tank, in-service inspection and testing, isolation valves\"\norigin = \"nrc\"\nsources = [{ document = \"ornl-tm-2018-976\", section = \"3.2.4\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / engineered-safety-features\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/engineered-safety-features/emergency-core-cooling\"\ntitle = \"Emergency core cooling system\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"6.3\" }, { document = \"nureg-1537-part1\", section = \"6.2.3\" }, { document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 35\" }, { document = \"10cfr50\", section = \"\u{a7} 50.46 Acceptance criteria for emergency core cooling systems for light-water nuclear power reactors\" }, { document = \"10cfr50\", section = \"Appendix K, ECCS Evaluation Models\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/engineered-safety-features/confinement\"\ntitle = \"Confinement\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"6.2.1 Confinement\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/engineered-safety-features/control-room-habitability\"\ntitle = \"Control room habitability system\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"6.4\" }, { document = \"ornl-tm-2018-976\", section = \"3.3.4\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/engineered-safety-features/fission-product-cleanup-systems\"\ntitle = \"ESF atmosphere cleanup and fission product control systems\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"6.5.1-6.5.5\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/engineered-safety-features/esf-materials\"\ntitle = \"Engineered safety features materials\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"6.1.1; 6.1.2\" }, { document = \"ornl-tm-2018-976\", section = \"3.3.1\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / containment\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/containment/functional-containment\"\ntitle = \"Functional containment\"\norigin = \"nrc\"\nsources = [{ document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 16 Containment design (and its rationale)\" }, { document = \"ornl-tm-2018-976\", section = \"3.3.2\" }]\nnote = \"The brief suggested functional containment under fuel-system-design; the skeleton\'s containment node names it, so it is placed here (TRISO retention stays under fuel-system-design).\"\ncross_links = [\"02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/containment/containment-functional-design\"\ntitle = \"Containment functional design (incl. mass and energy release analyses)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"6.2.1-6.2.1.5\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/containment/containment-heat-removal\"\ntitle = \"Containment heat removal systems\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"6.2.2\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/containment/containment-isolation-and-leakage-testing\"\ntitle = \"Containment isolation and leakage testing\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"6.2.4; 6.2.6\" }, { document = \"10cfr50\", section = \"Appendix J, Primary Reactor Containment Leakage Testing for Water-Cooled Power Reactors\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/containment/combustible-gas-control\"\ntitle = \"Combustible gas control in containment\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"6.2.5\" }, { document = \"10cfr50\", section = \"\u{a7} 50.44 Combustible gas control for nuclear power reactors\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / instrumentation-and-control\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/instrumentation-and-control/reactor-protection-system\"\ntitle = \"Reactor trip (protection) system\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"7.2 Reactor Trip System\" }, { document = \"nureg-1537-part1\", section = \"7.4 Reactor Protection System\" }, { document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 20-25\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/instrumentation-and-control/engineered-safety-features-actuation\"\ntitle = \"Engineered safety features (actuation) systems\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"7.3\" }, { document = \"nureg-1537-part1\", section = \"7.5\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/instrumentation-and-control/control-systems\"\ntitle = \"Control systems\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"7.7 Control Systems\" }, { document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 13 Instrumentation and control\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/instrumentation-and-control/information-systems-and-displays\"\ntitle = \"Information systems important to safety; control console and display instruments\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"7.5\" }, { document = \"nureg-1537-part1\", section = \"7.6\" }, { document = \"10cfr53\", section = \"\u{a7} 53.730 Defining, fulfilling, and maintaining the role of personnel in ensuring safe operations, (b) Human system interface design requirements\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/instrumentation-and-control/radiation-monitoring-systems\"\ntitle = \"Radiation monitoring systems\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"7.7\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/instrumentation-and-control/digital-i-and-c-software\"\ntitle = \"Digital instrumentation and control: software reviews\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"7.0-A; BTP 7-14 Guidance on Software Reviews for Digital Computer-Based Instrumentation and Controls Systems\" }]\ncross_links = [\"07-regulatory-framework/quality-assurance/software-quality-assurance\"]\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / auxiliary-systems\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/auxiliary-systems/fuel-storage-and-handling\"\ntitle = \"Fuel storage and handling (incl. criticality safety of fresh and spent fuel)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"9.1.1-9.1.5\" }, { document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 61, 62\" }, { document = \"ornl-tm-2018-976\", section = \"3.4.1\" }, { document = \"10cfr50\", section = \"\u{a7} 50.68 Criticality accident requirements, (b)\" }, { document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (m)(2)\" }]\ncross_links = [\"16-nuclear-fuel-cycle/nuclear-criticality-safety\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/auxiliary-systems/cooling-water-and-ultimate-heat-sink\"\ntitle = \"Cooling fluids and ultimate heat sink\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"9.2.1-9.2.6 (9.2.5 Ultimate Heat Sink)\" }, { document = \"ornl-tm-2018-976\", section = \"3.4.2 (incl. fuel salt drain tank cooling system)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/auxiliary-systems/ventilation-systems\"\ntitle = \"Heating, ventilation, and air conditioning systems\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"9.4.1-9.4.5\" }, { document = \"nureg-1537-part1\", section = \"9.1\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/auxiliary-systems/fire-protection\"\ntitle = \"Fire protection program\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"9.5.1\" }, { document = \"10cfr50\", section = \"\u{a7} 50.48 Fire protection\" }, { document = \"10cfr50\", section = \"Appendix R, Fire Protection Program for Nuclear Power Facilities Operating Prior to January 1, 1979\" }, { document = \"10cfr53\", section = \"\u{a7} 53.875 Fire protection\" }, { document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (e)\" }, { document = \"10cfr53\", section = \"\u{a7} 53.450 Analysis requirements, (g)(1) Fire protection\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/auxiliary-systems/cover-gas-control\"\ntitle = \"Cover gas control in closed primary coolant systems\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"9.6\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / steam-and-power-conversion   (SRP Ch. 10)\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/steam-and-power-conversion/turbine-generator\"\ntitle = \"Turbine generator\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"10.2; 10.2.3\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/steam-and-power-conversion/main-steam-supply\"\ntitle = \"Main steam supply system\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"10.3; 10.3.6\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/steam-and-power-conversion/main-condensers\"\ntitle = \"Main condensers\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"10.4.1; 10.4.2\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/steam-and-power-conversion/circulating-water-system\"\ntitle = \"Circulating water system\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"10.4.5\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/steam-and-power-conversion/condensate-and-feedwater\"\ntitle = \"Condensate and feedwater system\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"10.4.6; 10.4.7; 10.4.9\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/steam-and-power-conversion/power-cycle-flowsheet-simulation\"\ntitle = \"Power-conversion cycle and flowsheet simulation\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 10 Steam and Power Conversion System\" }]\nwhy = \"the Rankine cycle and the DWSIM equipment models (heat exchangers, compressors, expanders) assemble the Ch. 10 systems.\"\ncross_links = [\"18-industrial-involvement/process-heat-and-industrial-applications\"]\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / conduct-of-operations, initial-test-program, technical-specifications\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/conduct-of-operations/operator-training-and-requalification\"\ntitle = \"Operator training and requalification\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"13.2.1; 13.2.2\" }, { document = \"nureg-1537-part1\", section = \"12.10\" }, { document = \"10cfr50\", section = \"\u{a7} 50.120 Training and qualification of nuclear power plant personnel\" }, { document = \"10cfr53\", section = \"\u{a7} 53.745 Operator license requirements\" }, { document = \"10cfr53\", section = \"\u{a7} 53.760 Operator licensing\" }, { document = \"10cfr53\", section = \"\u{a7} 53.780 Training, examination, and proficiency program\" }, { document = \"10cfr53\", section = \"\u{a7} 53.830 Training and qualification of commercial nuclear personnel\" }]\ncross_links = [\"10-human-resource-development/knowledge-management-and-education\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/conduct-of-operations/procedures\"\ntitle = \"Administrative, operating and emergency operating procedures\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"13.5.1.1; 13.5.2.1\" }, { document = \"nureg-1537-part1\", section = \"12.3\" }, { document = \"10cfr53\", section = \"\u{a7} 53.910 Procedures and guidelines\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/conduct-of-operations/review-and-audit\"\ntitle = \"Review and audit activities\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"12.2\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/conduct-of-operations/startup-plan\"\ntitle = \"Startup plan\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"12.11\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/initial-test-program/initial-plant-test-program\"\ntitle = \"Initial plant test program\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"14.2; 14.2.1\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/initial-test-program/itaac\"\ntitle = \"Inspections, tests, analyses, and acceptance criteria\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"14.3-14.3.12\" }, { document = \"10cfr52\", section = \"\u{a7} 52.99 Inspection during construction; ITAAC schedules and notifications; NRC notices\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1449 Inspection during construction\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/technical-specifications/safety-limits-and-limiting-safety-system-settings\"\ntitle = \"Safety limits and limiting safety system settings\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"Ch. 14 appendix, 2.1 Safety Limits; 2.2 Limiting Safety System Settings\" }, { document = \"10cfr50\", section = \"\u{a7} 50.36 Technical specifications, (c)(1)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/technical-specifications/limiting-conditions-for-operation\"\ntitle = \"Limiting conditions for operation and surveillance requirements\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"Ch. 14 appendix, 3.1-3.9 and 4.x\" }, { document = \"nureg-0800-toc-rev6\", section = \"16.0\" }, { document = \"10cfr50\", section = \"\u{a7} 50.36 Technical specifications, (c)(2)-(3)\" }, { document = \"10cfr53\", section = \"\u{a7} 53.710 Maintaining capabilities and availability of structures, systems, and components, (a)(3)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/technical-specifications/risk-informed-technical-specifications\"\ntitle = \"Risk-informed decision making: technical specifications\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"16.1\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / accident-analysis   (SRP Ch. 15 event categories; NUREG-1537 Ch. 13)\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/transient-and-accident-analysis-methods\"\ntitle = \"Review of transient and accident analysis methods\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"15.0.2\" }, { document = \"nureg-1537-part1\", section = \"13.2 Accident Analysis and Determination of Consequences\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/increase-in-heat-removal\"\ntitle = \"Increase in heat removal by the secondary system\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"15.1.1-15.1.5 (feedwater temperature and flow, steam flow, relief valve opening, steam system piping failures)\" }]\nnote = \"The SRP ToC lists the 15.1-15.9 events but prints no category titles; the category titles here are composed from the listed events and should be checked against SRP 15.0. Category title composed from the listed SRP events; to verify against SRP 15.0 (not in the corpus).\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/decrease-in-heat-removal\"\ntitle = \"Decrease in heat removal by the secondary system (incl. loss of normal AC power)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"15.2.1-15.2.8\" }, { document = \"nureg-1537-part1\", section = \"13.1.7 Loss of Normal Electrical Power\" }]\nnote = \"Category title composed from the listed SRP events; to verify against SRP 15.0 (not in the corpus).\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/decrease-in-reactor-coolant-flow\"\ntitle = \"Decrease in reactor coolant flow\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"15.3.1-15.3.4\" }, { document = \"nureg-1537-part1\", section = \"13.1.4 Loss of Coolant Flow\" }]\nnote = \"Category title composed from the listed SRP events; to verify against SRP 15.0 (not in the corpus).\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/reactivity-and-power-distribution-anomalies\"\ntitle = \"Reactivity and power distribution anomalies (rod withdrawal, ejection, drop; insertion of excess reactivity)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"15.4.1-15.4.9\" }, { document = \"nureg-1537-part1\", section = \"13.1.2 Insertion of Excess Reactivity\" }]\nnote = \"Category title composed from the listed SRP events; to verify against SRP 15.0 (not in the corpus).\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/increase-in-reactor-coolant-inventory\"\ntitle = \"Increase in reactor coolant inventory\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"15.5.1-15.5.2\" }]\nnote = \"Category title composed from the listed SRP events; to verify against SRP 15.0 (not in the corpus).\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/decrease-in-reactor-coolant-inventory\"\ntitle = \"Decrease in reactor coolant inventory (loss of coolant accidents)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"15.6.1-15.6.5\" }, { document = \"nureg-1537-part1\", section = \"13.1.3 Loss of Coolant\" }]\nnote = \"Category title composed from the listed SRP events; to verify against SRP 15.0 (not in the corpus).\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/releases-from-subsystems-and-fuel-handling\"\ntitle = \"Radioactive release from a subsystem or component; mishandling or malfunction of fuel\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"15.7.3-15.7.5\" }, { document = \"nureg-1537-part1\", section = \"13.1.5 Mishandling or Malfunction of Fuel\" }]\nnote = \"Category title composed from the listed SRP events; to verify against SRP 15.0 (not in the corpus).\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/anticipated-transients-without-scram\"\ntitle = \"Anticipated transients without scram\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"15.8\" }, { document = \"10cfr50\", section = \"\u{a7} 50.62 Requirements for reduction of risk from anticipated transients without scram (ATWS) events for light-water-cooled nuclear power plants\" }]\nnote = \"Category title composed from the listed SRP events; to verify against SRP 15.0 (not in the corpus).\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/maximum-hypothetical-accident\"\ntitle = \"Maximum hypothetical accident\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"13.1.1\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/external-events-and-experiment-malfunction\"\ntitle = \"External events; experiment malfunction; mishandling or malfunction of equipment\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"13.1.6; 13.1.8; 13.1.9\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / severe-accidents, source-terms\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/severe-accidents/probabilistic-risk-assessment\"\ntitle = \"Probabilistic risk assessment (technical adequacy; risk-informed changes)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"19.0; 19.1; 19.2\" }, { document = \"10cfr53\", section = \"\u{a7} 53.450 Analysis requirements, (a)-(c)\" }, { document = \"10cfr50\", section = \"\u{a7} 50.71 Maintenance of records, making of reports, (h)\" }, { document = \"10cfr52\", section = \"\u{a7} 52.47 Contents of applications; technical information, (a)(27)\" }, { document = \"10cfr52\", section = \"\u{a7} 52.79 Contents of applications; technical information in final safety analysis report, (a)(46)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/severe-accidents/severe-accident-evaluation\"\ntitle = \"Severe accident evaluation\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"19.0\" }, { document = \"nureg-1555\", section = \"7.2 Severe Accidents; 7.3 Severe Accident Mitigation Alternatives\" }, { document = \"10cfr52\", section = \"\u{a7} 52.47 Contents of applications; technical information, (a)(23)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/source-terms/coolant-source-terms\"\ntitle = \"Coolant source terms (normal operation and anticipated operational occurrences)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"11.1 Source Terms\" }, { document = \"ornl-tm-2018-976\", section = \"3.5.1 Section 11.1, Coolant Source Terms\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/source-terms/accident-source-terms\"\ntitle = \"Accident source terms and radiological consequence analyses\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"15.0.1 Radiological Consequence Analyses Using Alternate Source Terms; 15.0.3\" }, { document = \"nureg-1555\", section = \"7.1 Design Basis Accidents\" }, { document = \"10cfr50\", section = \"\u{a7} 50.67 Accident source term\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/source-terms/fission-product-inventory\"\ntitle = \"Fission product inventory (gap inventory as a release fraction)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"I (gap inventory); II.3.C.ix Fission Product Inventory; Appendix B, D. Fission Product Inventory\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 07-regulatory-framework / quality-assurance   (SRP Ch. 17)\n# ============================================================================\n\n[[concept]]\npath = \"07-regulatory-framework/quality-assurance/quality-assurance-program\"\ntitle = \"Quality assurance during design, construction and operations; QA program description\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"Appendix B, Quality Assurance Criteria for Nuclear Power Plants and Fuel Reprocessing Plants\" }, { document = \"nureg-0800-toc-rev6\", section = \"17.1; 17.2; 17.3; 17.5\" }, { document = \"nureg-1537-part1\", section = \"12.9 Quality Assurance\" }, { document = \"10cfr53\", section = \"\u{a7} 53.865 Quality assurance\" }]\ncross_links = [\"03-management\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/quality-assurance/reliability-assurance-program\"\ntitle = \"Reliability assurance program\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"17.4\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/quality-assurance/maintenance-rule\"\ntitle = \"Maintenance rule\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"17.6\" }, { document = \"10cfr50\", section = \"\u{a7} 50.65 Requirements for monitoring the effectiveness of maintenance at nuclear power plants\" }, { document = \"10cfr53\", section = \"\u{a7} 53.715 Maintenance, repair, and inspection programs\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/quality-assurance/software-quality-assurance\"\ntitle = \"Software quality assurance\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 17; BTP 7-14 (software reviews)\" }, { document = \"nureg-1520-rev2\", section = \"5.4.3.1.7.1 Calculational Method Validation\" }, { document = \"nureg-br-0167\", section = \"the NRC\'s SQA program and guidelines for software developed for NRC staff use (whole document)\" }, { document = \"doe-std-1172-2003\", section = \"Required Technical Competencies 1-12\" }, { document = \"doe-g-414.1-4\", section = \"2.1 software types; 2.2 graded application (levels A, B, C); 5.2 the ten SQA work activities; Table B-2\" }]\nwhy = \"agreed on #726 as the home of NQA-1-style software QA; the SRP names QA and I&C software reviews but not scientific-software QA.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/quality-assurance/software-quality-assurance/verification-and-validation-records\"\ntitle = \"Verification and validation records (gates, oracles, recorded results)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"5.4.3.1.7.1\" }]\nwhy = \"every outram-park V&V gate records methodology and results (root CLAUDE.md); these modules hold them.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/quality-assurance/software-quality-assurance/configuration-and-accounting-records\"\ntitle = \"Configuration and accounting records (commit trailers, historian reports)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 17\" }]\nwhy = \"kovan-metrics keeps the per-commit records and the pre-merge historian report.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/quality-assurance/software-quality-assurance/code-review\"\ntitle = \"Code review and code walks\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"BTP 7-14\" }]\nwhy = \"kovan\'s Code Review tab and code walks: the review machinery itself.\"\nstatus = \"approved\"\n\n# ============================================================================\n# 08-radiation-protection / radiation-protection\n# ============================================================================\n\n[[concept]]\npath = \"08-radiation-protection/radiation-protection/protection-of-plant-workers/alara\"\ntitle = \"Assuring occupational radiation exposures are as low as is reasonably achievable\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"12.1\" }, { document = \"nureg-1537-part1\", section = \"11.1.3 ALARA Program\" }, { document = \"nureg-1520-rev2\", section = \"4.4.2\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"08-radiation-protection/radiation-protection/radiation-sources\"\ntitle = \"Radiation sources\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"12.2\" }, { document = \"nureg-1537-part1\", section = \"11.1.1\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"08-radiation-protection/radiation-protection/shielding\"\ntitle = \"Radiation protection design features and biological shielding\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"12.3-12.4\" }, { document = \"nureg-1537-part1\", section = \"4.4 Biological Shield\" }]\ncross_links = [\"02-nuclear-safety/nuclear-design/neutron-transport\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"08-radiation-protection/radiation-protection/monitoring-exposure-control-and-dosimetry\"\ntitle = \"Radiation monitoring, exposure control and dosimetry\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"11.1.4; 11.1.5; 11.1.6\" }, { document = \"nureg-1520-rev2\", section = \"4.4.7\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"08-radiation-protection/radiation-protection/operational-radiation-protection-program\"\ntitle = \"Operational radiation protection program\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"12.5\" }, { document = \"nureg-1537-part1\", section = \"11.1.2\" }, { document = \"10cfr53\", section = \"\u{a7} 53.850 Radiation protection, (a)\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 09-electrical-grid / electric-power   (SRP Ch. 8)\n# ============================================================================\n\n[[concept]]\npath = \"09-electrical-grid/electric-power/offsite-power-system\"\ntitle = \"Offsite power system (incl. stability of offsite power systems)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"8.2; BTP 8-3\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"09-electrical-grid/electric-power/onsite-power-systems\"\ntitle = \"Onsite AC and DC power systems; emergency electrical power\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"8.3.1; 8.3.2\" }, { document = \"nureg-1537-part1\", section = \"8.2 Emergency Electrical Power Systems\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"09-electrical-grid/electric-power/station-blackout\"\ntitle = \"Station blackout\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"8.4\" }, { document = \"10cfr50\", section = \"\u{a7} 50.63 Loss of all alternating current power\" }]\ncross_links = [\"02-nuclear-safety/accident-analysis/decrease-in-heat-removal\"]\nstatus = \"approved\"\n\n# ============================================================================\n# 10-human-resource-development / knowledge-management-and-education\n# ============================================================================\n\n[[concept]]\npath = \"10-human-resource-development/knowledge-management-and-education/knowledge-management\"\ntitle = \"Nuclear knowledge management (literature corpus, concept map)\"\norigin = \"outram-park\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.10\" }]\nwhy = \"kovan; the L2 node\'s only source is the IAEA issue itself, which names no sub-concepts.\"\nnote = \"The NRC guides have no knowledge-management chapter, so this sits on the IAEA text (private tier, cited by section).\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"10-human-resource-development/knowledge-management-and-education/education-and-outreach\"\ntitle = \"Education and outreach (lessons, tutorials, interactive demos)\"\norigin = \"outram-park\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.10\" }]\nwhy = \"the maintainer\'s scope note for issue 10: outreach and education; dhoby-ghaut\'s web-demo framework and workbench.\"\nstatus = \"approved\"\n\n# ============================================================================\n# 12-site-and-supporting-facilities / site-characteristics\n# ============================================================================\n\n[[concept]]\npath = \"12-site-and-supporting-facilities/site-characteristics/geography-and-demography\"\ntitle = \"Geography and demography (site location, exclusion area, population distribution)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"2.1.1-2.1.3\" }, { document = \"nureg-1537-part1\", section = \"2.1\" }, { document = \"10cfr53\", section = \"\u{a7} 53.530 Population-related considerations\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"12-site-and-supporting-facilities/site-characteristics/nearby-industrial-transportation-and-military-facilities\"\ntitle = \"Nearby industrial, transportation, and military facilities\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"2.2.1-2.2.3\" }, { document = \"nureg-1537-part1\", section = \"2.2\" }, { document = \"10cfr53\", section = \"\u{a7} 53.510 External hazards\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"12-site-and-supporting-facilities/site-characteristics/hydrologic-engineering\"\ntitle = \"Hydrology (floods, groundwater, accidental releases of radioactive liquid effluents)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"2.4.1-2.4.14 (2.4.12 Groundwater; 2.4.13 Accidental Releases of Radioactive Liquid Effluents in Ground and Surface Waters)\" }, { document = \"nureg-1537-part1\", section = \"2.4\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"12-site-and-supporting-facilities/site-characteristics/geology-seismology-and-geotechnical-engineering\"\ntitle = \"Geology, seismology, and geotechnical engineering\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"2.5.1-2.5.5\" }, { document = \"nureg-1537-part1\", section = \"2.5\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 13-environmental-protection\n# ============================================================================\n\n[[concept]]\npath = \"13-environmental-protection/meteorology-and-air-quality/climatology-and-meteorology\"\ntitle = \"Regional climatology and local meteorology\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"2.3.1; 2.3.2\" }, { document = \"nureg-1537-part1\", section = \"2.3.1; 2.3.2\" }, { document = \"nureg-1555\", section = \"2.7\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"13-environmental-protection/meteorology-and-air-quality/onsite-meteorological-measurements\"\ntitle = \"Onsite meteorological measurements programs\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"2.3.3\" }, { document = \"nureg-1555\", section = \"6.4 Meteorological Monitoring\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"13-environmental-protection/meteorology-and-air-quality/short-term-accident-dispersion\"\ntitle = \"Short term atmospheric dispersion estimates for accident releases\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"2.3.4\" }]\ncross_links = [\"14-emergency-planning/accident-assessment\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"13-environmental-protection/meteorology-and-air-quality/long-term-routine-dispersion\"\ntitle = \"Long-term atmospheric dispersion estimates for routine releases\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"2.3.5\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"13-environmental-protection/meteorology-and-air-quality/relative-concentration-and-deposition\"\ntitle = \"Relative concentration (chi/Q) and relative deposition (D/Q), incl. wet deposition and decay in transit\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1555\", section = \"2.7, III. Review Procedures, item (5)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"13-environmental-protection/radiological-impacts-of-normal-operation/exposure-pathways\"\ntitle = \"Exposure pathways\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1555\", section = \"5.4.1\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"13-environmental-protection/radiological-impacts-of-normal-operation/doses-to-members-of-the-public\"\ntitle = \"Radiation doses to members of the public\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1555\", section = \"5.4.2; 5.4.3\" }, { document = \"10cfr53\", section = \"\u{a7} 53.260 Normal operations\" }, { document = \"10cfr53\", section = \"\u{a7} 53.425 Design features and functional design criteria for normal operations\" }, { document = \"10cfr53\", section = \"\u{a7} 53.450 Analysis requirements, (g)(3) Dose to members of the public\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1645 Reports of radiation exposure to members of the public\" }, { document = \"10cfr50\", section = \"Appendix I, Numerical Guides for Design Objectives and Limiting Conditions for Operation To Meet the Criterion \\\"As Low as is Reasonably Achievable\\\" for Radioactive Material in Light-Water-Cooled Nuclear Power Reactor Effluents\" }]\ncross_links = [\"08-radiation-protection/radiation-protection\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"13-environmental-protection/radiological-impacts-of-normal-operation/impacts-to-biota\"\ntitle = \"Impacts to biota other than members of the public\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1555\", section = \"5.4.4\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"13-environmental-protection/environmental-monitoring/radiological-monitoring\"\ntitle = \"Radiological monitoring\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1555\", section = \"6.2\" }, { document = \"nureg-1537-part1\", section = \"11.1.7 Environmental Monitoring\" }, { document = \"10cfr53\", section = \"\u{a7} 53.850 Radiation protection, (b) (radiological environmental monitoring program)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"13-environmental-protection/postulated-accident-impacts/design-basis-accident-consequences\"\ntitle = \"Environmental consequences of design basis accidents\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1555\", section = \"7.1\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"13-environmental-protection/postulated-accident-impacts/severe-accident-consequences\"\ntitle = \"Severe accidents and severe accident mitigation alternatives\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1555\", section = \"7.2; 7.3\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"13-environmental-protection/postulated-accident-impacts/transportation-accidents\"\ntitle = \"Transportation accidents\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1555\", section = \"7.4\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 14-emergency-planning / accident-assessment   (NUREG-0654 Rev. 2, Planning standard I)\n# World codes only; outram-park crates link here by \'aspiration\', never as leaves (#724).\n# ============================================================================\n\n[[concept]]\npath = \"14-emergency-planning/accident-assessment/release-magnitude-and-isotopic-composition\"\ntitle = \"Magnitude and isotopic composition of an ongoing or potential release\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"I.1.a; I.4\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"14-emergency-planning/accident-assessment/airborne-radiological-assessment-model\"\ntitle = \"Radiological assessment model for airborne releases (dispersion model)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"I.1.b\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"14-emergency-planning/accident-assessment/field-monitoring-and-plume-tracking\"\ntitle = \"Field monitoring teams, plume location and tracking\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"I.1.c; I.5; I.7; I.9\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"14-emergency-planning/accident-assessment/dose-projection\"\ntitle = \"Dose projection and its validation with field data\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"I.8; I.10\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"14-emergency-planning/accident-assessment/drinking-water-contamination\"\ntitle = \"Contamination of drinking water through liquid release or deposition\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"I.2\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 16-nuclear-fuel-cycle\n# ============================================================================\n\n[[concept]]\npath = \"16-nuclear-fuel-cycle/nuclear-criticality-safety/subcriticality-and-double-contingency\"\ntitle = \"Subcriticality and double-contingency principle\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"5.4.3.1.4\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"16-nuclear-fuel-cycle/nuclear-criticality-safety/calculational-method-validation\"\ntitle = \"Calculational method validation (criticality code validation, margin of subcriticality)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"5.4.3.1.7.1; Appendix 5-B (margin of subcriticality)\" }]\ncross_links = [\"02-nuclear-safety/nuclear-design/neutron-transport\", \"07-regulatory-framework/quality-assurance/software-quality-assurance/verification-and-validation-records\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"16-nuclear-fuel-cycle/nuclear-criticality-safety/criticality-safety-evaluations\"\ntitle = \"Criticality safety evaluations and controlled parameters\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"5.4.3.1.7.2; 5.4.3.1.7.3\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"16-nuclear-fuel-cycle/nuclear-criticality-safety/criticality-accident-alarm-system\"\ntitle = \"Criticality accident alarm system\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"5.4.3.1.2\" }, { document = \"10cfr50\", section = \"\u{a7} 50.68 Criticality accident requirements\" }, { document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (m)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"16-nuclear-fuel-cycle/integrated-safety-analysis/isa-methodology-and-summary\"\ntitle = \"Integrated safety analysis: methodology, documentation and summary\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"3.3.1; 3.3.2; 5.4.3.2.1-5.4.3.2.3\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"18-industrial-involvement/process-heat-and-industrial-applications/chemical-process-simulation\"\ntitle = \"Chemical process simulation (flowsheets, reactors, separation columns)\"\norigin = \"outram-park\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.18\" }, { document = \"nureg-1520-rev2\", section = \"Ch. 6 Chemical Process Safety\" }]\nwhy = \"the DWSIM port (chemical processes; process heat applications), maintainer 2026-10-06.\"\nnote = \"Provisional (maintainer, 2026-10-06): under 18 industrial involvement, cross-linked to fuel-cycle chemical process safety.\"\ncross_links = [\"16-nuclear-fuel-cycle/chemical-process-safety\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"16-nuclear-fuel-cycle/fuel-depletion/depletion-solvers\"\ntitle = \"Depletion solvers (Bateman equations, matrix exponential, Monte Carlo transmutation)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.1 (changes in core reactivity with fuel burnup, plutonium buildup, and poisons)\" }, { document = \"iaea-ng-g-3.1-rev1\", section = \"3.16\" }]\nwhy = \"outram-mc\'s burnup loop, ONIX\'s CRAM and boon-lay\'s Lagrangian transmutation.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"16-nuclear-fuel-cycle/fuel-depletion/decay-data\"\ntitle = \"Radioactive decay data and decay chains\"\norigin = \"outram-park\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.16\" }]\nwhy = \"boon-lay\'s nuclide decay library feeds depletion and source terms.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"16-nuclear-fuel-cycle/fuel-depletion/irradiation-history\"\ntitle = \"Burnup and fast-fluence accumulation (irradiation history)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"II.3.C.i (burnup distribution in the fuel)\" }]\nwhy = \"fuel-performance bookkeeping that feeds the fuel-system models.\"\nstatus = \"approved\"\n\n# ============================================================================\n# 17-radioactive-waste-management\n# ============================================================================\n\n[[concept]]\npath = \"17-radioactive-waste-management/radioactive-waste-management/liquid-waste-management\"\ntitle = \"Liquid waste management system\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"11.2\" }, { document = \"ornl-tm-2018-976\", section = \"3.5.2\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"17-radioactive-waste-management/radioactive-waste-management/gaseous-waste-management\"\ntitle = \"Gaseous waste management system (incl. MSR off-gas)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"11.3\" }, { document = \"ornl-tm-2018-976\", section = \"3.5.3\" }]\ncross_links = [\"13-environmental-protection/meteorology-and-air-quality/long-term-routine-dispersion\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"17-radioactive-waste-management/radioactive-waste-management/solid-waste-management\"\ntitle = \"Solid waste management system\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"11.4\" }, { document = \"ornl-tm-2018-976\", section = \"3.5.4\" }, { document = \"10cfr53\", section = \"\u{a7} 53.850 Radiation protection, (c) (Process Control Program)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"17-radioactive-waste-management/radioactive-waste-management/effluent-monitoring-and-release\"\ntitle = \"Process and effluent radiological monitoring; release of radioactive waste\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"11.5\" }, { document = \"nureg-1537-part1\", section = \"11.2.3 Release of Radioactive Waste\" }, { document = \"10cfr53\", section = \"\u{a7} 53.850 Radiation protection, (b) (Offsite Dose Calculations Manual)\" }, { document = \"10cfr50\", section = \"\u{a7} 50.36a Technical specifications on effluents from nuclear power reactors\" }]\ncross_links = [\"13-environmental-protection/environmental-monitoring/radiological-monitoring\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"17-radioactive-waste-management/decommissioning/decommissioning-plan-and-alternatives\"\ntitle = \"Decommissioning plan and alternatives\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"17.1.1-17.1.3\" }, { document = \"10cfr50\", section = \"\u{a7} 50.82 Termination of license\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1070 Termination of license\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1075 Program requirements during decommissioning\" }, { document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (l)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"17-radioactive-waste-management/decommissioning/release-criteria-and-final-survey\"\ntitle = \"Release criteria and final survey\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"17.1.4\" }, { document = \"10cfr50\", section = \"\u{a7} 50.83 Release of part of a power reactor facility or site for unrestricted use\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1080 Release of part of a commercial nuclear plant or site for unrestricted use\" }]\nstatus = \"approved\"\n\n# ############################################################################\n# LEVEL 4 \u{2014} outram-park implementation leaves (seed kovan-concept tags)\n# ############################################################################\n\n# ---- design of SSCs -------------------------------------------------------\n\n[[implementation]]\nconcept = \"02-nuclear-safety/design-of-structures-systems-and-components/mechanical-systems-and-components\"\ncrate = \"farrer-park\"\nmodule = \"assembly\"\ntitle = \"FEM assembly (elasticity and J2 plasticity)\"\nwhat = \"Finite-element assembly for structural mechanics on the shared outram-foam numerical backend.\"\nkind = \"port\"\nupstream = \"MOOSE / PRISMS-Plasticity (LGPL-2.1)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/design-of-structures-systems-and-components/mechanical-systems-and-components\"\ncrate = \"farrer-park\"\nmodule = \"crystal\"\ntitle = \"Crystal plasticity\"\nwhat = \"Plastic flow as crystallographic slip on discrete slip systems.\"\nkind = \"port\"\nupstream = \"PRISMS-Plasticity\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/design-of-structures-systems-and-components/mechanical-systems-and-components\"\ncrate = \"farrer-park\"\nmodule = \"fatigue\"\ntitle = \"Fatigue indicator parameters\"\nwhat = \"Scalar measures that rank microstructural sites by fatigue-crack initiation propensity.\"\nkind = \"port\"\nupstream = \"PRISMS-Fatigue\"\nstatus = \"proposed\"\n\n# ---- fuel system design ---------------------------------------------------\n\n[[implementation]]\nconcept = \"02-nuclear-safety/fuel-system-design/fuel-system-damage/oxidation-hydriding-and-crud\"\ncrate = \"outram-park-fork-offbeat\"\nmodule = \"corrosion\"\ntitle = \"Cladding waterside corrosion and hydrogen pickup\"\nwhat = \"Oxide growth kinetics, hydrogen uptake and solver acceleration for cladding corrosion.\"\nkind = \"port\"\nupstream = \"OFFBEAT\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/fuel-system-design/fuel-system-damage/rod-internal-gas-pressure\"\ncrate = \"outram-park-fork-offbeat\"\nmodule = \"fgr\"\ntitle = \"Fission-gas release\"\nwhat = \"Xenon and krypton release from the fuel and its effect on rod internal pressure.\"\nkind = \"port\"\nupstream = \"OFFBEAT\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/fuel-system-design/fuel-rod-failure/pellet-cladding-interaction\"\ncrate = \"outram-park-fork-offbeat\"\nmodule = \"gap\"\ntitle = \"Fuel/cladding gap: gas, conductance and contact\"\nwhat = \"Gap gas composition, gap conductance and pellet-cladding contact by axial slice.\"\nkind = \"port\"\nupstream = \"OFFBEAT\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/fuel-system-design/fuel-system-damage\"\ncrate = \"outram-park-fork-offbeat\"\nmodule = \"mechanics\"\ntitle = \"Fuel-performance solid mechanics\"\nwhat = \"The displacement solve for fuel and cladding stress and strain.\"\nkind = \"port\"\nupstream = \"OFFBEAT\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/fuel-system-design/fuel-system-damage\"\ncrate = \"outram-park-fork-offbeat\"\nmodule = \"rheology\"\ntitle = \"Fuel and cladding constitutive laws\"\nwhat = \"Creep, yield and material-specific rheology laws.\"\nkind = \"port\"\nupstream = \"OFFBEAT (code_aster behaviour laws)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/fuel-system-design/fuel-salt-chemistry\"\ncrate = \"outram-park-fork-thermochimica\"\nmodule = \"gem\"\ntitle = \"CALPHAD Gibbs-energy minimisation\"\nwhat = \"Molten-salt equilibrium thermochemistry: fission-product speciation, redox and solubility.\"\nkind = \"port\"\nupstream = \"ORNL Thermochimica (BSD-3)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-failure\"\ncrate = \"boon-lay\"\nmodule = \"fuel_failure\"\ntitle = \"boon-lay fuel failure (PANAMA-I equations)\"\nwhat = \"TRISO pressure-vessel failure fractions under accident conditions, from the published PANAMA-I equations.\"\nkind = \"new-work\"\nupstream = \"Verfondern & Nabielek, PANAMA-I mathematical basis (equations only; no code)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release\"\ncrate = \"boon-lay\"\nmodule = \"triso_atops_fork\"\ntitle = \"TRISO-ATOPS fork: continuum fission-product release\"\nwhat = \"Eulerian diffusion and release of fission products through TRISO layers, normal operation and accident.\"\nkind = \"port\"\nupstream = \"INL TRISO-ATOPS\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release\"\ncrate = \"boon-lay\"\nmodule = \"triso_atops_extensions\"\ntitle = \"TRISO-ATOPS extensions\"\nwhat = \"Questions upstream\'s model cannot express, kept apart from the faithful port.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-conduction\"\ncrate = \"tampines\"\nmodule = \"pebble_bed/triso\"\ntitle = \"TRISO coated-particle conduction\"\nwhat = \"Steady radial conduction through the five concentric TRISO regions.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/fuel-system-design/fuel-performance-analytical-predictions/fuel-temperatures-and-stored-energy\"\ncrate = \"teh-o-prke\"\nmodule = \"fuel_temperature_feedback\"\ntitle = \"Lumped fuel temperature model\"\nwhat = \"Fuel temperature from power and gap conductance, feeding Doppler feedback.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n# ---- nuclear design -------------------------------------------------------\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/core-configuration-and-criticality\"\ncrate = \"nee_soon\"\nmodule = \"htr10_rmc\"\ntitle = \"HTR-10 core model, code-to-code against RMC\"\nwhat = \"HTR-10 first-criticality core built for outram-mc and verified against the published RMC benchmark.\"\nkind = \"new-work\"\nupstream = \"Li, Yu & Wei (2014), HTR-10 benchmark with RMC\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/core-configuration-and-criticality\"\ncrate = \"nee_soon\"\nmodule = \"det_six_factor\"\ntitle = \"Six-factor decomposition of a deterministic solve\"\nwhat = \"Breaks a diffusion/SP3 k_eff into the six-factor formula for comparison with Monte Carlo.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/core-configurations/pebble-bed/pebble-bed-packing\"\ncrate = \"outram-park-fork-liggghts\"\nmodule = \"granular\"\ntitle = \"Granular contact pipeline (pair_style gran)\"\nwhat = \"Surface, normal and tangential contact models for settling a pebble bed.\"\nkind = \"port\"\nupstream = \"LIGGGHTS (logic translated; see crate NOTICE on GPL-2)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/core-configurations/pebble-bed/pebble-bed-packing\"\ncrate = \"outram-park-fork-liggghts\"\nmodule = \"integrator\"\ntitle = \"Velocity-Verlet integration for spheres\"\nwhat = \"Kick-drift-kick propagation of DEM spheres (nve/sphere).\"\nkind = \"port\"\nupstream = \"LIGGGHTS FixNVESphere\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/core-configurations/pebble-bed/pebble-bed-packing\"\ncrate = \"outram-park-fork-liggghts\"\nmodule = \"htr10_fill\"\ntitle = \"HTR-10 pebble pour\"\nwhat = \"Pours and settles the HTR-10 pebble bed under gravity, steppable from a UI.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/core-configurations/pebble-bed/pebble-bed-packing\"\ncrate = \"outram-mc-libs\"\nmodule = \"pebble_beds/crp_packing\"\ntitle = \"Close random packing of pebbles\"\nwhat = \"Random sphere packing for pebble positions.\"\nkind = \"port\"\nupstream = \"OpenMC openmc.model.pack_spheres\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/core-configurations/pebble-bed/pebble-bed-packing\"\ncrate = \"outram-mc-libs\"\nmodule = \"pebble_beds/dem_bed\"\ntitle = \"DEM-settled pebble beds for transport\"\nwhat = \"Hands a bed settled by the LIGGGHTS fork to Monte Carlo transport.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/power-distribution\"\ncrate = \"bedok\"\nmodule = \"calc_relpower3d\"\ntitle = \"3-D relative power distribution\"\nwhat = \"Relative nodal power from the BEDOK diffusion solution.\"\nkind = \"port\"\nupstream = \"BEDOK MATLAB (Than Yan Ren, SNRSI)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/reactivity-coefficients\"\ncrate = \"outram-foam-appbuilder-lib\"\nmodule = \"genfoam/thermo_mechanics\"\ntitle = \"Thermal-expansion feedback\"\nwhat = \"Linear-elastic thermal expansion feeding geometry and density changes back to neutronics.\"\nkind = \"port\"\nupstream = \"GeN-Foam thermoMechanics\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/reactivity-coefficients\"\ncrate = \"outram-foam-appbuilder-lib\"\nmodule = \"genfoam/multi_region/reactivity_feedback\"\ntitle = \"Reactivity feedback across regions\"\nwhat = \"Feedback coefficients applied between coupled neutronics and TH meshes.\"\nkind = \"port\"\nupstream = \"GeN-Foam multiRegion\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/kinetics-parameters\"\ncrate = \"outram-mc-libs\"\nmodule = \"physics/kinetics\"\ntitle = \"beta_eff and generation time from Monte Carlo\"\nwhat = \"Point-kinetics parameters from a Monte Carlo run, feeding teh-o-prke.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/kinetics-parameters\"\ncrate = \"outram-mc-libs\"\nmodule = \"physics/ifp\"\ntitle = \"Iterated fission probability\"\nwhat = \"Adjoint-weighted beta_eff and Lambda.\"\nkind = \"port\"\nupstream = \"OpenMC src/ifp.cpp (afa7a14)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/reactor-kinetic-behaviour\"\ncrate = \"teh-o-prke\"\nmodule = \"zero_power_prke\"\ntitle = \"Six-group point reactor kinetics\"\nwhat = \"Zero-power point-kinetics equations with six precursor groups (explicit and implicit solvers).\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/reactor-kinetic-behaviour\"\ncrate = \"teh-o-prke\"\nmodule = \"delayed_neutron_layer\"\ntitle = \"Reusable delayed-neutron layer\"\nwhat = \"Backward-Euler precursor bank, O(1) per step.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/reactor-kinetic-behaviour\"\ncrate = \"outram-foam-appbuilder-lib\"\nmodule = \"genfoam/neutronics/point_kinetics\"\ntitle = \"GeN-Foam point kinetics\"\nwhat = \"0-D reactor kinetics with precursor groups inside the multiphysics solver.\"\nkind = \"port\"\nupstream = \"GeN-Foam pointKinetics\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/reactivity-worths\"\ncrate = \"teh-o-prke\"\nmodule = \"control_rod_feedback\"\ntitle = \"Control-rod reactivity\"\nwhat = \"Rod-position-dependent reactivity insertion for point kinetics.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/reactivity-worths\"\ncrate = \"nee_soon\"\nmodule = \"rod_insertion\"\ntitle = \"Smeared control-rod absorber in a multigroup library\"\nwhat = \"Applies rod atom densities to a multigroup set for rod-worth studies.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/reactivity-worths\"\ncrate = \"nee_soon\"\nmodule = \"htr10_rmc/control_rod\"\ntitle = \"HTR-10 control rods\"\nwhat = \"Rod composition and geometry for HTR-10 worth calculations.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/burnup-and-poison-reactivity-effects\"\ncrate = \"teh-o-prke\"\nmodule = \"feedback_mechanisms/fission_product_poisons\"\ntitle = \"Fission-product poisons (xenon, samarium)\"\nwhat = \"Poison build-up and decay and its reactivity effect.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/excess-reactivity-and-shutdown-margin\"\ncrate = \"bedok\"\nmodule = \"criticalboron_xyz\"\ntitle = \"Critical boron search\"\nwhat = \"Searches the soluble-boron concentration that makes the core critical.\"\nkind = \"port\"\nupstream = \"BEDOK MATLAB\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/accuracy-of-analytical-methods\"\ncrate = \"outram-mc-libs\"\nmodule = \"stats/uq\"\ntitle = \"Uncertainty quantification by input sampling\"\nwhat = \"Propagates input uncertainties (densities, packing) to k_eff and tallies.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/accuracy-of-analytical-methods\"\ncrate = \"outram-mc-libs\"\nmodule = \"tally/derivative\"\ntitle = \"Tally derivatives\"\nwhat = \"Sensitivity coefficients from a single run.\"\nkind = \"port\"\nupstream = \"OpenMC src/tallies/derivative.cpp (afa7a14)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/core-configurations/liquid-fuelled/delayed-neutron-precursor-drift\"\ncrate = \"outram-park-fork-moltres\"\nmodule = \"precursors\"\ntitle = \"Precursor advection-decay transport\"\nwhat = \"Delayed-neutron precursors carried by the moving fuel salt.\"\nkind = \"port\"\nupstream = \"Moltres (formulation; LGPL)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/core-configurations/liquid-fuelled/delayed-neutron-precursor-drift\"\ncrate = \"outram-park-fork-moltres\"\nmodule = \"circulating\"\ntitle = \"Circulating-fuel k-eigenvalue\"\nwhat = \"Multigroup diffusion coupled to advected precursors: the MSRE reactivity loss.\"\nkind = \"port\"\nupstream = \"Moltres\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/core-configurations/liquid-fuelled/delayed-neutron-precursor-drift\"\ncrate = \"outram-foam-appbuilder-lib\"\nmodule = \"genfoam/neutronics/diffusion/precursor_drift\"\ntitle = \"GeN-Foam precursor drift\"\nwhat = \"Precursor transport with the fuel flow inside the diffusion solver.\"\nkind = \"port\"\nupstream = \"GeN-Foam\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/surface-tracking\"\ncrate = \"outram-blender\"\nmodule = \"csg\"\ntitle = \"CSG description and navigation kernel\"\nwhat = \"Surfaces, cells, universes and lattices and the pure distance-to-boundary kernel (moved from outram-mc, #486).\"\nkind = \"port\"\nupstream = \"OpenMC surface.cpp / cell.cpp / geometry.cpp\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/surface-tracking\"\ncrate = \"outram-mc-libs\"\nmodule = \"geometry/crossing\"\ntitle = \"Surface crossing on the transport state\"\nwhat = \"The crossing work that needs outram-mc\'s RNG, kinematics and materials, as extension traits on blender\'s CSG.\"\nkind = \"port\"\nupstream = \"OpenMC geometry.cpp\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/surface-tracking\"\ncrate = \"outram-mc-libs\"\nmodule = \"physics/transport_csg\"\ntitle = \"Power iteration over general CSG\"\nwhat = \"k-eigenvalue transport with surface tracking and boundary conditions on any CSG geometry.\"\nkind = \"port\"\nupstream = \"OpenMC eigenvalue/transport loop\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/delta-tracking\"\ncrate = \"outram-mc-libs\"\nmodule = \"physics/delta_tracking\"\ntitle = \"Delta (Woodcock) tracking\"\nwhat = \"Moves a neutron by rejection against a majorant instead of by surface search.\"\nkind = \"new-work\"\nupstream = \"Woodcock et al. (1965) method; OpenMC has no delta tracking\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/delta-tracking\"\ncrate = \"outram-mc-libs\"\nmodule = \"pebble_beds/delta_tracking\"\ntitle = \"Delta tracking in pebble beds\"\nwhat = \"Delta tracking specialised to doubly heterogeneous beds.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/delta-tracking/majorant\"\ncrate = \"outram-mc-libs\"\nmodule = \"physics/delta_tracking/majorant\"\ntitle = \"Majorant cross section\"\nwhat = \"Builds and guards the energy-dependent bound every delta-tracking flight samples on.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/doubly-heterogeneous-geometry\"\ncrate = \"outram-mc-libs\"\nmodule = \"dh_universe\"\ntitle = \"Doubly heterogeneous universes\"\nwhat = \"One enum to choose how TRISO particles are resolved (explicit, RPT, stochastic).\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/doubly-heterogeneous-geometry\"\ncrate = \"outram-mc-libs\"\nmodule = \"stochastic\"\ntitle = \"Stochastic-media transport (CLS, SCLS)\"\nwhat = \"Random geometry sampled during transport rather than stored.\"\nkind = \"new-work\"\nupstream = \"chord-length sampling literature; absent from OpenMC\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/doubly-heterogeneous-geometry\"\ncrate = \"outram-mc-libs\"\nmodule = \"pebble_beds\"\ntitle = \"Pebble-bed specialisation\"\nwhat = \"TRISO-in-pebble-in-bed transport slice for HTR-10 and FHR pebbles.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/doubly-heterogeneous-geometry\"\ncrate = \"outram-blender\"\nmodule = \"csg/triso_particle\"\ntitle = \"Five-shell TRISO particle builder\"\nwhat = \"CSG construction of a TRISO particle.\"\nkind = \"port\"\nupstream = \"OpenMC openmc.model.TRISO\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/continuous-energy-collision-physics\"\ncrate = \"outram-mc-libs\"\nmodule = \"physics/scatter\"\ntitle = \"Elastic and inelastic scattering kinematics\"\nwhat = \"Samples outgoing energy and angle from the evaluation\'s own laws.\"\nkind = \"port\"\nupstream = \"OpenMC physics.cpp, physics_common.cpp\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/continuous-energy-collision-physics\"\ncrate = \"outram-mc-libs\"\nmodule = \"physics/fission\"\ntitle = \"Fission neutron production\"\nwhat = \"Banks fission sites for the next generation.\"\nkind = \"port\"\nupstream = \"OpenMC physics.cpp fission()\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/eigenvalue-and-fixed-source-calculations\"\ncrate = \"outram-mc-libs\"\nmodule = \"physics/keff\"\ntitle = \"k-eigenvalue power iteration (bare sphere driver)\"\nwhat = \"Minimal criticality driver with generation statistics.\"\nkind = \"port\"\nupstream = \"OpenMC eigenvalue.cpp\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/eigenvalue-and-fixed-source-calculations\"\ncrate = \"outram-mc-libs\"\nmodule = \"physics/fixed_source\"\ntitle = \"Fixed-source mode\"\nwhat = \"Source-driven transport without eigenvalue iteration.\"\nkind = \"port\"\nupstream = \"OpenMC\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/variance-reduction\"\ncrate = \"outram-mc-libs\"\nmodule = \"physics/weight_windows\"\ntitle = \"Mesh weight windows and MAGIC\"\nwhat = \"Splitting, roulette and weight-window generation from a flux tally.\"\nkind = \"port\"\nupstream = \"OpenMC src/weight_windows.cpp (afa7a14)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/variance-reduction\"\ncrate = \"outram-mc-libs\"\nmodule = \"physics/ufs\"\ntitle = \"Uniform fission site weighting\"\nwhat = \"Flattens fission-site density for better local tally statistics.\"\nkind = \"port\"\nupstream = \"OpenMC src/eigenvalue.cpp ufs_* (afa7a14)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/monte-carlo-tallies\"\ncrate = \"outram-mc-libs\"\nmodule = \"tally\"\ntitle = \"Tallies, filters and triggers\"\nwhat = \"Scoring of fluxes and reaction rates with filters, arithmetic and triggers.\"\nkind = \"port\"\nupstream = \"OpenMC src/tallies/\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/monte-carlo-tallies\"\ncrate = \"outram-mc-libs\"\nmodule = \"tally/mesh_unstructured\"\ntitle = \"Unstructured-mesh tally scoring\"\nwhat = \"Track-length scoring on blender\'s neutral unstructured mesh (#492).\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/deterministic-neutronics\"\ncrate = \"outram-foam-appbuilder-lib\"\nmodule = \"genfoam/neutronics/diffusion\"\ntitle = \"Multigroup neutron diffusion\"\nwhat = \"Eigenvalue and transient multigroup diffusion on an FV mesh.\"\nkind = \"port\"\nupstream = \"GeN-Foam diffusionNeutronics\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/deterministic-neutronics\"\ncrate = \"outram-foam-appbuilder-lib\"\nmodule = \"genfoam/neutronics/sp3\"\ntitle = \"Simplified P3 transport\"\nwhat = \"SP3 neutronics on the diffusion machinery.\"\nkind = \"port\"\nupstream = \"GeN-Foam SP3Neutronics\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/deterministic-neutronics\"\ncrate = \"outram-foam-appbuilder-lib\"\nmodule = \"genfoam/neutronics/sn\"\ntitle = \"Discrete ordinates (Sn)\"\nwhat = \"Angular quadrature and sweeps for Sn eigenvalue problems.\"\nkind = \"port\"\nupstream = \"GeN-Foam\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/deterministic-neutronics\"\ncrate = \"outram-park-fork-moltres\"\nmodule = \"diffusion\"\ntitle = \"Static-fuel multigroup diffusion\"\nwhat = \"k-eigenvalue multigroup diffusion on an FvMesh.\"\nkind = \"port\"\nupstream = \"Moltres (formulation)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/deterministic-neutronics\"\ncrate = \"bedok\"\nmodule = \"sanodaldiffusion_solverxyz\"\ntitle = \"3-D semi-analytic nodal diffusion\"\nwhat = \"Nodal diffusion solver of the BEDOK simulator.\"\nkind = \"port\"\nupstream = \"BEDOK MATLAB (Than & Xiao 2026)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/multigroup-cross-section-condensation\"\ncrate = \"nee_soon\"\nmodule = \"mgxs\"\ntitle = \"MGXS condensed from Monte Carlo\"\nwhat = \"Flux-weighted group constants tallied by outram-mc.\"\nkind = \"new-work\"\nupstream = \"OpenMC openmc.mgxs (concept)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/multigroup-cross-section-condensation\"\ncrate = \"nee_soon\"\nmodule = \"genfoam_xs\"\ntitle = \"MGXS handed to GeN-Foam\"\nwhat = \"Turns condensed group constants into GeN-Foam\'s nuclear-data input.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"endf\"\ntitle = \"ENDF tape model\"\nwhat = \"Parses and represents ENDF-6 tapes, the substrate of every NJOY module.\"\nkind = \"port\"\nupstream = \"NJOY2016 endf.f90\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"nuclear_data\"\ntitle = \"Nuclear-data provider surface\"\nwhat = \"What the transport crates pull from njoy (all nuclear data lives here).\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/resonance-reconstruction\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"reconr\"\ntitle = \"RECONR\"\nwhat = \"Reconstructs pointwise cross sections from resonance parameters onto a PENDF tape.\"\nkind = \"port\"\nupstream = \"NJOY2016 reconr.f90\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/resonance-reconstruction\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"samm\"\ntitle = \"SAMM R-matrix kernel\"\nwhat = \"Reich-Moore and R-matrix-limited (LRF=7) cross sections.\"\nkind = \"port\"\nupstream = \"NJOY2016 samm.f90\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/doppler-broadening\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"broadr\"\ntitle = \"BROADR (SIGMA1)\"\nwhat = \"Doppler broadening of pointwise cross sections.\"\nkind = \"port\"\nupstream = \"NJOY2016 broadr.f90\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/doppler-broadening\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"wmp\"\ntitle = \"Windowed multipole\"\nwhat = \"Analytic on-the-fly Doppler broadening from multipole data.\"\nkind = \"port\"\nupstream = \"MIT CRPG windowed multipole (WMP_Library, MIT licence)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/unresolved-resonance-probability-tables\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"purr\"\ntitle = \"PURR probability tables\"\nwhat = \"URR probability tables for Monte Carlo self-shielding.\"\nkind = \"port\"\nupstream = \"NJOY2016 purr.f90\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/unresolved-resonance-probability-tables\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"unresr\"\ntitle = \"UNRESR\"\nwhat = \"Bondarenko self-shielded cross sections in the unresolved range.\"\nkind = \"port\"\nupstream = \"NJOY2016 unresr.f90\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/thermal-scattering\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"thermr\"\ntitle = \"THERMR\"\nwhat = \"Bound-atom thermal cross sections and secondary distributions from MF=7.\"\nkind = \"port\"\nupstream = \"NJOY2016 thermr.f90\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/thermal-scattering\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"leapr\"\ntitle = \"LEAPR\"\nwhat = \"Generates S(alpha, beta) from a phonon model.\"\nkind = \"port\"\nupstream = \"NJOY2016 leapr.f90\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/thermal-scattering\"\ncrate = \"outram-mc-libs\"\nmodule = \"material/thermal\"\ntitle = \"S(alpha, beta) in transport\"\nwhat = \"Samples bound-atom scattering from thermal tables during transport.\"\nkind = \"port\"\nupstream = \"OpenMC src/thermal.cpp\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/ace-library-generation\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"acer\"\ntitle = \"ACER\"\nwhat = \"Assembles and writes continuous-energy ACE libraries.\"\nkind = \"port\"\nupstream = \"NJOY2016 acefc.f90\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/multigroup-data-generation\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"groupr\"\ntitle = \"GROUPR\"\nwhat = \"Self-shielded multigroup cross sections and transfer matrices.\"\nkind = \"port\"\nupstream = \"NJOY2016 groupr.f90\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/multigroup-data-generation\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"gaminr\"\ntitle = \"GAMINR\"\nwhat = \"Multigroup photoatomic cross sections and matrices.\"\nkind = \"port\"\nupstream = \"NJOY2016 gaminr.f90\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/nuclear-data-covariances\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"errorr\"\ntitle = \"ERRORR\"\nwhat = \"Multigroup covariance matrices from ENDF covariance files.\"\nkind = \"port\"\nupstream = \"NJOY2016 errorr.f90\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/nuclear-data-covariances\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"covr\"\ntitle = \"COVR\"\nwhat = \"Correlation matrices and covariance reports from ERRORR output.\"\nkind = \"port\"\nupstream = \"NJOY2016 covr.f90\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/heating-and-damage\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"heatr\"\ntitle = \"HEATR\"\nwhat = \"KERMA heating and damage cross sections.\"\nkind = \"port\"\nupstream = \"NJOY2016 heatr.f90\"\nstatus = \"proposed\"\n\n# ---- moderator and reflector ----------------------------------------------\n\n[[implementation]]\nconcept = \"02-nuclear-safety/moderator-and-reflector/moderator-and-reflector-nuclear-design\"\ncrate = \"nee_soon\"\nmodule = \"htr10_rmc/reflector\"\ntitle = \"HTR-10 graphite reflector\"\nwhat = \"Reflector materials and channels of the HTR-10 model.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/moderator-and-reflector/moderator-and-reflector-nuclear-design\"\ncrate = \"nee_soon\"\nmodule = \"htr10_rmc/reflector_geometry\"\ntitle = \"HTR-10 reflector geometry\"\nwhat = \"CSG geometry of the HTR-10 side, top and bottom reflectors.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n# ---- thermal-hydraulic design --------------------------------------------\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/core-coolant-hydraulics\"\ncrate = \"tuas_boussinesq_solver\"\nmodule = \"lib/fluid_mechanics_correlations\"\ntitle = \"Friction factor and pipe hydraulics correlations\"\nwhat = \"Churchill friction factor, custom fLDK and pipe pressure-loss calculations.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/heat-transfer-to-coolant\"\ncrate = \"tuas_boussinesq_solver\"\nmodule = \"lib/heat_transfer_correlations\"\ntitle = \"Heat-transfer correlations\"\nwhat = \"Nusselt correlations, thermal resistances, view factors and parallel heat exchangers.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/power-density-and-heat-flux-distribution\"\ncrate = \"bedok\"\nmodule = \"fuelrodheat_1dcylnd\"\ntitle = \"1-D cylindrical fuel-rod heat conduction\"\nwhat = \"Radial fuel-rod temperature from nodal power.\"\nkind = \"port\"\nupstream = \"BEDOK MATLAB\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/fuel-heat-removal-limits\"\ncrate = \"outram-foam-multiphase\"\nmodule = \"chf\"\ntitle = \"Critical heat flux correlations\"\nwhat = \"CHF, DNB and dryout point correlations.\"\nkind = \"new-work\"\nupstream = \"published CHF correlations\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/fuel-heat-removal-limits\"\ncrate = \"outram-foam-multiphase\"\nmodule = \"wall_boiling\"\ntitle = \"Wall-boiling framework\"\nwhat = \"Wall-boiling closure architecture for the two-fluid model.\"\nkind = \"port\"\nupstream = \"OpenFOAM multiphaseEuler wall boiling\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/fuel-heat-removal-limits\"\ncrate = \"bedok\"\nmodule = \"w3chf\"\ntitle = \"W-3 critical heat flux\"\nwhat = \"W-3 CHF correlation for DNBR.\"\nkind = \"port\"\nupstream = \"BEDOK MATLAB\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/natural-convection-cooling\"\ncrate = \"tuas_boussinesq_solver\"\nmodule = \"lib/pre_built_components/ciet_nat_circ_tests\"\ntitle = \"CIET natural-circulation tests\"\nwhat = \"Steady natural-circulation verification suites on the CIET DRACS loop.\"\nkind = \"new-work\"\nupstream = \"Ong, Xiao & Peterson (2025), doi:10.1016/j.jandt.2025.03.006 (peer reviewed)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/natural-convection-cooling\"\ncrate = \"tuas_boussinesq_solver\"\nmodule = \"lib/pre_built_components/uw_madison_flibe_loop_components\"\ntitle = \"UW-Madison FLiBe loop\"\nwhat = \"Pre-built components for a molten-fluoride-salt natural/forced circulation loop.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/shutdown-decay-heat\"\ncrate = \"teh-o-prke\"\nmodule = \"decay_heat\"\ntitle = \"23-group fission-product decay heat\"\nwhat = \"Decay heat after shutdown from the 1978 draft ANS standard.\"\nkind = \"new-work\"\nupstream = \"ANS 5.1 (1978 draft) 23-group fit\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/pulsing-reactor-analysis\"\ncrate = \"teh-o-prke\"\nmodule = \"nordheim_fuchs\"\ntitle = \"Nordheim-Fuchs exact timestepper\"\nwhat = \"Closed-form prompt excursion with adiabatic fuel-temperature feedback.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics\"\ncrate = \"outram-foam-basic-lib\"\nmodule = \"fv_operators\"\ntitle = \"Finite-volume operators (fvm, fvc)\"\nwhat = \"Implicit and explicit FV discretisation operators.\"\nkind = \"port\"\nupstream = \"OpenFOAM finiteVolume\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics\"\ncrate = \"outram-foam-basic-lib\"\nmodule = \"ldu_matrix\"\ntitle = \"LDU matrices and solvers\"\nwhat = \"Face-addressed sparse matrices with PCG and GAMG.\"\nkind = \"port\"\nupstream = \"OpenFOAM lduMatrix\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics\"\ncrate = \"outram-foam-appbuilder-lib\"\nmodule = \"solvers/pimple_foam\"\ntitle = \"pimpleFoam / icoFoam\"\nwhat = \"Incompressible PISO/PIMPLE solver.\"\nkind = \"port\"\nupstream = \"OpenFOAM pimpleFoam\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics\"\ncrate = \"outram-foam-appbuilder-lib\"\nmodule = \"solvers/rho_pimple_foam\"\ntitle = \"rhoPimpleFoam\"\nwhat = \"Compressible transient PIMPLE solver.\"\nkind = \"port\"\nupstream = \"OpenFOAM rhoPimpleFoam\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics\"\ncrate = \"outram-foam-turbulence-lib\"\nmodule = \"k_omega_sst\"\ntitle = \"k-omega SST turbulence model\"\nwhat = \"Menter (1994) RAS model.\"\nkind = \"port\"\nupstream = \"OpenFOAM kOmegaSST\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics\"\ncrate = \"outram-foam-turbulence-lib\"\nmodule = \"wall_functions\"\ntitle = \"Turbulence wall functions\"\nwhat = \"Near-wall treatment for RAS models.\"\nkind = \"port\"\nupstream = \"OpenFOAM wall functions\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics\"\ncrate = \"outram-foam-appbuilder-lib\"\nmodule = \"genfoam/thermal_hydraulics\"\ntitle = \"GeN-Foam reactor thermal hydraulics\"\nwhat = \"Single- and two-phase porous-medium reactor thermal hydraulics.\"\nkind = \"port\"\nupstream = \"GeN-Foam thermalHydraulics\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics/mesh-generation\"\ncrate = \"outram-foam-mesh\"\nmodule = \"block_mesh\"\ntitle = \"blockMesh\"\nwhat = \"Structured hexahedral block meshing from a blockMeshDict.\"\nkind = \"port\"\nupstream = \"OpenFOAM blockMesh\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics/mesh-generation\"\ncrate = \"outram-foam-mesh\"\nmodule = \"snappy_hex_mesh\"\ntitle = \"snappyHexMesh\"\nwhat = \"Split-hex meshing around STL surfaces.\"\nkind = \"port\"\nupstream = \"OpenFOAM snappyHexMesh\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics/mesh-generation\"\ncrate = \"outram-park-fork-cfmesh\"\nmodule = \"pipeline\"\ntitle = \"cfMesh tet-dual pipeline\"\nwhat = \"Tetrahedralisation, polyhedral dual and boundary layers to a volume mesh.\"\nkind = \"port\"\nupstream = \"cfMesh (GPL-3.0)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics/mesh-generation\"\ncrate = \"outram-blender\"\nmodule = \"foam_mesh\"\ntitle = \"Volume-meshing bridge\"\nwhat = \"Blender surface mesh to cfMesh pipeline to OpenFOAM polyMesh.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics/mesh-generation\"\ncrate = \"outram-blender\"\nmodule = \"unstructured\"\ntitle = \"Neutral unstructured mesh\"\nwhat = \"One mesh description shared by the FV, FEM and Monte Carlo solvers (#492).\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/system-thermal-hydraulics\"\ncrate = \"tuas_boussinesq_solver\"\nmodule = \"lib/single_control_vol\"\ntitle = \"Single control-volume node\"\nwhat = \"The lumped thermal node and its node-to-node interactions.\"\nkind = \"new-work\"\nupstream = \"Ong, Xiao & Peterson (2025) (peer reviewed)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/system-thermal-hydraulics\"\ncrate = \"tuas_boussinesq_solver\"\nmodule = \"lib/array_fluid_collections\"\ntitle = \"Array control volumes and fluid networks\"\nwhat = \"Spatially resolved 1-D components and networks of them.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/two-phase-flow\"\ncrate = \"tampines\"\nmodule = \"multiphase_1d\"\ntitle = \"1-D two-phase system-code solvers\"\nwhat = \"Two-fluid and drift-flux 1-D solvers reduced from the 3-D reference.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/two-phase-flow\"\ncrate = \"outram-foam-multiphase\"\nmodule = \"two_fluid\"\ntitle = \"Euler-Euler two-fluid model\"\nwhat = \"Phase and interfacial-momentum-transfer foundation.\"\nkind = \"port\"\nupstream = \"OpenFOAM multiphaseEuler\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/two-phase-flow\"\ncrate = \"outram-foam-appbuilder-lib\"\nmodule = \"solvers/hrm_foam\"\ntitle = \"HRMFoam flashing flow\"\nwhat = \"Homogeneous relaxation model for flashing two-phase flow.\"\nkind = \"port\"\nupstream = \"HRMFoam\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/coolant-thermophysical-properties\"\ncrate = \"tampines-steam-tables\"\nmodule = \"interfaces\"\ntitle = \"IAPWS-IF97 steam tables\"\nwhat = \"Water and steam properties by region, with forward and backward equations.\"\nkind = \"new-work\"\nupstream = \"IAPWS-IF97 formulation\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/coolant-thermophysical-properties\"\ncrate = \"outram-park-fork-coolprop\"\nmodule = \"eos\"\ntitle = \"Helmholtz-energy equations of state\"\nwhat = \"Reduced Helmholtz energy and derivatives for ~120 fluids.\"\nkind = \"port\"\nupstream = \"CoolProp (MIT)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/coolant-thermophysical-properties\"\ncrate = \"outram-park-fork-coolprop\"\nmodule = \"flash\"\ntitle = \"Single-phase flashes\"\nwhat = \"(p,T), (p,h), (p,s) to a full fluid state.\"\nkind = \"port\"\nupstream = \"CoolProp\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/coolant-thermophysical-properties\"\ncrate = \"tuas_boussinesq_solver\"\nmodule = \"lib/boussinesq_thermophysical_properties\"\ntitle = \"Liquid and solid property library\"\nwhat = \"Salt, oil and solid thermophysical properties for Boussinesq models.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/pebble-bed-thermal-hydraulics\"\ncrate = \"tampines\"\nmodule = \"gas_phase/kta_bed\"\ntitle = \"KTA 3102.3 packed-bed pressure drop\"\nwhat = \"Gas pressure gradient through a randomly packed bed of spheres.\"\nkind = \"new-work\"\nupstream = \"KTA 3102.3 correlation\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/pebble-bed-thermal-hydraulics\"\ncrate = \"tampines\"\nmodule = \"pebble_bed/zbs\"\ntitle = \"Zehner-Bauer-Schlunder effective conductivity\"\nwhat = \"Analytic effective thermal conductivity of a packed bed.\"\nkind = \"new-work\"\nupstream = \"ZBS model as in KTA 3102.4\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/pebble-bed-thermal-hydraulics\"\ncrate = \"tampines\"\nmodule = \"pebble_bed/cht\"\ntitle = \"Pebble-bed conjugate heat transfer\"\nwhat = \"Pebble-to-coolant heat transfer in the nested conduction stack.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/pebble-bed-thermal-hydraulics\"\ncrate = \"outram-park-fork-liggghts\"\nmodule = \"thermal\"\ntitle = \"Thermal DEM contact conduction\"\nwhat = \"Particle-particle and particle-wall contact conduction.\"\nkind = \"port\"\nupstream = \"LIGGGHTS heat transfer\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/pebble-bed-thermal-hydraulics\"\ncrate = \"outram-park-fork-liggghts\"\nmodule = \"thermal_radiation\"\ntitle = \"Radiative and gas-gap heat transfer between particles\"\nwhat = \"Particle-scale radiation and near-field gas-gap conduction.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/pebble-bed-thermal-hydraulics\"\ncrate = \"tuas_boussinesq_solver\"\nmodule = \"lib/pre_built_components/insulated_porous_media_fluid_components\"\ntitle = \"Porous-media fluid components\"\nwhat = \"Fluid flowing through a porous solid matrix (packed bed).\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/neutronics-thermal-hydraulics-coupling\"\ncrate = \"outram-foam-appbuilder-lib\"\nmodule = \"genfoam/multi_region\"\ntitle = \"Multi-mesh coupling\"\nwhat = \"Couples the neutronics, TH and thermo-mechanics meshes with outer iterations.\"\nkind = \"port\"\nupstream = \"GeN-Foam multiRegion\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/neutronics-thermal-hydraulics-coupling\"\ncrate = \"nee_soon\"\nmodule = \"coupling\"\ntitle = \"Monte Carlo to GeN-Foam via MGXS\"\nwhat = \"Carries one reactor model from Monte Carlo to a deterministic solve on the same geometry.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/neutronics-thermal-hydraulics-coupling\"\ncrate = \"nee_soon\"\nmodule = \"direct_coupling\"\ntitle = \"Monte Carlo directly against GeN-Foam thermal hydraulics\"\nwhat = \"outram-mc as the neutronics solver in the coupled loop.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/neutronics-thermal-hydraulics-coupling\"\ncrate = \"outram-park-fork-moltres\"\nmodule = \"thermal\"\ntitle = \"Reduced fuel-salt thermal model and feedback loop\"\nwhat = \"Power/temperature feedback coupling for the circulating-fuel solve.\"\nkind = \"port\"\nupstream = \"Moltres (formulation)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/neutronics-thermal-hydraulics-coupling\"\ncrate = \"bedok\"\nmodule = \"thdiffusion_solverxyz\"\ntitle = \"Coupled TH and nodal diffusion\"\nwhat = \"Steady thermal-hydraulics coupled to the 3-D diffusion solve.\"\nkind = \"port\"\nupstream = \"BEDOK MATLAB\"\nstatus = \"proposed\"\n\n# ---- reactor coolant system ----------------------------------------------\n\n[[implementation]]\nconcept = \"02-nuclear-safety/reactor-coolant-system/residual-heat-removal\"\ncrate = \"tuas_boussinesq_solver\"\nmodule = \"lib/pre_built_components/ciet_three_branch_plus_dracs\"\ntitle = \"CIET primary loop plus DRACS\"\nwhat = \"Three-branch primary loop coupled to the passive DRACS decay-heat removal loop.\"\nkind = \"new-work\"\nupstream = \"Ong, Xiao & Peterson (2025) (peer reviewed)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/reactor-coolant-system/steam-generators\"\ncrate = \"tampines\"\nmodule = \"components/helical_coil_steam_generator\"\ntitle = \"Helical-coil once-through steam generator\"\nwhat = \"Spatially resolved counter-flow hot fluid, tube metal and water/steam.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/reactor-coolant-system/primary-heat-exchangers\"\ncrate = \"tuas_boussinesq_solver\"\nmodule = \"lib/pre_built_components/shell_and_tube_heat_exchanger\"\ntitle = \"Shell-and-tube heat exchanger\"\nwhat = \"Single-pass parallel-flow shell-and-tube model.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/reactor-coolant-system/primary-heat-exchangers\"\ncrate = \"tampines\"\nmodule = \"components/heat_exchanger\"\ntitle = \"TAMPINES heat exchanger\"\nwhat = \"Heat exchanger component of the TH framework.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/reactor-coolant-system/coolant-pumps-and-circulators\"\ncrate = \"tampines\"\nmodule = \"gas_phase/circulator\"\ntitle = \"Idealised helium circulator\"\nwhat = \"Pressure-raising machine of a gas-cooled primary circuit.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n# ---- containment ---------------------------------------------------------\n\n[[implementation]]\nconcept = \"02-nuclear-safety/containment/functional-containment\"\ncrate = \"bishan\"\nmodule = \"building\"\ntitle = \"HTR-10 vented confinement as one control volume\"\nwhat = \"The reactor building\'s lumped response, the outermost barrier of functional containment.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n# ---- I&C ------------------------------------------------------------------\n\n[[implementation]]\nconcept = \"02-nuclear-safety/instrumentation-and-control/control-systems\"\ncrate = \"chem-eng-real-time-process-control-simulator\"\nmodule = \"lib/stable\"\ntitle = \"Transfer functions and PID control\"\nwhat = \"Real-time process-control blocks (first/second-order, PID).\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/instrumentation-and-control/control-systems\"\ncrate = \"petir\"\nmodule = \"transfer_fn\"\ntitle = \"Transfer-function numerics\"\nwhat = \"Continuous and discrete transfer functions (Octave-derived).\"\nkind = \"port\"\nupstream = \"GNU Octave\"\nstatus = \"proposed\"\n\n# ---- steam and power conversion ------------------------------------------\n\n[[implementation]]\nconcept = \"02-nuclear-safety/steam-and-power-conversion/turbine-generator\"\ncrate = \"tampines\"\nmodule = \"components/turbine\"\ntitle = \"Steam turbine\"\nwhat = \"Turbine component of the balance of plant.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/steam-and-power-conversion/turbine-generator\"\ncrate = \"tampines-steam-tables\"\nmodule = \"steam_turbine_equations\"\ntitle = \"Steam-turbine equations\"\nwhat = \"Expansion-line relations on IF97 properties.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/steam-and-power-conversion/main-condensers\"\ncrate = \"tampines\"\nmodule = \"components/condenser\"\ntitle = \"Condenser\"\nwhat = \"Condenser component of the balance of plant.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/steam-and-power-conversion/circulating-water-system\"\ncrate = \"tampines\"\nmodule = \"cooling_tower\"\ntitle = \"Cooling tower (scaffold)\"\nwhat = \"Intended home of the Merkel / effectiveness-NTU cooling-tower engine; scaffold only.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/steam-and-power-conversion/power-cycle-flowsheet-simulation\"\ncrate = \"tampines\"\nmodule = \"balance_of_plant\"\ntitle = \"Balance of plant and Rankine cycle\"\nwhat = \"System-level assembly of BOP components.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/steam-and-power-conversion/power-cycle-flowsheet-simulation\"\ncrate = \"outram-park-fork-dwsim-libs\"\nmodule = \"heat_exchanger\"\ntitle = \"Heat exchanger rating\"\nwhat = \"LMTD, effectiveness-NTU, multi-pass correction and Tinker\'s method.\"\nkind = \"port\"\nupstream = \"DWSIM\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/steam-and-power-conversion/power-cycle-flowsheet-simulation\"\ncrate = \"outram-park-fork-dwsim-libs\"\nmodule = \"compressor\"\ntitle = \"Compressor\"\nwhat = \"Compressor unit operation.\"\nkind = \"port\"\nupstream = \"DWSIM\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/steam-and-power-conversion/power-cycle-flowsheet-simulation\"\ncrate = \"outram-park-fork-dwsim-libs\"\nmodule = \"expander\"\ntitle = \"Expander (isentropic)\"\nwhat = \"Expander/turbine unit operation.\"\nkind = \"port\"\nupstream = \"DWSIM\"\nstatus = \"proposed\"\n\n# ---- accident analysis, severe accidents, source terms --------------------\n\n[[implementation]]\nconcept = \"02-nuclear-safety/accident-analysis/reactivity-and-power-distribution-anomalies\"\ncrate = \"nee_soon\"\nmodule = \"xin_wang_sp3_workflow\"\ntitle = \"Control-rod-removal transient (Xin Wang SP3 workflow)\"\nwhat = \"Four-stage multiphysics pipeline reproducing a published rod-removal transient.\"\nkind = \"new-work\"\nupstream = \"Xin Wang (2018) UC Berkeley dissertation, Fig. 4.29\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/accident-analysis/decrease-in-reactor-coolant-inventory\"\ncrate = \"tampines\"\nmodule = \"critical_flow\"\ntitle = \"Choked two-phase flow\"\nwhat = \"HEM critical flow for blowdown (Edwards, Marviken).\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/accident-analysis/decrease-in-reactor-coolant-inventory\"\ncrate = \"sembawang\"\nmodule = \"htr10\"\ntitle = \"HTR-10 depressurised loss of forced cooling\"\nwhat = \"Joins boon-lay fuel failure and TRISO-ATOPS release on the HTR-10 DLOFC.\"\nkind = \"new-work\"\ncross_links = [\"13-environmental-protection/postulated-accident-impacts\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/severe-accidents/probabilistic-risk-assessment\"\ncrate = \"raffles\"\nmodule = \"scram\"\ntitle = \"Fault-tree quantification\"\nwhat = \"Minimal cut sets, BDD/ZBDD, probability and importance measures.\"\nkind = \"port\"\nupstream = \"SCRAM (rakhimov/scram)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/source-terms/coolant-source-terms\"\ncrate = \"boon-lay\"\nmodule = \"triso_atops_fork/activities\"\ntitle = \"Coolant activity and source terms\"\nwhat = \"Circulating and plate-out activity in the primary coolant.\"\nkind = \"port\"\nupstream = \"INL TRISO-ATOPS\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/source-terms/accident-source-terms\"\ncrate = \"sembawang\"\nmodule = \"accident\"\ntitle = \"Accident-phase release over TRISO-ATOPS\"\nwhat = \"Prescribed transient plus inventory out to a source term; no release physics of its own.\"\nkind = \"new-work\"\ncross_links = [\"13-environmental-protection/postulated-accident-impacts\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/source-terms/accident-source-terms\"\ncrate = \"sembawang\"\nmodule = \"chain\"\ntitle = \"Source term handed to changi\"\nwhat = \"Passes the released source term on for dispersion and deposition.\"\nkind = \"new-work\"\ncross_links = [\"13-environmental-protection/postulated-accident-impacts\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/source-terms/fission-product-inventory\"\ncrate = \"sembawang\"\nmodule = \"inventory\"\ntitle = \"Prescribed core inventory\"\nwhat = \"Core radionuclide inventory taken as a cited input, not computed.\"\nkind = \"new-work\"\ncross_links = [\"13-environmental-protection/postulated-accident-impacts\"]\nstatus = \"proposed\"\n\n# ---- quality assurance ----------------------------------------------------\n\n[[implementation]]\nconcept = \"07-regulatory-framework/quality-assurance/software-quality-assurance/verification-and-validation-records\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"vv\"\ntitle = \"V&V gate helpers\"\nwhat = \"Shared oracle-comparison gates used by njoy and outram-mc.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"07-regulatory-framework/quality-assurance/software-quality-assurance/verification-and-validation-records\"\ncrate = \"outram-mc-libs\"\nmodule = \"vv\"\ntitle = \"outram-mc V&V gates and oracle tables\"\nwhat = \"Committed oracle tables (NJOY golden values, graphite) the crate is measured against.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"07-regulatory-framework/quality-assurance/software-quality-assurance/configuration-and-accounting-records\"\ncrate = \"kovan-metrics\"\nmodule = \"historian\"\ntitle = \"Historian report\"\nwhat = \"Pre-merge accounting report for develop to main.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"07-regulatory-framework/quality-assurance/software-quality-assurance/configuration-and-accounting-records\"\ncrate = \"kovan-metrics\"\nmodule = \"trailer\"\ntitle = \"Commit token trailers\"\nwhat = \"Per-commit API-usage trailers.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"07-regulatory-framework/quality-assurance/software-quality-assurance/code-review\"\ncrate = \"kovan\"\nmodule = \"commands/code_walk\"\ntitle = \"Code walks\"\nwhat = \"Call chains from an entry point to the function implementing a concept.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n# ---- radiation protection -------------------------------------------------\n\n[[implementation]]\nconcept = \"08-radiation-protection/radiation-protection/shielding\"\ncrate = \"outram-mc-libs\"\nmodule = \"physics/weight_windows\"\ntitle = \"Weight windows for deep penetration\"\nwhat = \"Variance reduction that makes shielding problems tractable.\"\nkind = \"port\"\nupstream = \"OpenMC src/weight_windows.cpp\"\nstatus = \"proposed\"\n\n# ---- knowledge management and education ----------------------------------\n\n[[implementation]]\nconcept = \"10-human-resource-development/knowledge-management-and-education/knowledge-management\"\ncrate = \"kovan\"\nmodule = \"corpus\"\ntitle = \"Built-in nuclear-engineering corpus\"\nwhat = \"The curated standard corpus compiled into Kovan.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"10-human-resource-development/knowledge-management-and-education/knowledge-management\"\ncrate = \"kovan\"\nmodule = \"mindmap\"\ntitle = \"Interactive mind map\"\nwhat = \"The concept map, Kovan\'s home view.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"10-human-resource-development/knowledge-management-and-education/knowledge-management\"\ncrate = \"kovan\"\nmodule = \"classify\"\ntitle = \"Fine-grained classification\"\nwhat = \"Classification of literature below paper level.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"10-human-resource-development/knowledge-management-and-education/knowledge-management\"\ncrate = \"kovan-literature\"\nmodule = \"pdf_import\"\ntitle = \"PDF ingestion\"\nwhat = \"PDF to Markdown, metadata and BibTeX for the archive.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"10-human-resource-development/knowledge-management-and-education/education-and-outreach\"\ncrate = \"dhoby-ghaut\"\nmodule = \"web_demo\"\ntitle = \"Web-demo framework\"\nwhat = \"The track-independent half of every browser tutorial demo.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"10-human-resource-development/knowledge-management-and-education/education-and-outreach\"\ncrate = \"dhoby-ghaut\"\nmodule = \"workbench\"\ntitle = \"Guided simulation workbench\"\nwhat = \"The wizard that walks a learner through a high-fidelity simulation.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n# ---- site characteristics -------------------------------------------------\n\n[[implementation]]\nconcept = \"12-site-and-supporting-facilities/site-characteristics/hydrologic-engineering\"\ncrate = \"outram-park-fork-pflotran\"\nmodule = \"flow/richards\"\ntitle = \"Richards variably saturated groundwater flow\"\nwhat = \"Liquid-phase mass conservation in the subsurface.\"\nkind = \"port\"\nupstream = \"PFLOTRAN RICHARDS mode\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"12-site-and-supporting-facilities/site-characteristics/hydrologic-engineering\"\ncrate = \"outram-park-fork-pflotran\"\nmodule = \"reactive_transport\"\ntitle = \"Reactive transport\"\nwhat = \"Operator-split solute transport with equilibrium geochemistry.\"\nkind = \"port\"\nupstream = \"PFLOTRAN\"\nstatus = \"proposed\"\n\n# ---- environmental protection: dispersion and dose -------------------------\n\n[[implementation]]\nconcept = \"13-environmental-protection/meteorology-and-air-quality/short-term-accident-dispersion\"\ncrate = \"changi\"\nmodule = \"flexpart\"\ntitle = \"FLEXPART Lagrangian particle dispersion\"\nwhat = \"Advection, turbulence, convection and deposition of computational particles.\"\nkind = \"port\"\nupstream = \"FLEXPART v10.4 (GPL-3.0)\"\naspirations = [\"14-emergency-planning/accident-assessment\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"13-environmental-protection/meteorology-and-air-quality/short-term-accident-dispersion\"\ncrate = \"changi\"\nmodule = \"puff\"\ntitle = \"Gaussian puff forward dispersion\"\nwhat = \"A continuous release as a train of Gaussian puffs.\"\nkind = \"port\"\nupstream = \"puff R package 0.1.1 (MIT)\"\naspirations = [\"14-emergency-planning/accident-assessment\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"13-environmental-protection/meteorology-and-air-quality/short-term-accident-dispersion\"\ncrate = \"changi\"\nmodule = \"activity/accident_airborne_release\"\ntitle = \"Published HTR-10 accident airborne release\"\nwhat = \"Design-basis-accident release stored as cited reference data.\"\nkind = \"new-work\"\naspirations = [\"14-emergency-planning/accident-assessment\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"13-environmental-protection/meteorology-and-air-quality/long-term-routine-dispersion\"\ncrate = \"buangkok\"\nmodule = \"pydoseia/dispersion\"\ntitle = \"Gaussian-plume dilution for routine releases\"\nwhat = \"pyDOSEIA\'s plume dilution factors.\"\nkind = \"port\"\nupstream = \"pyDOSEIA\"\naspirations = [\"14-emergency-planning/accident-assessment\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"13-environmental-protection/meteorology-and-air-quality/relative-concentration-and-deposition\"\ncrate = \"changi\"\nmodule = \"activity/chi_over_q\"\ntitle = \"Unit-release dilution factors by travel time\"\nwhat = \"chi/Q from a unit release, binned by travel time.\"\nkind = \"new-work\"\naspirations = [\"14-emergency-planning/accident-assessment\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"13-environmental-protection/meteorology-and-air-quality/relative-concentration-and-deposition\"\ncrate = \"changi\"\nmodule = \"flexpart/dry_deposition\"\ntitle = \"Dry deposition\"\nwhat = \"Resistance model for gases and particle deposition velocity.\"\nkind = \"port\"\nupstream = \"FLEXPART v10.4\"\naspirations = [\"14-emergency-planning/accident-assessment\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"13-environmental-protection/meteorology-and-air-quality/relative-concentration-and-deposition\"\ncrate = \"changi\"\nmodule = \"flexpart/wet_deposition\"\ntitle = \"Wet deposition\"\nwhat = \"Below- and in-cloud scavenging and mass loss.\"\nkind = \"port\"\nupstream = \"FLEXPART v10.4\"\naspirations = [\"14-emergency-planning/accident-assessment\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"13-environmental-protection/meteorology-and-air-quality/relative-concentration-and-deposition\"\ncrate = \"changi\"\nmodule = \"flexpart/decay\"\ntitle = \"Radioactive decay in transit\"\nwhat = \"Per-species decay of airborne and deposited activity.\"\nkind = \"port\"\nupstream = \"FLEXPART v10.4\"\naspirations = [\"14-emergency-planning/accident-assessment\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"13-environmental-protection/radiological-impacts-of-normal-operation/exposure-pathways\"\ncrate = \"buangkok\"\nmodule = \"pydoseia/ingestion\"\ntitle = \"Terrestrial food-chain ingestion pathway\"\nwhat = \"IAEA SRS 19 screening equations and H-3/C-14 specific-activity models.\"\nkind = \"port\"\nupstream = \"pyDOSEIA\"\naspirations = [\"14-emergency-planning/accident-assessment\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"13-environmental-protection/radiological-impacts-of-normal-operation/exposure-pathways\"\ncrate = \"buangkok\"\nmodule = \"pydoseia/plume_shine\"\ntitle = \"Plume shine\"\nwhat = \"External gamma dose from the passing cloud (finite-cloud model).\"\nkind = \"port\"\nupstream = \"pyDOSEIA\"\naspirations = [\"14-emergency-planning/accident-assessment\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"13-environmental-protection/radiological-impacts-of-normal-operation/doses-to-members-of-the-public\"\ncrate = \"buangkok\"\nmodule = \"pydoseia/dose\"\ntitle = \"Five-pathway dose\"\nwhat = \"Dose to members of the public summed over pathways (research use only).\"\nkind = \"port\"\nupstream = \"pyDOSEIA\"\naspirations = [\"14-emergency-planning/accident-assessment\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"13-environmental-protection/radiological-impacts-of-normal-operation/doses-to-members-of-the-public\"\ncrate = \"buangkok\"\nmodule = \"coefficients\"\ntitle = \"EPA Federal Guidance Report dose coefficients\"\nwhat = \"FGR-11/13/15 coefficients for the tracked nuclides.\"\nkind = \"new-work\"\nupstream = \"US EPA FGR-11, FGR-13, FGR-15\"\naspirations = [\"14-emergency-planning/accident-assessment\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"13-environmental-protection/postulated-accident-impacts/design-basis-accident-consequences\"\ncrate = \"buangkok\"\nmodule = \"published/accident_dose_by_distance\"\ntitle = \"Published accident dose by distance\"\nwhat = \"Cited reference dose tables; computes nothing.\"\nkind = \"new-work\"\naspirations = [\"14-emergency-planning/accident-assessment\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n# ---- nuclear fuel cycle ---------------------------------------------------\n\n[[implementation]]\nconcept = \"16-nuclear-fuel-cycle/nuclear-criticality-safety/calculational-method-validation\"\ncrate = \"outram-mc-libs\"\nmodule = \"vv\"\ntitle = \"ICSBEP and oracle validation of the transport code\"\nwhat = \"Benchmark gates that validate the criticality method.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"18-industrial-involvement/process-heat-and-industrial-applications/chemical-process-simulation\"\ncrate = \"outram-park-fork-dwsim-libs\"\nmodule = \"flowsheet_solver\"\ntitle = \"Sequential-modular flowsheet solver\"\nwhat = \"Ordering, recycle and spec handling for process flowsheets.\"\nkind = \"port\"\nupstream = \"DWSIM\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"18-industrial-involvement/process-heat-and-industrial-applications/chemical-process-simulation\"\ncrate = \"outram-park-fork-dwsim-libs\"\nmodule = \"reactors\"\ntitle = \"Chemical reactor unit operations\"\nwhat = \"Conversion, equilibrium, Gibbs, CSTR and PFR reactors.\"\nkind = \"port\"\nupstream = \"DWSIM.UnitOperations/Reactors\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"18-industrial-involvement/process-heat-and-industrial-applications/chemical-process-simulation\"\ncrate = \"outram-park-fork-dwsim-libs\"\nmodule = \"columns\"\ntitle = \"Rigorous distillation and absorption columns\"\nwhat = \"MESH column models and solvers.\"\nkind = \"port\"\nupstream = \"DWSIM (commit 1abf72d)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"18-industrial-involvement/process-heat-and-industrial-applications/chemical-process-simulation\"\ncrate = \"outram-park-fork-dwsim-libs\"\nmodule = \"dynamics\"\ntitle = \"Dynamic flowsheet simulation\"\nwhat = \"Schedules, integrators, events and cause-and-effect matrices.\"\nkind = \"port\"\nupstream = \"DWSIM\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"18-industrial-involvement/process-heat-and-industrial-applications/chemical-process-simulation\"\ncrate = \"dover\"\nmodule = \"smr\"\ntitle = \"Steam-methane-reforming CSTR deck\"\nwhat = \"Deck-driven SMR reactor model on the DWSIM fork.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"16-nuclear-fuel-cycle/fuel-depletion/depletion-solvers\"\ncrate = \"outram-mc-libs\"\nmodule = \"depletion\"\ntitle = \"Monte Carlo burnup loop\"\nwhat = \"Bateman evolution coupled to transport reaction rates.\"\nkind = \"port\"\nupstream = \"OpenMC openmc.deplete\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"16-nuclear-fuel-cycle/fuel-depletion/depletion-solvers\"\ncrate = \"outram-park-fork-onix\"\nmodule = \"cram\"\ntitle = \"CRAM matrix exponential\"\nwhat = \"Chebyshev rational approximation of exp(A dt) for depletion.\"\nkind = \"port\"\nupstream = \"ONIX (MIT)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"16-nuclear-fuel-cycle/fuel-depletion/depletion-solvers\"\ncrate = \"boon-lay\"\nmodule = \"lagrangian_transmutation_and_fission_simulator\"\ntitle = \"Lagrangian Monte Carlo transmutation\"\nwhat = \"Competing-rate depletion without a burnup matrix.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"16-nuclear-fuel-cycle/fuel-depletion/decay-data\"\ncrate = \"boon-lay\"\nmodule = \"nuclide_reaction_and_decay_data\"\ntitle = \"Nuclide decay library\"\nwhat = \"Decay data by element group and its parsing.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"16-nuclear-fuel-cycle/fuel-depletion/decay-data\"\ncrate = \"boon-lay\"\nmodule = \"lagrangian_decay_simulator\"\ntitle = \"Lagrangian decay simulator\"\nwhat = \"Monte Carlo decay of single radionuclides and chains.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"16-nuclear-fuel-cycle/fuel-depletion/irradiation-history\"\ncrate = \"outram-park-fork-offbeat\"\nmodule = \"burnup\"\ntitle = \"Burnup and fast-fluence accumulation\"\nwhat = \"Irradiation-history bookkeeping of a fuel-performance run.\"\nkind = \"port\"\nupstream = \"OFFBEAT\"\nstatus = \"proposed\"\n\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/air-and-moisture-ingress\"\ntitle = \"Air and moisture (water) ingress\"\norigin = \"nrc\"\nsources = [{ document = \"rg-1.232-rev0\", section = \"Appendix C, MHTGR-DC 14 (reactor helium pressure boundary: unacceptable ingress of moisture, air, secondary coolant); MHTGR-DC 30 (means of detecting ingress)\" }]\nstatus = \"approved\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/accident-analysis/air-and-moisture-ingress\"\ncrate = \"boon-lay\"\nmodule = \"chemistry\"\ntitle = \"Graphite and fuel chemical attack\"\nwhat = \"IG-110 graphite oxidation by steam and air, and UO2 kernel hydrolysis: cited closed-form rate laws used by htgr_sim_v1\'s water-ingress stage.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"16-nuclear-fuel-cycle/fuel-cycle-scenarios\"\ncrate = \"kaki-bukit\"\nmodule = \"agents\"\ntitle = \"Agent-based fuel-cycle simulation\"\nwhat = \"Facilities as agents exchanging material over time: fuel-cycle flows and inventories.\"\nkind = \"port\"\nupstream = \"CYCLUS\"\nstatus = \"proposed\"\n\n\n[[implementation]]\nconcept = \"10-human-resource-development/knowledge-management-and-education/education-and-outreach\"\ncrate = \"outram-park-digital-twin-engine\"\nmodule = \"app_scaffold\"\ntitle = \"Digital-twin engine: offline educational plant simulators\"\nwhat = \"The egui simulator framework behind htgr_sim_v1, fhr_sim_v2 and distillation_sim_v1: offline demonstrations only (RESPONSIBLE_USE.md).\"\nkind = \"new-work\"\naspirations = [\"02-nuclear-safety/conduct-of-operations/operator-training-and-requalification\", \"02-nuclear-safety/instrumentation-and-control/information-systems-and-displays\", \"02-nuclear-safety/accident-analysis/transient-and-accident-analysis-methods\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"10-human-resource-development/knowledge-management-and-education/education-and-outreach\"\ncrate = \"outram-park-digital-twin-engine\"\nmodule = \"htr10\"\ntitle = \"Digital-twin engine: HTR-10 plant model\"\nwhat = \"The HTR-10 plant model the htgr_sim_v1 simulator runs: offline, educational.\"\nkind = \"new-work\"\naspirations = [\"02-nuclear-safety/conduct-of-operations/operator-training-and-requalification\", \"02-nuclear-safety/instrumentation-and-control/information-systems-and-displays\", \"02-nuclear-safety/accident-analysis/transient-and-accident-analysis-methods\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[concept]]\npath = \"01-national-position/need-for-power/power-system-description\"\ntitle = \"Description of power system\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1555\", section = \"8.1\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"01-national-position/need-for-power/power-demand\"\ntitle = \"Power demand (power and energy requirements; factors affecting growth of demand)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1555\", section = \"8.2 (8.2.1, 8.2.2)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"01-national-position/need-for-power/power-supply\"\ntitle = \"Power supply\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1555\", section = \"8.3\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"01-national-position/need-for-power/need-for-power-assessment\"\ntitle = \"Assessment of need for power\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1555\", section = \"8.4\" }]\nstatus = \"approved\"\n\n# DEFERRED until a citation is in the corpus (maintainer, 2026-10-06), under\n# 01-national-position, beside need-for-power:\n#   - industrial process (process heat / non-electric applications)\n#   - medical isotope production\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/core-configurations\"\ntitle = \"Core configurations by reactor type\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.1 (core configurations)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/core-configurations/prismatic\"\ntitle = \"Prismatic (block-type) VHTR cores\"\norigin = \"nrc\"\nsources = [{ document = \"rg-1.232-rev0\", section = \"Appendix C (MHTGR-DC)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/core-configurations/pebble-bed\"\ntitle = \"Pebble-bed VHTR cores\"\norigin = \"nrc\"\nsources = [{ document = \"rg-1.232-rev0\", section = \"Appendix C (MHTGR-DC)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/core-configurations/liquid-fuelled\"\ntitle = \"Liquid-fuelled cores (MSR)\"\norigin = \"nrc\"\nsources = [{ document = \"ornl-tm-2018-976\", section = \"2 (homogeneous fuel; fuel salt boundary as the first fission-product barrier)\" }]\ncross_links = [\"02-nuclear-safety/thermal-hydraulic-design/gas-generation-and-entrainment\", \"02-nuclear-safety/msr-coolant-loop/coolant-loop-materials-and-chemistry\"]\nnote = \"For MSRs, thermal hydraulics becomes more of a feedback and structural-materials issue (maintainer, 2026-10-06): hence its links to gas generation, coolant-loop materials and (via precursor drift) reactivity.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/core-configurations/liquid-metal-cooled\"\ntitle = \"Liquid-metal-cooled fast reactor cores (SFR, LFR)\"\norigin = \"nrc\"\nsources = [{ document = \"rg-1.232-rev0\", section = \"Appendix B (SFR-DC); Appendix A (ARDC)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/core-configurations/gas-cooled-fast\"\ntitle = \"Gas-cooled fast reactor cores (GFR)\"\norigin = \"nrc\"\nsources = [{ document = \"rg-1.232-rev0\", section = \"Appendix A (ARDC, developed with GFRs in view)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/core-configurations/supercritical-water-cooled\"\ntitle = \"Supercritical-water-cooled reactor cores (SCWR)\"\norigin = \"nrc\"\nsources = []\nnote = \"No NRC guidance specific to SCWR (RG 1.232 covers non-LWRs only); world reference to obtain: Generation IV International Forum SCWR documents.\"\nstatus = \"deferred\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods\"\ntitle = \"Thermal-hydraulic analysis methods\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6 (a detailed description of the analytical methods used in the thermal-hydraulic design)\" }, { document = \"ornl-tm-2018-976\", section = \"3.1.3 (acceptable analytical methods)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/gas-generation-and-entrainment\"\ntitle = \"Gas generation and entrainment (MSR flow-instability mechanisms)\"\norigin = \"nrc\"\nsources = [{ document = \"ornl-tm-2018-976\", section = \"3.1.3 (LWR DNB/CHF/CPR measures are not applicable to MSR technology, \'but other mechanisms may exist, such as gas generation or entrainment\')\" }]\ncross_links = [\"02-nuclear-safety/nuclear-design/reactivity-coefficients\", \"02-nuclear-safety/msr-coolant-loop/coolant-loop-materials-and-chemistry\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/experimental-loops-and-test-reactor-support\"\ntitle = \"Experimental loops and test-reactor support for thermal-hydraulic design (MSR)\"\norigin = \"nrc\"\nsources = [{ document = \"ornl-tm-2018-976\", section = \"3.1.3 (justified extrapolation from proven designs \'will rely heavily on experimental loops and perhaps a test reactor\')\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/steam-and-power-conversion/alternate-power-cycles\"\ntitle = \"Alternate (non-steam) power conversion cycles\"\norigin = \"nrc\"\nsources = [{ document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 4 rationale (very high-speed, very high-energy gas turbines inside the reactor helium pressure boundary)\" }, { document = \"jrc-eur-28712\", section = \"power conversion: direct-cycle helium gas turbine (Brayton); combined cycles\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/steam-and-power-conversion/alternate-power-cycles/direct-cycle-helium-brayton\"\ntitle = \"Direct-cycle helium gas turbine (Brayton)\"\norigin = \"nrc\"\nsources = [{ document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 4 rationale\" }, { document = \"jrc-eur-28712\", section = \"Brayton cycle with a gas turbine placed directly in the hot gas\" }]\ncross_links = [\"02-nuclear-safety/reactor-coolant-system/coolant-pumps-and-circulators\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/steam-and-power-conversion/alternate-power-cycles/combined-cycles\"\ntitle = \"Combined cycles (helium / gas mixtures)\"\norigin = \"nrc\"\nsources = [{ document = \"jrc-eur-28712\", section = \"power conversion options table (He/mixture: combined cycles)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/steam-and-power-conversion/alternate-power-cycles/supercritical-co2-cycles\"\ntitle = \"Supercritical CO2 power cycles\"\norigin = \"nrc\"\nsources = []\nnote = \"No source in the corpus yet; deferred until literature is supplied.\"\nstatus = \"deferred\"\n\n[[concept]]\npath = \"02-nuclear-safety/steam-and-power-conversion/alternate-power-cycles/air-brayton-combined-cycle\"\ntitle = \"Air-Brayton combined cycle (FHR)\"\norigin = \"nrc\"\nsources = []\nnote = \"No source in the corpus yet; deferred until literature is supplied.\"\nstatus = \"deferred\"\n\n[[concept]]\npath = \"07-regulatory-framework/reactor-licensing/two-step-licensing\"\ntitle = \"Two-step licensing: construction permit, then operating license\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"Part 50\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1300 Construction permits\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1360 Operating licenses\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/reactor-licensing/one-step-licensing\"\ntitle = \"Design certification, early site permit and combined license\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr52\", section = \"Part 52\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1140 Early site permits\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1230 Standard design certifications\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1410 Combined licenses\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/reactor-licensing/risk-informed-technology-inclusive-framework\"\ntitle = \"Risk-informed, technology-inclusive framework (advanced reactors)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"Part 53, Subparts A-J, M\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/reactor-licensing/general-design-criteria\"\ntitle = \"General design criteria (and their non-LWR adaptations)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"Appendix A, General Design Criteria for Nuclear Power Plants\" }, { document = \"rg-1.232-rev0\", section = \"Appendices A-C (ARDC, SFR-DC, MHTGR-DC)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"08-radiation-protection/radiation-protection/shielding/radiation-transport\"\ntitle = \"Radiation transport for shielding (neutrons, photons, coupled, charged particles)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"12.3-12.4 Radiation Protection Design Features\" }, { document = \"nureg-1537-part1\", section = \"4.4 Biological Shield\" }]\nwhy = \"maintainer 2026-10-06: shielding needs radiation transport generally, not neutron transport alone.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"08-radiation-protection/radiation-protection/shielding/radiation-transport/neutron-shielding-transport\"\ntitle = \"Neutron transport for shielding (deep penetration, fixed source)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"12.3-12.4 Radiation Protection Design Features\" }, { document = \"nureg-1537-part1\", section = \"4.4 Biological Shield\" }]\nwhy = \"fixed-source, deep-penetration neutron transport; shares methods with nuclear design.\"\ncross_links = [\"02-nuclear-safety/nuclear-design/neutron-transport\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"08-radiation-protection/radiation-protection/shielding/radiation-transport/photon-transport\"\ntitle = \"Photon (gamma) transport\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"12.3-12.4 Radiation Protection Design Features\" }, { document = \"nureg-1537-part1\", section = \"4.4 Biological Shield\" }]\nwhy = \"gamma shielding and dose; needs photo-atomic data (njoy\'s photo-atomic ACE class) and photon collision physics.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"08-radiation-protection/radiation-protection/shielding/radiation-transport/coupled-neutron-photon-transport\"\ntitle = \"Coupled neutron-photon transport (secondary gammas)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"12.3-12.4 Radiation Protection Design Features\" }, { document = \"nureg-1537-part1\", section = \"4.4 Biological Shield\" }]\nwhy = \"capture and inelastic gammas produced by neutrons; the usual reactor-shielding calculation.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"08-radiation-protection/radiation-protection/shielding/radiation-transport/charged-particle-transport\"\ntitle = \"Charged-particle (electron, positron, ion) transport\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"12.3-12.4 Radiation Protection Design Features\" }, { document = \"nureg-1537-part1\", section = \"4.4 Biological Shield\" }]\nwhy = \"bremsstrahlung and energy deposition; where detector and medical codes (e.g. Geant4) lead.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"18-industrial-involvement/process-heat-and-industrial-applications/nuclear-techniques-for-plastic-pollution\"\ntitle = \"Nuclear techniques against plastic pollution (radiation-assisted recycling; isotopic tracing of marine microplastics)\"\norigin = \"iaea\"\nsources = [{ document = \"iaea-nutec-plastics\", section = \"whole document (8 pp.)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/licensing-basis-events\"\ntitle = \"Licensing-basis events (anticipated, unlikely and very unlikely event sequences, and design-basis accidents)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.240 Licensing-basis events\" }, { document = \"10cfr53\", section = \"\u{a7} 53.450 Analysis requirements, (e) Analyses of licensing-basis events other than design-basis accidents; (f) Analysis of design-basis accidents\" }, { document = \"10cfr53\", section = \"\u{a7} 53.020 Definitions (Licensing-basis events; Design-basis accidents)\" }]\ncross_links = [\"02-nuclear-safety/accident-analysis/maximum-hypothetical-accident\", \"02-nuclear-safety/accident-analysis/reactivity-and-power-distribution-anomalies\", \"02-nuclear-safety/severe-accidents/probabilistic-risk-assessment\", \"02-nuclear-safety/accident-analysis/transient-and-accident-analysis-methods\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/licensing-basis-events/safety-criteria-for-design-basis-accidents\"\ntitle = \"Safety criteria for design-basis accidents (25 rem TEDE at the exclusion area boundary and low-population zone)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.210 Safety criteria for design-basis accidents\" }, { document = \"10cfr53\", section = \"\u{a7} 53.530 Population-related considerations, (a)\" }, { document = \"10cfr50\", section = \"\u{a7} 50.34 Contents of applications; technical information, (a)(1)(ii)(D)\" }, { document = \"10cfr52\", section = \"\u{a7} 52.47 Contents of applications; technical information, (a)(2)(iv)\" }]\ncross_links = [\"13-environmental-protection/postulated-accident-impacts/design-basis-accident-consequences\", \"02-nuclear-safety/source-terms/accident-source-terms\", \"12-site-and-supporting-facilities/site-characteristics/geography-and-demography\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/licensing-basis-events/safety-criteria-for-licensing-basis-events-other-than-dbas\"\ntitle = \"Safety criteria for licensing-basis events other than design-basis accidents (comprehensive risk metrics)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.220 Safety criteria for licensing-basis events other than design-basis accidents\" }, { document = \"10cfr53\", section = \"\u{a7} 53.450 Analysis requirements, (e)\" }]\ncross_links = [\"02-nuclear-safety/severe-accidents/probabilistic-risk-assessment\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/defense-in-depth\"\ntitle = \"Defense in depth\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.250 Defense in depth\" }, { document = \"10cfr53\", section = \"\u{a7} 53.020 Definitions (Defense in depth)\" }, { document = \"10cfr53\", section = \"\u{a7} 53.450 Analysis requirements, (b)(3)\" }]\ncross_links = [\"02-nuclear-safety/design-of-structures-systems-and-components/safety-functions\", \"02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel\", \"02-nuclear-safety/containment/functional-containment\", \"02-nuclear-safety/severe-accidents\", \"02-nuclear-safety/severe-accidents/probabilistic-risk-assessment\", \"02-nuclear-safety/containment/functional-containment\"]\nnote = \"Placement is a judgement call: Part 53 ties defense in depth to the uncertainties in the analysis of licensing-basis events other than DBAs (\u{a7} 53.250(a)-(c)), hence accident-analysis; it is cross-cutting and could equally sit under design-of-structures-systems-and-components.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/transient-and-accident-analysis-methods/qualification-of-analytical-codes\"\ntitle = \"Qualification of analytical codes\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.450 Analysis requirements, (d) Qualification of analytical codes\" }, { document = \"10cfr50\", section = \"\u{a7} 50.43 Additional standards and provisions affecting class 103 licenses and certifications for commercial power, (e)(1)(iii)\" }]\ncross_links = [\"07-regulatory-framework/quality-assurance/software-quality-assurance/verification-and-validation-records\", \"02-nuclear-safety/engineered-safety-features/emergency-core-cooling\"]\nnote = \"\u{a7} 53.450(d) names thermodynamics, reactor physics, fuel performance and mechanistic source term codes: the regulatory home of outram-park\'s V&V work.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/safety-functions\"\ntitle = \"Safety functions (primary: limiting the release of radioactive materials; additional: reactivity, heat generation, heat removal, chemical interactions)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.230 Safety functions\" }, { document = \"10cfr53\", section = \"\u{a7} 53.400 Design features for licensing-basis events\" }]\ncross_links = [\"02-nuclear-safety/containment/functional-containment\", \"02-nuclear-safety/accident-analysis/licensing-basis-events\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/safety-functions/functional-design-criteria\"\ntitle = \"Functional design criteria\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.410 Functional design criteria for design-basis accidents\" }, { document = \"10cfr53\", section = \"\u{a7} 53.420 Functional design criteria for licensing-basis events other than design-basis accidents\" }, { document = \"10cfr53\", section = \"\u{a7} 53.425 Design features and functional design criteria for normal operations\" }, { document = \"10cfr53\", section = \"\u{a7} 53.430 Design features and functional design criteria for protection of plant workers\" }, { document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (a)\" }]\ncross_links = [\"07-regulatory-framework/reactor-licensing/general-design-criteria\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/safety-categorization-and-special-treatments\"\ntitle = \"Safety categorization and special treatments (safety-related; non-safety-related but safety-significant; non-safety-significant)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.460 Safety categorization and special treatments\" }, { document = \"10cfr50\", section = \"\u{a7} 50.69 Risk-informed categorization and treatment of structures, systems and components for nuclear power reactors\" }]\ncross_links = [\"07-regulatory-framework/quality-assurance\", \"02-nuclear-safety/severe-accidents/probabilistic-risk-assessment\", \"07-regulatory-framework/quality-assurance/quality-assurance-program\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/codes-and-standards\"\ntitle = \"Codes and standards\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.55a Codes and standards\" }, { document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (b)\" }]\ncross_links = [\"02-nuclear-safety/design-of-structures-systems-and-components/mechanical-systems-and-components\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/inservice-inspection-and-inservice-testing\"\ntitle = \"Inservice inspection and inservice testing\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.880 Inservice inspection and inservice testing\" }, { document = \"10cfr50\", section = \"\u{a7} 50.55a Codes and standards, (f) Preservice and inservice testing requirements; (g) Preservice and inservice inspection requirements\" }]\ncross_links = [\"02-nuclear-safety/msr-coolant-loop/coolant-loop-drain-tank-and-isolation\", \"02-nuclear-safety/fuel-system-design/testing-inspection-and-surveillance\", \"02-nuclear-safety/design-of-structures-systems-and-components/codes-and-standards\", \"07-regulatory-framework/quality-assurance/maintenance-rule\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/integrity-assessment-programs\"\ntitle = \"Integrity assessment programs (plant aging, cyclic or transient load limits, degradation mechanisms)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.870 Integrity assessment programs\" }, { document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (d)\" }]\ncross_links = [\"02-nuclear-safety/reactor-coolant-system/reactor-vessel-integrity\", \"07-regulatory-framework/quality-assurance/maintenance-rule\", \"02-nuclear-safety/engineered-safety-features/esf-materials\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/missile-protection/aircraft-impact-assessment\"\ntitle = \"Aircraft impact assessment\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.150 Aircraft impact assessment\" }, { document = \"10cfr52\", section = \"\u{a7} 52.47 Contents of applications; technical information, (a)(28)\" }, { document = \"10cfr52\", section = \"\u{a7} 52.79 Contents of applications; technical information in final safety analysis report, (a)(47)\" }, { document = \"10cfr53\", section = \"\u{a7} 53.910 Procedures and guidelines, (b)(7) (potential aircraft threat)\" }]\ncross_links = [\"02-nuclear-safety/design-of-structures-systems-and-components/missile-protection\", \"02-nuclear-safety/severe-accidents/severe-accident-evaluation\", \"15-nuclear-security/physical-protection\"]\nnote = \"Part 53 has no design-specific aircraft impact assessment of its own; it only requires procedures for a notified aircraft threat (\u{a7} 53.910(b)(7)). \u{a7} 50.150 is a beyond-design-basis assessment, distinct from SRP 3.5.1.6 aircraft hazards already cited under missile-protection.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/severe-accidents/mitigation-of-beyond-design-basis-events\"\ntitle = \"Mitigation of beyond-design-basis events (mitigation strategies; extensive damage mitigation guidelines)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.155 Mitigation of beyond-design-basis events\" }]\ncross_links = [\"02-nuclear-safety/severe-accidents/severe-accident-evaluation\", \"09-electrical-grid/electric-power/station-blackout\", \"02-nuclear-safety/auxiliary-systems/cooling-water-and-ultimate-heat-sink\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/initial-test-program/initial-plant-test-program/safety-feature-testing-and-prototype-plants\"\ntitle = \"Demonstration of safety-feature performance for innovative designs; prototype plant testing\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.43 Additional standards and provisions affecting class 103 licenses and certifications for commercial power, (e)\" }, { document = \"10cfr52\", section = \"\u{a7} 52.1 Definitions (Prototype plant)\" }, { document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (a)(1)\" }]\ncross_links = [\"02-nuclear-safety/thermal-hydraulic-design/experimental-loops-and-test-reactor-support\", \"02-nuclear-safety/accident-analysis/transient-and-accident-analysis-methods/qualification-of-analytical-codes\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/reactor-coolant-system/reactor-coolant-system-venting\"\ntitle = \"Reactor coolant system venting systems\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.46a Acceptance criteria for reactor coolant system venting systems\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/reactor-coolant-system/reactor-vessel-integrity/pressurized-thermal-shock\"\ntitle = \"Fracture toughness requirements for protection against pressurized thermal shock events\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.61 Fracture toughness requirements for protection against pressurized thermal shock events\" }, { document = \"10cfr50\", section = \"\u{a7} 50.61a Alternate fracture toughness requirements for protection against pressurized thermal shock events\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/conduct-of-operations/self-reliant-mitigation-facilities\"\ntitle = \"Self-reliant-mitigation facilities and generally licensed reactor operators\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.725 General staffing, training, personnel qualifications, and human factors requirements, (a)\" }, { document = \"10cfr53\", section = \"\u{a7} 53.800 Facility licensees for self-reliant-mitigation facilities\" }, { document = \"10cfr53\", section = \"\u{a7} 53.805 Facility licensee requirements related to generally licensed reactor operators\" }, { document = \"10cfr53\", section = \"\u{a7} 53.810 Generally licensed reactor operators\" }, { document = \"10cfr53\", section = \"\u{a7} 53.815 Generally licensed reactor operator training, examination, and proficiency programs\" }]\ncross_links = [\"10-human-resource-development\", \"02-nuclear-safety/human-factors-engineering\", \"02-nuclear-safety/conduct-of-operations/operator-training-and-requalification\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/conduct-of-operations/operator-training-and-requalification/simulation-facilities\"\ntitle = \"Simulation facilities (scope, fidelity and performance testing)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.780 Training, examination, and proficiency program, (e) Simulation facilities\" }, { document = \"10cfr53\", section = \"\u{a7} 53.815 Generally licensed reactor operator training, examination, and proficiency programs, (e) Simulation facilities\" }, { document = \"10cfr53\", section = \"\u{a7} 53.725 General staffing, training, personnel qualifications, and human factors requirements, (c) (Simulation facility; Performance testing; Reference plant)\" }]\ncross_links = [\"10-human-resource-development\", \"10-human-resource-development/knowledge-management-and-education/education-and-outreach\"]\nnote = \"outram-park\'s egui simulators are offline educational demonstrations (RESPONSIBLE_USE.md), not simulation facilities in the \u{a7} 53.780(e) sense; any link from them should be an \'aspiration\', never a home.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/conduct-of-operations/staffing-plan\"\ntitle = \"Staffing plan (on-shift staffing and engineering expertise)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.730 Defining, fulfilling, and maintaining the role of personnel in ensuring safe operations, (f) Staffing plan\" }, { document = \"10cfr53\", section = \"\u{a7} 53.740 Facility licensee requirements\u{2014}general, (b)\" }, { document = \"10cfr50\", section = \"\u{a7} 50.54 Conditions of licenses, (m)\" }]\ncross_links = [\"02-nuclear-safety/human-factors-engineering/concept-of-operations-and-function-allocation\", \"10-human-resource-development\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/conduct-of-operations/operating-experience\"\ntitle = \"Operating experience program\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.730 Defining, fulfilling, and maintaining the role of personnel in ensuring safe operations, (e) Operating experience\" }, { document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (a)(2)\" }, { document = \"10cfr52\", section = \"\u{a7} 52.47 Contents of applications; technical information, (a)(22)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/conduct-of-operations/event-notification-and-reporting\"\ntitle = \"Immediate notification requirements and the licensee event report system\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.72 Immediate notification requirements for operating nuclear power reactors\" }, { document = \"10cfr50\", section = \"\u{a7} 50.73 Licensee event report system\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1630 Immediate notification requirements for operating commercial nuclear plants\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1640 Licensee event report system\" }]\ncross_links = [\"07-regulatory-framework\", \"14-emergency-planning/notification-methods-and-procedures\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/human-factors-engineering/concept-of-operations-and-function-allocation\"\ntitle = \"Concept of operations; functional requirements analysis and function allocation\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.730 Defining, fulfilling, and maintaining the role of personnel in ensuring safe operations, (c) Concept of operations; (d) Functional requirements analysis and function allocation\" }, { document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (n)(3)-(4)\" }]\ncross_links = [\"02-nuclear-safety/design-of-structures-systems-and-components/safety-functions\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"04-funding-and-financing/financial-qualifications/financial-protection-and-accident-insurance\"\ntitle = \"Financial protection; insurance required to stabilize and decontaminate plant following an accident\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.1710 Financial protection\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1720 Insurance required to stabilize and decontaminate plant following an accident\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1730 Financial protection requirements\" }, { document = \"10cfr50\", section = \"\u{a7} 50.54 Conditions of licenses, (w)\" }]\ncross_links = [\"05-legal-framework\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/reactor-licensing/manufacturing-licenses\"\ntitle = \"Manufacturing licenses (manufactured reactors installed at sites not identified in the application)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr52\", section = \"\u{a7} 52.151 Scope of subpart (Subpart F, Manufacturing Licenses)\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1270 Manufacturing licenses\" }, { document = \"10cfr53\", section = \"\u{a7} 53.620 Manufacturing\" }]\ncross_links = [\"07-regulatory-framework/reactor-licensing/one-step-licensing\", \"18-industrial-involvement\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/reactor-licensing/standard-design-approvals\"\ntitle = \"Standard design approvals\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr52\", section = \"\u{a7} 52.131 Scope of subpart (Subpart E, Standard Design Approvals)\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1200 Standard design approvals\" }]\ncross_links = [\"07-regulatory-framework/reactor-licensing/one-step-licensing\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/reactor-licensing/standardization-at-multiple-sites\"\ntitle = \"Standardization of nuclear power plant designs: reactors of identical design at multiple sites\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"Appendix N, Standardization of Nuclear Power Plant Designs: Permits To Construct and Licenses To Operate Nuclear Power Reactors of Identical Design at Multiple Sites\" }, { document = \"10cfr52\", section = \"Appendix N, Standardization of Nuclear Power Plant Designs: Combined Licenses To Construct and Operate Nuclear Power Reactors of Identical Design at Multiple Sites\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1470 Standardization of commercial nuclear plant designs: licenses to construct and operate nuclear power reactors of identical design at multiple sites\" }]\ncross_links = [\"07-regulatory-framework/reactor-licensing/one-step-licensing\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/reactor-licensing/limited-work-authorization\"\ntitle = \"Limited work authorization\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.10 License required; limited work authorization\" }, { document = \"10cfr52\", section = \"\u{a7} 52.91 Authorization to conduct limited work authorization activities\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1130 Limited work authorizations\" }]\ncross_links = [\"07-regulatory-framework/reactor-licensing/one-step-licensing\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/reactor-licensing/changes-tests-and-experiments\"\ntitle = \"Changes, tests, and experiments (evaluating changes to the facility as described in the Final Safety Analysis Report)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.59 Changes, tests, and experiments\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1540 Updating licensing-basis information and determining the need for NRC approval\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1550 Evaluating changes to facility as described in Final Safety Analysis Reports\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/reactor-licensing/backfitting\"\ntitle = \"Backfitting\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.109 Backfitting\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1590 Backfitting\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"08-radiation-protection/radiation-protection/protection-of-plant-workers\"\ntitle = \"Protection of plant workers (occupational dose under 10 CFR part 20)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.270 Protection of plant workers\" }, { document = \"10cfr53\", section = \"\u{a7} 53.430 Design features and functional design criteria for protection of plant workers\" }]\ncross_links = [\"08-radiation-protection/radiation-protection/protection-of-plant-workers/alara\"]\nnote = \"Close to the existing ALARA concept (occupational exposure); kept separate because Part 53 makes it a top-level safety requirement (Subpart B) with its own functional design criteria. Merging into alara as extra sources is the alternative.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"14-emergency-planning/emergency-plan-development-and-review/performance-based-emergency-preparedness\"\ntitle = \"Emergency preparedness for small modular reactors, non-light-water reactors, and non-power production or utilization facilities (performance-based framework)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.160 Emergency preparedness for small modular reactors, non-light-water reactors, and non-power production or utilization facilities\" }, { document = \"10cfr53\", section = \"\u{a7} 53.855 Emergency preparedness, (a)\" }]\ncross_links = [\"14-emergency-planning/exercises-and-drills\", \"14-emergency-planning/accident-assessment\"]\nnote = \"Placed under planning standard P (planning effort) because \u{a7} 50.160 is an alternative to the whole \u{a7} 50.47(b)/Appendix E plan; its performance objectives are demonstrated by drills, hence the cross-link to exercises-and-drills.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"14-emergency-planning/protective-response/plume-exposure-pathway-epz-size\"\ntitle = \"Plume exposure pathway emergency planning zone (EPZ) size (dose-based: 1 rem TEDE over 96 hours)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.33 Contents of applications; general information, (g)(1)-(2)\" }, { document = \"10cfr50\", section = \"\u{a7} 50.160 Emergency preparedness for small modular reactors, non-light-water reactors, and non-power production or utilization facilities, (b)(3) Emergency planning zone\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1109 Contents of applications; general information, (g)\" }]\ncross_links = [\"14-emergency-planning/accident-assessment/dose-projection\", \"13-environmental-protection/meteorology-and-air-quality/short-term-accident-dispersion\"]\nnote = \"EPZ sizing under \u{a7} 50.33(g)(2)(i) is a consequence calculation (accident likelihood, source term, timing, meteorology). Per #724 the dispersion crates may link here only as \'aspiration\'.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"15-nuclear-security/physical-protection/access-authorization-and-fitness-for-duty\"\ntitle = \"Access authorization and fitness-for-duty programs\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.860 Security programs, (b) Fitness-for-duty; (c) Access authorization\" }, { document = \"10cfr52\", section = \"\u{a7} 52.79 Contents of applications; technical information in final safety analysis report, (a)(44)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"15-nuclear-security/physical-protection/safety-and-security-in-design\"\ntitle = \"Safety and security considered together in the design process\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (f)\" }]\ncross_links = [\"02-nuclear-safety/accident-analysis/defense-in-depth\", \"02-nuclear-safety/design-of-structures-systems-and-components/safety-functions\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"17-radioactive-waste-management/decommissioning/decommissioning-financial-assurance\"\ntitle = \"Financial assurance and cost estimates for decommissioning\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.75 Reporting and recordkeeping for decommissioning planning\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1010 Financial assurance for decommissioning\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1020 Cost estimates for decommissioning\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1030 Annual adjustments to cost estimates for decommissioning\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1040 Methods for providing financial assurance for decommissioning\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1045 Limitations on the use of decommissioning trust funds\" }]\ncross_links = [\"04-funding-and-financing/financial-qualifications\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"15-nuclear-security/cybersecurity/information-security\"\ntitle = \"Information security\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.860 Security programs, (e) Information security\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"15-nuclear-security/cybersecurity/cybersecurity-program\"\ntitle = \"Cybersecurity program\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.860 Security programs, (d) Cybersecurity\" }, { document = \"10cfr50\", section = \"\u{a7} 50.34 Contents of applications; technical information, (c)(2)\" }]\ncross_links = [\"02-nuclear-safety/instrumentation-and-control/digital-i-and-c-software\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/reactor-licensing/license-renewal\"\ntitle = \"License renewal\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.135 Renewal of non-power production or utilization facility licenses issued under \u{a7} 50.22\" }, { document = \"10cfr52\", section = \"\u{a7} 52.29-52.33, 52.57-52.61, 52.107, 52.177-52.181 (application, criteria and duration of renewal)\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1173-53.1179, 53.1254-53.1260, 53.1295, 53.1402, 53.1458 (renewal)\" }]\ncross_links = [\"02-nuclear-safety/design-of-structures-systems-and-components/integrity-assessment-programs\"]\nnote = \"Power-reactor operating-license renewal is 10 CFR Part 54 (not in the corpus yet); the integrity assessment programs of \u{a7} 53.870 cover plant aging.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"10-human-resource-development/knowledge-management-and-education/nuclear-history-and-lessons-learned\"\ntitle = \"Nuclear history and lessons learned (TMI, Chernobyl, Fukushima)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.34 Contents of applications; technical information, (f) Additional TMI-related requirements\" }, { document = \"wash-1400\", section = \"Executive Summary and Main Report\" }]\ncross_links = [\"02-nuclear-safety/conduct-of-operations/operating-experience\", \"02-nuclear-safety/severe-accidents\"]\nnote = \"Maintainer 2026-10-06: TMI fits here. Chernobyl and Fukushima await literature.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/core-configurations/light-water-cooled\"\ntitle = \"Light-water-cooled reactor cores (PWR, BWR, light-water SMRs)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 4 Reactor; Ch. 5 Reactor Coolant System and Connected Systems (the LWR review plan)\" }, { document = \"10cfr50\", section = \"Appendix A, General Design Criteria for Nuclear Power Plants (water-cooled)\" }]\nnote = \"Added 2026-10-06 for the maintainer\'s corpus re-filing (6 papers: WASH-1400, NUREG-1465, RG 1.183, NuScale SER ch. 6 and ER, NUREG/KM-0004).\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/structural-materials\"\ntitle = \"Structural materials\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"4.5.1 Control Rod Drive Structural Materials; 4.5.2 Reactor Internal and Core Support Structure Materials; 5.2.3 Reactor Coolant Pressure Boundary Materials; 5.3.1 Reactor Vessel Materials; 6.1.1 Engineered Safety Features Materials\" }]\ncross_links = [\"02-nuclear-safety/control-rods-and-drives/control-rod-drive-structural-materials\", \"02-nuclear-safety/reactor-coolant-system/reactor-vessel-integrity\", \"02-nuclear-safety/engineered-safety-features/esf-materials\", \"02-nuclear-safety/msr-coolant-loop/coolant-loop-materials-and-chemistry\", \"02-nuclear-safety/moderator-and-reflector\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"08-radiation-protection/radiation-protection/dose-limits-and-criteria\"\ntitle = \"Dose limits and criteria (workers, public, design-basis accidents)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.210 Safety criteria for design-basis accidents (25 rem total effective dose equivalent); \u{a7} 53.020 Definitions (total effective dose equivalent); \u{a7} 53.270 Protection of plant workers (10 CFR part 20)\" }, { document = \"nureg-1555\", section = \"5.4.2 Radiation Doses to Members of the Public\" }]\ncross_links = [\"02-nuclear-safety/accident-analysis/licensing-basis-events/safety-criteria-for-design-basis-accidents\", \"13-environmental-protection/radiological-impacts-of-normal-operation/doses-to-members-of-the-public\", \"08-radiation-protection/radiation-protection/protection-of-plant-workers\"]\nnote = \"10 CFR part 20 (standards for protection against radiation: occupational and public dose limits) is the primary rule and is not in the corpus yet.\"\nstatus = \"approved\"\n";
```

## Module `lookup_review`

The PDF follow-up of identifier lookup (#756): field-by-field review of a
fetched record against an extracted document.
The PDF follow-up of identifier lookup (GitHub #756): after a PDF is
ingested and an identifier is found in it, kovan can OFFER to fill the
document's metadata from the record a lookup fetched. Nothing here goes
online and nothing is applied automatically:

```text
PDF --extract_metadata--> KovanDocument (extracted)
     \--identifiers_in_document--> Identifier --(user asks; caller fetches)--> ZoteroItem
propose(extracted, fetched, user_edited) --> LookupProposal: one row per field,
    extracted value | fetched value | Same / Fill / Replace / UserEdited / NothingFetched
user confirms some rows (or all) --> apply(doc, proposal, accepted) --> KovanDocument
    (only accepted rows change; UserEdited rows never; zotero_item = fetched item)
```

**Fields the user edited are never overwritten.** The caller says which
fields the user changed (the TUI review form knows: `ReviewState::is_edited`);
those rows are [`Decision::UserEdited`] and [`apply`] skips them even when
a caller passes them as accepted. KovanDocument has no persisted record of
edits, and none is added here (schema unchanged): a later lookup on a saved
document gets the same protection only for the fields its caller names.

**Schema:** the fetched item is stored in the existing optional
`KovanDocument.zotero_item`; no field is added, renamed or re-typed.

```rust
pub mod lookup_review { /* ... */ }
```

### Types

#### Enum `DocField`

A metadata field the proposal covers.

```rust
pub enum DocField {
    Title,
    Authors,
    Year,
    Doi,
    Journal,
    Institution,
    Publisher,
    Volume,
    Number,
    Pages,
    Abstract,
    Keywords,
    DocumentType,
}
```

##### Variants

###### `Title`

`title`.

###### `Authors`

`authors`.

###### `Year`

`year`.

###### `Doi`

`doi`.

###### `Journal`

`journal`.

###### `Institution`

`institution`.

###### `Publisher`

`publisher`.

###### `Volume`

`volume`.

###### `Number`

`number` (issue).

###### `Pages`

`pages`.

###### `Abstract`

`abstract_text`.

###### `Keywords`

`keywords`.

###### `DocumentType`

`document_type`.

##### Implementations

###### Methods

- ```rust
  pub fn name(self: Self) -> &'static str { /* ... */ }
  ```
  The `KovanDocument` field name.

- ```rust
  pub fn from_name(name: &str) -> Option<DocField> { /* ... */ }
  ```
  The field with this name (as [`DocField::name`]).

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
    fn clone(self: &Self) -> DocField { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &K) -> Ordering { /* ... */ }
    ```

  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &DocField) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DocField) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &DocField) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Enum `Decision`

What the proposal suggests for one field.

```rust
pub enum Decision {
    Same,
    Fill,
    Replace,
    NothingFetched,
    UserEdited,
}
```

##### Variants

###### `Same`

Both say the same: nothing to do.

###### `Fill`

The document has nothing; the record has a value.

###### `Replace`

Both have values and they differ.

###### `NothingFetched`

The record has no value: the document's is kept.

###### `UserEdited`

The user edited this field: never overwritten.

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
    fn clone(self: &Self) -> Decision { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &Decision) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `FieldProposal`

One row: a field, the document's value, the record's value, the
suggestion. Values are rendered as text for display.

```rust
pub struct FieldProposal {
    pub field: DocField,
    pub current: String,
    pub fetched: String,
    pub decision: Decision,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `field` | `DocField` | The field. |
| `current` | `String` | The document's current value ("" when empty). |
| `fetched` | `String` | The fetched record's value ("" when it has none). |
| `decision` | `Decision` | The suggestion. |

##### Implementations

###### Methods

- ```rust
  pub fn changes(self: &Self) -> bool { /* ... */ }
  ```
  Whether accepting this row would change the document.

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
    fn clone(self: &Self) -> FieldProposal { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &FieldProposal) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `LookupProposal`

The field-by-field comparison of a document with a fetched record.

```rust
pub struct LookupProposal {
    pub fields: Vec<FieldProposal>,
    pub fetched_item: kovan_common::zotero::ZoteroItem,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `fields` | `Vec<FieldProposal>` | One row per [`DocField::ALL`]. |
| `fetched_item` | `kovan_common::zotero::ZoteroItem` | The fetched item (stored in `zotero_item` on [`apply`]). |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn changes(self: &Self) -> impl Iterator<Item = &FieldProposal> { /* ... */ }
  ```
  The rows that would change the document if accepted.

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
    fn clone(self: &Self) -> LookupProposal { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &LookupProposal) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
### Functions

#### Function `identifiers_in_document`

The identifiers in an extracted document worth looking up: its DOI when
the extractor found one, then `extractIdentifiers` over the start of the
Markdown body (title page, abstract; the first 4000 characters, so a
reference list's identifiers are not mistaken for the document's own).

```rust
pub fn identifiers_in_document(doc: &kovan_common::KovanDocument) -> Vec<crate::zotero::search::Identifier> { /* ... */ }
```

#### Function `propose`

Compare `current` with the fetched record, field by field. `user_edited`
names the fields the user changed: those are [`Decision::UserEdited`].

```rust
pub fn propose(current: &kovan_common::KovanDocument, fetched: &kovan_common::zotero::ZoteroItem, user_edited: &[DocField]) -> LookupProposal { /* ... */ }
```

#### Function `apply`

Apply the rows the user accepted. Rows that are not
[`Decision::Fill`]/[`Decision::Replace`] are skipped whatever `accepted`
says (a user-edited field is never overwritten). The fetched item goes
into `zotero_item` when at least one row was accepted, or when
`keep_record` is set. The id, slug, source, body and every field not
accepted are left as they are.

```rust
pub fn apply(current: &kovan_common::KovanDocument, proposal: &LookupProposal, accepted: &[DocField], keep_record: bool) -> kovan_common::KovanDocument { /* ... */ }
```

#### Function `all_changes`

Every row that would change the document ("accept all").

```rust
pub fn all_changes(proposal: &LookupProposal) -> Vec<DocField> { /* ... */ }
```

## Module `zotero`

Zotero in kovan-literature (epic #747). The item model is
`kovan_common::zotero` (#748); this module holds what reads and writes
files.

| Module | What |
|---|---|
| [`framework`] | the translation framework (Zotero's translate API) the translators run in (#749) |
| [`translators`] | the import/export translators (#749), verified code-to-code against a running Zotero translation-server (#752; see `tests/zotero_translators.rs` and `scripts/zotero-reference.sh`) |
| [`search`] | identifier lookup, "Add Item by Identifier" (#756): `extractIdentifiers` and the search translators, network-free (the caller fetches; see the module docs), verified code-to-code on recorded responses (`tests/zotero_search.rs`, `scripts/zotero-search-reference.mjs`) |
| [`local_library`] | read a Zotero data folder (`zotero.sqlite` + `storage/`), or its database from bytes, and import it into kovan (#750). Pure Rust; every target, wasm32 and Android included (since 2026-10-07; before, native desktop only). |

```rust
pub mod zotero { /* ... */ }
```

### Modules

## Module `framework`

The translation framework: the parts of Zotero's translate API that
import and export translators call, and what the framework does around
them.

```text
import:  text --ImportInput--> translator --item.complete()--> _itemDone
              (Zotero.read)                                   (item_done)
         --> ImportResult.items (translator format)
         --> itemToAPIJSON (api_json) --> Web API JSON --> ZoteroItem

export:  Web API JSON --endpoint prep + ItemGetter (export_items)-->
         translator (Zotero.nextItem / Zotero.write) --> text
```

| Module | What | Upstream |
|---|---|---|
| [`item`] | [`TranslatorItem`], the sandbox `Zotero.Item`; [`JsObject`], an ordered JS object | translate.js `_makeSandboxItem` |
| [`io`] | `Zotero.read` (lines or n characters), `Zotero.write` | translate.js `IO.String` |
| [`options`] | translator headers, `getOption`, `getHiddenPref`, the environment | translate.js, translation-server |
| [`item_done`] | `item.complete()`: `_itemDone`, `_cleanTags`, `_cleanTitle` | translate.js |
| [`api_json`] | `itemToAPIJSON`, the deterministic key sequence, child-note folding | utilities_item.js, importEndpoint.js |
| [`export_items`] | the export endpoint's item preparation, `itemToLegacyExportFormat`, `ItemGetter` | exportEndpoint.js, translate_item.js, utilities_item.js |
| [`context`] | [`ImportContext`], [`ExportContext`], [`ImportResult`] | translate.js sandbox |
| [`utilities`] | `ZU.cleanAuthor`, `cleanDOI`, `trimInternal`, `text2html`, `removeDiacritics`, ... | utilities.js |
| [`html`] | `ZU.unescapeHTML` (HTML5 parse, then `textContent`) | utilities.js + the WHATWG parser |
| [`csl`] | `ZU.itemFromCSLJSON` into a translator item, `ZU.itemToCSLJSON` of an export item | utilities_item.js |
| [`js`] | JavaScript string semantics (whitespace, `ToString`, truthiness) | ECMA-262 |
| [`identifiers`] | `ZU.cleanISBN`, `ZU.cleanISSN` (#749) | utilities.js |
| [`title_case`] | `ZU.capitalizeTitle` as the translation-server runs it (#749) | utilities.js, utilities_translate.js |
| [`openurl`] | `ZU.createContextObject` (OpenURL 1.0 KEV, for COinS; #749), `ZU.parseContextObject` (XML ContextObject) | openurl.js |
| [`xml`] | the XML DOM the XML translators see (`Zotero.getXML`, `DOMParser`, node accessors, mutation, `getElementsByTagName(NS)`, `querySelectorAll`) (#749) | translate.js `parseDOMXML`, jsdom (WHATWG DOM) |
| [`xml_parse`] | XML parsing as jsdom drives saxes (well-formedness errors, entities, namespaces) | jsdom xml.js, saxes 6.0.0 |
| [`xml_serialize`] | `XMLSerializer.serializeToString`, XML `innerHTML` | w3c-xmlserializer 5.0.0 |
| [`xpath`] | `ZU.xpath`, `ZU.xpathText` over wicked-good-xpath's semantics, quirks included | wicked-good-xpath 1.3.1-z002, utilities.js |
| [`child`] | child translators (`Zotero.loadTranslator`, METS running MODS/MARCXML) | translate.js |
| [`html_dom`] | HTML documents for the note exporters: parsing (html5ever), `outerHTML`, selectors, `element.style` | jsdom, parse5 8.0.0 |
| [`rdf`] | `Zotero.RDF`: an RDF store, RDF/XML parser and serializer (#749) | translate's src/rdf/ |

**Adding a translator** is one module under `translators/` plus a
variant of [`super::translators::Translator`]: an `import` function over
an [`ImportContext`] and/or an `export` function over an
[`ExportContext`], and `detect_import` where upstream has
`detectImport`.

**Which upstream this follows.** The translators are ported from
translators commit 3d1c78530f42, which is what the reference
translation-server ran. ~~The framework follows the translation-server's
own submodules (translate e0fe482b8a07, utilities 1dd38e27edf8) wherever
they decide output, except that dates and CSL-JSON come from
kovan-common, which ports the newer utilities 4051881d59c6 (EDTF dates,
ranges, quoted literal dates). Where the two utilities versions disagree
the reference comparisons record the difference (see the tests in
`tests/zotero_translators.rs`).~~ **CORRECTED 2026-10-07** (#749): the
reference server now runs the submodule commits Zotero desktop
9cbba8c4d281 pins (translate dd524aea9a55, utilities 4051881d59c6,
zotero-schema b86c79b56479; `reference/manifest.json`), the versions
kovan-common ports, so there is no version skew left to record (see the
methodology in `tests/zotero_translators.rs`). File headers that cite
e0fe482b8a07 / 1dd38e27edf8 name the code read when they were written.

**Maturity: AI draft (1).** Not yet human-reviewed.

```rust
pub mod framework { /* ... */ }
```

### Modules

## Module `api_json`

Translator items to Zotero Web API JSON (`itemToAPIJSON`), which is what
the translation-server's `/import` returns and what
[`kovan_common::zotero::ZoteroItem`] reads.

```rust
pub mod api_json { /* ... */ }
```

### Types

#### Struct `KeyGenerator`

Where `itemToAPIJSON`'s item keys come from.

Upstream draws each key at random (`generateObjectKey`). This port is
deterministic: the n-th key is `"KVN"` followed by n written in base 33
over [`ALLOWED_KEY_CHARS`] (five digits), so the first is `KVN22222`.
The reference harness (`scripts/zotero-reference.mjs`, `normKey`)
rewrites upstream's random keys to the same sequence, in order of first
appearance, which is the only normalisation applied to upstream output.
A caller merging imports into one library should start each import at a
fresh index ([`super::TranslateOptions::first_key_index`]).

```rust
pub struct KeyGenerator {
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
  pub fn new(first: u64) -> Self { /* ... */ }
  ```
  Start at key number `first`.

- ```rust
  pub fn key(n: u64) -> String { /* ... */ }
  ```
  The n-th key.

- ```rust
  pub fn next_key(self: &mut Self) -> String { /* ... */ }
  ```
  The next key.

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
    fn clone(self: &Self) -> KeyGenerator { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &KeyGenerator) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
### Functions

#### Function `type_field_for_base`

`Zotero.ItemFields.getFieldIDFromTypeAndBase` as the translation
framework implements it (cachedTypes.js:154-171): the type-specific field
of `item_type` that maps to the base field `base`, or `None`. Unlike the
Zotero client's version (kovan-common's
[`field_from_type_and_base`](kovan_common::zotero::schema::field_from_type_and_base)),
a base field the type uses directly does not map to itself, and a field
that is not a base field gives `None`.

```rust
pub fn type_field_for_base(item_type: kovan_common::zotero::schema_generated::ItemType, base: kovan_common::zotero::schema_generated::Field) -> Option<kovan_common::zotero::schema_generated::Field> { /* ... */ }
```

#### Function `item_to_api_json`

`itemToAPIJSON` (utilities_item.js:850-988): one translator item (after
`_itemDone`) as Web API JSON items, the item first and then one child
`note` item per note.

`now_iso` is what an `accessDate` of `"CURRENT_TIMESTAMP"` becomes
(upstream: the current time, :973-975).

```rust
pub fn item_to_api_json(item: &super::item::TranslatorItem, keys: &mut KeyGenerator, now_iso: &str) -> Vec<serde_json::Value> { /* ... */ }
```

#### Function `fold_child_notes`

Fold child notes into their parents' `notes` arrays, as Zotero's export
format holds them (`itemToExportFormat`): each item whose `itemType` is
`note` and whose `parentItem` names an earlier item in the list moves into
that item's `notes`. The reference harness does the same before posting
import output to `/export` (`foldChildNotes` in
`scripts/zotero-reference.mjs`).

```rust
pub fn fold_child_notes(items: &[serde_json::Value]) -> Vec<serde_json::Value> { /* ... */ }
```

### Constants and Statics

#### Constant `ALLOWED_KEY_CHARS`

`Zotero.Utilities.allowedKeyChars`.

```rust
pub const ALLOWED_KEY_CHARS: &str = "23456789ABCDEFGHIJKLMNPQRSTUVWXYZ";
```

## Module `context`

What a translator sees while it runs: [`ImportContext`] and
[`ExportContext`], the Rust form of the `Zotero` object in upstream's
sandbox.

```rust
pub mod context { /* ... */ }
```

### Types

#### Enum `TranslateError`

Why a translation failed (upstream: the translator threw, or the
framework refused).

```rust
pub enum TranslateError {
    Unsupported {
        translator: &'static str,
        direction: &'static str,
    },
    Translator(String),
    BadExportInput(String),
}
```

##### Variants

###### `Unsupported`

The translator does not do this direction.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `translator` | `&'static str` | The translator's label. |
| `direction` | `&'static str` | "import" or "export". |

###### `Translator`

The translator raised an error (the message upstream would throw).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `BadExportInput`

The export input was not a JSON array of item objects (the
translation-server answers 400 "Input must be an array of items as
JSON").

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
    fn clone(self: &Self) -> TranslateError { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
- **Error**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(e: XPathError) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(e: TranslateError) -> Self { /* ... */ }
    ```

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TranslateError) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `ImportContext`

The `Zotero` object of an import translation.

```rust
pub struct ImportContext {
    pub input: super::io::ImportInput,
    pub options: super::options::TranslateOptions,
    pub meta: &'static super::options::TranslatorMetadata,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `input` | `super::io::ImportInput` | The input (`Zotero.read`). |
| `options` | `super::options::TranslateOptions` | The options. |
| `meta` | `&'static super::options::TranslatorMetadata` | The running translator's header. |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(input: &str, meta: &'static TranslatorMetadata, options: TranslateOptions) -> Self { /* ... */ }
  ```
  A context over `input`.

- ```rust
  pub fn read_line(self: &mut Self) -> Option<String> { /* ... */ }
  ```
  `Zotero.read()`: the next line.

- ```rust
  pub fn read_chars(self: &mut Self, n: usize) -> Option<String> { /* ... */ }
  ```
  `Zotero.read(n)`: the next `n` characters.

- ```rust
  pub fn get_option(self: &Self, name: &str) -> Option<&Value> { /* ... */ }
  ```
  `Zotero.getOption(name)`.

- ```rust
  pub fn get_hidden_pref(self: &Self, name: &str) -> Option<Value> { /* ... */ }
  ```
  `Zotero.getHiddenPref(name)`.

- ```rust
  pub fn in_child_translator(self: &Self) -> bool { /* ... */ }
  ```
  `Zotero.parentTranslator` is set.

- ```rust
  pub fn item_done(self: &mut Self, item: TranslatorItem) { /* ... */ }
  ```
  `item.complete()`: the framework's `_itemDone`, then the item saver.

- ```rust
  pub fn collection_done(self: &mut Self, collection: TranslatorCollection) { /* ... */ }
  ```
  `collection.complete()`.

- ```rust
  pub fn items(self: &Self) -> &[TranslatorItem] { /* ... */ }
  ```
  The items completed so far.

- ```rust
  pub fn items_mut(self: &mut Self) -> &mut [TranslatorItem] { /* ... */ }
  ```
  The items completed so far, mutable. Upstream saves items only when

- ```rust
  pub fn finish(self: Self) -> ImportResult { /* ... */ }
  ```
  Finish: the import's result.

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
    fn clone(self: &Self) -> ImportContext { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `ImportResult`

What an import produced.

```rust
pub struct ImportResult {
    pub items: Vec<super::item::TranslatorItem>,
    pub collections: Vec<super::item::TranslatorCollection>,
    pub first_key_index: u64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `items` | `Vec<super::item::TranslatorItem>` | The items as the framework saved them (after `_itemDone`), in<br>translator format. |
| `collections` | `Vec<super::item::TranslatorCollection>` | The collections the translator completed (the translation-server<br>drops these). |
| `first_key_index` | `u64` | The index of the first key [`ImportResult::api_json`] assigns. |

##### Implementations

###### Methods

- ```rust
  pub fn api_json_at(self: &Self, now_iso: &str) -> Vec<Value> { /* ... */ }
  ```
  The items as Zotero Web API JSON, as the translation-server's

- ```rust
  pub fn api_json(self: &Self) -> Vec<Value> { /* ... */ }
  ```
  [`ImportResult::api_json_at`] with the current UTC time (on targets

- ```rust
  pub fn zotero_items(self: &Self) -> Result<Vec<ZoteroItem>, ZoteroJsonError> { /* ... */ }
  ```
  The items as kovan-common [`ZoteroItem`]s (read from

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
    fn clone(self: &Self) -> ImportResult { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &ImportResult) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `ExportContext`

The `Zotero` object of an export translation.

```rust
pub struct ExportContext {
    pub output: super::io::ExportOutput,
    pub options: super::options::TranslateOptions,
    pub meta: &'static super::options::TranslatorMetadata,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `output` | `super::io::ExportOutput` | The output (`Zotero.write`). |
| `options` | `super::options::TranslateOptions` | The options. |
| `meta` | `&'static super::options::TranslatorMetadata` | The running translator's header. |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(items: &[JsObject], meta: &'static TranslatorMetadata, options: TranslateOptions) -> Self { /* ... */ }
  ```
  A context over items in Web API / export JSON (each a JSON object in

- ```rust
  pub fn next_item(self: &mut Self) -> Option<TranslatorItem> { /* ... */ }
  ```
  `Zotero.nextItem()`.

- ```rust
  pub fn set_collections(self: &mut Self, collections: Vec<JsObject>) { /* ... */ }
  ```
  Give the export the collections [`ExportContext::next_collection`]

- ```rust
  pub fn next_collection(self: &mut Self) -> Option<JsObject> { /* ... */ }
  ```
  `Zotero.nextCollection()` (translate.js:839-845): the next collection,

- ```rust
  pub fn write(self: &mut Self, data: &str) { /* ... */ }
  ```
  `Zotero.write(data)`.

- ```rust
  pub fn get_option(self: &Self, name: &str) -> Option<&Value> { /* ... */ }
  ```
  `Zotero.getOption(name)`.

- ```rust
  pub fn get_hidden_pref(self: &Self, name: &str) -> Option<Value> { /* ... */ }
  ```
  `Zotero.getHiddenPref(name)`.

- ```rust
  pub fn finish(self: Self) -> String { /* ... */ }
  ```
  The text written.

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
    fn clone(self: &Self) -> ExportContext { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
### Functions

#### Function `parse_export_input`

Parse export input: a JSON array of item objects, each kept in its own
key order (as `JSON.parse` does). The translation-server requires a
non-empty array whose first element has an `itemType`
(exportEndpoint.js:55-57).

```rust
pub fn parse_export_input(json: &str) -> Result<Vec<super::item::JsObject>, TranslateError> { /* ... */ }
```

#### Function `export_input_from_zotero_items`

Export input from kovan-common items (their JSON, key order sorted).

```rust
pub fn export_input_from_zotero_items(items: &[kovan_common::zotero::ZoteroItem]) -> Vec<super::item::JsObject> { /* ... */ }
```

## Module `csl`

`ZU.itemFromCSLJSON` and `ZU.itemToCSLJSON` as translators call them.

kovan-common's [`kovan_common::zotero::csl`] ports both for a Zotero
*library* item (`isZoteroItem`: values go through `setField` into the
type-specific field). A translator passes its sandbox item instead, and
upstream then writes raw values under the **base** field names
(`item[field] = cslItem[variable]`, :507) and leaves the base-to-type
mapping to `itemToAPIJSON`; [`item_from_csl_json`] ports that branch.
[`item_to_csl_json`] converts an export item to a
[`ZoteroItem`] and calls kovan-common's port.

```rust
pub mod csl { /* ... */ }
```

### Functions

#### Function `item_from_csl_json`

`ZU.itemFromCSLJSON(item, cslItem)` for a translator's item
(utilities_item.js:417-630, the `isZoteroItem == false` branch).

Omitted: `creator.creatorTypeID` (a numeric schema id, which nothing
downstream of a translator reads; `itemToAPIJSON` keeps only
`creatorType`).

```rust
pub fn item_from_csl_json(item: &mut super::item::TranslatorItem, csl_in: &kovan_common::zotero::csl::CslItem, opts: &kovan_common::zotero::date::DateOptions) -> Result<(), String> { /* ... */ }
```

#### Function `export_item_to_zotero_item`

A translator's export item as a kovan-common [`ZoteroItem`], for
`itemToCSLJSON`: string properties named like fields become fields (what
`field in zoteroItem` finds; upstream skips non-string text values);
creators with a known type keep their names (`name`, or `fieldMode: 1`
with a last name and no first name, is a literal; upstream skips unknown
creator types too, since they have no CSL mapping and cannot be the
primary type); `uri` and `note` carry over.

```rust
pub fn export_item_to_zotero_item(item: &super::item::TranslatorItem) -> Result<kovan_common::zotero::ZoteroItem, String> { /* ... */ }
```

#### Function `item_to_csl_json`

`ZU.itemToCSLJSON(item)` for an export item (kovan-common's port of
utilities 4051881d59c6).

```rust
pub fn item_to_csl_json(item: &super::item::TranslatorItem, opts: &kovan_common::zotero::date::DateOptions) -> Result<kovan_common::zotero::csl::CslItem, String> { /* ... */ }
```

## Module `export_items`

The items an export translator reads (`Zotero.nextItem()`), built from
the Web API / export-format JSON a caller supplies, the way the
translation-server's `/export` endpoint builds them.

```rust
pub mod export_items { /* ... */ }
```

### Functions

#### Function `item_to_legacy_export_format`

`itemToLegacyExportFormat` (utilities_item.js:996-1076).

The random `itemID` and `key` it assigns (`randomString(6)`) are replaced
by deterministic values: `itemID` is overwritten by the item getter's
counter anyway (translate_item.js:76), and `key` is set to `legacy_key`.

```rust
pub fn item_to_legacy_export_format(item: &mut super::item::TranslatorItem, legacy_key: &str) { /* ... */ }
```

#### Function `prepare_export_items`

The items an export translator will read, in order: the endpoint's
preparation (exportEndpoint.js:65-80), then `ItemGetter.nextItem`
(translate_item.js:50-78) and `Sandbox.Export.nextItem`
(translate.js:798-808), for every item.

* `legacy`: the translator's `minVersion` is below 4.0.27
  ([`super::TranslatorMetadata::legacy_export`]); dates go to SQL form
  and items to the legacy format. Upstream's endpoint, in legacy mode,
  writes the SQL form of `dateModified` into `dateAdded` (:76); ported as
  is.
* `export_tags`: the `exportTags` display option, when the translator
  declares it; `Some(false)` empties every item's tags.

```rust
pub fn prepare_export_items(items: &[super::item::JsObject], legacy: bool, export_tags: Option<bool>) -> Vec<super::item::TranslatorItem> { /* ... */ }
```

#### Function `legacy_key`

The deterministic stand-in for the random 6-character `key`
`itemToLegacyExportFormat` assigns: `"KVN"` and the item's 1-based index,
zero-padded to three digits.

```rust
pub fn legacy_key(index: usize) -> String { /* ... */ }
```

## Module `html`

`Zotero.Utilities.unescapeHTML`: the plain text of an HTML fragment, as
the translation-server computes it (jsdom's HTML parser, then
`textContent`).

Modelled, because they change the text: input newline normalisation
(CR LF and CR become LF); tags, comments, doctypes and bogus comments
(`<!...>`, `<?...>`, `</ ...>`) removed, including `>` inside quoted
attribute values; a `<` that does not start a tag kept as text; every
named character reference (the full WHATWG table, legacy forms without
`;` included, longest match) and numeric ones (with the C1 and invalid
code point replacements); RCDATA (`title`, `textarea`, entities decoded)
and raw text (`style`, `script`, `xmp`, `iframe`, `noembed`, `noframes`,
`plaintext`) elements, whose content is text; whitespace dropped at the
start of the document (the initial, "before html" and "before head"
insertion modes ignore it); the newline dropped after `<pre>`,
`<listing>` and `<textarea>`; NUL dropped in data and replaced in raw
text; the content of `<template>` (not part of `textContent`).

Not modelled (documented divergences, none seen in the reference
fixtures): foster parenting, which moves text that sits directly inside
a `<table>` before the table; the script-data escape states
(`<!--` inside `<script>`); a byte-order mark at the very start.

```rust
pub mod html { /* ... */ }
```

### Functions

#### Function `unescape_html`

`Zotero.Utilities.unescapeHTML(str)`.

```rust
pub fn unescape_html(s: &str) -> String { /* ... */ }
```

#### Function `text_content`

`documentElement.textContent` of `s` parsed as an HTML document.

```rust
pub fn text_content(s: &str) -> String { /* ... */ }
```

## Module `identifiers`

`ZU.cleanISBN` and `ZU.cleanISSN`.

Upstream matches with JavaScript regular expressions whose `\b` is an
ASCII word boundary and whose `\s` is JavaScript whitespace. The port
runs the same patterns over an ASCII stand-in of the text with one
character per character (JavaScript whitespace becomes a space, any
other non-ASCII character `!`, neither of which is a word character),
so `\b`, `\d` and `\s` mean what they mean upstream, and reads the match
back from the original text by position.

```rust
pub mod identifiers { /* ... */ }
```

### Functions

#### Function `clean_isbn`

`Zotero.Utilities.cleanISBN(isbnStr, dontValidate)` (:532-566): the first
ISBN-10 or ISBN-13 in the text whose check digit is valid (any, with
`dont_validate`), without spaces and dashes; `None` (upstream `false`)
when there is none.

```rust
pub fn clean_isbn(isbn_str: &str, dont_validate: bool) -> Option<String> { /* ... */ }
```

#### Function `clean_issn`

`Zotero.Utilities.cleanISSN(issnStr)` (:603-627): the first ISSN in the
text with a valid check digit, as `NNNN-NNNN`; `None` (upstream `false`)
when there is none.

```rust
pub fn clean_issn(issn_str: &str) -> Option<String> { /* ... */ }
```

#### Function `to_isbn13`

`Zotero.Utilities.toISBN13(isbnStr)` (:576-597, #756): the first ISBN in
the text (not validated) as ISBN-13 with its check digit recomputed;
`None` where upstream throws "ISBN not found".

```rust
pub fn to_isbn13(isbn_str: &str) -> Option<String> { /* ... */ }
```

## Module `openurl`

`ZU.createContextObject(item, version)`: an OpenURL ContextObject in
key-encoded-value form, for an export item (COinS export).

~~Not ported: `parseContextObject` (the reverse direction, used only by
COinS' web translator, which is out of scope)~~ **CORRECTED 2026-10-07**
(#749): XML ContextObject (an import translator) calls it too;
[`parse_context_object`] ports it (openurl.js:202-449). Not ported: the
`asObj` form of `createContextObject`, which no ported translator asks
for.

**Maturity: AI draft (1).**

```rust
pub mod openurl { /* ... */ }
```

### Types

#### Enum `OpenUrlVersion`

The OpenURL version `createContextObject` writes.

```rust
pub enum OpenUrlVersion {
    V0_1,
    V1_0,
}
```

##### Variants

###### `V0_1`

"0.1".

###### `V1_0`

"1.0".

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
    fn clone(self: &Self) -> OpenUrlVersion { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &OpenUrlVersion) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
### Functions

#### Function `encode_uri_component`

JavaScript `encodeURIComponent`: every UTF-8 byte of a character outside
`A-Z a-z 0-9 - _ . ! ~ * ' ( )` as `%XX` (upper-case hex).

```rust
pub fn encode_uri_component(s: &str) -> String { /* ... */ }
```

#### Function `first_creator`

`Zotero.Utilities.Item.getFirstCreatorFromItemJSON(json)`
(utilities_item.js:640-656): the first creator of the item type's primary
type or `author`, else the first `editor`.

```rust
pub fn first_creator(item: &super::TranslatorItem) -> Option<&super::TranslatorCreator> { /* ... */ }
```

#### Function `create_context_object`

`Zotero.Utilities.createContextObject(item, version)` (openurl.js:35-195)
on an export item, in string form. `dates` are the options of
`Zotero.Date.strToISO`.

```rust
pub fn create_context_object(item: &super::TranslatorItem, version: OpenUrlVersion, dates: &kovan_common::zotero::date::DateOptions) -> String { /* ... */ }
```

#### Function `parse_context_object`

`Zotero.OpenURL.parseContextObject(co, item)` (openurl.js:202-449,
utilities 4051881d59c6; `ZU.parseContextObject` in a translator), added
for XML ContextObject (#749): fill `item` from a key-encoded-value
ContextObject. `Ok(false)` where upstream returns false (no recognised
`rft_val_fmt`); `Err` where upstream throws (`decodeURIComponent`'s
URIError, a key without `=`, the duplicate check reading `.length` of an
undefined first name).

```rust
pub fn parse_context_object(co: &str, item: &mut super::TranslatorItem) -> Result<bool, super::TranslateError> { /* ... */ }
```

## Module `title_case`

`ZU.capitalizeTitle(string, force)`.

In a translator, `ZU.capitalizeTitle` without `force` reads the
`capitalizeTitles` preference, which the translation-server does not set
(its `Zotero.Prefs.get` reads the server config): so there it only
normalises whitespace and `" : "`. [`capitalize_title`] takes `force`
as the caller passes it, with `false` meaning that server behaviour.

Lengths and offsets count `char`s where upstream counts UTF-16 code
units; the two differ only on astral-plane characters.

```rust
pub mod title_case { /* ... */ }
```

### Functions

#### Function `capitalize_title`

`Zotero.Utilities.capitalizeTitle(string, force)` (:1067-1138) as a
translator on the translation-server runs it (module docs).

```rust
pub fn capitalize_title(string: &str, force: bool) -> String { /* ... */ }
```

## Module `html_entities`

**Attributes:**

- `Other("#[rustfmt::skip]")`

```rust
pub mod html_entities { /* ... */ }
```

### Constants and Statics

#### Constant `NAMED`

Every named character reference: (name, replacement).

```rust
pub const NAMED: &[(&str, &str)] = _;
```

#### Constant `MAX_NAME_LEN`

The longest name, in characters.

```rust
pub const MAX_NAME_LEN: usize = 32;
```

## Module `io`

Translator input and output: `Zotero.read` and `Zotero.write` over a
string (`Zotero.Translate.IO.String`).

Positions count `char`s, where upstream counts UTF-16 code units; the two
agree except on astral-plane characters (see [`super::js`]).

```rust
pub mod io { /* ... */ }
```

### Types

#### Struct `ImportInput`

The input of an import translator (`Zotero.read`).

```rust
pub struct ImportInput {
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
  pub fn new(text: &str) -> Self { /* ... */ }
  ```
  Input over a string (`new Zotero.Translate.IO.String(string)`).

- ```rust
  pub fn rewind(self: &mut Self) { /* ... */ }
  ```
  `init()`: rewind to the start (the framework does this between

- ```rust
  pub fn text(self: &Self) -> String { /* ... */ }
  ```
  The whole text.

- ```rust
  pub fn position(self: &Self) -> usize { /* ... */ }
  ```
  The number of characters consumed so far (`bytesRead`).

- ```rust
  pub fn read_chars(self: &mut Self, n: usize) -> Option<String> { /* ... */ }
  ```
  `Zotero.read(n)` (:2899-2903): the next `n` characters, or `None`

- ```rust
  pub fn read_line(self: &mut Self) -> Option<String> { /* ... */ }
  ```
  `Zotero.read()` (:2904-2930): the next line without its terminator, or

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
    fn clone(self: &Self) -> ImportInput { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `ExportOutput`

The output of an export translator (`Zotero.write`).

```rust
pub struct ExportOutput {
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
  pub fn new() -> Self { /* ... */ }
  ```
  Empty output.

- ```rust
  pub fn write(self: &mut Self, data: &str) { /* ... */ }
  ```
  `Zotero.write(data)` (:2933).

- ```rust
  pub fn as_str(self: &Self) -> &str { /* ... */ }
  ```
  What has been written.

- ```rust
  pub fn into_string(self: Self) -> String { /* ... */ }
  ```
  The written text, consuming the output.

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
    fn clone(self: &Self) -> ExportOutput { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> ExportOutput { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
## Module `item`

The item a translator builds or reads: upstream's sandbox `Zotero.Item`.

In JavaScript this is a plain object: the class declares `itemType`,
`creators`, `notes`, `tags`, `seeAlso` and `attachments` (in that order),
and translators then assign any property they like (`item.title`,
`item.backupPublisher`, `item.uniqueFields`, ...). [`TranslatorItem`]
keeps the six declared members typed and every other property, in
insertion order, in [`TranslatorItem::props`] as a JSON value — insertion
order matters, because the framework's `itemToAPIJSON` walks the
properties in that order and the first of two fields mapping to the same
slot wins.

```rust
pub mod item { /* ... */ }
```

### Types

#### Struct `JsObject`

An ordered JavaScript object: properties in insertion order, values as
JSON. Setting an existing property keeps its position; removing it and
setting it again moves it to the end, as in JavaScript.

```rust
pub struct JsObject(/* private field */);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `private` | *Private field* |

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  An empty object.

- ```rust
  pub fn get(self: &Self, key: &str) -> Option<&Value> { /* ... */ }
  ```
  The value of a property (`None` is `undefined`).

- ```rust
  pub fn get_mut(self: &mut Self, key: &str) -> Option<&mut Value> { /* ... */ }
  ```
  A mutable reference to the value of a property.

- ```rust
  pub fn get_str(self: &Self, key: &str) -> Option<&str> { /* ... */ }
  ```
  The value of a property when it is a string.

- ```rust
  pub fn truthy(self: &Self, key: &str) -> bool { /* ... */ }
  ```
  JavaScript truthiness of a property.

- ```rust
  pub fn contains(self: &Self, key: &str) -> bool { /* ... */ }
  ```
  Whether the property exists (`key in obj`).

- ```rust
  pub fn set</* synthetic */ impl Into<String>: Into<String>, /* synthetic */ impl Into<Value>: Into<Value>>(self: &mut Self, key: impl Into<String>, value: impl Into<Value>) { /* ... */ }
  ```
  Set a property (`obj[key] = value`).

- ```rust
  pub fn remove(self: &mut Self, key: &str) -> Option<Value> { /* ... */ }
  ```
  Remove a property (`delete obj[key]`), returning its value.

- ```rust
  pub fn iter(self: &Self) -> impl Iterator<Item = (&str, &Value)> { /* ... */ }
  ```
  The properties in order.

- ```rust
  pub fn keys(self: &Self) -> impl Iterator<Item = &str> { /* ... */ }
  ```
  The property names in order.

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  The number of properties.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether there are no properties.

- ```rust
  pub fn into_entries(self: Self) -> Vec<(String, Value)> { /* ... */ }
  ```
  The properties, consuming the object.

- ```rust
  pub fn from_map(map: &Map<String, Value>) -> Self { /* ... */ }
  ```
  From a JSON object (whose keys `serde_json` keeps sorted; use

- ```rust
  pub fn to_value(self: &Self) -> Value { /* ... */ }
  ```
  As a JSON object (key order is lost: `serde_json` sorts).

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
    fn clone(self: &Self) -> JsObject { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> JsObject { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<D: Deserializer<''de>>(d: D) -> Result<Self, <D as >::Error> { /* ... */ }
    ```

- **DeserializeOwned**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **FromIterator**
  - ```rust
    fn from_iter<T: IntoIterator<Item = (String, Value)>>(iter: T) -> Self { /* ... */ }
    ```

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &JsObject) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `TranslatorCreator`

A creator as translators handle it: `{firstName, lastName, creatorType,
fieldMode}`; anything else a translator sets (e.g. `creatorTypeID` from
`itemFromCSLJSON`) is kept in `other`.

```rust
pub struct TranslatorCreator {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub creator_type: Option<String>,
    pub field_mode: Option<i64>,
    pub other: JsObject,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `first_name` | `Option<String>` | `firstName`. |
| `last_name` | `Option<String>` | `lastName`. |
| `creator_type` | `Option<String>` | `creatorType`. |
| `field_mode` | `Option<i64>` | `fieldMode` (1 = single-field name). |
| `other` | `JsObject` | Other properties, in order. |

##### Implementations

###### Methods

- ```rust
  pub fn new</* synthetic */ impl Into<String>: Into<String>, /* synthetic */ impl Into<String>: Into<String>, /* synthetic */ impl Into<String>: Into<String>>(first_name: impl Into<String>, last_name: impl Into<String>, creator_type: impl Into<String>) -> Self { /* ... */ }
  ```
  A two-field creator.

- ```rust
  pub fn single</* synthetic */ impl Into<String>: Into<String>, /* synthetic */ impl Into<String>: Into<String>>(last_name: impl Into<String>, creator_type: impl Into<String>) -> Self { /* ... */ }
  ```
  A single-field creator (`fieldMode: 1`).

- ```rust
  pub fn from_value(v: &Value) -> Self { /* ... */ }
  ```
  From a JSON creator object (`name` is kept in `other`; the export

- ```rust
  pub fn to_value(self: &Self) -> Value { /* ... */ }
  ```
  As a JSON object.

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
    fn clone(self: &Self) -> TranslatorCreator { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> TranslatorCreator { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &TranslatorCreator) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `TranslatorTag`

A tag: what a translator pushes (a string, or `{tag, type}`).

```rust
pub struct TranslatorTag {
    pub tag: String,
    pub tag_type: Option<i64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `tag` | `String` | The tag text. |
| `tag_type` | `Option<i64>` | `type`: 0 manual, 1 automatic; `None` when not given. |

##### Implementations

###### Methods

- ```rust
  pub fn new</* synthetic */ impl Into<String>: Into<String>>(tag: impl Into<String>) -> Self { /* ... */ }
  ```
  A tag with no type (a translator pushing a plain string).

- ```rust
  pub fn from_value(v: &Value) -> Option<Self> { /* ... */ }
  ```
  From a JSON tag (a string, or an object with `tag` or `name`).

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
    fn clone(self: &Self) -> TranslatorTag { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &TranslatorTag) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `TranslatorNote`

A note: `note` is its HTML; on export, the rest of the child note's JSON
(`key`, `tags`, ...) is in `props`.

```rust
pub struct TranslatorNote {
    pub note: String,
    pub props: JsObject,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `note` | `String` | The note's HTML. |
| `props` | `JsObject` | Other properties, in order. |

##### Implementations

###### Methods

- ```rust
  pub fn new</* synthetic */ impl Into<String>: Into<String>>(note: impl Into<String>) -> Self { /* ... */ }
  ```
  A note with only text.

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
    fn clone(self: &Self) -> TranslatorNote { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> TranslatorNote { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &TranslatorNote) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `TranslatorItem`

A translator's item (upstream's sandbox `Zotero.Item`; module docs).

```rust
pub struct TranslatorItem {
    pub item_type: String,
    pub creators: Vec<TranslatorCreator>,
    pub notes: Vec<TranslatorNote>,
    pub tags: Vec<TranslatorTag>,
    pub see_also: Vec<serde_json::Value>,
    pub attachments: Vec<JsObject>,
    pub props: JsObject,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `item_type` | `String` | `itemType` (a string: translators may set a type the schema lacks;<br>`itemToAPIJSON` turns an unknown one into `webpage`). |
| `creators` | `Vec<TranslatorCreator>` | `creators`. |
| `notes` | `Vec<TranslatorNote>` | `notes`. |
| `tags` | `Vec<TranslatorTag>` | `tags`. |
| `see_also` | `Vec<serde_json::Value>` | `seeAlso`. |
| `attachments` | `Vec<JsObject>` | `attachments`, each a plain object (`{path, mimeType, title, url, ...}`). |
| `props` | `JsObject` | Every other property, in insertion order. |

##### Implementations

###### Methods

- ```rust
  pub fn new</* synthetic */ impl Into<String>: Into<String>>(item_type: impl Into<String>) -> Self { /* ... */ }
  ```
  `new Zotero.Item(itemType)`.

- ```rust
  pub fn get(self: &Self, key: &str) -> Option<&Value> { /* ... */ }
  ```
  A property's value.

- ```rust
  pub fn get_str(self: &Self, key: &str) -> Option<&str> { /* ... */ }
  ```
  A property's value when it is a string.

- ```rust
  pub fn get_string(self: &Self, key: &str) -> Option<String> { /* ... */ }
  ```
  A property as JavaScript would concatenate it (`"" + item[key]`), or

- ```rust
  pub fn truthy(self: &Self, key: &str) -> bool { /* ... */ }
  ```
  JavaScript truthiness of a property.

- ```rust
  pub fn set</* synthetic */ impl Into<String>: Into<String>, /* synthetic */ impl Into<Value>: Into<Value>>(self: &mut Self, key: impl Into<String>, value: impl Into<Value>) { /* ... */ }
  ```
  `item[key] = value`.

- ```rust
  pub fn remove(self: &mut Self, key: &str) -> Option<Value> { /* ... */ }
  ```
  `delete item[key]`.

- ```rust
  pub fn set_extra(self: &mut Self, field: &str, value: &str) { /* ... */ }
  ```
  `Zotero.Item#setExtra(field, value)` (translate.js:1952-1962): replace

- ```rust
  pub fn from_js_object(o: &JsObject) -> Self { /* ... */ }
  ```
  From a JSON object in its key order (an item of the translator export

- ```rust
  pub fn to_value(self: &Self) -> Value { /* ... */ }
  ```
  As a JSON object in translator format (the shape of upstream's test

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
    fn clone(self: &Self) -> TranslatorItem { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> TranslatorItem { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &TranslatorItem) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Enum `CollectionChild`

One member of a [`TranslatorCollection`].

```rust
pub enum CollectionChild {
    Item {
        id: String,
    },
    Collection(TranslatorCollection),
}
```

##### Variants

###### `Item`

`{type: 'item', id}`: an item, by the id the translator gave it.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `id` | `String` | The item's id (a BibTeX citation key, for instance). |

###### `Collection`

A subcollection.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `TranslatorCollection` |  |

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
    fn clone(self: &Self) -> CollectionChild { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &CollectionChild) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `TranslatorCollection`

A collection a translator builds (`new Zotero.Collection()`,
translate.js:1970-1981). The translation-server discards collections
(its `ItemSaver.saveCollection` is a no-op); they are returned here so a
caller can keep them.

```rust
pub struct TranslatorCollection {
    pub name: String,
    pub children: Vec<CollectionChild>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | `name`. |
| `children` | `Vec<CollectionChild>` | `children`, in order. |

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
    fn clone(self: &Self) -> TranslatorCollection { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> TranslatorCollection { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &TranslatorCollection) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
## Module `item_done`

What the framework does to an item when a translator calls
`item.complete()`.

```rust
pub mod item_done { /* ... */ }
```

### Functions

#### Function `clean_title`

`_cleanTitle` (:1551-1556): drop a trailing ": a novel" from a book or
book section title.

```rust
pub fn clean_title(title: &str, item_type: &str) -> String { /* ... */ }
```

#### Function `clean_tags`

`_cleanTags` (:1563-1581) on typed tags: drop empty ones, and a type
that is not truthy (`if(tag.type) newTag.type = tag.type`).

```rust
pub fn clean_tags(tags: &[super::item::TranslatorTag]) -> Vec<super::item::TranslatorTag> { /* ... */ }
```

#### Function `clean_tags_value`

`_cleanTags` on a JSON array (attachments' and notes' `tags`).

```rust
pub fn clean_tags_value(tags: &serde_json::Value) -> serde_json::Value { /* ... */ }
```

#### Function `item_done`

Port of `_itemDone` (:83-237) for a top-level import translation whose
items are saved (the translation-server passes `libraryID: 1`): returns
the item as the framework hands it to the item saver.

Omitted, because nothing observable depends on them: the random `id`
(:186; `itemToAPIJSON` drops it), the deprecation debug messages, and the
connector-only conversion of `attachment.document` (:165-169), which no
import translator sets.

```rust
pub fn item_done(item: super::item::TranslatorItem, in_child_translator: bool) -> super::item::TranslatorItem { /* ... */ }
```

#### Function `item_done_with`

`_itemDone` with the saving branch explicit: `saved` false is a
translation run with `libraryID: false` (the translation-server's
`/search`, #756), where `_itemDone` returns the item at :178-183, before
the note, `version` and `accessDate` steps, exactly as it does in a child
translator.

```rust
pub fn item_done_with(item: super::item::TranslatorItem, in_child_translator: bool, saved: bool) -> super::item::TranslatorItem { /* ... */ }
```

## Module `js`

JavaScript semantics the translator port depends on.

Rust and JavaScript disagree on small things that change output:

* **Whitespace.** JavaScript's `\s` and `String.prototype.trim` use
  ECMA-262 WhiteSpace + LineTerminator, which includes U+FEFF and excludes
  U+0085; Rust's `char::is_whitespace`, `str::trim` and the `regex` crate's
  Unicode `\s` use Unicode `White_Space`, which is the other way round.
  Use [`is_space`], [`trim`] and [`WS`] (a regex class) instead.
* **`\w` and `\b`** are ASCII in JavaScript and Unicode in `regex`. Write
  `[A-Za-z0-9_]` ([`WORD`]) for `\w` and `(?-u:\b)` for `\b`.
* **String units.** JavaScript indexes UTF-16 code units; this port
  indexes Unicode scalar values (`char`s). The two agree on every
  character in the Basic Multilingual Plane, i.e. everything except
  astral-plane characters (emoji, some CJK extensions), where a JavaScript
  `length`, `substr` or `read(n)` counts 2 and this port counts 1.

```rust
pub mod js { /* ... */ }
```

### Functions

#### Function `is_space`

Whether `c` matches JavaScript's `\s`.

```rust
pub fn is_space(c: char) -> bool { /* ... */ }
```

#### Function `is_word_char`

Whether `c` matches JavaScript's `\w`.

```rust
pub fn is_word_char(c: char) -> bool { /* ... */ }
```

#### Function `trim`

`String.prototype.trim`.

```rust
pub fn trim(s: &str) -> &str { /* ... */ }
```

#### Function `trim_start`

`String.prototype.trimStart`.

```rust
pub fn trim_start(s: &str) -> &str { /* ... */ }
```

#### Function `trim_end`

`String.prototype.trimEnd`.

```rust
pub fn trim_end(s: &str) -> &str { /* ... */ }
```

#### Function `truthy`

JavaScript truthiness of a JSON value (`undefined` is `None`).

```rust
pub fn truthy(v: Option<&serde_json::Value>) -> bool { /* ... */ }
```

#### Function `to_js_string`

JavaScript `ToString` of a JSON value: what `val.toString()` or `"" + val`
gives (arrays join with ",", objects are `[object Object]`).

```rust
pub fn to_js_string(v: &serde_json::Value) -> String { /* ... */ }
```

#### Function `number_to_string`

`Number.prototype.toString` for the numbers JSON carries: integers print
without a decimal point, as in JavaScript.

```rust
pub fn number_to_string(n: &serde_json::Number) -> String { /* ... */ }
```

#### Function `substr`

`str.substr(start, len)` over chars, with JavaScript's clamping.

```rust
pub fn substr(s: &str, start: usize, len: usize) -> String { /* ... */ }
```

#### Function `len`

The number of units JavaScript's `length` would report, counted in chars
(see the module docs on string units).

```rust
pub fn len(s: &str) -> usize { /* ... */ }
```

### Constants and Statics

#### Constant `WS`

A regex character class equal to JavaScript's `\s` (ECMA-262 WhiteSpace
and LineTerminator).

```rust
pub const WS: &str = r"[\t\n\x0B\x0C\r \x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}\x{FEFF}]";
```

#### Constant `NOT_WS`

A regex character class equal to JavaScript's `\S`.

```rust
pub const NOT_WS: &str = r"[^\t\n\x0B\x0C\r \x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}\x{FEFF}]";
```

#### Constant `WORD`

A regex character class equal to JavaScript's `\w`.

```rust
pub const WORD: &str = "[A-Za-z0-9_]";
```

## Module `options`

Translator metadata (the JSON header of each translator file) and the
options a translation runs with.

```rust
pub mod options { /* ... */ }
```

### Modules

## Module `translator_type`

`translatorType` bits.

```rust
pub mod translator_type { /* ... */ }
```

### Constants and Statics

#### Constant `IMPORT`

Import.

```rust
pub const IMPORT: u8 = 1;
```

#### Constant `EXPORT`

Export.

```rust
pub const EXPORT: u8 = 2;
```

#### Constant `WEB`

Web.

```rust
pub const WEB: u8 = 4;
```

#### Constant `SEARCH`

Search.

```rust
pub const SEARCH: u8 = 8;
```

### Types

#### Enum `HeaderValue`

A default value in a translator header (`displayOptions`,
`hiddenPrefs`, `configOptions`).

```rust
pub enum HeaderValue {
    Bool(bool),
    Str(&'static str),
}
```

##### Variants

###### `Bool`

A boolean.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `bool` |  |

###### `Str`

A string.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&'static str` |  |

##### Implementations

###### Methods

- ```rust
  pub fn to_value(self: Self) -> Value { /* ... */ }
  ```
  As JSON.

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
    fn clone(self: &Self) -> HeaderValue { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &HeaderValue) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `TranslatorMetadata`

A translator's header, verbatim from its file.

```rust
pub struct TranslatorMetadata {
    pub id: &'static str,
    pub label: &'static str,
    pub creator: &'static str,
    pub target: &'static str,
    pub min_version: &'static str,
    pub priority: u32,
    pub translator_type: u8,
    pub config_options: &'static [(&'static str, HeaderValue)],
    pub display_options: &'static [(&'static str, HeaderValue)],
    pub hidden_prefs: &'static [(&'static str, HeaderValue)],
    pub last_updated: &'static str,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `id` | `&'static str` | `translatorID`. |
| `label` | `&'static str` | `label`. |
| `creator` | `&'static str` | `creator`. |
| `target` | `&'static str` | `target` (for import/export translators, the file extension). |
| `min_version` | `&'static str` | `minVersion`. |
| `priority` | `u32` | `priority` (lower is tried first). |
| `translator_type` | `u8` | `translatorType` ([`translator_type`] bits). |
| `config_options` | `&'static [(&'static str, HeaderValue)]` | `configOptions`. |
| `display_options` | `&'static [(&'static str, HeaderValue)]` | `displayOptions`: the options and their defaults. |
| `hidden_prefs` | `&'static [(&'static str, HeaderValue)]` | `hiddenPrefs`: defaults for `Zotero.getHiddenPref`. |
| `last_updated` | `&'static str` | `lastUpdated`. |

##### Implementations

###### Methods

- ```rust
  pub fn can_import(self: &Self) -> bool { /* ... */ }
  ```
  Whether the translator imports.

- ```rust
  pub fn can_export(self: &Self) -> bool { /* ... */ }
  ```
  Whether the translator exports.

- ```rust
  pub fn legacy_export(self: &Self) -> bool { /* ... */ }
  ```
  Whether the framework gives this translator the legacy (pre-4.0.27)

- ```rust
  pub fn default_display_options(self: &Self) -> JsObject { /* ... */ }
  ```
  The display options with their defaults (what `getOption` returns when

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
    fn clone(self: &Self) -> TranslatorMetadata { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &TranslatorMetadata) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `TranslationEnv`

What the translation environment supplies besides the translator's own
options. The defaults are the Zotero translation-server's: version
"5.0.97" (`Zotero.version`, translation-server zotero.js:35), en-US dates
without the client's day suffixes, UTC.

```rust
pub struct TranslationEnv {
    pub zotero_version: String,
    pub dates: kovan_common::zotero::date::DateOptions,
    pub parent_translator: Option<String>,
    pub now_unix_secs: Option<i64>,
    pub first_blank_node_id: u64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `zotero_version` | `String` | `Zotero.Utilities.getVersion()`. |
| `dates` | `kovan_common::zotero::date::DateOptions` | Date parsing and the "local" time zone (kovan-common's<br>[`DateOptions`]). |
| `parent_translator` | `Option<String>` | `Zotero.parentTranslator` is set: the translation runs inside another<br>translator. Some translators change behaviour (BibTeX splits keywords<br>on spaces, unescapes HTML entities). `None` at top level. |
| `now_unix_secs` | `Option<i64>` | "Now", in seconds since the Unix epoch, for translators that read the<br>clock (`new Date()`; RIS import of an access date without a time).<br>`None`: the system clock (the epoch on targets without one). |
| `first_blank_node_id` | `u64` | The id the RDF library gives the next blank node (#749). Upstream's<br>counter (`Term.NextId`, translate src/rdf/term.js) is global to the<br>process, so a long-running Zotero hands out ids that depend on what it<br>did before; the ids show in RDF/XML output (`rdf:nodeID="n42"`), and<br>their length decides upstream's line packing. 0 by default. |

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
    fn clone(self: &Self) -> TranslationEnv { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &TranslationEnv) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `TranslateOptions`

The options a translation runs with.

```rust
pub struct TranslateOptions {
    pub display: super::item::JsObject,
    pub hidden_prefs: super::item::JsObject,
    pub env: TranslationEnv,
    pub first_key_index: u64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `display` | `super::item::JsObject` | Display options (`Zotero.getOption`). Start from<br>[`TranslatorMetadata::default_display_options`]. |
| `hidden_prefs` | `super::item::JsObject` | Hidden preferences (`Zotero.getHiddenPref`), overriding the header's<br>`hiddenPrefs` defaults (upstream reads `translators.<name>` prefs). |
| `env` | `TranslationEnv` | The environment. |
| `first_key_index` | `u64` | The first key `itemToAPIJSON` assigns (import); keys count up from<br>here. See [`super::api_json::KeyGenerator`]. |

##### Implementations

###### Methods

- ```rust
  pub fn for_translator(meta: &TranslatorMetadata) -> Self { /* ... */ }
  ```
  A translator's default options in the default environment.

- ```rust
  pub fn get_option(self: &Self, name: &str) -> Option<&Value> { /* ... */ }
  ```
  `Zotero.getOption(name)`.

- ```rust
  pub fn option_truthy(self: &Self, name: &str) -> bool { /* ... */ }
  ```
  JavaScript truthiness of `Zotero.getOption(name)`.

- ```rust
  pub fn get_hidden_pref(self: &Self, meta: &TranslatorMetadata, name: &str) -> Option<Value> { /* ... */ }
  ```
  `Zotero.getHiddenPref(name)`: the caller's value, else the header

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
    fn clone(self: &Self) -> TranslateOptions { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &TranslateOptions) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
### Functions

#### Function `semver_compare`

`Zotero.Utilities.semverCompare(a, b)` (utilities.js:1719-1727): split
on ".", compare components pairwise (as integers when `parseInt` reads
one, else as strings; a number against a string compares false both ways
in JavaScript, so they count as equal), and when every shared component
is equal the version with more components is greater.

```rust
pub fn semver_compare(a: &str, b: &str) -> std::cmp::Ordering { /* ... */ }
```

## Module `rdf`

RDF for the translators whose `dataMode` is `rdf/xml` (Zotero RDF, RDF,
Bibliontology RDF, Unqualified Dublin Core RDF).

```text
import:  text --dom (framework::xml)--> DOM --parser (rdfparser.js)--> Store
         translator <--Zotero.RDF (sandbox)--> Store
export:  translator --Zotero.RDF--> Store --serializer (serialize.js)--> text
```

| Module | What | Upstream |
|---|---|---|
| [`term`] | terms (symbol, blank node, literal, collection) and their order | term.js |
| [`store`] | `IndexedFormula`: statements, indexes, `owl:sameAs` smushing | identity.js |
| [`dom`] | the XML DOM the parser walks (a mutable copy of the framework XML layer's parse) | translate.js `parseDOMXML` + jsdom |
| [`parser`] | RDF/XML to triples | rdfparser.js |
| [`serializer`] | triples to RDF/XML, byte for byte | serialize.js `statementsToXML` |
| [`uri`] | `Util.uri.join` | uri.js |
| [`sandbox`] | `Zotero.RDF`: `getTargets`, `getStatementsMatching`, `addStatement`, ... | translate.js `_RDFSandbox` |

**Why a port and not an RDF crate.** The translators' output depends on
this library's exact behaviour: statement order (insertion order, which
decides the order of items, tags and creators on import, and of elements
on export), which blank nodes nest, the serializer's line packing, and
the parser's quirks. A general RDF crate would be correct RDF and a
different answer.

**Blank node ids** come from a per-store counter starting at 0; upstream's
is global to the server process, so its ids differ run to run. Only their
order is meaningful, and the reference comparisons renumber `rdf:nodeID`s
and `_:n` names by first appearance on both sides (`tests/zotero_translators.rs`).

**Maturity: AI draft (1).** Not yet human-reviewed.

```rust
pub mod rdf { /* ... */ }
```

### Modules

## Module `dom`

The XML DOM the RDF parser walks: what `parseDOMXML` gives it.

Upstream's RDF data mode parses with the same `parseDOMXML` as the XML
translators' `Zotero.getXML()`, so the document comes from the framework's
XML layer ([`super::super::xml::XmlDocument::parse`]: saxes as jsdom
drives it, the `parsererror` check, `normalize()`). This module copies it
into a small arena the RDF parser can mutate as upstream's does (it
removes attributes as it consumes them): element namespaces, prefixes and
local names; attributes in document order, `xmlns` declarations included
(the parser registers and removes them, as upstream's `buildFrame` does);
text, CDATA, comments and processing instructions as nodes of their own,
since they count in `childNodes.length`.

~~This module built its DOM with xml-rs until the XML layer existed.~~
**CHANGED 2026-10-07** (merge with develop dbb9e26eb1): the XML layer is
the one parser; the RDF references were identical with both.

```rust
pub mod dom { /* ... */ }
```

### Types

#### Type Alias `NodeId`

A DOM node's index in [`Dom::nodes`].

```rust
pub type NodeId = usize;
```

#### Struct `Attr`

An attribute.

```rust
pub struct Attr {
    pub namespace: Option<String>,
    pub local_name: String,
    pub prefix: Option<String>,
    pub value: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `namespace` | `Option<String>` | `namespaceURI` (`None` is `null`). |
| `local_name` | `String` | `localName`. |
| `prefix` | `Option<String>` | `prefix`. |
| `value` | `String` | `nodeValue`. |

##### Implementations

###### Methods

- ```rust
  pub fn node_name(self: &Self) -> String { /* ... */ }
  ```
  `nodeName` / `name`: the qualified name.

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
    fn clone(self: &Self) -> Attr { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &Attr) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Enum `NodeKind`

What a node is.

```rust
pub enum NodeKind {
    Document,
    Element {
        namespace: Option<String>,
        local_name: String,
        prefix: Option<String>,
        attrs: Vec<Attr>,
    },
    Text(String),
    CData(String),
    ProcessingInstruction,
    Comment,
    DocumentType,
}
```

##### Variants

###### `Document`

The document (nodeType 9).

###### `Element`

An element (nodeType 1).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `namespace` | `Option<String>` | `namespaceURI`. |
| `local_name` | `String` | `localName`. |
| `prefix` | `Option<String>` | `prefix`. |
| `attrs` | `Vec<Attr>` | `attributes`, in document order (mutable: the parser removes them). |

###### `Text`

A text node (nodeType 3).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `CData`

A CDATA section (nodeType 4).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `ProcessingInstruction`

A processing instruction (nodeType 7).

###### `Comment`

A comment (nodeType 8).

###### `DocumentType`

A document type (nodeType 10).

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
    fn clone(self: &Self) -> NodeKind { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &NodeKind) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `DomNode`

A node and its children.

```rust
pub struct DomNode {
    pub kind: NodeKind,
    pub children: Vec<NodeId>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `kind` | `NodeKind` | What it is. |
| `children` | `Vec<NodeId>` | `childNodes`. |

##### Implementations

###### Methods

- ```rust
  pub fn node_type(self: &Self) -> u8 { /* ... */ }
  ```
  `nodeType`.

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
    fn clone(self: &Self) -> DomNode { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &DomNode) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `Dom`

A parsed document; node 0 is the document node.

```rust
pub struct Dom {
    pub nodes: Vec<DomNode>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nodes` | `Vec<DomNode>` | Every node. |

##### Implementations

###### Methods

- ```rust
  pub fn parse(input: &str) -> Result<Dom, String> { /* ... */ }
  ```
  `parseDOMXML(input)` ([`XmlDocument::parse`]), copied into the arena.

- ```rust
  pub fn node(self: &Self, id: NodeId) -> &DomNode { /* ... */ }
  ```
  The node.

- ```rust
  pub fn attrs(self: &Self, id: NodeId) -> &[Attr] { /* ... */ }
  ```
  The attributes of an element (empty for any other node).

- ```rust
  pub fn attrs_mut(self: &mut Self, id: NodeId) -> Option<&mut Vec<Attr>> { /* ... */ }
  ```
  The attributes of an element, mutably.

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
    fn clone(self: &Self) -> Dom { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &Dom) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
## Module `parser`

RDF/XML into a [`Store`]: upstream's frame machine, ported step for step,
quirks included (the parser is the specification of what Zotero reads).

Quirks worth knowing, all upstream's:
* text is only a literal when it is an element's only child; text beside
  other nodes (including comments) is skipped;
* a typed node element that also has an `rdf:type` attribute keeps the
  attribute, which then becomes a literal-valued property;
* `rdf:parseType="Literal"` stores the literal `[object Element]` (the
  element's `toString()`), not its markup;
* after an `rdf:parseType="Resource"` or `"Collection"` arc, or a
  collection member, the walk resumes from the arc's frame, so later
  sibling properties hang off that frame (`pframe` in `parseDOM`).

```rust
pub mod parser { /* ... */ }
```

### Functions

#### Function `parse`

`RDFParser.parse(document, base)`: add the document's triples to `store`.

```rust
pub fn parse(dom: &mut super::dom::Dom, store: &mut super::store::Store, base: &str) -> Result<(), String> { /* ... */ }
```

## Module `sandbox`

`Zotero.RDF`: the API RDF translators use, over a [`Store`].

JavaScript values cross this API loosely typed; here a resource argument
is a [`Res`] (a URI string, a term object, or one of the non-objects
translators sometimes pass: `undefined`, `false`, `null`) and a value
coming back is an [`RdfValue`] (a literal comes back as a string, any
other term as the term).

As upstream, a string argument becomes a new symbol (`_getResource`), and
`undefined` or `false` become the symbols `<undefined>` / `<false>`
(`new Symbol(undefined)`): they find nothing unless a document names a
resource "undefined" or "false", where upstream's `uri` comparison
(`undefined == "undefined"` is false) would still find nothing on two-part
patterns. That corner is not reproduced. `null` is a wildcard, as in
upstream's `statementsMatching`.

```rust
pub mod sandbox { /* ... */ }
```

### Types

#### Enum `Res`

A resource argument (see the module docs).

```rust
pub enum Res {
    Uri(String),
    Node(super::term::Node),
    Undefined,
    False,
    Null,
}
```

##### Variants

###### `Uri`

A URI string: becomes a new symbol.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Node`

A term object.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::term::Node` |  |

###### `Undefined`

`undefined`.

###### `False`

`false`.

###### `Null`

`null`.

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
    fn clone(self: &Self) -> Res { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(s: &str) -> Res { /* ... */ }
    ```

  - ```rust
    fn from(s: String) -> Res { /* ... */ }
    ```

  - ```rust
    fn from(s: &String) -> Res { /* ... */ }
    ```

  - ```rust
    fn from(n: Node) -> Res { /* ... */ }
    ```

  - ```rust
    fn from(n: &Node) -> Res { /* ... */ }
    ```

  - ```rust
    fn from(v: &RdfValue) -> Res { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Enum `RdfValue`

A value returned to a translator: a literal as a string, any other term
as an object.

```rust
pub enum RdfValue {
    Str(String),
    Node(super::term::Node),
}
```

##### Variants

###### `Str`

A string (a literal's value).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Node`

A term object.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::term::Node` |  |

##### Implementations

###### Methods

- ```rust
  pub fn as_str(self: &Self) -> Option<&str> { /* ... */ }
  ```
  The string, if a string (`typeof x == "string"`).

- ```rust
  pub fn as_node(self: &Self) -> Option<&Node> { /* ... */ }
  ```
  The node, if an object.

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
    fn clone(self: &Self) -> RdfValue { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(v: &RdfValue) -> Res { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Type Alias `Triple`

A statement as `getStatementsMatching` returns it: `[subject, predicate,
object]`, the object a string when a literal.

```rust
pub type Triple = (super::term::Node, super::term::Node, RdfValue);
```

#### Struct `RdfSandbox`

`Zotero.RDF` (module docs).

```rust
pub struct RdfSandbox {
    pub store: super::store::Store,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `store` | `super::store::Store` | The data store (`_dataStore`). |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(first_id: u64) -> Self { /* ... */ }
  ```
  An empty store whose blank nodes are numbered from `first_id`.

- ```rust
  pub fn from_xml(text: &str, first_id: u64) -> Result<Self, String> { /* ... */ }
  ```
  `_initRDF` on `text`: parse it as XML (`parseDOMXML`), then as RDF/XML

- ```rust
  pub fn serialize(self: &Self) -> Result<String, String> { /* ... */ }
  ```
  `serialize()` (RDF/XML).

- ```rust
  pub fn add_statement</* synthetic */ impl Into<Res>: Into<Res>, /* synthetic */ impl Into<Res>: Into<Res>, /* synthetic */ impl Into<Res>: Into<Res>>(self: &mut Self, about: impl Into<Res>, relation: impl Into<Res>, value: impl Into<Res>) -> Result<(), String> { /* ... */ }
  ```
  `addStatement(about, relation, value, false)`: a resource object.

- ```rust
  pub fn add_literal</* synthetic */ impl Into<Res>: Into<Res>, /* synthetic */ impl Into<Res>: Into<Res>>(self: &mut Self, about: impl Into<Res>, relation: impl Into<Res>, value: &str) -> Result<(), String> { /* ... */ }
  ```
  `addStatement(about, relation, value, true)`: a literal, with the

- ```rust
  pub fn new_resource(self: &mut Self) -> Node { /* ... */ }
  ```
  `newResource()`: a new blank node.

- ```rust
  pub fn new_container</* synthetic */ impl Into<Res>: Into<Res>>(self: &mut Self, ty: &str, about: impl Into<Res>) -> Result<Node, String> { /* ... */ }
  ```
  `newContainer(type, about)`.

- ```rust
  pub fn add_container_element</* synthetic */ impl Into<Res>: Into<Res>>(self: &mut Self, about: impl Into<Res>, element: Node) -> Result<(), String> { /* ... */ }
  ```
  `addContainerElement(about, element, false)`.

- ```rust
  pub fn get_container_elements</* synthetic */ impl Into<Res>: Into<Res>>(self: &mut Self, about: impl Into<Res>) -> Vec<Option<RdfValue>> { /* ... */ }
  ```
  `getContainerElements(about)`: the `rdf:_n` members by `n`, as a

- ```rust
  pub fn add_namespace(self: &mut Self, prefix: &str, uri: &str) { /* ... */ }
  ```
  `addNamespace(prefix, uri)`.

- ```rust
  pub fn get_resource_uri(self: &mut Self, r: &RdfValue) -> RdfValue { /* ... */ }
  ```
  `getResourceURI(resource)` (:3209-3222): a URI string, a blank node's

- ```rust
  pub fn get_resource_uri_string(self: &mut Self, r: &RdfValue) -> String { /* ... */ }
  ```
  [`RdfSandbox::get_resource_uri`] when the caller only uses a string

- ```rust
  pub fn get_all_resources(self: &Self) -> Vec<Node> { /* ... */ }
  ```
  `getAllResources()`: the subject of the first statement under each

- ```rust
  pub fn get_arcs_in</* synthetic */ impl Into<Res>: Into<Res>>(self: &mut Self, r: impl Into<Res>) -> Option<Vec<String>> { /* ... */ }
  ```
  `getArcsIn(resource)`: the predicates of the statements pointing at

- ```rust
  pub fn get_arcs_out</* synthetic */ impl Into<Res>: Into<Res>>(self: &mut Self, r: impl Into<Res>) -> Option<Vec<String>> { /* ... */ }
  ```
  `getArcsOut(resource)`.

- ```rust
  pub fn get_sources</* synthetic */ impl Into<Res>: Into<Res>, /* synthetic */ impl Into<Res>: Into<Res>>(self: &mut Self, r: impl Into<Res>, property: impl Into<Res>) -> Option<Vec<Node>> { /* ... */ }
  ```
  `getSources(resource, property)`: subjects (`None` is `false`).

- ```rust
  pub fn get_targets</* synthetic */ impl Into<Res>: Into<Res>, /* synthetic */ impl Into<Res>: Into<Res>>(self: &mut Self, r: impl Into<Res>, property: impl Into<Res>) -> Option<Vec<RdfValue>> { /* ... */ }
  ```
  `getTargets(resource, property)`: objects, literals as strings

- ```rust
  pub fn get_statements_matching(self: &mut Self, subj: &Res, pred: &Res, obj: &Res, obj_literal: bool) -> Option<Vec<Triple>> { /* ... */ }
  ```
  `getStatementsMatching(subj, pred, obj, objLiteral, justOne)`

- ```rust
  pub fn get_statements_matching_one(self: &mut Self, subj: &Res, pred: &Res, obj: &Res, obj_literal: bool) -> Option<Vec<Triple>> { /* ... */ }
  ```
  [`RdfSandbox::get_statements_matching`] with `justOne`.

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
    fn clone(self: &Self) -> RdfSandbox { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
## Module `serializer`

A store as RDF/XML, byte for byte as upstream's `statementsToXML` writes
it: subjects in order of their first statement, each subject's
statements in insertion order (upstream's sort compares a statement with
itself and so never reorders), blank nodes with exactly one incoming arc
nested, the rest as `rdf:nodeID` roots, and upstream's line packing
(width 80, indent 4, lengths in UTF-16 code units).

```rust
pub mod serializer { /* ... */ }
```

### Functions

#### Function `serialize_xml`

`Zotero.RDF.serialize()` (translate.js:3077-3090) in RDF/XML:
`Serializer(store)`, `suggestPrefix` for each of the store's namespaces,
`statementsToXML(store.statements)`.

```rust
pub fn serialize_xml(store: &super::store::Store) -> Result<String, String> { /* ... */ }
```

## Module `store`

The triple store (`IndexedFormula`): statements in insertion order,
indexed by subject, predicate and object, with upstream's "smushing" of
nodes declared identical (`owl:sameAs`, inverse-functional and functional
properties).

Upstream keys its indexes by `hashString()` in JavaScript objects, whose
string keys iterate in insertion order; [`OrderedIndex`] keeps that order,
which `Zotero.RDF.getAllResources` exposes. The provenance (`why`) index
is not kept: nothing a translator can call reads it.

```rust
pub mod store { /* ... */ }
```

### Types

#### Struct `Statement`

A triple.

```rust
pub struct Statement {
    pub subject: super::term::Node,
    pub predicate: super::term::Node,
    pub object: super::term::Node,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `subject` | `super::term::Node` | `subject`. |
| `predicate` | `super::term::Node` | `predicate`. |
| `object` | `super::term::Node` | `object`. |

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
    fn clone(self: &Self) -> Statement { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `OrderedIndex`

A JavaScript object used as a map from hash strings to lists, keys in
insertion order (none of the keys is integer-like: they start with `<`,
`_` or `"`).

```rust
pub struct OrderedIndex {
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
  pub fn get(self: &Self, h: &str) -> Option<&Vec<usize>> { /* ... */ }
  ```
  `ix[h]`.

- ```rust
  pub fn entries(self: &Self) -> impl Iterator<Item = (&str, &Vec<usize>)> { /* ... */ }
  ```
  The keys in iteration order with their lists.

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
    fn clone(self: &Self) -> OrderedIndex { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> OrderedIndex { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Enum `Part`

Which position of a statement.

```rust
pub enum Part {
    Subject,
    Predicate,
    Object,
}
```

##### Variants

###### `Subject`

`subject`.

###### `Predicate`

`predicate`.

###### `Object`

`object`.

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
    fn clone(self: &Self) -> Part { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &Part) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `Store`

`IndexedFormula` (module docs).

```rust
pub struct Store {
    pub statements: Vec<Statement>,
    pub subject_index: OrderedIndex,
    pub predicate_index: OrderedIndex,
    pub object_index: OrderedIndex,
    pub namespaces: Vec<(String, String)>,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `statements` | `Vec<Statement>` | `statements`, in insertion order. |
| `subject_index` | `OrderedIndex` | `subjectIndex`. |
| `predicate_index` | `OrderedIndex` | `predicateIndex`. |
| `object_index` | `OrderedIndex` | `objectIndex`. |
| `namespaces` | `Vec<(String, String)>` | `namespaces`: prefix -> URI, in insertion order. |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  `new IndexedFormula()` with the default features (sameAs,

- ```rust
  pub fn with_first_id(first: u64) -> Self { /* ... */ }
  ```
  A store whose blank nodes are numbered from `first`. Upstream's

- ```rust
  pub fn sym</* synthetic */ impl Into<String>: Into<String>>(self: &mut Self, uri: impl Into<String>) -> Node { /* ... */ }
  ```
  `sym(uri)`: a new symbol object.

- ```rust
  pub fn literal</* synthetic */ impl Into<String>: Into<String>>(self: &mut Self, value: impl Into<String>, lang: Option<&str>, datatype: Option<&str>) -> Node { /* ... */ }
  ```
  `literal(value, lang, datatype)` (an empty `lang` is none).

- ```rust
  pub fn bnode(self: &mut Self) -> Node { /* ... */ }
  ```
  `bnode()` / `new BlankNode()`: the next id.

- ```rust
  pub fn collection(self: &mut Self) -> Node { /* ... */ }
  ```
  `collection()` / `new Collection()`.

- ```rust
  pub fn collection_append(self: &mut Self, id: u64, el: Node) { /* ... */ }
  ```
  `collection.append(el)`.

- ```rust
  pub fn collection_elements(self: &Self, id: u64) -> &[Node] { /* ... */ }
  ```
  The elements of a collection.

- ```rust
  pub fn term_to_string(self: &Self, t: &Term) -> String { /* ... */ }
  ```
  A term's JavaScript `toString()` (a collection lists its elements:

- ```rust
  pub fn set_prefix_for_uri(self: &mut Self, prefix: &str, uri: &str) { /* ... */ }
  ```
  `setPrefixForURI(prefix, uri)` (identity.js:132-139).

- ```rust
  pub fn canon(self: &Self, n: &Node) -> Node { /* ... */ }
  ```
  `canon(term)` (identity.js:218-223).

- ```rust
  pub fn add(self: &mut Self, subject: Node, predicate: Node, object: Node) -> usize { /* ... */ }
  ```
  `add(subj, pred, obj)` (identity.js:282-321): run the predicate's

- ```rust
  pub fn equate(self: &mut Self, u1: &Node, u2: &Node) { /* ... */ }
  ```
  `equate(u1, u2)` (identity.js:152-166): replace the bigger term with the

- ```rust
  pub fn part(self: &Self, st: usize, p: Part) -> &Node { /* ... */ }
  ```
  The node of statement `st` at `p`.

- ```rust
  pub fn statements_matching(self: &Self, subject: Option<&Node>, predicate: Option<&Node>, object: Option<&Node>, just_one: bool) -> Vec<usize> { /* ... */ }
  ```
  `statementsMatching(subj, pred, obj, undefined, justOne)`

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
    fn clone(self: &Self) -> Store { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Store { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
## Module `term`

RDF terms as upstream's AJAW library represents them.

JavaScript gives every term object an identity; the translators compare
by identity in one place (`RDF.js` `detectType`, `processedParts.includes`).
A [`Node`] therefore carries `inst`, the identity of the JavaScript object
it stands for: each `sym()` call upstream makes a new object, while a blank
node or a collection is one object per id. Equality of terms
([`Term::same_term`]) ignores `inst`, as upstream's `sameTerm` does.

```rust
pub mod term { /* ... */ }
```

### Types

#### Enum `Term`

An RDF term (upstream's `termType`s: symbol, bnode, literal, collection).

```rust
pub enum Term {
    Symbol(String),
    BlankNode(u64),
    Literal {
        value: String,
        lang: Option<String>,
        datatype: Option<String>,
    },
    Collection(u64),
}
```

##### Variants

###### `Symbol`

`Symbol(uri)`: a named resource.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `BlankNode`

`BlankNode`, by its `id` (upstream's global counter `Term.NextId`).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `u64` |  |

###### `Literal`

`Literal(value, lang, datatype)`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `value` | `String` | `value`. |
| `lang` | `Option<String>` | `lang` (`undefined` when empty). |
| `datatype` | `Option<String>` | `datatype`, a symbol's URI. |

###### `Collection`

`Collection` (`rdf:parseType="Collection"`), by its `id`; its elements
live in the store.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `u64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn literal</* synthetic */ impl Into<String>: Into<String>>(value: impl Into<String>) -> Term { /* ... */ }
  ```
  A literal with neither language nor datatype.

- ```rust
  pub fn to_nt(self: &Self) -> String { /* ... */ }
  ```
  `toNT()`, which is also upstream's `hashString()` (identity.js:31-34).

- ```rust
  pub fn to_js_string_simple(self: &Self) -> String { /* ... */ }
  ```
  `toString()` for every term but a collection (whose `toString` lists

- ```rust
  pub fn term_type(self: &Self) -> &'static str { /* ... */ }
  ```
  `termType`.

- ```rust
  pub fn same_term(self: &Self, other: &Term) -> bool { /* ... */ }
  ```
  `sameTerm` (term.js:424-460).

- ```rust
  pub fn uri(self: &Self) -> Option<&str> { /* ... */ }
  ```
  The symbol's URI (`term.uri`), if a symbol.

- ```rust
  pub fn compare_term(self: &Self, other: &Term) -> Ordering { /* ... */ }
  ```
  `compareTerm` (term.js:472-498): class order, then value / URI (as

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
    fn clone(self: &Self) -> Term { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Term) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `Node`

A term as a JavaScript object: the term and the identity of the object
(module docs).

```rust
pub struct Node {
    pub term: Term,
    pub inst: u64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `term` | `Term` | The term. |
| `inst` | `u64` | The JavaScript object's identity. |

##### Implementations

###### Methods

- ```rust
  pub fn same_object(self: &Self, other: &Node) -> bool { /* ... */ }
  ```
  Whether two nodes are the same JavaScript object (`===`).

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
    fn clone(self: &Self) -> Node { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(n: Node) -> Res { /* ... */ }
    ```

  - ```rust
    fn from(n: &Node) -> Res { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
### Constants and Statics

#### Constant `RDF_NS`

The RDF namespace.

```rust
pub const RDF_NS: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
```

## Module `uri`

`$rdf.Util.uri.join`: resolve a URI against a base the way the RDF parser
does (not RFC 3986: the AJAW library's own rules, ported as they are).
Indices are byte offsets; every character the function searches for is
ASCII, so they agree with upstream's UTF-16 offsets on the same text.

```rust
pub mod uri { /* ... */ }
```

### Functions

#### Function `join`

`join(given, base)`.

```rust
pub fn join(given: &str, base: &str) -> String { /* ... */ }
```

### Re-exports

#### Re-export `RdfSandbox`

```rust
pub use sandbox::RdfSandbox;
```

#### Re-export `RdfValue`

```rust
pub use sandbox::RdfValue;
```

#### Re-export `Res`

```rust
pub use sandbox::Res;
```

#### Re-export `Triple`

```rust
pub use sandbox::Triple;
```

#### Re-export `Node`

```rust
pub use term::Node;
```

#### Re-export `Term`

```rust
pub use term::Term;
```

#### Re-export `RDF_NS`

```rust
pub use term::RDF_NS;
```

## Module `utilities`

`Zotero.Utilities` (`ZU`) as the four ported translators use it.

Dates (`strToDate`, `strToISO`, `formatDate`) are kovan-common's
[`kovan_common::zotero::date`]; `unescapeHTML` is [`super::html`].

```rust
pub mod utilities { /* ... */ }
```

### Types

#### Struct `CleanedAuthor`

The result of [`clean_author`].

```rust
pub struct CleanedAuthor {
    pub first_name: Option<String>,
    pub last_name: String,
    pub creator_type: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `first_name` | `Option<String>` | `firstName` (`None` is `undefined`). |
| `last_name` | `String` | `lastName`. |
| `creator_type` | `String` | `creatorType`, as passed in. |

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
    fn clone(self: &Self) -> CleanedAuthor { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &CleanedAuthor) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
### Functions

#### Function `capitalize_name`

`Zotero.Utilities.capitalizeName` (utilities.js:199-216; #756): each
space-separated part that is all upper or all lower case is lower-cased
and every letter at its start or after a non-letter upper-cased
(`XRegExp('(^|[^\\pL])\\pL', 'g')`, the whole match upper-cased). The
same as the private copy in `translators::crossref_unixref_xml`.

```rust
pub fn capitalize_name(s: &str) -> String { /* ... */ }
```

#### Function `trim`

`Zotero.Utilities.trim` (:297-305).

```rust
pub fn trim(s: &str) -> String { /* ... */ }
```

#### Function `trim_internal`

`Zotero.Utilities.trimInternal` (:310-317): every run of
`[\xA0\r\n\s]` becomes one space, then trim.

```rust
pub fn trim_internal(s: &str) -> String { /* ... */ }
```

#### Function `lpad`

`Zotero.Utilities.lpad` (:975-981) for a string argument (an empty one
is treated as `''`, as `string ? string + '' : ''` does).

```rust
pub fn lpad(s: &str, pad: &str, length: usize) -> String { /* ... */ }
```

#### Function `clean_author`

`Zotero.Utilities.cleanAuthor(author, type, useComma)` (:227-290).

```rust
pub fn clean_author(author: &str, creator_type: &str, use_comma: bool) -> CleanedAuthor { /* ... */ }
```

#### Function `decode_uri_component`

JavaScript `decodeURIComponent`, or `None` where it throws `URIError`
(a `%` not followed by two hex digits, or bytes that are not UTF-8).

```rust
pub fn decode_uri_component(s: &str) -> Option<String> { /* ... */ }
```

#### Function `clean_doi`

`Zotero.Utilities.cleanDOI` (:481-523): the DOI in `x`, or `None`
(upstream `null`).

```rust
pub fn clean_doi(x: &str) -> Option<String> { /* ... */ }
```

#### Function `html_special_chars`

`Zotero.Utilities.htmlSpecialChars` (:668-693).

```rust
pub fn html_special_chars(s: &str) -> String { /* ... */ }
```

#### Function `text2html`

`Zotero.Utilities.text2html(str, singleNewlineIsParagraph)` (:638-660).

```rust
pub fn text2html(s: &str, single_newline_is_paragraph: bool) -> String { /* ... */ }
```

#### Function `remove_diacritics`

`Zotero.Utilities.removeDiacritics(str, lowercaseOnly)` (:1153-1169).

```rust
pub fn remove_diacritics(s: &str, lowercase_only: bool) -> String { /* ... */ }
```

#### Function `get_creators_for_type`

`Zotero.Utilities.getCreatorsForType` (:1291-1300): the creator types
valid for an item type, primary first. The order is schema.json's; the
translation-server's type snapshot (`zoteroTypeSchemaData.js`) orders
some secondary types differently, but the primary (first) type agrees for
every item type (checked 2026-10-07).

```rust
pub fn get_creators_for_type(item_type: &str) -> Vec<&'static str> { /* ... */ }
```

#### Function `str_to_iso`

`Zotero.Date.strToISO(str)` (date.js:600-614, the same in utilities
1dd38e27edf8 and 4051881d59c6) on kovan-common's `strToDate`, with
upstream's truthiness: `if (date.year)` is true for any non-empty year
string, including `"0"` (what `strToDate("0000")` gives), so
`strToISO("0000")` is `"0000"` (checked by running upstream's date.js,
2026-10-07). ~~kovan-common's
[`str_to_iso`](kovan_common::zotero::date::str_to_iso) returns `None` for
a `"0"` year; this one is used by the translators.~~ **CORRECTED
2026-10-07:** kovan-common's `str_to_iso` was fixed the same day and now
agrees on year 0; this copy is kept because it follows the translators'
`if(date.day)` truthiness on the day explicitly.

```rust
pub fn str_to_iso(s: &str, opts: &kovan_common::zotero::date::DateOptions) -> Option<String> { /* ... */ }
```

#### Function `field_is_valid_for_type`

`Zotero.Utilities.fieldIsValidForType(field, type)` (:1308-1310).

```rust
pub fn field_is_valid_for_type(field: &str, item_type: &str) -> bool { /* ... */ }
```

#### Function `item_type_exists`

`Zotero.Utilities.Item.itemTypeExists` (utilities_item.js:41-49).

```rust
pub fn item_type_exists(item_type: &str) -> bool { /* ... */ }
```

## Module `child`

Child translators: a translator running another one
(`Zotero.loadTranslator("import")`, `setString`, `setHandler("itemDone",
...)`, `translate()`), as METS runs MODS and MARCXML.

The child gets a context of its own over the string, with
`parent_translator` set, so its `item.complete()` applies the child form
of `_itemDone` (normalised, not saved). The parent receives those items
in completion order (its `itemDone` handler) and completes them itself.

```rust
pub mod child { /* ... */ }
```

### Functions

#### Function `child_import_context`

A child import context over `input` for the translator `meta`, called
from `parent` (the environment is the parent's; `Zotero.parentTranslator`
is the parent's translatorID).

```rust
pub fn child_import_context(parent: &super::context::ImportContext, input: &str, meta: &'static super::options::TranslatorMetadata) -> super::context::ImportContext { /* ... */ }
```

#### Function `run_child_import`

Run a child import (`run` is the child translator's `doImport` over the
child context) and return the items it completed, in order, ready for
the parent's `itemDone` handler. An error thrown by the child is the
parent's error.

```rust
pub fn run_child_import(parent: &super::context::ImportContext, input: &str, meta: &'static super::options::TranslatorMetadata, run: fn(&mut super::context::ImportContext) -> Result<(), super::context::TranslateError>) -> Result<Vec<super::item::TranslatorItem>, super::context::TranslateError> { /* ... */ }
```

## Module `html_dom`

HTML documents in the [`XmlDocument`] arena: HTML elements are elements
in the XHTML namespace with no prefix, as in the DOM.

Checked against jsdom 29.0.1 (node, 2026-10-07): `element.style`
recognises only lowercase property names (`TEXT-DECORATION: ...` reads
as unset) and serialises `name: value;` joined by spaces, colours
`#rgb`/`#rrggbb` as `rgb(r, g, b)`, invalid declarations dropped.

Not modelled (none occurs in the note exporters' inputs): template
contents (dropped), other CSS value normalisations, `noscript` with
scripting enabled (DOMParser documents have scripting disabled).

```rust
pub mod html_dom { /* ... */ }
```

### Types

#### Struct `OwnedElemName`

The element name html5ever asks for.

```rust
pub struct OwnedElemName(/* private field */);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `private` | *Private field* |

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
- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **ElemName**
  - ```rust
    fn ns(self: &Self) -> &Namespace { /* ... */ }
    ```

  - ```rust
    fn local_name(self: &Self) -> &LocalName { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
### Functions

#### Function `parse_html_document`

`new DOMParser().parseFromString(s, "text/html")`.

```rust
pub fn parse_html_document(s: &str) -> super::xml::XmlDocument { /* ... */ }
```

#### Function `set_inner_html`

`element.innerHTML = markup` in an HTML document: the HTML fragment
parsing algorithm with `el` as the context, its children replaced by the
result.

```rust
pub fn set_inner_html(doc: &mut super::xml::XmlDocument, el: super::xml::NodeId, markup: &str) { /* ... */ }
```

#### Function `create_html_element`

`document.createElement(name)` in an HTML document: lowercased, in the
XHTML namespace.

```rust
pub fn create_html_element(doc: &mut super::xml::XmlDocument, name: &str) -> super::xml::NodeId { /* ... */ }
```

#### Function `is_html_element`

Whether `n` is an element in the HTML namespace.

```rust
pub fn is_html_element(doc: &super::xml::XmlDocument, n: super::xml::NodeId) -> bool { /* ... */ }
```

#### Function `html_node_name`

`nodeName` in an HTML document: the qualified name uppercased for HTML
elements, `#text`, `#comment`, ... otherwise.

```rust
pub fn html_node_name(doc: &super::xml::XmlDocument, n: super::xml::NodeId) -> String { /* ... */ }
```

#### Function `next_element_sibling`

`nextElementSibling`.

```rust
pub fn next_element_sibling(doc: &super::xml::XmlDocument, n: super::xml::NodeId) -> Option<super::xml::NodeId> { /* ... */ }
```

#### Function `class_list_contains`

`classList.contains(token)`.

```rust
pub fn class_list_contains(doc: &super::xml::XmlDocument, n: super::xml::NodeId, token: &str) -> bool { /* ... */ }
```

#### Function `outer_html`

`element.outerHTML` in an HTML document (parse5 `serializeOuter`).

```rust
pub fn outer_html(doc: &super::xml::XmlDocument, n: super::xml::NodeId) -> String { /* ... */ }
```

#### Function `inner_html`

`element.innerHTML` (getter) in an HTML document.

```rust
pub fn inner_html(doc: &super::xml::XmlDocument, n: super::xml::NodeId) -> String { /* ... */ }
```

#### Function `escape_text`

entities `escapeText`: `&`, `<`, `>`, U+00A0.

```rust
pub fn escape_text(s: &str) -> String { /* ... */ }
```

#### Function `escape_attribute`

entities `escapeAttribute`: `"`, `&`, U+00A0.

```rust
pub fn escape_attribute(s: &str) -> String { /* ... */ }
```

#### Function `query_selector_all`

`root.querySelectorAll(selectors)` for a selector list of compound
selectors (type, `.class`, `[attr]`, `[attr="v"]`, `:not(...)`) joined
by descendant combinators. Type selectors match HTML elements ASCII
case-insensitively. Results in tree order, `root` excluded.

```rust
pub fn query_selector_all(doc: &super::xml::XmlDocument, root: super::xml::NodeId, selectors: &str) -> Vec<super::xml::NodeId> { /* ... */ }
```

#### Function `style_get`

`element.style.<property>` (CSS property name, e.g. `padding-left`).

```rust
pub fn style_get(doc: &super::xml::XmlDocument, n: super::xml::NodeId, prop: &str) -> String { /* ... */ }
```

#### Function `style_set`

`element.style.<property> = value` ("" removes it): the `style`
attribute rewritten as jsdom serialises it.

```rust
pub fn style_set(doc: &mut super::xml::XmlDocument, n: super::xml::NodeId, prop: &str, value: &str) { /* ... */ }
```

## Module `xml`

An XML DOM for the XML translators (#749): what `Zotero.getXML()`,
`new DOMParser().parseFromString(s, "text/xml")` and the exporters'
`createElementNS`/`appendChild`/`XMLSerializer` give a translator in the
translation-server.

The document is an arena: nodes are [`NodeId`]s into one
[`XmlDocument`], and an attribute is addressed as [`XNode::Attr`] (its
element and its index), which is what XPath returns for `@name`.

| Module | What |
|---|---|
| this one | the tree, node accessors (`textContent`, `getAttribute`, `children`, `lookupNamespaceURI`, ...), mutation, `getElementsByTagName(NS)`, `querySelectorAll` (the selector subset the translators use) |
| [`super::xml_parse`] | parsing: saxes 6.0.0 as jsdom drives it |
| [`super::xml_serialize`] | `XMLSerializer` / `innerHTML`: w3c-xmlserializer 5.0.0 |
| [`super::xpath`] | `ZU.xpath` / `ZU.xpathText` over wicked-good-xpath 1.3.1-z002 |

```rust
pub mod xml { /* ... */ }
```

### Modules

## Module `node_type`

DOM `nodeType` values.

```rust
pub mod node_type { /* ... */ }
```

### Constants and Statics

#### Constant `ELEMENT`

`ELEMENT_NODE`.

```rust
pub const ELEMENT: u8 = 1;
```

#### Constant `ATTRIBUTE`

`ATTRIBUTE_NODE`.

```rust
pub const ATTRIBUTE: u8 = 2;
```

#### Constant `TEXT`

`TEXT_NODE`.

```rust
pub const TEXT: u8 = 3;
```

#### Constant `CDATA`

`CDATA_SECTION_NODE`.

```rust
pub const CDATA: u8 = 4;
```

#### Constant `PI`

`PROCESSING_INSTRUCTION_NODE`.

```rust
pub const PI: u8 = 7;
```

#### Constant `COMMENT`

`COMMENT_NODE`.

```rust
pub const COMMENT: u8 = 8;
```

#### Constant `DOCUMENT`

`DOCUMENT_NODE`.

```rust
pub const DOCUMENT: u8 = 9;
```

#### Constant `DOCTYPE`

`DOCUMENT_TYPE_NODE`.

```rust
pub const DOCTYPE: u8 = 10;
```

### Types

#### Struct `NodeId`

A node of an [`XmlDocument`].

```rust
pub struct NodeId(pub usize);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

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
    fn clone(self: &Self) -> NodeId { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &K) -> Ordering { /* ... */ }
    ```

  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(n: NodeId) -> Self { /* ... */ }
    ```

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &NodeId) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NodeId) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &NodeId) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Enum `XNode`

A node or an attribute: what an XPath step can select.

```rust
pub enum XNode {
    Node(NodeId),
    Attr(NodeId, usize),
}
```

##### Variants

###### `Node`

A tree node.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `NodeId` |  |

###### `Attr`

The `index`-th attribute of an element.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `NodeId` |  |
| 1 | `usize` |  |

##### Implementations

###### Methods

- ```rust
  pub fn node(self: Self) -> NodeId { /* ... */ }
  ```
  The tree node (for an attribute, its element).

- ```rust
  pub fn as_node(self: Self) -> Option<NodeId> { /* ... */ }
  ```
  The tree node, `None` for an attribute.

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
    fn clone(self: &Self) -> XNode { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(n: NodeId) -> Self { /* ... */ }
    ```

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &XNode) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `XmlAttr`

An attribute: namespace, prefix, local name, value (DOM `Attr`).

```rust
pub struct XmlAttr {
    pub namespace: Option<String>,
    pub prefix: Option<String>,
    pub local: String,
    pub value: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `namespace` | `Option<String>` | `namespaceURI`. |
| `prefix` | `Option<String>` | `prefix`. |
| `local` | `String` | `localName`. |
| `value` | `String` | `value`. |

##### Implementations

###### Methods

- ```rust
  pub fn qualified_name(self: &Self) -> String { /* ... */ }
  ```
  `name`: the qualified name.

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
    fn clone(self: &Self) -> XmlAttr { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &XmlAttr) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `ElementData`

An element's name and attributes.

```rust
pub struct ElementData {
    pub namespace: Option<String>,
    pub prefix: Option<String>,
    pub local: String,
    pub attrs: Vec<XmlAttr>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `namespace` | `Option<String>` | `namespaceURI`. |
| `prefix` | `Option<String>` | `prefix`. |
| `local` | `String` | `localName`. |
| `attrs` | `Vec<XmlAttr>` | The attributes in order (namespace declarations included, as in the<br>DOM). |

##### Implementations

###### Methods

- ```rust
  pub fn qualified_name(self: &Self) -> String { /* ... */ }
  ```
  `tagName` (= qualified name in an XML document).

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
    fn clone(self: &Self) -> ElementData { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &ElementData) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Enum `NodeKind`

What a node is.

```rust
pub enum NodeKind {
    Document,
    DocumentType {
        name: String,
        public_id: String,
        system_id: String,
    },
    Element(ElementData),
    Text(String),
    CData(String),
    Comment(String),
    Pi {
        target: String,
        data: String,
    },
}
```

##### Variants

###### `Document`

The document.

###### `DocumentType`

`<!DOCTYPE>`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | `name`. |
| `public_id` | `String` | `publicId`. |
| `system_id` | `String` | `systemId`. |

###### `Element`

An element.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `ElementData` |  |

###### `Text`

Text.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `CData`

A CDATA section.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Comment`

A comment.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Pi`

A processing instruction.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `target` | `String` | `target`. |
| `data` | `String` | `data`. |

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
    fn clone(self: &Self) -> NodeKind { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &NodeKind) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `XmlDocument`

An XML document (DOM `XMLDocument`); node 0 is the document node.

```rust
pub struct XmlDocument {
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
  pub fn new() -> Self { /* ... */ }
  ```
  An empty document (no document element).

- ```rust
  pub fn parse(s: &str) -> Result<XmlDocument, XmlParseError> { /* ... */ }
  ```
  `Zotero.Translate.IO.parseDOMXML(s)` (translate.js:2858-2885):

- ```rust
  pub fn parse_from_string(s: &str) -> XmlDocument { /* ... */ }
  ```
  `new DOMParser().parseFromString(s, type)` as an exporter calls it

- ```rust
  pub fn document(self: &Self) -> NodeId { /* ... */ }
  ```
  The document node.

- ```rust
  pub fn document_element(self: &Self) -> Option<NodeId> { /* ... */ }
  ```
  `documentElement`.

- ```rust
  pub fn kind(self: &Self, n: NodeId) -> &NodeKind { /* ... */ }
  ```
  The node's kind.

- ```rust
  pub fn element(self: &Self, n: NodeId) -> Option<&ElementData> { /* ... */ }
  ```
  The element data, `None` for another node.

- ```rust
  pub fn is_element(self: &Self, n: NodeId) -> bool { /* ... */ }
  ```
  Whether the node is an element.

- ```rust
  pub fn node_type(self: &Self, x: XNode) -> u8 { /* ... */ }
  ```
  `nodeType`.

- ```rust
  pub fn parent(self: &Self, n: NodeId) -> Option<NodeId> { /* ... */ }
  ```
  `parentNode` (an attribute has none).

- ```rust
  pub fn children(self: &Self, n: NodeId) -> &[NodeId] { /* ... */ }
  ```
  `childNodes`.

- ```rust
  pub fn element_children(self: &Self, n: NodeId) -> Vec<NodeId> { /* ... */ }
  ```
  `children`: the element children.

- ```rust
  pub fn has_child_nodes(self: &Self, n: NodeId) -> bool { /* ... */ }
  ```
  `hasChildNodes()`.

- ```rust
  pub fn first_child(self: &Self, n: NodeId) -> Option<NodeId> { /* ... */ }
  ```
  `firstChild`.

- ```rust
  pub fn next_sibling(self: &Self, n: NodeId) -> Option<NodeId> { /* ... */ }
  ```
  `nextSibling`.

- ```rust
  pub fn previous_sibling(self: &Self, n: NodeId) -> Option<NodeId> { /* ... */ }
  ```
  `previousSibling`.

- ```rust
  pub fn attr(self: &Self, x: XNode) -> Option<&XmlAttr> { /* ... */ }
  ```
  The attribute an [`XNode::Attr`] addresses.

- ```rust
  pub fn node_name(self: &Self, x: XNode) -> String { /* ... */ }
  ```
  `nodeName`: the qualified name of an element or attribute, `#text`,

- ```rust
  pub fn tag_name(self: &Self, n: NodeId) -> String { /* ... */ }
  ```
  `tagName` of an element (its qualified name), "" for another node.

- ```rust
  pub fn local_name(self: &Self, x: XNode) -> Option<&str> { /* ... */ }
  ```
  `localName` (elements and attributes; `None` for other nodes).

- ```rust
  pub fn namespace_uri(self: &Self, x: XNode) -> Option<&str> { /* ... */ }
  ```
  `namespaceURI` (elements and attributes).

- ```rust
  pub fn prefix(self: &Self, x: XNode) -> Option<&str> { /* ... */ }
  ```
  `prefix` (elements and attributes).

- ```rust
  pub fn node_value(self: &Self, x: XNode) -> Option<String> { /* ... */ }
  ```
  `nodeValue`: an attribute's value, the data of text, CDATA, comments

- ```rust
  pub fn text_content(self: &Self, x: XNode) -> Option<String> { /* ... */ }
  ```
  `textContent`: for an element, its descendant text (Text and CDATA

- ```rust
  pub fn text</* synthetic */ impl Into<XNode>: Into<XNode>>(self: &Self, x: impl Into<XNode>) -> String { /* ... */ }
  ```
  `textContent` as a string ("" where it is null).

- ```rust
  pub fn attributes(self: &Self, n: NodeId) -> &[XmlAttr] { /* ... */ }
  ```
  The attributes of an element (empty for another node).

- ```rust
  pub fn get_attribute(self: &Self, n: NodeId, name: &str) -> Option<&str> { /* ... */ }
  ```
  `getAttribute(qualifiedName)`: the first attribute whose qualified

- ```rust
  pub fn has_attribute(self: &Self, n: NodeId, name: &str) -> bool { /* ... */ }
  ```
  `hasAttribute(qualifiedName)`.

- ```rust
  pub fn get_attribute_ns(self: &Self, n: NodeId, ns: Option<&str>, local: &str) -> Option<&str> { /* ... */ }
  ```
  `getAttributeNS(namespace, localName)` (an empty namespace is null).

- ```rust
  pub fn lookup_namespace_uri(self: &Self, n: NodeId, prefix: Option<&str>) -> Option<String> { /* ... */ }
  ```
  `lookupNamespaceURI(prefix)` (DOM "locate a namespace"), on an

- ```rust
  pub fn descendants(self: &Self, root: NodeId) -> Vec<NodeId> { /* ... */ }
  ```
  Every descendant of `root` in tree order (not `root` itself).

- ```rust
  pub fn get_elements_by_tag_name(self: &Self, root: NodeId, name: &str) -> Vec<NodeId> { /* ... */ }
  ```
  `getElementsByTagName(qualifiedName)` (XML document: exact qualified

- ```rust
  pub fn get_elements_by_tag_name_ns(self: &Self, root: NodeId, ns: Option<&str>, local: &str) -> Vec<NodeId> { /* ... */ }
  ```
  `getElementsByTagNameNS(namespace, localName)` (`*` matches any

- ```rust
  pub fn get_element_by_id(self: &Self, id: &str) -> Option<NodeId> { /* ... */ }
  ```
  `document.getElementById(id)`: the first element in tree order with

- ```rust
  pub fn query_selector_all(self: &Self, root: NodeId, selector: &str) -> Vec<NodeId> { /* ... */ }
  ```
  `querySelectorAll(selectors)` for the selectors the translators use:

- ```rust
  pub fn query_selector(self: &Self, root: NodeId, selector: &str) -> Option<NodeId> { /* ... */ }
  ```
  `querySelector(selectors)`: the first of [`Self::query_selector_all`].

- ```rust
  pub fn compare_order(self: &Self, a: XNode, b: XNode) -> std::cmp::Ordering { /* ... */ }
  ```
  Document order of two nodes (DOM `compareDocumentPosition`): an

- ```rust
  pub fn normalize(self: &mut Self, n: NodeId) { /* ... */ }
  ```
  `normalize()`: merge adjacent Text nodes (not CDATA) and drop empty

- ```rust
  pub fn create_element_ns(self: &mut Self, ns: Option<&str>, qname: &str) -> NodeId { /* ... */ }
  ```
  `createElementNS(namespace, qualifiedName)` (an empty namespace is

- ```rust
  pub fn create_element(self: &mut Self, local: &str) -> NodeId { /* ... */ }
  ```
  `createElement(localName)` in an XML document: no namespace, no

- ```rust
  pub fn create_text_node(self: &mut Self, data: &str) -> NodeId { /* ... */ }
  ```
  `createTextNode(data)`.

- ```rust
  pub fn create_cdata_section(self: &mut Self, data: &str) -> NodeId { /* ... */ }
  ```
  `createCDATASection(data)`.

- ```rust
  pub fn create_comment(self: &mut Self, data: &str) -> NodeId { /* ... */ }
  ```
  `createComment(data)`.

- ```rust
  pub fn create_processing_instruction(self: &mut Self, target: &str, data: &str) -> NodeId { /* ... */ }
  ```
  `createProcessingInstruction(target, data)`.

- ```rust
  pub fn append_text_data(self: &mut Self, n: NodeId, s: &str) { /* ... */ }
  ```
  Append to a Text or CDATA node's data (`appendData`).

- ```rust
  pub fn set_text_data(self: &mut Self, n: NodeId, s: &str) { /* ... */ }
  ```
  Replace a Text, CDATA or comment node's data (`node.data = s`).

- ```rust
  pub fn push_attribute(self: &mut Self, n: NodeId, a: XmlAttr) { /* ... */ }
  ```
  Append an attribute as given (no lookup; the HTML parser's

- ```rust
  pub fn set_element_namespace(self: &mut Self, n: NodeId, ns: Option<&str>) { /* ... */ }
  ```
  Set an element's namespace (HTML `createElement`).

- ```rust
  pub fn import_subtree(self: &mut Self, other: &XmlDocument, n: NodeId) -> NodeId { /* ... */ }
  ```
  A detached deep copy of `other`'s node `n` in this document

- ```rust
  pub fn clone_subtree(self: &mut Self, n: NodeId) -> NodeId { /* ... */ }
  ```
  `node.cloneNode(true)`: a detached deep copy.

- ```rust
  pub fn append_child(self: &mut Self, parent: NodeId, child: NodeId) { /* ... */ }
  ```
  `parent.appendChild(child)` (a child with a parent is moved).

- ```rust
  pub fn insert_before(self: &mut Self, parent: NodeId, child: NodeId, reference: Option<NodeId>) { /* ... */ }
  ```
  `parent.insertBefore(child, reference)` (`None`: append).

- ```rust
  pub fn detach(self: &mut Self, child: NodeId) { /* ... */ }
  ```
  `node.remove()` / `parent.removeChild(node)`.

- ```rust
  pub fn set_text_content(self: &mut Self, n: NodeId, s: &str) { /* ... */ }
  ```
  `element.textContent = s`: replace all children by one text node (none

- ```rust
  pub fn set_attribute(self: &mut Self, n: NodeId, name: &str, value: &str) { /* ... */ }
  ```
  `setAttribute(qualifiedName, value)` on an element of an XML

- ```rust
  pub fn set_attribute_ns(self: &mut Self, n: NodeId, ns: Option<&str>, qname: &str, value: &str) { /* ... */ }
  ```
  `setAttributeNS(namespace, qualifiedName, value)`: change the

- ```rust
  pub fn remove_attribute(self: &mut Self, n: NodeId, name: &str) { /* ... */ }
  ```
  `removeAttribute(qualifiedName)`.

- ```rust
  pub fn serialize(self: &Self, n: NodeId) -> String { /* ... */ }
  ```
  `new XMLSerializer().serializeToString(node)` (w3c-xmlserializer,

- ```rust
  pub fn inner_html(self: &Self, n: NodeId) -> Result<String, String> { /* ... */ }
  ```
  `element.innerHTML` in an XML document: each child serialized on its

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
    fn clone(self: &Self) -> XmlDocument { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &XmlDocument) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `XmlParseError`

Why `parseFromString` gave a `<parsererror>` document (saxes' message).

```rust
pub struct XmlParseError(pub String);
```

##### Fields

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
    fn clone(self: &Self) -> XmlParseError { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &XmlParseError) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
### Functions

#### Function `get_xml`

`Zotero.getXML()` (translate.js:2989-2997): the whole input parsed as
XML; a parse failure is the error the translator sees thrown.

```rust
pub fn get_xml(ctx: &super::context::ImportContext) -> Result<XmlDocument, super::context::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Constant `XML_NS`

The XML namespace (`xml:` prefix).

```rust
pub const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";
```

#### Constant `XMLNS_NS`

The XMLNS namespace (`xmlns` attributes).

```rust
pub const XMLNS_NS: &str = "http://www.w3.org/2000/xmlns/";
```

#### Constant `HTML_NS`

The HTML namespace.

```rust
pub const HTML_NS: &str = "http://www.w3.org/1999/xhtml";
```

## Module `xml_parse`

Parsing XML the way the translation-server does (`DOMParser` with
"text/xml"): saxes' events turned into the tree jsdom builds.

Modelled: a leading byte-order mark skipped; CR LF and CR read as LF
everywhere; tab and newline in attribute values read as spaces; the five
predefined entities, character references, and internal
`<!ENTITY name "value">` declarations (their value inserted verbatim, as
jsdom does); namespaces (`xmlns` attributes kept as attributes in the
XMLNS namespace; the default namespace not applied to attributes);
`xml:` bound to the XML namespace; the XML declaration (only at the very
start, never a PI node); doctype, comments, PIs and CDATA nodes; text
outside the root element dropped.

Every well-formedness error saxes reports makes jsdom return a
`<parsererror>` document, so a failure here is an `Err` with saxes'
message (without its line:column prefix). Checked: unmatched, unclosed
and stray tags; a second root or no root; text or entities outside the
root; undefined entities and malformed character references; `<` in an
attribute value; duplicate attributes; unbound prefixes and the
`xml`/`xmlns` binding rules; `]]>` in text; a misplaced XML declaration
or doctype; malformed comments; characters outside the XML 1.0 `Char`
production; names that are not XML names.

```rust
pub mod xml_parse { /* ... */ }
```

### Functions

#### Function `parse_document`

Parse a whole document (jsdom `parseIntoDocument`); `Err` is the first
saxes error.

```rust
pub fn parse_document(input: &str) -> Result<super::xml::XmlDocument, super::xml::XmlParseError> { /* ... */ }
```

## Module `xml_serialize`

`XMLSerializer.serializeToString` and `innerHTML`, byte for byte as the
translation-server produces them.

Not modelled: the `requireWellFormed` checks on XML names (xml-name-
validator) and the `Char` production of text; the translators' exports
never reach them. HTML-namespace void elements are modelled.

```rust
pub mod xml_serialize { /* ... */ }
```

### Functions

#### Function `serialize`

Serialize `node` (w3c-xmlserializer's module export): fresh prefix map
with `xml`, context namespace null, prefix index 1.

```rust
pub fn serialize(doc: &super::xml::XmlDocument, node: super::xml::NodeId, require_well_formed: bool) -> Result<String, String> { /* ... */ }
```

#### Function `escape_text`

`serializeText`: `&`, `<`, `>` escaped.

```rust
pub fn escape_text(s: &str) -> String { /* ... */ }
```

#### Function `escape_attr`

`serializeAttributeValue`.

```rust
pub fn escape_attr(s: &str) -> String { /* ... */ }
```

## Module `xpath`

`ZU.xpath` and `ZU.xpathText`: XPath 1.0 as wicked-good-xpath evaluates
it over jsdom, quirks included, because translators were written (and
their test cases recorded) against exactly this engine.

Quirks that differ from the XPath 1.0 recommendation and are kept:

* An **unprefixed name test** matches elements in the document's default
  namespace (`document.lookupNamespaceURI(null)`, i.e. the root element's
  default namespace), not elements in no namespace. So `crossref/journal`
  finds elements in the Crossref namespace when that is the default.
* `//name` (child axis, no positional predicate) searches with
  `getElementsByTagName(name)`, which matches the **qualified** name: an
  element written `x:name` is not found by `//name` even when `x` is
  bound to the default namespace (the child axis would find it).
* The attribute axis matches by `getNamedItem(qualifiedName)` (or
  `getNamedItemNS` for a prefixed test) and ignores the default
  namespace; `@*` includes `xmlns` attributes.
* `text()` matches Text nodes only, not CDATA sections.
* A step whose first predicate is a bare `[@name]` (child-like axes) tests
  it as `!!getAttribute(localName)`: an empty attribute does not count.
* `number()` of an empty node-set is 0; comparing a node-set with a
  boolean converts each node's string value to a boolean.
* `namespace-uri()` is always "" and `lang()` always false.
* The string value of the document node concatenates the text of every
  non-element node under it, comments and PIs included.

Strings index `char`s where JavaScript indexes UTF-16 units (see
[`super::js`]); the two differ only on astral-plane characters.

```rust
pub mod xpath { /* ... */ }
```

### Types

#### Struct `XPathError`

An XPath error (upstream throws; `ZU.xpath` rethrows it).

```rust
pub struct XPathError(pub String);
```

##### Fields

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
    fn clone(self: &Self) -> XPathError { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(e: XPathError) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(e: XPathError) -> Self { /* ... */ }
    ```

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &XPathError) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
### Functions

#### Function `evaluate`

`document.evaluate(expr, context, resolver, ORDERED_NODE_ITERATOR_TYPE)`
on one context node: the nodes, in document order.

```rust
pub fn evaluate(doc: &super::xml::XmlDocument, context: super::xml::XNode, expr: &str, ns: &[(&str, &str)]) -> Result<Vec<super::xml::XNode>, XPathError> { /* ... */ }
```

#### Function `xpath`

`ZU.xpath(element, xpath, namespaces)` on one node (`namespaces`: prefix
and URI pairs; an empty slice is no resolver).

```rust
pub fn xpath</* synthetic */ impl Into<XNode>: Into<super::xml::XNode>>(doc: &super::xml::XmlDocument, node: impl Into<super::xml::XNode>, expr: &str, ns: &[(&str, &str)]) -> Result<Vec<super::xml::XNode>, super::context::TranslateError> { /* ... */ }
```

#### Function `xpath_all`

`ZU.xpath(elements, xpath, namespaces)` on several nodes: each node's
results in turn (not merged).

```rust
pub fn xpath_all(doc: &super::xml::XmlDocument, nodes: &[super::xml::XNode], expr: &str, ns: &[(&str, &str)]) -> Result<Vec<super::xml::XNode>, super::context::TranslateError> { /* ... */ }
```

#### Function `xpath_text`

`ZU.xpathText(node, xpath, namespaces, delimiter)`: the matches' values
(an attribute's value, else `textContent`) joined by `delimiter`
(default ", "), `None` (null) when nothing matches.

```rust
pub fn xpath_text</* synthetic */ impl Into<XNode>: Into<super::xml::XNode>>(doc: &super::xml::XmlDocument, node: impl Into<super::xml::XNode>, expr: &str, ns: &[(&str, &str)], delimiter: Option<&str>) -> Result<Option<String>, super::context::TranslateError> { /* ... */ }
```

#### Function `xpath_text_all`

[`xpath_text`] over several context nodes.

```rust
pub fn xpath_text_all(doc: &super::xml::XmlDocument, nodes: &[super::xml::XNode], expr: &str, ns: &[(&str, &str)], delimiter: Option<&str>) -> Result<Option<String>, super::context::TranslateError> { /* ... */ }
```

### Re-exports

#### Re-export `js_number_to_string`

```rust
pub use eval::js_number_to_string;
```

#### Re-export `js_to_number`

```rust
pub use eval::js_to_number;
```

### Re-exports

#### Re-export `fold_child_notes`

```rust
pub use api_json::fold_child_notes;
```

#### Re-export `item_to_api_json`

```rust
pub use api_json::item_to_api_json;
```

#### Re-export `KeyGenerator`

```rust
pub use api_json::KeyGenerator;
```

#### Re-export `export_input_from_zotero_items`

```rust
pub use context::export_input_from_zotero_items;
```

#### Re-export `parse_export_input`

```rust
pub use context::parse_export_input;
```

#### Re-export `ExportContext`

```rust
pub use context::ExportContext;
```

#### Re-export `ImportContext`

```rust
pub use context::ImportContext;
```

#### Re-export `ImportResult`

```rust
pub use context::ImportResult;
```

#### Re-export `TranslateError`

```rust
pub use context::TranslateError;
```

#### Re-export `ExportOutput`

```rust
pub use io::ExportOutput;
```

#### Re-export `ImportInput`

```rust
pub use io::ImportInput;
```

#### Re-export `CollectionChild`

```rust
pub use item::CollectionChild;
```

#### Re-export `JsObject`

```rust
pub use item::JsObject;
```

#### Re-export `TranslatorCollection`

```rust
pub use item::TranslatorCollection;
```

#### Re-export `TranslatorCreator`

```rust
pub use item::TranslatorCreator;
```

#### Re-export `TranslatorItem`

```rust
pub use item::TranslatorItem;
```

#### Re-export `TranslatorNote`

```rust
pub use item::TranslatorNote;
```

#### Re-export `TranslatorTag`

```rust
pub use item::TranslatorTag;
```

#### Re-export `HeaderValue`

```rust
pub use options::HeaderValue;
```

#### Re-export `TranslateOptions`

```rust
pub use options::TranslateOptions;
```

#### Re-export `TranslationEnv`

```rust
pub use options::TranslationEnv;
```

#### Re-export `TranslatorMetadata`

```rust
pub use options::TranslatorMetadata;
```

## Module `local_library`

Read a Zotero data folder (`zotero.sqlite` + `storage/`) into
[`ZoteroLibrary`]s, and import it into kovan ([`import`]). GitHub #750.

```no_run
use kovan_literature::zotero::local_library::{read_data_folder, ReadOptions};
let folder = read_data_folder("/home/me/Zotero".as_ref(), &ReadOptions::default()).unwrap();
for lib in &folder.libraries {
    println!("{:?}: {} items", lib.kind, lib.contents.items.len());
}
let import = kovan_literature::zotero::local_library::import::import(&folder, &Default::default());
println!("{} documents, lossy: {:?}", import.documents.len(), import.losses);
```

## Never touching the live database

Zotero opens `zotero.sqlite` with `PRAGMA locking_mode=EXCLUSIVE` and, in
this version, `journal_mode=WAL` (db.js:28-32, :1644-1653), so while Zotero
runs no other SQLite connection can read it. **This reader never opens the
file in the data folder as a database at all.** ~~It copies
`zotero.sqlite` and, when they exist, its `-journal` and `-wal` files into
a private temporary directory and opens the copy~~ **CORRECTED
2026-10-07:** it reads the bytes of `zotero.sqlite` and, when they exist,
of its `-wal` and `-journal` into memory ([`DatabaseFiles`]) and opens
that in-memory copy, which is how Zotero itself reads a database it does
not own (db.js:1871-1885 copies the database and its WAL to a temporary
file before touching them). No temporary file is needed any more: the
SQLite engine is `turso_core`, a pure-Rust rewrite of SQLite, which reads
the files from memory (module `db`), so the same code runs on wasm32 in a
browser, from files the user picks ([`read_database_files`]). The engine
replays the copied WAL in memory (validating its salts and checksums and
stopping at the last commit frame, as SQLite's wal.c does; checked
against real SQLite in `format_tests`), and `PRAGMA quick_check` must
report `ok`. ~~SQLite rolls back the copied journal~~ **A hot rollback
journal** (one SQLite would roll back first: non-empty, header not
zeroed, pager.c `hasHotJournal`) **is refused** rather than rolled back:
Zotero runs in WAL mode, so such a journal means an older or foreign
writer was mid-transaction, and the reader falls back to the backup.
If the copy fails (Zotero was mid-write while the bytes were copied),
the reader falls back to `zotero.sqlite.bak`, Zotero's own periodic backup
(db.js:1358, :2472), when [`ReadOptions::fall_back_to_backup`] is set
(the default), and records which file it read in
[`ZoteroDataFolder::source`]. [`DbSource::Backup`] reads the backup
directly. Whatever the engine writes while opening (a WAL checkpoint)
stays in memory and is dropped. Nothing is ever written to the data
folder.

**Encoding.** A database in UTF-16 (`PRAGMA encoding`) is refused with an
error (turso_core 0.8.2 reads UTF-8 only); Zotero's databases are UTF-8.

With Zotero running, the copy is a best-effort snapshot (a write landing
between copying the database and copying its WAL can still give a
structurally valid but slightly stale copy); for an exact read, close
Zotero first.

## Database versions

Zotero 5.0 and later: `userdata` schema version **80** (the Zotero 5
migration, schema.js:3012) up to **130**, the version of the bundled
`userdata.sql` at the upstream commit. Older databases (Zotero 4, e.g.
upstream's `test/tests/data/zotero-4.0.sqlite.zip`, userdata 77) have a
different layout and are refused with [`ZoteroDbError::TooOld`]: open them
once in a current Zotero, which upgrades them. A database whose
`compatibility` version exceeds 9 (`_maxCompatibility`, schema.js:45) is
refused like Zotero refuses it ([`ZoteroDbError::TooNew`]) unless
[`ReadOptions::allow_newer_schema`] is set. Tables and columns added after
80 are probed, not assumed: `itemAnnotations` (112; `authorName` 119),
`deletedCollections`/`deletedSearches` (111), `publicationsItems` (93),
`itemAttachments.lastRead` (124/130).

Type, field and creator-type ids are **not** assumed: they are read from
the database's own `itemTypesCombined`, `fieldsCombined` and
`creatorTypes` tables and resolved by name, because they differ between a
database created fresh (ids assigned in `schema.json` order by
`_updateGlobalSchema`, schema.js:459) and one upgraded from Zotero 4
(`system-107.sql` ids).

## What is read, and how (faithful to Zotero's own loaders)

| Zotero tables | Read into | Upstream behaviour mirrored |
|---|---|---|
| `libraries`, `groups`, `settings` (`account` `userID`/`localUserKey`) | one [`LocalLibrary`] per user/group library, its URI | uri.js `getLibraryPath` |
| `items`, `deletedItems`, `publicationsItems` | key, version, type, `dateAdded`/`dateModified` (ISO), `deleted`/`inPublications` (only when true) | items.js `_primaryDataSQLParts`, item.js `toJSON`, dataObject.js `_postToJSON` |
| `itemData`, `itemDataValues`, `fieldsCombined` | [`ZoteroItem::fields`](kovan_common::zotero::ZoteroItem::fields) | `_loadItemData` + `setField(.., loadIn)`: trim, base -> type-specific field, fields invalid for the type ignored (and reported), newlines stripped outside `abstractNote`/`extra`/`address`; `getField`: multipart dates -> user string; `toJSON`: `accessDate` -> ISO |
| `itemNotes` | `note` | `_loadNotes`: the `zotero-note znv` wrapper stripped; a plain-text note converted to HTML in memory (upstream also writes it back; this reader does not) |
| `itemCreators`, `creators`, `creatorTypes` | creators, in `orderIndex` order (regular items only) | `_loadCreators`, `toJSON` |
| `itemTags`, `tags` | tags; `type: 1` kept, 0 dropped | `_loadTags`, tags.js `cleanData` |
| `collectionItems` | `collections` of top-level items | `_loadCollections`, `toJSON` |
| `itemRelations`, `relationPredicates` | `relations` | `_loadRelations` |
| `itemAttachments`, `charsets` | [`AttachmentData`](kovan_common::zotero::AttachmentData) and [`LocalLibrary::files`] | `_parseRowData`, `toJSON`, `attachmentFilename`, `getFilePath`; `md5`/`mtime` from `storageHash`/`storageModTime` (the `syncedStorageProperties` form) |
| `itemAnnotations` | [`AnnotationData`](kovan_common::zotero::AnnotationData); `isExternal` as `other["annotationIsExternal"]` when true | `_loadAnnotations`, `_loadAnnotationsDeferred`, `toJSON` |
| `collections`, `deletedCollections`, `collectionRelations` | [`ZoteroCollection`](kovan_common::zotero::ZoteroCollection)s (the tree through `parentCollection`) | collections.js, collection.js `toJSON` |
| `savedSearches`, `savedSearchConditions`, `deletedSearches` | [`ZoteroSearch`](kovan_common::zotero::ZoteroSearch)es, conditions as stored | searches.js `_loadConditions`, search.js `toJSON` |

## Not read, and why

* **Feed libraries** (`libraries.type = 'feed'`, `feeds`, `feedItems`):
  RSS subscriptions that Zotero cleans up by itself (`cleanupReadAfter`),
  not part of the user's library and not in Zotero's own exports. Skipped
  and counted in [`ReadReport::notes`].
* **Sync bookkeeping** (`syncCache`, `syncDeleteLog`, `syncQueue`,
  `storageDeleteLog`, `synced`/`clientVersion` columns): state of the
  Zotero server sync, which kovan replaces with git.
* **`fulltextItems`** (and the `.zotero-ft-cache` files): a search index
  derived from the attachment files, which are read instead.
* **`retractedItems`**: Zotero's cached copy of Retraction Watch data,
  refreshed from a web service; not the user's data.
* **`groupItems`, `users`**: who created/modified a group item; personal
  data of other people, not in `toJSON`.
* **`syncedSettings`, `settings`** (beyond the account ids for URIs):
  UI preferences such as tag colours.
* **`proxies`, `translatorCache`, `custom*` tables**: application state;
  `customItemTypes`/`customFields` are unused upstream ("These shouldn't
  be used yet", userdata.sql).

## Deliberate differences from upstream, all stated

* **Unicode normalisation.** `setField` and `cleanData` NFC-normalise on
  load; this reader only trims. Zotero normalises on save as well, so
  values Zotero wrote are already NFC.
* **ISBN hyphenation** on load (item.js:829) is not ported; Zotero also
  hyphenates on save, so stored ISBNs are normally already hyphenated.
* **Saved-search conditions** that Zotero no longer registers are kept
  (upstream drops them on load); the registry (searchConditions.js) is not
  ported. The conversions `_loadConditions` applies are mirrored
  (`childNote` -> `note` + `resultLevel`, `itemTypeID` -> `itemType`,
  `0_KEY` -> `KEY`, NULL value -> `""`).
* **Rows upstream would throw on** (an unknown item or annotation type, an
  unknown creator type) are skipped and listed in [`ReadReport::skipped`]
  instead of failing the whole read.
* **`attachments:` paths** (relative to the linked-attachment base
  directory) are resolved only when [`ReadOptions::base_attachment_path`]
  is given: that directory is a Zotero *preference* (`baseAttachmentPath`
  in `prefs.js` of the Zotero profile), not stored in the data folder.

**Maturity: AI draft (1).** Verified code-to-code against databases built
from upstream's own schema files (`tests/zotero_local_library.rs`), and
its SQLite layer against real SQLite on the file-format cases (overflow
pages, deep B-trees, every serial type, freelists, WAL replay and
damaged WALs, page sizes 1024-65536, auto-vacuum; `format_tests.rs`);
not yet run on a real library by a human. Run on wasm32-unknown-unknown
once (2026-10-07, Node via `wasm-bindgen-test-runner`, outside the
repository): a Zotero database with its items only in the WAL read back
identical to the native read; there is no committed wasm runtime test
yet.

```rust
pub mod local_library { /* ... */ }
```

### Modules

## Module `import`

Import a read Zotero data folder into kovan: one [`KovanDocument`] per
literature item, with a report of what the import does not carry.

**What becomes a document.** Every top-level regular item (a book, an
article, ...) and every top-level ("standalone") attachment, in each user
and group library. Its child attachments and notes go with it in Zotero's
export shape (`attachments`, `notes`), so they are kept in
`KovanDocument::zotero_item` and survive kovan -> Zotero. The conversion
is `ZoteroItem::to_kovan_document` (kovan-common, #748), unchanged; this
module only adds `source_path` when that conversion leaves it empty: the
resolved file of the first PDF attachment (else the first attachment with
a file), since a stored file's location is known only to the reader.

**What the import does not carry, reported in [`Losses`]:** trashed items
(skipped unless [`ImportOptions::include_trashed`]); standalone notes (not
literature); annotations (a Zotero item kind the export shape has no slot
for; they stay in the read [`ZoteroLibrary`](kovan_common::zotero::ZoteroLibrary));
collection names and the collection tree (each document keeps only its
collection *keys*, in `zotero_item`); saved searches; which library a
document came from (the id is `zotero:<KEY>`, so two libraries can give
the same id: listed in [`Losses::duplicate_ids`]); the attachment files
themselves (referenced by path, never copied); and attachments whose file
is missing. Every document is `Visibility::Proprietary` (the conversion's
rule: Zotero records no redistribution right).

Nothing is written unless [`write_documents`] is called with a folder.
**Data policy (#747, workspace `DATA_POLICY.md`):** a Zotero library is the
user's own, usually proprietary, data; write it to the local corpus, never
into `reactor-literature` or this repository.

```rust
pub mod import { /* ... */ }
```

### Types

#### Struct `ImportOptions`

What to import.

```rust
pub struct ImportOptions {
    pub include_trashed: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `include_trashed` | `bool` | Import items in Zotero's trash too (default false). |

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
    fn clone(self: &Self) -> ImportOptions { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> ImportOptions { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &ImportOptions) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `ImportedDocument`

One imported document.

```rust
pub struct ImportedDocument {
    pub library_id: i64,
    pub key: String,
    pub document: kovan_common::KovanDocument,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `library_id` | `i64` | The Zotero library it came from. |
| `key` | `String` | The Zotero item key. |
| `document` | `kovan_common::KovanDocument` | The document. |

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
    fn clone(self: &Self) -> ImportedDocument { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &ImportedDocument) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `Losses`

What the import did not carry into kovan (see the module docs).

```rust
pub struct Losses {
    pub trashed_items_skipped: usize,
    pub standalone_notes: usize,
    pub annotations: usize,
    pub collections: usize,
    pub saved_searches: usize,
    pub duplicate_ids: Vec<String>,
    pub missing_files: Vec<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `trashed_items_skipped` | `usize` | Trashed top-level items not imported. |
| `standalone_notes` | `usize` | Standalone (top-level) notes not imported. |
| `annotations` | `usize` | Annotations, kept only in the read library. |
| `collections` | `usize` | Collections whose names and nesting are not in any document. |
| `saved_searches` | `usize` | Saved searches not imported. |
| `duplicate_ids` | `Vec<String>` | Document ids produced by more than one library. |
| `missing_files` | `Vec<String>` | Attachment keys whose file is not on disk (or unresolvable). |

##### Implementations

###### Methods

- ```rust
  pub fn summary(self: &Self) -> Vec<String> { /* ... */ }
  ```
  One line per non-empty loss, for a report.

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
    fn clone(self: &Self) -> Losses { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Losses { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &Losses) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `ZoteroImport`

The result of [`import`].

```rust
pub struct ZoteroImport {
    pub documents: Vec<ImportedDocument>,
    pub losses: Losses,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `documents` | `Vec<ImportedDocument>` | The documents, library by library, in item order. |
| `losses` | `Losses` | What was not carried. |

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
    fn clone(self: &Self) -> ZoteroImport { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> ZoteroImport { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &ZoteroImport) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
### Functions

#### Function `import`

Turn every library of `folder` into kovan documents (no disk writes).

```rust
pub fn import(folder: &super::ZoteroDataFolder, opts: &ImportOptions) -> ZoteroImport { /* ... */ }
```

#### Function `write_documents`

Write each document as pretty JSON to
`<target>/<user|group-ID>/<KEY>.json`. Never overwrites: an existing file
is an error (`AlreadyExists`) and nothing after it is written.

```rust
pub fn write_documents(folder: &super::ZoteroDataFolder, import: &ZoteroImport, target: &std::path::Path) -> std::io::Result<Vec<std::path::PathBuf>> { /* ... */ }
```

### Types

#### Enum `DbSource`

Which database file to read.

```rust
pub enum DbSource {
    LiveCopy,
    Backup,
}
```

##### Variants

###### `LiveCopy`

A private (in-memory) copy of `zotero.sqlite` (with its `-wal`).

###### `Backup`

A private (in-memory) copy of `zotero.sqlite.bak`, Zotero's own
backup.

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
    fn clone(self: &Self) -> DbSource { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> DbSource { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &DbSource) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `ReadOptions`

How to read a data folder.

```rust
pub struct ReadOptions {
    pub source: DbSource,
    pub fall_back_to_backup: bool,
    pub base_attachment_path: Option<std::path::PathBuf>,
    pub allow_newer_schema: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `source` | `DbSource` | Which file to read (default [`DbSource::LiveCopy`]). |
| `fall_back_to_backup` | `bool` | Read `zotero.sqlite.bak` when the copy of the live database fails<br>`PRAGMA quick_check` (default true). |
| `base_attachment_path` | `Option<std::path::PathBuf>` | Zotero's `baseAttachmentPath` preference, to resolve `attachments:`<br>linked-file paths (default none: they stay unresolved). |
| `allow_newer_schema` | `bool` | Read a database whose `compatibility` version is above<br>[`MAX_COMPATIBILITY`] (default false: refused, as Zotero refuses it). |

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
    fn clone(self: &Self) -> ReadOptions { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &ReadOptions) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `SchemaVersions`

The schema versions recorded in the database's `version` table.

```rust
pub struct SchemaVersions {
    pub userdata: i64,
    pub system: Option<i64>,
    pub triggers: Option<i64>,
    pub compatibility: Option<i64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `userdata` | `i64` | `userdata` (the last upgrade step run). |
| `system` | `Option<i64>` | `system`. |
| `triggers` | `Option<i64>` | `triggers`. |
| `compatibility` | `Option<i64>` | `compatibility`. |

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
    fn clone(self: &Self) -> SchemaVersions { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &SchemaVersions) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Enum `LibraryKind`

What kind of Zotero library this is.

```rust
pub enum LibraryKind {
    User,
    Group {
        group_id: i64,
        name: String,
        description: String,
    },
}
```

##### Variants

###### `User`

My Library.

###### `Group`

A group library.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `group_id` | `i64` | The group's id on zotero.org. |
| `name` | `String` | The group's name. |
| `description` | `String` | The group's description (HTML). |

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
    fn clone(self: &Self) -> LibraryKind { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &LibraryKind) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Enum `AttachmentFile`

Where an attachment's file is, resolved as `Item#getFilePath` resolves it
(item.js:2899).

```rust
pub enum AttachmentFile {
    Stored(std::path::PathBuf),
    Linked(std::path::PathBuf),
    LinkedRelative {
        relative: String,
        resolved: Option<std::path::PathBuf>,
    },
    NoFile,
    Unresolvable(String),
}
```

##### Variants

###### `Stored`

A stored file: `<data dir>/storage/<KEY>/<filename>`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `std::path::PathBuf` |  |

###### `Linked`

A linked file at an absolute path.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `std::path::PathBuf` |  |

###### `LinkedRelative`

A linked file relative to the base attachment directory
(`attachments:` prefix); `resolved` when
[`ReadOptions::base_attachment_path`] was given.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `relative` | `String` | The path after `attachments:`, as stored. |
| `resolved` | `Option<std::path::PathBuf>` | The absolute path, if the base directory is known. |

###### `NoFile`

A linked URL: no file.

###### `Unresolvable`

No usable path; the reason is upstream's (empty path, invalid stored
path, an old Mac alias record, ...).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

##### Implementations

###### Methods

- ```rust
  pub fn path(self: &Self) -> Option<&Path> { /* ... */ }
  ```
  The absolute path, when there is one.

- ```rust
  pub fn exists(self: &Self) -> bool { /* ... */ }
  ```
  Whether the file exists now (checked on the file system).

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
    fn clone(self: &Self) -> AttachmentFile { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &AttachmentFile) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `LocalLibrary`

One Zotero library of the data folder.

```rust
pub struct LocalLibrary {
    pub library_id: i64,
    pub kind: LibraryKind,
    pub editable: bool,
    pub files_editable: bool,
    pub version: i64,
    pub uri: String,
    pub contents: kovan_common::zotero::ZoteroLibrary,
    pub files: std::collections::BTreeMap<String, AttachmentFile>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `library_id` | `i64` | `libraries.libraryID` (1 is My Library). |
| `kind` | `LibraryKind` | User or group. |
| `editable` | `bool` | `libraries.editable`. |
| `files_editable` | `bool` | `libraries.filesEditable`. |
| `version` | `i64` | `libraries.version` (the last synced library version). |
| `uri` | `String` | The library URI, e.g. `http://zotero.org/users/local/abcd1234` or<br>`http://zotero.org/groups/123` (uri.js `getLibraryURI`); relation<br>objects point at `<uri>/items/<KEY>`. |
| `contents` | `kovan_common::zotero::ZoteroLibrary` | Collections, items (with children) and saved searches. |
| `files` | `std::collections::BTreeMap<String, AttachmentFile>` | Attachment key -> its file. |

##### Implementations

###### Methods

- ```rust
  pub fn item_uri(self: &Self, key: &str) -> String { /* ... */ }
  ```
  The URI of an item of this library (uri.js `getItemURI`).

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
    fn clone(self: &Self) -> LocalLibrary { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &LocalLibrary) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `ReadIssue`

A row that was skipped or a value that was dropped.

```rust
pub struct ReadIssue {
    pub library_id: i64,
    pub what: String,
    pub reason: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `library_id` | `i64` | The library. |
| `what` | `String` | The object, e.g. `item ABCD2345` or `item ABCD2345 field foo`. |
| `reason` | `String` | Why. |

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
    fn clone(self: &Self) -> ReadIssue { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &ReadIssue) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `ReadReport`

What the read could not take over as stored.

```rust
pub struct ReadReport {
    pub skipped: Vec<ReadIssue>,
    pub dropped: Vec<ReadIssue>,
    pub notes: Vec<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `skipped` | `Vec<ReadIssue>` | Objects skipped whole (unknown item type, ...). |
| `dropped` | `Vec<ReadIssue>` | Values dropped from an object that was read (a field invalid for the<br>item type, which upstream also ignores; an unknown creator type, ...). |
| `notes` | `Vec<String>` | Informational notes (which file was read, libraries skipped, ...). |

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
    fn clone(self: &Self) -> ReadReport { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> ReadReport { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &ReadReport) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `ZoteroDataFolder`

A whole Zotero data folder.

```rust
pub struct ZoteroDataFolder {
    pub data_dir: std::path::PathBuf,
    pub source: DbSource,
    pub schema: SchemaVersions,
    pub libraries: Vec<LocalLibrary>,
    pub report: ReadReport,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `data_dir` | `std::path::PathBuf` | The data folder. |
| `source` | `DbSource` | Which database file was read. |
| `schema` | `SchemaVersions` | Its schema versions. |
| `libraries` | `Vec<LocalLibrary>` | User library first, then groups, by `libraryID`. |
| `report` | `ReadReport` | What could not be read as stored. |

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
    fn clone(self: &Self) -> ZoteroDataFolder { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &ZoteroDataFolder) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Enum `ZoteroDbError`

Why a data folder could not be read.

```rust
pub enum ZoteroDbError {
    NoDatabase(std::path::PathBuf),
    Io {
        path: std::path::PathBuf,
        message: String,
    },
    Sqlite(String),
    Corrupt(String),
    NotZotero(String),
    TooOld {
        userdata: i64,
    },
    TooNew {
        compatibility: i64,
    },
}
```

##### Variants

###### `NoDatabase`

Neither `zotero.sqlite` nor (when allowed) `zotero.sqlite.bak` exists.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `std::path::PathBuf` |  |

###### `Io`

Copying the database failed.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `path` | `std::path::PathBuf` | The file. |
| `message` | `String` | The error. |

###### `Sqlite`

SQLite reported an error.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Corrupt`

The copy failed `PRAGMA quick_check` (and no usable backup).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `NotZotero`

The file is not a Zotero database (no `version` table / `userdata`).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `TooOld`

Older than Zotero 5.0 (`userdata` < [`MIN_USERDATA_VERSION`]).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `userdata` | `i64` | The `userdata` version found. |

###### `TooNew`

Newer than this reader knows (`compatibility` > [`MAX_COMPATIBILITY`]).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `compatibility` | `i64` | The `compatibility` version found. |

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
    fn clone(self: &Self) -> ZoteroDbError { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
- **Error**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(e: turso_core::LimboError) -> Self { /* ... */ }
    ```

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ZoteroDbError) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `DatabaseFiles`

The database files of a Zotero data folder, as bytes: what the reader
needs when there is no file system (a browser, where the user picks the
files) or when the caller has the files already. [`read_data_folder`]
builds one from a directory with [`DatabaseFiles::from_data_folder`].

`zotero.sqlite.bak` is not here: it is only read when the live database
fails, so [`read_database_files`] takes it separately.

```rust
pub struct DatabaseFiles {
    pub database: Option<Vec<u8>>,
    pub wal: Option<Vec<u8>>,
    pub journal: Option<Vec<u8>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `database` | `Option<Vec<u8>>` | `zotero.sqlite`. |
| `wal` | `Option<Vec<u8>>` | `zotero.sqlite-wal`: committed transactions not yet checkpointed<br>into `zotero.sqlite`. Without it, recent changes are missing. |
| `journal` | `Option<Vec<u8>>` | `zotero.sqlite-journal`: a hot rollback journal makes the live file<br>unreadable here (see the module docs, "Never touching the live<br>database"). |

##### Implementations

###### Methods

- ```rust
  pub fn from_data_folder(data_dir: &Path) -> Result<DatabaseFiles, ZoteroDbError> { /* ... */ }
  ```
  Read `zotero.sqlite`, `zotero.sqlite-wal` and `zotero.sqlite-journal`

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
    fn clone(self: &Self) -> DatabaseFiles { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> DatabaseFiles { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &DatabaseFiles) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
### Functions

#### Function `read_data_folder`

Read a Zotero data folder: the directory holding `zotero.sqlite` and
`storage/` (Zotero's "Data Directory Location"). Read-only; see the
module docs for how the live database is left untouched.

```rust
pub fn read_data_folder(data_dir: &std::path::Path, opts: &ReadOptions) -> Result<ZoteroDataFolder, ZoteroDbError> { /* ... */ }
```

#### Function `read_database_files`

Read a Zotero database from memory: the same read as
[`read_data_folder`], for callers without a file system (wasm32 in a
browser) or with the bytes already in hand. Works on every target.

* `files`: `zotero.sqlite` and its `-wal`/`-journal`.
* `backup`: called only if `zotero.sqlite.bak` is needed (asked for with
  [`DbSource::Backup`], or the live database failed and
  [`ReadOptions::fall_back_to_backup`] is set); it returns `Ok(None)`
  when there is none, so `|| Ok(None)` means "no backup".
* `data_dir`: where the data folder is (or was). Only used to build the
  paths of attachment files (`<data_dir>/storage/<KEY>/<file>`) and in
  messages; nothing is read from it.

```
use kovan_literature::zotero::local_library::{
    read_database_files, DatabaseFiles, ReadOptions, ZoteroDbError,
};
// Not a database: refused, and there is no backup to fall back to.
let files = DatabaseFiles { database: Some(b"not sqlite".to_vec()), ..Default::default() };
let read = read_database_files(&files, || Ok(None), "/zotero".as_ref(), &ReadOptions::default());
assert!(matches!(read, Err(ZoteroDbError::Sqlite(_) | ZoteroDbError::Corrupt(_))));
```

```rust
pub fn read_database_files</* synthetic */ impl FnOnce() -> Result<Option<Vec<u8>>, ZoteroDbError>: FnOnce() -> Result<Option<Vec<u8>>, ZoteroDbError>>(files: &DatabaseFiles, backup: impl FnOnce() -> Result<Option<Vec<u8>>, ZoteroDbError>, data_dir: &std::path::Path, opts: &ReadOptions) -> Result<ZoteroDataFolder, ZoteroDbError> { /* ... */ }
```

### Constants and Statics

#### Constant `MIN_USERDATA_VERSION`

The oldest `userdata` schema version read (Zotero 5.0's migration step,
schema.js:3012).

```rust
pub const MIN_USERDATA_VERSION: i64 = 80;
```

#### Constant `BUNDLED_USERDATA_VERSION`

The `userdata` version of the upstream `userdata.sql` this port follows.

```rust
pub const BUNDLED_USERDATA_VERSION: i64 = 130;
```

#### Constant `MAX_COMPATIBILITY`

Zotero's `_maxCompatibility` (schema.js:45) at the upstream commit.

```rust
pub const MAX_COMPATIBILITY: i64 = 9;
```

## Module `search`

Zotero's "Add Item by Identifier" (GitHub #756): find a DOI, ISBN,
arXiv ID, ADS bibcode or PubMed ID in text ([`extract_identifiers`]) and
fetch its record with Zotero's search translators ([`SearchTranslator`]).

**This module never touches the network.** A search runs until it needs
an HTTP answer it does not have, and then stops and returns the request
([`SearchStep::Pending`]); the caller fetches it however its target can
(kovan's native `ureq` backend, a browser `fetch`, a recorded fixture, a
fake in a test), hands the answer back ([`LookupSession::answer`]) and
steps again. The translators are deterministic given their answers, so
re-running from the start is exact; a lookup makes a handful of
requests, so the repeated work is negligible. Synchronous callers use
[`LookupSession::run_with`] with a closure.

```text
text --extract_identifiers--> Identifier --setIdentifier--> search item
  --detectSearch on every SearchTranslator (priority order)--> candidates
  --doSearch on the first; none found -> the next--> items
  --Search/Web/Base _itemDone--> itemToAPIJSON --> ZoteroItem
```

Every failure is a [`LookupError`] value: offline, connect/DNS, timeout,
HTTP status, rate limiting (429 with `Retry-After`), malformed response,
not found, unsupported on this target. Nothing panics on network data.

Verified code-to-code against upstream run in-process on the same
recorded responses: `tests/zotero_search.rs`, fixtures from
`scripts/zotero-search-reference.mjs`.

```rust
pub mod search { /* ... */ }
```

### Modules

## Module `context`

[`SearchContext`]: the `Zotero` object of a search translation, and the
framework's handling of each item a search translator completes.

```rust
pub mod context { /* ... */ }
```

### Types

#### Enum `SearchError`

Why a search translator stopped.

```rust
pub enum SearchError {
    Pending(super::http::HttpRequest),
    Http(super::http::HttpFailure),
    Translator(String),
}
```

##### Variants

###### `Pending`

The run needs this request answered before it can go on (not a
failure: the caller fetches it and runs the search again). A
translator must never swallow this one.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::http::HttpRequest` |  |

###### `Http`

A request failed.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::http::HttpFailure` |  |

###### `Translator`

The translator (or a child import translator) threw.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

##### Implementations

###### Methods

- ```rust
  pub fn is_pending(self: &Self) -> bool { /* ... */ }
  ```
  Whether this is [`SearchError::Pending`] (which no `catch` in a

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
    fn clone(self: &Self) -> SearchError { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(e: HttpFailure) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(e: TranslateError) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(e: XPathError) -> Self { /* ... */ }
    ```

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SearchError) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `SearchOptions`

What a search runs with.

```rust
pub struct SearchOptions {
    pub env: crate::zotero::framework::options::TranslationEnv,
    pub hidden_prefs: crate::zotero::framework::item::JsObject,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `env` | `crate::zotero::framework::options::TranslationEnv` | The environment (dates, the clock: `now_unix_secs` fixes "now" for<br>the `accessDate` the framework stamps). |
| `hidden_prefs` | `crate::zotero::framework::item::JsObject` | Hidden preferences (`Z.getHiddenPref`), e.g. `CrossrefREST.email`<br>(the translation-server's `translators.CrossrefREST.email` config).<br>Empty by default: nothing user-identifying is sent unless the user<br>sets it. |

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
    fn clone(self: &Self) -> SearchOptions { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &SearchOptions) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `ChildSearch`

What a child search translation produced (`Zotero.loadTranslator("search")`):
the items its `itemDone` handler received, and the error its `error`
handler received (upstream's caller chooses whether to care).

```rust
pub struct ChildSearch {
    pub items: Vec<crate::zotero::framework::item::TranslatorItem>,
    pub error: Option<SearchError>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `items` | `Vec<crate::zotero::framework::item::TranslatorItem>` | Items, after the child's `_itemDone`. |
| `error` | `Option<SearchError>` | The failure, if the child failed or found nothing<br>("No items returned from any translator"). |

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
    fn clone(self: &Self) -> ChildSearch { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &ChildSearch) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `SearchContext`

The `Zotero` object of a search translation (and of the search
translations it loads).

```rust
pub struct SearchContext {
    pub options: SearchOptions,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `options` | `SearchOptions` | The options. |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(cache: HttpCache, options: SearchOptions) -> Self { /* ... */ }
  ```
  A context answering requests from `cache`.

- ```rust
  pub fn requests(self: &Self) -> &[HttpRequest] { /* ... */ }
  ```
  The requests made so far, in order.

- ```rust
  pub fn into_cache(self: Self) -> HttpCache { /* ... */ }
  ```
  The cache back.

- ```rust
  pub fn meta(self: &Self) -> Option<&'static TranslatorMetadata> { /* ... */ }
  ```
  The running translator's header.

- ```rust
  pub fn child_search(self: &mut Self, t: SearchTranslator, search: &JsObject) -> Result<ChildSearch, SearchError> { /* ... */ }
  ```
  `Zotero.loadTranslator("search")` + `setTranslator(t)` +

- ```rust
  pub fn child_import(self: &mut Self, t: Translator, text: &str) -> Result<Vec<TranslatorItem>, SearchError> { /* ... */ }
  ```
  `Zotero.loadTranslator("import")` + `setTranslator(t)` +

- ```rust
  pub fn complete(self: &mut Self, item: TranslatorItem) -> Result<(), SearchError> { /* ... */ }
  ```
  `item.complete()` in a search translator:

- ```rust
  pub fn get_hidden_pref(self: &Self, name: &str) -> Option<Value> { /* ... */ }
  ```
  `Z.getHiddenPref(name)`.

- ```rust
  pub fn now_iso(self: &Self) -> String { /* ... */ }
  ```
  "Now" as `Zotero.Date.dateToISO(new Date())`.

- ```rust
  pub fn str_to_iso(self: &Self, s: &str) -> Option<String> { /* ... */ }
  ```
  `ZU.strToISO(s)` in this environment.

- ```rust
  pub fn request(self: &mut Self, url: &str, opts: &RequestOptions) -> Result<HttpResponse, SearchError> { /* ... */ }
  ```
  `request(url, options)` (utilities_translate.js:307-370): the

- ```rust
  pub fn request_text(self: &mut Self, url: &str, opts: &RequestOptions) -> Result<String, SearchError> { /* ... */ }
  ```
  `requestText(url, options)`.

- ```rust
  pub fn request_json(self: &mut Self, url: &str, opts: &RequestOptions) -> Result<Value, SearchError> { /* ... */ }
  ```
  `requestJSON(url, options)`.

- ```rust
  pub fn request_document(self: &mut Self, url: &str, opts: &RequestOptions) -> Result<(XmlDocument, String), SearchError> { /* ... */ }
  ```
  `requestDocument(url, options)` (HTML or XML; a meta refresh within

- ```rust
  pub fn do_get(self: &mut Self, url: &str) -> Result<String, SearchError> { /* ... */ }
  ```
  `ZU.doGet(url, callback)` (utilities_translate.js:477-524): the

- ```rust
  pub fn process_document(self: &mut Self, url: &str) -> Result<(XmlDocument, String), SearchError> { /* ... */ }
  ```
  `ZU.processDocuments(url, processor)` for one URL: the document and

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
    fn clone(self: &Self) -> SearchContext { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
### Functions

#### Function `epoch_to_iso`

`Zotero.Date.dateToISO` of a Unix time.

```rust
pub fn epoch_to_iso(secs: i64) -> String { /* ... */ }
```

#### Function `clean_isbns`

Web `_itemDone`'s ISBN clean-up (:671-688): every match of
`/\b(?:97[89][\s\x2D\xAD‐-―⁃−]*)?(?:\d[...]*){9}[\dx](?![\x2D\xAD‐-―⁃−])\b/gi`
that `cleanISBN` accepts, as ISBN-13, without duplicates (an invalid
match restarts the search one character later).

```rust
pub fn clean_isbns(s: &str) -> Vec<String> { /* ... */ }
```

## Module `extract`

Finding identifiers in text, as Zotero's "Add Item by Identifier" box
does: DOIs first, then ISBNs, then arXiv IDs, then ADS bibcodes, then
PubMed IDs (each tried only when the earlier kinds found nothing).

The upstream regular expressions use lookaheads, which the `regex`
crate does not have, so each is a small hand-written matcher that
reproduces the expression's backtracking on ASCII-class input
(JavaScript's `\d`, `\b`, `[A-Za-z]` are ASCII; `\s` is JavaScript's
whitespace, [`js::is_space`]). Quirks kept: ADS bibcodes and PMIDs are
not de-duplicated (upstream tests a match object, not the string,
against its `Set`).

```rust
pub mod extract { /* ... */ }
```

### Types

#### Enum `Identifier`

One identifier `extractIdentifiers` found.

```rust
pub enum Identifier {
    Doi(String),
    Isbn(String),
    ArXiv(String),
    AdsBibcode(String),
    Pmid(String),
}
```

##### Variants

###### `Doi`

`{DOI}`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Isbn`

`{ISBN}` (cleaned: digits and X, no hyphens).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `ArXiv`

`{arXiv}` (without its version).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `AdsBibcode`

`{adsBibcode}`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Pmid`

`{PMID}`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

##### Implementations

###### Methods

- ```rust
  pub fn value(self: &Self) -> &str { /* ... */ }
  ```
  The identifier's value.

- ```rust
  pub fn kind(self: &Self) -> &'static str { /* ... */ }
  ```
  The property name upstream uses (`DOI`, `ISBN`, `arXiv`,

- ```rust
  pub fn to_value(self: &Self) -> Value { /* ... */ }
  ```
  As upstream's object (`{"DOI": "10..."}`).

- ```rust
  pub fn search_item(self: &Self) -> JsObject { /* ... */ }
  ```
  The search item `Zotero.Translate.Search#setIdentifier` builds

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
    fn clone(self: &Self) -> Identifier { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Identifier) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
### Functions

#### Function `extract_identifiers`

`Zotero.Utilities.extractIdentifiers(text)` (utilities.js:383-470).

```rust
pub fn extract_identifiers(text: &str) -> Vec<Identifier> { /* ... */ }
```

#### Function `identifier_for_search`

What the translation-server's `/search` takes from the text
(searchEndpoint.js:40-47): the first identifier, except that a PMID
counts only when it is all the text (optionally `pmid:`-prefixed).

```rust
pub fn identifier_for_search(text: &str) -> Option<Identifier> { /* ... */ }
```

## Module `http`

The HTTP layer the search translators see. **No networking happens
here**: the library builds [`HttpRequest`]s and interprets
[`HttpResponse`]s exactly as Zotero's framework does; who sends the
request is the caller's business (see [`super::lookup`]):

```text
translator --request_text/json/document/do_get--> SearchContext
    --HttpRequest--> [answer cached?] --no--> SearchStep::Pending(request)
                           |yes                      | caller fetches it (native
                           v                         | ureq in kovan, browser fetch,
    status / Content-Type checks, charset decode,    | a recorded fixture, a fake)
    JSON parse, HTML/XML document                    v
                                          answer cached, search re-run
```

Every failure is a value ([`FetchError`] from the caller's backend,
[`HttpFailure`] after Zotero's checks); nothing here panics on network
data.

```rust
pub mod http { /* ... */ }
```

### Types

#### Struct `HttpRequest`

An HTTP request as Zotero's `Zotero.HTTP.request` would send it. The
headers are the ones Zotero sets (`Accept: */*` by default, the
translator's own, a default `Content-Type` on a POST body), in order;
the `User-Agent` is the backend's to add.

```rust
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `method` | `String` | "GET", "POST", ... |
| `url` | `String` | The URL as `new URL(url).href` serializes it. |
| `headers` | `Vec<(String, String)>` | Request headers, in order, without `User-Agent`. |
| `body` | `Option<String>` | The body (POST), as sent. |

##### Implementations

###### Methods

- ```rust
  pub fn key(self: &Self) -> String { /* ... */ }
  ```
  What identifies a request for caching and replay: method, URL and

- ```rust
  pub fn header(self: &Self, name: &str) -> Option<&str> { /* ... */ }
  ```
  A header's value (case-insensitive name).

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
    fn clone(self: &Self) -> HttpRequest { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &HttpRequest) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `HttpResponse`

What came back.

```rust
pub struct HttpResponse {
    pub status: u16,
    pub url: String,
    pub content_type: Option<String>,
    pub retry_after: Option<String>,
    pub body: Vec<u8>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `status` | `u16` | The status code. |
| `url` | `String` | The final URL (after redirects). |
| `content_type` | `Option<String>` | The `Content-Type` header, if any. |
| `retry_after` | `Option<String>` | The `Retry-After` header, if any (for a 429). |
| `body` | `Vec<u8>` | The body, decompressed, as bytes. |

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
    fn clone(self: &Self) -> HttpResponse { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &HttpResponse) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Enum `FetchError`

Why a backend could not produce a response at all. The backend (native,
browser, fake) chooses the variant; nothing below a response's status is
a `FetchError`.

```rust
pub enum FetchError {
    Offline,
    Connect(String),
    Timeout,
    TooLarge,
    Unsupported(String),
    Other(String),
}
```

##### Variants

###### `Offline`

No network (the machine is offline).

###### `Connect`

DNS resolution or the TCP/TLS connection failed.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Timeout`

The connect or read timeout elapsed.

###### `TooLarge`

The response was larger than the backend accepts.

###### `Unsupported`

Not possible on this target (e.g. a browser blocking a cross-origin
request, or a build without a network backend).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Other`

Anything else the backend reports.

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
    fn clone(self: &Self) -> FetchError { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &FetchError) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Enum `HttpFailure`

A request that failed, after Zotero's checks.

```rust
pub enum HttpFailure {
    Fetch(FetchError),
    Status {
        code: u16,
        url: String,
    },
    RateLimited {
        url: String,
        retry_after: Option<String>,
    },
    UnsupportedFormat(String),
    Malformed(String),
    BadUrl(String),
}
```

##### Variants

###### `Fetch`

The backend could not fetch it.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `FetchError` |  |

###### `Status`

A status Zotero does not count as success (2xx by default).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `code` | `u16` | The status code. |
| `url` | `String` | The URL requested. |

###### `RateLimited`

HTTP 429, with `Retry-After` when the server sent one.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `url` | `String` | The URL requested. |
| `retry_after` | `Option<String>` | The `Retry-After` header. |

###### `UnsupportedFormat`

No `Content-Type` header, or one the request cannot take
(`Zotero.HTTP.UnsupportedFormatError`).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Malformed`

The body could not be read as asked (bad JSON, bad XML, an unknown
charset).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `BadUrl`

The URL is not one Zotero would request (`new URL` throws, or not
http/https).

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
    fn clone(self: &Self) -> HttpFailure { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(e: HttpFailure) -> Self { /* ... */ }
    ```

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &HttpFailure) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `RequestOptions`

Options for [`super::SearchContext::request`] (the translator's
`options` object).

```rust
pub struct RequestOptions {
    pub method: Option<String>,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
    pub success_codes: Option<Vec<u16>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `method` | `Option<String>` | `method` (default GET). |
| `headers` | `Vec<(String, String)>` | `headers`, in order. |
| `body` | `Option<String>` | `body`. |
| `success_codes` | `Option<Vec<u16>>` | `successCodes`: `None` is 2xx; `Some(vec![])` is `false` (any). |

##### Implementations

###### Methods

- ```rust
  pub fn headers(headers: &[(&str, &str)]) -> Self { /* ... */ }
  ```
  GET with these headers.

- ```rust
  pub fn post</* synthetic */ impl Into<String>: Into<String>>(body: impl Into<String>, headers: &[(&str, &str)]) -> Self { /* ... */ }
  ```
  POST `body` with these headers.

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
    fn clone(self: &Self) -> RequestOptions { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> RequestOptions { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &RequestOptions) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `HttpCache`

Answers already obtained, by request key, in the order they were
obtained. The `n`-th time a run asks for the same request it gets the
`n`-th answer; when there is none, the run stops and asks the caller.

```rust
pub struct HttpCache {
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
  pub fn new() -> Self { /* ... */ }
  ```
  An empty cache.

- ```rust
  pub fn insert(self: &mut Self, req: &HttpRequest, answer: Result<HttpResponse, FetchError>) { /* ... */ }
  ```
  Record the answer to `req`.

- ```rust
  pub fn get(self: &Self, req: &HttpRequest, n: usize) -> Option<&Result<HttpResponse, FetchError>> { /* ... */ }
  ```
  The `n`-th answer to `req`, if obtained.

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  How many answers are held.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether nothing is held.

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
    fn clone(self: &Self) -> HttpCache { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> HttpCache { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &HttpCache) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
### Functions

#### Function `build_request`

Build the request `Zotero.HTTP.request(method, url, options)` sends:
`Object.assign({'User-Agent', Accept: '*/*'}, headers)` (the user agent
left to the backend), and for a non-GET/HEAD body a default
`Content-Type: application/x-www-form-urlencoded` (http.js:104-125).

```rust
pub fn build_request(url: &str, opts: &RequestOptions) -> Result<HttpRequest, HttpFailure> { /* ... */ }
```

#### Function `check_status`

The status check of `customRequest` (http.js:397-413): success codes
given, `false` (any), or 2xx.

```rust
pub fn check_status(req: &HttpRequest, resp: &HttpResponse, success_codes: Option<&[u16]>) -> Result<(), HttpFailure> { /* ... */ }
```

#### Function `mime`

A MIME type's essence (`type/subtype`, lower case) and its `charset`.

```rust
pub fn mime(content_type: &str) -> (String, Option<String>) { /* ... */ }
```

#### Function `is_html`

whatwg-mimetype `isHTML`.

```rust
pub fn is_html(essence: &str) -> bool { /* ... */ }
```

#### Function `is_xml`

whatwg-mimetype `isXML`.

```rust
pub fn is_xml(essence: &str) -> bool { /* ... */ }
```

#### Function `decode_text`

The body as text, as `Zotero.HTTP.request` decodes a `text` response
(http.js:191-204: the Content-Type charset through iconv-lite, UTF-8
when there is none or it is unknown; iconv-lite drops a leading BOM).
Charsets ported: UTF-8, ISO-8859-1/Latin-1, US-ASCII, Windows-1252;
another charset iconv-lite knows is reported as
[`HttpFailure::Malformed`] rather than mis-decoded.

```rust
pub fn decode_text(resp: &HttpResponse) -> Result<String, HttpFailure> { /* ... */ }
```

#### Function `decode_json`

`JSON.parse(body.toString())` (http.js:188-190; UTF-8, a BOM is a
syntax error as in JavaScript).

```rust
pub fn decode_json(resp: &HttpResponse) -> Result<serde_json::Value, HttpFailure> { /* ... */ }
```

#### Function `decode_document`

A `document` response (http.js:159-187): HTML through the HTML parser,
XML through the XML parser; any other type is
`UnsupportedFormatError`. Returns the document and, for HTML, a meta
refresh target (within 15 s) to follow.

```rust
pub fn decode_document(resp: &HttpResponse) -> Result<(super::super::framework::xml::XmlDocument, Option<String>), HttpFailure> { /* ... */ }
```

#### Function `resolve_url`

`url.resolve(base, href)` for the cases translators produce: an absolute
URL is returned as is, a scheme-relative or root-relative one is joined
to the base's origin, anything else to the base's directory.

```rust
pub fn resolve_url(base: &str, href: &str) -> String { /* ... */ }
```

#### Function `url_href`

`new URL(url).href` for an absolute http(s) URL (WHATWG URL
serialization): scheme and host lower-cased, a default port dropped, an
empty path made "/", backslashes in the path read as "/", and the path,
query and fragment percent-encoded with their WHATWG percent-encode
sets (non-ASCII as UTF-8). A host that is not ASCII (IDNA) is refused
rather than mis-encoded.

```rust
pub fn url_href(url: &str) -> Result<String, HttpFailure> { /* ... */ }
```

#### Function `encode_uri_component`

JavaScript `encodeURIComponent`.

```rust
pub fn encode_uri_component(s: &str) -> String { /* ... */ }
```

## Module `translators`

The search translators, one module each. Each has `METADATA`,
`detect_search(&JsObject) -> bool` and
`do_search(&mut SearchContext, &JsObject) -> Result<(), SearchError>`;
[`super::SearchTranslator`] dispatches to them.

```rust
pub mod translators { /* ... */ }
```

### Modules

## Module `ads_bibcode`

The ADS Bibcode search translator: NASA ADS's RIS export for a
bibcode, imported by RIS and corrected (theses, proceedings, arXiv).

As upstream, the export API is called with the anonymous access token
ADS's own bootstrap endpoint hands out; the token is fetched at lookup
time (only when the user runs a lookup) and never stored. The port's
tests use a kovan-authored fixture with a placeholder token; ADS was
never contacted to make it (DATA_POLICY.md).

```rust
pub mod ads_bibcode { /* ... */ }
```

### Functions

#### Function `detect_search`

`detectSearch` (:57-59).

```rust
pub fn detect_search(search: &crate::zotero::framework::item::JsObject) -> bool { /* ... */ }
```

#### Function `do_search`

`doSearch` (:61-65).

```rust
pub fn do_search(ctx: &mut crate::zotero::search::SearchContext, search: &crate::zotero::framework::item::JsObject) -> Result<(), crate::zotero::search::SearchError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `arxiv`

The arXiv.org search translator: the arXiv API's Atom feed for one ID,
each entry a preprint; an entry with a DOI is completed from DOI
Content Negotiation (a child search).

```rust
pub mod arxiv { /* ... */ }
```

### Functions

#### Function `detect_search`

`detectSearch` (:240-242): `!!item.arXiv`.

```rust
pub fn detect_search(search: &crate::zotero::framework::item::JsObject) -> bool { /* ... */ }
```

#### Function `do_search`

`doSearch` (:244-248).

```rust
pub fn do_search(ctx: &mut crate::zotero::search::SearchContext, search: &crate::zotero::framework::item::JsObject) -> Result<(), crate::zotero::search::SearchError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `bnf_isbn`

The BnF ISBN search translator: an SRU query to the Bibliothèque
nationale de France catalogue; each MarcXchange record is re-labelled as
MARCXML and imported by MARCXML.

Kept from upstream: both XPaths inside the record loop start with `//`,
so they search the whole response, not the record (with several records,
every record is imported once per record).

```rust
pub mod bnf_isbn { /* ... */ }
```

### Functions

#### Function `detect_search`

`detectSearch` (:44-46): `!!item.ISBN`.

```rust
pub fn detect_search(search: &crate::zotero::framework::item::JsObject) -> bool { /* ... */ }
```

#### Function `do_search`

`doSearch` (:48-87).

```rust
pub fn do_search(ctx: &mut crate::zotero::search::SearchContext, search: &crate::zotero::framework::item::JsObject) -> Result<(), crate::zotero::search::SearchError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `camara_isbn`

The Câmara Brasileira do Livro ISBN search translator: Brazilian ISBNs
(groups 65 and 85) looked up in the CBL's ISBN search index (an Azure
Cognitive Search endpoint), each result mapped to a book.

**Deviation from upstream (DATA_POLICY.md, #756).** Upstream sends the
index's query key as an `api-key` header written into the translator.
The workspace's data policy forbids API keys in the repository, so the
port does not carry it: the key is read from the hidden preference
`CamaraBrasileiraDoLivro.apiKey` ([`SearchContext::get_hidden_pref`]),
which the user sets. Unset, the translator fails with a message saying
so, and an identifier search goes on to the next ISBN translator.
Everything else (URL, body, other headers, the mapping) is upstream's.

Not reproduced: `cleanData` in `detectSearch` also overwrites the
shared search item's `ISBN` with its cleaned form (upstream passes the
same object to every translator); the port's detection does not mutate
the search item. An identifier search's ISBN is already clean, so the
two agree on every input `extractIdentifiers` produces.

```rust
pub mod camara_isbn { /* ... */ }
```

### Functions

#### Function `detect_search`

`detectSearch` (:37-40).

```rust
pub fn detect_search(search: &crate::zotero::framework::item::JsObject) -> bool { /* ... */ }
```

#### Function `do_search`

`doSearch` (:42-77).

```rust
pub fn do_search(ctx: &mut crate::zotero::search::SearchContext, search: &crate::zotero::framework::item::JsObject) -> Result<(), crate::zotero::search::SearchError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

#### Constant `API_KEY_PREF`

The hidden preference holding the index's query key.

```rust
pub const API_KEY_PREF: &str = "CamaraBrasileiraDoLivro.apiKey";
```

## Module `crossref_rest`

The Crossref REST search translator: `api.crossref.org/works?filter=doi:`
mapped to an item. Its `detectSearch` is `false` upstream, so identifier
lookup never picks it; it runs only when called by translator ID
([`crate::zotero::search::LookupSession::forced`]).

Item properties are created in upstream's assignment order, with an
assignment of `undefined`/`null` kept as a `null` placeholder (dropped by
`_itemDone` as upstream drops it), because `itemToAPIJSON` reads the
properties in order and base-field mapping (publisher/institution/
university) depends on it.

```rust
pub mod crossref_rest { /* ... */ }
```

### Functions

#### Function `detect_search`

`detectSearch` (:357-359).

```rust
pub fn detect_search(_search: &crate::zotero::framework::item::JsObject) -> bool { /* ... */ }
```

#### Function `do_search`

`doSearch` (:361-398).

```rust
pub fn do_search(ctx: &mut crate::zotero::search::SearchContext, search: &crate::zotero::framework::item::JsObject) -> Result<(), crate::zotero::search::SearchError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `doi_content_negotiation`

The DOI Content Negotiation search translator: `https://doi.org/<DOI>`
asked for DataCite JSON, Crossref Unixref XML or CSL JSON, imported by
the matching import translator.

Upstream's Crossref-outage branch (:69-101) is `if (false)` and is not
ported.

```rust
pub mod doi_content_negotiation { /* ... */ }
```

### Functions

#### Function `detect_search`

`detectSearch` (:37-39).

```rust
pub fn detect_search(search: &crate::zotero::framework::item::JsObject) -> bool { /* ... */ }
```

#### Function `do_search`

`doSearch` (:62-66).

```rust
pub fn do_search(ctx: &mut crate::zotero::search::SearchContext, search: &crate::zotero::framework::item::JsObject) -> Result<(), crate::zotero::search::SearchError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `eidr`

The EIDR search translator: an EIDR content record (a DOI under
10.5240) resolved at resolve.eidr.org, read as a film, TV broadcast or
video recording.

One deliberate difference: upstream's credit loop (`while (c) { t =
creatorMap[c.nodeName]; if (!t) continue; ... }`) never advances past a
credit it has no creator type for, so upstream hangs on one. The port
fails the translation there instead (kovan never hangs).

```rust
pub mod eidr { /* ... */ }
```

### Functions

#### Function `detect_search`

`detectSearch` (:57-73).

```rust
pub fn detect_search(search: &crate::zotero::framework::item::JsObject) -> bool { /* ... */ }
```

#### Function `do_search`

`doSearch` (:75-142).

```rust
pub fn do_search(ctx: &mut crate::zotero::search::SearchContext, search: &crate::zotero::framework::item::JsObject) -> Result<(), crate::zotero::search::SearchError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `eric`

The ERIC search translator: the ERIC API (`api.ies.ed.gov/eric`) by ERIC
number. No identifier that `extractIdentifiers` finds sets
`ericNumber`, so it runs only when called with a search item that has
one.

```rust
pub mod eric { /* ... */ }
```

### Functions

#### Function `detect_search`

`detectSearch` (:178-180).

```rust
pub fn detect_search(search: &crate::zotero::framework::item::JsObject) -> bool { /* ... */ }
```

#### Function `do_search`

`doSearch` (:182-229).

```rust
pub fn do_search(ctx: &mut crate::zotero::search::SearchContext, search: &crate::zotero::framework::item::JsObject) -> Result<(), crate::zotero::search::SearchError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `gbv_isbn`

The Gemeinsamer Bibliotheksverbund ISBN search translator: upstream's
placeholder after the translator was renamed K10plus ISBN; it detects
nothing and does nothing.

```rust
pub mod gbv_isbn { /* ... */ }
```

### Functions

#### Function `detect_search`

`detectSearch` (:19-21): always false.

```rust
pub fn detect_search(_search: &crate::zotero::framework::item::JsObject) -> bool { /* ... */ }
```

#### Function `do_search`

`doSearch` (:23): nothing.

```rust
pub fn do_search(_ctx: &mut crate::zotero::search::SearchContext, _search: &crate::zotero::framework::item::JsObject) -> Result<(), crate::zotero::search::SearchError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header. Upstream's `target` and `maxVersion` are `null`.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `k10plus_isbn`

The K10plus ISBN search translator: an SRU query to K10plus (the merged
GBV and SWB catalogue) for MARCXML, imported by MARCXML, with a table of
contents link, the queried ISBN first, no call number and no tags.

```rust
pub mod k10plus_isbn { /* ... */ }
```

### Functions

#### Function `detect_search`

`detectSearch` (:37-39): `!!item.ISBN`.

```rust
pub fn detect_search(search: &crate::zotero::framework::item::JsObject) -> bool { /* ... */ }
```

#### Function `do_search`

`doSearch` (:41-107).

```rust
pub fn do_search(ctx: &mut crate::zotero::search::SearchContext, search: &crate::zotero::framework::item::JsObject) -> Result<(), crate::zotero::search::SearchError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `libris_isbn`

The LIBRIS ISBN search translator: Swedish ISBNs (group 91) looked up
in LIBRIS xsearch as MARCXML, the first `collection > record` imported
by MARCXML, without the "Bok" tag.

```rust
pub mod libris_isbn { /* ... */ }
```

### Functions

#### Function `detect_search`

`detectSearch` (:37-45): a string ISBN whose cleaned form matches
`/^(97[8-9])?91/` (`cleanISBN` giving `false` tests as "false").

```rust
pub fn detect_search(search: &crate::zotero::framework::item::JsObject) -> bool { /* ... */ }
```

#### Function `do_search`

`doSearch` (:47-67).

```rust
pub fn do_search(ctx: &mut crate::zotero::search::SearchContext, search: &crate::zotero::framework::item::JsObject) -> Result<(), crate::zotero::search::SearchError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `loc_isbn`

The Library of Congress ISBN search translator: an SRU query to the
Library of Congress catalogue for MARCXML, imported by MARCXML.

```rust
pub mod loc_isbn { /* ... */ }
```

### Functions

#### Function `detect_search`

`detectSearch` (:14-20): `!!item.ISBN`.

```rust
pub fn detect_search(search: &crate::zotero::framework::item::JsObject) -> bool { /* ... */ }
```

#### Function `do_search`

`doSearch` (:23-43).

```rust
pub fn do_search(ctx: &mut crate::zotero::search::SearchContext, search: &crate::zotero::framework::item::JsObject) -> Result<(), crate::zotero::search::SearchError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `lulu`

The Lulu search translator (ISBN -> lulu.com product page, scraped).
**Disabled upstream**: `detectSearch` returns `false` ("no longer
working"), so no identifier lookup reaches it; `doSearch` is ported for
completeness and verified on a kovan-authored page
(`fixtures/synthetic_lulu.json`).

```rust
pub mod lulu { /* ... */ }
```

### Functions

#### Function `detect_search`

`detectSearch` (:106-120): `if (true) return false;`.

```rust
pub fn detect_search(_search: &crate::zotero::framework::item::JsObject) -> bool { /* ... */ }
```

#### Function `do_search`

`doSearch` (:122-147) for one search item.

```rust
pub fn do_search(ctx: &mut crate::zotero::search::SearchContext, search: &crate::zotero::framework::item::JsObject) -> Result<(), crate::zotero::search::SearchError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `medra`

The mEDRA search translator (DOI -> the mEDRA DOI resolution page,
scraped). **Disabled upstream**: `detectSearch` returns `false` (TEMP,
"This translator is broken"), so no identifier lookup reaches it;
`doSearch` is ported for completeness and verified on a kovan-authored
page (`fixtures/synthetic_medra.json`).

```rust
pub mod medra { /* ... */ }
```

### Functions

#### Function `detect_search`

`detectSearch` (:302-313): `return false;` (TEMP, disabled upstream).

```rust
pub fn detect_search(_search: &crate::zotero::framework::item::JsObject) -> bool { /* ... */ }
```

#### Function `do_search`

`doSearch` (:315-323): each DOI's mEDRA page through `doWeb`, in turn.

```rust
pub fn do_search(ctx: &mut crate::zotero::search::SearchContext, search: &crate::zotero::framework::item::JsObject) -> Result<(), crate::zotero::search::SearchError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `nlp_isbn`

The National Library of Poland ISBN search translator: Polish ISBNs
(group 83) looked up in the Biblioteka Narodowa's data API as MARCXML,
the first `collection > record` imported by MARCXML.

```rust
pub mod nlp_isbn { /* ... */ }
```

### Functions

#### Function `detect_search`

`detectSearch` (:37-48): a string ISBN whose digits (spaces and hyphens
removed) start with 97883, 97983 or 83.

```rust
pub fn detect_search(search: &crate::zotero::framework::item::JsObject) -> bool { /* ... */ }
```

#### Function `do_search`

`doSearch` (:68-93).

```rust
pub fn do_search(ctx: &mut crate::zotero::search::SearchContext, search: &crate::zotero::framework::item::JsObject) -> Result<(), crate::zotero::search::SearchError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `open_worldcat`

The Open WorldCat search translator (ISBN or OCLC number -> WorldCat's
search API, after scraping a session token from its home page).

**Not ported: AES-GCM.** WorldCat may answer with an encrypted body
(`{p, x, l}`), which upstream decrypts with WebCrypto AES-GCM. The key
and ciphertext are split out as upstream does ([`split`]), but the
decryption itself needs an AES-GCM implementation, which kovan-literature
does not depend on; such a response fails the translator with a
[`SearchError::Translator`] naming the reason (the search then moves to
the next translator, as upstream does after any failure). A plain JSON
answer is handled exactly.

WorldCat is scraped behind a session token and was not contacted for
the fixtures; `fixtures/synthetic_open_worldcat.json` is kovan-authored.

```rust
pub mod open_worldcat { /* ... */ }
```

### Functions

#### Function `detect_search`

`detectSearch` (:270-272).

```rust
pub fn detect_search(search: &crate::zotero::framework::item::JsObject) -> bool { /* ... */ }
```

#### Function `do_search`

`doSearch` (:274-327).

```rust
pub fn do_search(ctx: &mut crate::zotero::search::SearchContext, search: &crate::zotero::framework::item::JsObject) -> Result<(), crate::zotero::search::SearchError> { /* ... */ }
```

#### Function `wc_hyphenate_isbn`

`wcHyphenateISBN(isbn)` (:345-424): the ISBN hyphenated by the ranges
table, `None` (upstream '') when it cannot be.

```rust
pub fn wc_hyphenate_isbn(isbn: &str) -> Option<String> { /* ... */ }
```

#### Function `split`

`split(input, mode)` (:476-485): (secretKey, encryptedData).

```rust
pub fn split(input: &str, mode_str: &str) -> Option<(String, String)> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `openalex`

The OpenAlex search translator: the OpenAlex works API filtered by
OpenAlex ID, imported by OpenAlex JSON. No identifier that
`extractIdentifiers` finds sets `openAlex`, so it runs only when called
with a search item that has one.

```rust
pub mod openalex { /* ... */ }
```

### Functions

#### Function `detect_search`

`detectSearch` (:49-51): `!!item.openAlex`.

```rust
pub fn detect_search(search: &crate::zotero::framework::item::JsObject) -> bool { /* ... */ }
```

#### Function `do_search`

`doSearch` (:53-55): `scrape([item.openAlex])`.

```rust
pub fn do_search(ctx: &mut crate::zotero::search::SearchContext, search: &crate::zotero::framework::item::JsObject) -> Result<(), crate::zotero::search::SearchError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `pubmed`

The PubMed search translator: NCBI E-utilities `efetch` for the PMIDs,
imported by PubMed XML.

```rust
pub mod pubmed { /* ... */ }
```

### Functions

#### Function `detect_search`

`detectSearch` (:261-275). A `getPMID` that throws makes detection
fail, which upstream treats as not detected.

```rust
pub fn detect_search(search: &crate::zotero::framework::item::JsObject) -> bool { /* ... */ }
```

#### Function `do_search`

`doSearch` (:277-287).

```rust
pub fn do_search(ctx: &mut crate::zotero::search::SearchContext, search: &crate::zotero::framework::item::JsObject) -> Result<(), crate::zotero::search::SearchError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `who`

The WHO search translator (search half): World Health Organization
ISBNs (978-92-4) looked up in WHO IRIS as RIS, imported by RIS, with the
record page's `citation_pdf_url` added as a PDF attachment.

```rust
pub mod who { /* ... */ }
```

### Functions

#### Function `detect_search`

`detectSearch` (:154-156).

```rust
pub fn detect_search(search: &crate::zotero::framework::item::JsObject) -> bool { /* ... */ }
```

#### Function `do_search`

`doSearch` (:158-192).

```rust
pub fn do_search(ctx: &mut crate::zotero::search::SearchContext, search: &crate::zotero::framework::item::JsObject) -> Result<(), crate::zotero::search::SearchError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

### Types

#### Enum `SearchTranslator`

A ported Zotero search translator (translatorType 8, or 12 with a
search half). No trait objects: one variant per translator.

```rust
pub enum SearchTranslator {
    Eidr,
    CrossrefRest,
    Who,
    LibraryOfCongressIsbn,
    BnfIsbn,
    CamaraBrasileiraDoLivroIsbn,
    LibrisIsbn,
    NationalLibraryOfPolandIsbn,
    GemeinsamerBibliotheksverbundIsbn,
    K10plusIsbn,
    AdsBibcode,
    DoiContentNegotiation,
    Eric,
    OpenWorldCat,
    OpenAlex,
    PubMed,
    ArXiv,
    Lulu,
    Medra,
}
```

##### Variants

###### `Eidr`

`EIDR.js`.

###### `CrossrefRest`

`Crossref REST.js`.

###### `Who`

`WHO.js`.

###### `LibraryOfCongressIsbn`

`Library of Congress ISBN.js`.

###### `BnfIsbn`

`BnF ISBN.js`.

###### `CamaraBrasileiraDoLivroIsbn`

`Camara Brasileira do Livro ISBN.js`.

###### `LibrisIsbn`

`LIBRIS ISBN.js`.

###### `NationalLibraryOfPolandIsbn`

`National Library of Poland ISBN.js`.

###### `GemeinsamerBibliotheksverbundIsbn`

`Gemeinsamer Bibliotheksverbund ISBN.js`.

###### `K10plusIsbn`

`K10plus ISBN.js`.

###### `AdsBibcode`

`ADS Bibcode.js`.

###### `DoiContentNegotiation`

`DOI Content Negotiation.js`.

###### `Eric`

`ERIC.js`.

###### `OpenWorldCat`

`Open WorldCat.js`.

###### `OpenAlex`

`OpenAlex.js`.

###### `PubMed`

`PubMed.js`.

###### `ArXiv`

`arXiv.org.js`.

###### `Lulu`

`Lulu.js`.

###### `Medra`

`mEDRA.js`.

##### Implementations

###### Methods

- ```rust
  pub fn metadata(self: Self) -> &'static TranslatorMetadata { /* ... */ }
  ```
  The translator's header.

- ```rust
  pub fn from_id(id: &str) -> Option<SearchTranslator> { /* ... */ }
  ```
  The translator with this `translatorID`.

- ```rust
  pub fn detect(self: Self, search: &JsObject) -> bool { /* ... */ }
  ```
  `detectSearch(search)`.

- ```rust
  pub fn do_search(self: Self, ctx: &mut SearchContext, search: &JsObject) -> Result<(), SearchError> { /* ... */ }
  ```
  `doSearch(search)`, its items going through `ctx.complete`.

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
    fn clone(self: &Self) -> SearchTranslator { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SearchTranslator) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Enum `LookupError`

Why a lookup failed. Every network path ends in one of these; none
panics.

```rust
pub enum LookupError {
    NoIdentifier,
    NoTranslator,
    NotFound {
        attempts: Vec<(String, String)>,
    },
    Offline,
    Connect(String),
    Timeout,
    HttpStatus {
        code: u16,
        url: String,
    },
    RateLimited {
        url: String,
        retry_after: Option<String>,
    },
    Malformed(String),
    Unsupported(String),
    Translator(String),
}
```

##### Variants

###### `NoIdentifier`

No identifier in the text (upstream would run a text search, which
kovan does not).

###### `NoTranslator`

No translator handles this identifier (upstream 501 "No translators
available").

###### `NotFound`

Every translator answered and none found a record (upstream 501 "No
items returned from any translator"; an HTTP 404 counts as "no
record").

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `attempts` | `Vec<(String, String)>` | Each translator tried, with what happened. |

###### `Offline`

No network.

###### `Connect`

DNS or connection failure.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Timeout`

A timeout.

###### `HttpStatus`

An HTTP status that is not success.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `code` | `u16` | The code. |
| `url` | `String` | The URL. |

###### `RateLimited`

HTTP 429.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `url` | `String` | The URL. |
| `retry_after` | `Option<String>` | `Retry-After`, when sent. |

###### `Malformed`

The response could not be read (bad JSON/XML, unexpected shape,
unsupported content type, too large).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Unsupported`

Not possible on this target (e.g. a browser's CORS refusal, a build
without a network backend).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Translator`

A translator refused the input or failed for its own reasons.

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
    fn clone(self: &Self) -> LookupError { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
- **Error**
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
    fn eq(self: &Self, other: &LookupError) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `Attempt`

What one translator did in a search.

```rust
pub struct Attempt {
    pub translator: SearchTranslator,
    pub error: Option<SearchError>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `translator` | `SearchTranslator` | The translator. |
| `error` | `Option<SearchError>` | `None`: it returned no items; `Some`: it failed. |

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
    fn clone(self: &Self) -> Attempt { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &Attempt) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `SearchRun`

A finished search.

```rust
pub struct SearchRun {
    pub translators: Vec<SearchTranslator>,
    pub used: SearchTranslator,
    pub items: Vec<super::framework::item::TranslatorItem>,
    pub attempts: Vec<Attempt>,
    pub requests: Vec<HttpRequest>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `translators` | `Vec<SearchTranslator>` | The translators tried, in order. |
| `used` | `SearchTranslator` | The one whose items were kept. |
| `items` | `Vec<super::framework::item::TranslatorItem>` | Its items (after `_itemDone`, translator format). |
| `attempts` | `Vec<Attempt>` | The translators that found nothing before it. |
| `requests` | `Vec<HttpRequest>` | Every request made, in order. |

##### Implementations

###### Methods

- ```rust
  pub fn api_json(self: &Self, now_iso: &str) -> Vec<Value> { /* ... */ }
  ```
  The items as the translation-server's `/search` returns them

- ```rust
  pub fn zotero_items(self: &Self, now_iso: &str) -> Result<Vec<ZoteroItem>, ZoteroJsonError> { /* ... */ }
  ```
  The items as kovan-common [`ZoteroItem`]s.

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
    fn clone(self: &Self) -> SearchRun { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &SearchRun) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Enum `SearchStep`

One step of a search.

```rust
pub enum SearchStep {
    Done(Result<SearchRun, LookupError>),
    Pending(HttpRequest),
}
```

##### Variants

###### `Done`

Finished.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Result<SearchRun, LookupError>` |  |

###### `Pending`

Needs this request answered first.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `HttpRequest` |  |

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
    fn clone(self: &Self) -> SearchStep { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &SearchStep) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `LookupSession`

A lookup in progress: what to search, which translators, and the
answers so far. Owns everything (no borrows), so an async caller can
hold it across awaits.

```rust
pub struct LookupSession {
    pub identifier: Option<Identifier>,
    pub search: super::framework::item::JsObject,
    pub translators: Vec<SearchTranslator>,
    pub options: SearchOptions,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `identifier` | `Option<Identifier>` | The identifier, when the session came from text. |
| `search` | `super::framework::item::JsObject` | The search item. |
| `translators` | `Vec<SearchTranslator>` | The translators to try, in order. |
| `options` | `SearchOptions` | The options. |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn for_text(text: &str, options: SearchOptions) -> Result<LookupSession, LookupError> { /* ... */ }
  ```
  A session for the identifier the translation-server's `/search`

- ```rust
  pub fn for_identifier(id: Identifier, options: SearchOptions) -> LookupSession { /* ... */ }
  ```
  A session for one identifier.

- ```rust
  pub fn forced(search: JsObject, translator: SearchTranslator, options: SearchOptions) -> LookupSession { /* ... */ }
  ```
  A session running one translator on a search item

- ```rust
  pub fn step(self: &Self) -> SearchStep { /* ... */ }
  ```
  Run as far as the answers allow.

- ```rust
  pub fn answer(self: &mut Self, req: &HttpRequest, answer: Result<HttpResponse, FetchError>) { /* ... */ }
  ```
  Give the answer to a pending request.

- ```rust
  pub fn run_with<F>(self: Self, fetch: F) -> Result<SearchRun, LookupError>
where
    F: FnMut(&HttpRequest) -> Result<HttpResponse, FetchError> { /* ... */ }
  ```
  Run to the end, fetching each pending request with `fetch`

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
    fn clone(self: &Self) -> LookupSession { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &LookupSession) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
### Functions

#### Function `detect_translators`

`getTranslators()` of a search: every translator whose `detectSearch`
is truthy, in priority order.

```rust
pub fn detect_translators(search: &super::framework::item::JsObject) -> Vec<SearchTranslator> { /* ... */ }
```

#### Function `run_search`

Run a search from the start with the answers in `cache`
(`translate.translate()` with `Search#complete`'s fallback).

```rust
pub fn run_search(search: &super::framework::item::JsObject, translators: &[SearchTranslator], cache: HttpCache, options: &SearchOptions) -> SearchStep { /* ... */ }
```

### Constants and Statics

#### Constant `MAX_REQUESTS`

Safety bound on requests per lookup (upstream lookups make one to four).

```rust
pub const MAX_REQUESTS: usize = 64;
```

### Re-exports

#### Re-export `ChildSearch`

```rust
pub use context::ChildSearch;
```

#### Re-export `SearchContext`

```rust
pub use context::SearchContext;
```

#### Re-export `SearchError`

```rust
pub use context::SearchError;
```

#### Re-export `SearchOptions`

```rust
pub use context::SearchOptions;
```

#### Re-export `extract_identifiers`

```rust
pub use extract::extract_identifiers;
```

#### Re-export `identifier_for_search`

```rust
pub use extract::identifier_for_search;
```

#### Re-export `Identifier`

```rust
pub use extract::Identifier;
```

#### Re-export `FetchError`

```rust
pub use http::FetchError;
```

#### Re-export `HttpCache`

```rust
pub use http::HttpCache;
```

#### Re-export `HttpFailure`

```rust
pub use http::HttpFailure;
```

#### Re-export `HttpRequest`

```rust
pub use http::HttpRequest;
```

#### Re-export `HttpResponse`

```rust
pub use http::HttpResponse;
```

#### Re-export `RequestOptions`

```rust
pub use http::RequestOptions;
```

## Module `translators`

Zotero's import/export translators, one module each, behind the
[`Translator`] enum (no trait objects: a new translator is a new variant
and a new module).

| Translator | Import | Export | Module |
|---|---|---|---|
| BibTeX | yes | yes | [`bibtex`] |
| BibLaTeX | — | yes | [`biblatex`] |
| RIS | yes | yes | [`ris`] |
| CSL JSON | yes | yes | [`csl_json`] |
| MODS | yes | yes | [`mods`] (#749, XML) |
| Endnote XML | yes | yes | [`endnote_xml`] (#749, XML) |
| TEI | — | yes | [`tei`] (#749, XML) |
| Crossref Unixref XML | yes | — | [`crossref_unixref_xml`] (#749, XML) |
| MARCXML | yes | — | [`marcxml`] (#749, XML; runs [`marc`]'s record model) |
| MARC | yes | — | [`marc`] (#749, binary/line MARC) |
| PubMed XML | yes | — | [`pubmed_xml`] (#749, XML) |
| METS | yes | — | [`mets`] (#749, XML; runs MODS or MARCXML as child translators) |
| Primo Normalized XML | yes | — | [`primo_normalized_xml`] (#749, XML) |
| DSpace Intermediate Metadata | yes | — | [`dspace_intermediate_metadata`] (#749, XML) |
| Citavi 5 XML | yes | — | [`citavi5_xml`] (#749, XML) |
| XML ContextObject | yes | — | [`xml_contextobject`] (#749, XML; OpenURL) |
| Note HTML | — | yes | [`note_html`] (#749, DOM) |
| Note Markdown | — | yes | [`note_markdown`] (#749, DOM; turndown) |
| Refer/BibIX | yes | yes | [`refer`] |
| RefWorks Tagged | yes | yes | [`refworks_tagged`] |
| Bookmarks | yes | yes | [`bookmarks`] |
| MEDLINE/nbib | yes | — | [`medline_nbib`] |
| OVID Tagged | yes | — | [`ovid_tagged`] |
| Web of Science Tagged | yes | — | [`wos_tagged`] |
| MAB2 | yes | — | [`mab2`] |
| Datacite JSON | yes | — | [`datacite_json`] |
| OpenAlex JSON | yes | — | [`openalex_json`] |
| CSV | — | yes | [`csv`] |
| COinS | — | yes | [`coins`] |
| Wikipedia Citation Templates | — | yes | [`wikipedia_citation_templates`] |
| Wikidata QuickStatements | — | yes | [`wikidata_quickstatements`] |
| CFF | — | yes | [`cff`] |
| CFF References | — | yes | [`cff_references`] |
| Simple Evernote Export | — | yes | [`evernote`] |

~~Not ported from #749's list: Note HTML and Note Markdown (they parse the
note with a DOM, `DOMParser` and XPath; Note Markdown also bundles
turndown), which wait for the XML/DOM layer.~~ **CORRECTED 2026-10-07**:
both ported on the XML/DOM layer (`framework::xml`, `framework::xpath`).
| Zotero RDF | — | yes | [`zotero_rdf`] |
| RDF | yes | — | [`rdf`] |
| Bibliontology RDF | yes | yes | [`bibliontology`] |
| Unqualified Dublin Core RDF | — | yes | [`dc_rdf`] |

The four RDF translators (#749) run on the framework's RDF data mode
([`super::framework::rdf`]); [`rdf_support`] holds the JavaScript
behaviour they share.

```rust
pub mod translators { /* ... */ }
```

### Modules

## Module `biblatex`

The BibLaTeX translator: export.

```rust
pub mod biblatex { /* ... */ }
```

### Functions

#### Function `do_export`

`doExport` (:492-883).

```rust
pub fn do_export(ctx: &mut crate::zotero::framework::ExportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `bibtex`

The BibTeX translator: import and export.

| File | Upstream |
|---|---|
| [`import`] | `doImport` and everything it calls (parser, `processField`, `unescapeBibTeX`, JabRef groups) |
| [`export`] | `doExport` (`writeField`, escaping, case protection, creators, Extra identifiers) |
| [`text`] | `splitUnprotected`, `unescapeBibTeX`, `mapTeXmarkup`, file records |
| [`common`] | Extra parsing, citation keys, file paths (shared with BibLaTeX) |
| [`mapping_table`], [`reverse_mapping_table`] | the two LaTeX <-> Unicode tables, generated from the JS |

Not ported: `setKeywordSplitOnSpace` and `setKeywordDelimRe`, the
`exports` other translators call through `loadTranslator` (no ported
translator calls them; the defaults they would change are used).

```rust
pub mod bibtex { /* ... */ }
```

### Modules

## Module `common`

What the BibTeX and BibLaTeX exporters share: Extra parsing, citation
keys, file-path cleaning.

```rust
pub mod common { /* ... */ }
```

## Module `export`

BibTeX export (`doExport`).

```rust
pub mod export { /* ... */ }
```

### Functions

#### Function `do_export`

`doExport` (:1341-1573).

```rust
pub fn do_export(ctx: &mut crate::zotero::framework::ExportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

## Module `import`

BibTeX import (`doImport`).

The parser reads one character at a time (`Zotero.read(1)`), as upstream
does. Where upstream's control flow depends on JavaScript coercions the
port reproduces them: at the end of input `Zotero.read(1)` is `false`,
which upstream's tests read as the string "false" (so `keyRe.test(false)`
is true); a `,` inside an `@string` record dereferences an undefined item
and throws; a JabRef "intersection" group calls the undefined
`jabrefMap` and throws. Each such throw fails the import, as upstream's
`reject(e)` does.

```rust
pub mod import { /* ... */ }
```

### Functions

#### Function `do_import`

`doImport` (:1008-1022) and `readString` (:1024-1070).

```rust
pub fn do_import(ctx: &mut crate::zotero::framework::ImportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

## Module `mapping_table`

**Attributes:**

- `Other("#[rustfmt::skip]")`

BibTeX.js `mappingTable`: Unicode character -> LaTeX, used on export when the export charset is not UTF-8.

```rust
pub mod mapping_table { /* ... */ }
```

### Constants and Statics

#### Constant `TABLE`

`mappingTable`, in property order.

```rust
pub const TABLE: &[(&str, &str)] = _;
```

## Module `reverse_mapping_table`

**Attributes:**

- `Other("#[rustfmt::skip]")`

BibTeX.js `reversemappingTable`: LaTeX -> Unicode, used by `unescapeBibTeX` on import.

```rust
pub mod reverse_mapping_table { /* ... */ }
```

### Constants and Statics

#### Constant `TABLE`

`reversemappingTable`, in property order.

```rust
pub const TABLE: &[(&str, &str)] = _;
```

## Module `text`

BibTeX import's text helpers: splitting outside braces, LaTeX to
Unicode/HTML (`unescapeBibTeX`), JabRef/Mendeley file records.

```rust
pub mod text { /* ... */ }
```

### Functions

#### Function `detect_import`

`detectImport` (:42-82): a line (or the end of a 4096-character read)
whose non-space text starts with `@type{` or `@type(`; `%` comments to
the end of the line; at most 1 MiB scanned. Falls off the end
(`undefined`, falsy) when nothing matches.

```rust
pub fn detect_import(ctx: &mut crate::zotero::framework::ImportContext) -> bool { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

### Re-exports

#### Re-export `do_export`

```rust
pub use export::do_export;
```

#### Re-export `do_import`

```rust
pub use import::do_import;
```

## Module `csl_json`

The CSL JSON translator: import and export.

```rust
pub mod csl_json { /* ... */ }
```

### Functions

#### Function `detect_import`

`detectImport` (:57-85).

```rust
pub fn detect_import(ctx: &mut crate::zotero::framework::ImportContext) -> bool { /* ... */ }
```

#### Function `do_import`

`doImport` (:87-152).

```rust
pub fn do_import(ctx: &mut crate::zotero::framework::ImportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

#### Function `do_export`

`doExport` (:154-168).

```rust
pub fn do_export(ctx: &mut crate::zotero::framework::ExportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

#### Function `json_stringify_tab`

`JSON.stringify(value, null, "\t")`: tab indentation, `[]`/`{}` for
empty containers; keys in serde's order, except the top object's when
`order` is given.

```rust
pub fn json_stringify_tab(v: &serde_json::Value) -> String { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `ris`

The RIS translator: import and export.

| File | Upstream |
|---|---|
| [`tables`] | type maps, `fieldMap`, `degenerateImportFieldMap`, `exportOrder`, ProCite maps (generated from RIS.js) |
| [`mapper`] | `TagMapper` |
| [`reader`] | `RISReader`, `TagCleaner`, `ProCiteCleaner`, `EndNoteCleaner`, `CitaviCleaner` |
| [`import`] | `processTag`, `applyValue`, `dateRIStoZotero`, `completeItem`, `importNext` |
| [`export`] | `addTag`, `doExport` |

Not ported: `exportedOptions` (:68-73: `itemType`, `defaultItemType`,
`typeMap`, `fieldMap`), which only another translator calling this one
through `loadTranslator` can set; they are all `false` for a direct
import or export, and the code paths that read them are ported for that
value. Also not ported: `saveFile` for attachments on export
(`exportFileData`), which the translation-server does not support.

**Maturity: AI draft (1).** Verified code-to-code against the Zotero
translation-server (`tests/zotero_translators.rs`, `ris_*`).

```rust
pub mod ris { /* ... */ }
```

### Modules

## Module `export`

RIS export.

```rust
pub mod export { /* ... */ }
```

### Functions

#### Function `do_export`

`doExport` (:2030-2179).

```rust
pub fn do_export(ctx: &mut crate::zotero::framework::ExportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

## Module `import`

RIS import.

```rust
pub mod import { /* ... */ }
```

### Functions

#### Function `do_import`

`doImport` (:1806-1912).

```rust
pub fn do_import(ctx: &mut crate::zotero::framework::ImportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

#### Function `date_ris_to_zotero`

`dateRIStoZotero(risDate, zField)` (:1563-1696).

```rust
pub fn date_ris_to_zotero(ctx: &crate::zotero::framework::ImportContext, ris_date: &str, z_field: &str) -> String { /* ... */ }
```

## Module `mapper`

RIS tag <-> Zotero field mapping.

```rust
pub mod mapper { /* ... */ }
```

### Types

#### Type Alias `MapTable`

A list of tag maps (upstream's `fieldMap`-shaped objects).

```rust
pub type MapTable = &'static [(&'static str, super::tables::TagMap)];
```

#### Struct `TagMapper`

`TagMapper` (:519): maps tried in order; `deprecated` is the map whose
tags are only used when nothing else maps to their field.

Upstream caches results per item type and tag; the lookups are pure, so
this port recomputes them.

```rust
pub struct TagMapper {
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
  pub fn new(maps: Vec<MapTable>, deprecated: Option<MapTable>, keep_id: bool) -> Self { /* ... */ }
  ```
  `new TagMapper(mapList, deprecatedMap)`.

- ```rust
  pub fn is_deprecated(self: &Self, tag: &str) -> bool { /* ... */ }
  ```
  `isDeprecated(tag)` (:526): the deprecated map has the tag.

- ```rust
  pub fn get_field(self: &Self, item_type: &str, tag: &str) -> Option<&'static str> { /* ... */ }
  ```
  `getField(itemType, tag)` (:538-587).

- ```rust
  pub fn reverse_lookup(self: &Self, item_type: &str, z_field: &str) -> Option<&'static str> { /* ... */ }
  ```
  `reverseLookup(itemType, zField)` (:598-645): the first RIS tag that

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
    fn clone(self: &Self) -> TagMapper { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
## Module `reader`

Reading RIS entries and cleaning up their tags.

Upstream's entry is an array of `{tag, value, raw}` objects plus a
`tags` property mapping each tag to the same objects; cleaners mutate the
objects through either view. Here the objects live in an arena
([`Entry::pairs`]) and both views hold indices into it, so object
identity (`indexOf`, shared mutation) carries over.

```rust
pub mod reader { /* ... */ }
```

### Types

#### Struct `TagValue`

A tag-value pair (`{tag, value, raw}`).

```rust
pub struct TagValue {
    pub tag: String,
    pub value: String,
    pub raw: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `tag` | `String` | The RIS tag. |
| `value` | `String` | The value, continuation lines joined. |
| `raw` | `String` | The line(s) as read. |

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
    fn clone(self: &Self) -> TagValue { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &TagValue) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `Entry`

One RIS entry.

```rust
pub struct Entry {
    pub pairs: Vec<TagValue>,
    pub order: Vec<usize>,
    pub tags: std::collections::BTreeMap<String, Vec<usize>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pairs` | `Vec<TagValue>` | Every pair object ever created for this entry. |
| `order` | `Vec<usize>` | The entry array: pair ids in order. |
| `tags` | `std::collections::BTreeMap<String, Vec<usize>>` | `entry.tags`: tag -> pair ids. |

##### Implementations

###### Methods

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  `entry.length`.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether the entry has no pairs.

- ```rust
  pub fn at(self: &Self, i: usize) -> &TagValue { /* ... */ }
  ```
  `entry[i]`.

- ```rust
  pub fn at_mut(self: &mut Self, i: usize) -> &mut TagValue { /* ... */ }
  ```
  `entry[i]`, mutably.

- ```rust
  pub fn index_of(self: &Self, id: usize) -> Option<usize> { /* ... */ }
  ```
  `entry.indexOf(pair)` (`None` is -1).

- ```rust
  pub fn first(self: &Self, tag: &str) -> Option<&TagValue> { /* ... */ }
  ```
  The first pair for a tag (`entry.tags[tag][0]`).

- ```rust
  pub fn has(self: &Self, tag: &str) -> bool { /* ... */ }
  ```
  `entry.tags[tag]` is truthy (an array, even an empty one).

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
    fn clone(self: &Self) -> Entry { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Entry { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `RisReader`

The RIS reader's state (`RISReader`, :661-820) and the ProCite cleaner's
`proCiteMode` flag (:870), both singletons that persist across entries.

```rust
pub struct RisReader {
    pub pro_cite_mode: bool,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pro_cite_mode` | `bool` | `ProCiteCleaner.proCiteMode`. |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn next_entry(self: &mut Self, ctx: &mut ImportContext) -> Option<Entry> { /* ... */ }
  ```
  `nextEntry` (:675-701).

- ```rust
  pub fn pro_cite_clean(self: &mut Self, entry: &mut Entry, item_type: &str, fields: &TagMapper) -> Result<(), TranslateError> { /* ... */ }
  ```
  `ProCiteCleaner.cleanTags(entry, item)` (:928-1119).

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
    fn clone(self: &Self) -> RisReader { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> RisReader { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
### Functions

#### Function `change_tag`

`TagCleaner.changeTag(entry, at, toTags)` (:836-860): retag the pair at
`at` (or remove it when `to_tags` is empty), adding copies for further tags.

```rust
pub fn change_tag(entry: &mut Entry, at: usize, to_tags: &[&str]) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

#### Function `end_note_clean`

`EndNoteCleaner.cleanTags` (:1231-1238): authors of an edited book are
editors (A3).

```rust
pub fn end_note_clean(entry: &mut Entry) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

#### Function `citavi_clean`

`CitaviCleaner.cleanTags` (:1245-1274): the first H1/H2 pair becomes
DP/CN.

```rust
pub fn citavi_clean(entry: &mut Entry) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

## Module `tables`

**Attributes:**

- `Other("#[rustfmt::skip]")`

The RIS translator's tables.

```rust
pub mod tables { /* ... */ }
```

### Types

#### Enum `Sel`

The value under a field in a type-dependent tag map.

```rust
pub enum Sel {
    Types(&'static [&'static str]),
    Field(&'static str),
}
```

##### Variants

###### `Types`

A list of item types (an explicit mapping, or `__exclude`).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&'static [&'static str]` |  |

###### `Field`

A field name (`__default`).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&'static str` |  |

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
    fn clone(self: &Self) -> Sel { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &Sel) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Enum `TagMap`

A tag's mapping: one field for every item type, or item-type dependent.

```rust
pub enum TagMap {
    Field(&'static str),
    ByType(&'static [(&'static str, Sel)]),
}
```

##### Variants

###### `Field`

The same Zotero field for every item type.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&'static str` |  |

###### `ByType`

`{ field: [itemTypes], __default: field, __exclude: [itemTypes] }`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&'static [(&'static str, Sel)]` |  |

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
    fn clone(self: &Self) -> TagMap { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &TagMap) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
### Constants and Statics

#### Static `EXPORT_TYPE_MAP`

`exportTypeMap` after the degenerate types are merged in (:83-126, :178-180).

```rust
pub static EXPORT_TYPE_MAP: &[(&str, &str)] = _;
```

#### Static `IMPORT_TYPE_MAP`

`importTypeMap` after `exportTypeMap` is added (:131-167).

```rust
pub static IMPORT_TYPE_MAP: &[(&str, &str)] = _;
```

#### Static `FIELD_MAP`

`fieldMap` (:207-435).

```rust
pub static FIELD_MAP: &[(&str, TagMap)] = _;
```

#### Static `DEGENERATE_IMPORT_FIELD_MAP`

`degenerateImportFieldMap` (:440-504).

```rust
pub static DEGENERATE_IMPORT_FIELD_MAP: &[(&str, TagMap)] = _;
```

#### Static `EXPORT_ORDER_DEFAULT`

`exportOrder.__default` (:1921-1964).

```rust
pub static EXPORT_ORDER_DEFAULT: &[&str] = _;
```

#### Static `EXPORT_ORDER_BILL`

`exportOrder.bill` (:1966-2009).

```rust
pub static EXPORT_ORDER_BILL: &[&str] = _;
```

#### Static `PROCITE_AUTHOR_ROLE`

`proCiteMap['Author Role']` (:873-901).

```rust
pub static PROCITE_AUTHOR_ROLE: &[(&str, &str)] = _;
```

#### Static `PROCITE_MAP`

`proCiteMap` without `Author Role` (:902-917).

```rust
pub static PROCITE_MAP: &[(&str, &str)] = _;
```

### Functions

#### Function `detect_import`

`detectImport` (:47-62): a `TY  - ` line among the first five
non-blank lines.

```rust
pub fn detect_import(ctx: &mut crate::zotero::framework::ImportContext) -> bool { /* ... */ }
```

#### Function `do_import`

`doImport` (:1806-1912).

```rust
pub fn do_import(ctx: &mut crate::zotero::framework::ImportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

#### Function `do_export`

`doExport` (:2030-2179).

```rust
pub fn do_export(ctx: &mut crate::zotero::framework::ExportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header. (`getCollections` is the string "true" upstream.)

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `citavi5_xml`

The Citavi 5 XML translator (import): a Citavi 5 or 6 project exported
as XML (`CitaviExchangeData`): references, persons, periodicals, series,
publishers, keywords, groups (tags), knowledge items (notes), locations
(attachments, call numbers), tasks (standalone notes) and categories
(collections).

**Shared objects.** Upstream saves items when the translation ends, and
a saved item shares its `creators` array and creator objects with the
translator's object. `importUnfinished` turns a container's `author`s
into `bookAuthor`s while copying them to a contribution, after the
container was completed; the saved container changes too. The port keeps
its own copy of every item (`itemIdList`) and applies the same change to
the completed item ([`ImportContext::items_mut`]).

**Maturity: AI draft (1).**

```rust
pub mod citavi5_xml { /* ... */ }
```

### Functions

#### Function `detect_import`

`detectImport` (:51-54).

```rust
pub fn detect_import(ctx: &mut crate::zotero::framework::ImportContext) -> bool { /* ... */ }
```

#### Function `do_import`

`doImport` (:435-472).

```rust
pub fn do_import(ctx: &mut crate::zotero::framework::ImportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `crossref_unixref_xml`

The Crossref Unixref XML translator (import): Crossref's `unixref`
records (`doi_records/doi_record/crossref/...`).

```rust
pub mod crossref_unixref_xml { /* ... */ }
```

### Functions

#### Function `detect_import`

`detectImport` (:161-175): `<crossref>` on one of the first nine
non-empty lines.

```rust
pub fn detect_import(ctx: &mut crate::zotero::framework::ImportContext) -> bool { /* ... */ }
```

#### Function `do_import`

`doImport` (:178-477).

```rust
pub fn do_import(ctx: &mut crate::zotero::framework::ImportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `dspace_intermediate_metadata`

The DSpace Intermediate Metadata translator (import): DSpace's DIM
(`dim:field` elements, usually inside a METS wrapper).

```rust
pub mod dspace_intermediate_metadata { /* ... */ }
```

### Functions

#### Function `detect_import`

`detectImport` (:37-40): the first 1000 characters name the DIM
namespace.

```rust
pub fn detect_import(ctx: &mut crate::zotero::framework::ImportContext) -> bool { /* ... */ }
```

#### Function `do_import`

`doImport` (:60-121).

```rust
pub fn do_import(ctx: &mut crate::zotero::framework::ImportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `endnote_xml`

The Endnote XML translator: import ([`import`]) and export ([`export`]).

Collections: the header asks for `getCollections`, but neither direction
uses them (upstream calls neither `Zotero.nextCollection` nor
`Zotero.Collection`), so nothing is lost.

```rust
pub mod endnote_xml { /* ... */ }
```

### Functions

#### Function `detect_import`

`detectImport` (:22-31): the document has a `record/ref-type` with text.
A parse error thrown by `getXML` makes detection fail (the framework
catches it).

```rust
pub fn detect_import(ctx: &mut crate::zotero::framework::ImportContext) -> bool { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

### Re-exports

#### Re-export `do_export`

```rust
pub use export::do_export;
```

#### Re-export `do_import`

```rust
pub use import::do_import;
```

## Module `marc`

The MARC translator (import): binary MARC 21 and UNIMARC records.

| Module | Upstream |
|---|---|
| [`record`] | the record model (`importBinary`, `addField`, `getField`, `extractSubfields`) and the cleaning functions; what MARCXML uses through `getTranslatorObject` |
| [`translate`] | `record.prototype.translate` and its `_associate*` helpers |

**Maturity: AI draft (1).**

```rust
pub mod marc { /* ... */ }
```

### Modules

## Module `record`

MARC's record model (`marc.record` in upstream, which MARCXML gets with
`getTranslatorObject`), with JavaScript's string semantics.

The record content is kept in UTF-16 code units, as upstream's string
is: `importBinary` pads every non-ASCII code unit with NULs so that the
directory's byte offsets line up (a code unit above U+07FF gets two NULs,
one above U+007F one; `charCodeAt` never exceeds U+FFFF, so an astral
character's two surrogates get two NULs each), and the NULs are removed
when a field is read. Numbers from `parseInt` are `f64` so that `NaN`
flows through `substr` as it does in JavaScript.

```rust
pub mod record { /* ... */ }
```

### Types

#### Struct `Subfields`

The subfields of one field (`extractSubfields`' object), in insertion
order.

```rust
pub struct Subfields(pub Vec<(String, String)>);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<(String, String)>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn get(self: &Self, code: &str) -> Option<&str> { /* ... */ }
  ```
  `subfields[code]` (`None` is undefined).

- ```rust
  pub fn truthy(self: &Self, code: &str) -> bool { /* ... */ }
  ```
  JavaScript truthiness of `subfields[code]`.

- ```rust
  pub fn set(self: &mut Self, code: &str, value: String) { /* ... */ }
  ```
  `subfields[code] = value`.

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
    fn clone(self: &Self) -> Subfields { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Subfields { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &Subfields) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
#### Struct `Record`

A MARC record (`new marc.record()`, :129-137).

```rust
pub struct Record {
    pub leader: Option<String>,
    pub indicator_length: f64,
    pub subfield_code_length: f64,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `leader` | `Option<String>` | `leader` (`None` is null, which MARCXML can set). |
| `indicator_length` | `f64` | `indicatorLength`. |
| `subfield_code_length` | `f64` | `subfieldCodeLength`. |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  `new record()`.

- ```rust
  pub fn import_binary(self: &mut Self, record: &str) { /* ... */ }
  ```
  `importBinary(record)` (:140-182).

- ```rust
  pub fn add_field(self: &mut Self, field: &str, indicator: &str, value: &str) { /* ... */ }
  ```
  `addField(field, indicator, value)` (:185-207); `field` as the

- ```rust
  pub fn get_field(self: &Self, field: &str) -> Vec<(String, String)> { /* ... */ }
  ```
  `getField(field)` (:210-230): `[indicator, value]` of every field

- ```rust
  pub fn extract_subfields(self: &Self, field_str: &str) -> Subfields { /* ... */ }
  ```
  `extractSubfields(fieldStr, tag)` (:233-259).

- ```rust
  pub fn get_field_subfields(self: &Self, tag: &str) -> Vec<Subfields> { /* ... */ }
  ```
  `getFieldSubfields(tag)` (:262-271).

- ```rust
  pub fn translate(self: &Self, item: &mut TranslatorItem) -> Result<(), TranslateError> { /* ... */ }
  ```
  `translate(item)` (:350-836).

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
    fn clone(self: &Self) -> Record { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
    fn eq(self: &Self, other: &Record) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
### Functions

#### Function `js_parse_int`

JavaScript `parseInt(s)` (radix 10): leading whitespace, a sign, digits;
`NaN` when there are none.

```rust
pub fn js_parse_int(s: &str) -> f64 { /* ... */ }
```

#### Function `substr16`

`String.prototype.substr(start, length)` over UTF-16 code units (`len`
`None` is `undefined`).

```rust
pub fn substr16(v: &[u16], start: f64, len: Option<f64>) -> Vec<u16> { /* ... */ }
```

#### Function `substr`

`substr` on a string.

```rust
pub fn substr(s: &str, start: f64, len: Option<f64>) -> String { /* ... */ }
```

#### Function `len16`

JavaScript `length` of a string.

```rust
pub fn len16(s: &str) -> f64 { /* ... */ }
```

#### Function `clean`

`clean(value)` (:59-77): `None` (undefined) gives `None` (null).

```rust
pub fn clean(value: Option<&str>) -> Option<String> { /* ... */ }
```

#### Function `pull_number`

`pullNumber(text)` (:80-87).

```rust
pub fn pull_number(text: &str) -> String { /* ... */ }
```

#### Function `pull_isbn`

`pullISBN(text)` (:90-97).

```rust
pub fn pull_isbn(text: &str) -> String { /* ... */ }
```

#### Function `glue_together`

`glueTogether(part1, part2, delimiter)` (:106-123), with JavaScript
truthiness (`None` and "" are falsy).

```rust
pub fn glue_together(part1: Option<String>, part2: Option<String>, delimiter: Option<&str>) -> Option<String> { /* ... */ }
```

### Constants and Statics

#### Constant `FIELD_TERMINATOR`

`fieldTerminator`.

```rust
pub const FIELD_TERMINATOR: char = '\x1E';
```

#### Constant `RECORD_TERMINATOR`

`recordTerminator`.

```rust
pub const RECORD_TERMINATOR: char = '\x1D';
```

#### Constant `SUBFIELD_DELIMITER`

`subfieldDelimiter`.

```rust
pub const SUBFIELD_DELIMITER: char = '\x1F';
```

## Module `translate`

`record.translate(item)`: a MARC record (MARC 21 or UNIMARC) into an
item.

```rust
pub mod translate { /* ... */ }
```

### Functions

#### Function `detect_import`

`detectImport` (:38-45): the first 8 characters match
`/^[0-9]{5}[a-z ]{3}$/`.

```rust
pub fn detect_import(ctx: &mut crate::zotero::framework::ImportContext) -> bool { /* ... */ }
```

#### Function `do_import`

`doImport` (:838-860): the input read 4096 characters at a time, split
on the record terminator; every complete record is imported (text after
the last terminator is held over and, at the end, dropped).

```rust
pub fn do_import(ctx: &mut crate::zotero::framework::ImportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

### Re-exports

#### Re-export `Record`

```rust
pub use record::Record;
```

#### Re-export `FIELD_TERMINATOR`

```rust
pub use record::FIELD_TERMINATOR;
```

#### Re-export `RECORD_TERMINATOR`

```rust
pub use record::RECORD_TERMINATOR;
```

#### Re-export `SUBFIELD_DELIMITER`

```rust
pub use record::SUBFIELD_DELIMITER;
```

## Module `marcxml`

The MARCXML translator (import): MARC 21 slim XML records, mapped by
MARC's record model ([`super::marc`], upstream's `getTranslatorObject`).
METS runs it as a child translator.

```rust
pub mod marcxml { /* ... */ }
```

### Functions

#### Function `detect_import`

`detectImport` (:37-51): the MARC slim namespace declared in one of the
first seven non-empty lines.

```rust
pub fn detect_import(ctx: &mut crate::zotero::framework::ImportContext) -> bool { /* ... */ }
```

#### Function `parse_document`

`parseDocument(xml)` (:57-95): one MARC record per `marc:record` with at
least one `datafield`.

```rust
pub fn parse_document(doc: &crate::zotero::framework::xml::XmlDocument) -> Result<Vec<super::marc::Record>, crate::zotero::framework::TranslateError> { /* ... */ }
```

#### Function `do_import`

`doImport` (:98-106).

```rust
pub fn do_import(ctx: &mut crate::zotero::framework::ImportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `mets`

The METS translator: import. The descriptive metadata in each
`dmdSec` is handed to another translator (MODS, or for MARC the MARCXML
translator) as a child translation.

Not ported: MAB2 (the third entry of the MARC translator list), which
never runs: an import given an array of translators runs its first one
only ([`crate::zotero::framework::child`]).

**Maturity: AI draft (1).** Verified code-to-code against the Zotero
translation-server (`tests/zotero_xml_translators.rs`, `mets_*`).

```rust
pub mod mets { /* ... */ }
```

### Functions

#### Function `detect_import`

`detectImport` (:41-55): `<mets` in one of the first nine non-empty
lines.

```rust
pub fn detect_import(ctx: &mut crate::zotero::framework::ImportContext) -> bool { /* ... */ }
```

#### Function `do_import`

`doImport` (:57-76).

```rust
pub fn do_import(ctx: &mut crate::zotero::framework::ImportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

#### Function `atob`

`window.atob` (forgiving-base64 decode): the bytes as a Latin-1 string;
invalid input throws `InvalidCharacterError`.

```rust
pub fn atob(s: &str) -> Result<String, crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `mods`

The MODS translator: import and export.

| File | Upstream |
|---|---|
| this one | header, type tables, `detectImport` |
| [`import`] | `processTitleInfo` ... `processIdentifiers`, `getFirstResult`, `doImport` (:699-1440) |
| [`export`] | `mapProperty`, `doExport` (:351-697) |

**Maturity: AI draft (1).** Verified code-to-code against the Zotero
translation-server (`tests/zotero_xml_translators.rs`, `mods_*`).

```rust
pub mod mods { /* ... */ }
```

### Modules

## Module `export`

MODS export: a `modsCollection` document built with the DOM and written
with `XMLSerializer`.

```rust
pub mod export { /* ... */ }
```

### Functions

#### Function `do_export`

`doExport` (:365-697).

```rust
pub fn do_export(ctx: &mut crate::zotero::framework::ExportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

## Module `import`

MODS import.

Properties are assigned in upstream's order, `null` included (JavaScript
creates the property even for a null value; `_itemDone` drops it later),
because the order decides which of two fields with the same base field
`itemToAPIJSON` keeps.

```rust
pub mod import { /* ... */ }
```

### Functions

#### Function `do_import`

`doImport` (:1083-1440).

```rust
pub fn do_import(ctx: &mut crate::zotero::framework::ImportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Functions

#### Function `lookup`

A lookup in one of the tables (`table[key]`).

```rust
pub fn lookup(table: &[(&str, &'static str)], key: &str) -> Option<&'static str> { /* ... */ }
```

#### Function `detect_import`

`detectImport` (:325-338): the root element is in the MODS namespace and
its tag name ends with `modsCollection` or `mods`.

```rust
pub fn detect_import(ctx: &mut crate::zotero::framework::ImportContext) -> bool { /* ... */ }
```

#### Function `do_import`

`doImport`.

```rust
pub fn do_import(ctx: &mut crate::zotero::framework::ImportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

#### Function `do_export`

`doExport`.

```rust
pub fn do_export(ctx: &mut crate::zotero::framework::ExportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

#### Constant `NS`

The MODS namespace (`ns`, :322).

```rust
pub const NS: &str = "http://www.loc.gov/mods/v3";
```

#### Constant `XNS`

`xns` (:323).

```rust
pub const XNS: &[(&str, &str)] = _;
```

#### Constant `FROM_MARC_GENRE`

`fromMarcGenre` (:43-148), in upstream's key order.

```rust
pub const FROM_MARC_GENRE: &[(&str, &str)] = _;
```

#### Constant `TO_MARC_GENRE`

`toMarcGenre` (:150-185).

```rust
pub const TO_MARC_GENRE: &[(&str, &str)] = _;
```

#### Constant `DCT_GENRES`

`dctGenres` (:187-203).

```rust
pub const DCT_GENRES: &[(&str, &str)] = _;
```

#### Constant `FROM_TYPE_OF_RESOURCE`

`fromTypeOfResource` (:205-216).

```rust
pub const FROM_TYPE_OF_RESOURCE: &[(&str, &str)] = _;
```

#### Constant `TO_TYPE_OF_RESOURCE`

`toTypeOfResource` (:218-253).

```rust
pub const TO_TYPE_OF_RESOURCE: &[(&str, &str)] = _;
```

#### Constant `MODS_TYPE_REGEX`

`modsTypeRegex` (:255-289): (type, case-insensitive pattern), in key
order. `newspaper\*article` is upstream's (a literal `*`).

```rust
pub const MODS_TYPE_REGEX: &[(&str, &str)] = _;
```

#### Constant `MODS_INTERNET_MEDIA_TYPES`

`modsInternetMediaTypes` (:291-294).

```rust
pub const MODS_INTERNET_MEDIA_TYPES: &[(&str, &str)] = _;
```

#### Constant `MARC_RELATORS`

`marcRelators` (:296-307).

```rust
pub const MARC_RELATORS: &[(&str, &str)] = _;
```

#### Constant `PARTIAL_ITEM_TYPES`

`partialItemTypes` (:310-319).

```rust
pub const PARTIAL_ITEM_TYPES: &[&str] = _;
```

## Module `primo_normalized_xml`

The Primo Normalized XML translator (import): Ex Libris Primo PNX
records.

```rust
pub mod primo_normalized_xml { /* ... */ }
```

### Functions

#### Function `detect_import`

`detectImport` (:41-44).

```rust
pub fn detect_import(ctx: &mut crate::zotero::framework::ImportContext) -> bool { /* ... */ }
```

#### Function `do_import`

`doImport` (:47-384).

```rust
pub fn do_import(ctx: &mut crate::zotero::framework::ImportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `pubmed_xml`

The PubMed XML translator (import): `PubmedArticleSet` (articles and
NCBI Bookshelf books and chapters).

```rust
pub mod pubmed_xml { /* ... */ }
```

### Functions

#### Function `detect_import`

`detectImport` (:44-47).

```rust
pub fn detect_import(ctx: &mut crate::zotero::framework::ImportContext) -> bool { /* ... */ }
```

#### Function `do_import`

`doImport` (:92-397).

```rust
pub fn do_import(ctx: &mut crate::zotero::framework::ImportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `tei`

The TEI translator (export): a TEI P5 `listBibl` of `biblStruct`s.

Collections: the translation-server's item getter answers
`Zotero.nextCollection()` with `false` (translate_item.js:80-82), so the
"Export Collections" path never runs there; [`ExportContext`] has no
collections and this port takes the same branch (`generateCollection`
is not ported: no input can reach it).

```rust
pub mod tei { /* ... */ }
```

### Functions

#### Function `do_export`

`doExport` (:560-646).

```rust
pub fn do_export(ctx: &mut crate::zotero::framework::ExportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `xml_contextobject`

The XML ContextObject translator (import): OpenURL ContextObjects in
XML, turned into key-encoded-value strings (COinS titles) and read with
`ZU.parseContextObject`
([`crate::zotero::framework::openurl::parse_context_object`]).

```rust
pub mod xml_contextobject { /* ... */ }
```

### Functions

#### Function `detect_import`

`detectImport` (:58-68): the ctx namespace in one of the first 100
lines.

```rust
pub fn detect_import(ctx: &mut crate::zotero::framework::ImportContext) -> bool { /* ... */ }
```

#### Function `do_import`

`doImport` (:71-89).

```rust
pub fn do_import(ctx: &mut crate::zotero::framework::ImportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `note_html`

The Note HTML translator (export): the notes among the exported items
(standalone notes and attachments' notes) as one HTML document.

The note HTML is parsed, edited and serialised as jsdom does it
([`crate::zotero::framework::html_dom`]); the XPath steps run on the
wicked-good-xpath port over the HTML document.

```rust
pub mod note_html { /* ... */ }
```

### Functions

#### Function `do_export`

`doExport` (:46-206).

```rust
pub fn do_export(ctx: &mut crate::zotero::framework::ExportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `note_markdown`

The Note Markdown translator (export): each note among the exported
items (standalone notes and attachments' notes) as Markdown, separated
by `---`.

```rust
pub mod note_markdown { /* ... */ }
```

### Modules

## Module `turndown`

Turndown (HTML to Markdown) with the gfm plugin and Note Markdown's
options, escapes and list-item rule, over the HTML DOM of
[`crate::zotero::framework::html_dom`].

Only the configuration Note Markdown uses is ported: `headingStyle: atx`,
`bulletListMarker: '-'`, `emDelimiter: '*'`, `codeBlockStyle: fenced`,
the other options at turndown's defaults (`hr: '* * *'`,
`strongDelimiter: '**'`, `linkStyle: inlined`, `br: '  '`,
`preformattedCode: false`). Rules are tried in the order the bundle
builds them (each `addRule` puts its rule first).

```rust
pub mod turndown { /* ... */ }
```

### Types

#### Struct `Turndown`

A turndown run over one document.

```rust
pub struct Turndown {
    pub doc: crate::zotero::framework::xml::XmlDocument,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `doc` | `crate::zotero::framework::xml::XmlDocument` | The document (owned for the run: turndown works on a clone of the<br>element, which lives in it). |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(doc: XmlDocument) -> Self { /* ... */ }
  ```
  A run over `doc`.

- ```rust
  pub fn turndown(self: &mut Self, input: NodeId) -> Result<String, crate::zotero::framework::TranslateError> { /* ... */ }
  ```
  `turndownService.turndown(element)` (:1186-1197): a deep clone of

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
- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
    ```

- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
### Functions

#### Function `escape`

Note Markdown's `TurndownService.prototype.escape` (:1414-1440).

```rust
pub fn escape(s: &str) -> String { /* ... */ }
```

### Functions

#### Function `do_export`

`doExport` (:1619-1638).

```rust
pub fn do_export(ctx: &mut crate::zotero::framework::ExportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `refer`

The Refer/BibIX translator: import and export.

Every routine of the translator is ported. One upstream behaviour worth
knowing: an input with no `%` line makes `doImport` throw (`false.replace`
on the end of input, :216), reported here as
[`TranslateError::Translator`].

**Maturity: AI draft (1).** Verified code-to-code against upstream
(`tests/zotero_translators.rs`, `refer_*`).

```rust
pub mod refer { /* ... */ }
```

### Functions

#### Function `detect_import`

`detectImport` (:17-36): two Refer lines before any other non-blank
line.

```rust
pub fn detect_import(ctx: &mut crate::zotero::framework::ImportContext) -> bool { /* ... */ }
```

#### Function `do_import`

`doImport` (:211-256).

```rust
pub fn do_import(ctx: &mut crate::zotero::framework::ImportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

#### Function `do_export`

`doExport` (:264-313).

```rust
pub fn do_export(ctx: &mut crate::zotero::framework::ExportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `refworks_tagged`

The RefWorks Tagged translator: import and export.

| File | Upstream |
|---|---|
| [`tables`] | type maps, `fieldMap`, `degenerateImportFieldMap`, `exportOrder` (generated from the translator) |
| [`import`] | `processTag`, `applyValue`, `dateRWtoZotero`, `completeItem`, `getLine`, `doImport` |
| [`export`] | `addTag`, `doExport` |

Not ported: `exportedOptions.itemType` (:47-49), which only a translator
calling this one through `loadTranslator` can set (it is `false` for a
direct import, and that path is ported); `doImport`'s `attachments`
argument, likewise only passed by a calling translator; and
`saveFile` for attachments on export, which the translation-server never
provides (its `ItemGetter.exportFiles` is a no-op, so the attachment URL
is exported, as here).

**Maturity: AI draft (1).** Verified code-to-code against upstream
(`tests/zotero_translators.rs`, `refworks_tagged_*`).

```rust
pub mod refworks_tagged { /* ... */ }
```

### Modules

## Module `export`

RefWorks Tagged export.

```rust
pub mod export { /* ... */ }
```

### Functions

#### Function `do_export`

`doExport` (:843-971).

```rust
pub fn do_export(ctx: &mut crate::zotero::framework::ExportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

## Module `import`

RefWorks Tagged import.

```rust
pub mod import { /* ... */ }
```

### Functions

#### Function `do_import`

`doImport` (:775-807).

```rust
pub fn do_import(ctx: &mut crate::zotero::framework::ImportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

## Module `tables`

**Attributes:**

- `Other("#[rustfmt::skip]")`

The RefWorks Tagged translator's tables.

```rust
pub mod tables { /* ... */ }
```

### Constants and Statics

#### Static `EXPORT_TYPE_MAP`

`exportTypeMap` with `degenerateExportTypeMap` merged in (:60-102, :130-132).

```rust
pub static EXPORT_TYPE_MAP: &[(&str, &str)] = _;
```

#### Static `IMPORT_TYPE_MAP`

`importTypeMap` supplemented from `exportTypeMap` (:107-127).

```rust
pub static IMPORT_TYPE_MAP: &[(&str, &str)] = _;
```

#### Static `FIELD_MAP`

`fieldMap` (:141-330).

```rust
pub static FIELD_MAP: &[(&str, crate::zotero::translators::ris::tables::TagMap)] = _;
```

#### Static `DEGENERATE_IMPORT_FIELD_MAP`

`degenerateImportFieldMap` (:333-345).

```rust
pub static DEGENERATE_IMPORT_FIELD_MAP: &[(&str, crate::zotero::translators::ris::tables::TagMap)] = _;
```

#### Static `EXPORT_ORDER_DEFAULT`

`exportOrder.__default` (:818-820; "OP," with its comma, as upstream).

```rust
pub static EXPORT_ORDER_DEFAULT: &[&str] = _;
```

#### Static `EXPORT_ORDER_BILL`

`exportOrder.bill` (:822-824).

```rust
pub static EXPORT_ORDER_BILL: &[&str] = _;
```

### Functions

#### Function `detect_import`

`detectImport` (:28-43): a line matching `/^RT\s+./` before more than
151 other non-blank lines. (Upstream returns `undefined`, falsy, at the
end of the input.)

```rust
pub fn detect_import(ctx: &mut crate::zotero::framework::ImportContext) -> bool { /* ... */ }
```

#### Function `get_fields`

`TagMapper.getFields(itemType, tag)` (:353-399): for each map, the tag's
field for the item type. Within a type-dependent map the LAST matching
field wins (the loop does not break), `__default` applies when nothing
matched and the type is not excluded. Ported with upstream's `var`
quirk: `field` and `def` are declared with `var` inside the loop without
initialisers, so they keep their values from the previous map; a field
found in one map is therefore pushed again for every later map. Only
`getFields(...)[0]` is read, which is the first map's field when it has
one.

```rust
pub fn get_fields(maps: &[&'static [(&'static str, crate::zotero::translators::ris::tables::TagMap)]], item_type: &str, tag: &str) -> Vec<&'static str> { /* ... */ }
```

#### Function `do_import`

`doImport` (:775-807).

```rust
pub fn do_import(ctx: &mut crate::zotero::framework::ImportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

#### Function `do_export`

`doExport` (:843-971).

```rust
pub fn do_export(ctx: &mut crate::zotero::framework::ExportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `bookmarks`

The Bookmarks translator (Netscape bookmark file format): import and
export.

Upstream parses with JavaScript regular expressions that use
backreferences and lookahead, which the `regex` crate lacks. Each of the
five is ported as a hand-written matcher with the same backtracking
outcome (documented on each), over the text as `char`s (upstream's
positions are UTF-16 code units; they differ only on astral-plane
characters). The `i` flag compares ASCII letters without case, as
JavaScript does for these all-ASCII patterns.

Collections are kept in [`crate::zotero::framework::ImportResult`]'s
`collections` (the translation-server discards them).

**Maturity: AI draft (1).** Verified code-to-code against upstream
(`tests/zotero_translators.rs`, `bookmarks_*`).

```rust
pub mod bookmarks { /* ... */ }
```

### Functions

#### Function `detect_import`

`detectImport` (:57-76).

```rust
pub fn detect_import(ctx: &mut crate::zotero::framework::ImportContext) -> bool { /* ... */ }
```

#### Function `do_import`

`doImport` (:78-202).

```rust
pub fn do_import(ctx: &mut crate::zotero::framework::ImportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

#### Function `do_export`

`doExport` (:218-245).

```rust
pub fn do_export(ctx: &mut crate::zotero::framework::ExportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `medline_nbib`

The MEDLINE/nbib translator: import (PubMed's and ERIC's `.nbib`
tagged format).

Upstream's `doImport` and `finalizeItem` are `async` but await nothing
but `item.complete()`; they are ported as ordinary functions.

**Maturity: AI draft (1).** Verified code-to-code against Zotero
(`tests/zotero_translators.rs`, `medline_nbib_import_matches_upstream`).

```rust
pub mod medline_nbib { /* ... */ }
```

### Functions

#### Function `detect_import`

`detectImport` (:42-59). Upstream's `/^PMID( {1, 2})?- /` is not a
quantifier (`{1, 2}` with a space is literal text in JavaScript), so on
the first six characters it only matches `"PMID- "`.

```rust
pub fn detect_import(ctx: &mut crate::zotero::framework::ImportContext) -> bool { /* ... */ }
```

#### Function `do_import`

`doImport` (:199-242).

```rust
pub fn do_import(ctx: &mut crate::zotero::framework::ImportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `ovid_tagged`

The OVID Tagged translator: import.

JavaScript regular expressions are rewritten for the `regex` crate with
their JavaScript meaning: `\s` is [`js::WS`], `\d` `[0-9]`, `\w`
`[A-Za-z0-9_]`, `\b` `(?-u:\b)` and `.` anything but a line terminator
([`DOT`]). Both engines pick the leftmost match and, at that position,
the same (leftmost-first) submatch.

**Maturity: AI draft (1).** Verified code-to-code against Zotero
(`tests/zotero_translators.rs`, `ovid_tagged_import_matches_upstream`).

```rust
pub mod ovid_tagged { /* ... */ }
```

### Functions

#### Function `detect_import`

`detectImport` (:40-57).

```rust
pub fn detect_import(ctx: &mut crate::zotero::framework::ImportContext) -> bool { /* ... */ }
```

#### Function `do_import`

`doImport` (:171-230).

```rust
pub fn do_import(ctx: &mut crate::zotero::framework::ImportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

#### Constant `DOT`

JavaScript's `.`: any character but a line terminator.

```rust
pub const DOT: &str = r"[^\n\r\x{2028}\x{2029}]";
```

## Module `wos_tagged`

The Web of Science Tagged translator: import (the "Plain text" /
"Other file format: tab-delimited" tagged export of Web of Science).

Upstream's `ItemMap.records` is a JavaScript `Map` (insertion order;
`set` on an existing key keeps its place) from tag to an array of
lines; [`Records`] keeps that behaviour. Where upstream would throw a
`TypeError` on malformed input (an empty `DT`/`PT`/`PD`/`PI` value, a
dangling continuation line), the port returns
[`TranslateError::Translator`].

**Maturity: AI draft (1).** Verified code-to-code against Zotero
(`tests/zotero_translators.rs`, `wos_tagged_import_matches_upstream`).

```rust
pub mod wos_tagged { /* ... */ }
```

### Functions

#### Function `detect_import`

`detectImport` (:89-104): a `PT` or `DT` tag among the first ten
non-empty lines.

```rust
pub fn detect_import(ctx: &mut crate::zotero::framework::ImportContext) -> bool { /* ... */ }
```

#### Function `do_import`

`doImport` (:106-118).

```rust
pub fn do_import(ctx: &mut crate::zotero::framework::ImportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `mab2`

The MAB2 translator (Maschinelles Austauschformat für Bibliotheken, in
MARC-like binary records): import.

Records are handled in UTF-16 code units, as upstream does: the directory
gives byte offsets, which upstream reconciles by padding each non-ASCII
code unit with NUL characters (one for U+0080..U+07FF, two above), and
`substr`/`length` count code units. The input is read in chunks of 4096
characters (`Zotero.read(4096)`), which the framework counts in `char`s
where upstream counts code units; the two differ only with astral-plane
characters, where a chunk boundary can fall in a different place.

Not ported: `addField` (:153-175) and the `exports` object (:354-359),
which only another translator calling this one uses (no translator in
Zotero's repository does; MARC.js has its own `record`).

**Maturity: AI draft (1).** Verified code-to-code against upstream
(`tests/zotero_translators.rs`, `mab2_import_matches_upstream`).

```rust
pub mod mab2 { /* ... */ }
```

### Functions

#### Function `detect_import`

`detectImport` (:15-21): the first eight characters match
`/^[0-9]{3}[a-z ]{2}[a-z ]{3}$/`. (`Zotero.read(8)` at the end of the
input is `false`, tested as the string "false", which does not match.)

```rust
pub fn detect_import(ctx: &mut crate::zotero::framework::ImportContext) -> bool { /* ... */ }
```

#### Function `do_import`

`doImport` (:323-352): records end at `\x1D`; the text after the last
one is never imported.

```rust
pub fn do_import(ctx: &mut crate::zotero::framework::ImportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `datacite_json`

The Datacite JSON translator: import (a DataCite REST API JSON record).

The record is read with JavaScript's semantics where they decide the
result: a property read on `undefined`/`null` throws (the import fails,
as upstream's does), `for...of` over a non-iterable throws, string
concatenation turns values into strings with `ToString`, `a + b` of two
numbers adds. These helpers ([`JsVal`] and friends) are shared with the
OpenAlex JSON port.

Duplicate creators are removed as upstream does it: by
`JSON.stringify` of each creator object, so two creators that differ
only in key order (for instance `{lastName, firstName, creatorType}`
from DataCite's own name fields and `{firstName, lastName, creatorType}`
from `ZU.cleanAuthor`) are both kept.

Not ported: nothing; `datasetType` is evaluated against the schema at run
time (`dataset` exists in the schema the port follows, so the
pre-6.0.26 `document` branch is inert, as upstream on that schema).

**Maturity: AI draft (1).** Verified code-to-code against upstream
(`tests/zotero_translators.rs`, `datacite_json_*`).

```rust
pub mod datacite_json { /* ... */ }
```

### Functions

#### Function `detect_import`

`detectImport` (:60-66). A detector that throws (a non-string
`schemaVersion`) detects nothing.

```rust
pub fn detect_import(ctx: &mut crate::zotero::framework::ImportContext) -> bool { /* ... */ }
```

#### Function `do_import`

`doImport` (:105-336).

```rust
pub fn do_import(ctx: &mut crate::zotero::framework::ImportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `openalex_json`

The OpenAlex JSON translator: import (an OpenAlex work, or a page of
`results`).

JavaScript value semantics come from the Datacite JSON port's helpers
([`super::datacite_json`]). The PDF attachment upstream adds is kept on
the item; the translation-server's `itemToAPIJSON` drops attachments, so
it does not show in the Web API JSON.

Not ported: nothing.

**Maturity: AI draft (1).** Verified code-to-code against upstream
(`tests/zotero_translators.rs`, `openalex_json_*`).

```rust
pub mod openalex_json { /* ... */ }
```

### Functions

#### Function `detect_import`

`detectImport` (:54-64). A detector that throws (`results[0]` undefined)
detects nothing.

```rust
pub fn detect_import(ctx: &mut crate::zotero::framework::ImportContext) -> bool { /* ... */ }
```

#### Function `do_import`

`doImport` (:97-108).

```rust
pub fn do_import(ctx: &mut crate::zotero::framework::ImportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `csv`

The CSV translator: export (one row per regular item, a fixed column
set, a byte-order mark first).

The item is the legacy export format (minVersion 4.0.26 < 4.0.27), which
the framework prepares. Where upstream throws (an item with no `uri`, or
one not ending in `[A-Z0-9]+`: `item.uri.match(...)[1]`), the port
returns [`TranslateError::Translator`] and nothing is written, as the
endpoint answers 500.

**Maturity: AI draft (1).** Verified code-to-code against the Zotero
translation-server (`tests/zotero_translators.rs`, `csv_*`).

```rust
pub mod csv { /* ... */ }
```

### Functions

#### Function `do_export`

`doExport` (:163-178).

```rust
pub fn do_export(ctx: &mut crate::zotero::framework::ExportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `coins`

The COinS translator: export (one `<span class='Z3988'>` per item, its
`title` an OpenURL 1.0 ContextObject).

Not ported: the web half (`detectWeb`, `doWeb` and its helpers,
:38-247), which reads COinS spans from a web page and looks items up
through search translators; kovan has no web translation.

**Maturity: AI draft (1).** Verified code-to-code against the Zotero
translation-server (`tests/zotero_translators.rs`, `coins_*`).

```rust
pub mod coins { /* ... */ }
```

### Functions

#### Function `do_export`

`doExport` (:249-260).

```rust
pub fn do_export(ctx: &mut crate::zotero::framework::ExportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `wikipedia_citation_templates`

The Wikipedia Citation Templates translator: export (`{{Cite ...}}`
templates, one per item, separated by CRLF).

The item is the legacy export format (minVersion 1.0.0b4.r1 < 4.0.27):
dates are SQL, single-field creators have a `lastName` and no
`firstName`. JavaScript values are kept as JavaScript would concatenate
them (an undefined name is "undefined"; undefined + undefined is "NaN").
Where upstream throws (a string method on a value that is not a string;
`formatFirstAuthor` of an empty list for a "Cite email") the port
returns [`TranslateError::Translator`], as the endpoint answers 500.

**Maturity: AI draft (1).** Verified code-to-code against the Zotero
translation-server (`tests/zotero_translators.rs`, `wikipedia_*`).

```rust
pub mod wikipedia_citation_templates { /* ... */ }
```

### Functions

#### Function `localized_creator_type`

`Zotero.Utilities.getLocalizedCreatorType(type)` as JavaScript would
concatenate it: the label, or "false" for an unknown (or missing) type.

```rust
pub fn localized_creator_type(t: Option<&str>) -> &'static str { /* ... */ }
```

#### Function `do_export`

`doExport` (:122-428).

```rust
pub fn do_export(ctx: &mut crate::zotero::framework::ExportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `wikidata_quickstatements`

The Wikidata QuickStatements translator: export, one `CREATE` block of
QuickStatements commands per item (items whose Extra has a `QID: ` line
are skipped).

The translator's `minVersion` (3.0) is below 4.0.27, so it reads the
legacy export item format the framework prepares.

Not ported: lookups of inherited `Object.prototype` names in the mapping
tables (`typeMapping["constructor"]`, `"constructor" in
languageMapping`), which only an item type, Extra `itemType:` line or
language spelled like a JavaScript built-in would reach.

**Maturity: AI draft (1).** Verified code-to-code against upstream run
in-process (`tests/zotero_translators.rs`,
`wikidata_quickstatements_export_matches_upstream`).

```rust
pub mod wikidata_quickstatements { /* ... */ }
```

### Functions

#### Function `do_export`

`doExport` (:313-323).

```rust
pub fn do_export(ctx: &mut crate::zotero::framework::ExportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `cff`

The CFF translator: export of datasets and software as a
`CITATION.cff` file (other item types are skipped).

The helpers shared with CFF References ([`CffValue`], [`get_value`]) are
here.

Not ported: nothing.

**Maturity: AI draft (1).** Verified code-to-code against upstream run
in-process (`tests/zotero_translators.rs`, `cff_export_matches_upstream`).

```rust
pub mod cff { /* ... */ }
```

### Functions

#### Function `do_export`

`doExport` (:68-108).

```rust
pub fn do_export(ctx: &mut crate::zotero::framework::ExportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `cff_references`

The CFF References translator: export of any items as the `references`
list of a `CITATION.cff` file.

Ported as upstream behaves, including two upstream quirks the reference
output shows: `writeCreators` pushes every creator whatever role is asked
for (both branches of its `if` push), so `authors`, `editors`,
`recipients` and `translators` each list all creators; and `pmcid` is
the whole `match` array written with `Array#toString` ("pmcid: X,X").

Not ported: nothing.

**Maturity: AI draft (1).** Verified code-to-code against upstream run
in-process (`tests/zotero_translators.rs`,
`cff_references_export_matches_upstream`).

```rust
pub mod cff_references { /* ... */ }
```

### Functions

#### Function `do_export`

`doExport` (:77-165).

```rust
pub fn do_export(ctx: &mut crate::zotero::framework::ExportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `evernote`

The Simple Evernote Export translator: export, an Evernote `.enex`
document with one note per item (title, child notes, dates, tags, URL).

The item is the legacy export format (minVersion 2.1.9 < 4.0.27):
`dateAdded` holds the SQL form of the input's `dateModified` (the
endpoint's quirk, ported in the framework) and `dateModified` stays ISO,
so `<updated>` ends in "ZZ", as upstream's does. An item without
`dateAdded` or `dateModified` (or with a non-string one) makes upstream
throw (`item.dateAdded.replace`): the port returns
[`TranslateError::Translator`], as the endpoint answers 500 and writes
nothing.

Lengths (`title.length > 252`, `tag.length > 95`) and `substr` count
UTF-16 code units, as upstream does.

**Maturity: AI draft (1).** Verified code-to-code against the Zotero
translation-server (`tests/zotero_translators.rs`, `evernote_*`).

```rust
pub mod evernote { /* ... */ }
```

### Functions

#### Function `do_export`

`doExport` (:35-104).

```rust
pub fn do_export(ctx: &mut crate::zotero::framework::ExportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `bibliontology`

The Bibliontology RDF translator (BIBO, FOAF, Dublin Core terms):
import and export.

```rust
pub mod bibliontology { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

### Re-exports

#### Re-export `do_export`

```rust
pub use export::do_export;
```

#### Re-export `detect_import`

```rust
pub use import::detect_import;
```

#### Re-export `do_import`

```rust
pub use import::do_import;
```

## Module `dc_rdf`

The Unqualified Dublin Core RDF export translator.

```rust
pub mod dc_rdf { /* ... */ }
```

### Functions

#### Function `do_export`

`doExport()` (:39-153).

```rust
pub fn do_export(ctx: &mut crate::zotero::framework::ExportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `rdf`

The RDF import translator: Zotero RDF, Dublin Core, PRISM, BIBO,
schema.org, Open Graph, eprints and vCard terms, read into items, notes,
attachments and collections.

Values move between the RDF store and the item the way they do in
JavaScript; [`V`] is a JavaScript value as this translator handles it. An
RDF term assigned to an item property becomes its `toString()` (what
`_itemDone` turns it into), except under the property names `_itemDone`
lets hold objects, where it is kept as the JSON `JSON.stringify` gives.

```rust
pub mod rdf { /* ... */ }
```

### Functions

#### Function `detect_import`

`detectImport()` (:46-54): any input the RDF data mode can parse (its
`getAllResources()` is an array, and an array is truthy).

```rust
pub fn detect_import(ctx: &mut crate::zotero::framework::ImportContext) -> bool { /* ... */ }
```

#### Function `do_import`

`doImport()` / `startImport` / `importNext` (:1483-1569).

```rust
pub fn do_import(ctx: &mut crate::zotero::framework::ImportContext) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

## Module `rdf_creator_types`

`Zotero.Utilities.getCreatorsForType` as the translation-server answers
it. The RDF import translator walks every creator type of an item type
in this order and appends the creators of each, so the order decides the
order of the imported creators. The framework's
[`get_creators_for_type`](crate::zotero::framework::utilities::get_creators_for_type)
takes kovan-common's schema.json order, which agrees on the first
(primary) type but orders some secondary types differently; this table is
the server's own snapshot.

```rust
pub mod rdf_creator_types { /* ... */ }
```

### Functions

#### Function `server_creators_for_type`

The creator types of `item_type` in the server's order (empty for an
unknown type, `note` and `attachment`).

```rust
pub fn server_creators_for_type(item_type: &str) -> &'static [&'static str] { /* ... */ }
```

### Constants and Statics

#### Constant `SERVER_CREATOR_TYPES`

Item type -> creator types, primary first, in the server's order.

```rust
pub const SERVER_CREATOR_TYPES: &[(&str, &[&str])] = _;
```

## Module `rdf_support`

JavaScript behaviour the RDF translators lean on that the framework does
not already have.

```rust
pub mod rdf_support { /* ... */ }
```

### Functions

#### Function `encode_uri`

`encodeURI(s)`: percent-encode (UTF-8, upper-case hex) everything but
`A-Z a-z 0-9 ; , / ? : @ & = + $ - _ . ! ~ * ' ( ) #`.

```rust
pub fn encode_uri(s: &str) -> String { /* ... */ }
```

#### Function `js_key`

The JavaScript property key a value becomes (`obj[v]`): `undefined` is
`"undefined"`.

```rust
pub fn js_key(v: Option<&serde_json::Value>) -> String { /* ... */ }
```

#### Function `is_array_index`

Whether `s` is an array index (`for-in` visits these first, in numeric
order).

```rust
pub fn is_array_index(s: &str) -> bool { /* ... */ }
```

#### Function `js_key_order`

Keys in JavaScript `for-in` order: array indices ascending, then the
rest in insertion order.

```rust
pub fn js_key_order<''a, /* synthetic */ impl IntoIterator<Item = &'a str>: IntoIterator<Item = &'a str>>(keys: impl IntoIterator<Item = &'a str>) -> Vec<&'a str> { /* ... */ }
```

#### Function `unique_fields_order`

The keys of a legacy item's `uniqueFields` in the order upstream's object
holds them (insertion order), which `for (var p in item.uniqueFields)`
visits.

The framework keeps `uniqueFields` as a JSON object, whose keys
`serde_json` sorts, so the order is rebuilt by replaying the legacy
conversion's visit (`itemToLegacyExportFormat`, utilities_item.js
:1179-1240): it walks the item's properties in order and inserts each
valid field, or its base field when the type maps it to one (a key
inserted a second time keeps its first place); then `version`,
`mimeType` and `note` when it set them.

`props` should be the item as the caller gave it, before the conversion
(the export input object): the conversion removes `versionNumber` from the
item, so the converted item no longer shows where it was. Given the
converted item instead, the order is right except for a removed property.

```rust
pub fn unique_fields_order(item_type: &str, props: &crate::zotero::framework::JsObject, uf: &serde_json::Map<String, serde_json::Value>) -> Vec<String> { /* ... */ }
```

#### Function `item_unique_fields_order`

[`unique_fields_order`] of a [`TranslatorItem`], replayed on `original`
(the export input object it was made from) when given.

```rust
pub fn item_unique_fields_order(item: &crate::zotero::framework::TranslatorItem, original: Option<&crate::zotero::framework::JsObject>) -> Vec<String> { /* ... */ }
```

### Constants and Statics

#### Constant `INHERITED_NAMES`

The names a JavaScript array or plain object inherits (`[]["map"]` is a
function, so `used[x]` is truthy for these keys).

```rust
pub const INHERITED_NAMES: [&str; 47] = _;
```

## Module `zotero_rdf`

The Zotero RDF export translator: Zotero's full-fidelity library format
(items, notes, attachments, tags, related items and collections).

Values are read as upstream reads them from the export item, which here is
loosely typed JSON: a missing value where upstream calls
`Zotero.RDF.addStatement` (a creator with no last name, a note with no
text) is the same error upstream throws.

```rust
pub mod zotero_rdf { /* ... */ }
```

### Functions

#### Function `do_export`

`doExport()` (:502-582).

`originals` are the export input objects the context's items were made
from, in order; they give the order of each item's `uniqueFields`
([`super::rdf_support::unique_fields_order`]).

```rust
pub fn do_export(ctx: &mut crate::zotero::framework::ExportContext, originals: &[crate::zotero::framework::JsObject]) -> Result<(), crate::zotero::framework::TranslateError> { /* ... */ }
```

### Constants and Statics

#### Static `METADATA`

The translator header.

```rust
pub static METADATA: crate::zotero::framework::options::TranslatorMetadata = _;
```

### Types

#### Enum `Translator`

A ported Zotero translator.

```rust
pub enum Translator {
    BibTeX,
    BibLaTeX,
    Ris,
    CslJson,
    Refer,
    RefWorksTagged,
    Bookmarks,
    MedlineNbib,
    OvidTagged,
    WosTagged,
    Mab2,
    DataciteJson,
    OpenAlexJson,
    Csv,
    Coins,
    WikipediaCitationTemplates,
    WikidataQuickStatements,
    Cff,
    CffReferences,
    Evernote,
    Mods,
    EndnoteXml,
    Tei,
    CrossrefUnixrefXml,
    MarcXml,
    Marc,
    PubMedXml,
    Mets,
    PrimoNormalizedXml,
    DSpaceIntermediateMetadata,
    Citavi5Xml,
    XmlContextObject,
    NoteHtml,
    NoteMarkdown,
    ZoteroRdf,
    Rdf,
    BibliontologyRdf,
    DcRdf,
}
```

##### Variants

###### `BibTeX`

`BibTeX.js` (import and export).

###### `BibLaTeX`

`BibLaTeX.js` (export).

###### `Ris`

`RIS.js` (import and export).

###### `CslJson`

`CSL JSON.js` (import and export).

###### `Refer`

`ReferBibIX.js` (import and export).

###### `RefWorksTagged`

`RefWorks Tagged.js` (import and export).

###### `Bookmarks`

`Bookmarks.js` (import and export).

###### `MedlineNbib`

`MEDLINEnbib.js` (import).

###### `OvidTagged`

`OVID Tagged.js` (import).

###### `WosTagged`

`Web of Science Tagged.js` (import).

###### `Mab2`

`MAB2.js` (import).

###### `DataciteJson`

`Datacite JSON.js` (import).

###### `OpenAlexJson`

`OpenAlex JSON.js` (import).

###### `Csv`

`CSV.js` (export).

###### `Coins`

`COinS.js` (export).

###### `WikipediaCitationTemplates`

`Wikipedia Citation Templates.js` (export).

###### `WikidataQuickStatements`

`Wikidata QuickStatements.js` (export).

###### `Cff`

`CFF.js` (export).

###### `CffReferences`

`CFF References.js` (export).

###### `Evernote`

`Evernote.js` (export).

###### `Mods`

`MODS.js` (import and export).

###### `EndnoteXml`

`Endnote XML.js` (import and export).

###### `Tei`

`TEI.js` (export).

###### `CrossrefUnixrefXml`

`Crossref Unixref XML.js` (import).

###### `MarcXml`

`MARCXML.js` (import).

###### `Marc`

`MARC.js` (import).

###### `PubMedXml`

`PubMed XML.js` (import).

###### `Mets`

`METS.js` (import).

###### `PrimoNormalizedXml`

`Primo Normalized XML.js` (import).

###### `DSpaceIntermediateMetadata`

`DSpace Intermediate Metadata.js` (import).

###### `Citavi5Xml`

`Citavi 5 XML.js` (import).

###### `XmlContextObject`

`XML ContextObject.js` (import).

###### `NoteHtml`

`Note HTML.js` (export).

###### `NoteMarkdown`

`Note Markdown.js` (export).

###### `ZoteroRdf`

`Zotero RDF.js` (export).

###### `Rdf`

`RDF.js` (import).

###### `BibliontologyRdf`

`Bibliontology RDF.js` (import and export).

###### `DcRdf`

`Unqualified Dublin Core RDF.js` (export).

##### Implementations

###### Methods

- ```rust
  pub fn metadata(self: Self) -> &'static TranslatorMetadata { /* ... */ }
  ```
  The translator's header.

- ```rust
  pub fn format_name(self: Self) -> &'static str { /* ... */ }
  ```
  The translation-server's format name (`/export?format=`,

- ```rust
  pub fn from_format_name(name: &str) -> Option<Translator> { /* ... */ }
  ```
  The translator with this format name.

- ```rust
  pub fn from_id(id: &str) -> Option<Translator> { /* ... */ }
  ```
  The translator with this `translatorID`.

- ```rust
  pub fn default_options(self: Self) -> TranslateOptions { /* ... */ }
  ```
  The options the translator runs with by default (its header's

- ```rust
  pub fn detect_import(self: Self, input: &str) -> bool { /* ... */ }
  ```
  `detectImport` on `input` (false for an export-only translator).

- ```rust
  pub fn import(self: Self, input: &str, options: &TranslateOptions) -> Result<ImportResult, TranslateError> { /* ... */ }
  ```
  Import `input` (`doImport`), returning the saved items.

- ```rust
  pub fn export(self: Self, items: &[JsObject], options: &TranslateOptions) -> Result<String, TranslateError> { /* ... */ }
  ```
  Export items given as JSON objects in their own key order (Web API

- ```rust
  pub fn export_with_collections(self: Self, items: &[JsObject], collections: &[JsObject], options: &TranslateOptions) -> Result<String, TranslateError> { /* ... */ }
  ```
  Export items and collections (#749): the collections are what

- ```rust
  pub fn export_json(self: Self, json: &str, options: &TranslateOptions) -> Result<String, TranslateError> { /* ... */ }
  ```
  Export a JSON array of items (what the translation-server's

- ```rust
  pub fn export_zotero_items(self: Self, items: &[ZoteroItem], options: &TranslateOptions) -> Result<String, TranslateError> { /* ... */ }
  ```
  Export kovan-common items.

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
    fn clone(self: &Self) -> Translator { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Translator) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
### Functions

#### Function `detect_import`

The import translators whose `detectImport` accepts `input`, in the
order the translation-server tries them; its `/import` uses the first.
(Upstream tries all of Zotero's import translators; this covers the
ported ones only.)

```rust
pub fn detect_import(input: &str) -> Vec<Translator> { /* ... */ }
```

## Module `storage`

Roots of the on-disk storage tree, relative to the crate directory.

Implements `docs/kovan.md`, "Storage Layout".

```rust
pub mod storage { /* ... */ }
```

### Functions

#### Function `generated_dir_for`

Directory a generated artifact of `kind` and `visibility` belongs in,
joined onto `base` (usually the `kovan-literature` crate directory).

The generated tree is split by [`Visibility`] one level below each artifact
kind — `generated/bibtex/open/`, `generated/bibtex/proprietary/`, and so on
— because the two halves have different distribution rules:

- **open** — committed to the repository, and for BibTeX also *published*
  in the packaged crate (citation entries are small bibliographic facts).
- **proprietary** — never committed and never published; an artifact
  derived from user-owned content is equally user-owned.

Both rules are enforced outside this function (the root `.gitignore` and
the `exclude` list in this crate's `Cargo.toml`); this is the single place
that decides *which* directory a writer should target, so the two
mechanisms and the code cannot drift apart.

`kind` should be one of [`BIBTEX_DIR`], [`MARKDOWN_DIR`], [`ASSETS_DIR`].

```rust
pub fn generated_dir_for(base: &std::path::Path, kind: &str, visibility: super::Visibility) -> std::path::PathBuf { /* ... */ }
```

#### Function `root_for`

Return the storage root for a given [`Visibility`], joined onto `base`
(usually the `kovan-literature` crate directory).

```rust
pub fn root_for(base: &std::path::Path, visibility: super::Visibility) -> std::path::PathBuf { /* ... */ }
```

#### Function `visibility_from_path`

Infer a document's [`Visibility`] from where its source file lives.

**Closed by default.** A document is [`Visibility::Open`] only when its
path explicitly contains an `open/` component. Everything else —
including `proprietary/`, and including any path with neither marker —
is [`Visibility::Proprietary`].

# Why the default is closed

The two ways of being wrong here are not symmetric:

- Mislabelling an **open** document as proprietary costs a reviewer a
  minute and keeps a committable file out of git. Recoverable.
- Mislabelling a **proprietary** document as open invites it into
  `open/`, which `.gitignore` deliberately un-ignores for PDFs, and from
  there into a public repository. That is a licence violation, and
  pushed history is not something you can quietly take back.

So the rule fails towards the recoverable error. This matches the
instruction in `kovan_import/README.md` — "unsure -> treat as
proprietary and ask" — and `DATA_POLICY.md`.

# The bug this replaced

Until 2026-08-11 this defaulted to [`Visibility::Open`] and only
special-cased `proprietary/`, so a source file staged anywhere else —
notably `kovan_import/`, the gitignored drop area where documents sit
*before* their access tier has been decided — was silently labelled
Open. That is precisely the unrecoverable direction. It was found when
Tobias (1980), a Pergamon Press work with all rights reserved, imported
as `visibility: Open` despite being written to proprietary output paths
(bead `op-nv6g`). The old doc comment claimed the function existed "so
proprietary material never gets an open label by accident", which is
what it should have done and did not.

Note this is a *storage-layout* inference, not a licence determination.
The access tier is decided by a human reading the document's own
copyright page, then expressed by choosing where to put the file.

```rust
pub fn visibility_from_path(path: &std::path::Path) -> super::Visibility { /* ... */ }
```

#### Function `document_type_from_path`

Infer a [`super::DocumentType`] from a storage sub-directory name in the
source path (`papers/`, `reports/`, `standards/`, `benchmarks/`,
`manuals/`, `theses/` or `dissertations/`), falling back to
[`super::DocumentType::Other`] when none is present.

```rust
pub fn document_type_from_path(path: &std::path::Path) -> super::DocumentType { /* ... */ }
```

### Constants and Statics

#### Constant `OPEN_ROOT`

Directory for redistributable content that may be committed.

```rust
pub const OPEN_ROOT: &str = "open";
```

#### Constant `PROPRIETARY_ROOT`

Directory for user-owned content that must never be committed.

```rust
pub const PROPRIETARY_ROOT: &str = "proprietary";
```

#### Constant `GENERATED_ROOT`

Directory for reproducible generated artifacts.

```rust
pub const GENERATED_ROOT: &str = "generated";
```

#### Constant `BIBTEX_DIR`

Sub-directory of [`GENERATED_ROOT`] holding generated BibTeX entries.

```rust
pub const BIBTEX_DIR: &str = "bibtex";
```

#### Constant `MARKDOWN_DIR`

Sub-directory of [`GENERATED_ROOT`] holding generated Markdown bodies.

```rust
pub const MARKDOWN_DIR: &str = "markdown";
```

#### Constant `ASSETS_DIR`

Sub-directory of [`GENERATED_ROOT`] holding extracted image assets.

```rust
pub const ASSETS_DIR: &str = "assets";
```

## Types

### Enum `LiteratureError`

Errors produced by the literature pipeline.

```rust
pub enum LiteratureError {
    Unimplemented(&'static str),
    Io(String),
}
```

#### Variants

##### `Unimplemented`

The requested operation is not implemented yet (placeholder stage).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&'static str` |  |

##### `Io`

A source file could not be read, parsed, or was malformed.

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> LiteratureError { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &Q) -> Ordering { /* ... */ }
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

  - ```rust
    fn equivalent(self: &Self, key: &Q) -> bool { /* ... */ }
    ```

- **ErasedDestructor**
- **Error**
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
    fn eq(self: &Self, other: &LiteratureError) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WithSubscriber**
## Constants and Statics

### Constant `MAX_MARKDOWN_PAGES`

Target maximum number of pages per generated Markdown document. Larger
documents should be split with [`split_markdown_by_page_limit`]. See
`docs/kovan.md`, "PDF Processing" (`≤ 30 pages` per Markdown document).

```rust
pub const MAX_MARKDOWN_PAGES: u32 = 30;
```

## Re-exports

### Re-export `Author`

```rust
pub use kovan_common::Author;
```

### Re-export `DocumentType`

```rust
pub use kovan_common::DocumentType;
```

### Re-export `KovanBenchmark`

```rust
pub use kovan_common::KovanBenchmark;
```

### Re-export `KovanDocument`

```rust
pub use kovan_common::KovanDocument;
```

### Re-export `Visibility`

```rust
pub use kovan_common::Visibility;
```

### Re-export `parse_bib_entries`

```rust
pub use bibtex::parse_bib_entries;
```

### Re-export `render_entries`

```rust
pub use bibtex::render_entries;
```

### Re-export `render_entry`

```rust
pub use bibtex::render_entry;
```

### Re-export `to_bibtex`

```rust
pub use bibtex::to_bibtex;
```

### Re-export `BibEntry`

```rust
pub use bibtex::BibEntry;
```

### Re-export `BibParseError`

```rust
pub use bibtex::BibParseError;
```

### Re-export `markdown_outline`

```rust
pub use markdown::markdown_outline;
```

### Re-export `split_markdown_by_page_limit`

```rust
pub use markdown::split_markdown_by_page_limit;
```

### Re-export `text_to_markdown`

```rust
pub use markdown::text_to_markdown;
```

### Re-export `Heading`

```rust
pub use markdown::Heading;
```

### Re-export `PAGE_SEPARATOR`

```rust
pub use markdown::PAGE_SEPARATOR;
```

### Re-export `concept_tree`

```rust
pub use concept_tree::concept_tree;
```

### Re-export `ConceptDocument`

```rust
pub use concept_tree::ConceptDocument;
```

### Re-export `ConceptNode`

```rust
pub use concept_tree::ConceptNode;
```

### Re-export `ConceptOrigin`

```rust
pub use concept_tree::ConceptOrigin;
```

### Re-export `ConceptSource`

```rust
pub use concept_tree::ConceptSource;
```

### Re-export `ConceptStatus`

```rust
pub use concept_tree::ConceptStatus;
```

### Re-export `ConceptTree`

```rust
pub use concept_tree::ConceptTree;
```

### Re-export `ConceptTreeError`

```rust
pub use concept_tree::ConceptTreeError;
```

### Re-export `DocumentTier`

```rust
pub use concept_tree::DocumentTier;
```

### Re-export `extract_metadata`

```rust
pub use metadata::extract_metadata;
```

### Re-export `extract_assets`

```rust
pub use pdf_import::extract_assets;
```

### Re-export `extract_pdf_text`

```rust
pub use pdf_import::extract_pdf_text;
```

### Re-export `pdf_to_markdown`

```rust
pub use pdf_import::pdf_to_markdown;
```

