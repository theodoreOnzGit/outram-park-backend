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
[`KovanDocument`]s (GitHub #750). Native desktop targets only: it is
compiled out on wasm32 and Android (SQLite is C; see `Cargo.toml`).

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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> DocumentTier { /* ... */ }
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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ConceptDocument { /* ... */ }
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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ConceptSource { /* ... */ }
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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ConceptOrigin { /* ... */ }
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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ConceptStatus { /* ... */ }
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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ConceptNode { /* ... */ }
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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ConceptTreeError { /* ... */ }
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
pub const SKELETON_TOML: &str = "# Kovan\'s concept-tree skeleton (OUTRAM PARK GitHub #724, #726, #727).\n# Lives in kovan-literature, beside the corpus it links to (the reactor-literature\n# submodule is mounted in this crate); wasm-clean, so the Code Review tab can be\n# built for the browser as well as the desktop.\n#\n# Two fixed levels, shared by the Literature and Code Review tabs:\n#   L1: the 19 infrastructure issues of IAEA NG-G-3.1 (Rev. 1), numbered so\n#       `NN-` is the document\'s section 3.NN (decided 2026-10-06);\n#   L2: categories from the open NRC guides (NUREG-0800, NUREG-1537 Part 1,\n#       RG 1.232, NUREG-1520, NUREG-1555, NUREG-0654/FEMA-REP-1) and ORNL\'s\n#       MSR gap analysis, plain-word segments with no number.\n# The owner\'s concepts hang below L2 as folders, as today.\n#\n# Every source names a [[document]] by `id`; each document names its file in\n# a corpus repository (`tier = \"standard\"`: reactor-literature; `\"private\"`:\n# the owner\'s private corpus). This is the INITIAL linking, at document and\n# section level (2026-10-06); finer artifact links (page anchors, quotes) are\n# the owner\'s, added later. Section numbers are the documents\' own.\n# `cross_links` names other nodes a node also belongs to (shown as a dotted\n# link; the node\'s home is its path).\n# A node is shown greyed (and hidden by the \'show empty nodes\' toggle) when\n# nothing is classified under it; that is computed, not stored here.\n\n[[document]]\nid = \"iaea-ng-g-3.1-rev1\"\ntitle = \"IAEA Nuclear Energy Series No. NG-G-3.1 (Rev. 1), Milestones in the Development of a National Infrastructure for Nuclear Power\"\npublisher = \"IAEA, Vienna\"\ndate = \"2015\"\ntier = \"private\"\nfile = \"reports/iaea2015ngg31rev1-milestones.pdf\"\nlicence = \"(c) IAEA; cited by section and page only\"\n\n[[document]]\nid = \"nureg-0800-toc-rev6\"\ntitle = \"NUREG-0800, Standard Review Plan for the Review of Safety Analysis Reports for Nuclear Power Plants, Table of Contents, Revision 6\"\npublisher = \"U.S. NRC\"\ndate = \"March 2007\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/nrc/ML070810350.pdf\"\nlicence = \"U.S. Government work\"\n\n[[document]]\nid = \"nureg-0800-4.2-rev3\"\ntitle = \"NUREG-0800, Standard Review Plan, Section 4.2, Revision 3, Fuel System Design\"\npublisher = \"U.S. NRC\"\ndate = \"March 2007\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/nrc/ML070740002.pdf\"\nlicence = \"U.S. Government work\"\n\n[[document]]\nid = \"nureg-1537-part1\"\ntitle = \"NUREG-1537, Part 1, Guidelines for Preparing and Reviewing Applications for the Licensing of Non-Power Reactors: Format and Content\"\npublisher = \"U.S. NRC, Office of Nuclear Reactor Regulation\"\ndate = \"February 1996\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/nrc/nureg-1537-part1-1996.pdf\"\nlicence = \"U.S. Government work\"\n\n[[document]]\nid = \"rg-1.232-rev0\"\ntitle = \"Regulatory Guide 1.232, Revision 0, Guidance for Developing Principal Design Criteria for Non-Light-Water Reactors\"\npublisher = \"U.S. NRC\"\ndate = \"April 2018\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/nrc/ML17325A611.pdf\"\nlicence = \"U.S. Government work\"\n\n[[document]]\nid = \"ornl-tm-2018-976\"\ntitle = \"ORNL/TM-2018/976, Regulatory Gap Analysis of Select NUREG-0800 Chapters for Applicability to Molten Salt Reactors (Belles, Flanagan)\"\npublisher = \"Oak Ridge National Laboratory for U.S. DOE\"\ndate = \"October 2018\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/us-doe/ornl-tm-2018-976-msr-nureg0800-gap-analysis.pdf\"\nlicence = \"approved for public release, distribution unlimited\"\n\n[[document]]\nid = \"nureg-1520-rev2\"\ntitle = \"NUREG-1520, Revision 2, Standard Review Plan for Fuel Cycle Facilities License Applications\"\npublisher = \"U.S. NRC, Office of Nuclear Material Safety and Safeguards\"\ndate = \"2015\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/nrc/nureg-1520-rev2-2015.pdf\"\nlicence = \"U.S. Government work\"\n\n[[document]]\nid = \"nureg-1555\"\ntitle = \"NUREG-1555, Standard Review Plans for Environmental Reviews for Nuclear Power Plants\"\npublisher = \"U.S. NRC, Office of Nuclear Reactor Regulation\"\ndate = \"October 1999\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/nrc/nureg-1555-1999.pdf\"\nlicence = \"U.S. Government work\"\n\n[[document]]\nid = \"nureg-0654-rev2\"\ntitle = \"NUREG-0654/FEMA-REP-1, Revision 2, Criteria for Preparation and Evaluation of Radiological Emergency Response Plans and Preparedness in Support of Nuclear Power Plants\"\npublisher = \"U.S. NRC and FEMA\"\ndate = \"December 2019\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/nrc/nureg-0654-fema-rep-1-rev2-2019.pdf\"\nlicence = \"U.S. Government work\"\n\n[[document]]\nid = \"jrc-eur-28712\"\ntitle = \"K. Kugeler, H. Nabielek, D. Buckthorpe, The High Temperature Gas-cooled Reactor: Safety considerations of the (V)HTR-Modul, EUR 28712 EN\"\npublisher = \"European Commission, Joint Research Centre (Publications Office of the European Union)\"\ndate = \"2017\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/eu-jrc/kjna28712enn.pdf\"\nlicence = \"reuse authorised provided the source is acknowledged (Decision 2011/833/EU)\"\n\n[[document]]\nid = \"10cfr50\"\ntitle = \"10 CFR Part 50, Domestic Licensing of Production and Utilization Facilities (eCFR, as of 2 Oct 2026)\"\npublisher = \"U.S. Government (Office of the Federal Register; NRC regulations, 10 CFR Chapter I)\"\ndate = \"2 October 2026 (eCFR)\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/cfr/10cfr50-ecfr-2026-10-02.pdf\"\nlicence = \"U.S. Government work (17 U.S.C. 105); eCFR: authoritative but unofficial\"\n\n[[document]]\nid = \"10cfr52\"\ntitle = \"10 CFR Part 52, Licenses, Certifications, and Approvals for Nuclear Power Plants (eCFR, as of 2 Oct 2026)\"\npublisher = \"U.S. Government (Office of the Federal Register; NRC regulations, 10 CFR Chapter I)\"\ndate = \"2 October 2026 (eCFR)\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/cfr/10cfr52-ecfr-2026-10-02.pdf\"\nlicence = \"U.S. Government work (17 U.S.C. 105); eCFR: authoritative but unofficial\"\n\n[[document]]\nid = \"10cfr53\"\ntitle = \"10 CFR Part 53, Risk-Informed, Technology-Inclusive Regulatory Framework for Commercial Nuclear Plants (eCFR, as of 2 Oct 2026; final rule 91 FR 15794, Mar. 30, 2026)\"\npublisher = \"U.S. Government (Office of the Federal Register; NRC regulations, 10 CFR Chapter I)\"\ndate = \"2 October 2026 (eCFR)\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/cfr/10cfr53-ecfr-2026-10-02.pdf\"\nlicence = \"U.S. Government work (17 U.S.C. 105); eCFR: authoritative but unofficial\"\n\n[[document]]\nid = \"iaea-nutec-plastics\"\ntitle = \"IAEA, NUTEC Plastics: Scaling up solutions and partnerships for global impact\"\npublisher = \"IAEA\"\ndate = \"July 2026\"\ntier = \"private\"\nfile = \"reports/iaea2026-nutec-plastics-scaling-up.pdf\"\nlicence = \"no reuse licence found; cited only\"\n\n[[document]]\nid = \"wash-1400\"\ntitle = \"WASH-1400 (NUREG-75/014), Reactor Safety Study: An Assessment of Accident Risks in U.S. Commercial Nuclear Power Plants\"\npublisher = \"U.S. NRC\"\ndate = \"October 1975\"\ntier = \"standard\"\nfile = \"kovan-standard-open-corpus/nrc/ML15334A199.pdf\"\nlicence = \"U.S. Government work\"\n\n[[node]]\npath = \"01-national-position\"\ntitle = \"National position\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.1\", page = \"10\" }]\n\n[[node]]\npath = \"01-national-position/need-for-power\"\ntitle = \"Need for power\"\nsources = [{ document = \"nureg-1555\", section = \"Ch. 8\" }]\n\n[[node]]\npath = \"02-nuclear-safety\"\ntitle = \"Nuclear safety\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.2\", page = \"14\" }]\n\n[[node]]\npath = \"02-nuclear-safety/facility-description\"\ntitle = \"Facility description\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 1\" }, { document = \"nureg-1537-part1\", section = \"Ch. 1\" }, { document = \"rg-1.232-rev0\", section = \"Sec. I, Overall requirements\" }]\n\n[[node]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components\"\ntitle = \"Design of structures, systems and components\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 3\" }, { document = \"nureg-1537-part1\", section = \"Ch. 3\" }, { document = \"10cfr53\", section = \"\u{a7} 53.415 Protection against external hazards\" }]\n\n[[node]]\npath = \"02-nuclear-safety/fuel-system-design\"\ntitle = \"Fuel system design\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"4.2\" }, { document = \"nureg-0800-4.2-rev3\", section = \"whole section\" }, { document = \"nureg-1537-part1\", section = \"4.2.1\" }, { document = \"rg-1.232-rev0\", section = \"Sec. II, Multiple barriers (MHTGR-DC 10, 16 functional containment)\" }, { document = \"ornl-tm-2018-976\", section = \"3.1.1\" }]\n\n[[node]]\npath = \"02-nuclear-safety/nuclear-design\"\ntitle = \"Nuclear design and core physics\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"4.3\" }, { document = \"nureg-1537-part1\", section = \"4.5 (4.5.2 core physics parameters)\" }, { document = \"rg-1.232-rev0\", section = \"Sec. III, Reactivity control\" }, { document = \"ornl-tm-2018-976\", section = \"3.1.2\" }]\n\n[[node]]\npath = \"02-nuclear-safety/moderator-and-reflector\"\ntitle = \"Moderator and reflector\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.2.3\" }]\n\n[[node]]\npath = \"02-nuclear-safety/thermal-hydraulic-design\"\ntitle = \"Thermal-hydraulic design\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"4.4\" }, { document = \"nureg-1537-part1\", section = \"4.6\" }, { document = \"rg-1.232-rev0\", section = \"Sec. IV, Fluid systems\" }, { document = \"ornl-tm-2018-976\", section = \"3.1.3\" }]\n\n[[node]]\npath = \"02-nuclear-safety/control-rods-and-drives\"\ntitle = \"Control rods and drives\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"4.5, 4.6\" }, { document = \"nureg-1537-part1\", section = \"4.2.2\" }, { document = \"ornl-tm-2018-976\", section = \"3.1.4, 3.1.6\" }]\n\n[[node]]\npath = \"02-nuclear-safety/reactor-coolant-system\"\ntitle = \"Reactor coolant system\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 5\" }, { document = \"nureg-1537-part1\", section = \"Ch. 5\" }, { document = \"rg-1.232-rev0\", section = \"Sec. IV\" }, { document = \"ornl-tm-2018-976\", section = \"3.2 (proposed 5.4.20 fuel salt drain tank, 5.4.21 freeze valve boundary)\" }]\n\n[[node]]\npath = \"02-nuclear-safety/msr-coolant-loop\"\ntitle = \"MSR coolant loop\"\nsources = [{ document = \"ornl-tm-2018-976\", section = \"3.2.4 (proposed SRP Section 5.5)\" }]\n\n[[node]]\npath = \"02-nuclear-safety/engineered-safety-features\"\ntitle = \"Engineered safety features\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 6\" }, { document = \"nureg-1537-part1\", section = \"Ch. 6\" }, { document = \"ornl-tm-2018-976\", section = \"3.3\" }]\n\n[[node]]\npath = \"02-nuclear-safety/containment\"\ntitle = \"Containment (incl. functional containment)\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"6.2\" }, { document = \"rg-1.232-rev0\", section = \"Sec. V, Reactor containment\" }, { document = \"ornl-tm-2018-976\", section = \"3.3.2\" }]\n\n[[node]]\npath = \"02-nuclear-safety/instrumentation-and-control\"\ntitle = \"Instrumentation and control\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 7\" }, { document = \"nureg-1537-part1\", section = \"Ch. 7\" }]\n\n[[node]]\npath = \"02-nuclear-safety/auxiliary-systems\"\ntitle = \"Auxiliary systems\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 9\" }, { document = \"nureg-1537-part1\", section = \"Ch. 9\" }, { document = \"ornl-tm-2018-976\", section = \"3.4 (proposed 9.2.X drain tank cooling)\" }]\n\n[[node]]\npath = \"02-nuclear-safety/steam-and-power-conversion\"\ntitle = \"Steam and power conversion system\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 10\" }]\n\n[[node]]\npath = \"02-nuclear-safety/experimental-facilities-and-utilization\"\ntitle = \"Experimental facilities and utilization\"\nsources = [{ document = \"nureg-1537-part1\", section = \"Ch. 10\" }]\n\n[[node]]\npath = \"02-nuclear-safety/conduct-of-operations\"\ntitle = \"Conduct of operations\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 13\" }, { document = \"nureg-1537-part1\", section = \"Ch. 12\" }]\n\n[[node]]\npath = \"02-nuclear-safety/initial-test-program\"\ntitle = \"Initial test program and ITAAC\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 14\" }]\n\n[[node]]\npath = \"02-nuclear-safety/accident-analysis\"\ntitle = \"Accident analysis\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 15 (15.0.2 review of transient and accident analysis methods)\" }, { document = \"nureg-1537-part1\", section = \"Ch. 13\" }]\n\n[[node]]\npath = \"02-nuclear-safety/technical-specifications\"\ntitle = \"Technical specifications\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 16\" }, { document = \"nureg-1537-part1\", section = \"Ch. 14\" }, { document = \"10cfr50\", section = \"\u{a7} 50.36 Technical specifications\" }, { document = \"10cfr53\", section = \"\u{a7} 53.710 Maintaining capabilities and availability of structures, systems, and components, (a)\" }]\n\n[[node]]\npath = \"02-nuclear-safety/human-factors-engineering\"\ntitle = \"Human factors engineering\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 18\" }, { document = \"10cfr53\", section = \"\u{a7} 53.730 Defining, fulfilling, and maintaining the role of personnel in ensuring safe operations, (a) Human factors engineering design requirements\" }, { document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (n)(1)\" }]\n\n[[node]]\npath = \"02-nuclear-safety/severe-accidents\"\ntitle = \"Severe accidents\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 19\" }]\n\n[[node]]\npath = \"02-nuclear-safety/source-terms\"\ntitle = \"Source terms\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"11.1 Source Terms; 15.0.1 Radiological Consequence Analyses Using Alternate Source Terms\" }, { document = \"nureg-1555\", section = \"Ch. 7 (postulated accidents)\" }, { document = \"ornl-tm-2018-976\", section = \"3.5.1 (coolant source terms, MSR)\" }]\ncross_links = [\"13-environmental-protection\"]\n\n[[node]]\npath = \"03-management\"\ntitle = \"Management\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.3\", page = \"17\" }]\n\n[[node]]\npath = \"03-management/organization-and-administration\"\ntitle = \"Organization and administration\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"Ch. 2\" }]\n\n[[node]]\npath = \"03-management/management-measures\"\ntitle = \"Management measures\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"Ch. 11\" }]\n\n[[node]]\npath = \"04-funding-and-financing\"\ntitle = \"Funding and financing\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.4\", page = \"20\" }]\n\n[[node]]\npath = \"04-funding-and-financing/financial-qualifications\"\ntitle = \"Financial qualifications\"\nsources = [{ document = \"nureg-1537-part1\", section = \"Ch. 15\" }, { document = \"10cfr50\", section = \"\u{a7} 50.33 Contents of applications; general information, (f)\" }, { document = \"10cfr50\", section = \"Appendix C, A Guide for the Financial Data and Related Information Required To Establish Financial Qualifications for Construction Permits and Combined Licenses\" }, { document = \"10cfr50\", section = \"\u{a7} 50.76 Licensee\'s change of status; financial qualifications\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1670 Financial qualifications\" }]\n\n[[node]]\npath = \"05-legal-framework\"\ntitle = \"Legal framework\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.5\", page = \"25\" }]\n\n[[node]]\npath = \"05-legal-framework/nuclear-legislation\"\ntitle = \"Nuclear legislation (safety, security, safeguards and civil liability)\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.5\", page = \"25\" }]\n\n[[node]]\npath = \"05-legal-framework/international-legal-instruments\"\ntitle = \"International legal instruments (conventions adopted under IAEA auspices)\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.5, Box 1\", page = \"26\" }]\n\n[[node]]\npath = \"05-legal-framework/independent-regulatory-body\"\ntitle = \"Independent regulatory body (separation of regulatory and promotional functions)\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.5\", page = \"25\" }]\ncross_links = [\"07-regulatory-framework\"]\n\n[[node]]\npath = \"05-legal-framework/civil-liability-for-nuclear-damage\"\ntitle = \"Civil liability for nuclear damage (Vienna, Paris, CSC; U.S. financial protection)\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.5, Box 1\", page = \"26\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1710-53.1730 (financial protection under part 140)\" }]\ncross_links = [\"04-funding-and-financing/financial-qualifications/financial-protection-and-accident-insurance\"]\n\n[[node]]\npath = \"05-legal-framework/licensee-obligations\"\ntitle = \"Licensee obligations (deliberate misconduct, employee protection, completeness and accuracy, violations, license transfers, exemptions)\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.5, 50.7, 50.9, 50.110, 50.80, 50.12\" }, { document = \"10cfr52\", section = \"\u{a7} 52.4-52.7\" }, { document = \"10cfr53\", section = \"\u{a7} 53.050-53.080, 53.1570\" }]\n\n[[node]]\npath = \"06-safeguards\"\ntitle = \"Safeguards\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.6\", page = \"28\" }]\n\n[[node]]\npath = \"06-safeguards/material-control-and-accounting\"\ntitle = \"Material control and accounting\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"Ch. 12\" }, { document = \"10cfr50\", section = \"\u{a7} 50.78 Facility information and verification\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1650 Facility information and verification\" }]\n\n[[node]]\npath = \"07-regulatory-framework\"\ntitle = \"Regulatory framework\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.7\", page = \"30\" }]\n\n[[node]]\npath = \"07-regulatory-framework/reactor-licensing\"\ntitle = \"Reactor licensing\"\nsources = [{ document = \"10cfr50\", section = \"Part 50\" }, { document = \"10cfr52\", section = \"Part 52\" }, { document = \"10cfr53\", section = \"Part 53\" }]\n\n[[node]]\npath = \"07-regulatory-framework/quality-assurance\"\ntitle = \"Quality assurance (incl. software QA)\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 17\" }]\n\n[[node]]\npath = \"07-regulatory-framework/other-license-considerations\"\ntitle = \"Other license considerations\"\nsources = [{ document = \"nureg-1537-part1\", section = \"Ch. 16\" }]\n\n[[node]]\npath = \"08-radiation-protection\"\ntitle = \"Radiation protection\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.8\", page = \"35\" }]\n\n[[node]]\npath = \"08-radiation-protection/radiation-protection\"\ntitle = \"Radiation protection\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 12\" }, { document = \"nureg-1537-part1\", section = \"Ch. 11\" }, { document = \"nureg-1520-rev2\", section = \"Ch. 4\" }]\n\n[[node]]\npath = \"09-electrical-grid\"\ntitle = \"Electrical grid\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.9\", page = \"36\" }]\n\n[[node]]\npath = \"09-electrical-grid/electric-power\"\ntitle = \"Electric power\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 8\" }, { document = \"nureg-1537-part1\", section = \"Ch. 8\" }]\n\n[[node]]\npath = \"10-human-resource-development\"\ntitle = \"Human resource development\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.10\", page = \"38\" }]\n\n[[node]]\npath = \"10-human-resource-development/knowledge-management-and-education\"\ntitle = \"Knowledge management and education\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"\u{a7}3.10, p. 38\" }]\n\n[[node]]\npath = \"11-stakeholder-involvement\"\ntitle = \"Stakeholder involvement\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.11\", page = \"42\" }]\n\n[[node]]\npath = \"11-stakeholder-involvement/public-information-and-communication\"\ntitle = \"Public information and communication (surveys, information tools, benefits and risks)\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.11.1\", page = \"43\" }]\n\n[[node]]\npath = \"11-stakeholder-involvement/stakeholder-involvement-programmes\"\ntitle = \"Stakeholder involvement programmes (government, owner/operator, regulatory body; neighbouring countries)\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.11.1-3.11.2\", page = \"43\" }]\n\n[[node]]\npath = \"11-stakeholder-involvement/public-participation-in-licensing\"\ntitle = \"Public participation in licensing (public inspection of applications, notice for comment, hearings, ACRS)\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.39, 50.58, 50.91\" }, { document = \"10cfr52\", section = \"\u{a7} 52.21, 52.85, 52.163\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1121, 53.1155\" }]\ncross_links = [\"07-regulatory-framework/reactor-licensing\"]\n\n[[node]]\npath = \"12-site-and-supporting-facilities\"\ntitle = \"Site and supporting facilities\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.12\", page = \"45\" }]\n\n[[node]]\npath = \"12-site-and-supporting-facilities/site-characteristics\"\ntitle = \"Site characteristics\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 2\" }, { document = \"nureg-1537-part1\", section = \"Ch. 2\" }, { document = \"10cfr53\", section = \"\u{a7} 53.500 General siting and siting assessment\" }, { document = \"10cfr53\", section = \"\u{a7} 53.520 Site characteristics\" }, { document = \"10cfr50\", section = \"Appendix Q, Pre-Application Early Review of Site Suitability Issues\" }]\n\n[[node]]\npath = \"13-environmental-protection\"\ntitle = \"Environmental protection\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.13\", page = \"48\" }]\n\n[[node]]\npath = \"13-environmental-protection/environmental-impact-statement\"\ntitle = \"Environmental impact statement\"\nsources = [{ document = \"nureg-1555\", section = \"Ch. 1\" }]\n\n[[node]]\npath = \"13-environmental-protection/environmental-description\"\ntitle = \"Environmental description\"\nsources = [{ document = \"nureg-1555\", section = \"Ch. 2\" }]\n\n[[node]]\npath = \"13-environmental-protection/meteorology-and-air-quality\"\ntitle = \"Meteorology and air quality (atmospheric dispersion)\"\nsources = [{ document = \"nureg-1555\", section = \"2.7\" }, { document = \"nureg-0800-toc-rev6\", section = \"2.3 (2.3.4 short-term, 2.3.5 long-term dispersion)\" }, { document = \"nureg-1537-part1\", section = \"2.3\" }]\n\n[[node]]\npath = \"13-environmental-protection/plant-description\"\ntitle = \"Plant description\"\nsources = [{ document = \"nureg-1555\", section = \"Ch. 3\" }]\n\n[[node]]\npath = \"13-environmental-protection/construction-impacts\"\ntitle = \"Environmental impacts of construction\"\nsources = [{ document = \"nureg-1555\", section = \"Ch. 4\" }]\n\n[[node]]\npath = \"13-environmental-protection/station-operation-impacts\"\ntitle = \"Environmental impacts of station operation\"\nsources = [{ document = \"nureg-1555\", section = \"Ch. 5\" }]\n\n[[node]]\npath = \"13-environmental-protection/radiological-impacts-of-normal-operation\"\ntitle = \"Radiological impacts of normal operation\"\nsources = [{ document = \"nureg-1555\", section = \"5.4\" }]\n\n[[node]]\npath = \"13-environmental-protection/environmental-monitoring\"\ntitle = \"Environmental measurements and monitoring programs\"\nsources = [{ document = \"nureg-1555\", section = \"Ch. 6\" }, { document = \"10cfr50\", section = \"\u{a7} 50.36b Environmental conditions\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1112 Environmental conditions\" }]\n\n[[node]]\npath = \"13-environmental-protection/postulated-accident-impacts\"\ntitle = \"Environmental impacts of postulated accidents\"\nsources = [{ document = \"nureg-1555\", section = \"Ch. 7\" }]\n\n[[node]]\npath = \"13-environmental-protection/alternatives\"\ntitle = \"Alternatives to the proposed action\"\nsources = [{ document = \"nureg-1555\", section = \"Ch. 9\" }]\n\n[[node]]\npath = \"13-environmental-protection/environmental-consequences\"\ntitle = \"Environmental consequences of the proposed action\"\nsources = [{ document = \"nureg-1555\", section = \"Ch. 10\" }]\n\n[[node]]\npath = \"13-environmental-protection/facility-environmental-protection\"\ntitle = \"Environmental protection (fuel cycle facilities)\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"Ch. 9\" }]\n\n[[node]]\npath = \"14-emergency-planning\"\ntitle = \"Emergency planning\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.14\", page = \"50\" }]\n\n[[node]]\npath = \"14-emergency-planning/assignment-of-responsibility\"\ntitle = \"Assignment of responsibility\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard A\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(1)\" }]\n\n[[node]]\npath = \"14-emergency-planning/emergency-response-organization\"\ntitle = \"Emergency response organization\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard B\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(2)\" }]\n\n[[node]]\npath = \"14-emergency-planning/emergency-response-support-and-resources\"\ntitle = \"Emergency response support and resources\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard C\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(3)\" }]\n\n[[node]]\npath = \"14-emergency-planning/emergency-classification-system\"\ntitle = \"Emergency classification system\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard D\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(4)\" }]\n\n[[node]]\npath = \"14-emergency-planning/notification-methods-and-procedures\"\ntitle = \"Notification methods and procedures\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard E\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(5)\" }]\n\n[[node]]\npath = \"14-emergency-planning/emergency-communications\"\ntitle = \"Emergency communications\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard F\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(6)\" }]\n\n[[node]]\npath = \"14-emergency-planning/public-education-and-information\"\ntitle = \"Public education and information\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard G\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(7)\" }]\n\n[[node]]\npath = \"14-emergency-planning/emergency-facilities-and-equipment\"\ntitle = \"Emergency facilities and equipment\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard H\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(8)\" }]\n\n[[node]]\npath = \"14-emergency-planning/accident-assessment\"\ntitle = \"Accident assessment\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard I\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(9)\" }]\n\n[[node]]\npath = \"14-emergency-planning/protective-response\"\ntitle = \"Protective response\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard J\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(10)\" }]\n\n[[node]]\npath = \"14-emergency-planning/radiological-exposure-control\"\ntitle = \"Radiological exposure control\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard K\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(11)\" }]\n\n[[node]]\npath = \"14-emergency-planning/medical-and-public-health-support\"\ntitle = \"Medical and public health support\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard L\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(12)\" }]\n\n[[node]]\npath = \"14-emergency-planning/recovery-reentry-and-post-accident-operations\"\ntitle = \"Recovery, reentry, and post-accident operations\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard M\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(13)\" }]\n\n[[node]]\npath = \"14-emergency-planning/exercises-and-drills\"\ntitle = \"Exercises and drills\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard N\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(14)\" }]\n\n[[node]]\npath = \"14-emergency-planning/radiological-emergency-response-training\"\ntitle = \"Radiological emergency response training\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard O\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(15)\" }]\n\n[[node]]\npath = \"14-emergency-planning/emergency-plan-development-and-review\"\ntitle = \"Responsibility for the planning effort\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"Planning standard P\" }, { document = \"10cfr50\", section = \"\u{a7} 50.47 Emergency plans, (b)(16)\" }, { document = \"10cfr50\", section = \"Appendix E, Emergency Planning and Preparedness for Production and Utilization Facilities\" }, { document = \"10cfr53\", section = \"\u{a7} 53.855 Emergency preparedness\" }]\n\n[[node]]\npath = \"14-emergency-planning/emergency-management\"\ntitle = \"Emergency management (fuel cycle facilities)\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"Ch. 8\" }]\n\n[[node]]\npath = \"15-nuclear-security\"\ntitle = \"Nuclear security\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.15\", page = \"52\" }]\n\n[[node]]\npath = \"15-nuclear-security/cybersecurity\"\ntitle = \"Cybersecurity and information security\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.860 Security programs, (d) Cybersecurity; (e) Information security\" }, { document = \"10cfr50\", section = \"\u{a7} 50.34 Contents of applications; technical information, (c)(2)\" }]\n\n[[node]]\npath = \"15-nuclear-security/physical-protection\"\ntitle = \"Physical protection\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"Ch. 13\" }, { document = \"10cfr53\", section = \"\u{a7} 53.860 Security programs, (a) Physical protection program\" }, { document = \"10cfr50\", section = \"\u{a7} 50.34 Contents of applications; technical information, (c) Physical security plan\" }]\n\n[[node]]\npath = \"16-nuclear-fuel-cycle\"\ntitle = \"Nuclear fuel cycle\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.16\", page = \"54\" }]\n\n# Maintainer 2026-10-06: waste disposal sits in the fuel cycle (back end);\n# topics await 10 CFR Parts 61/63 or IAEA literature.\n[[node]]\npath = \"16-nuclear-fuel-cycle/waste-disposal\"\ntitle = \"Waste disposal (near-surface and geological)\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.16\", page = \"54\" }]\ncross_links = [\"17-radioactive-waste-management\"]\n\n# Maintainer 2026-10-06: kaki-bukit (CYCLUS port) lives here.\n[[node]]\npath = \"16-nuclear-fuel-cycle/fuel-cycle-scenarios\"\ntitle = \"Fuel cycle scenarios and material flows\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.16\", page = \"54\" }]\n\n[[node]]\npath = \"16-nuclear-fuel-cycle/fuel-cycle-facility-general-information\"\ntitle = \"General information (fuel cycle facilities)\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"Ch. 1\" }]\n\n[[node]]\npath = \"16-nuclear-fuel-cycle/integrated-safety-analysis\"\ntitle = \"Integrated safety analysis\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"Ch. 3\" }]\n\n[[node]]\npath = \"16-nuclear-fuel-cycle/nuclear-criticality-safety\"\ntitle = \"Nuclear criticality safety\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"Ch. 5\" }]\n\n[[node]]\npath = \"16-nuclear-fuel-cycle/chemical-process-safety\"\ntitle = \"Chemical process safety\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"Ch. 6\" }]\n\n[[node]]\npath = \"16-nuclear-fuel-cycle/fire-safety\"\ntitle = \"Fire safety\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"Ch. 7\" }]\n\n[[node]]\npath = \"16-nuclear-fuel-cycle/heu-to-leu-conversion\"\ntitle = \"HEU to LEU conversion\"\nsources = [{ document = \"nureg-1537-part1\", section = \"Ch. 18\" }, { document = \"10cfr50\", section = \"\u{a7} 50.64 Limitations on the use of highly enriched uranium (HEU) in domestic non-power reactors\" }]\n\n[[node]]\npath = \"16-nuclear-fuel-cycle/fuel-depletion\"\ntitle = \"Fuel depletion and burnup\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.16\", page = \"54\" }]\n\n[[node]]\npath = \"17-radioactive-waste-management\"\ntitle = \"Radioactive waste management\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.17\", page = \"56\" }]\n\n[[node]]\npath = \"17-radioactive-waste-management/radioactive-waste-management\"\ntitle = \"Radioactive waste management\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 11\" }, { document = \"nureg-1537-part1\", section = \"Ch. 11\" }, { document = \"ornl-tm-2018-976\", section = \"3.5\" }, { document = \"10cfr50\", section = \"\u{a7} 50.34a Design objectives for equipment to control releases of radioactive material in effluents\u{2014}nuclear power reactors\" }, { document = \"10cfr50\", section = \"Appendix F, Policy Relating to the Siting of Fuel Reprocessing Plants and Related Waste Management Facilities\" }]\n\n[[node]]\npath = \"17-radioactive-waste-management/decommissioning\"\ntitle = \"Decommissioning\"\nsources = [{ document = \"nureg-1537-part1\", section = \"Ch. 17\" }, { document = \"nureg-1520-rev2\", section = \"Ch. 10\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1000 Scope and purpose (Subpart G, Decommissioning Requirements)\" }]\n\n[[node]]\npath = \"18-industrial-involvement\"\ntitle = \"Industrial involvement\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.18\", page = \"58\" }]\n\n# Maintainer 2026-10-06: process heat, chemical processes (DWSIM, provisional).\n# World knowledge to add: IAEA NUTEC Plastics (nuclear techniques against plastic\n# pollution) -- literature to be supplied.\n[[node]]\npath = \"18-industrial-involvement/process-heat-and-industrial-applications\"\ntitle = \"Process heat and industrial applications\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.18\", page = \"58\" }]\n\n[[node]]\npath = \"19-procurement\"\ntitle = \"Procurement\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.19\", page = \"60\" }]\n\n[[node]]\npath = \"19-procurement/procurement-capability-and-policy\"\ntitle = \"Procurement capability and policy\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.19.1-3.19.3\", page = \"60\" }]\n\n[[node]]\npath = \"19-procurement/supplier-quality-and-specifications\"\ntitle = \"Supplier quality and specifications\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.19\", page = \"60\" }, { document = \"10cfr50\", section = \"Appendix B, IV. Procurement Document Control; VII. Control of Purchased Material, Equipment, and Services\" }]\ncross_links = [\"07-regulatory-framework/quality-assurance\"]\n\n[[node]]\npath = \"19-procurement/reporting-of-defects-and-noncompliance\"\ntitle = \"Reporting of defects and noncompliance\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.55(e)\" }, { document = \"10cfr53\", section = \"\u{a7} 53.605 Reporting of defects and noncompliance\" }]\ncross_links = [\"18-industrial-involvement\"]\n\n[[node]]\npath = \"19-procurement/emergency-procurement\"\ntitle = \"Emergency procurement (urgent supply; pre-positioned emergency equipment)\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.19.3\", page = \"61\" }]\ncross_links = [\"14-emergency-planning\"]\n";
```

#### Constant `PROPOSALS_TOML`

The concepts file (level 3, plus the level-4 seeds this module ignores),
compiled in.

```rust
pub const PROPOSALS_TOML: &str = "# Kovan concept-tree PROPOSALS: level 3 (concepts) and level 4 (outram-park\n# implementation leaves). OUTRAM PARK GitHub #724, #726, #727, #729.\n#\n# STATUS: every entry here is `status = \"proposed\"`. NOTHING in this file is\n# part of the tree until the maintainer approves it, line by line. Approved\n# concepts move into `concept_skeleton.toml` (or a public-concepts file beside\n# it); rejected ones are deleted here. The readable review copy is\n# `docs/concept-proposals.md`, regenerated from this file by\n# `KOVAN_REGENERATE_PROPOSALS=1 cargo test --release -p kovan-literature --test concept_proposals`\n# (the test fails while the two disagree).\n#\n# [[concept]] (L3; may nest under another proposed concept)\n#   path    full path, under an L2 node of the skeleton (or under another\n#           proposed concept); plain lower-case hyphenated segments\n#   title   the source document\'s own words, verbatim or lightly shortened\n#   origin  \"nrc\"         = a subsection the cited document names itself;\n#           \"outram-park\" = a concept the outram-park code needs that the\n#                           cited text names only implicitly (`why` says why)\n#   sources [{ document, section }], document = a skeleton [[document]] id;\n#           sections read from the document text (2026-10-06), never from memory\n#   note    optional: a judgement call the maintainer should look at\n#\n# [[implementation]] (L4, outram-park only; world leaves are the maintainer\'s)\n#   concept the full path of a proposed concept (or a skeleton node)\n#   crate   a workspace member directory under crates/\n#   module  a file or directory under crates/<crate>/src/, without `.rs`\n#   kind    \"port\" (with `upstream`) or \"new-work\" (`upstream` = what it is\n#           built from, when that is a published method rather than code)\n# L4 will be REGENERATED by kovan from `//! kovan-concept: <L3 path>` lines in\n# each module\'s own doc comment (maintainer, 2026-10-06); these entries are the\n# SEED set of those tags. The test `tests/concept_proposals.rs` checks every\n# crate/module still exists on develop, so a refactor that moves a module\n# fails until this file is updated.\n\n# ============================================================================\n# 02-nuclear-safety / design-of-structures-systems-and-components\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/wind-and-tornado-loadings\"\ntitle = \"Wind and tornado loadings (meteorological damage)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"3.3.1 Wind Loading; 3.3.2 Tornado Loads\" }, { document = \"nureg-1537-part1\", section = \"3.2 Meteorological Damage\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/flood-protection\"\ntitle = \"Flood protection (water damage)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"3.4.1 Internal Flood Protection for Onsite Equipment Failures; 3.4.2 Analysis Procedures\" }, { document = \"nureg-1537-part1\", section = \"3.3 Water Damage\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/missile-protection\"\ntitle = \"Protection against missiles\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"3.5.1.1-3.5.3 (internally generated, turbine, tornado and site-proximity missiles; aircraft hazards; barrier design)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/protection-against-piping-failures\"\ntitle = \"Protection against postulated piping failures\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"3.6.1-3.6.3 (incl. 3.6.3 Leak-Before-Break Evaluation Procedures)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/seismic-design\"\ntitle = \"Seismic design (parameters, system and subsystem analysis, instrumentation)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"3.7.1-3.7.4\" }, { document = \"nureg-1537-part1\", section = \"3.4 Seismic Damage\" }, { document = \"10cfr50\", section = \"Appendix S, Earthquake Engineering Criteria for Nuclear Power Plants\" }, { document = \"10cfr53\", section = \"\u{a7} 53.480 Earthquake engineering\" }, { document = \"10cfr53\", section = \"\u{a7} 53.720 Response to seismic events\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/seismic-category-i-structures\"\ntitle = \"Containment and other Seismic Category I structures and foundations\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"3.8.1-3.8.5\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/mechanical-systems-and-components\"\ntitle = \"Mechanical systems and components\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"3.9.1 Special Topics for Mechanical Components; 3.9.2-3.9.8; 3.12; 3.13\" }, { document = \"nureg-1537-part1\", section = \"3.5 Systems and Components\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/equipment-qualification\"\ntitle = \"Seismic, dynamic and environmental qualification of equipment\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"3.10; 3.11\" }, { document = \"10cfr50\", section = \"\u{a7} 50.49 Environmental qualification of electric equipment important to safety for nuclear power plants\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / fuel-system-design   (SRP 4.2 Rev. 3 review areas)\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/fuel-system-damage\"\ntitle = \"Fuel system damage\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"II.1.A Fuel System Damage\" }, { document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 10 Reactor design\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/fuel-system-damage/oxidation-hydriding-and-crud\"\ntitle = \"Oxidation, hydriding, and the buildup of corrosion products (crud)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"II.1.A.iv\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/fuel-system-damage/rod-internal-gas-pressure\"\ntitle = \"Fuel and burnable poison rod internal gas pressures\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"II.1.A.vi; II.3.C.vii Fuel Rod Pressure\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/fuel-rod-failure\"\ntitle = \"Fuel rod failure\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"II.1.B Fuel Rod Failure (i hydriding ... viii mechanical fracturing)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/fuel-rod-failure/pellet-cladding-interaction\"\ntitle = \"Pellet/cladding interaction\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"II.1.B.vi\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/fuel-coolability\"\ntitle = \"Fuel coolability\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"II.1.C Fuel Coolability (cladding embrittlement, violent expulsion, melting, ballooning, structural deformation)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/fuel-performance-analytical-predictions\"\ntitle = \"Analytical predictions of fuel performance\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"II.3.C Analytical Predictions\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/fuel-performance-analytical-predictions/fuel-temperatures-and-stored-energy\"\ntitle = \"Fuel temperatures (stored energy)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"II.3.C.i\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/testing-inspection-and-surveillance\"\ntitle = \"Testing, inspection, and surveillance plans\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"I.4; II.4 (new fuel, online monitoring, postirradiation surveillance)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/reactivity-initiated-accident-criteria\"\ntitle = \"Acceptance criteria for reactivity-initiated accidents\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"Appendix B, Interim Acceptance Criteria and Guidance for the Reactivity Initiated Accidents\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/fuel-assembly-structural-response\"\ntitle = \"Fuel assembly structural response to externally applied forces\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"Appendix A\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/fuel-salt-chemistry\"\ntitle = \"Fuel salt chemistry (MSR)\"\norigin = \"nrc\"\nsources = [{ document = \"ornl-tm-2018-976\", section = \"3.1.1 Section 4.2, Fuel System Design\" }]\nnote = \"ORNL, not NRC; origin \'nrc\' here means \'named by the cited regulatory text\'.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel\"\ntitle = \"TRISO coated-particle fuel as the primary fission-product barrier\"\norigin = \"nrc\"\nsources = [{ document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 10 Reactor design (rationale: TRISO is the primary fission product barrier; SARRDLs)\" }]\ncross_links = [\"02-nuclear-safety/containment/functional-containment\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-failure\"\ntitle = \"TRISO coated-particle failure (pressure-vessel failure under accident conditions)\"\norigin = \"outram-park\"\nsources = [{ document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 10 rationale\" }]\nwhy = \"boon-lay computes particle failure fractions; the RG names TRISO retention, not the failure mechanisms.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release\"\ntitle = \"Fission-product diffusion and release from TRISO particles\"\norigin = \"outram-park\"\nsources = [{ document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 10 rationale; MHTGR-DC 16\" }]\nwhy = \"boon-lay\'s TRISO-ATOPS fork is the release model behind every outram-park HTGR source term.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-conduction\"\ntitle = \"Heat conduction through the TRISO particle layers\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"II.3.C.i (fuel and cladding temperature distribution)\" }]\nwhy = \"the innermost of the pebble-bed conduction scales; SRP 4.2\'s temperature distribution in TRISO geometry.\"\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / nuclear-design\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/core-configuration-and-criticality\"\ntitle = \"Normal operating conditions: core configurations and criticality physics\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.1 Normal Operating Conditions\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/core-configurations/pebble-bed/pebble-bed-packing\"\ntitle = \"Pebble-bed packing (core geometry of a pebble-bed reactor)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.1 (core geometry and configurations)\" }]\nwhy = \"the packing fraction and pebble positions are the core configuration of HTR-10 (gh #216); DEM and packing algorithms produce it.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/power-distribution\"\ntitle = \"Power distribution (axial and radial neutron flux densities, peaking)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.2 Reactor Core Physics Parameters\" }, { document = \"ornl-tm-2018-976\", section = \"3.1.2 (SRP 4.3 \'focuses on the core power distribution and reactivity coefficients\')\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/reactivity-coefficients\"\ntitle = \"Coefficients of reactivity (fuel and moderator temperature, void, power)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.2\" }, { document = \"ornl-tm-2018-976\", section = \"3.1.2\" }, { document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 11 Reactor inherent protection\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/kinetics-parameters\"\ntitle = \"Neutron lifetime and effective delayed neutron fraction\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.2\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/reactor-kinetic-behaviour\"\ntitle = \"Kinetic behaviour of the reactor for steady-state and transient operation\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 Nuclear Design (introduction); 4.5.1 (analyses of the reactor kinetic behavior)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/reactivity-worths\"\ntitle = \"Reactivity worths of fuel, reflector, experimental components and control rods\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.1\" }, { document = \"ornl-tm-2018-976\", section = \"3.1.2 (control rods, control rod patterns, and reactivity worths)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/burnup-and-poison-reactivity-effects\"\ntitle = \"Changes in core reactivity with fuel burnup, plutonium buildup, and poisons\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.1\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/excess-reactivity-and-shutdown-margin\"\ntitle = \"Operating limits: excess reactivity and shutdown margin\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.3 Operating Limits\" }, { document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 26 Reactivity control systems; MHTGR-DC 28 Reactivity limits\" }, { document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (g)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/suppression-of-power-oscillations\"\ntitle = \"Suppression of reactor power oscillations\"\norigin = \"nrc\"\nsources = [{ document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 12\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/accuracy-of-analytical-methods\"\ntitle = \"Estimates of the accuracy of the analytical methods (uncertainty and sensitivity)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 Nuclear Design (introduction)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/core-configurations/liquid-fuelled/delayed-neutron-precursor-drift\"\ntitle = \"Delayed-neutron precursor drift in circulating fuel (MSR)\"\norigin = \"outram-park\"\nsources = [{ document = \"ornl-tm-2018-976\", section = \"3.1.2 (MSRs may use ... intrinsic nuclear phenomena, such as flow, to provide reactor control)\" }]\nwhy = \"the MSRE reactivity effect; moltres and GeN-Foam\'s precursor_drift model it.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/neutron-transport\"\ntitle = \"Neutron transport methods\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 Nuclear Design (introduction: a detailed description of the analytical methods ... computer codes)\" }]\nwhy = \"the NRC asks for the analytical methods but does not name them; outram-park\'s transport solvers live here.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/neutron-transport/surface-tracking\"\ntitle = \"Surface tracking through constructive solid geometry\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 (introduction)\" }]\nwhy = \"OpenMC\'s tracking method; the CSG description and its navigation kernel.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/neutron-transport/delta-tracking\"\ntitle = \"Delta (Woodcock) tracking\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 (introduction)\" }]\nwhy = \"NEW WORK in outram-mc (OpenMC has none); the tracking that makes doubly heterogeneous pebble beds affordable.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/neutron-transport/delta-tracking/majorant\"\ntitle = \"The majorant cross section\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 (introduction)\" }]\nwhy = \"the bound every delta-tracking flight is sampled on; a correctness invariant of its own.\"\nnote = \"Nested under delta-tracking rather than a sibling of it, since it exists only for delta tracking.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/neutron-transport/doubly-heterogeneous-geometry\"\ntitle = \"Doubly heterogeneous geometry (TRISO particles in pebbles, stochastic media)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.1 (interacting effects of fuel, neutron moderators and reflectors)\" }]\nwhy = \"pebble-bed specialisation of outram-mc: explicit, RPT and stochastic (CLS/SCLS) treatments.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/neutron-transport/continuous-energy-collision-physics\"\ntitle = \"Continuous-energy collision physics (scattering, fission)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 (introduction)\" }]\nwhy = \"the per-collision kernels of Monte Carlo transport, ported from OpenMC.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/neutron-transport/eigenvalue-and-fixed-source-calculations\"\ntitle = \"k-eigenvalue (power iteration) and fixed-source calculations\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.1 (calculated core reactivities for all core configurations)\" }]\nwhy = \"the run modes that produce k_eff and source-driven flux.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/neutron-transport/variance-reduction\"\ntitle = \"Variance reduction (weight windows, uniform fission sites)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 (estimates of the accuracy of the analytical methods)\" }]\nwhy = \"Monte Carlo efficiency techniques ported from OpenMC.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/neutron-transport/monte-carlo-tallies\"\ntitle = \"Monte Carlo tallies (structured and unstructured mesh, filters, triggers)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.2 (axial and radial distributions of neutron flux densities)\" }]\nwhy = \"how fluxes and reaction rates are estimated from histories.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/neutron-transport/deterministic-neutronics\"\ntitle = \"Deterministic neutronics (multigroup diffusion, SP3, discrete ordinates, nodal)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 (introduction)\" }]\nwhy = \"the GeN-Foam, Moltres and BEDOK solver family.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/neutron-transport/multigroup-cross-section-condensation\"\ntitle = \"Multigroup cross sections condensed from Monte Carlo\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 (methods of obtaining parameters such as cross sections)\" }]\nwhy = \"the bridge from stochastic to deterministic transport (nee_soon).\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/nuclear-data-processing\"\ntitle = \"Nuclear data processing (evaluated data to transport libraries)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 (introduction: methods of obtaining parameters such as cross sections)\" }]\nwhy = \"the NJOY2016 port; the NRC names cross sections as a method input only.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/resonance-reconstruction\"\ntitle = \"Resonance reconstruction of pointwise cross sections\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 (introduction)\" }]\nwhy = \"NJOY RECONR and the SAMM R-matrix kernel.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/doppler-broadening\"\ntitle = \"Doppler broadening of cross sections\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.2 (fuel temperature coefficient)\" }]\nwhy = \"the temperature dependence behind the fuel temperature coefficient: NJOY BROADR and windowed multipole.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/unresolved-resonance-probability-tables\"\ntitle = \"Unresolved-resonance self-shielding and probability tables\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 (introduction)\" }]\nwhy = \"NJOY UNRESR/PURR; on by default in outram-mc since 2026-09-20 (root CLAUDE.md).\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/thermal-scattering\"\ntitle = \"Thermal neutron scattering, S(alpha, beta)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.2.3 Neutron Moderator and Reflector; 4.5\" }]\nwhy = \"bound-atom scattering in graphite and other moderators: NJOY LEAPR/THERMR and its use in transport.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/ace-library-generation\"\ntitle = \"Continuous-energy (ACE) library generation\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 (introduction)\" }]\nwhy = \"NJOY ACER, the hand-off from processing to Monte Carlo transport.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/multigroup-data-generation\"\ntitle = \"Multigroup cross sections and transfer matrices from evaluated data\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 (introduction)\" }]\nwhy = \"NJOY GROUPR/GAMINR.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/nuclear-data-covariances\"\ntitle = \"Nuclear-data covariances\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5 (estimates of the accuracy of the analytical methods)\" }]\nwhy = \"NJOY ERRORR/COVR: the data half of the accuracy estimate.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/heating-and-damage\"\ntitle = \"Heating (KERMA) and damage cross sections\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6 (thermal power density distribution)\" }]\nwhy = \"NJOY HEATR: where deposited power comes from.\"\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / moderator-and-reflector   (NUREG-1537 4.2.3)\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/moderator-and-reflector/moderator-and-reflector-nuclear-design\"\ntitle = \"Nuclear design of the moderator and reflector\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.2.3 (the nuclear design of the moderator and reflector should be discussed in Section 4.5)\" }]\ncross_links = [\"02-nuclear-safety/nuclear-design/nuclear-data-processing/thermal-scattering\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/moderator-and-reflector/cooling-radiation-damage-and-encapsulation\"\ntitle = \"Provisions for cooling, radiation damage, and failure of encapsulation\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.2.3\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / thermal-hydraulic-design   (SRP 4.4, NUREG-1537 4.6)\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/core-coolant-hydraulics\"\ntitle = \"Coolant hydraulic characteristics of the core (flow rates, pressures, frictional and buoyant forces)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6 Thermal-Hydraulic Design, first item\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/power-density-and-heat-flux-distribution\"\ntitle = \"Thermal power density distribution and heat fluxes into the coolant\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6, second item\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/heat-transfer-to-coolant\"\ntitle = \"Transfer of heat to the coolant (thermal-hydraulic methodology)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6, third item\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/fuel-heat-removal-limits\"\ntitle = \"Fuel heat-removal limits (onset of nucleate boiling, departure from nucleate boiling, flow instability)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6, fifth item\" }, { document = \"nureg-0800-4.2-rev3\", section = \"I (SRP 4.4 provides DNBR and CPR criteria)\" }]\nnote = \"LWR mechanisms (ONB, DNB, CHF, CPR): not applicable to MSR technology (ORNL/TM-2018/976 3.1.3); see gas-generation-and-entrainment.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-stability\"\ntitle = \"Susceptibility to thermal-hydraulic instability\"\norigin = \"nrc\"\nsources = [{ document = \"ornl-tm-2018-976\", section = \"3.1.3 (quoting SRP 4.4: \'(4) is not susceptible to thermal-hydraulic instability\')\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/natural-convection-cooling\"\ntitle = \"Natural-convection cooling (forced-to-natural transition; decay-heat removal)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6, first and sixth items\" }, { document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 34 Passive residual heat removal\" }]\nnote = \"The brief proposed \'natural-circulation\' as outram-park origin; NUREG-1537 4.6 names it, so it is marked nrc.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/shutdown-decay-heat\"\ntitle = \"Shutdown decay heat\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6, fourth item (operating conditions should include ... shutdown decay heat)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/pulsing-reactor-analysis\"\ntitle = \"Pulse analysis (feedback coefficients and thermal-hydraulic evolution during a pulse)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6, last item\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics\"\ntitle = \"Computational fluid dynamics (finite volume)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6 (a detailed description of the analytical methods used in the thermal-hydraulic design)\" }]\nwhy = \"the OpenFOAM port (outram-foam-*); the NRC asks for methods without naming them.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics/mesh-generation\"\ntitle = \"Mesh generation for finite-volume and finite-element solvers\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6 (introduction)\" }]\nwhy = \"blockMesh/snappyHexMesh/cfMesh ports and the neutral unstructured mesh (#492).\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/system-thermal-hydraulics\"\ntitle = \"System (1-D network) thermal hydraulics\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6 (introduction)\" }, { document = \"ornl-tm-2018-976\", section = \"3.1.3 (focus on acceptable analytical methods)\" }]\nwhy = \"TUAS and TAMPINES control-volume networks; the peer-reviewed TUAS work sits here.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/two-phase-flow\"\ntitle = \"Two-phase flow and boiling\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6, fifth item\" }]\nwhy = \"two-fluid, drift-flux and homogeneous models behind the boiling limits.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/coolant-thermophysical-properties\"\ntitle = \"Coolant and structure thermophysical properties\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6, third item (uncertainties in thermal-hydraulic ... parameters)\" }]\nwhy = \"IAPWS-IF97, CoolProp\'s Helmholtz EOS, and TUAS\'s salt and solid property library.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/pebble-bed-thermal-hydraulics\"\ntitle = \"Pebble-bed thermal hydraulics (packed-bed pressure drop, effective conductivity)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6, first and third items\" }]\nwhy = \"KTA 3102 correlations, ZBS conductivity and contact conduction in the bed.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/neutronics-thermal-hydraulics-coupling\"\ntitle = \"Neutronics and thermal-hydraulics coupling\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6, second item (heat fluxes derived from the fuel loading and neutron flux characteristics)\" }]\nwhy = \"multiphysics coupling in GeN-Foam, nee_soon and BEDOK.\"\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / control-rods-and-drives\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/control-rods-and-drives/control-rods\"\ntitle = \"Control rods\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.2.2 Control Rods\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/control-rods-and-drives/control-rod-drive-structural-materials\"\ntitle = \"Control rod drive structural materials\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"4.5.1\" }, { document = \"ornl-tm-2018-976\", section = \"3.1.4\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/control-rods-and-drives/functional-design-of-control-rod-drive-system\"\ntitle = \"Functional design of control rod drive system\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"4.6\" }, { document = \"ornl-tm-2018-976\", section = \"3.1.6\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / reactor-coolant-system\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/reactor-coolant-system/reactor-coolant-pressure-boundary\"\ntitle = \"Reactor coolant (helium) pressure boundary\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"5.2.1.1-5.2.5\" }, { document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 14, 15, 30-32\" }, { document = \"ornl-tm-2018-976\", section = \"3.2.1\" }]\ncross_links = [\"02-nuclear-safety/accident-analysis/air-and-moisture-ingress\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/reactor-coolant-system/reactor-vessel-integrity\"\ntitle = \"Reactor vessel materials and integrity\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"5.3.1-5.3.3\" }, { document = \"ornl-tm-2018-976\", section = \"3.2.2\" }, { document = \"10cfr50\", section = \"\u{a7} 50.60 Acceptance criteria for fracture prevention measures for lightwater nuclear power reactors for normal operation\" }, { document = \"10cfr50\", section = \"\u{a7} 50.66 Requirements for thermal annealing of the reactor pressure vessel\" }, { document = \"10cfr50\", section = \"Appendix G, Fracture Toughness Requirements\" }, { document = \"10cfr50\", section = \"Appendix H, Reactor Vessel Material Surveillance Program Requirements\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/reactor-coolant-system/residual-heat-removal\"\ntitle = \"Residual heat removal (passive, in the MHTGR criteria)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"5.4.7 Residual Heat Removal (RHR) System\" }, { document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 34, 36, 37\" }, { document = \"ornl-tm-2018-976\", section = \"3.2.3\" }, { document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (h)\" }]\ncross_links = [\"02-nuclear-safety/thermal-hydraulic-design/natural-convection-cooling\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/reactor-coolant-system/steam-generators\"\ntitle = \"Steam generators\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"5.4.2.1 Steam Generator Materials; 5.4.2.2 Steam Generator Program\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/reactor-coolant-system/primary-heat-exchangers\"\ntitle = \"Primary heat exchanger\"\norigin = \"nrc\"\nsources = [{ document = \"ornl-tm-2018-976\", section = \"3.2.3 (proposed MSR subsection)\" }, { document = \"nureg-1537-part1\", section = \"5.2 Primary Coolant System; 5.3 Secondary Coolant System\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/reactor-coolant-system/coolant-pumps-and-circulators\"\ntitle = \"Reactor coolant pumps (fuel salt pump; gas circulator)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"5.4.1.1 Pump Flywheel Integrity (PWR)\" }, { document = \"ornl-tm-2018-976\", section = \"3.2.3 (fuel salt pump)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/reactor-coolant-system/primary-coolant-cleanup-and-makeup\"\ntitle = \"Primary coolant cleanup and makeup water systems\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"5.4; 5.5\" }, { document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 33 Reactor coolant makeup\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/reactor-coolant-system/nitrogen-16-control\"\ntitle = \"Nitrogen-16 control system\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"5.6\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/reactor-coolant-system/fuel-salt-drain-tank-and-freeze-valve\"\ntitle = \"Fuel salt drain tank and freeze valve boundary (MSR)\"\norigin = \"nrc\"\nsources = [{ document = \"ornl-tm-2018-976\", section = \"3.2.3 (proposed 5.4.20, 5.4.21)\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / msr-coolant-loop   (ORNL 3.2.4, proposed SRP 5.5)\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/msr-coolant-loop/coolant-loop-materials-and-chemistry\"\ntitle = \"Coolant loop materials and chemistry\"\norigin = \"nrc\"\nsources = [{ document = \"ornl-tm-2018-976\", section = \"3.2.4 (coolant loop materials; coolant loop chemistry)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/msr-coolant-loop/coolant-loop-cleanup-sampling-and-makeup\"\ntitle = \"Coolant loop cleanup, sampling and makeup systems\"\norigin = \"nrc\"\nsources = [{ document = \"ornl-tm-2018-976\", section = \"3.2.4\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/msr-coolant-loop/coolant-loop-pump-and-heat-exchangers\"\ntitle = \"Coolant loop pump and coolant heat exchangers\"\norigin = \"nrc\"\nsources = [{ document = \"ornl-tm-2018-976\", section = \"3.2.4\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/msr-coolant-loop/coolant-loop-drain-tank-and-isolation\"\ntitle = \"Coolant loop drain tank, in-service inspection and testing, isolation valves\"\norigin = \"nrc\"\nsources = [{ document = \"ornl-tm-2018-976\", section = \"3.2.4\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / engineered-safety-features\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/engineered-safety-features/emergency-core-cooling\"\ntitle = \"Emergency core cooling system\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"6.3\" }, { document = \"nureg-1537-part1\", section = \"6.2.3\" }, { document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 35\" }, { document = \"10cfr50\", section = \"\u{a7} 50.46 Acceptance criteria for emergency core cooling systems for light-water nuclear power reactors\" }, { document = \"10cfr50\", section = \"Appendix K, ECCS Evaluation Models\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/engineered-safety-features/confinement\"\ntitle = \"Confinement\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"6.2.1 Confinement\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/engineered-safety-features/control-room-habitability\"\ntitle = \"Control room habitability system\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"6.4\" }, { document = \"ornl-tm-2018-976\", section = \"3.3.4\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/engineered-safety-features/fission-product-cleanup-systems\"\ntitle = \"ESF atmosphere cleanup and fission product control systems\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"6.5.1-6.5.5\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/engineered-safety-features/esf-materials\"\ntitle = \"Engineered safety features materials\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"6.1.1; 6.1.2\" }, { document = \"ornl-tm-2018-976\", section = \"3.3.1\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / containment\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/containment/functional-containment\"\ntitle = \"Functional containment\"\norigin = \"nrc\"\nsources = [{ document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 16 Containment design (and its rationale)\" }, { document = \"ornl-tm-2018-976\", section = \"3.3.2\" }]\nnote = \"The brief suggested functional containment under fuel-system-design; the skeleton\'s containment node names it, so it is placed here (TRISO retention stays under fuel-system-design).\"\ncross_links = [\"02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/containment/containment-functional-design\"\ntitle = \"Containment functional design (incl. mass and energy release analyses)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"6.2.1-6.2.1.5\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/containment/containment-heat-removal\"\ntitle = \"Containment heat removal systems\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"6.2.2\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/containment/containment-isolation-and-leakage-testing\"\ntitle = \"Containment isolation and leakage testing\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"6.2.4; 6.2.6\" }, { document = \"10cfr50\", section = \"Appendix J, Primary Reactor Containment Leakage Testing for Water-Cooled Power Reactors\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/containment/combustible-gas-control\"\ntitle = \"Combustible gas control in containment\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"6.2.5\" }, { document = \"10cfr50\", section = \"\u{a7} 50.44 Combustible gas control for nuclear power reactors\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / instrumentation-and-control\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/instrumentation-and-control/reactor-protection-system\"\ntitle = \"Reactor trip (protection) system\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"7.2 Reactor Trip System\" }, { document = \"nureg-1537-part1\", section = \"7.4 Reactor Protection System\" }, { document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 20-25\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/instrumentation-and-control/engineered-safety-features-actuation\"\ntitle = \"Engineered safety features (actuation) systems\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"7.3\" }, { document = \"nureg-1537-part1\", section = \"7.5\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/instrumentation-and-control/control-systems\"\ntitle = \"Control systems\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"7.7 Control Systems\" }, { document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 13 Instrumentation and control\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/instrumentation-and-control/information-systems-and-displays\"\ntitle = \"Information systems important to safety; control console and display instruments\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"7.5\" }, { document = \"nureg-1537-part1\", section = \"7.6\" }, { document = \"10cfr53\", section = \"\u{a7} 53.730 Defining, fulfilling, and maintaining the role of personnel in ensuring safe operations, (b) Human system interface design requirements\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/instrumentation-and-control/radiation-monitoring-systems\"\ntitle = \"Radiation monitoring systems\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"7.7\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/instrumentation-and-control/digital-i-and-c-software\"\ntitle = \"Digital instrumentation and control: software reviews\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"7.0-A; BTP 7-14 Guidance on Software Reviews for Digital Computer-Based Instrumentation and Controls Systems\" }]\ncross_links = [\"07-regulatory-framework/quality-assurance/software-quality-assurance\"]\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / auxiliary-systems\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/auxiliary-systems/fuel-storage-and-handling\"\ntitle = \"Fuel storage and handling (incl. criticality safety of fresh and spent fuel)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"9.1.1-9.1.5\" }, { document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 61, 62\" }, { document = \"ornl-tm-2018-976\", section = \"3.4.1\" }, { document = \"10cfr50\", section = \"\u{a7} 50.68 Criticality accident requirements, (b)\" }, { document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (m)(2)\" }]\ncross_links = [\"16-nuclear-fuel-cycle/nuclear-criticality-safety\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/auxiliary-systems/cooling-water-and-ultimate-heat-sink\"\ntitle = \"Cooling fluids and ultimate heat sink\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"9.2.1-9.2.6 (9.2.5 Ultimate Heat Sink)\" }, { document = \"ornl-tm-2018-976\", section = \"3.4.2 (incl. fuel salt drain tank cooling system)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/auxiliary-systems/ventilation-systems\"\ntitle = \"Heating, ventilation, and air conditioning systems\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"9.4.1-9.4.5\" }, { document = \"nureg-1537-part1\", section = \"9.1\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/auxiliary-systems/fire-protection\"\ntitle = \"Fire protection program\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"9.5.1\" }, { document = \"10cfr50\", section = \"\u{a7} 50.48 Fire protection\" }, { document = \"10cfr50\", section = \"Appendix R, Fire Protection Program for Nuclear Power Facilities Operating Prior to January 1, 1979\" }, { document = \"10cfr53\", section = \"\u{a7} 53.875 Fire protection\" }, { document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (e)\" }, { document = \"10cfr53\", section = \"\u{a7} 53.450 Analysis requirements, (g)(1) Fire protection\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/auxiliary-systems/cover-gas-control\"\ntitle = \"Cover gas control in closed primary coolant systems\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"9.6\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / steam-and-power-conversion   (SRP Ch. 10)\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/steam-and-power-conversion/turbine-generator\"\ntitle = \"Turbine generator\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"10.2; 10.2.3\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/steam-and-power-conversion/main-steam-supply\"\ntitle = \"Main steam supply system\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"10.3; 10.3.6\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/steam-and-power-conversion/main-condensers\"\ntitle = \"Main condensers\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"10.4.1; 10.4.2\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/steam-and-power-conversion/circulating-water-system\"\ntitle = \"Circulating water system\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"10.4.5\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/steam-and-power-conversion/condensate-and-feedwater\"\ntitle = \"Condensate and feedwater system\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"10.4.6; 10.4.7; 10.4.9\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/steam-and-power-conversion/power-cycle-flowsheet-simulation\"\ntitle = \"Power-conversion cycle and flowsheet simulation\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 10 Steam and Power Conversion System\" }]\nwhy = \"the Rankine cycle and the DWSIM equipment models (heat exchangers, compressors, expanders) assemble the Ch. 10 systems.\"\ncross_links = [\"18-industrial-involvement/process-heat-and-industrial-applications\"]\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / conduct-of-operations, initial-test-program, technical-specifications\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/conduct-of-operations/operator-training-and-requalification\"\ntitle = \"Operator training and requalification\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"13.2.1; 13.2.2\" }, { document = \"nureg-1537-part1\", section = \"12.10\" }, { document = \"10cfr50\", section = \"\u{a7} 50.120 Training and qualification of nuclear power plant personnel\" }, { document = \"10cfr53\", section = \"\u{a7} 53.745 Operator license requirements\" }, { document = \"10cfr53\", section = \"\u{a7} 53.760 Operator licensing\" }, { document = \"10cfr53\", section = \"\u{a7} 53.780 Training, examination, and proficiency program\" }, { document = \"10cfr53\", section = \"\u{a7} 53.830 Training and qualification of commercial nuclear personnel\" }]\ncross_links = [\"10-human-resource-development/knowledge-management-and-education\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/conduct-of-operations/procedures\"\ntitle = \"Administrative, operating and emergency operating procedures\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"13.5.1.1; 13.5.2.1\" }, { document = \"nureg-1537-part1\", section = \"12.3\" }, { document = \"10cfr53\", section = \"\u{a7} 53.910 Procedures and guidelines\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/conduct-of-operations/review-and-audit\"\ntitle = \"Review and audit activities\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"12.2\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/conduct-of-operations/startup-plan\"\ntitle = \"Startup plan\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"12.11\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/initial-test-program/initial-plant-test-program\"\ntitle = \"Initial plant test program\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"14.2; 14.2.1\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/initial-test-program/itaac\"\ntitle = \"Inspections, tests, analyses, and acceptance criteria\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"14.3-14.3.12\" }, { document = \"10cfr52\", section = \"\u{a7} 52.99 Inspection during construction; ITAAC schedules and notifications; NRC notices\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1449 Inspection during construction\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/technical-specifications/safety-limits-and-limiting-safety-system-settings\"\ntitle = \"Safety limits and limiting safety system settings\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"Ch. 14 appendix, 2.1 Safety Limits; 2.2 Limiting Safety System Settings\" }, { document = \"10cfr50\", section = \"\u{a7} 50.36 Technical specifications, (c)(1)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/technical-specifications/limiting-conditions-for-operation\"\ntitle = \"Limiting conditions for operation and surveillance requirements\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"Ch. 14 appendix, 3.1-3.9 and 4.x\" }, { document = \"nureg-0800-toc-rev6\", section = \"16.0\" }, { document = \"10cfr50\", section = \"\u{a7} 50.36 Technical specifications, (c)(2)-(3)\" }, { document = \"10cfr53\", section = \"\u{a7} 53.710 Maintaining capabilities and availability of structures, systems, and components, (a)(3)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/technical-specifications/risk-informed-technical-specifications\"\ntitle = \"Risk-informed decision making: technical specifications\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"16.1\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / accident-analysis   (SRP Ch. 15 event categories; NUREG-1537 Ch. 13)\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/transient-and-accident-analysis-methods\"\ntitle = \"Review of transient and accident analysis methods\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"15.0.2\" }, { document = \"nureg-1537-part1\", section = \"13.2 Accident Analysis and Determination of Consequences\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/increase-in-heat-removal\"\ntitle = \"Increase in heat removal by the secondary system\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"15.1.1-15.1.5 (feedwater temperature and flow, steam flow, relief valve opening, steam system piping failures)\" }]\nnote = \"The SRP ToC lists the 15.1-15.9 events but prints no category titles; the category titles here are composed from the listed events and should be checked against SRP 15.0. Category title composed from the listed SRP events; to verify against SRP 15.0 (not in the corpus).\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/decrease-in-heat-removal\"\ntitle = \"Decrease in heat removal by the secondary system (incl. loss of normal AC power)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"15.2.1-15.2.8\" }, { document = \"nureg-1537-part1\", section = \"13.1.7 Loss of Normal Electrical Power\" }]\nnote = \"Category title composed from the listed SRP events; to verify against SRP 15.0 (not in the corpus).\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/decrease-in-reactor-coolant-flow\"\ntitle = \"Decrease in reactor coolant flow\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"15.3.1-15.3.4\" }, { document = \"nureg-1537-part1\", section = \"13.1.4 Loss of Coolant Flow\" }]\nnote = \"Category title composed from the listed SRP events; to verify against SRP 15.0 (not in the corpus).\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/reactivity-and-power-distribution-anomalies\"\ntitle = \"Reactivity and power distribution anomalies (rod withdrawal, ejection, drop; insertion of excess reactivity)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"15.4.1-15.4.9\" }, { document = \"nureg-1537-part1\", section = \"13.1.2 Insertion of Excess Reactivity\" }]\nnote = \"Category title composed from the listed SRP events; to verify against SRP 15.0 (not in the corpus).\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/increase-in-reactor-coolant-inventory\"\ntitle = \"Increase in reactor coolant inventory\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"15.5.1-15.5.2\" }]\nnote = \"Category title composed from the listed SRP events; to verify against SRP 15.0 (not in the corpus).\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/decrease-in-reactor-coolant-inventory\"\ntitle = \"Decrease in reactor coolant inventory (loss of coolant accidents)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"15.6.1-15.6.5\" }, { document = \"nureg-1537-part1\", section = \"13.1.3 Loss of Coolant\" }]\nnote = \"Category title composed from the listed SRP events; to verify against SRP 15.0 (not in the corpus).\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/releases-from-subsystems-and-fuel-handling\"\ntitle = \"Radioactive release from a subsystem or component; mishandling or malfunction of fuel\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"15.7.3-15.7.5\" }, { document = \"nureg-1537-part1\", section = \"13.1.5 Mishandling or Malfunction of Fuel\" }]\nnote = \"Category title composed from the listed SRP events; to verify against SRP 15.0 (not in the corpus).\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/anticipated-transients-without-scram\"\ntitle = \"Anticipated transients without scram\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"15.8\" }, { document = \"10cfr50\", section = \"\u{a7} 50.62 Requirements for reduction of risk from anticipated transients without scram (ATWS) events for light-water-cooled nuclear power plants\" }]\nnote = \"Category title composed from the listed SRP events; to verify against SRP 15.0 (not in the corpus).\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/maximum-hypothetical-accident\"\ntitle = \"Maximum hypothetical accident\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"13.1.1\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/external-events-and-experiment-malfunction\"\ntitle = \"External events; experiment malfunction; mishandling or malfunction of equipment\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"13.1.6; 13.1.8; 13.1.9\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 02-nuclear-safety / severe-accidents, source-terms\n# ============================================================================\n\n[[concept]]\npath = \"02-nuclear-safety/severe-accidents/probabilistic-risk-assessment\"\ntitle = \"Probabilistic risk assessment (technical adequacy; risk-informed changes)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"19.0; 19.1; 19.2\" }, { document = \"10cfr53\", section = \"\u{a7} 53.450 Analysis requirements, (a)-(c)\" }, { document = \"10cfr50\", section = \"\u{a7} 50.71 Maintenance of records, making of reports, (h)\" }, { document = \"10cfr52\", section = \"\u{a7} 52.47 Contents of applications; technical information, (a)(27)\" }, { document = \"10cfr52\", section = \"\u{a7} 52.79 Contents of applications; technical information in final safety analysis report, (a)(46)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/severe-accidents/severe-accident-evaluation\"\ntitle = \"Severe accident evaluation\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"19.0\" }, { document = \"nureg-1555\", section = \"7.2 Severe Accidents; 7.3 Severe Accident Mitigation Alternatives\" }, { document = \"10cfr52\", section = \"\u{a7} 52.47 Contents of applications; technical information, (a)(23)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/source-terms/coolant-source-terms\"\ntitle = \"Coolant source terms (normal operation and anticipated operational occurrences)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"11.1 Source Terms\" }, { document = \"ornl-tm-2018-976\", section = \"3.5.1 Section 11.1, Coolant Source Terms\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/source-terms/accident-source-terms\"\ntitle = \"Accident source terms and radiological consequence analyses\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"15.0.1 Radiological Consequence Analyses Using Alternate Source Terms; 15.0.3\" }, { document = \"nureg-1555\", section = \"7.1 Design Basis Accidents\" }, { document = \"10cfr50\", section = \"\u{a7} 50.67 Accident source term\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/source-terms/fission-product-inventory\"\ntitle = \"Fission product inventory (gap inventory as a release fraction)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"I (gap inventory); II.3.C.ix Fission Product Inventory; Appendix B, D. Fission Product Inventory\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 07-regulatory-framework / quality-assurance   (SRP Ch. 17)\n# ============================================================================\n\n[[concept]]\npath = \"07-regulatory-framework/quality-assurance/quality-assurance-program\"\ntitle = \"Quality assurance during design, construction and operations; QA program description\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"Appendix B, Quality Assurance Criteria for Nuclear Power Plants and Fuel Reprocessing Plants\" }, { document = \"nureg-0800-toc-rev6\", section = \"17.1; 17.2; 17.3; 17.5\" }, { document = \"nureg-1537-part1\", section = \"12.9 Quality Assurance\" }, { document = \"10cfr53\", section = \"\u{a7} 53.865 Quality assurance\" }]\ncross_links = [\"03-management\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/quality-assurance/reliability-assurance-program\"\ntitle = \"Reliability assurance program\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"17.4\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/quality-assurance/maintenance-rule\"\ntitle = \"Maintenance rule\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"17.6\" }, { document = \"10cfr50\", section = \"\u{a7} 50.65 Requirements for monitoring the effectiveness of maintenance at nuclear power plants\" }, { document = \"10cfr53\", section = \"\u{a7} 53.715 Maintenance, repair, and inspection programs\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/quality-assurance/software-quality-assurance\"\ntitle = \"Software quality assurance\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 17; BTP 7-14 (software reviews)\" }, { document = \"nureg-1520-rev2\", section = \"5.4.3.1.7.1 Calculational Method Validation\" }]\nwhy = \"agreed on #726 as the home of NQA-1-style software QA; the SRP names QA and I&C software reviews but not scientific-software QA.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/quality-assurance/software-quality-assurance/verification-and-validation-records\"\ntitle = \"Verification and validation records (gates, oracles, recorded results)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"5.4.3.1.7.1\" }]\nwhy = \"every outram-park V&V gate records methodology and results (root CLAUDE.md); these modules hold them.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/quality-assurance/software-quality-assurance/configuration-and-accounting-records\"\ntitle = \"Configuration and accounting records (commit trailers, historian reports)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 17\" }]\nwhy = \"kovan-metrics keeps the per-commit records and the pre-merge historian report.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/quality-assurance/software-quality-assurance/code-review\"\ntitle = \"Code review and code walks\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"BTP 7-14\" }]\nwhy = \"kovan\'s Code Review tab and code walks: the review machinery itself.\"\nstatus = \"approved\"\n\n# ============================================================================\n# 08-radiation-protection / radiation-protection\n# ============================================================================\n\n[[concept]]\npath = \"08-radiation-protection/radiation-protection/protection-of-plant-workers/alara\"\ntitle = \"Assuring occupational radiation exposures are as low as is reasonably achievable\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"12.1\" }, { document = \"nureg-1537-part1\", section = \"11.1.3 ALARA Program\" }, { document = \"nureg-1520-rev2\", section = \"4.4.2\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"08-radiation-protection/radiation-protection/radiation-sources\"\ntitle = \"Radiation sources\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"12.2\" }, { document = \"nureg-1537-part1\", section = \"11.1.1\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"08-radiation-protection/radiation-protection/shielding\"\ntitle = \"Radiation protection design features and biological shielding\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"12.3-12.4\" }, { document = \"nureg-1537-part1\", section = \"4.4 Biological Shield\" }]\ncross_links = [\"02-nuclear-safety/nuclear-design/neutron-transport\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"08-radiation-protection/radiation-protection/monitoring-exposure-control-and-dosimetry\"\ntitle = \"Radiation monitoring, exposure control and dosimetry\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"11.1.4; 11.1.5; 11.1.6\" }, { document = \"nureg-1520-rev2\", section = \"4.4.7\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"08-radiation-protection/radiation-protection/operational-radiation-protection-program\"\ntitle = \"Operational radiation protection program\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"12.5\" }, { document = \"nureg-1537-part1\", section = \"11.1.2\" }, { document = \"10cfr53\", section = \"\u{a7} 53.850 Radiation protection, (a)\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 09-electrical-grid / electric-power   (SRP Ch. 8)\n# ============================================================================\n\n[[concept]]\npath = \"09-electrical-grid/electric-power/offsite-power-system\"\ntitle = \"Offsite power system (incl. stability of offsite power systems)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"8.2; BTP 8-3\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"09-electrical-grid/electric-power/onsite-power-systems\"\ntitle = \"Onsite AC and DC power systems; emergency electrical power\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"8.3.1; 8.3.2\" }, { document = \"nureg-1537-part1\", section = \"8.2 Emergency Electrical Power Systems\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"09-electrical-grid/electric-power/station-blackout\"\ntitle = \"Station blackout\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"8.4\" }, { document = \"10cfr50\", section = \"\u{a7} 50.63 Loss of all alternating current power\" }]\ncross_links = [\"02-nuclear-safety/accident-analysis/decrease-in-heat-removal\"]\nstatus = \"approved\"\n\n# ============================================================================\n# 10-human-resource-development / knowledge-management-and-education\n# ============================================================================\n\n[[concept]]\npath = \"10-human-resource-development/knowledge-management-and-education/knowledge-management\"\ntitle = \"Nuclear knowledge management (literature corpus, concept map)\"\norigin = \"outram-park\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.10\" }]\nwhy = \"kovan; the L2 node\'s only source is the IAEA issue itself, which names no sub-concepts.\"\nnote = \"The NRC guides have no knowledge-management chapter, so this sits on the IAEA text (private tier, cited by section).\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"10-human-resource-development/knowledge-management-and-education/education-and-outreach\"\ntitle = \"Education and outreach (lessons, tutorials, interactive demos)\"\norigin = \"outram-park\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.10\" }]\nwhy = \"the maintainer\'s scope note for issue 10: outreach and education; dhoby-ghaut\'s web-demo framework and workbench.\"\nstatus = \"approved\"\n\n# ============================================================================\n# 12-site-and-supporting-facilities / site-characteristics\n# ============================================================================\n\n[[concept]]\npath = \"12-site-and-supporting-facilities/site-characteristics/geography-and-demography\"\ntitle = \"Geography and demography (site location, exclusion area, population distribution)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"2.1.1-2.1.3\" }, { document = \"nureg-1537-part1\", section = \"2.1\" }, { document = \"10cfr53\", section = \"\u{a7} 53.530 Population-related considerations\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"12-site-and-supporting-facilities/site-characteristics/nearby-industrial-transportation-and-military-facilities\"\ntitle = \"Nearby industrial, transportation, and military facilities\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"2.2.1-2.2.3\" }, { document = \"nureg-1537-part1\", section = \"2.2\" }, { document = \"10cfr53\", section = \"\u{a7} 53.510 External hazards\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"12-site-and-supporting-facilities/site-characteristics/hydrologic-engineering\"\ntitle = \"Hydrology (floods, groundwater, accidental releases of radioactive liquid effluents)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"2.4.1-2.4.14 (2.4.12 Groundwater; 2.4.13 Accidental Releases of Radioactive Liquid Effluents in Ground and Surface Waters)\" }, { document = \"nureg-1537-part1\", section = \"2.4\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"12-site-and-supporting-facilities/site-characteristics/geology-seismology-and-geotechnical-engineering\"\ntitle = \"Geology, seismology, and geotechnical engineering\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"2.5.1-2.5.5\" }, { document = \"nureg-1537-part1\", section = \"2.5\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 13-environmental-protection\n# ============================================================================\n\n[[concept]]\npath = \"13-environmental-protection/meteorology-and-air-quality/climatology-and-meteorology\"\ntitle = \"Regional climatology and local meteorology\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"2.3.1; 2.3.2\" }, { document = \"nureg-1537-part1\", section = \"2.3.1; 2.3.2\" }, { document = \"nureg-1555\", section = \"2.7\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"13-environmental-protection/meteorology-and-air-quality/onsite-meteorological-measurements\"\ntitle = \"Onsite meteorological measurements programs\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"2.3.3\" }, { document = \"nureg-1555\", section = \"6.4 Meteorological Monitoring\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"13-environmental-protection/meteorology-and-air-quality/short-term-accident-dispersion\"\ntitle = \"Short term atmospheric dispersion estimates for accident releases\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"2.3.4\" }]\ncross_links = [\"14-emergency-planning/accident-assessment\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"13-environmental-protection/meteorology-and-air-quality/long-term-routine-dispersion\"\ntitle = \"Long-term atmospheric dispersion estimates for routine releases\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"2.3.5\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"13-environmental-protection/meteorology-and-air-quality/relative-concentration-and-deposition\"\ntitle = \"Relative concentration (chi/Q) and relative deposition (D/Q), incl. wet deposition and decay in transit\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1555\", section = \"2.7, III. Review Procedures, item (5)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"13-environmental-protection/radiological-impacts-of-normal-operation/exposure-pathways\"\ntitle = \"Exposure pathways\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1555\", section = \"5.4.1\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"13-environmental-protection/radiological-impacts-of-normal-operation/doses-to-members-of-the-public\"\ntitle = \"Radiation doses to members of the public\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1555\", section = \"5.4.2; 5.4.3\" }, { document = \"10cfr53\", section = \"\u{a7} 53.260 Normal operations\" }, { document = \"10cfr53\", section = \"\u{a7} 53.425 Design features and functional design criteria for normal operations\" }, { document = \"10cfr53\", section = \"\u{a7} 53.450 Analysis requirements, (g)(3) Dose to members of the public\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1645 Reports of radiation exposure to members of the public\" }, { document = \"10cfr50\", section = \"Appendix I, Numerical Guides for Design Objectives and Limiting Conditions for Operation To Meet the Criterion \\\"As Low as is Reasonably Achievable\\\" for Radioactive Material in Light-Water-Cooled Nuclear Power Reactor Effluents\" }]\ncross_links = [\"08-radiation-protection/radiation-protection\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"13-environmental-protection/radiological-impacts-of-normal-operation/impacts-to-biota\"\ntitle = \"Impacts to biota other than members of the public\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1555\", section = \"5.4.4\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"13-environmental-protection/environmental-monitoring/radiological-monitoring\"\ntitle = \"Radiological monitoring\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1555\", section = \"6.2\" }, { document = \"nureg-1537-part1\", section = \"11.1.7 Environmental Monitoring\" }, { document = \"10cfr53\", section = \"\u{a7} 53.850 Radiation protection, (b) (radiological environmental monitoring program)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"13-environmental-protection/postulated-accident-impacts/design-basis-accident-consequences\"\ntitle = \"Environmental consequences of design basis accidents\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1555\", section = \"7.1\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"13-environmental-protection/postulated-accident-impacts/severe-accident-consequences\"\ntitle = \"Severe accidents and severe accident mitigation alternatives\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1555\", section = \"7.2; 7.3\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"13-environmental-protection/postulated-accident-impacts/transportation-accidents\"\ntitle = \"Transportation accidents\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1555\", section = \"7.4\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 14-emergency-planning / accident-assessment   (NUREG-0654 Rev. 2, Planning standard I)\n# World codes only; outram-park crates link here by \'aspiration\', never as leaves (#724).\n# ============================================================================\n\n[[concept]]\npath = \"14-emergency-planning/accident-assessment/release-magnitude-and-isotopic-composition\"\ntitle = \"Magnitude and isotopic composition of an ongoing or potential release\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"I.1.a; I.4\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"14-emergency-planning/accident-assessment/airborne-radiological-assessment-model\"\ntitle = \"Radiological assessment model for airborne releases (dispersion model)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"I.1.b\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"14-emergency-planning/accident-assessment/field-monitoring-and-plume-tracking\"\ntitle = \"Field monitoring teams, plume location and tracking\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"I.1.c; I.5; I.7; I.9\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"14-emergency-planning/accident-assessment/dose-projection\"\ntitle = \"Dose projection and its validation with field data\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"I.8; I.10\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"14-emergency-planning/accident-assessment/drinking-water-contamination\"\ntitle = \"Contamination of drinking water through liquid release or deposition\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0654-rev2\", section = \"I.2\" }]\nstatus = \"approved\"\n\n# ============================================================================\n# 16-nuclear-fuel-cycle\n# ============================================================================\n\n[[concept]]\npath = \"16-nuclear-fuel-cycle/nuclear-criticality-safety/subcriticality-and-double-contingency\"\ntitle = \"Subcriticality and double-contingency principle\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"5.4.3.1.4\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"16-nuclear-fuel-cycle/nuclear-criticality-safety/calculational-method-validation\"\ntitle = \"Calculational method validation (criticality code validation, margin of subcriticality)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"5.4.3.1.7.1; Appendix 5-B (margin of subcriticality)\" }]\ncross_links = [\"02-nuclear-safety/nuclear-design/neutron-transport\", \"07-regulatory-framework/quality-assurance/software-quality-assurance/verification-and-validation-records\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"16-nuclear-fuel-cycle/nuclear-criticality-safety/criticality-safety-evaluations\"\ntitle = \"Criticality safety evaluations and controlled parameters\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"5.4.3.1.7.2; 5.4.3.1.7.3\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"16-nuclear-fuel-cycle/nuclear-criticality-safety/criticality-accident-alarm-system\"\ntitle = \"Criticality accident alarm system\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"5.4.3.1.2\" }, { document = \"10cfr50\", section = \"\u{a7} 50.68 Criticality accident requirements\" }, { document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (m)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"16-nuclear-fuel-cycle/integrated-safety-analysis/isa-methodology-and-summary\"\ntitle = \"Integrated safety analysis: methodology, documentation and summary\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1520-rev2\", section = \"3.3.1; 3.3.2; 5.4.3.2.1-5.4.3.2.3\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"18-industrial-involvement/process-heat-and-industrial-applications/chemical-process-simulation\"\ntitle = \"Chemical process simulation (flowsheets, reactors, separation columns)\"\norigin = \"outram-park\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.18\" }, { document = \"nureg-1520-rev2\", section = \"Ch. 6 Chemical Process Safety\" }]\nwhy = \"the DWSIM port (chemical processes; process heat applications), maintainer 2026-10-06.\"\nnote = \"Provisional (maintainer, 2026-10-06): under 18 industrial involvement, cross-linked to fuel-cycle chemical process safety.\"\ncross_links = [\"16-nuclear-fuel-cycle/chemical-process-safety\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"16-nuclear-fuel-cycle/fuel-depletion/depletion-solvers\"\ntitle = \"Depletion solvers (Bateman equations, matrix exponential, Monte Carlo transmutation)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.1 (changes in core reactivity with fuel burnup, plutonium buildup, and poisons)\" }, { document = \"iaea-ng-g-3.1-rev1\", section = \"3.16\" }]\nwhy = \"outram-mc\'s burnup loop, ONIX\'s CRAM and boon-lay\'s Lagrangian transmutation.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"16-nuclear-fuel-cycle/fuel-depletion/decay-data\"\ntitle = \"Radioactive decay data and decay chains\"\norigin = \"outram-park\"\nsources = [{ document = \"iaea-ng-g-3.1-rev1\", section = \"3.16\" }]\nwhy = \"boon-lay\'s nuclide decay library feeds depletion and source terms.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"16-nuclear-fuel-cycle/fuel-depletion/irradiation-history\"\ntitle = \"Burnup and fast-fluence accumulation (irradiation history)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-0800-4.2-rev3\", section = \"II.3.C.i (burnup distribution in the fuel)\" }]\nwhy = \"fuel-performance bookkeeping that feeds the fuel-system models.\"\nstatus = \"approved\"\n\n# ============================================================================\n# 17-radioactive-waste-management\n# ============================================================================\n\n[[concept]]\npath = \"17-radioactive-waste-management/radioactive-waste-management/liquid-waste-management\"\ntitle = \"Liquid waste management system\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"11.2\" }, { document = \"ornl-tm-2018-976\", section = \"3.5.2\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"17-radioactive-waste-management/radioactive-waste-management/gaseous-waste-management\"\ntitle = \"Gaseous waste management system (incl. MSR off-gas)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"11.3\" }, { document = \"ornl-tm-2018-976\", section = \"3.5.3\" }]\ncross_links = [\"13-environmental-protection/meteorology-and-air-quality/long-term-routine-dispersion\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"17-radioactive-waste-management/radioactive-waste-management/solid-waste-management\"\ntitle = \"Solid waste management system\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"11.4\" }, { document = \"ornl-tm-2018-976\", section = \"3.5.4\" }, { document = \"10cfr53\", section = \"\u{a7} 53.850 Radiation protection, (c) (Process Control Program)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"17-radioactive-waste-management/radioactive-waste-management/effluent-monitoring-and-release\"\ntitle = \"Process and effluent radiological monitoring; release of radioactive waste\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"11.5\" }, { document = \"nureg-1537-part1\", section = \"11.2.3 Release of Radioactive Waste\" }, { document = \"10cfr53\", section = \"\u{a7} 53.850 Radiation protection, (b) (Offsite Dose Calculations Manual)\" }, { document = \"10cfr50\", section = \"\u{a7} 50.36a Technical specifications on effluents from nuclear power reactors\" }]\ncross_links = [\"13-environmental-protection/environmental-monitoring/radiological-monitoring\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"17-radioactive-waste-management/decommissioning/decommissioning-plan-and-alternatives\"\ntitle = \"Decommissioning plan and alternatives\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"17.1.1-17.1.3\" }, { document = \"10cfr50\", section = \"\u{a7} 50.82 Termination of license\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1070 Termination of license\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1075 Program requirements during decommissioning\" }, { document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (l)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"17-radioactive-waste-management/decommissioning/release-criteria-and-final-survey\"\ntitle = \"Release criteria and final survey\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"17.1.4\" }, { document = \"10cfr50\", section = \"\u{a7} 50.83 Release of part of a power reactor facility or site for unrestricted use\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1080 Release of part of a commercial nuclear plant or site for unrestricted use\" }]\nstatus = \"approved\"\n\n# ############################################################################\n# LEVEL 4 \u{2014} outram-park implementation leaves (seed kovan-concept tags)\n# ############################################################################\n\n# ---- design of SSCs -------------------------------------------------------\n\n[[implementation]]\nconcept = \"02-nuclear-safety/design-of-structures-systems-and-components/mechanical-systems-and-components\"\ncrate = \"farrer-park\"\nmodule = \"assembly\"\ntitle = \"FEM assembly (elasticity and J2 plasticity)\"\nwhat = \"Finite-element assembly for structural mechanics on the shared outram-foam numerical backend.\"\nkind = \"port\"\nupstream = \"MOOSE / PRISMS-Plasticity (LGPL-2.1)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/design-of-structures-systems-and-components/mechanical-systems-and-components\"\ncrate = \"farrer-park\"\nmodule = \"crystal\"\ntitle = \"Crystal plasticity\"\nwhat = \"Plastic flow as crystallographic slip on discrete slip systems.\"\nkind = \"port\"\nupstream = \"PRISMS-Plasticity\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/design-of-structures-systems-and-components/mechanical-systems-and-components\"\ncrate = \"farrer-park\"\nmodule = \"fatigue\"\ntitle = \"Fatigue indicator parameters\"\nwhat = \"Scalar measures that rank microstructural sites by fatigue-crack initiation propensity.\"\nkind = \"port\"\nupstream = \"PRISMS-Fatigue\"\nstatus = \"proposed\"\n\n# ---- fuel system design ---------------------------------------------------\n\n[[implementation]]\nconcept = \"02-nuclear-safety/fuel-system-design/fuel-system-damage/oxidation-hydriding-and-crud\"\ncrate = \"outram-park-fork-offbeat\"\nmodule = \"corrosion\"\ntitle = \"Cladding waterside corrosion and hydrogen pickup\"\nwhat = \"Oxide growth kinetics, hydrogen uptake and solver acceleration for cladding corrosion.\"\nkind = \"port\"\nupstream = \"OFFBEAT\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/fuel-system-design/fuel-system-damage/rod-internal-gas-pressure\"\ncrate = \"outram-park-fork-offbeat\"\nmodule = \"fgr\"\ntitle = \"Fission-gas release\"\nwhat = \"Xenon and krypton release from the fuel and its effect on rod internal pressure.\"\nkind = \"port\"\nupstream = \"OFFBEAT\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/fuel-system-design/fuel-rod-failure/pellet-cladding-interaction\"\ncrate = \"outram-park-fork-offbeat\"\nmodule = \"gap\"\ntitle = \"Fuel/cladding gap: gas, conductance and contact\"\nwhat = \"Gap gas composition, gap conductance and pellet-cladding contact by axial slice.\"\nkind = \"port\"\nupstream = \"OFFBEAT\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/fuel-system-design/fuel-system-damage\"\ncrate = \"outram-park-fork-offbeat\"\nmodule = \"mechanics\"\ntitle = \"Fuel-performance solid mechanics\"\nwhat = \"The displacement solve for fuel and cladding stress and strain.\"\nkind = \"port\"\nupstream = \"OFFBEAT\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/fuel-system-design/fuel-system-damage\"\ncrate = \"outram-park-fork-offbeat\"\nmodule = \"rheology\"\ntitle = \"Fuel and cladding constitutive laws\"\nwhat = \"Creep, yield and material-specific rheology laws.\"\nkind = \"port\"\nupstream = \"OFFBEAT (code_aster behaviour laws)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/fuel-system-design/fuel-salt-chemistry\"\ncrate = \"outram-park-fork-thermochimica\"\nmodule = \"gem\"\ntitle = \"CALPHAD Gibbs-energy minimisation\"\nwhat = \"Molten-salt equilibrium thermochemistry: fission-product speciation, redox and solubility.\"\nkind = \"port\"\nupstream = \"ORNL Thermochimica (BSD-3)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-failure\"\ncrate = \"boon-lay\"\nmodule = \"fuel_failure\"\ntitle = \"boon-lay fuel failure (PANAMA-I equations)\"\nwhat = \"TRISO pressure-vessel failure fractions under accident conditions, from the published PANAMA-I equations.\"\nkind = \"new-work\"\nupstream = \"Verfondern & Nabielek, PANAMA-I mathematical basis (equations only; no code)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release\"\ncrate = \"boon-lay\"\nmodule = \"triso_atops_fork\"\ntitle = \"TRISO-ATOPS fork: continuum fission-product release\"\nwhat = \"Eulerian diffusion and release of fission products through TRISO layers, normal operation and accident.\"\nkind = \"port\"\nupstream = \"INL TRISO-ATOPS\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release\"\ncrate = \"boon-lay\"\nmodule = \"triso_atops_extensions\"\ntitle = \"TRISO-ATOPS extensions\"\nwhat = \"Questions upstream\'s model cannot express, kept apart from the faithful port.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-conduction\"\ncrate = \"tampines\"\nmodule = \"pebble_bed/triso\"\ntitle = \"TRISO coated-particle conduction\"\nwhat = \"Steady radial conduction through the five concentric TRISO regions.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/fuel-system-design/fuel-performance-analytical-predictions/fuel-temperatures-and-stored-energy\"\ncrate = \"teh-o-prke\"\nmodule = \"fuel_temperature_feedback\"\ntitle = \"Lumped fuel temperature model\"\nwhat = \"Fuel temperature from power and gap conductance, feeding Doppler feedback.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n# ---- nuclear design -------------------------------------------------------\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/core-configuration-and-criticality\"\ncrate = \"nee_soon\"\nmodule = \"htr10_rmc\"\ntitle = \"HTR-10 core model, code-to-code against RMC\"\nwhat = \"HTR-10 first-criticality core built for outram-mc and verified against the published RMC benchmark.\"\nkind = \"new-work\"\nupstream = \"Li, Yu & Wei (2014), HTR-10 benchmark with RMC\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/core-configuration-and-criticality\"\ncrate = \"nee_soon\"\nmodule = \"det_six_factor\"\ntitle = \"Six-factor decomposition of a deterministic solve\"\nwhat = \"Breaks a diffusion/SP3 k_eff into the six-factor formula for comparison with Monte Carlo.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/core-configurations/pebble-bed/pebble-bed-packing\"\ncrate = \"outram-park-fork-liggghts\"\nmodule = \"granular\"\ntitle = \"Granular contact pipeline (pair_style gran)\"\nwhat = \"Surface, normal and tangential contact models for settling a pebble bed.\"\nkind = \"port\"\nupstream = \"LIGGGHTS (logic translated; see crate NOTICE on GPL-2)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/core-configurations/pebble-bed/pebble-bed-packing\"\ncrate = \"outram-park-fork-liggghts\"\nmodule = \"integrator\"\ntitle = \"Velocity-Verlet integration for spheres\"\nwhat = \"Kick-drift-kick propagation of DEM spheres (nve/sphere).\"\nkind = \"port\"\nupstream = \"LIGGGHTS FixNVESphere\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/core-configurations/pebble-bed/pebble-bed-packing\"\ncrate = \"outram-park-fork-liggghts\"\nmodule = \"htr10_fill\"\ntitle = \"HTR-10 pebble pour\"\nwhat = \"Pours and settles the HTR-10 pebble bed under gravity, steppable from a UI.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/core-configurations/pebble-bed/pebble-bed-packing\"\ncrate = \"outram-mc-libs\"\nmodule = \"pebble_beds/crp_packing\"\ntitle = \"Close random packing of pebbles\"\nwhat = \"Random sphere packing for pebble positions.\"\nkind = \"port\"\nupstream = \"OpenMC openmc.model.pack_spheres\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/core-configurations/pebble-bed/pebble-bed-packing\"\ncrate = \"outram-mc-libs\"\nmodule = \"pebble_beds/dem_bed\"\ntitle = \"DEM-settled pebble beds for transport\"\nwhat = \"Hands a bed settled by the LIGGGHTS fork to Monte Carlo transport.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/power-distribution\"\ncrate = \"bedok\"\nmodule = \"calc_relpower3d\"\ntitle = \"3-D relative power distribution\"\nwhat = \"Relative nodal power from the BEDOK diffusion solution.\"\nkind = \"port\"\nupstream = \"BEDOK MATLAB (Than Yan Ren, SNRSI)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/reactivity-coefficients\"\ncrate = \"outram-foam-appbuilder-lib\"\nmodule = \"genfoam/thermo_mechanics\"\ntitle = \"Thermal-expansion feedback\"\nwhat = \"Linear-elastic thermal expansion feeding geometry and density changes back to neutronics.\"\nkind = \"port\"\nupstream = \"GeN-Foam thermoMechanics\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/reactivity-coefficients\"\ncrate = \"outram-foam-appbuilder-lib\"\nmodule = \"genfoam/multi_region/reactivity_feedback\"\ntitle = \"Reactivity feedback across regions\"\nwhat = \"Feedback coefficients applied between coupled neutronics and TH meshes.\"\nkind = \"port\"\nupstream = \"GeN-Foam multiRegion\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/kinetics-parameters\"\ncrate = \"outram-mc-libs\"\nmodule = \"physics/kinetics\"\ntitle = \"beta_eff and generation time from Monte Carlo\"\nwhat = \"Point-kinetics parameters from a Monte Carlo run, feeding teh-o-prke.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/kinetics-parameters\"\ncrate = \"outram-mc-libs\"\nmodule = \"physics/ifp\"\ntitle = \"Iterated fission probability\"\nwhat = \"Adjoint-weighted beta_eff and Lambda.\"\nkind = \"port\"\nupstream = \"OpenMC src/ifp.cpp (afa7a14)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/reactor-kinetic-behaviour\"\ncrate = \"teh-o-prke\"\nmodule = \"zero_power_prke\"\ntitle = \"Six-group point reactor kinetics\"\nwhat = \"Zero-power point-kinetics equations with six precursor groups (explicit and implicit solvers).\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/reactor-kinetic-behaviour\"\ncrate = \"teh-o-prke\"\nmodule = \"delayed_neutron_layer\"\ntitle = \"Reusable delayed-neutron layer\"\nwhat = \"Backward-Euler precursor bank, O(1) per step.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/reactor-kinetic-behaviour\"\ncrate = \"outram-foam-appbuilder-lib\"\nmodule = \"genfoam/neutronics/point_kinetics\"\ntitle = \"GeN-Foam point kinetics\"\nwhat = \"0-D reactor kinetics with precursor groups inside the multiphysics solver.\"\nkind = \"port\"\nupstream = \"GeN-Foam pointKinetics\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/reactivity-worths\"\ncrate = \"teh-o-prke\"\nmodule = \"control_rod_feedback\"\ntitle = \"Control-rod reactivity\"\nwhat = \"Rod-position-dependent reactivity insertion for point kinetics.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/reactivity-worths\"\ncrate = \"nee_soon\"\nmodule = \"rod_insertion\"\ntitle = \"Smeared control-rod absorber in a multigroup library\"\nwhat = \"Applies rod atom densities to a multigroup set for rod-worth studies.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/reactivity-worths\"\ncrate = \"nee_soon\"\nmodule = \"htr10_rmc/control_rod\"\ntitle = \"HTR-10 control rods\"\nwhat = \"Rod composition and geometry for HTR-10 worth calculations.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/burnup-and-poison-reactivity-effects\"\ncrate = \"teh-o-prke\"\nmodule = \"feedback_mechanisms/fission_product_poisons\"\ntitle = \"Fission-product poisons (xenon, samarium)\"\nwhat = \"Poison build-up and decay and its reactivity effect.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/excess-reactivity-and-shutdown-margin\"\ncrate = \"bedok\"\nmodule = \"criticalboron_xyz\"\ntitle = \"Critical boron search\"\nwhat = \"Searches the soluble-boron concentration that makes the core critical.\"\nkind = \"port\"\nupstream = \"BEDOK MATLAB\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/accuracy-of-analytical-methods\"\ncrate = \"outram-mc-libs\"\nmodule = \"stats/uq\"\ntitle = \"Uncertainty quantification by input sampling\"\nwhat = \"Propagates input uncertainties (densities, packing) to k_eff and tallies.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/accuracy-of-analytical-methods\"\ncrate = \"outram-mc-libs\"\nmodule = \"tally/derivative\"\ntitle = \"Tally derivatives\"\nwhat = \"Sensitivity coefficients from a single run.\"\nkind = \"port\"\nupstream = \"OpenMC src/tallies/derivative.cpp (afa7a14)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/core-configurations/liquid-fuelled/delayed-neutron-precursor-drift\"\ncrate = \"outram-park-fork-moltres\"\nmodule = \"precursors\"\ntitle = \"Precursor advection-decay transport\"\nwhat = \"Delayed-neutron precursors carried by the moving fuel salt.\"\nkind = \"port\"\nupstream = \"Moltres (formulation; LGPL)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/core-configurations/liquid-fuelled/delayed-neutron-precursor-drift\"\ncrate = \"outram-park-fork-moltres\"\nmodule = \"circulating\"\ntitle = \"Circulating-fuel k-eigenvalue\"\nwhat = \"Multigroup diffusion coupled to advected precursors: the MSRE reactivity loss.\"\nkind = \"port\"\nupstream = \"Moltres\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/core-configurations/liquid-fuelled/delayed-neutron-precursor-drift\"\ncrate = \"outram-foam-appbuilder-lib\"\nmodule = \"genfoam/neutronics/diffusion/precursor_drift\"\ntitle = \"GeN-Foam precursor drift\"\nwhat = \"Precursor transport with the fuel flow inside the diffusion solver.\"\nkind = \"port\"\nupstream = \"GeN-Foam\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/surface-tracking\"\ncrate = \"outram-blender\"\nmodule = \"csg\"\ntitle = \"CSG description and navigation kernel\"\nwhat = \"Surfaces, cells, universes and lattices and the pure distance-to-boundary kernel (moved from outram-mc, #486).\"\nkind = \"port\"\nupstream = \"OpenMC surface.cpp / cell.cpp / geometry.cpp\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/surface-tracking\"\ncrate = \"outram-mc-libs\"\nmodule = \"geometry/crossing\"\ntitle = \"Surface crossing on the transport state\"\nwhat = \"The crossing work that needs outram-mc\'s RNG, kinematics and materials, as extension traits on blender\'s CSG.\"\nkind = \"port\"\nupstream = \"OpenMC geometry.cpp\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/surface-tracking\"\ncrate = \"outram-mc-libs\"\nmodule = \"physics/transport_csg\"\ntitle = \"Power iteration over general CSG\"\nwhat = \"k-eigenvalue transport with surface tracking and boundary conditions on any CSG geometry.\"\nkind = \"port\"\nupstream = \"OpenMC eigenvalue/transport loop\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/delta-tracking\"\ncrate = \"outram-mc-libs\"\nmodule = \"physics/delta_tracking\"\ntitle = \"Delta (Woodcock) tracking\"\nwhat = \"Moves a neutron by rejection against a majorant instead of by surface search.\"\nkind = \"new-work\"\nupstream = \"Woodcock et al. (1965) method; OpenMC has no delta tracking\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/delta-tracking\"\ncrate = \"outram-mc-libs\"\nmodule = \"pebble_beds/delta_tracking\"\ntitle = \"Delta tracking in pebble beds\"\nwhat = \"Delta tracking specialised to doubly heterogeneous beds.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/delta-tracking/majorant\"\ncrate = \"outram-mc-libs\"\nmodule = \"physics/delta_tracking/majorant\"\ntitle = \"Majorant cross section\"\nwhat = \"Builds and guards the energy-dependent bound every delta-tracking flight samples on.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/doubly-heterogeneous-geometry\"\ncrate = \"outram-mc-libs\"\nmodule = \"dh_universe\"\ntitle = \"Doubly heterogeneous universes\"\nwhat = \"One enum to choose how TRISO particles are resolved (explicit, RPT, stochastic).\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/doubly-heterogeneous-geometry\"\ncrate = \"outram-mc-libs\"\nmodule = \"stochastic\"\ntitle = \"Stochastic-media transport (CLS, SCLS)\"\nwhat = \"Random geometry sampled during transport rather than stored.\"\nkind = \"new-work\"\nupstream = \"chord-length sampling literature; absent from OpenMC\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/doubly-heterogeneous-geometry\"\ncrate = \"outram-mc-libs\"\nmodule = \"pebble_beds\"\ntitle = \"Pebble-bed specialisation\"\nwhat = \"TRISO-in-pebble-in-bed transport slice for HTR-10 and FHR pebbles.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/doubly-heterogeneous-geometry\"\ncrate = \"outram-blender\"\nmodule = \"csg/triso_particle\"\ntitle = \"Five-shell TRISO particle builder\"\nwhat = \"CSG construction of a TRISO particle.\"\nkind = \"port\"\nupstream = \"OpenMC openmc.model.TRISO\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/continuous-energy-collision-physics\"\ncrate = \"outram-mc-libs\"\nmodule = \"physics/scatter\"\ntitle = \"Elastic and inelastic scattering kinematics\"\nwhat = \"Samples outgoing energy and angle from the evaluation\'s own laws.\"\nkind = \"port\"\nupstream = \"OpenMC physics.cpp, physics_common.cpp\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/continuous-energy-collision-physics\"\ncrate = \"outram-mc-libs\"\nmodule = \"physics/fission\"\ntitle = \"Fission neutron production\"\nwhat = \"Banks fission sites for the next generation.\"\nkind = \"port\"\nupstream = \"OpenMC physics.cpp fission()\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/eigenvalue-and-fixed-source-calculations\"\ncrate = \"outram-mc-libs\"\nmodule = \"physics/keff\"\ntitle = \"k-eigenvalue power iteration (bare sphere driver)\"\nwhat = \"Minimal criticality driver with generation statistics.\"\nkind = \"port\"\nupstream = \"OpenMC eigenvalue.cpp\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/eigenvalue-and-fixed-source-calculations\"\ncrate = \"outram-mc-libs\"\nmodule = \"physics/fixed_source\"\ntitle = \"Fixed-source mode\"\nwhat = \"Source-driven transport without eigenvalue iteration.\"\nkind = \"port\"\nupstream = \"OpenMC\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/variance-reduction\"\ncrate = \"outram-mc-libs\"\nmodule = \"physics/weight_windows\"\ntitle = \"Mesh weight windows and MAGIC\"\nwhat = \"Splitting, roulette and weight-window generation from a flux tally.\"\nkind = \"port\"\nupstream = \"OpenMC src/weight_windows.cpp (afa7a14)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/variance-reduction\"\ncrate = \"outram-mc-libs\"\nmodule = \"physics/ufs\"\ntitle = \"Uniform fission site weighting\"\nwhat = \"Flattens fission-site density for better local tally statistics.\"\nkind = \"port\"\nupstream = \"OpenMC src/eigenvalue.cpp ufs_* (afa7a14)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/monte-carlo-tallies\"\ncrate = \"outram-mc-libs\"\nmodule = \"tally\"\ntitle = \"Tallies, filters and triggers\"\nwhat = \"Scoring of fluxes and reaction rates with filters, arithmetic and triggers.\"\nkind = \"port\"\nupstream = \"OpenMC src/tallies/\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/monte-carlo-tallies\"\ncrate = \"outram-mc-libs\"\nmodule = \"tally/mesh_unstructured\"\ntitle = \"Unstructured-mesh tally scoring\"\nwhat = \"Track-length scoring on blender\'s neutral unstructured mesh (#492).\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/deterministic-neutronics\"\ncrate = \"outram-foam-appbuilder-lib\"\nmodule = \"genfoam/neutronics/diffusion\"\ntitle = \"Multigroup neutron diffusion\"\nwhat = \"Eigenvalue and transient multigroup diffusion on an FV mesh.\"\nkind = \"port\"\nupstream = \"GeN-Foam diffusionNeutronics\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/deterministic-neutronics\"\ncrate = \"outram-foam-appbuilder-lib\"\nmodule = \"genfoam/neutronics/sp3\"\ntitle = \"Simplified P3 transport\"\nwhat = \"SP3 neutronics on the diffusion machinery.\"\nkind = \"port\"\nupstream = \"GeN-Foam SP3Neutronics\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/deterministic-neutronics\"\ncrate = \"outram-foam-appbuilder-lib\"\nmodule = \"genfoam/neutronics/sn\"\ntitle = \"Discrete ordinates (Sn)\"\nwhat = \"Angular quadrature and sweeps for Sn eigenvalue problems.\"\nkind = \"port\"\nupstream = \"GeN-Foam\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/deterministic-neutronics\"\ncrate = \"outram-park-fork-moltres\"\nmodule = \"diffusion\"\ntitle = \"Static-fuel multigroup diffusion\"\nwhat = \"k-eigenvalue multigroup diffusion on an FvMesh.\"\nkind = \"port\"\nupstream = \"Moltres (formulation)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/deterministic-neutronics\"\ncrate = \"bedok\"\nmodule = \"sanodaldiffusion_solverxyz\"\ntitle = \"3-D semi-analytic nodal diffusion\"\nwhat = \"Nodal diffusion solver of the BEDOK simulator.\"\nkind = \"port\"\nupstream = \"BEDOK MATLAB (Than & Xiao 2026)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/multigroup-cross-section-condensation\"\ncrate = \"nee_soon\"\nmodule = \"mgxs\"\ntitle = \"MGXS condensed from Monte Carlo\"\nwhat = \"Flux-weighted group constants tallied by outram-mc.\"\nkind = \"new-work\"\nupstream = \"OpenMC openmc.mgxs (concept)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/neutron-transport/multigroup-cross-section-condensation\"\ncrate = \"nee_soon\"\nmodule = \"genfoam_xs\"\ntitle = \"MGXS handed to GeN-Foam\"\nwhat = \"Turns condensed group constants into GeN-Foam\'s nuclear-data input.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"endf\"\ntitle = \"ENDF tape model\"\nwhat = \"Parses and represents ENDF-6 tapes, the substrate of every NJOY module.\"\nkind = \"port\"\nupstream = \"NJOY2016 endf.f90\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"nuclear_data\"\ntitle = \"Nuclear-data provider surface\"\nwhat = \"What the transport crates pull from njoy (all nuclear data lives here).\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/resonance-reconstruction\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"reconr\"\ntitle = \"RECONR\"\nwhat = \"Reconstructs pointwise cross sections from resonance parameters onto a PENDF tape.\"\nkind = \"port\"\nupstream = \"NJOY2016 reconr.f90\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/resonance-reconstruction\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"samm\"\ntitle = \"SAMM R-matrix kernel\"\nwhat = \"Reich-Moore and R-matrix-limited (LRF=7) cross sections.\"\nkind = \"port\"\nupstream = \"NJOY2016 samm.f90\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/doppler-broadening\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"broadr\"\ntitle = \"BROADR (SIGMA1)\"\nwhat = \"Doppler broadening of pointwise cross sections.\"\nkind = \"port\"\nupstream = \"NJOY2016 broadr.f90\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/doppler-broadening\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"wmp\"\ntitle = \"Windowed multipole\"\nwhat = \"Analytic on-the-fly Doppler broadening from multipole data.\"\nkind = \"port\"\nupstream = \"MIT CRPG windowed multipole (WMP_Library, MIT licence)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/unresolved-resonance-probability-tables\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"purr\"\ntitle = \"PURR probability tables\"\nwhat = \"URR probability tables for Monte Carlo self-shielding.\"\nkind = \"port\"\nupstream = \"NJOY2016 purr.f90\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/unresolved-resonance-probability-tables\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"unresr\"\ntitle = \"UNRESR\"\nwhat = \"Bondarenko self-shielded cross sections in the unresolved range.\"\nkind = \"port\"\nupstream = \"NJOY2016 unresr.f90\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/thermal-scattering\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"thermr\"\ntitle = \"THERMR\"\nwhat = \"Bound-atom thermal cross sections and secondary distributions from MF=7.\"\nkind = \"port\"\nupstream = \"NJOY2016 thermr.f90\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/thermal-scattering\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"leapr\"\ntitle = \"LEAPR\"\nwhat = \"Generates S(alpha, beta) from a phonon model.\"\nkind = \"port\"\nupstream = \"NJOY2016 leapr.f90\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/thermal-scattering\"\ncrate = \"outram-mc-libs\"\nmodule = \"material/thermal\"\ntitle = \"S(alpha, beta) in transport\"\nwhat = \"Samples bound-atom scattering from thermal tables during transport.\"\nkind = \"port\"\nupstream = \"OpenMC src/thermal.cpp\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/ace-library-generation\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"acer\"\ntitle = \"ACER\"\nwhat = \"Assembles and writes continuous-energy ACE libraries.\"\nkind = \"port\"\nupstream = \"NJOY2016 acefc.f90\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/multigroup-data-generation\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"groupr\"\ntitle = \"GROUPR\"\nwhat = \"Self-shielded multigroup cross sections and transfer matrices.\"\nkind = \"port\"\nupstream = \"NJOY2016 groupr.f90\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/multigroup-data-generation\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"gaminr\"\ntitle = \"GAMINR\"\nwhat = \"Multigroup photoatomic cross sections and matrices.\"\nkind = \"port\"\nupstream = \"NJOY2016 gaminr.f90\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/nuclear-data-covariances\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"errorr\"\ntitle = \"ERRORR\"\nwhat = \"Multigroup covariance matrices from ENDF covariance files.\"\nkind = \"port\"\nupstream = \"NJOY2016 errorr.f90\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/nuclear-data-covariances\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"covr\"\ntitle = \"COVR\"\nwhat = \"Correlation matrices and covariance reports from ERRORR output.\"\nkind = \"port\"\nupstream = \"NJOY2016 covr.f90\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/nuclear-design/nuclear-data-processing/heating-and-damage\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"heatr\"\ntitle = \"HEATR\"\nwhat = \"KERMA heating and damage cross sections.\"\nkind = \"port\"\nupstream = \"NJOY2016 heatr.f90\"\nstatus = \"proposed\"\n\n# ---- moderator and reflector ----------------------------------------------\n\n[[implementation]]\nconcept = \"02-nuclear-safety/moderator-and-reflector/moderator-and-reflector-nuclear-design\"\ncrate = \"nee_soon\"\nmodule = \"htr10_rmc/reflector\"\ntitle = \"HTR-10 graphite reflector\"\nwhat = \"Reflector materials and channels of the HTR-10 model.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/moderator-and-reflector/moderator-and-reflector-nuclear-design\"\ncrate = \"nee_soon\"\nmodule = \"htr10_rmc/reflector_geometry\"\ntitle = \"HTR-10 reflector geometry\"\nwhat = \"CSG geometry of the HTR-10 side, top and bottom reflectors.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n# ---- thermal-hydraulic design --------------------------------------------\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/core-coolant-hydraulics\"\ncrate = \"tuas_boussinesq_solver\"\nmodule = \"lib/fluid_mechanics_correlations\"\ntitle = \"Friction factor and pipe hydraulics correlations\"\nwhat = \"Churchill friction factor, custom fLDK and pipe pressure-loss calculations.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/heat-transfer-to-coolant\"\ncrate = \"tuas_boussinesq_solver\"\nmodule = \"lib/heat_transfer_correlations\"\ntitle = \"Heat-transfer correlations\"\nwhat = \"Nusselt correlations, thermal resistances, view factors and parallel heat exchangers.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/power-density-and-heat-flux-distribution\"\ncrate = \"bedok\"\nmodule = \"fuelrodheat_1dcylnd\"\ntitle = \"1-D cylindrical fuel-rod heat conduction\"\nwhat = \"Radial fuel-rod temperature from nodal power.\"\nkind = \"port\"\nupstream = \"BEDOK MATLAB\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/fuel-heat-removal-limits\"\ncrate = \"outram-foam-multiphase\"\nmodule = \"chf\"\ntitle = \"Critical heat flux correlations\"\nwhat = \"CHF, DNB and dryout point correlations.\"\nkind = \"new-work\"\nupstream = \"published CHF correlations\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/fuel-heat-removal-limits\"\ncrate = \"outram-foam-multiphase\"\nmodule = \"wall_boiling\"\ntitle = \"Wall-boiling framework\"\nwhat = \"Wall-boiling closure architecture for the two-fluid model.\"\nkind = \"port\"\nupstream = \"OpenFOAM multiphaseEuler wall boiling\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/fuel-heat-removal-limits\"\ncrate = \"bedok\"\nmodule = \"w3chf\"\ntitle = \"W-3 critical heat flux\"\nwhat = \"W-3 CHF correlation for DNBR.\"\nkind = \"port\"\nupstream = \"BEDOK MATLAB\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/natural-convection-cooling\"\ncrate = \"tuas_boussinesq_solver\"\nmodule = \"lib/pre_built_components/ciet_nat_circ_tests\"\ntitle = \"CIET natural-circulation tests\"\nwhat = \"Steady natural-circulation verification suites on the CIET DRACS loop.\"\nkind = \"new-work\"\nupstream = \"Ong, Xiao & Peterson (2025), doi:10.1016/j.jandt.2025.03.006 (peer reviewed)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/natural-convection-cooling\"\ncrate = \"tuas_boussinesq_solver\"\nmodule = \"lib/pre_built_components/uw_madison_flibe_loop_components\"\ntitle = \"UW-Madison FLiBe loop\"\nwhat = \"Pre-built components for a molten-fluoride-salt natural/forced circulation loop.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/shutdown-decay-heat\"\ncrate = \"teh-o-prke\"\nmodule = \"decay_heat\"\ntitle = \"23-group fission-product decay heat\"\nwhat = \"Decay heat after shutdown from the 1978 draft ANS standard.\"\nkind = \"new-work\"\nupstream = \"ANS 5.1 (1978 draft) 23-group fit\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/pulsing-reactor-analysis\"\ncrate = \"teh-o-prke\"\nmodule = \"nordheim_fuchs\"\ntitle = \"Nordheim-Fuchs exact timestepper\"\nwhat = \"Closed-form prompt excursion with adiabatic fuel-temperature feedback.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics\"\ncrate = \"outram-foam-basic-lib\"\nmodule = \"fv_operators\"\ntitle = \"Finite-volume operators (fvm, fvc)\"\nwhat = \"Implicit and explicit FV discretisation operators.\"\nkind = \"port\"\nupstream = \"OpenFOAM finiteVolume\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics\"\ncrate = \"outram-foam-basic-lib\"\nmodule = \"ldu_matrix\"\ntitle = \"LDU matrices and solvers\"\nwhat = \"Face-addressed sparse matrices with PCG and GAMG.\"\nkind = \"port\"\nupstream = \"OpenFOAM lduMatrix\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics\"\ncrate = \"outram-foam-appbuilder-lib\"\nmodule = \"solvers/pimple_foam\"\ntitle = \"pimpleFoam / icoFoam\"\nwhat = \"Incompressible PISO/PIMPLE solver.\"\nkind = \"port\"\nupstream = \"OpenFOAM pimpleFoam\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics\"\ncrate = \"outram-foam-appbuilder-lib\"\nmodule = \"solvers/rho_pimple_foam\"\ntitle = \"rhoPimpleFoam\"\nwhat = \"Compressible transient PIMPLE solver.\"\nkind = \"port\"\nupstream = \"OpenFOAM rhoPimpleFoam\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics\"\ncrate = \"outram-foam-turbulence-lib\"\nmodule = \"k_omega_sst\"\ntitle = \"k-omega SST turbulence model\"\nwhat = \"Menter (1994) RAS model.\"\nkind = \"port\"\nupstream = \"OpenFOAM kOmegaSST\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics\"\ncrate = \"outram-foam-turbulence-lib\"\nmodule = \"wall_functions\"\ntitle = \"Turbulence wall functions\"\nwhat = \"Near-wall treatment for RAS models.\"\nkind = \"port\"\nupstream = \"OpenFOAM wall functions\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics\"\ncrate = \"outram-foam-appbuilder-lib\"\nmodule = \"genfoam/thermal_hydraulics\"\ntitle = \"GeN-Foam reactor thermal hydraulics\"\nwhat = \"Single- and two-phase porous-medium reactor thermal hydraulics.\"\nkind = \"port\"\nupstream = \"GeN-Foam thermalHydraulics\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics/mesh-generation\"\ncrate = \"outram-foam-mesh\"\nmodule = \"block_mesh\"\ntitle = \"blockMesh\"\nwhat = \"Structured hexahedral block meshing from a blockMeshDict.\"\nkind = \"port\"\nupstream = \"OpenFOAM blockMesh\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics/mesh-generation\"\ncrate = \"outram-foam-mesh\"\nmodule = \"snappy_hex_mesh\"\ntitle = \"snappyHexMesh\"\nwhat = \"Split-hex meshing around STL surfaces.\"\nkind = \"port\"\nupstream = \"OpenFOAM snappyHexMesh\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics/mesh-generation\"\ncrate = \"outram-park-fork-cfmesh\"\nmodule = \"pipeline\"\ntitle = \"cfMesh tet-dual pipeline\"\nwhat = \"Tetrahedralisation, polyhedral dual and boundary layers to a volume mesh.\"\nkind = \"port\"\nupstream = \"cfMesh (GPL-3.0)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics/mesh-generation\"\ncrate = \"outram-blender\"\nmodule = \"foam_mesh\"\ntitle = \"Volume-meshing bridge\"\nwhat = \"Blender surface mesh to cfMesh pipeline to OpenFOAM polyMesh.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics/mesh-generation\"\ncrate = \"outram-blender\"\nmodule = \"unstructured\"\ntitle = \"Neutral unstructured mesh\"\nwhat = \"One mesh description shared by the FV, FEM and Monte Carlo solvers (#492).\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/system-thermal-hydraulics\"\ncrate = \"tuas_boussinesq_solver\"\nmodule = \"lib/single_control_vol\"\ntitle = \"Single control-volume node\"\nwhat = \"The lumped thermal node and its node-to-node interactions.\"\nkind = \"new-work\"\nupstream = \"Ong, Xiao & Peterson (2025) (peer reviewed)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/system-thermal-hydraulics\"\ncrate = \"tuas_boussinesq_solver\"\nmodule = \"lib/array_fluid_collections\"\ntitle = \"Array control volumes and fluid networks\"\nwhat = \"Spatially resolved 1-D components and networks of them.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/two-phase-flow\"\ncrate = \"tampines\"\nmodule = \"multiphase_1d\"\ntitle = \"1-D two-phase system-code solvers\"\nwhat = \"Two-fluid and drift-flux 1-D solvers reduced from the 3-D reference.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/two-phase-flow\"\ncrate = \"outram-foam-multiphase\"\nmodule = \"two_fluid\"\ntitle = \"Euler-Euler two-fluid model\"\nwhat = \"Phase and interfacial-momentum-transfer foundation.\"\nkind = \"port\"\nupstream = \"OpenFOAM multiphaseEuler\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/two-phase-flow\"\ncrate = \"outram-foam-appbuilder-lib\"\nmodule = \"solvers/hrm_foam\"\ntitle = \"HRMFoam flashing flow\"\nwhat = \"Homogeneous relaxation model for flashing two-phase flow.\"\nkind = \"port\"\nupstream = \"HRMFoam\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/coolant-thermophysical-properties\"\ncrate = \"tampines-steam-tables\"\nmodule = \"interfaces\"\ntitle = \"IAPWS-IF97 steam tables\"\nwhat = \"Water and steam properties by region, with forward and backward equations.\"\nkind = \"new-work\"\nupstream = \"IAPWS-IF97 formulation\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/coolant-thermophysical-properties\"\ncrate = \"outram-park-fork-coolprop\"\nmodule = \"eos\"\ntitle = \"Helmholtz-energy equations of state\"\nwhat = \"Reduced Helmholtz energy and derivatives for ~120 fluids.\"\nkind = \"port\"\nupstream = \"CoolProp (MIT)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/coolant-thermophysical-properties\"\ncrate = \"outram-park-fork-coolprop\"\nmodule = \"flash\"\ntitle = \"Single-phase flashes\"\nwhat = \"(p,T), (p,h), (p,s) to a full fluid state.\"\nkind = \"port\"\nupstream = \"CoolProp\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/coolant-thermophysical-properties\"\ncrate = \"tuas_boussinesq_solver\"\nmodule = \"lib/boussinesq_thermophysical_properties\"\ntitle = \"Liquid and solid property library\"\nwhat = \"Salt, oil and solid thermophysical properties for Boussinesq models.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/pebble-bed-thermal-hydraulics\"\ncrate = \"tampines\"\nmodule = \"gas_phase/kta_bed\"\ntitle = \"KTA 3102.3 packed-bed pressure drop\"\nwhat = \"Gas pressure gradient through a randomly packed bed of spheres.\"\nkind = \"new-work\"\nupstream = \"KTA 3102.3 correlation\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/pebble-bed-thermal-hydraulics\"\ncrate = \"tampines\"\nmodule = \"pebble_bed/zbs\"\ntitle = \"Zehner-Bauer-Schlunder effective conductivity\"\nwhat = \"Analytic effective thermal conductivity of a packed bed.\"\nkind = \"new-work\"\nupstream = \"ZBS model as in KTA 3102.4\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/pebble-bed-thermal-hydraulics\"\ncrate = \"tampines\"\nmodule = \"pebble_bed/cht\"\ntitle = \"Pebble-bed conjugate heat transfer\"\nwhat = \"Pebble-to-coolant heat transfer in the nested conduction stack.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/pebble-bed-thermal-hydraulics\"\ncrate = \"outram-park-fork-liggghts\"\nmodule = \"thermal\"\ntitle = \"Thermal DEM contact conduction\"\nwhat = \"Particle-particle and particle-wall contact conduction.\"\nkind = \"port\"\nupstream = \"LIGGGHTS heat transfer\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/pebble-bed-thermal-hydraulics\"\ncrate = \"outram-park-fork-liggghts\"\nmodule = \"thermal_radiation\"\ntitle = \"Radiative and gas-gap heat transfer between particles\"\nwhat = \"Particle-scale radiation and near-field gas-gap conduction.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/pebble-bed-thermal-hydraulics\"\ncrate = \"tuas_boussinesq_solver\"\nmodule = \"lib/pre_built_components/insulated_porous_media_fluid_components\"\ntitle = \"Porous-media fluid components\"\nwhat = \"Fluid flowing through a porous solid matrix (packed bed).\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/neutronics-thermal-hydraulics-coupling\"\ncrate = \"outram-foam-appbuilder-lib\"\nmodule = \"genfoam/multi_region\"\ntitle = \"Multi-mesh coupling\"\nwhat = \"Couples the neutronics, TH and thermo-mechanics meshes with outer iterations.\"\nkind = \"port\"\nupstream = \"GeN-Foam multiRegion\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/neutronics-thermal-hydraulics-coupling\"\ncrate = \"nee_soon\"\nmodule = \"coupling\"\ntitle = \"Monte Carlo to GeN-Foam via MGXS\"\nwhat = \"Carries one reactor model from Monte Carlo to a deterministic solve on the same geometry.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/neutronics-thermal-hydraulics-coupling\"\ncrate = \"nee_soon\"\nmodule = \"direct_coupling\"\ntitle = \"Monte Carlo directly against GeN-Foam thermal hydraulics\"\nwhat = \"outram-mc as the neutronics solver in the coupled loop.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/neutronics-thermal-hydraulics-coupling\"\ncrate = \"outram-park-fork-moltres\"\nmodule = \"thermal\"\ntitle = \"Reduced fuel-salt thermal model and feedback loop\"\nwhat = \"Power/temperature feedback coupling for the circulating-fuel solve.\"\nkind = \"port\"\nupstream = \"Moltres (formulation)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/neutronics-thermal-hydraulics-coupling\"\ncrate = \"bedok\"\nmodule = \"thdiffusion_solverxyz\"\ntitle = \"Coupled TH and nodal diffusion\"\nwhat = \"Steady thermal-hydraulics coupled to the 3-D diffusion solve.\"\nkind = \"port\"\nupstream = \"BEDOK MATLAB\"\nstatus = \"proposed\"\n\n# ---- reactor coolant system ----------------------------------------------\n\n[[implementation]]\nconcept = \"02-nuclear-safety/reactor-coolant-system/residual-heat-removal\"\ncrate = \"tuas_boussinesq_solver\"\nmodule = \"lib/pre_built_components/ciet_three_branch_plus_dracs\"\ntitle = \"CIET primary loop plus DRACS\"\nwhat = \"Three-branch primary loop coupled to the passive DRACS decay-heat removal loop.\"\nkind = \"new-work\"\nupstream = \"Ong, Xiao & Peterson (2025) (peer reviewed)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/reactor-coolant-system/steam-generators\"\ncrate = \"tampines\"\nmodule = \"components/helical_coil_steam_generator\"\ntitle = \"Helical-coil once-through steam generator\"\nwhat = \"Spatially resolved counter-flow hot fluid, tube metal and water/steam.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/reactor-coolant-system/primary-heat-exchangers\"\ncrate = \"tuas_boussinesq_solver\"\nmodule = \"lib/pre_built_components/shell_and_tube_heat_exchanger\"\ntitle = \"Shell-and-tube heat exchanger\"\nwhat = \"Single-pass parallel-flow shell-and-tube model.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/reactor-coolant-system/primary-heat-exchangers\"\ncrate = \"tampines\"\nmodule = \"components/heat_exchanger\"\ntitle = \"TAMPINES heat exchanger\"\nwhat = \"Heat exchanger component of the TH framework.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/reactor-coolant-system/coolant-pumps-and-circulators\"\ncrate = \"tampines\"\nmodule = \"gas_phase/circulator\"\ntitle = \"Idealised helium circulator\"\nwhat = \"Pressure-raising machine of a gas-cooled primary circuit.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n# ---- containment ---------------------------------------------------------\n\n[[implementation]]\nconcept = \"02-nuclear-safety/containment/functional-containment\"\ncrate = \"bishan\"\nmodule = \"building\"\ntitle = \"HTR-10 vented confinement as one control volume\"\nwhat = \"The reactor building\'s lumped response, the outermost barrier of functional containment.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n# ---- I&C ------------------------------------------------------------------\n\n[[implementation]]\nconcept = \"02-nuclear-safety/instrumentation-and-control/control-systems\"\ncrate = \"chem-eng-real-time-process-control-simulator\"\nmodule = \"lib/stable\"\ntitle = \"Transfer functions and PID control\"\nwhat = \"Real-time process-control blocks (first/second-order, PID).\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/instrumentation-and-control/control-systems\"\ncrate = \"petir\"\nmodule = \"transfer_fn\"\ntitle = \"Transfer-function numerics\"\nwhat = \"Continuous and discrete transfer functions (Octave-derived).\"\nkind = \"port\"\nupstream = \"GNU Octave\"\nstatus = \"proposed\"\n\n# ---- steam and power conversion ------------------------------------------\n\n[[implementation]]\nconcept = \"02-nuclear-safety/steam-and-power-conversion/turbine-generator\"\ncrate = \"tampines\"\nmodule = \"components/turbine\"\ntitle = \"Steam turbine\"\nwhat = \"Turbine component of the balance of plant.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/steam-and-power-conversion/turbine-generator\"\ncrate = \"tampines-steam-tables\"\nmodule = \"steam_turbine_equations\"\ntitle = \"Steam-turbine equations\"\nwhat = \"Expansion-line relations on IF97 properties.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/steam-and-power-conversion/main-condensers\"\ncrate = \"tampines\"\nmodule = \"components/condenser\"\ntitle = \"Condenser\"\nwhat = \"Condenser component of the balance of plant.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/steam-and-power-conversion/circulating-water-system\"\ncrate = \"tampines\"\nmodule = \"cooling_tower\"\ntitle = \"Cooling tower (scaffold)\"\nwhat = \"Intended home of the Merkel / effectiveness-NTU cooling-tower engine; scaffold only.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/steam-and-power-conversion/power-cycle-flowsheet-simulation\"\ncrate = \"tampines\"\nmodule = \"balance_of_plant\"\ntitle = \"Balance of plant and Rankine cycle\"\nwhat = \"System-level assembly of BOP components.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/steam-and-power-conversion/power-cycle-flowsheet-simulation\"\ncrate = \"outram-park-fork-dwsim-libs\"\nmodule = \"heat_exchanger\"\ntitle = \"Heat exchanger rating\"\nwhat = \"LMTD, effectiveness-NTU, multi-pass correction and Tinker\'s method.\"\nkind = \"port\"\nupstream = \"DWSIM\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/steam-and-power-conversion/power-cycle-flowsheet-simulation\"\ncrate = \"outram-park-fork-dwsim-libs\"\nmodule = \"compressor\"\ntitle = \"Compressor\"\nwhat = \"Compressor unit operation.\"\nkind = \"port\"\nupstream = \"DWSIM\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/steam-and-power-conversion/power-cycle-flowsheet-simulation\"\ncrate = \"outram-park-fork-dwsim-libs\"\nmodule = \"expander\"\ntitle = \"Expander (isentropic)\"\nwhat = \"Expander/turbine unit operation.\"\nkind = \"port\"\nupstream = \"DWSIM\"\nstatus = \"proposed\"\n\n# ---- accident analysis, severe accidents, source terms --------------------\n\n[[implementation]]\nconcept = \"02-nuclear-safety/accident-analysis/reactivity-and-power-distribution-anomalies\"\ncrate = \"nee_soon\"\nmodule = \"xin_wang_sp3_workflow\"\ntitle = \"Control-rod-removal transient (Xin Wang SP3 workflow)\"\nwhat = \"Four-stage multiphysics pipeline reproducing a published rod-removal transient.\"\nkind = \"new-work\"\nupstream = \"Xin Wang (2018) UC Berkeley dissertation, Fig. 4.29\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/accident-analysis/decrease-in-reactor-coolant-inventory\"\ncrate = \"tampines\"\nmodule = \"critical_flow\"\ntitle = \"Choked two-phase flow\"\nwhat = \"HEM critical flow for blowdown (Edwards, Marviken).\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/accident-analysis/decrease-in-reactor-coolant-inventory\"\ncrate = \"sembawang\"\nmodule = \"htr10\"\ntitle = \"HTR-10 depressurised loss of forced cooling\"\nwhat = \"Joins boon-lay fuel failure and TRISO-ATOPS release on the HTR-10 DLOFC.\"\nkind = \"new-work\"\ncross_links = [\"13-environmental-protection/postulated-accident-impacts\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/severe-accidents/probabilistic-risk-assessment\"\ncrate = \"raffles\"\nmodule = \"scram\"\ntitle = \"Fault-tree quantification\"\nwhat = \"Minimal cut sets, BDD/ZBDD, probability and importance measures.\"\nkind = \"port\"\nupstream = \"SCRAM (rakhimov/scram)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/source-terms/coolant-source-terms\"\ncrate = \"boon-lay\"\nmodule = \"triso_atops_fork/activities\"\ntitle = \"Coolant activity and source terms\"\nwhat = \"Circulating and plate-out activity in the primary coolant.\"\nkind = \"port\"\nupstream = \"INL TRISO-ATOPS\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/source-terms/accident-source-terms\"\ncrate = \"sembawang\"\nmodule = \"accident\"\ntitle = \"Accident-phase release over TRISO-ATOPS\"\nwhat = \"Prescribed transient plus inventory out to a source term; no release physics of its own.\"\nkind = \"new-work\"\ncross_links = [\"13-environmental-protection/postulated-accident-impacts\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/source-terms/accident-source-terms\"\ncrate = \"sembawang\"\nmodule = \"chain\"\ntitle = \"Source term handed to changi\"\nwhat = \"Passes the released source term on for dispersion and deposition.\"\nkind = \"new-work\"\ncross_links = [\"13-environmental-protection/postulated-accident-impacts\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/source-terms/fission-product-inventory\"\ncrate = \"sembawang\"\nmodule = \"inventory\"\ntitle = \"Prescribed core inventory\"\nwhat = \"Core radionuclide inventory taken as a cited input, not computed.\"\nkind = \"new-work\"\ncross_links = [\"13-environmental-protection/postulated-accident-impacts\"]\nstatus = \"proposed\"\n\n# ---- quality assurance ----------------------------------------------------\n\n[[implementation]]\nconcept = \"07-regulatory-framework/quality-assurance/software-quality-assurance/verification-and-validation-records\"\ncrate = \"njoy-outram-park-fork\"\nmodule = \"vv\"\ntitle = \"V&V gate helpers\"\nwhat = \"Shared oracle-comparison gates used by njoy and outram-mc.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"07-regulatory-framework/quality-assurance/software-quality-assurance/verification-and-validation-records\"\ncrate = \"outram-mc-libs\"\nmodule = \"vv\"\ntitle = \"outram-mc V&V gates and oracle tables\"\nwhat = \"Committed oracle tables (NJOY golden values, graphite) the crate is measured against.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"07-regulatory-framework/quality-assurance/software-quality-assurance/configuration-and-accounting-records\"\ncrate = \"kovan-metrics\"\nmodule = \"historian\"\ntitle = \"Historian report\"\nwhat = \"Pre-merge accounting report for develop to main.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"07-regulatory-framework/quality-assurance/software-quality-assurance/configuration-and-accounting-records\"\ncrate = \"kovan-metrics\"\nmodule = \"trailer\"\ntitle = \"Commit token trailers\"\nwhat = \"Per-commit API-usage trailers.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"07-regulatory-framework/quality-assurance/software-quality-assurance/code-review\"\ncrate = \"kovan\"\nmodule = \"commands/code_walk\"\ntitle = \"Code walks\"\nwhat = \"Call chains from an entry point to the function implementing a concept.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n# ---- radiation protection -------------------------------------------------\n\n[[implementation]]\nconcept = \"08-radiation-protection/radiation-protection/shielding\"\ncrate = \"outram-mc-libs\"\nmodule = \"physics/weight_windows\"\ntitle = \"Weight windows for deep penetration\"\nwhat = \"Variance reduction that makes shielding problems tractable.\"\nkind = \"port\"\nupstream = \"OpenMC src/weight_windows.cpp\"\nstatus = \"proposed\"\n\n# ---- knowledge management and education ----------------------------------\n\n[[implementation]]\nconcept = \"10-human-resource-development/knowledge-management-and-education/knowledge-management\"\ncrate = \"kovan\"\nmodule = \"corpus\"\ntitle = \"Built-in nuclear-engineering corpus\"\nwhat = \"The curated standard corpus compiled into Kovan.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"10-human-resource-development/knowledge-management-and-education/knowledge-management\"\ncrate = \"kovan\"\nmodule = \"mindmap\"\ntitle = \"Interactive mind map\"\nwhat = \"The concept map, Kovan\'s home view.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"10-human-resource-development/knowledge-management-and-education/knowledge-management\"\ncrate = \"kovan\"\nmodule = \"classify\"\ntitle = \"Fine-grained classification\"\nwhat = \"Classification of literature below paper level.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"10-human-resource-development/knowledge-management-and-education/knowledge-management\"\ncrate = \"kovan-literature\"\nmodule = \"pdf_import\"\ntitle = \"PDF ingestion\"\nwhat = \"PDF to Markdown, metadata and BibTeX for the archive.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"10-human-resource-development/knowledge-management-and-education/education-and-outreach\"\ncrate = \"dhoby-ghaut\"\nmodule = \"web_demo\"\ntitle = \"Web-demo framework\"\nwhat = \"The track-independent half of every browser tutorial demo.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"10-human-resource-development/knowledge-management-and-education/education-and-outreach\"\ncrate = \"dhoby-ghaut\"\nmodule = \"workbench\"\ntitle = \"Guided simulation workbench\"\nwhat = \"The wizard that walks a learner through a high-fidelity simulation.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n# ---- site characteristics -------------------------------------------------\n\n[[implementation]]\nconcept = \"12-site-and-supporting-facilities/site-characteristics/hydrologic-engineering\"\ncrate = \"outram-park-fork-pflotran\"\nmodule = \"flow/richards\"\ntitle = \"Richards variably saturated groundwater flow\"\nwhat = \"Liquid-phase mass conservation in the subsurface.\"\nkind = \"port\"\nupstream = \"PFLOTRAN RICHARDS mode\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"12-site-and-supporting-facilities/site-characteristics/hydrologic-engineering\"\ncrate = \"outram-park-fork-pflotran\"\nmodule = \"reactive_transport\"\ntitle = \"Reactive transport\"\nwhat = \"Operator-split solute transport with equilibrium geochemistry.\"\nkind = \"port\"\nupstream = \"PFLOTRAN\"\nstatus = \"proposed\"\n\n# ---- environmental protection: dispersion and dose -------------------------\n\n[[implementation]]\nconcept = \"13-environmental-protection/meteorology-and-air-quality/short-term-accident-dispersion\"\ncrate = \"changi\"\nmodule = \"flexpart\"\ntitle = \"FLEXPART Lagrangian particle dispersion\"\nwhat = \"Advection, turbulence, convection and deposition of computational particles.\"\nkind = \"port\"\nupstream = \"FLEXPART v10.4 (GPL-3.0)\"\naspirations = [\"14-emergency-planning/accident-assessment\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"13-environmental-protection/meteorology-and-air-quality/short-term-accident-dispersion\"\ncrate = \"changi\"\nmodule = \"puff\"\ntitle = \"Gaussian puff forward dispersion\"\nwhat = \"A continuous release as a train of Gaussian puffs.\"\nkind = \"port\"\nupstream = \"puff R package 0.1.1 (MIT)\"\naspirations = [\"14-emergency-planning/accident-assessment\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"13-environmental-protection/meteorology-and-air-quality/short-term-accident-dispersion\"\ncrate = \"changi\"\nmodule = \"activity/accident_airborne_release\"\ntitle = \"Published HTR-10 accident airborne release\"\nwhat = \"Design-basis-accident release stored as cited reference data.\"\nkind = \"new-work\"\naspirations = [\"14-emergency-planning/accident-assessment\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"13-environmental-protection/meteorology-and-air-quality/long-term-routine-dispersion\"\ncrate = \"buangkok\"\nmodule = \"pydoseia/dispersion\"\ntitle = \"Gaussian-plume dilution for routine releases\"\nwhat = \"pyDOSEIA\'s plume dilution factors.\"\nkind = \"port\"\nupstream = \"pyDOSEIA\"\naspirations = [\"14-emergency-planning/accident-assessment\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"13-environmental-protection/meteorology-and-air-quality/relative-concentration-and-deposition\"\ncrate = \"changi\"\nmodule = \"activity/chi_over_q\"\ntitle = \"Unit-release dilution factors by travel time\"\nwhat = \"chi/Q from a unit release, binned by travel time.\"\nkind = \"new-work\"\naspirations = [\"14-emergency-planning/accident-assessment\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"13-environmental-protection/meteorology-and-air-quality/relative-concentration-and-deposition\"\ncrate = \"changi\"\nmodule = \"flexpart/dry_deposition\"\ntitle = \"Dry deposition\"\nwhat = \"Resistance model for gases and particle deposition velocity.\"\nkind = \"port\"\nupstream = \"FLEXPART v10.4\"\naspirations = [\"14-emergency-planning/accident-assessment\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"13-environmental-protection/meteorology-and-air-quality/relative-concentration-and-deposition\"\ncrate = \"changi\"\nmodule = \"flexpart/wet_deposition\"\ntitle = \"Wet deposition\"\nwhat = \"Below- and in-cloud scavenging and mass loss.\"\nkind = \"port\"\nupstream = \"FLEXPART v10.4\"\naspirations = [\"14-emergency-planning/accident-assessment\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"13-environmental-protection/meteorology-and-air-quality/relative-concentration-and-deposition\"\ncrate = \"changi\"\nmodule = \"flexpart/decay\"\ntitle = \"Radioactive decay in transit\"\nwhat = \"Per-species decay of airborne and deposited activity.\"\nkind = \"port\"\nupstream = \"FLEXPART v10.4\"\naspirations = [\"14-emergency-planning/accident-assessment\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"13-environmental-protection/radiological-impacts-of-normal-operation/exposure-pathways\"\ncrate = \"buangkok\"\nmodule = \"pydoseia/ingestion\"\ntitle = \"Terrestrial food-chain ingestion pathway\"\nwhat = \"IAEA SRS 19 screening equations and H-3/C-14 specific-activity models.\"\nkind = \"port\"\nupstream = \"pyDOSEIA\"\naspirations = [\"14-emergency-planning/accident-assessment\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"13-environmental-protection/radiological-impacts-of-normal-operation/exposure-pathways\"\ncrate = \"buangkok\"\nmodule = \"pydoseia/plume_shine\"\ntitle = \"Plume shine\"\nwhat = \"External gamma dose from the passing cloud (finite-cloud model).\"\nkind = \"port\"\nupstream = \"pyDOSEIA\"\naspirations = [\"14-emergency-planning/accident-assessment\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"13-environmental-protection/radiological-impacts-of-normal-operation/doses-to-members-of-the-public\"\ncrate = \"buangkok\"\nmodule = \"pydoseia/dose\"\ntitle = \"Five-pathway dose\"\nwhat = \"Dose to members of the public summed over pathways (research use only).\"\nkind = \"port\"\nupstream = \"pyDOSEIA\"\naspirations = [\"14-emergency-planning/accident-assessment\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"13-environmental-protection/radiological-impacts-of-normal-operation/doses-to-members-of-the-public\"\ncrate = \"buangkok\"\nmodule = \"coefficients\"\ntitle = \"EPA Federal Guidance Report dose coefficients\"\nwhat = \"FGR-11/13/15 coefficients for the tracked nuclides.\"\nkind = \"new-work\"\nupstream = \"US EPA FGR-11, FGR-13, FGR-15\"\naspirations = [\"14-emergency-planning/accident-assessment\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"13-environmental-protection/postulated-accident-impacts/design-basis-accident-consequences\"\ncrate = \"buangkok\"\nmodule = \"published/accident_dose_by_distance\"\ntitle = \"Published accident dose by distance\"\nwhat = \"Cited reference dose tables; computes nothing.\"\nkind = \"new-work\"\naspirations = [\"14-emergency-planning/accident-assessment\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n# ---- nuclear fuel cycle ---------------------------------------------------\n\n[[implementation]]\nconcept = \"16-nuclear-fuel-cycle/nuclear-criticality-safety/calculational-method-validation\"\ncrate = \"outram-mc-libs\"\nmodule = \"vv\"\ntitle = \"ICSBEP and oracle validation of the transport code\"\nwhat = \"Benchmark gates that validate the criticality method.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"18-industrial-involvement/process-heat-and-industrial-applications/chemical-process-simulation\"\ncrate = \"outram-park-fork-dwsim-libs\"\nmodule = \"flowsheet_solver\"\ntitle = \"Sequential-modular flowsheet solver\"\nwhat = \"Ordering, recycle and spec handling for process flowsheets.\"\nkind = \"port\"\nupstream = \"DWSIM\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"18-industrial-involvement/process-heat-and-industrial-applications/chemical-process-simulation\"\ncrate = \"outram-park-fork-dwsim-libs\"\nmodule = \"reactors\"\ntitle = \"Chemical reactor unit operations\"\nwhat = \"Conversion, equilibrium, Gibbs, CSTR and PFR reactors.\"\nkind = \"port\"\nupstream = \"DWSIM.UnitOperations/Reactors\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"18-industrial-involvement/process-heat-and-industrial-applications/chemical-process-simulation\"\ncrate = \"outram-park-fork-dwsim-libs\"\nmodule = \"columns\"\ntitle = \"Rigorous distillation and absorption columns\"\nwhat = \"MESH column models and solvers.\"\nkind = \"port\"\nupstream = \"DWSIM (commit 1abf72d)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"18-industrial-involvement/process-heat-and-industrial-applications/chemical-process-simulation\"\ncrate = \"outram-park-fork-dwsim-libs\"\nmodule = \"dynamics\"\ntitle = \"Dynamic flowsheet simulation\"\nwhat = \"Schedules, integrators, events and cause-and-effect matrices.\"\nkind = \"port\"\nupstream = \"DWSIM\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"18-industrial-involvement/process-heat-and-industrial-applications/chemical-process-simulation\"\ncrate = \"dover\"\nmodule = \"smr\"\ntitle = \"Steam-methane-reforming CSTR deck\"\nwhat = \"Deck-driven SMR reactor model on the DWSIM fork.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"16-nuclear-fuel-cycle/fuel-depletion/depletion-solvers\"\ncrate = \"outram-mc-libs\"\nmodule = \"depletion\"\ntitle = \"Monte Carlo burnup loop\"\nwhat = \"Bateman evolution coupled to transport reaction rates.\"\nkind = \"port\"\nupstream = \"OpenMC openmc.deplete\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"16-nuclear-fuel-cycle/fuel-depletion/depletion-solvers\"\ncrate = \"outram-park-fork-onix\"\nmodule = \"cram\"\ntitle = \"CRAM matrix exponential\"\nwhat = \"Chebyshev rational approximation of exp(A dt) for depletion.\"\nkind = \"port\"\nupstream = \"ONIX (MIT)\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"16-nuclear-fuel-cycle/fuel-depletion/depletion-solvers\"\ncrate = \"boon-lay\"\nmodule = \"lagrangian_transmutation_and_fission_simulator\"\ntitle = \"Lagrangian Monte Carlo transmutation\"\nwhat = \"Competing-rate depletion without a burnup matrix.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"16-nuclear-fuel-cycle/fuel-depletion/decay-data\"\ncrate = \"boon-lay\"\nmodule = \"nuclide_reaction_and_decay_data\"\ntitle = \"Nuclide decay library\"\nwhat = \"Decay data by element group and its parsing.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"16-nuclear-fuel-cycle/fuel-depletion/decay-data\"\ncrate = \"boon-lay\"\nmodule = \"lagrangian_decay_simulator\"\ntitle = \"Lagrangian decay simulator\"\nwhat = \"Monte Carlo decay of single radionuclides and chains.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"16-nuclear-fuel-cycle/fuel-depletion/irradiation-history\"\ncrate = \"outram-park-fork-offbeat\"\nmodule = \"burnup\"\ntitle = \"Burnup and fast-fluence accumulation\"\nwhat = \"Irradiation-history bookkeeping of a fuel-performance run.\"\nkind = \"port\"\nupstream = \"OFFBEAT\"\nstatus = \"proposed\"\n\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/air-and-moisture-ingress\"\ntitle = \"Air and moisture (water) ingress\"\norigin = \"nrc\"\nsources = [{ document = \"rg-1.232-rev0\", section = \"Appendix C, MHTGR-DC 14 (reactor helium pressure boundary: unacceptable ingress of moisture, air, secondary coolant); MHTGR-DC 30 (means of detecting ingress)\" }]\nstatus = \"approved\"\n\n[[implementation]]\nconcept = \"02-nuclear-safety/accident-analysis/air-and-moisture-ingress\"\ncrate = \"boon-lay\"\nmodule = \"chemistry\"\ntitle = \"Graphite and fuel chemical attack\"\nwhat = \"IG-110 graphite oxidation by steam and air, and UO2 kernel hydrolysis: cited closed-form rate laws used by htgr_sim_v1\'s water-ingress stage.\"\nkind = \"new-work\"\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"16-nuclear-fuel-cycle/fuel-cycle-scenarios\"\ncrate = \"kaki-bukit\"\nmodule = \"agents\"\ntitle = \"Agent-based fuel-cycle simulation\"\nwhat = \"Facilities as agents exchanging material over time: fuel-cycle flows and inventories.\"\nkind = \"port\"\nupstream = \"CYCLUS\"\nstatus = \"proposed\"\n\n\n[[implementation]]\nconcept = \"10-human-resource-development/knowledge-management-and-education/education-and-outreach\"\ncrate = \"outram-park-digital-twin-engine\"\nmodule = \"app_scaffold\"\ntitle = \"Digital-twin engine: offline educational plant simulators\"\nwhat = \"The egui simulator framework behind htgr_sim_v1, fhr_sim_v2 and distillation_sim_v1: offline demonstrations only (RESPONSIBLE_USE.md).\"\nkind = \"new-work\"\naspirations = [\"02-nuclear-safety/conduct-of-operations/operator-training-and-requalification\", \"02-nuclear-safety/instrumentation-and-control/information-systems-and-displays\", \"02-nuclear-safety/accident-analysis/transient-and-accident-analysis-methods\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[implementation]]\nconcept = \"10-human-resource-development/knowledge-management-and-education/education-and-outreach\"\ncrate = \"outram-park-digital-twin-engine\"\nmodule = \"htr10\"\ntitle = \"Digital-twin engine: HTR-10 plant model\"\nwhat = \"The HTR-10 plant model the htgr_sim_v1 simulator runs: offline, educational.\"\nkind = \"new-work\"\naspirations = [\"02-nuclear-safety/conduct-of-operations/operator-training-and-requalification\", \"02-nuclear-safety/instrumentation-and-control/information-systems-and-displays\", \"02-nuclear-safety/accident-analysis/transient-and-accident-analysis-methods\"]\naspiration_prerequisites = [\"validation\", \"risk assessment\", \"licensing\"]\nstatus = \"proposed\"\n\n[[concept]]\npath = \"01-national-position/need-for-power/power-system-description\"\ntitle = \"Description of power system\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1555\", section = \"8.1\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"01-national-position/need-for-power/power-demand\"\ntitle = \"Power demand (power and energy requirements; factors affecting growth of demand)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1555\", section = \"8.2 (8.2.1, 8.2.2)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"01-national-position/need-for-power/power-supply\"\ntitle = \"Power supply\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1555\", section = \"8.3\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"01-national-position/need-for-power/need-for-power-assessment\"\ntitle = \"Assessment of need for power\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1555\", section = \"8.4\" }]\nstatus = \"approved\"\n\n# DEFERRED until a citation is in the corpus (maintainer, 2026-10-06), under\n# 01-national-position, beside need-for-power:\n#   - industrial process (process heat / non-electric applications)\n#   - medical isotope production\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/core-configurations\"\ntitle = \"Core configurations by reactor type\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.5.1 (core configurations)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/core-configurations/prismatic\"\ntitle = \"Prismatic (block-type) VHTR cores\"\norigin = \"nrc\"\nsources = [{ document = \"rg-1.232-rev0\", section = \"Appendix C (MHTGR-DC)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/core-configurations/pebble-bed\"\ntitle = \"Pebble-bed VHTR cores\"\norigin = \"nrc\"\nsources = [{ document = \"rg-1.232-rev0\", section = \"Appendix C (MHTGR-DC)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/core-configurations/liquid-fuelled\"\ntitle = \"Liquid-fuelled cores (MSR)\"\norigin = \"nrc\"\nsources = [{ document = \"ornl-tm-2018-976\", section = \"2 (homogeneous fuel; fuel salt boundary as the first fission-product barrier)\" }]\ncross_links = [\"02-nuclear-safety/thermal-hydraulic-design/gas-generation-and-entrainment\", \"02-nuclear-safety/msr-coolant-loop/coolant-loop-materials-and-chemistry\"]\nnote = \"For MSRs, thermal hydraulics becomes more of a feedback and structural-materials issue (maintainer, 2026-10-06): hence its links to gas generation, coolant-loop materials and (via precursor drift) reactivity.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/core-configurations/liquid-metal-cooled\"\ntitle = \"Liquid-metal-cooled fast reactor cores (SFR, LFR)\"\norigin = \"nrc\"\nsources = [{ document = \"rg-1.232-rev0\", section = \"Appendix B (SFR-DC); Appendix A (ARDC)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/core-configurations/gas-cooled-fast\"\ntitle = \"Gas-cooled fast reactor cores (GFR)\"\norigin = \"nrc\"\nsources = [{ document = \"rg-1.232-rev0\", section = \"Appendix A (ARDC, developed with GFRs in view)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/core-configurations/supercritical-water-cooled\"\ntitle = \"Supercritical-water-cooled reactor cores (SCWR)\"\norigin = \"nrc\"\nsources = []\nnote = \"No NRC guidance specific to SCWR (RG 1.232 covers non-LWRs only); world reference to obtain: Generation IV International Forum SCWR documents.\"\nstatus = \"deferred\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods\"\ntitle = \"Thermal-hydraulic analysis methods\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-1537-part1\", section = \"4.6 (a detailed description of the analytical methods used in the thermal-hydraulic design)\" }, { document = \"ornl-tm-2018-976\", section = \"3.1.3 (acceptable analytical methods)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/gas-generation-and-entrainment\"\ntitle = \"Gas generation and entrainment (MSR flow-instability mechanisms)\"\norigin = \"nrc\"\nsources = [{ document = \"ornl-tm-2018-976\", section = \"3.1.3 (LWR DNB/CHF/CPR measures are not applicable to MSR technology, \'but other mechanisms may exist, such as gas generation or entrainment\')\" }]\ncross_links = [\"02-nuclear-safety/nuclear-design/reactivity-coefficients\", \"02-nuclear-safety/msr-coolant-loop/coolant-loop-materials-and-chemistry\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/thermal-hydraulic-design/experimental-loops-and-test-reactor-support\"\ntitle = \"Experimental loops and test-reactor support for thermal-hydraulic design (MSR)\"\norigin = \"nrc\"\nsources = [{ document = \"ornl-tm-2018-976\", section = \"3.1.3 (justified extrapolation from proven designs \'will rely heavily on experimental loops and perhaps a test reactor\')\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/steam-and-power-conversion/alternate-power-cycles\"\ntitle = \"Alternate (non-steam) power conversion cycles\"\norigin = \"nrc\"\nsources = [{ document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 4 rationale (very high-speed, very high-energy gas turbines inside the reactor helium pressure boundary)\" }, { document = \"jrc-eur-28712\", section = \"power conversion: direct-cycle helium gas turbine (Brayton); combined cycles\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/steam-and-power-conversion/alternate-power-cycles/direct-cycle-helium-brayton\"\ntitle = \"Direct-cycle helium gas turbine (Brayton)\"\norigin = \"nrc\"\nsources = [{ document = \"rg-1.232-rev0\", section = \"App. C, MHTGR-DC 4 rationale\" }, { document = \"jrc-eur-28712\", section = \"Brayton cycle with a gas turbine placed directly in the hot gas\" }]\ncross_links = [\"02-nuclear-safety/reactor-coolant-system/coolant-pumps-and-circulators\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/steam-and-power-conversion/alternate-power-cycles/combined-cycles\"\ntitle = \"Combined cycles (helium / gas mixtures)\"\norigin = \"nrc\"\nsources = [{ document = \"jrc-eur-28712\", section = \"power conversion options table (He/mixture: combined cycles)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/steam-and-power-conversion/alternate-power-cycles/supercritical-co2-cycles\"\ntitle = \"Supercritical CO2 power cycles\"\norigin = \"nrc\"\nsources = []\nnote = \"No source in the corpus yet; deferred until literature is supplied.\"\nstatus = \"deferred\"\n\n[[concept]]\npath = \"02-nuclear-safety/steam-and-power-conversion/alternate-power-cycles/air-brayton-combined-cycle\"\ntitle = \"Air-Brayton combined cycle (FHR)\"\norigin = \"nrc\"\nsources = []\nnote = \"No source in the corpus yet; deferred until literature is supplied.\"\nstatus = \"deferred\"\n\n[[concept]]\npath = \"07-regulatory-framework/reactor-licensing/two-step-licensing\"\ntitle = \"Two-step licensing: construction permit, then operating license\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"Part 50\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1300 Construction permits\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1360 Operating licenses\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/reactor-licensing/one-step-licensing\"\ntitle = \"Design certification, early site permit and combined license\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr52\", section = \"Part 52\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1140 Early site permits\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1230 Standard design certifications\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1410 Combined licenses\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/reactor-licensing/risk-informed-technology-inclusive-framework\"\ntitle = \"Risk-informed, technology-inclusive framework (advanced reactors)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"Part 53, Subparts A-J, M\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/reactor-licensing/general-design-criteria\"\ntitle = \"General design criteria (and their non-LWR adaptations)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"Appendix A, General Design Criteria for Nuclear Power Plants\" }, { document = \"rg-1.232-rev0\", section = \"Appendices A-C (ARDC, SFR-DC, MHTGR-DC)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"08-radiation-protection/radiation-protection/shielding/radiation-transport\"\ntitle = \"Radiation transport for shielding (neutrons, photons, coupled, charged particles)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"12.3-12.4 Radiation Protection Design Features\" }, { document = \"nureg-1537-part1\", section = \"4.4 Biological Shield\" }]\nwhy = \"maintainer 2026-10-06: shielding needs radiation transport generally, not neutron transport alone.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"08-radiation-protection/radiation-protection/shielding/radiation-transport/neutron-shielding-transport\"\ntitle = \"Neutron transport for shielding (deep penetration, fixed source)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"12.3-12.4 Radiation Protection Design Features\" }, { document = \"nureg-1537-part1\", section = \"4.4 Biological Shield\" }]\nwhy = \"fixed-source, deep-penetration neutron transport; shares methods with nuclear design.\"\ncross_links = [\"02-nuclear-safety/nuclear-design/neutron-transport\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"08-radiation-protection/radiation-protection/shielding/radiation-transport/photon-transport\"\ntitle = \"Photon (gamma) transport\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"12.3-12.4 Radiation Protection Design Features\" }, { document = \"nureg-1537-part1\", section = \"4.4 Biological Shield\" }]\nwhy = \"gamma shielding and dose; needs photo-atomic data (njoy\'s photo-atomic ACE class) and photon collision physics.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"08-radiation-protection/radiation-protection/shielding/radiation-transport/coupled-neutron-photon-transport\"\ntitle = \"Coupled neutron-photon transport (secondary gammas)\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"12.3-12.4 Radiation Protection Design Features\" }, { document = \"nureg-1537-part1\", section = \"4.4 Biological Shield\" }]\nwhy = \"capture and inelastic gammas produced by neutrons; the usual reactor-shielding calculation.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"08-radiation-protection/radiation-protection/shielding/radiation-transport/charged-particle-transport\"\ntitle = \"Charged-particle (electron, positron, ion) transport\"\norigin = \"outram-park\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"12.3-12.4 Radiation Protection Design Features\" }, { document = \"nureg-1537-part1\", section = \"4.4 Biological Shield\" }]\nwhy = \"bremsstrahlung and energy deposition; where detector and medical codes (e.g. Geant4) lead.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"18-industrial-involvement/process-heat-and-industrial-applications/nuclear-techniques-for-plastic-pollution\"\ntitle = \"Nuclear techniques against plastic pollution (radiation-assisted recycling; isotopic tracing of marine microplastics)\"\norigin = \"iaea\"\nsources = [{ document = \"iaea-nutec-plastics\", section = \"whole document (8 pp.)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/licensing-basis-events\"\ntitle = \"Licensing-basis events (anticipated, unlikely and very unlikely event sequences, and design-basis accidents)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.240 Licensing-basis events\" }, { document = \"10cfr53\", section = \"\u{a7} 53.450 Analysis requirements, (e) Analyses of licensing-basis events other than design-basis accidents; (f) Analysis of design-basis accidents\" }, { document = \"10cfr53\", section = \"\u{a7} 53.020 Definitions (Licensing-basis events; Design-basis accidents)\" }]\ncross_links = [\"02-nuclear-safety/accident-analysis/maximum-hypothetical-accident\", \"02-nuclear-safety/accident-analysis/reactivity-and-power-distribution-anomalies\", \"02-nuclear-safety/severe-accidents/probabilistic-risk-assessment\", \"02-nuclear-safety/accident-analysis/transient-and-accident-analysis-methods\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/licensing-basis-events/safety-criteria-for-design-basis-accidents\"\ntitle = \"Safety criteria for design-basis accidents (25 rem TEDE at the exclusion area boundary and low-population zone)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.210 Safety criteria for design-basis accidents\" }, { document = \"10cfr53\", section = \"\u{a7} 53.530 Population-related considerations, (a)\" }, { document = \"10cfr50\", section = \"\u{a7} 50.34 Contents of applications; technical information, (a)(1)(ii)(D)\" }, { document = \"10cfr52\", section = \"\u{a7} 52.47 Contents of applications; technical information, (a)(2)(iv)\" }]\ncross_links = [\"13-environmental-protection/postulated-accident-impacts/design-basis-accident-consequences\", \"02-nuclear-safety/source-terms/accident-source-terms\", \"12-site-and-supporting-facilities/site-characteristics/geography-and-demography\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/licensing-basis-events/safety-criteria-for-licensing-basis-events-other-than-dbas\"\ntitle = \"Safety criteria for licensing-basis events other than design-basis accidents (comprehensive risk metrics)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.220 Safety criteria for licensing-basis events other than design-basis accidents\" }, { document = \"10cfr53\", section = \"\u{a7} 53.450 Analysis requirements, (e)\" }]\ncross_links = [\"02-nuclear-safety/severe-accidents/probabilistic-risk-assessment\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/defense-in-depth\"\ntitle = \"Defense in depth\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.250 Defense in depth\" }, { document = \"10cfr53\", section = \"\u{a7} 53.020 Definitions (Defense in depth)\" }, { document = \"10cfr53\", section = \"\u{a7} 53.450 Analysis requirements, (b)(3)\" }]\ncross_links = [\"02-nuclear-safety/design-of-structures-systems-and-components/safety-functions\", \"02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel\", \"02-nuclear-safety/containment/functional-containment\", \"02-nuclear-safety/severe-accidents\", \"02-nuclear-safety/severe-accidents/probabilistic-risk-assessment\", \"02-nuclear-safety/containment/functional-containment\"]\nnote = \"Placement is a judgement call: Part 53 ties defense in depth to the uncertainties in the analysis of licensing-basis events other than DBAs (\u{a7} 53.250(a)-(c)), hence accident-analysis; it is cross-cutting and could equally sit under design-of-structures-systems-and-components.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/accident-analysis/transient-and-accident-analysis-methods/qualification-of-analytical-codes\"\ntitle = \"Qualification of analytical codes\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.450 Analysis requirements, (d) Qualification of analytical codes\" }, { document = \"10cfr50\", section = \"\u{a7} 50.43 Additional standards and provisions affecting class 103 licenses and certifications for commercial power, (e)(1)(iii)\" }]\ncross_links = [\"07-regulatory-framework/quality-assurance/software-quality-assurance/verification-and-validation-records\", \"02-nuclear-safety/engineered-safety-features/emergency-core-cooling\"]\nnote = \"\u{a7} 53.450(d) names thermodynamics, reactor physics, fuel performance and mechanistic source term codes: the regulatory home of outram-park\'s V&V work.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/safety-functions\"\ntitle = \"Safety functions (primary: limiting the release of radioactive materials; additional: reactivity, heat generation, heat removal, chemical interactions)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.230 Safety functions\" }, { document = \"10cfr53\", section = \"\u{a7} 53.400 Design features for licensing-basis events\" }]\ncross_links = [\"02-nuclear-safety/containment/functional-containment\", \"02-nuclear-safety/accident-analysis/licensing-basis-events\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/safety-functions/functional-design-criteria\"\ntitle = \"Functional design criteria\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.410 Functional design criteria for design-basis accidents\" }, { document = \"10cfr53\", section = \"\u{a7} 53.420 Functional design criteria for licensing-basis events other than design-basis accidents\" }, { document = \"10cfr53\", section = \"\u{a7} 53.425 Design features and functional design criteria for normal operations\" }, { document = \"10cfr53\", section = \"\u{a7} 53.430 Design features and functional design criteria for protection of plant workers\" }, { document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (a)\" }]\ncross_links = [\"07-regulatory-framework/reactor-licensing/general-design-criteria\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/safety-categorization-and-special-treatments\"\ntitle = \"Safety categorization and special treatments (safety-related; non-safety-related but safety-significant; non-safety-significant)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.460 Safety categorization and special treatments\" }, { document = \"10cfr50\", section = \"\u{a7} 50.69 Risk-informed categorization and treatment of structures, systems and components for nuclear power reactors\" }]\ncross_links = [\"07-regulatory-framework/quality-assurance\", \"02-nuclear-safety/severe-accidents/probabilistic-risk-assessment\", \"07-regulatory-framework/quality-assurance/quality-assurance-program\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/codes-and-standards\"\ntitle = \"Codes and standards\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.55a Codes and standards\" }, { document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (b)\" }]\ncross_links = [\"02-nuclear-safety/design-of-structures-systems-and-components/mechanical-systems-and-components\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/inservice-inspection-and-inservice-testing\"\ntitle = \"Inservice inspection and inservice testing\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.880 Inservice inspection and inservice testing\" }, { document = \"10cfr50\", section = \"\u{a7} 50.55a Codes and standards, (f) Preservice and inservice testing requirements; (g) Preservice and inservice inspection requirements\" }]\ncross_links = [\"02-nuclear-safety/msr-coolant-loop/coolant-loop-drain-tank-and-isolation\", \"02-nuclear-safety/fuel-system-design/testing-inspection-and-surveillance\", \"02-nuclear-safety/design-of-structures-systems-and-components/codes-and-standards\", \"07-regulatory-framework/quality-assurance/maintenance-rule\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/integrity-assessment-programs\"\ntitle = \"Integrity assessment programs (plant aging, cyclic or transient load limits, degradation mechanisms)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.870 Integrity assessment programs\" }, { document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (d)\" }]\ncross_links = [\"02-nuclear-safety/reactor-coolant-system/reactor-vessel-integrity\", \"07-regulatory-framework/quality-assurance/maintenance-rule\", \"02-nuclear-safety/engineered-safety-features/esf-materials\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/missile-protection/aircraft-impact-assessment\"\ntitle = \"Aircraft impact assessment\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.150 Aircraft impact assessment\" }, { document = \"10cfr52\", section = \"\u{a7} 52.47 Contents of applications; technical information, (a)(28)\" }, { document = \"10cfr52\", section = \"\u{a7} 52.79 Contents of applications; technical information in final safety analysis report, (a)(47)\" }, { document = \"10cfr53\", section = \"\u{a7} 53.910 Procedures and guidelines, (b)(7) (potential aircraft threat)\" }]\ncross_links = [\"02-nuclear-safety/design-of-structures-systems-and-components/missile-protection\", \"02-nuclear-safety/severe-accidents/severe-accident-evaluation\", \"15-nuclear-security/physical-protection\"]\nnote = \"Part 53 has no design-specific aircraft impact assessment of its own; it only requires procedures for a notified aircraft threat (\u{a7} 53.910(b)(7)). \u{a7} 50.150 is a beyond-design-basis assessment, distinct from SRP 3.5.1.6 aircraft hazards already cited under missile-protection.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/severe-accidents/mitigation-of-beyond-design-basis-events\"\ntitle = \"Mitigation of beyond-design-basis events (mitigation strategies; extensive damage mitigation guidelines)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.155 Mitigation of beyond-design-basis events\" }]\ncross_links = [\"02-nuclear-safety/severe-accidents/severe-accident-evaluation\", \"09-electrical-grid/electric-power/station-blackout\", \"02-nuclear-safety/auxiliary-systems/cooling-water-and-ultimate-heat-sink\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/initial-test-program/initial-plant-test-program/safety-feature-testing-and-prototype-plants\"\ntitle = \"Demonstration of safety-feature performance for innovative designs; prototype plant testing\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.43 Additional standards and provisions affecting class 103 licenses and certifications for commercial power, (e)\" }, { document = \"10cfr52\", section = \"\u{a7} 52.1 Definitions (Prototype plant)\" }, { document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (a)(1)\" }]\ncross_links = [\"02-nuclear-safety/thermal-hydraulic-design/experimental-loops-and-test-reactor-support\", \"02-nuclear-safety/accident-analysis/transient-and-accident-analysis-methods/qualification-of-analytical-codes\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/reactor-coolant-system/reactor-coolant-system-venting\"\ntitle = \"Reactor coolant system venting systems\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.46a Acceptance criteria for reactor coolant system venting systems\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/reactor-coolant-system/reactor-vessel-integrity/pressurized-thermal-shock\"\ntitle = \"Fracture toughness requirements for protection against pressurized thermal shock events\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.61 Fracture toughness requirements for protection against pressurized thermal shock events\" }, { document = \"10cfr50\", section = \"\u{a7} 50.61a Alternate fracture toughness requirements for protection against pressurized thermal shock events\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/conduct-of-operations/self-reliant-mitigation-facilities\"\ntitle = \"Self-reliant-mitigation facilities and generally licensed reactor operators\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.725 General staffing, training, personnel qualifications, and human factors requirements, (a)\" }, { document = \"10cfr53\", section = \"\u{a7} 53.800 Facility licensees for self-reliant-mitigation facilities\" }, { document = \"10cfr53\", section = \"\u{a7} 53.805 Facility licensee requirements related to generally licensed reactor operators\" }, { document = \"10cfr53\", section = \"\u{a7} 53.810 Generally licensed reactor operators\" }, { document = \"10cfr53\", section = \"\u{a7} 53.815 Generally licensed reactor operator training, examination, and proficiency programs\" }]\ncross_links = [\"10-human-resource-development\", \"02-nuclear-safety/human-factors-engineering\", \"02-nuclear-safety/conduct-of-operations/operator-training-and-requalification\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/conduct-of-operations/operator-training-and-requalification/simulation-facilities\"\ntitle = \"Simulation facilities (scope, fidelity and performance testing)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.780 Training, examination, and proficiency program, (e) Simulation facilities\" }, { document = \"10cfr53\", section = \"\u{a7} 53.815 Generally licensed reactor operator training, examination, and proficiency programs, (e) Simulation facilities\" }, { document = \"10cfr53\", section = \"\u{a7} 53.725 General staffing, training, personnel qualifications, and human factors requirements, (c) (Simulation facility; Performance testing; Reference plant)\" }]\ncross_links = [\"10-human-resource-development\", \"10-human-resource-development/knowledge-management-and-education/education-and-outreach\"]\nnote = \"outram-park\'s egui simulators are offline educational demonstrations (RESPONSIBLE_USE.md), not simulation facilities in the \u{a7} 53.780(e) sense; any link from them should be an \'aspiration\', never a home.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/conduct-of-operations/staffing-plan\"\ntitle = \"Staffing plan (on-shift staffing and engineering expertise)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.730 Defining, fulfilling, and maintaining the role of personnel in ensuring safe operations, (f) Staffing plan\" }, { document = \"10cfr53\", section = \"\u{a7} 53.740 Facility licensee requirements\u{2014}general, (b)\" }, { document = \"10cfr50\", section = \"\u{a7} 50.54 Conditions of licenses, (m)\" }]\ncross_links = [\"02-nuclear-safety/human-factors-engineering/concept-of-operations-and-function-allocation\", \"10-human-resource-development\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/conduct-of-operations/operating-experience\"\ntitle = \"Operating experience program\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.730 Defining, fulfilling, and maintaining the role of personnel in ensuring safe operations, (e) Operating experience\" }, { document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (a)(2)\" }, { document = \"10cfr52\", section = \"\u{a7} 52.47 Contents of applications; technical information, (a)(22)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/conduct-of-operations/event-notification-and-reporting\"\ntitle = \"Immediate notification requirements and the licensee event report system\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.72 Immediate notification requirements for operating nuclear power reactors\" }, { document = \"10cfr50\", section = \"\u{a7} 50.73 Licensee event report system\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1630 Immediate notification requirements for operating commercial nuclear plants\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1640 Licensee event report system\" }]\ncross_links = [\"07-regulatory-framework\", \"14-emergency-planning/notification-methods-and-procedures\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/human-factors-engineering/concept-of-operations-and-function-allocation\"\ntitle = \"Concept of operations; functional requirements analysis and function allocation\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.730 Defining, fulfilling, and maintaining the role of personnel in ensuring safe operations, (c) Concept of operations; (d) Functional requirements analysis and function allocation\" }, { document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (n)(3)-(4)\" }]\ncross_links = [\"02-nuclear-safety/design-of-structures-systems-and-components/safety-functions\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"04-funding-and-financing/financial-qualifications/financial-protection-and-accident-insurance\"\ntitle = \"Financial protection; insurance required to stabilize and decontaminate plant following an accident\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.1710 Financial protection\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1720 Insurance required to stabilize and decontaminate plant following an accident\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1730 Financial protection requirements\" }, { document = \"10cfr50\", section = \"\u{a7} 50.54 Conditions of licenses, (w)\" }]\ncross_links = [\"05-legal-framework\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/reactor-licensing/manufacturing-licenses\"\ntitle = \"Manufacturing licenses (manufactured reactors installed at sites not identified in the application)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr52\", section = \"\u{a7} 52.151 Scope of subpart (Subpart F, Manufacturing Licenses)\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1270 Manufacturing licenses\" }, { document = \"10cfr53\", section = \"\u{a7} 53.620 Manufacturing\" }]\ncross_links = [\"07-regulatory-framework/reactor-licensing/one-step-licensing\", \"18-industrial-involvement\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/reactor-licensing/standard-design-approvals\"\ntitle = \"Standard design approvals\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr52\", section = \"\u{a7} 52.131 Scope of subpart (Subpart E, Standard Design Approvals)\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1200 Standard design approvals\" }]\ncross_links = [\"07-regulatory-framework/reactor-licensing/one-step-licensing\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/reactor-licensing/standardization-at-multiple-sites\"\ntitle = \"Standardization of nuclear power plant designs: reactors of identical design at multiple sites\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"Appendix N, Standardization of Nuclear Power Plant Designs: Permits To Construct and Licenses To Operate Nuclear Power Reactors of Identical Design at Multiple Sites\" }, { document = \"10cfr52\", section = \"Appendix N, Standardization of Nuclear Power Plant Designs: Combined Licenses To Construct and Operate Nuclear Power Reactors of Identical Design at Multiple Sites\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1470 Standardization of commercial nuclear plant designs: licenses to construct and operate nuclear power reactors of identical design at multiple sites\" }]\ncross_links = [\"07-regulatory-framework/reactor-licensing/one-step-licensing\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/reactor-licensing/limited-work-authorization\"\ntitle = \"Limited work authorization\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.10 License required; limited work authorization\" }, { document = \"10cfr52\", section = \"\u{a7} 52.91 Authorization to conduct limited work authorization activities\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1130 Limited work authorizations\" }]\ncross_links = [\"07-regulatory-framework/reactor-licensing/one-step-licensing\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/reactor-licensing/changes-tests-and-experiments\"\ntitle = \"Changes, tests, and experiments (evaluating changes to the facility as described in the Final Safety Analysis Report)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.59 Changes, tests, and experiments\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1540 Updating licensing-basis information and determining the need for NRC approval\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1550 Evaluating changes to facility as described in Final Safety Analysis Reports\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/reactor-licensing/backfitting\"\ntitle = \"Backfitting\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.109 Backfitting\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1590 Backfitting\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"08-radiation-protection/radiation-protection/protection-of-plant-workers\"\ntitle = \"Protection of plant workers (occupational dose under 10 CFR part 20)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.270 Protection of plant workers\" }, { document = \"10cfr53\", section = \"\u{a7} 53.430 Design features and functional design criteria for protection of plant workers\" }]\ncross_links = [\"08-radiation-protection/radiation-protection/protection-of-plant-workers/alara\"]\nnote = \"Close to the existing ALARA concept (occupational exposure); kept separate because Part 53 makes it a top-level safety requirement (Subpart B) with its own functional design criteria. Merging into alara as extra sources is the alternative.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"14-emergency-planning/emergency-plan-development-and-review/performance-based-emergency-preparedness\"\ntitle = \"Emergency preparedness for small modular reactors, non-light-water reactors, and non-power production or utilization facilities (performance-based framework)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.160 Emergency preparedness for small modular reactors, non-light-water reactors, and non-power production or utilization facilities\" }, { document = \"10cfr53\", section = \"\u{a7} 53.855 Emergency preparedness, (a)\" }]\ncross_links = [\"14-emergency-planning/exercises-and-drills\", \"14-emergency-planning/accident-assessment\"]\nnote = \"Placed under planning standard P (planning effort) because \u{a7} 50.160 is an alternative to the whole \u{a7} 50.47(b)/Appendix E plan; its performance objectives are demonstrated by drills, hence the cross-link to exercises-and-drills.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"14-emergency-planning/protective-response/plume-exposure-pathway-epz-size\"\ntitle = \"Plume exposure pathway emergency planning zone (EPZ) size (dose-based: 1 rem TEDE over 96 hours)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.33 Contents of applications; general information, (g)(1)-(2)\" }, { document = \"10cfr50\", section = \"\u{a7} 50.160 Emergency preparedness for small modular reactors, non-light-water reactors, and non-power production or utilization facilities, (b)(3) Emergency planning zone\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1109 Contents of applications; general information, (g)\" }]\ncross_links = [\"14-emergency-planning/accident-assessment/dose-projection\", \"13-environmental-protection/meteorology-and-air-quality/short-term-accident-dispersion\"]\nnote = \"EPZ sizing under \u{a7} 50.33(g)(2)(i) is a consequence calculation (accident likelihood, source term, timing, meteorology). Per #724 the dispersion crates may link here only as \'aspiration\'.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"15-nuclear-security/physical-protection/access-authorization-and-fitness-for-duty\"\ntitle = \"Access authorization and fitness-for-duty programs\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.860 Security programs, (b) Fitness-for-duty; (c) Access authorization\" }, { document = \"10cfr52\", section = \"\u{a7} 52.79 Contents of applications; technical information in final safety analysis report, (a)(44)\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"15-nuclear-security/physical-protection/safety-and-security-in-design\"\ntitle = \"Safety and security considered together in the design process\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.440 Design requirements, (f)\" }]\ncross_links = [\"02-nuclear-safety/accident-analysis/defense-in-depth\", \"02-nuclear-safety/design-of-structures-systems-and-components/safety-functions\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"17-radioactive-waste-management/decommissioning/decommissioning-financial-assurance\"\ntitle = \"Financial assurance and cost estimates for decommissioning\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.75 Reporting and recordkeeping for decommissioning planning\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1010 Financial assurance for decommissioning\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1020 Cost estimates for decommissioning\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1030 Annual adjustments to cost estimates for decommissioning\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1040 Methods for providing financial assurance for decommissioning\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1045 Limitations on the use of decommissioning trust funds\" }]\ncross_links = [\"04-funding-and-financing/financial-qualifications\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"15-nuclear-security/cybersecurity/information-security\"\ntitle = \"Information security\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.860 Security programs, (e) Information security\" }]\nstatus = \"approved\"\n\n[[concept]]\npath = \"15-nuclear-security/cybersecurity/cybersecurity-program\"\ntitle = \"Cybersecurity program\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.860 Security programs, (d) Cybersecurity\" }, { document = \"10cfr50\", section = \"\u{a7} 50.34 Contents of applications; technical information, (c)(2)\" }]\ncross_links = [\"02-nuclear-safety/instrumentation-and-control/digital-i-and-c-software\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"07-regulatory-framework/reactor-licensing/license-renewal\"\ntitle = \"License renewal\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.135 Renewal of non-power production or utilization facility licenses issued under \u{a7} 50.22\" }, { document = \"10cfr52\", section = \"\u{a7} 52.29-52.33, 52.57-52.61, 52.107, 52.177-52.181 (application, criteria and duration of renewal)\" }, { document = \"10cfr53\", section = \"\u{a7} 53.1173-53.1179, 53.1254-53.1260, 53.1295, 53.1402, 53.1458 (renewal)\" }]\ncross_links = [\"02-nuclear-safety/design-of-structures-systems-and-components/integrity-assessment-programs\"]\nnote = \"Power-reactor operating-license renewal is 10 CFR Part 54 (not in the corpus yet); the integrity assessment programs of \u{a7} 53.870 cover plant aging.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"10-human-resource-development/knowledge-management-and-education/nuclear-history-and-lessons-learned\"\ntitle = \"Nuclear history and lessons learned (TMI, Chernobyl, Fukushima)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr50\", section = \"\u{a7} 50.34 Contents of applications; technical information, (f) Additional TMI-related requirements\" }, { document = \"wash-1400\", section = \"Executive Summary and Main Report\" }]\ncross_links = [\"02-nuclear-safety/conduct-of-operations/operating-experience\", \"02-nuclear-safety/severe-accidents\"]\nnote = \"Maintainer 2026-10-06: TMI fits here. Chernobyl and Fukushima await literature.\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/nuclear-design/core-configurations/light-water-cooled\"\ntitle = \"Light-water-cooled reactor cores (PWR, BWR, light-water SMRs)\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"Ch. 4 Reactor; Ch. 5 Reactor Coolant System and Connected Systems (the LWR review plan)\" }, { document = \"10cfr50\", section = \"Appendix A, General Design Criteria for Nuclear Power Plants (water-cooled)\" }]\nnote = \"Added 2026-10-06 for the maintainer\'s corpus re-filing (6 papers: WASH-1400, NUREG-1465, RG 1.183, NuScale SER ch. 6 and ER, NUREG/KM-0004).\"\nstatus = \"approved\"\n\n[[concept]]\npath = \"02-nuclear-safety/design-of-structures-systems-and-components/structural-materials\"\ntitle = \"Structural materials\"\norigin = \"nrc\"\nsources = [{ document = \"nureg-0800-toc-rev6\", section = \"4.5.1 Control Rod Drive Structural Materials; 4.5.2 Reactor Internal and Core Support Structure Materials; 5.2.3 Reactor Coolant Pressure Boundary Materials; 5.3.1 Reactor Vessel Materials; 6.1.1 Engineered Safety Features Materials\" }]\ncross_links = [\"02-nuclear-safety/control-rods-and-drives/control-rod-drive-structural-materials\", \"02-nuclear-safety/reactor-coolant-system/reactor-vessel-integrity\", \"02-nuclear-safety/engineered-safety-features/esf-materials\", \"02-nuclear-safety/msr-coolant-loop/coolant-loop-materials-and-chemistry\", \"02-nuclear-safety/moderator-and-reflector\"]\nstatus = \"approved\"\n\n[[concept]]\npath = \"08-radiation-protection/radiation-protection/dose-limits-and-criteria\"\ntitle = \"Dose limits and criteria (workers, public, design-basis accidents)\"\norigin = \"nrc\"\nsources = [{ document = \"10cfr53\", section = \"\u{a7} 53.210 Safety criteria for design-basis accidents (25 rem total effective dose equivalent); \u{a7} 53.020 Definitions (total effective dose equivalent); \u{a7} 53.270 Protection of plant workers (10 CFR part 20)\" }, { document = \"nureg-1555\", section = \"5.4.2 Radiation Doses to Members of the Public\" }]\ncross_links = [\"02-nuclear-safety/accident-analysis/licensing-basis-events/safety-criteria-for-design-basis-accidents\", \"13-environmental-protection/radiological-impacts-of-normal-operation/doses-to-members-of-the-public\", \"08-radiation-protection/radiation-protection/protection-of-plant-workers\"]\nnote = \"10 CFR part 20 (standards for protection against radiation: occupational and public dose limits) is the primary rule and is not in the corpus yet.\"\nstatus = \"approved\"\n";
```

## Module `zotero`

The parts of the Zotero port that live in kovan-literature (epic #747).
The data model itself is `kovan_common::zotero` (#748).

| Module | What |
|---|---|
| [`local_library`] | read a Zotero data folder (`zotero.sqlite` + `storage/`) and import it into kovan (#750). Native desktop only: not compiled for wasm32 or Android. |

```rust
pub mod zotero { /* ... */ }
```

### Modules

## Module `local_library`

**Attributes:**

- `Other("#[attr = CfgTrace([Not(Any([NameValue { name: \"target_arch\", value: Some(\"wasm32\"), span: crates/kovan-literature/src/zotero/mod.rs:11:15: 11:37 (#0) }, NameValue { name: \"target_os\", value: Some(\"android\"), span: crates/kovan-literature/src/zotero/mod.rs:11:39: 11:60 (#0) }], crates/kovan-literature/src/zotero/mod.rs:11:14: 11:61 (#0)), crates/kovan-literature/src/zotero/mod.rs:11:10: 11:62 (#0))])]")`

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
file in the data folder at all.** It copies `zotero.sqlite` and, when they
exist, its `-journal` and `-wal` files into a private temporary directory
and opens the copy, which is how Zotero itself reads a database it does
not own (db.js:1871-1885 copies the database and its WAL to a temporary
file before touching them). SQLite replays the copied WAL or rolls back the
copied journal **in the private copy**; the connection is then set
`PRAGMA query_only=ON`, and `PRAGMA quick_check` must report `ok`. If the
copy fails that check (Zotero was mid-write while the bytes were copied),
the reader falls back to `zotero.sqlite.bak`, Zotero's own periodic backup
(db.js:1358, :2472), when [`ReadOptions::fall_back_to_backup`] is set
(the default), and records which file it read in
[`ZoteroDataFolder::source`]. [`DbSource::Backup`] reads the backup
directly. The temporary copy is deleted when reading ends. Nothing is
ever written to the data folder.

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
from upstream's own schema files (`tests/zotero_local_library.rs`); not yet
run on a real library by a human.

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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ImportOptions { /* ... */ }
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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ImportedDocument { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Losses { /* ... */ }
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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ZoteroImport { /* ... */ }
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
    fn default() -> ZoteroImport { /* ... */ }
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

A private copy of `zotero.sqlite` (with its `-journal`/`-wal`).

###### `Backup`

A private copy of `zotero.sqlite.bak`, Zotero's own backup.

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
    fn clone(self: &Self) -> DbSource { /* ... */ }
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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ReadOptions { /* ... */ }
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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> SchemaVersions { /* ... */ }
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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> LibraryKind { /* ... */ }
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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> AttachmentFile { /* ... */ }
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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> LocalLibrary { /* ... */ }
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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ReadIssue { /* ... */ }
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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ReadReport { /* ... */ }
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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ZoteroDataFolder { /* ... */ }
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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ZoteroDbError { /* ... */ }
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
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(e: rusqlite::Error) -> Self { /* ... */ }
    ```

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
### Functions

#### Function `read_data_folder`

Read a Zotero data folder: the directory holding `zotero.sqlite` and
`storage/` (Zotero's "Data Directory Location"). Read-only; see the
module docs for how the live database is left untouched.

```rust
pub fn read_data_folder(data_dir: &std::path::Path, opts: &ReadOptions) -> Result<ZoteroDataFolder, ZoteroDbError> { /* ... */ }
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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> LiteratureError { /* ... */ }
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

