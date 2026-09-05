# Crate Documentation

**Version:** 0.0.0

**Format Version:** 61

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
committed *and* published to crates.io; open PDFs and generated open Markdown
are committed but **not** published (licence scope and size); everything
proprietary is neither.

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
`[crate::digitiser]`, now `kovan::digitiser`; binaries `kovan-digitise`,
`kovan-digitise-tui`, `kovan-gui`) — see that crate's `NOTICE`. It moved
so it can depend on `kopitiam-pdf` (AGPL-3.0-only, GitHub issue #30's
PDF-native digitising) without pulling this crate — used well beyond the
GUI — into that relicense. This crate stays GPL-3.0-only and carries no
digitiser code, no `image`/`eframe`/`egui`/`ratatui` dependency, and no
`digitise-*` feature.

## Modules

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

